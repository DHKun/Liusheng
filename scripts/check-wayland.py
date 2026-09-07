#!/usr/bin/env python3
"""Run the desktop/UI suite on a real, isolated Weston Wayland compositor.

Usage: python3 scripts/check-wayland.py /path/to/liusheng --output target/qa/wayland
Requires weston, qt6-wayland and the normal UI test dependencies. Uses a CPU renderer.
The compositor is private to this test; existing desktop sessions are untouched.
"""
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import sys
import tempfile
import time


def execute(command: list[str], env: dict[str, str], log: Path, timeout: float) -> None:
    with log.open('w') as stream:
        process = subprocess.Popen(command, env=env, stdout=stream, stderr=subprocess.STDOUT,
                                   start_new_session=True)
        try:
            code = process.wait(timeout=timeout)
        except subprocess.TimeoutExpired:
            os.killpg(process.pid, signal.SIGKILL)
            process.wait()
            raise RuntimeError(f'Timeout running {command}; see {log}') from None
    if code:
        raise RuntimeError(f'Command returned {code}: {command}\n{log.read_text()}')


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('binary', type=Path)
    parser.add_argument('--output', type=Path, default=Path('target/qa/wayland'))
    parser.add_argument('--scales', type=float, nargs='+', default=[1.0, 1.25, 1.5])
    args = parser.parse_args()
    binary = str(args.binary.resolve(strict=True))
    args.output.mkdir(parents=True, exist_ok=True)
    weston = shutil.which('weston')
    if not weston: parser.error('Install weston to run native Wayland tests')
    if any(scale < 1 or scale > 3 for scale in args.scales): parser.error('Scales must be 1–3')
    project = Path(__file__).resolve().parent.parent
    with tempfile.TemporaryDirectory(prefix='liusheng-wl-') as directory:
        root = Path(directory)
        runtime = root / 'runtime'; runtime.mkdir(mode=0o700)
        env = os.environ.copy()
        env.update(XDG_RUNTIME_DIR=str(runtime), XDG_SESSION_TYPE='wayland',
                   XDG_CONFIG_HOME=str(root / 'config'), XDG_CACHE_HOME=str(root / 'cache'),
                   HOME=str(root / 'home'), LANG='C.UTF-8')
        for key in ('DISPLAY', 'WAYLAND_DISPLAY', 'WAYLAND_SOCKET', 'QT_QPA_PLATFORMTHEME'):
            env.pop(key, None)
        socket = runtime / 'wayland-liusheng'
        log = args.output.resolve() / 'weston.log'
        with (args.output / 'compositor-launch.log').open('w') as stream:
            compositor = subprocess.Popen(
                [weston, '--backend=headless', '--renderer=pixman', '--width=2560', '--height=1600',
                 '--socket=wayland-liusheng', '--no-config', '--idle-time=0', f'--log={log}'],
                env=env, stdout=stream, stderr=subprocess.STDOUT, start_new_session=True)
            try:
                deadline = time.monotonic() + 12
                while not socket.exists():
                    if compositor.poll() is not None or time.monotonic() > deadline:
                        raise RuntimeError(f'Weston did not create a Wayland socket; see {log}')
                    time.sleep(0.1)
                # Absolute socket path remains valid while each fixture creates its own XDG runtime.
                env.update(WAYLAND_DISPLAY=str(socket), QT_QPA_PLATFORM='wayland',
                           LIUSHENG_TEST_PLATFORM='wayland', QT_QUICK_BACKEND='software')
                rows = []
                for scale in args.scales:
                    name = f'{int(round(scale * 100))}'
                    out = args.output.resolve() / name
                    out.mkdir(parents=True, exist_ok=True)
                    env['QT_SCALE_FACTOR'] = str(scale)
                    execute([sys.executable, str(project / 'scripts/check-controls.py')], env,
                            out / 'controls.log', 65)
                    execute([sys.executable, str(project / 'scripts/check-ui.py'), binary, '--output', str(out)],
                            env, out / 'suite.log', 110)
                    page_log = (out / 'pages.log').read_text()
                    if '[platform] wayland' not in page_log:
                        raise RuntimeError('Wayland backend was not positively identified in the test log')
                    rows.append({'scale': scale, 'platform': 'wayland', 'passed': True})
                    print(f'Wayland {name}%: controls, pages, popup input and functional checks passed', flush=True)
                (args.output / 'results.json').write_text(json.dumps({
                    'compositor': 'Weston headless/pixman', 'qt_backend': 'wayland/software', 'results': rows,
                    'scope': 'real Wayland windows + synthetic Qt input; KDE tray host, physical seat, native portals and GPU drivers need desktop validation'
                }, indent=2))
            finally:
                if compositor.poll() is None:
                    os.killpg(compositor.pid, signal.SIGTERM)
                    try: compositor.wait(timeout=5)
                    except subprocess.TimeoutExpired:
                        os.killpg(compositor.pid, signal.SIGKILL); compositor.wait()
    return 0


if __name__ == '__main__':
    try:
        raise SystemExit(main())
    except (RuntimeError, subprocess.SubprocessError) as exc:
        print(exc, file=sys.stderr)
        raise SystemExit(1)
