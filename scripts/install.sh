#!/usr/bin/env bash
set -euo pipefail

project_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)
prefix=${PREFIX:-"${HOME:?无法确定用户目录}/.local"}
destdir=${DESTDIR:-}
desktop_id=io.github.dhkun.Liusheng
build_release=true

if [[ ${1:-} == "--no-build" ]]; then
    build_release=false
    shift
fi
if (( $# > 0 )); then
    printf '用法：%s [--no-build]\n' "$0" >&2
    exit 2
fi

if [[ "$prefix" != /* ]]; then
    printf 'PREFIX 必须是绝对路径：%s\n' "$prefix" >&2
    exit 2
fi
if [[ -n "$destdir" && "$destdir" != /* ]]; then
    printf 'DESTDIR 必须是绝对路径：%s\n' "$destdir" >&2
    exit 2
fi
if [[ "$destdir" == *$'\n'* || "$destdir" == *$'\r'* ]]; then
    printf 'DESTDIR 不能包含换行符\n' >&2
    exit 2
fi
if [[ "$prefix" == *'"'* || "$prefix" == *'`'* || "$prefix" == *'$'* \
    || "$prefix" == *'\'* || "$prefix" == *$'\n'* || "$prefix" == *$'\r'* ]]; then
    printf 'PREFIX 含有 desktop Exec 不支持的字符：%s\n' "$prefix" >&2
    exit 2
fi

install_root="${destdir}${prefix}"
desktop_template="$project_root/resources/$desktop_id.desktop.in"
desktop_file=$(mktemp --suffix=.desktop)
trap 'rm -f -- "$desktop_file"' EXIT

escaped_prefix=${prefix//&/\\&}
escaped_prefix=${escaped_prefix//|/\\|}
sed "s|@PREFIX@|$escaped_prefix|g" "$desktop_template" >"$desktop_file"
if command -v desktop-file-validate >/dev/null 2>&1; then
    desktop-file-validate "$desktop_file"
fi

if [[ "$build_release" == true ]]; then
    cargo build --release --locked -p liusheng --manifest-path "$project_root/Cargo.toml"
elif [[ ! -x "$project_root/target/release/liusheng" ]]; then
    printf '未找到 release 二进制：%s\n' "$project_root/target/release/liusheng" >&2
    exit 1
fi
install -Dm755 "$project_root/target/release/liusheng" "$install_root/bin/liusheng"
install -Dm644 "$desktop_file" "$install_root/share/applications/$desktop_id.desktop"
install -Dm644 \
    "$project_root/crates/liusheng/qml/assets/tray.svg" \
    "$install_root/share/icons/hicolor/scalable/apps/$desktop_id.svg"

if [[ -z "$destdir" ]]; then
    if command -v update-desktop-database >/dev/null 2>&1; then
        update-desktop-database "$prefix/share/applications" || \
            printf '警告：desktop 缓存更新失败\n' >&2
    fi
    if [[ -f "$prefix/share/icons/hicolor/index.theme" ]] \
        && command -v gtk-update-icon-cache >/dev/null 2>&1; then
        gtk-update-icon-cache -f -t "$prefix/share/icons/hicolor" || \
            printf '警告：图标缓存更新失败\n' >&2
    fi
fi

printf '留声已安装到 %s\n' "$install_root"
