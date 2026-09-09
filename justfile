default: build

build:
    cargo build --workspace

test:
    cargo test --workspace

check:
    cargo clippy --workspace --all-targets -- -D warnings
    cargo fmt --check

run:
    cargo run -p liusheng

release:
    cargo build --release -p liusheng

package-deb:
    bash ./scripts/package.sh deb

package-rpm:
    bash ./scripts/package.sh rpm

package-appimage:
    python3 scripts/package-appimage.py

# 发布目标和 AppImage 打包回归。
package-contract-test:
    python3 -m unittest discover -s tests -p 'test_release_targets.py' -v
    python3 -m unittest discover -s tests -p 'test_appimage_package.py' -v

package-macos:
    bash ./scripts/package-macos.sh

# 开发用：扫描曲库并打印
scan dir="/data/Music":
    cargo run -p liusheng-core --example dev -- scan {{ dir }}

# 开发用：解码一首歌输出 wav，验证解码正确性
decode file out="/tmp/liusheng-decode-test.wav":
    cargo run -p liusheng-core --example dev -- decode "{{ file }}" "{{ out }}"

# 开发用：Linux 经 PipeWire、macOS 经 CoreAudio 播放，验证声音路径
[positional-arguments]
play +files:
    cargo run -p liusheng-core --example dev -- play "$@"

# 用静音验证目标设备的原生格式与 44.1 kHz 重采样
alsa-probe device="hw:Hybrid,0":
    cargo run -p liusheng-core --example dev -- alsa-probe "{{ device }}"

# 真实执行共享、独占、共享输出切换
output-smoke:
    timeout 20s env QT_QPA_PLATFORM=offscreen cargo run -p liusheng -- --output-smoke-test

volume-probe device="hw:Hybrid" element="PCM":
    cargo run -p liusheng-core --example dev -- volume-probe "{{ device }}" "{{ element }}"

# Invoke Bash explicitly so copied checkouts also work when script execute bits are lost.
install:
    bash ./scripts/install.sh

uninstall:
    bash ./scripts/uninstall.sh

# 使用隔离 HOME、曲库与 D-Bus 会话验证所有页面、歌单和播放恢复。
ui-test:
    cargo build --locked -p liusheng
    python3 scripts/check-ui.py "${CARGO_TARGET_DIR:-target}/debug/liusheng" --output target/qa

# 合成缓存曲库启动基准：离屏渲染，单独记录首帧、可交互与进程总时间。
startup-bench:
    cargo build --release --locked -p liusheng
    python3 scripts/benchmark-startup.py "${CARGO_TARGET_DIR:-target}/release/liusheng" --output target/qa/startup-benchmark.json

# 在 Linux 编译 CPAL 通用接口并运行额外缓冲测试；原生 macOS 由 CI 验证。
audio-contract-test:
    cargo test --workspace --features liusheng-core/coreaudio-compile-check --locked

# 鼠标与键盘控件回归：Qt Quick Test + SVG 插件。
ui-controls-test:
    python3 scripts/check-controls.py

# 使用隔离虚构曲库生成真实界面截图；需要 Pillow。
ui-preview:
    cargo build --locked -p liusheng
    python3 scripts/preview-ui.py "${CARGO_TARGET_DIR:-target}/debug/liusheng" --output target/qa/gui-preview

# Weston 原生 Wayland + Qt 输入回归，包含 100% / 125% / 150%。
wayland-test:
    cargo build --locked -p liusheng
    python3 scripts/check-wayland.py "${CARGO_TARGET_DIR:-target}/debug/liusheng" --output target/qa/wayland

# SVG 源资产、PNG 尺寸和 macOS ICNS 的可重复导出。
icons-check:
    python3 scripts/generate-icons.py --check

icons-export:
    python3 scripts/generate-icons.py --contact-sheet target/qa/icons.png

# Read-only inspection of installed launchers, binary hashes and old processes.
icons-diagnose:
    python3 scripts/diagnose-icons.py

# Exercise actual StatusNotifierItem pixels with a conflicting desktop theme.
tray-icon-test:
    cargo build --locked -p liusheng
    python3 scripts/check-tray-icon.py "${CARGO_TARGET_DIR:-target}/debug/liusheng" --output target/qa/tray-icon

# 原生开发示例回归：帮助、参数、扫描、搜索及 16/24 位解码；无需音频设备。
dev-cli-test:
    cargo build --locked -p liusheng-core --example dev
    python3 scripts/check-dev-cli.py "${CARGO_TARGET_DIR:-target}/debug/examples/dev"

# 原生文件监听 + 可控时钟防抖：每轮均须通过，首次失败立即退出。
watcher-test:
    python3 scripts/check-watcher.py --rounds 10 --output target/qa/watcher

# Apple Silicon 原生系统媒体控制回归（独立 AppKit 测试包）。
macos-media-test:
    python3 scripts/check-macos-media.py
