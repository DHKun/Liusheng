"""Portable tests of macOS packaging shell orchestration using explicit tool doubles.

These tests verify path/version/error handling, not native linking or codesigning.
All output is confined to a temporary directory; the real macOS CI handles binaries.
Run: python3 -m unittest discover -s tests -p 'test_macos_package.py' -v
"""
from __future__ import annotations

import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest
import zipfile

ROOT = Path(__file__).resolve().parent.parent


class MacosPackageTests(unittest.TestCase):
    def setUp(self) -> None:
        temporary = tempfile.TemporaryDirectory(prefix="liusheng-macos-package-test-")
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.project = self.root / "checkout with spaces"
        for name in ("scripts/package-macos.sh", "crates/liusheng/Cargo.toml",
                     "packaging/macos/Info.plist.in", "crates/liusheng/qml/assets/app-icon/Liusheng.icns"):
            destination = self.project / name
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(ROOT / name, destination)
        self.tools = self.root / "tools"
        self.tools.mkdir()
        self.stub("uname", '#!/bin/sh\ncase "$1" in -s) echo Darwin;; -m) echo arm64;; *) exit 1;; esac\n')
        self.stub("cargo", f'''#!{sys.executable}
import json, os, pathlib, sys
arguments = sys.argv[1:]
pathlib.Path(os.environ['MOCK_ARGS']).write_text(json.dumps(arguments))
if os.environ.get('MOCK_BUILD_FAIL'): sys.exit(101)
out = pathlib.Path(arguments[arguments.index('--target-dir') + 1]) / 'release/liusheng'
out.parent.mkdir(parents=True, exist_ok=True)
out.write_bytes(b'compiled-in-requested-target')
out.chmod(0o755)
''')
        self.stub("macdeployqt", f'''#!{sys.executable}
import os, pathlib, sys
if os.environ.get('MOCK_DEPLOY_FAIL'): sys.exit(5)
contents = pathlib.Path(sys.argv[1]) / 'Contents'
for name, payload in [('PlugIns/platforms/libqcocoa.dylib', b'cocoa-only-fixture'),
                      ('Resources/qt.conf', b'[Paths]\\nPlugins = PlugIns\\nQmlImports = Resources/qml\\n')]:
    if os.environ.get('MOCK_DEPLOY_SKIP') == name: continue
    target = contents / name
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_bytes(b'' if os.environ.get('MOCK_DEPLOY_EMPTY') == name else payload)
''')
        self.stub("codesign", '#!/bin/sh\nexit 0\n')
        self.stub("ditto", f'''#!{sys.executable}
import pathlib, sys, zipfile
source, output = map(pathlib.Path, sys.argv[-2:])
with zipfile.ZipFile(output, 'w') as archive:
    for path in source.rglob('*'):
        if path.is_file(): archive.write(path, path.relative_to(source.parent))
''')
        self.stub("unzip", f'''#!{sys.executable}
import sys, zipfile
with zipfile.ZipFile(sys.argv[-1]) as archive:
    sys.exit(1 if archive.testzip() else 0)
''')
        (self.root / "home").mkdir()
        (self.root / "tmp").mkdir()
        self.env = {"PATH": f"{self.tools}:/usr/bin:/bin", "HOME": str(self.root / "home"),
                    "TMPDIR": str(self.root / "tmp"), "LANG": "C.UTF-8",
                    "MOCK_ARGS": str(self.root / "cargo-args.json")}
        self.output = self.root / "packages"

    def stub(self, name: str, content: str) -> None:
        path = self.tools / name
        path.write_text(content)
        path.chmod(0o755)

    def invoke(self, *args: str) -> subprocess.CompletedProcess[str]:
        return subprocess.run(["bash", "scripts/package-macos.sh", "--output", str(self.output), *args],
                              cwd=self.project, env=self.env, text=True, stdout=subprocess.PIPE,
                              stderr=subprocess.STDOUT, timeout=20)

    def verify_bundle(self, target: Path) -> None:
        result = self.invoke()
        self.assertEqual(result.returncode, 0, result.stdout)
        arguments = json.loads(Path(self.env["MOCK_ARGS"]).read_text())
        self.assertEqual(arguments[arguments.index("--target-dir") + 1], str(target))
        packages = list(self.output.glob("*.zip"))
        self.assertEqual(len(packages), 1)
        with zipfile.ZipFile(packages[0]) as archive:
            self.assertEqual(archive.read("Liusheng.app/Contents/MacOS/Liusheng"), b"compiled-in-requested-target")
            self.assertEqual(archive.read("Liusheng.app/Contents/Resources/Liusheng.icns"),
                             (self.project / "crates/liusheng/qml/assets/app-icon/Liusheng.icns").read_bytes())
            self.assertNotIn(b"@VERSION@", archive.read("Liusheng.app/Contents/Info.plist"))
            self.assertEqual(archive.read("Liusheng.app/Contents/PlugIns/platforms/libqcocoa.dylib"),
                             b"cocoa-only-fixture")
            self.assertIn(b"Plugins = PlugIns", archive.read("Liusheng.app/Contents/Resources/qt.conf"))
            self.assertFalse(any("offscreen" in name for name in archive.namelist()))
        self.assertEqual(list((self.root / "tmp").iterdir()), [])

    def test_default_target_is_explicit(self) -> None:
        self.verify_bundle(self.project / "target")

    def test_relative_custom_target_with_spaces(self) -> None:
        self.env["CARGO_TARGET_DIR"] = "build output"
        self.verify_bundle(self.project / "build output")

    def test_failed_build_keeps_stale_binary_out_of_package(self) -> None:
        stale = self.project / "target/release/liusheng"
        stale.parent.mkdir(parents=True)
        stale.write_bytes(b"stale-binary")
        stale.chmod(0o755)
        self.env["MOCK_BUILD_FAIL"] = "1"
        result = self.invoke()
        self.assertEqual(result.returncode, 101, result.stdout)
        self.assertFalse(self.output.exists())
        self.assertEqual(stale.read_bytes(), b"stale-binary")

    def test_deployment_resources_are_required_before_archiving(self) -> None:
        for resource in ("PlugIns/platforms/libqcocoa.dylib", "Resources/qt.conf"):
            with self.subTest(resource=resource):
                self.env["MOCK_DEPLOY_SKIP"] = resource
                result = self.invoke()
                self.assertEqual(result.returncode, 1, result.stdout)
                self.assertIn(resource, result.stdout)
                self.assertEqual(list(self.output.glob("*.zip")), [])
                self.assertEqual(list((self.root / "tmp").iterdir()), [])

    def test_empty_cocoa_plugin_is_rejected(self) -> None:
        self.env["MOCK_DEPLOY_EMPTY"] = "PlugIns/platforms/libqcocoa.dylib"
        result = self.invoke()
        self.assertEqual(result.returncode, 1, result.stdout)
        self.assertIn("libqcocoa.dylib", result.stdout)
        self.assertEqual(list(self.output.glob("*.zip")), [])

    def test_deploy_failure_preserves_previous_package(self) -> None:
        self.output.mkdir()
        previous = self.output / "previous.zip"
        previous.write_bytes(b"previous-release-fixture")
        self.env["MOCK_DEPLOY_FAIL"] = "1"
        result = self.invoke()
        self.assertEqual(result.returncode, 5, result.stdout)
        self.assertEqual(previous.read_bytes(), b"previous-release-fixture")
        self.assertEqual(list(self.output.iterdir()), [previous])

    def test_intel_target_is_rejected_before_compilation(self) -> None:
        self.stub("uname", '#!/bin/sh\ncase "$1" in -s) echo Darwin;; -m) echo x86_64;; *) exit 1;; esac\n')
        result = self.invoke()
        self.assertEqual(result.returncode, 1, result.stdout)
        self.assertIn("Apple Silicon arm64", result.stdout)
        self.assertFalse(Path(self.env["MOCK_ARGS"]).exists())
        self.assertFalse(self.output.exists())

    def test_version_mismatch_fails_before_build(self) -> None:
        result = self.invoke("--version", "999.0.0")
        self.assertEqual(result.returncode, 1, result.stdout)
        self.assertFalse(Path(self.env["MOCK_ARGS"]).exists())
        self.assertFalse(self.output.exists())


if __name__ == "__main__":
    unittest.main()
