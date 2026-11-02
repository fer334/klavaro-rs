// SPDX-License-Identifier: GPL-3.0-or-later
//! Interfaz gráfica en iced (arquitectura Elm: estado, mensajes, `update` y `view`).
//!
//! Toda la lógica vive en [`crate::tutor`]; este módulo sólo traduce eventos y dibuja.

use std::time::{Instant, SystemTime, UNIX_EPOCH};

use iced::keyboard::{self, key::Named};
use iced::widget::text::Span;
use iced::widget::{
    Space, button, canvas, column, container, pick_list, rich_text, row, scrollable, span, text,
    tooltip,
};
use iced::{Alignment, Color, Element, Event, Font, Length, Subscription, Task, Theme, event};

use klavaro_core::Module;
use klavaro_core::generators::basic::MAX_LESSONS;
use klavaro_core::ranking::RankingEntry;
use klavaro_core::tracker::KeyTracker;
use klavaro_i18n::Translator;
use klavaro_store::date::format_date_time;

use crate::assets;
use crate::exercise::{ExerciseSpec, comment_text, module_name, notice_text, recorded_text};
use crate::plot::Plot;
use crate::profile::Profile;
use crate::progress::{Chart, ChartQuery, Metric};
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
}

/// Estado de la pantalla de progreso.
struct ProgressView {
    module: Module,
    metric: Metric,
    /// Lección del curso básico (0 = todas).
    lesson: u16,
    chart: Chart,
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
    tr: Translator,
    clock: Instant,
    /// Errores y tiempos por tecla del teclado elegido. Durante un ejercicio, el tutor tiene
    /// la copia al día ([`App::leave_tutor`] la recupera).
    tracker: KeyTracker,
}

#[derive(Debug, Clone)]
enum Message {
    Start(Module),
    InterfaceSelected(UiLanguage),
    LanguageSelected(String),
    KeyboardSelected(String),
    Key(Input),
    Back,
    Restart,
    LessonDelta(i32),
    ToggleTheme,
    /// Activa o desactiva la sesión especial de Adaptabilidad.
    ToggleSpecial,
    /// Abre el progreso del módulo (o del último usado).
    Progress(Option<Module>),
    ProgressMetric(Metric),
    ProgressLesson(i32),
    Top10,
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
        let ui = klavaro_i18n::resolve(config.language.iter().chain(&system).map(String::as_str));
        let tr = Translator::new(ui);
        let text_language = config
            .text_language
            .clone()
            .unwrap_or_else(|| ui.to_owned());
        let error = keyboards.is_empty().then(|| {
            let dir = assets::dir().display().to_string();
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
            clock: Instant::now(),
            profile,
            tracker: KeyTracker::new(),
        };
        app.load_tracker();
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

    /// Lee el rastreo por tecla del teclado elegido.
    fn load_tracker(&mut self) {
        self.tracker = self.profile.tracker(&self.keyboard).unwrap_or_else(|e| {
            self.error = Some(e.to_string());
            KeyTracker::new()
        });
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
            Ok(r) => self.note = recorded_text(&self.tr, r),
            Err(e) => self.error = Some(e.to_string()),
        }
        if let Err(e) = self.profile.save_tracker(&self.keyboard, t.tracker()) {
            self.error = Some(e.to_string());
        }
    }

