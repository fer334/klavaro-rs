// SPDX-License-Identifier: GPL-3.0-or-later
//! Un ejercicio en curso: evalúa cada tecla contra el texto objetivo.
//!
//! Equivale a los estados `QUERY_START → QUERY_PROCESS_TOUCHS → QUERY_END` de `tutor.c`. El
//! estado `QUERY_INTRO` (texto de introducción y "presione una tecla") es responsabilidad de
//! la interfaz: una [`Session`] se crea cuando el ejercicio empieza.
//!
//! Hay dos modos de evaluación:
//!
//! - **Sin corrección** (Básico, Adaptabilidad, Velocidad): cada tecla avanza el cursor; los
//!   errores quedan marcados y Backspace se ignora.
//! - **Con corrección** (Fluidez): tras un error el cursor sigue avanzando, pero hay que volver
//!   con Backspace y reescribir; lo reescrito queda como "corregido" y cuenta como error.

use std::time::Duration;

use crate::Module;
use crate::metrics::{MAX_TOUCH_INTERVAL, Stats};

/// Tecla recibida de la interfaz, ya traducida.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    /// Un carácter imprimible (incluido el espacio).
    Char(char),
    /// Enter. Cierra una línea (`\n` en el texto objetivo) o vale como espacio.
    Enter,
    /// Backspace o Ctrl+H.
    Backspace,
    /// Ctrl+W o Ctrl+Backspace: borra hasta el primer error.
    DeleteWord,
}

/// Estado visual de cada carácter del texto objetivo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CharState {
    /// Todavía no se tecleó.
    Untouched,
    /// Tecleado bien a la primera.
    Correct,
    /// Tecleado mal.
    Wrong,
    /// Tecleado bien después de corregir (sólo Fluidez).
    Retouched,
}

/// Lo que pasó con una tecla, para que la interfaz pinte, suene y actualice el rastreo.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Feedback {
    /// Acierto. `interval` es el tiempo desde la tecla anterior (sin el primer toque).
    Correct {
        /// Carácter esperado.
        expected: char,
        /// Tiempo desde la tecla anterior, con tope de 4 s.
        interval: Option<Duration>,
    },
    /// Acierto al reescribir tras corregir (cuenta como error).
    Retouched {
        /// Carácter esperado.
        expected: char,
    },
    /// Error.
    Wrong {
        /// Carácter esperado.
        expected: char,
        /// Tecla recibida.
        typed: Key,
    },
    /// Se borró un carácter (sólo Fluidez).
    Erased,
    /// Tecla no válida en este momento (Backspace fuera de Fluidez, Backspace sin errores,
    /// teclas después de terminar). El original emite un pitido.
    Ignored,
    /// El ejercicio terminó con esta tecla.
    Finished(Stats),
}

/// Ejercicio en curso.
#[derive(Debug, Clone)]
pub struct Session {
    module: Module,
    target: Vec<char>,
    states: Vec<CharState>,
    pos: usize,
    touches: u32,
    errors: u32,
    /// Caracteres tecleados después del primer error sin corregir.
    retro: usize,
    /// Caracteres borrados pendientes de reescribir.
    correcting: usize,
    started_at: Option<Duration>,
    last_at: Option<Duration>,
    intervals: Vec<Duration>,
    result: Option<Stats>,
}

impl Session {
    /// Crea un ejercicio para un módulo con su texto objetivo. Un texto vacío termina con la
    /// primera tecla.
    pub fn new(module: Module, target: &str) -> Self {
        let target: Vec<char> = target.chars().collect();
        Self {
            module,
            states: vec![CharState::Untouched; target.len()],
            target,
            pos: 0,
            touches: 0,
            errors: 0,
            retro: 0,
            correcting: 0,
            started_at: None,
            last_at: None,
            intervals: Vec::new(),
            result: None,
        }
    }

    /// Módulo del ejercicio.
    pub fn module(&self) -> Module {
        self.module
    }

    /// Texto objetivo.
    pub fn target(&self) -> &[char] {
        &self.target
    }

    /// Estado de cada carácter, alineado con [`Session::target`].
    pub fn states(&self) -> &[CharState] {
        &self.states
    }

    /// Posición del cursor.
    pub fn cursor(&self) -> usize {
        self.pos
    }

    /// Resultado, si el ejercicio terminó.
    pub fn result(&self) -> Option<&Stats> {
        self.result.as_ref()
    }

    /// Si el ejercicio terminó.
    pub fn is_finished(&self) -> bool {
        self.result.is_some()
    }

