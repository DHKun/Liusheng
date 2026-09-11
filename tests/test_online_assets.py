"""Online-resource runner, privacy and native-platform delivery contracts."""
from __future__ import annotations
import contextlib
import importlib.util
import io
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parent.parent
SPEC = importlib.util.spec_from_file_location("online_runner", ROOT / "scripts/check-online-assets.py")
runner = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(runner)


def valid_report():
    return "\n".join("PASS : Tests::" + name + "()" for name in runner.REQUIRED) + "\nTotals: 36 passed, 0 failed, 0 skipped\n"


class OnlineRunnerTests(unittest.TestCase):
    def test_requires_complete_success_without_skips_or_warnings(self):
        self.assertEqual(runner.validate_results(valid_report()), 36)
        for value in ("", "Totals: 0 passed, 0 failed, 0 skipped", valid_report().replace("0 failed", "1 failed"),
                      valid_report().replace("0 skipped", "1 skipped"), valid_report()+"QWARN unexpected", valid_report().replace(runner.REQUIRED[0], "removed")):
            with self.subTest(value=value), self.assertRaises(ValueError):
                runner.validate_results(value)

    def invoke(self, output: Path, action):
        with patch("sys.argv", ["check-online-assets.py", "--output", str(output)]), \
             patch.object(runner.shutil, "which", return_value="qmake6"), \
             patch.object(runner.subprocess, "run", side_effect=action), contextlib.redirect_stdout(io.StringIO()):
            return runner.main()

    def test_compile_failure_stops_before_test_execution(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory)
            calls = []
            def execute(command, **kwargs):
                calls.append(command)
                return subprocess.CompletedProcess(command, 1 if command[0] == "make" else 0, "fixture compilation error")
            self.assertEqual(self.invoke(output, execute), 1)
            self.assertEqual(len(calls), 2)
            self.assertFalse(json.loads((output / "summary.json").read_text())["passed"])

    def test_old_success_is_removed_before_new_test_run(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory)
            (output / "results.txt").write_text(valid_report())
            (output / "summary.json").write_text('{"passed":true}')
            self.assertEqual(self.invoke(output, lambda cmd, **kw: subprocess.CompletedProcess(cmd, 0, "")), 1)
            self.assertFalse(json.loads((output / "summary.json").read_text())["passed"])

    def test_success_records_actual_completed_suite(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory)
            def execute(command, **kwargs):
                if Path(command[0]).name == "online_assets_test":
                    (output / "results.txt").write_text(valid_report())
                return subprocess.CompletedProcess(command, 0, "")
            self.assertEqual(self.invoke(output, execute), 0)
            report = json.loads((output / "summary.json").read_text())
            self.assertTrue(report["passed"])
            self.assertEqual(report["qt_passed_including_setup_teardown"], 36)
            self.assertNotIn("live_passed", report)


class OnlineDeliveryTests(unittest.TestCase):
    def test_native_cpp_test_runs_on_linux_and_apple_silicon(self):
        content = (ROOT / ".github/workflows/quality.yml").read_text()
        linux, macos = content.split("  macos:", 1)
        self.assertIn("scripts/check-online-assets.py", linux)
        self.assertIn("scripts/check-online-assets.py", macos)
        self.assertIn("runs-on: macos-15", macos)
        self.assertNotIn("--live", content)
        for workflow in ("check.yml", "release.yml"):
            self.assertIn("uses: ./.github/workflows/quality.yml", (ROOT / ".github/workflows" / workflow).read_text())

    def test_service_and_dialog_sources_are_packaged(self):
        content = (ROOT / "crates/liusheng/build.rs").read_text()
        for file in ("online_service.h", "online_service.cpp", "OnlineAssetsDialog.qml", "OnlineBatchDialog.qml"):
            self.assertIn(file, content)
        for path in (ROOT / "crates/liusheng/src/online").glob("*.cpp"):
            self.assertIn("online/"+path.name, content)

    def test_previews_use_window_items_and_parented_native_file_picker(self):
        dialog = (ROOT / "crates/liusheng/qml/OnlineAssetsDialog.qml").read_text()
        self.assertIn("QuietDialog {", dialog)
        self.assertIn("textFormat: Text.PlainText", dialog)
        self.assertIn("parentWindow:", dialog)
        self.assertIn("popupType: Popup.Item", (ROOT / "crates/liusheng/qml/QuietDialog.qml").read_text())
        self.assertIn("OnlineAssetsDialog", (ROOT / "scripts/check-controls.py").read_text())

    def test_supplementary_sources_are_packaged_with_preview_validation(self):
        build = (ROOT / "crates/liusheng/build.rs").read_text()
        for name in ("providers.cpp", "multi_lookup.cpp", "OnlineSourcePicker.qml", "OnlineSourceInfo.qml"):
            self.assertIn(name, build)
        source = (ROOT / "crates/liusheng/src/online/providers.cpp").read_text()
        self.assertIn("AbortOnBase64DecodingErrors", source)
        self.assertIn("neteaseCoverUrl", source)
        self.assertIn("safeSourcePage", source)
        self.assertIn("lyricsPending", source)

    def test_extra_sources_have_separate_automatic_consent(self):
        settings = (ROOT / "crates/liusheng-core/src/settings.rs").read_text()
        self.assertIn("online_extra_sources: false", settings)
        main = (ROOT / "crates/liusheng/qml/Main.qml").read_text()
        self.assertIn("autoExtraSources: root.preferences.online_extra_sources === true", main)
        self.assertIn("startBatchWithSource", main)
        dialog = (ROOT / "crates/liusheng/qml/OnlineAssetsDialog.qml").read_text()
        self.assertIn("searchWithSource", dialog)
        self.assertIn("OnlineSourceInfo", dialog)
        self.assertNotIn('objectName: "onlineQueryHint"', dialog)

    def test_fail_fast_sources_keep_results_and_do_not_store_partial_absence(self):
        source = (ROOT / "crates/liusheng/src/online/multi_lookup.cpp").read_text()
        self.assertIn('"partialResults"', source)
        self.assertIn("run->failed", source)
        self.assertIn("multi_ != run", source)
        self.assertIn("run->providers.size() > 1", source)
        service = (ROOT / "crates/liusheng/src/online_service.cpp").read_text()
        self.assertIn("safeSourcePage", service)
        self.assertNotIn("X-Real-IP", service)

    def test_automated_checks_and_benchmarks_explicitly_block_online_music(self):
        service = (ROOT / "crates/liusheng/src/online_service.cpp").read_text()
        for flag in ("--no-online-metadata", "--ui-test", "--functional-test", "--startup-benchmark", "--smoke-test"):
            self.assertIn(flag, service)
        self.assertIn("#ifdef LIUSHENG_ONLINE_TEST", service)
        transport = (ROOT / "crates/liusheng/src/online/transport.cpp").read_text()
        self.assertIn("ManualRedirectPolicy", transport)
        self.assertNotIn("ignoreSslErrors", transport)


if __name__ == "__main__":
    unittest.main()
