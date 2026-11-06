// SPDX-License-Identifier: GPL-3.0-or-later
//! Tema y estilos de la interfaz: colores, tarjetas, botones, avisos y campos.
//!
//! Los widgets de iced toman el color de la paleta del tema; lo que la paleta no cubre
//! (superficies, bordes, texto atenuado) sale de [`Colors`].

use iced::border::Radius;
use iced::widget::{button, container, pick_list, progress_bar, text, text_input};
use iced::{Background, Border, Color, Font, Shadow, Theme, Vector};

/// Radio de las esquinas de botones y campos.
const RADIUS: f32 = 8.0;
/// Radio de las esquinas de las tarjetas.
const CARD_RADIUS: f32 = 12.0;

/// Fuente de títulos y valores. Con `Weight::Bold` iced cae en una monoespaciada cuando el
/// sistema no tiene la variante en negrita de la fuente por defecto, así que se usa la normal.
pub(crate) const BOLD: Font = Font::DEFAULT;

/// Colores de la interfaz, en claro u oscuro.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Colors {
    pub(crate) background: Color,
    /// Fondo de tarjetas y botones secundarios.
    pub(crate) surface: Color,
    /// Fondo de filas alternas y elementos apagados.
    pub(crate) sunken: Color,
    pub(crate) border: Color,
    pub(crate) text: Color,
    /// Texto secundario: ayudas, etiquetas, unidades.
    pub(crate) muted: Color,
    pub(crate) primary: Color,
    pub(crate) success: Color,
    pub(crate) warning: Color,
    pub(crate) danger: Color,
}

impl Colors {
    const LIGHT: Self = Self {
        background: Color::from_rgb8(0xf3, 0xf4, 0xf8),
        surface: Color::WHITE,
        sunken: Color::from_rgb8(0xf6, 0xf7, 0xfa),
        border: Color::from_rgb8(0xdf, 0xe2, 0xea),
        text: Color::from_rgb8(0x1e, 0x22, 0x30),
        muted: Color::from_rgb8(0x6b, 0x71, 0x86),
        primary: Color::from_rgb8(0x4c, 0x5a, 0xd9),
        success: Color::from_rgb8(0x23, 0x8c, 0x4e),
        warning: Color::from_rgb8(0xb5, 0x7a, 0x0e),
        danger: Color::from_rgb8(0xcc, 0x33, 0x40),
    };

    const DARK: Self = Self {
        background: Color::from_rgb8(0x14, 0x16, 0x1c),
        surface: Color::from_rgb8(0x1e, 0x21, 0x2a),
        sunken: Color::from_rgb8(0x19, 0x1b, 0x23),
        border: Color::from_rgb8(0x2e, 0x32, 0x3e),
        text: Color::from_rgb8(0xe4, 0xe7, 0xef),
        muted: Color::from_rgb8(0x8e, 0x94, 0xa6),
        primary: Color::from_rgb8(0x7e, 0x89, 0xf7),
        success: Color::from_rgb8(0x5c, 0xc9, 0x8a),
        warning: Color::from_rgb8(0xe3, 0xb3, 0x5a),
        danger: Color::from_rgb8(0xf0, 0x6a, 0x6a),
    };

    pub(crate) fn new(dark: bool) -> Self {
        if dark { Self::DARK } else { Self::LIGHT }
    }

    pub(crate) fn of(theme: &Theme) -> Self {
        Self::new(theme.extended_palette().is_dark)
    }
}

/// Tema de iced con los colores de [`Colors`].
pub(crate) fn theme(dark: bool) -> Theme {
    let c = Colors::new(dark);
    let palette = iced::theme::Palette {
        background: c.background,
        text: c.text,
        primary: c.primary,
        success: c.success,
        warning: c.warning,
        danger: c.danger,
    };
    Theme::custom(
        if dark {
            "klavaro-dark"
        } else {
            "klavaro-light"
        },
        palette,
    )
}

