#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Genera el par de claves minisign con el que se firman las actualizaciones automáticas.
# Se hace una sola vez:
#
#   - la pública queda en packaging/updater.pub (se versiona; el workflow la embebe en la app);
#   - la secreta queda en el archivo que se indique, FUERA del repositorio. Su contenido va en
#     el secreto UPDATER_PRIVATE_KEY del repositorio de GitHub y conviene guardarla en un gestor
#     de contraseñas: si se pierde, las copias instaladas no aceptan paquetes firmados con otra.
#
# Uso: tools/update-keys.sh RUTA_DE_LA_CLAVE_SECRETA   (necesita minisign)

set -euo pipefail

[ $# -eq 1 ] || { echo "uso: $0 RUTA_DE_LA_CLAVE_SECRETA" >&2; exit 1; }
secret="$1"
root="$(cd "$(dirname "$0")/.." && pwd)"
public="$root/packaging/updater.pub"

case "$(cd "$(dirname "$secret")" && pwd)/" in
    "$root"/*) echo "la clave secreta no puede quedar dentro del repositorio" >&2; exit 1 ;;
esac
[ -e "$public" ] && { echo "ya existe $public: no se pisa" >&2; exit 1; }

# -W: sin contraseña, para que el workflow pueda firmar sin pedirla.
minisign -G -W -p "$public" -s "$secret"
chmod 600 "$secret"
echo
echo "Pública: $public (commitearla)"
echo "Secreta: $secret (copiar su contenido al secreto UPDATER_PRIVATE_KEY de GitHub)"
