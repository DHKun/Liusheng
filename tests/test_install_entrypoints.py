"""Installation entrypoint regression tests; all writes stay in temporary directories.

Run: python3 -m unittest discover -s tests -p 'test_install_entrypoints.py' -v
Requires Bash and just. Cargo is a test double; this suite checks installation,
permissions, icon copying and cleanup, while the normal build CI checks compilation.
"""
from __future__ import annotations

import os
import hashlib
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parent.parent
DESKTOP_ID = "io.github.dhkun.Liusheng"


class InstallEntrypointTests(unittest.TestCase):
    def setUp(self) -> None:
        self.just = shutil.which("just")
        if not self.just:
            self.fail("Install just to run installation entrypoint tests")
        temporary = tempfile.TemporaryDirectory(prefix="liusheng-install-test-")
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.project = self.root / "copied checkout"
        self.project.mkdir()
        for name in ("justfile", "scripts/install.sh", "scripts/uninstall.sh",
                     f"resources/{DESKTOP_ID}.desktop.in"):
            destination = self.project / name
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(ROOT / name, destination)
            destination.chmod(0o644)  # Reproduce the user's lost execute bit.
        self.assets = Path("crates/liusheng/qml/assets/app-icon")
        shutil.copytree(ROOT / self.assets, self.project / self.assets)
        home = self.root / "home"
        home.mkdir()
        tools = self.root / "tools"
        tools.mkdir()
        # Create a tiny, identifiable binary through the normal Cargo invocation.
        self.stub(tools / "cargo", '''#!/bin/bash
set -eu
printf '%s\\n' "$@" > "$HOME/cargo-arguments.txt"
mkdir -p "$CARGO_TARGET_DIR/release"
printf '#!/bin/sh\\nexit 0\\n' > "$CARGO_TARGET_DIR/release/liusheng"
chmod 755 "$CARGO_TARGET_DIR/release/liusheng"
''')
        for name in ("update-desktop-database", "gtk-update-icon-cache", "kbuildsycoca6"):
            self.stub(tools / name, "#!/bin/sh\nexit 0\n")
        self.env = {
            "PATH": f"{tools}:/usr/bin:/bin",
            "HOME": str(home),
            "LANG": "C.UTF-8",
            "CARGO_TARGET_DIR": str(self.root / "build output"),
            "XDG_CONFIG_HOME": str(home / ".config"),
            "XDG_CACHE_HOME": str(home / ".cache"),
            "XDG_DATA_HOME": str(home / ".local/share"),
        }
        self.prefix = home / ".local"

    @staticmethod
    def stub(path: Path, content: str) -> None:
        path.write_text(content)
        path.chmod(0o755)

    def invoke(self, command: list[str]) -> subprocess.CompletedProcess[str]:
        return subprocess.run(command, cwd=self.project, env=self.env, text=True,
                              stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=20)

    def recipe(self, name: str) -> None:
        result = self.invoke([str(self.just), name])
        self.assertEqual(result.returncode, 0, result.stdout)

    def verify_install(self, prefix: Path) -> None:
        binary = prefix / "bin/liusheng"
        self.assertTrue(binary.is_file())
        self.assertEqual(binary.stat().st_mode & 0o777, 0o755)
        icon_root = prefix / "share/icons/hicolor"
        icons = list(icon_root.glob(f"*/apps/{DESKTOP_ID}*"))
        self.assertEqual(len(icons), 11)
        self.assertEqual((icon_root / f"scalable/apps/{DESKTOP_ID}.svg").read_bytes(),
                         (self.project / self.assets / "liusheng.svg").read_bytes())

    def test_install_and_uninstall_when_execute_bits_are_missing(self) -> None:
        denied = self.invoke(["bash", "-c", "./scripts/install.sh"])
        self.assertEqual(denied.returncode, 126)
        self.recipe("install")
        self.verify_install(self.prefix)
        arguments = (Path(self.env["HOME"]) / "cargo-arguments.txt").read_text().splitlines()
        self.assertIn("--release", arguments)
        self.assertIn("--locked", arguments)
        unrelated = self.prefix / "bin/keep-me"
        unrelated.write_text("unrelated application")
        data = self.prefix / "share/liusheng/library.db"
        data.parent.mkdir(parents=True, exist_ok=True)
        data.write_bytes(b"preserved library fixture")
        self.recipe("uninstall")
        self.assertFalse((self.prefix / "bin/liusheng").exists())
        self.assertEqual(unrelated.read_text(), "unrelated application")
        self.assertEqual(data.read_bytes(), b"preserved library fixture")

    def test_staged_install_uses_prefix_and_preserves_host_home(self) -> None:
        self.env.update(PREFIX="/usr", DESTDIR=str(self.root / "stage"))
        self.recipe("install")
        staged = self.root / "stage/usr"
        self.verify_install(staged)
        desktop = (staged / f"share/applications/{DESKTOP_ID}.desktop").read_text()
        self.assertIn('Exec="/usr/bin/liusheng" %U', desktop)
        self.assertFalse(self.prefix.exists())
        self.recipe("uninstall")
        self.assertFalse((staged / "bin/liusheng").exists())

    def test_desktop_uses_exact_content_addressed_icon(self) -> None:
        self.recipe("install")
        data = (self.project / self.assets / "liusheng.svg").read_bytes()
        digest = hashlib.sha256(data).hexdigest()[:16]
        desktop = (self.prefix / f"share/applications/{DESKTOP_ID}.desktop").read_text()
        value = next(line[5:] for line in desktop.splitlines() if line.startswith("Icon="))
        icon = Path(value)
        self.assertTrue(icon.is_absolute())
        self.assertEqual(icon.name, f"{DESKTOP_ID}.brand-{digest}.svg")
        self.assertEqual(icon.read_bytes(), data)
        # A theme can still supply the old common name; our desktop pins the file.
        themed = self.prefix / f"share/icons/ConflictingTheme/scalable/apps/{DESKTOP_ID}.svg"
        themed.parent.mkdir(parents=True)
        themed.write_text("old unrelated theme icon")
        self.recipe("install")
        self.assertEqual(themed.read_text(), "old unrelated theme icon")
        self.assertEqual(icon.read_bytes(), data)
        self.recipe("uninstall")
        self.assertFalse(icon.exists())
        self.assertTrue(themed.exists())

    def test_new_icon_content_gets_new_cache_key(self) -> None:
        self.recipe("install")
        desktop = self.prefix / f"share/applications/{DESKTOP_ID}.desktop"
        old = next(line[5:] for line in desktop.read_text().splitlines() if line.startswith("Icon="))
        asset = self.project / self.assets / "liusheng.svg"
        asset.write_bytes(asset.read_bytes() + b"\n")
        self.recipe("install")
        new = next(line[5:] for line in desktop.read_text().splitlines() if line.startswith("Icon="))
        self.assertNotEqual(old, new)
        self.assertEqual(Path(new).read_bytes(), asset.read_bytes())
        self.recipe("uninstall")
        self.assertFalse(Path(old).exists())
        self.assertFalse(Path(new).exists())

    def test_target_dir_is_explicit_and_stage_is_not_embedded(self) -> None:
        self.env.update(PREFIX="/usr", DESTDIR=str(self.root / "stage"))
        self.recipe("install")
        arguments = (Path(self.env["HOME"]) / "cargo-arguments.txt").read_text().splitlines()
        self.assertEqual(arguments[arguments.index("--target-dir") + 1], self.env["CARGO_TARGET_DIR"])
        desktop = (self.root / "stage/usr" / f"share/applications/{DESKTOP_ID}.desktop").read_text()
        value = next(line[5:] for line in desktop.splitlines() if line.startswith("Icon="))
        self.assertTrue(value.startswith("/usr/share/icons/"))
        self.assertNotIn(str(self.root), value)
        self.assertTrue(Path(self.env["DESTDIR"] + value).is_file())

    def test_missing_prebuilt_binary_stops_before_install(self) -> None:
        result = self.invoke(["bash", "scripts/install.sh", "--no-build"])
        self.assertEqual(result.returncode, 1, result.stdout)
        self.assertIn("release", result.stdout)
        self.assertFalse((self.prefix / "bin/liusheng").exists())


if __name__ == "__main__":
    unittest.main()
