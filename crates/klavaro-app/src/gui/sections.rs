// SPDX-License-Identifier: GPL-3.0-or-later
//! Vistas de la Progresión: selección de sección, grilla de lecciones y resultado.

use iced::widget::{
    Space, button, column, container, operation, progress_bar, row, scrollable, text,
};
use iced::{Alignment, Element, Length, Task};

use klavaro_core::progression::{
    FIRST_SECTION, FULL_STARS_ACCURACY, LAST_SECTION, LESSONS_PER_SECTION, LessonResult, MAX_STARS,
    Milestone, NextStep, SECTION_STARS, SECTION_STEP, SECTIONS, SectionStatus, Slot, TOTAL_STARS,
    sections,
};

use klavaro_data::catalog::CatalogEntry;

use super::{App, Message, ResetStep, action, bar_with_back, card, page_title, stat, top_bar};
use crate::sections::{
    LESSON_COLUMNS, SECTION_COLUMNS, SectionView, SectionsView, clock, source_name, star_line,
};
use crate::style;
use crate::tutor::{Outcome, Tutor};

/// Licencia de los textos.
const LICENSE_NAME: &str = "CC BY-SA 4.0";
const LICENSE_URL: &str = "https://creativecommons.org/licenses/by-sa/4.0/";

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
                    SectionStatus::Skipped => (tr.tr("sections-skipped"), colors.muted),
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
                button(content)
                    .width(Length::Fill)
                    .padding(10)
                    .style(style::tile(section == v.cursor))
                    .on_press(Message::OpenSection(section))
                    .into()
            })
            .collect();
        let total = format!("★ {}/{TOTAL_STARS}", self.ladder.total_stars());
        column![
            top_bar(
                tr,
                page_title(tr.tr("module-progression")),
                [
                    text(total).size(18).color(colors.warning).into(),
                    action(tr.tr("sections-credits"), Message::OpenCredits),
                ],
            ),
            text(tr.tr_args("sections-intro", &[("goal", &LAST_SECTION.to_string())]))
                .size(14)
                .style(style::muted),
            self.overall_progress(),
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
            text(tr.tr(if self.ladder.section_cleared(section) {
                "sections-skipped-rule"
            } else if self.ladder.section_unlocked(section) {
                "sections-rule"
            } else {
                "sections-locked-rule"
            }))
            .style(style::muted)
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
            self.exam_row(section),
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
        missing: &[char],
    ) -> Element<'a, Message> {
        let colors = style::Colors::new(self.dark);
        let time = format!("{} / {}", clock(elapsed), clock(limit));
        let time = if elapsed > limit {
            text(time).color(colors.danger)
        } else {
            text(time)
        };
        let mut line = row![
            text(self.tr.tr_args(
                "sections-goal",
                &[("value", &format!("{:.1}", slot.target_wpm()))],
            ))
            .size(15)
            .font(style::BOLD),
            time.size(15).font(style::BOLD),
        ]
        .spacing(16);
        if !missing.is_empty() {
            let keys: String = missing.iter().collect();
            line = line.push(
                text(self.tr.tr_args("sections-missing-keys", &[("keys", &keys)]))
                    .size(13)
                    .color(colors.warning),
            );
        }
        line.into()
    }

    /// Resultado de una lección: estrellas, medidas y qué sigue.
    pub(super) fn lesson_result<'a>(
        &'a self,
        tutor: &'a Tutor,
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
        let weak = tutor.weak_keys();
        if !weak.is_empty() {
            let keys: Vec<String> = weak.iter().map(|&key| self.key_name(key)).collect();
            summary = summary.push(
                text(tr.tr_args("sections-weak-keys", &[("keys", &keys.join(" "))]))
                    .size(13)
                    .color(colors.warning),
            );
        }
        let mut buttons = row![action(
            tr.tr("menu-progress"),
            Message::LessonProgress(result.slot),
        )]
        .spacing(8);
        if tutor.practice_suggested() {
            buttons = buttons.push(action(
                tr.tr("sections-practice"),
                Message::PracticeWeakKeys,
            ));
        }
        summary = summary.push(buttons);
        if let Some(source) = &outcome.source {
            summary = summary.push(self.source_link(source));
        }
        column![stats, card(summary)].spacing(12).into()
    }

    /// Enlace al artículo del que salió un texto.
    fn source_link<'a>(&self, source: &CatalogEntry) -> Element<'a, Message> {
        let label = self.tr.tr_args(
            "sections-source",
            &[
                ("title", &source.title),
                ("source", source_name(&source.source)),
                ("license", LICENSE_NAME),
            ],
        );
        button(text(label).size(12))
            .style(style::ghost)
            .padding([2, 0])
            .on_press(Message::OpenLink(source.url()))
            .into()
    }

    /// Origen y licencia de los textos: cada lección con un enlace a su artículo.
    pub(super) fn view_credits<'a>(&'a self, entries: &'a [CatalogEntry]) -> Element<'a, Message> {
        let tr = &self.tr;
        let lines = entries.iter().map(|entry| {
            let label = format!(
                "{}.{}  {} ({})",
                entry.slot.section(),
                entry.slot.lesson(),
                entry.title,
                source_name(&entry.source)
            );
            button(text(label).size(13))
                .style(style::ghost)
                .padding([3, 8])
                .on_press(Message::OpenLink(entry.url()))
                .into()
        });
        column![
            bar_with_back(
                &tr.tr("sections-back"),
                page_title(tr.tr("sections-credits")),
                [action(
                    tr.tr_args("sections-license", &[("license", LICENSE_NAME)]),
                    Message::OpenLink(LICENSE_URL.to_owned()),
                )],
            ),
            text(tr.tr_args("sections-credits-intro", &[("license", LICENSE_NAME)]))
                .size(14)
                .style(style::muted),
            scrollable(column(lines).spacing(2).width(Length::Fill)).height(Length::Fill),
        ]
        .spacing(16)
        .into()
    }

    /// Botón del examen de una sección que todavía no quedó atrás.
    fn exam_row<'a>(&self, section: u16) -> Element<'a, Message> {
        if !self.ladder.exam_available(section) {
            return Space::new().height(Length::Shrink).into();
        }
        row![
            button(text(self.tr.tr("sections-exam")))
                .style(style::primary)
                .padding([8, 14])
                .on_press(Message::StartExam(section)),
            text(self.tr.tr_args(
                "sections-exam-tip",
                &[
                    ("goal", &section.to_string()),
                    ("accuracy", &format!("{FULL_STARS_ACCURACY:.0}")),
                ],
            ))
            .size(13)
            .style(style::muted),
        ]
        .spacing(12)
        .align_y(Alignment::Center)
        .into()
    }

    /// Nombre de una tecla para mostrar.
    fn key_name(&self, key: char) -> String {
        if key == ' ' {
            self.tr.tr("sections-space")
        } else {
            key.to_string()
        }
    }

    /// Avance hacia la última meta.
    fn overall_progress<'a>(&self) -> Element<'a, Message> {
        let total = u32::from(SECTIONS) * u32::from(LESSONS_PER_SECTION);
        let passed = self.ladder.total_lessons_passed();
        let best = self.ladder.highest_passed().map_or_else(
            || "—".to_owned(),
            |section| self.wpm_label(&section.to_string()),
        );
        #[allow(clippy::cast_precision_loss)]
        let ratio = passed as f32 / total as f32;
        column![
            progress_bar(0.0..=1.0, ratio)
                .girth(6)
                .style(style::progress),
            text(self.tr.tr_args(
                "sections-overall",
                &[
                    ("done", &passed.to_string()),
                    ("total", &total.to_string()),
                    ("best", &best),
                    ("goal", &self.wpm_label(&LAST_SECTION.to_string())),
                ],
            ))
            .size(13)
            .style(style::muted),
        ]
        .spacing(6)
        .into()
    }

    /// Estrellas de cada sección, como barras.
    pub(super) fn stars_chart<'a>(&self) -> Element<'a, Message> {
        let colors = style::Colors::new(self.dark);
        let lines = sections().map(|section| {
            let stars = self.ladder.section_stars(section);
            #[allow(clippy::cast_precision_loss)]
            let ratio = stars as f32 / SECTION_STARS as f32;
            row![
                text(self.wpm_label(&section.to_string()))
                    .size(13)
                    .width(Length::Fixed(80.0)),
                progress_bar(0.0..=1.0, ratio)
                    .girth(10)
                    .style(style::progress),
                text(format!("★ {stars}/{SECTION_STARS}"))
                    .size(13)
                    .color(colors.warning)
                    .width(Length::Fixed(90.0)),
            ]
            .spacing(10)
            .align_y(Alignment::Center)
            .into()
        });
        scrollable(column(lines).spacing(8).padding([0, 12])).into()
    }

    /// Pantalla al aprobar una sección o completar la Progresión.
    pub(super) fn view_milestone<'a>(&self, milestone: Milestone) -> Element<'a, Message> {
        let tr = &self.tr;
        let colors = style::Colors::new(self.dark);
        let (title, detail) = match milestone {
            Milestone::SectionPassed(section) => (
                tr.tr_args("sections-passed", &[("section", &section.to_string())]),
                tr.tr_args(
                    "sections-next-goal",
                    &[("section", &(section + SECTION_STEP).to_string())],
                ),
            ),
            Milestone::Completed => (
                tr.tr("sections-completed"),
                tr.tr_args(
                    "sections-completed-detail",
                    &[("goal", &LAST_SECTION.to_string())],
                ),
            ),
        };
        let content = column![
            text(star_line(MAX_STARS)).size(44).color(colors.warning),
            text(title).size(26).font(style::BOLD).center(),
            text(detail).size(16).style(style::muted).center(),
            text(format!("★ {}/{TOTAL_STARS}", self.ladder.total_stars()))
                .size(18)
                .color(colors.warning),
            button(text(tr.tr("sections-continue")))
                .style(style::primary)
                .padding([10, 20])
                .on_press(Message::MilestoneContinue),
            text(tr.tr("sections-milestone-keys"))
                .size(12)
                .style(style::muted),
        ]
        .spacing(16)
        .align_x(Alignment::Center);
        container(content).center(Length::Fill).into()
    }

    /// Tarjeta de Preferencias para reiniciar la Progresión.
    pub(super) fn reset_card<'a>(&self) -> Element<'a, Message> {
        let tr = &self.tr;
        let colors = style::Colors::new(self.dark);
        let controls: Element<'a, Message> = match self.reset {
            ResetStep::Idle => action(tr.tr("sections-reset"), Message::ResetAsk),
            ResetStep::Confirming => row![
                text(tr.tr("sections-reset-confirm"))
                    .size(14)
                    .color(colors.danger),
                action(tr.tr("sections-reset-yes"), Message::ResetConfirm),
                action(tr.tr("sections-reset-no"), Message::ResetCancel),
            ]
            .spacing(10)
            .align_y(Alignment::Center)
            .into(),
            ResetStep::Done => text(tr.tr("sections-reset-done"))
                .size(14)
                .color(colors.success)
                .into(),
        };
        card(
            column![
                text(tr.tr("module-progression")).size(18).font(style::BOLD),
                text(tr.tr("sections-reset-tip"))
                    .size(12)
                    .style(style::muted),
                controls,
            ]
            .spacing(14),
        )
        .into()
    }
}
