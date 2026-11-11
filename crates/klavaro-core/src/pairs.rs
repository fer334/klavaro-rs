// SPDX-License-Identifier: GPL-3.0-or-later
//! Rastreo de tiempos y errores por **par de letras**, para ver qué transiciones le cuestan
//! más al estudiante (ver `PLAN_TRANSICIONES_DEBILES.md`).
//!
//! Es el equivalente de [`crate::tracker::KeyTracker`], pero la unidad es el par (`er`, `th`)
//! y no la tecla. Sólo lee lo que ya midió la Progresión: no cambia la evaluación, el puntaje
//! ni las estrellas.
//!
//! Un par son dos letras minúsculas **distintas** seguidas dentro de una palabra. Los
//! espacios, los signos, los números, las mayúsculas y las letras repetidas no se rastrean: el
//! inicio de una palabra es lento por otra razón, una mayúscula mide el Shift y no la
//! transición, y las letras repetidas casi no dicen nada de la velocidad.

use std::collections::BTreeMap;
use std::time::Duration;

use crate::session::CharState;
use crate::transitions::{Letter, TransitionKind};

/// Últimos tiempos guardados por par.
pub const MAX_SAMPLES: usize = 30;
/// Un tiempo entre teclas mayor que éste es una pausa, no una transición lenta.
pub const MAX_IKI: Duration = Duration::from_secs(1);
/// Muestras necesarias para confiar en el tiempo de un par.
pub const MIN_SAMPLES: usize = 6;

/// Lo que se sabe de un par de letras.
#[derive(Debug, Clone, PartialEq)]
pub struct PairEntry {
    /// Primera letra, en minúscula.
    pub from: char,
    /// Segunda letra, en minúscula.
    pub to: char,
    /// Tipo de transición según los dedos, la última vez que se midió.
    pub kind: TransitionKind,
    samples: Vec<f64>,
    /// Veces que se llegó a la segunda letra con la primera bien escrita.
    pub typed: u32,
    /// De ésas, veces que la segunda letra salió mal.
    pub wrong: u32,
}

impl PairEntry {
    /// Crea una entrada con datos guardados. Sobran los tiempos más viejos.
    pub fn new(
        from: char,
        to: char,
        kind: TransitionKind,
        samples: Vec<f64>,
        typed: u32,
        wrong: u32,
    ) -> Self {
        let mut samples = samples;
        let overflow = samples.len().saturating_sub(MAX_SAMPLES);
        samples.drain(..overflow);
        Self {
            from,
            to,
            kind,
            samples,
            typed,
            wrong,
        }
    }

    /// Tiempos guardados (segundos), del más viejo al más nuevo.
    pub fn samples(&self) -> &[f64] {
        &self.samples
    }

    /// Mediana de los tiempos guardados, si hay alguno.
    pub fn median(&self) -> Option<Duration> {
        let mut sorted = self.samples.clone();
        sorted.sort_by(f64::total_cmp);
        let mid = sorted.len() / 2;
        let secs = if sorted.is_empty() {
            return None;
        } else if sorted.len().is_multiple_of(2) {
            f64::midpoint(sorted[mid - 1], sorted[mid])
        } else {
            sorted[mid]
        };
        Some(Duration::from_secs_f64(secs))
    }

    /// Mediana ajustada por el tipo de transición, para comparar pares entre sí: un par del
    /// mismo dedo es físicamente más lento que uno de manos distintas.
    pub fn adjusted(&self) -> Option<Duration> {
        Some(self.median()?.div_f64(self.kind.factor()))
    }

    /// Fracción de las veces que la segunda letra salió mal (0 si nunca se escribió).
    pub fn error_rate(&self) -> f64 {
        if self.typed == 0 {
            0.0
        } else {
            f64::from(self.wrong) / f64::from(self.typed)
        }
    }

    fn push(&mut self, secs: f64) {
        if self.samples.len() >= MAX_SAMPLES {
            self.samples.remove(0);
        }
        self.samples.push(secs);
    }
}

/// Acumulador de tiempos y errores por par para una distribución de teclado.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PairTracker {
    entries: BTreeMap<(char, char), PairEntry>,
}

