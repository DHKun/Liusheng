#!/usr/bin/env python3
"""Isolated Qt/QML, playlists, session and single-instance regression checks.

Usage: python3 scripts/check-ui.py /absolute/path/to/liusheng [--output DIR]
Requires Python 3, dbus-run-session and gdbus on Linux. No user music/data is read.
"""
from __future__ import annotations
import argparse
import json
import os
from pathlib import Path
import shutil
import signal
import sqlite3
import struct
import subprocess
import sys
import tempfile
import time
import wave
import zlib

BUS = "org.mpris.MediaPlayer2.io.github.dhkun.Liusheng"
OBJECT = "/org/mpris/MediaPlayer2"


def run(command: list[str], env: dict[str, str], timeout: float = 65) -> str:
    with subprocess.Popen(command, env=env, text=True, stdout=subprocess.PIPE,
                          stderr=subprocess.STDOUT, start_new_session=True) as process:
        try:
            output, _ = process.communicate(timeout=timeout)
        except subprocess.TimeoutExpired:
            # This process group contains only the isolated test invocation.
            os.killpg(process.pid, signal.SIGKILL)
            output, _ = process.communicate()
            raise RuntimeError(f"Timed out after {timeout}s: {command}\n{output}") from None
        if process.returncode:
            raise RuntimeError(f"Exit {process.returncode}: {command}\n{output}")
        return output


def inspect_log(text: str) -> None:
    errors = ("ReferenceError:", "TypeError:", "Binding loop detected", "is not a type",
              "Cannot assign", "Required property", "failed to load component", "FUNCTIONAL FAIL", "Error decoding:", "Unsupported image format", "Unable to assign", "Failed to create grabbing popup", "QMenuClassWindow", "No transient parent", "Binding loop")
    for error in errors:
        if error.lower() in text.lower():
            raise RuntimeError(f"QML diagnostic: {error}\n{text}")


def png(path: Path) -> None:
    def chunk(kind: bytes, data: bytes) -> bytes:
        return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data))
    width = height = 64
    pixels = b"".join(b"\0" + bytes([150 + y, 80, 110]) * width for y in range(height))
    path.write_bytes(b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0))
                    + chunk(b"IDAT", zlib.compress(pixels)) + chunk(b"IEND", b""))


def fixture(root: Path) -> dict[str, str]:
    for name in ("home", "config/liusheng", "data/liusheng", "cache", "runtime", "music", "screens"):
        (root / name).mkdir(parents=True, exist_ok=True)
    (root / "runtime").chmod(0o700)
    (root / ".liusheng-test-workspace").touch()
    settings = {"version": 1, "music_roots": [str(root / "music")],
                "close_to_tray": False, "restore_session": True}
    (root / "config/liusheng/settings.json").write_text(json.dumps(settings))
    for number in range(1, 6):
        with wave.open(str(root / f"music/Track {number}.wav"), "wb") as out:
            out.setnchannels(2)
            out.setsampwidth(2)
            out.setframerate(48000)
            out.writeframes(struct.pack("<hh", 1000, -1000) * 96000)
    (root / "music/Track 1.lrc").write_text("[00:00.00]开始\n[00:00.00]Beginning\n[00:00.20]同步歌词\n[00:00.20]Synchronized lyrics\n", encoding="utf-8")
    png(root / "music/cover.png")
    (root / "online-test.lrc").write_text("[00:00.00]Online resource fixture\n[00:00.20]Second resource line\n", encoding="utf-8")
    (root / "word-timing.ttml").write_text('<tt xmlns:itunes="http://music.apple.com/lyric-ttml-internal" itunes:timing="Word"><body><p begin="0s" end="2s"><span begin="0s" end="0.6s">Word </span><span begin="1s" end="2s">timing fixture</span></p></body></tt>', encoding="utf-8")
    env = os.environ.copy()
    env.update(HOME=str(root / "home"), XDG_CONFIG_HOME=str(root / "config"),
               XDG_DATA_HOME=str(root / "data"), XDG_CACHE_HOME=str(root / "cache"),
               XDG_RUNTIME_DIR=str(root / "runtime"), PIPEWIRE_RUNTIME_DIR=str(root / "runtime"),
               QT_QPA_PLATFORM=os.environ.get("LIUSHENG_TEST_PLATFORM", "offscreen"), QT_QUICK_BACKEND="software", LANG="C.UTF-8",
               LIUSHENG_QA_DIR=str(root), LIUSHENG_SCREENSHOT_DIR=str(root / "screens"),
               LIUSHENG_PROFILE="1")
    env.pop("LIUSHENG_ALLOW_MULTIPLE", None)
    return env


def dbus(method: str, *args: str) -> list[str]:
    return ["gdbus", "call", "--session", "--dest", BUS, "--object-path", OBJECT,
            "--method", method, *args]


