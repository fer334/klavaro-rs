// SPDX-License-Identifier: GPL-3.0-or-later
//! Interfaz gráfica en iced (arquitectura Elm: estado, mensajes, `update` y `view`).
//!
//! Toda la lógica vive en [`crate::tutor`]; este módulo sólo traduce eventos y dibuja.

use std::time::{Instant, SystemTime, UNIX_EPOCH};

use iced::keyboard::{self, key::Named};
use iced::widget::text::Span;
use iced::widget::{button, column, container, pick_list, rich_text, row, scrollable, span, text};
use iced::{Alignment, Color, Element, Event, Font, Length, Subscription, Task, Theme, event};

use klavaro_core::Module;

use crate::assets;
use crate::exercise::{ExerciseSpec, comment_text, module_name, notice_text, recorded_text};
use crate::profile::Profile;
use crate::tutor::{Input, NamedKey, Segment, SegmentStyle, Tutor, translate};

/// Abre la ventana principal.
pub(crate) fn run() -> iced::Result {
    iced::application(App::new, App::update, App::view)
        .title(App::title)
        .subscription(App::subscription)
        .theme(App::theme)
        .window_size((960.0, 680.0))
        .run()
}

enum Screen {
    Menu,
    Tutor(Box<Tutor>),
}

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
    clock: Instant,
}

#[derive(Debug, Clone)]
enum Message {
    Start(Module),
    LanguageSelected(String),
    KeyboardSelected(String),
    Key(Input),
    Back,
    Restart,
    LessonDelta(i32),
    ToggleTheme,
}

impl App {
    fn new() -> Self {
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
        let text_language = config.text_language.clone().unwrap_or_else(|| {
            let prefs = config.language.iter().chain(&system).map(String::as_str);
            klavaro_i18n::resolve(prefs).to_owned()
        });
        let error = keyboards
            .is_empty()
            .then(|| format!("No se encontraron datos en {}", assets::dir().display()));
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
            notice: profile.notice.as_ref().map(notice_text),
            screen: Screen::Menu,
            clock: Instant::now(),
            profile,
        };
        if app.profile.config.autostart
            && let Some(m) = app.profile.config.last_module
            && let Err(e) = app.start(m)
        {
            app.error = Some(e.to_string());
        }
        app
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

