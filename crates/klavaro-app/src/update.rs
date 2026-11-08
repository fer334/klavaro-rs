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
//!
//! # Actualización automática
//!
//! Donde la instalación lo permite ([`can_install`]), el aviso ofrece instalar la versión nueva
//! sin pasar por el navegador, con `cargo-packager-updater`: baja el `latest.json` del último
//! release, descarga el paquete de esta plataforma, verifica su firma minisign con la clave
//! pública fijada al compilar (`KLAVARO_UPDATE_PUBKEY`, el `.pub` de minisign en base64) y lo
//! instala. Se puede en:
//!
//! - macOS: la app dentro de un `klavaro-rs.app` (se reemplaza el bundle entero);
//! - Windows: la instalada con el `.msi` (se corre el `.msi` nuevo, que reabre la app);
//! - Linux: la `AppImage` (se reemplaza el archivo).
//!
//! El `.exe` portable, el `.deb` y el `.tar.gz` siguen con el aviso y el enlace de descarga.

use std::path::Path;
use std::process::{Command, Stdio};

/// Repositorio de GitHub con los releases (`dueño/repo`), si se fijó al compilar.
pub(crate) const REPO: Option<&str> = option_env!("KLAVARO_RELEASES");

/// Clave pública de las firmas de los paquetes (el `.pub` de minisign en base64), si se fijó
/// al compilar.
const PUBKEY: Option<&str> = option_env!("KLAVARO_UPDATE_PUBKEY");

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

/// Cómo se instaló esta copia, según dónde está el ejecutable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Install {
    /// Dentro de un bundle de macOS: se reemplaza el `.app`.
    MacApp,
    /// Instalada con el `.msi` (en `Program Files\klavaro-rs`).
    WindowsMsi,
    /// Una `AppImage` (el runtime deja su ruta en `APPIMAGE`).
    AppImage,
    /// Portable, `.deb` o `.tar.gz`: sólo el aviso con el enlace.
    Other,
}

// Sin la función `update` sólo lo usan las pruebas.
#[cfg_attr(not(feature = "update"), allow(dead_code))]
/// Tipo de instalación de un ejecutable en `exe`, para el sistema `os` (`std::env::consts::OS`).
/// `appimage` es la variable `APPIMAGE` y `program_files`, la carpeta `ProgramFiles`.
fn install_kind(
    os: &str,
    exe: &Path,
    appimage: Option<&str>,
    program_files: Option<&Path>,
) -> Install {
    match os {
        "macos" => {
            let in_bundle = exe
                .parent()
                .filter(|macos| macos.ends_with("Contents/MacOS"))
                .and_then(|macos| macos.parent()?.parent())
                .is_some_and(|app| app.extension().is_some_and(|e| e == "app"));
            if in_bundle {
                Install::MacApp
            } else {
                Install::Other
            }
        }
        "windows" => {
            let installed = program_files.is_some_and(|pf| {
                exe.parent() == Some(pf.join("klavaro-rs").as_path())
                    && exe
                        .file_name()
                        .is_some_and(|n| n.eq_ignore_ascii_case("klavaro-rs.exe"))
            });
            if installed {
                Install::WindowsMsi
            } else {
                Install::Other
            }
        }
        "linux" if appimage.is_some_and(|a| !a.is_empty()) => Install::AppImage,
        _ => Install::Other,
    }
}

/// Tipo de instalación de esta copia.
#[cfg(feature = "update")]
fn this_install() -> Install {
    let Ok(exe) = std::env::current_exe() else {
        return Install::Other;
    };
    let appimage = std::env::var("APPIMAGE").ok();
    let program_files = std::env::var_os("ProgramFiles").map(std::path::PathBuf::from);
    install_kind(
        std::env::consts::OS,
        &exe,
        appimage.as_deref(),
        program_files.as_deref(),
    )
}

/// Si esta copia puede instalar sola la versión nueva (ver la documentación del módulo).
pub(crate) fn can_install() -> bool {
    #[cfg(feature = "update")]
    {
        available()
            && PUBKEY.is_some_and(|k| !k.trim().is_empty())
            && this_install() != Install::Other
    }
    #[cfg(not(feature = "update"))]
    false
}

