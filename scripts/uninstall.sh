#!/usr/bin/env bash
set -euo pipefail

prefix=${PREFIX:-"${HOME:?无法确定用户目录}/.local"}
destdir=${DESTDIR:-}
desktop_id=io.github.dhkun.Liusheng

if [[ "$prefix" != /* ]]; then
    printf 'PREFIX 必须是绝对路径：%s\n' "$prefix" >&2
    exit 2
fi
if [[ -n "$destdir" && "$destdir" != /* ]]; then
    printf 'DESTDIR 必须是绝对路径：%s\n' "$destdir" >&2
    exit 2
fi
if [[ "$prefix" == *$'\n'* || "$prefix" == *$'\r'* \
    || "$destdir" == *$'\n'* || "$destdir" == *$'\r'* ]]; then
    printf '安装路径不能包含换行符\n' >&2
    exit 2
fi

install_root="${destdir}${prefix}"
rm -f -- \
    "$install_root/bin/liusheng" \
    "$install_root/share/applications/$desktop_id.desktop" \
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

printf '留声已从 %s 移除，曲库数据保持不变\n' "$install_root"
