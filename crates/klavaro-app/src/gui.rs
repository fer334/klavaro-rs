// SPDX-License-Identifier: GPL-3.0-or-later
//! Interfaz gráfica en iced (arquitectura Elm: estado, mensajes, `update` y `view`).
//!
//! Toda la lógica vive en [`crate::tutor`]; este módulo sólo traduce eventos y dibuja.

mod sections;

use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};
use std::sync::OnceLock;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use iced::advanced::input_method;
use iced::keyboard::{self, key::Named};
use iced::mouse;
use iced::widget::operation::{self, RelativeOffset};
use iced::widget::text::Span;
use iced::widget::{
    Space, button, canvas, checkbox, column, container, pick_list, progress_bar, rich_text, row,
    scrollable, span, stack, text, text_input, tooltip,
};
use iced::{Alignment, Color, Element, Event, Font, Length, Subscription, Task, Theme, event};

use klavaro_core::Module;
use klavaro_core::generators::basic::MAX_LESSONS;
use klavaro_core::keyboard::FingerMap;
use klavaro_core::progression::{self, LESSONS_PER_SECTION, Ladder, Milestone, NextStep, Slot};
use klavaro_core::ranking::RankingEntry;
use klavaro_core::tracker::KeyTracker;
use klavaro_data::catalog::CatalogEntry;
use klavaro_i18n::Translator;
use klavaro_store::LessonBest;
use klavaro_store::date::format_date_time;

use crate::assets;
use crate::audio::{Audio, Engine};
use crate::exercise::{ExerciseSpec, comment_text, module_name, notice_text, recorded_text};
use crate::hints::{finger_text, hint};
use crate::keys::Keys;
use crate::plot::Plot;
use crate::profile::{Profile, Recorded};
use crate::progress::{Chart, ChartQuery, Metric};
use crate::sections::{LESSONS_LANGUAGE, SectionView, SectionsView};
use crate::settings::{GoalField, SettingsForm};
use crate::speech::{Say, spoken};
use crate::strokes::{KeyId, Stroke};
use crate::style;
use crate::tutor::{
    Direction, FLASH_LEVELS, Input, NamedKey, Segment, SegmentStyle, Tutor, translate,
};
use crate::typing::Typing;
use crate::update::{self, Release};

/// Abre la ventana principal.
pub(crate) fn run() -> iced::Result {
    iced::application(App::new, App::update, App::view)
        .title(App::title)
        .subscription(App::subscription)
        .theme(App::theme)
        .window_size((960.0, 680.0))
        .run()
}

/// Idioma de la interfaz en la lista desplegable.
#[derive(Debug, Clone, PartialEq, Eq)]
struct UiLanguage {
    code: &'static str,
    name: &'static str,
}

impl std::fmt::Display for UiLanguage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name)
    }
}

/// Idioma de los textos en la lista desplegable: el código, mostrado con su nombre.
#[derive(Debug, Clone, PartialEq, Eq)]
struct TextLanguage(String);

impl std::fmt::Display for TextLanguage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(klavaro_i18n::native_name(&self.0).unwrap_or(&self.0))
    }
}

fn ui_languages() -> Vec<UiLanguage> {
    klavaro_i18n::LANGUAGES
        .iter()
        .map(|&(code, name)| UiLanguage { code, name })
        .collect()
}

enum Screen {
    Menu,
    Tutor(Box<Tutor>),
    Progress(ProgressView),
    Top10(Vec<RankingEntry>),
    Settings(Box<SettingsForm>),
    /// Progresión: selección de sección.
    Sections(SectionsView),
    /// Progresión: lecciones de una sección.
    Section(SectionView),
    /// Progresión: origen y licencia de los textos.
    Credits(Vec<CatalogEntry>),
    /// Progresión: se aprobó una sección.
    Milestone(Milestone),
}

/// Estado de la pantalla de progreso.
struct ProgressView {
    module: Module,
    metric: Metric,
    /// Lección del curso básico o de la Progresión (0 = todas).
    lesson: u16,
    /// Sección de la Progresión (0 = todas).
    section: u16,
    chart: Chart,
}

/// Pasos para reiniciar la Progresión desde Preferencias.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum ResetStep {
    #[default]
    Idle,
    Confirming,
    Done,
}

// Las banderas (tema, dictado, instalación, puntero) son independientes entre sí.
#[allow(clippy::struct_excessive_bools)]
struct App {
    screen: Screen,
    languages: Vec<String>,
    keyboards: Vec<String>,
    language: String,
    keyboard: String,
    lesson: u16,
    dark: bool,
    error: Option<String>,
    /// Nota sobre el registro del último ejercicio (Top 10, no registrado…).
    note: Option<String>,
    /// Aviso al abrir (importación o problemas con los archivos).
    notice: Option<String>,
    profile: Profile,
    tr: Translator,
    /// Errores y tiempos por tecla del teclado elegido. Durante un ejercicio, el tutor tiene
    /// la copia al día ([`App::leave_tutor`] la recupera).
    tracker: KeyTracker,
    audio: Audio,
    /// Posición del último dictado (ejercicio y cursor), para no repetirlo.
    dictated: Option<(u32, usize)>,
    /// Dedo de cada posición (`fingers_position.txt`).
    fingers: Option<FingerMap>,
    /// Cuándo dictar el dedo (curso básico, 3 s sin teclear) y para qué posición.
    finger_tip: Option<(Instant, (u32, usize))>,
    /// Si ya hay un temporizador en marcha para [`App::finger_tip`].
    tip_waiting: bool,
    /// Versión nueva publicada, si la hay.
    release: Option<Release>,
    /// Si se está descargando e instalando la versión nueva.
    installing: bool,
    /// Progresión: estrellas de cada lección.
    ladder: Ladder,
    /// Progresión: mejores resultados de cada lección.
    bests: BTreeMap<Slot, LessonBest>,
    /// Logro de la última lección, para mostrarlo al seguir.
    milestone: Option<Milestone>,
    /// En qué paso está el botón de reiniciar la Progresión en Preferencias.
    reset: ResetStep,
    /// Si el puntero está oculto porque se está tecleando (vuelve al mover el mouse).
    cursor_hidden: bool,
}

#[derive(Debug, Clone)]
enum Message {
    Start(Module),
    InterfaceSelected(UiLanguage),
    LanguageSelected(String),
    KeyboardSelected(String),
    /// Una tecla traducida, con el instante en que llegó y su tecla física.
    Key(Input, Stroke),
    /// Texto que entrega el método de entrada (letras combinadas con teclas muertas), con el
    /// instante en que llegó.
    Typed(String, Duration),
    /// Se soltó una tecla física.
    KeyUp(KeyId, Duration),
    /// Un cuadro mientras hay un destello encendido: sólo sirve para redibujar.
    FlashTick,
    Back,
    Restart,
    LessonDelta(i32),
    ToggleTheme,
    /// Activa o desactiva la sesión especial de Adaptabilidad.
    ToggleSpecial,
    /// Activa o desactiva el modo dictado.
    ToggleSpeech(bool),
    /// Muestra u oculta el teclado virtual.
    ToggleKeyboard,
    /// Venció la espera para dictar el dedo.
    FingerTip,
    /// Abre el progreso del módulo (o del último usado).
    Progress(Option<Module>),
    ProgressMetric(Metric),
    ProgressLesson(i32),
    Top10,
    Settings,
    SettingsName(String),
    SettingsParagraphs(i32),
    SettingsAutostart(bool),
    SettingsBeep(bool),
    SettingsCheckUpdates(bool),
    SettingsGoal(Module, GoalField, String),
    SettingsDefaults,
    SettingsSave,
    /// Resultado de la búsqueda de una versión nueva.
    ReleaseFound(Option<Release>),
    /// Abre la página de la versión nueva.
    OpenRelease,
    /// Descarga e instala la versión nueva.
    InstallUpdate,
    /// Terminó la instalación (o el error, para mostrarlo).
    UpdateInstalled(Result<(), String>),
    /// Abre la selección de sección de la Progresión.
    OpenSections,
    /// Abre las lecciones de una sección.
    OpenSection(u16),
    /// Empieza una lección.
    StartLesson(Slot),
    /// Empieza el examen de una sección.
    StartExam(u16),
    /// Abre el origen y la licencia de los textos.
    OpenCredits,
    /// Abre una página del navegador (fuente o licencia de un texto).
    OpenLink(String),
    /// Sigue después de un logro.
    MilestoneContinue,
    /// Abre el gráfico de una lección.
    LessonProgress(Slot),
    /// Abre una sesión especial de Adaptabilidad con las teclas débiles.
    PracticeWeakKeys,
    /// Pide confirmar el reinicio de la Progresión.
    ResetAsk,
    /// Confirma el reinicio.
    ResetConfirm,
    /// Cancela el reinicio.
    ResetCancel,
    /// Cambia la sección del gráfico de la Progresión.
    ProgressSection(i32),
    /// Se movió o se usó el mouse: vuelve a mostrar el puntero.
    MouseMoved,
}

