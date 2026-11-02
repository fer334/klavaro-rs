// SPDX-License-Identifier: GPL-3.0-or-later
//! Estado de la pantalla del tutor, independiente de iced.
//!
//! La interfaz traduce cada evento de teclado a un [`Input`] y llama a
//! [`Tutor::handle`]; después dibuja [`Tutor::segments`]. Así toda la lógica de la pantalla
//! se prueba sin abrir una ventana.

use std::io;
use std::time::Duration;

use klavaro_core::Module;
use klavaro_core::generators::basic::FIXED_LESSONS;
use klavaro_core::goals::{Comment, CommentContext, GoalCheck, Goals};
use klavaro_core::keyboard::KeyboardLayout;
use klavaro_core::metrics::Stats;
use klavaro_core::session::{CharState, Feedback, Key, Session};

use crate::exercise::{self, ExerciseSpec};

/// Tecla especial, ya separada del framework de interfaz.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NamedKey {
    /// Enter.
    Enter,
    /// Backspace.
    Backspace,
    /// Escape.
    Escape,
    /// Barra espaciadora (cuando el sistema no entrega texto).
    Space,
}

/// Acción del usuario en la pantalla del tutor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Input {
    /// Carácter tecleado.
    Char(char),
    /// Enter.
    Enter,
    /// Backspace o Ctrl+H.
    Backspace,
    /// Ctrl+W.
    DeleteWord,
    /// Esc o Ctrl+R: nuevo ejercicio.
    Restart,
    /// Ctrl+A: colores alternativos.
    ToggleTheme,
    /// Ctrl+`+`: lección siguiente.
    LessonUp,
    /// Ctrl+`-`: lección anterior.
    LessonDown,
}

/// Traduce una tecla a una acción, con los atajos del original.
pub(crate) fn translate(named: Option<NamedKey>, text: Option<&str>, ctrl: bool) -> Option<Input> {
    if ctrl {
        let c = text
            .and_then(|t| t.chars().next())
            .map(|c| c.to_ascii_lowercase());
        return match c {
            Some('h') => Some(Input::Backspace),
            Some('w') => Some(Input::DeleteWord),
            Some('r') => Some(Input::Restart),
            Some('a') => Some(Input::ToggleTheme),
            Some('+' | '=') => Some(Input::LessonUp),
            Some('-') => Some(Input::LessonDown),
            _ => None,
        };
    }
    match named {
        Some(NamedKey::Enter) => return Some(Input::Enter),
        Some(NamedKey::Backspace) => return Some(Input::Backspace),
        Some(NamedKey::Escape) => return Some(Input::Restart),
        Some(NamedKey::Space) => return Some(Input::Char(' ')),
        None => {}
    }
    let mut chars = text?.chars().filter(|c| !c.is_control());
    match (chars.next(), chars.next()) {
        (Some(c), None) => Some(Input::Char(c)),
        _ => None,
    }
}

/// Estilo de un tramo de texto.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SegmentStyle {
    /// Por teclear.
    Untouched,
    /// Bien.
    Correct,
    /// Mal.
    Wrong,
    /// Reescrito tras corregir.
    Retouched,
    /// Posición del cursor.
    Cursor,
}

/// Tramo de texto con un mismo estilo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Segment {
    pub(crate) text: String,
    pub(crate) style: SegmentStyle,
}

/// Resultado mostrado al terminar.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Outcome {
    pub(crate) stats: Stats,
    pub(crate) check: GoalCheck,
    pub(crate) comment: Comment,
    /// En Básico: se aprobó y Enter pasa a la lección siguiente.
    pub(crate) advance: bool,
}

/// Efecto de una acción que la interfaz tiene que aplicar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct Effect {
    /// Hubo un error o una tecla no válida (el original pita).
    pub(crate) beep: bool,
    /// Alternar colores.
    pub(crate) toggle_theme: bool,
    /// El ejercicio acaba de terminar: hay que registrarlo.
    pub(crate) finished: bool,
}

/// Pantalla del tutor: ejercicio en curso y su resultado.
#[derive(Debug, Clone)]
pub(crate) struct Tutor {
    spec: ExerciseSpec,
    layout: KeyboardLayout,
    goals: Goals,
    session: Session,
    outcome: Option<Outcome>,
    seed: u64,
}

impl Tutor {
    /// Crea la pantalla con un primer ejercicio.
    pub(crate) fn new(
        spec: ExerciseSpec,
        layout: KeyboardLayout,
        goals: Goals,
        seed: u64,
    ) -> io::Result<Self> {
        let text = exercise::generate(&spec, &layout, seed)?;
        let session = Session::new(spec.module, &text);
        Ok(Self {
            spec,
            layout,
            goals,
            session,
            outcome: None,
            seed,
        })
    }

    /// Datos del ejercicio.
    pub(crate) fn spec(&self) -> &ExerciseSpec {
        &self.spec
    }

