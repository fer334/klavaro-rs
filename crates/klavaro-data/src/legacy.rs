// SPDX-License-Identifier: GPL-3.0-or-later
//! Historial y preferencias del Klavaro original, para importarlos.
//!
//! - `stat_basic.txt`, `stat_adapt.txt`, `stat_velo.txt`, `stat_fluid.txt`: una fila por
//!   sesión (precisión, velocidad, fluidez, fecha, hora, lección, teclado o idioma).
//! - `scores_fluid.txt`: puntaje, fecha, hora, caracteres, idioma.
//! - `preferences.ini`: formato INI de `GKeyFile`.
//!
//! Los números usan punto decimal (el original escribe con el locale `C`).

use std::collections::BTreeMap;

use crate::{ParseError, ParseErrorKind, lines};

/// Una fila de `stat_<módulo>.txt`.
#[derive(Debug, Clone, PartialEq)]
pub struct StatRecord {
    /// Precisión (%).
    pub accuracy: f64,
    /// Velocidad (PPM).
    pub wpm: f64,
    /// Fluidez.
    pub fluidity: f64,
    /// Fecha `AAAA-MM-DD`.
    pub date: String,
    /// Hora `HH:MM`.
    pub time: String,
    /// Lección (Básico) o nombre de la lista de textos (Velocidad, Fluidez); en
    /// Adaptabilidad, el teclado.
    pub lesson: String,
    /// Teclado (Básico) o idioma (Adaptabilidad, Velocidad, Fluidez).
    pub tag: String,
}

/// Una fila de `scores_fluid.txt`.
#[derive(Debug, Clone, PartialEq)]
pub struct ScoreRecord {
    /// Puntaje.
    pub score: f64,
    /// Fecha `AAAA-MM-DD`.
    pub date: String,
    /// Hora `HH:MM`.
    pub time: String,
    /// Caracteres tecleados.
    pub chars: u32,
    /// Idioma.
    pub language: String,
}

/// Resultado de importar un archivo de historial: filas válidas y errores de las demás.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Imported<T> {
    /// Filas válidas.
    pub records: Vec<T>,
    /// Filas descartadas.
    pub errors: Vec<ParseError>,
}

/// Lee un `stat_<módulo>.txt`. La primera línea es el encabezado.
pub fn parse_stats(text: &str) -> Imported<StatRecord> {
    parse_rows(text, 7, |f| {
        Ok(StatRecord {
            accuracy: number(f[0])?,
            wpm: number(f[1])?,
            fluidity: number(f[2])?,
            date: f[3].to_owned(),
            time: f[4].to_owned(),
            lesson: f[5].to_owned(),
            tag: f[6..].join(" "),
        })
    })
}

/// Lee `scores_fluid.txt`. La primera línea es el encabezado.
pub fn parse_scores(text: &str) -> Imported<ScoreRecord> {
    parse_rows(text, 5, |f| {
        Ok(ScoreRecord {
            score: number(f[0])?,
            date: f[1].to_owned(),
            time: f[2].to_owned(),
            chars: f[3]
                .parse()
                .map_err(|_| ParseErrorKind::InvalidField(f[3].to_owned()))?,
            language: f[4].to_owned(),
        })
    })
}

fn parse_rows<T>(
    text: &str,
    min_fields: usize,
    build: impl Fn(&[&str]) -> Result<T, ParseErrorKind>,
) -> Imported<T> {
    let mut out = Imported {
        records: Vec::new(),
        errors: Vec::new(),
    };
    for (i, line) in lines(text).enumerate().skip(1) {
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.is_empty() {
            continue;
        }
        if fields.len() < min_fields {
            out.errors.push(ParseError::at(
                i + 1,
                ParseErrorKind::InvalidField(line.to_owned()),
            ));
            continue;
        }
        match build(&fields) {
            Ok(r) => out.records.push(r),
            Err(kind) => out.errors.push(ParseError::at(i + 1, kind)),
        }
    }
    out
}

fn number(s: &str) -> Result<f64, ParseErrorKind> {
    s.parse::<f64>()
        .ok()
        .filter(|v| v.is_finite())
        .ok_or_else(|| ParseErrorKind::InvalidField(s.to_owned()))
}

/// Un archivo INI: sección → clave → valor.
pub type Ini = BTreeMap<String, BTreeMap<String, String>>;

