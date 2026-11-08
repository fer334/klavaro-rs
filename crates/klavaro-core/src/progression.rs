// SPDX-License-Identifier: GPL-3.0-or-later
//! Progresión: secciones de velocidad con metas que suben lección a lección.
//!
//! Cada sección lleva el nombre de la meta a la que llega (20, 25… 100 PPM) y tiene 30
//! lecciones. Los 5 PPM que separan dos secciones se reparten en partes iguales, así que cada
//! lección pide 1/6 de PPM más que la anterior. Al terminar una lección se ganan de 1 a 5
//! estrellas, y las estrellas abren las lecciones y las secciones siguientes.
//!
//! Quien ya escribe rápido puede rendir el examen de cualquier sección: es su lección 30, y con 5
//! estrellas da por superadas esa sección y las anteriores, sin regalar sus estrellas.

use std::collections::BTreeMap;
use std::time::Duration;

use crate::metrics::Stats;

/// Primera sección, en PPM.
pub const FIRST_SECTION: u16 = 20;
/// Última sección, en PPM.
pub const LAST_SECTION: u16 = 100;
/// Diferencia de meta entre dos secciones, en PPM.
pub const SECTION_STEP: u16 = 5;
/// Lecciones de cada sección.
pub const LESSONS_PER_SECTION: u16 = 30;
/// Cantidad de secciones.
pub const SECTIONS: u16 = (LAST_SECTION - FIRST_SECTION) / SECTION_STEP + 1;
/// Máximo de estrellas de una lección.
pub const MAX_STARS: u8 = 5;
/// Estrellas posibles en una sección.
pub const SECTION_STARS: u32 = LESSONS_PER_SECTION as u32 * MAX_STARS as u32;
/// Estrellas posibles en toda la Progresión.
pub const TOTAL_STARS: u32 = SECTIONS as u32 * SECTION_STARS;
/// Estrellas necesarias para abrir la lección siguiente.
pub const PASS_STARS: u8 = 3;
/// Precisión necesaria (%) para poder ganar [`MAX_STARS`] estrellas.
pub const FULL_STARS_ACCURACY: f64 = 95.0;
/// Por debajo de esta precisión (%) no se ganan más de [`LOW_ACCURACY_STARS`] estrellas.
pub const LOW_ACCURACY: f64 = 90.0;
/// Máximo de estrellas por debajo de [`LOW_ACCURACY`].
pub const LOW_ACCURACY_STARS: u8 = 2;
/// Duración aproximada de una lección escrita a su meta.
pub const SECONDS_PER_LESSON: f64 = 60.0;

/// Las secciones, de la primera a la última.
pub fn sections() -> impl Iterator<Item = u16> {
    (FIRST_SECTION..=LAST_SECTION).step_by(usize::from(SECTION_STEP))
}

/// Si `section` es la meta de una sección.
pub const fn is_section(section: u16) -> bool {
    section >= FIRST_SECTION
        && section <= LAST_SECTION
        && (section - FIRST_SECTION).is_multiple_of(SECTION_STEP)
}

/// Una lección de la Progresión.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Slot {
    section: u16,
    lesson: u16,
}

impl Slot {
    /// La primera lección de la primera sección.
    pub const FIRST: Self = Self {
        section: FIRST_SECTION,
        lesson: 1,
    };

    /// Lección `lesson` (de 1 a 30) de la sección `section`, si existe.
    pub const fn new(section: u16, lesson: u16) -> Option<Self> {
        if is_section(section) && lesson >= 1 && lesson <= LESSONS_PER_SECTION {
            Some(Self { section, lesson })
        } else {
            None
        }
    }

    /// Meta de la sección, en PPM.
    pub const fn section(self) -> u16 {
        self.section
    }

    /// Número de lección, de 1 a 30.
    pub const fn lesson(self) -> u16 {
        self.lesson
    }

