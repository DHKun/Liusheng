default: build

build:
    cargo build --workspace

test:
    cargo test --workspace

check:
    cargo clippy --workspace --all-targets -- -D warnings
    cargo fmt --check

# 开发用：扫描曲库并打印
scan dir="/data/Music":
    cargo run -p liusheng-core --example dev -- scan {{dir}}

# 开发用：解码一首歌输出 wav，验证解码正确性
decode file out="/tmp/liusheng-decode-test.wav":
    cargo run -p liusheng-core --example dev -- decode "{{file}}" "{{out}}"

install:
    @echo "应用 crate 尚未就绪，先装 Qt6 开发包，见 README"
