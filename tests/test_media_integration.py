"""Portable checks for the native adapter ABI and macOS test entrypoint.

The production Objective-C++ implementation is compiled and exercised by the
arm64 job using scripts/check-macos-media.py. These tests check its entrypoint
and build contracts without claiming a macOS runtime test on Linux.
"""
from __future__ import annotations
import contextlib
import importlib.util
import io
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parent.parent
spec = importlib.util.spec_from_file_location("macos_media_runner", ROOT / "scripts/check-macos-media.py")
runner = importlib.util.module_from_spec(spec)
spec.loader.exec_module(runner)


class MediaIntegrationTests(unittest.TestCase):
    def test_native_abi_header_is_c_and_cpp_compatible(self):
        compiler = shutil.which("clang") or shutil.which("cc")
        self.assertIsNotNone(compiler, "A C compiler is required by the workspace")
        source = '#include "macos_media.h"\nstatic bool accept(int32_t code, double value) { return code > 0 && value >= 0; }\nvoid test(void) { LiushengMediaCallback callback = accept; (void)liusheng_media_start(callback); }\n'
        for language in ("c", "c++"):
            with self.subTest(language=language):
                result = subprocess.run([compiler, "-x", language, "-Werror", "-fsyntax-only", "-I", str(ROOT / "crates/liusheng/src"), "-"],
                                        input=source, text=True, capture_output=True, timeout=15)
                self.assertEqual(result.returncode, 0, result.stderr)

    def test_objcpp_error_out_parameters_use_pointer_null(self):
        # In Objective-C++, nil is an object pointer and NSError** requires nullptr.
        for name in ("crates/liusheng/src/macos_media.mm", "tests/native/macos_media_test.mm"):
            text = (ROOT / name).read_text()
            self.assertNotIn("error:nil", text)
            self.assertIn("error:nullptr", text)
        source = """__attribute__((objc_root_class)) @interface Sample
        - (void)call:(Sample **)error;
        @end
        void test(Sample *value) { [value call:nullptr]; }
        """
        result = subprocess.run(["clang++", "-x", "objective-c++", "-std=c++17", "-fobjc-runtime=macosx-13.0",
                                 "-fobjc-arc", "-Werror", "-fsyntax-only", "-"],
                                input=source, text=True, capture_output=True, timeout=15)
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_native_test_is_required_in_arm64_ci_and_release(self):
        for path in (".github/workflows/check.yml", ".github/workflows/release.yml"):
            content = (ROOT / path).read_text().split("  macos:", 1)[1]
            self.assertIn("runs-on: macos-15", content)
            self.assertIn("scripts/check-macos-media.py", content)
            self.assertNotIn("macos-15-intel", content)
        self.assertIn('use macos_media as mpris;', (ROOT / "crates/liusheng/src/main.rs").read_text())
        self.assertIn('"MediaPlayer"', (ROOT / "crates/liusheng/build.rs").read_text())

    def test_native_runner_rejects_an_unsupported_host(self):
        with patch.object(sys, "argv", ["check-macos-media.py"]), patch.object(runner.platform, "system", return_value="Linux"), patch.object(runner.subprocess, "run") as run:
            with contextlib.redirect_stderr(io.StringIO()), self.assertRaises(SystemExit) as raised:
                runner.main()
            self.assertEqual(raised.exception.code, 2)
            run.assert_not_called()

    def test_native_compile_failure_stops_before_runtime(self):
        with tempfile.TemporaryDirectory() as directory:
            with patch.object(sys, "argv", ["check-macos-media.py", "--output", directory]), patch.object(runner.platform, "system", return_value="Darwin"), patch.object(runner.platform, "machine", return_value="arm64"):
                with patch.object(runner.subprocess, "run", side_effect=subprocess.CalledProcessError(1, ["clang++"])) as run:
                    with contextlib.redirect_stdout(io.StringIO()), self.assertRaises(subprocess.CalledProcessError):
                        runner.main()
                    self.assertEqual(run.call_count, 1)
                    arguments = run.call_args.args[0]
                    self.assertIn("-fobjc-arc", arguments)
                    self.assertIn("-Werror", arguments)
                    self.assertIn("arm64", arguments)
                    self.assertIn("MediaPlayer", arguments)


if __name__ == "__main__":
    unittest.main()
