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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ItemType {
    Application,
    File,
    Folder,
    Command,
    Script,
}

#[derive(Clone, Debug)]
pub struct SearchResult {
    pub item: LauncherItem,
    pub score: f32,
}
