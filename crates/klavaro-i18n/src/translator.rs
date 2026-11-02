// SPDX-License-Identifier: GPL-3.0-or-later
//! Textos traducidos con Fluent.
//!
//! Cada idioma se arma con dos fuentes: las traducciones del Klavaro original
//! (`locales/klavaro/`, generadas por `tools/po2ftl.py`) y los textos propios de klavaro-rs
//! (`locales/app/`), que tienen prioridad. Lo que falte se toma del inglés (`C`).

use fluent_bundle::concurrent::FluentBundle;
use fluent_bundle::{FluentArgs, FluentResource};
use unic_langid::LanguageIdentifier;

use crate::catalog::ORIGINAL;
use crate::{FALLBACK, LANGUAGES};

/// Textos propios de klavaro-rs por idioma.
const APP: &[(&str, &str)] = &[
    ("C", include_str!("../locales/app/C.ftl")),
    ("es", include_str!("../locales/app/es.ftl")),
];

type Bundle = FluentBundle<FluentResource>;

/// Traductor de un idioma, con el inglés de respaldo.
pub struct Translator {
    code: &'static str,
    bundle: Bundle,
    fallback: Option<Bundle>,
}

impl std::fmt::Debug for Translator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Translator")
            .field("code", &self.code)
            .finish_non_exhaustive()
    }
}

impl Translator {
    /// Traductor para un código del catálogo ([`LANGUAGES`]). Un código desconocido usa el
    /// inglés.
    pub fn new(code: &str) -> Self {
        let code = LANGUAGES
            .iter()
            .map(|(c, _)| *c)
            .find(|c| *c == code)
            .unwrap_or(FALLBACK);
        Self {
            code,
            bundle: bundle(code),
            fallback: (code != FALLBACK).then(|| bundle(FALLBACK)),
        }
    }

    /// Código del idioma.
    pub fn code(&self) -> &'static str {
        self.code
    }

    /// Texto de un mensaje sin argumentos.
    pub fn tr(&self, id: &str) -> String {
        self.tr_args(id, &[])
    }

    /// Texto de un mensaje con argumentos (`{ $nombre }`). Si el mensaje no existe en ningún
    /// idioma, devuelve el identificador, para que el hueco se note sin romper la interfaz.
    pub fn tr_args(&self, id: &str, args: &[(&str, &str)]) -> String {
        let mut fluent_args = FluentArgs::new();
        for (k, v) in args {
            fluent_args.set(*k, *v);
        }
        std::iter::once(&self.bundle)
            .chain(self.fallback.as_ref())
            .find_map(|b| format(b, id, &fluent_args))
            .unwrap_or_else(|| id.to_owned())
    }

    /// Si el mensaje está traducido a este idioma (sin contar el respaldo).
    pub fn has(&self, id: &str) -> bool {
        self.bundle.has_message(id)
    }
}

fn format(bundle: &Bundle, id: &str, args: &FluentArgs<'_>) -> Option<String> {
    let pattern = bundle.get_message(id)?.value()?;
    let mut errors = Vec::new();
    let text = bundle.format_pattern(pattern, Some(args), &mut errors);
    // Un argumento faltante deja `{$x}` en el texto; se muestra igual.
    Some(text.into_owned())
}

/// Identificador de idioma para Fluent (`pt_BR` → `pt-BR`, `C` → `en`).
fn langid(code: &str) -> LanguageIdentifier {
    let tag = if code == FALLBACK {
        "en".to_owned()
    } else {
        code.replace('_', "-")
    };
    tag.parse().unwrap_or_default()
}

fn bundle(code: &str) -> Bundle {
    let mut bundle = FluentBundle::new_concurrent(vec![langid(code)]);
    // Sin marcas de aislamiento bidireccional: la interfaz las dibujaría como cajas.
    bundle.set_use_isolating(false);
    for source in [ORIGINAL, APP] {
        if let Some((_, text)) = source.iter().find(|(c, _)| *c == code) {
            bundle.add_resource_overriding(resource(text));
        }
    }
    bundle
}

