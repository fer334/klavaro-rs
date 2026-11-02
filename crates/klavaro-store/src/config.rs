// SPDX-License-Identifier: GPL-3.0-or-later
//! Preferencias del usuario y su archivo `config.toml`.
//!
//! El archivo se lee con tolerancia: las claves que faltan toman el valor por defecto y los
//! valores fuera de rango se corrigen con [`Config::sanitized`].

use std::fs;
use std::io;
use std::path::Path;

use klavaro_core::Module;
use klavaro_core::generators::basic::MAX_LESSONS;
use klavaro_core::generators::fluidity::{DEFAULT_PARAGRAPHS, MAX_PARAGRAPHS};
use klavaro_core::goals::{Goal, Goals};
use klavaro_data::legacy::LegacyPreferences;
use serde::{Deserialize, Serialize};

use crate::{Result, StoreError, io_error};

/// Configuración del usuario.
// Son preferencias independientes de sí/no; agruparlas en un enum no aporta.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    /// Idioma de interfaz; `None` = el del sistema.
    pub language: Option<String>,
    /// Idioma de los textos de práctica; `None` = el de la interfaz.
    pub text_language: Option<String>,
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
    /// Mostrar el teclado virtual con los dedos durante el ejercicio.
    pub show_keyboard: bool,
    /// Fuente del ejercicio (familia y tamaño).
    pub lesson_font: Option<String>,
    /// Nombre para el Top 10; `None` = el del usuario del sistema.
    pub player_name: Option<String>,
    /// Metas y niveles.
    pub goals: Goals,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            language: None,
            text_language: None,
            keyboard: None,
            basic_lesson: 1,
            fluid_paragraphs: u8::try_from(DEFAULT_PARAGRAPHS).unwrap_or(3),
            speech: true,
            beep: false,
            autostart: false,
            last_module: None,
            alt_colors: false,
            show_keyboard: true,
            lesson_font: None,
            player_name: None,
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
        for goal in [
            &mut self.goals.basic,
            &mut self.goals.adaptability,
            &mut self.goals.velocity,
            &mut self.goals.fluidity,
        ] {
            goal.accuracy = clamp_or(goal.accuracy, 0.0, 100.0, 95.0);
            goal.speed = clamp_or(goal.speed, 0.0, 200.0, 10.0);
            goal.fluidity = goal.fluidity.map(|f| clamp_or(f, 0.0, 100.0, 70.0));
        }
        self
    }

    /// Lee `config.toml`. Devuelve `None` si el archivo no existe.
    ///
    /// # Errors
    ///
    /// Si no se puede leer o no es TOML válido.
    pub fn load(path: &Path) -> Result<Option<Self>> {
        let text = match fs::read_to_string(path) {
            Ok(t) => t,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(io_error(path)(e)),
        };
        Self::from_toml(&text)
            .map(Some)
            .map_err(|message| StoreError::Config {
                path: path.to_path_buf(),
                message,
            })
    }

    /// Guarda `config.toml` (crea la carpeta si hace falta y reemplaza el archivo de una vez).
    ///
    /// # Errors
    ///
    /// Si no se puede escribir.
    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir).map_err(io_error(dir))?;
        }
        let tmp = path.with_extension("toml.tmp");
        fs::write(&tmp, self.to_toml()).map_err(io_error(&tmp))?;
        fs::rename(&tmp, path).map_err(io_error(path))
    }

    /// Interpreta el contenido de `config.toml`.
    ///
    /// # Errors
    ///
    /// Descripción del error de sintaxis o de tipos.
    pub fn from_toml(text: &str) -> std::result::Result<Self, String> {
        let file: ConfigFile = toml::from_str(text).map_err(|e| e.to_string())?;
        Ok(file.into_config().sanitized())
    }

    /// Contenido de `config.toml`.
    pub fn to_toml(&self) -> String {
        // Serializar estructuras propias sin claves raras no puede fallar.
        toml::to_string(&ConfigFile::from_config(self)).unwrap_or_default()
    }

    /// Aplica las preferencias importadas del Klavaro original.
    #[must_use]
    pub fn with_legacy(mut self, prefs: &LegacyPreferences) -> Self {
        if let Some(l) = &prefs.language {
            self.language = Some(l.clone());
        }
        if let Some(k) = &prefs.keyboard {
            // El original guarda el nombre con espacios en lugar de `_`.
            self.keyboard = Some(k.replace(' ', "_"));
        }
        if let Some(l) = prefs.basic_lesson {
            self.basic_lesson = l;
        }
        if let Some(p) = prefs.fluid_paragraphs {
            self.fluid_paragraphs = p;
        }
        if let Some(s) = prefs.speech {
            self.speech = s;
        }
        if let Some(f) = &prefs.lesson_font {
            self.lesson_font = Some(f.clone());
        }
        self.sanitized()
    }
}

