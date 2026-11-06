// SPDX-License-Identifier: GPL-3.0-or-later
//! Historial de sesiones, Top 10 local y puntajes de Fluidez, en una base `SQLite`.
//!
//! Equivale a los `stat_*.txt`, `scores_fluid.txt` y el Top 10 local del original, en un
//! solo archivo con transacciones.

use std::path::Path;
use std::time::Duration;

use klavaro_core::Module;
use klavaro_core::metrics::Stats;
use klavaro_core::progression::Slot;
use klavaro_core::ranking::{Ranking, RankingEntry};
use rusqlite::{Connection, OptionalExtension, params};

use crate::config::module_key;
use crate::{Result, io_error};

/// Versión del esquema (`PRAGMA user_version`).
const SCHEMA_VERSION: i64 = 3;

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
CREATE TABLE IF NOT EXISTS progression_attempts (
    id         INTEGER PRIMARY KEY,
    at         INTEGER NOT NULL,
    language   TEXT    NOT NULL,
    section    INTEGER NOT NULL,
    lesson     INTEGER NOT NULL,
    target_wpm REAL    NOT NULL,
    text_id    TEXT    NOT NULL,
    keyboard   TEXT    NOT NULL,
    wpm        REAL    NOT NULL,
    accuracy   REAL    NOT NULL,
    fluidity   REAL    NOT NULL,
    score      REAL    NOT NULL,
    stars      INTEGER NOT NULL,
    elapsed_ms INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS progression_by_lesson
    ON progression_attempts (language, section, lesson);
CREATE TABLE IF NOT EXISTS meta (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
-- Versión 2: rastreo por tecla (accuracy.log_* y proficiency.log_*), en el orden guardado.
CREATE TABLE IF NOT EXISTS key_errors (
    keyboard TEXT    NOT NULL,
    pos      INTEGER NOT NULL,
    ch       TEXT    NOT NULL,
    wrong    INTEGER NOT NULL,
    correct  INTEGER NOT NULL,
    PRIMARY KEY (keyboard, pos)
);
CREATE TABLE IF NOT EXISTS key_times (
    keyboard TEXT    NOT NULL,
    pos      INTEGER NOT NULL,
    ch       TEXT    NOT NULL,
    samples  TEXT    NOT NULL,
    PRIMARY KEY (keyboard, pos)
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

/// Un intento de una lección de la Progresión.
#[derive(Debug, Clone, PartialEq)]
pub struct LessonAttempt {
    /// Instante (segundos desde 1970).
    pub at: i64,
    /// Idioma de los textos.
    pub language: String,
    /// La lección.
    pub slot: Slot,
    /// Meta de la lección en este intento, en PPM.
    pub target_wpm: f64,
    /// Texto escrito. Es sólo un registro: el progreso se asocia a la lección, así un texto
    /// reemplazado en una versión nueva no hace perder estrellas.
    pub text_id: String,
    /// Distribución de teclado.
    pub keyboard: String,
    /// Velocidad (PPM).
    pub wpm: f64,
    /// Precisión (%).
    pub accuracy: f64,
    /// Fluidez.
    pub fluidity: f64,
    /// Puntaje.
    pub score: f64,
    /// Estrellas.
    pub stars: u8,
    /// Duración.
    pub elapsed: Duration,
}

/// Mejores valores de una lección entre todos sus intentos.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LessonBest {
    /// La lección.
    pub slot: Slot,
    /// Más estrellas.
    pub stars: u8,
    /// Mayor velocidad (PPM).
    pub wpm: f64,
    /// Mayor puntaje.
    pub score: f64,
}

/// Qué sesiones leer con [`History::sessions`]. Los campos en `None` no filtran.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionFilter {
    /// Módulo.
    pub module: Module,
    /// Número de lección (Básico) o sección (Progresión).
    pub lesson: Option<u16>,
    /// Lección exacta, por su nombre en el historial (Progresión: `75-03`).
    pub lesson_name: Option<String>,
    /// Distribución de teclado.
    pub keyboard: Option<String>,
    /// Idioma de los textos.
    pub language: Option<String>,
}

impl SessionFilter {
    /// Todas las sesiones de un módulo.
    pub fn module(module: Module) -> Self {
        Self {
            module,
            lesson: None,
            lesson_name: None,
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
    pub(crate) conn: Connection,
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

    /// Guarda un intento de una lección de la Progresión y su sesión en el historial, en una
    /// transacción.
    ///
    /// # Errors
    ///
    /// Si falla la escritura; en ese caso no se guarda ninguno de los dos.
    pub fn record_lesson(
        &mut self,
        attempt: &LessonAttempt,
        session: &SessionRecord,
    ) -> Result<()> {
        let tx = self.conn.transaction()?;
        tx.execute(
            "INSERT INTO progression_attempts (at, language, section, lesson, target_wpm, text_id,
                                               keyboard, wpm, accuracy, fluidity, score, stars,
                                               elapsed_ms)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                attempt.at,
                attempt.language,
                attempt.slot.section(),
                attempt.slot.lesson(),
                attempt.target_wpm,
                attempt.text_id,
                attempt.keyboard,
                attempt.wpm,
                attempt.accuracy,
                attempt.fluidity,
                attempt.score,
                attempt.stars,
                i64::try_from(attempt.elapsed.as_millis()).unwrap_or(i64::MAX),
            ],
        )?;
        insert_session(&tx, session)?;
        tx.commit()?;
        Ok(())
    }

    /// Borra los intentos de la Progresión en un idioma. No toca las sesiones ni el Top 10.
    ///
    /// # Errors
    ///
    /// Si falla la escritura.
    pub fn reset_lessons(&self, language: &str) -> Result<usize> {
        Ok(self.conn.execute(
            "DELETE FROM progression_attempts WHERE language = ?1",
            [language],
        )?)
    }

    /// Mejores valores de cada lección jugada en un idioma, en orden. Los valores salen de todos
    /// los intentos, así que un resultado peor nunca los baja. Se ignoran las lecciones que no
    /// existen en esta versión.
    ///
    /// # Errors
    ///
    /// Si falla la lectura.
    pub fn lesson_bests(&self, language: &str) -> Result<Vec<LessonBest>> {
        let mut stmt = self.conn.prepare(
            "SELECT section, lesson, MAX(stars), MAX(wpm), MAX(score)
             FROM progression_attempts
             WHERE language = ?1
             GROUP BY section, lesson
             ORDER BY section, lesson",
        )?;
        let rows = stmt.query_map([language], |row| {
            Ok((
                row.get::<_, u16>(0)?,
                row.get::<_, u16>(1)?,
                row.get::<_, u8>(2)?,
                row.get::<_, f64>(3)?,
                row.get::<_, f64>(4)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (section, lesson, stars, wpm, score) = row?;
            if let Some(slot) = Slot::new(section, lesson) {
                out.push(LessonBest {
                    slot,
                    stars,
                    wpm,
                    score,
                });
            }
        }
        Ok(out)
    }

    /// Las últimas `limit` sesiones de un módulo, de la más vieja a la más nueva.
    ///
    /// # Errors
    ///
    /// Si falla la lectura.
    pub fn recent(&self, module: Module, limit: usize) -> Result<Vec<SessionRecord>> {
        self.sessions(&SessionFilter::module(module), limit)
    }

    /// Las últimas `limit` sesiones que cumplen el filtro, de la más vieja a la más nueva.
    ///
    /// # Errors
    ///
    /// Si falla la lectura.
    pub fn sessions(&self, filter: &SessionFilter, limit: usize) -> Result<Vec<SessionRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT at, accuracy, wpm, fluidity, touches, errors, elapsed_ms, lesson, keyboard,
                    language
             FROM sessions
             WHERE module = ?1
               AND (?2 IS NULL OR CAST(lesson AS INTEGER) = ?2)
               AND (?3 IS NULL OR keyboard = ?3)
               AND (?4 IS NULL OR language = ?4)
               AND (?6 IS NULL OR lesson = ?6)
             ORDER BY at DESC, id DESC LIMIT ?5",
        )?;
        let limit = i64::try_from(limit).unwrap_or(i64::MAX);
        let module = filter.module;
        let mut out = stmt
            .query_map(
                params![
                    module_key(module),
                    filter.lesson,
                    filter.keyboard,
                    filter.language,
                    limit,
                    filter.lesson_name
                ],
                |row| {
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
                },
            )?
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

    /// Los últimos `limit` puntajes de Fluidez (de un idioma, si se indica), del más viejo
    /// al más nuevo.
    ///
    /// # Errors
    ///
    /// Si falla la lectura.
    pub fn recent_scores(&self, language: Option<&str>, limit: usize) -> Result<Vec<ScoreRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT at, score, chars, language FROM scores
             WHERE ?1 IS NULL OR language = ?1
             ORDER BY at DESC, rowid DESC LIMIT ?2",
        )?;
        let limit = i64::try_from(limit).unwrap_or(i64::MAX);
        let mut out = stmt
            .query_map(params![language, limit], |row| {
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

    fn slot(section: u16, lesson: u16) -> Slot {
        Slot::new(section, lesson).expect("lección")
    }

    fn attempt(language: &str, slot: Slot, stars: u8, wpm: f64) -> LessonAttempt {
        LessonAttempt {
            at: 10,
            language: language.into(),
            slot,
            target_wpm: slot.target_wpm(),
            text_id: "t1".into(),
            keyboard: "dvorak".into(),
            wpm,
            accuracy: 97.0,
            fluidity: 70.0,
            score: wpm / 10.0,
            stars,
            elapsed: Duration::from_secs(60),
        }
    }

    fn save(h: &mut History, a: &LessonAttempt) {
        let session = SessionRecord::from_stats(
            Module::Progression,
            a.at,
            &Stats::compute(100, 0, a.elapsed, &[]),
        );
        h.record_lesson(a, &session).expect("guardar");
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
    fn a_worse_attempt_keeps_the_best_results() {
        let mut h = History::in_memory().expect("base");
        let lesson = slot(75, 3);
        save(&mut h, &attempt("es", lesson, 4, 70.0));
        save(&mut h, &attempt("es", lesson, 2, 50.0));
        save(&mut h, &attempt("es", slot(20, 1), 5, 22.0));
        let bests = h.lesson_bests("es").expect("leer");
        assert_eq!(bests.len(), 2);
        assert_eq!(bests[0].slot, slot(20, 1));
        assert_eq!(bests[1].slot, lesson);
        assert_eq!(bests[1].stars, 4);
        assert!((bests[1].wpm - 70.0).abs() < 1e-9);
        assert!((bests[1].score - 7.0).abs() < 1e-9);
    }

    #[test]
    fn progress_stays_when_a_text_is_replaced() {
        let mut h = History::in_memory().expect("base");
        let lesson = slot(20, 1);
        let old = LessonAttempt {
            text_id: "provisional".into(),
            ..attempt("C", lesson, 5, 22.0)
        };
        save(&mut h, &old);
        let new = LessonAttempt {
            text_id: "simple-425".into(),
            ..attempt("C", lesson, 3, 18.0)
        };
        save(&mut h, &new);
        let bests = h.lesson_bests("C").expect("leer");
        assert_eq!(bests.len(), 1);
        assert_eq!(bests[0].stars, 5);
    }

    #[test]
    fn lesson_progress_survives_closing_the_database() {
        let dir = tempfile::tempdir().expect("carpeta");
        let path = dir.path().join("history.sqlite3");
        {
            let mut h = History::open(&path).expect("abrir");
            save(&mut h, &attempt("es", slot(20, 1), 4, 22.0));
        }
        let h = History::open(&path).expect("reabrir");
        let bests = h.lesson_bests("es").expect("leer");
        assert_eq!(bests.len(), 1);
        assert_eq!(bests[0].stars, 4);
    }

    #[test]
    fn resetting_the_progression_keeps_sessions_and_ranking() {
        let mut h = History::in_memory().expect("base");
        save(&mut h, &attempt("C", slot(20, 1), 5, 22.0));
        save(&mut h, &attempt("es", slot(20, 1), 5, 22.0));
        let mut top = Ranking::default();
        top.insert(entry(5.0), true).expect("entra");
        h.save_ranking("C", &top).expect("ranking");
        assert_eq!(h.reset_lessons("C").expect("borrar"), 1);
        assert_eq!(h.lesson_bests("C").expect("leer"), Vec::new());
        assert_eq!(h.lesson_bests("es").expect("leer").len(), 1);
        assert_eq!(h.count(Module::Progression).expect("contar"), 2);
        assert_eq!(h.ranking("C").expect("ranking").entries().len(), 1);
    }

    #[test]
    fn filters_sessions_by_exact_lesson_name() {
        let h = History::in_memory().expect("base");
        for (at, lesson) in [(1, "20-01"), (2, "20-02"), (3, "25-01")] {
            let mut record = session(Module::Progression, at, 20.0);
            record.lesson = Some(lesson.into());
            h.record(&record).expect("guardar");
        }
        let mut filter = SessionFilter::module(Module::Progression);
        filter.lesson_name = Some("20-02".into());
        assert_eq!(h.sessions(&filter, 10).expect("leer").len(), 1);
        let mut filter = SessionFilter::module(Module::Progression);
        filter.lesson = Some(20);
        assert_eq!(h.sessions(&filter, 10).expect("leer").len(), 2);
    }

    #[test]
    fn lesson_progress_is_kept_per_language() {
        let mut h = History::in_memory().expect("base");
        save(&mut h, &attempt("es", slot(20, 1), 5, 22.0));
        assert_eq!(h.lesson_bests("C").expect("leer"), Vec::new());
        assert_eq!(h.lesson_bests("es").expect("leer").len(), 1);
    }

    #[test]
    fn a_lesson_attempt_also_enters_the_history() {
        let mut h = History::in_memory().expect("base");
        save(&mut h, &attempt("es", slot(20, 1), 5, 22.0));
        assert_eq!(h.count(Module::Progression).expect("contar"), 1);
    }

    #[test]
    fn unknown_lessons_are_ignored() {
        let mut h = History::in_memory().expect("base");
        save(&mut h, &attempt("es", slot(20, 1), 5, 22.0));
        h.conn
            .execute("UPDATE progression_attempts SET section = 21", [])
            .expect("editar");
        assert_eq!(h.lesson_bests("es").expect("leer"), Vec::new());
    }

    #[test]
    fn upgrades_a_version_2_database_without_losing_data() {
        let conn = Connection::open_in_memory().expect("base");
        conn.execute_batch(
            "CREATE TABLE sessions (
                 id INTEGER PRIMARY KEY, module TEXT NOT NULL, at INTEGER NOT NULL,
                 accuracy REAL NOT NULL, wpm REAL NOT NULL, fluidity REAL NOT NULL,
                 touches INTEGER, errors INTEGER, elapsed_ms INTEGER, lesson TEXT,
                 keyboard TEXT, language TEXT);
             INSERT INTO sessions (module, at, accuracy, wpm, fluidity)
                 VALUES ('fluid', 5, 98.0, 40.0, 70.0);
             PRAGMA user_version = 2;",
        )
        .expect("esquema 2");
        let mut h = History::init(conn).expect("actualizar");
        let version: i64 = h
            .conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .expect("versión");
        assert_eq!(version, SCHEMA_VERSION);
        assert_eq!(h.count(Module::Fluidity).expect("contar"), 1);
        save(&mut h, &attempt("es", slot(20, 1), 5, 22.0));
        assert_eq!(h.lesson_bests("es").expect("leer").len(), 1);
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
    fn filters_sessions_by_lesson_keyboard_and_language() {
        let h = History::in_memory().expect("base");
        let lesson = |at, lesson: &str, keyboard: &str| {
            let mut r = session(Module::Basic, at, 30.0);
            r.lesson = Some(lesson.into());
            r.keyboard = Some(keyboard.into());
            h.record(&r).expect("guardar");
        };
        lesson(1, "03", "qwerty_es");
        lesson(2, "3", "qwerty_es");
        lesson(3, "04", "qwerty_es");
        lesson(4, "03", "dvorak");
        let filter = SessionFilter {
            lesson_name: None,
            lesson: Some(3),
            keyboard: Some("qwerty_es".into()),
            ..SessionFilter::module(Module::Basic)
        };
        let found = h.sessions(&filter, 50).expect("leer");
        assert_eq!(found.iter().map(|r| r.at).collect::<Vec<_>>(), [1, 2]);

        let mut velo = session(Module::Velocity, 5, 40.0);
        velo.language = Some("es".into());
        h.record(&velo).expect("guardar");
        let filter = |l: &str| SessionFilter {
            lesson_name: None,
            language: Some(l.into()),
            ..SessionFilter::module(Module::Velocity)
        };
        assert_eq!(h.sessions(&filter("es"), 50).expect("leer").len(), 1);
        assert_eq!(h.sessions(&filter("C"), 50).expect("leer").len(), 0);
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
        let scores = h.recent_scores(None, 2).expect("leer");
        assert_eq!(scores.iter().map(|s| s.at).collect::<Vec<_>>(), [2, 3]);
        assert_eq!(h.recent_scores(Some("es"), 10).expect("leer").len(), 3);
        assert_eq!(h.recent_scores(Some("C"), 10).expect("leer").len(), 0);
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
