"""Portable packaging-contract tests; legacy manual Arch tooling is retained outside CI/release."""
from __future__ import annotations

import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import tomllib
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parent.parent


def load(name: str):
    spec = importlib.util.spec_from_file_location(name.replace("-", "_"), ROOT / "scripts" / f"{name}.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


deps = load("arch-dependencies")


class DependencyMetadataTests(unittest.TestCase):
    def test_runtime_build_check_and_arch_dependencies_are_combined(self):
        metadata = """pkgbase = liusheng
\tdepends = qt6-base>=6.8
\tdepends = qt6-svg
\tdepends_x86_64 = qt6-wayland
\tdepends_aarch64 = other-backend
\tmakedepends = rust
\tmakedepends_x86_64 = clang
\tcheckdepends = python
\toptdepends = unrelated: optional tool
pkgname = liusheng
\tdepends = qt6-svg
"""
        all_deps, runtime = deps.parse_dependencies(metadata)
        self.assertEqual(runtime, ["qt6-base>=6.8", "qt6-svg", "qt6-wayland"])
        self.assertEqual(all_deps, ["clang", "python", "qt6-base>=6.8", "qt6-svg", "qt6-wayland", "rust"])

    def test_missing_runtime_metadata_fails_closed(self):
        with self.assertRaises(ValueError):
            deps.parse_dependencies("makedepends = rust\n")

    def test_dependency_options_and_whitespace_are_rejected(self):
        for item in ("--nodeps", "qt6-svg --nodeps", "$(command)", "qt6-svg;command"):
            with self.subTest(item=item), self.assertRaises(ValueError):
                deps.parse_dependencies(f"depends = {item}\n")

    def test_metadata_generation_requires_ordinary_builder(self):
        with patch.object(deps.os, "geteuid", return_value=0), self.assertRaises(ValueError):
            deps.generate(ROOT, Path("unused"))

    def test_new_manifest_dependencies_flow_into_install_and_constraint_files(self):
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary) / "metadata"
            metadata = "depends = qt6-svg>=6.8\ndepends = qt6-wayland\nmakedepends = rust\n"
            def makepkg(command, **kwargs):
                self.assertEqual(command, ["makepkg", "--printsrcinfo"])
                source = (Path(kwargs["cwd"]) / "PKGBUILD").read_text()
                self.assertNotIn("@VERSION@", source)
                self.assertNotIn("@SHA256@", source)
                return subprocess.CompletedProcess(command, 0, metadata, "")
            with patch.object(deps.os, "geteuid", return_value=1000), patch.object(deps.subprocess, "run", side_effect=makepkg):
                deps.generate(ROOT, output)
            self.assertEqual((output / "packages.txt").read_text(), "qt6-svg\nqt6-wayland\nrust\n")
            self.assertIn("qt6-svg>=6.8", (output / "dependencies.txt").read_text())
            self.assertEqual((output / ".SRCINFO").read_text(), metadata)

    def test_makepkg_failure_propagates_without_partial_metadata(self):
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary) / "metadata"
            with patch.object(deps.os, "geteuid", return_value=1000), patch.object(deps.subprocess, "run", side_effect=subprocess.CalledProcessError(8, ["makepkg"], stderr="dependency error")):
                with self.assertRaises(subprocess.CalledProcessError):
                    deps.generate(ROOT, output)
            self.assertFalse(output.exists())


class ArchBuildContractTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix="arch-template-")
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.srcdir = self.root / "sources with spaces"
        self.project = self.srcdir / "Liusheng-9.8.7"
        (self.project / "scripts").mkdir(parents=True)
        self.tools = self.root / "tools"
        self.tools.mkdir()
        cargo = self.tools / "cargo"
        cargo.write_text(f'''#!{sys.executable}
import json,os,pathlib,sys
args=sys.argv[1:]
pathlib.Path(os.environ['TRACE_CARGO']).write_text(json.dumps(args))
if os.environ.get('BUILD_FAILURE'): sys.exit(101)
root=pathlib.Path(args[args.index('--target-dir')+1])
(root/'release').mkdir(parents=True,exist_ok=True)
(root/'release/liusheng').write_text('fresh build')
''')
        cargo.chmod(0o755)
        installer = self.project / "scripts/install.sh"
        installer.write_text('''#!/usr/bin/env bash
set -euo pipefail
[[ "$1" == --no-build ]]
[[ "$PREFIX" == /usr ]]
[[ "$(cat "$CARGO_TARGET_DIR/release/liusheng")" == 'fresh build' ]]
printf '%s' "$CARGO_TARGET_DIR" > "$TRACE_INSTALL"
''')
        installer.chmod(0o644)
        self.pkgbuild = self.root / "PKGBUILD"
        self.pkgbuild.write_text((ROOT / "packaging/arch/PKGBUILD.in").read_text()
                                .replace("@VERSION@", "9.8.7").replace("@SHA256@", "0" * 64))
        self.env = {**os.environ, "PATH": f"{self.tools}:/usr/bin:/bin", "srcdir": str(self.srcdir),
                    "pkgdir": str(self.root / "package"), "CARGO_TARGET_DIR": str(self.root / "stale target"),
                    "TRACE_CARGO": str(self.root / "cargo.json"), "TRACE_INSTALL": str(self.root / "installed-target")}

    def invoke(self):
        return subprocess.run(["bash", "-c", 'set -euo pipefail; source "$1"; (cd "$srcdir"; build); (cd "$srcdir"; package)', "test", str(self.pkgbuild)],
                              env=self.env, text=True, capture_output=True, timeout=10)

    def test_build_and_install_share_isolated_target_and_support_0644_installer(self):
        result = self.invoke()
        self.assertEqual(result.returncode, 0, result.stderr)
        target = str(self.project / "target")
        arguments = json.loads(Path(self.env["TRACE_CARGO"]).read_text())
        self.assertEqual(arguments[arguments.index("--target-dir")+1], target)
        self.assertEqual(Path(self.env["TRACE_INSTALL"]).read_text(), target)
        self.assertFalse((self.root / "stale target").exists())

    def test_failed_compilation_stops_before_install(self):
        self.env["BUILD_FAILURE"] = "1"
        result = self.invoke()
        self.assertEqual(result.returncode, 101, result.stderr)
        self.assertFalse(Path(self.env["TRACE_INSTALL"]).exists())

    def test_template_retains_svg_wayland_and_dependency_checks(self):
        result = subprocess.run(["bash", "-c", 'source "$1"; printf "%s\\n" "${depends[@]}"', "test", str(self.pkgbuild)],
                                text=True, capture_output=True, check=True)
        self.assertIn("qt6-svg", result.stdout.splitlines())
        self.assertIn("qt6-wayland", result.stdout.splitlines())
        command = (ROOT / "scripts/package.sh").read_text()
        self.assertIn("makepkg --cleanbuild --noconfirm", command)
        self.assertNotIn("--nodeps", command)
        self.assertNotIn("--skipinteg", command)

    def test_crate_and_lock_versions_are_consistent(self):
        version = tomllib.loads((ROOT / "crates/liusheng/Cargo.toml").read_text())["package"]["version"]
        other = tomllib.loads((ROOT / "crates/liusheng-core/Cargo.toml").read_text())["package"]["version"]
        self.assertEqual(version, other)
        locked = tomllib.loads((ROOT / "Cargo.lock").read_text())
        self.assertEqual({p["version"] for p in locked["package"] if p["name"] in {"liusheng", "liusheng-core"}}, {version})


if __name__ == "__main__":
    unittest.main()
