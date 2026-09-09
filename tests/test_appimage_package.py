"""AppImage boundary tests. All fixtures stay temporary; deployment tools are mocked.

Real image creation, relocation, linked-library checks and native Wayland tests
run separately in package-appimage.yml. Requires Python 3.11+ and Bash.
"""
from __future__ import annotations

import hashlib
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parent.parent
spec = importlib.util.spec_from_file_location("appimage_packaging_tests", ROOT / "scripts/package-appimage.py")
packager = importlib.util.module_from_spec(spec)
spec.loader.exec_module(packager)


class AppImagePackagingTests(unittest.TestCase):
    def setUp(self):
        temp = tempfile.TemporaryDirectory(prefix="liusheng-appimage-tests-")
        self.addCleanup(temp.cleanup)
        self.root = Path(temp.name)

    def appdir(self):
        root = self.root / "portable AppDir"
        for name in [*packager.REQUIRED_RUNTIME_FILES, *("usr/lib/" + n for n in packager.FORCED_CLIENT_LIBRARIES)]:
            path = root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(b"test fixture")
        (root / f"{packager.DESKTOP_ID}.desktop").write_text(
            f"[Desktop Entry]\nExec=liusheng %F\nIcon={packager.DESKTOP_ID}\n")
        return root

    def test_required_runtime_contract(self):
        required = packager.REQUIRED_RUNTIME_FILES
        for item in ("usr/plugins/platforms/libqwayland-generic.so", "usr/plugins/imageformats/libqsvg.so",
                     "usr/qml/QtQuick/Controls/Basic/qmldir", "usr/lib/libpipewire-0.3.so.0",
                     "usr/lib/libasound.so.2", "usr/share/pipewire/client.conf",
                     "usr/plugins/tls/libqopensslbackend.so"):
            self.assertIn(item, required)
        packager.validate_appdir(self.appdir())

    def test_every_required_runtime_file_is_enforced(self):
        root = self.appdir()
        for name in [*packager.REQUIRED_RUNTIME_FILES, *("usr/lib/" + n for n in packager.FORCED_CLIENT_LIBRARIES)]:
            path = root / name
            contents = path.read_bytes()
            path.unlink()
            with self.subTest(name=name), self.assertRaises(ValueError):
                packager.validate_appdir(root)
            path.write_bytes(contents)

    def test_external_links_are_rejected(self):
        root = self.appdir()
        outside = self.root / "host-dependency"
        outside.write_bytes(b"host-only")
        (root / "external").symlink_to(outside)
        with self.assertRaises(ValueError):
            packager.validate_appdir(root)

    def test_system_fonts_stay_out_of_package(self):
        root = self.appdir()
        for extension in ("ttf", "otf", "ttc", "woff", "woff2"):
            path = root / f"host-font.{extension}"
            path.write_bytes(b"fixture")
            with self.subTest(extension=extension), self.assertRaises(ValueError):
                packager.validate_appdir(root)
            path.unlink()

    def test_portable_desktop_keeps_metadata(self):
        text = '[Desktop Entry]\nName=留声\nExec="/old/path/bin/liusheng" %U\nTryExec=/old/path/bin/liusheng\nIcon=/old/icon.svg\nMimeType=audio/flac;\n'
        changed = packager.portable_desktop(text)
        self.assertIn("Name=留声", changed)
        self.assertIn("Exec=liusheng %F", changed)
        self.assertIn("TryExec=liusheng", changed)
        self.assertIn(f"Icon={packager.DESKTOP_ID}", changed)
        self.assertIn("MimeType=audio/flac;", changed)
        self.assertNotIn("/old/", changed)

    def test_version_mismatch_is_rejected(self):
        path = self.root / "crates/liusheng/Cargo.toml"
        path.parent.mkdir(parents=True)
        path.write_text('[package]\nversion = "9.8.7"\n')
        self.assertEqual(packager.version_for(self.root, "9.8.7"), "9.8.7")
        with self.assertRaises(ValueError):
            packager.version_for(self.root, "9.8.6")

    def test_elf_architecture_and_permissions(self):
        path = self.root / "liusheng"
        data = bytearray(20)
        data[:6] = b"\x7fELF\x02\x01"
        data[18:20] = b"\x3e\x00"
        path.write_bytes(data)
        path.chmod(0o755)
        packager.check_elf(path)
        path.chmod(0o644)
        with self.assertRaises(ValueError):
            packager.check_elf(path)
        path.chmod(0o755)
        data[18:20] = b"\xb7\x00"
        path.write_bytes(data)
        with self.assertRaises(ValueError):
            packager.check_elf(path)

    def locked_tool(self, content=b"approved-tool"):
        path = self.root / "packaging/appimage/tools.lock.json"
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(json.dumps({"tool.AppImage": {
            "url": "https://github.com/AppImage/appimagetool/releases/download/test/tool.AppImage",
            "sha256": hashlib.sha256(content).hexdigest()}}))
        return self.root / "cache"

    def test_cached_tools_are_checked_before_execution(self):
        cache = self.locked_tool()
        cache.mkdir()
        tool = cache / "tool.AppImage"
        tool.write_bytes(b"approved-tool")
        with patch.object(packager, "run") as run:
            self.assertEqual(packager.verified_tools(self.root, cache)[tool.name], tool)
            run.assert_not_called()
        tool.write_bytes(b"corrupt")
        with self.assertRaisesRegex(ValueError, "SHA-256 mismatch"):
            packager.verified_tools(self.root, cache)

    def test_download_hash_mismatch_never_enters_cache(self):
        cache = self.locked_tool()
        def download(command, **kwargs):
            self.assertIn("--proto-redir", command)
            Path(command[command.index("--output") + 1]).write_bytes(b"corrupt-download")
        with patch.object(packager, "run", side_effect=download), self.assertRaisesRegex(ValueError, "SHA-256 mismatch"):
            packager.verified_tools(self.root, cache)
        self.assertFalse((cache / "tool.AppImage").exists())

    def test_cache_symlinks_are_rejected(self):
        cache = self.locked_tool()
        cache.mkdir()
        outside = self.root / "outside"
        outside.write_bytes(b"approved-tool")
        (cache / "tool.AppImage").symlink_to(outside)
        with self.assertRaises(ValueError):
            packager.verified_tools(self.root, cache)

    def test_lock_contains_fixed_versions_and_hashes(self):
        entries = json.loads((ROOT / "packaging/appimage/tools.lock.json").read_text())
        self.assertEqual(len(entries), 4)
        for entry in entries.values():
            self.assertRegex(entry["sha256"], r"^[0-9a-f]{64}$")
            self.assertNotIn("/continuous/", entry["url"])
            self.assertTrue(entry["url"].startswith("https://github.com/"))

    def test_cross_target_cannot_reuse_an_old_native_binary(self):
        with patch.object(packager, "ROOT", ROOT), patch.object(packager, "run") as run, \
             patch.object(packager.platform, "system", return_value="Linux"), \
             patch.object(packager.platform, "machine", return_value="x86_64"), \
             patch.dict(os.environ, {"CARGO_BUILD_TARGET": "aarch64-unknown-linux-gnu"}), \
             patch.object(sys, "argv", ["package-appimage.py"]), self.assertRaises(SystemExit):
            packager.main()
        run.assert_not_called()

    def test_failed_build_stops_before_deployment(self):
        with patch.object(packager, "run", side_effect=subprocess.CalledProcessError(101, ["cargo"])) as run, \
             patch.object(packager, "verified_tools") as tools, \
             patch.object(packager.platform, "system", return_value="Linux"), \
             patch.object(packager.platform, "machine", return_value="x86_64"), \
             patch.dict(os.environ, {"CARGO_BUILD_TARGET": "", "CARGO_TARGET_DIR": str(self.root / "build directory")}), \
             patch.object(sys, "argv", ["package-appimage.py"]), self.assertRaises(subprocess.CalledProcessError):
            packager.main()
        args = run.call_args.args[0]
        self.assertEqual(args[args.index("--target-dir") + 1], str(self.root / "build directory"))
        tools.assert_not_called()