    /// Procesa una tecla recibida en el instante `at` (medido desde cualquier origen fijo,
    /// por ejemplo la creación de la pantalla).
    pub fn press(&mut self, key: Key, at: Duration) -> Feedback {
        if self.result.is_some() {
            return Feedback::Ignored;
        }
        let feedback = if self.module.allows_correction() {
            self.eval_with_correction(key, at)
        } else {
            self.eval_forward(key, at)
        };
        if self.pos >= self.target.len() && self.retro == 0 {
            let start = self.started_at.unwrap_or(at);
            let stats = Stats::compute(
                self.touches,
                self.errors,
                at.saturating_sub(start),
                &self.intervals,
            );
            self.result = Some(stats);
            return Feedback::Finished(stats);
        }
        feedback
    }

    fn eval_forward(&mut self, key: Key, at: Duration) -> Feedback {
        let Some(&expected) = self.target.get(self.pos) else {
            return Feedback::Ignored;
        };
        if matches!(key, Key::Backspace | Key::DeleteWord) {
            return Feedback::Ignored;
        }
        self.touches += 1;
        let feedback = if matches_key(expected, key) {
            self.states[self.pos] = CharState::Correct;
            Feedback::Correct {
                expected,
                interval: self.record_interval(at),
            }
        } else {
            self.states[self.pos] = CharState::Wrong;
            self.errors += 1;
            self.mark_time(at);
            Feedback::Wrong {
                expected,
                typed: key,
            }
        };
        self.pos += 1;
        feedback
    }

    fn eval_with_correction(&mut self, key: Key, at: Duration) -> Feedback {
        match key {
            Key::Backspace => self.erase_one(),
            Key::DeleteWord => {
                if self.retro == 0 {
                    return Feedback::Ignored;
                }
                while self.retro > 0 {
                    self.erase_one();
                }
                Feedback::Erased
            }
            Key::Char(_) | Key::Enter => {
                let Some(&expected) = self.target.get(self.pos) else {
                    return Feedback::Ignored;
                };
                self.touches += 1;
                let feedback = if self.retro == 0 && matches_key(expected, key) {
                    if self.correcting > 0 {
                        self.states[self.pos] = CharState::Retouched;
                        self.errors += 1;
                        self.mark_time(at);
                        Feedback::Retouched { expected }
                    } else {
                        self.states[self.pos] = CharState::Correct;
                        Feedback::Correct {
                            expected,
                            interval: self.record_interval(at),
                        }
                    }
                } else {
                    self.states[self.pos] = CharState::Wrong;
                    self.retro += 1;
                    self.mark_time(at);
                    Feedback::Wrong {
                        expected,
                        typed: key,
                    }
                };
                self.correcting = self.correcting.saturating_sub(1);
                self.pos += 1;
                feedback
            }
        }
    }

    fn erase_one(&mut self) -> Feedback {
        if self.retro == 0 || self.pos == 0 {
            return Feedback::Ignored;
        }
        self.pos -= 1;
        self.states[self.pos] = CharState::Untouched;
        self.retro -= 1;
        self.correcting += 1;
        Feedback::Erased
    }

    /// Registra el instante y devuelve el intervalo desde la tecla anterior.
    fn record_interval(&mut self, at: Duration) -> Option<Duration> {
        let interval = self
            .last_at
            .map(|last| at.saturating_sub(last).min(MAX_TOUCH_INTERVAL));
        if let Some(i) = interval {
            self.intervals.push(i);
        }
        self.mark_time(at);
        interval
    }

    fn mark_time(&mut self, at: Duration) {
        if self.started_at.is_none() {
            self.started_at = Some(at);
        }
        self.last_at = Some(at);
    }
}

