// SPDX-License-Identifier: GPL-3.0-or-later
//! Generación de ejercicios y textos traducidos de cada resultado. Compartido por la
//! interfaz gráfica y la práctica en terminal.

use std::io;

use klavaro_core::Module;
use klavaro_core::generators::{LanguageRules, adaptability, basic, fluidity, velocity};
use klavaro_core::goals::Comment;
use klavaro_core::keyboard::KeyboardLayout;
use klavaro_core::ranking::{MIN_CHARS, Rejection};
use klavaro_core::rng::SplitMix64;
use klavaro_core::tracker::KeyTracker;
use klavaro_i18n::Translator;

use crate::assets;
use crate::profile::{Notice, Recorded};

/// Qué ejercicio generar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExerciseSpec {
    pub(crate) module: Module,
    /// Lección del curso básico (1–43).
    pub(crate) lesson: u16,
    /// Párrafos de Fluidez (0 = texto completo).
    pub(crate) paragraphs: usize,
    pub(crate) language: String,
}

/// Genera el texto de un ejercicio con los datos de `assets/`. `weak_keys` (ordenado, ver
/// [`KeyTracker::sort`]) hace que Adaptabilidad insista en las teclas débiles.
pub(crate) fn generate(
    spec: &ExerciseSpec,
    layout: &KeyboardLayout,
    seed: u64,
    weak_keys: Option<&KeyTracker>,
) -> io::Result<String> {
    let rules = LanguageRules::new(spec.language.clone());
    let mut rng = SplitMix64::new(seed);
    let text = match spec.module {
        Module::Basic => {
            let masks = assets::basic_lessons()?;
            let index = usize::from(spec.lesson.clamp(1, basic::FIXED_LESSONS)) - 1;
            let set = masks
                .get(index)
                .map(|m| m.charset(layout))
                .unwrap_or_default();
            basic::generate(&set, &rules, &mut rng)
        }
        Module::Adaptability => Some(adaptability::generate(layout, &rules, weak_keys, &mut rng)),
        Module::Velocity => velocity::generate(&assets::words(&spec.language)?, &rules, &mut rng),
        Module::Fluidity => fluidity::select(
            &assets::paragraphs(&spec.language)?,
            spec.paragraphs,
            false,
            &mut rng,
        ),
    };
    text.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "no se pudo generar el ejercicio con este teclado e idioma",
        )
    })
}

/// Clave de Fluent del nombre de un módulo.
pub(crate) fn module_key(m: Module) -> &'static str {
    match m {
        Module::Basic => "module-basic",
        Module::Adaptability => "module-adaptability",
        Module::Velocity => "module-velocity",
        Module::Fluidity => "module-fluidity",
    }
}

/// Nombre del módulo para mostrar.
pub(crate) fn module_name(tr: &Translator, m: Module) -> String {
    tr.tr(module_key(m))
}

/// Un número sin decimales, como los `%.0f` del original.
pub(crate) fn whole(v: f64) -> String {
    format!("{v:.0}")
}

/// Texto del comentario de fin de ejercicio.
pub(crate) fn comment_text(tr: &Translator, c: Comment) -> String {
    let (key, target) = match c {
        Comment::TooManyErrors => ("comment-too-many-errors", None),
        Comment::BasicNextLesson => ("comment-basic-next", None),
        Comment::BasicCourseCompleted => ("comment-basic-done", None),
        Comment::AdaptTryAgain { target } => ("comment-adapt-try-again", Some(target)),
        Comment::AdaptImproving { target } => ("comment-adapt-improving", Some(target)),
        Comment::AdaptAlmostThere { target } => ("comment-adapt-almost", Some(target)),
        Comment::AdaptDone { target } => ("comment-adapt-done", Some(target)),
        Comment::VeloBeginning => ("comment-velo-beginning", None),
        Comment::VeloHighway { target } => ("comment-velo-highway", Some(target)),
        Comment::VeloFaster { target } => ("comment-velo-faster", Some(target)),
        Comment::VeloRunning { target } => ("comment-velo-running", Some(target)),
        Comment::VeloAlmostThere { target } => ("comment-velo-almost", Some(target)),
        Comment::VeloEnough => ("comment-velo-enough", None),
        Comment::VeloProfessional { target } => ("comment-velo-professional", Some(target)),
        Comment::VeloSpeedRacer { target } => ("comment-velo-speed-racer", Some(target)),
        Comment::VeloKungFu { target } => ("comment-velo-kung-fu", Some(target)),
        Comment::VeloMaster => ("comment-velo-master", None),
        Comment::FluidTooSlow { target } => ("comment-fluid-too-slow", Some(target)),
        Comment::FluidIrregular { target } => ("comment-fluid-irregular", Some(target)),
        Comment::FluidAlmostThere { target } => ("comment-fluid-almost", Some(target)),
        Comment::FluidCongratulations => ("comment-fluid-congratulations", None),
        Comment::FluidBeyondExpectations => ("comment-fluid-beyond", None),
    };
    match target {
        Some(t) => tr.tr_args(key, &[("target", &whole(t))]),
        None => tr.tr(key),
    }
}

