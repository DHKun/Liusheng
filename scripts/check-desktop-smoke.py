#!/usr/bin/env python3
"""Load the actual desktop executable's initial QML scene and exit in isolation.

Usage: python3 scripts/check-desktop-smoke.py /path/to/liusheng [--output LOG]
Linux uses offscreen; macOS uses its native Cocoa plugin, including for deployed
.app bundles. Software rendering keeps this startup test independent of a GPU.
Audio and physical desktop UX have dedicated platform tests and device acceptance.
"""
from __future__ import annotations

import argparse
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile


# Developer Qt paths can conceal deployment omissions or select a foreign plugin.
QT_OVERRIDES = (
    "QT_QPA_PLATFORM", "QT_QPA_PLATFORM_PLUGIN_PATH", "QT_QPA_PLATFORMTHEME",
    "QT_QPA_GENERIC_PLUGINS", "QT_PLUGIN_PATH", "QML_IMPORT_PATH", "QML2_IMPORT_PATH",
    "QML_DISK_CACHE_PATH", "QT_QUICK_CONTROLS_CONF", "QT_QUICK_CONTROLS_STYLE",
    "QT_QUICK_BACKEND", "QSG_RHI_BACKEND", "QT_LOGGING_RULES", "QT_DEBUG_PLUGINS",
)
ERRORS = (
    "failed to load component", "is not a type", "Cannot assign", "ReferenceError:",
    "TypeError:", "Binding loop", "Required property", "Unable to assign", "Error decoding:",
    "Library not loaded:", "Symbol not found:", "Could not load the Qt platform plugin",
    "Could not find the Qt platform plugin", "no Qt platform plugin could be initialized",
)


def smoke_environment(binary: Path, root: Path, platform: str = "auto",
                      debug_plugins: bool = False) -> dict[str, str]:
    if platform == "auto":
        platform = "cocoa" if sys.platform == "darwin" else "offscreen"
    if platform not in {"cocoa", "offscreen"}:
        raise ValueError(f"Unsupported smoke platform: {platform}")
    if platform == "cocoa" and sys.platform != "darwin":
        raise ValueError("Cocoa smoke requires a native macOS runner")
    env = os.environ.copy()
    for name in QT_OVERRIDES:
        env.pop(name, None)
    for name in ("home", "config", "cache", "data", "runtime"):
        (root / name).mkdir(mode=0o700)
    env.update(HOME=str(root / "home"), XDG_CONFIG_HOME=str(root / "config"),
               XDG_CACHE_HOME=str(root / "cache"), XDG_DATA_HOME=str(root / "data"),
               XDG_RUNTIME_DIR=str(root / "runtime"), QT_QPA_PLATFORM=platform,
               QT_QUICK_BACKEND="software", LIUSHENG_ALLOW_MULTIPLE="1", LANG="C.UTF-8")
    if debug_plugins:
        env["QT_DEBUG_PLUGINS"] = "1"
    contents = binary.parent.parent.resolve(strict=True)
    bundled = (binary.parent.name == "MacOS" and contents.name == "Contents"
               and contents.parent.suffix.lower() == ".app")
    if sys.platform == "darwin" and bundled:
        if platform != "cocoa":
            raise ValueError("Packaged macOS startup must validate the Cocoa runtime plugin")
        # Keep the signed bundle unchanged. macdeployqt's relative qt.conf owns
        # plugin/QML lookup; clearing environment overrides exposes missing files.
        for relative in ("PlugIns/platforms/libqcocoa.dylib", "Resources/qt.conf"):
            resource = contents / relative
            if (not resource.is_file() or resource.stat().st_size == 0
                    or not resource.resolve().is_relative_to(contents)):
                raise ValueError(f"Missing or external macOS bundle resource: {resource}")
        for name in list(env):
            if name.startswith("DYLD_"):
                env.pop(name)
    return env


def run_smoke(binary: Path, output_path: Path | None = None, platform: str = "auto",
              debug_plugins: bool = False, timeout: float = 25) -> None:
    binary = binary.resolve(strict=True)
    if not binary.is_file() or not os.access(binary, os.X_OK):
        raise ValueError(f"Desktop executable is missing or not executable: {binary}")
    transcript = f"Desktop smoke: {binary}\nHost: {sys.platform}\n"
    try:
        with tempfile.TemporaryDirectory(prefix="liusheng-desktop-smoke-") as temporary:
            root = Path(temporary)
            env = smoke_environment(binary, root, platform, debug_plugins)
            transcript += f"QPA: {env['QT_QPA_PLATFORM']}\nRenderer: software\n"
            command = [str(binary), "--smoke-test", "--no-update-check", "--no-online-metadata"]
            with subprocess.Popen(command, cwd=root, env=env, stdout=subprocess.PIPE,
                                  stderr=subprocess.STDOUT, text=True, encoding="utf-8",
                                  errors="replace", start_new_session=True) as process:
                try:
                    output, _ = process.communicate(timeout=timeout)
                except subprocess.TimeoutExpired:
                    try:
                        os.killpg(process.pid, signal.SIGKILL)
                    except ProcessLookupError:
                        pass
                    output, _ = process.communicate()
                    transcript += output
                    raise RuntimeError(f"Desktop smoke timed out after {timeout:g}s:\n{output}") from None
            transcript += output
            if process.returncode or any(error.lower() in output.lower() for error in ERRORS):
                raise RuntimeError(f"Desktop smoke failed ({process.returncode}):\n{output}")
            transcript += "Desktop QML startup and clean exit passed\n"
    except (OSError, ValueError, RuntimeError, subprocess.SubprocessError) as error:
        transcript += f"SMOKE CHECK FAILED: {error}\n"
        raise
    finally:
        if output_path:
            output_path.parent.mkdir(parents=True, exist_ok=True)
            output_path.write_text(transcript, encoding="utf-8")
    print(f"Desktop QML startup and clean exit passed on {sys.platform} "
          f"(QPA={env['QT_QPA_PLATFORM']}): {binary}")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", type=Path)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--platform", choices=("auto", "offscreen", "cocoa"), default="auto",
                        help="auto selects Cocoa on macOS and offscreen on Linux")
    parser.add_argument("--debug-plugins", action="store_true",
                        help="retain Qt plugin discovery diagnostics in the output log")
    args = parser.parse_args()
    run_smoke(args.binary, args.output, args.platform, args.debug_plugins)
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, ValueError, RuntimeError, subprocess.SubprocessError) as error:
        print(f"SMOKE CHECK FAILED: {error}", file=sys.stderr)
        raise SystemExit(1)
