use crate::models::{ItemType, LauncherItem};
use anyhow::Result;
use std::process::Command;
use urlencoding;

#[derive(Debug, Clone)]
pub enum Action {
    LaunchApp(String),
    OpenFile(String),
    OpenUrl(String),
    RunCommand(String),
    Custom(String, Vec<String>),
}

pub struct ActionHandler;

impl ActionHandler {
    pub fn execute(item: &LauncherItem) -> Result<()> {
        let action = Self::determine_action(item);
        Self::perform_action(action)
    }

    fn determine_action(item: &LauncherItem) -> Action {
        match item.item_type {
            ItemType::Application => {
                if let Some(path) = &item.path {
                    Action::LaunchApp(path.clone())
                } else {
                    Action::OpenUrl(format!("https://www.google.com/search?q={}", urlencoding::encode(&item.title)))
                }
            }

            ItemType::File | ItemType::Folder => {
                if let Some(path) = &item.path {
                    Action::OpenFile(path.clone())
                } else {
                    Action::RunCommand("echo No Path Available".to_string())
                }
            }

            ItemType::Command => {
                if let Some(path) = &item.path {
                    Action::RunCommand(path.clone())
                } else {
                    Action::RunCommand(item.title.clone())
                }
            }

            ItemType::Script => {
                if let Some(path) = &item.path {
                    Action::RunCommand(path.clone())
                } else {
                    Action::RunCommand(item.title.clone())
                }
            }

            ItemType::Url => {
                if let Some(path) = &item.path {
                    Action::OpenUrl(path.clone())
                } else {
                    Action::RunCommand(item.title.clone())
                }
            }
        }
    }

    fn perform_action(action: Action) -> Result<()> {
        match action {
            Action::LaunchApp(path) => {
                #[cfg(target_os = "macos")]
                {
                    Command::new("open").arg("-a").arg(&path).status()?;
                }
                #[cfg(target_os = "linux")]
                {
                    let clean = path
                        .split_whitespace()
                        .filter(|arg| !(arg.starts_with("%") && arg.len() == 2))
                        .collect::<Vec<_>>()
                        .join(" ");

                    let mut parts = clean.split_whitespace();
                    let program = match parts.next() {
                        Some(p) => p.to_string(),
                        None => return Err(anyhow::anyhow!("Empty exec path")),
                    };
                    let args: Vec<String> = parts.map(|s| s.to_string()).collect();

                    Command::new(&program)
                        .args(&args)
                        .spawn()?;
                }
                #[cfg(target_os = "windows")]
                {
                    open::that(&path)?;
                }
                Ok(())
            }

            Action::OpenFile(path) | Action::OpenUrl(path) => {
                open::that(&path)?;
                Ok(())
            }

            Action::RunCommand(cmd) => {
                #[cfg(target_os = "macos")]
                {
                    Command::new("sh")
                        .arg("-c")
                        .arg(&cmd)
                        .spawn()?;
                }
                #[cfg(target_os = "linux")]
                {
                    Command::new("sh")
                        .arg("-c")
                        .arg(&cmd)
                        .spawn()?;
                }
                #[cfg(target_os = "windows")]
                {
                    Command::new("cmd")
                        .arg("/C")
                        .arg(&cmd)
                        .spawn()?;
                }
                Ok(())
            }

            Action::Custom(name, args) => {
                Command::new(name).args(args).spawn()?;
                Ok(())
            }
        }
    }

    pub fn execute_shortcut(action: Action) -> Result<()> {
        Self::perform_action(action)
    }

    pub fn execute_item_or_shortcut(item: &LauncherItem, shortcut_action: Option<Action>) -> Result<()> {
        if let Some(action) = shortcut_action {
            Self::execute_shortcut(action)
        } else {
            Self::execute(item)
        }
    }
}

pub fn create_special_item(query: &str) -> Option<LauncherItem> {
    if query.starts_with("http") || query.contains(".com") || query.contains(".org") {
        let web_url = query.split("//").last().expect("Not a URL");
        return Some(LauncherItem {
            id: format!("web:{}", web_url.to_string()),
            title: format!("Open {}", web_url.to_string()),
            subtitle: Some("Web Search / URL".into()),
            path: Some(query.to_string()),
            icon_path: None,
            item_type: ItemType::Url,
            tags: vec!["web".into()]
        });
    }

    if query.chars().any(|c| c.is_numeric() || "+-*/".contains(c)) {
        return Some(LauncherItem {
            id: format!("calc:{}", query.to_string()),
            title: format!("Calculate: {}", query.to_string()),
            subtitle: Some("Basic Calculator".into()),
            path: Some(query.to_string()),
            icon_path: None,
            item_type: ItemType::Command,
            tags: vec!["calc".into()]
        });
    }

    None
}
