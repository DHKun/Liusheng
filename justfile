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