    /// Meta de la lección, en PPM: `sección − 5 + lección × 5 / 30`.
    pub fn target_wpm(self) -> f64 {
        f64::from(self.section) - f64::from(SECTION_STEP)
            + f64::from(self.lesson) * f64::from(SECTION_STEP) / f64::from(LESSONS_PER_SECTION)
    }

    /// Caracteres de un texto que, escrito a la meta, dura [`SECONDS_PER_LESSON`].
    pub fn target_chars(self) -> f64 {
        self.target_wpm() * 5.0 * SECONDS_PER_LESSON / 60.0
    }

    /// La lección siguiente, también al pasar de sección. `None` después de la última.
    pub const fn next(self) -> Option<Self> {
        if self.lesson < LESSONS_PER_SECTION {
            Some(Self {
                section: self.section,
                lesson: self.lesson + 1,
            })
        } else if self.section < LAST_SECTION {
            Some(Self {
                section: self.section + SECTION_STEP,
                lesson: 1,
            })
        } else {
            None
        }
    }

    /// Si es la última lección de su sección.
    pub const fn closes_section(self) -> bool {
        self.lesson == LESSONS_PER_SECTION
    }
}

/// Tiempo máximo para escribir `chars` caracteres a la meta de la lección.
pub fn time_limit(chars: usize, slot: Slot) -> Duration {
    let chars = f64::from(u32::try_from(chars).unwrap_or(u32::MAX));
    Duration::from_secs_f64(12.0 * chars / slot.target_wpm())
}

/// Velocidad de una lección terminada, en PPM. Una lección sólo termina con todo el texto bien
/// escrito, así que sale del largo y del tiempo: `velocidad ≥ meta` equivale exactamente a
/// terminar dentro de [`time_limit`]. No usa las PPM de [`Stats`], que suben un poco si se
/// escribe mal varias veces la misma letra. Con tiempo 0 devuelve 0.
pub fn speed(chars: usize, elapsed: Duration) -> f64 {
    let secs = elapsed.as_secs_f64();
    if secs <= 0.0 {
        return 0.0;
    }
    12.0 * f64::from(u32::try_from(chars).unwrap_or(u32::MAX)) / secs
}

/// Parte de la meta que hay que alcanzar para cada cantidad de estrellas (la primera es la de 1).
const SPEED_RATIOS: [f64; MAX_STARS as usize] = [0.0, 0.7, 0.8, 0.9, 1.0];

/// Parte de la meta que pide `stars` estrellas.
fn speed_ratio(stars: u8) -> f64 {
    SPEED_RATIOS[usize::from(stars.clamp(1, MAX_STARS) - 1)]
}

/// Precisión mínima (%) para poder ganar `stars` estrellas.
pub fn accuracy_for(stars: u8) -> f64 {
    if stars >= MAX_STARS {
        FULL_STARS_ACCURACY
    } else if stars > LOW_ACCURACY_STARS {
        LOW_ACCURACY
    } else {
        0.0
    }
}

/// Estrellas de una lección terminada. La velocidad da de 1 a 5 según cuánto de la meta se
/// alcanzó, con tope por precisión.
pub fn stars(speed: f64, accuracy: f64, target_wpm: f64) -> u8 {
    let ratio = speed / target_wpm;
    (1..=MAX_STARS)
        .rev()
        .find(|&n| ratio >= speed_ratio(n) && accuracy >= accuracy_for(n))
        .unwrap_or(1)
}

/// Lo que faltó para la próxima estrella que vale la pena.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Goal {
    /// Estrellas a las que apunta.
    pub stars: u8,
    /// Velocidad necesaria (PPM), si la alcanzada no basta.
    pub speed: Option<f64>,
    /// Precisión necesaria (%), si la alcanzada no basta.
    pub accuracy: Option<f64>,
}

/// Resultado de una lección terminada.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LessonResult {
    /// La lección.
    pub slot: Slot,
    /// Meta de la lección, en PPM.
    pub target_wpm: f64,
    /// Tiempo máximo.
    pub time_limit: Duration,
    /// Velocidad, en PPM (ver [`speed`]).
    pub speed: f64,
    /// Estrellas ganadas.
    pub stars: u8,
}