    /// Registra el ejercicio recién terminado.
    fn record(&mut self) {
        let Screen::Tutor(t) = &self.screen else {
            return;
        };
        let Some(outcome) = t.outcome() else {
            return;
        };
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
            Ok(r) => self.note = recorded_text(r),
            Err(e) => self.error = Some(e.to_string()),
        }
    }

    fn title(&self) -> String {
        match &self.screen {
            Screen::Menu => "klavaro-rs".to_owned(),
            Screen::Tutor(t) => format!("klavaro-rs · {}", module_name(t.spec().module)),
        }
    }

    fn theme(&self) -> Theme {
        if self.dark { Theme::Dark } else { Theme::Light }
    }

    // iced pide `Fn(&State)`.
    #[allow(clippy::unused_self)]
    fn subscription(&self) -> Subscription<Message> {
        event::listen_with(on_event)
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        let result = match message {
            Message::Start(module) => self.start(module),
            Message::LanguageSelected(l) => {
                self.language = l;
                self.persist();
                Ok(())
            }
            Message::KeyboardSelected(k) => {
                self.keyboard = k;
                self.persist();
                Ok(())
            }
            Message::ToggleTheme => {
                self.dark = !self.dark;
                self.persist();
                Ok(())
            }
            Message::Back => {
                self.screen = Screen::Menu;
                Ok(())
            }
            Message::Restart => match &mut self.screen {
                Screen::Tutor(t) => {
                    self.note = None;
                    t.restart()
                }
                Screen::Menu => Ok(()),
            },
            Message::LessonDelta(delta) => match &mut self.screen {
                Screen::Tutor(t) => {
                    let lesson = i32::from(t.spec().lesson) + delta;
                    let r = t.set_lesson(u16::try_from(lesson.max(1)).unwrap_or(1));
                    self.note = None;
                    self.sync_lesson();
                    r
                }
                Screen::Menu => Ok(()),
            },
            Message::Key(input) => self.key(input),
        };
        if let Err(e) = result {
            self.error = Some(e.to_string());
        }
        Task::none()
    }

    fn start(&mut self, module: Module) -> std::io::Result<()> {
        let layout = assets::keyboard(&self.keyboard)?;
        let config = &self.profile.config;
        let spec = ExerciseSpec {
            module,
            lesson: self.lesson,
            paragraphs: usize::from(config.fluid_paragraphs),
            language: self.language.clone(),
        };
        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(1, |d| d.as_secs());
        let tutor = Tutor::new(spec, layout, config.goals.clone(), seed)?;
        self.screen = Screen::Tutor(Box::new(tutor));
        self.error = None;
        self.note = None;
        self.notice = None;
        self.profile.config.last_module = Some(module);
        self.persist();
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

    fn key(&mut self, input: Input) -> std::io::Result<()> {
        match &mut self.screen {
            Screen::Tutor(t) => {
                let was_done = t.outcome().is_some();
                let effect = t.handle(input, self.clock.elapsed())?;
                if was_done && t.outcome().is_none() {
                    self.note = None;
                }
                if effect.finished {
                    self.record();
                }
                self.sync_lesson();
                if effect.toggle_theme {
                    self.dark = !self.dark;
                    self.persist();
                }
                Ok(())
            }
            Screen::Menu => {
                // En el menú, 1–4 abren cada módulo.
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
        };
        let mut page = column![body].spacing(12).padding(24);
        if let (Screen::Menu, Some(n)) = (&self.screen, &self.notice) {
            page = page.push(text(n.clone()).size(14));
        }
        if let Some(e) = &self.error {
            page = page.push(text(e.clone()).color(Color::from_rgb(0.8, 0.1, 0.1)));
        }
        container(page)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    fn view_menu(&self) -> Element<'_, Message> {
        let modules = column(Module::ALL.iter().enumerate().map(|(i, &m)| {
            button(text(format!("{} - {}", i + 1, module_name(m))).size(18))
                .width(Length::Fixed(280.0))
                .padding(10)
                .on_press(Message::Start(m))
                .into()
        }))
        .spacing(8);
        let language = pick_list(
            self.languages.clone(),
            Some(self.language.clone()),
            Message::LanguageSelected,
        );
        let keyboard = pick_list(
            self.keyboards.clone(),
            Some(self.keyboard.clone()),
            Message::KeyboardSelected,
        );
        let colors = button(text(if self.dark {
            "Colores claros"
        } else {
            "Colores oscuros"
        }))
        .on_press(Message::ToggleTheme);
        column![
            text("klavaro-rs").size(36),
            text("Tutor de mecanografía").size(16),
            modules,
            row![text("Idioma").width(Length::Fixed(80.0)), language]
                .spacing(8)
                .align_y(Alignment::Center),
            row![text("Teclado").width(Length::Fixed(80.0)), keyboard]
                .spacing(8)
                .align_y(Alignment::Center),
            colors,
            text("Atajos: 1–4 abren un módulo · Ctrl+A cambia los colores").size(13),
        ]
        .spacing(16)
        .into()
    }

    fn view_tutor<'a>(&'a self, tutor: &'a Tutor) -> Element<'a, Message> {
        let spec = tutor.spec();
        let mut header = row![
            button(text("← Menú")).on_press(Message::Back),
            text(format!(
                "{} · {} · {}",
                module_name(spec.module),
                spec.language,
                self.keyboard
            ))
            .size(18),
            button(text("Nuevo (Esc)")).on_press(Message::Restart),
        ]
        .spacing(12)
        .align_y(Alignment::Center);
        if spec.module == Module::Basic {
            header = header.push(
                row![
                    button(text("−")).on_press(Message::LessonDelta(-1)),
                    text(format!("Lección {}", spec.lesson)),
                    button(text("+")).on_press(Message::LessonDelta(1)),
                ]
                .spacing(6)
                .align_y(Alignment::Center),
            );
        }

        let palette = Palette::new(self.dark);
        let spans: Vec<Span<'_, Message>> = tutor
            .segments()
            .into_iter()
            .map(|s| styled_span(s, &palette))
            .collect();
        let exercise = scrollable(
            container(rich_text(spans).font(Font::MONOSPACE).size(24))
                .padding(24)
                .width(Length::Fill),
        )
        .height(Length::Fill);

        let footer: Element<'_, Message> = if let Some(o) = tutor.outcome() {
            let mark = |ok: bool| if ok { palette.correct } else { palette.wrong };
            let mut results = column![
                text(format!("Tiempo: {:.1} s", o.stats.elapsed.as_secs_f64())),
                text(format!("Precisión: {:.1} %", o.stats.accuracy)).color(mark(o.check.accuracy)),
                text(format!("Velocidad: {:.1} PPM", o.stats.wpm)).color(mark(o.check.speed)),
            ]
            .spacing(4);
            if let Some(ok) = o.check.fluidity {
                results =
                    results.push(text(format!("Fluidez: {:.1}", o.stats.fluidity)).color(mark(ok)));
            }
            if let Some(note) = &self.note {
                results = results.push(text(note.clone()).size(14));
            }
            results
                .push(text(comment_text(o.comment)).size(18))
                .push(text("Enter: seguir").size(13))
                .into()
        } else {
            let (done, total) = tutor.progress();
            text(format!(
                "{done}/{total} · Esc: nuevo ejercicio · Ctrl+A: colores{}",
                if spec.module == Module::Fluidity {
                    " · Backspace corrige"
                } else {
                    ""
                }
            ))
            .size(13)
            .into()
        };

        column![header, exercise, footer].spacing(12).into()
    }
}

