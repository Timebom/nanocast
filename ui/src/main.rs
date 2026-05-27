use iced::{
    keyboard::{
        self,
        Key
    },
    widget::{
        row,
        column,
        container,
        scrollable,
        text,
        text_input,
        operation,
        image,
        svg,
        mouse_area,
        Id,
        Column,
    },
    window,
    Task,
    Alignment,
    Element,
    Color,
    Theme,
    Subscription,
    Length
};
use engine::{
    Action,
    ActionHandler,
    CalculatorEngine,
    Config,
    IndexBuilder,
    SearchEngine,
    SearchResult,
    ShortcutEngine,
    LauncherItem,
    create_special_item
};
use engine::shortcuts::CommandModeState;
use engine::hotkey;
use tracing_subscriber;
use std::sync::LazyLock;

static INPUT_ID: LazyLock<Id> = LazyLock::new(Id::unique);
static SCROLLABLE_ID: LazyLock<Id> = LazyLock::new(Id::unique);
static CONFIG: LazyLock<Config> = LazyLock::new(|| Config::load().unwrap_or_default());

#[derive(Debug, Clone)]
enum Message {
    QueryChanged(String),
    SelectUp,
    SelectDown,
    Execute,
    CopySelected,
    Hide,
    Show,
    PollHotkey,
    HotkeyTriggered,
    WindowIdFound(Option<window::Id>),
    RetryWindowId,
    WindowUnfocused,
    Tab,
    ClickSelect(usize),
    Ignored,
}

#[derive(Debug, Clone, PartialEq)]
enum InputMode {
    Normal,
    Command,
}

pub struct Launcher {
    search_engine: SearchEngine,
    query: String,
    results: Vec<SearchResult>,
    selected: usize,
    hotkey_handler: Option<hotkey::HotkeyHandler>,
    window_id: Option<window::Id>,
    is_visible: bool,
    shown_at: Option<std::time::Instant>,
    shortcut_engine: ShortcutEngine,
    pending_shortcut: Option<Action>,
    mode: InputMode,
    command_state: Option<CommandModeState>,
    calc_result: Option<String>,
}

impl Launcher {
    fn new() -> (Self, Task<Message>) {
        let items = IndexBuilder::new(CONFIG.clone())
            .build()
            .unwrap_or_default();

        let mut engine = SearchEngine::new();
        engine.set_items(items);

        let hotkey_handler = match hotkey::HotkeyHandler::new(&CONFIG) {
            Ok(h) => Some(h),
            Err(e) => {
                eprintln!("Failed to register hotkey: {}", e);
                None
            }
        };

        let mut app = Self {
            search_engine: engine,
            query: String::new(),
            results: Vec::new(),
            selected: 0,
            hotkey_handler: hotkey_handler,
            window_id: None,
            is_visible: false,
            shown_at: None,
            shortcut_engine: ShortcutEngine::new(&CONFIG),
            pending_shortcut: None,
            mode: InputMode::Normal,
            command_state: None,
            calc_result: None,
        };
        app.update_results();
        let init_task = window::oldest().map(Message::WindowIdFound);
        (app, init_task)
    }

