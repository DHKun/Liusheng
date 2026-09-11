#!/usr/bin/env python3
"""Run packaging contracts through physical and symlinked temporary directories.

Usage: python3 scripts/check-path-contracts.py --scope macos --output target/qa/paths
macos runs portable Cocoa and packaging fixtures; linux runs all Python tests.
This reproduces /var -> /private/var path spelling differences on either host.
It never creates or modifies the operating system's /var or /private directories.
"""
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parent.parent
SCOPES = {
    "macos": ("test_macos_package.py", "test_desktop_smoke.py"),
    "linux": ("test_*.py",),
}


def checked_test_count(returncode: int, output: str) -> int:
    counts = re.findall(r"^Ran (\d+) tests? in ", output, re.MULTILINE)
    if (returncode != 0 or len(counts) != 1 or int(counts[0]) <= 0
            or not re.search(r"^OK$", output, re.MULTILINE)
            or re.search(r"^(?:FAILED|ERROR:|FAIL:)", output, re.MULTILINE)
            or re.search(r"\bskipped[= ]", output)):
        raise RuntimeError("Path contracts failed, were skipped, or ran no tests")
    return int(counts[0])


def run_contracts(output: Path, scope: str) -> dict:
    if scope not in SCOPES:
        raise ValueError(f"Unknown path-contract scope: {scope}")
    output = output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    report = {"passed": False, "scope": scope, "host": sys.platform,
              "validation": "portable filesystem and process fixtures", "runs": []}
    try:
        with tempfile.TemporaryDirectory(prefix="fixtures-", dir=output) as temporary:
            root = Path(temporary).resolve(strict=True)
            physical = root / "private/var/folders/temporary with spaces"
            physical.mkdir(parents=True)
            alias = root / "var"
            alias.symlink_to(root / "private/var", target_is_directory=True)
            linked = alias / "folders/temporary with spaces"
            if str(linked) == str(physical) or not linked.samefile(physical):
                raise RuntimeError("The regression fixture must have two names for one directory")
            for spelling, directory in (("physical", physical), ("symlinked", linked)):
                for pattern in SCOPES[scope]:
                    command = [sys.executable, "-m", "unittest", "discover", "-s", str(ROOT / "tests"),
                               "-p", pattern, "-v"]
                    env = dict(os.environ, TMPDIR=str(directory), TMP=str(directory), TEMP=str(directory),
                               PYTHONDONTWRITEBYTECODE="1")
                    name = f"{spelling}-{pattern.replace('*', 'all')}.log"
                    record = {"spelling": spelling, "pattern": pattern, "log": name,
                              "tempdir": str(directory), "resolved_tempdir": str(directory.resolve()),
                              "passed": False}
                    report["runs"].append(record)
                    try:
                        result = subprocess.run(command, cwd=ROOT, env=env, capture_output=True,
                                                text=True, encoding="utf-8", errors="replace", timeout=120)
                    except subprocess.TimeoutExpired as error:
                        text = b"".join(x if isinstance(x, bytes) else (x or "").encode()
                                        for x in (error.stdout, error.stderr)).decode("utf-8", "replace")
                        (output / name).write_text(text + "\nPATH CONTRACT TIMEOUT\n", encoding="utf-8")
                        raise RuntimeError(f"Path contracts timed out: {name}") from error
                    text = result.stdout + result.stderr
                    (output / name).write_text(text, encoding="utf-8")
                    record["returncode"] = result.returncode
                    record["tests"] = checked_test_count(result.returncode, text)
                    record["passed"] = True
                    print(f"{spelling} {pattern}: {record['tests']} tests passed", flush=True)
        report["passed"] = True
        return report
    except (OSError, ValueError, RuntimeError, subprocess.SubprocessError) as error:
        report["error"] = str(error)
        raise
    finally:
        (output / "summary.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n",
                                             encoding="utf-8")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--scope", choices=SCOPES, default="macos")
    parser.add_argument("--output", type=Path, default=ROOT / "target/qa/path-contracts")
    args = parser.parse_args()
    run_contracts(args.output, args.scope)
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, ValueError, RuntimeError, subprocess.SubprocessError) as error:
        print(f"PATH CONTRACT CHECK FAILED: {error}", file=sys.stderr)
        raise SystemExit(1)
