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

rm -f -- "$install_root/share/icons/hicolor/scalable/apps/$desktop_id-symbolic.svg"
# Remove only the content-addressed files created by our installer.
for file in "$install_root/share/icons/hicolor/scalable/apps/$desktop_id".brand-*.svg; do
    name=${file##*/}
    suffix=${name#"$desktop_id.brand-"}
    if [[ "$suffix" =~ ^[0-9a-f]{16}\.svg$ ]]; then rm -f -- "$file"; fi
done
for size in 16 24 32 48 64 128 256 512; do
    rm -f -- "$install_root/share/icons/hicolor/${size}x${size}/apps/$desktop_id.png"
done

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
        kbuildsycoca6 --noincremental || printf '警告：KDE 应用缓存更新失败\n' >&2
    fi

fi

printf '留声已从 %s 移除，曲库数据保持不变\n' "$install_root"