    fn update(&mut self, message: Message) -> iced::Task<Message> {
        match message {
            Message::QueryChanged(new_query) => {
                match self.mode {
                    InputMode::Command => {
                        if let Some(cs) = self.command_state.as_mut() {
                            cs.set_active_value(new_query.clone());
                        }
                        self.query = new_query;
                        self.update_results();
                    }
                    InputMode::Normal => {
                        if new_query.starts_with('>') {
                            let after = new_query[1..].trim();
                            if after.contains(' ') {
                                if let Some(cs) = self.shortcut_engine.detect_command_mode(&new_query) {
                                    self.mode = InputMode::Command;
                                    let first_value = cs.active_value().to_string();
                                    self.command_state = Some(cs);
                                    self.query = first_value;
                                    self.update_results();
                                    return iced::Task::none();
                                }
                            }
                        } else {
                            if self.mode == InputMode::Command {
                                self.exit_command_mode();
                            }
                        }
                        self.query = new_query;
                        self.update_results();
                    }
                }
            }
            Message::SelectDown => {
                if !self.results.is_empty() {
                    self.selected = (self.selected + 1).min(self.results.len() - 1);
                    return self.scroll_to_selected();
                }
            }
            Message::SelectUp => {
                self.selected = self.selected.saturating_sub(1);
                return self.scroll_to_selected();
            }
            Message::Execute => {
                if self.mode == InputMode::Command {
                    if let Some(cs) = &self.command_state {
                        let action = cs.build_action();
                        if let Err(e) = ActionHandler::execute_shortcut(action) {
                            eprintln!("Command mode execution error: {}", e);
                        }
                    }
                } else if let Some(action) = self.pending_shortcut.clone() {
                    if let Err(e) = ActionHandler::execute_shortcut(action) {
                        eprintln!("Shortcut execution error: {}", e);
                    }
                } else if let Some(result) = self.results.get(self.selected) {
                    if result.item.id == "special:quit" {
                        if let Err(e) = ActionHandler::execute_shortcut(Action::Quit) {
                            eprintln!("Quit error: {}", e);
                        }
                        return iced::Task::done(Message::Hide);
                    }
                    if result.item.id.contains("calc:") {
                        let copy_action = ActionHandler::copy_action_for(&result.item);
                        if let Err(e) = ActionHandler::execute_shortcut(copy_action) {
                            eprintln!("Calc copy error: {}", e);
                        }
                        return iced::Task::done(Message::Hide);
                    }
                    println!("Executing: {}", result.item.title);
                    if let Err(e) = ActionHandler::execute(&result.item) {
                        eprintln!("Action execution error: {}", e);
                    }
                }
                return iced::Task::done(Message::Hide);
            }
            Message::CopySelected => {
                if let Some(result) = self.results.get(self.selected) {
                    let action = ActionHandler::copy_action_for(&result.item);
                    if let Err(e) = ActionHandler::execute_shortcut(action) {
                        eprintln!("Copy error: {}", e);
                    }
                }
            }
            Message::Hide => {
                self.is_visible = false;
                self.shown_at = None;
                self.query.clear();
                self.exit_command_mode();
                self.update_results();
                self.selected = 0;
                self.pending_shortcut = None;
                if let Some(id) = self.window_id {
                    #[cfg(target_os = "linux")]
                    {
                        return Task::batch([
                            window::set_level(id, window::Level::Normal),
                            window::minimize(id, true)
                        ]);
                    }
                    #[cfg(not(target_os = "linux"))]
                    {
                        return window::set_mode(id, window::Mode::Hidden);
                    }
                }
            }
            Message::Show => {
                self.is_visible = true;
                self.shown_at = Some(std::time::Instant::now());
                self.query.clear();
                self.exit_command_mode();
                self.update_results();
                self.selected = 0;
                self.pending_shortcut = None;
                if let Some(id) = self.window_id {
                    #[cfg(target_os = "linux")]
                    {
                        return Task::batch([
                            window::set_mode(id, window::Mode::Windowed),
                            window::set_level(id, window::Level::AlwaysOnTop),
                            window::minimize(id, false),
                            window::gain_focus(id),
                            operation::focus(INPUT_ID.clone()),
                        ]);
                    }
                    #[cfg(not(target_os = "linux"))]
                    {
                        return Task::batch([
                            window::set_mode(id, window::Mode::Windowed),
                            window::gain_focus(id),
                            operation::focus(INPUT_ID.clone()),
                        ]);
                    }
                };
            }
            Message::PollHotkey => {
                if let Some(handler) = &self.hotkey_handler {
                    if handler.try_recv().is_some() {
                        return iced::Task::done(Message::HotkeyTriggered);
                    }
                }
            }
            Message::HotkeyTriggered => {
                return iced::Task::done(Message::Show);
            }
            Message::WindowIdFound(id) => {
                if id.is_some() {
                    self.window_id = id;
                    if let Some(id) = self.window_id {
                        return window::set_mode(id, window::Mode::Hidden);
                    }
                } else {
                    // Retry after short delay
                    return Task::perform(
                        async {
                            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                        },
                        |_| Message::RetryWindowId,
                    );
                }
            }
            Message::RetryWindowId => {
                return window::oldest().map(Message::WindowIdFound);
            }
            Message::WindowUnfocused => {
                let too_soon = self.shown_at
                    .map(|t| t.elapsed() < std::time::Duration::from_millis(150)) // was 150ms
                    .unwrap_or(false);

                if self.is_visible && !too_soon {
                    return iced::Task::done(Message::Hide);
                }
            }
            Message::Tab => {
                if self.mode == InputMode::Command {
                    if let Some(cs) = self.command_state.as_mut() {
                        let moved = cs.tab_next();
                        if moved {
                            let val = cs.active_value().to_string();
                            self.query = val;
                            self.update_results();
                        }
                    }
                } else {
                    if let Some(result) = self.results.get(self.selected) {
                        if result.item.id.starts_with("shortcut:") {
                            let trigger = result.item.id.trim_start_matches("shortcut:");

                            let prefixed = format!("> {} ", trigger);
                            if let Some(cs) = self.shortcut_engine.detect_command_mode(&prefixed) {
                                self.mode = InputMode::Command;
                                let first_value = cs.active_value().to_string();
                                self.command_state = Some(cs);
                                self.query = first_value;
                                self.update_results();
                            }
                        }
                    }
                }
            }
            Message::ClickSelect(index) => {
                self.selected = index.min(self.results.len().saturating_sub(1));
            }
            Message::Ignored => {}
        }
        iced::Task::none()
    }

