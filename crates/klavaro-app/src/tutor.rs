// SPDX-License-Identifier: GPL-3.0-or-later
//! Estado de la pantalla del tutor, independiente de iced.
//!
//! La interfaz traduce cada evento de teclado a un [`Input`] y llama a
//! [`Tutor::handle`]; después dibuja [`Tutor::segments`]. Así toda la lógica de la pantalla
//! se prueba sin abrir una ventana.

use std::collections::BTreeMap;
use std::io;
use std::time::Duration;

use klavaro_core::Module;
use klavaro_core::generators::basic::FIXED_LESSONS;
use klavaro_core::goals::{Comment, CommentContext, GoalCheck, Goals};
use klavaro_core::keyboard::KeyboardLayout;
use klavaro_core::metrics::Stats;
use klavaro_core::progression::LessonResult;
use klavaro_core::session::{CharState, Feedback, Key, Session};
use klavaro_core::tracker::{KeyTracker, SpecialSession};
use klavaro_data::catalog::CatalogEntry;

use crate::assets;
use crate::exercise::{self, ExerciseSpec};
use crate::speech::{self, Say};

/// Flecha del teclado, para moverse por las grillas de la Progresión.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Direction {
    Up,
    Down,
    Left,
    Right,
}

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
    /// Una flecha.
    Arrow(Direction),
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
    /// Una flecha: mueve la selección en las grillas. En un ejercicio no hace nada.
    Move(Direction),
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
        Some(NamedKey::Arrow(d)) => return Some(Input::Move(d)),
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
    /// En la Progresión: estrellas, meta y tiempo de la lección.
    pub(crate) lesson: Option<LessonResult>,
    /// En la Progresión: de dónde salió el texto.
    pub(crate) source: Option<CatalogEntry>,
}

/// Efecto de una acción que la interfaz tiene que aplicar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[allow(clippy::struct_excessive_bools)] // Son efectos independientes entre sí.
pub(crate) struct Effect {
    /// Hubo un error o una tecla no válida (el original pita).
    pub(crate) beep: bool,
    /// Alternar colores.
    pub(crate) toggle_theme: bool,
    /// El ejercicio acaba de terminar: hay que registrarlo.
    pub(crate) finished: bool,
    /// Progresión: se pidió seguir después de terminar la lección.
    pub(crate) next_lesson: bool,
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
    /// Errores y tiempos por tecla del teclado actual. Como en el original, no se rastrea el
    /// curso básico.
    tracker: KeyTracker,
    /// Adaptabilidad con palabras que insisten en las teclas débiles.
    special: bool,
    /// Cuántos ejercicios se generaron (para saber si cambió el texto).
    generation: u32,
    /// En la Progresión: de dónde salió el texto.
    source: Option<CatalogEntry>,
    /// Errores de este ejercicio por tecla.
    mistakes: BTreeMap<char, u32>,
    /// Hay un error sin corregir: lo que se teclee hasta borrarlo no cuenta como otro error.
    typo_open: bool,
}

impl Tutor {
    /// Crea la pantalla con un primer ejercicio.
    pub(crate) fn new(
        spec: ExerciseSpec,
        layout: KeyboardLayout,
        goals: Goals,
        seed: u64,
        mut tracker: KeyTracker,
    ) -> io::Result<Self> {
        tracker.sort();
        let text = exercise::generate(&spec, &layout, seed, None)?;
        let session = Session::new(spec.module, &text);
        let source = spec
            .slot
            .map(|slot| assets::lesson(&spec.language, slot))
            .transpose()?;
        Ok(Self {
            spec,
            layout,
            goals,
            session,
            outcome: None,
            seed,
            tracker,
            special: false,
            generation: 0,
            source,
            mistakes: BTreeMap::new(),
            typo_open: false,
        })
    }

    /// Rastreo por tecla.
    pub(crate) fn tracker(&self) -> &KeyTracker {
        &self.tracker
    }

    /// Devuelve el rastreo al salir de la pantalla.
    pub(crate) fn into_tracker(self) -> KeyTracker {
        self.tracker
    }

    /// Por qué convendría una sesión especial de Adaptabilidad (el original muestra entonces
    /// un botón para activarla).
    pub(crate) fn special_suggested(&self) -> Option<SpecialSession> {
        (self.spec.module == Module::Adaptability)
            .then(|| self.tracker.special_session())
            .flatten()
    }

