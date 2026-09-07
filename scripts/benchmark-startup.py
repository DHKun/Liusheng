#!/usr/bin/env python3
"""Measure cached startup with synthetic, offline SQLite libraries in an isolated home.

Usage: python3 scripts/benchmark-startup.py ./target/release/liusheng --runs 7
Metrics describe this process/platform/backend, not the user's desktop or audible latency.
"""
from __future__ import annotations
import argparse
import json
import os
from pathlib import Path
import re
import sqlite3
import statistics
import subprocess
import tempfile
import time


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", type=Path)
    parser.add_argument("--runs", type=int, default=7)
    parser.add_argument("--sizes", type=int, nargs="+", default=[1000, 10000])
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    if not 1 <= args.runs <= 50 or any(not 0 <= size <= 100000 for size in args.sizes):
        parser.error("Use 1–50 runs and 0–100000 tracks per library")
    binary = str(args.binary.resolve(strict=True))
    rows = []
    with tempfile.TemporaryDirectory(prefix="liusheng-benchmark-") as directory:
        base = Path(directory)
        for name in ("home", "config/liusheng", "data", "cache", "runtime"):
            (base / name).mkdir(parents=True, exist_ok=True)
        (base / "runtime").chmod(0o700)
        settings = {"version": 1, "music_roots": [str(base / "offline")],
                    "restore_session": False, "close_to_tray": False}
        (base / "config/liusheng/settings.json").write_text(json.dumps(settings))
        env = os.environ.copy()
        env.update(HOME=str(base / "home"), XDG_CONFIG_HOME=str(base / "config"),
                   XDG_DATA_HOME=str(base / "data"), XDG_CACHE_HOME=str(base / "cache"),
                   XDG_RUNTIME_DIR=str(base / "runtime"), QT_QPA_PLATFORM="offscreen",
                   QT_QUICK_BACKEND="software", LIUSHENG_ALLOW_MULTIPLE="1", LIUSHENG_PROFILE="1", LANG="C.UTF-8")
        command = ["dbus-run-session", "--", binary, "--startup-benchmark"]

        def measure() -> dict[str, float]:
            start = time.perf_counter()
            result = subprocess.run(command, env=env, text=True, stdout=subprocess.PIPE,
                                    stderr=subprocess.STDOUT, timeout=30)
            if result.returncode:
                raise RuntimeError(result.stdout)
            value: dict[str, float] = {"process_wall_ms": (time.perf_counter() - start) * 1000}
            for key in ("first_frame_ms", "interactive_ms", "cached_library_ms"):
                found = re.search(r"\[perf\]\s+" + key + r"[=\s]+([0-9.]+)", result.stdout)
                if not found:
                    raise RuntimeError(f"Missing {key}:\n{result.stdout}")
                value[key] = float(found.group(1))
            return value

        measure()  # Create and migrate an empty database, and warm Qt/font caches.
        conn = sqlite3.connect(base / "data/liusheng/library.db")
        for count in args.sizes:
            conn.execute("DELETE FROM tracks")
            values = []
            for index in range(count):
                album = f"Album {index // 10:05d}"
                artist = f"Artist {index // 100:04d}"
                title = f"Track {index:06d}"
                values.append((str(base / f"offline/{index}.flac"), 0, 10000, title, artist, album,
                               artist, index % 10 + 1, 240000, 48000, 24, 2,
                               title.lower(), artist.lower(), album.lower()))
            conn.executemany("INSERT INTO tracks(path,mtime,file_size,title,artist,album,album_artist,track_no,"
                             "duration_ms,sample_rate,bit_depth,channels,title_search,artist_search,album_search) "
                             "VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)", values)
            conn.commit()
            measure()  # Warm the cache for this dataset; cold filesystem startup is a separate metric.
            samples = [measure() for _ in range(args.runs)]
            medians = {key: round(statistics.median(sample[key] for sample in samples), 2) for key in samples[0]}
            result = {"tracks": count, "albums": (count + 9) // 10, "runs": args.runs,
                      "warm_median_ms": medians, "samples": samples}
            rows.append(result)
            print(f"{count} tracks: {medians}")
        conn.close()
    report = {"binary": binary, "backend": "Qt offscreen/software", "library": "synthetic cached metadata; music root offline",
              "timing_origin": "first_frame/interactive measured from DesktopBridge creation; process_wall includes dbus-run-session and teardown",
              "comparison": "No pre-upgrade measurements; no speedup ratio is inferred", "results": rows}
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(report, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