    fn exit_command_mode(&mut self) {
        self.mode = InputMode::Normal;
        self.command_state = None;
    }

    fn scroll_to_selected(&self) -> Task<Message> {
        // Window 500px - search input (~80px) - padding(~40px) = ~380px visible
        // 380px / 70px per item ~= 5 visible items
        const ITEM_HEIGHT: f32 = 70.0;
        const PADDING_TOP: f32 = 16.0;
        const INPUT_AREA: f32 = 88.0;
        const FOOTER_HEIGHT: f32 = 50.0;

        let scrollable_height = CONFIG.window.height - INPUT_AREA - FOOTER_HEIGHT - PADDING_TOP * 2.0;
        if scrollable_height <= 0.0 || self.results.is_empty() {
            return iced::Task::none();
        }

        let visible_items = (scrollable_height / ITEM_HEIGHT).floor() as usize;
        let total_results = self.results.len();
        let scroll_y = if self.selected < visible_items {
            0.0
        } else if self.selected < visible_items.saturating_sub(2) {
            0.0
        } else if self.selected > total_results.saturating_sub(visible_items.saturating_sub(2)) {
            (total_results as f32 * ITEM_HEIGHT - scrollable_height).max(0.0)
        } else {
            let target_position = (self.selected as f32 * ITEM_HEIGHT) - (scrollable_height * 0.25).max(40.0);
            target_position.max(0.0)
        };

        let max_scroll = (total_results as f32 * ITEM_HEIGHT - scrollable_height).max(0.0);
        let final_scroll_y = scroll_y.min(max_scroll);

        operation::scroll_to(
            SCROLLABLE_ID.clone(),
            scrollable::AbsoluteOffset {
                x: 0.0,
                y: final_scroll_y,
            },
        )
    }

