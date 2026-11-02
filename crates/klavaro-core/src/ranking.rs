// SPDX-License-Identifier: GPL-3.0-or-later
//! Top 10 local de Fluidez (`top10.c` del original, sin la parte de red).

use crate::metrics::Stats;

/// Lugares del ranking.
pub const TOP: usize = 10;
/// Mínimo de caracteres para que una sesión entre al ranking.
///
/// Por confirmar con el código original (`MIN_CHARS_TO_LOG`).
pub const MIN_CHARS: u32 = 500;
/// Umbral de la prueba de distribución de letras: por encima, el texto no se parece al idioma.
pub const DISTRIBUTION_THRESHOLD: f64 = 0.205;
/// Cantidad de letras más frecuentes que se comparan.
const TOP_LETTERS: usize = 9;

/// Una entrada del ranking.
#[derive(Debug, Clone, PartialEq)]
pub struct RankingEntry {
    /// Nombre a mostrar, con el teclado entre corchetes (`Ana [qwerty_es]`).
    pub name: String,
    /// Código de idioma.
    pub language: String,
    /// Fecha y hora (segundos desde 1970).
    pub when: i64,
    /// Caracteres tecleados.
    pub chars: u32,
    /// Precisión (%).
    pub accuracy: f64,
    /// Velocidad (PPM).
    pub wpm: f64,
    /// Fluidez.
    pub fluidity: f64,
    /// Puntaje.
    pub score: f64,
}

impl RankingEntry {
    /// Crea una entrada desde un resultado.
    pub fn from_stats(name: String, language: String, when: i64, stats: &Stats) -> Self {
        Self {
            name,
            language,
            when,
            chars: stats.touches,
            accuracy: stats.accuracy,
            wpm: stats.wpm,
            fluidity: stats.fluidity,
            score: stats.score(),
        }
    }

    /// Validaciones de rango del original, contra resultados imposibles o manipulados.
    pub fn is_valid(&self) -> bool {
        self.chars >= MIN_CHARS
            && (0.0..=100.0).contains(&self.accuracy)
            && (0.0..222.222).contains(&self.wpm)
            && (0.0..=95.0).contains(&self.fluidity)
            && (0.0..=17.3333).contains(&self.score)
    }
}

/// Por qué una sesión no entró al ranking.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rejection {
    /// Fuera de los rangos válidos (o muy pocos caracteres).
    Invalid,
    /// El texto no se parece al idioma (por ejemplo, una sola letra repetida).
    TextNotSimilar,
    /// El puntaje no alcanza para el Top 10.
    ScoreTooLow,
}

/// Ranking local de un idioma.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Ranking {
    entries: Vec<RankingEntry>,
}

impl Ranking {
    /// Ranking desde entradas guardadas (se ordenan y recortan).
    pub fn new(mut entries: Vec<RankingEntry>) -> Self {
        entries.sort_by(|a, b| b.score.total_cmp(&a.score));
        entries.truncate(TOP);
        Self { entries }
    }

    /// Entradas de mayor a menor puntaje.
    pub fn entries(&self) -> &[RankingEntry] {
        &self.entries
    }

    /// Intenta agregar una entrada. Devuelve el puesto (desde 1) si entra.
    ///
    /// # Errors
    ///
    /// Devuelve el motivo si la entrada no es válida, el texto no pasa la prueba de
    /// distribución (`text_ok == false`) o el puntaje no alcanza.
    pub fn insert(&mut self, entry: RankingEntry, text_ok: bool) -> Result<usize, Rejection> {
        if !entry.is_valid() {
            return Err(Rejection::Invalid);
        }
        if !text_ok {
            return Err(Rejection::TextNotSimilar);
        }
        let pos = self
            .entries
            .iter()
            .position(|e| entry.score > e.score)
            .unwrap_or(self.entries.len());
        if pos >= TOP {
            return Err(Rejection::ScoreTooLow);
        }
        self.entries.insert(pos, entry);
        self.entries.truncate(TOP);
        Ok(pos + 1)
    }
}

/// Compara la distribución de letras de `typed` con la del idioma (`model`, por ejemplo los
/// párrafos por defecto). Devuelve la desviación: menor a [`DISTRIBUTION_THRESHOLD`] = aprobado.
pub fn letter_distribution_deviation(model: &str, typed: &str) -> f64 {
    let m = top_letters(model);
    let t = top_letters(typed);
    if m.is_empty() {
        return 0.0;
    }
    let mut sum = 0.0;
    for (letter, freq) in &m {
        let other = t
            .iter()
            .find(|(l, _)| l == letter)
            .map_or(-1.0, |(_, f)| *f);
        sum += (freq - other).powi(2);
    }
    (sum / TOP_LETTERS as f64).sqrt()
}

/// Si el texto tecleado se parece al idioma.
pub fn text_resembles_language(model: &str, typed: &str) -> bool {
    letter_distribution_deviation(model, typed) < DISTRIBUTION_THRESHOLD
}

/// Las [`TOP_LETTERS`] letras más frecuentes con su frecuencia relativa a la primera.
fn top_letters(text: &str) -> Vec<(char, f64)> {
    let mut counts: Vec<(char, u32)> = Vec::new();
    for c in text
        .chars()
        .filter(|c| c.is_alphabetic())
        .flat_map(char::to_lowercase)
    {
        match counts.iter_mut().find(|(l, _)| *l == c) {
            Some((_, n)) => *n += 1,
            None => counts.push((c, 1)),
        }
    }
    counts.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    let Some(&(_, max)) = counts.first() else {
        return Vec::new();
    };
    counts
        .into_iter()
        .take(TOP_LETTERS)
        .map(|(c, n)| (c, f64::from(n) / f64::from(max)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SPANISH: &str = "En un lugar de la Mancha, de cuyo nombre no quiero acordarme, no ha \
        mucho tiempo que vivía un hidalgo de los de lanza en astillero, adarga antigua, rocín \
        flaco y galgo corredor.";

    fn entry(score: f64) -> RankingEntry {
        RankingEntry {
            name: "Ana [qwerty_es]".to_owned(),
            language: "es".to_owned(),
            when: 0,
            chars: 800,
            accuracy: 98.0,
            wpm: 60.0,
            fluidity: 80.0,
            score,
        }
    }

    #[test]
    fn similar_text_passes_and_junk_fails() {
        // Mismas palabras en otro orden: misma distribución de letras.
        let typed: String = SPANISH.split(' ').rev().collect::<Vec<_>>().join(" ");
        assert!(letter_distribution_deviation(SPANISH, &typed) < 1e-9);
        assert!(text_resembles_language(SPANISH, &typed));
        assert!(!text_resembles_language(SPANISH, "zzzz zzzz zzzz qqqq"));
    }

    #[test]
    fn inserts_in_order_and_keeps_ten() {
        let mut r = Ranking::default();
        for i in 0..12 {
            let _ = r.insert(entry(f64::from(i)), true);
        }
        assert_eq!(r.entries().len(), TOP);
        assert!((r.entries()[0].score - 11.0).abs() < 1e-9);
        assert_eq!(r.insert(entry(0.5), true), Err(Rejection::ScoreTooLow));
        assert_eq!(r.insert(entry(15.0), true), Ok(1));
    }

    #[test]
    fn rejects_invalid_and_dissimilar() {
        let mut r = Ranking::default();
        let mut bad = entry(5.0);
        bad.chars = 10;
        assert_eq!(r.insert(bad, true), Err(Rejection::Invalid));
        assert_eq!(r.insert(entry(5.0), false), Err(Rejection::TextNotSimilar));
    }
}
