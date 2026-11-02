// SPDX-License-Identifier: GPL-3.0-or-later
//! Dictado y pitido con programas del sistema, sin bibliotecas de audio.
//!
//! - Dictado: `espeak-ng` o `espeak` (el original usa `espeak -v<idioma> -k1`) y, en macOS,
//!   `say` si no hay espeak.
//! - Pitido al equivocarse: el sonido de alerta del sistema (`afplay` en macOS,
//!   `canberra-gtk-play` o `paplay` en Linux, `MessageBeep` en Windows).
//!
//! Los programas se lanzan sin shell, con el texto como argumento. Cada dictado nuevo corta el
//! anterior para no acumular atraso al teclear rápido. Si un programa no se puede lanzar, se
//! deja de intentar (como `espeak_OK` del original).

use std::env;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

/// Sintetizador de voz.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Engine {
    /// `espeak` o `espeak-ng`, en esa ruta.
    Espeak(PathBuf),
    /// `say` de macOS.
    Say(PathBuf),
}

impl Engine {
    /// Busca un sintetizador en el `PATH`.
    pub(crate) fn detect() -> Option<Self> {
        find("espeak-ng")
            .or_else(|| find("espeak"))
            .map(Self::Espeak)
            .or_else(|| {
                cfg!(target_os = "macos")
                    .then(|| find("say"))
                    .flatten()
                    .map(Self::Say)
            })
    }

    /// Nombre para mostrar.
    pub(crate) fn name(&self) -> String {
        let path = match self {
            Self::Espeak(p) | Self::Say(p) => p,
        };
        path.file_stem()
            .map_or_else(String::new, |s| s.to_string_lossy().into_owned())
    }

    /// Comando para decir `text` con la voz `voice` (código de dos letras). `punct` hace que
    /// espeak pronuncie los signos, como hace el original con caracteres sueltos y palabras de
    /// Velocidad.
    pub(crate) fn command(&self, voice: &str, text: &str, punct: bool) -> Command {
        match self {
            Self::Espeak(path) => {
                let mut c = Command::new(path);
                c.arg(format!("-v{voice}")).arg("-k1");
                if punct {
                    c.arg("--punct");
                }
                // `--` para que un texto que empieza con `-` no se tome como opción.
                c.arg("--").arg(text);
                c
            }
            Self::Say(path) => {
                let mut c = Command::new(path);
                c.arg("--").arg(text);
                c
            }
        }
    }
}

/// Comando del sonido de alerta del sistema, si hay uno.
fn beep_command() -> Option<Command> {
    if cfg!(target_os = "macos") {
        let sound = Path::new("/System/Library/Sounds/Tink.aiff");
        let player = find("afplay")?;
        sound.exists().then(|| {
            let mut c = Command::new(player);
            c.arg(sound);
            c
        })
    } else if cfg!(windows) {
        let mut c = Command::new("rundll32");
        c.args(["user32.dll,MessageBeep"]);
        Some(c)
    } else if let Some(p) = find("canberra-gtk-play") {
        let mut c = Command::new(p);
        c.args(["--id", "bell"]);
        Some(c)
    } else {
        let sound = Path::new("/usr/share/sounds/freedesktop/stereo/bell.oga");
        let player = find("paplay")?;
        sound.exists().then(|| {
            let mut c = Command::new(player);
            c.arg(sound);
            c
        })
    }
}

/// Busca un ejecutable en el `PATH`.
fn find(name: &str) -> Option<PathBuf> {
    let exe = if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_owned()
    };
    env::split_paths(&env::var_os("PATH")?)
        .map(|dir| dir.join(&exe))
        .find(|p| p.is_file())
}

/// Un programa a la vez: lanzar uno nuevo corta el anterior.
#[derive(Debug, Default)]
struct Slot {
    child: Option<Child>,
    broken: bool,
}

impl Slot {
    fn run(&mut self, mut command: Command) {
        if self.broken {
            return;
        }
        self.stop();
        command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        match command.spawn() {
            Ok(child) => self.child = Some(child),
            Err(_) => self.broken = true,
        }
    }

    fn stop(&mut self) {
        if let Some(mut child) = self.child.take() {
            // Si ya terminó, `kill` falla sin problema; `wait` evita procesos zombis.
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

impl Drop for Slot {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Dictado y pitido de la interfaz.
#[derive(Debug, Default)]
pub(crate) struct Audio {
    engine: Option<Engine>,
    speech: Slot,
    beep: Slot,
}

impl Audio {
    /// Audio con el sintetizador dado (ver [`Engine::detect`]).
    pub(crate) fn new(engine: Option<Engine>) -> Self {
        Self {
            engine,
            ..Self::default()
        }
    }

    /// Sintetizador disponible.
    pub(crate) fn engine(&self) -> Option<&Engine> {
        self.engine.as_ref()
    }

    /// Dice `text` con la voz `voice`.
    pub(crate) fn say(&mut self, voice: &str, text: &str, punct: bool) {
        if text.trim().is_empty() {
            return;
        }
        if let Some(command) = self.engine().map(|e| e.command(voice, text, punct)) {
            self.speech.run(command);
        }
    }

    /// Calla el dictado en curso.
    pub(crate) fn hush(&mut self) {
        self.speech.stop();
    }

    /// Pitido de error.
    pub(crate) fn beep(&mut self) {
        if let Some(command) = beep_command() {
            self.beep.run(command);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(c: &Command) -> Vec<String> {
        c.get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn espeak_gets_voice_and_text_as_arguments() {
        let e = Engine::Espeak(PathBuf::from("/usr/bin/espeak-ng"));
        assert_eq!(e.name(), "espeak-ng");
        let c = e.command("es", "-; rm -rf ~", true);
        assert_eq!(args(&c), ["-ves", "-k1", "--punct", "--", "-; rm -rf ~"]);
        assert_eq!(
            args(&e.command("en", "hola", false)),
            ["-ven", "-k1", "--", "hola"]
        );
    }

    #[test]
    fn say_ignores_the_voice() {
        let e = Engine::Say(PathBuf::from("/usr/bin/say"));
        assert_eq!(args(&e.command("es", "hola", true)), ["--", "hola"]);
    }

    #[test]
    fn a_missing_program_is_not_retried() {
        let mut slot = Slot::default();
        slot.run(Command::new("/no/existe/klavaro-audio"));
        assert!(slot.broken);
        slot.run(Command::new("/no/existe/klavaro-audio"));
        assert!(slot.child.is_none());
    }
}
