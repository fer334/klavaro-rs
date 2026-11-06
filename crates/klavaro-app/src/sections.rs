// SPDX-License-Identifier: GPL-3.0-or-later
//! Pantallas de la Progresión, independientes de iced: qué sección o lección está elegida,
//! cómo se mueve la selección y los textos de cada lección.

use std::time::Duration;

use klavaro_core::progression::{
    FIRST_SECTION, LAST_SECTION, LESSONS_PER_SECTION, Ladder, MAX_STARS, SECTION_STEP, Slot,
};
use klavaro_store::LessonBest;

use crate::tutor::Direction;

/// Secciones por fila en la selección de sección.
pub(crate) const SECTION_COLUMNS: u16 = 4;
/// Lecciones por fila en la grilla de una sección.
pub(crate) const LESSON_COLUMNS: u16 = 6;
/// Identificador de los textos provisionales en el registro de intentos.
pub(crate) const PROVISIONAL_TEXT_ID: &str = "provisional";
/// Tolerancia en el largo de un texto, respecto del que dura 60 s a la meta de la lección.
pub(crate) const LENGTH_TOLERANCE: f64 = 0.15;

/// Selección de sección.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SectionsView {
    /// Sección elegida con el teclado.
    pub(crate) cursor: u16,
}

impl SectionsView {
    /// Empieza en la sección en la que toca jugar.
    pub(crate) fn new(ladder: &Ladder) -> Self {
        Self {
            cursor: ladder.current_section(),
        }
    }

    /// Mueve la selección. En el borde no cambia.
    pub(crate) fn step(&mut self, direction: Direction) {
        let index = (self.cursor - FIRST_SECTION) / SECTION_STEP;
        let last = (LAST_SECTION - FIRST_SECTION) / SECTION_STEP;
        if let Some(moved) = moved(index, last, SECTION_COLUMNS, direction) {
            self.cursor = FIRST_SECTION + moved * SECTION_STEP;
        }
    }
}

/// Grilla de lecciones de una sección.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SectionView {
    /// Meta de la sección.
    pub(crate) section: u16,
    /// Lección elegida con el teclado (de 1 a 30).
    pub(crate) cursor: u16,
}

impl SectionView {
    /// Empieza en la lección en la que toca jugar.
    pub(crate) fn new(section: u16, ladder: &Ladder) -> Self {
        let cursor = ladder.current_lesson(section).map_or(1, Slot::lesson);
        Self { section, cursor }
    }

    /// Lección elegida.
    pub(crate) fn slot(self) -> Option<Slot> {
        Slot::new(self.section, self.cursor)
    }

    /// Mueve la selección. En el borde no cambia.
    pub(crate) fn step(&mut self, direction: Direction) {
        if let Some(moved) = moved(
            self.cursor - 1,
            LESSONS_PER_SECTION - 1,
            LESSON_COLUMNS,
            direction,
        ) {
            self.cursor = moved + 1;
        }
    }
}

/// Posición después de moverse en una grilla de `columns` columnas con posiciones de `0` a
/// `last`. `None` si se sale de la grilla.
fn moved(index: u16, last: u16, columns: u16, direction: Direction) -> Option<u16> {
    let target = match direction {
        Direction::Left if index.is_multiple_of(columns) => return None,
        Direction::Right if (index + 1).is_multiple_of(columns) => return None,
        Direction::Left => index.checked_sub(1),
        Direction::Right => Some(index + 1),
        Direction::Up => index.checked_sub(columns),
        Direction::Down => Some(index + columns),
    };
    target.filter(|&t| t <= last)
}

/// Estado de la Progresión a partir de los mejores resultados.
pub(crate) fn ladder(bests: &[LessonBest]) -> Ladder {
    Ladder::new(bests.iter().map(|b| (b.slot, b.stars)))
}

/// Estrellas llenas y vacías (`★★★☆☆`).
pub(crate) fn star_line(stars: u8) -> String {
    let full = stars.min(MAX_STARS);
    let mut line = "★".repeat(usize::from(full));
    line.push_str(&"☆".repeat(usize::from(MAX_STARS - full)));
    line
}

/// Duración como `mm:ss`, redondeada al segundo.
pub(crate) fn clock(d: Duration) -> String {
    let secs = d.as_millis().saturating_add(500) / 1000;
    format!("{:02}:{:02}", secs / 60, secs % 60)
}

/// Texto provisional de una lección: oraciones de los párrafos del idioma, siempre las mismas
/// para cada lección, con el largo que le corresponde. Se reemplaza por el catálogo definitivo.
///
/// Devuelve `None` si no hay párrafos.
pub(crate) fn provisional_text(paragraphs: &[String], slot: Slot) -> Option<String> {
    let sentences: Vec<&str> = paragraphs
        .iter()
        .flat_map(|p| p.split_inclusive(". "))
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    if sentences.is_empty() {
        return None;
    }
    let target = slot.target_chars();
    let min = target * (1.0 - LENGTH_TOLERANCE);
    let max = target * (1.0 + LENGTH_TOLERANCE);
    let start = usize::from(slot.section()) * 31 + usize::from(slot.lesson()) * 17;
    let mut text = String::new();
    for sentence in sentences.iter().cycle().skip(start % sentences.len()) {
        if !text.is_empty() {
            text.push(' ');
        }
        text.push_str(sentence);
        if length(&text) >= min {
            break;
        }
    }
    if length(&text) > max {
        text = shortened(&text, max);
    }
    Some(text)
}