impl App {
    fn new() -> (Self, Task<Message>) {
        let profile = Profile::open();
        let config = &profile.config;
        let languages = assets::languages();
        let keyboards = assets::keyboards();
        let pick = |list: &[String], wanted: &str, fallback: &str| {
            [wanted, fallback]
                .into_iter()
                .find(|w| list.iter().any(|x| x == w))
                .map(str::to_owned)
                .or_else(|| list.first().cloned())
                .unwrap_or_default()
        };
        let system = klavaro_i18n::system_preferences();
        let ui = klavaro_i18n::resolve(config.language.iter().chain(&system).map(String::as_str));
        let tr = Translator::new(ui);
        let text_language = config
            .text_language
            .clone()
            .unwrap_or_else(|| ui.to_owned());
        let error = keyboards.is_empty().then(|| {
            let dir = assets::location();
            tr.tr_args("error-no-data", &[("dir", &dir)])
        });
        let mut app = Self {
            language: pick(&languages, &text_language, "es"),
            keyboard: pick(
                &keyboards,
                config.keyboard.as_deref().unwrap_or("qwerty_es"),
                "qwerty_us",
            ),
            languages,
            keyboards,
            lesson: config.basic_lesson,
            dark: config.alt_colors,
            error,
            note: None,
            notice: profile.notice.as_ref().map(|n| notice_text(&tr, n)),
            tr,
            screen: Screen::Menu,
            profile,
            tracker: KeyTracker::new(),
            audio: Audio::new(Engine::detect()),
            dictated: None,
            fingers: assets::fingers().ok(),
            finger_tip: None,
            tip_waiting: false,
            release: None,
            installing: false,
            ladder: Ladder::default(),
            bests: BTreeMap::new(),
            milestone: None,
            reset: ResetStep::Idle,
            cursor_hidden: false,
        };
        app.load_tracker();
        app.load_lessons();
        if app.profile.config.autostart
            && let Some(m) = app.profile.config.last_module
            && let Err(e) = app.start(m)
        {
            app.error = Some(e.to_string());
        }
        let check = if app.profile.config.check_updates && update::available() {
            check_release()
        } else {
            Task::none()
        };
        (app, check)
    }

    /// Idiomas de los textos para las listas desplegables.
    fn text_languages(&self) -> Vec<TextLanguage> {
        self.languages.iter().cloned().map(TextLanguage).collect()
    }

    /// Copia el estado de la pantalla a la configuración y la guarda.
    fn persist(&mut self) {
        let c = &mut self.profile.config;
        c.text_language = Some(self.language.clone());
        c.keyboard = Some(self.keyboard.clone());
        c.basic_lesson = self.lesson;
        c.alt_colors = self.dark;
        if let Err(e) = self.profile.save_config() {
            self.error = Some(e.to_string());
        }
    }

    /// Lee el rastreo por tecla del teclado elegido.
    fn load_tracker(&mut self) {
        self.tracker = self.profile.tracker(&self.keyboard).unwrap_or_else(|e| {
            self.error = Some(e.to_string());
            KeyTracker::new()
        });
    }

    /// Lee los resultados de la Progresión.
    fn load_lessons(&mut self) {
        let bests = self
            .profile
            .lesson_bests(LESSONS_LANGUAGE)
            .unwrap_or_else(|e| {
                self.error = Some(e.to_string());
                Vec::new()
            });
        let exam = self
            .profile
            .exam_section(LESSONS_LANGUAGE)
            .unwrap_or_else(|e| {
                self.error = Some(e.to_string());
                0
            });
        self.ladder = crate::sections::ladder(&bests, exam);
        self.bests = bests.into_iter().map(|b| (b.slot, b)).collect();
    }

    /// Abre la selección de sección.
    fn open_sections(&mut self) {
        self.leave_tutor();
        self.screen = Screen::Sections(SectionsView::new(&self.ladder));
    }

    /// Abre las lecciones de una sección. Una bloqueada también se puede ver: desde ahí se rinde
    /// su examen.
    fn open_section(&mut self, section: u16) {
        self.leave_tutor();
        self.screen = Screen::Section(SectionView::new(section, &self.ladder));
    }

    /// Empieza el examen de una sección: su última lección, sin pedir las anteriores.
    fn start_exam(&mut self, section: u16) -> std::io::Result<()> {
        if !self.ladder.exam_available(section) {
            return Ok(());
        }
        let spec = ExerciseSpec {
            module: Module::Progression,
            lesson: self.lesson,
            paragraphs: 0,
            language: LESSONS_LANGUAGE.to_owned(),
            slot: Ladder::exam_slot(section),
            exam: true,
        };
        self.start_spec(spec)
    }

    /// Empieza una lección, si está abierta.
    fn start_lesson(&mut self, slot: Slot) -> std::io::Result<()> {
        if !self.ladder.lesson_unlocked(slot) {
            return Ok(());
        }
        let spec = ExerciseSpec {
            module: Module::Progression,
            lesson: self.lesson,
            paragraphs: 0,
            language: LESSONS_LANGUAGE.to_owned(),
            slot: Some(slot),
            exam: false,
        };
        self.start_spec(spec)
    }

    /// Enter al terminar una lección: pasa a la siguiente si está abierta y si no, la repite.
    fn next_lesson(&mut self) -> std::io::Result<()> {
        if let Some(milestone) = self.milestone.take() {
            self.note = None;
            self.leave_tutor();
            self.screen = Screen::Milestone(milestone);
            return Ok(());
        }
        let Screen::Tutor(t) = &mut self.screen else {
            return Ok(());
        };
        let Some(slot) = t.spec().slot else {
            return Ok(());
        };
        self.note = None;
        match self.ladder.next_step(slot) {
            NextStep::Open(next) => self.start_lesson(next),
            NextStep::Completed => {
                self.open_section(slot.section());
                Ok(())
            }
            NextStep::NeedsPassStars | NextStep::NeedsSection => t.restart(),
        }
    }

    /// Sigue después de un logro: a la sección que abrió o, al final, a la lista de secciones.
    fn milestone_continue(&mut self, milestone: Milestone) -> std::io::Result<()> {
        match milestone {
            Milestone::SectionPassed(section) => {
                if let Some(next) = Slot::new(section + progression::SECTION_STEP, 1) {
                    return self.start_lesson(next);
                }
            }
            Milestone::Completed => self.open_sections(),
        }
        Ok(())
    }

    /// Vuelve a la pantalla anterior.
    fn back(&mut self) {
        let section = match &self.screen {
            Screen::Tutor(t) => t.spec().slot.map(Slot::section),
            Screen::Progress(v) if v.module == Module::Progression && v.section != 0 => {
                Some(v.section)
            }
            _ => None,
        };
        let in_section = matches!(
            self.screen,
            Screen::Section(_) | Screen::Credits(_) | Screen::Milestone(_)
        );
        self.milestone = None;
        self.reset = ResetStep::Idle;
        self.leave_tutor();
        if let Some(section) = section {
            self.open_section(section);
        } else if in_section {
            self.open_sections();
        } else {
            self.screen = Screen::Menu;
        }
    }

    /// Teclas en las pantallas de la Progresión.
    fn sections_key(&mut self, input: Input) -> std::io::Result<()> {
        match input {
            Input::Restart => self.back(),
            Input::ToggleTheme => {
                self.dark = !self.dark;
                self.persist();
            }
            Input::Move(direction) => match &mut self.screen {
                Screen::Sections(v) => v.step(direction),
                Screen::Section(v) => v.step(direction),
                _ => {}
            },
            Input::Char('e' | 'E') => {
                if let Screen::Section(v) = self.screen {
                    return self.start_exam(v.section);
                }
            }
            Input::Enter => match self.screen {
                Screen::Milestone(milestone) => return self.milestone_continue(milestone),
                Screen::Sections(v) => self.open_section(v.cursor),
                Screen::Section(v) => {
                    if let Some(slot) = v.slot() {
                        return self.start_lesson(slot);
                    }
                }
                _ => {}
            },
            _ => {}
        }
        Ok(())
    }

    /// Sale de la pantalla del tutor (si está en ella): recupera y guarda el rastreo.
    fn leave_tutor(&mut self) {
        if let Screen::Tutor(t) = std::mem::replace(&mut self.screen, Screen::Menu) {
            self.tracker = t.into_tracker();
            if let Err(e) = self.profile.save_tracker(&self.keyboard, &self.tracker) {
                self.error = Some(e.to_string());
            }
        }
    }

    /// Registra el ejercicio recién terminado.
    fn record(&mut self) {
        let Screen::Tutor(t) = &self.screen else {
            return;
        };
        let Some(outcome) = t.outcome() else {
            return;
        };
        // Hay que leerlo antes de guardar: después, el intento anterior sería éste.
        let previous = t
            .spec()
            .slot
            .filter(|_| outcome.transitions.is_some_and(|s| s.measured > 0))
            .and_then(|slot| self.profile.previous_transitions(&t.spec().language, slot));
        let model = if t.spec().module == klavaro_core::Module::Fluidity {
            assets::paragraphs(&t.spec().language)
                .map(|p| p.join(" "))
                .unwrap_or_default()
        } else {
            String::new()
        };
        match self
            .profile
            .record(t.spec(), &self.keyboard, outcome, &t.text(), &model)
        {
            Ok(r) => {
                self.note = recorded_text(&self.tr, r);
                if let Recorded::Lesson {
                    gained, milestone, ..
                } = r
                {
                    self.milestone = milestone;
                    if gained > 0 && self.profile.config.beep {
                        self.audio.beep();
                    }
                }
            }
            Err(e) => self.error = Some(e.to_string()),
        }
        if let Err(e) = self.profile.save_tracker(&self.keyboard, t.tracker()) {
            self.error = Some(e.to_string());
        }
        let progression = t.spec().module == Module::Progression;
        if let Screen::Tutor(t) = &mut self.screen {
            t.set_previous_transitions(previous);
        }
        if progression {
            self.load_lessons();
        }
    }

