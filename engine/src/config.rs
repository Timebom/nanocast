use anyhow;
use dirs;
use toml;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub max_results: usize,
    pub fuzzy_threshold: f32,

    #[serde(default)]
    pub index: IndexConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct IndexConfig {
    pub index_applications: bool,
    pub index_files: bool,
    pub index_folders: bool,

    pub applications_paths: Vec<String>,
    pub file_paths: Vec<String>,
    pub ignored_patterns: Vec<String>
}

impl Default for Config {
    fn default() -> Self {
        Self {
            max_results: 50,
            fuzzy_threshold: 0.0,
            index: IndexConfig {
                index_applications: true,
                index_files: true,
                index_folders: false,
                applications_paths: vec![
                    "/Applications".to_string(),
                    "~/Applications".to_string(),
                    "/usr/bin".to_string(),
                    "/usr/local/bin".to_string()
                ],
                file_paths: vec![
                    "~/Downloads".to_string(),
                    "~/Documents".to_string(),
                    "~/Pictures".to_string(),
                    "~/Music".to_string()
                ],
                ignored_patterns: vec![
                    "/sys".to_string(),
                    "/tmp".to_string(),
                    "/sbin".to_string(),
                    "/proc".to_string(),
                    "/boot".to_string(),
                    ".git".to_string(),
                    "node_modules".to_string()
                ]
            }
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
            Ok(toml::from_str(&content)?)
        }
    }

    pub fn save(&self) ->  anyhow::Result<()> {
        let config_path = dirs::config_dir()
            .unwrap()
            .join("nanocast/config.toml");
        std::fs::write(config_path, toml::to_string_pretty(self)?)?;
        Ok(())
    }
}
