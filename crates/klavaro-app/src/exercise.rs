// SPDX-License-Identifier: GPL-3.0-or-later
//! Generación de ejercicios y textos traducidos de cada resultado. Compartido por la
//! interfaz gráfica y la práctica en terminal.

use std::io;

use klavaro_core::Module;
use klavaro_core::generators::{LanguageRules, adaptability, basic, fluidity, velocity};
use klavaro_core::goals::Comment;
use klavaro_core::keyboard::KeyboardLayout;
use klavaro_core::progression::{LessonResult, Milestone, NextStep, Slot};
use klavaro_core::ranking::{MIN_CHARS, Rejection};
use klavaro_core::rng::SplitMix64;
use klavaro_core::tracker::KeyTracker;
use klavaro_core::transitions::TransitionSummary;
use klavaro_i18n::Translator;
use klavaro_store::PreviousTransitions;

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
    /// Lección de la Progresión.
    pub(crate) slot: Option<Slot>,
    /// Si la lección se juega como examen de su sección.
    pub(crate) exam: bool,
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
        Module::Progression => match spec.slot {
            Some(slot) => Some(assets::lesson(&spec.language, slot)?.text),
            None => None,
        },
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
        Module::Progression => "module-progression",
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
        Recorded::Lesson {
            record,
            gained,
            next,
            milestone,
            exam,
        } => lesson_note(tr, record, gained, next, milestone, exam),
        Recorded::TooShort { chars } => Some(tr.tr_args(
            "ps-not-logged",
            &[
                ("chars", &chars.to_string()),
                ("min", &MIN_CHARS.to_string()),
            ],
        )),
    }
}

/// Nota al terminar una lección de la Progresión: lo que se ganó y qué falta.
fn lesson_note(
    tr: &Translator,
    record: bool,
    gained: u8,
    next: NextStep,
    milestone: Option<Milestone>,
    exam: bool,
) -> Option<String> {
    let mut notes = Vec::new();
    if gained > 0 {
        notes.push(tr.tr_args("sections-gained", &[("count", &gained.to_string())]));
    }
    if record {
        notes.push(tr.tr("sections-record"));
    }
    match milestone {
        Some(Milestone::SectionPassed(section)) => {
            notes.push(tr.tr_args("sections-passed", &[("section", &section.to_string())]));
        }
        Some(Milestone::Completed) => notes.push(tr.tr("sections-completed")),
        None if exam => notes.push(tr.tr("sections-exam-failed")),
        None => match next {
            NextStep::NeedsPassStars => notes.push(tr.tr("sections-need-pass")),
            NextStep::NeedsSection => notes.push(tr.tr("sections-need-goal")),
            NextStep::Open(_) | NextStep::Completed => {}
        },
    }
    (!notes.is_empty()).then(|| notes.join(" · "))
}

/// Lo que faltó para la próxima meta de estrellas, si falta algo.
#[cfg_attr(not(feature = "gui"), allow(dead_code))]
pub(crate) fn goal_text(tr: &Translator, result: &LessonResult, accuracy: f64) -> Option<String> {
    let goal = result.goal(accuracy)?;
    let stars = goal.stars.to_string();
    let percent = |value: f64| format!("{value:.0}");
    Some(match (goal.speed, goal.accuracy) {
        (Some(speed), None) => {
            // Redondea hacia arriba para no decir que falta 0,0.
            let missing = ((speed - result.speed) * 10.0).ceil() / 10.0;
            tr.tr_args(
                "sections-goal-speed",
                &[
                    ("stars", &stars),
                    ("speed", &format!("{speed:.1}")),
                    ("missing", &format!("{missing:.1}")),
                ],
            )
        }
        (None, Some(needed)) => tr.tr_args(
            "sections-goal-accuracy",
            &[
                ("stars", &stars),
                ("accuracy", &percent(needed)),
                ("value", &format!("{accuracy:.1}")),
            ],
        ),
        (Some(speed), Some(needed)) => tr.tr_args(
            "sections-goal-both",
            &[
                ("stars", &stars),
                ("speed", &format!("{speed:.1}")),
                ("accuracy", &percent(needed)),
            ],
        ),
        (None, None) => return None,
    })
}

/// Cuánto puede bajar la precisión (en puntos) sin dejar de felicitar por más transiciones.
pub(crate) const ACCURACY_SLACK: f64 = 1.0;

/// Cómo se compara un intento con el anterior.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Comparison {
    /// Más transiciones fluidas sin perder precisión: se felicita.
    Better(String),
    /// Más transiciones fluidas, pero con menos precisión: no se felicita.
    LessAccurate(String),
}

