"""Line-only lyrics and native tray-menu delivery regression contracts."""
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parent.parent


class LineLyricsAndTrayDelivery(unittest.TestCase):
    def test_progressive_renderer_is_retired_from_the_application(self):
        build = (ROOT / "crates/liusheng/build.rs").read_text()
        for source in ("timed_lyric_text.cpp", "timed_lyric_text.h"):
            self.assertNotIn(source, build)
            self.assertFalse((ROOT / "crates/liusheng/src" / source).exists())
        self.assertNotIn("TimedLyricText", (ROOT / "crates/liusheng/qml/LyricsView.qml").read_text())

    def test_line_color_has_no_estimation_or_transition_delay(self):
        view = (ROOT / "crates/liusheng/qml/LyricsView.qml").read_text()
        self.assertIn('objectName: "lyricText"', view)
        self.assertIn("textFormat: Text.PlainText", view)
        self.assertIn("color: row.current", view)
        for removed in ("Behavior on color", "estimated:", "revealRects", "highlightMode", "lyricTimingLabel"):
            self.assertNotIn(removed, view)
        self.assertIn("positionMs - controller.lyricsOffsetMs", view)

    def test_removed_preference_cannot_reactivate_word_highlighting(self):
        for source in ("qml/Theme.qml", "qml/SettingsDialog.qml", "qml/Main.qml"):
            text = (ROOT / "crates/liusheng" / source).read_text()
            self.assertNotIn("lyric_highlight", text)
            self.assertNotIn("lyricHighlight", text)
        self.assertNotIn("pub lyric_highlight", (ROOT / "crates/liusheng-core/src/settings.rs").read_text())

    def test_clock_keeps_normal_ui_rate_and_hidden_pause_behavior(self):
        clock = (ROOT / "crates/liusheng/qml/PlaybackClock.qml").read_text()
        self.assertIn("interval: 33", clock)
        self.assertIn("clock.playing && clock.displayed", clock)
        self.assertNotIn("smooth", clock)
        tools = (ROOT / "scripts/control_test_tools.py").read_text()
        self.assertIn("control_test_main.cpp", tools)
        self.assertNotIn("timed_lyric_text", tools)

    def test_tray_menu_is_attached_during_construction_on_wayland(self):
        main = (ROOT / "crates/liusheng/qml/Main.qml").read_text()
        self.assertIn("menu: Platform.Menu {", main)
        self.assertNotIn("menu: DesktopBridge.wayland ? null", main)
        self.assertNotIn("nativeTrayMenuFactory", main)
        self.assertIn("showNormal();", main)
        self.assertIn("trayQuickMenu.openAt(root.contentItem", main)
        self.assertIn("popupType: Popup.Item", (ROOT / "crates/liusheng/qml/QuietMenu.qml").read_text())

    def test_release_and_quality_gates_run_real_hidden_menu_actions(self):
        workflow = (ROOT / ".github/workflows/quality.yml").read_text()
        self.assertIn("scripts/check-tray-icon.py", workflow)
        self.assertIn("test_lyric_delivery.py", workflow)
        self.assertNotIn("check-lyric-renderer.py", workflow)
        self.assertNotIn("check-lyric-wayland.py", workflow)
        for name in ("check.yml", "release.yml"):
            self.assertIn("uses: ./.github/workflows/quality.yml", (ROOT / ".github/workflows" / name).read_text())
        transport = (ROOT / "scripts/check-tray-icon.py").read_text()
        self.assertIn("/NO_DBUSMENU", transport)
        self.assertIn("check_menu(bus", transport)
        checks = (ROOT / "scripts/tray_menu_checks.py").read_text()
        for method in ("menu.GetLayout", "menu.Event", "menu.AboutToShow", "item.ContextMenu", "item.Activate"):
            self.assertIn(method, checks)

    def test_existing_lyric_imports_remain_readable(self):
        dialog = (ROOT / "crates/liusheng/qml/OnlineAssetsDialog.qml").read_text()
        for extension in ("*.ttml", "*.alrc", "*.yrc", "*.qrc", "*.lrc"):
            self.assertIn(extension, dialog)
        self.assertIn("Lyrics::read_file", (ROOT / "crates/liusheng/src/app_controller/online.rs").read_text())


if __name__ == "__main__":
    unittest.main()