    fn title(&self) -> String {
        match &self.screen {
            Screen::Menu => "klavaro-rs".to_owned(),
            Screen::Tutor(t) => {
                format!("klavaro-rs · {}", module_name(&self.tr, t.spec().module))
            }
            Screen::Sections(_)
            | Screen::Section(_)
            | Screen::Credits(_)
            | Screen::Milestone(_) => {
                format!(
                    "klavaro-rs · {}",
                    module_name(&self.tr, Module::Progression)
                )
            }
            Screen::Progress(_) => format!("klavaro-rs · {}", self.tr.tr("menu-progress")),
            Screen::Top10(_) => format!("klavaro-rs · {}", self.tr.tr("menu-top10")),
            Screen::Settings(_) => format!("klavaro-rs · {}", self.tr.tr("menu-settings")),
        }
    }

    fn theme(&self) -> Theme {
        style::theme(self.dark)
    }

    fn subscription(&self) -> Subscription<Message> {
        let mut subs = vec![event::listen_with(on_event)];
        // El mouse sólo se escucha mientras el puntero está oculto, para no redibujar con
        // cada movimiento.
        if self.cursor_hidden {
            subs.push(event::listen_with(on_mouse));
        }
        // Mientras un destello se apaga hay que redibujar en cada cuadro; sin destellos, nada.
        if let Screen::Tutor(t) = &self.screen
            && t.flashing(now())
        {
            subs.push(iced::window::frames().map(|_| Message::FlashTick));
        }
        Subscription::batch(subs)
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        let message = match self.early(message) {
            Ok(m) => m,
            Err(task) => return task,
        };
        let result = match message {
            Message::Start(module) => self.start(module),
            m @ (Message::InterfaceSelected(_)
            | Message::LanguageSelected(_)
            | Message::KeyboardSelected(_)
            | Message::ToggleTheme) => {
                self.update_choices(m);
                Ok(())
            }
            Message::Back => {
                self.back();
                Ok(())
            }
            m @ (Message::OpenSections
            | Message::OpenSection(_)
            | Message::StartLesson(_)
            | Message::StartExam(_)
            | Message::OpenCredits
            | Message::OpenLink(_)
            | Message::MilestoneContinue
            | Message::LessonProgress(_)
            | Message::PracticeWeakKeys
            | Message::ResetAsk
            | Message::ResetConfirm
            | Message::ResetCancel) => self.update_sections(&m),
            Message::ToggleSpecial | Message::Restart | Message::LessonDelta(_) => {
                self.update_tutor(&message)
            }
            m @ (Message::Progress(_)
            | Message::ProgressMetric(_)
            | Message::ProgressLesson(_)
            | Message::ProgressSection(_)) => {
                self.update_progress(&m);
                Ok(())
            }
            Message::Top10 => {
                self.leave_tutor();
                self.show_top10();
                Ok(())
            }
            Message::Settings => {
                self.leave_tutor();
                self.error = None;
                self.reset = ResetStep::Idle;
                self.screen = Screen::Settings(Box::new(SettingsForm::new(&self.profile.config)));
                Ok(())
            }
            m @ (Message::SettingsName(_)
            | Message::SettingsParagraphs(_)
            | Message::SettingsAutostart(_)
            | Message::SettingsBeep(_)
            | Message::SettingsCheckUpdates(_)
            | Message::ToggleSpeech(_)
            | Message::ToggleKeyboard
            | Message::SettingsGoal(..)
            | Message::SettingsDefaults
            | Message::SettingsSave) => {
                self.update_settings(m);
                Ok(())
            }
            Message::ReleaseFound(release) => {
                self.release = release;
                Ok(())
            }
            Message::OpenRelease => {
                if let Some(r) = &self.release {
                    update::open_in_browser(&r.url);
                }
                Ok(())
            }
            m @ (Message::Key(..)
            | Message::Typed(..)
            | Message::KeyUp(..)
            | Message::MouseMoved) => self.update_input(m),
            // Se atienden al principio.
            Message::FlashTick
            | Message::FingerTip
            | Message::InstallUpdate
            | Message::UpdateInstalled(_) => Ok(()),
        };
        if let Err(e) = result {
            self.error = Some(e.to_string());
        }
        // El aviso de la importación (o de preferencias guardadas) es para el menú: al salir
        // de él ya se vio.
        if !matches!(self.screen, Screen::Menu) {
            self.notice = None;
        }
        let tip = match self.dictate() {
            Some(at) if self.basic_tutor() => self.schedule_tip(at),
            _ => {
                self.finger_tip = None;
                Task::none()
            }
        };
        Task::batch([tip, self.follow_cursor(), self.follow_grid()])
    }

    /// Mensajes que devuelven una tarea propia, o ninguna (`FlashTick` sólo pide redibujar):
    /// se atienden antes que el resto. Los demás vuelven en `Ok`.
    fn early(&mut self, message: Message) -> Result<Message, Task<Message>> {
        match message {
            Message::FlashTick => Err(Task::none()),
            Message::FingerTip => Err(self.finger_tip()),
            m @ (Message::InstallUpdate | Message::UpdateInstalled(_)) => {
                Err(self.install_update(m))
            }
            m => Ok(m),
        }
    }

    /// Opciones del menú: idiomas, teclado y colores.
    fn update_choices(&mut self, message: Message) {
        match message {
            Message::InterfaceSelected(l) => {
                self.tr = Translator::new(l.code);
                self.profile.config.language = Some(l.code.to_owned());
                self.persist();
            }
            Message::LanguageSelected(l) => {
                self.language = l;
                self.persist();
                if matches!(self.screen, Screen::Top10(_)) {
                    self.show_top10();
                }
            }
            Message::KeyboardSelected(k) => {
                self.leave_tutor();
                self.keyboard = k;
                self.persist();
                self.load_tracker();
            }
            Message::ToggleTheme => {
                self.dark = !self.dark;
                self.persist();
            }
            _ => {}
        }
    }

    /// Mensajes de las pantallas de la Progresión.
    fn update_sections(&mut self, message: &Message) -> std::io::Result<()> {
        match message {
            Message::OpenSections => self.open_sections(),
            Message::OpenSection(section) => self.open_section(*section),
            Message::StartLesson(slot) => return self.start_lesson(*slot),
            Message::StartExam(section) => return self.start_exam(*section),
            Message::OpenCredits => {
                self.screen = Screen::Credits(assets::lessons_catalog(LESSONS_LANGUAGE)?);
            }
            Message::OpenLink(url) => update::open_in_browser(url),
            Message::MilestoneContinue => {
                if let Screen::Milestone(milestone) = self.screen {
                    return self.milestone_continue(milestone);
                }
            }
            Message::LessonProgress(slot) => {
                self.leave_tutor();
                self.show_progress(
                    Module::Progression,
                    Metric::Speed,
                    slot.section(),
                    slot.lesson(),
                );
            }
            Message::PracticeWeakKeys => {
                self.start(Module::Adaptability)?;
                if let Screen::Tutor(t) = &mut self.screen {
                    t.toggle_special()?;
                }
            }
            Message::ResetAsk => self.reset = ResetStep::Confirming,
            Message::ResetCancel => self.reset = ResetStep::Idle,
            Message::ResetConfirm => {
                self.reset = ResetStep::Done;
                if let Err(e) = self.profile.reset_lessons(LESSONS_LANGUAGE) {
                    self.reset = ResetStep::Idle;
                    self.error = Some(e.to_string());
                }
                self.load_lessons();
            }
            _ => {}
        }
        Ok(())
    }

    /// Mensajes de los botones del ejercicio (fuera de él no hacen nada).
    fn update_tutor(&mut self, message: &Message) -> std::io::Result<()> {
        let Screen::Tutor(t) = &mut self.screen else {
            return Ok(());
        };
        self.note = None;
        match *message {
            Message::ToggleSpecial => t.toggle_special(),
            Message::Restart => t.restart(),
            Message::LessonDelta(delta) => {
                let lesson = i32::from(t.spec().lesson) + delta;
                let r = t.set_lesson(u16::try_from(lesson.max(1)).unwrap_or(1));
                self.sync_lesson();
                r
            }
            _ => Ok(()),
        }
    }

    /// Mensajes de la pantalla de progreso.
    fn update_progress(&mut self, message: &Message) {
        match *message {
            Message::Progress(module) => {
                let module = module
                    .or(self.profile.config.last_module)
                    .unwrap_or(Module::Basic);
                // Al cambiar de módulo se mantiene el gráfico elegido si existe en el nuevo.
                let metric = match &self.screen {
                    Screen::Progress(v) => v.metric,
                    _ => Metric::Accuracy,
                };
                self.leave_tutor();
                let lesson = if module == Module::Progression {
                    0
                } else {
                    self.lesson
                };
                self.show_progress(module, metric, 0, lesson);
            }
            Message::ProgressMetric(metric) => {
                if let Screen::Progress(v) = &self.screen {
                    self.show_progress(v.module, metric, v.section, v.lesson);
                }
            }
            Message::ProgressLesson(delta) => {
                if let Screen::Progress(v) = &self.screen {
                    let top = match v.module {
                        Module::Progression if v.section == 0 => 0,
                        Module::Progression => LESSONS_PER_SECTION,
                        _ => MAX_LESSONS,
                    };
                    let lesson = (i32::from(v.lesson) + delta).clamp(0, i32::from(top));
                    let lesson = u16::try_from(lesson).unwrap_or(0);
                    self.show_progress(v.module, v.metric, v.section, lesson);
                }
            }
            Message::ProgressSection(delta) => {
                if let Screen::Progress(v) = &self.screen {
                    let section = crate::sections::stepped_section(v.section, delta);
                    let lesson = if section == 0 { 0 } else { v.lesson };
                    self.show_progress(v.module, v.metric, section, lesson);
                }
            }
            _ => {}
        }
    }

