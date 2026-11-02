// SPDX-License-Identifier: GPL-3.0-or-later
//! Adaptabilidad: pseudopalabras con todo el teclado.
//!
//! Alterna consonantes y vocales de la distribución activa para que las "palabras" sean
//! pronunciables, con símbolos y números de vez en cuando. En una sesión especial, parte de
//! las palabras sale del [`KeyTracker`] para insistir en las teclas débiles.

use crate::generators::{LanguageRules, capitalize_first, close_paragraph};
use crate::keyboard::{KeyboardLayout, is_diacritic};
use crate::rng::RandomSource;
use crate::tracker::{KeyTracker, MAX_WORD_LEN};

/// Párrafos por ejercicio.
pub const PARAGRAPHS: usize = 4;
/// Palabras por párrafo.
pub const WORDS: usize = 10;

/// Genera el ejercicio. `weak_keys`, si se pasa, debe estar ordenado
/// ([`KeyTracker::sort`]) y activa la sesión especial.
pub fn generate(
    layout: &KeyboardLayout,
    rules: &LanguageRules,
    weak_keys: Option<&KeyTracker>,
    rng: &mut impl RandomSource,
) -> String {
    let pools = Pools::new(layout);
    let mut out = String::new();
    for _ in 0..PARAGRAPHS {
        let mut words = Vec::with_capacity(WORDS);
        for i in 0..WORDS {
            let weak = weak_keys.and_then(|t| {
                if rng.one_in(2) {
                    t.weak_word(layout, rng)
                } else {
                    None
                }
            });
            let mut word = match weak {
                Some(w) => w,
                None if rng.one_in(15) => number(&pools, rng),
                None => random_word(layout, &pools, rules, rng),
            };
            if i == 0 {
                word = capitalize_with(layout, &word);
            }
            words.push(word);
        }
        out.push_str(&close_paragraph(&words, rules));
    }
    out
}

struct Pools {
    vowels: Vec<char>,
    consonants: Vec<char>,
    symbols: Vec<char>,
    alt_digits: Vec<char>,
}

impl Pools {
    fn new(layout: &KeyboardLayout) -> Self {
        let consonants: Vec<char> = layout
            .consonants()
            .into_iter()
            .filter(|c| c.is_lowercase() || !c.is_alphabetic())
            .collect();
        Self {
            vowels: layout.vowels(),
            consonants,
            // Sin \ ni ´ sueltos dentro de palabras (corrección del original 2.01).
            symbols: layout
                .symbols()
                .into_iter()
                .filter(|&c| !matches!(c, '\\' | '´' | '`' | '-'))
                .collect(),
            alt_digits: layout.alt_digits(),
        }
    }
}

fn random_word(
    layout: &KeyboardLayout,
    pools: &Pools,
    rules: &LanguageRules,
    rng: &mut impl RandomSource,
) -> String {
    let len = 1 + rng.below(MAX_WORD_LEN - 1);
    let mut word: Vec<char> = Vec::with_capacity(len + 1);
    for pos in 0..len {
        if !pools.symbols.is_empty() && rng.one_in(25) && pos > 0 {
            if let Some(&s) = rng.pick(&pools.symbols) {
                word.push(s);
            }
            break;
        }
        let want_consonant = if pos % 2 == 0 {
            !rng.one_in(50)
        } else {
            rng.one_in(30)
        };
        let pool = if want_consonant && !pools.consonants.is_empty() {
            &pools.consonants
        } else {
            &pools.vowels
        };
        if let Some(&c) = rng.pick(pool) {
            if is_diacritic(c) && word.last().is_none_or(|&p| is_diacritic(p)) {
                if let Some(&v) = rng.pick(&pools.vowels) {
                    word.push(v);
                }
                continue;
            }
            word.push(c);
        }
    }
    if rng.one_in(20) && layout.contains(rules.comma()) {
        word.push(rules.comma());
    }
    let word: String = word.into_iter().collect();
    if rng.one_in(7) {
        capitalize_with(layout, &word)
    } else {
        word
    }
}

fn number(pools: &Pools, rng: &mut impl RandomSource) -> String {
    let use_alt = pools.alt_digits.len() > 5 && !rng.one_in(7);
    (0..4)
        .map(|_| {
            if use_alt {
                rng.pick(&pools.alt_digits).copied().unwrap_or('0')
            } else {
                char::from_digit(u32::try_from(rng.below(10)).unwrap_or(0), 10).unwrap_or('0')
            }
        })
        .collect()
}

/// Mayúscula inicial sólo si la distribución la tiene.
fn capitalize_with(layout: &KeyboardLayout, word: &str) -> String {
    let mut chars = word.chars();
    match chars.next() {
        Some(first) if layout.to_upper(first) != first => std::iter::once(layout.to_upper(first))
            .chain(chars)
            .collect(),
        Some(_) => word.to_owned(),
        None => capitalize_first(word),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keyboard::test_support::qwerty_us;
    use crate::rng::SplitMix64;

    #[test]
    fn only_uses_keys_from_layout() {
        let k = qwerty_us();
        for seed in 0..20 {
            let text = generate(
                &k,
                &LanguageRules::default(),
                None,
                &mut SplitMix64::new(seed),
            );
            for c in text.chars().filter(|c| !c.is_whitespace()) {
                assert!(
                    k.contains(c),
                    "carácter fuera del teclado: {c:?} (semilla {seed})"
                );
            }
            assert_eq!(text.lines().count(), PARAGRAPHS);
            for line in text.lines() {
                assert_eq!(line.split(' ').count(), WORDS);
                assert!(line.ends_with('.'));
            }
        }
    }

    #[test]
    fn special_session_mixes_weak_keys() {
        let k = qwerty_us();
        let mut tracker = KeyTracker::new();
        for c in "qwertyuiop".chars() {
            tracker.wrong(&k, c);
        }
        for _ in 0..30 {
            tracker.wrong(&k, 'x');
        }
        tracker.sort();
        let text = generate(
            &k,
            &LanguageRules::default(),
            Some(&tracker),
            &mut SplitMix64::new(9),
        );
        let normal = generate(&k, &LanguageRules::default(), None, &mut SplitMix64::new(9));
        let count = |t: &str| t.chars().filter(|&c| c == 'x').count();
        assert!(count(&text) > count(&normal));
    }

    #[test]
    fn numbers_have_four_digits() {
        let pools = Pools::new(&qwerty_us());
        let n = number(&pools, &mut SplitMix64::new(2));
        assert_eq!(n.chars().count(), 4);
        assert!(n.chars().all(|c| c.is_ascii_digit()));
    }
}
