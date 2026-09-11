"""Exercise the application's exported tray menu on the test's private D-Bus."""
from __future__ import annotations

import time


def check_menu(bus, service: str, path: str, process) -> dict:
    import dbus

    interface = "com.canonical.dbusmenu"
    menu = dbus.Interface(bus.get_object(service, path, introspect=False), interface)
    item = dbus.Interface(bus.get_object(service, "/StatusNotifierItem", introspect=False), "org.kde.StatusNotifierItem")
    media_name = "org.mpris.MediaPlayer2.io.github.dhkun.Liusheng"
    deadline = time.monotonic() + 4
    while not bus.name_has_owner(media_name):
        if time.monotonic() >= deadline:
            raise AssertionError("MPRIS did not register on the private session")
        time.sleep(0.04)
    media = dbus.Interface(bus.get_object(media_name, "/org/mpris/MediaPlayer2", introspect=False), "org.freedesktop.DBus.Properties")
    root_media = dbus.Interface(bus.get_object(media_name, "/org/mpris/MediaPlayer2", introspect=False), "org.mpris.MediaPlayer2")

    def entries() -> dict:
        _, layout = menu.GetLayout(0, -1, dbus.Array([], signature="s"), timeout=2)
        return {str(row[1].get("label", "")): (int(row[0]), row[1]) for row in layout[2] if row[1].get("label")}

    def until(predicate, message: str):
        deadline = time.monotonic() + 4
        while time.monotonic() < deadline:
            if process.poll() is not None:
                raise AssertionError(f"Application exited during {message}: {process.returncode}")
            if predicate():
                return
            time.sleep(0.04)
        raise AssertionError(message)

    def enabled(name: str) -> bool:
        return bool(entries()[name][1].get("enabled", True))

    def click(name: str):
        row_id, attributes = entries()[name]
        if not attributes.get("enabled", True):
            raise AssertionError(f"Menu action is disabled: {name}")
        menu.Event(dbus.Int32(row_id), "clicked", dbus.Int32(0, variant_level=1), dbus.UInt32(0), timeout=2)

    def title() -> str:
        return str(media.Get("org.mpris.MediaPlayer2.Player", "Metadata", timeout=2).get("xesam:title", ""))

    required = {"显示留声", "隐藏到托盘", "上一首", "继续播放", "下一首", "退出"}
    missing = required - entries().keys()
    if missing:
        raise AssertionError(f"Missing tray actions: {sorted(missing)}")
    until(lambda: title() == "Track 1" and enabled("下一首"), "Restored queue did not become available")
    # Opening the host-rendered menu must leave the application window hidden.
    click("隐藏到托盘")
    until(lambda: not enabled("隐藏到托盘"), "Hide action did not hide the window")
    menu.AboutToShow(0, timeout=2)
    assert required <= entries().keys()
    assert not enabled("隐藏到托盘"), "Opening the exported menu restored the application window"
    click("下一首")
    until(lambda: title() == "Track 2", "Tray Next did not change the current track")
    assert not enabled("隐藏到托盘"), "Track navigation restored the hidden window"
    click("上一首")
    until(lambda: title() == "Track 1", "Tray Previous did not restore the previous track")
    assert str(media.Get("org.mpris.MediaPlayer2.Player", "PlaybackStatus", timeout=2)) == "Paused"
    click("显示留声")
    until(lambda: enabled("隐藏到托盘"), "Show action did not restore the window")
    click("隐藏到托盘")
    until(lambda: not enabled("隐藏到托盘"), "Second hide failed")
    item.Activate(0, 0, timeout=2)
    until(lambda: enabled("隐藏到托盘"), "Tray left click did not restore the window")
    click("隐藏到托盘")
    until(lambda: not enabled("隐藏到托盘"), "Third hide failed")
    root_media.Raise(timeout=2)
    until(lambda: enabled("隐藏到托盘"), "MPRIS Raise did not restore the window")
    # Legacy hosts use ContextMenu instead of importing DBusMenu. The application
    # provides an in-window fallback and preserves the exported native menu.
    click("隐藏到托盘")
    until(lambda: not enabled("隐藏到托盘"), "Fourth hide failed")
    item.ContextMenu(1200, 20, timeout=2)
    until(lambda: enabled("隐藏到托盘"), "Legacy ContextMenu did not restore its fallback window")
    time.sleep(0.25)
    menu.AboutToShow(0, timeout=2)
    click("隐藏到托盘")
    until(lambda: not enabled("隐藏到托盘"), "Hide after legacy context failed")
    click("退出")
    code = process.wait(timeout=5)
    assert code == 0, f"Tray Quit did not exit cleanly: {code}"
    return {"menu_layout": True, "hidden_menu": True, "hidden_next_previous": True,
            "paused_state_preserved": True, "show_action": True, "left_click_restore": True,
            "mpris_restore": True, "legacy_context_restore": True, "hidden_quit": True}