    fn update_results(&mut self) {
        self.calc_result = None;

        if self.mode == InputMode::Command {
            if let Some(cs) = &self.command_state {
                let hint = cs.slot_hint();
                let tab_tip = if cs.slots.len() > 1 && cs.active_slot < cs.slots.len() - 1 {
                    format!("Tab -> next slot | Enter to run | {}", hint)
                } else {
                    format!("Enter to run | {}", hint)
                };
                self.results = vec![SearchResult {
                    item: LauncherItem {
                        id: format!("cmd:{}", cs.trigger),
                        title: format!("⌘ {} - {}", cs.shortcut_name, cs.trigger),
                        subtitle: Some(tab_tip),
                        ..Default::default()
                    },
                    score: 100.0,
                }]
            }
            return;
        }

        let q = self.query.trim();
        if !q.is_empty() && CalculatorEngine::looks_like_math(q) {
            if let Some(result_str) = CalculatorEngine::evaluate(q) {
                self.calc_result = Some(result_str.clone());
                let calc_item = SearchResult {
                    item: LauncherItem {
                        id: format!("calc:{}", q),
                        title: format!("= {}", result_str),
                        subtitle: Some(format!("{} -> {}", q, result_str)),
                        path: Some(format!("{}", result_str)),
                        icon_path: None,
                        item_type: engine::ItemType::Command,
                        tags: vec!["calc".into()],
                    },
                    score: 100.0
                };
                let mut rest = self.search_engine.search(q);
                rest.retain(|r| !r.item.id.starts_with("calc:"));
                self.results = std::iter::once(calc_item).chain(rest).collect();
                self.selected = 0;
                return;
            }
        }

        if let Some(shortcut) = self.shortcut_engine.detect(&self.query) {
            self.results = vec![SearchResult {
                item: LauncherItem {
                    id: format!("shortcut:{}", shortcut.trigger),
                    title: format!("> {} - {}", shortcut.name, shortcut.trigger),
                    subtitle: Some("Press Enter to run | Tab for command mode (slot filling)".into()),
                    ..Default::default()
                },
                score: 100.0,
            }];
            self.pending_shortcut = Some(shortcut.action);
        } else {
            self.pending_shortcut = None;
            if self.query.trim().is_empty() {
                self.results = self.search_engine.search("");
            } else if let Some(special) = create_special_item(&self.query) {
                self.results = vec![SearchResult {
                    item: special,
                    score: 100.0,
                }];
            } else {
                if self.query.starts_with('>') {
                    let prefix = self.query[1..].trim();
                    let matching = self.shortcut_engine.matching_shortcuts(prefix);
                    if !matching.is_empty() {
                        self.results = matching
                            .iter()
                            .map(|sc| SearchResult {
                                item: LauncherItem {
                                    id: format!("shortcut:{}", sc.trigger),
                                    title: format!("> {} ({})", sc.name, sc.trigger),
                                    subtitle: Some(format!("Tab to fill slots · action: {}", sc.action_type)),
                                    ..Default::default()
                                },
                                score: 90.0,
                            })
                            .collect();
                        self.selected = 0;
                        return;
                    }
                }
                self.results = self.search_engine.search(&self.query);
            }
        }
        self.selected = 0;
    }

    fn view(&self) -> Element<'_, Message> {
        let (placeholder, input_label) = match (&self.mode, &self.command_state) {
            (InputMode::Command, Some(cs)) => {
                let slot = cs.slots.get(cs.active_slot);
                let ph = slot.map(|s| s.name.clone()).unwrap_or_else(|| "value".into());

                let label = format!(
                    "⌘ {} > {} ({}/{})",
                    cs.shortcut_name,
                    ph,
                    cs.active_slot + 1,
                    cs.slots.iter().len()
                );
                (ph, Some(label))
            }
            _ => ("Search or type > for commands...".into(), None),
        };

        let input = text_input(&placeholder, &self.query)
            .id(INPUT_ID.clone())
            .on_input(Message::QueryChanged)
            .size(24)
            .padding(16)
            .style(|theme, status| text_input::Style {
                background: iced::Background::Color(iced::Color::from_rgb(0.12, 0.12, 0.14)),
                ..text_input::default(theme, status)
            });

