use crate::models::{ItemType, LauncherItem};
use anyhow::Result;
use std::process::Command;
use urlencoding;
use arboard::Clipboard;
use evalexpr::*;

#[derive(Debug, Clone)]
pub enum Action {
    LaunchApp(String),
    OpenFile(String),
    OpenUrl(String),
    RunCommand(String),
    Custom(String, Vec<String>),
    CopyToClipboard(String),
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

            Action::CopyToClipboard(text) => {
                let value = text.clone();
                std::thread::spawn(move || {
                    if let Ok(mut clipboard) = Clipboard::new().map_err(|e| anyhow::anyhow!("Failed to open clipboard: {}", e)) {
                        let _ = clipboard.set_text(&value)
                            .map_err(|e| anyhow::anyhow!("Failed to write to clipboard: {}", e));
                        std::thread::sleep(std::time::Duration::from_millis(200));
                    }
                });
                println!("Copied to clipboard: {}", text);
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

    pub fn copy_action_for(item: &LauncherItem) -> Action {
        let text = item.path.clone().unwrap_or_else(|| item.title.clone());
        Action::CopyToClipboard(text)
    }
}

pub struct CalculatorEngine;

impl CalculatorEngine {
    pub fn evaluate(expr: &str) -> Option<String> {
        let expr = expr.trim();
        if expr.is_empty() {
            return None;
        }

        let normalised = expr
            .replace('x', "*")
            .replace('÷', "/")
            .replace('^', "**")
            .replace('π', "3.141592653589793")
            .replace("pi", "3.141592653589793")
            .replace("tau", "6.283185307179586");

        match eval(&normalised) {
            Ok(Value::Float(f)) => {
                if f.fract() == 0.0 && f.abs() < 1e15 {
                    Some(format!("{}", f as i64))
                } else {
                    Some(format!("{}", f))
                }
            }
            Ok(Value::Int(i)) => Some(format!("{}", i)),
            Ok(Value::Boolean(b)) => Some(format!("{}", b)),
            _ => None,
        }
    }

    pub fn looks_like_math(query: &str) -> bool {
        let q = query.trim();
        if q.len() < 2 {
            return false;
        }

        if !q.chars().any(|c| c.is_ascii_digit()) {
            return false;
        }

        let has_op = q.contains('+')
            || q.contains('-')
            || q.contains('*')
            || q.contains('/')
            || q.contains('^')
            || q.contains('%')
            || q.contains('x')
            || q.contains('÷');

        let has_fn = q.contains("sin")
            || q.contains("cos")
            || q.contains("tan")
            || q.contains("sqrt")
            || q.contains("log")
            || q.contains("abs")
            || q.contains("pi")
            || q.contains('π')
            || q.contains("tau");

        has_op || has_fn
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

    if CalculatorEngine::looks_like_math(query) {
        if let Some(result) = CalculatorEngine::evaluate(query) {
            let payload = format!("{}", result);

            return Some(LauncherItem {
                id: format!("calc:{}", query),
                title: format!("= {}", result),
                subtitle: Some(format!("{} -> {}", query, result)),
                path: Some(payload),
                icon_path: None,
                item_type: ItemType::Command,
                tags: vec!["calc".into(), "calculator".into()]
            });
        }
        return Some(LauncherItem {
            id: format!("calc:{}", query),
            title: "...".into(),
            subtitle: Some(format!("Calculating: {}", query)),
            path: None,
            icon_path: None,
            item_type: ItemType::Command,
            tags: vec!["calc".into()]
        });
    }

    None
}
