// SPDX-License-Identifier: GPL-3.0-or-later
//! Transiciones fluidas: pasos de una letra a la siguiente que son rápidos, correctos y
//! coordinados.
//!
//! Es una recompensa de la Progresión (ver `PLAN_TRANSICIONES.md`). No cambia la evaluación,
//! el puntaje ni las estrellas: sólo lee los estados y los tiempos de cada letra al final.
//!
//! Una transición une la letra `i − 1` con la `i` y es **fluida** cuando:
//!
//! - las dos letras se escribieron bien a la primera ([`CharState::Correct`]);
//! - las dos tienen tiempos propios (no vienen del método de entrada ni de la autorrepetición);
//! - el IKI (tiempo entre las dos bajadas) no supera el umbral de su tipo ([`iki_limit`]);
//! - la tecla anterior no se mantuvo apretada de más ([`hold_limit`]).
//!
//! El rollover (bajar la tecla siguiente antes de soltar la anterior) **no** es condición: se
//! cuenta aparte, como dato.

use std::time::Duration;

use crate::keyboard::Finger;
use crate::session::CharState;

/// Fracción del IKI de la meta que tiene que alcanzar una transición para ser fluida.
pub const FAST_FACTOR: f64 = 0.8;
/// Factor del umbral cuando las letras son de manos distintas (o una es del pulgar).
pub const OTHER_HAND_FACTOR: f64 = 0.9;
/// Factor del umbral cuando son de la misma mano y de dedos distintos.
pub const SAME_HAND_FACTOR: f64 = 1.0;
/// Factor del umbral cuando las dos letras son del mismo dedo (incluida la misma tecla).
pub const SAME_FINGER_FACTOR: f64 = 1.25;
/// Factor del umbral cuando no se sabe qué dedos se usan.
pub const UNKNOWN_FACTOR: f64 = 1.0;
/// Una tecla retenida más de este múltiplo de la mediana del ejercicio es retención
/// artificial.
pub const HOLD_FACTOR: f64 = 1.5;
/// Retención que nunca se considera artificial, aunque la mediana sea baja.
pub const HOLD_FLOOR: Duration = Duration::from_millis(180);
/// Retenciones necesarias para confiar en la mediana. Con menos, el límite es sólo
/// [`HOLD_FLOOR`] multiplicado por [`HOLD_FACTOR`].
pub const HOLD_MIN_SAMPLES: usize = 10;

/// Una letra del texto con lo que se sabe de cómo se escribió.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Letter {
    /// Estado final de la letra.
    pub state: CharState,
    /// Cuándo bajó la tecla que la escribió (la última vez). `None` si no hay tiempos propios.
    pub down: Option<Duration>,
    /// Cuándo se soltó esa tecla, si se sabe.
    pub up: Option<Duration>,
    /// Dedo que la escribe, si se sabe.
    pub finger: Option<Finger>,
}

impl Letter {
    /// Cuánto se mantuvo apretada la tecla, si se sabe.
    pub fn hold(&self) -> Option<Duration> {
        Some(self.up?.saturating_sub(self.down?))
    }
}

/// Tipo de transición según los dedos que la escriben.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransitionKind {
    /// Manos distintas, o una de las letras es del pulgar.
    OtherHand,
    /// Misma mano, dedos distintos.
    SameHand,
    /// Mismo dedo.
    SameFinger,
    /// Falta el dedo de alguna de las dos.
    Unknown,
}

impl TransitionKind {
    /// Clasifica una transición por los dedos de sus dos letras.
    pub fn of(from: Option<Finger>, to: Option<Finger>) -> Self {
        let (Some(a), Some(b)) = (from, to) else {
            return Self::Unknown;
        };
        if a == b {
            return Self::SameFinger;
        }
        match (hand(a), hand(b)) {
            (Some(x), Some(y)) if x == y => Self::SameHand,
            _ => Self::OtherHand,
        }
    }

