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
/// Pares medidos necesarios para saber cuál es el tiempo habitual del estudiante.
pub const MIN_PAIRS: usize = 8;
/// Un par es lento si su tiempo ajustado llega a este múltiplo del tiempo habitual.
pub const SLOW_FACTOR: f64 = 1.25;
/// Un par se falla seguido si la segunda letra sale mal esta fracción de las veces.
pub const ERROR_RATE: f64 = 0.15;
/// Errores necesarios para decir que un par se falla seguido: uno solo es un tropiezo.
pub const MIN_ERRORS: u32 = 2;
/// Veces escritas a partir de las cuales la cuenta de errores se reduce a la mitad, para que
/// los errores viejos pesen cada vez menos.
pub const ERROR_WINDOW: u32 = 60;

/// Cuántas veces aparece cada par en un texto de referencia (ver [`frequencies`]).
pub type PairFrequencies = BTreeMap<(char, char), u32>;

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

    /// Suma una vez escrito (y un error, si `wrong`). Pasada [`ERROR_WINDOW`], las dos cuentas
    /// se reducen a la mitad: la proporción queda igual y lo nuevo pesa más.
    fn count(&mut self, wrong: bool) {
        self.typed = self.typed.saturating_add(1);
        self.wrong = self.wrong.saturating_add(u32::from(wrong));
        if self.typed > ERROR_WINDOW {
            self.typed /= 2;
            self.wrong /= 2;
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
        entry.count(false);
        if let Some(iki) = iki.filter(|d| !d.is_zero() && *d <= MAX_IKI) {
            entry.push(iki.as_secs_f64());
        }
    }

    /// Anota que la segunda letra del par salió mal.
    pub fn record_error(&mut self, from: char, to: char, kind: TransitionKind) {
        if let Some(entry) = self.entry(from, to, kind) {
            entry.count(true);
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

    /// Tiempo habitual del estudiante: la mediana de los tiempos ajustados de sus pares con
    /// al menos [`MIN_SAMPLES`] muestras. `None` con menos de [`MIN_PAIRS`] pares medidos.
    pub fn typical(&self) -> Option<Duration> {
        let mut adjusted: Vec<Duration> = self
            .entries
            .values()
            .filter(|e| e.samples.len() >= MIN_SAMPLES)
            .filter_map(PairEntry::adjusted)
            .collect();
        if adjusted.len() < MIN_PAIRS {
            return None;
        }
        adjusted.sort_unstable();
        let mid = adjusted.len() / 2;
        Some(if adjusted.len().is_multiple_of(2) {
            (adjusted[mid - 1] + adjusted[mid]) / 2
        } else {
            adjusted[mid]
        })
    }

    /// Los pares débiles, del que más cuesta al que menos, hasta `limit`.
    ///
    /// Un par es débil si es **lento** (tiene [`MIN_SAMPLES`] muestras y su tiempo ajustado
    /// llega a [`SLOW_FACTOR`] veces el habitual) o si **se falla seguido** (se escribió
    /// [`MIN_SAMPLES`] veces y la segunda letra salió mal [`ERROR_RATE`] de ellas o más, con
    /// al menos [`MIN_ERRORS`] errores).
    ///
    /// El orden es por gravedad × frecuencia: un par común que cuesta un poco molesta más
    /// que uno raro que cuesta mucho. `frequencies` sale de [`frequencies`]; un par que no
    /// está ahí cuenta como si apareciera una vez.
    pub fn weak(&self, frequencies: &PairFrequencies, limit: usize) -> Vec<WeakPair> {
        let typical = self.typical();
        let mut weak: Vec<(f64, WeakPair)> = self
            .entries
            .values()
            .filter_map(|e| {
                let pair = WeakPair::of(e, typical)?;
                let frequency = frequencies.get(&(e.from, e.to)).copied().unwrap_or(0);
                Some((pair.severity() * (1.0 + f64::from(frequency)), pair))
            })
            .collect();
        weak.sort_by(|(a, x), (b, y)| {
            b.total_cmp(a)
                .then_with(|| (x.from, x.to).cmp(&(y.from, y.to)))
        });
        weak.into_iter().take(limit).map(|(_, pair)| pair).collect()
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

/// Un par que le cuesta al estudiante (ver [`PairTracker::weak`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WeakPair {
    /// Primera letra.
    pub from: char,
    /// Segunda letra.
    pub to: char,
    /// Tipo de transición según los dedos.
    pub kind: TransitionKind,
    /// Mediana sin ajustar, que es lo que se muestra. `None` si todavía no hay tiempos.
    pub median: Option<Duration>,
    /// Tiempo ajustado dividido por el tiempo habitual (1 = como siempre). `None` si falta
    /// alguno de los dos o hay pocas muestras.
    pub excess: Option<f64>,
    /// Tiempos guardados.
    pub samples: usize,
    /// Fracción de las veces que la segunda letra salió mal.
    pub error_rate: f64,
    /// Es débil por lento.
    pub slow: bool,
    /// Es débil porque se falla seguido.
    pub error_prone: bool,
}

impl WeakPair {
    /// Evalúa un par contra el tiempo habitual. `None` si no es débil.
    fn of(e: &PairEntry, typical: Option<Duration>) -> Option<Self> {
        let excess = (e.samples.len() >= MIN_SAMPLES)
            .then(|| Some(e.adjusted()?.as_secs_f64() / typical?.as_secs_f64()))
            .flatten()
            .filter(|x| x.is_finite());
        let slow = excess.is_some_and(|x| x >= SLOW_FACTOR);
        let error_rate = e.error_rate();
        let error_prone =
            e.typed as usize >= MIN_SAMPLES && e.wrong >= MIN_ERRORS && error_rate >= ERROR_RATE;
        (slow || error_prone).then_some(Self {
            from: e.from,
            to: e.to,
            kind: e.kind,
            median: e.median(),
            excess,
            samples: e.samples.len(),
            error_rate,
            slow,
            error_prone,
        })
    }

    /// Cuánto pasa el par de sus umbrales: 1 es justo en el límite.
    fn severity(&self) -> f64 {
        let slow = self.excess.unwrap_or(0.0) / SLOW_FACTOR;
        let errors = self.error_rate / ERROR_RATE;
        slow.max(errors)
    }
}

/// Cuenta cuántas veces aparece cada par en una lista de palabras, para saber cuáles son
/// comunes en el idioma.
pub fn frequencies<'a>(words: impl IntoIterator<Item = &'a str>) -> PairFrequencies {
    let mut out = PairFrequencies::new();
    for word in words {
        let letters: Vec<char> = word.chars().collect();
        for w in letters.windows(2) {
            if let Some(key) = pair_key(w[0], w[1]) {
                let n = out.entry(key).or_insert(0);
                *n = n.saturating_add(1);
            }
        }
    }
    out
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

    /// Rastreo con ocho pares normales de la misma mano a 100 ms: el tiempo habitual.
    fn baseline() -> PairTracker {
        let mut t = PairTracker::new();
        for to in ['b', 'c', 'd', 'f', 'g', 'h', 'j', 'k'] {
            fill(&mut t, ('a', to), TransitionKind::SameHand, 100, 10);
        }
        t
    }

    fn fill(t: &mut PairTracker, pair: (char, char), kind: TransitionKind, iki: u64, n: usize) {
        for _ in 0..n {
            t.record(pair.0, pair.1, kind, Some(ms(iki)));
        }
    }

    fn names(weak: &[WeakPair]) -> Vec<String> {
        weak.iter().map(|w| format!("{}{}", w.from, w.to)).collect()
    }

    #[test]
    fn typical_needs_enough_measured_pairs() {
        assert_eq!(PairTracker::new().typical(), None);
        let mut t = PairTracker::new();
        for to in ['b', 'c', 'd', 'f', 'g', 'h', 'j'] {
            fill(&mut t, ('a', to), TransitionKind::SameHand, 100, 10);
        }
        // Siete pares medidos y uno con pocas muestras: todavía no alcanza.
        fill(
            &mut t,
            ('a', 'k'),
            TransitionKind::SameHand,
            100,
            MIN_SAMPLES - 1,
        );
        assert_eq!(t.typical(), None);
        assert_eq!(t.weak(&PairFrequencies::new(), 10), vec![]);
        fill(&mut t, ('a', 'k'), TransitionKind::SameHand, 100, 1);
        assert_eq!(t.typical(), Some(ms(100)));
        // El tiempo habitual usa los tiempos ajustados: un par de manos distintas a 90 ms
        // vale 100 ms.
        fill(&mut t, ('a', 'l'), TransitionKind::OtherHand, 90, 10);
        assert_eq!(t.typical(), Some(ms(100)));
    }

    #[test]
    fn a_slow_pair_with_enough_samples_is_weak() {
        let mut t = baseline();
        fill(&mut t, ('e', 'r'), TransitionKind::SameHand, 130, 10);
        // Lento, pero con pocas muestras.
        fill(&mut t, ('z', 'x'), TransitionKind::SameHand, 400, 3);
        // Apenas por debajo del umbral.
        fill(&mut t, ('o', 'u'), TransitionKind::SameHand, 124, 10);
        let weak = t.weak(&PairFrequencies::new(), 10);
        assert_eq!(names(&weak), ["er"]);
        let er = weak[0];
        assert_eq!(er.median, Some(ms(130)));
        assert!((er.excess.expect("exceso") - 1.3).abs() < 1e-9);
        assert_eq!(er.samples, 10);
        assert!(er.slow && !er.error_prone);
        assert_eq!(er.kind, TransitionKind::SameHand);
    }

    #[test]
    fn a_same_finger_pair_is_judged_by_its_adjusted_time() {
        let mut t = baseline();
        // 150 ms con el mismo dedo son 120 ms ajustados: no llega al umbral de 125 ms.
        fill(&mut t, ('d', 'e'), TransitionKind::SameFinger, 150, 10);
        // Los mismos 150 ms con manos distintas son 167 ms ajustados.
        fill(&mut t, ('t', 'i'), TransitionKind::OtherHand, 150, 10);
        assert_eq!(names(&t.weak(&PairFrequencies::new(), 10)), ["ti"]);
    }

    #[test]
    fn a_fast_pair_that_fails_often_is_weak() {
        let mut t = baseline();
        fill(&mut t, ('i', 'o'), TransitionKind::SameHand, 80, 8);
        for _ in 0..2 {
            t.record_error('i', 'o', TransitionKind::SameHand);
        }
        // Un solo error en muchas veces no alcanza.
        fill(&mut t, ('u', 'i'), TransitionKind::SameHand, 80, 20);
        t.record_error('u', 'i', TransitionKind::SameHand);
        // Un solo error en pocas veces pasa la proporción, pero es un tropiezo.
        fill(&mut t, ('o', 'r'), TransitionKind::SameHand, 80, 5);
        t.record_error('o', 'r', TransitionKind::SameHand);
        // Siempre mal, pero pocas veces.
        for _ in 0..MIN_SAMPLES - 1 {
            t.record_error('q', 'w', TransitionKind::SameHand);
        }
        let weak = t.weak(&PairFrequencies::new(), 10);
        assert_eq!(names(&weak), ["io"]);
        assert!(!weak[0].slow && weak[0].error_prone);
        assert!((weak[0].error_rate - 0.2).abs() < 1e-9);

        // Un par que siempre sale mal no tiene tiempos, y es débil igual. No hace falta el
        // tiempo habitual para verlo.
        let mut t = PairTracker::new();
        for _ in 0..MIN_SAMPLES {
            t.record_error('q', 'w', TransitionKind::SameHand);
        }
        let weak = t.weak(&PairFrequencies::new(), 10);
        assert_eq!(names(&weak), ["qw"]);
        assert_eq!((weak[0].median, weak[0].excess), (None, None));
    }

    #[test]
    fn frequent_pairs_come_first() {
        let mut t = baseline();
        fill(&mut t, ('z', 'x'), TransitionKind::SameHand, 200, 10);
        fill(&mut t, ('e', 'r'), TransitionKind::SameHand, 130, 10);
        fill(&mut t, ('t', 'i'), TransitionKind::SameHand, 130, 10);
        let freq = frequencies(["her", "over", "tie", "Zx", "a-er", "eel"]);
        assert_eq!(freq.get(&('e', 'r')), Some(&3));
        assert_eq!(freq.get(&('t', 'i')), Some(&1));
        // Ni mayúsculas ni letras repetidas.
        assert_eq!(freq.get(&('z', 'x')), None);
        assert_eq!(freq.get(&('e', 'e')), None);
        // Igual de lentos, «er» es más común que «ti»; «zx» es el más lento pero no aparece.
        assert_eq!(names(&t.weak(&freq, 10)), ["er", "ti", "zx"]);
        assert_eq!(names(&t.weak(&freq, 2)), ["er", "ti"]);
        // Sin frecuencias, manda la gravedad.
        assert_eq!(names(&t.weak(&PairFrequencies::new(), 1)), ["zx"]);
    }

    #[test]
    fn a_pair_that_improved_stops_being_weak() {
        let mut t = baseline();
        fill(
            &mut t,
            ('e', 'r'),
            TransitionKind::SameHand,
            200,
            MAX_SAMPLES,
        );
        for _ in 0..12 {
            t.record_error('e', 'r', TransitionKind::SameHand);
        }
        let weak = t.weak(&PairFrequencies::new(), 10);
        assert!(weak[0].slow && weak[0].error_prone);
        // Las últimas muestras son rápidas y sin errores: los tiempos viejos se van y los
        // errores viejos pesan cada vez menos.
        fill(
            &mut t,
            ('e', 'r'),
            TransitionKind::SameHand,
            100,
            4 * MAX_SAMPLES,
        );
        assert_eq!(t.weak(&PairFrequencies::new(), 10), vec![]);
        let er = t.get('e', 'r').expect("er");
        assert!(er.typed <= ERROR_WINDOW, "{er:?}");
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
