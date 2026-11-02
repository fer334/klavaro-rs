// SPDX-License-Identifier: GPL-3.0-or-later
//! Importación de las preferencias y el historial del Klavaro original.
//!
//! Se hace una sola vez: la base guarda la marca [`IMPORTED_KEY`] y las siguientes llamadas no
//! hacen nada, para no duplicar sesiones.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use klavaro_core::Module;
use klavaro_data::legacy::{self, LegacyPreferences, StatRecord};

use crate::date::parse_date_time;
use crate::history::{History, ScoreRecord, SessionRecord};
use crate::{Result, io_error};

/// Clave de la tabla `meta` que marca que ya se importó.
pub const IMPORTED_KEY: &str = "legacy_imported";

/// Qué se importó.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ImportReport {
    /// Carpetas leídas.
    pub dirs: Vec<PathBuf>,
    /// Sesiones agregadas al historial.
    pub sessions: usize,
    /// Puntajes de Fluidez agregados.
    pub scores: usize,
    /// Filas descartadas por estar dañadas.
    pub skipped: usize,
    /// Preferencias encontradas, para aplicarlas a la configuración.
    pub preferences: Option<LegacyPreferences>,
}

impl ImportReport {
    /// Si no se encontró nada.
    pub fn is_empty(&self) -> bool {
        self.sessions == 0 && self.scores == 0 && self.preferences.is_none()
    }
}

/// Importa desde las carpetas dadas (ver [`crate::paths::legacy_dirs`]) si todavía no se hizo.
/// Devuelve `None` si ya se había importado.
///
/// # Errors
///
/// Si un archivo existe pero no se puede leer, o falla la base. Las filas dañadas no son error:
/// se cuentan en [`ImportReport::skipped`].
pub fn import_once(history: &mut History, dirs: &[PathBuf]) -> Result<Option<ImportReport>> {
    if history.meta(IMPORTED_KEY)?.is_some() {
        return Ok(None);
    }
    let report = import(history, dirs)?;
    history.set_meta(IMPORTED_KEY, &report.sessions.to_string())?;
    Ok(Some(report))
}

/// Importa sin verificar la marca.
///
/// # Errors
///
/// Como [`import_once`].
pub fn import(history: &mut History, dirs: &[PathBuf]) -> Result<ImportReport> {
    let mut report = ImportReport::default();
    let mut sessions = Vec::new();
    for dir in dirs {
        let mut found = false;
        if let Some(text) = read_optional(&dir.join("preferences.ini"))? {
            found = true;
            report.preferences = Some(LegacyPreferences::from_ini(&legacy::parse_ini(&text)));
        }
        for module in Module::ALL {
            let file = dir.join(format!("stat_{}.txt", module.legacy_name()));
            if let Some(text) = read_optional(&file)? {
                found = true;
                let imported = legacy::parse_stats(&text);
                report.skipped += imported.errors.len();
                for r in &imported.records {
                    match session_from_stat(module, r) {
                        Some(s) => sessions.push(s),
                        None => report.skipped += 1,
                    }
                }
            }
        }
        if let Some(text) = read_optional(&dir.join("scores_fluid.txt"))? {
            found = true;
            let imported = legacy::parse_scores(&text);
            report.skipped += imported.errors.len();
            for s in imported.records {
                match parse_date_time(&s.date, &s.time) {
                    Some(at) => {
                        history.record_score(&ScoreRecord {
                            at,
                            score: s.score,
                            chars: s.chars,
                            language: s.language,
                        })?;
                        report.scores += 1;
                    }
                    None => report.skipped += 1,
                }
            }
        }
        if found {
            report.dirs.push(dir.clone());
        }
    }
    history.record_all(&sessions)?;
    report.sessions = sessions.len();
    Ok(report)
}