/// Textos de las transiciones fluidas al terminar una lección.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TransitionLines {
    /// Cuántas fueron fluidas y la mejor racha.
    pub(crate) main: String,
    /// La comparación con el intento anterior, si mejoró.
    pub(crate) comparison: Option<Comparison>,
    /// El rollover, como dato.
    pub(crate) rollover: String,
}

/// Porcentaje entero de `n` sobre `total`.
fn percent_of(n: u32, total: u32) -> f64 {
    if total == 0 {
        0.0
    } else {
        100.0 * f64::from(n) / f64::from(total)
    }
}

/// Textos de las transiciones. `None` si no se midió ninguna (por ejemplo, con el método de
/// entrada). Si no mejoró respecto del intento anterior, no se dice nada: no es un castigo.
#[cfg_attr(not(feature = "gui"), allow(dead_code))]
pub(crate) fn transition_lines(
    tr: &Translator,
    summary: TransitionSummary,
    accuracy: f64,
    previous: Option<PreviousTransitions>,
) -> Option<TransitionLines> {
    if summary.measured == 0 {
        return None;
    }
    let rate = percent_of(summary.fluent, summary.measured);
    let main = tr.tr_args(
        "sections-transitions",
        &[
            ("fluent", &summary.fluent.to_string()),
            ("measured", &summary.measured.to_string()),
            ("percent", &format!("{rate:.0}")),
            ("streak", &summary.best_streak.to_string()),
        ],
    );
    let rollover = tr.tr_args(
        "sections-transitions-rollover",
        &[(
            "percent",
            &format!("{:.0}", percent_of(summary.rollovers, summary.measured)),
        )],
    );
    let comparison = previous.and_then(|p| {
        // En la misma lección se comparan cantidades; con otra, porcentajes (el largo cambia).
        let gain = if p.same_lesson {
            f64::from(summary.fluent) - f64::from(p.fluent)
        } else {
            (rate - percent_of(p.fluent, p.measured)).round()
        };
        if gain < 1.0 {
            return None;
        }
        if accuracy + ACCURACY_SLACK < p.accuracy {
            return Some(Comparison::LessAccurate(
                tr.tr("sections-transitions-less-accurate"),
            ));
        }
        let (key, arg) = if p.same_lesson {
            ("sections-transitions-more", "count")
        } else {
            ("sections-transitions-more-percent", "points")
        };
        Some(Comparison::Better(
            tr.tr_args(key, &[(arg, &format!("{gain:.0}"))]),
        ))
    });
    Some(TransitionLines {
        main,
        comparison,
        rollover,
    })
}

