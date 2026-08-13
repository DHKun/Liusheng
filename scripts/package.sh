#!/usr/bin/env bash
set -euo pipefail

project_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)
format=${1:-}
if [[ -z "$format" ]]; then
    printf '用法：%s <deb|rpm|arch> [--no-build] [--output DIR] [--version VERSION]\n' "$0" >&2
    exit 2
fi
shift

build_release=true
output_dir="$project_root/dist"
requested_version=
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

case "$format" in
    deb | rpm | arch) ;;
    *)
        printf '不支持的格式：%s\n' "$format" >&2
        exit 2
        ;;
esac

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
if [[ "$machine" != "x86_64" ]]; then
    printf '当前仅支持 x86_64 打包，检测到 %s\n' "$machine" >&2
    exit 1
fi

mkdir -p -- "$output_dir"
output_dir=$(cd -- "$output_dir" && pwd -P)
work_dir=$(mktemp -d --tmpdir liusheng-package.XXXXXXXX)
cleanup() {
    rm -rf -- "$work_dir"
}
trap cleanup EXIT

build_arch() {
    if [[ "$build_release" == false ]]; then
        printf 'Arch 打包需要在 makepkg 隔离目录中执行构建，不能使用 --no-build\n' >&2
        exit 2
    fi
    if (( EUID == 0 )); then
        printf 'makepkg 拒绝以 root 运行，请使用普通用户执行 Arch 打包\n' >&2
        exit 1
    fi
    if ! command -v makepkg >/dev/null 2>&1; then
        printf '缺少 makepkg，请在 Arch Linux 中安装 base-devel\n' >&2
        exit 1
    fi

    arch_work="$work_dir/arch"
    archive="$arch_work/liusheng-$version.tar.gz"
    mkdir -p -- "$arch_work"
    tar \
        --create \
        --gzip \
        --file "$archive" \
        --exclude='./.git' \
        --exclude='./dist' \
        --exclude='./target' \
        --transform "s|^\\./|Liusheng-$version/|" \
        --transform "s|^\\.$|Liusheng-$version|" \
        --directory "$project_root" \
        .
    archive_sha256=$(sha256sum "$archive" | awk '{ print $1 }')
    sed \
        -e "s/@VERSION@/$version/g" \
        -e "s/@SHA256@/$archive_sha256/g" \
        "$project_root/packaging/arch/PKGBUILD.in" >"$arch_work/PKGBUILD"

    (
        cd -- "$arch_work"
        makepkg --cleanbuild --noconfirm
    )
    mapfile -t artifacts < <(find "$arch_work" -maxdepth 1 -type f -name 'liusheng-*.pkg.tar.zst' | sort)
    if (( ${#artifacts[@]} != 1 )); then
        printf '预期生成一个 Arch 包，实际生成 %s 个\n' "${#artifacts[@]}" >&2
        exit 1
    fi
    artifact="$output_dir/$(basename -- "${artifacts[0]}")"
    install -Dm644 "${artifacts[0]}" "$artifact"
    printf '已生成 %s\n' "$artifact"
}

if [[ "$format" == "arch" ]]; then
    build_arch
    exit 0
fi

if [[ "$build_release" == true ]]; then
    cargo build --release --locked -p liusheng --manifest-path "$project_root/Cargo.toml"
elif [[ ! -x "$project_root/target/release/liusheng" ]]; then
    printf '未找到 release 二进制：%s\n' "$project_root/target/release/liusheng" >&2
    exit 1
fi

stage_root="$work_dir/root"
PREFIX=/usr DESTDIR="$stage_root" "$project_root/scripts/install.sh" --no-build

build_deb() {
    if ! command -v dpkg-deb >/dev/null 2>&1; then
        printf '缺少 dpkg-deb\n' >&2
        exit 1
    fi

    control_dir="$stage_root/DEBIAN"
    mkdir -p -- "$control_dir"
    installed_size=$(du -sk "$stage_root/usr" | awk '{ print $1 }')
    sed \
        -e "s/@VERSION@/$version/g" \
        -e "s/@ARCH@/amd64/g" \
        -e "s/@INSTALLED_SIZE@/$installed_size/g" \
        "$project_root/packaging/debian/control.in" >"$control_dir/control"
    install -Dm755 "$project_root/packaging/debian/postinst" "$control_dir/postinst"
    install -Dm755 "$project_root/packaging/debian/postrm" "$control_dir/postrm"

    artifact="$output_dir/liusheng_${version}_amd64.deb"
    dpkg-deb --root-owner-group --build "$stage_root" "$artifact"
    printf '已生成 %s\n' "$artifact"
}

build_rpm() {
    if ! command -v rpmbuild >/dev/null 2>&1; then
        printf '缺少 rpmbuild\n' >&2
        exit 1
    fi

    rpm_topdir="$work_dir/rpmbuild"
    mkdir -p -- \
        "$rpm_topdir/BUILD" \
        "$rpm_topdir/BUILDROOT" \
        "$rpm_topdir/RPMS" \
        "$rpm_topdir/SOURCES" \
        "$rpm_topdir/SPECS" \
        "$rpm_topdir/SRPMS"
    install -Dm755 "$stage_root/usr/bin/liusheng" "$rpm_topdir/SOURCES/liusheng"
    install -Dm644 \
        "$stage_root/usr/share/applications/io.github.dhkun.Liusheng.desktop" \
        "$rpm_topdir/SOURCES/io.github.dhkun.Liusheng.desktop"
    install -Dm644 \
        "$stage_root/usr/share/icons/hicolor/scalable/apps/io.github.dhkun.Liusheng.svg" \
        "$rpm_topdir/SOURCES/io.github.dhkun.Liusheng.svg"
    sed "s/@VERSION@/$version/g" \
        "$project_root/packaging/rpm/liusheng.spec.in" \
        >"$rpm_topdir/SPECS/liusheng.spec"

    rpmbuild -bb \
        --define "_topdir $rpm_topdir" \
        "$rpm_topdir/SPECS/liusheng.spec"
    mapfile -t artifacts < <(find "$rpm_topdir/RPMS" -type f -name 'liusheng-*.rpm' | sort)
    if (( ${#artifacts[@]} != 1 )); then
        printf '预期生成一个 RPM，实际生成 %s 个\n' "${#artifacts[@]}" >&2
        exit 1
    fi
    artifact="$output_dir/$(basename -- "${artifacts[0]}")"
    install -Dm644 "${artifacts[0]}" "$artifact"
    printf '已生成 %s\n' "$artifact"
}

case "$format" in
    deb) build_deb ;;
    rpm) build_rpm ;;
esac