/// Nota sobre el registro del ejercicio, si hay algo que contar.
#[cfg_attr(not(feature = "gui"), allow(dead_code))]
pub(crate) fn recorded_text(tr: &Translator, r: Recorded) -> Option<String> {
    match r {
        Recorded::Logged(None | Some(Rejection::ScoreTooLow)) => None,
        Recorded::Logged(Some(Rejection::Invalid)) => Some(tr.tr("ps-invalid")),
        Recorded::Logged(Some(Rejection::TextNotSimilar)) => Some(tr.tr("ps-not-similar")),
        Recorded::Ranked(_) => Some(tr.tr("ps-ranked")),
        Recorded::TooShort { chars } => Some(tr.tr_args(
            "ps-not-logged",
            &[
                ("chars", &chars.to_string()),
                ("min", &MIN_CHARS.to_string()),
            ],
        )),
    }
}

/// Mensaje al abrir la aplicación.
#[cfg_attr(not(feature = "gui"), allow(dead_code))]
pub(crate) fn notice_text(tr: &Translator, n: &Notice) -> String {
    match n {
        Notice::Imported { sessions } => {
            tr.tr_args("notice-imported", &[("sessions", &sessions.to_string())])
        }
        Notice::Error(e) => tr.tr_args("notice-error", &[("error", e)]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_module_generates_with_repo_assets() {
        let layout = assets::keyboard("qwerty_es").expect("kbd");
        for module in Module::ALL {
            let spec = ExerciseSpec {
                module,
                lesson: 1,
                paragraphs: 3,
                language: "es".to_owned(),
            };
            let text = generate(&spec, &layout, 3, None).expect("texto");
            assert!(!text.trim().is_empty(), "{module:?}");
        }
    }

    /// Ningún texto queda sin traducir (mostraría la clave) en inglés ni en español.
    #[test]
    fn every_message_is_translated() {
        let comments = [
            Comment::TooManyErrors,
            Comment::BasicNextLesson,
            Comment::BasicCourseCompleted,
            Comment::AdaptTryAgain { target: 90.0 },
            Comment::AdaptImproving { target: 95.0 },
            Comment::AdaptAlmostThere { target: 98.0 },
            Comment::AdaptDone { target: 98.0 },
            Comment::VeloBeginning,
            Comment::VeloHighway { target: 20.0 },
            Comment::VeloFaster { target: 30.0 },
            Comment::VeloRunning { target: 40.0 },
            Comment::VeloAlmostThere { target: 50.0 },
            Comment::VeloEnough,
            Comment::VeloProfessional { target: 70.0 },
            Comment::VeloSpeedRacer { target: 80.0 },
            Comment::VeloKungFu { target: 90.0 },
            Comment::VeloMaster,
            Comment::FluidTooSlow { target: 50.0 },
            Comment::FluidIrregular { target: 60.0 },
            Comment::FluidAlmostThere { target: 70.0 },
            Comment::FluidCongratulations,
            Comment::FluidBeyondExpectations,
        ];
        for code in ["C", "es"] {
            let tr = Translator::new(code);
            for c in comments {
                let text = comment_text(&tr, c);
                assert!(!text.starts_with("comment-"), "{code}: {c:?}");
                assert!(!text.contains("{$"), "{code}: {c:?} sin argumento");
            }
            for m in Module::ALL {
                assert!(tr.has(module_key(m)), "{code}: {m:?}");
            }
            let notes = [
                Recorded::Logged(Some(Rejection::Invalid)),
                Recorded::Logged(Some(Rejection::TextNotSimilar)),
                Recorded::Ranked(1),
                Recorded::TooShort { chars: 10 },
            ];
            for r in notes {
                let text = recorded_text(&tr, r).expect("nota");
                assert!(
                    !text.starts_with("ps-") && !text.contains("{$"),
                    "{code}: {r:?}"
                );
            }
        }
        let es = Translator::new("es");
        assert!(comment_text(&es, Comment::VeloRunning { target: 40.0 }).contains("40 PPM"));
        assert!(
            recorded_text(&es, Recorded::TooShort { chars: 120 })
                .is_some_and(|t| t.contains("(120)") && t.contains("500"))
        );
    }
}
