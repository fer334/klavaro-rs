// SPDX-License-Identifier: GPL-3.0-or-later
//! Archivos de distribución de teclado (`*.kbd`).
//!
//! Ocho líneas UTF-8: las cuatro filas de la capa normal y las cuatro de la capa con Shift,
//! de la fila de números a la inferior. Cada posición es un carácter; un espacio indica que
//! no hay tecla. Las líneas cortas se completan con espacios.

use klavaro_core::keyboard::{KEY_LINE_LEN, KeyboardLayout, Layer, ROWS};

use crate::{ParseError, ParseErrorKind, lines};

/// Lee un `.kbd`. `name` es el nombre del archivo sin extensión.
///
/// # Errors
///
/// Falla si hay menos de 8 líneas o alguna fila supera [`KEY_LINE_LEN`] caracteres.
pub fn parse(name: &str, text: &str) -> Result<KeyboardLayout, ParseError> {
    let rows: Vec<&str> = lines(text).collect();
    let needed = 2 * ROWS;
    let found = rows.iter().take(needed).count();
    if found < needed || rows.iter().take(needed).all(|r| r.trim().is_empty()) {
        return Err(ParseError {
            line: None,
            kind: ParseErrorKind::TooFewLines {
                expected: needed,
                found,
            },
        });
    }
    let mut normal: Layer = [[' '; KEY_LINE_LEN]; ROWS];
    let mut shifted: Layer = [[' '; KEY_LINE_LEN]; ROWS];
    for (i, row) in rows.iter().take(needed).enumerate() {
        let chars: Vec<char> = row.chars().collect();
        if chars.len() > KEY_LINE_LEN {
            return Err(ParseError::at(
                i + 1,
                ParseErrorKind::LineTooLong {
                    max: KEY_LINE_LEN,
                    found: chars.len(),
                },
            ));
        }
        let target = if i < ROWS {
            &mut normal[i]
        } else {
            &mut shifted[i - ROWS]
        };
        for (slot, c) in target.iter_mut().zip(chars) {
            *slot = if c.is_control() { ' ' } else { c };
        }
    }
    Ok(KeyboardLayout::new(name, normal, shifted))
}

/// Escribe un `.kbd` (sin espacios finales en cada fila).
pub fn serialize(layout: &KeyboardLayout) -> String {
    use klavaro_core::keyboard::Level;
    let mut out = String::new();
    for level in [Level::Normal, Level::Shifted] {
        for row in layout.layer(level) {
            let line: String = row.iter().collect();
            out.push_str(line.trim_end_matches(' '));
            out.push('\n');
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use klavaro_core::keyboard::{KeyPos, Level};

    const QWERTY_US: &str = "`1234567890-=\nqwertyuiop[]\\\nasdfghjkl;'\nzxcvbnm,./\n\
                             ~!@#$%^&*()_+\nQWERTYUIOP{}|\nASDFGHJKL:\"\nZXCVBNM<>?\n";

    #[test]
    fn parses_qwerty() {
        let k = parse("qwerty_us", QWERTY_US).expect("válido");
        assert_eq!(k.name(), "qwerty_us");
        assert_eq!(k.key(KeyPos { row: 2, col: 0 }, Level::Normal), Some('a'));
        assert_eq!(k.key(KeyPos { row: 3, col: 9 }, Level::Shifted), Some('?'));
        assert_eq!(k.key(KeyPos { row: 3, col: 12 }, Level::Normal), None);
    }

    #[test]
    fn round_trip() {
        let k = parse("qwerty_us", QWERTY_US).expect("válido");
        assert_eq!(serialize(&k), QWERTY_US);
        assert_eq!(parse("qwerty_us", &serialize(&k)).expect("válido"), k);
    }

    #[test]
    fn accepts_crlf_and_unicode() {
        let text = QWERTY_US.replace('\n', "\r\n").replace(";'", "ñ´");
        let k = parse("qwerty_es", &text).expect("válido");
        assert_eq!(k.key(KeyPos { row: 2, col: 9 }, Level::Normal), Some('ñ'));
    }

    #[test]
    fn rejects_short_or_long_files() {
        assert!(matches!(
            parse("x", "abc\n").map_err(|e| e.kind),
            Err(ParseErrorKind::TooFewLines { .. })
        ));
        let long = format!("{}\n", "a".repeat(15)).repeat(8);
        let err = parse("x", &long).expect_err("fila larga");
        assert_eq!(err.line, Some(1));
    }
}
