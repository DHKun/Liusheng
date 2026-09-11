"""Regression for path-matrix orchestration; subprocess results are explicit fixtures."""
from __future__ import annotations

from contextlib import redirect_stdout
import importlib.util
import io
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parent.parent
SPEC = importlib.util.spec_from_file_location("path_contracts", ROOT / "scripts/check-path-contracts.py")
runner = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(runner)


class PathContractRunnerTests(unittest.TestCase):
    def test_result_requires_executed_tests_without_failures_or_skips(self):
        self.assertEqual(runner.checked_test_count(0, "Ran 17 tests in 0.2s\n\nOK\n"), 17)
        self.assertEqual(runner.checked_test_count(0, "Ran 1 test in 0.2s\n\nOK\n"), 1)
        for code, text in ((1, "Ran 1 test in 0.2s\nOK\n"), (0, "Ran 0 tests in 0.2s\nOK\n"),
                           (0, "Ran 1 test in 0.2s\nOK (skipped=1)\n"), (0, ""),
                           (0, "FAIL: fixture\nRan 1 test in 0.2s\nOK\n")):
            with self.subTest(code=code, text=text), self.assertRaises(RuntimeError):
                runner.checked_test_count(code, text)

    def test_matrix_uses_real_aliases_and_cleans_up_fixtures(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "report"
            seen = []
            def child(command, **kwargs):
                path = Path(kwargs["env"]["TMPDIR"])
                self.assertTrue(path.is_dir())
                self.assertEqual(kwargs["env"]["TMP"], str(path))
                self.assertEqual(kwargs["env"]["TEMP"], str(path))
                self.assertEqual(command[0], runner.sys.executable)
                self.assertEqual(kwargs["cwd"], ROOT)
                seen.append((str(path), str(path.resolve())))
                return subprocess.CompletedProcess(command, 0, "", "Ran 1 test in 0.01s\n\nOK\n")
            with patch.object(runner.subprocess, "run", side_effect=child), redirect_stdout(io.StringIO()):
                report = runner.run_contracts(output, "macos")
            self.assertTrue(report["passed"])
            self.assertEqual(len(seen), 4)
            self.assertEqual(seen[0][0], seen[0][1])
            self.assertNotEqual(seen[2][0], seen[2][1])
            self.assertEqual(seen[0][1], seen[2][1])
            self.assertFalse(any(output.glob("fixtures-*")))
            self.assertTrue(json.loads((output / "summary.json").read_text())["passed"])

    def test_failure_replaces_old_success_and_retains_child_output(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory)
            (output / "summary.json").write_text('{"passed":true}')
            result = subprocess.CompletedProcess([], 1, "reported failure\n", "Ran 1 test in 0.1s\nFAILED\n")
            with patch.object(runner.subprocess, "run", return_value=result) as child, self.assertRaises(RuntimeError):
                runner.run_contracts(output, "macos")
            self.assertEqual(child.call_count, 1)
            self.assertFalse(json.loads((output / "summary.json").read_text())["passed"])
            self.assertIn("reported failure", next(output.glob("*.log")).read_text())
            self.assertFalse(any(output.glob("fixtures-*")))

    def test_timeout_preserves_output_and_failed_summary(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory)
            error = subprocess.TimeoutExpired(["fixture"], 120, output=b"partial output", stderr=b" waiting")
            with patch.object(runner.subprocess, "run", side_effect=error), self.assertRaisesRegex(RuntimeError, "timed out"):
                runner.run_contracts(output, "macos")
            self.assertIn("partial output waiting", next(output.glob("*.log")).read_text())
            self.assertFalse(json.loads((output / "summary.json").read_text())["passed"])
            self.assertFalse(any(output.glob("fixtures-*")))

    def test_main_and_release_use_same_native_bundle_verifier(self):
        quality = (ROOT / ".github/workflows/quality.yml").read_text()
        linux, macos = quality.split("  macos:\n", 1)
        self.assertIn("check-path-contracts.py --scope linux", linux)
        self.assertIn("check-path-contracts.py --scope macos", macos)
        self.assertIn("bash scripts/package-macos.sh --output target/qa/macos/packages", macos)
        self.assertIn("bash scripts/check-macos-bundle.sh target/qa/macos/packages/", macos)
        self.assertIn("if: github.ref_type != 'tag'", macos)
        self.assertLess(macos.index("Native macOS package and relocated startup"),
                        macos.index("Archive native macOS diagnostics"))
        release = (ROOT / ".github/workflows/release.yml").read_text()
        self.assertIn("bash scripts/check-macos-bundle.sh dist/", release)
        self.assertNotIn("continue-on-error: true", quality + release)


if __name__ == "__main__":
    unittest.main()
