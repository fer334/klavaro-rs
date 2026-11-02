// SPDX-License-Identifier: GPL-3.0-or-later
//! Embebe los datos de `assets/` en el binario: genera `$OUT_DIR/assets.rs` con una tabla
//! `(nombre, contenido)` hecha de `include_str!`. Así el ejecutable no depende de dónde se
//! instale la carpeta de datos.

use std::env;
use std::fmt::Write as _;
use std::fs;
use std::path::PathBuf;

fn main() {
    let manifest = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap_or_default());
    let dir = manifest.join("../../assets");
    println!("cargo::rerun-if-changed={}", dir.display());

    let mut files: Vec<PathBuf> = fs::read_dir(&dir)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .map(|e| e.path())
                .filter(|p| p.is_file())
                .collect()
        })
        .unwrap_or_default();
    files.sort();

    let mut code = String::from("static EMBEDDED: &[(&str, &str)] = &[\n");
    for path in files {
        let (Some(name), Ok(abs)) = (
            path.file_name().and_then(|n| n.to_str()),
            path.canonicalize(),
        ) else {
            continue;
        };
        // `{:?}` escribe un literal de cadena válido de Rust, con las barras escapadas.
        let _ = writeln!(
            code,
            "    ({name:?}, include_str!({:?})),",
            abs.display().to_string()
        );
    }
    code.push_str("];\n");

    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap_or_default()).join("assets.rs");
    if let Err(e) = fs::write(&out, code) {
        panic!("no se pudo escribir {}: {e}", out.display());
    }
}
