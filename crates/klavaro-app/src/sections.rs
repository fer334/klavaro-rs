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
/// Idioma de los ejercicios: por ahora la Progresión es sólo en inglés, sea cual sea el idioma
/// de los textos elegido para los demás módulos.
pub(crate) const LESSONS_LANGUAGE: &str = "C";

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

/// Sección siguiente o anterior en el filtro de los gráficos, donde 0 quiere decir todas.
pub(crate) fn stepped_section(section: u16, delta: i32) -> u16 {
    let index = if section == 0 {
        0
    } else {
        i32::from((section - FIRST_SECTION) / SECTION_STEP) + 1
    };
    let last = i32::from((LAST_SECTION - FIRST_SECTION) / SECTION_STEP) + 1;
    match u16::try_from((index + delta).clamp(0, last)).unwrap_or(0) {
        0 => 0,
        moved => FIRST_SECTION + (moved - 1) * SECTION_STEP,
    }
}

/// Estado de la Progresión a partir de los mejores resultados y del examen aprobado.
pub(crate) fn ladder(bests: &[LessonBest], exam_section: u16) -> Ladder {
    Ladder::new(bests.iter().map(|b| (b.slot, b.stars))).with_exam(exam_section)
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

/// Nombre de la wiki de origen de un texto.
pub(crate) fn source_name(source: &str) -> &'static str {
    if source == "simple" {
        "Simple English Wikipedia"
    } else {
        "Wikipedia"
    }
}

#[cfg(test)]
mod tests {
    use klavaro_core::progression::sections;

    use super::*;
    use crate::assets;

    #[test]
    fn every_lesson_has_a_text_in_the_embedded_catalog() {
        let catalog = assets::lessons_catalog(LESSONS_LANGUAGE).expect("catálogo");
        assert_eq!(catalog.len(), 510);
        for section in sections() {
            for lesson in 1..=LESSONS_PER_SECTION {
                let slot = Slot::new(section, lesson).expect("lección");
                assert!(catalog.iter().any(|entry| entry.slot == slot), "{slot:?}");
            }
        }
        let first = assets::lesson(LESSONS_LANGUAGE, Slot::FIRST).expect("texto");
        assert_eq!(first.slot, Slot::FIRST);
    }

    #[test]
    fn a_language_without_a_catalog_is_an_error() {
        assert!(assets::lesson("es", Slot::FIRST).is_err());
    }

    #[test]
    fn steps_through_the_section_filter() {
        assert_eq!(stepped_section(0, 1), 20);
        assert_eq!(stepped_section(20, 1), 25);
        assert_eq!(stepped_section(20, -1), 0);
        assert_eq!(stepped_section(0, -1), 0);
        assert_eq!(stepped_section(100, 1), 100);
        assert_eq!(stepped_section(100, -1), 95);
    }

    #[test]
    fn names_the_source_wiki() {
        assert_eq!(source_name("simple"), "Simple English Wikipedia");
        assert_eq!(source_name("en"), "Wikipedia");
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
