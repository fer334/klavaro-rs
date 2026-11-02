// SPDX-License-Identifier: GPL-3.0-or-later
//! Widget invisible que pide el método de entrada del sistema mientras se teclea un ejercicio.
//!
//! En macOS las teclas muertas (´, ¨, ˆ…) sólo se combinan con la tecla siguiente cuando la
//! ventana tiene activado el método de entrada: sin él, el acento llega como una tecla suelta
//! y la letra sin acento como otra. Con él, el acento queda pendiente y la letra combinada
//! llega en un [`iced::advanced::input_method::Event::Commit`]. En Linux y Windows el sistema
//! ya combina las teclas muertas en el texto de la tecla, así que no se pide nada.

use iced::advanced::input_method::{InputMethod, Purpose};
use iced::advanced::layout::{self, Layout};
use iced::advanced::widget::Tree;
use iced::advanced::{Clipboard, Shell, Widget, mouse, renderer};
use iced::{Element, Event, Length, Rectangle, Size, window};

/// Pide el método de entrada en cada cuadro mientras está en pantalla.
///
/// Va como capa transparente sobre el texto del ejercicio y ocupa todo su espacio: `column`
/// y `stack` descartan sin aviso a los hijos que miden 0 en algún eje, así que no puede
/// medir cero. No dibuja nada ni toma el mouse, y la ventana de candidatos del sistema
/// aparece sobre el texto.
pub(crate) struct Typing;

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer> for Typing
where
    Renderer: renderer::Renderer,
{
    fn size(&self) -> Size<Length> {
        Size::new(Length::Fill, Length::Fill)
    }

    fn layout(
        &mut self,
        _tree: &mut Tree,
        _renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        layout::Node::new(limits.max())
    }

    fn draw(
        &self,
        _tree: &Tree,
        _renderer: &mut Renderer,
        _theme: &Theme,
        _style: &renderer::Style,
        _layout: Layout<'_>,
        _cursor: mouse::Cursor,
        _viewport: &Rectangle,
    ) {
    }

    fn update(
        &mut self,
        _tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        _cursor: mouse::Cursor,
        _renderer: &Renderer,
        _clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) {
        // iced sólo atiende el pedido durante `RedrawRequested`, y hay que repetirlo en cada
        // cuadro para que siga activo.
        if cfg!(target_os = "macos")
            && matches!(event, Event::Window(window::Event::RedrawRequested(_)))
        {
            shell.request_input_method(&InputMethod::<&str>::Enabled {
                cursor: layout.bounds(),
                purpose: Purpose::Normal,
                preedit: None,
            });
        }
    }
}

impl<'a, Message, Theme, Renderer> From<Typing> for Element<'a, Message, Theme, Renderer>
where
    Renderer: renderer::Renderer,
    Message: 'a,
{
    fn from(typing: Typing) -> Self {
        Element::new(typing)
    }
}
