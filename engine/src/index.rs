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

        #[cfg(target_os = "linux")]
        {
            if self.config.index.index_applications {
                self.index_linux_desktop_files(&mut items)?;
            }
        }

        #[cfg(target_os = "macos")]
        {
            if self.config.index.index_applications {
                self.index_macos_applications(&mut items)?;
            }
        }

        if self.config.index.index_files {
            for path in &self.config.index.file_paths {
                let expanded = tilde(path);
                self.index_files(&expanded, &mut items)?;
            }
        }

        let shortcut_engine = crate::shortcuts::ShortcutEngine::new(&self.config);
        items.extend(shortcut_engine.all_shortcuts().iter().map(|sc| {
                LauncherItem {
                    id: format!("shortcut:{}", sc.trigger),
                    title: format!("{} -> {}", sc.trigger, sc.name),
                    subtitle: Some(format!("Shortcut: {}", sc.action_type)),
                    path: None,
                    icon_path: sc.icon.clone(),
                    item_type: ItemType::default(),
                    tags: vec!["shortcut".into()],
                }
            })
        );

        // Remove duplicates by path
        items.sort_by_key(|i| i.path.clone());
        items.dedup_by_key(|i| i.path.clone());

        Ok(items)
    }

    #[cfg(target_os = "linux")]
    fn index_linux_desktop_files(&self, items: &mut Vec<LauncherItem>) -> Result<()> {
        use freedesktop_file_parser::{parse, EntryType};
        use freedesktop_icons::lookup;

        let desktop_dirs = vec![
            "/usr/share/applications",
            "/usr/local/share/applications",
            "~/.local/share/applications",
        ];

        for dir in desktop_dirs {
            let path = tilde(dir).into_owned();
            let dir_path = Path::new(&path);
            if !dir_path.exists() {
                continue;
            }

            for entry in WalkDir::new(dir_path)
                .max_depth(2)
                .into_iter()
                .filter_map(|e| e.ok())
            {
                let p = entry.path();
                if p.extension().and_then(|s| s.to_str()) != Some("desktop") {
                    continue;
                }

                let content = match std::fs::read_to_string(p) {
                    Ok(c) => c,
                    Err(_) => continue,
                };

                let desktop = match parse(&content) {
                    Ok(d) => d,
                    Err(_) => continue,
                };

                let app = match desktop.entry.entry_type {
                    EntryType::Application(app) => app,
                    _ => continue,
                };

                if desktop.entry.no_display.unwrap_or(false) || desktop.entry.hidden.unwrap_or(false) {
                    continue;
                }

                let name = desktop.entry.name.default;

                let exec = app.exec
                    .or(app.try_exec)
                    .unwrap_or_default();

                let icon = desktop.entry.icon
                    .map(|i| i.content)
                    .unwrap_or_else(|| "assets/icons/icon.png".into());

                let icon_path = lookup(&icon.as_str())
                    .with_size(48)
                    .with_cache()
                    .find()
                    .map(|p| p.to_string_lossy().to_string());

                let tag = desktop.entry.generic_name
                    .map(|g| g.default)
                    .unwrap_or_else(|| name.clone());

                items.push(LauncherItem {
                    id: format!("app:{}", p.to_string_lossy()),
                    title: name,
                    subtitle: Some("Application".to_string()),
                    path: Some(exec),
                    icon_path: icon_path,
                    item_type: ItemType::Application,
                    tags: vec![tag],
                });
            }
        }
        Ok(())
    }

    #[cfg(target_os = "macos")]
    fn index_macos_applications(&self, items: &mut Vec<LauncherItem>) -> Result<()> {
        self.index_applications("/Applications", items)?;
        self.index_applications("~/Applications", items)?;
        Ok(())
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
