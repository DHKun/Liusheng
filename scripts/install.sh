#!/usr/bin/env bash
set -euo pipefail

project_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)
target_dir=${CARGO_TARGET_DIR:-"$project_root/target"}
if [[ "$target_dir" != /* ]]; then target_dir="$PWD/$target_dir"; fi
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
icon_assets="$project_root/crates/liusheng/qml/assets/app-icon"
icon_hash=$(sha256sum "$icon_assets/liusheng.svg")
icon_hash=${icon_hash%% *}
# Absolute, content-addressed path prevents both theme substitution and stale
# pixmaps cached under the previous application icon name. DESTDIR stays local.
app_icon_path="$prefix/share/icons/hicolor/scalable/apps/$desktop_id.brand-${icon_hash:0:16}.svg"
desktop_template="$project_root/resources/$desktop_id.desktop.in"
desktop_file=$(mktemp --suffix=.desktop)
trap 'rm -f -- "$desktop_file"' EXIT

escaped_prefix=${prefix//&/\\&}
escaped_prefix=${escaped_prefix//|/\\|}
escaped_icon_path=${app_icon_path//&/\\&}
escaped_icon_path=${escaped_icon_path//|/\\|}
sed -e "s|@PREFIX@|$escaped_prefix|g" \
    -e "s|@APP_ICON_PATH@|$escaped_icon_path|g" "$desktop_template" >"$desktop_file"
if command -v desktop-file-validate >/dev/null 2>&1; then
    desktop-file-validate "$desktop_file"
fi

if [[ "$build_release" == true ]]; then
    cargo build --release --locked -p liusheng --manifest-path "$project_root/Cargo.toml" \
        --target-dir "$target_dir"
elif [[ ! -x "$target_dir/release/liusheng" ]]; then
    printf '未找到 release 二进制：%s\n' "$target_dir/release/liusheng" >&2
    exit 1
fi
install -Dm755 "$target_dir/release/liusheng" "$install_root/bin/liusheng"
install -Dm644 "$icon_assets/liusheng.svg" "${destdir}${app_icon_path}"
install -Dm644 "$desktop_file" "$install_root/share/applications/$desktop_id.desktop"
install -Dm644 \
    "$project_root/crates/liusheng/qml/assets/app-icon/liusheng.svg" \
    "$install_root/share/icons/hicolor/scalable/apps/$desktop_id.svg"

install -Dm644 "$icon_assets/liusheng-symbolic.svg" "$install_root/share/icons/hicolor/scalable/apps/$desktop_id-symbolic.svg"
for size in 16 24 32 48 64 128 256 512; do
    install -Dm644 "$icon_assets/$size.png" "$install_root/share/icons/hicolor/${size}x${size}/apps/$desktop_id.png"
    touch "$install_root/share/icons/hicolor/${size}x${size}/apps"
done
# Updating existing files alone may leave the icon theme directory timestamp unchanged.
touch "$install_root/share/icons/hicolor/scalable/apps" "$install_root/share/icons/hicolor"

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
    if command -v kbuildsycoca6 >/dev/null 2>&1; then
        kbuildsycoca6 --noincremental || \
            printf '警告：KDE 应用缓存更新失败，请重新登录桌面后查看图标\n' >&2
    fi
fi

printf '留声已安装到 %s\n' "$install_root"
printf '启动器图标：%s\n' "$app_icon_path"
printf '程序 SHA-256：'
sha256sum "$install_root/bin/liusheng" | cut -d ' ' -f 1
if [[ -z "$destdir" ]] && command -v pgrep >/dev/null 2>&1; then
    if pgrep -u "$(id -u)" -x liusheng >/dev/null; then
        printf '提示：检测到留声进程仍在运行。请在旧窗口按 Ctrl+Q 退出，再启动 %s/bin/liusheng。\n' "$prefix"
    fi
fi
