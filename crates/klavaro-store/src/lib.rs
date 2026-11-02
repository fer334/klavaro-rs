// SPDX-License-Identifier: GPL-3.0-or-later
//! Persistencia de klavaro-rs.
//!
//! - [`config`]: preferencias del usuario en `config.toml`.
//! - [`history`]: historial de sesiones, Top 10 y puntajes de Fluidez en `SQLite`.
//! - [`import`]: preferencias e historial del Klavaro original.
//! - [`paths`]: dónde vive cada archivo según el sistema.
//! - [`date`]: conversión entre fechas del calendario y segundos desde 1970.

pub mod config;
pub mod date;
pub mod history;
pub mod import;
pub mod paths;

use std::fmt;
use std::io;
use std::path::PathBuf;

pub use config::Config;
pub use history::{History, SessionRecord};
pub use paths::Paths;

/// Error de persistencia.
#[derive(Debug)]
pub enum StoreError {
    /// No se pudo leer o escribir un archivo.
    Io {
        /// Archivo afectado.
        path: PathBuf,
        /// Error del sistema.
        source: io::Error,
    },
    /// `config.toml` no es válido.
    Config {
        /// Archivo afectado.
        path: PathBuf,
        /// Descripción del problema.
        message: String,
    },
    /// Error de la base de datos del historial.
    Database(rusqlite::Error),
    /// El sistema no informa una carpeta personal.
    NoHome,
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, source } => write!(f, "{}: {source}", path.display()),
            Self::Config { path, message } => write!(f, "{}: {message}", path.display()),
            Self::Database(e) => write!(f, "historial: {e}"),
            Self::NoHome => write!(f, "no se encontró la carpeta personal del usuario"),
        }
    }
}

impl std::error::Error for StoreError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Database(e) => Some(e),
            Self::Config { .. } | Self::NoHome => None,
        }
    }
}

impl From<rusqlite::Error> for StoreError {
    fn from(e: rusqlite::Error) -> Self {
        Self::Database(e)
    }
}

/// Resultado de este crate.
pub type Result<T> = std::result::Result<T, StoreError>;

pub(crate) fn io_error(path: impl Into<PathBuf>) -> impl FnOnce(io::Error) -> StoreError {
    let path = path.into();
    move |source| StoreError::Io { path, source }
}
