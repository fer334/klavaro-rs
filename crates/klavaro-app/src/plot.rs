// SPDX-License-Identifier: GPL-3.0-or-later
//! Dibujo de los gráficos de progreso con el `canvas` de iced, con los colores de `plot.h`.
//!
//! Igual que en el original, cada sesión ocupa una posición fija de [`DATA_POINTS`] + 2, así
//! que con pocas sesiones la línea queda a la izquierda. Al pasar el mouse se muestra la
//! fecha y el valor del punto más cercano.

use iced::alignment::{Horizontal, Vertical};
use iced::widget::canvas::{self, Frame, Geometry, Path, Stroke, Text};
use iced::{Color, Pixels, Point, Rectangle, Renderer, Size, Theme, mouse};

use klavaro_store::date::format_date_time;

use crate::progress::{Chart, DATA_POINTS, Metric};

/// Márgenes del área de datos: izquierda (etiquetas del eje), arriba (dato señalado), derecha
/// y abajo.
const LEFT: f32 = 44.0;
const TOP: f32 = 26.0;
const RIGHT: f32 = 12.0;
const BOTTOM: f32 = 12.0;

/// Gráfico listo para el `canvas`.
pub(crate) struct Plot<'a> {
    /// Datos.
    pub(crate) chart: &'a Chart,
    /// Unidad del valor señalado (` %`, ` PPM`…).
    pub(crate) unit: String,
}

/// Línea, borde y halo de la serie, como `PLOT_*_2` y `PLOT_*_3` del original.
fn series_colors(metric: Metric) -> (Color, Color) {
    match metric {
        Metric::Accuracy => (
            Color::from_rgb8(0x66, 0xaa, 0x88),
            Color::from_rgb8(0xaa, 0xee, 0xbb),
        ),
        Metric::Speed => (
            Color::from_rgb8(0xcc, 0x44, 0x66),
            Color::from_rgb8(0xee, 0xaa, 0xaa),
        ),
        Metric::Fluidity => (
            Color::from_rgb8(0x66, 0x66, 0xbb),
            Color::from_rgb8(0xb0, 0xb0, 0xff),
        ),
        Metric::Score => (
            Color::from_rgb8(0xee, 0xbb, 0x88),
            Color::from_rgb8(0xff, 0xee, 0xaa),
        ),
    }
}

impl Plot<'_> {
    /// Área de datos dentro del `canvas`.
    fn area(size: Size) -> Rectangle {
        Rectangle {
            x: LEFT,
            y: TOP,
            width: (size.width - LEFT - RIGHT).max(1.0),
            height: (size.height - TOP - BOTTOM).max(1.0),
        }
    }

    // Las coordenadas de pantalla son f32; la precisión de f64 no hace falta para dibujar.
    #[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
    fn x(area: Rectangle, index: usize) -> f32 {
        area.x + area.width * (index + 1) as f32 / (DATA_POINTS + 1) as f32
    }

    #[allow(clippy::cast_possible_truncation)]
    fn y(&self, area: Rectangle, value: f64) -> f32 {
        let span = (self.chart.max - self.chart.min).max(f64::EPSILON);
        let t = ((value - self.chart.min) / span).clamp(0.0, 1.0) as f32;
        area.y + area.height * (1.0 - t)
    }

    /// Índice del punto más cercano al mouse, si está sobre el gráfico.
    fn hovered(&self, area: Rectangle, cursor: Point) -> Option<usize> {
        if self.chart.points.is_empty() || !area.expand(8.0).contains(cursor) {
            return None;
        }
        (0..self.chart.points.len()).min_by(|&a, &b| {
            let da = (Self::x(area, a) - cursor.x).abs();
            let db = (Self::x(area, b) - cursor.x).abs();
            da.total_cmp(&db)
        })
    }
}

impl<Message> canvas::Program<Message> for Plot<'_> {
    type State = ();

    fn draw(
        &self,
        _state: &(),
        renderer: &Renderer,
        theme: &Theme,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        let area = Self::area(bounds.size());
        let ink = theme.palette().text;
        let faint = Color { a: 0.25, ..ink };

        for v in self.chart.grid() {
            let y = self.y(area, v);
            frame.stroke(
                &Path::line(Point::new(area.x, y), Point::new(area.x + area.width, y)),
                Stroke::default().with_color(faint).with_width(1.0),
            );
            frame.fill_text(Text {
                content: format!("{v:.0}"),
                position: Point::new(area.x - 6.0, y),
                color: ink,
                size: Pixels(12.0),
                align_x: Horizontal::Right.into(),
                align_y: Vertical::Center,
                ..Text::default()
            });
        }
        frame.stroke(
            &Path::rectangle(area.position(), area.size()),
            Stroke::default().with_color(ink).with_width(1.0),
        );

        if let Some(goal) = self.chart.goal {
            let y = self.y(area, goal);
            frame.stroke(
                &Path::line(Point::new(area.x, y), Point::new(area.x + area.width, y)),
                Stroke::default()
                    .with_color(Color::from_rgb8(0x80, 0x80, 0x80))
                    .with_width(2.0),
            );
        }

        let points: Vec<Point> = self
            .chart
            .points
            .iter()
            .enumerate()
            .map(|(i, p)| Point::new(Self::x(area, i), self.y(area, p.value)))
            .collect();
        let (line, halo) = series_colors(self.chart.metric);
        if points.len() > 1 {
            let path = Path::new(|b| {
                for (i, &p) in points.iter().enumerate() {
                    if i == 0 { b.move_to(p) } else { b.line_to(p) }
                }
            });
            frame.stroke(&path, Stroke::default().with_color(halo).with_width(5.0));
            frame.stroke(&path, Stroke::default().with_color(line).with_width(2.0));
        }
        for &p in &points {
            frame.fill(&Path::circle(p, 3.5), ink);
            frame.fill(&Path::circle(p, 2.0), halo);
        }

        if let Some(i) = cursor
            .position_in(bounds)
            .and_then(|c| self.hovered(area, c))
            && let (Some(&p), Some(data)) = (points.get(i), self.chart.points.get(i))
        {
            frame.stroke(
                &Path::circle(p, 6.0),
                Stroke::default().with_color(ink).with_width(2.0),
            );
            frame.fill_text(Text {
                content: format!(
                    "{} · {:.1}{}",
                    format_date_time(data.at),
                    data.value,
                    self.unit
                ),
                position: Point::new(area.x, 4.0),
                color: ink,
                size: Pixels(14.0),
                ..Text::default()
            });
        }

        vec![frame.into_geometry()]
    }

    fn mouse_interaction(
        &self,
        _state: &(),
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        let area = Self::area(bounds.size());
        match cursor.position_in(bounds) {
            Some(c) if self.hovered(area, c).is_some() => mouse::Interaction::Crosshair,
            _ => mouse::Interaction::default(),
        }
    }
}