/// Convierte una fila de `stat_<módulo>.txt`. Las columnas 6 y 7 cambian de sentido según el
/// módulo (ver `tutor.c`).
pub fn session_from_stat(module: Module, r: &StatRecord) -> Option<SessionRecord> {
    let at = parse_date_time(&r.date, &r.time)?;
    let (lesson, keyboard, language) = match module {
        Module::Basic => (Some(r.lesson.clone()), Some(r.tag.clone()), None),
        Module::Adaptability => (None, Some(r.lesson.clone()), Some(r.tag.clone())),
        Module::Velocity | Module::Fluidity => (Some(r.lesson.clone()), None, Some(r.tag.clone())),
    };
    Some(SessionRecord {
        module,
        at,
        accuracy: r.accuracy,
        wpm: r.wpm,
        fluidity: r.fluidity,
        touches: None,
        errors: None,
        elapsed: None,
        lesson,
        keyboard,
        language,
    })
}

fn read_optional(path: &Path) -> Result<Option<String>> {
    match fs::read(path) {
        // El original escribe UTF-8; si hay bytes inválidos se reemplazan en lugar de fallar.
        Ok(bytes) => Ok(Some(String::from_utf8_lossy(&bytes).into_owned())),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(io_error(path)(e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(dir: &Path, name: &str, text: &str) {
        fs::write(dir.join(name), text).expect("escribir");
    }

    #[test]
    fn imports_stats_scores_and_preferences_once() {
        let tmp = tempfile::tempdir().expect("tmp");
        let config = tmp.path().join("config");
        let data = tmp.path().join("data");
        fs::create_dir_all(&config).expect("carpeta");
        fs::create_dir_all(&data).expect("carpeta");
        write(
            &config,
            "preferences.ini",
            "[tutor]\nkeyboard=qwerty_es\nbasic_lesson=7\n",
        );
        write(
            &data,
            "stat_basic.txt",
            "Accuracy\tVelocity\tFluidness\tDate\tHour\tLesson\tKeyboard\n\
             97.50\t23.40\t61.20\t2021-03-04\t10:15\t05\tqwerty_es\n\
             98.00\t24.00\t60.00\t2021-13-04\t10:20\t05\tqwerty_es\n",
        );
        write(
            &data,
            "stat_adapt.txt",
            "Accuracy\tVelocity\tFluidness\tDate\tHour\tKeyboard\tLanguage\n\
             90.00\t20.00\t50.00\t2021-03-05\t09:00\tqwerty_es\tes\n",
        );
        write(
            &data,
            "scores_fluid.txt",
            "Score\tDate\tTime\t# chars\tLang\n3.250\t2021-03-06\t08:00\t650\tes\n",
        );

        let mut h = History::in_memory().expect("base");
        let dirs = [config.clone(), data.clone(), tmp.path().join("no_existe")];
        let report = import_once(&mut h, &dirs)
            .expect("importar")
            .expect("primera vez");
        assert_eq!(report.dirs, [config, data]);
        assert_eq!(report.sessions, 2);
        assert_eq!(report.scores, 1);
        assert_eq!(report.skipped, 1);
        assert_eq!(report.preferences.and_then(|p| p.basic_lesson), Some(7));

        let basic = h.recent(Module::Basic, 10).expect("leer");
        assert_eq!(basic[0].lesson.as_deref(), Some("05"));
        assert_eq!(basic[0].keyboard.as_deref(), Some("qwerty_es"));
        let adapt = h.recent(Module::Adaptability, 10).expect("leer");
        assert_eq!(adapt[0].keyboard.as_deref(), Some("qwerty_es"));
        assert_eq!(adapt[0].language.as_deref(), Some("es"));

        assert_eq!(import_once(&mut h, &dirs).expect("segunda vez"), None);
        assert_eq!(h.count(Module::Basic).expect("contar"), 1);
    }

    #[test]
    fn nothing_to_import() {
        let tmp = tempfile::tempdir().expect("tmp");
        let mut h = History::in_memory().expect("base");
        let report = import(&mut h, &[tmp.path().to_path_buf()]).expect("importar");
        assert!(report.is_empty());
        assert_eq!(report.dirs, [] as [std::path::PathBuf; 0]);
    }
}
