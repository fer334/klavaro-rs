#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Compila klavaro-rs en modo release y arma los paquetes del sistema actual en dist/:
#
#   macOS:   klavaro-rs-<versión>-macos-<arq>.dmg con klavaro-rs.app (sin firmar) y el atajo a
#            Aplicaciones para instalarla arrastrándola
#   Linux:   klavaro-rs-<versión>-linux-<arq>.tar.gz con bin/, share/applications e ícono, y
#            klavaro-rs_<versión>_<arq-debian>.deb (si hay dpkg-deb)
#   Windows: klavaro-rs-<versión>-windows-<arq>.msi (instalador, WiX 5) y
#            klavaro-rs-<versión>-windows-<arq>.exe (portable), desde Git Bash o MSYS2
#
# Uso: tools/package.sh [TRIPLE]   (por defecto, el del sistema)
#
# Los datos de assets/ van embebidos en el binario, así que los paquetes no los incluyen aparte.

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
outputs=()

docs() {
    cp LICENSE README.md "$1/"
}

# Paquete .deb a partir de la carpeta ya armada para el .tar.gz.
deb() {
    if ! command -v dpkg-deb >/dev/null; then
        echo "==> sin dpkg-deb: no se arma el .deb"
        return
    fi
    local deb_arch
    case "$arch" in
        x86_64) deb_arch=amd64 ;;
        aarch64) deb_arch=arm64 ;;
        *) deb_arch="$arch" ;;
    esac
    local tree="target/package/deb"
    rm -rf "$tree"
    mkdir -p "$tree/DEBIAN" "$tree/usr" "$tree/usr/share/doc/klavaro-rs"
    cp -R "$stage/bin" "$stage/share" "$tree/usr/"
    cp README.md "$tree/usr/share/doc/klavaro-rs/"
    cat > "$tree/usr/share/doc/klavaro-rs/copyright" <<EOF
Format: https://www.debian.org/doc/packaging-manuals/copyright-format/1.0/
Upstream-Name: klavaro-rs
Source: https://github.com/fer334/klavaro-rs

Files: *
License: GPL-3.0-or-later
 On Debian systems, the full text of the GNU General Public License version 3
 can be found in /usr/share/common-licenses/GPL-3.
 .
 The Progression texts come from Wikipedia and Simple English Wikipedia, under
 CC BY-SA 4.0 (https://creativecommons.org/licenses/by-sa/4.0/), shortened and
 adapted. The Credits screen links each text to its source article.
EOF
    # Bibliotecas que necesita el binario, como las calcula dpkg-shlibdeps (que pide una
    # carpeta debian/ con control); si falla, sólo libc.
    local depends="libc6"
    if command -v dpkg-shlibdeps >/dev/null; then
        local probe="target/package/shlibdeps"
        rm -rf "$probe"
        mkdir -p "$probe/debian"
        touch "$probe/debian/control"
        if found="$(cd "$probe" && dpkg-shlibdeps -O "$root/$tree/usr/bin/klavaro-rs" 2>/dev/null)"; then
            depends="${found#shlibs:Depends=}"
        fi
    fi
    local size
    size="$(du -sk "$tree/usr" | cut -f1)"
    cat > "$tree/DEBIAN/control" <<EOF
Package: klavaro-rs
Version: $version
Architecture: $deb_arch
Maintainer: Fer Rios <luis.fer334.fa@gmail.com>
Installed-Size: $size
Depends: $depends
Section: education
Priority: optional
Homepage: https://github.com/fer334/klavaro-rs
Description: tutor de mecanografía (reimplementación de Klavaro)
 Curso básico, adaptabilidad, velocidad y fluidez, con teclado en pantalla,
 dictado, gráficos de progreso y Top 10.
EOF
    local out="dist/klavaro-rs_${version}_${deb_arch}.deb"
    dpkg-deb --root-owner-group --build "$tree" "$out" >/dev/null
    outputs+=("$out")
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
        # Imagen de disco con la app y el atajo a Aplicaciones para arrastrarla.
        ln -s /Applications "$stage/Applications"
        out="dist/$name.dmg"
        rm -f "$out"
        hdiutil create -volname klavaro-rs -srcfolder "$stage" -fs HFS+ -format UDZO \
            -ov "$out" >/dev/null
        outputs+=("$out")
        ;;
    linux)
        mkdir -p "$stage/bin" "$stage/share/applications" \
            "$stage/share/icons/hicolor/scalable/apps" "$stage/share/icons/hicolor/256x256/apps"
        cp "$bin_dir/klavaro-rs" "$stage/bin/"
        cp packaging/linux/io.sourceforge.klavaro-rs.desktop "$stage/share/applications/"
        cp packaging/klavaro-rs.svg "$stage/share/icons/hicolor/scalable/apps/"
        cp packaging/klavaro-rs.png "$stage/share/icons/hicolor/256x256/apps/"
        deb
        docs "$stage"
        tar -C target/package -czf "dist/$name.tar.gz" "$name"
        outputs+=("dist/$name.tar.gz")
        ;;
    windows)
        # Portable: el .exe solo (los datos van embebidos).
        cp "$bin_dir/klavaro-rs.exe" "dist/$name.exe"
        outputs+=("dist/$name.exe")
        # Instalador con WiX 5 (herramienta de .NET; se instala si falta).
        if ! command -v wix >/dev/null; then
            dotnet tool install --global wix --version 5.0.2 >/dev/null
            export PATH="$PATH:$HOME/.dotnet/tools:${USERPROFILE:-$HOME}/.dotnet/tools"
        fi
        # Windows Installer sólo admite versiones numéricas (sin `-beta` ni `+...`).
        msi_version="${version%%[-+]*}"
        case "$arch" in
            aarch64) msi_arch=arm64 ;;
            *) msi_arch=x64 ;;
        esac
        out="dist/$name.msi"
        rm -f "$out"
        wix build packaging/windows/klavaro-rs.wxs -arch "$msi_arch" \
            -d "Version=$msi_version" -d "Exe=$bin_dir/klavaro-rs.exe" -d "Root=." \
            -o "$out"
        rm -f "dist/$name.wixpdb"
        outputs+=("$out")
        ;;
esac

for out in "${outputs[@]}"; do
    echo "==> $out"
done