    /// Nombre estable del tipo, para guardarlo.
    pub const fn code(self) -> &'static str {
        match self {
            Self::OtherHand => "other_hand",
            Self::SameHand => "same_hand",
            Self::SameFinger => "same_finger",
            Self::Unknown => "unknown",
        }
    }

    /// Tipo guardado con [`TransitionKind::code`]. Un nombre desconocido es [`Self::Unknown`].
    pub fn from_code(code: &str) -> Self {
        match code {
            "other_hand" => Self::OtherHand,
            "same_hand" => Self::SameHand,
            "same_finger" => Self::SameFinger,
            _ => Self::Unknown,
        }
    }

    /// Factor que ajusta el umbral de IKI a lo que permite la mano.
    pub fn factor(self) -> f64 {
        match self {
            Self::OtherHand => OTHER_HAND_FACTOR,
            Self::SameHand => SAME_HAND_FACTOR,
            Self::SameFinger => SAME_FINGER_FACTOR,
            Self::Unknown => UNKNOWN_FACTOR,
        }
    }
}

/// Mano de un dedo: `Some(false)` izquierda, `Some(true)` derecha, `None` los pulgares.
fn hand(f: Finger) -> Option<bool> {
    match f {
        Finger::LeftPinky | Finger::LeftRing | Finger::LeftMiddle | Finger::LeftIndex => {
            Some(false)
        }
        Finger::Thumbs => None,
        Finger::RightIndex | Finger::RightMiddle | Finger::RightRing | Finger::RightPinky => {
            Some(true)
        }
    }
}

/// IKI promedio que pide una meta de velocidad: `60 s / (meta × 5 caracteres)`.
pub fn target_iki(target_wpm: f64) -> Duration {
    if target_wpm <= 0.0 || !target_wpm.is_finite() {
        return Duration::MAX;
    }
    Duration::from_secs_f64(12.0 / target_wpm)
}

/// IKI máximo de una transición fluida de este tipo con esta meta.
pub fn iki_limit(target_wpm: f64, kind: TransitionKind) -> Duration {
    let base = target_iki(target_wpm);
    if base == Duration::MAX {
        return base;
    }
    base.mul_f64(FAST_FACTOR * kind.factor())
}

/// Retención máxima antes de considerarla artificial, dada la mediana del ejercicio.
pub fn hold_limit(median: Option<Duration>) -> Duration {
    median
        .unwrap_or(HOLD_FLOOR)
        .max(HOLD_FLOOR)
        .mul_f64(HOLD_FACTOR)
}

/// Mediana de las retenciones conocidas, si hay al menos [`HOLD_MIN_SAMPLES`].
pub fn median_hold(letters: &[Letter]) -> Option<Duration> {
    let mut holds: Vec<Duration> = letters.iter().filter_map(Letter::hold).collect();
    if holds.len() < HOLD_MIN_SAMPLES {
        return None;
    }
    holds.sort_unstable();
    let mid = holds.len() / 2;
    Some(if holds.len().is_multiple_of(2) {
        (holds[mid - 1] + holds[mid]) / 2
    } else {
        holds[mid]
    })
}

/// Una transición medida: de la letra `at − 1` a la `at`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transition {
    /// Índice de la letra que cierra la transición.
    pub at: usize,
    /// Tiempo entre las dos bajadas.
    pub iki: Duration,
    /// La letra anterior se soltó después de bajar ésta.
    pub rollover: bool,
    /// La letra anterior se retuvo más de lo normal en este ejercicio.
    pub held_too_long: bool,
    /// Tipo según los dedos.
    pub kind: TransitionKind,
    /// Cumple todas las condiciones.
    pub fluent: bool,
}

/// Totales de un ejercicio.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TransitionSummary {
    /// Transiciones con tiempos en las dos letras.
    pub measured: u32,
    /// Transiciones fluidas.
    pub fluent: u32,
    /// Transiciones medidas con rollover (sólo como dato).
    pub rollovers: u32,
    /// Mayor cantidad de transiciones fluidas seguidas.
    pub best_streak: u32,
    /// Transiciones fluidas seguidas al final (la racha en curso).
    pub current_streak: u32,
}

/// Resultado del análisis.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Analysis {
    /// Cada transición medida, en orden.
    pub transitions: Vec<Transition>,
    /// Totales.
    pub summary: TransitionSummary,
}

