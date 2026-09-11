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
        for name in ("scripts/package-macos.sh", "scripts/check-macos-bundle.sh", "scripts/check-desktop-smoke.py",
                     "Cargo.toml", "crates/liusheng/Cargo.toml",
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
        self.stub("codesign", '#!/bin/sh\nif [ "$1" = --verify ] && [ -n "${MOCK_VERIFY_FAIL:-}" ]; then exit 4; fi\nexit 0\n')
        self.stub("lipo", '#!/bin/sh\nprintf "%s\\n" "${MOCK_ARCH:-arm64}"\n')
        self.stub("python3", f'''#!{sys.executable}
import json, os, pathlib, sys
arguments = sys.argv[1:]
pathlib.Path(os.environ['MOCK_SMOKE_ARGS']).write_text(json.dumps(arguments))
assert pathlib.Path(arguments[1]).is_file()
output = pathlib.Path(arguments[arguments.index('--output') + 1])
output.write_text('portable fixture smoke called\\n')
sys.exit(int(os.environ.get('MOCK_SMOKE_FAIL', '0')))
''')
        self.stub("ditto", f'''#!{sys.executable}
import pathlib, sys, zipfile
source, output = map(pathlib.Path, sys.argv[-2:])
if '-x' in sys.argv:
    with zipfile.ZipFile(source) as archive:
        archive.extractall(output)
        for info in archive.infolist():
            path = output / info.filename
            if info.external_attr >> 16: path.chmod((info.external_attr >> 16) & 0o777)
    sys.exit(0)
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
                    "MOCK_ARGS": str(self.root / "cargo-args.json"),
                    "MOCK_SMOKE_ARGS": str(self.root / "smoke-args.json")}
        self.output = self.root / "packages"

    def stub(self, name: str, content: str) -> None:
        path = self.tools / name
        path.write_text(content)
        path.chmod(0o755)

    def invoke(self, *args: str, cwd: Path | None = None) -> subprocess.CompletedProcess[str]:
        return subprocess.run(["bash", str(self.project / "scripts/package-macos.sh"),
                               "--output", str(self.output), *args],
                              cwd=cwd or self.project, env=self.env, text=True, stdout=subprocess.PIPE,
                              stderr=subprocess.STDOUT, timeout=20)

    def verify_build_inputs(self, target: Path) -> None:
        arguments = json.loads(Path(self.env["MOCK_ARGS"]).read_text())
        for flag, expected in (("--target-dir", target),
                               ("--manifest-path", self.project / "Cargo.toml")):
            self.assertEqual(arguments.count(flag), 1)
            actual = Path(arguments[arguments.index(flag) + 1])
            self.assertTrue(actual.is_absolute(), f"{flag} must be absolute: {actual}")
            self.assertTrue(actual.exists(), f"{flag} must exist after the build: {actual}")
            # macOS /var and /private/var (and checkout/cache symlinks) may
            # name the same inode. Keep the directory identity check exact.
            self.assertTrue(actual.samefile(expected),
                            f"{flag} selects {actual.resolve()} instead of {expected.resolve()}")
        self.assertTrue(target.is_dir())
        self.assertIn("--locked", arguments)
        self.assertIn("--release", arguments)

    def verify_archive(self, target: Path, payload: bytes) -> None:
        packages = list(self.output.glob("*.zip"))
        self.assertEqual(len(packages), 1)
        with zipfile.ZipFile(packages[0]) as archive:
            bundled = archive.read("Liusheng.app/Contents/MacOS/Liusheng")
            self.assertEqual(bundled, payload)
            self.assertEqual(bundled, (target / "release/liusheng").read_bytes())
            self.assertEqual(archive.read("Liusheng.app/Contents/Resources/Liusheng.icns"),
                             (self.project / "crates/liusheng/qml/assets/app-icon/Liusheng.icns").read_bytes())
            self.assertNotIn(b"@VERSION@", archive.read("Liusheng.app/Contents/Info.plist"))
            self.assertEqual(archive.read("Liusheng.app/Contents/PlugIns/platforms/libqcocoa.dylib"),
                             b"cocoa-only-fixture")
            self.assertIn(b"Plugins = PlugIns", archive.read("Liusheng.app/Contents/Resources/qt.conf"))
            self.assertFalse(any("offscreen" in name for name in archive.namelist()))
        self.assertEqual(list((self.root / "tmp").iterdir()), [])

    def verify_bundle(self, target: Path, cwd: Path | None = None) -> None:
        result = self.invoke(cwd=cwd)
        self.assertEqual(result.returncode, 0, result.stdout)
        self.verify_build_inputs(target)
        self.verify_archive(target, b"compiled-in-requested-target")

    def linked_checkout(self) -> None:
        alias = self.root / "linked checkout"
        alias.symlink_to(self.project.resolve(strict=True), target_is_directory=True)
        self.project = alias

    def test_linked_checkout_default_target(self) -> None:
        self.linked_checkout()
        self.verify_bundle(self.project / "target")
        actual = json.loads(Path(self.env["MOCK_ARGS"]).read_text())
        self.assertNotEqual(actual[actual.index("--target-dir") + 1],
                            str(self.project / "target"))

    def test_linked_checkout_relative_target_and_temporary_directory(self) -> None:
        self.linked_checkout()
        alias = self.root / "linked temp with spaces"
        alias.symlink_to((self.root / "tmp").resolve(strict=True), target_is_directory=True)
        self.env.update(CARGO_TARGET_DIR="build output", TMPDIR=str(alias))
        self.verify_bundle(self.project / "build output")
        self.assertEqual(list(alias.iterdir()), [])

    def test_absolute_target_through_link_and_stale_default_binary(self) -> None:
        physical = self.root / "cache directory"
        physical.mkdir()
        alias = self.root / "cache link"
        alias.symlink_to(physical.resolve(strict=True), target_is_directory=True)
        target = alias / "new build"
        self.env["CARGO_TARGET_DIR"] = str(target)
        stale = self.project / "target/release/liusheng"
        stale.parent.mkdir(parents=True)
        stale.write_bytes(b"stale default binary")
        self.verify_bundle(target)
        self.assertEqual(stale.read_bytes(), b"stale default binary")

    def test_relative_target_is_based_on_callers_directory(self) -> None:
        caller = self.root / "separate caller"
        caller.mkdir()
        alias = self.root / "caller link"
        alias.symlink_to(caller.resolve(strict=True), target_is_directory=True)
        self.env["CARGO_TARGET_DIR"] = "build output"
        self.verify_bundle(alias / "build output", cwd=alias)
        self.assertFalse((self.project / "build output").exists())

    def test_symlinked_output_receives_archive_and_preserves_other_files(self) -> None:
        physical = self.root / "published packages"
        physical.mkdir()
        self.output.symlink_to(physical.resolve(strict=True), target_is_directory=True)
        keep = physical / "keep.txt"
        keep.write_text("unrelated fixture")
        self.verify_bundle(self.project / "target")
        self.assertEqual(keep.read_text(), "unrelated fixture")
        self.assertEqual(len(list(physical.glob("*.zip"))), 1)

    def test_no_build_uses_exact_custom_target_through_link(self) -> None:
        target = self.root / "prebuilt"
        binary = target / "release/liusheng"
        binary.parent.mkdir(parents=True)
        binary.write_bytes(b"chosen prebuilt binary")
        binary.chmod(0o755)
        alias = self.root / "prebuilt link"
        alias.symlink_to(target.resolve(strict=True), target_is_directory=True)
        self.env["CARGO_TARGET_DIR"] = str(alias)
        result = self.invoke("--no-build")
        self.assertEqual(result.returncode, 0, result.stdout)
        self.assertFalse(Path(self.env["MOCK_ARGS"]).exists())
        self.verify_archive(alias, b"chosen prebuilt binary")

    def test_path_assertion_rejects_wrong_or_relative_build_directory(self) -> None:
        target = self.project / "target"
        self.verify_bundle(target)
        path = Path(self.env["MOCK_ARGS"])
        original = json.loads(path.read_text())
        wrong = self.root / "other checkout/target"
        wrong.mkdir(parents=True)
        for supplied in (str(wrong), "target", str(self.root / "absent target")):
            with self.subTest(supplied=supplied):
                arguments = list(original)
                arguments[arguments.index("--target-dir") + 1] = supplied
                path.write_text(json.dumps(arguments))
                with self.assertRaises(AssertionError):
                    self.verify_build_inputs(target)

    def verify_relocated(self) -> subprocess.CompletedProcess[str]:
        package = next(self.output.glob("*.zip"))
        return subprocess.run(["bash", str(self.project / "scripts/check-macos-bundle.sh"),
                               str(package), str(self.root / "bundle logs")],
                              cwd=self.root, env=self.env, text=True, capture_output=True, timeout=20)

    def test_shared_verifier_handles_aliases_and_uses_native_smoke(self) -> None:
        self.linked_checkout()
        alias = self.root / "linked temp"
        alias.symlink_to((self.root / "tmp").resolve(strict=True), target_is_directory=True)
        self.env["TMPDIR"] = str(alias)
        self.verify_bundle(self.project / "target")
        package = next(self.output.glob("*.zip"))
        original = package.read_bytes()
        result = self.verify_relocated()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        arguments = json.loads(Path(self.env["MOCK_SMOKE_ARGS"]).read_text())
        self.assertTrue(Path(arguments[0]).samefile(self.project / "scripts/check-desktop-smoke.py"))
        self.assertEqual(arguments[arguments.index("--platform") + 1], "cocoa")
        self.assertIn("--debug-plugins", arguments)
        self.assertEqual(package.read_bytes(), original)
        self.assertIn("Architecture: arm64", (self.root / "bundle logs/bundle.log").read_text())
        self.assertTrue((self.root / "bundle logs/desktop.log").is_file())
        self.assertEqual(list(alias.iterdir()), [])

    def test_shared_verifier_preserves_failure_and_cleans_temporary_bundle(self) -> None:
        self.verify_bundle(self.project / "target")
        for variable in ("MOCK_VERIFY_FAIL", "MOCK_ARCH", "MOCK_SMOKE_FAIL"):
            with self.subTest(variable=variable):
                marker = Path(self.env["MOCK_SMOKE_ARGS"])
                marker.unlink(missing_ok=True)
                self.env[variable] = "wrong-architecture" if variable == "MOCK_ARCH" else "7"
                result = self.verify_relocated()
                self.env.pop(variable)
                self.assertNotEqual(result.returncode, 0)
                self.assertTrue((self.root / "bundle logs/bundle.log").is_file())
                self.assertEqual(marker.exists(), variable == "MOCK_SMOKE_FAIL")
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
