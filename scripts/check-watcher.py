#!/usr/bin/env python3
"""Repeat native watcher and library integration contracts; every round must pass.

Usage: python3 scripts/check-watcher.py --rounds 10 --output target/qa/watcher
Builds once using Cargo JSON artifact metadata, then executes those exact test binaries.
A failed round stops immediately. This is repeated validation, with no failure retry.
Every Rust fixture uses a private temporary directory; user music is never opened.
"""
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import time

FILTER = "library::watcher::tests::"
SUITES = {
    "liusheng_core": ("lib", FILTER, {
        FILTER + "directory_and_file_notifications_preserve_both_scopes",
        FILTER + "rescan_flag_precedes_kind_and_path_filters",
        FILTER + "native_files_converge_after_create_modify_rename_and_delete",
        FILTER + "native_populated_directory_move_rename_and_remove_converge",
    }),
    "library": ("test", "", {"watcher_changes_drive_incremental_refresh", "scan_search_and_remove"}),
}


def execute(command: list[str], project: Path, timeout: int) -> tuple[int, str]:
    with subprocess.Popen(command, cwd=project, stdout=subprocess.PIPE,
                          stderr=subprocess.STDOUT, encoding="utf-8", errors="replace",
                          start_new_session=True) as process:
        try:
            output, _ = process.communicate(timeout=timeout)
            return process.returncode, output
        except subprocess.TimeoutExpired:
            # Only terminate this isolated test invocation's process group.
            os.killpg(process.pid, signal.SIGKILL)
            output, _ = process.communicate()
            return 124, output + f"\nWatcher validation timed out after {timeout}s\n"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--rounds", type=int, default=10)
    parser.add_argument("--test-threads", type=int, default=4)
    parser.add_argument("--output", type=Path, default=Path("target/qa/watcher"))
    args = parser.parse_args()
    if not 1 <= args.rounds <= 200 or not 1 <= args.test_threads <= 32:
        parser.error("rounds must be 1..200 and test-threads must be 1..32")
    project = Path(__file__).resolve().parent.parent
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    report: dict = {"platform": sys.platform, "requested_rounds": args.rounds,
                    "test_threads": args.test_threads, "rounds": [], "passed": False}

    def save() -> None:
        (output / "results.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")

    def fail(message: str, log: str = "") -> int:
        report["error"] = message
        save()
        print(message + "\n" + log, file=sys.stderr)
        return 1

    save()  # A fresh invocation cannot leave an old success result in place.
    code, build = execute(["cargo", "test", "-p", "liusheng-core", "--lib", "--test", "library",
                           "--locked", "--no-run", "--message-format=json"], project, 300)
    (output / "build.log").write_text(build, encoding="utf-8")
    if code:
        return fail(f"Cargo test build failed ({code})", build)
    binaries: dict[str, set[str]] = {name: set() for name in SUITES}
    for line in build.splitlines():
        try:
            item = json.loads(line)
        except json.JSONDecodeError:
            continue
        target = item.get("target", {})
        name = target.get("name")
        if (item.get("reason") == "compiler-artifact" and item.get("profile", {}).get("test")
                and name in SUITES and SUITES[name][0] in target.get("kind", [])
                and item.get("executable")):
            binaries[name].add(item["executable"])
    suites = []
    for name, (_, test_filter, required) in SUITES.items():
        if len(binaries[name]) != 1:
            return fail(f"Expected one {name} test executable; received {sorted(binaries[name])}")
        binary = binaries[name].pop()
        code, listing = execute([binary, test_filter, "--list"], project, 30)
        tests = {line.removesuffix(": test") for line in listing.splitlines() if line.endswith(": test")}
        if code or not required.issubset(tests):
            return fail(f"Regression tests missing from {name}", listing)
        suites.append({"name": name, "binary": binary, "filter": test_filter, "test_count": len(tests)})
    report.update(suites=suites, test_count=sum(suite["test_count"] for suite in suites))
    for iteration in range(1, args.rounds + 1):
        started = time.monotonic()
        round_info = {"round": iteration, "suites": []}
        report["rounds"].append(round_info)
        for suite in suites:
            code, log = execute([suite["binary"], suite["filter"], "--nocapture",
                                 f"--test-threads={args.test_threads}"], project, 90)
            (output / f"round-{iteration:03d}-{suite['name']}.log").write_text(log, encoding="utf-8")
            round_info["suites"].append({"name": suite["name"], "exit_code": code})
            if code:
                return fail(f"Round {iteration}/{suite['name']} failed ({code}); no retry", log)
        round_info["elapsed_seconds"] = round(time.monotonic() - started, 3)
        save()
        print(f"Watcher {sys.platform}: round {iteration}/{args.rounds}, {report['test_count']} tests passed", flush=True)
    report["passed"] = True
    save()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
