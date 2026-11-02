// SPDX-License-Identifier: GPL-3.0-or-later
//! Generación de ejercicios y textos provisorios en español (en la fase de i18n pasan a
//! Fluent). Compartido por la interfaz gráfica y la práctica en terminal.

use std::io;

use klavaro_core::Module;
use klavaro_core::generators::{LanguageRules, adaptability, basic, fluidity, velocity};
use klavaro_core::goals::Comment;
use klavaro_core::keyboard::KeyboardLayout;
use klavaro_core::rng::SplitMix64;

use crate::assets;

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

/// Genera el texto de un ejercicio con los datos de `assets/`.
pub(crate) fn generate(
    spec: &ExerciseSpec,
    layout: &KeyboardLayout,
    seed: u64,
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
        Module::Adaptability => Some(adaptability::generate(layout, &rules, None, &mut rng)),
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

/// Nombre del módulo para mostrar.
pub(crate) fn module_name(m: Module) -> &'static str {
    match m {
        Module::Basic => "Curso básico",
        Module::Adaptability => "Adaptabilidad",
        Module::Velocity => "Velocidad",
        Module::Fluidity => "Fluidez",
    }
}

/// Texto del comentario de fin de ejercicio.
pub(crate) fn comment_text(c: Comment) -> String {
    match c {
        Comment::TooManyErrors => ":-( Demasiados errores. Más despacio y con atención.".into(),
        Comment::BasicNextLesson => "¡Bien! Enter para pasar a la próxima lección.".into(),
        Comment::BasicCourseCompleted => "¡Completaste el curso básico!".into(),
        Comment::AdaptTryAgain { target } => format!("Intentá de nuevo, hasta {target:.0} %."),
        Comment::AdaptImproving { target } => format!("Vas bien. Llegá a {target:.0} %."),
        Comment::AdaptAlmostThere { target } => format!("Casi: la meta es {target:.0} %."),
        Comment::AdaptDone => "¡Muy bien! Seguí con el ejercicio de Velocidad.".into(),
        Comment::VeloBePatient => "Paciencia: practicá un poco todos los días.".into(),
        Comment::VeloKeepGoing { target } => format!("Buen ritmo. Próxima meta: {target:.0} PPM."),
        Comment::VeloDone {
            next_target: Some(t),
        } => {
            format!("¡Meta cumplida! Si querés más: {t:.0} PPM.")
        }
        Comment::VeloDone { next_target: None } => "¡Velocidad de profesional!".into(),
        Comment::FluidTooSlow { target } => {
            format!("Preciso pero lento: apuntá a {target:.0} PPM.")
        }
        Comment::FluidIrregular { target } => format!("Ritmo irregular: apuntá a {target:.0}."),
        Comment::FluidAlmostThere { target } => format!("Casi: fluidez {target:.0}."),
        Comment::FluidCongratulations => "¡Felicitaciones!".into(),
        Comment::FluidBeyondExpectations => "¡Superaste todas las expectativas!".into(),
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
            let text = generate(&spec, &layout, 3).expect("texto");
            assert!(!text.trim().is_empty(), "{module:?}");
        }
    }
}
