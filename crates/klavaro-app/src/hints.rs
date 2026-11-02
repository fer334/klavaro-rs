// SPDX-License-Identifier: GPL-3.0-or-later
//! Qué tecla y qué dedo usar para el carácter siguiente (`hints_update_from_char` y
//! `hints_finger_name_from_char` del original), independiente de iced.
//!
//! El dedo de cada posición sale de `fingers_position.txt`; el espacio es de los pulgares y
//! Enter del meñique derecho. Para un carácter con Shift se indica además la tecla Shift de la
//! otra mano, como enseña la introducción del original.

use klavaro_core::keyboard::{Finger, FingerMap, KeyPos, KeyboardLayout, Level};
use klavaro_i18n::Translator;

/// Mano.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Side {
    /// Izquierda.
    Left,
    /// Derecha.
    Right,
}

/// Mano de un dedo (`None` para los pulgares).
pub(crate) fn side(finger: Finger) -> Option<Side> {
    match finger.number() {
        1..=4 => Some(Side::Left),
        6..=9 => Some(Side::Right),
        _ => None,
    }
}

/// Tecla a resaltar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Target {
    /// Una tecla de las cuatro filas.
    Key(KeyPos),
    /// La barra espaciadora.
    Space,
    /// Enter.
    Enter,
}

/// Ayuda para un carácter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Hint {
    /// Tecla.
    pub(crate) target: Target,
    /// Dedo que la pulsa.
    pub(crate) finger: Option<Finger>,
    /// Shift a pulsar, del lado opuesto al dedo.
    pub(crate) shift: Option<Side>,
}

/// Ayuda para teclear `c`, o `None` si el teclado no lo tiene.
pub(crate) fn hint(layout: &KeyboardLayout, fingers: &FingerMap, c: char) -> Option<Hint> {
    match c {
        ' ' => Some(Hint {
            target: Target::Space,
            finger: Some(Finger::Thumbs),
            shift: None,
        }),
        '\n' => Some(Hint {
            target: Target::Enter,
            finger: Some(Finger::RightPinky),
            shift: None,
        }),
        _ => {
            let (pos, level) = layout.locate(c)?;
            let finger = fingers.finger_at(pos);
            let shift = (level == Level::Shifted)
                .then(|| finger.and_then(side))
                .flatten()
                .map(|s| match s {
                    Side::Left => Side::Right,
                    Side::Right => Side::Left,
                });
            Some(Hint {
                target: Target::Key(pos),
                finger,
                shift,
            })
        }
    }
}

/// Nombre del dedo para dictar (sólo el dedo, como el original).
pub(crate) fn finger_name(tr: &Translator, finger: Finger) -> String {
    tr.tr(match finger.number() {
        1 | 9 => "finger-small",
        2 | 8 => "finger-ring",
        3 | 7 => "finger-middle",
        4 | 6 => "finger-index",
        _ => "finger-thumbs",
    })
}

/// Dedo y mano para mostrar («dedo índice (mano derecha)»).
pub(crate) fn finger_text(tr: &Translator, finger: Finger) -> String {
    let name = finger_name(tr, finger);
    match side(finger) {
        Some(Side::Left) => tr.tr_args("hint-left", &[("finger", &name)]),
        Some(Side::Right) => tr.tr_args("hint-right", &[("finger", &name)]),
        None => name,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assets;

    fn setup() -> (KeyboardLayout, FingerMap) {
        (
            assets::keyboard("qwerty_us").expect("kbd"),
            assets::fingers().expect("dedos"),
        )
    }

    #[test]
    fn home_row_and_specials() {
        let (k, f) = setup();
        let a = hint(&k, &f, 'a').expect("a");
        assert_eq!(a.target, Target::Key(KeyPos { row: 2, col: 0 }));
        assert_eq!(a.finger, Some(Finger::LeftPinky));
        assert_eq!(a.shift, None);
        let j = hint(&k, &f, 'j').expect("j");
        assert_eq!(j.finger, Some(Finger::RightIndex));
        assert_eq!(hint(&k, &f, ' ').map(|h| h.target), Some(Target::Space));
        assert_eq!(hint(&k, &f, '\n').map(|h| h.target), Some(Target::Enter));
        assert_eq!(hint(&k, &f, 'ñ'), None);
    }

    #[test]
    fn capitals_use_the_opposite_shift() {
        let (k, f) = setup();
        assert_eq!(hint(&k, &f, 'A').and_then(|h| h.shift), Some(Side::Right));
        assert_eq!(hint(&k, &f, 'J').and_then(|h| h.shift), Some(Side::Left));
    }

    #[test]
    fn names_fingers_with_their_hand() {
        let es = Translator::new("es");
        assert_eq!(finger_name(&es, Finger::RightIndex), "dedo índice");
        assert_eq!(
            finger_text(&es, Finger::LeftPinky),
            "dedo meñique (mano izquierda)"
        );
        assert_eq!(finger_text(&es, Finger::Thumbs), "pulgares");
    }
}
