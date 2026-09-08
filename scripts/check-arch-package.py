#!/usr/bin/env python3
"""Check a built, installed Arch package's identity, runtime deps and icon assets.

Run inside a disposable Arch CI container after pacman -U:
python3 scripts/check-arch-package.py dist/liusheng-*.pkg.tar.zst --metadata target/qa/arch
"""
from __future__ import annotations

import argparse
import hashlib
from pathlib import Path, PurePosixPath
import subprocess
import tomllib

ROOT = Path(__file__).resolve().parent.parent
DESKTOP_ID = "io.github.dhkun.Liusheng"


def read_member(package: Path, name: str) -> bytes:
    return subprocess.run(["bsdtar", "-xOf", str(package), name], check=True,
                          capture_output=True, timeout=30).stdout


def fields(text: str) -> dict[str, list[str]]:
    result: dict[str, list[str]] = {}
    for line in text.splitlines():
        key, separator, value = line.partition(" = ")
        if separator:
            result.setdefault(key, []).append(value)
    return result


def validate(package: Path, metadata: Path, install_root: Path = Path("/")) -> None:
    version = tomllib.loads((ROOT / "crates/liusheng/Cargo.toml").read_text())["package"]["version"]
    info = fields(read_member(package, ".PKGINFO").decode())
    for key, expected in (("pkgname", "liusheng"), ("pkgver", version + "-1"), ("arch", "x86_64")):
        if info.get(key) != [expected]:
            raise ValueError(f"Package {key}: expected {expected!r}, got {info.get(key)!r}")
    dependencies = set((metadata / "runtime-dependencies.txt").read_text().splitlines())
    if set(info.get("depend", [])) != dependencies:
        raise ValueError("Packaged runtime dependencies differ from PKGBUILD metadata")
    subprocess.run(["pacman", "-T", "--", *sorted(dependencies)], check=True, timeout=30)

    members = subprocess.run(["bsdtar", "-tf", str(package)], check=True, text=True,
                             capture_output=True, timeout=30).stdout.splitlines()
    for member in members:
        path = PurePosixPath(member)
        if path.is_absolute() or ".." in path.parts:
            raise ValueError(f"Unsafe archive member: {member}")
    expected_files = ["usr/bin/liusheng", f"usr/share/applications/{DESKTOP_ID}.desktop",
                      f"usr/share/icons/hicolor/scalable/apps/{DESKTOP_ID}.svg",
                      f"usr/share/icons/hicolor/scalable/apps/{DESKTOP_ID}-symbolic.svg"]
    expected_files.extend(f"usr/share/icons/hicolor/{size}x{size}/apps/{DESKTOP_ID}.png"
                          for size in (16, 24, 32, 48, 64, 128, 256, 512))
    desktop = read_member(package, expected_files[1]).decode()
    icon = next((line.removeprefix("Icon=") for line in desktop.splitlines() if line.startswith("Icon=")), "")
    prefix = f"/usr/share/icons/hicolor/scalable/apps/{DESKTOP_ID}.brand-"
    if not icon.startswith(prefix) or not icon.endswith(".svg"):
        raise ValueError(f"Desktop icon is not the content-addressed installed icon: {icon!r}")
    expected_files.append(icon.lstrip("/"))
    for name in expected_files:
        data = read_member(package, name)
        installed = install_root / name
        if not installed.is_file() or installed.read_bytes() != data:
            raise ValueError(f"Installed payload mismatch: {name}")
    brand = (ROOT / "crates/liusheng/qml/assets/app-icon/liusheng.svg").read_bytes()
    if read_member(package, icon.lstrip("/")) != brand:
        raise ValueError("Packaged launcher icon differs from the reviewed brand resource")
    digest = hashlib.sha256(brand).hexdigest()[:16]
    if not icon.endswith(f".brand-{digest}.svg"):
        raise ValueError("Desktop icon cache key does not match the SVG contents")
    if not (install_root / "usr/bin/liusheng").stat().st_mode & 0o111:
        raise ValueError("Installed application lacks executable permission")
    print(f"Arch {version}: identity, {len(dependencies)} runtime dependencies, "
          f"{len(expected_files)} installed files and brand icon verified")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("package", type=Path)
    parser.add_argument("--metadata", type=Path, required=True)
    args = parser.parse_args()
    try:
        validate(args.package.resolve(strict=True), args.metadata)
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        parser.exit(1, f"Arch package validation failed: {error}\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