    /// Dicta lo que toca teclear si cambió desde el último dictado (modo dictado).
    fn dictate(&mut self) -> Option<(u32, usize)> {
        let next = match &self.screen {
            Screen::Tutor(t) if self.profile.config.speech => t.dictation(),
            _ => None,
        };
        let Some((at, say)) = next else {
            if self.dictated.take().is_some() {
                self.audio.hush();
            }
            return None;
        };
        if self.dictated == Some(at) {
            return None;
        }
        self.dictated = Some(at);
        // El original usa las dos primeras letras del código del idioma de la interfaz.
        let voice: String = self.tr.tr("speech-voice").chars().take(2).collect();
        let punct = matches!(say, Say::Char(_))
            || matches!(&self.screen, Screen::Tutor(t) if t.spec().module == Module::Velocity);
        self.audio.say(&voice, &spoken(&self.tr, &say), punct);
        Some(at)
    }

    /// Desplaza el texto del ejercicio para que el cursor quede a la vista. La posición es
    /// proporcional al avance: alcanza también para párrafos que se parten en varias líneas.
    fn follow_cursor(&self) -> Task<Message> {
        let Screen::Tutor(t) = &self.screen else {
            return Task::none();
        };
        let (done, total) = t.progress();
        #[allow(clippy::cast_precision_loss)]
        let y = done as f32 / total.max(1) as f32;
        operation::snap_to(EXERCISE_SCROLL, RelativeOffset { x: 0.0, y })
    }

    /// Si se está en un ejercicio del curso básico.
    fn basic_tutor(&self) -> bool {
        matches!(&self.screen, Screen::Tutor(t) if t.spec().module == Module::Basic)
    }

    /// Programa el dictado del dedo para dentro de 3 s, como `tutor_delayed_finger_tip`.
    /// Hay a lo sumo un temporizador: si ya hay uno, al vencer vuelve a esperar lo que falte.
    fn schedule_tip(&mut self, at: (u32, usize)) -> Task<Message> {
        self.finger_tip = Some((Instant::now() + FINGER_TIP_DELAY, at));
        if self.tip_waiting {
            return Task::none();
        }
        self.tip_waiting = true;
        wake_after(FINGER_TIP_DELAY)
    }

    /// Dicta el dedo si pasaron 3 s en la misma posición.
    fn finger_tip(&mut self) -> Task<Message> {
        self.tip_waiting = false;
        let Some((when, at)) = self.finger_tip else {
            return Task::none();
        };
        let now = Instant::now();
        if now < when {
            self.tip_waiting = true;
            return wake_after(when - now);
        }
        self.finger_tip = None;
        if self.dictated != Some(at) || !self.profile.config.speech {
            return Task::none();
        }
        let finger = match (&self.screen, &self.fingers) {
            (Screen::Tutor(t), Some(fingers)) => t
                .next_char()
                .and_then(|c| hint(t.layout(), fingers, c))
                .and_then(|h| h.finger),
            _ => None,
        };
        if let Some(f) = finger {
            let voice: String = self.tr.tr("speech-voice").chars().take(2).collect();
            self.audio.say(&voice, &finger_text(&self.tr, f), false);
        }
        Task::none()
    }

    /// Mensajes del formulario de preferencias (y el modo dictado del menú).
    fn update_settings(&mut self, message: Message) {
        match message {
            Message::SettingsSave => {
                self.save_settings();
                return;
            }
            Message::ToggleSpeech(on) => {
                self.profile.config.speech = on;
                self.persist();
                return;
            }
            Message::ToggleKeyboard => {
                self.profile.config.show_keyboard = !self.profile.config.show_keyboard;
                self.persist();
                return;
            }
            _ => {}
        }
        let Screen::Settings(f) = &mut self.screen else {
            return;
        };
        match message {
            Message::SettingsName(name) => f.name = name,
            Message::SettingsParagraphs(delta) => f.add_paragraphs(delta),
            Message::SettingsAutostart(on) => f.autostart = on,
            Message::SettingsBeep(on) => f.beep = on,
            Message::SettingsCheckUpdates(on) => f.check_updates = on,
            Message::SettingsGoal(module, field, value) => f.set_goal(module, field, value),
            Message::SettingsDefaults => f.default_goals(),
            _ => {}
        }
    }

    /// Aplica el formulario de preferencias y vuelve al menú, o marca los campos inválidos.
    fn save_settings(&mut self) {
        let Screen::Settings(form) = &self.screen else {
            return;
        };
        match form.apply(&self.profile.config) {
            Ok(config) => {
                self.profile.config = config;
                self.persist();
                self.screen = Screen::Menu;
                if self.error.is_none() {
                    self.notice = Some(self.tr.tr("settings-saved"));
                }
            }
            Err(_) => self.error = Some(self.tr.tr("settings-invalid")),
        }
    }

    fn start(&mut self, module: Module) -> std::io::Result<()> {
        let config = &self.profile.config;
        let spec = ExerciseSpec {
            module,
            lesson: self.lesson,
            paragraphs: usize::from(config.fluid_paragraphs),
            language: self.language.clone(),
            slot: None,
            exam: false,
        };
        self.start_spec(spec)?;
        self.profile.config.last_module = Some(module);
        self.persist();
        Ok(())
    }

    /// Abre la pantalla del tutor con un ejercicio.
    fn start_spec(&mut self, spec: ExerciseSpec) -> std::io::Result<()> {
        let layout = assets::keyboard(&self.keyboard)?;
        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(1, |d| d.as_secs());
        self.leave_tutor();
        let config = &self.profile.config;
        let tutor = Tutor::new(
            spec,
            layout,
            config.goals.clone(),
            seed,
            self.tracker.clone(),
        )?;
        self.screen = Screen::Tutor(Box::new(tutor));
        self.error = None;
        self.note = None;
        self.notice = None;
        Ok(())
    }

    /// Guarda la lección del curso básico si cambió.
    fn sync_lesson(&mut self) {
        if let Screen::Tutor(t) = &self.screen
            && t.spec().module == Module::Basic
            && t.spec().lesson != self.lesson
        {
            self.lesson = t.spec().lesson;
            self.persist();
        }
    }

    /// Abre (o actualiza) el gráfico de progreso.
    fn show_progress(&mut self, module: Module, metric: Metric, section: u16, lesson: u16) {
        // Cada módulo tiene sus gráficos: si no existe el pedido, se muestra el primero.
        // Fluidez y puntaje sólo existen en el módulo de Fluidez.
        let metric = if Metric::for_module(module).contains(&metric) {
            metric
        } else {
            Metric::for_module(module)
                .first()
                .copied()
                .unwrap_or(Metric::Accuracy)
        };
        let query = ChartQuery {
            module,
            metric,
            lesson,
            section,
            keyboard: self.keyboard.clone(),
            language: if module == Module::Progression {
                LESSONS_LANGUAGE.to_owned()
            } else {
                self.language.clone()
            },
        };
        let chart = if metric == Metric::Stars {
            Ok(Chart::new(metric, Vec::new(), None))
        } else if metric.per_key() {
            Ok(Chart::per_key(metric, &self.tracker))
        } else {
            self.profile.chart(&query)
        };
        let chart = chart.unwrap_or_else(|e| {
            self.error = Some(e.to_string());
            Chart::new(metric, Vec::new(), None)
        });
        self.screen = Screen::Progress(ProgressView {
            module,
            metric,
            lesson,
            section,
            chart,
        });
    }

    /// Abre (o actualiza) el Top 10 del idioma de los textos.
    fn show_top10(&mut self) {
        let entries = match self.profile.ranking(&self.language) {
            Ok(r) => r.entries().to_vec(),
            Err(e) => {
                self.error = Some(e.to_string());
                Vec::new()
            }
        };
        self.screen = Screen::Top10(entries);
    }

