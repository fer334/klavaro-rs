// SPDX-License-Identifier: GPL-3.0-or-later
//! Ubicación de los archivos de klavaro-rs y del Klavaro original.

use std::path::{Path, PathBuf};

use directories::{BaseDirs, ProjectDirs};

use crate::{Result, StoreError};

/// Variable de entorno que reemplaza todas las carpetas por una sola (pruebas, modo portátil).
pub const HOME_ENV: &str = "KLAVARO_HOME";

/// Archivos de klavaro-rs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Paths {
    /// Carpeta de la configuración.
    pub config_dir: PathBuf,
    /// Carpeta de los datos (historial).
    pub data_dir: PathBuf,
}

impl Paths {
    /// Las carpetas del sistema, o `KLAVARO_HOME` si está definida.
    ///
    /// # Errors
    ///
    /// [`StoreError::NoHome`] si el sistema no informa una carpeta personal.
    pub fn detect() -> Result<Self> {
        if let Some(home) = std::env::var_os(HOME_ENV).filter(|v| !v.is_empty()) {
            return Ok(Self::portable(home));
        }
        let dirs = ProjectDirs::from("io.sourceforge", "Klavaro", "klavaro-rs")
            .ok_or(StoreError::NoHome)?;
        Ok(Self {
            config_dir: dirs.config_dir().to_path_buf(),
            data_dir: dirs.data_dir().to_path_buf(),
        })
    }

    /// Todo en una sola carpeta.
    pub fn portable(dir: impl Into<PathBuf>) -> Self {
        let dir = dir.into();
        Self {
            config_dir: dir.clone(),
            data_dir: dir,
        }
    }

    /// `config.toml`.
    pub fn config_file(&self) -> PathBuf {
        self.config_dir.join("config.toml")
    }

    /// Base de datos del historial.
    pub fn history_file(&self) -> PathBuf {
        self.data_dir.join("history.sqlite3")
    }
}

/// Carpetas donde el Klavaro original guarda `preferences.ini` y `stat_*.txt`.
///
/// `GLib` usa las rutas XDG también en macOS (`~/.config/klavaro`, `~/.local/share/klavaro`);
/// en Windows, `%LOCALAPPDATA%\klavaro`. Las estadísticas pueden estar en cualquiera de las
/// dos carpetas según la versión.
pub fn legacy_dirs() -> Vec<PathBuf> {
    let Some(base) = BaseDirs::new() else {
        return Vec::new();
    };
    if cfg!(windows) {
        return vec![base.data_local_dir().join("klavaro")];
    }
    let xdg = |var: &str, fallback: &str| {
        std::env::var_os(var)
            .filter(|v| !v.is_empty())
            .map_or_else(|| base.home_dir().join(fallback), PathBuf::from)
    };
    let config = xdg("XDG_CONFIG_HOME", ".config").join("klavaro");
    let data = xdg("XDG_DATA_HOME", ".local/share").join("klavaro");
    vec![config, data]
}

/// Si una carpeta contiene algo del Klavaro original.
pub fn has_legacy_data(dir: &Path) -> bool {
    ["preferences.ini", "stat_basic.txt", "stat_velo.txt"]
        .iter()
        .any(|f| dir.join(f).is_file())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn portable_layout() {
        let p = Paths::portable("/tmp/k");
        assert_eq!(p.config_file(), Path::new("/tmp/k/config.toml"));
        assert_eq!(p.history_file(), Path::new("/tmp/k/history.sqlite3"));
    }
}