/// Descarga, verifica e instala la última versión (bloquea; llamar fuera del hilo de la
/// interfaz). En Windows el instalador cierra la app y la vuelve a abrir, así que no vuelve.
/// Después hay que llamar a [`relaunch`].
///
/// # Errors
///
/// Si no hay versión nueva para esta plataforma, falla la descarga o la firma no coincide.
#[cfg(feature = "update")]
pub(crate) fn install() -> Result<(), String> {
    use std::time::Duration;

    use cargo_packager_updater::{
        Config, UpdaterBuilder, WindowsConfig, WindowsUpdateInstallMode, semver::Version,
    };

    let (Some(repo), Some(pubkey)) = (REPO, PUBKEY) else {
        return Err("esta copia no se compiló con actualizaciones".to_owned());
    };
    let endpoint = format!(
        "https://github.com/{}/releases/latest/download/latest.json",
        repo.trim()
    );
    let config = Config {
        endpoints: vec![endpoint.parse().map_err(|e| format!("{e}"))?],
        pubkey: pubkey.trim().to_owned(),
        windows: Some(WindowsConfig {
            installer_args: None,
            install_mode: Some(WindowsUpdateInstallMode::Passive),
        }),
    };
    let current = Version::parse(CURRENT).map_err(|e| e.to_string())?;
    let update = UpdaterBuilder::new(current, config)
        .timeout(Duration::from_secs(120))
        .build()
        .and_then(|u| u.check())
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "no hay una versión nueva para este sistema".to_owned())?;
    update.download_and_install().map_err(|e| e.to_string())
}

/// Sin la función `update` no se instala nada.
#[cfg(not(feature = "update"))]
pub(crate) fn install() -> Result<(), String> {
    Err("esta copia no se compiló con actualizaciones".to_owned())
}

/// Abre la versión recién instalada (la app actual tiene que cerrarse después).
pub(crate) fn relaunch() {
    #[cfg(feature = "update")]
    {
        let Ok(exe) = std::env::current_exe() else {
            return;
        };
        let mut command = match this_install() {
            // `open -n` abre el bundle nuevo aunque la app vieja siga abierta.
            Install::MacApp => {
                let Some(app) = exe.ancestors().nth(3) else {
                    return;
                };
                let mut c = Command::new("open");
                c.arg("-n").arg(app);
                c
            }
            Install::AppImage => match std::env::var_os("APPIMAGE") {
                Some(path) => Command::new(path),
                None => return,
            },
            // En Windows lo reabre el instalador.
            Install::WindowsMsi | Install::Other => return,
        };
        let _ = command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
    }
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
    fn detects_how_it_was_installed() {
        let mac = |p: &str| install_kind("macos", Path::new(p), None, None);
        assert_eq!(
            mac("/Applications/klavaro-rs.app/Contents/MacOS/klavaro-rs"),
            Install::MacApp
        );
        assert_eq!(
            mac("/Users/x/klavaro-rust/target/debug/klavaro-rs"),
            Install::Other
        );
        assert_eq!(mac("/opt/Contents/MacOS/klavaro-rs"), Install::Other);

        let pf = Path::new(r"C:\Program Files");
        let win = |p: &str| install_kind("windows", Path::new(p), None, Some(pf));
        // Las pruebas corren en cualquier sistema: se arma la ruta con `join`.
        let installed = pf.join("klavaro-rs").join("klavaro-rs.exe");
        assert_eq!(win(installed.to_str().expect("utf-8")), Install::WindowsMsi);
        let portable =
            Path::new(r"C:\Users\x\Downloads").join("klavaro-rs-0.2.0-windows-x86_64.exe");
        assert_eq!(win(portable.to_str().expect("utf-8")), Install::Other);
        assert_eq!(
            install_kind("windows", &installed, None, None),
            Install::Other
        );

        let bin = Path::new("/tmp/.mount_x/usr/bin/klavaro-rs");
        assert_eq!(
            install_kind("linux", bin, Some("/home/x/klavaro-rs.AppImage"), None),
            Install::AppImage
        );
        assert_eq!(install_kind("linux", bin, Some(""), None), Install::Other);
        assert_eq!(
            install_kind("linux", Path::new("/usr/bin/klavaro-rs"), None, None),
            Install::Other
        );
    }

    #[test]
    fn current_version_is_valid() {
        assert!(parse_version(CURRENT).is_some());
    }
}