/// Mezcla `a` con `b` en proporción `t` (0 = `a`).
fn mix(a: Color, b: Color, t: f32) -> Color {
    Color::from_rgb(
        a.r + (b.r - a.r) * t,
        a.g + (b.g - a.g) * t,
        a.b + (b.b - a.b) * t,
    )
}

fn border(color: Color, radius: f32) -> Border {
    Border {
        color,
        width: 1.0,
        radius: Radius::from(radius),
    }
}

fn soft_shadow(c: &Colors) -> Shadow {
    Shadow {
        color: Color {
            a: if c.surface == Color::WHITE {
                0.06
            } else {
                0.25
            },
            ..Color::BLACK
        },
        offset: Vector::new(0.0, 1.0),
        blur_radius: 4.0,
    }
}

/// Tarjeta: superficie con borde y sombra suave.
pub(crate) fn card(theme: &Theme) -> container::Style {
    let c = Colors::of(theme);
    container::Style {
        text_color: Some(c.text),
        background: Some(Background::Color(c.surface)),
        border: border(c.border, CARD_RADIUS),
        shadow: soft_shadow(&c),
        snap: true,
    }
}

/// Fila alterna de una tabla.
pub(crate) fn stripe(theme: &Theme) -> container::Style {
    let c = Colors::of(theme);
    container::Style {
        background: Some(Background::Color(c.sunken)),
        border: Border {
            radius: Radius::from(6.0),
            ..Border::default()
        },
        ..container::Style::default()
    }
}

/// Círculo con el número de un módulo.
pub(crate) fn badge(theme: &Theme) -> container::Style {
    let c = Colors::of(theme);
    container::Style {
        text_color: Some(c.primary),
        background: Some(Background::Color(mix(c.surface, c.primary, 0.14))),
        border: Border {
            radius: Radius::from(999.0),
            ..Border::default()
        },
        ..container::Style::default()
    }
}

/// Aviso con el color de acento dado (error, información).
fn banner(theme: &Theme, accent: impl Fn(&Colors) -> Color) -> container::Style {
    let c = Colors::of(theme);
    let accent = accent(&c);
    container::Style {
        text_color: Some(mix(accent, c.text, 0.35)),
        background: Some(Background::Color(mix(c.surface, accent, 0.12))),
        border: border(mix(c.surface, accent, 0.4), RADIUS),
        ..container::Style::default()
    }
}

/// Aviso de error.
pub(crate) fn error_banner(theme: &Theme) -> container::Style {
    banner(theme, |c| c.danger)
}

/// Aviso informativo.
pub(crate) fn info_banner(theme: &Theme) -> container::Style {
    banner(theme, |c| c.primary)
}

/// Texto atenuado.
pub(crate) fn muted(theme: &Theme) -> text::Style {
    text::Style {
        color: Some(Colors::of(theme).muted),
    }
}

/// Botón principal (guardar).
pub(crate) fn primary(theme: &Theme, status: button::Status) -> button::Style {
    let mut s = button::primary(theme, status);
    s.border.radius = Radius::from(RADIUS);
    s
}

/// Botón secundario: superficie con borde que se tiñe al pasar el mouse.
pub(crate) fn secondary(theme: &Theme, status: button::Status) -> button::Style {
    let c = Colors::of(theme);
    let (bg, edge) = match status {
        button::Status::Hovered => (
            mix(c.surface, c.primary, 0.08),
            mix(c.border, c.primary, 0.5),
        ),
        button::Status::Pressed => (mix(c.surface, c.primary, 0.16), c.primary),
        button::Status::Active | button::Status::Disabled => (c.surface, c.border),
    };
    button::Style {
        background: Some(Background::Color(bg)),
        text_color: if status == button::Status::Disabled {
            c.muted
        } else {
            c.text
        },
        border: border(edge, RADIUS),
        shadow: Shadow::default(),
        snap: true,
    }
}