/// Si la tecla produce el carácter esperado. Enter vale para un salto de línea y, como en el
/// original, también para un espacio (los párrafos se cortan en espacios al mostrarse).
fn matches_key(expected: char, key: Key) -> bool {
    match key {
        Key::Char(c) => c == expected,
        Key::Enter => expected == '\n' || expected == ' ',
        Key::Backspace | Key::DeleteWord => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(v: u64) -> Duration {
        Duration::from_millis(v)
    }

    fn type_all(s: &mut Session, text: &str, start: u64, step: u64) -> Feedback {
        let mut last = Feedback::Ignored;
        for (i, c) in text.chars().enumerate() {
            let key = if c == '\n' { Key::Enter } else { Key::Char(c) };
            last = s.press(key, ms(start + step * i as u64));
        }
        last
    }

    #[test]
    fn perfect_run_finishes_with_full_accuracy() {
        let mut s = Session::new(Module::Velocity, "hola mundo");
        let last = type_all(&mut s, "hola mundo", 0, 200);
        let Feedback::Finished(stats) = last else {
            panic!("no terminó: {last:?}")
        };
        assert_eq!(stats.touches, 10);
        assert_eq!(stats.errors, 0);
        assert!((stats.accuracy - 100.0).abs() < 1e-9);
        // 10 caracteres en 1,8 s → 12 × 10 / 1,8 ≈ 66,7 PPM.
        assert!((stats.wpm - 12.0 * 10.0 / 1.8).abs() < 1e-6);
        assert!(s.states().iter().all(|&st| st == CharState::Correct));
        assert_eq!(s.press(Key::Char('x'), ms(5_000)), Feedback::Ignored);
    }

    #[test]
    fn errors_advance_without_correction() {
        let mut s = Session::new(Module::Basic, "asdf");
        assert!(matches!(
            s.press(Key::Char('a'), ms(0)),
            Feedback::Correct { .. }
        ));
        assert!(matches!(
            s.press(Key::Char('x'), ms(100)),
            Feedback::Wrong { expected: 's', .. }
        ));
        assert_eq!(s.press(Key::Backspace, ms(150)), Feedback::Ignored);
        s.press(Key::Char('d'), ms(200));
        let Feedback::Finished(stats) = s.press(Key::Char('f'), ms(300)) else {
            panic!("debía terminar")
        };
        assert_eq!(stats.errors, 1);
        assert!((stats.accuracy - 75.0).abs() < 1e-9);
        assert_eq!(s.states()[1], CharState::Wrong);
    }

    #[test]
    fn enter_closes_lines_and_counts_as_space() {
        let mut s = Session::new(Module::Basic, "ab\ncd ef");
        let last = type_all(&mut s, "ab\ncd", 0, 100);
        assert!(!matches!(last, Feedback::Finished(_)));
        assert!(matches!(
            s.press(Key::Enter, ms(600)),
            Feedback::Correct { expected: ' ', .. }
        ));
        s.press(Key::Char('e'), ms(700));
        assert!(matches!(
            s.press(Key::Char('f'), ms(800)),
            Feedback::Finished(_)
        ));
    }

    #[test]
    fn fluidity_requires_correction_before_finishing() {
        let mut s = Session::new(Module::Fluidity, "abc");
        s.press(Key::Char('a'), ms(0));
        assert!(matches!(
            s.press(Key::Char('x'), ms(100)),
            Feedback::Wrong { .. }
        ));
        // Aun acertando, después de un error no corregido sigue siendo error.
        assert!(matches!(
            s.press(Key::Char('c'), ms(200)),
            Feedback::Wrong { .. }
        ));
        assert!(
            !s.is_finished(),
            "no puede terminar con errores sin corregir"
        );
        assert_eq!(s.press(Key::Backspace, ms(300)), Feedback::Erased);
        assert_eq!(s.press(Key::Backspace, ms(400)), Feedback::Erased);
        assert_eq!(s.press(Key::Backspace, ms(450)), Feedback::Ignored);
        assert!(matches!(
            s.press(Key::Char('b'), ms(500)),
            Feedback::Retouched { .. }
        ));
        let Feedback::Finished(stats) = s.press(Key::Char('c'), ms(600)) else {
            panic!("debía terminar")
        };
        // 'b' y 'c' reescritos tras corregir cuentan como errores.
        assert_eq!(stats.errors, 2);
        assert_eq!(stats.touches, 5);
        assert_eq!(
            s.states(),
            &[
                CharState::Correct,
                CharState::Retouched,
                CharState::Retouched
            ]
        );
    }

    #[test]
    fn delete_word_clears_all_pending_errors() {
        let mut s = Session::new(Module::Fluidity, "abcd");
        s.press(Key::Char('a'), ms(0));
        s.press(Key::Char('x'), ms(100));
        s.press(Key::Char('y'), ms(200));
        assert_eq!(s.cursor(), 3);
        assert_eq!(s.press(Key::DeleteWord, ms(300)), Feedback::Erased);
        assert_eq!(s.cursor(), 1);
        assert_eq!(s.press(Key::DeleteWord, ms(350)), Feedback::Ignored);
    }

    #[test]
    fn intervals_are_capped() {
        let mut s = Session::new(Module::Velocity, "ab");
        s.press(Key::Char('a'), ms(0));
        let fb = s.press(Key::Char('b'), ms(10_000));
        assert!(matches!(fb, Feedback::Finished(_)));
        assert_eq!(s.intervals, vec![MAX_TOUCH_INTERVAL]);
    }

    #[test]
    fn empty_target_finishes_immediately() {
        let mut s = Session::new(Module::Velocity, "");
        assert!(matches!(
            s.press(Key::Char('a'), ms(0)),
            Feedback::Finished(_)
        ));
    }
}
