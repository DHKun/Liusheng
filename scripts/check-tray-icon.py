#!/usr/bin/env python3
"""Inspect the actual application's StatusNotifierItem pixels on a private D-Bus.

python3 scripts/check-tray-icon.py /path/to/liusheng --output target/qa/tray-icon
Requires Weston, python3-dbus, python3-gi, Pillow and CairoSVG, plus Qt Wayland.
A conflicting desktop theme deliberately supplies an old icon with the same name.
Existing user desktop services, files, settings and processes remain untouched.
"""
from __future__ import annotations

import argparse
import contextlib
import signal
import shutil
import importlib.util
import io
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import time

ROOT = Path(__file__).resolve().parent.parent
APP_ID = "io.github.dhkun.Liusheng"
WATCHER = "org.kde.StatusNotifierWatcher"
ITEM = "org.kde.StatusNotifierItem"
PROPERTIES = "org.freedesktop.DBus.Properties"


def child(binary: Path, output: Path) -> int:
    import cairosvg
    import dbus
    import dbus.mainloop.glib
    import dbus.service
    from gi.repository import GLib
    from PIL import Image

    dbus.mainloop.glib.DBusGMainLoop(set_as_default=True)
    bus = dbus.SessionBus()
    loop = GLib.MainLoop()
    captured: list[tuple[str, str]] = []

    class Watcher(dbus.service.Object):
        def __init__(self):
            self.owner = dbus.service.BusName(WATCHER, bus=bus, do_not_queue=True)
            super().__init__(bus, "/StatusNotifierWatcher")

        @dbus.service.method(WATCHER, in_signature="s", out_signature="", sender_keyword="sender")
        def RegisterStatusNotifierItem(self, service, sender=None):
            address = (str(sender), str(service)) if str(service).startswith("/") else (str(service), "/StatusNotifierItem")
            captured.append(address)
            self.StatusNotifierItemRegistered(address[0] + address[1])

        @dbus.service.method(WATCHER, in_signature="s", out_signature="")
        def RegisterStatusNotifierHost(self, _service):
            pass

        @dbus.service.signal(WATCHER, signature="s")
        def StatusNotifierItemRegistered(self, service):
            pass

        @dbus.service.method(PROPERTIES, in_signature="s", out_signature="a{sv}")
        def GetAll(self, interface):
            if interface != WATCHER:
                return {}
            return {"IsStatusNotifierHostRegistered": dbus.Boolean(True),
                    "ProtocolVersion": dbus.Int32(0),
                    "RegisteredStatusNotifierItems": dbus.Array([s + p for s, p in captured], signature="s")}

        @dbus.service.method(PROPERTIES, in_signature="ss", out_signature="v")
        def Get(self, interface, property_name):
            return self.GetAll(interface)[property_name]

    watcher = Watcher()  # Keep the owner and its exported object alive until process exit.
    spec = importlib.util.spec_from_file_location("ui_fixture", ROOT / "scripts/check-ui.py")
    qa = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(qa)
    result = {"passed": False, "transport": "StatusNotifierItem/Wayland", "conflicting_theme": True}
    with tempfile.TemporaryDirectory(prefix="liusheng-tray-test-") as directory, contextlib.ExitStack() as cleanup:
        root = Path(directory)
        env = qa.fixture(root)
        env.update(QT_QPA_PLATFORMTHEME="kde", KDE_SESSION_VERSION="6", XDG_CURRENT_DESKTOP="KDE")
        # The offscreen QPA integration has no native system tray backend.
        # Use the same Wayland window-system path as the affected desktop.
        weston = shutil.which("weston")
        if not weston:
            raise RuntimeError("Install weston for the native tray transport test")
        compositor_log = cleanup.enter_context((output / "weston.log").open("w"))
        compositor = subprocess.Popen([weston, "--backend=headless", "--renderer=pixman",
            "--width=1280", "--height=800", "--socket=wayland-tray-test", "--no-config", "--idle-time=0"],
            env=env, stdout=compositor_log, stderr=subprocess.STDOUT, start_new_session=True)
        def stop_compositor():
            if compositor.poll() is None:
                os.killpg(compositor.pid, signal.SIGTERM)
                try: compositor.wait(timeout=3)
                except subprocess.TimeoutExpired:
                    os.killpg(compositor.pid, signal.SIGKILL); compositor.wait()
        cleanup.callback(stop_compositor)
        socket = root / "runtime/wayland-tray-test"
        socket_deadline = time.monotonic() + 8
        while not socket.exists():
            if compositor.poll() is not None or time.monotonic() > socket_deadline:
                raise RuntimeError("Weston failed to initialize; see weston.log")
            time.sleep(0.05)
        env.update(QT_QPA_PLATFORM="wayland", WAYLAND_DISPLAY=str(socket), QT_LOGGING_RULES="qt.qpa.tray.debug=true")
        theme = root / "data/icons/Conflict"
        images = theme / "scalable/apps"
        images.mkdir(parents=True)
        (theme / "index.theme").write_text("[Icon Theme]\nName=Conflict\nDirectories=scalable/apps\n"
                                           "[scalable/apps]\nSize=24\nType=Scalable\nMinSize=1\nMaxSize=256\n")
        old = '<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24"><rect width="24" height="24" fill="#ff0000"/></svg>'
        for name in (APP_ID, APP_ID + "-symbolic"):
            (images / f"{name}.svg").write_text(old)
        (root / "config/kdeglobals").write_text("[Icons]\nTheme=Conflict\n")
        deadline = time.monotonic() + 12
        with (output / "application.log").open("w") as log:
            process = subprocess.Popen([str(binary)], env=env, stdout=log, stderr=subprocess.STDOUT)
            errors = []

            def inspect():
                if process.poll() is not None:
                    errors.append(f"Application exited before tray registration: {process.returncode}")
                    loop.quit()
                    return False
                if time.monotonic() > deadline:
                    result["registered_items"] = captured
                    errors.append("StatusNotifierItem did not export a usable icon within 12s")
                    loop.quit()
                    return False
                if not captured:
                    return True
                service, path = captured[0]
                try:
                    proxy = bus.get_object(service, path, introspect=False)
                    props = dbus.Interface(proxy, PROPERTIES).GetAll(ITEM, timeout=2)
                    name = str(props.get("IconName", ""))
                    pixmaps = props.get("IconPixmap", [])
                    if not pixmaps:
                        return True
                    result["icon_name"] = name
                    result["pixmap_sizes"] = [[int(w), int(h)] for w, h, _ in pixmaps]
                    first_w, first_h, first_pixels = pixmaps[0]
                    first_image = Image.frombytes("RGBA", (int(first_w), int(first_h)), bytes(first_pixels), "raw", "ARGB")
                    result["first_pixmap_top_left"] = list(first_image.getpixel((0, 0)))
                    if name:
                        raise AssertionError(f"Tray is still selecting a theme icon: {name}")
                    source = (ROOT / "crates/liusheng/qml/assets/icons/brand.svg").read_bytes()
                    differences = []
                    for width, height, payload in pixmaps:
                        width, height = int(width), int(height)
                        if not (0 < width == height <= 1024):
                            raise AssertionError(f"Invalid tray pixmap size {width}x{height}")
                        image = Image.frombytes("RGBA", (width, height), bytes(payload), "raw", "ARGB")
                        reference = Image.open(io.BytesIO(cairosvg.svg2png(bytestring=source,
                            output_width=width, output_height=height))).convert("RGBA")
                        alpha = list(image.getchannel("A").getdata())
                        ref_alpha = list(reference.getchannel("A").getdata())
                        # Qt SVG and Cairo use different edge antialiasing. Compare silhouette.
                        difference = sum(abs(a - b) for a, b in zip(alpha, ref_alpha)) / (255 * width * height)
                        if difference > 0.08 or alpha[0] != 0:
                            raise AssertionError(f"Exported pixels differ from the approved mark: {difference:.4f}")
                        image.save(output / f"tray-{width}.png")
                        differences.append(round(difference, 6))
                    result["alpha_mean_errors"] = differences
                    result["passed"] = True
                except Exception as error:
                    errors.append(str(error))
                loop.quit()
                return False

            timer = GLib.timeout_add(100, inspect)
            try:
                loop.run()
            finally:
                # Only the child started by this isolated test is terminated.
                process.terminate()
                try:
                    process.wait(timeout=3)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait()
        if errors:
            result["errors"] = errors
        (output / "results.json").write_text(json.dumps(result, indent=2) + "\n")
        print(json.dumps(result, indent=2))
    return 0 if result["passed"] else 1


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", type=Path)
    parser.add_argument("--output", type=Path, default=ROOT / "target/qa/tray-icon")
    parser.add_argument("--private-session", action="store_true", help=argparse.SUPPRESS)
    args = parser.parse_args()
    binary = args.binary.resolve(strict=True)
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    if args.private_session:
        return child(binary, output)
    return subprocess.run(["dbus-run-session", "--", sys.executable, __file__, str(binary),
                           "--output", str(output), "--private-session"], timeout=25).returncode


if __name__ == "__main__":
    raise SystemExit(main())
