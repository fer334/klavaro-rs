#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Compila klavaro-rs en modo release y arma el paquete del sistema actual en dist/:
#
#   macOS:   klavaro-rs-<versión>-macos-<arq>.zip con klavaro-rs.app (sin firmar)
#   Linux:   klavaro-rs-<versión>-linux-<arq>.tar.gz con bin/, share/applications e ícono
#   Windows: klavaro-rs-<versión>-windows-<arq>.zip con klavaro-rs.exe (Git Bash o MSYS2)
#
# Uso: tools/package.sh [TRIPLE]   (por defecto, el del sistema)
#
# Los datos de assets/ van embebidos en el binario, así que el paquete no los incluye aparte.

set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"

target="${1:-}"
version="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n 1)"
[ -n "$version" ] || { echo "no se encontró la versión en Cargo.toml" >&2; exit 1; }

build_args=(build --release --locked -p klavaro-app)
bin_dir="target/release"
if [ -n "$target" ]; then
    build_args+=(--target "$target")
    bin_dir="target/$target/release"
    arch="${target%%-*}"
else
    arch="$(uname -m)"
fi
[ "$arch" = "arm64" ] && arch="aarch64"

case "$(uname -s)" in
    Darwin) os=macos ;;
    Linux) os=linux ;;
    MINGW* | MSYS* | CYGWIN*) os=windows ;;
    *) echo "sistema no soportado: $(uname -s)" >&2; exit 1 ;;
esac

echo "==> cargo ${build_args[*]}"
cargo "${build_args[@]}"

name="klavaro-rs-$version-$os-$arch"
stage="target/package/$name"
rm -rf "$stage"
mkdir -p "$stage" dist

docs() {
    cp LICENSE README.md "$1/"
}

case "$os" in
    macos)
        app="$stage/klavaro-rs.app/Contents"
        mkdir -p "$app/MacOS" "$app/Resources"
        cp "$bin_dir/klavaro-rs" "$app/MacOS/"
        sed "s/@VERSION@/$version/g" packaging/macos/Info.plist > "$app/Info.plist"
        # Ícono .icns a partir del SVG (sips e iconutil vienen con macOS).
        iconset="target/package/klavaro-rs.iconset"
        rm -rf "$iconset"
        mkdir -p "$iconset"
        for size in 16 32 128 256 512; do
            sips -s format png -z "$size" "$size" packaging/klavaro-rs.svg \
                --out "$iconset/icon_${size}x${size}.png" >/dev/null
            double=$((size * 2))
            sips -s format png -z "$double" "$double" packaging/klavaro-rs.svg \
                --out "$iconset/icon_${size}x${size}@2x.png" >/dev/null
        done
        iconutil -c icns "$iconset" -o "$app/Resources/klavaro-rs.icns"
        docs "$stage"
        rm -f "dist/$name.zip"
        (cd "$stage" && zip -qry "$root/dist/$name.zip" .)
        out="dist/$name.zip"
        ;;
    linux)
        mkdir -p "$stage/bin" "$stage/share/applications" \
            "$stage/share/icons/hicolor/scalable/apps" "$stage/share/icons/hicolor/256x256/apps"
        cp "$bin_dir/klavaro-rs" "$stage/bin/"
        cp packaging/linux/io.sourceforge.klavaro-rs.desktop "$stage/share/applications/"
        cp packaging/klavaro-rs.svg "$stage/share/icons/hicolor/scalable/apps/"
        cp packaging/klavaro-rs.png "$stage/share/icons/hicolor/256x256/apps/"
        docs "$stage"
        tar -C target/package -czf "dist/$name.tar.gz" "$name"
        out="dist/$name.tar.gz"
        ;;
    windows)
        cp "$bin_dir/klavaro-rs.exe" "$stage/"
        docs "$stage"
        rm -f "dist/$name.zip"
        (cd target/package && 7z a -tzip "$root/dist/$name.zip" "$name" >/dev/null)
        out="dist/$name.zip"
        ;;
esac

echo "==> $out"
