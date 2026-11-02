// SPDX-License-Identifier: GPL-3.0-or-later
//! Distribuciones de teclado y dedos.
//!
//! Una distribución tiene 4 filas de hasta [`KEY_LINE_LEN`] teclas en dos capas (normal y
//! con Shift), igual que los archivos `.kbd` de Klavaro. Las teclas de control (Tab, Shift,
//! Ctrl…) no se representan; un espacio marca una posición sin tecla.

/// Cantidad de filas de teclas de caracteres.
pub const ROWS: usize = 4;
/// Teclas por fila en el formato `.kbd`.
pub const KEY_LINE_LEN: usize = 14;
/// Teclas visibles por fila en el teclado virtual del original (números, QWERTY, ASDF, ZXCV).
pub const VISIBLE_KEYS: [usize; ROWS] = [14, 13, 12, 12];

/// Una capa del teclado: 4 filas × 14 posiciones.
pub type Layer = [[char; KEY_LINE_LEN]; ROWS];

const EMPTY_LAYER: Layer = [[' '; KEY_LINE_LEN]; ROWS];

/// Posición de una tecla.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct KeyPos {
    /// Fila, de 0 (números) a 3 (fila inferior).
    pub row: usize,
    /// Columna, de 0 a 13.
    pub col: usize,
}

/// Capa en la que está un carácter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Level {
    /// Sin Shift.
    Normal,
    /// Con Shift.
    Shifted,
}

/// Distribución de teclado con nombre (por ejemplo `qwerty_us`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyboardLayout {
    name: String,
    normal: Layer,
    shifted: Layer,
}

impl KeyboardLayout {
    /// Crea una distribución a partir de sus dos capas.
    pub fn new(name: impl Into<String>, normal: Layer, shifted: Layer) -> Self {
        Self {
            name: name.into(),
            normal,
            shifted,
        }
    }

    /// Distribución vacía (todas las posiciones sin tecla), útil para el editor.
    pub fn empty(name: impl Into<String>) -> Self {
        Self::new(name, EMPTY_LAYER, EMPTY_LAYER)
    }

    /// Nombre de la distribución, sin extensión.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Cambia el nombre (al "Guardar como" del editor).
    pub fn set_name(&mut self, name: impl Into<String>) {
        self.name = name.into();
    }

    /// Capa pedida.
    pub fn layer(&self, level: Level) -> &Layer {
        match level {
            Level::Normal => &self.normal,
            Level::Shifted => &self.shifted,
        }
    }

    /// Carácter en una posición, o `None` si no hay tecla.
    pub fn key(&self, pos: KeyPos, level: Level) -> Option<char> {
        let c = *self.layer(level).get(pos.row)?.get(pos.col)?;
        (c != ' ').then_some(c)
    }

    /// País según la convención de nombres de Klavaro (`variante_país[_extra]`):
    /// los dos caracteres después del primer guion bajo, o `xx` si no hay.
    pub fn country(&self) -> String {
        self.name
            .split_once('_')
            .map(|(_, rest)| rest.chars().take(2).collect::<String>())
            .filter(|c| c.chars().count() == 2)
            .unwrap_or_else(|| "xx".to_owned())
    }

    /// Variante según la convención de Klavaro: la parte anterior al primer guion bajo más
    /// lo que sigue al segundo (`dvorak_us_prog` → `dvorak_prog`).
    pub fn variant(&self) -> String {
        let mut parts = self.name.splitn(3, '_');
        let base = parts.next().unwrap_or_default();
        let _country = parts.next();
        match parts.next() {
            Some(extra) if !extra.is_empty() => format!("{base}_{extra}"),
            _ => base.to_owned(),
        }
    }

    /// Busca dónde está un carácter. Las letras se buscan también en su otra capa.
    pub fn locate(&self, c: char) -> Option<(KeyPos, Level)> {
        for level in [Level::Normal, Level::Shifted] {
            for (row, keys) in self.layer(level).iter().enumerate() {
                if let Some(col) = keys.iter().position(|&k| k == c && k != ' ') {
                    return Some((KeyPos { row, col }, level));
                }
            }
        }
        None
    }

