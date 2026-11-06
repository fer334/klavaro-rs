// SPDX-License-Identifier: GPL-3.0-or-later
//! Vistas de la Progresión: selección de sección, grilla de lecciones y resultado.

use iced::widget::{Space, button, column, container, operation, row, scrollable, text};
use iced::{Alignment, Element, Length, Task};

use klavaro_core::progression::{
    FIRST_SECTION, FULL_STARS_ACCURACY, LAST_SECTION, LESSONS_PER_SECTION, LessonResult, NextStep,
    SECTION_STARS, SECTION_STEP, SECTIONS, SectionStatus, Slot, TOTAL_STARS, sections,
};

use super::{App, Message, bar_with_back, card, page_title, stat, top_bar};
use crate::sections::{
    LESSON_COLUMNS, SECTION_COLUMNS, SectionView, SectionsView, clock, star_line,
};
use crate::style;
use crate::tutor::Outcome;

/// Identificador del área con las grillas, para seguir a la selección.
const GRID_SCROLL: &str = "sections-grid";

/// Filas de `columns` elementos. Las celdas sobrantes de la última fila quedan vacías, para que
/// todas midan lo mismo.
fn grid(cells: Vec<Element<'_, Message>>, columns: u16) -> Element<'_, Message> {
    let columns = usize::from(columns);
    let mut cells = cells.into_iter().peekable();
    let mut rows = column![].spacing(10);
    while cells.peek().is_some() {
        let mut line = row![].spacing(10);
        for _ in 0..columns {
            line = match cells.next() {
                Some(cell) => line.push(cell),
                None => line.push(Space::new().width(Length::Fill)),
            };
        }
        rows = rows.push(line);
    }
    rows.into()
}

/// Posición vertical de la fila de `index` (de 0 a `last`), de 0 a 1.
fn row_offset(index: u16, last: u16, columns: u16) -> f32 {
    let last_row = last / columns;
    if last_row == 0 {
        0.0
    } else {
        f32::from(index / columns) / f32::from(last_row)
    }
}

impl App {
    /// Velocidad como «75 PPM», en el idioma de la interfaz.
    fn wpm_label(&self, value: &str) -> String {
        self.tr.tr_args("stats-wpm-value", &[("value", value)])
    }

    /// Título de una sección.
    fn section_name(&self, section: u16) -> String {
        self.tr
            .tr_args("sections-section", &[("section", &section.to_string())])
    }

    /// Mantiene a la vista la sección o lección elegida.
    pub(super) fn follow_grid(&self) -> Task<Message> {
        let y = match &self.screen {
            super::Screen::Sections(v) => row_offset(
                (v.cursor - FIRST_SECTION) / SECTION_STEP,
                SECTIONS - 1,
                SECTION_COLUMNS,
            ),
            super::Screen::Section(v) => {
                row_offset(v.cursor - 1, LESSONS_PER_SECTION - 1, LESSON_COLUMNS)
            }
            _ => return Task::none(),
        };
        operation::snap_to(GRID_SCROLL, operation::RelativeOffset { x: 0.0, y })
    }

    /// Tarjeta de la Progresión en el menú.
    pub(super) fn progression_card<'a>(&self) -> Element<'a, Message> {
        let tr = &self.tr;
        let badge = container(text("N").size(18).font(style::BOLD))
            .center(Length::Fixed(40.0))
            .style(style::badge);
        let colors = style::Colors::new(self.dark);
        button(
            row![
                badge,
                column![
                    text(tr.tr("module-progression")).size(18).font(style::BOLD),
                    text(tr.tr("module-progression-tip"))
                        .size(13)
                        .style(style::muted),
                ]
                .spacing(2)
                .width(Length::Fill),
                column![
                    text(format!("★ {}", self.ladder.total_stars()))
                        .size(18)
                        .color(colors.warning),
                    text(self.section_name(self.ladder.current_section()))
                        .size(13)
                        .style(style::muted),
                ]
                .spacing(2)
                .align_x(Alignment::End),
            ]
            .spacing(14)
            .align_y(Alignment::Center),
        )
        .width(Length::Fill)
        .padding(16)
        .style(style::module_card)
        .on_press(Message::OpenSections)
        .into()
    }

    /// Selección de sección.
    pub(super) fn view_sections<'a>(&'a self, v: &'a SectionsView) -> Element<'a, Message> {
        let tr = &self.tr;
        let colors = style::Colors::new(self.dark);
        let cards = sections()
            .map(|section| {
                let status = self.ladder.status(section);
                let (label, color) = match status {
                    SectionStatus::Locked => (tr.tr("sections-locked"), colors.muted),
                    SectionStatus::Available => (tr.tr("sections-available"), colors.primary),
                    SectionStatus::Passed => (tr.tr("sections-approved"), colors.success),
                };
                let stars = format!("★ {}/{SECTION_STARS}", self.ladder.section_stars(section));
                let lessons = tr.tr_args(
                    "sections-lessons-passed",
                    &[
                        ("done", &self.ladder.lessons_passed(section).to_string()),
                        ("total", &LESSONS_PER_SECTION.to_string()),
                    ],
                );
                let content = column![
                    row![
                        text(self.wpm_label(&section.to_string()))
                            .size(20)
                            .font(style::BOLD)
                            .width(Length::Fill),
                        text(label).size(12).color(color),
                    ]
                    .align_y(Alignment::Center),
                    text(stars).size(14).color(colors.warning),
                    text(lessons).size(12).style(style::muted),
                ]
                .spacing(3);
                let mut card = button(content)
                    .width(Length::Fill)
                    .padding(10)
                    .style(style::tile(section == v.cursor));
                if status != SectionStatus::Locked {
                    card = card.on_press(Message::OpenSection(section));
                }
                card.into()
            })
            .collect();
        let total = format!("★ {}/{TOTAL_STARS}", self.ladder.total_stars());
        column![
            top_bar(
                tr,
                page_title(tr.tr("module-progression")),
                [text(total).size(18).color(colors.warning).into()],
            ),
            text(tr.tr_args("sections-intro", &[("goal", &LAST_SECTION.to_string())]))
                .size(14)
                .style(style::muted),
            scrollable(grid(cards, SECTION_COLUMNS))
                .spacing(8)
                .id(GRID_SCROLL)
                .height(Length::Fill),
            text(tr.tr("sections-keys")).size(12).style(style::muted),
        ]
        .spacing(16)
        .into()
    }

    /// Grilla de lecciones de una sección.
    pub(super) fn view_section<'a>(&'a self, v: &'a SectionView) -> Element<'a, Message> {
        let tr = &self.tr;
        let colors = style::Colors::new(self.dark);
        let section = v.section;
        let cards = (1..=LESSONS_PER_SECTION)
            .filter_map(|lesson| Slot::new(section, lesson))
            .map(|slot| {
                let open = self.ladder.lesson_unlocked(slot);
                let best = self
                    .bests
                    .get(&slot)
                    .map_or_else(|| "—".to_owned(), |b| format!("{:.1}", b.wpm));
                let content = column![
                    row![
                        text(slot.lesson().to_string())
                            .size(16)
                            .font(style::BOLD)
                            .width(Length::Fill),
                        text(self.wpm_label(&format!("{:.1}", slot.target_wpm())))
                            .size(12)
                            .style(style::muted),
                    ]
                    .align_y(Alignment::Center),
                    text(star_line(self.ladder.stars(slot)))
                        .size(14)
                        .color(colors.warning),
                    text(tr.tr_args("sections-best", &[("value", &best)]))
                        .size(12)
                        .style(style::muted),
                ]
                .spacing(2);
                let mut tile = button(content)
                    .width(Length::Fill)
                    .padding(8)
                    .style(style::tile(slot.lesson() == v.cursor));
                if open {
                    tile = tile.on_press(Message::StartLesson(slot));
                }
                tile.into()
            })
            .collect();
        let summary = tr.tr_args(
            "sections-lessons-passed",
            &[
                ("done", &self.ladder.lessons_passed(section).to_string()),
                ("total", &LESSONS_PER_SECTION.to_string()),
            ],
        );
        let stars = format!("★ {}/{SECTION_STARS}", self.ladder.section_stars(section));
        let rule = if self.ladder.section_passed(section) {
            text(tr.tr("sections-approved")).color(colors.success)
        } else {
            text(tr.tr("sections-rule")).style(style::muted)
        };
        column![
            bar_with_back(
                &tr.tr("sections-back"),
                page_title(self.section_name(section)),
                [
                    text(summary).size(14).style(style::muted).into(),
                    text(stars).size(18).color(colors.warning).into(),
                ],
            ),
            rule.size(14),
            scrollable(grid(cards, LESSON_COLUMNS))
                .spacing(8)
                .id(GRID_SCROLL)
                .height(Length::Fill),
            text(tr.tr("sections-keys")).size(12).style(style::muted),
        ]
        .spacing(16)
        .into()
    }

    /// Título de una lección en curso.
    pub(super) fn lesson_title(&self, slot: Slot) -> String {
        self.tr.tr_args(
            "sections-lesson-title",
            &[
                ("section", &slot.section().to_string()),
                ("lesson", &slot.lesson().to_string()),
            ],
        )
    }

    /// Pie de una lección en curso: la meta y el tiempo, que se actualiza con cada tecla.
    pub(super) fn lesson_status<'a>(
        &self,
        slot: Slot,
        limit: std::time::Duration,
        elapsed: std::time::Duration,
    ) -> Element<'a, Message> {
        let colors = style::Colors::new(self.dark);
        let time = format!("{} / {}", clock(elapsed), clock(limit));
        let time = if elapsed > limit {
            text(time).color(colors.danger)
        } else {
            text(time)
        };
        row![
            text(self.tr.tr_args(
                "sections-goal",
                &[("value", &format!("{:.1}", slot.target_wpm()))],
            ))
            .size(15)
            .font(style::BOLD),
            time.size(15).font(style::BOLD),
        ]
        .spacing(16)
        .into()
    }

    /// Resultado de una lección: estrellas, medidas y qué sigue.
    pub(super) fn lesson_result<'a>(
        &'a self,
        outcome: &'a Outcome,
        result: LessonResult,
    ) -> Element<'a, Message> {
        let tr = &self.tr;
        let colors = style::Colors::new(self.dark);
        let mark = |ok: bool| if ok { colors.success } else { colors.danger };
        let on_time = outcome.stats.elapsed <= result.time_limit;
        let stats = row![
            stat(
                &tr.tr("sections-score"),
                format!("{:.2}", outcome.stats.score()),
                None,
            ),
            stat(
                &tr.tr("stats-wpm"),
                self.wpm_label(&format!("{:.1}", result.speed)),
                Some(mark(result.speed >= result.target_wpm)),
            ),
            stat(
                &tr.tr("stats-accuracy"),
                tr.tr_args(
                    "stats-percent",
                    &[("value", &format!("{:.1}", outcome.stats.accuracy))],
                ),
                Some(mark(outcome.stats.accuracy >= FULL_STARS_ACCURACY)),
            ),
            stat(
                &tr.tr("stats-elapsed"),
                format!(
                    "{} / {}",
                    clock(outcome.stats.elapsed),
                    clock(result.time_limit)
                ),
                Some(mark(on_time)),
            ),
        ]
        .spacing(12);
        let hint = match self.ladder.next_step(result.slot) {
            NextStep::Open(_) => "sections-end-next",
            NextStep::Completed => "sections-end-completed",
            NextStep::NeedsPassStars | NextStep::NeedsSection => "sections-end-retry",
        };
        let mut summary =
            column![text(star_line(result.stars)).size(30).color(colors.warning),].spacing(4);
        if let Some(note) = &self.note {
            summary = summary.push(text(note.clone()).size(15));
        }
        summary = summary.push(text(tr.tr(hint)).size(13).style(style::muted));
        column![stats, card(summary)].spacing(12).into()
    }
}
