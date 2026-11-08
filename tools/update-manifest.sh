#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Firma los paquetes que se actualizan solos y arma el latest.json que consulta la app (ver
# crates/klavaro-app/src/update.rs). Lo corre el workflow de releases sobre dist/, con los
# paquetes de todos los sistemas ya juntos.
#
# Uso: tools/update-manifest.sh DIST DUEÑO/REPO CLAVE_SECRETA
#
#   DIST           carpeta con los paquetes (de tools/package.sh)
#   DUEÑO/REPO     repositorio de GitHub donde se publica el release
#   CLAVE_SECRETA  clave secreta de minisign sin contraseña (tools/update-keys.sh)
#
# Deja en DIST un .sig por paquete y latest.json. Necesita minisign y jq.

set -euo pipefail

[ $# -eq 3 ] || { echo "uso: $0 DIST DUEÑO/REPO CLAVE_SECRETA" >&2; exit 1; }
dist="$1"
repo="$2"
key="$3"

root="$(cd "$(dirname "$0")/.." && pwd)"
version="$(sed -n 's/^version = "\(.*\)"/\1/p' "$root/Cargo.toml" | head -n 1)"
[ -n "$version" ] || { echo "no se encontró la versión en Cargo.toml" >&2; exit 1; }

# Paquete de cada plataforma del actualizador (`<sistema>-<arq>`) y su formato.
platforms='{}'
add() {
    local file="$1" platform="$2" format="$3"
    local path="$dist/$file"
    [ -f "$path" ] || { echo "==> falta $file: $platform no se actualizará sola"; return; }
    minisign -S -s "$key" -m "$path" -x "$path.sig" -t "klavaro-rs $version $platform" >/dev/null
    local signature
    signature="$(base64 < "$path.sig" | tr -d '\n')"
    local url="https://github.com/$repo/releases/download/v$version/$file"
    platforms="$(jq --arg p "$platform" --arg u "$url" --arg s "$signature" --arg f "$format" \
        '. + {($p): {url: $u, signature: $s, format: $f}}' <<<"$platforms")"
    echo "==> $platform: $file"
}

add "klavaro-rs-$version-macos-aarch64.app.tar.gz" macos-aarch64 app
add "klavaro-rs-$version-macos-x86_64.app.tar.gz" macos-x86_64 app
add "klavaro-rs-$version-windows-x86_64.msi" windows-x86_64 wix
add "klavaro-rs-$version-linux-x86_64.AppImage" linux-x86_64 appimage

[ "$platforms" != '{}' ] || { echo "no hay paquetes para el actualizador en $dist" >&2; exit 1; }

jq -n --arg v "$version" --arg d "$(date -u +%Y-%m-%dT%H:%M:%SZ)" --argjson p "$platforms" \
    '{version: $v, pub_date: $d, platforms: $p}' > "$dist/latest.json"
echo "==> $dist/latest.json"