    fn key(&mut self, input: Input, stroke: Stroke) -> std::io::Result<()> {
        match &mut self.screen {
            Screen::Tutor(t) => {
                let was_done = t.outcome().is_some();
                let effect = t.handle_stroke(input, stroke)?;
                if effect.beep && self.profile.config.beep {
                    self.audio.beep();
                }
                if was_done && t.outcome().is_none() {
                    self.note = None;
                    self.milestone = None;
                }
                if effect.finished {
                    self.record();
                }
                self.sync_lesson();
                if effect.toggle_theme {
                    self.dark = !self.dark;
                    self.persist();
                }
                if effect.next_lesson {
                    return self.next_lesson();
                }
                Ok(())
            }
            Screen::Sections(_)
            | Screen::Section(_)
            | Screen::Credits(_)
            | Screen::Milestone(_) => self.sections_key(input),
            Screen::Progress(_) | Screen::Top10(_) | Screen::Settings(_) => {
                match input {
                    Input::Restart => self.back(),
                    Input::ToggleTheme => {
                        self.dark = !self.dark;
                        self.persist();
                    }
                    _ => {}
                }
                Ok(())
            }
            Screen::Menu => {
                // En el menú, 1–4 abren cada módulo, P el progreso y T el Top 10.
                match input {
                    Input::Char('p' | 'P') => {
                        let module = self.profile.config.last_module.unwrap_or(Module::Basic);
                        self.show_progress(module, Metric::Accuracy, 0, self.lesson);
                        return Ok(());
                    }
                    Input::Char('n' | 'N') => {
                        self.open_sections();
                        return Ok(());
                    }
                    Input::Char('t' | 'T') => {
                        self.show_top10();
                        return Ok(());
                    }
                    Input::Char('s' | 'S') => {
                        self.reset = ResetStep::Idle;
                        self.screen =
                            Screen::Settings(Box::new(SettingsForm::new(&self.profile.config)));
                        return Ok(());
                    }
                    _ => {}
                }
                if let Input::Char(c) = input {
                    let index = c.to_digit(10).and_then(|d| usize::try_from(d).ok());
                    if let Some(&m) = index.and_then(|d| Module::ALL.get(d.wrapping_sub(1))) {
                        return self.start(m);
                    }
                }
                if input == Input::ToggleTheme {
                    self.dark = !self.dark;
                    self.persist();
                }
                Ok(())
            }
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let body = match &self.screen {
            Screen::Menu => self.view_menu(),
            Screen::Tutor(t) => self.view_tutor(t),
            Screen::Progress(v) => self.view_progress(v),
            Screen::Top10(entries) => self.view_top10(entries),
            Screen::Settings(form) => self.view_settings(form),
            Screen::Sections(v) => self.view_sections(v),
            Screen::Section(v) => self.view_section(v),
            Screen::Credits(entries) => self.view_credits(entries),
            Screen::Milestone(milestone) => self.view_milestone(*milestone),
        };
        let mut page = column![].spacing(16);
        if let Some(e) = &self.error {
            page = page.push(banner(e.clone(), style::error_banner));
        }
        if let (Screen::Menu, Some(n)) = (&self.screen, &self.notice) {
            page = page.push(banner(n.clone(), style::info_banner));
        }
        if let (Screen::Menu, Some(r)) = (&self.screen, &self.release) {
            page = page.push(self.release_banner(r));
        }
        let page = container(page.push(body))
            .padding(24)
            .width(Length::Fill)
            .height(Length::Fill);
        if self.cursor_hidden && matches!(self.screen, Screen::Tutor(_)) {
            // Una capa vacía encima de todo pide ocultar el puntero, aunque esté sobre un botón.
            stack![
                page,
                iced::widget::mouse_area(Space::new().width(Length::Fill).height(Length::Fill))
                    .interaction(mouse::Interaction::Hidden),
            ]
            .into()
        } else {
            page.into()
        }
    }

    /// Teclas y mouse: al teclear en un ejercicio se oculta el puntero; al mover el mouse vuelve.
    fn update_input(&mut self, message: Message) -> std::io::Result<()> {
        if matches!(message, Message::MouseMoved) {
            self.cursor_hidden = false;
            return Ok(());
        }
        if matches!(self.screen, Screen::Tutor(_)) {
            self.cursor_hidden = true;
        }
        match message {
            Message::Key(input, stroke) => self.key(input, stroke),
            Message::Typed(text, at) => text
                .chars()
                .try_for_each(|c| self.key(Input::Char(c), Stroke::bare(at))),
            Message::KeyUp(key, at) => {
                if let Screen::Tutor(t) = &mut self.screen {
                    t.key_up(key, at);
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }

    /// Instala la versión nueva en un hilo aparte y, al terminar, abre la nueva y cierra esta.
    fn install_update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::InstallUpdate if !self.installing => {
                self.installing = true;
                self.error = None;
                let (tx, rx) = iced::futures::channel::oneshot::channel();
                std::thread::spawn(move || {
                    let _ = tx.send(update::install());
                });
                Task::perform(rx, |r| {
                    Message::UpdateInstalled(r.unwrap_or_else(|_| Err("cancelado".to_owned())))
                })
            }
            Message::UpdateInstalled(Ok(())) => {
                update::relaunch();
                iced::exit()
            }
            Message::UpdateInstalled(Err(e)) => {
                self.installing = false;
                self.error = Some(self.tr.tr_args("update-failed", &[("error", &e)]));
                Task::none()
            }
            _ => Task::none(),
        }
    }

    /// Aviso de versión nueva, con el botón para instalarla (si esta copia puede) y el de ir a
    /// descargarla.
    fn release_banner<'a>(&self, release: &Release) -> Element<'a, Message> {
        let tr = &self.tr;
        let message = if self.installing {
            tr.tr_args("update-installing", &[("version", &release.version)])
        } else {
            tr.tr_args(
                "update-available",
                &[("version", &release.version), ("current", update::CURRENT)],
            )
        };
        let can_install = update::can_install();
        let download = button(text(tr.tr("update-download")).size(14))
            .style(if can_install {
                style::secondary
            } else {
                style::primary
            })
            .padding([6, 14])
            .on_press_maybe((!self.installing).then_some(Message::OpenRelease));
        let install = can_install.then(|| {
            button(text(tr.tr("update-install")).size(14))
                .style(style::primary)
                .padding([6, 14])
                .on_press_maybe((!self.installing).then_some(Message::InstallUpdate))
        });
        container(
            row![text(message).size(14).width(Length::Fill)]
                .push(install)
                .push(download)
                .spacing(12)
                .align_y(Alignment::Center),
        )
        .padding([8, 14])
        .width(Length::Fill)
        .style(style::info_banner)
        .into()
    }

    fn view_menu<'a>(&'a self) -> Element<'a, Message> {
        let tr = &self.tr;
        let module_card = |i: usize, m: Module| -> Element<'a, Message> {
            let tip = format!("{}-tip", crate::exercise::module_key(m));
            let number = container(text((i + 1).to_string()).size(18).font(style::BOLD))
                .center(Length::Fixed(40.0))
                .style(style::badge);
            button(
                row![
                    number,
                    column![
                        text(module_name(tr, m)).size(18).font(style::BOLD),
                        text(tr.tr(&tip)).size(13).style(style::muted),
                    ]
                    .spacing(2),
                ]
                .spacing(14)
                .align_y(Alignment::Center),
            )
            .width(Length::Fill)
            .padding(16)
            .style(style::module_card)
            .on_press(Message::Start(m))
            .into()
        };
        let modules = column(Module::ALL.chunks(2).enumerate().map(|(r, pair)| {
            row(pair
                .iter()
                .enumerate()
                .map(|(c, &m)| module_card(r * 2 + c, m)))
            .spacing(12)
            .into()
        }))
        .spacing(12);

