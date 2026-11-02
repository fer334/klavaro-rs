// SPDX-License-Identifier: GPL-3.0-or-later
//! Fluidez: párrafos completos con sentido.

use crate::rng::RandomSource;

/// Máximo de párrafos que se pueden elegir al azar.
pub const MAX_PARAGRAPHS: usize = 10;
/// Párrafos por defecto.
pub const DEFAULT_PARAGRAPHS: usize = 3;
/// Máximo de párrafos con "texto completo" (`count == 0`).
pub const FULL_TEXT_LIMIT: usize = 50;

/// Elige `count` párrafos distintos al azar (como mucho [`MAX_PARAGRAPHS`]). Con
/// `count == 0` devuelve el texto completo, en orden, hasta [`FULL_TEXT_LIMIT`] párrafos.
/// Con `double_spaces`, agrega un espacio después de cada fin de oración.
///
/// Devuelve `None` si no hay párrafos.
pub fn select(
    paragraphs: &[String],
    count: usize,
    double_spaces: bool,
    rng: &mut impl RandomSource,
) -> Option<String> {
    let usable: Vec<&str> = paragraphs
        .iter()
        .map(|p| p.trim())
        .filter(|p| p.chars().count() >= 2)
        .collect();
    if usable.is_empty() {
        return None;
    }
    let chosen: Vec<&str> = if count == 0 {
        usable.iter().copied().take(FULL_TEXT_LIMIT).collect()
    } else {
        let want = count.min(MAX_PARAGRAPHS).min(usable.len());
        let mut indexes: Vec<usize> = (0..usable.len()).collect();
        let mut picked = Vec::with_capacity(want);
        for _ in 0..want {
            let i = rng.below(indexes.len());
            picked.push(usable[indexes.swap_remove(i)]);
        }
        picked
    };
    let mut out = String::new();
    for p in chosen {
        if double_spaces {
            out.push_str(&add_double_spaces(p));
        } else {
            out.push_str(p);
        }
        out.push('\n');
    }
    Some(out)
}

fn add_double_spaces(p: &str) -> String {
    let mut out = String::with_capacity(p.len() + 8);
    let mut prev = None;
    for c in p.chars() {
        out.push(c);
        if c == ' ' && matches!(prev, Some('.' | '!' | '?')) {
            out.push(' ');
        }
        prev = Some(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rng::SplitMix64;

    fn sample() -> Vec<String> {
        (1..=6)
            .map(|i| format!("Párrafo número {i}. Fin."))
            .collect()
    }

    #[test]
    fn picks_distinct_paragraphs() {
        let text = select(&sample(), 3, false, &mut SplitMix64::new(4)).unwrap_or_default();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 3);
        let mut dedup = lines.clone();
        dedup.sort_unstable();
        dedup.dedup();
        assert_eq!(dedup.len(), 3);
    }

    #[test]
    fn zero_means_full_text_in_order() {
        let text = select(&sample(), 0, false, &mut SplitMix64::new(4)).unwrap_or_default();
        assert!(text.starts_with("Párrafo número 1."));
        assert_eq!(text.lines().count(), 6);
    }

    #[test]
    fn caps_and_double_spaces() {
        let text = select(&sample(), 99, true, &mut SplitMix64::new(4)).unwrap_or_default();
        assert_eq!(text.lines().count(), 6);
        assert!(text.contains(".  Fin."));
    }

    #[test]
    fn empty_input() {
        assert!(select(&[" ".to_owned()], 3, false, &mut SplitMix64::new(1)).is_none());
    }
}
