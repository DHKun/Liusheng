"""Regression contracts for shared gates and native installed-package validation.

These fixtures validate failure handling; native distribution jobs additionally
install the real packages and run their actual executables.
"""
from __future__ import annotations
import contextlib
import hashlib
import importlib.util
import io
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parent.parent
SPEC = importlib.util.spec_from_file_location("installed_package", ROOT / "scripts/check-installed-package.py")
installed = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(installed)


class InstalledPackageTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix="liusheng-installed-contract-")
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.prefix = self.root / "installation with spaces"
        self.binary = self.prefix / "bin/liusheng"
        self.binary.parent.mkdir(parents=True)
        self.binary.write_bytes(b"\x7fELF\x02\x01" + bytes(12) + b"\x3e\x00" + b"test fixture")
        self.binary.chmod(0o755)
        image = b'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"/>'
        digest = hashlib.sha256(image).hexdigest()[:16]
        self.icon = self.prefix / f"share/icons/hicolor/scalable/apps/{installed.APP_ID}.brand-{digest}.svg"
        self.icon.parent.mkdir(parents=True)
        self.icon.write_bytes(image)
        for size in (16, 24, 32, 48, 64, 128, 256, 512):
            path = self.prefix / f"share/icons/hicolor/{size}x{size}/apps/{installed.APP_ID}.png"
            path.parent.mkdir(parents=True)
            path.write_bytes(b"\x89PNG\r\n\x1a\nfixture")
        self.desktop = self.prefix / f"share/applications/{installed.APP_ID}.desktop"
        self.desktop.parent.mkdir(parents=True)
        self.desktop.write_text(f'[Desktop Entry]\nExec="{self.binary}" %U\nIcon={self.icon}\n')

    def test_valid_installation_uses_exact_binary_and_content_addressed_icon(self):
        result = installed.validate(self.prefix)
        self.assertTrue(Path(result["binary"]).samefile(self.binary))
        self.assertTrue(Path(result["icon"]).samefile(self.icon))
        self.assertEqual(result["sha256"], hashlib.sha256(self.binary.read_bytes()).hexdigest())

    def test_wrong_architecture_and_missing_executable_are_rejected(self):
        original = self.binary.read_bytes()
        for payload in (b"stale script", original[:18] + b"\xb7\x00" + original[20:]):
            self.binary.write_bytes(payload)
            with self.assertRaises(ValueError):
                installed.validate(self.prefix)
        self.binary.write_bytes(original)
        self.binary.chmod(0o644)
        with self.assertRaises(ValueError):
            installed.validate(self.prefix)

    def test_launcher_pointing_to_old_installation_is_rejected(self):
        self.desktop.write_text(f'[Desktop Entry]\nExec="/old/bin/liusheng" %U\nIcon={self.icon}\n')
        with self.assertRaisesRegex(ValueError, "different executable"):
            installed.validate(self.prefix)

    def test_aliased_installation_and_launcher_resolve_to_the_same_files(self):
        alias = self.root / "installation alias"
        alias.symlink_to(self.prefix.resolve(strict=True), target_is_directory=True)
        icon = alias / self.icon.relative_to(self.prefix)
        self.desktop.write_text(f'[Desktop Entry]\nExec="{alias / "bin/liusheng"}" %U\nIcon={icon}\n')
        for prefix in (self.prefix, alias):
            with self.subTest(prefix=prefix):
                report = installed.validate(prefix)
                self.assertTrue(Path(report["binary"]).samefile(self.binary))
                self.assertTrue(Path(report["icon"]).samefile(self.icon))

    def test_same_named_foreign_binary_and_extra_arguments_are_rejected(self):
        other = self.root / "old installation/bin/liusheng"
        other.parent.mkdir(parents=True)
        other.write_bytes(self.binary.read_bytes())
        other.chmod(0o755)
        for command in (f'"{other}" %U', '"bin/liusheng" %U',
                        f'"{self.binary}" --extra %U', f'"{self.binary}" %U; echo injected',
                        f'"{self.binary}" %F'):
            with self.subTest(command=command):
                self.desktop.write_text(f'[Desktop Entry]\nExec={command}\nIcon={self.icon}\n')
                with self.assertRaisesRegex(ValueError, "different executable"):
                    installed.validate(self.prefix)

    def test_external_icon_symlink_is_still_rejected(self):
        outside = self.root / self.icon.name
        outside.write_bytes(self.icon.read_bytes())
        self.icon.unlink()
        self.icon.symlink_to(outside.resolve(strict=True))
        with self.assertRaisesRegex(ValueError, "hicolor resource"):
            installed.validate(self.prefix)

    def test_tampered_icon_and_missing_png_are_rejected(self):
        original = self.icon.read_bytes()
        self.icon.write_bytes(original + b"tampered")
        with self.assertRaisesRegex(ValueError, "does not match"):
            installed.validate(self.prefix)
        self.icon.write_bytes(original)
        (self.prefix / f"share/icons/hicolor/32x32/apps/{installed.APP_ID}.png").unlink()
        with self.assertRaisesRegex(ValueError, "32px"):
            installed.validate(self.prefix)

    def test_smoke_failure_replaces_an_old_success_record(self):
        output = self.root / "logs"
        output.mkdir()
        (output / "result.json").write_text('{"passed":true}')
        with patch.object(sys, "argv", ["check-installed-package.py", "--prefix", str(self.prefix), "--output", str(output)]), \
             patch.object(installed.subprocess, "run", side_effect=subprocess.CalledProcessError(1, ["smoke"])) as run, \
             contextlib.redirect_stderr(io.StringIO()):
            self.assertEqual(installed.main(), 1)
        self.assertFalse(json.loads((output / "result.json").read_text())["passed"])
        self.assertTrue(Path(run.call_args.args[0][2]).samefile(self.binary))

    def test_success_requires_validation_and_actual_smoke_command(self):
        output = self.root / "logs"
        with patch.object(sys, "argv", ["check-installed-package.py", "--prefix", str(self.prefix), "--output", str(output)]), \
             patch.object(installed.subprocess, "run") as run:
            self.assertEqual(installed.main(), 0)
        self.assertTrue(json.loads((output / "result.json").read_text())["passed"])
        self.assertEqual(run.call_count, 1)
        self.assertTrue(run.call_args.kwargs["check"])