class AppRunTests(unittest.TestCase):
    def setUp(self):
        temp = tempfile.TemporaryDirectory(prefix="liusheng-apprun-")
        self.addCleanup(temp.cleanup)
        self.root = Path(temp.name)
        self.appdir = self.root / "moved app with spaces"
        (self.appdir / "usr/bin").mkdir(parents=True)
        (self.appdir / "usr/share/alsa").mkdir(parents=True)
        (self.appdir / "usr/share/alsa/alsa.conf").write_text("pcm.!default { type null }\n")
        shutil.copyfile(ROOT / "packaging/appimage/AppRun", self.appdir / "AppRun")
        program = self.appdir / "usr/bin/liusheng"
        program.write_text(f'''#!{sys.executable}
import json, os, sys
from pathlib import Path
config = Path(os.environ["ALSA_CONFIG_PATH"])
print(json.dumps({{"arguments": sys.argv[1:], "cwd": os.getcwd(), "env": dict(os.environ), "config": config.read_text() if config.exists() else None}}))
''')
        program.chmod(0o755)
        self.env = {"PATH": "/usr/bin:/bin", "HOME": str(self.root / "home"), "LANG": "C.UTF-8"}

    def launch(self, **extra):
        result = subprocess.run(["bash", str(self.appdir / "AppRun"), "relative song.flac", "--test"],
                                cwd=self.root, env={**self.env, **extra}, text=True, capture_output=True, check=True)
        return json.loads(result.stdout)

    def test_relocation_preserves_cwd_and_file_arguments(self):
        result = self.launch()
        self.assertEqual(result["cwd"], str(self.root))
        self.assertEqual(result["arguments"], ["relative song.flac", "--test"])
        self.assertEqual(result["env"]["HOME"], self.env["HOME"])

    def test_host_qt_paths_are_replaced(self):
        result = self.launch(QT_PLUGIN_PATH="/other/plugins", QML_IMPORT_PATH="/other/qml", QML2_IMPORT_PATH="/other/qml")
        for variable, suffix in (("QT_PLUGIN_PATH", "usr/plugins"), ("QML_IMPORT_PATH", "usr/qml"), ("QML2_IMPORT_PATH", "usr/qml")):
            self.assertEqual(result["env"][variable], str(self.appdir / suffix))

    def test_wayland_default_and_explicit_platform(self):
        self.assertEqual(self.launch(WAYLAND_DISPLAY="wayland-0")["env"]["QT_QPA_PLATFORM"], "wayland;xcb")
        self.assertEqual(self.launch(WAYLAND_DISPLAY="wayland-0", QT_QPA_PLATFORM="offscreen")["env"]["QT_QPA_PLATFORM"], "offscreen")
        self.assertNotIn("QT_QPA_PLATFORM", self.launch()["env"])

    def test_alsa_config_path_survives_spaces_in_bundle_path(self):
        result = self.launch()
        self.assertRegex(result["env"]["ALSA_CONFIG_PATH"], r"^/proc/self/fd/[0-9]+$")
        self.assertIn("type null", result["config"])

    def test_audio_clients_use_bundled_modules_and_keep_user_overrides(self):
        result = self.launch(PIPEWIRE_CONFIG_DIR="/custom/pipewire", ALSA_CONFIG_PATH="/custom/asound.conf")
        self.assertEqual(result["env"]["SPA_PLUGIN_DIR"], str(self.appdir / "usr/lib/spa-0.2"))
        self.assertEqual(result["env"]["PIPEWIRE_CONFIG_DIR"], "/custom/pipewire")
        self.assertEqual(result["env"]["ALSA_CONFIG_PATH"], "/custom/asound.conf")


if __name__ == "__main__":
    unittest.main()