/// Colores del texto del ejercicio.
struct Palette {
    correct: Color,
    wrong: Color,
    wrong_bg: Color,
    retouched: Color,
    cursor_bg: Color,
    untouched: Color,
}

impl Palette {
    fn new(dark: bool) -> Self {
        if dark {
            Self {
                correct: Color::from_rgb8(0x6c, 0xd6, 0x7c),
                wrong: Color::from_rgb8(0xff, 0x6b, 0x6b),
                wrong_bg: Color::from_rgb8(0x4a, 0x1c, 0x1c),
                retouched: Color::from_rgb8(0xe0, 0xb0, 0x60),
                cursor_bg: Color::from_rgb8(0x8a, 0x6b, 0xd6),
                untouched: Color::from_rgb8(0xd8, 0xd8, 0xd8),
            }
        } else {
            Self {
                correct: Color::from_rgb8(0x22, 0x88, 0x22),
                wrong: Color::from_rgb8(0xcc, 0x00, 0x33),
                wrong_bg: Color::from_rgb8(0xfb, 0xe0, 0xe0),
                retouched: Color::from_rgb8(0x88, 0x66, 0x00),
                cursor_bg: Color::from_rgb8(0xff, 0x88, 0xff),
                untouched: Color::from_rgb8(0x22, 0x22, 0x22),
            }
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
        SegmentStyle::Cursor => s.color(Color::BLACK).background(p.cursor_bg),
    }
}

/// Traduce los eventos de teclado de iced a acciones del tutor.
fn on_event(event: Event, status: event::Status, _window: iced::window::Id) -> Option<Message> {
    if matches!(status, event::Status::Captured) {
        return None;
    }
    let Event::Keyboard(keyboard::Event::KeyPressed {
        key,
        modifiers,
        text,
        ..
    }) = event
    else {
        return None;
    };
    let named = match key.as_ref() {
        keyboard::Key::Named(Named::Enter) => Some(NamedKey::Enter),
        keyboard::Key::Named(Named::Backspace) => Some(NamedKey::Backspace),
        keyboard::Key::Named(Named::Escape) => Some(NamedKey::Escape),
        keyboard::Key::Named(Named::Space) => Some(NamedKey::Space),
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
    translate(named, typed.as_deref(), ctrl).map(Message::Key)
}
