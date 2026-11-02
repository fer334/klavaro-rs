// SPDX-License-Identifier: GPL-3.0-or-later
//! Rastreo de errores y tiempos por tecla (`accuracy.c` del original).
//!
//! Se usa en Adaptabilidad, Velocidad y Fluidez (no en Básico) para:
//!
//! - los gráficos "errores por carácter" y "tiempo por tecla";
//! - decidir si conviene una sesión "especial" de Adaptabilidad;
//! - generar palabras que insisten en las teclas débiles.

use std::time::Duration;

use crate::keyboard::{KeyboardLayout, is_diacritic, to_lower};
use crate::rng::RandomSource;

/// Máximo de caracteres distintos que se rastrean.
pub const MAX_CHARS_EVALUATED: usize = 130;
/// Últimos tiempos guardados por carácter.
pub const MAX_TT_SAVED: usize = 30;
/// Aciertos seguidos a partir de los cuales un error viejo empieza a olvidarse.
pub const ERROR_INERTIA: u32 = 10;
/// Total de errores a partir del cual se sugiere una sesión especial.
pub const ERROR_LIMIT: u32 = 60;
/// Relación entre la 10.ª y la 40.ª tecla más lentas a partir de la cual se sugiere practicar.
pub const PROFI_LIMIT: f64 = 2.2;
/// Largo máximo de una palabra generada.
pub const MAX_WORD_LEN: usize = 9;

/// Errores de un carácter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ErrorEntry {
    /// Carácter.
    pub ch: char,
    /// Veces tecleado mal.
    pub wrong: u32,
    /// Aciertos desde el último olvido.
    pub correct: u32,
}

/// Tiempos de un carácter.
#[derive(Debug, Clone, PartialEq)]
pub struct TimeEntry {
    /// Carácter (en minúscula).
    pub ch: char,
    samples: Vec<f64>,
}

impl TimeEntry {
    /// Crea una entrada con tiempos iniciales (por ejemplo, el promedio importado).
    pub fn new(ch: char, samples: Vec<f64>) -> Self {
        let mut samples = samples;
        let overflow = samples.len().saturating_sub(MAX_TT_SAVED);
        samples.drain(..overflow);
        Self { ch, samples }
    }

    /// Tiempos guardados (segundos), del más viejo al más nuevo.
    pub fn samples(&self) -> &[f64] {
        &self.samples
    }

    /// Promedio en segundos de los tiempos válidos (entre 0 y 3 s).
    pub fn average(&self) -> f64 {
        let valid: Vec<f64> = self
            .samples
            .iter()
            .copied()
            .filter(|&t| t > 0.0 && t < 3.0)
            .collect();
        if valid.is_empty() {
            0.0
        } else {
            valid.iter().sum::<f64>() / valid.len() as f64
        }
    }
}

/// Acumulador de errores y tiempos por tecla para una distribución de teclado.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct KeyTracker {
    errors: Vec<ErrorEntry>,
    times: Vec<TimeEntry>,
}

impl KeyTracker {
    /// Rastreador vacío.
    pub fn new() -> Self {
        Self::default()
    }

    /// Reconstruye el rastreador desde datos guardados.
    pub fn from_parts(errors: Vec<ErrorEntry>, times: Vec<TimeEntry>) -> Self {
        let mut t = Self { errors, times };
        t.errors.truncate(MAX_CHARS_EVALUATED);
        t.times.truncate(MAX_CHARS_EVALUATED);
        t
    }

    /// Entradas de error, ordenadas por más errores primero (ver [`KeyTracker::sort`]).
    pub fn errors(&self) -> &[ErrorEntry] {
        &self.errors
    }

    /// Entradas de tiempo, ordenadas por más lentas primero (ver [`KeyTracker::sort`]).
    pub fn times(&self) -> &[TimeEntry] {
        &self.times
    }

    /// Borra los errores (al cumplir la meta de Adaptabilidad o a pedido del usuario).
    pub fn reset_errors(&mut self) {
        self.errors.clear();
    }

    /// Borra los tiempos.
    pub fn reset_times(&mut self) {
        self.times.clear();
    }

