# klavaro-rs

Reimplementación en Rust de [Klavaro](https://klavaro.sourceforge.io/), el tutor de
mecanografía libre e independiente del idioma y de la distribución de teclado.

> Estado: fase 7. Interfaz en iced (menú, ejercicio con colores y cursor, resultados) que
> guarda la configuración, el historial y el Top 10 de Fluidez, importa la primera vez los
> datos del Klavaro original, muestra los textos en los 40 idiomas del original y tiene
> gráficos de progreso y la tabla del Top 10 (teclas P y T en el menú). Rastrea errores y
> tiempos por tecla, con sus gráficos y la sesión especial de Adaptabilidad. Las preferencias
> (nombre del Top 10, párrafos de Fluidez, inicio automático y metas) se editan en pantalla. También hay una práctica alternativa en la terminal:
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
| `klavaro-core` | Dominio puro: sesión, métricas, metas, generadores, rastreo por tecla, ranking |
| `klavaro-data` | Formatos del original: `.kbd`, `basic_lessons.txt`, `.words`, `.paragraphs`, historial e INI |
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
