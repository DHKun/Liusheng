#!/usr/bin/env python3
"""Build and test the production Qt update checker against isolated loopback servers.

Requires Qt 6 (Core, Gui, Network, Qml, Test), qmake, a C++17 compiler and make.
Works on Linux and macOS. --live additionally performs one real GitHub metadata
request; all storage remains temporary, and browser actions are intercepted.
"""
from __future__ import annotations
import argparse
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parent.parent


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=ROOT / "target/qa/updates/native")
    parser.add_argument("--live", action="store_true")
    args = parser.parse_args()
    qmake = shutil.which("qmake6") or shutil.which("qmake")
    if not qmake or not shutil.which("make"):
        parser.error("Install Qt 6 qmake and make")
    args.output.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="liusheng-update-tests-") as temporary:
        work = Path(temporary)
        project = work / "updates.pro"
        source = ROOT / "crates/liusheng/src"
        project.write_text("\n".join([
            "QT += core gui network qml testlib", "CONFIG += console c++17 testcase", "CONFIG -= app_bundle",
            "TARGET = update-tests", "DEFINES += LIUSHENG_UPDATE_TEST",
            'QMAKE_CXXFLAGS += -Wall -Wextra -Werror',
            f'INCLUDEPATH += "{source}"',
            f'SOURCES += "{source / "update_service.cpp"}" "{ROOT / "tests/native/update_service_test.cpp"}"',
            f'HEADERS += "{source / "update_service.h"}"',
        ]) + "\n")
        env = dict(os.environ, LANG="C.UTF-8")
        steps = [("configure", [qmake, str(project)]), ("compile", ["make", "-j2"]),
                 ("run", [str(work / "update-tests")])]
        if args.live:
            steps.append(("live", [str(work / "update-tests"), "--live"]))
        for name, command in steps:
            with (args.output / f"{name}.log").open("w") as log:
                result = subprocess.run(command, cwd=work, env=env, stdout=log, stderr=subprocess.STDOUT, timeout=120)
            text = (args.output / f"{name}.log").read_text()
            if result.returncode:
                print(text)
                return result.returncode
            if name in ("run", "live"):
                print(text, end="")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
