// SPDX-License-Identifier: GPL-3.0-or-later
//! Importación de las preferencias y el historial del Klavaro original.
//!
//! Se hace una sola vez: la base guarda la marca [`IMPORTED_KEY`] y las siguientes llamadas no
//! hacen nada, para no duplicar sesiones.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use std::collections::BTreeMap;

use klavaro_core::Module;
use klavaro_core::ranking::{Ranking, RankingEntry};
use klavaro_core::tracker::{ERROR_INERTIA, ErrorEntry, KeyTracker, TimeEntry};
use klavaro_data::legacy::{self, LegacyPreferences, StatRecord};

use crate::date::{local_to_utc, parse_local};
use crate::history::{History, ScoreRecord, SessionRecord};
use crate::{Result, io_error};

/// Marca aparte para el Top 10 local (`ksc/local_<xx>.ksc`), que se agregó después.
pub const RANKING_IMPORTED_KEY: &str = "legacy_ranking_imported";

/// Marca de que ya se pasaron a UTC las horas importadas como locales.
pub const UTC_TIMES_KEY: &str = "legacy_times_utc";

/// Pasa a UTC las horas importadas antes de esta corrección, que se guardaron como hora
/// local. Sólo toca lo importado: sesiones sin pulsaciones (el original no las guarda) y los
/// puntajes de Fluidez del mismo instante que una de ellas. Se hace una sola vez; las bases
/// nuevas quedan marcadas sin cambios, porque se importan ya en UTC.
///
/// # Errors
///
/// Si falla la base.
pub fn utc_times_once(history: &mut History, fresh: bool) -> Result<bool> {
    if history.meta(UTC_TIMES_KEY)?.is_some() {
        return Ok(false);
    }
    if !fresh {
        let tx = history.conn.transaction()?;
        let times: Vec<i64> = {
            let mut stmt = tx.prepare("SELECT DISTINCT at FROM sessions WHERE touches IS NULL")?;
            stmt.query_map([], |row| row.get(0))?
                .collect::<rusqlite::Result<_>>()?
        };
        for at in times {
            let utc = local_to_utc(at);
            if utc == at {
                continue;
            }
            tx.execute(
                "UPDATE scores SET at = ?1 WHERE at = ?2",
                rusqlite::params![utc, at],
            )?;
            tx.execute(
                "UPDATE sessions SET at = ?1 WHERE at = ?2 AND touches IS NULL",
                rusqlite::params![utc, at],
            )?;
        }
        tx.commit()?;
    }
    history.set_meta(UTC_TIMES_KEY, "1")?;
    Ok(true)
}

/// Marca de que ya se pasaron a códigos los idiomas importados con su nombre (importaciones
/// hechas antes de esta corrección).
pub const LANGUAGE_CODES_KEY: &str = "legacy_language_codes";

