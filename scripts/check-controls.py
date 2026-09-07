#!/usr/bin/env python3
"""Exercise shared QML controls with real pointer/keyboard events in an isolated window.

Usage: python3 scripts/check-controls.py [--runner /path/to/qmltestrunner]
Requires Qt Quick Test, Qt Quick Controls, and the Qt SVG image plugin.
"""
from __future__ import annotations

import argparse
import os
from pathlib import Path
import shutil
import subprocess
import tempfile


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--runner", type=Path)
    args = parser.parse_args()
    runner = args.runner or shutil.which("qmltestrunner") or "/usr/lib/qt6/bin/qmltestrunner"
    project = Path(__file__).resolve().parent.parent
    source = project / "crates/liusheng/qml"
    components = ["Theme", "Icon", "TrackTable", "NavigationItem", "CollectionView", "CoverArt", "ImmersivePlayer", "OutputPopover"] + [p.stem for p in source.glob("Quiet*.qml")]
    with tempfile.TemporaryDirectory(prefix="liusheng-controls-") as directory:
        root = Path(directory)
        ui = root / "ui"
        ui.mkdir()
        for name in components:
            shutil.copyfile(source / f"{name}.qml", ui / f"{name}.qml")
        shutil.copytree(source / "assets", ui / "assets")
        (ui / "qmldir").write_text("\n".join(
            f"{'singleton ' if name == 'Theme' else ''}{name} 1.0 {name}.qml" for name in components
        ) + "\n")
        shutil.copyfile(project / "tests/qml/tst_controls.qml", root / "tst_controls.qml")
        runtime = root / "runtime"
        runtime.mkdir(mode=0o700)
        env = os.environ.copy()
        env.update(QT_QPA_PLATFORM=os.environ.get("LIUSHENG_TEST_PLATFORM", "offscreen"), QT_QUICK_BACKEND="software", LANG="C.UTF-8",
                   XDG_RUNTIME_DIR=str(runtime), XDG_CACHE_HOME=str(root / "cache"))
        result = subprocess.run([str(runner), "-input", str(root)], env=env,
                                text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=45)
        print(result.stdout, end="")
        diagnostics = ("Binding loop detected", "ReferenceError:", "TypeError:", "Error decoding:", "Unable to assign", "Failed to create grabbing popup", "QMenuClassWindow")
        return result.returncode or int(any(message in result.stdout for message in diagnostics))


if __name__ == "__main__":
    raise SystemExit(main())
