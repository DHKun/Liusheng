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
    ./scripts/package.sh deb

package-rpm:
    ./scripts/package.sh rpm

package-arch:
    ./scripts/package.sh arch

package-macos:
    ./scripts/package-macos.sh

# 开发用：扫描曲库并打印
scan dir="/data/Music":
    cargo run -p liusheng-core --example dev -- scan {{ dir }}

# 开发用：解码一首歌输出 wav，验证解码正确性
decode file out="/tmp/liusheng-decode-test.wav":
    cargo run -p liusheng-core --example dev -- decode "{{ file }}" "{{ out }}"

# 开发用：经 PipeWire 播放，验证声音路径
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

install:
    ./scripts/install.sh

uninstall:
    ./scripts/uninstall.sh

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
