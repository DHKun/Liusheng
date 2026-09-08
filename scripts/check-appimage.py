#!/usr/bin/env python3
"""Extract, relocate and execute an AppImage without FUSE or user music access."""
from __future__ import annotations

import argparse
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parent.parent


def load_packager():
    spec = importlib.util.spec_from_file_location("appimage_packaging", ROOT / "scripts/package-appimage.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def check_alsa_configuration(appdir: Path, work: Path, env: dict[str, str]) -> None:
    """Exercise this AppRun's inherited environment with the bundled ALSA library.

    A temporary probe executable opens ALSA's null PCM, requiring no audio device.
    The original extracted application and published image remain untouched.
    """
    probe = work / "audio probe with spaces"
    (probe / "usr/bin").mkdir(parents=True)
    (probe / "usr/lib").symlink_to(appdir / "usr/lib", target_is_directory=True)
    (probe / "usr/share").symlink_to(appdir / "usr/share", target_is_directory=True)
    shutil.copy2(appdir / "AppRun", probe / "AppRun")
    executable = probe / "usr/bin/liusheng"
    executable.write_text("#!/usr/bin/env python3\n" + """
import ctypes, os
from pathlib import Path
lib = ctypes.CDLL(str(Path(os.environ['APPDIR']) / 'usr/lib/libasound.so.2'))
lib.snd_config_update.restype = ctypes.c_int
assert lib.snd_config_update() >= 0, 'ALSA config could not load'
lib.snd_pcm_open.argtypes = [ctypes.POINTER(ctypes.c_void_p), ctypes.c_char_p, ctypes.c_int, ctypes.c_int]
lib.snd_pcm_open.restype = ctypes.c_int
lib.snd_pcm_close.argtypes = [ctypes.c_void_p]
handle = ctypes.c_void_p()
result = lib.snd_pcm_open(ctypes.byref(handle), b'null', 0, 0)
assert result == 0, f'ALSA null PCM failed: {result}'
lib.snd_pcm_close(handle)
print('Bundled ALSA config and null PCM passed with a spaced AppDir path')
""")
    executable.chmod(0o755)
    probe_env = dict(env)
    probe_env.pop("ALSA_CONFIG_PATH", None)
    probe_env.pop("ALSA_CONFIG_DIR", None)
    subprocess.run([str(probe / "AppRun")], check=True, env=probe_env, timeout=15)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("image", type=Path)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--full-ui", action="store_true")
    parser.add_argument("--wayland", action="store_true")
    args = parser.parse_args()
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps({"image": args.image.name, "passed": False, "status": "started"}) + "\n")
    image = args.image.resolve(strict=True)
    with image.open("rb") as source:
        header = source.read(20)
    if header[:4] != b"\x7fELF" or header[8:11] != b"AI\x02" or header[18:20] != b"\x3e\x00":
        raise ValueError("Expected a type-2 x86_64 AppImage")
    packager = load_packager()
    report = {"image": image.name, "sha256": packager.digest(image), "passed": False}
    with tempfile.TemporaryDirectory(prefix="liusheng-appimage-check-") as temporary:
        work = Path(temporary)
        subprocess.run([str(image), "--appimage-extract"], cwd=work, check=True,
                       stdout=subprocess.DEVNULL, timeout=90)
        extracted = work / "squashfs-root"
        relocated = work / "relocated path with spaces" / "Music.AppDir"
        relocated.parent.mkdir()
        extracted.rename(relocated)
        packager.validate_appdir(relocated)
        env = os.environ.copy()
        # An incompatible desktop Qt installation must not override bundled Qt.
        env.update(QT_PLUGIN_PATH="/invalid-host-qt/plugins", QML_IMPORT_PATH="/invalid-host-qt/qml",
                   QML2_IMPORT_PATH="/invalid-host-qt/qml", APPIMAGE_EXTRACT_AND_RUN="1")
        env.pop("APPIMAGE", None)
        env.pop("APPDIR", None)
        ldd_env = dict(env, LD_LIBRARY_PATH=str(relocated / "usr/lib"))
        dependencies = subprocess.run(["ldd", str(relocated / "usr/bin/liusheng")], env=ldd_env,
                                      check=True, text=True, capture_output=True).stdout
        if "not found" in dependencies:
            raise ValueError(f"Unresolved packaged dependencies:\n{dependencies}")
        for line in dependencies.splitlines():
            if any(lib in line for lib in ("libQt6", "libpipewire-", "libasound.")) and str(relocated) not in line:
                raise ValueError(f"A required library resolves outside the package: {line}")
        # Every explicit Qt/audio/QML module must have its link dependencies.
        for directory in ("usr/plugins", "usr/qml", "usr/lib/spa-0.2", "usr/lib/pipewire-0.3"):
            for plugin in (relocated / directory).rglob("*.so"):
                result = subprocess.run(["ldd", str(plugin)], env=ldd_env, text=True, capture_output=True)
                if result.returncode or "not found" in result.stdout:
                    raise ValueError(f"Unresolved plugin dependencies: {plugin}\n{result.stdout}{result.stderr}")
        check_alsa_configuration(relocated, work, env)
        report["alsa_configuration"] = True
        subprocess.run([sys.executable, str(ROOT / "scripts/check-desktop-smoke.py"), str(relocated / "AppRun")],
                       check=True, env=env, cwd=work, timeout=40)
        # Exercise the actual self-extracting runtime, not just its extracted ELF.
        copied = work / "Liusheng portable player.AppImage"
        shutil.copy2(image, copied)
        subprocess.run([sys.executable, str(ROOT / "scripts/check-desktop-smoke.py"), str(copied)],
                       check=True, env=env, cwd=work, timeout=90)
        diagnostics = args.output.resolve().parent if args.output else ROOT / "target/qa/appimage"
        if args.full_ui:
            subprocess.run([sys.executable, str(ROOT / "scripts/check-ui.py"), str(relocated / "AppRun"),
                            "--output", str(diagnostics / "ui")], check=True, env=env, timeout=180)
            report["full_ui"] = True
        if args.wayland:
            subprocess.run([sys.executable, str(ROOT / "scripts/check-wayland.py"), str(relocated / "AppRun"),
                            "--output", str(diagnostics / "wayland")], check=True, env=env, timeout=600)
            report["wayland_scales"] = [1.0, 1.25, 1.5]
        report.update(passed=True, relocated_startup=True, runtime_extract_and_run=True,
                      build=json.loads((relocated / "usr/share/liusheng/build-info.json").read_text()))
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report, indent=2))
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        print(f"AppImage validation failed: {error}", file=sys.stderr)
        raise SystemExit(1)
