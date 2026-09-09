#!/usr/bin/env python3
"""Build a relocatable Linux x86_64 AppImage with Qt/QML and audio client modules.

Use Debian 13 for official builds (glibc 2.41 baseline). Tools and the embedded
runtime are release-pinned and SHA-256 checked. FUSE is optional during packaging.
Usage: python3 scripts/package-appimage.py [--no-build] [--version X.Y.Z]
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import shutil
import subprocess
import sys
import tempfile
import tomllib

ROOT = Path(__file__).resolve().parent.parent
DESKTOP_ID = "io.github.dhkun.Liusheng"
# Desktop clients must carry these even when linuxdeploy lists them as host libraries.
FORCED_CLIENT_LIBRARIES = ("libasound.so.2", "libpipewire-0.3.so.0", "libharfbuzz.so.0",
                           "libcom_err.so.2", "libSM.so.6", "libICE.so.6", "libssl.so.3", "libcrypto.so.3")


def run(command: list[str], **kwargs) -> subprocess.CompletedProcess:
    return subprocess.run(command, check=True, **kwargs)


def output(command: list[str]) -> str:
    return run(command, text=True, stdout=subprocess.PIPE).stdout.strip()


def digest(path: Path) -> str:
    with path.open("rb") as source:
        return hashlib.file_digest(source, "sha256").hexdigest()


def version_for(root: Path, requested: str | None) -> str:
    version = tomllib.loads((root / "crates/liusheng/Cargo.toml").read_text())["package"]["version"]
    if not re.fullmatch(r"[0-9]+\.[0-9]+\.[0-9]+", version):
        raise ValueError("Application version must be X.Y.Z")
    if requested and requested != version:
        raise ValueError(f"Requested {requested}, application is {version}")
    return version


def check_elf(path: Path) -> None:
    with path.open("rb") as source:
        header = source.read(20)
    if header[:6] != b"\x7fELF\x02\x01" or header[18:20] != b"\x3e\x00":
        raise ValueError(f"Expected Linux x86_64 ELF: {path}")
    if not os.access(path, os.X_OK):
        raise ValueError(f"Executable permission required: {path}")


def verified_tools(root: Path, cache: Path) -> dict[str, Path]:
    entries = json.loads((root / "packaging/appimage/tools.lock.json").read_text())
    cache.mkdir(parents=True, exist_ok=True)
    result = {}
    for name, entry in entries.items():
        if not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9_.-]*", name) or not re.fullmatch(r"[0-9a-f]{64}", entry["sha256"]):
            raise ValueError("Invalid tool lock entry")
        if not entry["url"].startswith("https://github.com/"):
            raise ValueError("Tool download must use the locked official HTTPS release")
        path = cache / name
        if path.is_symlink():
            raise ValueError(f"Tool cache must contain regular files: {name}")
        if not path.exists():
            with tempfile.TemporaryDirectory(prefix="download-", dir=cache) as temporary:
                temporary_file = Path(temporary) / name
                run(["curl", "--fail", "--location", "--proto", "=https", "--proto-redir", "=https", "--tlsv1.2", "--retry", "3",
                     "--connect-timeout", "20", "--max-time", "240", "--output", str(temporary_file), entry["url"]])
                if digest(temporary_file) != entry["sha256"]:
                    raise ValueError(f"SHA-256 mismatch for downloaded {name}")
                os.replace(temporary_file, path)
        if not path.is_file() or digest(path) != entry["sha256"]:
            raise ValueError(f"SHA-256 mismatch for cached {name}")
        path.chmod(0o755)
        result[name] = path.resolve()
    return result


def portable_desktop(text: str) -> str:
    lines = []
    for line in text.splitlines():
        if line.startswith("Exec="):
            line = "Exec=liusheng %F"
        elif line.startswith("TryExec="):
            line = "TryExec=liusheng"
        elif line.startswith("Icon="):
            line = f"Icon={DESKTOP_ID}"
        lines.append(line)
    return "\n".join(lines) + "\n"


def copy_tree(source: Path, destination: Path) -> None:
    if not source.is_dir():
        raise ValueError(f"Required runtime directory is missing: {source}")
    shutil.copytree(source, destination, dirs_exist_ok=True, symlinks=False)


def stage_runtime(appdir: Path, qmake: str) -> None:
    plugins = Path(output([qmake, "-query", "QT_INSTALL_PLUGINS"]))
    libdir = Path(output(["pkg-config", "--variable=libdir", "libpipewire-0.3"]))
    spa = Path(output(["pkg-config", "--variable=plugindir", "libspa-0.2"]))
    # Qt's automatic plugin discovery historically omitted Wayland integration
    # plugins. Preserve this explicit runtime requirement alongside its scanner.
    platform_dir = appdir / "usr/plugins/platforms"
    platform_dir.mkdir(parents=True, exist_ok=True)
    for name in ("libqxcb.so", "libqoffscreen.so", "libqminimal.so", "libqwayland-generic.so", "libqwayland-egl.so"):
        shutil.copy2(plugins / "platforms" / name, platform_dir / name)
    for name in ("imageformats", "wayland-shell-integration", "wayland-graphics-integration-client", "wayland-decoration-client"):
        copy_tree(plugins / name, appdir / "usr/plugins" / name)
    # Native desktop portals remain on the host; only the Qt client is bundled.
    for name in ("platformthemes", "xcbglintegrations", "tls", "networkinformation"):
        if (plugins / name).is_dir():
            copy_tree(plugins / name, appdir / "usr/plugins" / name)
    copy_tree(spa, appdir / "usr/lib/spa-0.2")
    copy_tree(libdir / "pipewire-0.3", appdir / "usr/lib/pipewire-0.3")
    copy_tree(Path("/usr/share/pipewire"), appdir / "usr/share/pipewire")
    copy_tree(Path("/usr/share/alsa"), appdir / "usr/share/alsa")


def record_licenses(appdir: Path) -> None:
    """Carry Debian copyright/license notices for redistributed shared libraries."""
    licenses = appdir / "usr/share/doc/liusheng/bundled-licenses"
    licenses.mkdir(parents=True, exist_ok=True)
    owners = set()
    for path in (appdir / "usr/lib").rglob("*.so*"):
        result = subprocess.run(["dpkg-query", "--search", f"*/{path.name}"], text=True, capture_output=True)
        for line in result.stdout.splitlines():
            if ": " in line:
                owners.add(line.split(": ", 1)[0].split(":", 1)[0])
    for name in sorted(owners):
        copyright_file = Path("/usr/share/doc") / name / "copyright"
        if copyright_file.is_file():
            shutil.copyfile(copyright_file, licenses / f"{name}.copyright")
    notice = ("Bundled libraries are dynamically linked and retain their upstream licenses.\n"
              "Debian source packages and versions for this build are recorded below.\n"
              "Source package archive: https://snapshot.debian.org/\n"
              "Extract this AppImage with --appimage-extract to replace libraries under usr/lib.\n\n")
    if owners:
        notice += output(["dpkg-query", "-W", "-f=${binary:Package} ${Version} source=${source:Package} ${source:Version}\n", *sorted(owners)]) + "\n"
    (licenses / "SOURCES.txt").write_text(notice)
    common = Path("/usr/share/common-licenses")
    if common.exists():
        copy_tree(common, appdir / "usr/share/common-licenses")


REQUIRED_RUNTIME_FILES = ["AppRun", "usr/bin/liusheng", f"{DESKTOP_ID}.desktop", "usr/bin/qt.conf",
                "usr/lib/libQt6Core.so.6", "usr/lib/libQt6Quick.so.6", "usr/lib/libQt6Svg.so.6",
                "usr/lib/libpipewire-0.3.so.0", "usr/lib/libasound.so.2",
                "usr/plugins/platforms/libqxcb.so", "usr/plugins/platforms/libqwayland-generic.so",
                "usr/plugins/platforms/libqoffscreen.so", "usr/plugins/imageformats/libqsvg.so",
                "usr/plugins/tls/libqopensslbackend.so",
                "usr/qml/QtQuick/qmldir", "usr/qml/QtQuick/Controls/Basic/qmldir",
                "usr/qml/QtQuick/Shapes/qmldir", "usr/qml/Qt/labs/platform/qmldir",
                "usr/lib/spa-0.2/support/libspa-support.so", "usr/share/pipewire/client.conf",
                "usr/lib/pipewire-0.3/libpipewire-module-protocol-native.so", "usr/share/alsa/alsa.conf"]

def validate_appdir(appdir: Path) -> None:

    required = [*REQUIRED_RUNTIME_FILES, *("usr/lib/" + name for name in FORCED_CLIENT_LIBRARIES)]
    missing = [name for name in required if not (appdir / name).is_file()]
    if missing:
        raise ValueError(f"AppDir is missing required runtime files: {missing}")
    for path in appdir.rglob("*"):
        if path.is_symlink() and not path.resolve().is_relative_to(appdir.resolve()):
            raise ValueError(f"External symlink in AppDir: {path}")
    # Fonts are supplied by the host. Do not redistribute development-machine fonts.
    if any(p.suffix.lower() in {".ttf", ".otf", ".ttc", ".woff", ".woff2"} for p in appdir.rglob("*")):
        raise ValueError("AppDir must rely on system fonts")
    desktop = (appdir / f"{DESKTOP_ID}.desktop").read_text()
    if "Exec=liusheng %F" not in desktop or f"Icon={DESKTOP_ID}" not in desktop:
        raise ValueError("AppImage desktop entry must use relocatable executable and icon names")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--no-build", action="store_true")
    parser.add_argument("--version")
    parser.add_argument("--output", type=Path, default=ROOT / "dist")
    parser.add_argument("--tools-dir", type=Path, default=ROOT / "target/appimage-tools")
    args = parser.parse_args()
    if platform.system() != "Linux" or platform.machine() != "x86_64":
        parser.error("AppImage packaging requires Linux x86_64")
    version = version_for(ROOT, args.version)
    if os.environ.get("CARGO_BUILD_TARGET") not in (None, ""):
        parser.error("Clear CARGO_BUILD_TARGET before native AppImage packaging")
    target = Path(os.environ.get("CARGO_TARGET_DIR", str(ROOT / "target"))).resolve()
    binary = target / "release/liusheng"
    if not args.no_build:
        run(["cargo", "build", "--release", "--locked", "-p", "liusheng", "--manifest-path", str(ROOT / "Cargo.toml"), "--target-dir", str(target)], cwd=ROOT)
    check_elf(binary)
    tools = verified_tools(ROOT, args.tools_dir.resolve())
    qmake = os.environ.get("QMAKE") or shutil.which("qmake6") or "/usr/lib/qt6/bin/qmake"
    glibc_version = output(["getconf", "GNU_LIBC_VERSION"]).split()[-1]
    qt_version = output([qmake, "-query", "QT_VERSION"])
    if tuple(map(int, qt_version.split(".")[:2])) < (6, 8):
        raise ValueError("The application requires Qt 6.8 or newer")
    args.output.mkdir(parents=True, exist_ok=True)
    output_dir = args.output.resolve()
    artifact = output_dir / f"liusheng-{version}-linux-x86_64.AppImage"
    with tempfile.TemporaryDirectory(prefix="liusheng-appimage-") as temporary:
        work = Path(temporary)
        appdir = work / "AppDir"
        appdir.mkdir()
        env = dict(os.environ, PREFIX="/usr", DESTDIR=str(appdir), CARGO_TARGET_DIR=str(target), LANG="C.UTF-8")
        run(["bash", str(ROOT / "scripts/install.sh"), "--no-build"], env=env)
        desktop = appdir / "usr/share/applications" / f"{DESKTOP_ID}.desktop"
        desktop.write_text(portable_desktop(desktop.read_text()))
        stage_runtime(appdir, qmake)
        icon = appdir / "usr/share/icons/hicolor/256x256/apps" / f"{DESKTOP_ID}.png"
        env.update(APPIMAGE_EXTRACT_AND_RUN="1", QMAKE=qmake,
                   QML_SOURCES_PATHS=str(ROOT / "crates/liusheng/qml"),
                   EXTRA_QT_MODULES="svg", EXTRA_PLATFORM_PLUGINS="libqoffscreen.so;libqwayland-generic.so;libqwayland-egl.so",
                   PATH=str(args.tools_dir.resolve()) + ":" + env["PATH"])
        # These client libraries may be excluded by deployment tools by default.
        libdir = output([qmake, "-query", "QT_INSTALL_LIBS"])
        deploy = [str(tools["linuxdeploy-x86_64.AppImage"]), "--appdir", str(appdir),
                  "--desktop-file", str(desktop), "--icon-file", str(icon), "--executable", str(binary)]
        for name in FORCED_CLIENT_LIBRARIES:
            deploy.extend(["--library", str(Path(libdir) / name)])
        # Explicitly walk custom-copied Qt and PipeWire modules: loading the main
        # executable alone cannot reveal their dlopen dependencies.
        for directory in ("usr/plugins", "usr/lib/spa-0.2", "usr/lib/pipewire-0.3"):
            for module in sorted((appdir / directory).rglob("*.so")):
                deploy.extend(["--deploy-deps-only", str(module)])
        run([*deploy, "--plugin", "qt"], env=env, cwd=work)
        # Own one predictable launcher and one Qt prefix. Avoid loading host Qt
        # plugins into the bundled Qt version while retaining the user's files.
        apprun = appdir / "AppRun"
        if apprun.is_symlink():
            apprun.unlink()
        shutil.copyfile(ROOT / "packaging/appimage/AppRun", apprun)
        apprun.chmod(0o755)
        (appdir / "usr/bin/qt.conf").write_text("[Paths]\nPrefix=..\nLibraries=lib\nPlugins=plugins\nQmlImports=qml\nTranslations=translations\n")
        (appdir / "usr/share/liusheng").mkdir(parents=True, exist_ok=True)
        (appdir / "usr/share/liusheng/build-info.json").write_text(json.dumps({"version": version, "arch": "x86_64", "qt": qt_version, "glibc_minimum": glibc_version}, indent=2) + "\n")
        record_licenses(appdir)
        validate_appdir(appdir)
        built = work / artifact.name
        run([str(tools["appimagetool-x86_64.AppImage"]), "--runtime-file", str(tools["runtime-x86_64"]),
             "--no-appstream", str(appdir), str(built)], env=dict(env, ARCH="x86_64", VERSION=version), cwd=work)
        check_elf(built)
        # Verify content and execute from another path before publishing to dist.
        run([sys.executable, str(ROOT / "scripts/check-appimage.py"), str(built)])
        with tempfile.NamedTemporaryFile(prefix=".liusheng-", dir=output_dir, delete=False) as transfer:
            temp_output = Path(transfer.name)
        try:
            shutil.copyfile(built, temp_output)
            temp_output.chmod(0o755)
            os.replace(temp_output, artifact)
        finally:
            temp_output.unlink(missing_ok=True)
    print(f"AppImage ready: {artifact}\nSHA-256: {digest(artifact)}")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        print(f"AppImage packaging failed: {error}", file=sys.stderr)
        raise SystemExit(1)
