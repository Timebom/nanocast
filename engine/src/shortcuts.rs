use crate::actions::Action;
use crate::config::Config;

pub struct DetectedShortcut {
    pub action: Action,
    pub name: String,
    pub trigger: String,
}


#[derive(Debug, Clone)]
pub struct CommandSlot {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone)]
pub struct CommandModeState {
    pub trigger: String,
    pub shortcut_name: String,
    pub slots: Vec<CommandSlot>,
    pub active_slot: usize,
    pub action_type: String,
    pub template: Option<String>,
}

impl CommandModeState {
    pub fn build_action(&self) -> Action {
        match self.action_type.as_str() {
            "calculator" => {
                let expr = self.slots.first().map(|s| s.value.as_str()).unwrap_or("");
                Action::RunCommand(format!("echo 'Calculate: {}'", expr))
            }
            "open_url" | "web_search" => {
                let url = if let Some(tmpl) = &self.template {
                    let mut result = tmpl.clone();
                    for slot in &self.slots {
                        result = result.replace(
                            &format!("{{{}}}", slot.name),
                            &urlencoding::encode(&slot.value),
                        );
                    }
                    result
                } else {
                    let q = self.slots.first().map(|s| s.value.as_str()).unwrap_or("");
                    format!("https://google.com/search?q={}", urlencoding::encode(q))
                };
                Action::OpenUrl(url)
            }
            "command" | "script" => {
                if let Some(tmpl) = &self.template {
                    let mut result = tmpl.clone();
                    for slot in &self.slots {
                        result = result.replace(&format!("{{{}}}", slot.name), &slot.value);
                    }
                    Action::RunCommand(result)
                } else {
                    let q = self.slots.first().map(|s| s.value.as_str()).unwrap_or("");
                    Action::RunCommand(q.to_string())
                }
            }
            _ => {
                #[cfg(target_os = "windows")]
                {
                    Action::Custom("cmd".to_string(), vec!["/C".to_string(), "echo Do Nothing".to_string()])
                }
                #[cfg(not(target_os = "windows"))]
                {
                    Action::Custom("sh".to_string(), vec!["-c".to_string(), "echo Do Nothing".to_string()])
                }
            }
        }
    }

    pub fn tab_next(&mut self) -> bool {
        if self.slots.len() > 1 && self.active_slot < self.slots.len() - 1 {
            self.active_slot += 1;
            true
        } else {
            false
        }
    }

    pub fn set_active_value(&mut self, value: String) {
        if let Some(slot) = self.slots.get_mut(self.active_slot) {
            slot.value = value;
        }
    }

    pub fn active_value(&self) -> &str {
        self.slots.get(self.active_slot).map(|s| s.value.as_str()).unwrap_or("")
    }

    pub fn slot_hint(&self) -> String {
        self.slots
            .iter()
            .enumerate()
            .map(|(i, s)| {
                if i ==  self.active_slot {
                    format!("> {}: \"{}\"", s.name, s.value)
                } else if s.value.is_empty() {
                    format!("[{}]", s.name)
                } else {
                    format!("{}: \"{}\"", s.name, s.value)
                }
            })
            .collect::<Vec<_>>()
            .join(" ")
    }
}

fn extract_slots(template: &str) -> Vec<CommandSlot> {
    let mut slots = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut remaining = template;
    while let Some(start) = remaining.find('{') {
        remaining = &remaining[start + 1..];
        if let Some(end) = remaining.find('}') {
            let name = &remaining[..end];
            if !name.is_empty() && !seen.contains(name) {
                seen.insert(name.to_string());
                slots.push(CommandSlot {
                    name: name.to_string(),
                    value: String::new()
                });
            }
            remaining = &remaining[end + 1..];
        } else {
            break;
        }
    }

    if slots.is_empty() {
        slots.push(CommandSlot {
            name: "query".to_string(),
            value: String::new()
        });
    }
    slots
}

pub struct ShortcutEngine {
    shortcuts: Vec<crate::config::ShortcutConfig>,
}

impl ShortcutEngine {
    pub fn new(config: &Config) -> Self {
        Self {
            shortcuts: config.shortcuts.clone(),
        }
    }

    pub fn detect(&self, query: &str) -> Option<DetectedShortcut> {
        let query = query.trim();
        if query.is_empty() {
            return None;
        }

        let (trigger, remaining) = self.parse_trigger(query);

        for sc in &self.shortcuts {
            if sc.trigger == trigger {
                let action = self.create_action(sc, remaining);
                return Some(DetectedShortcut {
                    action,
                    name: sc.name.to_string(),
                    trigger: sc.trigger.to_string()
                });
            }
        }
        None
    }

    pub fn detect_command_mode(&self, query: &str) -> Option<CommandModeState> {
        let query = query.trim();
        if !query.starts_with('>') {
            return None;
        }
        let rest = query[1..].trim();

        let (trigger, first_value) = if let Some(pos) = rest.find(' ') {
            (rest[..pos].trim(), rest[pos..].trim())
        } else {
            (rest, "")
        };

        if trigger.is_empty() {
            return None;
        }

        for sc in &self.shortcuts {
            if sc.trigger == trigger {
                let mut slots = extract_slots(sc.command.as_deref().unwrap_or(""));
                if !first_value.is_empty() {
                    if let Some(first) = slots.get_mut(0) {
                        first.value = first_value.to_string();
                    }
                }
                return Some(CommandModeState {
                    trigger: trigger.to_string(),
                    shortcut_name: sc.name.clone(),
                    slots: slots,
                    active_slot: 0,
                    action_type: sc.action_type.clone(),
                    template: sc.command.clone(),
                });
            }
        }
        None
    }

    pub fn matching_shortcuts(&self, prefix: &str) -> Vec<&crate::config::ShortcutConfig> {
        self.shortcuts
            .iter()
            .filter(|sc| sc.trigger.starts_with(prefix))
            .collect()
    }

    fn parse_trigger<'a>(&self, query: &'a str) -> (&'a str, &'a str) {
        let query = query.trim();

        if query.starts_with('>') {
            let rest = query[1..].trim();
            if let Some(space_pos) = rest.find(' ') {
                (&rest[..space_pos], rest[space_pos..].trim())
            } else {
                (rest, "")
            }
        } else {
            if let Some(space_pos) = query.find(' ') {
                (query[..space_pos].trim(), query[space_pos..].trim())
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

    pub fn all_shortcuts(&self) -> &Vec<crate::config::ShortcutConfig> {
        &self.shortcuts
    }
}
