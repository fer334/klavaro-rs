// SPDX-License-Identifier: GPL-3.0-or-later
//! Gráficos de progreso, como la ventana «Progreso» del original (`plot.c`).
//!
//! Cada gráfico muestra las últimas [`DATA_POINTS`] sesiones de un módulo con la meta como
//! línea horizontal. Igual que en el original, el curso básico se filtra por lección (0 = todas)
//! y teclado, Adaptabilidad por teclado, y Velocidad y Fluidez por idioma de los textos.
//!
//! Adaptabilidad suma dos gráficos de barras por tecla, con el rastreo del teclado actual:
//! errores y tiempo promedio de pulsación, de mayor a menor.

use klavaro_core::Module;
use klavaro_core::goals::Goals;
use klavaro_core::progression::{FULL_STARS_ACCURACY, Slot};
use klavaro_core::tracker::KeyTracker;
use klavaro_store::{History, SessionFilter, StoreError};

/// Cantidad de sesiones que muestra cada gráfico (`DATA_POINTS` en `plot.h`).
pub(crate) const DATA_POINTS: usize = 50;

/// Qué se grafica.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Metric {
    /// Precisión (%).
    Accuracy,
    /// Velocidad (PPM).
    Speed,
    /// Fluidez (sólo módulo de Fluidez).
    Fluidity,
    /// Puntaje del Top 10 (sólo módulo de Fluidez).
    Score,
    /// Errores por tecla (sólo Adaptabilidad).
    Errors,
    /// Tiempo promedio de pulsación por tecla, en segundos (sólo Adaptabilidad).
    TouchTimes,
    /// Estrellas por sección (sólo Progresión). No sale del historial de sesiones.
    Stars,
}

impl Metric {
    /// Gráficos disponibles en un módulo.
    pub(crate) fn for_module(module: Module) -> &'static [Metric] {
        match module {
            Module::Fluidity => &[Self::Accuracy, Self::Speed, Self::Fluidity, Self::Score],
            Module::Adaptability => &[Self::Accuracy, Self::Speed, Self::Errors, Self::TouchTimes],
            Module::Progression => &[Self::Speed, Self::Accuracy, Self::Stars],
            Module::Basic | Module::Velocity => &[Self::Accuracy, Self::Speed],
        }
    }

    /// Clave del nombre en los `.ftl`.
    pub(crate) fn label_key(self) -> &'static str {
        match self {
            Self::Accuracy => "progress-accuracy",
            Self::Speed => "progress-speed",
            Self::Fluidity => "progress-fluidity",
            Self::Score => "stats-score",
            Self::Errors => "progress-errors",
            Self::TouchTimes => "progress-touch-times",
            Self::Stars => "sections-stars",
        }
    }

    /// Si es un gráfico de barras por tecla (sale del rastreo, no del historial).
    pub(crate) fn per_key(self) -> bool {
        matches!(self, Self::Errors | Self::TouchTimes)
    }

    /// Decimales para mostrar un valor.
    pub(crate) fn decimals(self) -> usize {
        match self {
            Self::Errors | Self::Stars => 0,
            Self::TouchTimes => 3,
            Self::Accuracy | Self::Speed | Self::Fluidity | Self::Score => 1,
        }
    }

    /// Rango del eje vertical antes de ajustarlo a los datos (el mismo del original).
    fn base_range(self) -> (f64, f64) {
        match self {
            Self::Accuracy => (60.0, 100.0),
            Self::Speed => (0.0, 120.0),
            Self::Fluidity => (0.0, 100.0),
            Self::Score => (0.0, 10.0),
            // Como el original: de 0 a un 5 % más que la barra más alta.
            Self::Errors => (0.0, 1.05),
            Self::TouchTimes => (0.0, 0.0105),
            Self::Stars => (0.0, 150.0),
        }
    }

    fn goal(self, goals: &Goals, module: Module) -> Option<f64> {
        let goal = goals.for_module(module);
        match self {
            Self::Accuracy => Some(goal.accuracy),
            Self::Speed => Some(goal.speed),
            Self::Fluidity => goal.fluidity,
            Self::Score | Self::Errors | Self::TouchTimes | Self::Stars => None,
        }
    }
}

/// Qué gráfico pedir.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ChartQuery {
    /// Módulo.
    pub(crate) module: Module,
    /// Qué se grafica.
    pub(crate) metric: Metric,
    /// Lección del curso básico o de la Progresión (0 = todas).
    pub(crate) lesson: u16,
    /// Sección de la Progresión (0 = todas).
    pub(crate) section: u16,
    /// Teclado actual.
    pub(crate) keyboard: String,
    /// Idioma actual de los textos.
    pub(crate) language: String,
}