    /// Registra un acierto con su intervalo.
    pub fn correct(&mut self, layout: &KeyboardLayout, ch: char, interval: Option<Duration>) {
        if !is_tracked(layout, ch) {
            return;
        }
        if let Some(e) = self.errors.iter_mut().find(|e| e.ch == ch) {
            if e.correct > ERROR_INERTIA {
                e.wrong = e.wrong.saturating_sub(1);
                e.correct = 1;
            } else {
                e.correct += 1;
            }
        }
        let Some(dt) = interval.map(|d| d.as_secs_f64()) else {
            return;
        };
        if dt < 0.001 {
            return;
        }
        let lower = to_lower(ch);
        if let Some(t) = self.times.iter_mut().find(|t| t.ch == lower) {
            if t.samples.len() >= MAX_TT_SAVED {
                t.samples.remove(0);
            }
            t.samples.push(dt);
        } else if self.times.len() < MAX_CHARS_EVALUATED {
            self.times.push(TimeEntry::new(lower, vec![dt]));
        }
    }

    /// Registra un error.
    pub fn wrong(&mut self, layout: &KeyboardLayout, ch: char) {
        if !is_tracked(layout, ch) {
            return;
        }
        if let Some(e) = self.errors.iter_mut().find(|e| e.ch == ch) {
            e.wrong += 1;
            return;
        }
        let entry = ErrorEntry {
            ch,
            wrong: 1,
            correct: 1,
        };
        if self.errors.len() < MAX_CHARS_EVALUATED {
            self.errors.push(entry);
        } else if let Some(weakest) = self
            .errors
            .iter_mut()
            .min_by(|a, b| error_ratio(a).total_cmp(&error_ratio(b)))
        {
            *weakest = entry;
        }
    }

    /// Total de errores registrados.
    pub fn error_total(&self) -> u32 {
        self.errors.iter().map(|e| e.wrong).sum()
    }

    /// Ordena: errores de más a menos (desempate: menos aciertos primero) y tiempos de más
    /// lento a más rápido, descartando caracteres sin tiempos válidos.
    pub fn sort(&mut self) {
        self.errors
            .sort_by(|a, b| b.wrong.cmp(&a.wrong).then(a.correct.cmp(&b.correct)));
        self.times
            .sort_by(|a, b| b.average().total_cmp(&a.average()));
        while self.times.last().is_some_and(|t| t.average() < 0.001) {
            self.times.pop();
        }
    }

    /// Relación entre el promedio de la 10.ª y la 40.ª tecla más lentas, si hay 40 o más.
    pub fn proficiency_ratio(&self) -> Option<f64> {
        if self.times.len() < 40 {
            return None;
        }
        let p10 = self.times.get(9)?.average();
        let p40 = self.times.get(39)?.average();
        (p40 > 0.0).then(|| p10 / p40)
    }

    /// Por qué conviene una sesión especial de Adaptabilidad, si conviene.
    pub fn special_session(&self) -> Option<SpecialSession> {
        if self.error_total() >= ERROR_LIMIT {
            Some(SpecialSession::TooManyErrors)
        } else if self.proficiency_ratio().is_some_and(|r| r > PROFI_LIMIT) {
            Some(SpecialSession::SlowKeys)
        } else {
            None
        }
    }

    /// Genera una pseudopalabra con las teclas débiles. Requiere [`KeyTracker::sort`] previo.
    ///
    /// Devuelve `None` si no hay datos suficientes (menos de 10 caracteres con errores y
    /// menos de 5 con tiempos).
    pub fn weak_word(
        &self,
        layout: &KeyboardLayout,
        rng: &mut impl RandomSource,
    ) -> Option<String> {
        if self.errors.len() < 10 && self.times.len() < 5 {
            return None;
        }
        let use_errors = !self.errors.is_empty()
            && (self.times.len() < 5
                || self.special_session() == Some(SpecialSession::TooManyErrors));
        let len = 1 + rng.below(MAX_WORD_LEN);
        let mut word: Vec<char> = Vec::with_capacity(len);
        let mut guard = 0;
        while word.len() < len && guard < 10_000 {
            guard += 1;
            let candidate = if use_errors {
                self.pick_by_errors(rng)
            } else {
                self.pick_by_time(rng)
            };
            let Some(c) = candidate else { continue };
            if word.last() == Some(&c) {
                continue;
            }
            if is_diacritic(c) && word.last().is_some_and(|&p| is_diacritic(p)) {
                if let Some(&v) = rng.pick(&layout.vowels()) {
                    word.push(v);
                }
                continue;
            }
            word.push(c);
        }
        (!word.is_empty()).then(|| word.into_iter().collect())
    }

