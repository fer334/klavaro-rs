// SPDX-License-Identifier: GPL-3.0-or-later
//! Aviso de versión nueva, independiente de iced.
//!
//! Al abrir, la interfaz consulta el último release publicado en GitHub
//! (`GET /repos/<repo>/releases/latest`, que no incluye borradores ni versiones previas) y, si
//! su versión es mayor que la de este binario, muestra un aviso con el enlace a la página de
//! descarga. No descarga ni instala nada.
//!
//! El repositorio se fija al compilar con la variable `KLAVARO_RELEASES` (`dueño/repo`); el
//! workflow de releases la toma de `github.repository`. Sin ella, o sin la función `update`,
//! no se consulta nada. Cualquier error de red se ignora: el aviso es sólo una ayuda.

use std::process::{Command, Stdio};

/// Repositorio de GitHub con los releases (`dueño/repo`), si se fijó al compilar.
pub(crate) const REPO: Option<&str> = option_env!("KLAVARO_RELEASES");

/// Versión de este binario.
pub(crate) const CURRENT: &str = env!("CARGO_PKG_VERSION");

/// Release más nuevo que el actual.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Release {
    /// Versión sin la `v` inicial («0.2.0»).
    pub(crate) version: String,
    /// Página del release.
    pub(crate) url: String,
}

/// Si la consulta puede hacerse (función `update` y repositorio fijado al compilar).
pub(crate) fn available() -> bool {
    cfg!(feature = "update") && REPO.is_some_and(|r| !r.trim().is_empty())
}

// Sin la función `update` sólo lo usan las pruebas.
#[cfg_attr(not(feature = "update"), allow(dead_code))]
/// Versión como números (`v1.2.3-beta` → `[1, 2, 3]`), o `None` si no es una versión.
fn parse_version(s: &str) -> Option<[u64; 3]> {
    let s = s.trim();
    let s = s.strip_prefix(['v', 'V']).unwrap_or(s);
    let core = s.split(['-', '+']).next()?;
    let mut out = [0; 3];
    let mut parts = core.split('.');
    for slot in &mut out {
        match parts.next() {
            Some(p) => *slot = p.parse().ok()?,
            None => break,
        }
    }
    parts.next().is_none().then_some(out)
}

#[cfg_attr(not(feature = "update"), allow(dead_code))]
/// El release de la etiqueta `tag` si es más nuevo que `current` y su página está en GitHub.
pub(crate) fn newer(current: &str, tag: &str, url: &str) -> Option<Release> {
    let theirs = parse_version(tag)?;
    let ours = parse_version(current)?;
    // El enlace se abre en el navegador: sólo se aceptan páginas de GitHub.
    let url_ok = url.starts_with("https://github.com/") && !url.contains(char::is_whitespace);
    (theirs > ours && url_ok).then(|| Release {
        version: tag.trim().trim_start_matches(['v', 'V']).to_owned(),
        url: url.to_owned(),
    })
}

/// Consulta el último release (bloquea; llamar fuera del hilo de la interfaz).
#[cfg(feature = "update")]
pub(crate) fn check() -> Option<Release> {
    use std::time::Duration;

    #[derive(serde::Deserialize)]
    struct Latest {
        tag_name: String,
        html_url: String,
    }

    let repo = REPO?.trim();
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(10)))
        .build()
        .into();
    let latest: Latest = agent
        .get(format!(
            "https://api.github.com/repos/{repo}/releases/latest"
        ))
        .header("Accept", "application/vnd.github+json")
        .header(
            "User-Agent",
            concat!("klavaro-rs/", env!("CARGO_PKG_VERSION")),
        )
        .call()
        .ok()?
        .body_mut()
        .read_json()
        .ok()?;
    newer(CURRENT, &latest.tag_name, &latest.html_url)
}

/// Sin la función `update` no se consulta nada.
#[cfg(not(feature = "update"))]
pub(crate) fn check() -> Option<Release> {
    None
}

/// Abre `url` en el navegador del sistema, sin shell.
pub(crate) fn open_in_browser(url: &str) {
    let mut command = if cfg!(target_os = "macos") {
        Command::new("open")
    } else if cfg!(windows) {
        let mut c = Command::new("rundll32");
        c.arg("url.dll,FileProtocolHandler");
        c
    } else {
        Command::new("xdg-open")
    };
    // Si no hay con qué abrirlo no hay mucho más que hacer: el aviso sigue mostrando la versión.
    let _ = command
        .arg(url)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
}

#[cfg(test)]
mod tests {
    use super::*;

    const PAGE: &str = "https://github.com/dueño/klavaro-rs/releases/tag/v0.2.0";

    #[test]
    fn parses_versions() {
        assert_eq!(parse_version("v0.2.0"), Some([0, 2, 0]));
        assert_eq!(parse_version("1.10"), Some([1, 10, 0]));
        assert_eq!(parse_version("V2.0.1-beta.1"), Some([2, 0, 1]));
        assert_eq!(parse_version("1.2.3.4"), None);
        assert_eq!(parse_version("último"), None);
    }

    #[test]
    fn only_newer_releases_on_github() {
        let r = newer("0.1.0", "v0.2.0", PAGE).expect("más nueva");
        assert_eq!(r.version, "0.2.0");
        assert_eq!(r.url, PAGE);
        // 0.10 es mayor que 0.9 (no se compara como texto).
        assert!(newer("0.9.0", "v0.10.0", PAGE).is_some());
        assert_eq!(newer("0.2.0", "v0.2.0", PAGE), None);
        assert_eq!(newer("0.3.0", "v0.2.0", PAGE), None);
        assert_eq!(newer("0.1.0", "nightly", PAGE), None);
        assert_eq!(newer("0.1.0", "v0.2.0", "https://ejemplo.com/x"), None);
        assert_eq!(newer("0.1.0", "v0.2.0", "https://github.com/a b"), None);
    }

    #[test]
    fn current_version_is_valid() {
        assert!(parse_version(CURRENT).is_some());
    }
}
