// SPDX-License-Identifier: GPL-3.0-or-later
//! Historial de sesiones, Top 10 local y puntajes de Fluidez, en una base `SQLite`.
//!
//! Equivale a los `stat_*.txt`, `scores_fluid.txt` y el Top 10 local del original, en un
//! solo archivo con transacciones.

use std::path::Path;
use std::time::Duration;

use klavaro_core::Module;
use klavaro_core::metrics::Stats;
use klavaro_core::ranking::{Ranking, RankingEntry};
use rusqlite::{Connection, OptionalExtension, params};

use crate::config::module_key;
use crate::{Result, io_error};

/// Versión del esquema (`PRAGMA user_version`).
const SCHEMA_VERSION: i64 = 1;

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS sessions (
    id         INTEGER PRIMARY KEY,
    module     TEXT    NOT NULL,
    at         INTEGER NOT NULL,
    accuracy   REAL    NOT NULL,
    wpm        REAL    NOT NULL,
    fluidity   REAL    NOT NULL,
    touches    INTEGER,
    errors     INTEGER,
    elapsed_ms INTEGER,
    lesson     TEXT,
    keyboard   TEXT,
    language   TEXT
);
CREATE INDEX IF NOT EXISTS sessions_by_module ON sessions (module, at);
CREATE TABLE IF NOT EXISTS ranking (
    language TEXT    NOT NULL,
    name     TEXT    NOT NULL,
    at       INTEGER NOT NULL,
    chars    INTEGER NOT NULL,
    accuracy REAL    NOT NULL,
    wpm      REAL    NOT NULL,
    fluidity REAL    NOT NULL,
    score    REAL    NOT NULL
);
CREATE TABLE IF NOT EXISTS scores (
    at       INTEGER NOT NULL,
    score    REAL    NOT NULL,
    chars    INTEGER NOT NULL,
    language TEXT    NOT NULL
);
CREATE TABLE IF NOT EXISTS meta (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
";

/// Una sesión registrada.
#[derive(Debug, Clone, PartialEq)]
pub struct SessionRecord {
    /// Módulo.
    pub module: Module,
    /// Instante (segundos desde 1970).
    pub at: i64,
    /// Precisión (%).
    pub accuracy: f64,
    /// Velocidad (PPM).
    pub wpm: f64,
    /// Fluidez.
    pub fluidity: f64,
    /// Pulsaciones (`None` en sesiones importadas del original).
    pub touches: Option<u32>,
    /// Errores (`None` en sesiones importadas).
    pub errors: Option<u32>,
    /// Duración (`None` en sesiones importadas).
    pub elapsed: Option<Duration>,
    /// Lección (Básico) o lista de textos (Velocidad, Fluidez).
    pub lesson: Option<String>,
    /// Distribución de teclado.
    pub keyboard: Option<String>,
    /// Idioma de los textos.
    pub language: Option<String>,
}

impl SessionRecord {
    /// Registro de un ejercicio recién terminado.
    pub fn from_stats(module: Module, at: i64, stats: &Stats) -> Self {
        Self {
            module,
            at,
            accuracy: stats.accuracy,
            wpm: stats.wpm,
            fluidity: stats.fluidity,
            touches: Some(stats.touches),
            errors: Some(stats.errors),
            elapsed: Some(stats.elapsed),
            lesson: None,
            keyboard: None,
            language: None,
        }
    }
}

/// Un puntaje de Fluidez (`scores_fluid.txt`), haya entrado o no al Top 10.
#[derive(Debug, Clone, PartialEq)]
pub struct ScoreRecord {
    /// Instante (segundos desde 1970).
    pub at: i64,
    /// Puntaje.
    pub score: f64,
    /// Caracteres tecleados.
    pub chars: u32,
    /// Idioma.
    pub language: String,
}

/// Base de datos del historial.
#[derive(Debug)]
pub struct History {
    conn: Connection,
}

impl History {
    /// Abre (o crea) la base en `path`.
    ///
    /// # Errors
    ///
    /// Si no se puede crear la carpeta o abrir la base.
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(io_error(dir))?;
        }
        Self::init(Connection::open(path)?)
    }

    /// Base en memoria (pruebas, o cuando no se puede escribir en disco).
    ///
    /// # Errors
    ///
    /// Si `SQLite` no puede crearla.
    pub fn in_memory() -> Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Self> {
        conn.execute_batch(SCHEMA)?;
        conn.pragma_update(None, "user_version", SCHEMA_VERSION)?;
        Ok(Self { conn })
    }

    /// Agrega una sesión.
    ///
    /// # Errors
    ///
    /// Si falla la escritura.
    pub fn record(&self, r: &SessionRecord) -> Result<()> {
        insert_session(&self.conn, r)
    }

    /// Agrega varias sesiones en una transacción.
    ///
    /// # Errors
    ///
    /// Si falla la escritura; en ese caso no se agrega ninguna.
    pub fn record_all(&mut self, records: &[SessionRecord]) -> Result<()> {
        let tx = self.conn.transaction()?;
        for r in records {
            insert_session(&tx, r)?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Las últimas `limit` sesiones de un módulo, de la más vieja a la más nueva.
    ///
    /// # Errors
    ///
    /// Si falla la lectura.
    pub fn recent(&self, module: Module, limit: usize) -> Result<Vec<SessionRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT at, accuracy, wpm, fluidity, touches, errors, elapsed_ms, lesson, keyboard,
                    language
             FROM sessions WHERE module = ?1 ORDER BY at DESC, id DESC LIMIT ?2",
        )?;
        let limit = i64::try_from(limit).unwrap_or(i64::MAX);
        let mut out = stmt
            .query_map(params![module_key(module), limit], |row| {
                Ok(SessionRecord {
                    module,
                    at: row.get(0)?,
                    accuracy: row.get(1)?,
                    wpm: row.get(2)?,
                    fluidity: row.get(3)?,
                    touches: row.get(4)?,
                    errors: row.get(5)?,
                    elapsed: row
                        .get::<_, Option<i64>>(6)?
                        .and_then(|ms| u64::try_from(ms).ok())
                        .map(Duration::from_millis),
                    lesson: row.get(7)?,
                    keyboard: row.get(8)?,
                    language: row.get(9)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        out.reverse();
        Ok(out)
    }

    /// Cantidad de sesiones por módulo.
    ///
    /// # Errors
    ///
    /// Si falla la lectura.
    pub fn count(&self, module: Module) -> Result<u64> {
        let n: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM sessions WHERE module = ?1",
            [module_key(module)],
            |row| row.get(0),
        )?;
        Ok(u64::try_from(n).unwrap_or(0))
    }

    /// Top 10 local de un idioma.
    ///
    /// # Errors
    ///
    /// Si falla la lectura.
    pub fn ranking(&self, language: &str) -> Result<Ranking> {
        let mut stmt = self.conn.prepare(
            "SELECT name, at, chars, accuracy, wpm, fluidity, score
             FROM ranking WHERE language = ?1",
        )?;
        let entries = stmt
            .query_map([language], |row| {
                Ok(RankingEntry {
                    name: row.get(0)?,
                    language: language.to_owned(),
                    when: row.get(1)?,
                    chars: row.get(2)?,
                    accuracy: row.get(3)?,
                    wpm: row.get(4)?,
                    fluidity: row.get(5)?,
                    score: row.get(6)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(Ranking::new(entries))
    }

    /// Reemplaza el Top 10 de un idioma.
    ///
    /// # Errors
    ///
    /// Si falla la escritura; en ese caso queda el ranking anterior.
    pub fn save_ranking(&mut self, language: &str, ranking: &Ranking) -> Result<()> {
        let tx = self.conn.transaction()?;
        tx.execute("DELETE FROM ranking WHERE language = ?1", [language])?;
        for e in ranking.entries() {
            tx.execute(
                "INSERT INTO ranking (language, name, at, chars, accuracy, wpm, fluidity, score)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    language, e.name, e.when, e.chars, e.accuracy, e.wpm, e.fluidity, e.score
                ],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Agrega un puntaje de Fluidez.
    ///
    /// # Errors
    ///
    /// Si falla la escritura.
    pub fn record_score(&self, s: &ScoreRecord) -> Result<()> {
        self.conn.execute(
            "INSERT INTO scores (at, score, chars, language) VALUES (?1, ?2, ?3, ?4)",
            params![s.at, s.score, s.chars, s.language],
        )?;
        Ok(())
    }

    /// Los últimos `limit` puntajes de Fluidez, del más viejo al más nuevo.
    ///
    /// # Errors
    ///
    /// Si falla la lectura.
    pub fn recent_scores(&self, limit: usize) -> Result<Vec<ScoreRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT at, score, chars, language FROM scores ORDER BY at DESC, rowid DESC LIMIT ?1",
        )?;
        let limit = i64::try_from(limit).unwrap_or(i64::MAX);
        let mut out = stmt
            .query_map([limit], |row| {
                Ok(ScoreRecord {
                    at: row.get(0)?,
                    score: row.get(1)?,
                    chars: row.get(2)?,
                    language: row.get(3)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        out.reverse();
        Ok(out)
    }

    /// Valor guardado en la tabla de metadatos.
    ///
    /// # Errors
    ///
    /// Si falla la lectura.
    pub fn meta(&self, key: &str) -> Result<Option<String>> {
        Ok(self
            .conn
            .query_row("SELECT value FROM meta WHERE key = ?1", [key], |row| {
                row.get(0)
            })
            .optional()?)
    }

    /// Guarda un valor en la tabla de metadatos.
    ///
    /// # Errors
    ///
    /// Si falla la escritura.
    pub fn set_meta(&self, key: &str, value: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO meta (key, value) VALUES (?1, ?2)
             ON CONFLICT (key) DO UPDATE SET value = excluded.value",
            [key, value],
        )?;
        Ok(())
    }
}

fn insert_session(conn: &Connection, r: &SessionRecord) -> Result<()> {
    conn.execute(
        "INSERT INTO sessions (module, at, accuracy, wpm, fluidity, touches, errors,
                               elapsed_ms, lesson, keyboard, language)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![
            module_key(r.module),
            r.at,
            r.accuracy,
            r.wpm,
            r.fluidity,
            r.touches,
            r.errors,
            r.elapsed
                .map(|d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX)),
            r.lesson,
            r.keyboard,
            r.language,
        ],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session(module: Module, at: i64, wpm: f64) -> SessionRecord {
        SessionRecord {
            lesson: Some("3".into()),
            keyboard: Some("qwerty_es".into()),
            ..SessionRecord::from_stats(
                module,
                at,
                &Stats::compute(100, 2, Duration::from_secs(60), &[]),
            )
        }
        .with_wpm(wpm)
    }

    impl SessionRecord {
        fn with_wpm(mut self, wpm: f64) -> Self {
            self.wpm = wpm;
            self
        }
    }

    fn entry(score: f64) -> RankingEntry {
        RankingEntry {
            name: "Ana [qwerty_es]".into(),
            language: "es".into(),
            when: 10,
            chars: 600,
            accuracy: 98.0,
            wpm: 50.0,
            fluidity: 70.0,
            score,
        }
    }

    #[test]
    fn records_and_reads_recent_sessions() {
        let h = History::in_memory().expect("base");
        for i in 0..5 {
            h.record(&session(
                Module::Basic,
                i,
                f64::from(u8::try_from(i).unwrap_or(0)),
            ))
            .expect("guardar");
        }
        h.record(&session(Module::Velocity, 3, 40.0))
            .expect("guardar");
        let recent = h.recent(Module::Basic, 3).expect("leer");
        assert_eq!(recent.iter().map(|r| r.at).collect::<Vec<_>>(), [2, 3, 4]);
        assert_eq!(recent[0].elapsed, Some(Duration::from_secs(60)));
        assert_eq!(recent[0].touches, Some(100));
        assert_eq!(recent[0].keyboard.as_deref(), Some("qwerty_es"));
        assert_eq!(h.count(Module::Basic).expect("contar"), 5);
        assert_eq!(h.count(Module::Fluidity).expect("contar"), 0);
    }

    #[test]
    fn record_all_is_atomic_and_keeps_missing_fields() {
        let mut h = History::in_memory().expect("base");
        let mut r = session(Module::Adaptability, 1, 20.0);
        r.touches = None;
        r.elapsed = None;
        h.record_all(&[r.clone(), r]).expect("guardar");
        let recent = h.recent(Module::Adaptability, 10).expect("leer");
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].touches, None);
        assert_eq!(recent[0].elapsed, None);
    }

    #[test]
    fn ranking_round_trip_per_language() {
        let mut h = History::in_memory().expect("base");
        let mut r = Ranking::default();
        r.insert(entry(5.0), true).expect("entra");
        r.insert(entry(7.0), true).expect("entra");
        h.save_ranking("es", &r).expect("guardar");
        assert_eq!(h.ranking("es").expect("leer"), r);
        assert_eq!(h.ranking("C").expect("leer").entries(), []);
        h.save_ranking("es", &Ranking::default()).expect("vaciar");
        assert_eq!(h.ranking("es").expect("leer").entries(), []);
    }

    #[test]
    fn scores_and_meta() {
        let h = History::in_memory().expect("base");
        for at in [3, 1, 2] {
            h.record_score(&ScoreRecord {
                at,
                score: 1.5,
                chars: 600,
                language: "es".into(),
            })
            .expect("guardar");
        }
        let scores = h.recent_scores(2).expect("leer");
        assert_eq!(scores.iter().map(|s| s.at).collect::<Vec<_>>(), [2, 3]);
        assert_eq!(h.meta("x").expect("leer"), None);
        h.set_meta("x", "1").expect("guardar");
        h.set_meta("x", "2").expect("guardar");
        assert_eq!(h.meta("x").expect("leer").as_deref(), Some("2"));
    }

    #[test]
    fn opens_files_on_disk() {
        let dir = tempfile::tempdir().expect("tmp");
        let path = dir.path().join("a").join("h.sqlite3");
        History::open(&path)
            .expect("crear")
            .record(&session(Module::Fluidity, 1, 30.0))
            .expect("guardar");
        let h = History::open(&path).expect("reabrir");
        assert_eq!(h.count(Module::Fluidity).expect("contar"), 1);
    }
}