        let colors = button(text(tr.tr(if self.dark {
            "menu-colors-light"
        } else {
            "menu-colors-dark"
        })))
        .style(style::ghost)
        .padding([8, 12])
        .on_press(Message::ToggleTheme);
        let content = column![
            row![
                column![
                    text(tr.tr("app-name")).size(32).font(style::BOLD),
                    text(tr.tr("app-tagline")).size(15).style(style::muted),
                ]
                .spacing(4),
                Space::new().width(Length::Fill),
                colors,
            ]
            .align_y(Alignment::Center),
            self.progression_card(),
            modules,
            self.menu_options(),
            row![
                action(tr.tr("menu-progress"), Message::Progress(None)),
                action(tr.tr("menu-top10"), Message::Top10),
                action(tr.tr("menu-settings"), Message::Settings),
            ]
            .spacing(8),
            text(tr.tr("menu-shortcuts")).size(12).style(style::muted),
        ]
        .spacing(20)
        .max_width(880);
        scrollable(container(content).center_x(Length::Fill))
            .height(Length::Fill)
            .into()
    }

    /// Tarjeta del menú con los idiomas, el teclado y el modo dictado.
    fn menu_options<'a>(&'a self) -> Element<'a, Message> {
        let tr = &self.tr;
        let languages = ui_languages();
        let current = languages.iter().find(|l| l.code == tr.code()).cloned();
        let interface = pick_list(languages, current, Message::InterfaceSelected)
            .width(Length::Fill)
            .style(style::pick);
        let language = pick_list(
            self.text_languages(),
            Some(TextLanguage(self.language.clone())),
            |l| Message::LanguageSelected(l.0),
        )
        .width(Length::Fill)
        .style(style::pick);
        let keyboard = pick_list(
            self.keyboards.clone(),
            Some(self.keyboard.clone()),
            Message::KeyboardSelected,
        )
        .width(Length::Fill)
        .style(style::pick);
        let field = |label: &str,
                     widget: Element<'a, Message>,
                     tip: Option<String>|
         -> Element<'a, Message> {
            let mut field = column![
                text(bare(&tr.tr(label))).size(13).style(style::muted),
                widget
            ]
            .spacing(6)
            .width(Length::Fill);
            if let Some(tip) = tip {
                field = field.push(text(tip).size(12).style(style::muted));
            }
            field.into()
        };
        let speech_tip = match self.audio.engine().map(Engine::name) {
            Some(engine) => tr.tr_args("menu-speech-tip", &[("engine", &engine)]),
            None => tr.tr("menu-speech-missing"),
        };
        card(
            column![
                row![
                    field("menu-interface-language", interface.into(), None),
                    field(
                        "menu-text-language",
                        language.into(),
                        Some(tr.tr("menu-text-language-tip"))
                    ),
                    field("menu-keyboard", keyboard.into(), None),
                ]
                .spacing(16),
                tooltip(
                    checkbox(self.profile.config.speech)
                        .label(tr.tr("menu-speech"))
                        .on_toggle(Message::ToggleSpeech),
                    tip_box(speech_tip),
                    tooltip::Position::Bottom,
                ),
            ]
            .spacing(16),
        )
        .into()
    }

    fn view_tutor<'a>(&'a self, tutor: &'a Tutor) -> Element<'a, Message> {
        let tr = &self.tr;
        let spec = tutor.spec();
        let title = self.tutor_title(spec);
        let heading = column![
            text(title).size(20).font(style::BOLD),
            text(format!("{} · {}", spec.language, self.keyboard))
                .size(13)
                .style(style::muted),
        ]
        .spacing(2);
        let mut actions: Vec<Element<'a, Message>> = Vec::new();
        // Como el original, el botón de la sesión especial aparece cuando conviene.
        if tutor.special_suggested().is_some() || tutor.special() {
            let label = tr.tr(if tutor.special() {
                "tutor-special-on"
            } else {
                "tutor-special"
            });
            actions.push(
                tooltip(
                    button(text(label))
                        .style(style::chip(tutor.special()))
                        .padding([8, 14])
                        .on_press(Message::ToggleSpecial),
                    tip_box(tr.tr("tutor-special-tip")),
                    tooltip::Position::Bottom,
                )
                .into(),
            );
        }
        if spec.module == Module::Basic {
            actions.push(stepper(
                tr.tr_args("tutor-lesson-n", &[("lesson", &spec.lesson.to_string())]),
                Message::LessonDelta(-1),
                Message::LessonDelta(1),
            ));
        }
        let has_keyboard = matches!(spec.module, Module::Basic | Module::Adaptability);
        if has_keyboard {
            actions.push(
                tooltip(
                    button(text(tr.tr("tutor-keyboard")))
                        .style(style::chip(self.profile.config.show_keyboard))
                        .padding([8, 14])
                        .on_press(Message::ToggleKeyboard),
                    tip_box(tr.tr("tutor-keyboard-tip")),
                    tooltip::Position::Bottom,
                )
                .into(),
            );
        }
        if let Some(slot) = spec.slot {
            actions.push(action(tr.tr("sections-retry"), Message::Restart));
            actions.push(action(
                tr.tr("menu-progress"),
                Message::LessonProgress(slot),
            ));
        } else {
            actions.push(action(tr.tr("tutor-new"), Message::Restart));
            actions.push(action(
                tr.tr("menu-progress"),
                Message::Progress(Some(spec.module)),
            ));
        }
        // Desde una lección, volver lleva a la grilla de su sección y no al menú.
        let back = if spec.slot.is_some() {
            "sections-back"
        } else {
            "menu-back"
        };
        let header = bar_with_back(&tr.tr(back), heading.into(), actions);

        let palette = Palette::new(self.dark);
        let spans: Vec<Span<'_, Message>> = tutor
            .segments_at(Some(now()))
            .into_iter()
            .map(|s| styled_span(s, &palette))
            .collect();
        let body = scrollable(
            container(
                rich_text(spans)
                    .font(Font::MONOSPACE)
                    .size(24)
                    .line_height(1.5),
            )
            .padding([24, 32])
            .width(Length::Fill),
        )
        .id(EXERCISE_SCROLL)
        .height(Length::Fill);
        let exercise = container(stack![body, Typing])
            .height(Length::Fill)
            .style(style::card);

        let mut page = column![header, exercise].spacing(16);
        if has_keyboard && self.profile.config.show_keyboard {
            page = page.push(self.keyboard_view(tutor));
        }
        page.push(self.tutor_footer(tutor)).into()
    }

    /// Título del ejercicio: el módulo o, en la Progresión, la lección o el examen.
    fn tutor_title(&self, spec: &ExerciseSpec) -> String {
        let tr = &self.tr;
        match spec.slot {
            None => module_name(tr, spec.module),
            Some(slot) if spec.exam => tr.tr_args(
                "sections-exam-title",
                &[("section", &slot.section().to_string())],
            ),
            Some(slot) => self.lesson_title(slot),
        }
    }

    /// Teclado virtual con la tecla y el dedo siguientes.
    fn keyboard_view<'a>(&'a self, tutor: &'a Tutor) -> Element<'a, Message> {
        let Some(fingers) = &self.fingers else {
            return Space::new().into();
        };
        let tr = &self.tr;
        let next = tutor
            .next_char()
            .and_then(|c| hint(tutor.layout(), fingers, c));
        let mut caption = next
            .and_then(|h| h.finger)
            .map(|f| finger_text(tr, f))
            .unwrap_or_default();
        if next.is_some_and(|h| h.shift.is_some()) {
            caption = format!("{caption} · {}", tr.tr("hint-shift"));
        }
        column![
            canvas(Keys {
                layout: tutor.layout(),
                fingers,
                hint: next,
            })
            .width(Length::Fill)
            .height(Length::Fixed(Keys::height())),
            text(caption).size(14).style(style::muted),
        ]
        .spacing(6)
        .align_x(Alignment::Center)
        .into()
    }

    /// Pie del ejercicio: avance mientras se teclea, resultados al terminar.
    fn tutor_footer<'a>(&'a self, tutor: &'a Tutor) -> Element<'a, Message> {
        let tr = &self.tr;
        let spec = tutor.spec();
        if let Some((o, result)) = tutor.outcome().and_then(|o| o.lesson.map(|r| (o, r))) {
            self.lesson_result(tutor, o, result)
        } else if let Some(o) = tutor.outcome() {
            let colors = style::Colors::new(self.dark);
            let mark = |ok: bool| if ok { colors.success } else { colors.danger };
            let value = |key: &str, v: f64| tr.tr_args(key, &[("value", &format!("{v:.1}"))]);
            let mut stats = vec![
                stat(
                    &tr.tr("stats-elapsed"),
                    value("stats-seconds", o.stats.elapsed.as_secs_f64()),
                    None,
                ),
                stat(
                    &tr.tr("stats-accuracy"),
                    value("stats-percent", o.stats.accuracy),
                    Some(mark(o.check.accuracy)),
                ),
                stat(
                    &tr.tr("stats-wpm"),
                    value("stats-wpm-value", o.stats.wpm),
                    Some(mark(o.check.speed)),
                ),
            ];
            if let Some(ok) = o.check.fluidity {
                stats.push(stat(
                    &tr.tr("stats-fluidity"),
                    format!("{:.1}", o.stats.fluidity),
                    Some(mark(ok)),
                ));
            }
            let mut comment =
                column![text(comment_text(tr, o.comment)).size(17).font(style::BOLD)].spacing(4);
            if let Some(note) = &self.note {
                comment = comment.push(text(note.clone()).size(14));
            }
            comment = comment.push(text(tr.tr("tutor-end")).size(13).style(style::muted));
            column![row(stats).spacing(12), card(comment)]
                .spacing(12)
                .into()
        } else {
            let (done, total) = tutor.progress();
            let key = match spec.module {
                Module::Fluidity => "tutor-hint-backspace",
                Module::Progression => "sections-hint",
                _ => "tutor-hint",
            };
            let lesson_status = spec.slot.map(|slot| {
                self.lesson_status(
                    slot,
                    // Cada letra que queda mal resta: el tiempo máximo es el de las bien escritas.
                    progression::time_limit(tutor.correct_letters(), slot),
                    tutor.elapsed(),
                    &tutor.missing_keys(),
                    tutor.streak(),
                )
            });
            let ratio = done as f32 / total.max(1) as f32;
            let (done, total) = (done.to_string(), total.to_string());
            let mut footer = column![].spacing(8);
            if let Some(status) = lesson_status {
                footer = footer.push(status);
            }
            footer
                .push(
                    progress_bar(0.0..=1.0, ratio)
                        .girth(6)
                        .style(style::progress),
                )
                .push(
                    text(tr.tr_args(key, &[("done", &done), ("total", &total)]))
                        .size(13)
                        .style(style::muted),
                )
                .into()
        }
    }

    /// Qué se está graficando y los controles para elegir lección y sección.
    fn chart_filters<'a>(&self, v: &ProgressView) -> iced::widget::Row<'a, Message> {
        let tr = &self.tr;
        // Igual que el título de la ventana del original: teclado o idioma según el módulo.
        let scope = match v.module {
            Module::Basic | Module::Adaptability => self.keyboard.clone(),
            Module::Velocity | Module::Fluidity | Module::Progression => {
                klavaro_i18n::native_name(&self.language)
                    .unwrap_or(&self.language)
                    .to_owned()
            }
        };
        let scope = if v.module == Module::Progression {
            klavaro_i18n::native_name(LESSONS_LANGUAGE)
                .unwrap_or(LESSONS_LANGUAGE)
                .to_owned()
        } else {
            scope
        };
        let mut filters = row![text(scope).size(13).style(style::muted)]
            .spacing(12)
            .align_y(Alignment::Center);
        if v.module == Module::Basic {
            let lesson = if v.lesson == 0 {
                tr.tr("progress-all-lessons")
            } else {
                tr.tr_args("tutor-lesson-n", &[("lesson", &v.lesson.to_string())])
            };
            filters = filters.push(stepper(
                lesson,
                Message::ProgressLesson(-1),
                Message::ProgressLesson(1),
            ));
        }

        if v.module == Module::Progression {
            let section = if v.section == 0 {
                tr.tr("progress-all-sections")
            } else {
                tr.tr_args("stats-wpm-value", &[("value", &v.section.to_string())])
            };
            filters = filters.push(stepper(
                section,
                Message::ProgressSection(-1),
                Message::ProgressSection(1),
            ));
            let lesson = if v.lesson == 0 {
                tr.tr("progress-all-lessons")
            } else {
                tr.tr_args("tutor-lesson-n", &[("lesson", &v.lesson.to_string())])
            };
            filters = filters.push(stepper(
                lesson,
                Message::ProgressLesson(-1),
                Message::ProgressLesson(1),
            ));
        }
        filters
    }

    fn view_progress<'a>(&'a self, v: &'a ProgressView) -> Element<'a, Message> {
        let tr = &self.tr;
        let choice = |label: String, active: bool, msg: Message| {
            button(text(label))
                .style(style::chip(active))
                .padding([6, 14])
                .on_press(msg)
        };
        let modules = row(Module::ALL
            .into_iter()
            .chain([Module::Progression])
            .map(|m| {
                choice(
                    module_name(tr, m),
                    m == v.module,
                    Message::Progress(Some(m)),
                )
                .into()
            }))
        .spacing(6);
        let metrics = row(Metric::for_module(v.module).iter().map(|&m| {
            choice(
                metric_label(tr, m),
                m == v.metric,
                Message::ProgressMetric(m),
            )
            .into()
        }))
        .spacing(6);

        let filters = self.chart_filters(v);

        let chart = &v.chart;
        let summary = match (chart.points.first(), chart.points.last()) {
            (Some(_), _) if v.metric.per_key() => tr.tr_args(
                "progress-keys",
                &[
                    ("count", &chart.points.len().to_string()),
                    ("keyboard", &self.keyboard),
                ],
            ),
            (Some(first), Some(last)) => tr.tr_args(
                if chart.points.len() == 1 {
                    "progress-session-one"
                } else {
                    "progress-sessions"
                },
                &[
                    ("count", &chart.points.len().to_string()),
                    ("first", &format_date_time(first.at)),
                    ("last", &format_date_time(last.at)),
                ],
            ),
            _ => tr.tr("progress-empty"),
        };
        let mut info = row![text(summary).size(13).style(style::muted)].spacing(16);
        if let Some(goal) = chart.goal {
            info = info.push(
                text(tr.tr_args("progress-goal", &[("value", &format!("{goal:.0}"))]))
                    .size(13)
                    .style(style::muted),
            );
        }
        let plot = Plot {
            chart,
            unit: metric_unit(tr, v.metric),
        };

        let content: Element<'a, Message> = if v.metric == Metric::Stars {
            self.stars_chart()
        } else {
            column![canvas(plot).width(Length::Fill).height(Length::Fill), info]
                .spacing(8)
                .into()
        };
        column![
            top_bar(
                tr,
                page_title(tr.tr("menu-progress")),
                [action(tr.tr("menu-top10"), Message::Top10)],
            ),
            row![modules, Space::new().width(Length::Fill), filters]
                .spacing(12)
                .align_y(Alignment::Center),
            metrics,
            card(content).height(Length::Fill),
        ]
        .spacing(14)
        .into()
    }

    fn view_settings<'a>(&'a self, form: &'a SettingsForm) -> Element<'a, Message> {
        let tr = &self.tr;
        let labeled = |label: String, widget: Element<'a, Message>| {
            row![text(label).width(Length::Fixed(220.0)), widget]
                .spacing(8)
                .align_y(Alignment::Center)
        };
        let name = text_input("", &form.name)
            .on_input(Message::SettingsName)
            .on_submit(Message::SettingsSave)
            .padding(8)
            .width(Length::Fixed(280.0))
            .style(style::input(false));
        let paragraphs = if form.paragraphs == 0 {
            tr.tr("settings-paragraphs-full")
        } else {
            form.paragraphs.to_string()
        };
        let paragraphs = stepper(
            paragraphs,
            Message::SettingsParagraphs(-1),
            Message::SettingsParagraphs(1),
        );

        let save = button(text(tr.tr("settings-save")))
            .style(style::primary)
            .padding([8, 18])
            .on_press(Message::SettingsSave);
        column![
            top_bar(tr, page_title(tr.tr("menu-settings")), [save.into()]),
            scrollable(
                column![
                    card(
                        column![
                            column![
                                labeled(tr.tr("settings-name"), name.into()),
                                text(tr.tr("settings-name-tip"))
                                    .size(12)
                                    .style(style::muted),
                            ]
                            .spacing(6),
                            labeled(tr.tr("tutor-paragraphs"), paragraphs),
                            checkbox(form.autostart)
                                .label(tr.tr("settings-autostart"))
                                .on_toggle(Message::SettingsAutostart),
                            checkbox(form.beep)
                                .label(tr.tr("settings-beep"))
                                .on_toggle(Message::SettingsBeep),
                        ]
                        .push(update::available().then(|| {
                            checkbox(form.check_updates)
                                .label(tr.tr("settings-check-updates"))
                                .on_toggle(Message::SettingsCheckUpdates)
                        }))
                        .spacing(16)
                    ),
                    card(
                        column![
                            text(tr.tr("settings-goals")).size(18).font(style::BOLD),
                            goals_table(tr, form),
                            text(tr.tr("settings-goals-tip"))
                                .size(12)
                                .style(style::muted),
                            button(text(tr.tr("settings-defaults")))
                                .style(style::secondary)
                                .padding([8, 14])
                                .on_press(Message::SettingsDefaults),
                        ]
                        .spacing(14)
                    ),
                    self.reset_card(),
                ]
                .spacing(16)
            )
            .height(Length::Fill),
        ]
        .spacing(16)
        .into()
    }

    fn view_top10<'a>(&'a self, entries: &'a [RankingEntry]) -> Element<'a, Message> {
        let tr = &self.tr;
        let widths = [
            Length::Fixed(32.0),
            Length::Fill,
            Length::Fixed(90.0),
            Length::Fixed(90.0),
            Length::Fixed(90.0),
            Length::Fixed(90.0),
            Length::Fixed(90.0),
            Length::Fixed(140.0),
        ];
        let line = |cells: [Element<'a, Message>; 8]| {
            row(cells
                .into_iter()
                .zip(widths)
                .map(|(c, w)| container(c).width(w).into()))
            .spacing(8)
            .align_y(Alignment::Center)
        };
        let head = |key: &str| -> Element<'a, Message> {
            text(tr.tr(key)).size(13).style(style::muted).into()
        };
        let header = line([
            head("ranking-place"),
            head("ranking-name"),
            head("stats-score"),
            head("progress-accuracy"),
            head("progress-speed"),
            head("progress-fluidity"),
            head("ranking-chars"),
            head("ranking-when"),
        ]);
        let mut table = column![container(header).padding([4, 10])].spacing(2);
        if entries.is_empty() {
            table = table.push(
                container(text(tr.tr("ranking-empty")).style(style::muted))
                    .center_x(Length::Fill)
                    .padding(32),
            );
        }
        for (i, e) in entries.iter().enumerate() {
            let place = text(format!("{}", i + 1)).font(style::BOLD);
            let entry = line([
                place.into(),
                text(e.name.clone()).into(),
                text(format!("{:.3}", e.score)).font(style::BOLD).into(),
                text(format!("{:.1}", e.accuracy)).into(),
                text(format!("{:.1}", e.wpm)).into(),
                text(format!("{:.1}", e.fluidity)).into(),
                text(e.chars.to_string()).into(),
                text(format_date_time(e.when))
                    .size(13)
                    .style(style::muted)
                    .into(),
            ]);
            let mut entry = container(entry).padding([8, 10]);
            if i % 2 == 0 {
                entry = entry.style(style::stripe);
            }
            table = table.push(entry);
        }
        let language = pick_list(
            self.text_languages(),
            Some(TextLanguage(self.language.clone())),
            |l| Message::LanguageSelected(l.0),
        )
        .style(style::pick);
        column![
            top_bar(
                tr,
                page_title(tr.tr("menu-top10")),
                [
                    text(tr.tr("ranking-local"))
                        .size(13)
                        .style(style::muted)
                        .into(),
                    language.into(),
                    action(
                        tr.tr("menu-progress"),
                        Message::Progress(Some(Module::Fluidity))
                    ),
                ],
            ),
            card(scrollable(table).height(Length::Shrink)),
        ]
        .spacing(16)
        .into()
    }
}

