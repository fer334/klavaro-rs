// SPDX-License-Identifier: GPL-3.0-or-later
//! klavaro-rs: tutor de mecanografía.
//!
//! La interfaz en iced llega en la fase 2. Por ahora el binario reconoce las opciones de
//! línea de comandos del original.

use std::io::{self, Write};
use std::process::ExitCode;

const HELP: &str = "\
klavaro-rs — tutor de mecanografía

Uso: klavaro-rs [OPCIONES]

Opciones:
  -h, --help      Muestra esta ayuda
  -v, --version   Muestra la versión
  -x, --velotxt   El módulo Velocidad usa textos completos, como Fluidez
";

/// Opciones de línea de comandos.
#[derive(Debug, Default, PartialEq, Eq)]
struct Options {
    help: bool,
    version: bool,
    velotxt: bool,
}

fn parse_args(args: impl IntoIterator<Item = String>) -> Result<Options, String> {
    let mut opts = Options::default();
    for arg in args {
        match arg.as_str() {
            "-h" | "--help" => opts.help = true,
            "-v" | "--version" => opts.version = true,
            "-x" | "--velotxt" => opts.velotxt = true,
            other => return Err(format!("opción desconocida: {other}")),
        }
    }
    Ok(opts)
}

fn main() -> ExitCode {
    let mut out = io::stdout().lock();
    let opts = match parse_args(std::env::args().skip(1)) {
        Ok(o) => o,
        Err(msg) => {
            let _ = writeln!(io::stderr(), "{msg}\nProbá `klavaro-rs --help`.");
            return ExitCode::FAILURE;
        }
    };
    let result = if opts.version {
        writeln!(out, "klavaro-rs {}", env!("CARGO_PKG_VERSION"))
    } else if opts.help {
        write!(out, "{HELP}")
    } else {
        writeln!(
            out,
            "klavaro-rs {}: la interfaz gráfica todavía no está disponible.",
            env!("CARGO_PKG_VERSION")
        )
    };
    if result.is_err() {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
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
        assert_eq!(
            o,
            Options {
                help: false,
                version: true,
                velotxt: true
            }
        );
    }

    #[test]
    fn rejects_unknown_options() {
        assert!(parse_args(args(&["--nope"])).is_err());
    }
}
