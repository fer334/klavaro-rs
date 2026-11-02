// SPDX-License-Identifier: GPL-3.0-or-later
//! Historial y preferencias del Klavaro original, para importarlos.
//!
//! - `stat_basic.txt`, `stat_adapt.txt`, `stat_velo.txt`, `stat_fluid.txt`: una fila por
//!   sesión (precisión, velocidad, fluidez, fecha, hora, lección, teclado o idioma).
//! - `scores_fluid.txt`: puntaje, fecha, hora, caracteres, idioma.
//! - `preferences.ini`: formato INI de `GKeyFile`.
//! - `accuracy.log_<teclado>`: errores por carácter (carácter, errores, aciertos).
//! - `proficiency.log_<teclado>`: tiempo promedio por carácter (carácter, segundos, relativo).
//! - `ksc/local_<xx>.ksc`: Top 10 local de un idioma, en binario (ver [`parse_ksc`]).
//!
//! Los números usan punto decimal (el original escribe con el locale `C`).

use std::collections::BTreeMap;

use crate::{ParseError, ParseErrorKind, lines};

/// Una fila de `stat_<módulo>.txt`.
#[derive(Debug, Clone, PartialEq)]
pub struct StatRecord {
    /// Precisión (%).
    pub accuracy: f64,
    /// Velocidad (PPM).
    pub wpm: f64,
    /// Fluidez.
    pub fluidity: f64,
    /// Fecha `AAAA-MM-DD`.
    pub date: String,
    /// Hora `HH:MM`.
    pub time: String,
    /// Lección (Básico) o nombre de la lista de textos (Velocidad, Fluidez); en
    /// Adaptabilidad, el teclado.
    pub lesson: String,
    /// Teclado (Básico) o idioma (Adaptabilidad, Velocidad, Fluidez).
    pub tag: String,
}

/// Una fila de `scores_fluid.txt`.
#[derive(Debug, Clone, PartialEq)]
pub struct ScoreRecord {
    /// Puntaje.
    pub score: f64,
    /// Fecha `AAAA-MM-DD`.
    pub date: String,
    /// Hora `HH:MM`.
    pub time: String,
    /// Caracteres tecleados.
    pub chars: u32,
    /// Idioma.
    pub language: String,
}

/// Resultado de importar un archivo de historial: filas válidas y errores de las demás.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Imported<T> {
    /// Filas válidas.
    pub records: Vec<T>,
    /// Filas descartadas.
    pub errors: Vec<ParseError>,
}

/// Lee un `stat_<módulo>.txt`. La primera línea es el encabezado.
pub fn parse_stats(text: &str) -> Imported<StatRecord> {
    parse_rows(text, 7, |f| {
        Ok(StatRecord {
            accuracy: number(f[0])?,
            wpm: number(f[1])?,
            fluidity: number(f[2])?,
            date: f[3].to_owned(),
            time: f[4].to_owned(),
            lesson: f[5].to_owned(),
            tag: f[6..].join(" "),
        })
    })
}

/// Lee `scores_fluid.txt`. La primera línea es el encabezado.
pub fn parse_scores(text: &str) -> Imported<ScoreRecord> {
    parse_rows(text, 5, |f| {
        Ok(ScoreRecord {
            score: number(f[0])?,
            date: f[1].to_owned(),
            time: f[2].to_owned(),
            chars: f[3]
                .parse()
                .map_err(|_| ParseErrorKind::InvalidField(f[3].to_owned()))?,
            language: f[4].to_owned(),
        })
    })
}

/// Errores de un carácter en `accuracy.log_<teclado>`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AccuracyRow {
    /// Carácter.
    pub ch: char,
    /// Veces tecleado mal.
    pub wrong: u32,
    /// Aciertos desde el último olvido.
    pub correct: u32,
}

/// Lee `accuracy.log_<teclado>`. Como `accur_init` del original, se detiene en la primera
/// línea mal formada o fuera de rango (errores entre 1 y 1233, aciertos hasta `inertia`).
pub fn parse_accuracy_log(text: &str, inertia: u32) -> Vec<AccuracyRow> {
    lines(text)
        .map_while(|line| {
            let mut f = line.split_whitespace();
            let ch = single_char(f.next()?)?;
            let wrong: u32 = f.next()?.parse().ok()?;
            let correct: u32 = f.next()?.parse().ok()?;
            ((1..1234).contains(&wrong) && correct <= inertia).then_some(AccuracyRow {
                ch,
                wrong,
                correct,
            })
        })
        .collect()
}