class SharedGateTests(unittest.TestCase):
    def test_main_and_tag_call_the_same_commit_local_full_quality_workflow(self):
        for name in ("check.yml", "release.yml"):
            text = (ROOT / ".github/workflows" / name).read_text()
            self.assertIn("  quality:\n    uses: ./.github/workflows/quality.yml", text)
        shared = (ROOT / ".github/workflows/quality.yml").read_text()
        self.assertIn("workflow_call:", shared)
        for command in ("cargo test --workspace", "cargo clippy --workspace --all-targets", "check-controls.py", "check-updates.py",
                        "check-ui.py", "check-wayland.py", "check-tray-icon.py", "check-watcher.py", "check-macos-media.py", "test_optimization_delivery.py"):
            self.assertIn(command, shared, command)
        self.assertIn("runs-on: macos-15", shared)
        self.assertNotIn("continue-on-error: true", shared)

    def test_only_publish_receives_repository_write_permission(self):
        text = (ROOT / ".github/workflows/release.yml").read_text()
        before, publish = text.split("  publish:\n", 1)
        self.assertIn("permissions:\n  contents: read", before)
        self.assertNotIn("contents: write", before)
        self.assertIn("      contents: write", publish)
        self.assertIn("needs: [deb, rpm, appimage, macos, deb-runtime, rpm-runtime]", publish)

    def test_installed_deb_rpm_and_relocated_macos_gate_release(self):
        text = (ROOT / ".github/workflows/release.yml").read_text()
        self.assertIn('apt-get install -y --no-install-recommends "$GITHUB_WORKSPACE"/dist/*.deb', text)
        self.assertIn("dnf install -y dist/*.rpm", text)
        self.assertEqual(text.count("scripts/check-installed-package.py --prefix /usr"), 2)
        self.assertIn('bash scripts/check-macos-bundle.sh dist/*-macos-arm64.zip', text)
        verifier = (ROOT / "scripts/check-macos-bundle.sh").read_text()
        self.assertIn('ditto -x -k "$package" "$work"', verifier)
        self.assertIn('--platform cocoa --debug-plugins', verifier)
        self.assertIn("name: native-package-validation-deb", text)
        self.assertIn("name: native-package-validation-rpm", text)

class BenchmarkComparisonTests(unittest.TestCase):
    @staticmethod
    def module():
        spec = importlib.util.spec_from_file_location("startup_benchmark", ROOT / "scripts/benchmark-startup.py")
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        return module

    @staticmethod
    def report(value=100.0):
        return {"backend": "offscreen/software", "library": "synthetic offline cache", "timing_origin": "first window",
                "results": [{"tracks": 1000, "warm_median_ms": {"interactive_ms": value, "first_frame_ms": 20.0}}]}

    def test_relative_results_keep_absolute_values_and_sign(self):
        result = self.module().compare_reports(self.report(), self.report(75.0))
        self.assertEqual(result[0]["metrics"]["interactive_ms"], {"before_ms": 100.0, "after_ms": 75.0, "change_percent": -25.0})

    def test_comparison_rejects_different_backends_and_datasets(self):
        for field in ("backend", "library", "timing_origin"):
            other = self.report()
            other[field] = "different"
            with self.assertRaises(ValueError):
                self.module().compare_reports(self.report(), other)
        other = self.report()
        other["results"][0]["tracks"] = 10000
        with self.assertRaises(ValueError):
            self.module().compare_reports(self.report(), other)

    def test_nonfinite_and_negative_timings_are_rejected(self):
        for value in (float("nan"), float("inf"), -1.0):
            with self.assertRaises(ValueError):
                self.module().compare_reports(self.report(), self.report(value))


if __name__ == "__main__":
    unittest.main()
