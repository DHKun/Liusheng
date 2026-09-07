"""The repeat-runner must reject missing tests and stop on the first failed round."""
from __future__ import annotations

import contextlib
import importlib.util
import io
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parent.parent
SPEC = importlib.util.spec_from_file_location("watcher_runner", ROOT / "scripts/check-watcher.py")
assert SPEC is not None and SPEC.loader is not None
runner = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(runner)


class WatcherRunnerTests(unittest.TestCase):
    def setUp(self) -> None:
        folder = tempfile.TemporaryDirectory(prefix="liusheng-runner-test-")
        self.addCleanup(folder.cleanup)
        self.output = Path(folder.name)
        self.runs: list[str] = []

    def fake_execute(self, command, project, timeout):
        if command[0] == "cargo":
            return 0, "\n".join(json.dumps({
                "reason": "compiler-artifact", "profile": {"test": True},
                "target": {"name": name, "kind": [definition[0]]},
                "executable": "/mock/" + name,
            }) for name, definition in runner.SUITES.items())
        name = Path(command[0]).name
        if "--list" in command:
            return 0, "\n".join(test + ": test" for test in runner.SUITES[name][2])
        self.runs.append(name)
        return 0, "test result: ok\n"

    def invoke(self, execute) -> tuple[int, dict]:
        with patch.object(runner, "execute", side_effect=execute), patch(
                "sys.argv", ["check-watcher.py", "--rounds", "3", "--output", str(self.output)]), \
                contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
            code = runner.main()
        return code, json.loads((self.output / "results.json").read_text())

    def test_every_planned_round_runs_both_suites(self):
        code, report = self.invoke(self.fake_execute)
        self.assertEqual(code, 0)
        self.assertTrue(report["passed"])
        self.assertEqual(len(report["rounds"]), 3)
        self.assertEqual(self.runs, ["liusheng_core", "library"] * 3)

    def test_first_failure_stops_without_retry_and_replaces_old_success(self):
        (self.output / "results.json").write_text('{"passed":true}')
        def execute(command, project, timeout):
            result = self.fake_execute(command, project, timeout)
            if "--nocapture" in command:
                return 101, "injected contract failure"
            return result
        code, report = self.invoke(execute)
        self.assertEqual(code, 1)
        self.assertFalse(report["passed"])
        self.assertEqual(len(report["rounds"]), 1)
        self.assertEqual(self.runs, ["liusheng_core"])
        self.assertIn("no retry", report["error"])

    def test_missing_required_tests_fail_before_rounds(self):
        def execute(command, project, timeout):
            if "--list" in command:
                return 0, "0 tests, 0 benchmarks\n"
            return self.fake_execute(command, project, timeout)
        code, report = self.invoke(execute)
        self.assertEqual(code, 1)
        self.assertFalse(report["passed"])
        self.assertEqual(report["rounds"], [])
        self.assertEqual(self.runs, [])

    def test_build_failure_runs_no_tests(self):
        code, report = self.invoke(lambda *args: (101, "injected compile error"))
        self.assertEqual(code, 1)
        self.assertFalse(report["passed"])
        self.assertEqual(self.runs, [])
        self.assertIn("Cargo test build failed", report["error"])


if __name__ == "__main__":
    unittest.main()
