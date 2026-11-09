// SPDX-License-Identifier: GPL-3.0-or-later
//! Dominio de klavaro-rs.
//!
//! Este crate contiene toda la lógica de Klavaro que no depende de la interfaz ni del disco:
//!
//! - [`keyboard`]: distribuciones de teclado, dedos y clasificación de caracteres.
//! - [`session`]: la máquina de estados de un ejercicio y la evaluación de cada tecla.
//! - [`metrics`]: precisión, velocidad, fluidez y puntaje.
//! - [`goals`]: metas, niveles y el comentario que se muestra al terminar.
//! - [`generators`]: el texto de cada módulo (básico, adaptabilidad, velocidad, fluidez).
//! - [`progression`]: secciones, metas por lección, estrellas y desbloqueo.
//! - [`tracker`]: errores y tiempos por tecla, y palabras para practicar teclas débiles.
//! - [`ranking`]: validaciones del Top 10.
//! - [`rng`]: generador pseudoaleatorio con semilla, para pruebas deterministas.
//!
//! El reloj y el azar se inyectan siempre: ninguna función de este crate lee la hora del
//! sistema ni usa una fuente de entropía global.

pub mod generators;
pub mod goals;
pub mod keyboard;
pub mod metrics;
pub mod progression;
pub mod ranking;
pub mod rng;
pub mod session;
pub mod tracker;

/// Los módulos de práctica: los cuatro de Klavaro (la Introducción no genera ejercicios) y la
/// Progresión, propia de klavaro-rs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Module {
    /// 1 - Curso básico.
    Basic,
    /// 2 - Adaptabilidad.
    Adaptability,
    /// 3 - Velocidad.
    Velocity,
    /// 4 - Fluidez.
    Fluidity,
    /// Progresión: tiene sus propias pantallas y metas, y por eso no está en [`Module::ALL`].
    Progression,
}

impl Module {
    /// Los módulos de Klavaro, en el orden del menú principal.
    pub const ALL: [Self; 4] = [
        Self::Basic,
        Self::Adaptability,
        Self::Velocity,
        Self::Fluidity,
    ];

    /// Nombre corto usado por Klavaro en archivos (`stat_basic.txt`, etc.).
    pub const fn legacy_name(self) -> &'static str {
        match self {
            Self::Basic => "basic",
            Self::Adaptability => "adapt",
            Self::Velocity => "velo",
            Self::Fluidity => "fluid",
            Self::Progression => "progression",
        }
    }

    /// Cómo se evalúa cada tecla en los ejercicios del módulo.
    pub const fn evaluation(self) -> Evaluation {
        match self {
            Self::Basic | Self::Adaptability | Self::Velocity => Evaluation::Forward,
            Self::Fluidity => Evaluation::Correction,
            Self::Progression => Evaluation::Free,
        }
    }
}

/// Forma de evaluar las teclas de un ejercicio (ver [`session`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Evaluation {
    /// Cada tecla avanza el cursor; los errores quedan marcados y Backspace se ignora.
    Forward,
    /// Tras un error hay que volver con Backspace y reescribir; lo reescrito cuenta como error.
    Correction,
    /// Cada tecla avanza el cursor y un error marca sólo su letra. Backspace vuelve atrás para
    /// corregir, sin ser obligatorio; el error corregido sigue contando.
    Free,
}
