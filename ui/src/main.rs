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
        Column
    },
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

mod hotkey;

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
    Ignored,
}

pub struct Launcher {
    search_engine: SearchEngine,
    query: String,
    results: Vec<SearchResult>,
    selected: usize,
    config: Config,
    is_visible: bool,
    hotkey_handler: Option<hotkey::HotkeyHandler>,
}

impl Launcher {
    fn new() -> Self {
        let config = Config::load().unwrap_or_default();
        let items = IndexBuilder::new(config.clone())
            .build()
            .unwrap_or_default();

        let mut engine = SearchEngine::new();
        engine.set_items(items);

        let hotkey_handler = match hotkey::HotkeyHandler::new(&config) {
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
            config,
            is_visible: true,
            hotkey_handler: hotkey_handler
        };
        app.update_results();
        app
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
                }
            }
            Message::SelectUp => {
                self.selected = self.selected.saturating_sub(1);
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
                self.is_visible = false;
                self.query.clear();
                self.update_results();
                self.selected = 0;
            }
            Message::Show => {
                self.query.clear();
                self.update_results();
                self.selected = 0
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
            Message::Ignored => {}
        }
        iced::Task::none()
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

                    let item_row = row![
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
            column![input, scrollable(results_list).spacing(4)]
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
