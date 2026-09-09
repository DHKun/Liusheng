#!/usr/bin/env python3
"""Compile and exercise the production MediaPlayer adapter in an isolated arm64 AppKit bundle.

Requires an Apple Silicon Mac with the Xcode command line tools. No Qt, audio
hardware, private APIs, or user's music files are required. Logs are retained;
the test process removes its command targets and metadata before exiting.
"""
from __future__ import annotations
import argparse
import os
from pathlib import Path
import platform
import plistlib
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parent.parent


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=ROOT / "target/qa/macos-media")
    args = parser.parse_args()
    if platform.system() != "Darwin" or platform.machine() != "arm64":
        parser.error("Run the native test on macOS arm64")
    args.output.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="liusheng-media-") as directory:
        root = Path(directory)
        contents = root / "MediaSmoke.app/Contents"
        (contents / "MacOS").mkdir(parents=True)
        executable = contents / "MacOS/MediaSmoke"
        (contents / "Info.plist").write_bytes(plistlib.dumps({
            "CFBundleIdentifier": "io.github.dhkun.Liusheng.MediaSmoke",
            "CFBundleName": "Liusheng media test", "CFBundleExecutable": "MediaSmoke",
            "CFBundlePackageType": "APPL", "LSUIElement": True,
        }))
        env = dict(os.environ, MACOSX_DEPLOYMENT_TARGET="13.0")
        command = ["xcrun", "clang++", "-std=c++17", "-fobjc-arc", "-fblocks", "-Wall", "-Wextra", "-Werror",
                   "-arch", "arm64", str(ROOT / "tests/native/macos_media_test.mm"), "-o", str(executable)]
        for framework in ("Foundation", "AppKit", "MediaPlayer", "ImageIO", "CoreGraphics"):
            command.extend(["-framework", framework])
        for name, invocation in (("compile", command), ("run", [str(executable)])):
            with (args.output / f"{name}.log").open("w") as log:
                try:
                    subprocess.run(invocation, env=env, stdout=log, stderr=subprocess.STDOUT,
                                   check=True, timeout=90 if name == "compile" else 30)
                except subprocess.SubprocessError:
                    log.flush()
                    print((args.output / f"{name}.log").read_text())
                    raise
    print("Native MediaPlayer metadata, commands, artwork and teardown passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
