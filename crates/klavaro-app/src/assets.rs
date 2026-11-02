// SPDX-License-Identifier: GPL-3.0-or-later
//! Acceso a los datos de `assets/` (distribuciones, lecciones, palabras y párrafos).
//!
//! Los datos van embebidos en el binario (ver `build.rs`), así el ejecutable funciona
//! instalado en cualquier lugar. `KLAVARO_ASSETS` apunta a otra carpeta para usar datos
//! propios o editados en lugar de los embebidos.

use std::fs;
use std::io;
use std::path::PathBuf;

use klavaro_core::generators::basic::LessonMask;
use klavaro_core::keyboard::KeyboardLayout;
use klavaro_data::{kbd, lessons, texts};

include!(concat!(env!("OUT_DIR"), "/assets.rs"));

/// Carpeta de datos indicada con `KLAVARO_ASSETS`, si la hay.
pub(crate) fn dir() -> Option<PathBuf> {
    std::env::var_os("KLAVARO_ASSETS").map(PathBuf::from)
}

/// De dónde salen los datos, para los mensajes de error.
pub(crate) fn location() -> String {
    dir().map_or_else(
        || "(datos embebidos)".to_owned(),
        |d| d.display().to_string(),
    )
}

/// Lee un archivo de texto de los datos.
pub(crate) fn read(name: &str) -> io::Result<String> {
    let Some(dir) = dir() else {
        return EMBEDDED
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, text)| (*text).to_owned())
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, name.to_owned()));
    };
    let path = dir.join(name);
    fs::read_to_string(&path)
        .map_err(|e| io::Error::new(e.kind(), format!("{}: {e}", path.display())))
}

fn invalid(msg: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg.into())
}

/// Nombres (sin extensión) de los archivos con una extensión dada, ordenados.
pub(crate) fn names_with_ext(ext: &str) -> Vec<String> {
    let suffix = format!(".{ext}");
    let mut out: Vec<String> = match dir() {
        None => EMBEDDED
            .iter()
            .filter_map(|(n, _)| n.strip_suffix(&suffix))
            .map(str::to_owned)
            .collect(),
        Some(dir) => fs::read_dir(dir)
            .map(|entries| {
                entries
                    .filter_map(Result::ok)
                    .filter_map(|e| e.file_name().into_string().ok())
                    .filter_map(|n| n.strip_suffix(&suffix).map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default(),
    };
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

    #[test]
    fn embeds_every_asset() {
        let on_disk = fs::read_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets"))
            .map_or(0, |d| d.filter_map(Result::ok).count());
        assert_eq!(EMBEDDED.len(), on_disk);
        assert!(read("basic_lessons.txt").is_ok_and(|t| !t.trim().is_empty()));
    }
}