/// Lee `proficiency.log_<teclado>`: carácter y tiempo promedio en segundos. Se detiene en la
/// primera línea mal formada o con tiempo no positivo, como el original.
pub fn parse_proficiency_log(text: &str) -> Vec<(char, f64)> {
    lines(text)
        .map_while(|line| {
            let mut f = line.split_whitespace();
            let ch = single_char(f.next()?)?;
            let dt = number(f.next()?).ok()?;
            f.next()?;
            (dt > 0.0).then_some((ch, dt))
        })
        .collect()
}

/// Una entrada del Top 10 en `local_<xx>.ksc`.
#[derive(Debug, Clone, PartialEq)]
pub struct KscEntry {
    /// Dos letras del idioma.
    pub lang: String,
    /// Instante (segundos desde 1970).
    pub when: i64,
    /// Caracteres tecleados.
    pub chars: u32,
    /// Precisión (%).
    pub accuracy: f64,
    /// Velocidad (PPM).
    pub wpm: f64,
    /// Fluidez.
    pub fluidity: f64,
    /// Puntaje.
    pub score: f64,
    /// Nombre («Nombre [teclado]»).
    pub name: String,
}

/// Lee un `.ksc` del original (`top10_read_stats_from_file`): hasta 10 registros con dos
/// letras de idioma, `x` o `w`, el instante y los caracteres (`i32`), precisión, velocidad,
/// fluidez y puntaje (`f32`), el largo del nombre (`i32`) y el nombre. Todo en el orden de
/// bytes de la máquina que lo escribió; se asume little-endian, como en x86 y ARM. Se detiene
/// en el primer registro dañado, como el original, y descarta los de relleno (`xxx`, puntaje 0).
pub fn parse_ksc(bytes: &[u8]) -> Vec<KscEntry> {
    let mut out = Vec::new();
    let mut r = Reader { bytes, pos: 0 };
    for _ in 0..10 {
        let Some(entry) = r.ksc_entry() else { break };
        if entry.when > 0 && entry.score > 0.0 {
            out.push(entry);
        }
    }
    out
}

struct Reader<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl Reader<'_> {
    fn take(&mut self, n: usize) -> Option<&[u8]> {
        let end = self.pos.checked_add(n)?;
        let slice = self.bytes.get(self.pos..end)?;
        self.pos = end;
        Some(slice)
    }

    fn i32(&mut self) -> Option<i32> {
        Some(i32::from_le_bytes(self.take(4)?.try_into().ok()?))
    }

    fn f32(&mut self) -> Option<f64> {
        let v = f32::from_le_bytes(self.take(4)?.try_into().ok()?);
        v.is_finite().then_some(f64::from(v))
    }

    fn ksc_entry(&mut self) -> Option<KscEntry> {
        let head = self.take(3)?;
        let (l0, l1, genv) = (head[0], head[1], head[2]);
        if !l0.is_ascii_alphabetic() || !matches!(genv, b'x' | b'w') {
            return None;
        }
        // Como el original: una segunda letra inválida es esperanto.
        let lang = if l1.is_ascii_alphabetic() {
            String::from_utf8_lossy(&[l0, l1]).into_owned()
        } else {
            "eo".to_owned()
        };
        let when = i64::from(self.i32()?);
        let chars = u32::try_from(self.i32()?).ok()?;
        let accuracy = self.f32().filter(|v| (0.0..=100.0).contains(v))?;
        let wpm = self.f32().filter(|v| (0.0..=300.0).contains(v))?;
        let fluidity = self.f32().filter(|v| (0.0..=100.0).contains(v))?;
        let score = self.f32().filter(|v| (0.0..=20.0).contains(v))?;
        let len = usize::try_from(self.i32()?).ok().filter(|&n| n <= 255)?;
        let name = String::from_utf8_lossy(self.take(len)?).into_owned();
        Some(KscEntry {
            lang,
            when,
            chars,
            accuracy,
            wpm,
            fluidity,
            score,
            name,
        })
    }
}

