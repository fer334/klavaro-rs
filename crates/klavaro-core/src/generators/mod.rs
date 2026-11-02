// SPDX-License-Identifier: GPL-3.0-or-later
//! Generadores del texto de cada módulo.
//!
//! Todos reciben el azar por parámetro y devuelven el texto objetivo listo para una
//! [`Session`](crate::session::Session): las líneas y párrafos terminan en `\n`, que se
//! teclea con Enter.

pub mod adaptability;
pub mod basic;
pub mod fluidity;
pub mod velocity;

/// Reglas tipográficas que dependen del idioma de la interfaz.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LanguageRules {
    code: String,
}

impl LanguageRules {
    /// Reglas para un código de idioma (`es`, `pt_BR`, `bo`…). `C` es inglés.
    pub fn new(code: impl Into<String>) -> Self {
        Self { code: code.into() }
    }

    fn prefix(&self) -> &str {
        self.code.get(..2).unwrap_or(&self.code)
    }

    /// Si es tibetano (separa sílabas con tsheg en lugar de espacios).
    pub fn is_tibetan(&self) -> bool {
        self.prefix() == "bo"
    }

    /// Separador de palabras.
    pub fn word_separator(&self) -> char {
        if self.is_tibetan() { '\u{0F0B}' } else { ' ' }
    }

    /// Signo de fin de frase, si el idioma usa uno al final de los ejercicios.
    pub fn stop_mark(&self) -> Option<char> {
        match self.prefix() {
            "ur" => Some('\u{06D4}'),
            "pa" => Some('\u{0964}'),
            "bo" => Some('\u{0F0D}'),
            "ar" | "bn" => None,
            _ => Some('.'),
        }
    }

    /// Coma del idioma.
    pub fn comma(&self) -> char {
        match self.prefix() {
            "ar" | "ur" => '\u{060C}',
            _ => ',',
        }
    }
}

impl Default for LanguageRules {
    fn default() -> Self {
        Self::new("C")
    }
}

/// Pone en mayúscula el primer carácter.
pub(crate) fn capitalize_first(word: &str) -> String {
    let mut chars = word.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// Cierra un párrafo: reemplaza el último separador por el signo de fin y agrega `\n`.
pub(crate) fn close_paragraph(words: &[String], rules: &LanguageRules) -> String {
    let sep = rules.word_separator().to_string();
    let mut out = words.join(&sep);
    if let Some(stop) = rules.stop_mark() {
        out.push(stop);
    }
    out.push('\n');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn language_rules() {
        assert_eq!(LanguageRules::new("es").stop_mark(), Some('.'));
        assert_eq!(LanguageRules::new("ar").stop_mark(), None);
        assert_eq!(LanguageRules::new("ur").comma(), '،');
        assert_eq!(LanguageRules::new("bo").word_separator(), '་');
        assert_eq!(LanguageRules::default().stop_mark(), Some('.'));
    }

    #[test]
    fn capitalize() {
        assert_eq!(capitalize_first("ñandú"), "Ñandú");
        assert_eq!(capitalize_first(""), "");
    }

    #[test]
    fn close() {
        let words = vec!["hola".to_owned(), "mundo".to_owned()];
        assert_eq!(
            close_paragraph(&words, &LanguageRules::new("es")),
            "hola mundo.\n"
        );
    }
}
