// SPDX-License-Identifier: GPL-3.0-or-later
//! Estado de la pantalla del tutor, independiente de iced.
//!
//! La interfaz traduce cada evento de teclado a un [`Input`] y llama a
//! [`Tutor::handle`]; después dibuja [`Tutor::segments`]. Así toda la lógica de la pantalla
//! se prueba sin abrir una ventana.

use std::collections::BTreeMap;
use std::io;
use std::time::Duration;

use klavaro_core::Module;
use klavaro_core::generators::basic::FIXED_LESSONS;
use klavaro_core::goals::{Comment, CommentContext, GoalCheck, Goals};
use klavaro_core::keyboard::FingerMap;
use klavaro_core::keyboard::KeyboardLayout;
use klavaro_core::metrics::Stats;
use klavaro_core::pairs::{self, PairFrequencies, PairTracker};
use klavaro_core::progression::LessonResult;
use klavaro_core::session::{CharState, Feedback, Key, Session};
use klavaro_core::tracker::{KeyTracker, SpecialSession};
use klavaro_core::transitions::{self, TransitionSummary};
use klavaro_data::catalog::CatalogEntry;
use klavaro_store::PreviousTransitions;

use crate::assets;
use crate::exercise::{self, ExerciseSpec};
use crate::speech::{self, Say};
use crate::strokes::{KeyId, Stroke, StrokeLog};

/// Flecha del teclado, para moverse por las grillas de la Progresión.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Direction {
    Up,
    Down,
    Left,
    Right,
}

/// Tecla especial, ya separada del framework de interfaz.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NamedKey {
    /// Enter.
    Enter,
    /// Backspace.
    Backspace,
    /// Escape.
    Escape,
    /// Barra espaciadora (cuando el sistema no entrega texto).
    Space,
    /// Una flecha.
    Arrow(Direction),
}

/// Acción del usuario en la pantalla del tutor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Input {
    /// Carácter tecleado.
    Char(char),
    /// Enter.
    Enter,
    /// Backspace o Ctrl+H.
    Backspace,
    /// Ctrl+W.
    DeleteWord,
    /// Esc o Ctrl+R: nuevo ejercicio.
    Restart,
    /// Ctrl+A: colores alternativos.
    ToggleTheme,
    /// Ctrl+`+`: lección siguiente.
    LessonUp,
    /// Ctrl+`-`: lección anterior.
    LessonDown,
    /// Una flecha: mueve la selección en las grillas. En un ejercicio no hace nada.
    Move(Direction),
}

/// Traduce una tecla a una acción, con los atajos del original.
pub(crate) fn translate(named: Option<NamedKey>, text: Option<&str>, ctrl: bool) -> Option<Input> {
    if ctrl {
        let c = text
            .and_then(|t| t.chars().next())
            .map(|c| c.to_ascii_lowercase());
        return match c {
            Some('h') => Some(Input::Backspace),
            Some('w') => Some(Input::DeleteWord),
            Some('r') => Some(Input::Restart),
            Some('a') => Some(Input::ToggleTheme),
            Some('+' | '=') => Some(Input::LessonUp),
            Some('-') => Some(Input::LessonDown),
            _ => None,
        };
    }
    match named {
        Some(NamedKey::Enter) => return Some(Input::Enter),
        Some(NamedKey::Backspace) => return Some(Input::Backspace),
        Some(NamedKey::Escape) => return Some(Input::Restart),
        Some(NamedKey::Space) => return Some(Input::Char(' ')),
        Some(NamedKey::Arrow(d)) => return Some(Input::Move(d)),
        None => {}
    }
    let mut chars = text?.chars().filter(|c| !c.is_control());
    match (chars.next(), chars.next()) {
        (Some(c), None) => Some(Input::Char(c)),
        _ => None,
    }
}

/// Estilo de un tramo de texto.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SegmentStyle {
    /// Por teclear.
    Untouched,
    /// Bien.
    Correct,
    /// Mal.
    Wrong,
    /// Reescrito tras corregir.
    Retouched,
    /// Posición del cursor.
    Cursor,
    /// Bien, y cerró una transición fluida hace un instante. El nivel (1 a
    /// [`FLASH_LEVELS`]) baja mientras el destello se apaga.
    Flash(u8),
}

/// Cuánto dura el destello de una transición fluida.
pub(crate) const FLASH: Duration = Duration::from_millis(250);
/// Niveles de intensidad del destello, del más tenue al más fuerte.
pub(crate) const FLASH_LEVELS: u8 = 4;
/// Racha mínima que se muestra mientras se escribe.
pub(crate) const STREAK_MIN: u32 = 5;

/// Tramo de texto con un mismo estilo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Segment {
    pub(crate) text: String,
    pub(crate) style: SegmentStyle,
}

/// Resultado mostrado al terminar.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Outcome {
    pub(crate) stats: Stats,
    pub(crate) check: GoalCheck,
    pub(crate) comment: Comment,
    /// En Básico: se aprobó y Enter pasa a la lección siguiente.
    pub(crate) advance: bool,
    /// En la Progresión: estrellas, meta y tiempo de la lección.
    pub(crate) lesson: Option<LessonResult>,
    /// En la Progresión: de dónde salió el texto.
    pub(crate) source: Option<CatalogEntry>,
    /// En la Progresión: transiciones fluidas. No cambia el puntaje ni las estrellas.
    pub(crate) transitions: Option<TransitionSummary>,
    /// En la Progresión: transiciones del intento con que se compara (lo pone la interfaz al
    /// guardar el intento).
    pub(crate) previous_transitions: Option<PreviousTransitions>,
}

/// Efecto de una acción que la interfaz tiene que aplicar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[allow(clippy::struct_excessive_bools)] // Son efectos independientes entre sí.
pub(crate) struct Effect {
    /// Hubo un error o una tecla no válida (el original pita).
    pub(crate) beep: bool,
    /// Alternar colores.
    pub(crate) toggle_theme: bool,
    /// El ejercicio acaba de terminar: hay que registrarlo.
    pub(crate) finished: bool,
    /// Progresión: se pidió seguir después de terminar la lección.
    pub(crate) next_lesson: bool,
}