/// Lee un INI de `GKeyFile` (`[sección]`, `clave=valor`, comentarios con `#`).
pub fn parse_ini(text: &str) -> Ini {
    let mut ini = Ini::new();
    let mut section = String::new();
    for line in lines(text) {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }
        if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            name.trim().clone_into(&mut section);
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            ini.entry(section.clone())
                .or_default()
                .insert(k.trim().to_owned(), v.trim().to_owned());
        }
    }
    ini
}

/// Preferencias del original que tienen equivalente en klavaro-rs.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct LegacyPreferences {
    /// Idioma de la interfaz (`interface/language`).
    pub language: Option<String>,
    /// Distribución de teclado (`tutor/keyboard`).
    pub keyboard: Option<String>,
    /// Lección actual del curso básico (`tutor/basic_lesson`).
    pub basic_lesson: Option<u16>,
    /// Párrafos de Fluidez (`tutor/fluid_paragraphs`).
    pub fluid_paragraphs: Option<u8>,
    /// Fuente del ejercicio (`tutor/lesson_font`).
    pub lesson_font: Option<String>,
    /// Voz activada (`interface/speech`).
    pub speech: Option<bool>,
    /// Colores (`colors/*`), en `#RRGGBB`.
    pub colors: BTreeMap<String, String>,
}

impl LegacyPreferences {
    /// Extrae las preferencias conocidas de un `preferences.ini`.
    pub fn from_ini(ini: &Ini) -> Self {
        let get = |s: &str, k: &str| ini.get(s).and_then(|m| m.get(k)).cloned();
        Self {
            language: get("interface", "language"),
            keyboard: get("tutor", "keyboard"),
            basic_lesson: get("tutor", "basic_lesson").and_then(|v| v.parse().ok()),
            fluid_paragraphs: get("tutor", "fluid_paragraphs").and_then(|v| v.parse().ok()),
            lesson_font: get("tutor", "lesson_font"),
            speech: get("interface", "speech").map(|v| v.eq_ignore_ascii_case("true")),
            colors: ini
                .get("colors")
                .map(|m| {
                    m.iter()
                        .filter(|(_, v)| is_hex_color(v))
                        .map(|(k, v)| (k.clone(), v.clone()))
                        .collect()
                })
                .unwrap_or_default(),
        }
    }
}

fn is_hex_color(v: &str) -> bool {
    v.len() == 7 && v.starts_with('#') && v[1..].chars().all(|c| c.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_stat_rows_and_reports_bad_ones() {
        let text = "Accuracy Velocity Fluidness Date Hour Lesson Keyboard\n\
                    97.50 23.40 61.20 2021-03-04 10:15 5 qwerty_us\n\
                    oops\n\
                    98.00 abc 60.00 2021-03-04 10:20 5 qwerty_us\n\
                    99.00 30.00 70.00 2021-03-05 08:00 Default es\n";
        let imported = parse_stats(text);
        assert_eq!(imported.records.len(), 2);
        assert_eq!(imported.errors.len(), 2);
        assert_eq!(imported.records[0].lesson, "5");
        assert_eq!(imported.records[0].tag, "qwerty_us");
        assert!((imported.records[1].wpm - 30.0).abs() < 1e-9);
        assert_eq!(imported.errors[0].line, Some(3));
    }

    #[test]
    fn parses_scores() {
        let text = "Score Date Time #chars Lang\n6.4321 2022-01-01 12:00 812 es\n";
        let imported = parse_scores(text);
        assert_eq!(
            imported.records,
            vec![ScoreRecord {
                score: 6.4321,
                date: "2022-01-01".to_owned(),
                time: "12:00".to_owned(),
                chars: 812,
                language: "es".to_owned(),
            }]
        );
    }

    #[test]
    fn parses_preferences() {
        let text = "[interface]\nlanguage=es\nspeech=false\n\n[tutor]\nkeyboard=qwerty_es\n\
                    basic_lesson=12\nfluid_paragraphs=3\nlesson_font=Monospace 14\n\
                    # comentario\n[colors]\nkey_1=#112233\nbad=rojo\n";
        let prefs = LegacyPreferences::from_ini(&parse_ini(text));
        assert_eq!(prefs.language.as_deref(), Some("es"));
        assert_eq!(prefs.keyboard.as_deref(), Some("qwerty_es"));
        assert_eq!(prefs.basic_lesson, Some(12));
        assert_eq!(prefs.fluid_paragraphs, Some(3));
        assert_eq!(prefs.speech, Some(false));
        assert_eq!(prefs.lesson_font.as_deref(), Some("Monospace 14"));
        assert_eq!(prefs.colors.len(), 1);
    }
}