/// Mensaje al abrir la aplicación.
#[cfg_attr(not(feature = "gui"), allow(dead_code))]
pub(crate) fn notice_text(tr: &Translator, n: &Notice) -> String {
    match n {
        Notice::Imported {
            sessions,
            preferences,
        } => {
            let key = if *preferences {
                "notice-imported"
            } else {
                "notice-imported-sessions"
            };
            tr.tr_args(key, &[("sessions", &sessions.to_string())])
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
                slot: None,
                exam: false,
            };
            let text = generate(&spec, &layout, 3, None).expect("texto");
            assert!(!text.trim().is_empty(), "{module:?}");
        }
    }

    #[test]
    fn progression_generates_the_same_text_for_a_lesson() {
        let layout = assets::keyboard("qwerty_es").expect("kbd");
        let spec = ExerciseSpec {
            module: Module::Progression,
            lesson: 1,
            paragraphs: 3,
            language: "C".to_owned(),
            slot: Slot::new(75, 3),
            exam: false,
        };
        let first = generate(&spec, &layout, 1, None).expect("texto");
        assert_eq!(first, generate(&spec, &layout, 99, None).expect("texto"));
        let without_lesson = ExerciseSpec { slot: None, ..spec };
        assert!(generate(&without_lesson, &layout, 1, None).is_err());
    }

    fn summary(measured: u32, fluent: u32) -> TransitionSummary {
        TransitionSummary {
            measured,
            fluent,
            rollovers: 25,
            best_streak: 6,
            current_streak: 0,
        }
    }

    #[test]
    fn transitions_praise_only_a_real_improvement() {
        let tr = Translator::new("es");
        let prev = |same_lesson, fluent, accuracy| {
            Some(PreviousTransitions {
                same_lesson,
                measured: 100,
                fluent,
                accuracy,
            })
        };
        let lines = |s, accuracy, p| transition_lines(&tr, s, accuracy, p).expect("medidas");

        // Sin nada medido no se muestra nada.
        assert_eq!(transition_lines(&tr, summary(0, 0), 99.0, None), None);

        let first = lines(summary(118, 42), 97.0, None);
        assert!(first.main.contains("42") && first.main.contains("118"));
        assert!(first.main.contains("36") && first.main.contains('6'));
        assert!(first.rollover.contains("21"));
        assert_eq!(first.comparison, None);

        // Misma lección: 18 más, con la misma precisión.
        let better = lines(summary(100, 38), 96.5, prev(true, 20, 97.0));
        assert!(matches!(&better.comparison, Some(Comparison::Better(t)) if t.contains("18")));

        // Más transiciones pero con más de un punto menos de precisión: no se felicita.
        let sloppy = lines(summary(100, 38), 94.0, prev(true, 20, 97.0));
        assert!(matches!(
            sloppy.comparison,
            Some(Comparison::LessAccurate(_))
        ));

        // Igual o peor: no se dice nada.
        assert_eq!(
            lines(summary(100, 20), 99.0, prev(true, 20, 97.0)).comparison,
            None
        );
        assert_eq!(
            lines(summary(100, 10), 99.0, prev(true, 20, 97.0)).comparison,
            None
        );

        // Otra lección: se comparan porcentajes (30 % contra 20 %).
        let other = lines(summary(200, 60), 97.0, prev(false, 20, 97.0));
        assert!(matches!(&other.comparison, Some(Comparison::Better(t)) if t.contains("10")));
    }

    #[test]
    fn transition_texts_are_translated() {
        for code in ["C", "es"] {
            let tr = Translator::new(code);
            let previous = |same_lesson, accuracy| {
                Some(PreviousTransitions {
                    same_lesson,
                    measured: 100,
                    fluent: 20,
                    accuracy,
                })
            };
            for (prev, accuracy) in [(previous(true, 97.0), 97.0), (previous(false, 99.0), 90.0)] {
                let lines = transition_lines(&tr, summary(100, 40), accuracy, prev).expect("hay");
                let comparison = match lines.comparison.expect("mejoró") {
                    Comparison::Better(t) | Comparison::LessAccurate(t) => t,
                };
                for t in [lines.main, comparison, lines.rollover] {
                    assert!(
                        !t.starts_with("sections-") && !t.contains("{$"),
                        "{code}: {t}"
                    );
                }
            }
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
            for m in Module::ALL.into_iter().chain([Module::Progression]) {
                assert!(tr.has(module_key(m)), "{code}: {m:?}");
            }
            let lesson = Slot::new(45, 10).expect("lección");
            for (speed, accuracy) in [(30.0, 99.0), (60.0, 93.0), (30.0, 85.0)] {
                let result = LessonResult {
                    slot: lesson,
                    target_wpm: lesson.target_wpm(),
                    time_limit: std::time::Duration::ZERO,
                    speed,
                    stars: klavaro_core::progression::stars(speed, accuracy, lesson.target_wpm()),
                };
                let text = goal_text(&tr, &result, accuracy).expect("meta");
                assert!(!text.starts_with("sections-"), "{code}: {text}");
                assert!(!text.contains("{$"), "{code}: {text}");
            }
            let notes = [
                Recorded::Logged(Some(Rejection::Invalid)),
                Recorded::Logged(Some(Rejection::TextNotSimilar)),
                Recorded::Ranked(1),
                Recorded::TooShort { chars: 10 },
                Recorded::Lesson {
                    record: true,
                    gained: 2,
                    next: NextStep::NeedsPassStars,
                    milestone: None,
                    exam: false,
                },
                Recorded::Lesson {
                    record: false,
                    gained: 0,
                    next: NextStep::NeedsSection,
                    milestone: None,
                    exam: true,
                },
                Recorded::Lesson {
                    record: false,
                    gained: 0,
                    next: NextStep::Completed,
                    milestone: Some(Milestone::Completed),
                    exam: false,
                },
                Recorded::Lesson {
                    record: false,
                    gained: 1,
                    next: NextStep::Open(Slot::new(25, 1).expect("lección")),
                    milestone: Some(Milestone::SectionPassed(20)),
                    exam: false,
                },
            ];
            for r in notes {
                let text = recorded_text(&tr, r).expect("nota");
                assert!(
                    !text.starts_with("ps-")
                        && !text.starts_with("sections-")
                        && !text.contains("{$"),
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