impl PairTracker {
    /// Rastreador vacío.
    pub fn new() -> Self {
        Self::default()
    }

    /// Reconstruye el rastreador desde datos guardados. Si un par se repite, vale el último.
    pub fn from_entries(entries: impl IntoIterator<Item = PairEntry>) -> Self {
        Self {
            entries: entries.into_iter().map(|e| ((e.from, e.to), e)).collect(),
        }
    }

    /// Todos los pares, en orden alfabético.
    pub fn entries(&self) -> impl Iterator<Item = &PairEntry> {
        self.entries.values()
    }

    /// Un par, si se rastreó alguna vez.
    pub fn get(&self, from: char, to: char) -> Option<&PairEntry> {
        self.entries.get(&(from, to))
    }

    /// Si no hay ningún par.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Anota que el par se escribió bien, con su tiempo entre teclas si se pudo medir. Un
    /// tiempo nulo o mayor a [`MAX_IKI`] cuenta como escrito, pero no deja muestra.
    pub fn record(&mut self, from: char, to: char, kind: TransitionKind, iki: Option<Duration>) {
        let Some(entry) = self.entry(from, to, kind) else {
            return;
        };
        entry.typed = entry.typed.saturating_add(1);
        if let Some(iki) = iki.filter(|d| !d.is_zero() && *d <= MAX_IKI) {
            entry.push(iki.as_secs_f64());
        }
    }

    /// Anota que la segunda letra del par salió mal.
    pub fn record_error(&mut self, from: char, to: char, kind: TransitionKind) {
        if let Some(entry) = self.entry(from, to, kind) {
            entry.typed = entry.typed.saturating_add(1);
            entry.wrong = entry.wrong.saturating_add(1);
        }
    }

    /// Carga un ejercicio terminado: `target` es el texto y `letters`, lo que se sabe de cómo
    /// se escribió cada letra (lo mismo que recibe [`crate::transitions::analyze`]).
    ///
    /// Sólo cuentan los pares cuya primera letra se escribió bien a la primera. Si la segunda
    /// también, se guarda el tiempo; si salió mal (aunque después se corrigiera), es un error.
    pub fn record_exercise(&mut self, target: &[char], letters: &[Letter]) {
        let chars = target.windows(2);
        for (pair, typed) in chars.zip(letters.windows(2)) {
            let (prev, cur) = (typed[0], typed[1]);
            if prev.state != CharState::Correct {
                continue;
            }
            let kind = TransitionKind::of(prev.finger, cur.finger);
            match cur.state {
                CharState::Correct => {
                    let iki = prev.down.zip(cur.down).and_then(|(a, b)| b.checked_sub(a));
                    self.record(pair[0], pair[1], kind, iki);
                }
                CharState::Wrong | CharState::Retouched => {
                    self.record_error(pair[0], pair[1], kind);
                }
                CharState::Untouched => {}
            }
        }
    }

    /// Los pares más lentos con al menos [`MIN_SAMPLES`] muestras, del más lento al menos
    /// lento según su tiempo ajustado por tipo.
    pub fn slowest(&self, limit: usize) -> Vec<&PairEntry> {
        let mut measured: Vec<(Duration, &PairEntry)> = self
            .entries
            .values()
            .filter(|e| e.samples.len() >= MIN_SAMPLES)
            .filter_map(|e| Some((e.adjusted()?, e)))
            .collect();
        measured.sort_by_key(|&(adjusted, e)| (std::cmp::Reverse(adjusted), e.from, e.to));
        measured.into_iter().take(limit).map(|(_, e)| e).collect()
    }

    /// Entrada del par, creada si hace falta. `None` si no es un par que se rastree.
    fn entry(&mut self, from: char, to: char, kind: TransitionKind) -> Option<&mut PairEntry> {
        let (from, to) = pair_key(from, to)?;
        let entry = self
            .entries
            .entry((from, to))
            .or_insert_with(|| PairEntry::new(from, to, kind, Vec::new(), 0, 0));
        entry.kind = kind;
        Some(entry)
    }
}