        let mode_badge: Element<_> = if let Some(label) = input_label {
            container(
                text(label)
                    .size(14)
                    .style(|_| text::Style {
                        color: Some(Color::from_rgb(0.4, 0.75, 1.0)),
                    }),
            )
            .padding([4, 12])
            .style(|_| container::Style {
                background: Some(iced::Background::Color(iced::Color::from_rgba(0.2, 0.5, 0.9, 0.18))),
                border: iced::Border {
                    radius: 6.0.into(),
                    ..Default::default()
                },
                ..Default::default()
            })
            .into()
        } else {
            text("").into()
        };

        let results_list: Element<_> = if self.results.is_empty() {
            text("No results").size(16).into()
        } else {
            self.results
                .iter()
                .enumerate()
                .take(20)
                .fold(Column::new().spacing(4), |col, (i, result)| {
                    let is_selected = i == self.selected;

                    if i == 0 {
                        if let Some(ref calc_val) = self.calc_result {
                            let expr_text = self.query.trim();
                            let bg_color = if is_selected {
                                Color::from_rgba(0.15, 0.55, 0.35, 0.85)
                            } else {
                                Color::from_rgba(0.10, 0.38, 0.24, 0.75)
                            };
                            let card = container(
                                column![
                                    text(calc_val.as_str())
                                        .size(36)
                                        .style(|_| text::Style {
                                            color: Some(Color::from_rgb(0.55, 1.0, 0.72)),
                                        }),
                                    text(format!("{} = {}", expr_text, calc_val))
                                        .size(13)
                                        .style(|_| text::Style {
                                            color: Some(Color::from_rgba(0.7, 0.95, 0.8, 0.7)),
                                        }),
                                    text(if is_selected { "⏎ copied to clipboard" } else { "Enter to copy result" })
                                        .size(11)
                                        .style(|_| text::Style {
                                            color: Some(Color::from_rgba(0.55, 0.85, 0.65, 0.55)),
                                        }),
                                ]
                                .spacing(4),
                            )
                            .padding([14, 18])
                            .width(Length::Fill)
                            .style(move |_| container::Style {
                                background: Some(iced::Background::Color(bg_color)),
                                border: iced::Border {
                                    radius: 10.0.into(),
                                    width: if is_selected { 1.5 } else { 1.0 },
                                    color: if is_selected {
                                        Color::from_rgba(0.3, 0.9, 0.55, 0.6)
                                    } else {
                                        Color::from_rgba(0.2, 0.65, 0.4, 0.35)
                                    },
                                },
                                ..Default::default()
                            });
                            return col.push(card);
                        }
                    }

                    let icon_widget: Element<_> = match &result.item.icon_path {
                        Some(path) if path.ends_with(".svg") => svg(path)
                            .width(32)
                            .height(32)
                            .into(),
                        Some(path) => image(path)
                            .width(32)
                            .height(32)
                            .into(),
                        None => container(text(""))
                            .width(0)
                            .height(0)
                            .into(),
                    };

                    let subtitle = if let Some(sub) = &result.item.subtitle {
                        text(sub).size(14).style(|_| text::Style {
                            color: Some(Color::from_rgb(0.65, 0.65, 0.7)),
                        })
                    } else {
                        text("")
                    };

                    let item_col = column![
                        text(&result.item.title).size(18),
                        subtitle
                    ]
                    .spacing(2);

                    let content = row![icon_widget, item_col]
                        .spacing(12)
                        .align_y(Alignment::Center);

                    let bg_color = if is_selected {
                        Color::from_rgb(0.25, 0.45, 0.75)
                    } else {
                        Color::TRANSPARENT
                    };

                    let item = container(content)
                        .padding(12)
                        .width(Length::Fill)
                        .style(move |_theme| container::Style {
                            background: Some(iced::Background::Color(bg_color)),
                            border: iced::Border {
                                radius: 8.0.into(),
                                ..Default::default()
                            },
                            ..Default::default()
                        });

                    let clickable_item = mouse_area(item)
                        .on_press(Message::ClickSelect(i))
                        .on_double_click(Message::Execute);

                    col.push(clickable_item)
                })
                .into()
        };