fn single_char(s: &str) -> Option<char> {
    let mut chars = s.chars();
    let c = chars.next()?;
    chars.next().is_none().then_some(c)
}

fn parse_rows<T>(
    text: &str,
    min_fields: usize,
    build: impl Fn(&[&str]) -> Result<T, ParseErrorKind>,
) -> Imported<T> {
    let mut out = Imported {
        records: Vec::new(),
        errors: Vec::new(),
    };
    for (i, line) in lines(text).enumerate().skip(1) {
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.is_empty() {
            continue;
        }
        if fields.len() < min_fields {
            out.errors.push(ParseError::at(
                i + 1,
                ParseErrorKind::InvalidField(line.to_owned()),
            ));
            continue;
        }
        match build(&fields) {
            Ok(r) => out.records.push(r),
            Err(kind) => out.errors.push(ParseError::at(i + 1, kind)),
        }
    }
    out
}

fn number(s: &str) -> Result<f64, ParseErrorKind> {
    s.parse::<f64>()
        .ok()
        .filter(|v| v.is_finite())
        .ok_or_else(|| ParseErrorKind::InvalidField(s.to_owned()))
}

/// Un archivo INI: sección → clave → valor.
pub type Ini = BTreeMap<String, BTreeMap<String, String>>;

/// Lee un INI de `GKeyFile` (`[sección]`, `clave=valor`, comentarios con `#`).
pub fn parse_ini(text: &str) -> Ini {
    let mut ini = Ini::new();
    let mut section = String::new();
    for line in lines(text) {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }
        if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            name.trim().clone_into(&mut section);
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            ini.entry(section.clone())
                .or_default()
                .insert(k.trim().to_owned(), v.trim().to_owned());
        }
    }
    ini
}

/// Preferencias del original que tienen equivalente en klavaro-rs.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct LegacyPreferences {
    /// Idioma de la interfaz (`interface/language`).
    pub language: Option<String>,
    /// Distribución de teclado (`tutor/keyboard`).
    pub keyboard: Option<String>,
    /// Lección actual del curso básico (`tutor/basic_lesson`).
    pub basic_lesson: Option<u16>,
    /// Párrafos de Fluidez (`tutor/fluid_paragraphs`).
    pub fluid_paragraphs: Option<u8>,
    /// Fuente del ejercicio (`tutor/lesson_font`).
    pub lesson_font: Option<String>,
    /// Voz activada (`interface/speech`).
    pub speech: Option<bool>,
    /// Colores (`colors/*`), en `#RRGGBB`.
    pub colors: BTreeMap<String, String>,
}

impl LegacyPreferences {
    /// Extrae las preferencias conocidas de un `preferences.ini`.
    pub fn from_ini(ini: &Ini) -> Self {
        let get = |s: &str, k: &str| ini.get(s).and_then(|m| m.get(k)).cloned();
        Self {
            language: get("interface", "language"),
            keyboard: get("tutor", "keyboard"),
            basic_lesson: get("tutor", "basic_lesson").and_then(|v| v.parse().ok()),
            fluid_paragraphs: get("tutor", "fluid_paragraphs").and_then(|v| v.parse().ok()),
            lesson_font: get("tutor", "lesson_font"),
            speech: get("interface", "speech").map(|v| v.eq_ignore_ascii_case("true")),
            colors: ini
                .get("colors")
                .map(|m| {
                    m.iter()
                        .filter(|(_, v)| is_hex_color(v))
                        .map(|(k, v)| (k.clone(), v.clone()))
                        .collect()
                })
                .unwrap_or_default(),
        }
    }
}

