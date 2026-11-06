// SPDX-License-Identifier: GPL-3.0-or-later
//! Dibujo de los gráficos de progreso con el `canvas` de iced, con los colores de `plot.h`.
//!
//! Igual que en el original, cada sesión ocupa una posición fija de [`DATA_POINTS`] + 2, así
//! que con pocas sesiones la línea queda a la izquierda. Al pasar el mouse se muestra la
//! fecha y el valor del punto más cercano. Los gráficos por tecla de Adaptabilidad son de
//! barras, con cada tecla debajo de su barra.

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
/// Margen inferior de los gráficos de barras, donde van las teclas.
const BOTTOM_KEYS: f32 = 24.0;

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
        // Las barras de errores usan el mismo verde que la precisión (`PLOT_GREEN_2`).
        Metric::Accuracy | Metric::Errors | Metric::Stars => (
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
        // `PLOT_PURPLE` en las barras de tiempos del original.
        Metric::TouchTimes => (
            Color::from_rgb8(0xbb, 0x44, 0xee),
            Color::from_rgb8(0xdd, 0xaa, 0xff),
        ),
    }
}

impl Plot<'_> {
    /// Área de datos dentro del `canvas`.
    fn area(&self, size: Size) -> Rectangle {
        let bottom = if self.chart.metric.per_key() {
            BOTTOM_KEYS
        } else {
            BOTTOM
        };
        Rectangle {
            x: LEFT,
            y: TOP,
            width: (size.width - LEFT - RIGHT).max(1.0),
            height: (size.height - TOP - bottom).max(1.0),
        }
    }

    /// Texto del punto señalado: fecha (o tecla) y valor con su unidad.
    fn describe(&self, index: usize) -> Option<String> {
        let p = self.chart.points.get(index)?;
        let decimals = self.chart.metric.decimals();
        let value = format!("{:.decimals$}{}", p.value, self.unit);
        Some(match p.key {
            Some(key) => format!("{key} · {value}"),
            None => format!("{} · {value}", format_date_time(p.at)),
        })
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

    /// Índice del punto más cercano al mouse, si está sobre el gráfico. En las barras cuenta
    /// sólo la distancia horizontal (la barra ocupa toda la columna); en las líneas, la
    /// distancia al punto dibujado.
    fn hovered(&self, area: Rectangle, cursor: Point) -> Option<usize> {
        if self.chart.points.is_empty() || !area.expand(8.0).contains(cursor) {
            return None;
        }
        let per_key = self.chart.metric.per_key();
        let distance = |i: usize| {
            let dx = Self::x(area, i) - cursor.x;
            if per_key {
                dx.abs()
            } else {
                let dy = self.y(area, self.chart.points[i].value) - cursor.y;
                dx.hypot(dy)
            }
        };
        (0..self.chart.points.len()).min_by(|&a, &b| distance(a).total_cmp(&distance(b)))
    }
}

impl<Message> canvas::Program<Message> for Plot<'_> {
    /// Punto señalado en el último dibujo.
    type State = Option<usize>;

    /// Pide redibujar cuando el mouse cambia de punto señalado; sin esto el `canvas` no se
    /// vuelve a dibujar mientras el mouse se mueve dentro de él.
    fn update(
        &self,
        state: &mut Option<usize>,
        event: &canvas::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        if !matches!(event, canvas::Event::Mouse(_)) {
            return None;
        }
        let area = self.area(bounds.size());
        let hovered = cursor
            .position_in(bounds)
            .and_then(|c| self.hovered(area, c));
        (hovered != *state).then(|| {
            *state = hovered;
            canvas::Action::request_redraw()
        })
    }

    fn draw(
        &self,
        _state: &Option<usize>,
        renderer: &Renderer,
        theme: &Theme,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        let area = self.area(bounds.size());
        let ink = theme.palette().text;
        let decimals = self.chart.axis_decimals();
        let faint = Color { a: 0.25, ..ink };

        for v in self.chart.grid() {
            let y = self.y(area, v);
            frame.stroke(
                &Path::line(Point::new(area.x, y), Point::new(area.x + area.width, y)),
                Stroke::default().with_color(faint).with_width(1.0),
            );
            frame.fill_text(Text {
                content: format!("{v:.decimals$}"),
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
        let hovered = cursor
            .position_in(bounds)
            .and_then(|c| self.hovered(area, c));
        if self.chart.metric.per_key() {
            self.draw_bars(&mut frame, area, &points, ink, hovered);
        } else {
            Self::draw_line(&mut frame, &points, ink, (line, halo));
            if let Some(&p) = hovered.and_then(|i| points.get(i)) {
                frame.stroke(
                    &Path::circle(p, 6.0),
                    Stroke::default().with_color(ink).with_width(2.0),
                );
            }
        }
        if let Some(content) = hovered.and_then(|i| self.describe(i)) {
            frame.fill_text(Text {
                content,
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
        _state: &Option<usize>,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        let area = self.area(bounds.size());
        match cursor.position_in(bounds) {
            Some(c) if self.hovered(area, c).is_some() => mouse::Interaction::Crosshair,
            _ => mouse::Interaction::default(),
        }
    }
}

impl Plot<'_> {
    fn draw_line(frame: &mut Frame, points: &[Point], ink: Color, (line, halo): (Color, Color)) {
        if points.len() > 1 {
            let path = Path::new(|b| {
                for (i, &p) in points.iter().enumerate() {
                    if i == 0 { b.move_to(p) } else { b.line_to(p) }
                }
            });
            frame.stroke(&path, Stroke::default().with_color(halo).with_width(5.0));
            frame.stroke(&path, Stroke::default().with_color(line).with_width(2.0));
        }
        for &p in points {
            frame.fill(&Path::circle(p, 3.5), ink);
            frame.fill(&Path::circle(p, 2.0), halo);
        }
    }

    /// Barras por tecla, con la tecla debajo; la señalada se resalta.
    fn draw_bars(
        &self,
        frame: &mut Frame,
        area: Rectangle,
        points: &[Point],
        ink: Color,
        hovered: Option<usize>,
    ) {
        let (line, halo) = series_colors(self.chart.metric);
        let base = self.y(area, self.chart.min);
        #[allow(clippy::cast_precision_loss)]
        let slot = area.width / (DATA_POINTS + 1) as f32;
        let width = (slot * 0.7).max(2.0);
        for (i, (&p, data)) in points.iter().zip(&self.chart.points).enumerate() {
            let top_left = Point::new(p.x - width / 2.0, p.y);
            let size = Size::new(width, (base - p.y).max(1.0));
            let fill = if hovered == Some(i) { line } else { halo };
            frame.fill_rectangle(top_left, size, fill);
            frame.stroke(
                &Path::rectangle(top_left, size),
                Stroke::default().with_color(line).with_width(1.0),
            );
            if let Some(key) = data.key {
                frame.fill_text(Text {
                    content: key.to_string(),
                    position: Point::new(p.x, base + 4.0),
                    color: ink,
                    size: Pixels(13.0),
                    align_x: Horizontal::Center.into(),
                    ..Text::default()
                });
            }
        }
    }
}
