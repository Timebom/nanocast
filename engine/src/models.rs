use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LauncherItem {
    pub id: String,
    pub title: String,
    pub subtitle: Option<String>,
    pub path: Option<String>,
    pub icon_path: Option<String>,
    pub item_type: ItemType,
    pub tags: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub enum ItemType {
    Application,
    File,
    Folder,
    #[default]
    Command,
    Script,
    Url,
}

impl Default for LauncherItem {
    fn default() -> Self {
        Self {
            id: String::new(),
            title: String::new(),
            subtitle: None,
            path: None,
            icon_path: None,
            item_type: ItemType::default(),
            tags: Vec::new(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct SearchResult {
    pub item: LauncherItem,
    pub score: f32,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub enum FilterMode {
    #[default]
    All,
    Applications,
    Files,
    Shortcuts,
}

impl FilterMode {
    #[warn(dead_code)]
    pub fn next(&self) -> Self {
        match self {
            Self::All => Self::Applications,
            Self::Applications => Self::Files,
            Self::Files => Self::Shortcuts,
            Self::Shortcuts => Self::All,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::All => "All",
            Self::Applications => "Applications",
            Self::Files => "Files",
            Self::Shortcuts => "Shortcuts",
        }
    }
}
