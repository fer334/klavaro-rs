// SPDX-License-Identifier: GPL-3.0-or-later
//! Práctica en la terminal, provisoria hasta que exista la interfaz en iced.
//!
//! Muestra un ejercicio, lee lo que se tecleó (línea por línea, con Enter) y lo evalúa con el
//! mismo núcleo que usará la interfaz gráfica. Como la terminal entrega la línea completa, no
//! hay tiempos por tecla: la fluidez no se calcula.

use std::fmt::Write as _;
use std::fs;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use klavaro_core::Module;
use klavaro_core::generators::{LanguageRules, adaptability, basic, fluidity, velocity};
use klavaro_core::goals::{Comment, CommentContext, Goals};
use klavaro_core::keyboard::KeyboardLayout;
use klavaro_core::rng::SplitMix64;
use klavaro_core::session::{CharState, Feedback, Key, Session};
use klavaro_data::{kbd, lessons, texts};

const GREEN: &str = "\x1b[32m";
const RED: &str = "\x1b[31;4m";
const YELLOW: &str = "\x1b[33m";
const DIM: &str = "\x1b[2m";
const BOLD: &str = "\x1b[1m";
const RESET: &str = "\x1b[0m";

/// Qué practicar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PracticeOptions {
    pub(crate) module: Module,
    pub(crate) lesson: u16,
    pub(crate) language: String,
    pub(crate) keyboard: String,
}

impl Default for PracticeOptions {
    fn default() -> Self {
        Self {
            module: Module::Velocity,
            lesson: 1,
            language: "es".to_owned(),
            keyboard: "qwerty_es".to_owned(),
        }
    }
}

/// Carpeta de datos: `KLAVARO_ASSETS` o la carpeta `assets/` del repositorio.
fn assets_dir() -> PathBuf {
    std::env::var_os("KLAVARO_ASSETS").map_or_else(
        || Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets"),
        PathBuf::from,
    )
}

fn read_asset(name: &str) -> io::Result<String> {
    let path = assets_dir().join(name);
    fs::read_to_string(&path)
        .map_err(|e| io::Error::new(e.kind(), format!("{}: {e}", path.display())))
}

fn invalid(msg: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg.into())
}

/// Genera el texto del ejercicio.
fn exercise_text(opts: &PracticeOptions, layout: &KeyboardLayout, seed: u64) -> io::Result<String> {
    let rules = LanguageRules::new(opts.language.clone());
    let mut rng = SplitMix64::new(seed);
    let text = match opts.module {
        Module::Basic => {
            let masks = lessons::parse_basic_lessons(&read_asset("basic_lessons.txt")?)
                .map_err(|e| invalid(e.to_string()))?;
            let index = usize::from(opts.lesson.clamp(1, basic::FIXED_LESSONS)) - 1;
            let set = masks
                .get(index)
                .map(|m| m.charset(layout))
                .unwrap_or_default();
            basic::generate(&set, &rules, &mut rng)
        }
        Module::Adaptability => Some(adaptability::generate(layout, &rules, None, &mut rng)),
        Module::Velocity => {
            let words = texts::parse_words(&read_asset(&format!("{}.words", opts.language))?);
            velocity::generate(&words, &rules, &mut rng)
        }
        Module::Fluidity => {
            let pars =
                texts::parse_paragraphs(&read_asset(&format!("{}.paragraphs", opts.language))?);
            fluidity::select(&pars, 1, false, &mut rng)
        }
    };
    text.ok_or_else(|| invalid("no se pudo generar el ejercicio con estos datos"))
}

/// Ejecuta una práctica interactiva.
pub(crate) fn run(opts: &PracticeOptions) -> io::Result<()> {
    let layout_text = read_asset(&format!("{}.kbd", opts.keyboard))?;
    let layout = kbd::parse(&opts.keyboard, &layout_text).map_err(|e| invalid(e.to_string()))?;
    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(1, |d| d.as_secs());
    let mut text = exercise_text(opts, &layout, seed)?;
    // En la terminal, un ejercicio corto alcanza: sólo las dos primeras líneas.
    text = text.lines().take(2).flat_map(|l| [l, "\n"]).collect();

    let mut out = io::stdout().lock();
    writeln!(
        out,
        "{BOLD}klavaro-rs · {} · {} · {}{RESET}",
        module_name(opts.module),
        opts.language,
        opts.keyboard
    )?;
    writeln!(
        out,
        "{DIM}Copiá cada línea y presioná Enter al final de cada una.{RESET}\n"
    )?;

    // La terminal no permite corregir con Backspace tecla por tecla: Fluidez se evalúa como
    // Velocidad (sin corrección).
    let eval_module = if opts.module == Module::Fluidity {
        Module::Velocity
    } else {
        opts.module
    };
    let mut session = Session::new(eval_module, &text);
    let stdin = io::stdin();
    let started = Instant::now();
    for (i, line) in text.lines().enumerate() {
        writeln!(out, "{YELLOW}{line}{RESET}")?;
        out.flush()?;
        let mut typed = String::new();
        if stdin.lock().read_line(&mut typed)? == 0 {
            return Ok(());
        }
        let typed = typed.trim_end_matches(['\n', '\r']);
        feed_line(
            &mut session,
            typed,
            line.chars().count(),
            started.elapsed(),
            i,
        );
        writeln!(out, "{}", paint_line(&session, i))?;
    }
    if let Some(stats) = session.result() {
        let goals = Goals::default();
        let check = goals.evaluate(eval_module, stats);
        let mark = |ok: bool| if ok { GREEN } else { RED };
        writeln!(out)?;
        writeln!(out, "Tiempo:     {:.1} s", stats.elapsed.as_secs_f64())?;
        writeln!(
            out,
            "Precisión:  {}{:.1} %{RESET}",
            mark(check.accuracy),
            stats.accuracy
        )?;
        writeln!(
            out,
            "Velocidad:  {}{:.1} PPM{RESET}",
            mark(check.speed),
            stats.wpm
        )?;
        let comment = goals.comment(eval_module, stats, CommentContext::default());
        writeln!(out, "\n{BOLD}{}{RESET}", comment_text(comment))?;
    }
    Ok(())
}

