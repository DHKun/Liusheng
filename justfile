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

install:
    ./scripts/install.sh

uninstall:
    ./scripts/uninstall.sh
