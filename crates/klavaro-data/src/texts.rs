// SPDX-License-Identifier: GPL-3.0-or-later
//! Listas de palabras (`*.words`), párrafos (`*.paragraphs`) y textos importados.

use klavaro_core::keyboard::is_diacritic;

use crate::lines;

/// Máximo de palabras que se guardan de un texto importado.
pub const MAX_WORDS: usize = 100_000;
/// Máximo de párrafos que se guardan de un texto importado.
pub const MAX_PARAGRAPHS: usize = 10_000;

/// Lee un `.words`: una palabra por línea; se ignoran líneas vacías.
pub fn parse_words(text: &str) -> Vec<String> {
    lines(text)
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(str::to_owned)
        .collect()
}

/// Lee un `.paragraphs`: un párrafo por línea; se ignoran líneas de menos de 2 caracteres.
pub fn parse_paragraphs(text: &str) -> Vec<String> {
    lines(text)
        .map(str::trim)
        .filter(|l| l.chars().count() >= 2)
        .map(str::to_owned)
        .collect()
}

/// Convierte un texto cualquiera (archivo, portapapeles, arrastrar y soltar) en una lista de
/// palabras para Velocidad: sólo letras y marcas combinantes, una palabra por línea.
pub fn words_from_text(text: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut current = String::new();
    for c in text.chars() {
        if c.is_alphabetic() || is_diacritic(c) {
            current.push(c);
        } else if !current.is_empty() {
            words.push(std::mem::take(&mut current));
            if words.len() == MAX_WORDS {
                return words;
            }
        }
    }
    if !current.is_empty() {
        words.push(current);
    }
    words
}

/// Convierte un texto cualquiera en párrafos para Fluidez. Conserva letras, números,
/// espacios y los símbolos que se pueden teclear (`typeable_symbols`, normalmente los de la
/// distribución activa); el resto se vuelve espacio. Cada salto de línea separa párrafos.
pub fn paragraphs_from_text(text: &str, typeable_symbols: &[char]) -> Vec<String> {
    let mut out = Vec::new();
    let text = text.strip_prefix('\u{FEFF}').unwrap_or(text);
    for raw in text.split(['\n', '\r']) {
        let cleaned: String = raw
            .chars()
            .map(|c| {
                if c.is_alphanumeric()
                    || c == ' '
                    || is_diacritic(c)
                    || typeable_symbols.contains(&c)
                {
                    c
                } else {
                    ' '
                }
            })
            .collect();
        let normalized = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
        if normalized.chars().count() >= 2 {
            out.push(normalized);
            if out.len() == MAX_PARAGRAPHS {
                break;
            }
        }
    }
    out
}

/// Escribe una lista de palabras en formato `.words`.
pub fn serialize_words(words: &[String]) -> String {
    let mut out = words.join("\n");
    out.push('\n');
    out
}

/// Escribe una lista de párrafos en formato `.paragraphs`.
pub fn serialize_paragraphs(paragraphs: &[String]) -> String {
    serialize_words(paragraphs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_words_and_paragraphs() {
        assert_eq!(
            parse_words("\u{FEFF}casa\r\n\nperro\n"),
            vec!["casa", "perro"]
        );
        assert_eq!(
            parse_paragraphs("Hola mundo.\nx\n\nOtro párrafo.\n"),
            vec!["Hola mundo.", "Otro párrafo."]
        );
    }

    #[test]
    fn words_from_arbitrary_text() {
        let words = words_from_text("¡Hola, mundo! 123 ñandú-rápido");
        assert_eq!(words, vec!["Hola", "mundo", "ñandú", "rápido"]);
    }

    #[test]
    fn paragraphs_from_arbitrary_text() {
        let symbols = ['.', ',', '!'];
        let text = "Primer   párrafo, con «comillas».\r\n\r\nSegundo! @@ 42\n";
        assert_eq!(
            paragraphs_from_text(text, &symbols),
            vec!["Primer párrafo, con comillas .", "Segundo! 42"]
        );
    }

    #[test]
    fn round_trip_words() {
        let words = vec!["uno".to_owned(), "dos".to_owned()];
        assert_eq!(parse_words(&serialize_words(&words)), words);
    }
}