        let body = scrollable(results_list)
            .id(SCROLLABLE_ID.clone())
            .spacing(4)
            .height(Length::Fill);

        let footer_hints: Element<_> = if self.mode == InputMode::Command {
            text("Tab -> next slot | Enter -> execute | Esc cancel")
                .size(14)
                .style(|_| text::Style {
                    color: Some(Color::from_rgb(0.6, 0.6, 0.65))
                })
                .into()
        } else if self.calc_result.is_some() {
            text("↑↓ select | Enter -> copy result | Ctrl+C copy path | Esc hide")
                .size(14)
                .style(|_| text::Style {
                    color: Some(Color::from_rgb(0.45, 0.8, 0.6))
                })
                .into()
        } else {
            text("↑↓ select  |  Enter execute  | Ctrl+C copy |  Tab → command mode  |  Esc hide")
                .size(14)
                .style(|_| text::Style {
                    color: Some(Color::from_rgb(0.6, 0.6, 0.65))
                })
                .into()
        };

        let footer = row![
            mode_badge,
            iced::widget::space::horizontal().width(Length::Fill),
            footer_hints,
        ]
            .align_y(Alignment::Center)
            .width(Length::Fill)
            .spacing(12);


        container(
            column![
                input,
                body,
                footer
            ]
                .spacing(14)
                .padding(16)
                .width(Length::Fill)
        )
        .width(Length::Fixed(CONFIG.window.width))
        .height(Length::Fixed(CONFIG.window.height))
        .clip(true)
        .style(|_theme| container::Style {
            background: Some(iced::Background::Color(iced::Color::from_rgba(0.08, 0.08, 0.10, 0.99))),
            border: iced::Border {
                radius: 20.0.into(),
                width: 1.0,
                color: Color::from_rgba(1.0, 1.0, 1.0, 0.1)
            },
            ..Default::default()
        })
        .into()
    }

    fn subscription(&self) -> Subscription<Message> {
        let keyboard_sub = keyboard::listen().map(|event| {
            match event {
                keyboard::Event::KeyPressed { key, modifiers, .. } => {
                    match key {
                        Key::Named(keyboard::key::Named::ArrowDown) => Message::SelectDown,
                        Key::Named(keyboard::key::Named::ArrowUp) => Message::SelectUp,
                        Key::Named(keyboard::key::Named::Enter) => Message::Execute,
                        Key::Named(keyboard::key::Named::Escape) => Message::Hide,
                        Key::Named(keyboard::key::Named::Tab) => Message::Tab,
                        Key::Character(ref c) if c.as_str() == "c" && modifiers.control() => {
                            Message::CopySelected
                        }
                        _ => Message::Ignored,
                    }
                }
                _ => Message::Ignored,
            }
        });

        let hotkey_sub = iced::time::every(std::time::Duration::from_millis(50))
            .map(|_| Message::PollHotkey);

        let window_sub = window::events().map(|(_, event)| {
            match event {
                window::Event::Unfocused => Message::WindowUnfocused,
                _ => Message::Ignored,
            }
        });

        Subscription::batch([keyboard_sub, hotkey_sub, window_sub])
    }
}

fn launcher_theme(_state: &Launcher) -> Theme {
    Theme::custom("Transparent", iced::theme::Palette {
        background: Color::TRANSPARENT,
        ..Theme::Dark.palette()
    })
}

fn main() -> iced::Result {
    tracing_subscriber::fmt::init();

    iced::application(Launcher::new, Launcher::update, Launcher::view)
        .title("Nanocast")
        .theme(launcher_theme)
        .subscription(Launcher::subscription)
        .window(iced::window::Settings {
            size: iced::Size::new(CONFIG.window.width, CONFIG.window.height),
            decorations: false,
            transparent: true,
            level: iced::window::Level::AlwaysOnTop,
            resizable: false,
            position: iced::window::Position::Centered,
            visible: false,
            ..Default::default()
        })
        .run()
}