/// Clave de la tabla `meta` que marca que ya se importó.
pub const IMPORTED_KEY: &str = "legacy_imported";
/// Marca aparte para el rastreo por tecla, que se agregó después (esquema 2): así también se
/// importa en bases creadas antes.
pub const KEYS_IMPORTED_KEY: &str = "legacy_keys_imported";

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
                match parse_local(&s.date, &s.time) {
                    Some(at) => {
                        history.record_score(&ScoreRecord {
                            at,
                            score: s.score,
                            chars: s.chars,
                            language: language_code(&s.language),
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

/// Importa `accuracy.log_<teclado>` y `proficiency.log_<teclado>` si todavía no se hizo.
/// Devuelve los teclados importados, o `None` si ya se había hecho. Un teclado que ya tiene
/// rastreo propio no se toca.
///
/// # Errors
///
/// Si una carpeta o un archivo existe pero no se puede leer, o falla la base.
pub fn import_keys_once(history: &mut History, dirs: &[PathBuf]) -> Result<Option<Vec<String>>> {
    if history.meta(KEYS_IMPORTED_KEY)?.is_some() {
        return Ok(None);
    }
    let mut found: BTreeMap<String, (Vec<ErrorEntry>, Vec<TimeEntry>)> = BTreeMap::new();
    for dir in dirs {
        for (keyboard, kind, path) in key_logs(dir)? {
            let Some(text) = read_optional(&path)? else {
                continue;
            };
            let entry = found.entry(keyboard).or_default();
            match kind {
                KeyLog::Accuracy => {
                    entry.0 = legacy::parse_accuracy_log(&text, ERROR_INERTIA)
                        .into_iter()
                        .map(|r| ErrorEntry {
                            ch: r.ch,
                            wrong: r.wrong,
                            correct: r.correct,
                        })
                        .collect();
                }
                // El original carga el promedio como tres tiempos iguales.
                KeyLog::Proficiency => {
                    entry.1 = legacy::parse_proficiency_log(&text)
                        .into_iter()
                        .map(|(ch, dt)| TimeEntry::new(ch, vec![dt; 3]))
                        .collect();
                }
            }
        }
    }
    let mut imported = Vec::new();
    for (keyboard, (errors, times)) in found {
        if errors.is_empty() && times.is_empty() {
            continue;
        }
        if history.tracker(&keyboard)? != KeyTracker::new() {
            continue;
        }
        history.save_tracker(&keyboard, &KeyTracker::from_parts(errors, times))?;
        imported.push(keyboard);
    }
    history.set_meta(KEYS_IMPORTED_KEY, &imported.len().to_string())?;
    Ok(Some(imported))
}

enum KeyLog {
    Accuracy,
    Proficiency,
}

/// Archivos de rastreo por tecla de una carpeta: teclado, tipo y ruta.
fn key_logs(dir: &Path) -> Result<Vec<(String, KeyLog, PathBuf)>> {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(io_error(dir)(e)),
    };
    let mut out = Vec::new();
    for entry in entries {
        let path = entry.map_err(io_error(dir))?.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let found = if let Some(kb) = name.strip_prefix("accuracy.log_") {
            Some((kb.to_owned(), KeyLog::Accuracy))
        } else {
            name.strip_prefix("proficiency.log_")
                .map(|kb| (kb.to_owned(), KeyLog::Proficiency))
        };
        if let Some((keyboard, kind)) = found.filter(|(kb, _)| !kb.is_empty()) {
            out.push((keyboard, kind, path));
        }
    }
    Ok(out)
}

/// Convierte una fila de `stat_<módulo>.txt`. Las columnas 6 y 7 cambian de sentido según el
/// módulo (ver `tutor.c`).
pub fn session_from_stat(module: Module, r: &StatRecord) -> Option<SessionRecord> {
    let at = parse_local(&r.date, &r.time)?;
    let (lesson, keyboard, language) = match module {
        Module::Basic => (Some(r.lesson.clone()), Some(r.tag.clone()), None),
        Module::Adaptability => (None, Some(r.lesson.clone()), Some(language_code(&r.tag))),
        Module::Velocity | Module::Fluidity => {
            (Some(r.lesson.clone()), None, Some(language_code(&r.tag)))
        }
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

/// Importa el Top 10 local del original (`ksc/local_<xx>.ksc`) si todavía no se hizo, para
/// los idiomas que todavía no tienen ranking propio. Devuelve los idiomas importados, o
/// `None` si ya se había hecho.
///
/// # Errors
///
/// Si una carpeta o un archivo existe pero no se puede leer, o falla la base.
pub fn import_ranking_once(history: &mut History, dirs: &[PathBuf]) -> Result<Option<Vec<String>>> {
    if history.meta(RANKING_IMPORTED_KEY)?.is_some() {
        return Ok(None);
    }
    let mut imported = Vec::new();
    for dir in dirs {
        let ksc = dir.join("ksc");
        let entries = match fs::read_dir(&ksc) {
            Ok(e) => e,
            Err(e) if e.kind() == io::ErrorKind::NotFound => continue,
            Err(e) => return Err(io_error(&ksc)(e)),
        };
        let mut files: Vec<PathBuf> = entries
            .filter_map(std::result::Result::ok)
            .map(|e| e.path())
            .collect();
        files.sort();
        for path in files {
            let Some(two) = path
                .file_name()
                .and_then(|n| n.to_str())
                .and_then(|n| n.strip_prefix("local_"))
                .and_then(|n| n.strip_suffix(".ksc"))
            else {
                continue;
            };
            let Some(language) = language_from_two_letters(two) else {
                continue;
            };
            if !history.ranking(&language)?.entries().is_empty() {
                continue;
            }
            let bytes = fs::read(&path).map_err(io_error(&path))?;
            let entries: Vec<RankingEntry> = legacy::parse_ksc(&bytes)
                .into_iter()
                .map(|k| RankingEntry {
                    name: k.name,
                    language: language.clone(),
                    when: k.when,
                    chars: k.chars,
                    accuracy: k.accuracy,
                    wpm: k.wpm,
                    fluidity: k.fluidity,
                    score: k.score,
                })
                .collect();
            if entries.is_empty() {
                continue;
            }
            history.save_ranking(&language, &Ranking::new(entries))?;
            imported.push(language);
        }
    }
    history.set_meta(RANKING_IMPORTED_KEY, &imported.len().to_string())?;
    Ok(Some(imported))
}

/// Código del idioma a partir de las dos letras del nombre del `.ksc` (`en` es `C`).
fn language_from_two_letters(two: &str) -> Option<String> {
    if two == "en" {
        return Some(klavaro_i18n::FALLBACK.to_owned());
    }
    klavaro_i18n::LANGUAGES
        .iter()
        .map(|(c, _)| *c)
        .find(|c| c.get(..2) == Some(two))
        .map(str::to_owned)
}

/// El original guarda el idioma por su nombre nativo («Español»); el historial usa el código
/// (`es`), como el resto de la app. Un nombre desconocido se deja tal cual.
fn language_code(name: &str) -> String {
    klavaro_i18n::code_from_name(name).map_or_else(|| name.to_owned(), str::to_owned)
}

/// Pasa a códigos los idiomas guardados por nombre en importaciones anteriores. Se hace una
/// sola vez; devuelve si se hizo ahora.
///
/// # Errors
///
/// Si falla la base.
pub fn normalize_languages_once(history: &mut History) -> Result<bool> {
    if history.meta(LANGUAGE_CODES_KEY)?.is_some() {
        return Ok(false);
    }
    let tx = history.conn.transaction()?;
    for (code, name) in klavaro_i18n::LANGUAGES {
        tx.execute(
            "UPDATE sessions SET language = ?1 WHERE language = ?2",
            [code, name],
        )?;
        tx.execute(
            "UPDATE scores SET language = ?1 WHERE language = ?2",
            [code, name],
        )?;
        tx.execute(
            "UPDATE ranking SET language = ?1 WHERE language = ?2",
            [code, name],
        )?;
    }
    tx.commit()?;
    history.set_meta(LANGUAGE_CODES_KEY, "1")?;
    Ok(true)
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

    #[test]
    fn imports_key_logs_once() {
        let tmp = tempfile::tempdir().expect("tmp");
        write(
            tmp.path(),
            "accuracy.log_qwerty_es",
            "ñ\t 4\t 2\nq\t 1\t 9\n",
        );
        write(
            tmp.path(),
            "proficiency.log_qwerty_es",
            "k\t0.500000\t2.0000\n",
        );
        write(tmp.path(), "accuracy.log_dvorak", "");

        let mut h = History::in_memory().expect("base");
        let dirs = [tmp.path().to_path_buf()];
        let keyboards = import_keys_once(&mut h, &dirs)
            .expect("importar")
            .expect("primera vez");
        assert_eq!(keyboards, ["qwerty_es"]);
        let t = h.tracker("qwerty_es").expect("leer");
        assert_eq!(t.error_total(), 5);
        assert_eq!(t.times()[0].samples(), [0.5, 0.5, 0.5]);
        assert_eq!(import_keys_once(&mut h, &dirs).expect("segunda vez"), None);
    }

    #[test]
    fn languages_become_codes() {
        let tmp = tempfile::tempdir().expect("tmp");
        write(
            tmp.path(),
            "stat_fluid.txt",
            "Accuracy\tVelocity\tFluidness\tDate\tHour\tLesson\tLanguage\n\
             92.89\t52.71\t71.46\t2024-12-09\t07:47\tDefault\tEspañol\n",
        );
        write(
            tmp.path(),
            "scores_fluid.txt",
            "Score\tDate\tTime\t# chars\tLang\n6.137\t2024-12-09\t07:47\t1491\tEspañol\n",
        );
        let mut h = History::in_memory().expect("base");
        import(&mut h, &[tmp.path().to_path_buf()]).expect("importar");
        let fluid = h.recent(Module::Fluidity, 10).expect("leer");
        assert_eq!(fluid[0].language.as_deref(), Some("es"));
        assert_eq!(h.recent_scores(Some("es"), 10).expect("leer").len(), 1);
    }

    #[test]
    fn fixes_names_from_earlier_imports_once() {
        let mut h = History::in_memory().expect("base");
        h.record_score(&ScoreRecord {
            at: 1,
            score: 3.0,
            chars: 600,
            language: "Español".into(),
        })
        .expect("guardar");
        assert!(normalize_languages_once(&mut h).expect("corregir"));
        assert_eq!(h.recent_scores(Some("es"), 10).expect("leer").len(), 1);
        assert!(!normalize_languages_once(&mut h).expect("segunda vez"));
    }

    #[test]
    fn imports_the_local_top10_once() {
        let tmp = tempfile::tempdir().expect("tmp");
        fs::create_dir_all(tmp.path().join("ksc")).expect("carpeta");
        let mut bytes = b"esx".to_vec();
        bytes.extend(1_700_000_000_i32.to_le_bytes());
        bytes.extend(650_i32.to_le_bytes());
        for v in [96.0_f32, 50.0, 72.0, 6.25] {
            bytes.extend(v.to_le_bytes());
        }
        bytes.extend(3_i32.to_le_bytes());
        bytes.extend(b"Ana");
        fs::write(tmp.path().join("ksc/local_es.ksc"), &bytes).expect("escribir");
        fs::write(tmp.path().join("ksc/global_es.ksc"), &bytes).expect("escribir");

        let mut h = History::in_memory().expect("base");
        let dirs = [tmp.path().to_path_buf()];
        let langs = import_ranking_once(&mut h, &dirs)
            .expect("importar")
            .expect("primera vez");
        assert_eq!(langs, ["es"]);
        let top = h.ranking("es").expect("leer");
        assert_eq!(top.entries().len(), 1);
        assert_eq!(top.entries()[0].name, "Ana");
        assert_eq!(top.entries()[0].chars, 650);
        assert_eq!(import_ranking_once(&mut h, &dirs).expect("segunda"), None);
    }

    #[test]
    fn moves_old_local_imports_to_utc_once() {
        let mut h = History::in_memory().expect("base");
        let local = crate::date::parse_date_time("2024-12-09", "07:47").expect("fecha");
        let mut imported = SessionRecord::from_stats(
            Module::Fluidity,
            local,
            &klavaro_core::metrics::Stats::compute(600, 5, std::time::Duration::from_secs(60), &[]),
        );
        imported.touches = None;
        h.record(&imported).expect("guardar");
        h.record_score(&ScoreRecord {
            at: local,
            score: 6.0,
            chars: 1491,
            language: "es".into(),
        })
        .expect("guardar");
        assert!(utc_times_once(&mut h, false).expect("corregir"));
        let utc = local_to_utc(local);
        assert_eq!(h.recent(Module::Fluidity, 1).expect("leer")[0].at, utc);
        assert_eq!(h.recent_scores(None, 1).expect("leer")[0].at, utc);
        assert!(!utc_times_once(&mut h, false).expect("segunda"));
    }
}
