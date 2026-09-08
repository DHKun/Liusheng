"""Release policy regression: DEB, RPM, AppImage and Apple Silicon only.

Run with Python 3.11+: python3 -m unittest discover -s tests -p 'test_release_targets.py' -v
File payloads here are test fixtures; real packagers validate executable artifacts.
"""
from __future__ import annotations

import importlib.util
from pathlib import Path
import re
import shutil
import subprocess
import tempfile
import tomllib
import unittest

ROOT = Path(__file__).resolve().parent.parent
spec = importlib.util.spec_from_file_location("release_assets", ROOT / "scripts/check-release-assets.py")
assets = importlib.util.module_from_spec(spec)
spec.loader.exec_module(assets)


class ReleaseAssetsTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix="liusheng-release-policy-")
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.names = ["liusheng_9.8.7_amd64.deb", "liusheng-9.8.7-1.fc44.x86_64.rpm",
                      "liusheng-9.8.7-linux-x86_64.AppImage", "liusheng-9.8.7-macos-arm64.zip"]
        for name in self.names:
            (self.root / name).write_bytes(b"fixture-package")

    def test_four_packages_generate_verifiable_checksums(self):
        paths = assets.validate(self.root, "9.8.7")
        text = assets.checksums(paths)
        self.assertEqual(len(text.splitlines()), 4)
        self.assertEqual([p.name for p in paths], sorted(self.names))
        (self.root / "SHA256SUMS").write_text(text)
        if shutil.which("sha256sum"):
            subprocess.run(["sha256sum", "--check", "SHA256SUMS"], cwd=self.root,
                           check=True, capture_output=True)

    def test_each_platform_is_required(self):
        for name in self.names:
            with self.subTest(name=name):
                path = self.root / name
                path.unlink()
                with self.assertRaises(ValueError):
                    assets.validate(self.root, "9.8.7")
                path.write_bytes(b"fixture-package")

    def test_retired_platform_assets_are_rejected(self):
        for name in ("liusheng-9.8.7-macos-x86_64.zip", "liusheng-9.8.7-1-x86_64.pkg.tar.zst"):
            with self.subTest(name=name):
                path = self.root / name
                path.write_bytes(b"retired")
                with self.assertRaises(ValueError):
                    assets.validate(self.root, "9.8.7")
                path.unlink()

    def test_old_versions_and_duplicate_rpm_are_rejected(self):
        for name in ("liusheng_9.8.6_amd64.deb", "liusheng-9.8.7-2.fc44.x86_64.rpm"):
            with self.subTest(name=name):
                path = self.root / name
                path.write_bytes(b"extra")
                with self.assertRaises(ValueError):
                    assets.validate(self.root, "9.8.7")
                path.unlink()

    def test_empty_symlink_and_directory_payloads_are_rejected(self):
        path = self.root / self.names[0]
        path.write_bytes(b"")
        with self.assertRaises(ValueError):
            assets.validate(self.root, "9.8.7")
        path.unlink()
        path.symlink_to(self.root / self.names[1])
        with self.assertRaises(ValueError):
            assets.validate(self.root, "9.8.7")
        path.unlink()
        path.mkdir()
        with self.assertRaises(ValueError):
            assets.validate(self.root, "9.8.7")

    def test_version_validation(self):
        for version in ("v9.8.7", "../9.8.7", "9.8.7;true", "9.8", "9.8.7\n"):
            with self.subTest(version=version), self.assertRaises(ValueError):
                assets.validate(self.root, version)


class WorkflowTargetsTests(unittest.TestCase):
    def test_active_workflows_exclude_intel_macos_and_arch(self):
        for path in (ROOT / ".github/workflows").glob("*.yml"):
            text = path.read_text()
            for retired in ("macos-15-intel", "x86_64-apple-darwin", "macos-x86_64", "package-arch.yml", ".pkg.tar.zst"):
                with self.subTest(file=path.name, value=retired):
                    self.assertNotIn(retired, text)

    def test_ci_and_release_share_appimage_workflow(self):
        for name in ("check.yml", "release.yml"):
            text = (ROOT / ".github/workflows" / name).read_text()
            self.assertIn("uses: ./.github/workflows/package-appimage.yml", text)
            self.assertIn("runs-on: macos-15", text)
        release = (ROOT / ".github/workflows/release.yml").read_text()
        self.assertIn("needs: [deb, rpm, appimage, macos]", release)
        self.assertIn("scripts/check-release-assets.py", release)
        self.assertIn("dist/*.AppImage", release)
        self.assertIn("dist/*-macos-arm64.zip", release)
        self.assertIn("fail_on_unmatched_files: true", release)

    def test_appimage_has_packaged_ui_and_clean_runtime_gates(self):
        text = (ROOT / ".github/workflows/package-appimage.yml").read_text()
        for required in ("--full-ui --wayland", "clean-runtime:", "needs: package", "chmod +x dist/*.AppImage", "clean-runtime.json"):
            self.assertIn(required, text)
        clean = text.split("  clean-runtime:", 1)[1]
        self.assertNotRegex(clean, r"apt-get install[^\n]*(?:qt6|pipewire)")

    def test_workflow_script_references_exist(self):
        for path in (ROOT / ".github/workflows").glob("*.yml"):
            for relative in re.findall(r"(?:scripts|tests)/[\w./-]+\.(?:py|sh)", path.read_text()):
                self.assertTrue((ROOT / relative).is_file(), f"{path.name}: {relative}")
        text = (ROOT / ".github/workflows/check.yml").read_text()
        for test in re.findall(r"-p '(test_[\w]+\.py)'", text):
            self.assertTrue((ROOT / "tests" / test).is_file(), test)

    def test_crate_and_workspace_lock_versions_match(self):
        version = tomllib.loads((ROOT / "crates/liusheng/Cargo.toml").read_text())["package"]["version"]
        core = tomllib.loads((ROOT / "crates/liusheng-core/Cargo.toml").read_text())["package"]["version"]
        locked = tomllib.loads((ROOT / "Cargo.lock").read_text())["package"]
        self.assertEqual(core, version)
        self.assertEqual({p["version"] for p in locked if p["name"] in {"liusheng", "liusheng-core"}}, {version})


if __name__ == "__main__":
    unittest.main()
