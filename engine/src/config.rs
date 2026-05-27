use anyhow;
use dirs;
use serde::{Deserialize, Serialize};
use toml;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Platform {
    MacOS,
    Linux,
    Windows,
}

impl Platform {
    pub const fn current() -> Self {
        #[cfg(target_os = "macos")]
        return Platform::MacOS;
        #[cfg(target_os = "linux")]
        return Platform::Linux;
        #[cfg(target_os = "windows")]
        return Platform::Windows;
        #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
        return Platform::Linux;
    }
}

pub const PLATFORM: Platform = Platform::current();

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub max_results: usize,
    pub fuzzy_threshold: f32,

    #[serde(default)]
    pub index: IndexConfig,

    #[serde(default)]
    pub hotkey: HotkeyConfig,

    #[serde(default)]
    pub window: WindowConfig,

    #[serde(default)]
    pub shortcuts: Vec<ShortcutConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexConfig {
    pub index_applications: bool,
    pub index_files: bool,
    pub index_folders: bool,

    pub applications_paths: Vec<String>,
    pub file_paths: Vec<String>,
    pub ignored_patterns: Vec<String>,
}

impl Default for IndexConfig {
    fn default() -> Self {
        Self {
            index_applications: true,
            index_files: true,
            index_folders: false,
            applications_paths: match PLATFORM {
                Platform::MacOS => vec![
                    "/Applications".to_string(),
                    "~/Applications".to_string(),
                    "/System/Applications".to_string(),
                    "/System/Library/CoreServices".to_string(),
                ],
                Platform::Linux => vec![
                    "/usr/share/applications".to_string(),
                    "/usr/local/share/applications".to_string(),
                    "~/.local/share/applications".to_string(),
                ],
                Platform::Windows => vec![
                    // TODO: Windows Applications Source
                ],
            },
            file_paths: match PLATFORM {
                Platform::MacOS => vec![
                    "~/Downloads".to_string(),
                    "~/Documents".to_string(),
                    "~/Desktop".to_string(),
                    "~/Pictures".to_string(),
                    "~/Movies".to_string(),
                    "~/Music".to_string(),
                ],
                Platform::Linux => vec![
                    "~/Downloads".to_string(),
                    "~/Documents".to_string(),
                    "~/Desktop".to_string(),
                    "~/Pictures".to_string(),
                    "~/Videos".to_string(),
                    "~/Music".to_string(),
                ],
                Platform::Windows => vec![
                    // TODO: Windows Files Source
                ],
            },
            ignored_patterns: vec![
                "/sys".to_string(),
                "/tmp".to_string(),
                "/sbin".to_string(),
                "/proc".to_string(),
                "/boot".to_string(),
                ".git".to_string(),
                "node_modules".to_string(),
                "Library/Caches".to_string(),
                "Library/Logs".to_string(),
            ],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotkeyConfig {
    pub modifiers: String,
    pub key: String,
}

impl Default for HotkeyConfig {
    fn default() -> Self {
        Self {
            modifiers: match PLATFORM {
                Platform::MacOS => "Meta".to_string(),
                _ => "Control".to_string(),
            },
            key: "Space".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowConfig {
    pub width: f32,
    pub height: f32,
    pub blur: bool,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            width: 700.0,
            height: 500.0,
            blur: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShortcutConfig {
    pub trigger: String,
    pub name: String,
    pub action_type: String,
    pub command: Option<String>,
    pub icon: Option<String>,
    pub key: Option<String>,
}

impl Default for ShortcutConfig {
    fn default() -> Self {
        ShortcutConfig {
            trigger: "google".into(),
            name: "Google Search".into(),
            action_type: "open_url".into(),
            command: Some("https://google.com/search?q={query}".into()),
            icon: None,
            key: Some("alt+q".to_string()),
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            max_results: 50,
            fuzzy_threshold: 0.0,
            index: IndexConfig::default(),
            hotkey: HotkeyConfig::default(),
            window: WindowConfig::default(),
            shortcuts: vec![
                ShortcutConfig {
                    trigger: "youtube".into(),
                    name: "YouTube".into(),
                    action_type: "open_url".into(),
                    command: Some("https://youtube.com/results?search_query={query}".into()),
                    icon: None,
                    key: Some("ctrl+y".to_string()),
                },
                ShortcutConfig {
                    trigger: "calc".into(),
                    name: "Calculator".into(),
                    action_type: "calculator".into(),
                    command: None,
                    icon: None,
                    key: None,
                },
                ShortcutConfig::default(),
            ],
        }
    }
}

impl Config {
    pub fn load() -> anyhow::Result<Self> {
        let config_dir = dirs::config_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not find config directory"))?
            .join("nanocast");

        std::fs::create_dir_all(&config_dir)?;

        let config_path = config_dir.join("config.toml");

        if !config_path.exists() {
            let default = Config::default();
            std::fs::write(&config_path, toml::to_string_pretty(&default)?)?;
            Ok(default)
        } else {
            let content = std::fs::read_to_string(&config_path)?;
            let mut config: Config = toml::from_str(&content)?;

            let default_shortcuts = Config::default().shortcuts;
            for default_sc in default_shortcuts {
                let already_defined = config
                    .shortcuts
                    .iter()
                    .any(|sc| sc.trigger == default_sc.trigger);

                if !already_defined {
                    config.shortcuts.push(default_sc);
                }
            }
            Ok(config)
        }
    }

    pub fn save(&self) -> anyhow::Result<()> {
        let config_path = dirs::config_dir().unwrap().join("nanocast/config.toml");
        std::fs::write(config_path, toml::to_string_pretty(self)?)?;
        Ok(())
    }
}