/// Un punto del gráfico.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ChartPoint {
    /// Instante (segundos desde 1970); 0 en los gráficos por tecla.
    pub(crate) at: i64,
    /// Tecla, en los gráficos por tecla.
    pub(crate) key: Option<char>,
    /// Valor.
    pub(crate) value: f64,
}

/// Datos listos para dibujar.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Chart {
    /// Qué se grafica.
    pub(crate) metric: Metric,
    /// Puntos, del más viejo al más nuevo.
    pub(crate) points: Vec<ChartPoint>,
    /// Meta del módulo, si la hay.
    pub(crate) goal: Option<f64>,
    /// Mínimo del eje vertical.
    pub(crate) min: f64,
    /// Máximo del eje vertical.
    pub(crate) max: f64,
    /// Separación de las líneas de la grilla.
    step: f64,
}

impl Chart {
    /// Arma el gráfico pedido con el historial.
    ///
    /// # Errors
    ///
    /// Si falla la lectura del historial.
    pub(crate) fn load(
        history: &History,
        goals: &Goals,
        q: &ChartQuery,
    ) -> Result<Self, StoreError> {
        let points: Vec<ChartPoint> = if q.metric == Metric::Score {
            history
                .recent_scores(Some(&q.language), DATA_POINTS)?
                .into_iter()
                .map(|s| ChartPoint {
                    at: s.at,
                    key: None,
                    value: s.score,
                })
                .collect()
        } else {
            history
                .sessions(&filter(q), DATA_POINTS)?
                .into_iter()
                .map(|s| ChartPoint {
                    at: s.at,
                    key: None,
                    value: match q.metric {
                        Metric::Speed => s.wpm,
                        Metric::Fluidity => s.fluidity,
                        _ => s.accuracy,
                    },
                })
                .collect()
        };
        let goal = if q.module == Module::Progression {
            progression_goal(q)
        } else {
            q.metric.goal(goals, q.module)
        };
        Ok(Self::new(q.metric, points, goal))
    }

    /// Barras por tecla con el rastreo de un teclado: las [`DATA_POINTS`] teclas con más
    /// errores, o las más lentas.
    pub(crate) fn per_key(metric: Metric, tracker: &KeyTracker) -> Self {
        let mut tracker = tracker.clone();
        tracker.sort();
        let points = if metric == Metric::Errors {
            tracker
                .errors()
                .iter()
                .filter(|e| e.wrong > 0)
                .map(|e| (e.ch, f64::from(e.wrong)))
                .collect::<Vec<_>>()
        } else {
            tracker
                .times()
                .iter()
                .map(|t| (t.ch, t.average()))
                .collect()
        };
        let points = points
            .into_iter()
            .take(DATA_POINTS)
            .map(|(ch, value)| ChartPoint {
                at: 0,
                key: Some(ch),
                value,
            })
            .collect();
        Self::new(metric, points, None)
    }

    /// Gráfico con el eje vertical ajustado para que entren todos los puntos y la meta.
    pub(crate) fn new(metric: Metric, points: Vec<ChartPoint>, goal: Option<f64>) -> Self {
        let (mut min, mut max) = metric.base_range();
        if metric.per_key() {
            let top = points.iter().map(|p| p.value).fold(0.0, f64::max);
            max = max.max(1.05 * top);
            let mut step = nice_step(max / 5.0);
            if metric == Metric::Errors {
                step = step.max(1.0);
            }
            return Self {
                metric,
                points,
                goal,
                min,
                max: (max / step).ceil() * step,
                step,
            };
        }
        let step = grid_step(metric);
        for v in points.iter().map(|p| p.value).chain(goal) {
            if v < min {
                min = (v / step).floor() * step;
            }
            if v > max {
                max = (v / step).ceil() * step;
            }
        }
        Self {
            metric,
            points,
            goal,
            min,
            max,
            step,
        }
    }

    /// Decimales de las etiquetas del eje vertical (paso 0.5 → 1, paso 0.02 → 2).
    pub(crate) fn axis_decimals(&self) -> usize {
        let mut d = 0;
        let mut s = self.step;
        while s < 0.999 && d < 4 {
            s *= 10.0;
            d += 1;
        }
        d
    }

    /// Valores del eje vertical donde van las líneas de la grilla.
    pub(crate) fn grid(&self) -> Vec<f64> {
        let step = self.step;
        let mut lines = Vec::new();
        let mut v = (self.min / step).ceil() * step;
        while v <= self.max + step * 1e-6 {
            lines.push(v);
            v += step;
        }
        lines
    }
}

fn grid_step(metric: Metric) -> f64 {
    match metric {
        Metric::Accuracy => 10.0,
        Metric::Speed | Metric::Fluidity => 20.0,
        Metric::Score => 2.0,
        Metric::Errors | Metric::TouchTimes => 1.0,
        Metric::Stars => 30.0,
    }
}