    /// Si el carácter se puede teclear con esta distribución.
    pub fn contains(&self, c: char) -> bool {
        c != ' ' && self.locate(c).is_some()
    }

    /// Todos los caracteres de ambas capas, sin repetir, en orden de lectura.
    pub fn chars(&self) -> Vec<char> {
        let mut out = Vec::new();
        for layer in [&self.normal, &self.shifted] {
            for &c in layer.iter().flatten() {
                if c != ' ' && !out.contains(&c) {
                    out.push(c);
                }
            }
        }
        out
    }

    /// Vocales presentes (en minúscula). Si la distribución no tiene ninguna vocal conocida,
    /// usa 5 caracteres de la fila guía, como el original.
    pub fn vowels(&self) -> Vec<char> {
        let mut out: Vec<char> = Vec::new();
        for c in self.chars() {
            let lower = to_lower(c);
            if is_known_vowel(lower) && !out.contains(&lower) {
                out.push(lower);
            }
        }
        if out.is_empty() {
            out = self.normal[2]
                .iter()
                .copied()
                .filter(|&c| c != ' ' && !is_diacritic(c))
                .take(5)
                .collect();
        }
        out
    }

    /// Consonantes: letras que no son vocales. Incluye mayúsculas sólo si la capa con Shift
    /// las ofrece y son distintas de la minúscula.
    pub fn consonants(&self) -> Vec<char> {
        let vowels = self.vowels();
        let mut out = Vec::new();
        for c in self.chars() {
            if c.is_alphabetic() && !vowels.contains(&to_lower(c)) && !out.contains(&c) {
                out.push(c);
            }
        }
        out
    }

    /// Signos de puntuación y símbolos.
    pub fn symbols(&self) -> Vec<char> {
        self.chars()
            .into_iter()
            .filter(|&c| !c.is_alphanumeric() && !c.is_whitespace() && !is_diacritic(c))
            .collect()
    }

    /// Dígitos no ASCII (por ejemplo, arábigo-índicos o devanagari).
    pub fn alt_digits(&self) -> Vec<char> {
        self.chars()
            .into_iter()
            .filter(|&c| c.is_numeric() && !c.is_ascii_digit() && u32::from(c) > 255)
            .collect()
    }

    /// Mayúscula de una letra sólo si la distribución la ofrece en la capa con Shift.
    pub fn to_upper(&self, c: char) -> char {
        let upper = to_upper(c);
        if upper != c && self.shifted.iter().flatten().any(|&k| k == upper) {
            upper
        } else {
            c
        }
    }

    /// Asigna un carácter a una tecla (editor de distribuciones). Si es una letra con
    /// mayúscula/minúscula, también actualiza la otra capa.
    pub fn set_key(&mut self, pos: KeyPos, level: Level, c: char) {
        if pos.row >= ROWS || pos.col >= KEY_LINE_LEN {
            return;
        }
        let c = if c.is_control() { ' ' } else { c };
        match level {
            Level::Normal => {
                self.normal[pos.row][pos.col] = c;
                let upper = to_upper(c);
                if c.is_alphabetic() && upper != c {
                    self.shifted[pos.row][pos.col] = upper;
                }
            }
            Level::Shifted => {
                self.shifted[pos.row][pos.col] = c;
                let lower = to_lower(c);
                if c.is_alphabetic() && lower != c {
                    self.normal[pos.row][pos.col] = lower;
                }
            }
        }
    }
}

/// Dedo responsable de una tecla, con la numeración 1–9 del original
/// (1–4 meñique a índice izquierdos, 5 pulgares, 6–9 índice a meñique derechos).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Finger {
    /// 1
    LeftPinky,
    /// 2
    LeftRing,
    /// 3
    LeftMiddle,
    /// 4
    LeftIndex,
    /// 5
    Thumbs,
    /// 6
    RightIndex,
    /// 7
    RightMiddle,
    /// 8
    RightRing,
    /// 9
    RightPinky,
}

