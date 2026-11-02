// SPDX-License-Identifier: GPL-3.0-or-later
//! Velocidad: palabras reales del idioma, al azar.

use crate::generators::{LanguageRules, capitalize_first, close_paragraph};
use crate::rng::RandomSource;

/// Párrafos por ejercicio.
pub const PARAGRAPHS: usize = 4;
/// Palabras por párrafo.
pub const WORDS: usize = 20;

/// Genera [`PARAGRAPHS`] párrafos de [`WORDS`] palabras tomadas al azar de la lista.
/// Devuelve `None` si la lista está vacía.
pub fn generate(
    words: &[String],
    rules: &LanguageRules,
    rng: &mut impl RandomSource,
) -> Option<String> {
    if words.is_empty() {
        return None;
    }
    let mut out = String::new();
    for _ in 0..PARAGRAPHS {
        let mut paragraph: Vec<String> = Vec::with_capacity(WORDS);
        for i in 0..WORDS {
            let w = rng.pick(words)?;
            paragraph.push(if i == 0 {
                capitalize_first(w)
            } else {
                w.clone()
            });
        }
        out.push_str(&close_paragraph(&paragraph, rules));
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rng::SplitMix64;

    #[test]
    fn four_paragraphs_of_twenty_words() {
        let words: Vec<String> = ["casa", "perro", "árbol", "mesa"]
            .map(str::to_owned)
            .to_vec();
        let text = generate(&words, &LanguageRules::new("es"), &mut SplitMix64::new(1))
            .unwrap_or_default();
        let paragraphs: Vec<&str> = text.lines().collect();
        assert_eq!(paragraphs.len(), PARAGRAPHS);
        for p in paragraphs {
            assert!(p.chars().next().is_some_and(char::is_uppercase));
            assert!(p.ends_with('.'));
            assert_eq!(p.trim_end_matches('.').split(' ').count(), WORDS);
        }
    }

    #[test]
    fn empty_list() {
        assert!(generate(&[], &LanguageRules::default(), &mut SplitMix64::new(1)).is_none());
    }
}
