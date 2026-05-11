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
};
use engine::{
    ActionHandler,
    Config,
    IndexBuilder,
    SearchEngine,
    SearchResult,
    create_special_item
};
use tracing_subscriber;
use std::sync::LazyLock;

mod hotkey;

static INPUT_ID: LazyLock<Id> = LazyLock::new(Id::unique);
static SCROLLABLE_ID: LazyLock<Id> = LazyLock::new(Id::unique);
static CONFIG: LazyLock<Config> = LazyLock::new(|| Config::load().unwrap_or_default());

#[derive(Debug, Clone)]
enum Message {
    QueryChanged(String),
    SelectUp,
    SelectDown,
    Execute,
    Hide,
    Show,
    PollHotkey,
    HotkeyTriggered,
    WindowIdFound(Option<window::Id>),
    Ignored,
}

pub struct Launcher {
    search_engine: SearchEngine,
    query: String,
    results: Vec<SearchResult>,
    selected: usize,
    hotkey_handler: Option<hotkey::HotkeyHandler>,
    window_id: Option<window::Id>,
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
        };
        app.update_results();
        let init_task = window::oldest().map(Message::WindowIdFound);
        (app, init_task)
    }

    fn update(&mut self, message: Message) -> iced::Task<Message> {
        match message {
            Message::QueryChanged(new_query) => {
                self.query = new_query;
                self.update_results();
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
                if let Some(result) = self.results.get(self.selected) {
                    println!("Executing: {}", result.item.title);
                    if let Err(e) = ActionHandler::execute(&result.item) {
                        eprintln!("Action Execution error: {}", e);
                    }
                }
                return iced::Task::done(Message::Hide);
            }
            Message::Hide => {
                self.query.clear();
                self.update_results();
                self.selected = 0;
                if let Some(id) = self.window_id {
                    return window::set_mode(id, window::Mode::Hidden);
                }
            }
            Message::Show => {
                self.query.clear();
                self.update_results();
                self.selected = 0;
                if let Some(id) = self.window_id {
                    return Task::batch([
                        window::set_mode(id, window::Mode::Windowed),
                        window::gain_focus(id),
                        operation::focus(INPUT_ID.clone()),
                    ]);
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
                self.window_id = id;
                if let Some(id) = self.window_id {
                    return window::set_mode(id, window::Mode::Hidden);
                }
            }
            Message::Ignored => {}
        }
        iced::Task::none()
    }

    fn scroll_to_selected(&self) -> Task<Message> {
        // Window 500px - search input (~80px) - padding(~40px) = ~380px visible
        // 380px / 70px per item ~= 5 visible items
        let visible_items = 5usize;

        let scroll_y = if self.selected < visible_items {
            0.0
        } else {
            (self.selected - visible_items + 1) as f32 * 70.0
        };

        // let offset = (self.selected as f32 * 70.0).max(0.0);
        operation::scroll_to(
            SCROLLABLE_ID.clone(),
            scrollable::AbsoluteOffset { x: 0.0, y: scroll_y },
        )
    }

    fn update_results(&mut self) {
        if self.query.trim().is_empty() {
            self.results = self.search_engine.search("");
        } else if let Some(special) = create_special_item(&self.query) {
            self.results = vec![SearchResult {
                item: special,
                score: 100.0,
            }];
        } else {
            self.results = self.search_engine.search(&self.query);
        }
        self.selected = 0;
    }

    fn view(&self) -> Element<'_, Message> {
        let input = text_input("Search...", &self.query)
            .id(INPUT_ID.clone())
            .on_input(Message::QueryChanged)
            .size(24)
            .padding(16)
            .style(|theme, status| text_input::Style {
                background: iced::Background::Color(iced::Color::from_rgb(0.12, 0.12, 0.14)),
                ..text_input::default(theme, status)
            });

        let results_list: Element<_> = if self.results.is_empty() {
            text("No results").size(16).into()
        } else {
            self.results
                .iter()
                .enumerate()
                .take(20)
                .fold(Column::new().spacing(4), |col, (i, result)| {
                    let is_selected = i == self.selected;

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

                    let item_row = row![
                        icon_widget,
                        text(&result.item.title).size(18),
                    ]
                    .spacing(12)
                    .align_y(Alignment::Center);

                    let subtitle = if let Some(sub) = &result.item.subtitle {
                        text(sub).size(14).style(|_| text::Style {
                            color: Some(Color::from_rgb(0.65, 0.65, 0.7)),
                        })
                    } else {
                        text("")
                    };

                    let content = column![item_row, subtitle].spacing(2);

                    let bg_color = if is_selected {
                        Color::from_rgb(0.25, 0.45, 0.75)
                    } else {
                        Color::TRANSPARENT
                    };

                    let item = container(content)
                        .padding(12)
                        .width(iced::Length::Fill)
                        .style(move |_theme| container::Style {
                            background: Some(iced::Background::Color(bg_color)),
                            border: iced::Border {
                                radius: 8.0.into(),
                                ..Default::default()
                            },
                            ..Default::default()
                        });
                    col.push(item)
                })
                .into()
        };

        container(
            column![input, scrollable(results_list).id(SCROLLABLE_ID.clone()).spacing(4)]
                .spacing(16)
                .padding(20)
                .width(iced::Length::Fill),
        )
        .width(iced::Length::Fixed(700.0))
        .height(iced::Length::Fixed(500.0))
        .style(|_theme| container::Style {
            background: Some(iced::Background::Color(iced::Color::from_rgba(0.08, 0.08, 0.10, 0.97))),
            border: iced::Border {
                radius: 20.0.into(),
                width: 1.0,
                color: Color::from_rgba(1.0, 1.0, 1.0, 0.1)
            },
            shadow: iced::Shadow {
                color: Color::BLACK,
                offset: iced::Vector::new(0.0, 10.0),
                blur_radius: 30.0,
            },
            ..Default::default()
        })
        .into()
    }

    fn subscription(&self) -> Subscription<Message> {
        let keyboard_sub = keyboard::listen().map(|event| {
            match event {
                keyboard::Event::KeyPressed { key, .. } => {
                    match key {
                        Key::Named(keyboard::key::Named::ArrowDown) => Message::SelectDown,
                        Key::Named(keyboard::key::Named::ArrowUp) => Message::SelectUp,
                        Key::Named(keyboard::key::Named::Enter) => Message::Execute,
                        Key::Named(keyboard::key::Named::Escape) => Message::Hide,
                        _ => Message::Ignored,
                    }
                }
                _ => Message::Ignored,
            }
        });

        let hotkey_sub = iced::time::every(std::time::Duration::from_millis(50))
            .map(|_| Message::PollHotkey);

        Subscription::batch([keyboard_sub, hotkey_sub])
    }
}

// fn launcher_theme(_state: &Launcher) -> Theme {
//     Theme::Dark
// }

fn main() -> iced::Result {
    tracing_subscriber::fmt::init();

    iced::application(Launcher::new, Launcher::update, Launcher::view)
        .title("Nanocast")
        // .theme(launcher_theme)
        .subscription(Launcher::subscription)
        .window(iced::window::Settings {
            size: iced::Size::new(700.0, 500.0),
            decorations: false,
            transparent: true,
            level: iced::window::Level::AlwaysOnTop,
            resizable: false,
            position: iced::window::Position::Centered,
            ..Default::default()
        })
        .run()
}
