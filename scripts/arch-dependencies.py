#!/usr/bin/env python3
"""Render Arch metadata with makepkg as an ordinary user; never install packages.

The committed PKGBUILD is the only application dependency manifest. Consumers
install packages.txt and verify dependencies.txt with pacman -T, retaining version
constraints. Usage: python3 scripts/arch-dependencies.py --output target/qa/arch
"""
from __future__ import annotations

import argparse
import os
from pathlib import Path
import re
import subprocess
import tempfile
import tomllib

ROOT = Path(__file__).resolve().parent.parent
DEPENDENCY = re.compile(r"([A-Za-z0-9@_+][A-Za-z0-9@_.+\-]*)(?:[<>=]{1,2}[^\s]+)?\Z")


def parse_dependencies(srcinfo: str, architecture: str = "x86_64") -> tuple[list[str], list[str]]:
    all_dependencies: set[str] = set()
    runtime: set[str] = set()
    for line in srcinfo.splitlines():
        key, separator, value = line.strip().partition(" = ")
        if not separator:
            continue
        base = key.removesuffix("_" + architecture)
        if base not in {"depends", "makedepends", "checkdepends"}:
            continue
        if not DEPENDENCY.fullmatch(value):
            raise ValueError(f"Invalid {key} dependency: {value!r}")
        all_dependencies.add(value)
        if base == "depends":
            runtime.add(value)
    if not runtime:
        raise ValueError("PKGBUILD metadata contains no runtime dependencies")
    return sorted(all_dependencies), sorted(runtime)


def generate(project: Path, output: Path) -> None:
    if os.geteuid() == 0:
        raise ValueError("Run Arch metadata generation as the ordinary package builder")
    manifest = tomllib.loads((project / "crates/liusheng/Cargo.toml").read_text())
    version = manifest["package"]["version"]
    template = (project / "packaging/arch/PKGBUILD.in").read_text()
    rendered = template.replace("@VERSION@", version).replace("@SHA256@", "0" * 64)
    # Metadata generation executes the committed PKGBUILD through makepkg under
    # the same unprivileged account as compilation. No source archive is built.
    with tempfile.TemporaryDirectory(prefix="liusheng-arch-metadata-") as directory:
        Path(directory, "PKGBUILD").write_text(rendered)
        result = subprocess.run(["makepkg", "--printsrcinfo"], cwd=directory,
                                text=True, capture_output=True, check=True, timeout=30)
    constraints, runtime = parse_dependencies(result.stdout)
    packages = sorted({DEPENDENCY.fullmatch(item).group(1) for item in constraints})
    output.mkdir(parents=True, exist_ok=True)
    (output / ".SRCINFO").write_text(result.stdout)
    for name, entries in (("dependencies.txt", constraints), ("packages.txt", packages),
                          ("runtime-dependencies.txt", runtime)):
        (output / name).write_text("\n".join(entries) + "\n")
    print(f"Arch {version}: {len(packages)} build/runtime packages resolved from PKGBUILD")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    try:
        generate(ROOT, args.output.resolve())
    except (ValueError, OSError, subprocess.SubprocessError) as error:
        details = getattr(error, "stderr", "") or ""
        parser.exit(1, f"Arch dependency metadata failed: {error}\n{details}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
