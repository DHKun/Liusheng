#!/usr/bin/env python3
"""Require all five platform packages for one version, then generate SHA256SUMS."""
from __future__ import annotations

import argparse
import hashlib
from pathlib import Path
import re


def validate(directory: Path, version: str) -> list[Path]:
    if not re.fullmatch(r"[0-9]+\.[0-9]+\.[0-9]+", version):
        raise ValueError("Release version must have the form X.Y.Z")
    expected = {
        "Debian": f"liusheng_{version}_amd64.deb",
        "Fedora": f"liusheng-{version}-*.x86_64.rpm",
        "Arch": f"liusheng-{version}-1-x86_64.pkg.tar.zst",
        "macOS arm64": f"liusheng-{version}-macos-arm64.zip",
        "macOS x86_64": f"liusheng-{version}-macos-x86_64.zip",
    }
    selected: list[Path] = []
    for platform, pattern in expected.items():
        matches = list(directory.glob(pattern))
        if len(matches) != 1:
            raise ValueError(f"{platform}: expected one {pattern}, found {len(matches)}")
        path = matches[0]
        if path.is_symlink() or not path.is_file() or path.stat().st_size == 0:
            raise ValueError(f"Invalid or empty package: {path.name}")
        selected.append(path)
    extras = [p.name for p in directory.iterdir() if p not in selected and p.name != "SHA256SUMS"]
    if extras:
        raise ValueError(f"Unexpected release assets: {sorted(extras)}")
    return sorted(selected)


def checksums(packages: list[Path]) -> str:
    lines = []
    for path in packages:
        with path.open("rb") as source:
            digest = hashlib.file_digest(source, "sha256").hexdigest()
        lines.append(f"{digest}  {path.name}\n")
    return "".join(lines)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--directory", type=Path, required=True)
    parser.add_argument("--version", required=True)
    args = parser.parse_args()
    try:
        packages = validate(args.directory, args.version)
        (args.directory / "SHA256SUMS").write_text(checksums(packages))
    except (OSError, ValueError) as error:
        parser.exit(1, f"Release asset validation failed: {error}\n")
    print(f"Release {args.version}: all five platform packages present; SHA256SUMS generated")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
