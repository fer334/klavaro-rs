# klavaro-rs

Reimplementación en Rust de [Klavaro](https://klavaro.sourceforge.io/), el tutor de
mecanografía libre e independiente del idioma y de la distribución de teclado.

> Estado: fases 0 y 1 (esqueleto y núcleo de dominio). Todavía no hay interfaz gráfica.

## Créditos

Klavaro es obra de Felipe E. F. de Castro y colaboradores (2005–2022), bajo GPL-3.0-or-later.
klavaro-rs reutiliza su diseño, sus fórmulas y sus datos (lecciones, distribuciones de
teclado, textos y traducciones), por lo que se distribuye con la misma licencia.

## Estructura

| Crate | Contenido |
| --- | --- |
| `klavaro-core` | Dominio puro: sesión, métricas, metas, generadores, rastreo por tecla, ranking |
| `klavaro-data` | Formatos del original: `.kbd`, `basic_lessons.txt`, `.words`, `.paragraphs`, historial e INI |
| `klavaro-store` | Configuración (y, más adelante, historial en SQLite) |
| `klavaro-i18n` | Catálogo de idiomas y detección (y, más adelante, traducciones Fluent) |
| `klavaro-app` | Binario `klavaro-rs` (interfaz iced en la fase 2) |

Ver [ARCHITECTURE.md](ARCHITECTURE.md).

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

## Licencia

GPL-3.0-or-later. Ver [LICENSE](LICENSE).
