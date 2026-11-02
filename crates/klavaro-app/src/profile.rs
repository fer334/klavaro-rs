// SPDX-License-Identifier: GPL-3.0-or-later
//! Datos del usuario: configuración, historial y Top 10, sobre `klavaro-store`.
//!
//! Si algo del disco falla, la app sigue funcionando: la configuración queda en memoria, el
//! historial pasa a una base en memoria (o, si ni eso se puede, no se registra) y el problema
//! se informa en [`Profile::notice`].

use std::time::{SystemTime, UNIX_EPOCH};

use klavaro_core::Module;
use klavaro_core::ranking::{MIN_CHARS, Ranking, RankingEntry, Rejection, text_resembles_language};
use klavaro_store::history::ScoreRecord;
use klavaro_store::{Config, History, Paths, SessionRecord, StoreError, import, paths};

use crate::exercise::ExerciseSpec;
use crate::tutor::Outcome;

/// Qué pasó al registrar un ejercicio terminado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Recorded {
    /// Se guardó en el historial (y, en Fluidez, no entró al Top 10 por el motivo dado).
    Logged(Option<Rejection>),
    /// Fluidez: entró al Top 10 en ese puesto.
    Ranked(usize),
    /// Fluidez con menos de [`MIN_CHARS`] caracteres: no se guarda, como en el original.
    TooShort {
        /// Caracteres tecleados.
        chars: u32,
    },
}

/// Mensaje para mostrar al abrir (importación o errores del disco).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Notice {
    /// Se importaron datos del Klavaro original.
    Imported {
        /// Sesiones agregadas.
        sessions: usize,
    },
    /// Error de lectura o escritura.
    Error(String),
}

/// Configuración e historial del usuario.
#[derive(Debug)]
pub(crate) struct Profile {
    paths: Option<Paths>,
    pub(crate) config: Config,
    history: Option<History>,
    pub(crate) notice: Option<Notice>,
}

impl Profile {
    /// Abre los archivos del usuario e importa los del Klavaro original la primera vez.
    pub(crate) fn open() -> Self {
        let paths = match Paths::detect() {
            Ok(p) => p,
            Err(e) => return Self::fallback(&e),
        };
        let mut notice = None;
        let mut fail = |e: StoreError| {
            notice.get_or_insert(Notice::Error(e.to_string()));
        };
        let config = Config::load(&paths.config_file()).unwrap_or_else(|e| {
            fail(e);
            Some(Config::default())
        });
        let mut history = match History::open(&paths.history_file()) {
            Ok(h) => h,
            Err(e) => {
                fail(e);
                return Self::fallback_with(Some(paths), config.unwrap_or_default(), notice);
            }
        };
        let is_new = config.is_none();
        let mut config = config.unwrap_or_default();
        match import::import_once(&mut history, &paths::legacy_dirs()) {
            Ok(Some(report)) if !report.is_empty() => {
                if is_new && let Some(prefs) = &report.preferences {
                    config = config.with_legacy(prefs);
                }
                notice.get_or_insert(Notice::Imported {
                    sessions: report.sessions,
                });
            }
            Ok(_) => {}
            Err(e) => fail(e),
        }
        let profile = Self {
            paths: Some(paths),
            config,
            history: Some(history),
            notice,
        };
        if is_new && let Err(e) = profile.save_config() {
            return Self {
                notice: Some(Notice::Error(e.to_string())),
                ..profile
            };
        }
        profile
    }

    /// Perfil en memoria, sin archivos.
    #[cfg(test)]
    pub(crate) fn in_memory(config: Config) -> Result<Self, StoreError> {
        Ok(Self {
            paths: None,
            config,
            history: Some(History::in_memory()?),
            notice: None,
        })
    }

    fn fallback(e: &StoreError) -> Self {
        Self::fallback_with(None, Config::default(), Some(Notice::Error(e.to_string())))
    }

    fn fallback_with(paths: Option<Paths>, config: Config, notice: Option<Notice>) -> Self {
        Self {
            paths,
            config,
            history: History::in_memory().ok(),
            notice,
        }
    }

    /// Guarda la configuración (sin archivos, no hace nada).
    pub(crate) fn save_config(&self) -> Result<(), StoreError> {
        match &self.paths {
            Some(p) => self.config.save(&p.config_file()),
            None => Ok(()),
        }
    }

    /// Historial (para las estadísticas).
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn history(&self) -> Option<&History> {
        self.history.as_ref()
    }