fn length(text: &str) -> f64 {
    f64::from(u32::try_from(text.chars().count()).unwrap_or(u32::MAX))
}

/// Corta en el último espacio que entra en `max` caracteres y termina con punto.
fn shortened(text: &str, max: f64) -> String {
    // `max` es positivo y chico: el redondeo hacia abajo es justo lo que se busca.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let room = max as usize;
    let cut: String = text.chars().take(room.saturating_sub(1)).collect();
    let cut = cut.rsplit_once(' ').map_or(cut.as_str(), |(head, _)| head);
    let mut out = cut
        .trim_end_matches(|c: char| !c.is_alphanumeric())
        .to_owned();
    out.push('.');
    out
}

#[cfg(test)]
mod tests {
    use klavaro_core::progression::sections;

    use super::*;
    use crate::assets;

    #[test]
    fn provisional_texts_fit_every_lesson() {
        for language in ["es", "C"] {
            let paragraphs = assets::paragraphs(language).expect("párrafos");
            for section in sections() {
                for lesson in 1..=LESSONS_PER_SECTION {
                    let slot = Slot::new(section, lesson).expect("lección");
                    let text = provisional_text(&paragraphs, slot).expect("texto");
                    let (chars, target) = (length(&text), slot.target_chars());
                    assert!(
                        chars >= target * (1.0 - LENGTH_TOLERANCE)
                            && chars <= target * (1.0 + LENGTH_TOLERANCE),
                        "{language} {slot:?}: {chars} caracteres, se pedían {target}"
                    );
                    assert!(!text.contains('\n'), "{slot:?}");
                    assert_eq!(text, provisional_text(&paragraphs, slot).expect("texto"));
                }
            }
        }
    }

    #[test]
    fn provisional_text_works_in_every_language() {
        let slot = Slot::new(20, 1).expect("lección");
        for language in assets::languages() {
            let paragraphs = assets::paragraphs(&language).expect("párrafos");
            assert!(
                provisional_text(&paragraphs, slot).is_some_and(|t| !t.is_empty()),
                "{language}"
            );
        }
        assert_eq!(provisional_text(&[], slot), None);
    }

    #[test]
    fn section_cursor_moves_in_the_grid() {
        let mut v = SectionsView { cursor: 20 };
        v.step(Direction::Left);
        v.step(Direction::Up);
        assert_eq!(v.cursor, 20);
        v.step(Direction::Right);
        assert_eq!(v.cursor, 25);
        v.step(Direction::Down);
        assert_eq!(v.cursor, 45);
        v.cursor = 35;
        v.step(Direction::Right);
        assert_eq!(v.cursor, 35, "no pasa a la fila siguiente");
        v.cursor = 95;
        v.step(Direction::Down);
        assert_eq!(v.cursor, 95, "no hay nada debajo");
        v.cursor = 80;
        v.step(Direction::Down);
        assert_eq!(v.cursor, 100);
        v.step(Direction::Right);
        assert_eq!(v.cursor, 100);
    }

    #[test]
    fn lesson_cursor_moves_in_the_grid() {
        let mut v = SectionView {
            section: 75,
            cursor: 1,
        };
        v.step(Direction::Up);
        assert_eq!(v.cursor, 1);
        v.step(Direction::Down);
        assert_eq!(v.cursor, 7);
        v.step(Direction::Left);
        assert_eq!(v.cursor, 7);
        v.cursor = 30;
        v.step(Direction::Right);
        v.step(Direction::Down);
        assert_eq!(v.cursor, 30);
        v.step(Direction::Up);
        assert_eq!(v.cursor, 24);
        assert_eq!(v.slot(), Slot::new(75, 24));
    }

    #[test]
    fn views_start_where_the_player_is() {
        let ladder = Ladder::new([(Slot::FIRST, 3)]);
        assert_eq!(SectionsView::new(&ladder).cursor, 20);
        assert_eq!(SectionView::new(20, &ladder).cursor, 2);
        assert_eq!(SectionView::new(25, &ladder).cursor, 1);
    }

    #[test]
    fn formats_stars_and_time() {
        assert_eq!(star_line(0), "☆☆☆☆☆");
        assert_eq!(star_line(3), "★★★☆☆");
        assert_eq!(star_line(5), "★★★★★");
        assert_eq!(clock(Duration::from_secs(0)), "00:00");
        assert_eq!(clock(Duration::from_millis(61_600)), "01:02");
    }
}