impl LessonResult {
    /// Calcula el resultado de un texto de `chars` caracteres.
    pub fn new(slot: Slot, chars: usize, stats: &Stats) -> Self {
        let target_wpm = slot.target_wpm();
        let speed = speed(chars, stats.elapsed);
        Self {
            slot,
            target_wpm,
            time_limit: time_limit(chars, slot),
            speed,
            stars: stars(speed, stats.accuracy, target_wpm),
        }
    }

    /// Lo que faltó para la próxima meta: [`PASS_STARS`] si no se llegó, si no una estrella más,
    /// y [`MAX_STARS`] en la lección que cierra la sección. `None` con todas las estrellas.
    pub fn goal(&self, accuracy: f64) -> Option<Goal> {
        if self.stars >= MAX_STARS {
            return None;
        }
        let stars = if self.slot.closes_section() {
            MAX_STARS
        } else {
            (self.stars + 1).max(PASS_STARS)
        };
        let ratio = speed_ratio(stars);
        let min_accuracy = accuracy_for(stars);
        Some(Goal {
            stars,
            speed: (self.speed / self.target_wpm < ratio).then_some(ratio * self.target_wpm),
            accuracy: (accuracy < min_accuracy).then_some(min_accuracy),
        })
    }
}

/// Estado de una sección.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SectionStatus {
    /// Falta aprobar la sección anterior.
    Locked,
    /// Se puede jugar y todavía no se aprobó.
    Available,
    /// Se saltó con un examen, sin aprobar todas sus lecciones.
    Skipped,
    /// Aprobada.
    Passed,
}

/// Qué abre una lección después de jugarla.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NextStep {
    /// La lección siguiente está abierta.
    Open(Slot),
    /// Falta alcanzar [`PASS_STARS`] estrellas en esta lección.
    NeedsPassStars,
    /// Falta aprobar la sección: es la última lección o queda alguna por debajo de lo pedido.
    NeedsSection,
    /// Se aprobó la última sección.
    Completed,
}

/// Un logro que merece una pantalla propia.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Milestone {
    /// Se acaba de aprobar la sección con esta meta.
    SectionPassed(u16),
    /// Se acaba de aprobar la última sección.
    Completed,
}

/// Mejores estrellas de cada lección, y lo que abren. Las reglas de desbloqueo viven acá.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Ladder {
    best: BTreeMap<Slot, u8>,
    /// Sección más alta superada con un examen (0 si no hay).
    exam_section: u16,
}

impl Ladder {
    /// Arma el estado con las mejores estrellas de cada lección jugada.
    pub fn new(best: impl IntoIterator<Item = (Slot, u8)>) -> Self {
        Self {
            best: best.into_iter().collect(),
            exam_section: 0,
        }
    }

    /// Agrega la sección más alta superada con un examen (0 si no hay).
    #[must_use]
    pub fn with_exam(mut self, section: u16) -> Self {
        self.exam_section = section;
        self
    }

    /// Si una sección quedó atrás: se aprobó o se superó con un examen.
    pub fn section_cleared(&self, section: u16) -> bool {
        section <= self.exam_section || self.section_passed(section)
    }

    /// Si esta sección admite examen: todavía no quedó atrás.
    pub fn exam_available(&self, section: u16) -> bool {
        is_section(section) && !self.section_cleared(section)
    }

    /// Lección que se juega en el examen de una sección: la última, que pide su meta.
    pub fn exam_slot(section: u16) -> Option<Slot> {
        Slot::new(section, LESSONS_PER_SECTION)
    }

    /// Mejores estrellas de una lección (0 si no se jugó).
    pub fn stars(&self, slot: Slot) -> u8 {
        self.best.get(&slot).copied().unwrap_or(0)
    }

    /// Total de estrellas ganadas.
    pub fn total_stars(&self) -> u32 {
        self.best.values().map(|&s| u32::from(s)).sum()
    }

