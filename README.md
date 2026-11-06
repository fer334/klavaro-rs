# klavaro-rs

Reimplementación en Rust de [Klavaro](https://klavaro.sourceforge.io/), el tutor de
mecanografía libre e independiente del idioma y de la distribución de teclado.

> Estado: fase 10. Interfaz en iced (menú, ejercicio con colores y cursor, resultados) que
> guarda la configuración, el historial y el Top 10 de Fluidez, importa la primera vez los
> datos del Klavaro original, muestra los textos en los 40 idiomas del original y tiene
> gráficos de progreso y la tabla del Top 10 (teclas P y T en el menú). Rastrea errores y
> tiempos por tecla, con sus gráficos y la sesión especial de Adaptabilidad. Las preferencias
> (nombre del Top 10, párrafos de Fluidez, inicio automático, pitido y metas) se editan en
> pantalla. Modo dictado con espeak (o `say` en macOS) y pitido al equivocarse. Hay paquetes
> para macOS, Linux y Windows (ver «Instalación»). En Básico y Adaptabilidad muestra el teclado
> virtual con el dedo de cada tecla. La sección Progresión (tecla N) sube de 20 a 100 PPM en 17
> secciones de 30 lecciones, con estrellas y textos en inglés. También hay una práctica alternativa
> en la terminal:
>
> ```sh
> cargo run                                   # abre la ventana
> cargo run -- --practicar basico --leccion 3 # práctica en la terminal
> cargo run -- --help
> ```
>
> `--no-default-features` compila sin iced (sólo terminal).

## Créditos

Klavaro es obra de Felipe E. F. de Castro y colaboradores (2005–2022), bajo GPL-3.0-or-later.
klavaro-rs reutiliza su diseño, sus fórmulas y sus datos (lecciones, distribuciones de
teclado, textos y traducciones), por lo que se distribuye con la misma licencia.

## Estructura

| Crate | Contenido |
| --- | --- |
| `klavaro-core` | Dominio puro: sesión, métricas, metas, generadores, rastreo por tecla, ranking, Progresión |
| `klavaro-data` | Formatos del original: `.kbd`, `basic_lessons.txt`, `.words`, `.paragraphs`, historial e INI, y el catálogo de la Progresión |
| `klavaro-store` | `config.toml`, historial y Top 10 en SQLite, importación del Klavaro original |
| `klavaro-i18n` | Catálogo de idiomas, detección y textos traducidos con Fluent |
| `klavaro-app` | Binario `klavaro-rs` (interfaz iced en la fase 2) |

Ver [ARCHITECTURE.md](ARCHITECTURE.md).

## Datos del usuario

| Archivo | Linux | macOS | Windows |
| --- | --- | --- | --- |
| `config.toml` | `~/.config/klavaro-rs/` | `~/Library/Application Support/io.sourceforge.Klavaro.klavaro-rs/` | `%APPDATA%\Klavaro\klavaro-rs\config\` |
| `history.sqlite3` | `~/.local/share/klavaro-rs/` | igual que `config.toml` | `%APPDATA%\Klavaro\klavaro-rs\data\` |

`KLAVARO_HOME=carpeta` guarda todo en esa carpeta (modo portátil y pruebas). Al abrir por
primera vez se importan `preferences.ini`, `stat_*.txt` y `scores_fluid.txt` del Klavaro
original (`~/.config/klavaro` y `~/.local/share/klavaro`), una sola vez.

## Instalación

Los paquetes salen de `tools/package.sh` (en `dist/`) o del workflow `Release` al subir una
etiqueta `v*`. Los datos de `assets/` van dentro del ejecutable; no hace falta instalarlos
aparte (`KLAVARO_ASSETS=carpeta` usa otros datos en su lugar).

- **macOS**: abrir el `.dmg` y arrastrar `klavaro-rs.app` a Aplicaciones. No está firmada:
  la primera vez, clic derecho → Abrir.
- **Linux**: en Debian y Ubuntu, `sudo apt install ./klavaro-rs_<versión>_amd64.deb`. Si no,
  descomprimir el `.tar.gz` y copiar `bin/` y `share/` a `~/.local/` (o a `/usr/local/`);
  incluye el `.desktop` y el ícono.
- **Windows**: el `.msi` instala en Archivos de programa con un acceso directo en el menú
  Inicio; el `.exe` suelto funciona sin instalar. No están firmados: Windows avisa la primera
  vez («Más información» → «Ejecutar de todas formas»). Se abre también una consola, que usa
  la práctica en la terminal.

Para el modo dictado hace falta `espeak-ng` o `espeak` (en macOS alcanza con `say`).

## Desarrollo

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets
cargo test --workspace
cargo run -- --help
```

Opcionales: `cargo deny check` (licencias y avisos de seguridad) y `cargo llvm-cov` (cobertura).

### Código original de referencia

Descargar Klavaro 3.14 en `upstream/` (no se versiona), por ejemplo:

```sh
mkdir upstream && cd upstream
apt source klavaro            # o descomprimir klavaro-3.14.tar.bz2 de SourceForge
```

## Traducciones

- `crates/klavaro-i18n/locales/klavaro/*.ftl`: mensajes del Klavaro original (nombres de
  módulos, estadísticas, comentarios, Top 10), generados desde sus `.po` con
  `python3 tools/po2ftl.py`. No se editan a mano.
- `crates/klavaro-i18n/locales/app/*.ftl`: textos propios de klavaro-rs (por ahora inglés y
  español). Una clave definida acá tiene prioridad sobre la del original; lo que falta en un
  idioma se muestra en inglés.

## Licencia

GPL-3.0-or-later. Ver [LICENSE](LICENSE).

Los textos de la Progresión (`assets/C.progression`) son obra de los colaboradores de
[Wikipedia](https://en.wikipedia.org) y [Simple English Wikipedia](https://simple.wikipedia.org), con
licencia [CC BY-SA 4.0](https://creativecommons.org/licenses/by-sa/4.0/). Se recortaron y adaptaron, y
esas adaptaciones se distribuyen con la misma licencia. Cada línea del archivo indica el artículo y
la revisión de origen, y la pantalla Créditos de la Progresión los enlaza.

Las herramientas que arman ese catálogo están en [tools/progression](tools/progression/README.md).