    fn title(&self) -> String {
        match &self.screen {
            Screen::Menu => "klavaro-rs".to_owned(),
            Screen::Tutor(t) => {
                format!("klavaro-rs · {}", module_name(&self.tr, t.spec().module))
            }
            Screen::Progress(_) => format!("klavaro-rs · {}", self.tr.tr("menu-progress")),
            Screen::Top10(_) => format!("klavaro-rs · {}", self.tr.tr("menu-top10")),
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
            Message::InterfaceSelected(l) => {
                self.tr = Translator::new(l.code);
                self.profile.config.language = Some(l.code.to_owned());
                self.persist();
                Ok(())
            }
            Message::LanguageSelected(l) => {
                self.language = l;
                self.persist();
                if matches!(self.screen, Screen::Top10(_)) {
                    self.show_top10();
                }
                Ok(())
            }
            Message::KeyboardSelected(k) => {
                self.leave_tutor();
                self.keyboard = k;
                self.persist();
                self.load_tracker();
                Ok(())
            }
            Message::ToggleSpecial => match &mut self.screen {
                Screen::Tutor(t) => {
                    self.note = None;
                    t.toggle_special()
                }
                Screen::Menu | Screen::Progress(_) | Screen::Top10(_) => Ok(()),
            },
            Message::ToggleTheme => {
                self.dark = !self.dark;
                self.persist();
                Ok(())
            }
            Message::Back => {
                self.leave_tutor();
                self.screen = Screen::Menu;
                Ok(())
            }
            Message::Restart => match &mut self.screen {
                Screen::Tutor(t) => {
                    self.note = None;
                    t.restart()
                }
                Screen::Menu | Screen::Progress(_) | Screen::Top10(_) => Ok(()),
            },
            Message::LessonDelta(delta) => match &mut self.screen {
                Screen::Tutor(t) => {
                    let lesson = i32::from(t.spec().lesson) + delta;
                    let r = t.set_lesson(u16::try_from(lesson.max(1)).unwrap_or(1));
                    self.note = None;
                    self.sync_lesson();
                    r
                }
                Screen::Menu | Screen::Progress(_) | Screen::Top10(_) => Ok(()),
            },
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
                self.show_progress(module, metric, self.lesson);
                Ok(())
            }
            Message::ProgressMetric(metric) => {
                if let Screen::Progress(v) = &self.screen {
                    self.show_progress(v.module, metric, v.lesson);
                }
                Ok(())
            }
            Message::ProgressLesson(delta) => {
                if let Screen::Progress(v) = &self.screen {
                    let lesson = (i32::from(v.lesson) + delta).clamp(0, i32::from(MAX_LESSONS));
                    let lesson = u16::try_from(lesson).unwrap_or(0);
                    self.show_progress(v.module, v.metric, lesson);
                }
                Ok(())
            }
            Message::Top10 => {
                self.leave_tutor();
                self.show_top10();
                Ok(())
            }
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

    /// Abre (o actualiza) el gráfico de progreso.
    fn show_progress(&mut self, module: Module, metric: Metric, lesson: u16) {
        // Fluidez y puntaje sólo existen en el módulo de Fluidez.
        let metric = if Metric::for_module(module).contains(&metric) {
            metric
        } else {
            Metric::Accuracy
        };
        let query = ChartQuery {
            module,
            metric,
            lesson,
            keyboard: self.keyboard.clone(),
            language: self.language.clone(),
        };
        let chart = if metric.per_key() {
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
            Screen::Progress(_) | Screen::Top10(_) => {
                match input {
                    Input::Restart => self.screen = Screen::Menu,
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
                        self.show_progress(module, Metric::Accuracy, self.lesson);
                        return Ok(());
                    }
                    Input::Char('t' | 'T') => {
                        self.show_top10();
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

    fn view_menu<'a>(&'a self) -> Element<'a, Message> {
        let tr = &self.tr;
        let modules = column(Module::ALL.iter().enumerate().map(|(i, &m)| {
            let tip = format!("{}-tip", crate::exercise::module_key(m));
            button(
                column![
                    text(format!("{} - {}", i + 1, module_name(tr, m))).size(18),
                    text(tr.tr(&tip)).size(12),
                ]
                .spacing(2),
            )
            .width(Length::Fixed(320.0))
            .padding(10)
            .on_press(Message::Start(m))
            .into()
        }))
        .spacing(8);
        let languages = ui_languages();
        let current = languages.iter().find(|l| l.code == tr.code()).cloned();
        let interface = pick_list(languages, current, Message::InterfaceSelected);
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
        let colors = button(text(tr.tr(if self.dark {
            "menu-colors-light"
        } else {
            "menu-colors-dark"
        })))
        .on_press(Message::ToggleTheme);
        let labeled = |label: String, widget: Element<'a, Message>| {
            row![text(label).width(Length::Fixed(110.0)), widget]
                .spacing(8)
                .align_y(Alignment::Center)
        };
        column![
            text(tr.tr("app-name")).size(36),
            text(tr.tr("app-tagline")).size(16),
            modules,
            labeled(tr.tr("menu-interface-language"), interface.into()),
            labeled(tr.tr("menu-text-language"), language.into()),
            text(tr.tr("menu-text-language-tip")).size(12),
            labeled(tr.tr("menu-keyboard"), keyboard.into()),
            row![
                button(text(tr.tr("menu-progress"))).on_press(Message::Progress(None)),
                button(text(tr.tr("menu-top10"))).on_press(Message::Top10),
                colors,
            ]
            .spacing(8),
            text(tr.tr("menu-shortcuts")).size(13),
        ]
        .spacing(16)
        .into()
    }

    fn view_tutor<'a>(&'a self, tutor: &'a Tutor) -> Element<'a, Message> {
        let tr = &self.tr;
        let spec = tutor.spec();
        let mut header = row![
            button(text(format!("← {}", tr.tr("menu-back")))).on_press(Message::Back),
            text(format!(
                "{} · {} · {}",
                module_name(tr, spec.module),
                spec.language,
                self.keyboard
            ))
            .size(18),
            button(text(tr.tr("tutor-new"))).on_press(Message::Restart),
            button(text(tr.tr("menu-progress"))).on_press(Message::Progress(Some(spec.module))),
        ]
        .spacing(12)
        .align_y(Alignment::Center);
        // Como el original, el botón de la sesión especial aparece cuando conviene.
        if tutor.special_suggested().is_some() || tutor.special() {
            let label = tr.tr(if tutor.special() {
                "tutor-special-on"
            } else {
                "tutor-special"
            });
            header = header.push(tooltip(
                button(text(label)).on_press(Message::ToggleSpecial),
                container(text(tr.tr("tutor-special-tip")).size(13)).padding(6),
                tooltip::Position::Bottom,
            ));
        }
        if spec.module == Module::Basic {
            header = header.push(
                row![
                    button(text("−")).on_press(Message::LessonDelta(-1)),
                    text(tr.tr_args("tutor-lesson-n", &[("lesson", &spec.lesson.to_string())])),
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
            let line = |label: &str, key: &str, v: f64| {
                let value = tr.tr_args(key, &[("value", &format!("{v:.1}"))]);
                text(format!("{} {value}", tr.tr(label)))
            };
            let mut results = column![
                line(
                    "stats-elapsed",
                    "stats-seconds",
                    o.stats.elapsed.as_secs_f64()
                ),
                line("stats-accuracy", "stats-percent", o.stats.accuracy)
                    .color(mark(o.check.accuracy)),
                line("stats-wpm", "stats-wpm-value", o.stats.wpm).color(mark(o.check.speed)),
            ]
            .spacing(4);
            if let Some(ok) = o.check.fluidity {
                results = results.push(
                    text(format!(
                        "{} {:.1}",
                        tr.tr("stats-fluidity"),
                        o.stats.fluidity
                    ))
                    .color(mark(ok)),
                );
            }
            if let Some(note) = &self.note {
                results = results.push(text(note.clone()).size(14));
            }
            results
                .push(text(comment_text(tr, o.comment)).size(18))
                .push(text(tr.tr("tutor-end")).size(13))
                .into()
        } else {
            let (done, total) = tutor.progress();
            let key = if spec.module == Module::Fluidity {
                "tutor-hint-backspace"
            } else {
                "tutor-hint"
            };
            let (done, total) = (done.to_string(), total.to_string());
            text(tr.tr_args(key, &[("done", &done), ("total", &total)]))
                .size(13)
                .into()
        };

        column![header, exercise, footer].spacing(12).into()
    }
    fn view_progress<'a>(&'a self, v: &'a ProgressView) -> Element<'a, Message> {
        let tr = &self.tr;
        let choice = |label: String, active: bool, msg: Message| {
            button(text(label))
                .style(if active {
                    button::primary
                } else {
                    button::secondary
                })
                .on_press(msg)
        };
        let modules = row(Module::ALL.iter().map(|&m| {
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

        // Igual que el título de la ventana del original: teclado o idioma según el módulo.
        let scope = match v.module {
            Module::Basic | Module::Adaptability => self.keyboard.clone(),
            Module::Velocity | Module::Fluidity => self.language.clone(),
        };
        let mut filters = row![text(scope).size(14)]
            .spacing(8)
            .align_y(Alignment::Center);
        if v.module == Module::Basic {
            let lesson = if v.lesson == 0 {
                tr.tr("progress-all-lessons")
            } else {
                tr.tr_args("tutor-lesson-n", &[("lesson", &v.lesson.to_string())])
            };
            filters = filters.push(
                row![
                    button(text("−")).on_press(Message::ProgressLesson(-1)),
                    text(lesson),
                    button(text("+")).on_press(Message::ProgressLesson(1)),
                ]
                .spacing(6)
                .align_y(Alignment::Center),
            );
        }

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
                "progress-sessions",
                &[
                    ("count", &chart.points.len().to_string()),
                    ("first", &format_date_time(first.at)),
                    ("last", &format_date_time(last.at)),
                ],
            ),
            _ => tr.tr("progress-empty"),
        };
        let mut info = row![text(summary).size(13)].spacing(16);
        if let Some(goal) = chart.goal {
            info = info.push(
                text(tr.tr_args("progress-goal", &[("value", &format!("{goal:.0}"))])).size(13),
            );
        }
        let plot = Plot {
            chart,
            unit: metric_unit(tr, v.metric),
        };

        column![
            row![
                button(text(format!("← {}", tr.tr("menu-back")))).on_press(Message::Back),
                text(tr.tr("menu-progress")).size(22),
                Space::new().width(Length::Fill),
                button(text(tr.tr("menu-top10"))).on_press(Message::Top10),
            ]
            .spacing(12)
            .align_y(Alignment::Center),
            modules,
            metrics,
            filters,
            canvas(plot).width(Length::Fill).height(Length::Fill),
            info,
        ]
        .spacing(10)
        .into()
    }

    fn view_top10<'a>(&'a self, entries: &'a [RankingEntry]) -> Element<'a, Message> {
        let tr = &self.tr;
        let cell = |s: String, w: f32| text(s).width(Length::Fixed(w));
        let header = row![
            cell(tr.tr("ranking-place"), 32.0),
            cell(tr.tr("ranking-name"), 240.0),
            cell(tr.tr("stats-score"), 70.0),
            cell(tr.tr("progress-accuracy"), 90.0),
            cell(tr.tr("progress-speed"), 90.0),
            cell(tr.tr("progress-fluidity"), 90.0),
            cell(tr.tr("ranking-chars"), 90.0),
            cell(tr.tr("ranking-when"), 150.0),
        ]
        .spacing(8);
        let mut table = column![header].spacing(6);
        if entries.is_empty() {
            table = table.push(text(tr.tr("ranking-empty")));
        }
        for (i, e) in entries.iter().enumerate() {
            table = table.push(
                row![
                    cell(format!("{}", i + 1), 32.0),
                    cell(e.name.clone(), 240.0),
                    cell(format!("{:.3}", e.score), 70.0),
                    cell(format!("{:.1}", e.accuracy), 90.0),
                    cell(format!("{:.1}", e.wpm), 90.0),
                    cell(format!("{:.1}", e.fluidity), 90.0),
                    cell(e.chars.to_string(), 90.0),
                    cell(format_date_time(e.when), 150.0),
                ]
                .spacing(8),
            );
        }
        let language = pick_list(
            self.languages.clone(),
            Some(self.language.clone()),
            Message::LanguageSelected,
        );
        column![
            row![
                button(text(format!("← {}", tr.tr("menu-back")))).on_press(Message::Back),
                text(tr.tr("menu-top10")).size(22),
                Space::new().width(Length::Fill),
                button(text(tr.tr("menu-progress")))
                    .on_press(Message::Progress(Some(Module::Fluidity))),
            ]
            .spacing(12)
            .align_y(Alignment::Center),
            row![text(tr.tr("ranking-local")).size(16), language]
                .spacing(12)
                .align_y(Alignment::Center),
            scrollable(table).height(Length::Fill),
        ]
        .spacing(12)
        .into()
    }
}

/// Nombre de un gráfico con su unidad, como en el original («Precisión (%)»).
fn metric_label(tr: &Translator, m: Metric) -> String {
    let unit = match m {
        Metric::Accuracy | Metric::Fluidity => "(%)".to_owned(),
        Metric::Speed => tr.tr("progress-wpm-unit"),
        Metric::Score => "(0-10)".to_owned(),
        // «Touch times (s)» ya trae la unidad.
        Metric::Errors | Metric::TouchTimes => return tr.tr(m.label_key()),
    };
    format!("{} {unit}", tr.tr(m.label_key()))
}

/// Unidad del valor señalado en el gráfico.
fn metric_unit(tr: &Translator, m: Metric) -> String {
    match m {
        Metric::Accuracy | Metric::Fluidity => " %".to_owned(),
        Metric::Speed => format!(" {}", tr.tr("progress-wpm-unit").trim_matches(['(', ')'])),
        Metric::Score | Metric::Errors => String::new(),
        Metric::TouchTimes => " s".to_owned(),
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