    /// Estrellas ganadas en una sección.
    pub fn section_stars(&self, section: u16) -> u32 {
        self.of_section(section).map(|(_, s)| u32::from(s)).sum()
    }

    /// Lecciones de una sección con al menos [`PASS_STARS`] estrellas.
    pub fn lessons_passed(&self, section: u16) -> u16 {
        let passed = self.of_section(section).filter(|&(_, s)| s >= PASS_STARS);
        u16::try_from(passed.count()).unwrap_or(u16::MAX)
    }

    /// Lecciones de toda la Progresión con al menos [`PASS_STARS`] estrellas.
    pub fn total_lessons_passed(&self) -> u32 {
        let passed = self.best.values().filter(|&&stars| stars >= PASS_STARS);
        u32::try_from(passed.count()).unwrap_or(u32::MAX)
    }

    /// La meta más alta entre las secciones aprobadas.
    pub fn highest_passed(&self) -> Option<u16> {
        sections()
            .filter(|&section| self.section_cleared(section))
            .last()
    }

    /// Qué logro alcanzó una lección, comparando el estado de antes con este.
    pub fn milestone(&self, before: &Self, slot: Slot) -> Option<Milestone> {
        let section = slot.section;
        if before.section_cleared(section) || !self.section_cleared(section) {
            None
        } else if section == LAST_SECTION {
            Some(Milestone::Completed)
        } else {
            Some(Milestone::SectionPassed(section))
        }
    }

    /// Si una sección está aprobada: todas sus lecciones con al menos [`PASS_STARS`] estrellas
    /// y la última, que pide la meta de la sección, con [`MAX_STARS`].
    pub fn section_passed(&self, section: u16) -> bool {
        self.lessons_passed(section) == LESSONS_PER_SECTION
            && Slot::new(section, LESSONS_PER_SECTION).is_some_and(|l| self.stars(l) >= MAX_STARS)
    }

    /// Si una sección se puede jugar.
    pub fn section_unlocked(&self, section: u16) -> bool {
        is_section(section)
            && (section == FIRST_SECTION || self.section_cleared(section - SECTION_STEP))
    }

    /// Estado de una sección.
    pub fn status(&self, section: u16) -> SectionStatus {
        if !self.section_unlocked(section) {
            SectionStatus::Locked
        } else if self.section_passed(section) {
            SectionStatus::Passed
        } else if self.section_cleared(section) {
            SectionStatus::Skipped
        } else {
            SectionStatus::Available
        }
    }

    /// Si una lección se puede jugar.
    pub fn lesson_unlocked(&self, slot: Slot) -> bool {
        if !self.section_unlocked(slot.section) {
            return false;
        }
        // Las lecciones de una sección que quedó atrás se pueden jugar en cualquier orden.
        slot.lesson == 1
            || self.section_cleared(slot.section)
            || Slot::new(slot.section, slot.lesson - 1).is_some_and(|p| self.stars(p) >= PASS_STARS)
    }

    /// La sección en la que toca jugar: la primera sin aprobar, o la última si están todas.
    pub fn current_section(&self) -> u16 {
        sections()
            .find(|&s| !self.section_cleared(s))
            .unwrap_or(LAST_SECTION)
    }

    /// La lección en la que toca jugar dentro de una sección abierta: la primera por debajo de
    /// [`PASS_STARS`] estrellas o, si no hay, la última mientras le falten estrellas.
    pub fn current_lesson(&self, section: u16) -> Option<Slot> {
        if !self.section_unlocked(section) {
            return None;
        }
        (1..=LESSONS_PER_SECTION)
            .filter_map(|l| Slot::new(section, l))
            .find(|&l| self.stars(l) < PASS_STARS)
            .or_else(|| {
                Slot::new(section, LESSONS_PER_SECTION).filter(|&l| self.stars(l) < MAX_STARS)
            })
    }