fn clamp_or(v: f64, min: f64, max: f64, fallback: f64) -> f64 {
    if v.is_finite() {
        v.clamp(min, max)
    } else {
        fallback
    }
}

/// Nombre de un módulo en `config.toml`.
pub fn module_key(m: Module) -> &'static str {
    m.legacy_name()
}

/// Módulo a partir de su nombre en `config.toml`.
pub fn module_from_key(s: &str) -> Option<Module> {
    Module::ALL.into_iter().find(|m| m.legacy_name() == s)
}

/// Forma del archivo en disco. Todo es opcional para tolerar archivos viejos o editados.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
struct ConfigFile {
    #[serde(skip_serializing_if = "Option::is_none")]
    language: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    text_language: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    keyboard: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    player_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    last_module: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    lesson_font: Option<String>,
    basic_lesson: Option<u16>,
    fluid_paragraphs: Option<u8>,
    speech: Option<bool>,
    beep: Option<bool>,
    autostart: Option<bool>,
    alt_colors: Option<bool>,
    show_keyboard: Option<bool>,
    goals: GoalsFile,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
struct GoalsFile {
    basic: Option<GoalFile>,
    adaptability: Option<GoalFile>,
    velocity: Option<GoalFile>,
    fluidity: Option<GoalFile>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
struct GoalFile {
    accuracy: f64,
    speed: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    fluidity: Option<f64>,
}

impl From<Goal> for GoalFile {
    fn from(g: Goal) -> Self {
        Self {
            accuracy: g.accuracy,
            speed: g.speed,
            fluidity: g.fluidity,
        }
    }
}

impl GoalFile {
    fn into_goal(self, default: Goal) -> Goal {
        Goal {
            accuracy: self.accuracy,
            speed: self.speed,
            // La meta de fluidez sólo existe en el módulo Fluidez.
            fluidity: default.fluidity.map(|d| self.fluidity.unwrap_or(d)),
        }
    }
}

impl ConfigFile {
    fn from_config(c: &Config) -> Self {
        Self {
            language: c.language.clone(),
            text_language: c.text_language.clone(),
            keyboard: c.keyboard.clone(),
            player_name: c.player_name.clone(),
            last_module: c.last_module.map(|m| module_key(m).to_owned()),
            lesson_font: c.lesson_font.clone(),
            basic_lesson: Some(c.basic_lesson),
            fluid_paragraphs: Some(c.fluid_paragraphs),
            speech: Some(c.speech),
            beep: Some(c.beep),
            autostart: Some(c.autostart),
            alt_colors: Some(c.alt_colors),
            show_keyboard: Some(c.show_keyboard),
            goals: GoalsFile {
                basic: Some(c.goals.basic.into()),
                adaptability: Some(c.goals.adaptability.into()),
                velocity: Some(c.goals.velocity.into()),
                fluidity: Some(c.goals.fluidity.into()),
            },
        }
    }