/// Pantalla del tutor: ejercicio en curso y su resultado.
#[derive(Debug, Clone)]
pub(crate) struct Tutor {
    spec: ExerciseSpec,
    layout: KeyboardLayout,
    goals: Goals,
    session: Session,
    outcome: Option<Outcome>,
    seed: u64,
    /// Errores y tiempos por tecla del teclado actual. Como en el original, no se rastrea el
    /// curso básico.
    tracker: KeyTracker,
    /// Adaptabilidad con palabras que insisten en las teclas débiles.
    special: bool,
    /// Cuántos ejercicios se generaron (para saber si cambió el texto).
    generation: u32,
    /// En la Progresión: de dónde salió el texto.
    source: Option<CatalogEntry>,
    /// Errores de este ejercicio por tecla.
    mistakes: BTreeMap<char, u32>,
    /// Hay un error sin corregir: lo que se teclee hasta borrarlo no cuenta como otro error.
    typo_open: bool,
    /// En la Progresión: cuándo se apretó y se soltó la tecla de cada letra. Sólo observa.
    strokes: Option<StrokeLog>,
    /// Dedos de cada tecla, para clasificar las transiciones (si se pudieron leer).
    fingers: Option<FingerMap>,
    /// Letras que cerraron una transición fluida y cuándo, para el destello.
    flashes: Vec<(usize, Duration)>,
    /// Transiciones fluidas seguidas hasta la última letra.
    streak: u32,
    /// Tiempos y errores por par de letras del teclado actual. Sólo los carga la Progresión.
    pairs: PairTracker,
}

/// Variable de entorno que, si está definida, imprime los tiempos de cada letra al terminar
/// una lección de la Progresión.
const KEYLOG_ENV: &str = "KLAVARO_KEYLOG";

/// Cuántos pares lentos lista el registro de depuración.
const KEYLOG_PAIRS: usize = 10;

/// Los pares de letras más lentos del rastreo, para calibrar (ver
/// `PLAN_TRANSICIONES_DEBILES.md`).
///
/// `frequencies` dice cuán común es cada par en el idioma, para ordenar los débiles.
fn slow_pairs_report(pairs: &PairTracker, frequencies: &PairFrequencies) -> String {
    use std::fmt::Write as _;
    let ms = |d: Option<Duration>| d.map_or(0.0, |d| d.as_secs_f64() * 1000.0);
    let mut out = String::from(
        "pares más lentos (acumulado):\npar\tmediana\tajustada\ttipo\tmuestras\terrores\n",
    );
    for e in pairs.slowest(KEYLOG_PAIRS) {
        let _ = writeln!(
            out,
            "{}{}\t{:.0}\t{:.0}\t{}\t{}\t{} de {}",
            e.from,
            e.to,
            ms(e.median()),
            ms(e.adjusted()),
            e.kind.code(),
            e.samples().len(),
            e.wrong,
            e.typed
        );
    }
    match pairs.typical() {
        Some(typical) => {
            let _ = writeln!(out, "tiempo habitual: {:.0} ms", ms(Some(typical)));
        }
        None => out.push_str("tiempo habitual: todavía no hay suficientes pares medidos\n"),
    }
    out.push_str("pares débiles:\npar\tmediana\texceso\terrores\tpor qué\tfrecuencia\n");
    for w in pairs.weak(frequencies, KEYLOG_PAIRS) {
        let why = match (w.slow, w.error_prone) {
            (true, true) => "lento y se falla",
            (true, false) => "lento",
            _ => "se falla",
        };
        let _ = writeln!(
            out,
            "{}{}\t{}\t{}\t{:.0} %\t{why}\t{}",
            w.from,
            w.to,
            w.median
                .map_or("-".to_owned(), |d| format!("{:.0}", ms(Some(d)))),
            w.excess.map_or("-".to_owned(), |x| format!("{x:.2}")),
            w.error_rate * 100.0,
            frequencies.get(&(w.from, w.to)).copied().unwrap_or(0)
        );
    }
    out
}

/// Registro de tiempos para un ejercicio nuevo: sólo en la Progresión.
fn stroke_log(module: Module, session: &Session) -> Option<StrokeLog> {
    (module == Module::Progression).then(|| StrokeLog::new(session.target().len()))
}

impl Tutor {
    /// Crea la pantalla con un primer ejercicio.
    pub(crate) fn new(
        spec: ExerciseSpec,
        layout: KeyboardLayout,
        goals: Goals,
        seed: u64,
        mut tracker: KeyTracker,
    ) -> io::Result<Self> {
        tracker.sort();
        let text = exercise::generate(&spec, &layout, seed, None)?;
        let session = Session::new(spec.module, &text);
        let source = spec
            .slot
            .map(|slot| assets::lesson(&spec.language, slot))
            .transpose()?;
        let strokes = stroke_log(spec.module, &session);
        // Sin el mapa de dedos las transiciones se miden igual, con el umbral general.
        let fingers = (spec.module == Module::Progression)
            .then(|| assets::fingers().ok())
            .flatten();
        Ok(Self {
            fingers,
            flashes: Vec::new(),
            streak: 0,
            pairs: PairTracker::new(),
            spec,
            layout,
            goals,
            session,
            strokes,
            outcome: None,
            seed,
            tracker,
            special: false,
            generation: 0,
            source,
            mistakes: BTreeMap::new(),
            typo_open: false,
        })
    }

    /// Parte del rastreo por par de letras guardado, en lugar de uno vacío.
    pub(crate) fn with_pairs(mut self, pairs: PairTracker) -> Self {
        self.pairs = pairs;
        self
    }

    /// Rastreo por par de letras.
    pub(crate) fn pairs(&self) -> &PairTracker {
        &self.pairs
    }

    /// Rastreo por tecla.
    pub(crate) fn tracker(&self) -> &KeyTracker {
        &self.tracker
    }

    /// Devuelve el rastreo al salir de la pantalla.
    pub(crate) fn into_tracker(self) -> KeyTracker {
        self.tracker
    }

    /// Por qué convendría una sesión especial de Adaptabilidad (el original muestra entonces
    /// un botón para activarla).
    pub(crate) fn special_suggested(&self) -> Option<SpecialSession> {
        (self.spec.module == Module::Adaptability)
            .then(|| self.tracker.special_session())
            .flatten()
    }

    /// Si la sesión especial está activa.
    pub(crate) fn special(&self) -> bool {
        self.special
    }

    /// Activa o desactiva la sesión especial y genera un ejercicio nuevo.
    pub(crate) fn toggle_special(&mut self) -> io::Result<()> {
        self.special = !self.special && self.spec.module == Module::Adaptability;
        self.restart()
    }

    /// Distribución de teclado.
    pub(crate) fn layout(&self) -> &KeyboardLayout {
        &self.layout
    }

    /// Carácter que toca teclear (ninguno si terminó).
    pub(crate) fn next_char(&self) -> Option<char> {
        if self.outcome.is_some() {
            return None;
        }
        self.session.target().get(self.session.cursor()).copied()
    }

    /// Datos del ejercicio.
    pub(crate) fn spec(&self) -> &ExerciseSpec {
        &self.spec
    }

    /// Resultado, si terminó.
    pub(crate) fn outcome(&self) -> Option<&Outcome> {
        self.outcome.as_ref()
    }

