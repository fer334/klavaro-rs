// SPDX-License-Identifier: GPL-3.0-or-later
//! Teclado virtual con los dedos (ventana de ayuda del original), dibujado con el `canvas`.
//!
//! Cada tecla lleva el color de su dedo (`KEYB_*` de `keyboard.h`: el mismo color para el
//! mismo dedo de cada mano). La tecla siguiente se resalta, y también el Shift de la otra
//! mano cuando hace falta. Las filas siguen la forma ISO: 14 teclas arriba, Tab, Bloq Mayús,
//! Shift izquierdo con la tecla extra, Enter a la derecha y la barra espaciadora.

use iced::alignment::{Horizontal, Vertical};
use iced::widget::canvas::{self, Frame, Geometry, Path, Stroke, Text};
use iced::{Color, Pixels, Point, Rectangle, Renderer, Size, Theme, mouse};

use klavaro_core::keyboard::{
    Finger, FingerMap, KeyPos, KeyboardLayout, Level, ROWS, VISIBLE_KEYS,
};

use crate::hints::{Hint, Side, Target};
use crate::style::Colors;

/// Ancho total de una fila, en teclas.
const ROW_UNITS: f32 = 15.0;
/// Separación entre teclas, en fracción del tamaño de una tecla.
const GAP: f32 = 0.08;
/// Lado máximo de una tecla, en píxeles.
const MAX_KEY: f32 = 44.0;

/// Dónde empieza cada fila (las teclas de función ocupan el resto).
const ROW_START: [f32; ROWS] = [0.0, 1.5, 1.75, 1.25];

/// Color de un dedo, como `hints_color_from_charcode`.
fn finger_color(finger: Finger) -> Color {
    match finger.number() {
        1 | 9 => Color::from_rgb8(0xbb, 0xbb, 0xff),
        2 | 8 => Color::from_rgb8(0xee, 0xaa, 0xaa),
        3 | 7 => Color::from_rgb8(0xaa, 0xee, 0xbb),
        4 => Color::from_rgb8(0xee, 0xee, 0x88),
        6 => Color::from_rgb8(0xff, 0xcc, 0x77),
        _ => Color::from_rgb8(0xcc, 0xaa, 0xcc),
    }
}

/// Teclado listo para el `canvas`.
pub(crate) struct Keys<'a> {
    pub(crate) layout: &'a KeyboardLayout,
    pub(crate) fingers: &'a FingerMap,
    /// Ayuda para el carácter siguiente.
    pub(crate) hint: Option<Hint>,
}

/// Una tecla dibujada: posición en teclas, ancho, dedo, etiqueta y si se resalta.
struct KeyBox {
    x: f32,
    row: usize,
    width: f32,
    finger: Option<Finger>,
    label: String,
    lit: bool,
}

impl Keys<'_> {
    /// Alto que necesita el teclado para un ancho dado.
    pub(crate) fn height() -> f32 {
        MAX_KEY * 5.0 + 8.0
    }

    fn boxes(&self) -> Vec<KeyBox> {
        let hint = self.hint;
        let target = hint.map(|h| h.target);
        let shift = hint.and_then(|h| h.shift);
        let special = |x: f32, row: usize, width: f32, label: &str, finger, lit| KeyBox {
            x,
            row,
            width,
            finger,
            label: label.to_owned(),
            lit,
        };
        let mut out = vec![
            special(0.0, 1, 1.5, "⇥", Some(Finger::LeftPinky), false),
            special(0.0, 2, 1.75, "⇪", Some(Finger::LeftPinky), false),
            special(
                0.0,
                3,
                1.25,
                "⇧",
                Some(Finger::LeftPinky),
                shift == Some(Side::Left),
            ),
            special(
                ROW_START[2] + 12.0,
                2,
                ROW_UNITS - ROW_START[2] - 12.0,
                "⏎",
                Some(Finger::RightPinky),
                target == Some(Target::Enter),
            ),
            special(
                ROW_START[3] + 12.0,
                3,
                ROW_UNITS - ROW_START[3] - 12.0,
                "⇧",
                Some(Finger::RightPinky),
                shift == Some(Side::Right),
            ),
            special(
                f32::from(u8::try_from(VISIBLE_KEYS[0]).unwrap_or(14)),
                0,
                1.0,
                "⌫",
                Some(Finger::RightPinky),
                false,
            ),
            special(
                4.0,
                4,
                6.0,
                "",
                Some(Finger::Thumbs),
                target == Some(Target::Space),
            ),
        ];
        for (row, &count) in VISIBLE_KEYS.iter().enumerate() {
            for col in 0..count {
                let pos = KeyPos { row, col };
                let normal = self.layout.key(pos, Level::Normal).unwrap_or(' ');
                let shifted = self.layout.key(pos, Level::Shifted).unwrap_or(' ');
                let label = match (normal, shifted) {
                    (' ', ' ') => String::new(),
                    (c, ' ') | (' ', c) => c.to_string(),
                    // Letras: basta con la mayúscula, como en un teclado de verdad.
                    (n, s) if n.to_uppercase().eq(s.to_uppercase()) => s.to_string(),
                    (n, s) => format!("{s}\n{n}"),
                };
                #[allow(clippy::cast_precision_loss)]
                let x = ROW_START[row] + col as f32;
                out.push(KeyBox {
                    x,
                    row,
                    width: 1.0,
                    finger: self.fingers.finger_at(pos),
                    label,
                    lit: target == Some(Target::Key(pos)),
                });
            }
        }
        out
    }
}

impl<Message> canvas::Program<Message> for Keys<'_> {
    type State = ();

    fn draw(
        &self,
        _state: &(),
        renderer: &Renderer,
        theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        let colors = Colors::of(theme);
        let dark = theme.extended_palette().is_dark;
        let unit = (bounds.width / ROW_UNITS).min(MAX_KEY);
        let left = (bounds.width - unit * ROW_UNITS) / 2.0;
        let gap = unit * GAP;

        for k in self.boxes() {
            #[allow(clippy::cast_precision_loss)]
            let top_left = Point::new(
                left + k.x * unit + gap / 2.0,
                k.row as f32 * unit + gap / 2.0,
            );
            let size = Size::new(k.width * unit - gap, unit - gap);
            let base = k.finger.map_or(colors.sunken, finger_color);
            // En modo oscuro los colores del original se apagan para no encandilar.
            let fill = if k.lit {
                base
            } else if dark {
                Color { a: 0.35, ..base }
            } else {
                Color { a: 0.75, ..base }
            };
            let key = Path::rounded_rectangle(top_left, size, (unit * 0.12).into());
            frame.fill(&key, fill);
            let (border, width) = if k.lit {
                (colors.primary, 3.0)
            } else {
                (colors.border, 1.0)
            };
            frame.stroke(&key, Stroke::default().with_color(border).with_width(width));
            if !k.label.is_empty() {
                let text_color = if k.lit && dark {
                    Color::BLACK
                } else {
                    colors.text
                };
                frame.fill_text(Text {
                    content: k.label,
                    position: Point::new(
                        top_left.x + size.width / 2.0,
                        top_left.y + size.height / 2.0,
                    ),
                    color: text_color,
                    size: Pixels((unit * 0.36).max(10.0)),
                    align_x: Horizontal::Center.into(),
                    align_y: Vertical::Center,
                    ..Text::default()
                });
            }
        }
        vec![frame.into_geometry()]
    }
}