    fn into_config(self) -> Config {
        let d = Config::default();
        let goal = |g: Option<GoalFile>, default: Goal| g.map_or(default, |g| g.into_goal(default));
        Config {
            language: self.language,
            text_language: self.text_language,
            keyboard: self.keyboard,
            player_name: self.player_name.filter(|n| !n.trim().is_empty()),
            last_module: self.last_module.as_deref().and_then(module_from_key),
            lesson_font: self.lesson_font,
            basic_lesson: self.basic_lesson.unwrap_or(d.basic_lesson),
            fluid_paragraphs: self.fluid_paragraphs.unwrap_or(d.fluid_paragraphs),
            speech: self.speech.unwrap_or(d.speech),
            beep: self.beep.unwrap_or(d.beep),
            autostart: self.autostart.unwrap_or(d.autostart),
            alt_colors: self.alt_colors.unwrap_or(d.alt_colors),
            show_keyboard: self.show_keyboard.unwrap_or(d.show_keyboard),
            goals: Goals {
                basic: goal(self.goals.basic, d.goals.basic),
                adaptability: goal(self.goals.adaptability, d.goals.adaptability),
                velocity: goal(self.goals.velocity, d.goals.velocity),
                fluidity: goal(self.goals.fluidity, d.goals.fluidity),
                ..d.goals
            },
        }
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
        let mut c = Config {
            basic_lesson: 999,
            ..Config::default()
        };
        c.goals.velocity.accuracy = 150.0;
        c.goals.basic.speed = f64::NAN;
        let c = c.sanitized();
        assert_eq!(c.basic_lesson, MAX_LESSONS);
        assert!((c.goals.velocity.accuracy - 100.0).abs() < 1e-9);
        assert!((c.goals.basic.speed - 10.0).abs() < 1e-9);
    }

    #[test]
    fn toml_round_trip() {
        let mut c = Config {
            language: Some("es".into()),
            keyboard: Some("qwerty_es".into()),
            basic_lesson: 12,
            last_module: Some(Module::Fluidity),
            alt_colors: true,
            ..Config::default()
        };
        c.goals.fluidity.fluidity = Some(80.0);
        let back = Config::from_toml(&c.to_toml()).expect("válido");
        assert_eq!(back, c);
    }

    #[test]
    fn partial_and_invalid_files() {
        let c =
            Config::from_toml("basic_lesson = 7\n[goals.velocity]\naccuracy = 90\nspeed = 40\n")
                .expect("válido");
        assert_eq!(c.basic_lesson, 7);
        assert!((c.goals.velocity.speed - 40.0).abs() < 1e-9);
        assert_eq!(c.goals.velocity.fluidity, None);
        assert_eq!(c.goals.fluidity, Goals::default().fluidity);
        assert!(Config::from_toml("").is_ok());
        assert!(Config::from_toml("basic_lesson = \"x\"").is_err());
        assert!(Config::from_toml("last_module = \"nada\"").is_ok());
    }

    #[test]
    fn saves_and_loads_files() {
        let dir = tempfile::tempdir().expect("tmp");
        let path = dir.path().join("sub").join("config.toml");
        assert_eq!(Config::load(&path).expect("sin archivo"), None);
        let c = Config {
            basic_lesson: 5,
            ..Config::default()
        };
        c.save(&path).expect("guardar");
        assert_eq!(Config::load(&path).expect("leer"), Some(c));
        fs::write(&path, "[[[").expect("escribir");
        assert!(matches!(
            Config::load(&path),
            Err(StoreError::Config { .. })
        ));
    }

    #[test]
    fn applies_legacy_preferences() {
        let prefs = LegacyPreferences {
            keyboard: Some("qwerty es".into()),
            basic_lesson: Some(400),
            speech: Some(false),
            ..LegacyPreferences::default()
        };
        let c = Config::default().with_legacy(&prefs);
        assert_eq!(c.keyboard.as_deref(), Some("qwerty_es"));
        assert_eq!(c.basic_lesson, MAX_LESSONS);
        assert!(!c.speech);
    }
}
