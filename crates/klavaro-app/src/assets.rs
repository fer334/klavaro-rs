// SPDX-License-Identifier: GPL-3.0-or-later
//! Acceso a los datos de `assets/` (distribuciones, lecciones, palabras y párrafos).
//!
//! Por ahora se leen del disco: `KLAVARO_ASSETS` o la carpeta `assets/` del repositorio.
//! Embeberlos en el binario queda para el empaquetado.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use klavaro_core::generators::basic::LessonMask;
use klavaro_core::keyboard::KeyboardLayout;
use klavaro_data::{kbd, lessons, texts};

/// Carpeta de datos.
pub(crate) fn dir() -> PathBuf {
    std::env::var_os("KLAVARO_ASSETS").map_or_else(
        || Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets"),
        PathBuf::from,
    )
}

/// Lee un archivo de texto de la carpeta de datos.
pub(crate) fn read(name: &str) -> io::Result<String> {
    let path = dir().join(name);
    fs::read_to_string(&path)
        .map_err(|e| io::Error::new(e.kind(), format!("{}: {e}", path.display())))
}

fn invalid(msg: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg.into())
}

/// Nombres (sin extensión) de los archivos con una extensión dada, ordenados.
pub(crate) fn names_with_ext(ext: &str) -> Vec<String> {
    let mut out: Vec<String> = fs::read_dir(dir())
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|e| e == ext))
                .filter_map(|p| p.file_stem().map(|s| s.to_string_lossy().into_owned()))
                .collect()
        })
        .unwrap_or_default();
    out.sort();
    out
}

/// Distribuciones disponibles.
pub(crate) fn keyboards() -> Vec<String> {
    names_with_ext("kbd")
}

/// Idiomas con textos disponibles.
pub(crate) fn languages() -> Vec<String> {
    names_with_ext("words")
}

/// Carga una distribución.
pub(crate) fn keyboard(name: &str) -> io::Result<KeyboardLayout> {
    kbd::parse(name, &read(&format!("{name}.kbd"))?).map_err(|e| invalid(e.to_string()))
}

/// Carga las 43 lecciones del curso básico.
pub(crate) fn basic_lessons() -> io::Result<Vec<LessonMask>> {
    lessons::parse_basic_lessons(&read("basic_lessons.txt")?).map_err(|e| invalid(e.to_string()))
}

/// Palabras de un idioma.
pub(crate) fn words(language: &str) -> io::Result<Vec<String>> {
    Ok(texts::parse_words(&read(&format!("{language}.words"))?))
}

/// Párrafos de un idioma.
pub(crate) fn paragraphs(language: &str) -> io::Result<Vec<String>> {
    Ok(texts::parse_paragraphs(&read(&format!(
        "{language}.paragraphs"
    ))?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_repository_assets() {
        assert!(keyboards().contains(&"qwerty_es".to_owned()));
        assert!(languages().contains(&"es".to_owned()));
        assert!(keyboard("qwerty_us").is_ok());
        assert!(keyboard("no_existe").is_err());
    }
}