/// Barra superior de cada pantalla: volver, título y acciones a la derecha.
fn top_bar<'a>(
    tr: &Translator,
    title: Element<'a, Message>,
    actions: impl IntoIterator<Item = Element<'a, Message>>,
) -> Element<'a, Message> {
    bar_with_back(&tr.tr("menu-back"), title, actions)
}

/// Barra superior con el texto dado en el botón de volver.
fn bar_with_back<'a>(
    back: &str,
    title: Element<'a, Message>,
    actions: impl IntoIterator<Item = Element<'a, Message>>,
) -> Element<'a, Message> {
    row![
        button(text(format!("← {back}")))
            .style(style::ghost)
            .padding([8, 12])
            .on_press(Message::Back),
        title,
        Space::new().width(Length::Fill),
    ]
    .extend(actions)
    .spacing(12)
    .align_y(Alignment::Center)
    .into()
}

/// Título de una pantalla.
fn page_title<'a>(title: String) -> Element<'a, Message> {
    text(title).size(22).font(style::BOLD).into()
}

/// Botón secundario de una barra de acciones.
fn action<'a>(label: String, message: Message) -> Element<'a, Message> {
    button(text(label))
        .style(style::secondary)
        .padding([8, 14])
        .on_press(message)
        .into()
}

/// Valor con botones para bajarlo y subirlo (lección, párrafos).
fn stepper<'a>(value: String, down: Message, up: Message) -> Element<'a, Message> {
    let small = |label: &'static str, message: Message| {
        button(text(label).center().width(Length::Fixed(14.0)))
            .style(style::secondary)
            .padding([6, 8])
            .on_press(message)
    };
    row![
        small("−", down),
        text(value).center().width(Length::Fixed(110.0)),
        small("+", up),
    ]
    .spacing(6)
    .align_y(Alignment::Center)
    .into()
}

/// Contenido envuelto en una tarjeta.
fn card<'a>(content: impl Into<Element<'a, Message>>) -> iced::widget::Container<'a, Message> {
    container(content)
        .padding(20)
        .width(Length::Fill)
        .style(style::card)
}

/// Resultado del ejercicio: etiqueta arriba y valor grande, en el color de su meta.
fn stat<'a>(label: &str, value: String, color: Option<Color>) -> Element<'a, Message> {
    let mut value = text(value).size(26).font(style::BOLD);
    if let Some(color) = color {
        value = value.color(color);
    }
    container(column![text(bare(label)).size(13).style(style::muted), value].spacing(4))
        .padding([12, 16])
        .width(Length::Fill)
        .style(style::card)
        .into()
}

