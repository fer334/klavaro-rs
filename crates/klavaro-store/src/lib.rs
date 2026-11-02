// SPDX-License-Identifier: GPL-3.0-or-later
//! Persistencia de klavaro-rs.
//!
//! Por ahora define la configuración en memoria con sus valores por defecto y validaciones.
//! La lectura y escritura en `config.toml` (`serde` + `toml`) y el historial en `SQLite`
//! (`rusqlite`) se agregan cuando haya acceso a crates.io.

use klavaro_core::Module;
use klavaro_core::generators::basic::MAX_LESSONS;
use klavaro_core::generators::fluidity::{DEFAULT_PARAGRAPHS, MAX_PARAGRAPHS};
use klavaro_core::goals::Goals;

/// Configuración del usuario.
// Son preferencias independientes de sí/no; agruparlas en un enum no aporta.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    /// Idioma de interfaz; `None` = el del sistema.
    pub language: Option<String>,
    /// Distribución de teclado; `None` = la del idioma.
    pub keyboard: Option<String>,
    /// Lección actual del curso básico (1–256).
    pub basic_lesson: u16,
    /// Párrafos de Fluidez (0 = texto completo, 1–10).
    pub fluid_paragraphs: u8,
    /// Leer en voz alta.
    pub speech: bool,
    /// Sonido al equivocarse.
    pub beep: bool,
    /// Abrir directamente el último módulo usado.
    pub autostart: bool,
    /// Último módulo usado.
    pub last_module: Option<Module>,
    /// Usar los colores alternativos (modo oscuro).
    pub alt_colors: bool,
    /// Fuente del ejercicio (familia y tamaño).
    pub lesson_font: Option<String>,
    /// Metas y niveles.
    pub goals: Goals,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            language: None,
            keyboard: None,
            basic_lesson: 1,
            fluid_paragraphs: u8::try_from(DEFAULT_PARAGRAPHS).unwrap_or(3),
            speech: true,
            beep: false,
            autostart: false,
            last_module: None,
            alt_colors: false,
            lesson_font: None,
            goals: Goals::default(),
        }
    }
}

impl Config {
    /// Corrige valores fuera de rango (por ejemplo, de un archivo editado a mano).
    #[must_use]
    pub fn sanitized(mut self) -> Self {
        self.basic_lesson = self.basic_lesson.clamp(1, MAX_LESSONS);
        let max_par = u8::try_from(MAX_PARAGRAPHS).unwrap_or(10);
        self.fluid_paragraphs = self.fluid_paragraphs.min(max_par);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults() {
        let c = Config::default();
        assert_eq!(c.basic_lesson, 1);
        assert_eq!(c.fluid_paragraphs, 3);
        assert!(c.speech);
    }

    #[test]
    fn sanitizes_out_of_range_values() {
        let c = Config {
            basic_lesson: 0,
            fluid_paragraphs: 99,
            ..Config::default()
        }
        .sanitized();
        assert_eq!(c.basic_lesson, 1);
        assert_eq!(c.fluid_paragraphs, 10);
        let c = Config {
            basic_lesson: 999,
            ..Config::default()
        }
        .sanitized();
        assert_eq!(c.basic_lesson, MAX_LESSONS);
    }
}
