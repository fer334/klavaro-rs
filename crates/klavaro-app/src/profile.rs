// SPDX-License-Identifier: GPL-3.0-or-later
//! Datos del usuario: configuración, historial y Top 10, sobre `klavaro-store`.
//!
//! Si algo del disco falla, la app sigue funcionando: la configuración queda en memoria, el
//! historial pasa a una base en memoria (o, si ni eso se puede, no se registra) y el problema
//! se informa en [`Profile::notice`].

use std::time::{SystemTime, UNIX_EPOCH};

use klavaro_core::Module;
use klavaro_core::progression::{NextStep, Slot};
use klavaro_core::ranking::{MIN_CHARS, Ranking, RankingEntry, Rejection, text_resembles_language};
use klavaro_core::tracker::KeyTracker;
use klavaro_store::history::ScoreRecord;
use klavaro_store::{
    Config, History, LessonAttempt, LessonBest, Paths, SessionRecord, StoreError, import, paths,
};

use crate::exercise::ExerciseSpec;
use crate::progress::{Chart, ChartQuery};
use crate::sections;
use crate::tutor::Outcome;

/// Qué pasó al registrar un ejercicio terminado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Recorded {
    /// Se guardó en el historial (y, en Fluidez, no entró al Top 10 por el motivo dado).
    Logged(Option<Rejection>),
    /// Fluidez: entró al Top 10 en ese puesto.
    Ranked(usize),
    /// Progresión: la lección jugada, si hubo récord y qué sigue.
    Lesson {
        /// Si superó el mejor resultado anterior de la lección.
        record: bool,
        /// Qué abre la lección.
        next: NextStep,
        /// La lección.
        slot: Slot,
    },
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
        /// Si además se aplicaron sus preferencias (sólo cuando no había `config.toml`).
        preferences: bool,
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
        // Antes de importar: una base importada con una versión anterior guardó horas locales.
        let imported_before = history.meta(import::IMPORTED_KEY).ok().flatten().is_some();
        if let Err(e) = import::utc_times_once(&mut history, !imported_before) {
            fail(e);
        }
        match import::import_once(&mut history, &paths::legacy_dirs()) {
            Ok(Some(report)) if !report.is_empty() => {
                let preferences = is_new && report.preferences.is_some();
                if is_new && let Some(prefs) = &report.preferences {
                    config = config.with_legacy(prefs);
                }
                notice.get_or_insert(Notice::Imported {
                    sessions: report.sessions,
                    preferences,
                });
            }
            Ok(_) => {}
            Err(e) => fail(e),
        }
        if let Err(e) = import::import_keys_once(&mut history, &paths::legacy_dirs()) {
            notice.get_or_insert(Notice::Error(e.to_string()));
        }
        if let Err(e) = import::import_ranking_once(&mut history, &paths::legacy_dirs()) {
            notice.get_or_insert(Notice::Error(e.to_string()));
        }
        if let Err(e) = import::normalize_languages_once(&mut history) {
            notice.get_or_insert(Notice::Error(e.to_string()));
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
        if spec.module == Module::Progression {
            return self.record_lesson(spec, keyboard, outcome);
        }
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
            Module::Velocity | Module::Fluidity | Module::Progression => {
                record.lesson = Some("Default".to_owned());
            }
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

    /// Rastreo por tecla guardado de un teclado (vacío si no hay historial).
    pub(crate) fn tracker(&self, keyboard: &str) -> Result<KeyTracker, StoreError> {
        self.history
            .as_ref()
            .map_or_else(|| Ok(KeyTracker::new()), |h| h.tracker(keyboard))
    }

    /// Guarda el rastreo por tecla de un teclado.
    pub(crate) fn save_tracker(
        &mut self,
        keyboard: &str,
        tracker: &KeyTracker,
    ) -> Result<(), StoreError> {
        match self.history.as_mut() {
            Some(h) => h.save_tracker(keyboard, tracker),
            None => Ok(()),
        }
    }

    /// Guarda el intento de una lección de la Progresión, sin el mínimo de caracteres ni el
    /// Top 10 de Fluidez.
    fn record_lesson(
        &mut self,
        spec: &ExerciseSpec,
        keyboard: &str,
        outcome: &Outcome,
    ) -> Result<Recorded, StoreError> {
        let (Some(result), Some(history)) = (outcome.lesson, self.history.as_mut()) else {
            return Ok(Recorded::Logged(None));
        };
        let slot = result.slot;
        let stats = &outcome.stats;
        let before = history
            .lesson_bests(&spec.language)?
            .into_iter()
            .find(|b| b.slot == slot);
        let at = now();
        let attempt = LessonAttempt {
            at,
            language: spec.language.clone(),
            slot,
            target_wpm: result.target_wpm,
            text_id: outcome
                .source
                .as_ref()
                .map_or_else(String::new, |source| source.text_id.clone()),
            keyboard: keyboard.to_owned(),
            wpm: result.speed,
            accuracy: stats.accuracy,
            fluidity: stats.fluidity,
            score: stats.score(),
            stars: result.stars,
            elapsed: stats.elapsed,
        };
        let mut session = SessionRecord::from_stats(spec.module, at, stats);
        session.wpm = result.speed;
        session.lesson = Some(format!("{}-{:02}", slot.section(), slot.lesson()));
        session.keyboard = Some(keyboard.to_owned());
        session.language = Some(spec.language.clone());
        history.record_lesson(&attempt, &session)?;
        let ladder = sections::ladder(&history.lesson_bests(&spec.language)?);
        Ok(Recorded::Lesson {
            record: before.is_some_and(|b| result.speed > b.wpm || result.stars > b.stars),
            next: ladder.next_step(slot),
            slot,
        })
    }

    /// Mejores resultados de cada lección de la Progresión en un idioma.
    pub(crate) fn lesson_bests(&self, language: &str) -> Result<Vec<LessonBest>, StoreError> {
        self.history
            .as_ref()
            .map_or_else(|| Ok(Vec::new()), |h| h.lesson_bests(language))
    }

    /// Gráfico de progreso (vacío si no hay historial).
    pub(crate) fn chart(&self, q: &ChartQuery) -> Result<Chart, StoreError> {
        match &self.history {
            Some(h) => Chart::load(h, &self.config.goals, q),
            None => Ok(Chart::new(q.metric, Vec::new(), None)),
        }
    }

    /// Top 10 de un idioma.
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

    use klavaro_core::goals::{Comment, GoalCheck, Goals};
    use klavaro_core::metrics::Stats;
    use klavaro_core::progression::Ladder;

    use super::*;
    use crate::tutor::{Input, Tutor};

    /// Juega una lección de punta a punta con el tutor real y la registra.
    fn play_lesson(profile: &mut Profile, slot: Slot, ms_per_key: u64) -> Recorded {
        let spec = ExerciseSpec {
            slot: Some(slot),
            language: sections::LESSONS_LANGUAGE.to_owned(),
            ..spec(Module::Progression)
        };
        let layout = crate::assets::keyboard("qwerty_es").expect("kbd");
        let mut tutor =
            Tutor::new(spec, layout, Goals::default(), 1, KeyTracker::new()).expect("tutor");
        for (i, c) in tutor.text().chars().enumerate() {
            let at = Duration::from_millis(ms_per_key * i as u64);
            tutor.handle(Input::Char(c), at).expect("tecla");
        }
        let outcome = tutor.outcome().cloned().expect("terminó");
        profile
            .record(tutor.spec(), "dvorak", &outcome, &tutor.text(), "")
            .expect("guardar")
    }

    fn stars_saved(profile: &Profile) -> Ladder {
        sections::ladder(
            &profile
                .lesson_bests(sections::LESSONS_LANGUAGE)
                .expect("leer"),
        )
    }

    #[test]
    fn passing_a_whole_section_opens_the_next_one() {
        let mut profile = Profile::in_memory(Config::default()).expect("perfil");
        assert!(!stars_saved(&profile).section_unlocked(25));
        // Lo bastante rápido para las 5 estrellas en cada meta de la sección de 20 PPM.
        for lesson in 1..=30 {
            let slot = Slot::new(20, lesson).expect("lección");
            assert!(stars_saved(&profile).lesson_unlocked(slot), "{slot:?}");
            let recorded = play_lesson(&mut profile, slot, 200);
            let Recorded::Lesson { next, .. } = recorded else {
                panic!("{recorded:?}");
            };
            assert_eq!(next, stars_saved(&profile).next_step(slot));
        }
        let saved = stars_saved(&profile);
        assert!(saved.section_passed(20));
        assert!(saved.lesson_unlocked(Slot::new(25, 1).expect("lección")));
        assert_eq!(saved.total_stars(), 150);
        let history = profile.history().expect("historial");
        assert_eq!(history.count(Module::Progression).expect("contar"), 30);
        assert_eq!(history.count(Module::Fluidity).expect("contar"), 0);
    }

    #[test]
    fn without_a_history_lessons_can_still_be_played() {
        let mut profile = Profile::in_memory(Config::default()).expect("perfil");
        profile.history = None;
        assert_eq!(
            play_lesson(&mut profile, Slot::FIRST, 200),
            Recorded::Logged(None)
        );
        assert_eq!(
            profile
                .lesson_bests(sections::LESSONS_LANGUAGE)
                .expect("leer"),
            Vec::new()
        );
    }

    #[test]
    fn a_short_lesson_is_saved_and_stays_out_of_the_top_10() {
        let mut profile = Profile::in_memory(Config::default()).expect("perfil");
        let recorded = play_lesson(&mut profile, Slot::FIRST, 200);
        assert_eq!(
            recorded,
            Recorded::Lesson {
                record: false,
                next: NextStep::Open(Slot::new(20, 2).expect("lección")),
                slot: Slot::FIRST,
            }
        );
        assert_eq!(profile.ranking("es").expect("top").entries().len(), 0);
    }

    #[test]
    fn a_worse_attempt_keeps_the_stars_and_a_better_one_is_a_record() {
        let mut profile = Profile::in_memory(Config::default()).expect("perfil");
        // A 15,17 PPM de meta toca una tecla cada 791 ms: 1100 ms da menos del 80 %.
        play_lesson(&mut profile, Slot::FIRST, 1100);
        assert_eq!(stars_saved(&profile).stars(Slot::FIRST), 2);
        assert!(matches!(
            play_lesson(&mut profile, Slot::FIRST, 200),
            Recorded::Lesson { record: true, .. }
        ));
        assert_eq!(stars_saved(&profile).stars(Slot::FIRST), 5);
        assert!(matches!(
            play_lesson(&mut profile, Slot::FIRST, 1100),
            Recorded::Lesson { record: false, .. }
        ));
        assert_eq!(stars_saved(&profile).stars(Slot::FIRST), 5);
    }

    const MODEL: &str = "En un lugar de la Mancha, de cuyo nombre no quiero acordarme, no ha \
        mucho tiempo que vivía un hidalgo de los de lanza en astillero, adarga antigua.";

    fn spec(module: Module) -> ExerciseSpec {
        ExerciseSpec {
            module,
            lesson: 4,
            paragraphs: 3,
            language: "es".to_owned(),
            slot: None,
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
            lesson: None,
            source: None,
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
                .recent_scores(None, 5)
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
