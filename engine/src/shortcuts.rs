use crate::actions::Action;
use crate::config::Config;

pub struct ShortcutEngine {
    shortcuts: Vec<crate::config::ShortcutConfig>,
}

impl ShortcutEngine {
    pub fn new(config: &Config) -> Self {
        Self {
            shortcuts: config.shortcuts.clone(),
        }
    }

    pub fn detect(&self, query: &str) -> Option<(Action, String)> {
        let query = query.trim();
        if query.is_empty() {
            return None;
        }

        let (trigger, remaining) = self.parse_trigger(query);

        for sc in &self.shortcuts {
            if sc.trigger == trigger {
                let action = self.create_action(sc, remaining);
                return Some((action, sc.name.clone()));
            }
        }
        None
    }

    fn parse_trigger<'a>(&self, query: &'a str) -> (&'a str, &'a str) {
        let query = query.trim();

        if query.starts_with('>') || query.starts_with('!') {
            let rest = query[1..].trim();
            if let Some(space_pos) = rest.find(' ') {
                let trigger = rest[..space_pos].trim();
                let remaining = rest[space_pos..].trim();
                (trigger, remaining)
            } else {
                (rest, "")
            }
        } else {
            if let Some(space_pos) = query.find(' ') {
                let trigger = query[..space_pos].trim();
                let remaining = query[space_pos..].trim();
                (trigger, remaining)
            } else {
                (query, "")
            }
        }
    }

    fn create_action(&self, sc: &crate::config::ShortcutConfig, remaining: &str) -> Action {
        match sc.action_type.as_str() {
            "calculator" => {
                Action::RunCommand(format!("echo 'Calculate: {}'", remaining))
            }

            "open_url" | "web_search" => {
                let url = if let Some(cmd) = &sc.command {
                    if cmd.contains("{query}") {
                        cmd.replace("{query}", remaining)
                    } else if cmd.contains("{q}") {
                        cmd.replace("{q}", remaining)
                    } else {
                        cmd.clone()
                    }
                } else {
                    format!("https://google.com/search?q={}", urlencoding::encode(remaining))
                };
                Action::OpenUrl(url)
            }

            "command" | "script" => {
                if let Some(cmd) = &sc.command {
                    let final_cmd = if cmd.contains("{query}") {
                        cmd.replace("{query}", remaining)
                    } else {
                        format!("{} {}", cmd, remaining)
                    };
                    Action::RunCommand(final_cmd)
                } else {
                    Action::RunCommand(remaining.to_string())
                }
            }

            _ => {
                #[cfg(target_os = "windows")] {
                    return Action::Custom("cmd".to_string(), vec!["/C".to_string(), "echo Do Nothing".to_string()])
                }
                #[cfg(not(target_os = "windows"))] {
                    Action::Custom("sh".to_string(), vec!["-c".to_string(), "echo Do Nothing".to_string()])
                }
            },
        }
    }

    pub fn detect_key(&self, key: &str) -> Option<(Action, String)> {
        for sc in &self.shortcuts {
            if sc.key.as_deref() == Some(key) {
                let action = self.create_action(sc, "");
                return Some((action, sc.name.clone()));
            }
        }
        None
    }
}
