"""Build the small Qt Quick Test runner with a monotonic-clock test dependency."""
from __future__ import annotations
import os
from pathlib import Path
import shutil
import subprocess

ROOT = Path(__file__).resolve().parent.parent


def build_runner(output: Path | None = None, compiler: str = "default") -> Path:
    output = (output or ROOT / "target/qa/controls-native" / compiler).resolve()
    output.mkdir(parents=True, exist_ok=True)
    source = ROOT / "crates/liusheng/src"
    def quote(path: Path) -> str:
        return '$$quote(' + str(path).replace('\\', '/') + ')'
    project = output / "controls.pro"
    project.write_text("\n".join([
        "QT += core gui quick qml qmltest testlib", "CONFIG += console c++17 testcase", "CONFIG -= app_bundle",
        "TARGET = control_test_runner", "QMAKE_CXXFLAGS += -Wall -Wextra -Werror",
        "INCLUDEPATH += " + quote(source),
        "SOURCES += " + quote(ROOT / "tests/native/control_test_main.cpp"),
        *(["QMAKE_CC = clang", "QMAKE_CXX = clang++", "QMAKE_LINK = clang++"] if compiler == "clang" else []),
    ]) + "\n")
    qmake = shutil.which("qmake6") or shutil.which("qmake")
    if not qmake:
        raise RuntimeError("Install Qt qmake and Qt Quick Test development libraries")
    for index, command in enumerate(([qmake, str(project)], ["make", "-j2"])):
        result = subprocess.run(command, cwd=output, env=dict(os.environ, LANG="C.UTF-8"), text=True,
                                stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=180)
        (output / f"build-{index}.log").write_text(result.stdout)
        if result.returncode:
            raise RuntimeError(f"Qt control test runner build failed: {result.stdout[-14000:]}")
    return output / "control_test_runner"