fn is_hex_color(v: &str) -> bool {
    v.len() == 7 && v.starts_with('#') && v[1..].chars().all(|c| c.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_stat_rows_and_reports_bad_ones() {
        let text = "Accuracy Velocity Fluidness Date Hour Lesson Keyboard\n\
                    97.50 23.40 61.20 2021-03-04 10:15 5 qwerty_us\n\
                    oops\n\
                    98.00 abc 60.00 2021-03-04 10:20 5 qwerty_us\n\
                    99.00 30.00 70.00 2021-03-05 08:00 Default es\n";
        let imported = parse_stats(text);
        assert_eq!(imported.records.len(), 2);
        assert_eq!(imported.errors.len(), 2);
        assert_eq!(imported.records[0].lesson, "5");
        assert_eq!(imported.records[0].tag, "qwerty_us");
        assert!((imported.records[1].wpm - 30.0).abs() < 1e-9);
        assert_eq!(imported.errors[0].line, Some(3));
    }

    #[test]
    fn parses_scores() {
        let text = "Score Date Time #chars Lang\n6.4321 2022-01-01 12:00 812 es\n";
        let imported = parse_scores(text);
        assert_eq!(
            imported.records,
            vec![ScoreRecord {
                score: 6.4321,
                date: "2022-01-01".to_owned(),
                time: "12:00".to_owned(),
                chars: 812,
                language: "es".to_owned(),
            }]
        );
    }

    #[test]
    fn parses_preferences() {
        let text = "[interface]\nlanguage=es\nspeech=false\n\n[tutor]\nkeyboard=qwerty_es\n\
                    basic_lesson=12\nfluid_paragraphs=3\nlesson_font=Monospace 14\n\
                    # comentario\n[colors]\nkey_1=#112233\nbad=rojo\n";
        let prefs = LegacyPreferences::from_ini(&parse_ini(text));
        assert_eq!(prefs.language.as_deref(), Some("es"));
        assert_eq!(prefs.keyboard.as_deref(), Some("qwerty_es"));
        assert_eq!(prefs.basic_lesson, Some(12));
        assert_eq!(prefs.fluid_paragraphs, Some(3));
        assert_eq!(prefs.speech, Some(false));
        assert_eq!(prefs.lesson_font.as_deref(), Some("Monospace 14"));
        assert_eq!(prefs.colors.len(), 1);
    }

    #[test]
    fn reads_accuracy_logs_until_the_first_bad_line() {
        let rows = parse_accuracy_log("é\t 3\t 2\nq\t12\t 1\nx\t 0\t 1\nz\t 5\t 1\n", 10);
        assert_eq!(
            rows,
            [
                AccuracyRow {
                    ch: 'é',
                    wrong: 3,
                    correct: 2
                },
                AccuracyRow {
                    ch: 'q',
                    wrong: 12,
                    correct: 1
                },
            ]
        );
        assert_eq!(parse_accuracy_log("", 10), [] as [AccuracyRow; 0]);
        assert_eq!(parse_accuracy_log("ab\t1\t1\n", 10), [] as [AccuracyRow; 0]);
    }

    #[test]
    fn reads_proficiency_logs() {
        let rows = parse_proficiency_log("k\t0.412000\t1.8000\nj\t0.229000\t1.0000\n");
        assert_eq!(rows, [('k', 0.412), ('j', 0.229)]);
        assert_eq!(
            parse_proficiency_log("k\t0.0\t1.0\n"),
            [] as [(char, f64); 0]
        );
    }

    fn ksc_record(lang: [u8; 2], when: i32, score: f32, name: &str) -> Vec<u8> {
        let mut b = lang.to_vec();
        b.push(b'x');
        b.extend(when.to_le_bytes());
        b.extend(600_i32.to_le_bytes());
        for v in [97.5_f32, 45.0, 70.0, score] {
            b.extend(v.to_le_bytes());
        }
        b.extend(i32::try_from(name.len()).unwrap_or(0).to_le_bytes());
        b.extend(name.as_bytes());
        b
    }

    #[test]
    fn reads_ksc_files_and_skips_padding() {
        let mut bytes = ksc_record(*b"es", 1_700_000_000, 6.5, "Ana [qwerty_es]");
        bytes.extend(ksc_record(*b"xx", 0, 0.0, "xxx"));
        bytes.extend(ksc_record(*b"es", 1_700_000_100, 4.0, "Luis"));
        let entries = parse_ksc(&bytes);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].lang, "es");
        assert_eq!(entries[0].name, "Ana [qwerty_es]");
        assert_eq!(entries[0].chars, 600);
        assert!((entries[0].score - 6.5).abs() < 1e-6);
        assert_eq!(entries[1].when, 1_700_000_100);
        // Un archivo cortado se lee hasta donde está sano.
        assert_eq!(parse_ksc(&bytes[..bytes.len() - 2]).len(), 1);
        assert_eq!(parse_ksc(b"").len(), 0);
    }
}