/// Analiza las transiciones de un texto con una meta de velocidad.
///
/// Una transición sin tiempos en alguna de sus letras no se mide y corta la racha, igual que
/// una que no es fluida.
pub fn analyze(letters: &[Letter], target_wpm: f64) -> Analysis {
    let max_hold = hold_limit(median_hold(letters));
    let mut out = Analysis::default();
    let mut streak = 0u32;
    for (i, pair) in letters.windows(2).enumerate() {
        let (prev, cur) = (pair[0], pair[1]);
        let Some(t) = measure(prev, cur, i + 1, target_wpm, max_hold) else {
            streak = 0;
            continue;
        };
        let s = &mut out.summary;
        s.measured += 1;
        s.rollovers += u32::from(t.rollover);
        if t.fluent {
            s.fluent += 1;
            streak += 1;
            s.best_streak = s.best_streak.max(streak);
        } else {
            streak = 0;
        }
        out.transitions.push(t);
    }
    out.summary.current_streak = streak;
    out
}

/// Mide una transición. `None` si falta la bajada de alguna de las dos letras o están fuera
/// de orden.
fn measure(
    prev: Letter,
    cur: Letter,
    at: usize,
    target_wpm: f64,
    max_hold: Duration,
) -> Option<Transition> {
    let (from, to) = (prev.down?, cur.down?);
    let iki = to.checked_sub(from).filter(|d| !d.is_zero())?;
    let kind = TransitionKind::of(prev.finger, cur.finger);
    let rollover = prev.up.is_some_and(|up| up > to);
    let held_too_long = prev.hold().is_some_and(|h| h > max_hold);
    let correct = prev.state == CharState::Correct && cur.state == CharState::Correct;
    let fluent = correct && !held_too_long && iki <= iki_limit(target_wpm, kind);
    Some(Transition {
        at,
        iki,
        rollover,
        held_too_long,
        kind,
        fluent,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Meta de las pruebas: 60 PPM pide 200 ms por letra; el umbral general es 160 ms.
    const WPM: f64 = 60.0;

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    /// Letra bien escrita que baja en `down` y se mantiene `hold` ms, con un dedo dado.
    fn key(down: u64, hold: u64, finger: Finger) -> Letter {
        Letter {
            state: CharState::Correct,
            down: Some(ms(down)),
            up: Some(ms(down + hold)),
            finger: Some(finger),
        }
    }

    /// Dos letras de manos distintas.
    fn pair(iki: u64, hold_prev: u64) -> [Letter; 2] {
        [
            key(1000, hold_prev, Finger::LeftIndex),
            key(1000 + iki, 100, Finger::RightIndex),
        ]
    }

    fn only(letters: &[Letter]) -> Transition {
        let a = analyze(letters, WPM);
        assert_eq!(a.transitions.len(), 1);
        a.transitions[0]
    }

    #[test]
    fn rollover_fast_and_correct_is_fluent() {
        let t = only(&pair(90, 120));
        assert!(t.rollover && t.fluent);
    }

    #[test]
    fn rollover_slow_is_not_fluent() {
        let t = only(&pair(240, 260));
        assert!(t.rollover && !t.fluent);
    }

    #[test]
    fn fast_without_rollover_is_fluent() {
        let t = only(&pair(85, 60));
        assert!(!t.rollover && t.fluent);
    }

    #[test]
    fn rollover_with_a_wrong_letter_is_not_fluent() {
        for wrong in [0, 1] {
            let mut letters = pair(90, 120);
            letters[wrong].state = CharState::Wrong;
            let t = only(&letters);
            assert!(t.rollover && !t.fluent);
        }
    }

    #[test]
    fn a_corrected_letter_is_not_fluent() {
        let mut letters = pair(90, 120);
        letters[1].state = CharState::Retouched;
        assert!(!only(&letters).fluent);
    }

    #[test]
    fn holding_the_previous_key_on_purpose_is_not_fluent() {
        // Diez retenciones normales de 100 ms y después una de 400 ms que pisa la siguiente.
        let mut letters: Vec<Letter> = (0..10)
            .map(|i| key(i * 150, 100, Finger::LeftIndex))
            .collect();
        letters.push(key(1600, 400, Finger::LeftMiddle));
        letters.push(key(1690, 100, Finger::RightIndex));
        let a = analyze(&letters, WPM);
        let last = a.transitions.last().copied().expect("transición");
        assert!(last.rollover && last.held_too_long && !last.fluent);
    }

    #[test]
    fn a_naturally_long_hold_is_not_punished() {
        // Retenciones de 200 ms en todo el ejercicio: 250 ms no es artificial.
        let mut letters: Vec<Letter> = (0..10)
            .map(|i| key(i * 300, 200, Finger::LeftIndex))
            .collect();
        letters.push(key(3100, 250, Finger::LeftMiddle));
        letters.push(key(3190, 100, Finger::RightIndex));
        let last = analyze(&letters, WPM).transitions.last().copied();
        assert!(last.is_some_and(|t| !t.held_too_long && t.fluent));
    }

    #[test]
    fn a_letter_without_timings_is_not_measured_and_breaks_the_streak() {
        let mut letters: Vec<Letter> = (0..6)
            .map(|i| key(i * 100, 80, Finger::LeftIndex))
            .collect();
        for (i, l) in letters.iter_mut().enumerate() {
            l.finger = Some(if i % 2 == 0 {
                Finger::LeftIndex
            } else {
                Finger::RightIndex
            });
        }
        // Una autorrepetición en la letra 3: sin tiempos.
        letters[3].down = None;
        letters[3].up = None;
        let a = analyze(&letters, WPM);
        assert_eq!(a.summary.measured, 3);
        assert_eq!(a.summary.fluent, 3);
        assert_eq!(a.summary.best_streak, 2);
        assert_eq!(a.summary.current_streak, 1);
    }

    #[test]
    fn same_finger_gets_more_time() {
        // 180 ms supera el umbral general (160) pero no el del mismo dedo (200).
        let same = [
            key(0, 100, Finger::LeftIndex),
            key(180, 100, Finger::LeftIndex),
        ];
        let other = [
            key(0, 100, Finger::LeftIndex),
            key(180, 100, Finger::RightIndex),
        ];
        assert!(only(&same).fluent);
        assert_eq!(only(&same).kind, TransitionKind::SameFinger);
        assert!(!only(&other).fluent);
    }

    #[test]
    fn classifies_transitions_by_finger() {
        use Finger::*;
        let of = |a, b| TransitionKind::of(Some(a), Some(b));
        assert_eq!(of(LeftIndex, RightIndex), TransitionKind::OtherHand);
        assert_eq!(of(LeftIndex, LeftPinky), TransitionKind::SameHand);
        assert_eq!(of(RightRing, RightRing), TransitionKind::SameFinger);
        assert_eq!(of(Thumbs, LeftIndex), TransitionKind::OtherHand);
        assert_eq!(of(Thumbs, Thumbs), TransitionKind::SameFinger);
        assert_eq!(
            TransitionKind::of(None, Some(LeftIndex)),
            TransitionKind::Unknown
        );
    }

    #[test]
    fn the_limit_follows_the_target() {
        let near = |d: Duration, want: f64| (d.as_secs_f64() * 1000.0 - want).abs() < 1e-3;
        assert!(near(iki_limit(60.0, TransitionKind::SameHand), 160.0));
        assert!(near(iki_limit(20.0, TransitionKind::SameHand), 480.0));
        assert!(near(iki_limit(60.0, TransitionKind::OtherHand), 144.0));
        assert_eq!(iki_limit(0.0, TransitionKind::SameHand), Duration::MAX);
    }

    #[test]
    fn counts_streaks_and_rollovers() {
        // Cinco transiciones fluidas, una lenta y dos fluidas más.
        let downs = [0, 100, 200, 300, 400, 500, 900, 1000, 1100];
        let letters: Vec<Letter> = downs
            .iter()
            .enumerate()
            .map(|(i, &d)| {
                let finger = if i % 2 == 0 {
                    Finger::LeftIndex
                } else {
                    Finger::RightIndex
                };
                key(d, 120, finger)
            })
            .collect();
        let s = analyze(&letters, WPM).summary;
        assert_eq!(s.measured, 8);
        assert_eq!(s.fluent, 7);
        assert_eq!(s.best_streak, 5);
        assert_eq!(s.current_streak, 2);
        // Retención de 120 ms con IKI de 100 ms: hay rollover salvo antes de la pausa.
        assert_eq!(s.rollovers, 7);
    }

    #[test]
    fn median_needs_enough_samples() {
        let few: Vec<Letter> = (0..5)
            .map(|i| key(i * 100, 50, Finger::LeftIndex))
            .collect();
        assert_eq!(median_hold(&few), None);
        assert_eq!(hold_limit(None), ms(270));
        let many: Vec<Letter> = (0..10)
            .map(|i| key(i * 300, 200, Finger::LeftIndex))
            .collect();
        assert_eq!(median_hold(&many), Some(ms(200)));
        assert_eq!(hold_limit(Some(ms(200))), ms(300));
    }
}
