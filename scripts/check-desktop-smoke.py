#!/usr/bin/env python3
"""Load the actual desktop executable's initial QML scene and exit in isolation.

Usage: python3 scripts/check-desktop-smoke.py /path/to/liusheng [--output LOG]
Runs on Linux and macOS with the native binary and Qt's offscreen platform plugin.
This checks loading/linking/QML initialization; audio and physical desktop UX are
covered by the dedicated platform tests and real-device acceptance.
"""
from __future__ import annotations

import argparse
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", type=Path)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    binary = args.binary.resolve(strict=True)
    with tempfile.TemporaryDirectory(prefix="liusheng-desktop-smoke-") as temporary:
        root = Path(temporary)
        for name in ("home", "config", "cache", "data", "runtime"):
            (root / name).mkdir(mode=0o700)
        env = os.environ.copy()
        env.update(HOME=str(root / "home"), XDG_CONFIG_HOME=str(root / "config"),
                   XDG_CACHE_HOME=str(root / "cache"), XDG_DATA_HOME=str(root / "data"),
                   XDG_RUNTIME_DIR=str(root / "runtime"), QT_QPA_PLATFORM="offscreen",
                   QT_QUICK_BACKEND="software", LIUSHENG_ALLOW_MULTIPLE="1", LANG="C.UTF-8")
        command = [str(binary), "--smoke-test"]
        with subprocess.Popen(command, env=env, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                              text=True, encoding="utf-8", start_new_session=True) as process:
            try:
                output, _ = process.communicate(timeout=25)
            except subprocess.TimeoutExpired:
                os.killpg(process.pid, signal.SIGKILL)
                output, _ = process.communicate()
                if args.output:
                    args.output.parent.mkdir(parents=True, exist_ok=True)
                    args.output.write_text(output, encoding="utf-8")
                raise RuntimeError(f"Desktop smoke timed out:\n{output}") from None
        if args.output:
            args.output.parent.mkdir(parents=True, exist_ok=True)
            args.output.write_text(output, encoding="utf-8")
        errors = ("failed to load component", "is not a type", "Cannot assign", "ReferenceError:",
                  "TypeError:", "Binding loop", "Required property", "Unable to assign", "Error decoding:",
                  "Library not loaded:", "Could not load the Qt platform plugin")
        if process.returncode or any(error.lower() in output.lower() for error in errors):
            raise RuntimeError(f"Desktop smoke failed ({process.returncode}):\n{output}")
    print(f"Desktop QML startup and clean exit passed on {sys.platform}: {binary}")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, RuntimeError, subprocess.SubprocessError) as error:
        print(f"SMOKE CHECK FAILED: {error}", file=sys.stderr)
        raise SystemExit(1)