/// Paso «redondo» (1, 2 o 5 por una potencia de 10) no menor que `raw`.
fn nice_step(raw: f64) -> f64 {
    let raw = raw.max(1e-6);
    let base = 10f64.powf(raw.log10().floor());
    [1.0, 2.0, 5.0, 10.0]
        .into_iter()
        .map(|m| m * base)
        .find(|&s| s >= raw * (1.0 - 1e-9))
        .unwrap_or(10.0 * base)
}

/// Meta de la línea de referencia en la Progresión: la de la lección elegida, la de la sección
/// si se ve toda, y ninguna para el conjunto.
fn progression_goal(q: &ChartQuery) -> Option<f64> {
    match q.metric {
        Metric::Accuracy => Some(FULL_STARS_ACCURACY),
        Metric::Speed if q.section == 0 => None,
        Metric::Speed => Slot::new(q.section, q.lesson).map_or_else(
            || Some(f64::from(q.section)),
            |slot| Some(slot.target_wpm()),
        ),
        _ => None,
    }
}

fn filter(q: &ChartQuery) -> SessionFilter {
    let mut f = SessionFilter::module(q.module);
    match q.module {
        Module::Basic => {
            f.lesson = (q.lesson != 0).then_some(q.lesson);
            f.keyboard = Some(q.keyboard.clone());
        }
        Module::Adaptability => f.keyboard = Some(q.keyboard.clone()),
        Module::Velocity | Module::Fluidity => f.language = Some(q.language.clone()),
        Module::Progression => {
            f.language = Some(q.language.clone());
            if q.section != 0 && q.lesson != 0 {
                f.lesson_name = Some(format!("{}-{:02}", q.section, q.lesson));
            } else if q.section != 0 {
                f.lesson = Some(q.section);
            }
        }
    }
    f
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use klavaro_core::metrics::Stats;
    use klavaro_store::SessionRecord;
    use klavaro_store::history::ScoreRecord;

    use super::*;

    #[test]
    fn progression_charts_filter_by_section_and_lesson() {
        let h = History::in_memory().expect("base");
        for (at, lesson, language) in [
            (1, "75-03", "C"),
            (2, "75-03", "C"),
            (3, "75-04", "C"),
            (4, "80-03", "C"),
            (5, "75-03", "es"),
        ] {
            let mut record = session(Module::Progression, at, lesson, "dvorak_us");
            record.language = Some(language.into());
            h.record(&record).expect("guardar");
        }
        let goals = Goals::default();
        let mut q = query(Module::Progression, Metric::Speed);
        q.language = "C".into();
        let load = |q: &ChartQuery| Chart::load(&h, &goals, q).expect("gráfico");

        let everything = load(&q);
        assert_eq!(everything.points.len(), 4);
        assert_eq!(everything.goal, None);

        q.section = 75;
        let section = load(&q);
        assert_eq!(section.points.len(), 3);
        assert_eq!(section.goal, Some(75.0));

        q.lesson = 3;
        let lesson = load(&q);
        assert_eq!(lesson.points.len(), 2, "sólo sus intentos, en este idioma");
        assert_eq!(
            lesson.goal,
            Slot::new(75, 3).map(klavaro_core::progression::Slot::target_wpm)
        );

        q.metric = Metric::Accuracy;
        assert_eq!(load(&q).goal, Some(FULL_STARS_ACCURACY));
    }

    fn query(module: Module, metric: Metric) -> ChartQuery {
        ChartQuery {
            section: 0,
            module,
            metric,
            lesson: 0,
            keyboard: "qwerty_es".into(),
            language: "es".into(),
        }
    }

    fn session(module: Module, at: i64, lesson: &str, keyboard: &str) -> SessionRecord {
        let stats = Stats::compute(100, 5, Duration::from_secs(60), &[]);
        SessionRecord {
            lesson: Some(lesson.into()),
            keyboard: Some(keyboard.into()),
            language: Some("es".into()),
            ..SessionRecord::from_stats(module, at, &stats)
        }
    }

    #[test]
    fn basic_charts_filter_by_lesson_and_keyboard() {
        let h = History::in_memory().expect("base");
        for (at, lesson, kb) in [
            (1, "01", "qwerty_es"),
            (2, "02", "qwerty_es"),
            (3, "02", "dvorak"),
        ] {
            h.record(&session(Module::Basic, at, lesson, kb))
                .expect("guardar");
        }
        let goals = Goals::default();
        let mut q = query(Module::Basic, Metric::Accuracy);
        let all = Chart::load(&h, &goals, &q).expect("leer");
        assert_eq!(all.points.iter().map(|p| p.at).collect::<Vec<_>>(), [1, 2]);
        assert_eq!(all.goal, Some(goals.basic.accuracy));
        q.lesson = 2;
        let one = Chart::load(&h, &goals, &q).expect("leer");
        assert_eq!(one.points.iter().map(|p| p.at).collect::<Vec<_>>(), [2]);
    }

    #[test]
    fn keeps_the_last_data_points() {
        let h = History::in_memory().expect("base");
        let n = i64::try_from(DATA_POINTS).unwrap_or(0) + 7;
        for at in 0..n {
            h.record(&session(Module::Velocity, at, "Default", "qwerty_es"))
                .expect("guardar");
        }
        let c = Chart::load(
            &h,
            &Goals::default(),
            &query(Module::Velocity, Metric::Speed),
        )
        .expect("leer");
        assert_eq!(c.points.len(), DATA_POINTS);
        assert_eq!(c.points.first().map(|p| p.at), Some(7));
        assert_eq!(c.points.last().map(|p| p.at), Some(n - 1));
    }

    #[test]
    fn scores_come_from_the_score_table() {
        let h = History::in_memory().expect("base");
        for (at, language) in [(1, "es"), (2, "C")] {
            h.record_score(&ScoreRecord {
                at,
                score: 4.5,
                chars: 600,
                language: language.into(),
            })
            .expect("guardar");
        }
        let c = Chart::load(
            &h,
            &Goals::default(),
            &query(Module::Fluidity, Metric::Score),
        )
        .expect("leer");
        assert_eq!(
            c.points,
            [ChartPoint {
                at: 1,
                key: None,
                value: 4.5
            }]
        );
        assert_eq!(c.goal, None);
    }

    #[test]
    fn axis_grows_to_fit_the_data() {
        let pts = |v: &[f64]| {
            v.iter()
                .map(|&value| ChartPoint {
                    at: 0,
                    key: None,
                    value,
                })
                .collect::<Vec<_>>()
        };
        let c = Chart::new(Metric::Accuracy, pts(&[95.0, 42.0]), Some(97.0));
        assert!((c.min - 40.0).abs() < 1e-9);
        assert!((c.max - 100.0).abs() < 1e-9);
        assert_eq!(c.grid().len(), 7);
        let c = Chart::new(Metric::Speed, pts(&[131.0]), Some(60.0));
        assert!((c.max - 140.0).abs() < 1e-9);
    }

    #[test]
    fn chart_names_are_translated() {
        for code in ["C", "es"] {
            let tr = klavaro_i18n::Translator::new(code);
            for &m in Metric::for_module(Module::Fluidity) {
                assert!(tr.has(m.label_key()), "{code}: {}", m.label_key());
            }
            for &m in Metric::for_module(Module::Adaptability) {
                assert!(tr.has(m.label_key()), "{code}: {}", m.label_key());
            }
            for key in [
                "progress-wpm-unit",
                "progress-empty",
                "progress-sessions",
                "ranking-place",
            ] {
                assert!(tr.has(key), "{code}: {key}");
            }
        }
    }

    #[test]
    fn fluidity_has_four_charts() {
        assert_eq!(Metric::for_module(Module::Fluidity).len(), 4);
        assert_eq!(
            Metric::for_module(Module::Basic),
            [Metric::Accuracy, Metric::Speed]
        );
    }

    #[test]
    fn per_key_bars_come_from_the_tracker() {
        use klavaro_core::tracker::{ErrorEntry, TimeEntry};

        let tracker = KeyTracker::from_parts(
            vec![
                ErrorEntry {
                    ch: 'a',
                    wrong: 2,
                    correct: 1,
                },
                ErrorEntry {
                    ch: 'q',
                    wrong: 7,
                    correct: 1,
                },
            ],
            vec![
                TimeEntry::new('k', vec![0.2]),
                TimeEntry::new('j', vec![0.6]),
            ],
        );
        let errors = Chart::per_key(Metric::Errors, &tracker);
        let keys: Vec<_> = errors.points.iter().filter_map(|p| p.key).collect();
        assert_eq!(keys, ['q', 'a']);
        assert!(errors.max >= 7.35 && errors.max <= 8.0, "{}", errors.max);
        assert_eq!(errors.axis_decimals(), 0);

        let times = Chart::per_key(Metric::TouchTimes, &tracker);
        assert_eq!(times.points.first().and_then(|p| p.key), Some('j'));
        assert!(times.max >= 0.63 && times.max <= 0.8, "{}", times.max);
        assert_eq!(times.axis_decimals(), 1);

        let empty = Chart::per_key(Metric::Errors, &KeyTracker::new());
        assert_eq!(empty.points, []);
        assert!(empty.grid().len() > 1);
    }

    #[test]
    fn nice_steps() {
        assert!((nice_step(0.13) - 0.2).abs() < 1e-12);
        assert!((nice_step(3.0) - 5.0).abs() < 1e-12);
        assert!((nice_step(10.0) - 10.0).abs() < 1e-12);
    }
}
