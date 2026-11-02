// SPDX-License-Identifier: GPL-3.0-or-later
//! klavaro-rs: tutor de mecanografía.
//!
//! Sin opciones abre la interfaz gráfica (iced). `--practicar` ofrece además una práctica
//! en la terminal con el mismo núcleo y los mismos datos.

// Parte de estos módulos sólo la usa la interfaz gráfica.
#[cfg_attr(not(feature = "gui"), allow(dead_code))]
mod assets;
#[cfg_attr(not(feature = "gui"), allow(dead_code))]
mod audio;
mod exercise;
#[cfg(feature = "gui")]
mod gui;
#[cfg(feature = "gui")]
mod plot;
mod practice;
#[cfg_attr(not(feature = "gui"), allow(dead_code))]
mod profile;
#[cfg_attr(not(feature = "gui"), allow(dead_code))]
mod progress;
#[cfg_attr(not(feature = "gui"), allow(dead_code))]
mod settings;
#[cfg_attr(not(feature = "gui"), allow(dead_code))]
mod speech;
#[cfg(feature = "gui")]
mod style;
#[cfg_attr(not(feature = "gui"), allow(dead_code))]
mod tutor;

use std::io::{self, Write};
use std::process::ExitCode;

use klavaro_core::Module;
use practice::PracticeOptions;

const HELP: &str = "\
klavaro-rs — tutor de mecanografía

Uso: klavaro-rs [OPCIONES]

Sin opciones abre la ventana del tutor.

Opciones:
  -h, --help              Muestra esta ayuda
  -v, --version           Muestra la versión
  -x, --velotxt           El módulo Velocidad usa textos completos, como Fluidez
  -p, --practicar [MÓD]   Práctica en la terminal. MÓD: basico, adapt, velocidad (por
                          defecto) o fluidez
      --leccion N         Lección del curso básico (1-43)
      --idioma CÓDIGO     Idioma de los textos (por defecto: es)
      --teclado NOMBRE    Distribución de teclado (por defecto: qwerty_es)

Ejemplos:
  klavaro-rs --practicar
  klavaro-rs --practicar basico --leccion 3
  klavaro-rs --practicar fluidez --idioma C --teclado qwerty_us
";

/// Opciones de línea de comandos.
#[derive(Debug, Default, PartialEq, Eq)]
struct Options {
    help: bool,
    version: bool,
    velotxt: bool,
    practice: Option<PracticeOptions>,
}

fn parse_module(s: &str) -> Option<Module> {
    Some(match s {
        "basico" | "básico" | "basic" | "1" => Module::Basic,
        "adapt" | "adaptabilidad" | "2" => Module::Adaptability,
        "velocidad" | "velo" | "3" => Module::Velocity,
        "fluidez" | "fluid" | "4" => Module::Fluidity,
        _ => return None,
    })
}

fn parse_args(args: impl IntoIterator<Item = String>) -> Result<Options, String> {
    let mut opts = Options::default();
    let mut practice = PracticeOptions::default();
    let mut wants_practice = false;
    let mut args = args.into_iter().peekable();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => opts.help = true,
            "-v" | "--version" => opts.version = true,
            "-x" | "--velotxt" => opts.velotxt = true,
            "-p" | "--practicar" => {
                wants_practice = true;
                if let Some(m) = args.peek().and_then(|a| parse_module(a)) {
                    practice.module = m;
                    args.next();
                }
            }
            "--leccion" | "--lección" => {
                let v = args.next().ok_or("falta el número de lección")?;
                practice.lesson = v.parse().map_err(|_| format!("lección no válida: {v}"))?;
            }
            "--idioma" => practice.language = args.next().ok_or("falta el idioma")?,
            "--teclado" => practice.keyboard = args.next().ok_or("falta el teclado")?,
            other => return Err(format!("opción desconocida: {other}")),
        }
    }
    if wants_practice {
        opts.practice = Some(practice);
    }
    Ok(opts)
}

fn main() -> ExitCode {
    let opts = match parse_args(std::env::args().skip(1)) {
        Ok(o) => o,
        Err(msg) => {
            let _ = writeln!(io::stderr(), "{msg}\nProbá `klavaro-rs --help`.");
            return ExitCode::FAILURE;
        }
    };
    let mut out = io::stdout().lock();
    let result = if opts.version {
        writeln!(out, "klavaro-rs {}", env!("CARGO_PKG_VERSION"))
    } else if opts.help {
        write!(out, "{HELP}")
    } else if let Some(p) = &opts.practice {
        drop(out);
        practice::run(p)
    } else {
        drop(out);
        open_window()
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            let _ = writeln!(io::stderr(), "error: {e}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(feature = "gui")]
fn open_window() -> io::Result<()> {
    gui::run().map_err(|e| io::Error::other(e.to_string()))
}

#[cfg(not(feature = "gui"))]
fn open_window() -> io::Result<()> {
    writeln!(
        io::stdout(),
        "Compilado sin interfaz gráfica. Probá `klavaro-rs --practicar` (ver --help)."
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn parses_known_options() {
        let o = parse_args(args(&["-x", "--version"])).expect("válido");
        assert!(o.version && o.velotxt && !o.help && o.practice.is_none());
    }

    #[test]
    fn parses_practice_options() {
        let o = parse_args(args(&[
            "--practicar",
            "basico",
            "--leccion",
            "3",
            "--idioma",
            "C",
        ]))
        .expect("válido");
        let p = o.practice.expect("práctica");
        assert_eq!(p.module, Module::Basic);
        assert_eq!(p.lesson, 3);
        assert_eq!(p.language, "C");
        assert_eq!(p.keyboard, "qwerty_es");
        let o = parse_args(args(&["-p"])).expect("válido");
        assert_eq!(o.practice.map(|p| p.module), Some(Module::Velocity));
    }

    #[test]
    fn rejects_unknown_options() {
        assert!(parse_args(args(&["--nope"])).is_err());
        assert!(parse_args(args(&["--leccion", "x"])).is_err());
    }
}
