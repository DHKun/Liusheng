#!/usr/bin/env python3
"""Run hardware-free developer CLI smoke tests on native Linux and macOS.

Build: cargo build --locked -p liusheng-core --example dev
Usage: python3 scripts/check-dev-cli.py target/debug/examples/dev
Only the supplied executable is run; files and the CLI's scan databases are temporary.
"""
from __future__ import annotations

import argparse
from pathlib import Path
import subprocess
import sys
import tempfile
import wave


def invoke(binary: Path, arguments: list[str], *, succeeds: bool = True) -> str:
    result = subprocess.run([str(binary), *arguments], text=True, encoding="utf-8",
                            stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=20)
    if (result.returncode == 0) != succeeds or "panicked at" in result.stdout:
        raise RuntimeError(f"Command {arguments!r} exited {result.returncode}:\n{result.stdout}")
    return result.stdout


def write_wav(path: Path, bits: int) -> bytes:
    samples = [-(1 << (bits - 1)), (1 << (bits - 1)) - 1, -123, 456, 0, 0] * 80
    pcm = b"".join(value.to_bytes(bits // 8, "little", signed=True) for value in samples)
    with wave.open(str(path), "wb") as writer:
        writer.setnchannels(2)
        writer.setsampwidth(bits // 8)
        writer.setframerate(8_000)
        writer.writeframes(pcm)
    return pcm


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", type=Path)
    args = parser.parse_args()
    binary = args.binary.resolve(strict=True)
    expected_backend = {"darwin": "CoreAudio", "linux": "PipeWire"}.get(sys.platform)
    if expected_backend is None:
        parser.error("This project supports Linux and macOS")
    help_text = invoke(binary, ["--help"])
    if expected_backend not in help_text:
        raise RuntimeError(f"Help must advertise {expected_backend}:\n{help_text}")
    for arguments in (["decode"], ["play"], ["unknown-command"]):
        invoke(binary, arguments, succeeds=False)
    checks = 4
    if sys.platform == "darwin":
        for command in ("alsa-probe", "volume-probe"):
            output = invoke(binary, [command], succeeds=False)
            if "Linux ALSA" not in output:
                raise RuntimeError(f"Missing platform explanation for {command}: {output}")
            if command in help_text:
                raise RuntimeError(f"Linux-only {command} advertised as available on macOS")
            checks += 1
    with tempfile.TemporaryDirectory(prefix="liusheng-dev-cli-") as temporary:
        root = Path(temporary)
        music = root / "音乐 with spaces"
        music.mkdir()
        for bits in (16, 24):
            source = music / f"sample {bits} bit.wav"
            output = root / f"output {bits}.wav"
            expected = write_wav(source, bits)
            invoke(binary, ["decode", str(source), str(output)])
            with wave.open(str(output), "rb") as reader:
                if (reader.getnchannels(), reader.getsampwidth(), reader.getframerate()) != (2, bits // 8, 8_000):
                    raise RuntimeError(f"Changed WAV format for {bits}-bit input")
                if reader.readframes(reader.getnframes()) != expected:
                    raise RuntimeError(f"Sample mismatch for {bits}-bit input")
            checks += 1
        scan = invoke(binary, ["scan", str(music)])
        if "sample 16 bit" not in scan or "sample 24 bit" not in scan:
            raise RuntimeError(f"Both fixtures should appear in the scan:\n{scan}")
        search = invoke(binary, ["search", str(music), "sample"])
        if "sample 16 bit" not in search:
            raise RuntimeError(f"Search should return the generated fixture:\n{search}")
        checks += 2
    print(f"Developer CLI: {checks} checks passed on {sys.platform} ({expected_backend}); no audio device opened")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, RuntimeError, subprocess.SubprocessError) as error:
        print(f"CLI CHECK FAILED: {error}", file=sys.stderr)
        raise SystemExit(1)
