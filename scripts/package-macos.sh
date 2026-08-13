#!/usr/bin/env bash
set -euo pipefail

project_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)
output_dir="$project_root/dist"
requested_version=
build_release=true

while (( $# > 0 )); do
    case "$1" in
        --no-build)
            build_release=false
            shift
            ;;
        --output)
            if (( $# < 2 )); then
                printf '%s 需要目录参数\n' "$1" >&2
                exit 2
            fi
            output_dir=$2
            shift 2
            ;;
        --version)
            if (( $# < 2 )); then
                printf '%s 需要版本参数\n' "$1" >&2
                exit 2
            fi
            requested_version=$2
            shift 2
            ;;
        *)
            printf '未知参数：%s\n' "$1" >&2
            exit 2
            ;;
    esac
done

if [[ $(uname -s) != "Darwin" ]]; then
    printf 'macOS 应用包必须在 macOS 上构建\n' >&2
    exit 1
fi
for command in cargo codesign ditto macdeployqt unzip; do
    if ! command -v "$command" >/dev/null 2>&1; then
        printf '缺少 %s\n' "$command" >&2
        exit 1
    fi
done

manifest="$project_root/crates/liusheng/Cargo.toml"
package_version=$(awk '
    /^\[package\]$/ { in_package = 1; next }
    /^\[/ { in_package = 0 }
    in_package && /^version = "/ {
        gsub(/^version = "|"$/, "")
        print
        exit
    }
' "$manifest")
if [[ -z "$package_version" ]]; then
    printf '无法从 %s 读取版本\n' "$manifest" >&2
    exit 1
fi
if [[ -n "$requested_version" && "$requested_version" != "$package_version" ]]; then
    printf '标签版本 %s 与应用版本 %s 不一致\n' "$requested_version" "$package_version" >&2
    exit 1
fi
version=${requested_version:-$package_version}

machine=$(uname -m)
case "$machine" in
    arm64) artifact_arch=arm64 ;;
    x86_64) artifact_arch=x86_64 ;;
    *)
        printf '不支持的 macOS 架构：%s\n' "$machine" >&2
        exit 1
        ;;
esac

if [[ "$build_release" == true ]]; then
    MACOSX_DEPLOYMENT_TARGET=${MACOSX_DEPLOYMENT_TARGET:-13.0} \
        cargo build --release --locked -p liusheng --manifest-path "$project_root/Cargo.toml"
elif [[ ! -x "$project_root/target/release/liusheng" ]]; then
    printf '未找到 release 二进制：%s\n' "$project_root/target/release/liusheng" >&2
    exit 1
fi

mkdir -p -- "$output_dir"
output_dir=$(cd -- "$output_dir" && pwd -P)
work_dir=$(mktemp -d "${TMPDIR:-/tmp}/liusheng-macos.XXXXXXXX")
cleanup() {
    rm -rf -- "$work_dir"
}
trap cleanup EXIT

app_bundle="$work_dir/Liusheng.app"
contents="$app_bundle/Contents"
mkdir -p -- "$contents/MacOS" "$contents/Resources"
install -m 755 "$project_root/target/release/liusheng" "$contents/MacOS/Liusheng"
sed -e "s/@VERSION@/$version/g" \
    "$project_root/packaging/macos/Info.plist.in" >"$contents/Info.plist"

macdeployqt "$app_bundle" \
    -always-overwrite \
    -qmldir="$project_root/crates/liusheng/qml"
codesign --force --deep --sign - "$app_bundle"
codesign --verify --deep --strict "$app_bundle"

artifact="$output_dir/liusheng-$version-macos-$artifact_arch.zip"
rm -f -- "$artifact"
ditto -c -k --sequesterRsrc --keepParent "$app_bundle" "$artifact"
unzip -tq "$artifact"
printf '已生成 %s\n' "$artifact"