/// Botón sin fondo (volver, cambiar colores).
pub(crate) fn ghost(theme: &Theme, status: button::Status) -> button::Style {
    let c = Colors::of(theme);
    let bg = match status {
        button::Status::Hovered => Some(mix(c.background, c.primary, 0.1)),
        button::Status::Pressed => Some(mix(c.background, c.primary, 0.18)),
        button::Status::Active | button::Status::Disabled => None,
    };
    button::Style {
        background: bg.map(Background::Color),
        text_color: c.text,
        border: Border {
            radius: Radius::from(RADIUS),
            ..Border::default()
        },
        shadow: Shadow::default(),
        snap: true,
    }
}

/// Opción de un grupo (módulo o gráfico): rellena si está elegida.
pub(crate) fn chip(active: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |theme, status| {
        let mut s = if active {
            button::primary(theme, status)
        } else {
            secondary(theme, status)
        };
        s.border.radius = Radius::from(999.0);
        s
    }
}

/// Tarjeta de módulo del menú: se resalta el borde al pasar el mouse.
pub(crate) fn module_card(theme: &Theme, status: button::Status) -> button::Style {
    let c = Colors::of(theme);
    let (bg, edge) = match status {
        button::Status::Hovered => (mix(c.surface, c.primary, 0.05), c.primary),
        button::Status::Pressed => (mix(c.surface, c.primary, 0.12), c.primary),
        button::Status::Active | button::Status::Disabled => (c.surface, c.border),
    };
    button::Style {
        background: Some(Background::Color(bg)),
        text_color: c.text,
        border: border(edge, CARD_RADIUS),
        shadow: soft_shadow(&c),
        snap: true,
    }
}

/// Casilla de una grilla de la Progresión: la elegida lleva el borde resaltado y una bloqueada
/// se ve apagada.
pub(crate) fn tile(selected: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |theme, status| {
        let c = Colors::of(theme);
        let (bg, edge) = match status {
            button::Status::Hovered => (mix(c.surface, c.primary, 0.05), c.primary),
            button::Status::Pressed => (mix(c.surface, c.primary, 0.12), c.primary),
            button::Status::Active => (c.surface, c.border),
            button::Status::Disabled => (c.sunken, c.border),
        };
        button::Style {
            background: Some(Background::Color(bg)),
            text_color: c.text,
            border: border(if selected { c.primary } else { edge }, RADIUS),
            shadow: Shadow::default(),
            snap: true,
        }
    }
}

/// Lista desplegable con las esquinas de los demás campos.
pub(crate) fn pick(theme: &Theme, status: pick_list::Status) -> pick_list::Style {
    let c = Colors::of(theme);
    let mut s = pick_list::default(theme, status);
    s.background = Background::Color(c.surface);
    s.border.radius = Radius::from(RADIUS);
    if matches!(status, pick_list::Status::Active) {
        s.border.color = c.border;
    }
    s
}

/// Campo de texto; con `invalid` el borde queda en rojo.
pub(crate) fn input(invalid: bool) -> impl Fn(&Theme, text_input::Status) -> text_input::Style {
    move |theme, status| {
        let c = Colors::of(theme);
        let mut s = text_input::default(theme, status);
        s.background = Background::Color(c.surface);
        s.border.radius = Radius::from(RADIUS);
        if matches!(status, text_input::Status::Active) {
            s.border.color = c.border;
        }
        if invalid {
            s.border.color = c.danger;
            s.border.width = 2.0;
        }
        s
    }
}

/// Barra de avance del ejercicio.
pub(crate) fn progress(theme: &Theme) -> progress_bar::Style {
    let c = Colors::of(theme);
    progress_bar::Style {
        background: Background::Color(c.border),
        bar: Background::Color(c.primary),
        border: Border {
            radius: Radius::from(999.0),
            ..Border::default()
        },
    }
}
