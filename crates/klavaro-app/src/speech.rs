// SPDX-License-Identifier: GPL-3.0-or-later
//! Modo dictado: qué decir antes de cada tecla, como `tutor_speak_char` y `tutor_speak_word`
//! del original.
//!
//! - Básico y Adaptabilidad dicen el carácter que toca teclear (algunos por su nombre).
//! - Velocidad dice la palabra siguiente cuando el cursor llega al comienzo de una.
//! - Fluidez no dice nada.
//!
//! El dictado del dedo a usar en Básico (a los 3 s sin teclear) no está implementado.

use klavaro_core::Module;
use klavaro_i18n::Translator;

/// Qué decir.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Say {
    /// Un carácter.
    Char(char),
    /// Una palabra.
    Word(String),
}

/// Lo que corresponde decir con el cursor en `cursor` (o nada).
pub(crate) fn utterance(module: Module, target: &[char], cursor: usize) -> Option<Say> {
    let &current = target.get(cursor)?;
    match module {
        Module::Basic | Module::Adaptability => Some(Say::Char(current)),
        Module::Velocity => {
            let at_word_start = cursor == 0 || target.get(cursor - 1).is_some_and(|&c| is_gap(c));
            if !at_word_start || is_gap(current) {
                return None;
            }
            let word: String = target[cursor..]
                .iter()
                .take_while(|&&c| !is_gap(c))
                .collect();
            Some(Say::Word(word))
        }
        Module::Fluidity => None,
    }
}

fn is_gap(c: char) -> bool {
    matches!(c, ' ' | '\n' | '\r')
}

/// Texto para el sintetizador, con los nombres traducidos de las teclas que lo necesitan.
pub(crate) fn spoken(tr: &Translator, say: &Say) -> String {
    match say {
        Say::Word(w) => w.clone(),
        Say::Char(c) => {
            let key = match c {
                ' ' => "speech-space",
                'y' | 'Y' => "speech-wye",
                '\n' => "speech-enter",
                '\'' => "speech-apostrophe",
                '"' => "speech-quote",
                '&' => "speech-ampersand",
                _ => return c.to_string(),
            };
            tr.tr(key)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chars(s: &str) -> Vec<char> {
        s.chars().collect()
    }

    #[test]
    fn says_the_current_char() {
        let t = chars("ab y");
        assert_eq!(utterance(Module::Basic, &t, 1), Some(Say::Char('b')));
        assert_eq!(utterance(Module::Adaptability, &t, 2), Some(Say::Char(' ')));
        assert_eq!(utterance(Module::Basic, &t, 4), None);
        assert_eq!(utterance(Module::Fluidity, &t, 0), None);
    }

    #[test]
    fn says_words_at_their_start() {
        let t = chars("hola mundo\nchau");
        assert_eq!(
            utterance(Module::Velocity, &t, 0),
            Some(Say::Word("hola".into()))
        );
        assert_eq!(utterance(Module::Velocity, &t, 1), None);
        assert_eq!(utterance(Module::Velocity, &t, 4), None);
        assert_eq!(
            utterance(Module::Velocity, &t, 5),
            Some(Say::Word("mundo".into()))
        );
        assert_eq!(
            utterance(Module::Velocity, &t, 11),
            Some(Say::Word("chau".into()))
        );
    }

    #[test]
    fn names_special_keys() {
        let es = Translator::new("es");
        assert_eq!(spoken(&es, &Say::Char(' ')), "espacio");
        assert_eq!(spoken(&es, &Say::Char('\n')), "intro");
        assert_eq!(spoken(&es, &Say::Char('k')), "k");
        assert_eq!(spoken(&es, &Say::Word("hola".into())), "hola");
        assert_eq!(Translator::new("C").tr("speech-voice"), "en");
        assert_eq!(es.tr("speech-voice"), "es");
    }
}