    fn pick_by_errors(&self, rng: &mut impl RandomSource) -> Option<char> {
        let top = self.errors.first()?.wrong.max(1);
        let e = rng.pick(&self.errors)?;
        let threshold = usize::try_from(top).unwrap_or(usize::MAX);
        let roll = u32::try_from(rng.below(threshold)).unwrap_or(u32::MAX);
        (roll < e.wrong).then_some(e.ch)
    }

    fn pick_by_time(&self, rng: &mut impl RandomSource) -> Option<char> {
        let slowest = self.times.first()?.average();
        if slowest <= 0.0 {
            return None;
        }
        let t = rng.pick(&self.times)?;
        let ratio = t.average() / slowest;
        let roll = rng.below(1_000) as f64 / 1_000.0;
        (roll < ratio).then_some(t.ch)
    }
}

/// Motivo para una sesión especial de Adaptabilidad.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpecialSession {
    /// Muchos errores acumulados.
    TooManyErrors,
    /// Hay teclas mucho más lentas que el resto.
    SlowKeys,
}

fn error_ratio(e: &ErrorEntry) -> f64 {
    f64::from(e.wrong) / f64::from(e.correct + e.wrong + 1)
}

fn is_tracked(layout: &KeyboardLayout, ch: char) -> bool {
    !matches!(ch, ' ' | '\n' | '\t' | '\u{8}') && layout.contains(ch)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keyboard::test_support::qwerty_us;
    use crate::rng::SplitMix64;

    #[allow(clippy::unnecessary_wraps)] // Comodidad: las llamadas reciben Option.
    fn ms(v: u64) -> Option<Duration> {
        Some(Duration::from_millis(v))
    }

    #[test]
    fn counts_errors_and_ignores_untracked_chars() {
        let k = qwerty_us();
        let mut t = KeyTracker::new();
        t.wrong(&k, 'a');
        t.wrong(&k, 'a');
        t.wrong(&k, ' ');
        t.wrong(&k, 'ñ');
        assert_eq!(t.error_total(), 2);
        assert_eq!(t.errors().len(), 1);
    }

    #[test]
    fn old_errors_fade_after_inertia() {
        let k = qwerty_us();
        let mut t = KeyTracker::new();
        t.wrong(&k, 'a');
        t.wrong(&k, 'a');
        for _ in 0..ERROR_INERTIA {
            t.correct(&k, 'a', None);
        }
        assert_eq!(t.error_total(), 2);
        t.correct(&k, 'a', None);
        assert_eq!(t.error_total(), 1);
    }

    #[test]
    fn sorts_slowest_first_and_keeps_last_samples() {
        let k = qwerty_us();
        let mut t = KeyTracker::new();
        for _ in 0..40 {
            t.correct(&k, 'a', ms(100));
            t.correct(&k, 'S', ms(400));
        }
        t.sort();
        assert_eq!(t.times()[0].ch, 's');
        assert_eq!(t.times()[0].samples.len(), MAX_TT_SAVED);
        assert!((t.times()[0].average() - 0.4).abs() < 1e-9);
    }

    #[test]
    fn special_session_on_many_errors() {
        let k = qwerty_us();
        let mut t = KeyTracker::new();
        assert_eq!(t.special_session(), None);
        for _ in 0..ERROR_LIMIT {
            t.wrong(&k, 'q');
        }
        assert_eq!(t.special_session(), Some(SpecialSession::TooManyErrors));
    }

    #[test]
    fn weak_words_use_weak_keys() {
        let k = qwerty_us();
        let mut t = KeyTracker::new();
        for c in "qwertyuiop".chars() {
            t.wrong(&k, c);
        }
        for _ in 0..20 {
            t.wrong(&k, 'z');
        }
        t.sort();
        let mut rng = SplitMix64::new(3);
        let mut z_count = 0;
        for _ in 0..50 {
            let w = t.weak_word(&k, &mut rng).unwrap_or_default();
            assert!(!w.is_empty() && w.chars().count() <= MAX_WORD_LEN);
            assert!(w.chars().all(|c| "qwertyuiopz".contains(c)), "{w}");
            z_count += w.chars().filter(|&c| c == 'z').count();
        }
        assert!(
            z_count > 20,
            "la tecla con más errores debería aparecer seguido"
        );
    }

    #[test]
    fn weak_word_needs_data() {
        let mut rng = SplitMix64::new(1);
        assert!(
            KeyTracker::new()
                .weak_word(&qwerty_us(), &mut rng)
                .is_none()
        );
    }
}
