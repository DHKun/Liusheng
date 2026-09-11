"""Portable regression for Cocoa-only macOS bundles and isolated smoke execution.

Fixture executables exercise the real checker on Linux and macOS. Native Cocoa
loading, codesigning and Mach-O linking are verified by the macOS CI jobs.
"""
from __future__ import annotations

from contextlib import redirect_stdout
import importlib.util
import io
import os
from pathlib import Path
import subprocess
import tempfile
import time
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parent.parent
spec = importlib.util.spec_from_file_location("desktop_smoke", ROOT / "scripts/check-desktop-smoke.py")
smoke = importlib.util.module_from_spec(spec)
spec.loader.exec_module(smoke)


class DesktopSmokeTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix="liusheng-smoke-contract-")
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name).resolve()
        self.binary = self.root / "relocated with spaces/Liusheng.app/Contents/MacOS/Liusheng"
        self.binary.parent.mkdir(parents=True)
        self.contents = self.binary.parent.parent
        self.plugin = self.contents / "PlugIns/platforms/libqcocoa.dylib"
        self.plugin.parent.mkdir(parents=True)
        self.plugin.write_bytes(b"portable Cocoa plugin presence fixture")
        self.config = self.contents / "Resources/qt.conf"
        self.config.parent.mkdir()
        self.config.write_text("[Paths]\nPlugins = PlugIns\nQmlImports = Resources/qml\n")
        self.log = self.root / "logs/desktop.log"
        self.executable('printf "fixture scene loaded\\n"\n')

    def executable(self, body):
        self.binary.write_text("#!/bin/sh\nset -eu\n" + body)
        self.binary.chmod(0o755)

    def env(self, host, requested="auto", debug=False):
        sandbox = self.root / "sandbox"
        sandbox.mkdir(exist_ok=True)
        with patch.object(smoke.sys, "platform", host):
            return smoke.smoke_environment(self.binary, sandbox, requested, debug)

    def check(self, host="darwin", **kwargs):
        with patch.object(smoke.sys, "platform", host), redirect_stdout(io.StringIO()):
            smoke.run_smoke(self.binary, self.log, **kwargs)

    def test_reported_cocoa_only_bundle_uses_native_backend(self):
        self.executable('''if [ "$QT_QPA_PLATFORM" != cocoa ]; then
    echo 'qt.qpa.plugin: Could not find the Qt platform plugin "offscreen" in ""'
    echo 'Available platform plugins are: cocoa.'
    exit 6
fi
printf 'fixture scene loaded\\n'
''')
        self.check()
        self.assertIn("QPA: cocoa", self.log.read_text())
        self.assertIn("startup and clean exit passed", self.log.read_text())
        self.assertFalse((self.plugin.parent / "libqoffscreen.dylib").exists())

    def test_source_mac_binary_also_defaults_to_cocoa(self):
        self.binary = self.root / "target/debug/liusheng"
        self.binary.parent.mkdir(parents=True)
        self.executable("exit 0\n")
        self.assertEqual(self.env("darwin")["QT_QPA_PLATFORM"], "cocoa")

    def test_linux_keeps_headless_platform_and_runtime_library_path(self):
        with patch.dict(os.environ, {"QT_QPA_PLATFORM": "wayland", "LD_LIBRARY_PATH": "/fixture/appimage/lib"}):
            env = self.env("linux")
        self.assertEqual(env["QT_QPA_PLATFORM"], "offscreen")
        self.assertEqual(env["LD_LIBRARY_PATH"], "/fixture/appimage/lib")
        self.assertEqual(env["QT_QUICK_BACKEND"], "software")

    def test_packaged_mac_rejects_offscreen_before_start(self):
        with patch.object(smoke.subprocess, "Popen") as popen:
            with self.assertRaisesRegex(ValueError, "Cocoa runtime plugin"):
                self.check(platform="offscreen")
            popen.assert_not_called()
        self.assertIn("SMOKE CHECK FAILED", self.log.read_text())

    def test_cocoa_requires_native_host(self):
        with self.assertRaisesRegex(ValueError, "native macOS"):
            self.env("linux", "cocoa")

    def test_missing_cocoa_cannot_be_supplied_by_host_environment(self):
        self.plugin.unlink()
        with patch.dict(os.environ, {"QT_PLUGIN_PATH": "/opt/homebrew/lib/qt/plugins"}):
            with patch.object(smoke.subprocess, "Popen") as popen:
                with self.assertRaisesRegex(ValueError, "libqcocoa"):
                    self.check()
                popen.assert_not_called()
        self.assertIn("libqcocoa.dylib", self.log.read_text())

    def test_missing_qt_configuration_fails_before_launch(self):
        self.config.unlink()
        with self.assertRaisesRegex(ValueError, "qt.conf"):
            self.check()

    def test_empty_plugin_and_external_symlink_fail(self):
        self.plugin.write_bytes(b"")
        with self.assertRaisesRegex(ValueError, "bundle resource"):
            self.check()
        self.plugin.unlink()
        external = self.root / "external.dylib"
        external.write_bytes(b"foreign fixture")
        self.plugin.symlink_to(external)
        with self.assertRaisesRegex(ValueError, "bundle resource"):
            self.check()

    def test_environment_is_private_and_discards_developer_overrides(self):
        overrides = {name: "/foreign/qt" for name in smoke.QT_OVERRIDES}
        overrides.update(DYLD_FRAMEWORK_PATH="/foreign/lib", DYLD_INSERT_LIBRARIES="/foreign/inject",
                         HTTPS_PROXY="http://proxy.example.invalid:3128")
        with patch.dict(os.environ, overrides):
            env = self.env("darwin")
        for key in ("QT_PLUGIN_PATH", "QML_IMPORT_PATH", "QML2_IMPORT_PATH", "QT_QPA_PLATFORM_PLUGIN_PATH",
                    "QT_QPA_PLATFORMTHEME", "DYLD_FRAMEWORK_PATH", "DYLD_INSERT_LIBRARIES"):
            self.assertNotIn(key, env)
        for key in ("HOME", "XDG_CONFIG_HOME", "XDG_CACHE_HOME", "XDG_DATA_HOME", "XDG_RUNTIME_DIR"):
            directory = Path(env[key])
            self.assertTrue(directory.is_relative_to(self.root / "sandbox"))
            self.assertEqual(directory.stat().st_mode & 0o777, 0o700)
        self.assertEqual(env["HTTPS_PROXY"], "http://proxy.example.invalid:3128")
        self.assertEqual(env["LIUSHENG_ALLOW_MULTIPLE"], "1")

    def test_plugin_diagnostics_are_explicit(self):
        self.assertEqual(self.env("darwin", debug=True)["QT_DEBUG_PLUGINS"], "1")

    def test_startup_arguments_isolation_and_bundle_bytes_survive(self):
        self.executable('''test "$#" = 3
test "$1" = --smoke-test
test "$2" = --no-update-check
test "$3" = --no-online-metadata
test "$PWD" != "$(dirname "$0")"
test -d "$HOME"
printf '%s\\n' "$HOME" > "$XDG_CACHE_HOME/fixture"
printf 'fixture private scene loaded\\n'
''')
        before = {p: p.read_bytes() for p in self.contents.rglob("*") if p.is_file()}
        self.check()
        self.assertEqual(before, {p: p.read_bytes() for p in self.contents.rglob("*") if p.is_file()})
        self.assertIn("fixture private scene loaded", self.log.read_text())

    def test_nonzero_exit_keeps_diagnostic_log(self):
        self.executable("printf 'fixture 构建包错误\\n'; exit 7\n")
        with self.assertRaisesRegex(RuntimeError, "failed \\(7\\)"):
            self.check()
        self.assertIn("fixture 构建包错误", self.log.read_text())

    def test_qml_and_platform_errors_fail_even_with_zero_exit(self):
        for diagnostic in ("Required property missing", "QQmlApplicationEngine failed to load component",
                           'Could not find the Qt platform plugin "offscreen"',
                           "no Qt platform plugin could be initialized", "Library not loaded: QtCore"):
            with self.subTest(diagnostic=diagnostic):
                self.executable("printf '%s\\n' '" + diagnostic + "'\n")
                with self.assertRaises(RuntimeError):
                    self.check()
                self.assertIn(diagnostic, self.log.read_text())

    def test_timeout_terminates_process_group_and_retains_output(self):
        self.executable("printf 'fixture startup waiting\\n'; sleep 10\n")
        start = time.monotonic()
        with self.assertRaisesRegex(RuntimeError, "timed out"):
            self.check(timeout=0.2)
        self.assertLess(time.monotonic() - start, 4)
        self.assertIn("fixture startup waiting", self.log.read_text())

    def test_unexpected_platform_is_reported(self):
        with self.assertRaisesRegex(ValueError, "Unsupported"):
            self.env("darwin", "other")

    def test_cli_platform_argument_is_validated(self):
        result = subprocess.run([smoke.sys.executable, str(ROOT / "scripts/check-desktop-smoke.py"),
                                 str(self.binary), "--platform", "other"], capture_output=True,
                                text=True, timeout=10)
        self.assertEqual(result.returncode, 2)


class DesktopSmokeWorkflowTests(unittest.TestCase):
    def test_quality_runs_checker_contract_on_linux_and_macos(self):
        quality = (ROOT / ".github/workflows/quality.yml").read_text()
        self.assertEqual(quality.count("-p 'test_desktop_smoke.py'"), 2)
        self.assertIn("--platform cocoa", quality)

    def test_relocated_package_uses_cocoa_and_archives_failure_diagnostics(self):
        release = (ROOT / ".github/workflows/release.yml").read_text()
        check = release.split("      - name: Verify relocated packaged macOS application", 1)[1]
        self.assertIn("--platform cocoa --debug-plugins", check)
        self.assertIn("codesign --verify --deep --strict", check)
        self.assertIn("lipo -archs", check)
        self.assertIn("Archive macOS release diagnostics", check)
        diagnostics = check.split("      - name: Archive macOS release diagnostics", 1)[1]
        self.assertIn("if: always()", diagnostics)
        self.assertIn("name: macos-release-validation", diagnostics)
        self.assertIn("needs: [deb, rpm, appimage, macos, deb-runtime, rpm-runtime]", release)


if __name__ == "__main__":
    unittest.main()
