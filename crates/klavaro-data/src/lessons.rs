// SPDX-License-Identifier: GPL-3.0-or-later
//! Lecciones del curso básico y mapa de dedos.

use klavaro_core::generators::basic::LessonMask;
use klavaro_core::keyboard::{Finger, FingerMap, KEY_LINE_LEN, ROWS};

use crate::{ParseError, ParseErrorKind, lines};

/// Líneas por lección en `basic_lessons.txt`: título, 4 filas normales, separador, 4 filas
/// con Shift y separador.
pub const LINES_PER_LESSON: usize = 11;

/// Lee `basic_lessons.txt`. Cada lección es un bloque de [`LINES_PER_LESSON`] líneas donde
/// un `1` marca una tecla incluida. Un bloque incompleto al final se ignora.
///
/// # Errors
///
/// Falla si una fila de máscara tiene caracteres distintos de `0`, `1` o espacio.
pub fn parse_basic_lessons(text: &str) -> Result<Vec<LessonMask>, ParseError> {
    let all: Vec<&str> = lines(text).collect();
    let mut lessons = Vec::new();
    for (block_index, block) in all.chunks(LINES_PER_LESSON).enumerate() {
        if block.len() < LINES_PER_LESSON - 1 {
            break;
        }
        let base = block_index * LINES_PER_LESSON;
        let mut mask = LessonMask::default();
        for r in 0..ROWS {
            mask.normal[r] = parse_mask_row(block[1 + r], base + 2 + r)?;
            mask.shifted[r] = parse_mask_row(block[6 + r], base + 7 + r)?;
        }
        lessons.push(mask);
    }
    Ok(lessons)
}

fn parse_mask_row(row: &str, line_no: usize) -> Result<[bool; KEY_LINE_LEN], ParseError> {
    let mut out = [false; KEY_LINE_LEN];
    for (i, c) in row.chars().enumerate() {
        match c {
            '1' if i < KEY_LINE_LEN => out[i] = true,
            // Columnas de más se ignoran, como en el original.
            '0' | '1' | ' ' | '\t' => {}
            other => return Err(ParseError::at(line_no, ParseErrorKind::InvalidChar(other))),
        }
    }
    Ok(out)
}

/// Lee `fingers_position.txt`: 4 filas con un dígito 1–9 por tecla (5 = pulgares). Otros
/// caracteres marcan posiciones sin dedo asignado.
///
/// # Errors
///
/// Falla si hay menos de 4 filas.
pub fn parse_fingers(text: &str) -> Result<FingerMap, ParseError> {
    let rows: Vec<&str> = lines(text).filter(|l| !l.trim().is_empty()).collect();
    if rows.len() < ROWS {
        return Err(ParseError {
            line: None,
            kind: ParseErrorKind::TooFewLines {
                expected: ROWS,
                found: rows.len(),
            },
        });
    }
    let mut map = [[None; KEY_LINE_LEN]; ROWS];
    for (r, row) in rows.iter().take(ROWS).enumerate() {
        for (c, ch) in row.chars().take(KEY_LINE_LEN).enumerate() {
            map[r][c] = Finger::from_digit(ch);
        }
    }
    Ok(FingerMap::new(map))
}

#[cfg(test)]
mod tests {
    use super::*;
    use klavaro_core::keyboard::KeyPos;

    fn block(title: &str, normal_row2: &str) -> String {
        let zero = "00000000000000";
        format!(
            "{title}\n{zero}\n{zero}\n{normal_row2}\n{zero}\n\n{zero}\n{zero}\n{zero}\n{zero}\n\n"
        )
    }

    #[test]
    fn parses_lesson_blocks() {
        let text = block("Lesson 01", "00010010000000") + &block("Lesson 02", "11110011110000");
        let lessons = parse_basic_lessons(&text).expect("válido");
        assert_eq!(lessons.len(), 2);
        assert!(lessons[0].normal[2][3] && lessons[0].normal[2][6]);
        assert_eq!(lessons[0].normal[2].iter().filter(|&&b| b).count(), 2);
        assert_eq!(lessons[1].normal[2].iter().filter(|&&b| b).count(), 8);
    }

    #[test]
    fn rejects_garbage_in_masks() {
        let text = block("Lesson 01", "000x0000000000");
        let err = parse_basic_lessons(&text).expect_err("inválido");
        assert_eq!(err.line, Some(4));
        assert_eq!(err.kind, ParseErrorKind::InvalidChar('x'));
    }

    #[test]
    fn parses_fingers() {
        let text = "11234456778999\n1234456778999\n1234456778999\n1234456778999\n";
        let map = parse_fingers(text).expect("válido");
        assert_eq!(
            map.finger_at(KeyPos { row: 2, col: 0 }),
            Some(Finger::LeftPinky)
        );
        assert_eq!(
            map.finger_at(KeyPos { row: 2, col: 4 }),
            Some(Finger::LeftIndex)
        );
        assert!(parse_fingers("123\n").is_err());
    }
}