    /// Texto del ejercicio.
    pub(crate) fn text(&self) -> String {
        self.session.target().iter().collect()
    }

    /// Qué dictar ahora (nada si terminó) y en qué posición: el número de ejercicio y el
    /// cursor, para no repetir el mismo dictado.
    pub(crate) fn dictation(&self) -> Option<((u32, usize), Say)> {
        if self.outcome.is_some() {
            return None;
        }
        let cursor = self.session.cursor();
        speech::utterance(self.spec.module, self.session.target(), cursor)
            .map(|say| ((self.generation, cursor), say))
    }

    /// Avance: caracteres tecleados y total.
    pub(crate) fn progress(&self) -> (usize, usize) {
        (self.session.cursor(), self.session.target().len())
    }

    /// Caracteres del texto que este teclado no tiene, sin repetir. No impide hacer el ejercicio.
    pub(crate) fn missing_keys(&self) -> Vec<char> {
        let mut missing: Vec<char> = self
            .session
            .target()
            .iter()
            .copied()
            .filter(|&c| !c.is_whitespace() && !self.layout.contains(c))
            .collect();
        missing.sort_unstable();
        missing.dedup();
        missing
    }

    /// Las teclas en las que más se erró en este ejercicio, de más a menos.
    pub(crate) fn weak_keys(&self) -> Vec<char> {
        let mut keys: Vec<(char, u32)> = self.mistakes.iter().map(|(&k, &n)| (k, n)).collect();
        keys.sort_by_key(|&(key, count)| (std::cmp::Reverse(count), key));
        keys.into_iter().take(5).map(|(key, _)| key).collect()
    }

    /// Si conviene una sesión especial de Adaptabilidad para las teclas débiles.
    pub(crate) fn practice_suggested(&self) -> bool {
        self.tracker.special_session().is_some()
    }

    /// Letras del texto que no están marcadas como error ahora mismo: las escritas bien, las
    /// corregidas y las que faltan escribir.
    pub(crate) fn correct_letters(&self) -> usize {
        self.session.target().len() - self.session.wrong_letters()
    }

    /// Tiempo desde la primera tecla hasta la última.
    pub(crate) fn elapsed(&self) -> Duration {
        self.session.elapsed()
    }