    /// Resultado, si terminó.
    pub(crate) fn outcome(&self) -> Option<&Outcome> {
        self.outcome.as_ref()
    }

    /// Texto del ejercicio.
    pub(crate) fn text(&self) -> String {
        self.session.target().iter().collect()
    }

    /// Avance: caracteres tecleados y total.
    pub(crate) fn progress(&self) -> (usize, usize) {
        (self.session.cursor(), self.session.target().len())
    }

    /// Genera un ejercicio nuevo con la misma configuración.
    pub(crate) fn restart(&mut self) -> io::Result<()> {
        self.seed = self
            .seed
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1);
        let text = exercise::generate(&self.spec, &self.layout, self.seed)?;
        self.session = Session::new(self.spec.module, &text);
        self.outcome = None;
        Ok(())
    }

    /// Cambia de lección (sólo Básico) y reinicia.
    pub(crate) fn set_lesson(&mut self, lesson: u16) -> io::Result<()> {
        self.spec.lesson = lesson.clamp(1, FIXED_LESSONS);
        self.restart()
    }

    /// Procesa una acción en el instante `at`.
    pub(crate) fn handle(&mut self, input: Input, at: Duration) -> io::Result<Effect> {
        let mut effect = Effect::default();
        match input {
            Input::ToggleTheme => effect.toggle_theme = true,
            Input::Restart => self.restart()?,
            Input::LessonUp if self.spec.module == Module::Basic => {
                self.set_lesson(self.spec.lesson.saturating_add(1))?;
            }
            Input::LessonDown if self.spec.module == Module::Basic => {
                self.set_lesson(self.spec.lesson.saturating_sub(1))?;
            }
            Input::LessonUp | Input::LessonDown => effect.beep = true,
            _ if self.outcome.is_some() => self.after_end(input, &mut effect)?,
            Input::Char(c) => self.press(Key::Char(c), at, &mut effect),
            Input::Enter => self.press(Key::Enter, at, &mut effect),
            Input::Backspace => self.press(Key::Backspace, at, &mut effect),
            Input::DeleteWord => self.press(Key::DeleteWord, at, &mut effect),
        }
        Ok(effect)
    }

    /// Teclas después de terminar: Enter sigue (y en Básico aprobado, avanza de lección);
    /// Backspace en Básico vuelve a la lección anterior, como el original.
    fn after_end(&mut self, input: Input, effect: &mut Effect) -> io::Result<()> {
        let advance = self.outcome.as_ref().is_some_and(|o| o.advance);
        match input {
            Input::Enter if advance => self.set_lesson(self.spec.lesson + 1),
            Input::Enter => self.restart(),
            Input::Backspace if self.spec.module == Module::Basic => {
                self.set_lesson(self.spec.lesson.saturating_sub(1))
            }
            _ => {
                effect.beep = true;
                Ok(())
            }
        }
    }

    fn press(&mut self, key: Key, at: Duration, effect: &mut Effect) {
        match self.session.press(key, at) {
            Feedback::Wrong { .. } | Feedback::Ignored => effect.beep = true,
            Feedback::Finished(stats) => {
                self.finish(stats);
                effect.finished = true;
            }
            Feedback::Correct { .. } | Feedback::Retouched { .. } | Feedback::Erased => {}
        }
    }

    fn finish(&mut self, stats: Stats) {
        let module = self.spec.module;
        let check = self.goals.evaluate(module, &stats);
        let passed_basic = module == Module::Basic && check.accuracy;
        let ctx = CommentContext {
            course_completed: passed_basic && self.spec.lesson >= FIXED_LESSONS,
        };
        let comment = self.goals.comment(module, &stats, ctx);
        let advance = passed_basic && self.spec.lesson < FIXED_LESSONS;
        self.outcome = Some(Outcome {
            stats,
            check,
            comment,
            advance,
        });
    }

    /// Texto del ejercicio en tramos con estilo. Los saltos de línea se muestran como `¶`.
    pub(crate) fn segments(&self) -> Vec<Segment> {
        let cursor = (self.outcome.is_none()).then(|| self.session.cursor());
        let mut out: Vec<Segment> = Vec::new();
        for (i, (&c, &state)) in self
            .session
            .target()
            .iter()
            .zip(self.session.states())
            .enumerate()
        {
            let style = if Some(i) == cursor {
                SegmentStyle::Cursor
            } else {
                match state {
                    CharState::Untouched => SegmentStyle::Untouched,
                    CharState::Correct => SegmentStyle::Correct,
                    CharState::Wrong => SegmentStyle::Wrong,
                    CharState::Retouched => SegmentStyle::Retouched,
                }
            };
            let shown = if c == '\n' {
                "¶\n".to_owned()
            } else {
                c.to_string()
            };
            match out.last_mut() {
                Some(last) if last.style == style && style != SegmentStyle::Cursor => {
                    last.text.push_str(&shown);
                }
                _ => out.push(Segment { text: shown, style }),
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assets;

    fn tutor(module: Module) -> Tutor {
        let layout = assets::keyboard("qwerty_es").expect("kbd");
        let spec = ExerciseSpec {
            module,
            lesson: 1,
            paragraphs: 1,
            language: "es".to_owned(),
        };
        Tutor::new(spec, layout, Goals::default(), 42).expect("tutor")
    }

    fn type_target(t: &mut Tutor, wrong_every: Option<usize>) {
        let target: Vec<char> = t.session.target().to_vec();
        for (i, c) in target.into_iter().enumerate() {
            let at = Duration::from_millis(200 * i as u64);
            let input = match (c, wrong_every) {
                (_, Some(n)) if i % n == 0 => Input::Char('#'),
                ('\n', _) => Input::Enter,
                (c, _) => Input::Char(c),
            };
            t.handle(input, at).expect("tecla");
        }
    }

    #[test]
    fn translates_keys_and_shortcuts() {
        assert_eq!(translate(None, Some("ñ"), false), Some(Input::Char('ñ')));
        assert_eq!(
            translate(Some(NamedKey::Enter), Some("\r"), false),
            Some(Input::Enter)
        );
        assert_eq!(
            translate(Some(NamedKey::Space), None, false),
            Some(Input::Char(' '))
        );
        assert_eq!(
            translate(Some(NamedKey::Escape), None, false),
            Some(Input::Restart)
        );
        assert_eq!(translate(None, Some("w"), true), Some(Input::DeleteWord));
        assert_eq!(translate(None, Some("R"), true), Some(Input::Restart));
        assert_eq!(translate(None, Some("x"), true), None);
        assert_eq!(translate(None, None, false), None);
    }

    #[test]
    fn perfect_basic_lesson_advances() {
        let mut t = tutor(Module::Basic);
        let text = t.text();
        let last = text.chars().last().expect("texto");
        let mut finished = 0;
        for (i, c) in text.chars().enumerate() {
            let input = if c == '\n' {
                Input::Enter
            } else {
                Input::Char(c)
            };
            let at = Duration::from_millis(200 * i as u64);
            finished += usize::from(t.handle(input, at).expect("tecla").finished);
        }
        assert_eq!(finished, 1, "termina una sola vez (última: {last:?})");
        let outcome = t.outcome().cloned().expect("terminó");
        assert!(outcome.advance);
        assert!((outcome.stats.accuracy - 100.0).abs() < 1e-9);
        t.handle(Input::Enter, Duration::ZERO).expect("enter");
        assert_eq!(t.spec().lesson, 2);
        assert!(t.outcome().is_none());
        assert_eq!(t.progress().0, 0);
    }

    #[test]
    fn failed_basic_lesson_repeats_and_backspace_goes_back() {
        let mut t = tutor(Module::Basic);
        t.set_lesson(5).expect("lección");
        type_target(&mut t, Some(3));
        let outcome = t.outcome().cloned().expect("terminó");
        assert!(!outcome.advance);
        assert_eq!(outcome.comment, Comment::TooManyErrors);
        t.handle(Input::Backspace, Duration::ZERO).expect("bs");
        assert_eq!(t.spec().lesson, 4);
    }

    #[test]
    fn wrong_keys_beep_and_shortcuts_work() {
        let mut t = tutor(Module::Velocity);
        let effect = t.handle(Input::Char('#'), Duration::ZERO).expect("tecla");
        assert!(effect.beep);
        assert!(
            t.handle(Input::ToggleTheme, Duration::ZERO)
                .expect("tema")
                .toggle_theme
        );
        assert!(
            t.handle(Input::LessonUp, Duration::ZERO)
                .expect("lección")
                .beep
        );
        t.handle(Input::Restart, Duration::ZERO).expect("reinicio");
        assert_eq!(t.progress().0, 0);
    }

    #[test]
    fn segments_cover_the_text_and_show_cursor() {
        let mut t = tutor(Module::Velocity);
        let first = t.session.target()[0];
        t.handle(Input::Char(first), Duration::ZERO).expect("tecla");
        t.handle(Input::Char('#'), Duration::from_millis(100))
            .expect("tecla");
        let segs = t.segments();
        assert_eq!(segs[0].style, SegmentStyle::Correct);
        assert_eq!(segs[1].style, SegmentStyle::Wrong);
        assert_eq!(segs[2].style, SegmentStyle::Cursor);
        let shown: String = segs.iter().map(|s| s.text.as_str()).collect();
        let expected: String = t
            .session
            .target()
            .iter()
            .map(|&c| {
                if c == '\n' {
                    "¶\n".to_owned()
                } else {
                    c.to_string()
                }
            })
            .collect();
        assert_eq!(shown, expected);
    }
}