    /// Registra un ejercicio terminado. `typed` es el texto del ejercicio y `model` un texto
    /// de referencia del idioma (para la prueba de distribución de letras del Top 10).
    pub(crate) fn record(
        &mut self,
        spec: &ExerciseSpec,
        keyboard: &str,
        outcome: &Outcome,
        typed: &str,
        model: &str,
    ) -> Result<Recorded, StoreError> {
        let stats = &outcome.stats;
        if spec.module == Module::Fluidity && stats.touches < MIN_CHARS {
            return Ok(Recorded::TooShort {
                chars: stats.touches,
            });
        }
        let Some(history) = self.history.as_mut() else {
            return Ok(Recorded::Logged(None));
        };
        let at = now();
        let mut record = SessionRecord::from_stats(spec.module, at, stats);
        record.keyboard = Some(keyboard.to_owned());
        match spec.module {
            Module::Basic => record.lesson = Some(format!("{:02}", spec.lesson)),
            Module::Adaptability => {}
            Module::Velocity | Module::Fluidity => record.lesson = Some("Default".to_owned()),
        }
        if spec.module != Module::Basic {
            record.language = Some(spec.language.clone());
        }
        history.record(&record)?;
        if spec.module != Module::Fluidity {
            return Ok(Recorded::Logged(None));
        }

        history.record_score(&ScoreRecord {
            at,
            score: stats.score(),
            chars: stats.touches,
            language: spec.language.clone(),
        })?;
        let name = ranking_name(self.config.player_name.as_deref(), keyboard);
        let entry = RankingEntry::from_stats(name, spec.language.clone(), at, stats);
        let mut top = history.ranking(&spec.language)?;
        match top.insert(entry, text_resembles_language(model, typed)) {
            Ok(place) => {
                history.save_ranking(&spec.language, &top)?;
                Ok(Recorded::Ranked(place))
            }
            Err(why) => Ok(Recorded::Logged(Some(why))),
        }
    }

    /// Top 10 de un idioma.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn ranking(&self, language: &str) -> Result<Ranking, StoreError> {
        self.history
            .as_ref()
            .map_or_else(|| Ok(Ranking::default()), |h| h.ranking(language))
    }
}

/// Nombre para el Top 10, con el teclado entre corchetes como el original.
fn ranking_name(player: Option<&str>, keyboard: &str) -> String {
    let name = player
        .map(str::to_owned)
        .or_else(system_user_name)
        .unwrap_or_else(|| "?".to_owned());
    format!("{name} [{keyboard}]")
}

/// Segundos desde 1970.
pub(crate) fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
}

fn system_user_name() -> Option<String> {
    ["USER", "USERNAME", "LOGNAME"]
        .iter()
        .find_map(|v| std::env::var(v).ok())
        .filter(|n| !n.trim().is_empty())
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use klavaro_core::goals::{Comment, GoalCheck};
    use klavaro_core::metrics::Stats;

    use super::*;

    const MODEL: &str = "En un lugar de la Mancha, de cuyo nombre no quiero acordarme, no ha \
        mucho tiempo que vivía un hidalgo de los de lanza en astillero, adarga antigua.";

    fn spec(module: Module) -> ExerciseSpec {
        ExerciseSpec {
            module,
            lesson: 4,
            paragraphs: 3,
            language: "es".to_owned(),
        }
    }

    fn outcome(touches: u32) -> Outcome {
        Outcome {
            stats: Stats {
                touches,
                errors: 2,
                elapsed: Duration::from_secs(120),
                accuracy: 99.0,
                wpm: 50.0,
                fluidity: 70.0,
            },
            check: GoalCheck {
                accuracy: true,
                speed: true,
                fluidity: Some(true),
            },
            comment: Comment::FluidCongratulations,
            advance: false,
        }
    }

    fn profile() -> Profile {
        let config = Config {
            player_name: Some("Ana".into()),
            ..Config::default()
        };
        Profile::in_memory(config).expect("perfil")
    }

    #[test]
    fn records_basic_sessions() {
        let mut p = profile();
        let r = p
            .record(&spec(Module::Basic), "qwerty_es", &outcome(100), "", MODEL)
            .expect("guardar");
        assert_eq!(r, Recorded::Logged(None));
        let saved = p
            .history()
            .expect("historial")
            .recent(Module::Basic, 5)
            .expect("leer");
        assert_eq!(saved[0].lesson.as_deref(), Some("04"));
        assert_eq!(saved[0].keyboard.as_deref(), Some("qwerty_es"));
        assert_eq!(saved[0].language, None);
    }

    #[test]
    fn short_fluidity_sessions_are_not_logged() {
        let mut p = profile();
        let r = p
            .record(
                &spec(Module::Fluidity),
                "qwerty_es",
                &outcome(120),
                MODEL,
                MODEL,
            )
            .expect("guardar");
        assert_eq!(r, Recorded::TooShort { chars: 120 });
        assert_eq!(
            p.history()
                .expect("historial")
                .count(Module::Fluidity)
                .expect("contar"),
            0
        );
    }

    #[test]
    fn fluidity_enters_the_top_10() {
        let mut p = profile();
        let r = p
            .record(
                &spec(Module::Fluidity),
                "qwerty_es",
                &outcome(700),
                MODEL,
                MODEL,
            )
            .expect("guardar");
        assert_eq!(r, Recorded::Ranked(1));
        let top = p.ranking("es").expect("ranking");
        assert_eq!(top.entries()[0].name, "Ana [qwerty_es]");
        assert_eq!(
            p.history()
                .expect("historial")
                .recent_scores(5)
                .expect("puntajes")
                .len(),
            1
        );

        let r = p
            .record(
                &spec(Module::Fluidity),
                "qwerty_es",
                &outcome(700),
                "zzzz qqqq",
                MODEL,
            )
            .expect("guardar");
        assert_eq!(r, Recorded::Logged(Some(Rejection::TextNotSimilar)));
        assert_eq!(
            p.history()
                .expect("historial")
                .count(Module::Fluidity)
                .expect("contar"),
            2
        );
    }
}
