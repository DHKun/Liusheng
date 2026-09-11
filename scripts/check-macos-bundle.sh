#!/usr/bin/env bash
set -euo pipefail

# Shared by main-branch CI and tag releases. Verify the actual relocated archive.
if (( $# != 2 )); then
    printf 'Usage: %s PACKAGE.zip OUTPUT_DIRECTORY\n' "$0" >&2
    exit 2
fi
if [[ $(uname -s) != Darwin ]]; then
    printf 'macOS bundle validation requires a native macOS runner\n' >&2
    exit 1
fi
for command in ditto codesign lipo python3; do
    command -v "$command" >/dev/null 2>&1 || { printf 'Missing %s\n' "$command" >&2; exit 1; }
done
package=$1
if [[ ! -f "$package" || ! -s "$package" ]]; then
    printf 'Package is missing or empty: %s\n' "$package" >&2
    exit 1
fi
project_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)
mkdir -p -- "$2"
output=$(cd -- "$2" && pwd -P)
work=$(mktemp -d "${TMPDIR:-/tmp}/liusheng-bundle-check.XXXXXXXX")
trap 'rm -rf -- "$work"' EXIT
bundle="$work/Liusheng.app"
binary="$bundle/Contents/MacOS/Liusheng"
{
    ditto -x -k "$package" "$work"
    test -x "$binary"
    for resource in "$bundle/Contents/PlugIns/platforms/libqcocoa.dylib" "$bundle/Contents/Resources/qt.conf"; do
        test -s "$resource" || { printf 'Missing bundle resource: %s\n' "$resource" >&2; exit 1; }
    done
    ls -l "$bundle/Contents/PlugIns/platforms/"
    cat "$bundle/Contents/Resources/qt.conf"
    codesign --verify --deep --strict "$bundle"
    architecture=$(lipo -archs "$binary")
    printf '\nArchitecture: %s\n' "$architecture"
    if [[ "$architecture" != arm64 ]]; then
        printf 'Expected an arm64-only bundle\n' >&2
        exit 1
    fi
} 2>&1 | tee "$output/bundle.log"
python3 "$project_root/scripts/check-desktop-smoke.py" "$binary" \
    --platform cocoa --debug-plugins --output "$output/desktop.log"