impl Finger {
    /// Convierte el dígito de `fingers_position.txt`.
    pub const fn from_digit(d: char) -> Option<Self> {
        Some(match d {
            '1' => Self::LeftPinky,
            '2' => Self::LeftRing,
            '3' => Self::LeftMiddle,
            '4' => Self::LeftIndex,
            '5' => Self::Thumbs,
            '6' => Self::RightIndex,
            '7' => Self::RightMiddle,
            '8' => Self::RightRing,
            '9' => Self::RightPinky,
            _ => return None,
        })
    }

    /// Número del dedo (1–9), usado también para los colores `key_1`…`key_9`.
    pub const fn number(self) -> u8 {
        match self {
            Self::LeftPinky => 1,
            Self::LeftRing => 2,
            Self::LeftMiddle => 3,
            Self::LeftIndex => 4,
            Self::Thumbs => 5,
            Self::RightIndex => 6,
            Self::RightMiddle => 7,
            Self::RightRing => 8,
            Self::RightPinky => 9,
        }
    }
}

/// Qué dedo usa cada posición (independiente de la distribución).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FingerMap {
    rows: [[Option<Finger>; KEY_LINE_LEN]; ROWS],
}

impl FingerMap {
    /// Crea el mapa desde sus filas.
    pub const fn new(rows: [[Option<Finger>; KEY_LINE_LEN]; ROWS]) -> Self {
        Self { rows }
    }

    /// Dedo de una posición.
    pub fn finger_at(&self, pos: KeyPos) -> Option<Finger> {
        *self.rows.get(pos.row)?.get(pos.col)?
    }

    /// Dedo para teclear un carácter con una distribución. El espacio es de los pulgares.
    pub fn finger_for(&self, layout: &KeyboardLayout, c: char) -> Option<Finger> {
        if c == ' ' {
            return Some(Finger::Thumbs);
        }
        layout.locate(c).and_then(|(pos, _)| self.finger_at(pos))
    }
}

/// Si el carácter es una marca combinante (acentos sueltos, harakat árabes, signos
/// vocálicos tibetanos y similares).
pub fn is_diacritic(c: char) -> bool {
    matches!(u32::from(c),
        0x0300..=0x036F   // Diacríticos combinantes
        | 0x0483..=0x0489 // Cirílico
        | 0x064B..=0x065F // Árabe
        | 0x0670
        | 0x06D6..=0x06ED // Árabe extendido (urdu)
        | 0x0900..=0x0903 | 0x093A..=0x094F // Devanagari
        | 0x0A01..=0x0A03 | 0x0A3C..=0x0A51 | 0x0A70..=0x0A71 // Gurmukhi
        | 0x0F71..=0x0F84 | 0x0F8D..=0x0FBC // Tibetano
        | 0x1AB0..=0x1AFF | 0x1DC0..=0x1DFF | 0x20D0..=0x20FF | 0xFE20..=0xFE2F)
}

/// Vocales que reconoce el original (latinas, griegas y cirílicas), en minúscula.
fn is_known_vowel(c: char) -> bool {
    const VOWELS: &str = "aeiouyáàâãäåæéèêëěíìîïóòôõöøőúùûüűůýÿœ\
                          αεηιουωάέήίόύώϊϋΐΰ\
                          аеёиоуыэюяіїєү";
    VOWELS.contains(c)
}

/// Minúscula de un carácter cuando es exactamente uno.
pub(crate) fn to_lower(c: char) -> char {
    let mut it = c.to_lowercase();
    match (it.next(), it.next()) {
        (Some(l), None) => l,
        _ => c,
    }
}

/// Mayúscula de un carácter cuando es exactamente uno.
pub(crate) fn to_upper(c: char) -> char {
    let mut it = c.to_uppercase();
    match (it.next(), it.next()) {
        (Some(u), None) => u,
        _ => c,
    }
}

