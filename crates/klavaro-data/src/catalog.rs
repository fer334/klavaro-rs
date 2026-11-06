// SPDX-License-Identifier: GPL-3.0-or-later
//! Catálogo de textos de la Progresión (`<idioma>.progression`).
//!
//! Lo genera `tools/progression/selection.py`. Tiene una lección por línea, con los campos
//! separados por tabuladores: sección, lección, identificador, tema, dificultad, fuente, título,
//! revisión y texto. Las líneas que empiezan con `#` son comentarios.

use klavaro_core::progression::Slot;

use crate::{ParseError, ParseErrorKind, lines};

const FIELDS: usize = 9;

/// Un texto del catálogo, con lo necesario para atribuirlo.
#[derive(Debug, Clone, PartialEq)]
pub struct CatalogEntry {
    /// La lección.
    pub slot: Slot,
    /// Identificador del texto, que se guarda en cada intento.
    pub text_id: String,
    /// Tema.
    pub topic: String,
    /// Dificultad, de 0 a 100.
    pub difficulty: f64,
    /// Wiki de origen (`en`, `simple`).
    pub source: String,
    /// Título del artículo.
    pub title: String,
    /// Revisión del artículo.
    pub revision: u64,
    /// Texto.
    pub text: String,
}

impl CatalogEntry {
    /// Enlace a la revisión exacta del artículo.
    pub fn url(&self) -> String {
        format!(
            "https://{}.wikipedia.org/w/index.php?oldid={}",
            self.source, self.revision
        )
    }
}

/// Lee un catálogo.
///
/// # Errors
///
/// Si a una línea le faltan campos o alguno no es válido.
pub fn parse(text: &str) -> Result<Vec<CatalogEntry>, ParseError> {
    let mut entries = Vec::new();
    for (index, line) in lines(text).enumerate() {
        if !line.trim().is_empty() && !line.starts_with('#') {
            entries.push(parse_line(line, index + 1)?);
        }
    }
    Ok(entries)
}

fn parse_line(line: &str, number: usize) -> Result<CatalogEntry, ParseError> {
    let invalid = |field: &str| ParseError::at(number, ParseErrorKind::InvalidField(field.into()));
    let fields: Vec<&str> = line.split('\t').collect();
    let [
        section,
        lesson,
        text_id,
        topic,
        difficulty,
        source,
        title,
        revision,
        text,
    ] = fields[..]
    else {
        return Err(ParseError::at(
            number,
            ParseErrorKind::TooFewLines {
                expected: FIELDS,
                found: fields.len(),
            },
        ));
    };
    let section: u16 = section.parse().map_err(|_| invalid(section))?;
    let lesson: u16 = lesson.parse().map_err(|_| invalid(lesson))?;
    let slot = Slot::new(section, lesson).ok_or_else(|| invalid("sección o lección"))?;
    // La fuente forma parte de un enlace que se abre en el navegador.
    if source.is_empty() || !source.chars().all(|c| c.is_ascii_lowercase()) {
        return Err(invalid(source));
    }
    if text.trim().is_empty() {
        return Err(invalid("texto vacío"));
    }
    Ok(CatalogEntry {
        slot,
        text_id: text_id.to_owned(),
        topic: topic.to_owned(),
        difficulty: difficulty.parse().map_err(|_| invalid(difficulty))?,
        source: source.to_owned(),
        title: title.to_owned(),
        revision: revision.parse().map_err(|_| invalid(revision))?,
        text: text.to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const LINE: &str = "75\t3\tsimple-9\tArts\t72.5\tsimple\tThe Title\t123\tSome text.";

    #[test]
    fn reads_a_line_with_its_attribution() {
        let entries = parse(&format!("# comentario\n\n{LINE}\n")).expect("catálogo");
        assert_eq!(entries.len(), 1);
        let entry = &entries[0];
        assert_eq!(entry.slot, Slot::new(75, 3).expect("lección"));
        assert_eq!(entry.text_id, "simple-9");
        assert_eq!(entry.topic, "Arts");
        assert!((entry.difficulty - 72.5).abs() < 1e-9);
        assert_eq!(entry.title, "The Title");
        assert_eq!(entry.text, "Some text.");
        assert_eq!(
            entry.url(),
            "https://simple.wikipedia.org/w/index.php?oldid=123"
        );
    }

    #[test]
    fn reports_the_line_of_a_bad_entry() {
        let error = parse(&format!("# c\n{}", "1\t2")).expect_err("faltan campos");
        assert_eq!(error.line, Some(2));
        for bad in [
            LINE.replace("75\t", "76\t"),
            LINE.replace("\t3\t", "\t31\t"),
            LINE.replace("72.5", "x"),
            LINE.replace("\t123\t", "\tabc\t"),
            LINE.replace("\tsimple\t", "\tevil.example/\t"),
            LINE.replace("Some text.", " "),
        ] {
            assert!(parse(&bad).is_err(), "{bad}");
        }
    }
}