    /// Qué sigue después de jugar una lección.
    pub fn next_step(&self, slot: Slot) -> NextStep {
        let Some(next) = slot.next() else {
            return if self.section_cleared(slot.section) {
                NextStep::Completed
            } else if self.stars(slot) < PASS_STARS {
                NextStep::NeedsPassStars
            } else {
                NextStep::NeedsSection
            };
        };
        if self.lesson_unlocked(next) {
            NextStep::Open(next)
        } else if self.stars(slot) < PASS_STARS {
            NextStep::NeedsPassStars
        } else {
            NextStep::NeedsSection
        }
    }

    fn of_section(&self, section: u16) -> impl Iterator<Item = (Slot, u8)> {
        self.best
            .iter()
            .filter(move |(slot, _)| slot.section == section)
            .map(|(&slot, &stars)| (slot, stars))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Module;
    use crate::session::{Key, Session};

    fn slot(section: u16, lesson: u16) -> Slot {
        Slot::new(section, lesson).expect("lección")
    }

    fn near(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    /// Todas las lecciones de una sección con las mismas estrellas.
    fn all_of(section: u16, stars: u8) -> Vec<(Slot, u8)> {
        (1..=LESSONS_PER_SECTION)
            .map(|l| (slot(section, l), stars))
            .collect()
    }

    #[test]
    fn there_are_17_sections_and_510_lessons() {
        assert_eq!(sections().count(), usize::from(SECTIONS));
        assert_eq!(SECTIONS, 17);
        assert_eq!(SECTION_STARS, 150);
        assert_eq!(TOTAL_STARS, 2550);
        assert_eq!(sections().next(), Some(20));
        assert_eq!(sections().last(), Some(100));
        assert!(is_section(75));
        assert!(!is_section(77));
        assert!(!is_section(15));
        assert!(!is_section(105));
        assert_eq!(Slot::new(75, 0), None);
        assert_eq!(Slot::new(75, 31), None);
        assert_eq!(Slot::new(76, 1), None);
    }

    #[test]
    fn a_section_climbs_to_its_goal() {
        assert!(near(slot(75, 30).target_wpm(), 75.0));
        assert!(near(slot(75, 1).target_wpm(), 70.0 + 1.0 / 6.0));
        assert!(near(slot(75, 2).target_wpm(), 70.0 + 2.0 / 6.0));
        assert!(near(slot(75, 15).target_wpm(), 72.5));
        assert!(near(slot(20, 1).target_wpm(), 15.0 + 1.0 / 6.0));
        assert!(near(slot(100, 30).target_wpm(), 100.0));
    }

    #[test]
    fn every_lesson_asks_more_than_the_previous_one() {
        let mut current = Slot::FIRST;
        let mut count = 1;
        while let Some(next) = current.next() {
            assert!(
                near(next.target_wpm() - current.target_wpm(), 1.0 / 6.0),
                "{current:?} → {next:?}"
            );
            current = next;
            count += 1;
        }
        assert_eq!(count, 510);
        assert_eq!(current, slot(100, 30));
    }

    #[test]
    fn time_limit_comes_from_length_and_goal() {
        assert_eq!(time_limit(300, slot(60, 30)), Duration::from_secs(60));
        assert_eq!(time_limit(375, slot(75, 30)), Duration::from_secs(60));
    }

    #[test]
    fn stars_follow_speed_limits() {
        let s = |speed| stars(speed, 100.0, 100.0);
        assert_eq!(s(100.0), 5);
        assert_eq!(s(99.99), 4);
        assert_eq!(s(90.0), 4);
        assert_eq!(s(89.99), 3);
        assert_eq!(s(80.0), 3);
        assert_eq!(s(79.99), 2);
        assert_eq!(s(70.0), 2);
        assert_eq!(s(69.99), 1);
        assert_eq!(s(1.0), 1);
    }

    #[test]
    fn accuracy_caps_the_stars() {
        assert_eq!(stars(120.0, 95.0, 100.0), 5);
        assert_eq!(stars(120.0, 94.99, 100.0), 4);
        assert_eq!(stars(120.0, 90.0, 100.0), 4);
        assert_eq!(stars(120.0, 89.99, 100.0), 2);
        assert_eq!(stars(70.0, 50.0, 100.0), 2);
        assert_eq!(stars(50.0, 50.0, 100.0), 1);
    }

    fn result(lesson: Slot, speed: f64, accuracy: f64) -> LessonResult {
        let target_wpm = lesson.target_wpm();
        LessonResult {
            slot: lesson,
            target_wpm,
            time_limit: Duration::ZERO,
            speed,
            stars: stars(speed, accuracy, target_wpm),
        }
    }

    #[test]
    fn goal_points_to_pass_stars_then_one_more() {
        let lesson = slot(45, 29);
        let target = lesson.target_wpm();
        let goal = |speed, accuracy| result(lesson, speed, accuracy).goal(accuracy);

        let one = goal(target * 0.5, 97.0).expect("meta");
        assert_eq!(one.stars, PASS_STARS);
        assert!(near(one.speed.expect("velocidad"), target * 0.8));
        assert_eq!(one.accuracy, None);

        let three = goal(target * 0.85, 97.0).expect("meta");
        assert_eq!(three.stars, 4);
        assert!(near(three.speed.expect("velocidad"), target * 0.9));

        let four = goal(target * 0.95, 97.0).expect("meta");
        assert_eq!(four.stars, MAX_STARS);
        assert!(near(four.speed.expect("velocidad"), target));

        let by_accuracy = goal(target * 1.1, 93.0).expect("meta");
        assert_eq!(by_accuracy.stars, MAX_STARS);
        assert_eq!(by_accuracy.speed, None);
        assert_eq!(by_accuracy.accuracy, Some(FULL_STARS_ACCURACY));

        let both = goal(target * 0.75, 88.0).expect("meta");
        assert_eq!(both.stars, PASS_STARS);
        assert!(both.speed.is_some());
        assert_eq!(both.accuracy, Some(LOW_ACCURACY));

        assert_eq!(goal(target, 99.0), None);
    }

    #[test]
    fn goal_of_the_last_lesson_is_max_stars() {
        let lesson = slot(45, LESSONS_PER_SECTION);
        let goal = result(lesson, lesson.target_wpm() * 0.85, 99.0)
            .goal(99.0)
            .expect("meta");
        assert_eq!(goal.stars, MAX_STARS);
    }

    #[test]
    fn five_stars_means_finishing_within_the_time_limit() {
        let lesson = slot(60, 30);
        let limit = time_limit(300, lesson);
        let on_time = speed(300, limit);
        assert_eq!(stars(on_time, 100.0, lesson.target_wpm()), 5);
        let late = speed(300, limit + Duration::from_millis(10));
        assert_eq!(stars(late, 100.0, lesson.target_wpm()), 4);
    }

    /// Escribe `text` con el módulo real, con `mistakes` errores corregidos en cada letra
    /// indicada, en `step` por tecla.
    fn play(text: &str, wrong_at: &[usize], mistakes: u32, step: Duration) -> Stats {
        let mut s = Session::new(Module::Progression, text);
        let mut at = Duration::ZERO;
        let mut press = |s: &mut Session, key| {
            let feedback = s.press(key, at);
            at += step;
            feedback
        };
        for (i, c) in text.chars().enumerate() {
            if wrong_at.contains(&i) {
                for _ in 0..mistakes {
                    press(&mut s, Key::Char('#'));
                    press(&mut s, Key::Backspace);
                }
            }
            press(&mut s, Key::Char(c));
        }
        s.result().copied().expect("terminó")
    }

    #[test]
    fn corrected_mistakes_cost_time_but_not_length() {
        let text = "uno dos tres cuatro cinco seis siete ocho nueve diez";
        let chars = text.chars().count();
        let step = Duration::from_millis(100);
        let clean = play(text, &[], 0, step);
        assert!(near(clean.wpm, speed(chars, clean.elapsed)));

        // Corregir una vez deja las PPM netas igual que la velocidad por tiempo.
        let once = play(text, &[3, 9], 1, step);
        assert!(near(once.wpm, speed(chars, once.elapsed)));
        assert!(once.elapsed > clean.elapsed);
        assert!(once.accuracy < 100.0);

        // Errar varias veces la misma letra infla las PPM netas: las estrellas no las usan.
        let repeated = play(text, &[3], 4, step);
        assert!(repeated.wpm > speed(chars, repeated.elapsed));
    }

    #[test]
    fn result_uses_the_lesson_goal() {
        let text = "a".repeat(300);
        let lesson = slot(60, 30);
        // 300 caracteres en 49,9 s: bastante más rápido que la meta.
        let stats = play(&text, &[], 0, Duration::from_millis(167));
        let result = LessonResult::new(lesson, 300, &stats);
        assert_eq!(result.stars, 5);
        assert!(near(result.target_wpm, 60.0));
        assert_eq!(result.time_limit, Duration::from_secs(60));
    }

    #[test]
    fn three_stars_open_the_next_lesson() {
        let mut ladder = Ladder::default();
        assert!(ladder.lesson_unlocked(Slot::FIRST));
        assert!(!ladder.lesson_unlocked(slot(20, 2)));
        assert_eq!(ladder.next_step(Slot::FIRST), NextStep::NeedsPassStars);

        ladder = Ladder::new([(Slot::FIRST, 2)]);
        assert!(!ladder.lesson_unlocked(slot(20, 2)));

        ladder = Ladder::new([(Slot::FIRST, 3)]);
        assert!(ladder.lesson_unlocked(slot(20, 2)));
        assert!(!ladder.lesson_unlocked(slot(20, 3)));
        assert_eq!(ladder.next_step(Slot::FIRST), NextStep::Open(slot(20, 2)));
    }

    #[test]
    fn passing_a_section_needs_five_stars_in_its_last_lesson() {
        let threes = Ladder::new(all_of(20, 3));
        assert_eq!(threes.lessons_passed(20), 30);
        assert!(!threes.section_passed(20));
        assert_eq!(threes.status(20), SectionStatus::Available);
        assert_eq!(threes.status(25), SectionStatus::Locked);
        assert!(!threes.lesson_unlocked(slot(25, 1)));
        assert_eq!(threes.next_step(slot(20, 30)), NextStep::NeedsSection);

        let mut best = all_of(20, 3);
        best.push((slot(20, 30), 5));
        let passed = Ladder::new(best);
        assert!(passed.section_passed(20));
        assert_eq!(passed.status(20), SectionStatus::Passed);
        assert_eq!(passed.status(25), SectionStatus::Available);
        assert!(passed.lesson_unlocked(slot(25, 1)));
        assert_eq!(passed.next_step(slot(20, 30)), NextStep::Open(slot(25, 1)));
        assert_eq!(passed.section_stars(20), 29 * 3 + 5);
        assert_eq!(passed.total_stars(), 29 * 3 + 5);
    }

    #[test]
    fn a_missing_lesson_blocks_the_section() {
        let mut best = all_of(20, 5);
        best.retain(|(s, _)| s.lesson() != 7);
        let ladder = Ladder::new(best);
        assert!(!ladder.section_passed(20));
        assert_eq!(ladder.current_lesson(20), Some(slot(20, 7)));
    }

    #[test]
    fn current_position_follows_the_stars() {
        let empty = Ladder::default();
        assert_eq!(empty.current_section(), 20);
        assert_eq!(empty.current_lesson(20), Some(Slot::FIRST));
        assert_eq!(empty.current_lesson(25), None);

        // Todas aprobadas, falta la meta en la última.
        let threes = Ladder::new(all_of(20, 3));
        assert_eq!(threes.current_lesson(20), Some(slot(20, 30)));

        let fives = Ladder::new(all_of(20, 5));
        assert_eq!(fives.current_lesson(20), None);
        assert_eq!(fives.current_section(), 25);
    }

    #[test]
    fn an_exam_clears_the_section_and_the_previous_ones() {
        let before = Ladder::default();
        assert!(before.exam_available(60));
        assert!(!before.exam_available(61));
        assert_eq!(Ladder::exam_slot(60), Slot::new(60, 30));

        let after = Ladder::new([(slot(60, 30), 5)]).with_exam(60);
        for section in [20, 40, 60] {
            assert!(after.section_cleared(section), "{section}");
            assert!(!after.exam_available(section), "{section}");
        }
        assert_eq!(after.status(20), SectionStatus::Skipped);
        assert_eq!(after.status(60), SectionStatus::Skipped);
        assert_eq!(after.status(65), SectionStatus::Available);
        assert_eq!(after.status(70), SectionStatus::Locked);
        assert_eq!(after.current_section(), 65);
        assert_eq!(after.highest_passed(), Some(60));
        assert_eq!(after.next_step(slot(60, 30)), NextStep::Open(slot(65, 1)));
        assert_eq!(
            after.milestone(&before, slot(60, 30)),
            Some(Milestone::SectionPassed(60))
        );
    }

    #[test]
    fn an_exam_gives_no_stars_and_keeps_old_lessons_open() {
        let after = Ladder::default().with_exam(60);
        assert_eq!(after.total_stars(), 0);
        assert_eq!(after.total_lessons_passed(), 0);
        assert_eq!(after.section_stars(20), 0);
        assert!(after.lesson_unlocked(slot(40, 17)), "abierta, sin orden");
        assert!(after.lesson_unlocked(slot(65, 1)));
        assert!(!after.lesson_unlocked(slot(65, 2)), "la nueva va en orden");
    }

    #[test]
    fn the_last_exam_completes_the_progression() {
        let after = Ladder::new([(slot(100, 30), 5)]).with_exam(100);
        assert_eq!(after.next_step(slot(100, 30)), NextStep::Completed);
        assert_eq!(
            after.milestone(&Ladder::default(), slot(100, 30)),
            Some(Milestone::Completed)
        );
    }

    #[test]
    fn counts_the_overall_progress() {
        let mut best = all_of(20, 5);
        best.push((slot(25, 1), 2));
        best.push((slot(25, 2), 3));
        let ladder = Ladder::new(best);
        assert_eq!(ladder.total_lessons_passed(), 31);
        assert_eq!(ladder.highest_passed(), Some(20));
        assert_eq!(Ladder::default().highest_passed(), None);
    }

    #[test]
    fn a_milestone_happens_only_when_the_section_is_just_passed() {
        let mut almost = all_of(20, 5);
        almost.push((slot(20, 30), 3));
        let before = Ladder::new(almost);
        let after = Ladder::new(all_of(20, 5));
        let last = slot(20, 30);
        assert_eq!(
            after.milestone(&before, last),
            Some(Milestone::SectionPassed(20))
        );
        assert_eq!(after.milestone(&after, last), None, "ya estaba aprobada");
        assert_eq!(before.milestone(&before, last), None, "todavía no");
        let everything: Vec<_> = sections().flat_map(|s| all_of(s, 5)).collect();
        let done = Ladder::new(everything);
        assert_eq!(
            done.milestone(&Ladder::default(), slot(100, 30)),
            Some(Milestone::Completed)
        );
    }

    #[test]
    fn the_last_section_completes_the_progression() {
        let everything: Vec<_> = sections().flat_map(|s| all_of(s, 5)).collect();
        let ladder = Ladder::new(everything);
        assert_eq!(ladder.next_step(slot(100, 30)), NextStep::Completed);
        assert_eq!(ladder.current_section(), 100);
        assert_eq!(ladder.total_stars(), 2550);

        let almost = Ladder::new(sections().flat_map(|s| all_of(s, 3)));
        assert_eq!(almost.next_step(slot(100, 30)), NextStep::NeedsSection);
        let low = Ladder::new([(slot(100, 30), 1)]);
        assert_eq!(low.next_step(slot(100, 30)), NextStep::NeedsPassStars);
    }
}