/// Pasa una línea tecleada a la sesión, repartiendo el tiempo en partes iguales, y termina
/// con Enter. Lo que falta se cuenta como error; lo que sobra se ignora.
fn feed_line(session: &mut Session, typed: &str, expected_len: usize, now: Duration, line: usize) {
    let keys: Vec<Key> = typed
        .chars()
        .chain(std::iter::repeat('\u{0}'))
        .take(expected_len)
        .map(Key::Char)
        .chain(std::iter::once(Key::Enter))
        .collect();
    let total = keys.len().max(1);
    let base = now.saturating_sub(Duration::from_millis(200 * total as u64));
    for (k, key) in keys.into_iter().enumerate() {
        let at = base + Duration::from_millis(200 * k as u64) + Duration::from_millis(line as u64);
        if let Feedback::Finished(_) = session.press(key, at) {
            break;
        }
    }
}

/// Pinta una línea del texto objetivo según el estado de cada carácter.
fn paint_line(session: &Session, line: usize) -> String {
    let mut out = String::new();
    let mut current = 0;
    for (c, state) in session.target().iter().zip(session.states()) {
        if *c == '\n' {
            current += 1;
            continue;
        }
        if current != line {
            continue;
        }
        let color = match state {
            CharState::Correct => GREEN,
            CharState::Wrong => RED,
            CharState::Retouched => YELLOW,
            CharState::Untouched => DIM,
        };
        let _ = write!(out, "{color}{c}{RESET}");
    }
    out
}

fn module_name(m: Module) -> &'static str {
    match m {
        Module::Basic => "Curso básico",
        Module::Adaptability => "Adaptabilidad",
        Module::Velocity => "Velocidad",
        Module::Fluidity => "Fluidez",
    }
}

/// Textos provisorios en español; en la fase 2 vienen de Fluent.
fn comment_text(c: Comment) -> String {
    match c {
        Comment::TooManyErrors => ":-( Demasiados errores. Más despacio y con atención.".into(),
        Comment::BasicNextLesson => "¡Bien! Pasá a la próxima lección.".into(),
        Comment::BasicCourseCompleted => "¡Completaste el curso básico!".into(),
        Comment::AdaptTryAgain { target } => format!("Intentá de nuevo, hasta {target:.0} %."),
        Comment::AdaptImproving { target } => format!("Vas bien. Llegá a {target:.0} %."),
        Comment::AdaptAlmostThere { target } => format!("Casi: la meta es {target:.0} %."),
        Comment::AdaptDone => "¡Muy bien! Seguí con el ejercicio de Velocidad.".into(),
        Comment::VeloBePatient => "Paciencia: practicá un poco todos los días.".into(),
        Comment::VeloKeepGoing { target } => format!("Buen ritmo. Próxima meta: {target:.0} PPM."),
        Comment::VeloDone {
            next_target: Some(t),
        } => {
            format!("¡Meta cumplida! Si querés más: {t:.0} PPM.")
        }
        Comment::VeloDone { next_target: None } => "¡Velocidad de profesional!".into(),
        Comment::FluidTooSlow { target } => {
            format!("Preciso pero lento: apuntá a {target:.0} PPM.")
        }
        Comment::FluidIrregular { target } => format!("Ritmo irregular: apuntá a {target:.0}."),
        Comment::FluidAlmostThere { target } => format!("Casi: fluidez {target:.0}."),
        Comment::FluidCongratulations => "¡Felicitaciones!".into(),
        Comment::FluidBeyondExpectations => "¡Superaste todas las expectativas!".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exercises_generate_from_repo_assets() {
        let layout =
            kbd::parse("qwerty_es", &read_asset("qwerty_es.kbd").expect("kbd")).expect("kbd");
        for module in Module::ALL {
            let opts = PracticeOptions {
                module,
                ..PracticeOptions::default()
            };
            let text = exercise_text(&opts, &layout, 3).expect("texto");
            assert!(!text.trim().is_empty(), "{module:?}");
        }
    }

    #[test]
    fn feeding_a_perfect_line_marks_it_correct() {
        let mut s = Session::new(Module::Velocity, "hola mundo\n");
        feed_line(&mut s, "hola mundo", 10, Duration::from_secs(10), 0);
        assert!(s.is_finished());
        assert!(paint_line(&s, 0).contains(GREEN));
        assert!(!paint_line(&s, 0).contains(RED));
    }

    #[test]
    fn missing_chars_count_as_errors() {
        let mut s = Session::new(Module::Velocity, "hola\n");
        feed_line(&mut s, "ho", 4, Duration::from_secs(10), 0);
        let stats = s.result().copied().expect("terminó");
        assert_eq!(stats.errors, 2);
    }
}