    /// Genera un ejercicio nuevo con la misma configuración.
    pub(crate) fn restart(&mut self) -> io::Result<()> {
        self.generation = self.generation.wrapping_add(1);
        self.seed = self
            .seed
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1);
        // El original ordena el rastreo al empezar cada ejercicio.
        self.tracker.sort();
        let weak =
            (self.special && self.spec.module == Module::Adaptability).then_some(&self.tracker);
        let text = exercise::generate(&self.spec, &self.layout, self.seed, weak)?;
        self.session = Session::new(self.spec.module, &text);
        self.strokes = stroke_log(self.spec.module, &self.session);
        self.flashes.clear();
        self.streak = 0;
        self.outcome = None;
        self.mistakes.clear();
        self.typo_open = false;
        Ok(())
    }

    /// Cambia de lección (sólo Básico) y reinicia.
    pub(crate) fn set_lesson(&mut self, lesson: u16) -> io::Result<()> {
        self.spec.lesson = lesson.clamp(1, FIXED_LESSONS);
        self.restart()
    }

    /// Procesa una acción en el instante `at`.
    pub(crate) fn handle(&mut self, input: Input, at: Duration) -> io::Result<Effect> {
        let mut effect = Effect::default();
        match input {
            Input::ToggleTheme => effect.toggle_theme = true,
            Input::Move(_) => {}
            Input::Restart => self.restart()?,
            Input::LessonUp if self.spec.module == Module::Basic => {
                self.set_lesson(self.spec.lesson.saturating_add(1))?;
            }
            Input::LessonDown if self.spec.module == Module::Basic => {
                self.set_lesson(self.spec.lesson.saturating_sub(1))?;
            }
            Input::LessonUp | Input::LessonDown => effect.beep = true,
            _ if self.outcome.is_some() => self.after_end(input, &mut effect)?,
            Input::Char(c) => self.press(Key::Char(c), at, &mut effect),
            Input::Enter => self.press(Key::Enter, at, &mut effect),
            Input::Backspace => self.press(Key::Backspace, at, &mut effect),
            Input::DeleteWord => self.press(Key::DeleteWord, at, &mut effect),
        }
        Ok(effect)
    }

    /// Como [`Tutor::handle`], pero además anota en qué instante y con qué tecla física se
    /// escribió cada letra. La evaluación es exactamente la misma.
    pub(crate) fn handle_stroke(&mut self, input: Input, stroke: Stroke) -> io::Result<Effect> {
        let typing = self.outcome.is_none() && matches!(input, Input::Char(_) | Input::Enter);
        let before = self.session.cursor();
        let effect = self.handle(input, stroke.at)?;
        if typing
            && self.session.cursor() > before
            && let Some(log) = &mut self.strokes
        {
            log.down(before, stroke);
            if !effect.finished {
                self.update_flow(before, stroke.at);
            }
        } else if matches!(input, Input::Backspace | Input::DeleteWord) {
            // Corregir corta la racha: la letra reescrita ya no cuenta como fluida.
            self.streak = 0;
        }
        if effect.finished {
            self.attach_transitions();
            self.report_strokes();
        }
        Ok(effect)
    }

    /// Se soltó una tecla física.
    pub(crate) fn key_up(&mut self, key: KeyId, at: Duration) {
        if let Some(log) = &mut self.strokes {
            log.up(key, at);
        }
        if self.outcome.is_some() {
            self.report_strokes();
        }
    }

    /// Después de anotar la letra `pos`: si cerró una transición fluida, destella, y la racha
    /// se actualiza. Se analiza sólo lo escrito hasta esa letra.
    fn update_flow(&mut self, pos: usize, at: Duration) {
        let (Some(log), Some(slot)) = (&self.strokes, self.spec.slot) else {
            return;
        };
        let end = pos + 1;
        let target = self.session.target().get(..end).unwrap_or_default();
        let states = self.session.states().get(..end).unwrap_or_default();
        let letters = log.letters(target, states, &self.layout, self.fingers.as_ref());
        let analysis = transitions::analyze(&letters, slot.target_wpm());
        let fluent = analysis
            .transitions
            .last()
            .is_some_and(|t| t.at == pos && t.fluent);
        self.streak = analysis.summary.current_streak;
        // Los destellos viejos ya no se ven: no hace falta guardarlos.
        self.flashes.retain(|&(_, t)| at.saturating_sub(t) < FLASH);
        if fluent {
            self.flashes.push((pos, at));
        }
    }

    /// Intensidad del destello de la letra `i` en el instante `now`, si sigue encendido.
    fn flash_level(&self, i: usize, now: Duration) -> Option<u8> {
        let &(_, at) = self.flashes.iter().rev().find(|&&(p, _)| p == i)?;
        let age = now.checked_sub(at)?;
        if age >= FLASH {
            return None;
        }
        let left = 1.0 - age.as_secs_f64() / FLASH.as_secs_f64();
        let levels = f64::from(FLASH_LEVELS);
        // Entre 1 y FLASH_LEVELS: redondea hacia arriba para no apagarse antes de tiempo.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        Some((left * levels).ceil().clamp(1.0, levels) as u8)
    }

    /// Si hay algún destello encendido en `now` (mientras haya, hay que redibujar).
    pub(crate) fn flashing(&self, now: Duration) -> bool {
        self.outcome.is_none()
            && self
                .flashes
                .iter()
                .any(|&(_, at)| now.checked_sub(at).is_some_and(|age| age < FLASH))
    }

    /// Racha en curso, si alcanza para mostrarla.
    pub(crate) fn streak(&self) -> Option<u32> {
        (self.outcome.is_none() && self.streak >= STREAK_MIN).then_some(self.streak)
    }

    /// Lo que se sabe de cómo se escribió cada letra (sólo en la Progresión).
    fn letters(&self) -> Option<Vec<transitions::Letter>> {
        let log = self.strokes.as_ref()?;
        Some(log.letters(
            self.session.target(),
            self.session.states(),
            &self.layout,
            self.fingers.as_ref(),
        ))
    }

    /// Analiza las transiciones del ejercicio recién terminado.
    fn analyze_transitions(&self) -> Option<transitions::Analysis> {
        let slot = self.spec.slot?;
        Some(transitions::analyze(&self.letters()?, slot.target_wpm()))
    }

    /// Agrega las transiciones al resultado y carga el rastreo por par. Se hace después de
    /// anotar la última letra, que la sesión evalúa antes de que se registren sus tiempos.
    fn attach_transitions(&mut self) {
        let summary = self.analyze_transitions().map(|a| a.summary);
        if let Some(outcome) = &mut self.outcome {
            outcome.transitions = summary;
        }
        if let Some(letters) = self.letters() {
            self.pairs.record_exercise(self.session.target(), &letters);
        }
    }

    /// Guarda con qué intento anterior se comparan las transiciones del resultado.
    pub(crate) fn set_previous_transitions(&mut self, previous: Option<PreviousTransitions>) {
        if let Some(outcome) = &mut self.outcome {
            outcome.previous_transitions = previous;
        }
    }

    /// Tiempos de cada letra (sólo en la Progresión).
    // Lo usan las pruebas y el cálculo de transiciones (Fase 2 de PLAN_TRANSICIONES.md).
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn strokes(&self) -> Option<&StrokeLog> {
        self.strokes.as_ref()
    }

    /// Con [`KEYLOG_ENV`] definida, imprime los tiempos una vez que el ejercicio terminó y se
    /// soltaron todas sus teclas.
    fn report_strokes(&mut self) {
        let Some(log) = &mut self.strokes else {
            return;
        };
        if std::env::var_os(KEYLOG_ENV).is_none() || !log.take_report() {
            return;
        }
        let analysis = self.analyze_transitions();
        if let Some(log) = &self.strokes {
            let (target, states) = (self.session.target(), self.session.states());
            eprintln!("{}", log.dump(target, states, analysis.as_ref()));
            // Sin la lista de palabras, los débiles se ordenan sólo por gravedad.
            let words = assets::words(&self.spec.language).unwrap_or_default();
            let frequencies = pairs::frequencies(words.iter().map(String::as_str));
            eprintln!("{}", slow_pairs_report(&self.pairs, &frequencies));
        }
    }

    /// Teclas después de terminar: Enter sigue (y en Básico aprobado, avanza de lección);
    /// Backspace en Básico vuelve a la lección anterior, como el original.
    fn after_end(&mut self, input: Input, effect: &mut Effect) -> io::Result<()> {
        let advance = self.outcome.as_ref().is_some_and(|o| o.advance);
        match input {
            Input::Enter if self.spec.module == Module::Progression => {
                effect.next_lesson = true;
                Ok(())
            }
            Input::Enter if advance => self.set_lesson(self.spec.lesson + 1),
            Input::Enter => self.restart(),
            Input::Backspace if self.spec.module == Module::Basic => {
                self.set_lesson(self.spec.lesson.saturating_sub(1))
            }
            _ => {
                effect.beep = true;
                Ok(())
            }
        }
    }

    fn press(&mut self, key: Key, at: Duration, effect: &mut Effect) {
        let track = self.spec.module != Module::Basic;
        match self.session.press(key, at) {
            Feedback::Correct { expected, interval } if track => {
                self.typo_open = false;
                self.tracker.correct(&self.layout, expected, interval);
            }
            Feedback::Retouched { expected } if track => {
                self.typo_open = false;
                self.tracker.correct(&self.layout, expected, None);
            }
            Feedback::Wrong { expected, .. } => {
                if track {
                    self.tracker.wrong(&self.layout, expected);
                }
                if !self.typo_open {
                    *self.mistakes.entry(expected).or_insert(0) += 1;
                }
                self.typo_open = true;
                effect.beep = true;
            }
            Feedback::Ignored => effect.beep = true,
            Feedback::Finished(stats) => {
                self.finish(stats);
                effect.finished = true;
            }
            Feedback::Erased => self.typo_open = false,
            Feedback::Correct { .. } | Feedback::Retouched { .. } => {}
        }
    }

    fn finish(&mut self, stats: Stats) {
        let module = self.spec.module;
        let check = self.goals.evaluate(module, &stats);
        let passed_basic = module == Module::Basic && check.accuracy;
        let ctx = CommentContext {
            course_completed: passed_basic && self.spec.lesson >= FIXED_LESSONS,
        };
        let comment = self.goals.comment(module, &stats, ctx);
        // Meta de Adaptabilidad cumplida: el original borra los errores acumulados.
        if matches!(comment, Comment::AdaptDone { .. }) {
            self.tracker.reset_errors();
            self.special = false;
        }
        let advance = passed_basic && self.spec.lesson < FIXED_LESSONS;
        let lesson = self
            .spec
            .slot
            .map(|slot| LessonResult::new(slot, self.correct_letters(), &stats));
        self.outcome = Some(Outcome {
            stats,
            check,
            comment,
            advance,
            lesson,
            source: self.source.clone(),
            transitions: None,
            previous_transitions: None,
        });
    }

    /// Texto del ejercicio en tramos con estilo. Los saltos de línea se muestran como `¶`.
    // La interfaz usa `segments_at`; ésta queda para las pruebas.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn segments(&self) -> Vec<Segment> {
        self.segments_at(None)
    }

    /// Como [`Tutor::segments`], con los destellos encendidos en el instante `now`.
    pub(crate) fn segments_at(&self, now: Option<Duration>) -> Vec<Segment> {
        let cursor = (self.outcome.is_none()).then(|| self.session.cursor());
        let mut out: Vec<Segment> = Vec::new();
        for (i, (&c, &state)) in self
            .session
            .target()
            .iter()
            .zip(self.session.states())
            .enumerate()
        {
            let style = if Some(i) == cursor {
                SegmentStyle::Cursor
            } else {
                match state {
                    CharState::Untouched => SegmentStyle::Untouched,
                    CharState::Correct => now
                        .and_then(|now| self.flash_level(i, now))
                        .map_or(SegmentStyle::Correct, SegmentStyle::Flash),
                    CharState::Wrong => SegmentStyle::Wrong,
                    CharState::Retouched => SegmentStyle::Retouched,
                }
            };
            let shown = if c == '\n' {
                "¶\n".to_owned()
            } else {
                c.to_string()
            };
            match out.last_mut() {
                Some(last) if last.style == style && style != SegmentStyle::Cursor => {
                    last.text.push_str(&shown);
                }
                _ => out.push(Segment { text: shown, style }),
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tutor(module: Module) -> Tutor {
        let layout = assets::keyboard("qwerty_es").expect("kbd");
        let spec = ExerciseSpec {
            module,
            lesson: 1,
            paragraphs: 1,
            language: "es".to_owned(),
            slot: None,
            exam: false,
        };
        Tutor::new(spec, layout, Goals::default(), 42, KeyTracker::new()).expect("tutor")
    }

    fn type_target(t: &mut Tutor, wrong_every: Option<usize>) {
        let target: Vec<char> = t.session.target().to_vec();
        for (i, c) in target.into_iter().enumerate() {
            let at = Duration::from_millis(200 * i as u64);
            let input = match (c, wrong_every) {
                (_, Some(n)) if i % n == 0 => Input::Char('#'),
                ('\n', _) => Input::Enter,
                (c, _) => Input::Char(c),
            };
            t.handle(input, at).expect("tecla");
        }
    }

    fn lesson_tutor(section: u16, lesson: u16) -> Tutor {
        let layout = assets::keyboard("qwerty_es").expect("kbd");
        let spec = ExerciseSpec {
            module: Module::Progression,
            lesson: 1,
            paragraphs: 1,
            language: crate::sections::LESSONS_LANGUAGE.to_owned(),
            slot: klavaro_core::progression::Slot::new(section, lesson),
            exam: false,
        };
        Tutor::new(spec, layout, Goals::default(), 42, KeyTracker::new()).expect("tutor")
    }

    /// Escribe todo el texto bien, con `ms` por tecla. Devuelve las veces que terminó.
    fn type_lesson(t: &mut Tutor, ms: u64) -> usize {
        let mut finished = 0;
        for (i, c) in t.text().chars().enumerate() {
            let at = Duration::from_millis(ms * i as u64);
            finished += usize::from(t.handle(Input::Char(c), at).expect("tecla").finished);
        }
        finished
    }

    #[test]
    fn a_lesson_on_time_earns_five_stars() {
        let mut t = lesson_tutor(20, 30);
        // A 20 PPM toca una tecla cada 600 ms.
        assert_eq!(type_lesson(&mut t, 500), 1);
        let result = t.outcome().and_then(|o| o.lesson).expect("resultado");
        assert_eq!(result.stars, 5);
        assert!((result.target_wpm - 20.0).abs() < 1e-9);
        assert!(t.elapsed() > Duration::ZERO);
    }

    #[test]
    fn a_slow_lesson_earns_fewer_stars() {
        let mut t = lesson_tutor(20, 30);
        type_lesson(&mut t, 800);
        let result = t.outcome().and_then(|o| o.lesson).expect("resultado");
        assert_eq!(result.stars, 2);
    }

    #[test]
    fn a_lesson_marks_only_the_wrong_letter_and_backspace_corrects() {
        let mut t = lesson_tutor(20, 1);
        let text: Vec<char> = t.text().chars().collect();
        let len = text.len();
        t.handle(Input::Char('#'), Duration::ZERO).expect("tecla");
        assert_eq!(t.progress().0, 1, "el cursor sigue tras el error");
        t.handle(Input::Char(text[1]), Duration::from_millis(10))
            .expect("tecla");
        let segs = t.segments();
        assert_eq!(segs[0].style, SegmentStyle::Wrong);
        assert_eq!(segs[1].style, SegmentStyle::Correct);
        assert_eq!(t.correct_letters(), len - 1);

        // Corregir es opcional: Backspace vuelve atrás y la letra reescrita queda corregida.
        t.handle(Input::Backspace, Duration::from_millis(20))
            .expect("borra");
        t.handle(Input::Backspace, Duration::from_millis(30))
            .expect("borra");
        assert_eq!(t.progress().0, 0);
        t.handle(Input::Char(text[0]), Duration::from_millis(40))
            .expect("corrige");
        assert_eq!(t.segments()[0].style, SegmentStyle::Retouched);
        assert_eq!(t.correct_letters(), len);
    }

    #[test]
    fn a_lesson_finishes_with_uncorrected_mistakes_and_net_speed() {
        let mut t = lesson_tutor(20, 30);
        let text: Vec<char> = t.text().chars().collect();
        let mut at = Duration::ZERO;
        for (i, &c) in text.iter().enumerate() {
            let c = if i == 0 { '#' } else { c };
            let key = if c == '\n' {
                Input::Enter
            } else {
                Input::Char(c)
            };
            t.handle(key, at).expect("tecla");
            at += Duration::from_millis(500);
        }
        let outcome = t.outcome().expect("terminó");
        assert_eq!(outcome.stats.errors, 1);
        let result = outcome.lesson.expect("resultado");
        let net = klavaro_core::progression::speed(text.len() - 1, outcome.stats.elapsed);
        assert!((result.speed - net).abs() < 1e-9);
    }

    #[test]
    fn after_a_lesson_enter_asks_to_continue_and_esc_repeats_the_text() {
        let mut t = lesson_tutor(20, 1);
        let text = t.text();
        type_lesson(&mut t, 500);
        let effect = t.handle(Input::Enter, Duration::ZERO).expect("enter");
        assert!(effect.next_lesson);
        assert!(t.outcome().is_some(), "sigue en el resultado");
        t.handle(Input::Restart, Duration::ZERO).expect("esc");
        assert!(t.outcome().is_none());
        assert_eq!(t.text(), text);
    }

    #[test]
    fn a_lesson_keeps_the_source_of_its_text() {
        let mut t = lesson_tutor(20, 1);
        assert_eq!(t.missing_keys(), Vec::<char>::new());
        type_lesson(&mut t, 500);
        let source = t.outcome().and_then(|o| o.source.clone()).expect("fuente");
        assert_eq!(source.text, t.text());
        assert!(source.url().contains("oldid="));
    }

    #[test]
    fn warns_about_keys_the_layout_does_not_have() {
        let layout = assets::keyboard("qwerty_us").expect("kbd");
        let foreign = assets::keyboards()
            .into_iter()
            .find(|k| assets::keyboard(k).is_ok_and(|l| !l.contains('a')))
            .expect("un teclado sin alfabeto latino");
        let spec = |slot| ExerciseSpec {
            module: Module::Progression,
            lesson: 1,
            paragraphs: 1,
            language: crate::sections::LESSONS_LANGUAGE.to_owned(),
            slot,
            exam: false,
        };
        let slot = klavaro_core::progression::Slot::new(20, 1);
        let t =
            Tutor::new(spec(slot), layout, Goals::default(), 1, KeyTracker::new()).expect("tutor");
        assert_eq!(t.missing_keys(), Vec::<char>::new());
        let layout = assets::keyboard(&foreign).expect("kbd");
        let t =
            Tutor::new(spec(slot), layout, Goals::default(), 1, KeyTracker::new()).expect("tutor");
        assert!(!t.missing_keys().is_empty(), "{foreign}");
    }

    #[test]
    fn counts_one_mistake_per_typo_and_lists_the_weakest_keys() {
        let mut t = lesson_tutor(20, 1);
        let text: Vec<char> = t.text().chars().collect();
        let at = Duration::ZERO;
        t.handle(Input::Char('#'), at).expect("error");
        t.handle(Input::Char('#'), at).expect("sigue mal");
        assert_eq!(t.weak_keys(), vec![text[0]], "un solo error por racha");
        t.handle(Input::Char(text[2]), at).expect("acierto");
        t.restart().expect("reintento");
        assert_eq!(t.weak_keys(), Vec::<char>::new());
    }

    #[test]
    fn arrows_do_nothing_during_an_exercise() {
        let mut t = lesson_tutor(20, 1);
        let effect = t
            .handle(Input::Move(Direction::Up), Duration::ZERO)
            .expect("flecha");
        assert_eq!(effect, Effect::default());
        assert_eq!(t.progress().0, 0);
    }

    #[test]
    fn translates_keys_and_shortcuts() {
        assert_eq!(translate(None, Some("ñ"), false), Some(Input::Char('ñ')));
        assert_eq!(
            translate(Some(NamedKey::Enter), Some("\r"), false),
            Some(Input::Enter)
        );
        assert_eq!(
            translate(Some(NamedKey::Space), None, false),
            Some(Input::Char(' '))
        );
        assert_eq!(
            translate(Some(NamedKey::Escape), None, false),
            Some(Input::Restart)
        );
        assert_eq!(
            translate(Some(NamedKey::Arrow(Direction::Left)), None, false),
            Some(Input::Move(Direction::Left))
        );
        assert_eq!(translate(None, Some("w"), true), Some(Input::DeleteWord));
        assert_eq!(translate(None, Some("R"), true), Some(Input::Restart));
        assert_eq!(translate(None, Some("x"), true), None);
        assert_eq!(translate(None, None, false), None);
    }

    #[test]
    fn perfect_basic_lesson_advances() {
        let mut t = tutor(Module::Basic);
        let text = t.text();
        let last = text.chars().last().expect("texto");
        let mut finished = 0;
        for (i, c) in text.chars().enumerate() {
            let input = if c == '\n' {
                Input::Enter
            } else {
                Input::Char(c)
            };
            let at = Duration::from_millis(200 * i as u64);
            finished += usize::from(t.handle(input, at).expect("tecla").finished);
        }
        assert_eq!(finished, 1, "termina una sola vez (última: {last:?})");
        let outcome = t.outcome().cloned().expect("terminó");
        assert!(outcome.advance);
        assert!((outcome.stats.accuracy - 100.0).abs() < 1e-9);
        t.handle(Input::Enter, Duration::ZERO).expect("enter");
        assert_eq!(t.spec().lesson, 2);
        assert!(t.outcome().is_none());
        assert_eq!(t.progress().0, 0);
    }

    #[test]
    fn failed_basic_lesson_repeats_and_backspace_goes_back() {
        let mut t = tutor(Module::Basic);
        t.set_lesson(5).expect("lección");
        type_target(&mut t, Some(3));
        let outcome = t.outcome().cloned().expect("terminó");
        assert!(!outcome.advance);
        assert_eq!(outcome.comment, Comment::TooManyErrors);
        t.handle(Input::Backspace, Duration::ZERO).expect("bs");
        assert_eq!(t.spec().lesson, 4);
    }

    #[test]
    fn wrong_keys_beep_and_shortcuts_work() {
        let mut t = tutor(Module::Velocity);
        let effect = t.handle(Input::Char('#'), Duration::ZERO).expect("tecla");
        assert!(effect.beep);
        assert!(
            t.handle(Input::ToggleTheme, Duration::ZERO)
                .expect("tema")
                .toggle_theme
        );
        assert!(
            t.handle(Input::LessonUp, Duration::ZERO)
                .expect("lección")
                .beep
        );
        t.handle(Input::Restart, Duration::ZERO).expect("reinicio");
        assert_eq!(t.progress().0, 0);
    }

    #[test]
    fn segments_cover_the_text_and_show_cursor() {
        let mut t = tutor(Module::Velocity);
        let first = t.session.target()[0];
        t.handle(Input::Char(first), Duration::ZERO).expect("tecla");
        t.handle(Input::Char('#'), Duration::from_millis(100))
            .expect("tecla");
        let segs = t.segments();
        assert_eq!(segs[0].style, SegmentStyle::Correct);
        assert_eq!(segs[1].style, SegmentStyle::Wrong);
        assert_eq!(segs[2].style, SegmentStyle::Cursor);
        let shown: String = segs.iter().map(|s| s.text.as_str()).collect();
        let expected: String = t
            .session
            .target()
            .iter()
            .map(|&c| {
                if c == '\n' {
                    "¶\n".to_owned()
                } else {
                    c.to_string()
                }
            })
            .collect();
        assert_eq!(shown, expected);
    }

    #[test]
    fn tracks_keys_except_in_the_basic_course() {
        let mut basic = tutor(Module::Basic);
        type_target(&mut basic, Some(4));
        assert_eq!(basic.tracker(), &KeyTracker::new());

        let mut velo = tutor(Module::Velocity);
        type_target(&mut velo, Some(4));
        assert!(velo.tracker().error_total() > 0);
        assert!(velo.tracker().times().len() > 1);
    }

    #[test]
    fn special_session_uses_weak_keys() {
        let layout = assets::keyboard("qwerty_es").expect("kbd");
        let mut tracker = KeyTracker::new();
        for _ in 0..80 {
            tracker.wrong(&layout, 'z');
        }
        for c in "qwertyuiop".chars() {
            tracker.wrong(&layout, c);
        }
        let spec = ExerciseSpec {
            module: Module::Adaptability,
            lesson: 1,
            paragraphs: 1,
            language: "es".to_owned(),
            slot: None,
            exam: false,
        };
        let mut t = Tutor::new(spec, layout, Goals::default(), 7, tracker).expect("tutor");
        assert_eq!(t.special_suggested(), Some(SpecialSession::TooManyErrors));
        let plain = t.text().matches('z').count();
        t.toggle_special().expect("especial");
        assert!(t.special());
        assert!(t.text().matches('z').count() > plain);
        assert!(tutor(Module::Velocity).special_suggested().is_none());
    }

    #[test]
    fn dictation_follows_the_cursor_and_the_exercise() {
        let mut t = tutor(Module::Adaptability);
        let first = t.text().chars().next().expect("texto");
        let (at, say) = t.dictation().expect("dictado");
        assert_eq!(at, (0, 0));
        assert_eq!(say, Say::Char(first));
        t.handle(Input::Char(first), Duration::ZERO).expect("tecla");
        assert_eq!(t.dictation().map(|d| d.0), Some((0, 1)));
        t.restart().expect("nuevo");
        assert_eq!(t.dictation().map(|d| d.0), Some((1, 0)));
        assert!(tutor(Module::Fluidity).dictation().is_none());
    }

    fn stroke(ms: u64, key: u64, repeat: bool) -> Stroke {
        Stroke {
            at: Duration::from_millis(ms),
            key: Some(KeyId(key)),
            repeat,
        }
    }

    /// Escribe la lección con teclas físicas: cada letra baja a `ms × i` y se suelta 120 ms
    /// después, así que con `ms` menor a 120 hay rollover.
    fn type_lesson_with_keys(t: &mut Tutor, ms: u64) {
        let text: Vec<char> = t.text().chars().collect();
        for (i, &c) in text.iter().enumerate() {
            let i = i as u64;
            t.handle_stroke(Input::Char(c), stroke(ms * i, i, false))
                .expect("tecla");
            if i > 0 {
                // Se suelta la anterior después de bajar ésta.
                t.key_up(KeyId(i - 1), Duration::from_millis(ms * (i - 1) + 120));
            }
        }
        let last = text.len() as u64 - 1;
        t.key_up(KeyId(last), Duration::from_millis(ms * last + 120));
    }

    #[test]
    fn a_lesson_records_when_each_letter_was_pressed_and_released() {
        let mut t = lesson_tutor(20, 30);
        type_lesson_with_keys(&mut t, 100);
        let timings = t.strokes().expect("registro").timings();
        assert_eq!(timings.len(), t.text().chars().count());
        for (i, timing) in timings.iter().enumerate() {
            let timing = timing.expect("cada letra tiene tiempos");
            let down = Duration::from_millis(100 * i as u64);
            assert_eq!(timing.down, down);
            assert_eq!(timing.up, Some(down + Duration::from_millis(120)));
        }
    }

    #[test]
    fn recording_keys_does_not_change_the_result() {
        let mut plain = lesson_tutor(20, 30);
        type_lesson(&mut plain, 100);
        let mut keyed = lesson_tutor(20, 30);
        type_lesson_with_keys(&mut keyed, 100);
        // Lo único que agrega el registro son las transiciones: lo demás es idéntico.
        let without = |t: &Tutor| {
            t.outcome().cloned().map(|mut o| {
                o.transitions = None;
                o
            })
        };
        assert_eq!(without(&plain), without(&keyed));
        assert!(plain.outcome().is_some());
    }

    #[test]
    fn a_finished_lesson_reports_its_transitions() {
        // A 20 PPM el umbral general es 480 ms: escribir cada 100 ms es fluido.
        let mut t = lesson_tutor(20, 30);
        type_lesson_with_keys(&mut t, 100);
        let letters = u32::try_from(t.text().chars().count()).expect("largo");
        let s = t
            .outcome()
            .and_then(|o| o.transitions)
            .expect("transiciones");
        // Todas medidas, incluida la última letra, que la sesión evalúa antes de anotarla.
        assert_eq!(s.measured, letters - 1);
        assert_eq!(s.fluent, s.measured);
        assert_eq!(s.best_streak, s.measured);
        // Cada tecla se suelta 120 ms después de bajar y la siguiente baja a los 100 ms. En la
        // última transición la tecla anterior todavía no se soltó al terminar: no se sabe.
        assert_eq!(s.rollovers, s.measured - 1);

        // Sin los datos de cada tecla (`handle` a secas) no hay transiciones.
        let mut plain = lesson_tutor(20, 30);
        type_lesson(&mut plain, 100);
        assert_eq!(plain.outcome().map(|o| o.transitions), Some(None));
    }

    #[test]
    fn a_finished_lesson_loads_the_pairs_of_letters() {
        use klavaro_core::pairs::pair_key;
        use klavaro_core::transitions::TransitionKind;

        let mut saved = PairTracker::new();
        saved.record_error('q', 'j', TransitionKind::Unknown);
        let mut t = lesson_tutor(20, 30).with_pairs(saved);
        type_lesson_with_keys(&mut t, 100);
        let text: Vec<char> = t.text().chars().collect();
        let expected = text
            .windows(2)
            .filter(|w| pair_key(w[0], w[1]).is_some())
            .count();
        assert!(expected > 0);
        // Lo guardado antes sigue ahí, y cada par dentro de una palabra sumó una vez.
        assert_eq!(t.pairs().get('q', 'j').map(|e| e.wrong), Some(1));
        let typed: u32 = t.pairs().entries().map(|e| e.typed).sum();
        assert_eq!(typed as usize, expected + 1);
        // Todas las letras se escribieron bien y cada 100 ms.
        let samples: Vec<f64> = t
            .pairs()
            .entries()
            .flat_map(|e| e.samples().to_vec())
            .collect();
        assert_eq!(samples.len(), expected);
        assert!(
            samples.iter().all(|s| (s - 0.1).abs() < 1e-9),
            "{samples:?}"
        );
        assert!(t.pairs().entries().all(|e| e.from != ' ' && e.to != ' '));
        let report = slow_pairs_report(t.pairs(), &PairFrequencies::new());
        assert!(report.starts_with("pares más lentos"), "{report}");
        // Todos los pares tardaron lo mismo: ninguno es débil, salvo el que siempre falla
        // si llegara a las muestras mínimas (acá tiene una sola).
        assert!(report.ends_with("frecuencia\n"), "{report}");

        // Sin los datos de cada tecla (`handle` a secas) no se carga nada.
        let mut plain = lesson_tutor(20, 30);
        type_lesson(&mut plain, 100);
        assert!(plain.pairs().is_empty());
    }

    #[test]
    fn other_modules_do_not_load_pairs() {
        let mut t = tutor(Module::Velocity);
        let text: Vec<char> = t.text().chars().collect();
        for (i, &c) in text.iter().enumerate() {
            let i = i as u64;
            t.handle_stroke(Input::Char(c), stroke(100 * i, i, false))
                .expect("tecla");
        }
        assert!(t.outcome().is_some());
        assert!(t.pairs().is_empty());
    }

    #[test]
    fn other_modules_report_no_transitions() {
        let mut t = tutor(Module::Velocity);
        type_target(&mut t, None);
        assert_eq!(t.outcome().map(|o| o.transitions), Some(None));
    }

    #[test]
    fn key_repeat_still_types_but_is_not_recorded() {
        let mut t = lesson_tutor(20, 30);
        let first = t.text().chars().next().expect("texto");
        t.handle_stroke(Input::Char(first), stroke(0, 1, true))
            .expect("tecla");
        assert_eq!(t.progress().0, 1);
        assert_eq!(t.strokes().expect("registro").timings()[0], None);
        assert!(
            t.strokes()
                .expect("registro")
                .dump(t.session.target(), t.session.states(), None)
                .contains("autorrepetición")
        );
    }

    #[test]
    fn other_modules_do_not_record_keys() {
        for module in [
            Module::Basic,
            Module::Adaptability,
            Module::Velocity,
            Module::Fluidity,
        ] {
            let mut t = tutor(module);
            let first = t.text().chars().next().expect("texto");
            t.handle_stroke(Input::Char(first), stroke(0, 1, false))
                .expect("tecla");
            t.key_up(KeyId(1), Duration::from_millis(50));
            assert!(t.strokes().is_none());
        }
    }

    #[test]
    fn a_new_text_starts_a_new_record() {
        let mut t = lesson_tutor(20, 30);
        let first = t.text().chars().next().expect("texto");
        t.handle_stroke(Input::Char(first), stroke(0, 1, false))
            .expect("tecla");
        t.restart().expect("reinicio");
        let timings = t.strokes().expect("registro").timings();
        assert_eq!(timings.len(), t.text().chars().count());
        assert!(timings.iter().all(Option::is_none));
    }

    /// Escribe las primeras `n` letras con teclas físicas, una cada `ms`, soltando cada una
    /// 60 ms después. Devuelve el instante de la última.
    fn type_first(t: &mut Tutor, n: usize, ms: u64) -> Duration {
        let text: Vec<char> = t.text().chars().take(n).collect();
        let mut at = Duration::ZERO;
        for (i, &c) in text.iter().enumerate() {
            let i = i as u64;
            at = Duration::from_millis(ms * i);
            t.handle_stroke(Input::Char(c), stroke(ms * i, i, false))
                .expect("tecla");
            t.key_up(KeyId(i), at + Duration::from_millis(60));
        }
        at
    }

    fn flashes(t: &Tutor, now: Duration) -> Vec<SegmentStyle> {
        t.segments_at(Some(now))
            .into_iter()
            .map(|s| s.style)
            .filter(|s| matches!(s, SegmentStyle::Flash(_)))
            .collect()
    }

    #[test]
    fn a_fluent_transition_flashes_and_fades() {
        let mut t = lesson_tutor(20, 30);
        let last = type_first(&mut t, 3, 100);
        // Recién escrita, la letra destella con la intensidad máxima.
        assert!(t.flashing(last));
        assert_eq!(
            flashes(&t, last).last(),
            Some(&SegmentStyle::Flash(FLASH_LEVELS))
        );
        // Se va apagando y a los 250 ms ya no está.
        assert_eq!(
            flashes(&t, last + Duration::from_millis(200)).last(),
            Some(&SegmentStyle::Flash(1))
        );
        assert_eq!(flashes(&t, last + FLASH), Vec::new());
        assert!(!t.flashing(last + FLASH));
        // Sin instante (las pruebas de siempre) no hay destellos.
        assert!(
            !t.segments()
                .iter()
                .any(|s| matches!(s.style, SegmentStyle::Flash(_)))
        );
    }

    #[test]
    fn slow_or_wrong_letters_do_not_flash() {
        // A 20 PPM el umbral entre manos es 432 ms y en la misma mano 480 ms.
        let mut slow = lesson_tutor(20, 30);
        let last = type_first(&mut slow, 3, 1000);
        assert!(!slow.flashing(last));

        let mut wrong = lesson_tutor(20, 30);
        type_first(&mut wrong, 2, 100);
        let at = Duration::from_millis(200);
        wrong
            .handle_stroke(Input::Char('#'), stroke(200, 9, false))
            .expect("tecla");
        // Sólo sigue encendida la letra anterior (escrita a los 100 ms); la mal escrita no.
        assert_eq!(flashes(&wrong, at).len(), 1);
        assert_eq!(wrong.streak(), None);
    }

    #[test]
    fn the_streak_shows_from_five_and_backspace_cuts_it() {
        let mut t = lesson_tutor(20, 30);
        type_first(&mut t, 5, 100);
        // Cinco letras son cuatro transiciones: todavía no se muestra.
        assert_eq!(t.streak(), None);
        type_first_from(&mut t, 5, 6, 100);
        assert_eq!(t.streak(), Some(5));
        t.handle_stroke(Input::Backspace, stroke(700, 99, false))
            .expect("tecla");
        assert_eq!(t.streak(), None);
    }

    /// Escribe las letras `from..to` siguiendo el ritmo de [`type_first`].
    fn type_first_from(t: &mut Tutor, from: usize, to: usize, ms: u64) {
        let text: Vec<char> = t.text().chars().collect();
        for (i, &c) in text.iter().enumerate().take(to).skip(from) {
            let at = ms * i as u64;
            t.handle_stroke(Input::Char(c), stroke(at, i as u64, false))
                .expect("tecla");
            t.key_up(KeyId(i as u64), Duration::from_millis(at + 60));
        }
    }

    #[test]
    fn other_modules_never_flash() {
        let mut t = tutor(Module::Velocity);
        let last = type_first(&mut t, 4, 100);
        assert!(!t.flashing(last));
        assert_eq!(t.streak(), None);
    }
}