/// Aviso a lo ancho de la ventana.
fn banner<'a>(
    message: String,
    style: fn(&Theme) -> iced::widget::container::Style,
) -> Element<'a, Message> {
    container(text(message).size(14))
        .padding([10, 14])
        .width(Length::Fill)
        .style(style)
        .into()
}

/// Recuadro de un `tooltip`.
fn tip_box<'a>(tip: String) -> Element<'a, Message> {
    container(text(tip).size(13))
        .padding([8, 10])
        .max_width(360)
        .style(style::card)
        .into()
}

/// Etiqueta sin los dos puntos finales («Teclado:» → «Teclado»), para ponerla sobre su campo.
fn bare(label: &str) -> String {
    label.trim_end_matches([':', '：', ' ']).to_owned()
}

/// Tabla de metas de Preferencias: una fila por módulo y una columna por medida.
fn goals_table<'a>(tr: &Translator, form: &'a SettingsForm) -> Element<'a, Message> {
    let cell = 110.0;
    let fields = [GoalField::Accuracy, GoalField::Speed, GoalField::Fluidity];
    let header = row![Space::new().width(Length::Fixed(220.0))]
        .extend(fields.iter().map(|&f| {
            text(goal_field_label(tr, f))
                .size(13)
                .style(style::muted)
                .width(Length::Fixed(cell))
                .into()
        }))
        .spacing(8)
        .align_y(Alignment::End);
    let mut goals = column![header].spacing(8);
    for module in Module::ALL {
        let input = form.goal(module);
        let mut line = row![text(module_name(tr, module)).width(Length::Fixed(220.0))]
            .spacing(8)
            .align_y(Alignment::Center);
        for field in fields {
            let widget: Element<'a, Message> = match input.get(field) {
                Some(value) => text_input("", value)
                    .on_input(move |v| Message::SettingsGoal(module, field, v))
                    .on_submit(Message::SettingsSave)
                    .padding(8)
                    .width(Length::Fixed(cell))
                    .style(style::input(form.is_invalid(module, field)))
                    .into(),
                None => Space::new().width(Length::Fixed(cell)).into(),
            };
            line = line.push(widget);
        }
        goals = goals.push(line);
    }
    goals.into()
}

/// Encabezado de una columna de metas, con su unidad.
fn goal_field_label(tr: &Translator, f: GoalField) -> String {
    match f {
        GoalField::Accuracy => metric_label(tr, Metric::Accuracy),
        GoalField::Speed => metric_label(tr, Metric::Speed),
        GoalField::Fluidity => metric_label(tr, Metric::Fluidity),
    }
}

/// Nombre de un gráfico con su unidad, como en el original («Precisión (%)»).
fn metric_label(tr: &Translator, m: Metric) -> String {
    let unit = match m {
        Metric::Accuracy | Metric::Fluidity => "(%)".to_owned(),
        Metric::Speed => tr.tr("progress-wpm-unit"),
        Metric::Score => "(0-10)".to_owned(),
        // «Touch times (s)» ya trae la unidad.
        Metric::Errors | Metric::TouchTimes | Metric::Stars => return tr.tr(m.label_key()),
    };
    format!("{} {unit}", tr.tr(m.label_key()))
}

/// Unidad del valor señalado en el gráfico.
fn metric_unit(tr: &Translator, m: Metric) -> String {
    match m {
        Metric::Accuracy | Metric::Fluidity => " %".to_owned(),
        Metric::Speed => format!(" {}", tr.tr("progress-wpm-unit").trim_matches(['(', ')'])),
        Metric::Score | Metric::Errors | Metric::Stars => String::new(),
        Metric::TouchTimes => " s".to_owned(),
    }
}

/// Colores del texto del ejercicio, tomados de los del tema.
struct Palette {
    correct: Color,
    wrong: Color,
    wrong_bg: Color,
    retouched: Color,
    cursor: Color,
    cursor_bg: Color,
    untouched: Color,
    /// Fondo del destello de una transición fluida, con su intensidad máxima.
    flash_bg: Color,
}

impl Palette {
    fn new(dark: bool) -> Self {
        let c = style::Colors::new(dark);
        Self {
            correct: c.success,
            wrong: c.danger,
            wrong_bg: Color {
                a: 0.18,
                ..c.danger
            },
            retouched: c.warning,
            cursor: if dark { c.background } else { Color::WHITE },
            cursor_bg: c.primary,
            untouched: c.text,
            flash_bg: Color {
                a: if dark { 0.45 } else { 0.32 },
                ..c.success
            },
        }
    }
}

fn styled_span(segment: Segment, p: &Palette) -> Span<'static, Message> {
    let s = span(segment.text);
    match segment.style {
        SegmentStyle::Untouched => s.color(p.untouched),
        SegmentStyle::Correct => s.color(p.correct),
        SegmentStyle::Wrong => s.color(p.wrong).background(p.wrong_bg),
        SegmentStyle::Retouched => s.color(p.retouched),
        SegmentStyle::Cursor => s.color(p.cursor).background(p.cursor_bg),
        SegmentStyle::Flash(level) => s.color(p.correct).background(Color {
            a: p.flash_bg.a * f32::from(level) / f32::from(FLASH_LEVELS),
            ..p.flash_bg
        }),
    }
}

/// Identificador del área con el texto del ejercicio, para seguir al cursor.
const EXERCISE_SCROLL: &str = "exercise";

/// Espera del dictado del dedo en el curso básico (`g_timeout_add (3000, …)` del original).
const FINGER_TIP_DELAY: Duration = Duration::from_secs(3);

/// Tarea que avisa con [`Message::FingerTip`] después de `delay`. El ejecutor de iced que se
/// usa no trae temporizadores, así que espera un hilo aparte (hay uno a la vez).
fn wake_after(delay: Duration) -> Task<Message> {
    let (tx, rx) = iced::futures::channel::oneshot::channel();
    std::thread::spawn(move || {
        std::thread::sleep(delay);
        let _ = tx.send(());
    });
    Task::perform(rx, |_| Message::FingerTip)
}

/// Tarea que busca una versión nueva en un hilo aparte (la consulta bloquea).
fn check_release() -> Task<Message> {
    let (tx, rx) = iced::futures::channel::oneshot::channel();
    std::thread::spawn(move || {
        let _ = tx.send(update::check());
    });
    Task::perform(rx, |r| Message::ReleaseFound(r.ok().flatten()))
}

/// Traduce los eventos de teclado de iced a acciones del tutor.
/// Cualquier movimiento o clic del mouse lo vuelve a mostrar.
// iced pide la firma de `listen_with`.
#[allow(clippy::needless_pass_by_value)]
fn on_mouse(event: Event, _status: event::Status, _window: iced::window::Id) -> Option<Message> {
    match event {
        Event::Mouse(
            mouse::Event::CursorMoved { .. }
            | mouse::Event::ButtonPressed(_)
            | mouse::Event::WheelScrolled { .. },
        ) => Some(Message::MouseMoved),
        _ => None,
    }
}

/// Instante actual, medido desde la primera vez que se pide. Los tiempos de las teclas se
/// toman al recibir el evento y no al procesar el mensaje, para que no se junten cuando
/// llegan varios en el mismo cuadro.
fn now() -> Duration {
    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now).elapsed()
}

/// Identidad estable de una tecla física.
fn key_id(physical: keyboard::key::Physical) -> KeyId {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    physical.hash(&mut h);
    KeyId(h.finish())
}

fn on_event(event: Event, status: event::Status, _window: iced::window::Id) -> Option<Message> {
    let at = now();
    // Las liberaciones se miran aunque otro widget haya capturado el evento: sólo sirven
    // para medir y no hacen nada fuera de un ejercicio.
    if let Event::Keyboard(keyboard::Event::KeyReleased { physical_key, .. }) = event {
        return Some(Message::KeyUp(key_id(physical_key), at));
    }
    if matches!(status, event::Status::Captured) {
        return None;
    }
    if let Event::InputMethod(input_method::Event::Commit(text)) = event {
        return Some(Message::Typed(text, at));
    }
    let Event::Keyboard(keyboard::Event::KeyPressed {
        key,
        modifiers,
        text,
        physical_key,
        repeat,
        ..
    }) = event
    else {
        return None;
    };
    let stroke = Stroke {
        at,
        key: Some(key_id(physical_key)),
        repeat,
    };
    let named = match key.as_ref() {
        keyboard::Key::Named(Named::Enter) => Some(NamedKey::Enter),
        keyboard::Key::Named(Named::Backspace) => Some(NamedKey::Backspace),
        keyboard::Key::Named(Named::Escape) => Some(NamedKey::Escape),
        keyboard::Key::Named(Named::Space) => Some(NamedKey::Space),
        keyboard::Key::Named(Named::ArrowUp) => Some(NamedKey::Arrow(Direction::Up)),
        keyboard::Key::Named(Named::ArrowDown) => Some(NamedKey::Arrow(Direction::Down)),
        keyboard::Key::Named(Named::ArrowLeft) => Some(NamedKey::Arrow(Direction::Left)),
        keyboard::Key::Named(Named::ArrowRight) => Some(NamedKey::Arrow(Direction::Right)),
        _ => None,
    };
    let ctrl = modifiers.control() || modifiers.command();
    let typed = if ctrl {
        match key.as_ref() {
            keyboard::Key::Character(c) => Some(c.to_owned()),
            _ => None,
        }
    } else {
        text.map(|t| t.to_string())
    };
    translate(named, typed.as_deref(), ctrl).map(|input| Message::Key(input, stroke))
}
