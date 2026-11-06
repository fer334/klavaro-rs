// SPDX-License-Identifier: GPL-3.0-or-later
//! Formatos de datos de Klavaro.
//!
//! Lee (y cuando corresponde, escribe) los archivos de texto del Klavaro original, para
//! reutilizar su contenido y para importar la configuración y el historial de quienes ya lo
//! usan:
//!
//! - [`catalog`]: textos de la Progresión (`*.progression`).
//! - [`kbd`]: distribuciones de teclado (`*.kbd`).
//! - [`lessons`]: lecciones del curso básico (`basic_lessons.txt`) y mapa de dedos
//!   (`fingers_position.txt`).
//! - [`texts`]: listas de palabras (`*.words`) y párrafos (`*.paragraphs`), y el filtrado de
//!   textos importados por el usuario.
//! - [`legacy`]: historial (`stat_*.txt`) y preferencias (`preferences.ini`) del original.
//!
//! Todas las funciones trabajan sobre `&str`: leer el disco es tarea de quien llama.

pub mod catalog;
pub mod kbd;
pub mod legacy;
pub mod lessons;
pub mod texts;

use std::fmt;

/// Error al interpretar un archivo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    /// Línea (desde 1) donde se encontró el problema, si aplica.
    pub line: Option<usize>,
    /// Descripción.
    pub kind: ParseErrorKind,
}

/// Tipo de error de formato.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseErrorKind {
    /// Faltan líneas.
    TooFewLines {
        /// Líneas esperadas.
        expected: usize,
        /// Líneas encontradas.
        found: usize,
    },
    /// Una fila tiene más caracteres de los permitidos.
    LineTooLong {
        /// Máximo permitido.
        max: usize,
        /// Encontrados.
        found: usize,
    },
    /// Carácter no válido en un archivo de máscaras o dedos.
    InvalidChar(char),
    /// Un número o campo no se pudo leer.
    InvalidField(String),
}

impl ParseError {
    pub(crate) fn at(line: usize, kind: ParseErrorKind) -> Self {
        Self {
            line: Some(line),
            kind,
        }
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(line) = self.line {
            write!(f, "línea {line}: ")?;
        }
        match &self.kind {
            ParseErrorKind::TooFewLines { expected, found } => {
                write!(f, "se esperaban {expected} líneas y hay {found}")
            }
            ParseErrorKind::LineTooLong { max, found } => {
                write!(f, "la fila tiene {found} caracteres (máximo {max})")
            }
            ParseErrorKind::InvalidChar(c) => write!(f, "carácter no válido {c:?}"),
            ParseErrorKind::InvalidField(s) => write!(f, "campo no válido {s:?}"),
        }
    }
}

impl std::error::Error for ParseError {}

/// Separa líneas aceptando finales `\n` y `\r\n`, y quita una marca BOM inicial.
pub(crate) fn lines(text: &str) -> impl Iterator<Item = &str> {
    text.strip_prefix('\u{FEFF}')
        .unwrap_or(text)
        .split('\n')
        .map(|l| l.strip_suffix('\r').unwrap_or(l))
}