    /// Si la sesión especial está activa.
    pub(crate) fn special(&self) -> bool {
        self.special
    }

    /// Activa o desactiva la sesión especial y genera un ejercicio nuevo.
    pub(crate) fn toggle_special(&mut self) -> io::Result<()> {
        self.special = !self.special && self.spec.module == Module::Adaptability;
        self.restart()
    }

    /// Distribución de teclado.
    pub(crate) fn layout(&self) -> &KeyboardLayout {
        &self.layout
    }

    /// Carácter que toca teclear (ninguno si terminó).
    pub(crate) fn next_char(&self) -> Option<char> {
        if self.outcome.is_some() {
            return None;
        }
        self.session.target().get(self.session.cursor()).copied()
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

    /// Qué dictar ahora (nada si terminó) y en qué posición: el número de ejercicio y el
    /// cursor, para no repetir el mismo dictado.
    pub(crate) fn dictation(&self) -> Option<((u32, usize), Say)> {
        if self.outcome.is_some() {
            return None;
        }
        let cursor = self.session.cursor();
        speech::utterance(self.spec.module, self.session.target(), cursor)
            .map(|say| ((self.generation, cursor), say))
    }

    /// Avance: caracteres tecleados y total.
    pub(crate) fn progress(&self) -> (usize, usize) {
        (self.session.cursor(), self.session.target().len())
    }

    /// Caracteres del texto que este teclado no tiene, sin repetir. No impide hacer el ejercicio.
    pub(crate) fn missing_keys(&self) -> Vec<char> {
        let mut missing: Vec<char> = self
            .session
            .target()
            .iter()
            .copied()
            .filter(|&c| !c.is_whitespace() && !self.layout.contains(c))
            .collect();
        missing.sort_unstable();
        missing.dedup();
        missing
    }

    /// Las teclas en las que más se erró en este ejercicio, de más a menos.
    pub(crate) fn weak_keys(&self) -> Vec<char> {
        let mut keys: Vec<(char, u32)> = self.mistakes.iter().map(|(&k, &n)| (k, n)).collect();
        keys.sort_by_key(|&(key, count)| (std::cmp::Reverse(count), key));
        keys.into_iter().take(5).map(|(key, _)| key).collect()
    }

    /// Si conviene una sesión especial de Adaptabilidad para las teclas débiles.
    pub(crate) fn practice_suggested(&self) -> bool {
        self.tracker.special_session().is_some()
    }

    /// Tiempo desde la primera tecla hasta la última.
    pub(crate) fn elapsed(&self) -> Duration {
        self.session.elapsed()
    }

    /// Genera un ejercicio nuevo con la misma configuración.
    pub(crate) fn restart(&mut self) -> io::Result<()> {
        self.generation = self.generation.wrapping_add(1);
        self.seed = self
            .seed
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1);
        // El original ordena el rastreo al empezar cada ejercicio.
        self.tracker.sort();
        let weak =
            (self.special && self.spec.module == Module::Adaptability).then_some(&self.tracker);
        let text = exercise::generate(&self.spec, &self.layout, self.seed, weak)?;
        self.session = Session::new(self.spec.module, &text);
        self.outcome = None;
        self.mistakes.clear();
        self.typo_open = false;
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
            Input::Move(_) => {}
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
            Input::Enter if self.spec.module == Module::Progression => {
                effect.next_lesson = true;
                Ok(())
            }
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
        let track = self.spec.module != Module::Basic;
        match self.session.press(key, at) {
            Feedback::Correct { expected, interval } if track => {
                self.typo_open = false;
                self.tracker.correct(&self.layout, expected, interval);
            }
            Feedback::Retouched { expected } if track => {
                self.typo_open = false;
                self.tracker.correct(&self.layout, expected, None);
            }
            Feedback::Wrong { expected, .. } => {
                if track {
                    self.tracker.wrong(&self.layout, expected);
                }
                if !self.typo_open {
                    *self.mistakes.entry(expected).or_insert(0) += 1;
                }
                self.typo_open = true;
                effect.beep = true;
            }
            Feedback::Ignored => effect.beep = true,
            Feedback::Finished(stats) => {
                self.finish(stats);
                effect.finished = true;
            }
            Feedback::Erased => self.typo_open = false,
            Feedback::Correct { .. } | Feedback::Retouched { .. } => {}
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
        // Meta de Adaptabilidad cumplida: el original borra los errores acumulados.
        if matches!(comment, Comment::AdaptDone { .. }) {
            self.tracker.reset_errors();
            self.special = false;
        }
        let advance = passed_basic && self.spec.lesson < FIXED_LESSONS;
        let lesson = self
            .spec
            .slot
            .map(|slot| LessonResult::new(slot, self.session.target().len(), &stats));
        self.outcome = Some(Outcome {
            stats,
            check,
            comment,
            advance,
            lesson,
            source: self.source.clone(),
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

    fn tutor(module: Module) -> Tutor {
        let layout = assets::keyboard("qwerty_es").expect("kbd");
        let spec = ExerciseSpec {
            module,
            lesson: 1,
            paragraphs: 1,
            language: "es".to_owned(),
            slot: None,
        };
        Tutor::new(spec, layout, Goals::default(), 42, KeyTracker::new()).expect("tutor")
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

    fn lesson_tutor(section: u16, lesson: u16) -> Tutor {
        let layout = assets::keyboard("qwerty_es").expect("kbd");
        let spec = ExerciseSpec {
            module: Module::Progression,
            lesson: 1,
            paragraphs: 1,
            language: crate::sections::LESSONS_LANGUAGE.to_owned(),
            slot: klavaro_core::progression::Slot::new(section, lesson),
        };
        Tutor::new(spec, layout, Goals::default(), 42, KeyTracker::new()).expect("tutor")
    }

    /// Escribe todo el texto bien, con `ms` por tecla. Devuelve las veces que terminó.
    fn type_lesson(t: &mut Tutor, ms: u64) -> usize {
        let mut finished = 0;
        for (i, c) in t.text().chars().enumerate() {
            let at = Duration::from_millis(ms * i as u64);
            finished += usize::from(t.handle(Input::Char(c), at).expect("tecla").finished);
        }
        finished
    }

    #[test]
    fn a_lesson_on_time_earns_five_stars() {
        let mut t = lesson_tutor(20, 30);
        // A 20 PPM toca una tecla cada 600 ms.
        assert_eq!(type_lesson(&mut t, 500), 1);
        let result = t.outcome().and_then(|o| o.lesson).expect("resultado");
        assert_eq!(result.stars, 5);
        assert!((result.target_wpm - 20.0).abs() < 1e-9);
        assert!(t.elapsed() > Duration::ZERO);
    }

    #[test]
    fn a_slow_lesson_earns_fewer_stars() {
        let mut t = lesson_tutor(20, 30);
        type_lesson(&mut t, 800);
        let result = t.outcome().and_then(|o| o.lesson).expect("resultado");
        assert_eq!(result.stars, 2);
    }

    #[test]
    fn a_lesson_needs_corrections_to_finish() {
        let mut t = lesson_tutor(20, 1);
        let first = t.text().chars().next().expect("texto");
        t.handle(Input::Char('#'), Duration::ZERO).expect("tecla");
        t.handle(Input::Backspace, Duration::from_millis(10))
            .expect("corrige");
        assert_eq!(t.progress().0, 0);
        t.handle(Input::Char(first), Duration::from_millis(20))
            .expect("tecla");
        assert_eq!(t.progress().0, 1);
    }

    #[test]
    fn after_a_lesson_enter_asks_to_continue_and_esc_repeats_the_text() {
        let mut t = lesson_tutor(20, 1);
        let text = t.text();
        type_lesson(&mut t, 500);
        let effect = t.handle(Input::Enter, Duration::ZERO).expect("enter");
        assert!(effect.next_lesson);
        assert!(t.outcome().is_some(), "sigue en el resultado");
        t.handle(Input::Restart, Duration::ZERO).expect("esc");
        assert!(t.outcome().is_none());
        assert_eq!(t.text(), text);
    }

    #[test]
    fn a_lesson_keeps_the_source_of_its_text() {
        let mut t = lesson_tutor(20, 1);
        assert_eq!(t.missing_keys(), Vec::<char>::new());
        type_lesson(&mut t, 500);
        let source = t.outcome().and_then(|o| o.source.clone()).expect("fuente");
        assert_eq!(source.text, t.text());
        assert!(source.url().contains("oldid="));
    }

    #[test]
    fn warns_about_keys_the_layout_does_not_have() {
        let layout = assets::keyboard("qwerty_us").expect("kbd");
        let foreign = assets::keyboards()
            .into_iter()
            .find(|k| assets::keyboard(k).is_ok_and(|l| !l.contains('a')))
            .expect("un teclado sin alfabeto latino");
        let spec = |slot| ExerciseSpec {
            module: Module::Progression,
            lesson: 1,
            paragraphs: 1,
            language: crate::sections::LESSONS_LANGUAGE.to_owned(),
            slot,
        };
        let slot = klavaro_core::progression::Slot::new(20, 1);
        let t =
            Tutor::new(spec(slot), layout, Goals::default(), 1, KeyTracker::new()).expect("tutor");
        assert_eq!(t.missing_keys(), Vec::<char>::new());
        let layout = assets::keyboard(&foreign).expect("kbd");
        let t =
            Tutor::new(spec(slot), layout, Goals::default(), 1, KeyTracker::new()).expect("tutor");
        assert!(!t.missing_keys().is_empty(), "{foreign}");
    }

    #[test]
    fn counts_one_mistake_per_typo_and_lists_the_weakest_keys() {
        let mut t = lesson_tutor(20, 1);
        let text: Vec<char> = t.text().chars().collect();
        let at = Duration::ZERO;
        t.handle(Input::Char('#'), at).expect("error");
        t.handle(Input::Char('#'), at).expect("sigue mal");
        assert_eq!(t.weak_keys(), vec![text[0]], "un solo error, no dos");
        t.handle(Input::Backspace, at).expect("borra");
        t.handle(Input::Backspace, at).expect("borra");
        t.handle(Input::Char(text[0]), at).expect("corrige");
        t.handle(Input::Char('#'), at).expect("otro error");
        t.handle(Input::Backspace, at).expect("borra");
        t.handle(Input::Char('#'), at).expect("repite el error");
        assert_eq!(t.weak_keys()[0], text[1]);
        t.restart().expect("reintento");
        assert_eq!(t.weak_keys(), Vec::<char>::new());
    }

    #[test]
    fn arrows_do_nothing_during_an_exercise() {
        let mut t = lesson_tutor(20, 1);
        let effect = t
            .handle(Input::Move(Direction::Up), Duration::ZERO)
            .expect("flecha");
        assert_eq!(effect, Effect::default());
        assert_eq!(t.progress().0, 0);
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
        assert_eq!(
            translate(Some(NamedKey::Arrow(Direction::Left)), None, false),
            Some(Input::Move(Direction::Left))
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

    #[test]
    fn tracks_keys_except_in_the_basic_course() {
        let mut basic = tutor(Module::Basic);
        type_target(&mut basic, Some(4));
        assert_eq!(basic.tracker(), &KeyTracker::new());

        let mut velo = tutor(Module::Velocity);
        type_target(&mut velo, Some(4));
        assert!(velo.tracker().error_total() > 0);
        assert!(velo.tracker().times().len() > 1);
    }

    #[test]
    fn special_session_uses_weak_keys() {
        let layout = assets::keyboard("qwerty_es").expect("kbd");
        let mut tracker = KeyTracker::new();
        for _ in 0..80 {
            tracker.wrong(&layout, 'z');
        }
        for c in "qwertyuiop".chars() {
            tracker.wrong(&layout, c);
        }
        let spec = ExerciseSpec {
            module: Module::Adaptability,
            lesson: 1,
            paragraphs: 1,
            language: "es".to_owned(),
            slot: None,
        };
        let mut t = Tutor::new(spec, layout, Goals::default(), 7, tracker).expect("tutor");
        assert_eq!(t.special_suggested(), Some(SpecialSession::TooManyErrors));
        let plain = t.text().matches('z').count();
        t.toggle_special().expect("especial");
        assert!(t.special());
        assert!(t.text().matches('z').count() > plain);
        assert!(tutor(Module::Velocity).special_suggested().is_none());
    }

    #[test]
    fn dictation_follows_the_cursor_and_the_exercise() {
        let mut t = tutor(Module::Adaptability);
        let first = t.text().chars().next().expect("texto");
        let (at, say) = t.dictation().expect("dictado");
        assert_eq!(at, (0, 0));
        assert_eq!(say, Say::Char(first));
        t.handle(Input::Char(first), Duration::ZERO).expect("tecla");
        assert_eq!(t.dictation().map(|d| d.0), Some((0, 1)));
        t.restart().expect("nuevo");
        assert_eq!(t.dictation().map(|d| d.0), Some((1, 0)));
        assert!(tutor(Module::Fluidity).dictation().is_none());
    }
}