/// Interpreta un `.ftl`; las entradas con errores de sintaxis se descartan (las pruebas
/// verifican que no haya ninguna).
fn resource(text: &str) -> FluentResource {
    FluentResource::try_new(text.to_owned()).unwrap_or_else(|(partial, _)| partial)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use fluent_bundle::FluentResource;
    use fluent_syntax::ast;

    use super::*;

    fn all_sources() -> impl Iterator<Item = (&'static str, &'static str)> {
        ORIGINAL.iter().chain(APP).copied()
    }

    /// Claves y variables de cada mensaje de un `.ftl`.
    fn messages(text: &str) -> Vec<(String, BTreeSet<String>)> {
        let res = FluentResource::try_new(text.to_owned()).expect("ftl válido");
        res.entries()
            .filter_map(|e| match e {
                ast::Entry::Message(m) => Some(m),
                _ => None,
            })
            .map(|m| {
                let vars = m
                    .value
                    .iter()
                    .flat_map(|p| &p.elements)
                    .filter_map(|el| match el {
                        ast::PatternElement::Placeable {
                            expression:
                                ast::Expression::Inline(ast::InlineExpression::VariableReference {
                                    id,
                                }),
                        } => Some(id.name.to_owned()),
                        _ => None,
                    })
                    .collect();
                (m.id.name.to_owned(), vars)
            })
            .collect()
    }

    fn english() -> Vec<(String, BTreeSet<String>)> {
        all_sources()
            .filter(|(c, _)| *c == FALLBACK)
            .flat_map(|(_, t)| messages(t))
            .collect()
    }

    #[test]
    fn every_file_parses() {
        for (code, text) in all_sources() {
            assert!(
                FluentResource::try_new(text.to_owned()).is_ok(),
                "{code}: error de sintaxis"
            );
        }
    }

    #[test]
    fn every_language_has_original_translations() {
        for (code, _) in LANGUAGES {
            assert!(ORIGINAL.iter().any(|(c, _)| c == code), "{code}");
        }
    }

    #[test]
    fn translations_use_the_english_keys_and_variables() {
        let en = english();
        for (code, text) in all_sources() {
            for (id, vars) in messages(text) {
                let base = en.iter().find(|(i, _)| *i == id);
                let base = base.unwrap_or_else(|| panic!("{code}: clave desconocida {id}"));
                assert_eq!(vars, base.1, "{code}: variables de {id}");
            }
        }
    }

    #[test]
    fn app_translations_are_complete() {
        let keys = |code| -> BTreeSet<String> {
            APP.iter()
                .filter(|(c, _)| *c == code)
                .flat_map(|(_, t)| messages(t))
                .map(|(id, _)| id)
                .collect()
        };
        let en = keys(FALLBACK);
        for (code, _) in APP {
            let have = keys(code);
            let missing: Vec<_> = en.difference(&have).collect();
            assert!(missing.is_empty(), "{code}: faltan {missing:?}");
        }
    }

    #[test]
    fn translates_with_fallback() {
        let es = Translator::new("es");
        assert_eq!(es.code(), "es");
        assert_eq!(es.tr("module-basic"), "Curso básico");
        assert_eq!(es.tr("menu-keyboard"), "Teclado:");
        assert_eq!(
            es.tr_args("tutor-lesson-n", &[("lesson", "3")]),
            "Lección 3"
        );
        assert!(
            es.tr_args("comment-velo-running", &[("target", "40")])
                .contains("40 PPM")
        );
        assert_eq!(es.tr("no-existe"), "no-existe");

        // Telugu tiene muy pocas traducciones del original: el resto sale en inglés.
        let te = Translator::new("te");
        assert!(!te.has("module-basic"));
        assert_eq!(te.tr("module-basic"), "Basic course");

        let unknown = Translator::new("xx");
        assert_eq!(unknown.code(), FALLBACK);
        assert_eq!(unknown.tr("stats-accuracy"), "Accuracy:");
    }

    #[test]
    fn multiline_comments_keep_line_breaks() {
        let c = Translator::new("C").tr("comment-basic-next");
        assert_eq!(c, "All right, now you got it!\nGo to the next lesson.");
    }
}
