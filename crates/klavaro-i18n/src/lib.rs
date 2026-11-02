// SPDX-License-Identifier: GPL-3.0-or-later
//! Idiomas de la interfaz.
//!
//! Por ahora contiene el catálogo de idiomas del Klavaro original y la regla para elegir el
//! idioma a partir de las preferencias del sistema. Los textos traducidos (Fluent, `.ftl`)
//! se agregan en la fase 2, cuando haya acceso a crates.io para `i18n-embed`.

/// Código del idioma por defecto (inglés). El original lo llama `C`.
pub const FALLBACK: &str = "C";

/// Idiomas de interfaz de Klavaro 3.14, con su nombre nativo.
pub const LANGUAGES: &[(&str, &str)] = &[
    ("ar", "العربية"),
    ("bg", "Български"),
    ("bn", "বাংলা"),
    ("ca", "Català"),
    ("cs", "Čeština"),
    ("da", "Dansk"),
    ("de", "Deutsch"),
    ("el", "Ελληνικά"),
    ("C", "English"),
    ("en_GB", "English (UK)"),
    ("eo", "Esperanto"),
    ("es", "Español"),
    ("eu", "Euskara"),
    ("fi", "Suomi"),
    ("fr", "Français"),
    ("gl", "Galego"),
    ("hr", "Hrvatski"),
    ("hu", "Magyar"),
    ("id", "Bahasa Indonesia"),
    ("it", "Italiano"),
    ("kk", "Қазақ"),
    ("ky", "Кыргыз"),
    ("nb", "Norsk bokmål"),
    ("nl", "Nederlands"),
    ("pa", "ਪੰਜਾਬੀ"),
    ("pl", "Polski"),
    ("pt_BR", "Português (Brasil)"),
    ("pt_PT", "Português (Portugal)"),
    ("ro", "Română"),
    ("ru", "Русский"),
    ("sl", "Slovenščina"),
    ("sr", "Српски"),
    ("sv", "Svenska"),
    ("te", "తెలుగు"),
    ("tr", "Türkçe"),
    ("uk", "Українська"),
    ("ur", "اردو"),
    ("vi", "Tiếng Việt"),
    ("wo", "Wolof"),
    ("zh_CN", "简体中文"),
];

/// Si el código está en el catálogo.
pub fn is_supported(code: &str) -> bool {
    LANGUAGES.iter().any(|(c, _)| *c == code)
}

/// Nombre nativo de un idioma.
pub fn native_name(code: &str) -> Option<&'static str> {
    LANGUAGES.iter().find(|(c, _)| *c == code).map(|(_, n)| *n)
}

/// Elige el idioma de interfaz a partir de una lista de preferencias en orden (la guardada
/// por el usuario primero, luego las del sistema como `es_PY.UTF-8`).
///
/// Para cada preferencia: coincidencia exacta; si no, inglés para cualquier `en*`; si no,
/// el primer idioma con el mismo prefijo de dos letras. Si nada coincide, [`FALLBACK`].
pub fn resolve<'a>(preferences: impl IntoIterator<Item = &'a str>) -> &'static str {
    for pref in preferences {
        let code = normalize(pref);
        if code.is_empty() {
            continue;
        }
        if let Some((c, _)) = LANGUAGES.iter().find(|(c, _)| *c == code) {
            return c;
        }
        if code.starts_with("en") {
            return FALLBACK;
        }
        let prefix = code.get(..2).unwrap_or(&code);
        if let Some((c, _)) = LANGUAGES.iter().find(|(c, _)| c.starts_with(prefix)) {
            return c;
        }
    }
    FALLBACK
}

/// Idiomas preferidos del sistema según las variables POSIX (`LC_ALL`, `LC_MESSAGES`,
/// `LANGUAGE`, `LANG`), en orden. `LANGUAGE` puede traer varios separados por `:`.
pub fn system_preferences() -> Vec<String> {
    let mut out = Vec::new();
    for var in ["LC_ALL", "LC_MESSAGES", "LANGUAGE", "LANG"] {
        if let Ok(v) = std::env::var(var) {
            out.extend(
                v.split(':')
                    .map(str::trim)
                    .filter(|s| !s.is_empty() && *s != "C" && *s != "POSIX")
                    .map(str::to_owned),
            );
        }
    }
    out
}

/// Quita la codificación y el modificador (`es_PY.UTF-8@euro` → `es_PY`) y usa `_` como
/// separador.
fn normalize(locale: &str) -> String {
    let base = locale.split(['.', '@']).next().unwrap_or_default();
    base.trim().replace('-', "_")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_matches_original_size() {
        assert_eq!(LANGUAGES.len(), 40);
        assert!(is_supported("pt_BR"));
        assert_eq!(native_name("es"), Some("Español"));
    }

    #[test]
    fn resolves_locales() {
        assert_eq!(resolve(["es_PY.UTF-8"]), "es");
        assert_eq!(resolve(["pt-BR"]), "pt_BR");
        assert_eq!(resolve(["pt_AO"]), "pt_BR");
        assert_eq!(resolve(["en_GB"]), "en_GB");
        assert_eq!(resolve(["en_US.UTF-8"]), "C");
        assert_eq!(resolve(["xx", "fr_CA"]), "fr");
        assert_eq!(resolve(["", "zz"]), FALLBACK);
        assert_eq!(resolve([]), FALLBACK);
    }
}
