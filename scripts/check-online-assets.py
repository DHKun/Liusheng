#!/usr/bin/env python3
"""Compile the production online-resource service and run local HTTP regression.

Requires Qt Core/Gui/Network/QML/Test, qmake and C++17. --live additionally probes
fixed public sample titles using production HTTPS; ordinary CI stays offline.
"""
from __future__ import annotations
import argparse
import json
import os
from pathlib import Path
import re
import shutil
import subprocess

ROOT = Path(__file__).resolve().parent.parent
REQUIRED = ("serviceManualSearchPreviewChooseAndOfflineRestart",
            "coverDownloadRedirectPreviewSaveAndReleaseGroupIdentity",
            "batchRateLimitPausesWithoutConsumingPendingItems",
            "replacementAutomaticTrackCancelsStaleResources",
            "manualApplyAcknowledgesDurableWriteAndIgnoresDoubleClick",
            "exactMissUsesBaseTitleAndLeavesJazzCandidateForManualReview",
            "coverRecordingFallbackWorksWithAnIncorrectAlbumTag",
            "transient503RetriesSameSearchThenSavesExactlyOnce",
            "cancelledBatchAccountsEveryItem",
            "exhaustedRetriesKeepOnePendingRowAndNeverCacheAbsence",
            "legacyCanceledBatchHasConsistentCountsAndDeduplicatedRecovery",
            "sourceAliasOnMatchingReleaseTrackKeepsTraditionalRecordingCandidate",
            "multiSourceFailureLeavesHealthyResultsAndDoesNotCachePartialAbsence",
            "manualQQLyricsLoadOnPreviewAndPersistOffline",
            "neteaseCoverDetailsMustMatchSongAndAlbumBeforeDownloading",
            "deezerCoverPreviewUsesValidatedCdnAndPersistsProviderIdentity",
            "extraAutomaticSourcesRequireOptInAndDisablingCancelsThem",
            "qqCoverRequestsPortableImageFormatsAndValidatesDownloadedPixels")


def validate_results(text: str) -> int:
    totals = re.search(r"Totals: (\d+) passed, (\d+) failed, (\d+) skipped", text)
    if not totals or int(totals[1]) < 30 or any(int(totals[i]) for i in (2, 3)) or any(name not in text for name in REQUIRED):
        raise ValueError("Online-resource regression suite is missing, failed or incomplete")
    if any(message in text for message in ("QWARN", "FAIL!", "QFATAL")):
        raise ValueError("Online-resource regression emitted a Qt diagnostic")
    return int(totals[1])


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=ROOT / "target/qa/online-assets/native")
    parser.add_argument("--compiler", choices=("default", "clang"), default="default")
    parser.add_argument("--live", action="store_true")
    args = parser.parse_args()
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    report = {"passed": False, "phase": "started", "compiler": args.compiler}
    result_file = output / "summary.json"
    result_file.write_text(json.dumps(report) + "\n")
    (output / "results.txt").unlink(missing_ok=True)
    # Different compilers receive separate object directories.
    build = output / ("build-" + args.compiler)
    build.mkdir(exist_ok=True)
    source = ROOT / "crates/liusheng/src"
    files = [source / "online_service.cpp", *sorted((source / "online").glob("*.cpp")), ROOT / "tests/native/online_assets_test.cpp"]
    def quote(path: Path) -> str:
        return '$$quote(' + str(path).replace('\\', '/') + ')'
    project = build / "online.pro"
    project.write_text("\n".join([
        "QT += core gui network qml testlib", "CONFIG += console c++17 testcase", "CONFIG -= app_bundle",
        "DEFINES += LIUSHENG_ONLINE_TEST", "TARGET = online_assets_test",
        "QMAKE_CXXFLAGS += -Wall -Wextra -Werror", "INCLUDEPATH += " + quote(source),
        "HEADERS += " + quote(source / "online_service.h"),
        "SOURCES += " + " ".join(quote(path) for path in files),
        *(["QMAKE_CC = clang", "QMAKE_CXX = clang++", "QMAKE_LINK = clang++"] if args.compiler == "clang" else []),
    ]) + "\n")
    qmake = shutil.which("qmake6") or shutil.which("qmake")
    if not qmake:
        parser.error("Install Qt qmake before running online-resource tests")
    env = dict(os.environ, QT_QPA_PLATFORM="offscreen", LANG="C.UTF-8")
    commands = [[qmake, str(project)], ["make", "-j2"], [str(build / "online_assets_test"), "-o", str(output / "results.txt") + ",txt"]]
    try:
        for index, command in enumerate(commands):
            result = subprocess.run(command, cwd=build, env=env, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                                    text=True, timeout=180 if index == 1 else 90)
            (output / f"step-{index}.log").write_text(result.stdout)
            if result.returncode:
                raise RuntimeError(f"Stage {index} failed: {result.stdout}")
        text = (output / "results.txt").read_text()
        report.update(passed=True, phase="completed", qt_passed_including_setup_teardown=validate_results(text),
                      scope="production service with deterministic local HTTP fixtures")
        print(text)
        if args.live:
            with (output / "live.log").open("w") as log:
                result = subprocess.run([str(build / "online_assets_test"), "--live"], cwd=build, env=env,
                                        stdout=log, stderr=subprocess.STDOUT, timeout=150)
            report["live_passed"] = result.returncode == 0
            print((output / "live.log").read_text())
    except (OSError, ValueError, RuntimeError, subprocess.SubprocessError) as error:
        report.update(passed=False, error=str(error))
        print(str(error))
        if (output / "results.txt").exists(): print((output / "results.txt").read_text())
    result_file.write_text(json.dumps(report, indent=2) + "\n")
    return 0 if report["passed"] and report.get("live_passed", True) else 1


if __name__ == "__main__":
    raise SystemExit(main())