/// Clave de un par: sus dos letras. `None` si alguna no es una letra, si alguna es mayúscula
/// o si son la misma.
///
/// Las mayúsculas quedan afuera porque se escriben con Shift: en una lección real, las «Th»
/// de inicio de oración tardaron 167–216 ms y las «th», 92–122 ms.
pub fn pair_key(from: char, to: char) -> Option<(char, char)> {
    let tracked = |c: char| c.is_alphabetic() && !c.is_uppercase();
    (tracked(from) && tracked(to) && from != to).then_some((from, to))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keyboard::Finger;

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    /// Letra bien escrita que baja en `down` ms.
    fn key(down: u64, finger: Finger) -> Letter {
        Letter {
            state: CharState::Correct,
            down: Some(ms(down)),
            up: Some(ms(down + 80)),
            finger: Some(finger),
        }
    }

    fn chars(s: &str) -> Vec<char> {
        s.chars().collect()
    }

    /// Teclea `text` con las bajadas en `downs` (ms) alternando manos.
    fn typed(text: &str, downs: &[u64]) -> (Vec<char>, Vec<Letter>) {
        let letters = downs
            .iter()
            .enumerate()
            .map(|(i, &d)| {
                let finger = if i % 2 == 0 {
                    Finger::LeftIndex
                } else {
                    Finger::RightIndex
                };
                key(d, finger)
            })
            .collect();
        (chars(text), letters)
    }

    #[test]
    fn records_the_time_of_each_pair_inside_a_word() {
        let mut t = PairTracker::new();
        let (target, letters) = typed("the", &[0, 120, 300]);
        t.record_exercise(&target, &letters);
        let th = t.get('t', 'h').expect("th");
        assert_eq!(th.samples(), &[0.12]);
        assert_eq!((th.typed, th.wrong), (1, 0));
        assert_eq!(th.kind, TransitionKind::OtherHand);
        assert_eq!(t.get('h', 'e').and_then(PairEntry::median), Some(ms(180)));
        assert_eq!(t.entries().count(), 2);
    }

    #[test]
    fn skips_spaces_signs_digits_and_repeated_letters() {
        let mut t = PairTracker::new();
        let (target, letters) = typed(
            "a b,c2d ee",
            &[0, 100, 200, 300, 400, 500, 600, 700, 800, 900],
        );
        t.record_exercise(&target, &letters);
        assert!(t.is_empty(), "{t:?}");
    }

    #[test]
    fn pairs_with_a_capital_letter_are_skipped() {
        let mut t = PairTracker::new();
        let (target, letters) = typed("The", &[0, 150, 250]);
        t.record_exercise(&target, &letters);
        // «Th» mide el Shift; «he» es una transición normal.
        assert_eq!(t.get('t', 'h'), None);
        assert_eq!(t.get('h', 'e').map(|e| e.typed), Some(1));
        assert_eq!(t.entries().count(), 1);
        assert_eq!(pair_key('t', 'h'), Some(('t', 'h')));
        assert_eq!(pair_key('T', 'h'), None);
        assert_eq!(pair_key('t', 'H'), None);
        assert_eq!(pair_key('ñ', 'o'), Some(('ñ', 'o')));
        // `record` a secas tampoco las acepta.
        t.record('T', 'h', TransitionKind::SameHand, Some(ms(100)));
        assert_eq!(t.entries().count(), 1);
    }

    #[test]
    fn a_wrong_second_letter_is_an_error_without_a_sample() {
        let mut t = PairTracker::new();
        for state in [CharState::Wrong, CharState::Retouched] {
            let (target, mut letters) = typed("er", &[0, 90]);
            letters[1].state = state;
            t.record_exercise(&target, &letters);
        }
        let er = t.get('e', 'r').expect("er");
        assert_eq!((er.typed, er.wrong), (2, 2));
        assert_eq!(er.samples().len(), 0);
        assert!((er.error_rate() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn a_wrong_first_letter_does_not_count_at_all() {
        let mut t = PairTracker::new();
        for state in [CharState::Wrong, CharState::Retouched, CharState::Untouched] {
            let (target, mut letters) = typed("er", &[0, 90]);
            letters[0].state = state;
            t.record_exercise(&target, &letters);
        }
        assert!(t.is_empty());
    }

    #[test]
    fn pauses_and_letters_without_timings_leave_no_sample() {
        let mut t = PairTracker::new();
        // Una pausa de más de un segundo.
        let (target, letters) = typed("er", &[0, 1001]);
        t.record_exercise(&target, &letters);
        // Una letra del método de entrada o de la autorrepetición.
        let (target, mut letters) = typed("er", &[0, 90]);
        letters[1].down = None;
        t.record_exercise(&target, &letters);
        // Tiempos fuera de orden.
        let (target, letters) = typed("er", &[90, 90]);
        t.record_exercise(&target, &letters);
        let er = t.get('e', 'r').expect("er");
        assert_eq!(er.samples().len(), 0);
        assert_eq!((er.typed, er.wrong), (3, 0));
        // Justo un segundo todavía cuenta.
        let (target, letters) = typed("er", &[0, 1000]);
        t.record_exercise(&target, &letters);
        assert_eq!(t.get('e', 'r').map(|e| e.samples().len()), Some(1));
    }

    #[test]
    fn keeps_only_the_latest_samples() {
        let mut t = PairTracker::new();
        for i in 0..40u64 {
            t.record('e', 'r', TransitionKind::SameHand, Some(ms(100 + i)));
        }
        let er = t.get('e', 'r').expect("er");
        assert_eq!(er.samples().len(), MAX_SAMPLES);
        assert!((er.samples()[0] - 0.110).abs() < 1e-9);
        assert_eq!(er.typed, 40);
        let loaded = PairEntry::new('e', 'r', TransitionKind::SameHand, vec![0.1; 50], 50, 0);
        assert_eq!(loaded.samples().len(), MAX_SAMPLES);
    }

    #[test]
    fn median_ignores_an_odd_sample() {
        let odd = PairEntry::new(
            'e',
            'r',
            TransitionKind::SameHand,
            vec![0.1, 0.9, 0.12],
            3,
            0,
        );
        assert_eq!(odd.median(), Some(ms(120)));
        let even = PairEntry::new('e', 'r', TransitionKind::SameHand, vec![0.1, 0.2], 2, 0);
        assert_eq!(even.median(), Some(ms(150)));
        let none = PairEntry::new('e', 'r', TransitionKind::SameHand, vec![], 0, 0);
        assert_eq!(none.median(), None);
        assert!(none.error_rate().abs() < f64::EPSILON);
    }

    #[test]
    fn slowest_needs_enough_samples_and_adjusts_by_kind() {
        let mut t = PairTracker::new();
        let mut fill = |from, to, kind, iki: u64, n: usize| {
            for _ in 0..n {
                t.record(from, to, kind, Some(ms(iki)));
            }
        };
        // Mismo dedo: 250 ms ajustado da 200 ms.
        fill('d', 'e', TransitionKind::SameFinger, 250, MIN_SAMPLES);
        // Manos distintas: 198 ms ajustado da 220 ms.
        fill('t', 'h', TransitionKind::OtherHand, 198, MIN_SAMPLES);
        fill('a', 's', TransitionKind::SameHand, 150, MIN_SAMPLES);
        // Muy lento, pero con pocas muestras.
        fill('z', 'x', TransitionKind::SameHand, 900, MIN_SAMPLES - 1);
        let order: Vec<(char, char)> = t.slowest(10).iter().map(|e| (e.from, e.to)).collect();
        assert_eq!(order, vec![('t', 'h'), ('d', 'e'), ('a', 's')]);
        assert_eq!(t.slowest(1).len(), 1);
        assert_eq!(PairTracker::new().slowest(10).len(), 0);
    }

    #[test]
    fn rebuilds_from_saved_entries() {
        let mut t = PairTracker::new();
        t.record('e', 'r', TransitionKind::SameHand, Some(ms(140)));
        t.record_error('t', 'h', TransitionKind::OtherHand);
        let copy = PairTracker::from_entries(t.entries().cloned());
        assert_eq!(copy, t);
    }
}
