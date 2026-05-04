use iced::{
    widget::{
        column,
        container,
        scrollable,
        text,
        text_input,
        Column
    },
    Alignment,
    Element,
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


#[derive(Debug, Clone)]
enum Message {
    QueryChanged(String),
    SelectUp,
    SelectDown,
    Execute,
    Hide,
    Ignored,
}

pub struct Launcher {
    search_engine: SearchEngine,
    query: String,
    results: Vec<SearchResult>,
    selected: usize,
    config: Config,
}

impl Launcher {
    fn new() -> Self {
        let config = Config::load().unwrap_or_default();
        let items = IndexBuilder::new(config.clone())
            .build()
            .unwrap_or_default();

        let mut engine = SearchEngine::new();
        engine.set_items(items);

        Self {
            search_engine: engine,
            query: String::new(),
            results: Vec::new(),
            selected: 0,
            config,
        }
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
                    if let Err(e) = ActionHandler::execute(&result.item) {
                        eprintln!("Action error: {}", e);
                    }
                }
                return iced::Task::perform(async {}, |_| Message::Hide);
            }
            Message::Hide => {
                self.query.clear();
                self.update_results();
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
            .padding(16);

        let results_list: Element<_> = if self.results.is_empty() {
            text("No results").into()
        } else {
            self.results
                .iter()
                .enumerate()
                .take(15)
                .fold(Column::new(), |col, (i, result)| {
                    let is_selected = i == self.selected;

                    let row = column![
                        text(&result.item.title).size(18),
                        text(result.item.subtitle.as_deref().unwrap_or(""))
                            .size(14)
                            .style(|_| text::Style {
                                color: Some(iced::Color::from_rgb(0.7, 0.7, 0.7)),
                            })
                    ]
                    .padding(8)
                    .spacing(4);

                    let styled = if is_selected {
                        container(row).style(|_| container::Style {
                            background: Some(iced::Background::Color(iced::Color::from_rgb(0.2, 0.3, 0.5))),
                            ..Default::default()
                        })
                    } else {
                        container(row)
                    };

                    col.push(styled)
                })
                .into()
        };

        container(
            column![input, scrollable(results_list)]
                .spacing(8)
                .align_x(Alignment::Center)
                .width(iced::Length::Fill),
        )
        .width(iced::Length::Fixed(700.0))
        .height(iced::Length::Fixed(500.0))
        .padding(20)
        .style(|_| container::Style {
            background: Some(iced::Background::Color(iced::Color::from_rgba(0.1, 0.1, 0.1, 0.95))),
            border: iced::Border {
                radius: 16.0.into(),
                ..Default::default()
            },
            ..Default::default()
        })
        .into()
    }
}

fn main() -> iced::Result {
    tracing_subscriber::fmt::init();

    iced::application(Launcher::new, Launcher::update, Launcher::view)
        .title("Nanocast")
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
