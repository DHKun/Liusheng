#!/usr/bin/env python3
"""Read-only diagnosis of Liusheng's installed binaries, launchers and running processes.

Run in the affected desktop session: python3 scripts/diagnose-icons.py
Uses the Python standard library and optional gdbus/kreadconfig6 commands.
The report contains only application paths, hashes and icon/launcher metadata.
"""
from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess

APP_ID = "io.github.dhkun.Liusheng"
PROJECT = Path(__file__).resolve().parent.parent


def digest(path: Path) -> str | None:
    try:
        with path.open("rb") as stream:
            result = hashlib.sha256()
            while chunk := stream.read(1024 * 1024):
                result.update(chunk)
            return result.hexdigest()
    except OSError:
        return None


def execute(command: list[str]) -> str:
    try:
        result = subprocess.run(command, capture_output=True, text=True, timeout=3)
        return result.stdout.strip() if result.returncode == 0 else ""
    except (OSError, subprocess.TimeoutExpired):
        return ""


def desktop_fields(path: Path) -> dict[str, str]:
    try:
        if path.stat().st_size > 128 * 1024:
            return {}
        text = path.read_text(errors="replace")
    except OSError:
        return {}
    group = ""
    values = {}
    for line in text.splitlines():
        if line.startswith("["):
            group = line.strip()
        elif group == "[Desktop Entry]" and "=" in line:
            key, value = line.split("=", 1)
            if key in ("Name", "Exec", "Icon", "Hidden", "StartupWMClass"):
                values[key] = value
    return values


def main() -> int:
    home = Path.home()
    data_home = Path(os.environ.get("XDG_DATA_HOME", str(home / ".local/share")))
    roots = [data_home, *[Path(p) for p in os.environ.get("XDG_DATA_DIRS", "/usr/local/share:/usr/share").split(":") if p]]
    prefix = Path(os.environ.get("PREFIX", str(home / ".local")))
    build_dir = Path(os.environ.get("CARGO_TARGET_DIR", str(PROJECT / "target")))
    if not build_dir.is_absolute():
        build_dir = Path.cwd() / build_dir
    paths = [prefix / "bin/liusheng", build_dir / "release/liusheng", Path("/usr/bin/liusheng"), Path("/usr/local/bin/liusheng")]
    found = shutil.which("liusheng")
    if found:
        paths.append(Path(found))
    report = {"session": os.environ.get("XDG_SESSION_TYPE", ""),
              "desktop": os.environ.get("XDG_CURRENT_DESKTOP", ""),
              "binaries": [], "processes": [], "desktop_entries": [], "pinned_liusheng_entries": []}
    for path in dict.fromkeys(paths):
        checksum = digest(path)
        if checksum:
            report["binaries"].append({"path": str(path), "sha256": checksum})
    # Inspect only same-user processes whose exact executable name is liusheng.
    try:
        processes = Path("/proc").iterdir()
        for proc in processes:
            try:
                if not proc.name.isdecimal() or proc.stat().st_uid != os.getuid():
                    continue
                if (proc / "comm").read_text().strip() != "liusheng":
                    continue
                executable = os.readlink(proc / "exe")
                checksum = digest(proc / "exe")
                report["processes"].append({"pid": int(proc.name), "exe": executable, "sha256": checksum})
            except OSError:
                continue
    except OSError:
        pass
    for root in dict.fromkeys(roots):
        directory = root / "applications"
        if not directory.is_dir():
            continue
        for index, path in enumerate(directory.glob("*.desktop")):
            if index >= 4096:
                break
            fields = desktop_fields(path)
            if not ("liusheng" in path.name.lower() or "liusheng" in fields.get("Exec", "").lower() or fields.get("Name") == "留声"):
                continue
            icon = fields.get("Icon", "")
            entry = {"path": str(path), **fields}
            if icon.startswith("/"):
                entry["icon_exists"] = Path(icon).is_file()
                entry["icon_sha256"] = digest(Path(icon))
            else:
                entry["lookup"] = "desktop icon theme (may substitute this name)"
            report["desktop_entries"].append(entry)
    source = PROJECT / "crates/liusheng/qml/assets/app-icon/liusheng.svg"
    report["source_icon_sha256"] = digest(source)
    config = Path(os.environ.get("XDG_CONFIG_HOME", str(home / ".config")))
    pinned = config / "plasma-org.kde.plasma.desktop-appletsrc"
    try:
        if pinned.stat().st_size <= 4 * 1024 * 1024:
            for line in pinned.read_text(errors="replace").splitlines():
                if line.startswith("launchers="):
                    report["pinned_liusheng_entries"].extend(value for value in line[10:].split(",") if "liusheng" in value.lower())
    except OSError:
        pass
    if shutil.which("kreadconfig6"):
        report["icon_theme"] = execute(["kreadconfig6", "--file", "kdeglobals", "--group", "Icons", "--key", "Theme"])
    if shutil.which("gdbus"):
        report["mpris_process_owner"] = execute(["gdbus", "call", "--session", "--dest", "org.freedesktop.DBus",
            "--object-path", "/org/freedesktop/DBus", "--method", "org.freedesktop.DBus.GetConnectionUnixProcessID",
            "org.mpris.MediaPlayer2.liusheng"])
    print(json.dumps(report, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