def ipc_check(binary: str, root: Path) -> None:
    env = os.environ.copy()
    with (root / "ipc-primary.log").open("w") as log:
        primary = subprocess.Popen([binary], env=env, stdout=log, stderr=subprocess.STDOUT)
        try:
            deadline = time.monotonic() + 8
            while True:
                try:
                    run(dbus("org.freedesktop.DBus.Properties.Get", "org.mpris.MediaPlayer2.Player", "PlaybackStatus"), env, 1)
                    break
                except (RuntimeError, subprocess.TimeoutExpired):
                    if primary.poll() is not None or time.monotonic() > deadline:
                        raise RuntimeError("Primary instance did not initialize MPRIS")
                    time.sleep(0.1)
            time.sleep(0.4)
            # An invocation with no files should raise the existing instance and exit quickly.
            started = time.monotonic()
            secondary = run([binary], env, 5)
            assert time.monotonic() - started < 5
            inspect_log(secondary)
            status = run(dbus("org.freedesktop.DBus.Properties.Get", "org.mpris.MediaPlayer2.Player", "PlaybackStatus"), env)
            assert "Paused" in status, status
            run(dbus("org.mpris.MediaPlayer2.Player.Next"), env)
            run(dbus("org.freedesktop.DBus.Properties.Set", "org.mpris.MediaPlayer2.Player", "LoopStatus", "<'Track'>"), env)
            run(dbus("org.freedesktop.DBus.Properties.Set", "org.mpris.MediaPlayer2.Player", "Shuffle", "<false>"), env)
            time.sleep(0.3)
            run(dbus("org.mpris.MediaPlayer2.Quit"), env)
            primary.wait(timeout=5)
            assert primary.returncode == 0
        finally:
            if primary.poll() is None:
                primary.terminate()
                try:
                    primary.wait(timeout=3)
                except subprocess.TimeoutExpired:
                    primary.kill()
                    primary.wait()
    inspect_log((root / "ipc-primary.log").read_text())
    session = json.loads((root / "data/liusheng/session.json").read_text())
    assert session["repeat_mode"] == 1 and session["shuffle"] is False, session
    print("Single-instance + MPRIS paused controls + graceful session save passed")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", type=Path)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--ipc-child", type=Path, help=argparse.SUPPRESS)
    args = parser.parse_args()
    binary = str(args.binary.resolve(strict=True))
    if args.ipc_child:
        ipc_check(binary, args.ipc_child)
        return 0
    for program in ("dbus-run-session", "gdbus"):
        if not shutil.which(program):
            parser.error(f"Required program: {program}")
    with tempfile.TemporaryDirectory(prefix="liusheng-ui-") as directory:
        root = Path(directory)
        env = fixture(root)
        for phase, flag in (("pages", "--ui-test"), ("functional", "--functional-test")):
            if phase == "functional":
                session = {"version": 1, "queue": [str(root / f"music/Track {n}.wav") for n in range(1, 4)],
                           "current_index": 0, "position_ms": 100, "page": "albums"}
                (root / "data/liusheng/session.json").write_text(json.dumps(session))
            phase_env = env.copy()
            trace_minimize = phase == "functional" and env["QT_QPA_PLATFORM"].startswith("wayland")
            if trace_minimize:
                phase_env["WAYLAND_DEBUG"] = "client"
            text = run(["dbus-run-session", "--", binary, flag], phase_env)
            if trace_minimize:
                assert ".set_minimized()" in text and "xdg_toplevel" in text, "Missing actual Wayland minimize request"
            (root / f"{phase}.log").write_text(text)
            if args.output:
                args.output.mkdir(parents=True, exist_ok=True)
                (args.output / f"{phase}.log").write_text(text)
                shutil.copytree(root / "screens", args.output / "screens", dirs_exist_ok=True)
            inspect_log(text)
            assert ("UI validation passed" if phase == "pages" else "Functional validation passed") in text, text
            print(f"{phase}: passed")
        for capture in ("40-imported-lyrics-line.png", "41-local-lyrics-line.png"):
            assert (root / "screens" / capture).is_file(), f"Missing lyric integration screenshot: {capture}"
        conn = sqlite3.connect(root / "data/liusheng/library.db")
        assert conn.execute("SELECT COUNT(*) FROM tracks").fetchone()[0] == 5
        assert conn.execute("SELECT name FROM playlists").fetchall() == [("QA renamed",)]
        assert conn.execute("SELECT COUNT(*) FROM playlist_entries").fetchone()[0] == 2
        conn.close()
        session = json.loads((root / "data/liusheng/session.json").read_text())
        assert len(session["queue"]) == 2 and session["repeat_mode"] == 2 and session["shuffle"]
        assert (root / "export.m3u8").read_text().startswith("#EXTM3U\n")
        settings = json.loads((root / "config/liusheng/settings.json").read_text())
        assert settings["lyric_offsets"][str(root / "music/Track 1.wav")] == 100
        thumbnails = list((root / "cache/liusheng/covers").glob("thumb-*.jpg"))
        assert thumbnails, "Visible artwork did not produce a thumbnail"
        text = run(["dbus-run-session", "--", sys.executable, str(Path(__file__).resolve()), binary,
                    "--ipc-child", str(root)], env)
        (root / "ipc.log").write_text(text)
        print(text.strip())
        if args.output:
            args.output.mkdir(parents=True, exist_ok=True)
            for file in root.glob("*.log"):
                shutil.copy2(file, args.output / file.name)
            shutil.copytree(root / "screens", args.output / "screens", dirs_exist_ok=True)
        print("All isolated UI checks passed; source music fixtures and temporary accounts removed")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (AssertionError, RuntimeError, subprocess.SubprocessError) as exc:
        print(f"CHECK FAILED: {exc}", file=sys.stderr)
        raise SystemExit(1)