#[cfg(test)]
pub(crate) mod test_support {
    use super::{KEY_LINE_LEN, KeyboardLayout, Layer, ROWS};

    fn layer(rows: [&str; ROWS]) -> Layer {
        let mut out = [[' '; KEY_LINE_LEN]; ROWS];
        for (r, line) in rows.iter().enumerate() {
            for (c, ch) in line.chars().take(KEY_LINE_LEN).enumerate() {
                out[r][c] = ch;
            }
        }
        out
    }

    /// QWERTY de EE. UU., la referencia de las pruebas.
    pub(crate) fn qwerty_us() -> KeyboardLayout {
        KeyboardLayout::new(
            "qwerty_us",
            layer([
                "`1234567890-=",
                "qwertyuiop[]\\",
                "asdfghjkl;'",
                "zxcvbnm,./",
            ]),
            layer([
                "~!@#$%^&*()_+",
                "QWERTYUIOP{}|",
                "ASDFGHJKL:\"",
                "ZXCVBNM<>?",
            ]),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::qwerty_us;
    use super::*;

    #[test]
    fn name_parts_follow_klavaro_convention() {
        let k = KeyboardLayout::empty("dvorak_us_prog");
        assert_eq!(k.country(), "us");
        assert_eq!(k.variant(), "dvorak_prog");
        let plain = KeyboardLayout::empty("qwerty_br");
        assert_eq!(plain.variant(), "qwerty");
        assert_eq!(KeyboardLayout::empty("custom").country(), "xx");
    }

    #[test]
    fn classifies_characters() {
        let k = qwerty_us();
        assert_eq!(k.vowels(), vec!['e', 'y', 'u', 'i', 'o', 'a']);
        assert!(k.consonants().contains(&'q'));
        assert!(k.consonants().contains(&'Q'));
        assert!(!k.consonants().contains(&'a'));
        assert!(k.symbols().contains(&';'));
        assert!(!k.symbols().contains(&'a'));
        assert_eq!(k.alt_digits(), [] as [char; 0]);
    }

    #[test]
    fn locates_keys_in_both_levels() {
        let k = qwerty_us();
        assert_eq!(
            k.locate('a'),
            Some((KeyPos { row: 2, col: 0 }, Level::Normal))
        );
        assert_eq!(
            k.locate('A'),
            Some((KeyPos { row: 2, col: 0 }, Level::Shifted))
        );
        assert!(!k.contains('ñ'));
        assert!(!k.contains(' '));
    }

    #[test]
    fn to_upper_only_when_layout_offers_it() {
        let k = qwerty_us();
        assert_eq!(k.to_upper('a'), 'A');
        assert_eq!(k.to_upper('ñ'), 'ñ');
    }

    #[test]
    fn editing_a_letter_updates_both_levels() {
        let mut k = KeyboardLayout::empty("custom_xx");
        let pos = KeyPos { row: 2, col: 9 };
        k.set_key(pos, Level::Normal, 'ñ');
        assert_eq!(k.key(pos, Level::Normal), Some('ñ'));
        assert_eq!(k.key(pos, Level::Shifted), Some('Ñ'));
        k.set_key(pos, Level::Shifted, ':');
        assert_eq!(k.key(pos, Level::Shifted), Some(':'));
        assert_eq!(k.key(pos, Level::Normal), Some('ñ'));
    }

    #[test]
    fn finger_map_lookup() {
        let mut rows = [[None; KEY_LINE_LEN]; ROWS];
        rows[2][0] = Finger::from_digit('1');
        let map = FingerMap::new(rows);
        let k = qwerty_us();
        assert_eq!(map.finger_for(&k, 'A'), Some(Finger::LeftPinky));
        assert_eq!(map.finger_for(&k, ' '), Some(Finger::Thumbs));
        assert_eq!(map.finger_for(&k, 'ñ'), None);
    }

    #[test]
    fn diacritics() {
        assert!(is_diacritic('\u{0301}'));
        assert!(is_diacritic('\u{064E}'));
        assert!(!is_diacritic('a'));
    }
}
