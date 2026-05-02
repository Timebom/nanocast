use crate::config::Config;
use crate::models::{LauncherItem, ItemType};
use anyhow::Result;
use std::path::Path;
use walkdir::WalkDir;
use shellexpand::tilde;

pub struct IndexBuilder {
    config: Config,
}

impl IndexBuilder {
    pub fn new(config: Config) -> Self {
        Self {
            config
        }
    }

    pub fn build(&self) -> Result<Vec<LauncherItem>> {
        let mut items = Vec::new();

        if self.config.index.index_applications {
            for path in &self.config.index.applications_paths {
                let expanded = tilde(path);
                self.index_applications(&expanded, &mut items)?;
            }
        }

        if self.config.index.index_files {
            for path in &self.config.index.file_paths {
                let expanded = tilde(path);
                self.index_files(&expanded, &mut items)?;
            }
        }

        // Remove duplicates by path
        items.sort_by_key(|i| i.path.clone());
        items.dedup_by_key(|i| i.path.clone());

        Ok(items)
    }

    fn index_applications(&self, base_path: &str, items: &mut Vec<LauncherItem>) -> Result<()> {
        let path = Path::new(base_path);
        if !path.exists() {
            return Ok(());
        }

        for entry in WalkDir::new(path)
            .max_depth(1)
            .follow_links(false)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let p = entry.path();
            if p.extension().and_then(|s| s.to_str()) == Some("app") {
                if let Some(name) = p.file_stem().and_then(|s| s.to_str()) {
                    items.push(LauncherItem {
                        id: format!("app:{}", p.to_string_lossy()),
                        title: name.to_string(),
                        subtitle: Some("Application".to_string()),
                        path: Some(p.to_string_lossy().to_string()),
                        icon_path: Some(format!("{}/Contents/Resources/AppIcon.icns", p.display())),
                        item_type: ItemType::Application,
                        tags: vec!["app".into()]
                    });
                }
            }
        }
        Ok(())
    }

    fn index_files(&self, base_path: &str, items: &mut Vec<LauncherItem>) -> Result<()> {
        let path = Path::new(base_path);
        if !path.exists() {
            return Ok(());
        }

        for entry in WalkDir::new(path)
            .max_depth(4)
            .follow_links(false)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let p = entry.path();
            if p.is_file() {
                let name = p.file_name().and_then(|s| s.to_str()).unwrap_or("");

                if self.should_ignore(name) {
                    continue;
                }

                items.push(LauncherItem {
                    id: format!("file:{}", p.to_string_lossy()),
                    title: name.to_string(),
                    subtitle: Some(p.parent()
                        .map(|p| p.to_string_lossy().to_string())
                        .unwrap_or_default()),
                    path: Some(p.to_string_lossy().to_string()),
                    icon_path: None,
                    item_type: ItemType::File,
                    tags: vec![]
                });
            }
        }
        Ok(())
    }

    fn should_ignore(&self, name: &str) -> bool {
        self.config.index.ignored_patterns.iter().any(|pat| name.contains(pat))
    }

    pub fn build_demo_index() -> Vec<LauncherItem> {
        let mut items = vec![
            LauncherItem {
                id: "1".into(),
                title: "Zed".into(),
                subtitle: Some("Code Editor".into()),
                path: Some("/usr/bin/zed".into()),
                icon_path: None,
                item_type: ItemType::Application,
                tags: vec!["code".into(), "editor".into()],
            },
            LauncherItem {
                id: "2".into(),
                title: "Firefox".into(),
                subtitle: Some("Web Browser".into()),
                path: Some("/opt/firefox/firefox".into()),
                icon_path: None,
                item_type: ItemType::Application,
                tags: vec!["browser".into()],
            },
            LauncherItem {
                id: "3".into(),
                title: "Spotify".into(),
                subtitle: Some("Streaming Music".into()),
                path: Some("/local/bin/spotify".into()),
                icon_path: None,
                item_type: ItemType::Application,
                tags: vec!["Music".into(), "Songs".into()],
            },
        ];

        if let Some(home) = dirs::home_dir() {
            let downloads = home.join("Downloads");
            for entry in WalkDir::new(downloads).max_depth(2).into_iter().filter_map(|e| e.ok()) {
                if let Some(name) = entry.file_name().to_str() {
                    if name.ends_with(".pdf") || name.ends_with(".txt") {
                        items.push(LauncherItem {
                            id: format!("file={}", items.len()),
                            title: name.to_string(),
                            subtitle: Some("File".into()),
                            path: Some(entry.path().to_string_lossy().into()),
                            icon_path: None,
                            item_type: ItemType::File,
                            tags: vec![]
                        });
                    }
                }
            }
        }

        items
    }
}
