#!/usr/bin/env python3
"""Validate a native Linux installation, then load its actual packaged QML scene.

CI invokes this in clean distribution containers after apt/dnf installation.
Usage: python3 scripts/check-installed-package.py --prefix /usr --output target/qa/native
"""
from __future__ import annotations
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys

APP_ID = "io.github.dhkun.Liusheng"
ROOT = Path(__file__).resolve().parent.parent


def validate(prefix: Path) -> dict:
    prefix = prefix.resolve(strict=True)
    binary = prefix / "bin/liusheng"
    desktop = prefix / f"share/applications/{APP_ID}.desktop"
    if not binary.is_file() or not os.access(binary, os.X_OK):
        raise ValueError("Installed executable is missing or not executable")
    with binary.open("rb") as stream:
        header = stream.read(20)
    if len(header) != 20 or header[:6] != b"\x7fELF\x02\x01" or header[18:20] != b"\x3e\x00":
        raise ValueError("Expected a 64-bit Linux x86_64 ELF executable")
    fields = dict(line.split("=", 1) for line in desktop.read_text().splitlines() if "=" in line)
    icon = Path(fields.get("Icon", ""))
    icon_root = prefix / "share/icons/hicolor/scalable/apps"
    if not icon.is_absolute() or icon.resolve().parent != icon_root or not icon.is_file():
        raise ValueError("Desktop icon must resolve to the installed hicolor resource")
    # Match the installer's exact quoted Exec grammar, then compare physical
    # paths. A prefix alias may legitimately remain in the installed launcher.
    command = re.fullmatch(r'"([^"`$\\\r\n\x00]+)" %U', fields.get("Exec", ""))
    launch = Path(command[1]) if command else None
    if (launch is None or not launch.is_absolute() or not launch.is_file()
            or launch.resolve(strict=True) != binary.resolve(strict=True)):
        raise ValueError("Desktop launcher points to a different executable")
    image = icon.read_bytes()
    digest = hashlib.sha256(image).hexdigest()[:16]
    if icon.name != f"{APP_ID}.brand-{digest}.svg":
        raise ValueError("Content-addressed desktop icon does not match its bytes")
    for size in (16, 24, 32, 48, 64, 128, 256, 512):
        png = prefix / f"share/icons/hicolor/{size}x{size}/apps/{APP_ID}.png"
        if not png.is_file() or png.read_bytes()[:8] != b"\x89PNG\r\n\x1a\n":
            raise ValueError(f"Missing or invalid installed {size}px icon")
    with binary.open("rb") as stream:
        binary_hash = hashlib.file_digest(stream, "sha256").hexdigest()
    return {"binary": str(binary), "sha256": binary_hash, "desktop": str(desktop), "icon": str(icon)}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--prefix", type=Path, default=Path("/usr"))
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    report = {"passed": False}
    try:
        report.update(validate(args.prefix))
        subprocess.run([sys.executable, str(ROOT / "scripts/check-desktop-smoke.py"), report["binary"],
                        "--output", str(args.output / "desktop.log")], check=True, timeout=40)
        report["passed"] = True
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        report["error"] = str(error)
        print(f"INSTALLED PACKAGE CHECK FAILED: {error}", file=sys.stderr)
    (args.output / "result.json").write_text(json.dumps(report, indent=2) + "\n")
    return 0 if report["passed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
