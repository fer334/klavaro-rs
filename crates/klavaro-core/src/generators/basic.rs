// SPDX-License-Identifier: GPL-3.0-or-later
//! Curso básico: lecciones definidas por posiciones del teclado.
//!
//! Las lecciones 1–43 vienen de `basic_lessons.txt`: cada una marca con `1` las teclas que
//! entran en juego. Por eso el curso no depende de la distribución: la lección 1 son las
//! mismas posiciones en QWERTY, Dvorak o AZERTY. De la 44 en adelante el usuario escribe
//! directamente los caracteres que quiere practicar.

use crate::generators::LanguageRules;
use crate::keyboard::{KEY_LINE_LEN, KeyPos, KeyboardLayout, Level, ROWS, is_diacritic, to_lower};
use crate::rng::RandomSource;

/// Lecciones fijas del curso.
pub const FIXED_LESSONS: u16 = 43;
/// Última lección posible (las 44+ son personalizadas).
pub const MAX_LESSONS: u16 = 256;
/// Líneas por ejercicio.
pub const LINES: usize = 8;
/// "Palabras" por línea.
pub const WORDS_PER_LINE: usize = 9;
/// Caracteres por "palabra".
pub const WORD_LEN: usize = 5;
/// Máximo de caracteres en una lección personalizada (8 filas × 14).
pub const MAX_CUSTOM_CHARS: usize = 2 * ROWS * KEY_LINE_LEN;

/// Posiciones activas de una lección fija.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LessonMask {
    /// Capa normal.
    pub normal: [[bool; KEY_LINE_LEN]; ROWS],
    /// Capa con Shift.
    pub shifted: [[bool; KEY_LINE_LEN]; ROWS],
}

impl LessonMask {
    /// Caracteres de la lección con una distribución dada, en minúscula y sin repetir.
    pub fn charset(&self, layout: &KeyboardLayout) -> Vec<char> {
        let mut out = Vec::new();
        for (level, mask) in [
            (Level::Normal, &self.normal),
            (Level::Shifted, &self.shifted),
        ] {
            for (row, cols) in mask.iter().enumerate() {
                for (col, &on) in cols.iter().enumerate() {
                    if !on {
                        continue;
                    }
                    if let Some(c) = layout.key(KeyPos { row, col }, level) {
                        let c = to_lower(c);
                        if !c.is_control() && !c.is_whitespace() && !out.contains(&c) {
                            out.push(c);
                        }
                    }
                }
            }
        }
        out
    }
}

/// Limpia el conjunto de caracteres de una lección personalizada: sin controles ni espacios,
/// sin repetidos y como mucho [`MAX_CUSTOM_CHARS`].
pub fn custom_charset(text: &str) -> Vec<char> {
    let mut out = Vec::new();
    for c in text
        .chars()
        .filter(|c| !c.is_control() && !c.is_whitespace())
    {
        if !out.contains(&c) {
            out.push(c);
        }
        if out.len() == MAX_CUSTOM_CHARS {
            break;
        }
    }
    out
}

/// Genera el ejercicio: [`LINES`] líneas de [`WORDS_PER_LINE`] grupos de [`WORD_LEN`]
/// caracteres, sorteados sin reposición de un "bolsón" con el conjunto de la lección para
/// que todas las teclas aparezcan parejo.
///
/// Devuelve `None` si la lección tiene menos de 2 caracteres.
pub fn generate(
    charset: &[char],
    rules: &LanguageRules,
    rng: &mut impl RandomSource,
) -> Option<String> {
    let letters: Vec<char> = charset
        .iter()
        .copied()
        .filter(|&c| !is_diacritic(c))
        .collect();
    if charset.len() < 2 {
        return None;
    }
    // Conjuntos chicos se duplican para que haya repeticiones, como en el original.
    let copies = if (5..=13).contains(&charset.len()) {
        2
    } else {
        1
    };
    let mut pool: Vec<char> = Vec::new();
    let sep = rules.word_separator();
    let mut out = String::new();
    for _ in 0..LINES {
        for w in 0..WORDS_PER_LINE {
            let mut prev: Option<char> = None;
            for _ in 0..WORD_LEN {
                if pool.is_empty() {
                    for _ in 0..copies {
                        pool.extend_from_slice(charset);
                    }
                }
                let mut i = rng.below(pool.len());
                // Una marca combinante no puede empezar un grupo ni ir tras otra marca (salvo
                // que la lección tenga sólo marcas, como algunas de gurmukhi).
                if !letters.is_empty() && is_diacritic(pool[i]) && prev.is_none_or(is_diacritic) {
                    if let Some(j) = pool.iter().position(|&c| !is_diacritic(c)) {
                        i = j;
                    } else {
                        pool.push(letters[rng.below(letters.len())]);
                        i = pool.len() - 1;
                    }
                }
                let c = pool.swap_remove(i);
                out.push(c);
                prev = Some(c);
            }
            out.push(if w + 1 == WORDS_PER_LINE { '\n' } else { sep });
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keyboard::test_support::qwerty_us;
    use crate::rng::SplitMix64;

    #[test]
    fn mask_selects_positions_in_layout() {
        let mut mask = LessonMask::default();
        mask.normal[2][3] = true; // F en QWERTY
        mask.normal[2][6] = true; // J
        mask.shifted[2][3] = true; // F mayúscula → se cuenta como f
        assert_eq!(mask.charset(&qwerty_us()), vec!['f', 'j']);
    }

    #[test]
    fn exercise_shape_and_charset() {
        let set = ['f', 'j', 'd', 'k'];
        let mut rng = SplitMix64::new(11);
        let text = generate(&set, &LanguageRules::default(), &mut rng).unwrap_or_default();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), LINES);
        for line in lines {
            let groups: Vec<&str> = line.split(' ').collect();
            assert_eq!(groups.len(), WORDS_PER_LINE);
            assert!(groups.iter().all(|g| g.chars().count() == WORD_LEN));
        }
        assert!(
            text.chars()
                .filter(|c| !c.is_whitespace())
                .all(|c| set.contains(&c))
        );
        assert!(
            set.iter().all(|c| text.contains(*c)),
            "todas las teclas deben aparecer"
        );
    }

    #[test]
    fn deterministic_with_seed() {
        let set = ['a', 's', 'd', 'f'];
        let a = generate(&set, &LanguageRules::default(), &mut SplitMix64::new(5));
        let b = generate(&set, &LanguageRules::default(), &mut SplitMix64::new(5));
        assert_eq!(a, b);
    }

    #[test]
    fn needs_two_chars() {
        assert!(generate(&['a'], &LanguageRules::default(), &mut SplitMix64::new(1)).is_none());
    }

    #[test]
    fn custom_charset_cleans_input() {
        assert_eq!(custom_charset("aa b\n;ñ"), vec!['a', 'b', ';', 'ñ']);
    }
}
