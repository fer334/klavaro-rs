// SPDX-License-Identifier: GPL-3.0-or-later
//! Gráficos de progreso, como la ventana «Progreso» del original (`plot.c`).
//!
//! Cada gráfico muestra las últimas [`DATA_POINTS`] sesiones de un módulo con la meta como
//! línea horizontal. Igual que en el original, el curso básico se filtra por lección (0 = todas)
//! y teclado, Adaptabilidad por teclado, y Velocidad y Fluidez por idioma de los textos.

use klavaro_core::Module;
use klavaro_core::goals::Goals;
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
}

impl Metric {
    /// Gráficos disponibles en un módulo.
    pub(crate) fn for_module(module: Module) -> &'static [Metric] {
        match module {
            Module::Fluidity => &[Self::Accuracy, Self::Speed, Self::Fluidity, Self::Score],
            Module::Basic | Module::Adaptability | Module::Velocity => {
                &[Self::Accuracy, Self::Speed]
            }
        }
    }

    /// Clave del nombre en los `.ftl`.
    pub(crate) fn label_key(self) -> &'static str {
        match self {
            Self::Accuracy => "progress-accuracy",
            Self::Speed => "progress-speed",
            Self::Fluidity => "progress-fluidity",
            Self::Score => "stats-score",
        }
    }

    /// Rango del eje vertical antes de ajustarlo a los datos (el mismo del original).
    fn base_range(self) -> (f64, f64) {
        match self {
            Self::Accuracy => (60.0, 100.0),
            Self::Speed => (0.0, 120.0),
            Self::Fluidity => (0.0, 100.0),
            Self::Score => (0.0, 10.0),
        }
    }

    fn goal(self, goals: &Goals, module: Module) -> Option<f64> {
        let goal = goals.for_module(module);
        match self {
            Self::Accuracy => Some(goal.accuracy),
            Self::Speed => Some(goal.speed),
            Self::Fluidity => goal.fluidity,
            Self::Score => None,
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
    /// Lección del curso básico (0 = todas).
    pub(crate) lesson: u16,
    /// Teclado actual.
    pub(crate) keyboard: String,
    /// Idioma actual de los textos.
    pub(crate) language: String,
}

/// Un punto del gráfico.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ChartPoint {
    /// Instante (segundos desde 1970).
    pub(crate) at: i64,
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
                    value: s.score,
                })
                .collect()
        } else {
            history
                .sessions(&filter(q), DATA_POINTS)?
                .into_iter()
                .map(|s| ChartPoint {
                    at: s.at,
                    value: match q.metric {
                        Metric::Accuracy => s.accuracy,
                        Metric::Speed => s.wpm,
                        Metric::Fluidity | Metric::Score => s.fluidity,
                    },
                })
                .collect()
        };
        Ok(Self::new(q.metric, points, q.metric.goal(goals, q.module)))
    }

    /// Gráfico con el eje vertical ajustado para que entren todos los puntos y la meta.
    pub(crate) fn new(metric: Metric, points: Vec<ChartPoint>, goal: Option<f64>) -> Self {
        let (mut min, mut max) = metric.base_range();
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
        }
    }

    /// Valores del eje vertical donde van las líneas de la grilla.
    pub(crate) fn grid(&self) -> Vec<f64> {
        let step = grid_step(self.metric);
        let mut lines = Vec::new();
        let mut v = (self.min / step).ceil() * step;
        while v <= self.max + f64::EPSILON {
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

    fn query(module: Module, metric: Metric) -> ChartQuery {
        ChartQuery {
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
        assert_eq!(c.points, [ChartPoint { at: 1, value: 4.5 }]);
        assert_eq!(c.goal, None);
    }

    #[test]
    fn axis_grows_to_fit_the_data() {
        let pts = |v: &[f64]| {
            v.iter()
                .map(|&value| ChartPoint { at: 0, value })
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
}
