use crate::config::Config;
use crate::models::{ItemType, LauncherItem};
use anyhow::Result;
use shellexpand::tilde;
use std::path::Path;
use walkdir::WalkDir;

pub struct IndexBuilder {
    config: Config,
}

impl IndexBuilder {
    pub fn new(config: Config) -> Self {
        Self { config }
    }

    pub fn build(&self) -> Result<Vec<LauncherItem>> {
        let mut items = Vec::new();

        // Applications
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

        // Files/Folders
        if self.config.index.index_files {
            for path in &self.config.index.file_paths {
                let expanded = tilde(path);
                self.index_files(&expanded, &mut items)?;
            }
        }

        // Shortcuts
        let shortcut_engine = crate::shortcuts::ShortcutEngine::new(&self.config);
        items.extend(
            shortcut_engine
                .all_shortcuts()
                .iter()
                .map(|sc| LauncherItem {
                    id: format!("shortcut:{}", sc.trigger),
                    title: format!("{} -> {}", sc.trigger, sc.name),
                    subtitle: Some(format!("Shortcut: {}", sc.action_type)),
                    path: None,
                    icon_path: sc.icon.clone(),
                    item_type: ItemType::default(),
                    tags: vec!["shortcut".into()],
                }),
        );

        // Remove duplicates by path and sort
        items.sort_by_key(|i| i.path.clone());
        items.dedup_by_key(|i| i.path.clone());

        Ok(items)
    }

    #[cfg(target_os = "linux")]
    fn index_linux_desktop_files(&self, items: &mut Vec<LauncherItem>) -> Result<()> {
        use freedesktop_file_parser::{EntryType, parse};
        use freedesktop_icons::lookup;

        for dir in &self.config.index.applications_paths {
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

                if desktop.entry.no_display.unwrap_or(false)
                    || desktop.entry.hidden.unwrap_or(false)
                {
                    continue;
                }

                let name = desktop.entry.name.default;

                let exec = app.exec.or(app.try_exec).unwrap_or_default();

                let icon = desktop
                    .entry
                    .icon
                    .map(|i| i.content)
                    .unwrap_or_else(|| "assets/icons/icon.png".into());

                let icon_path = lookup(&icon.as_str())
                    .with_size(48)
                    .with_cache()
                    .find()
                    .map(|p| p.to_string_lossy().to_string());

                let tag = desktop
                    .entry
                    .generic_name
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
        for dir in &self.config.index.applications_paths {
            let expanded = tilde(dir).into_owned();
            let path = Path::new(&expanded);
            if !path.exists() {
                continue;
            }

            for entry in WalkDir::new(path)
                .max_depth(1)
                .follow_links(false)
                .into_iter()
                .filter_map(|e| e.ok())
            {
                let p = entry.path();
                if p.extension().and_then(|s| s.to_str()) != Some("app") {
                    continue;
                }
                let (display_name, icon_file) = self.parse_macos_info_plist(p);
                let title = display_name.unwrap_or_else(|| {
                    p.file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("Unknown")
                        .to_string()
                });
                let icon_path = if let Some(icon_name) = icon_file {
                    let resources = p.join("Contents/Resources");
                    let full_icon_path = resources.join(icon_name);
                    if full_icon_path.exists() {
                        // Some(full_icon_path.to_string_lossy().to_string())
                        self.icns_to_png_cached(&full_icon_path.to_string_lossy())
                    } else {
                        None
                    }
                } else {
                    None
                };

                items.push(LauncherItem {
                    id: format!("app:{}", p.to_string_lossy()),
                    title: title,
                    subtitle: Some("Application".to_string()),
                    path: Some(p.to_string_lossy().to_string()),
                    icon_path: icon_path,
                    item_type: ItemType::Application,
                    tags: vec!["app".into()],
                });
            }
        }

        Ok(())
    }

    // Parse Info.plist to get accurate app name and icon file name
    #[cfg(target_os = "macos")]
    fn parse_macos_info_plist(&self, app_path: &Path) -> (Option<String>, Option<String>) {
        let plist_path = app_path.join("Contents/Info.plist");
        if !plist_path.exists() {
            return (None, None);
        }

        match plist::Value::from_file(&plist_path) {
            Ok(value) => {
                let dict = match value.as_dictionary() {
                    Some(d) => d,
                    None => return (None, None),
                };

                // Get display name (better than folder name)
                let display_name = dict
                    .get("CFBundleDisplayName")
                    .or_else(|| dict.get("CFBundleName"))
                    .and_then(|v| v.as_string())
                    .map(|s| s.to_string());

                // Get icon file name
                let icon_name = dict
                    .get("CFBundleIconFile")
                    .and_then(|v| v.as_string())
                    .map(|s| {
                        let mut name = s.to_string();
                        if !name.ends_with(".icns") {
                            name.push_str(".icns");
                        }
                        name
                    });

                (display_name, icon_name)
            }
            Err(_) => (None, None),
        }
    }

    #[cfg(target_os = "macos")]
    fn icns_to_png_cached(&self, icns_path: &str) -> Option<String> {
        use std::fs;

        // Cache dir: ~/.cache/nanocast/icons/
        let cache_dir = dirs::cache_dir()?.join("nanocast/icons");
        fs::create_dir_all(&cache_dir).ok()?;

        // Use a hash of the source path as filename to avoid collisions
        let hash = format!("{:x}", self.md5_of(icns_path));
        let png_path = cache_dir.join(format!("{}.png", hash));

        // Return cached version if already converted
        if png_path.exists() {
            return Some(png_path.to_string_lossy().to_string());
        }

        // Load and decode the .icns file
        let file = fs::File::open(icns_path).ok()?;
        let icon_family = icns::IconFamily::read(file).ok()?;

        // Try to get the best available size (128x128 preferred, fallback to others)
        let image = icon_family
            .get_icon_with_type(icns::IconType::RGBA32_128x128)
            .or_else(|_| icon_family.get_icon_with_type(icns::IconType::RGBA32_64x64))
            .or_else(|_| icon_family.get_icon_with_type(icns::IconType::RGBA32_32x32))
            .ok()?;

        // Convert to PNG bytes and write to cache
        let png_data = image.clone().into_data().to_vec(); // raw RGBA
        let width = image.width(); // before moving
        // Actually use the `image` crate to save properly:
        let img_buf = image::RgbaImage::from_raw(
            width as u32,
            width as u32, // icns images are square
            png_data,
        )?;
        img_buf.save(&png_path).ok()?;

        Some(png_path.to_string_lossy().to_string())
    }

    // Simple path-based hash to avoid pulling in md5 crate
    fn md5_of(&self, s: &str) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut h = DefaultHasher::new();
        s.hash(&mut h);
        h.finish()
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
                    subtitle: Some(
                        p.parent()
                            .map(|p| p.to_string_lossy().to_string())
                            .unwrap_or_default(),
                    ),
                    path: Some(p.to_string_lossy().to_string()),
                    icon_path: None,
                    item_type: ItemType::File,
                    tags: vec![],
                });
            }
        }
        Ok(())
    }

    fn should_ignore(&self, name: &str) -> bool {
        self.config
            .index
            .ignored_patterns
            .iter()
            .any(|pat| name.contains(pat))
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
            for entry in WalkDir::new(downloads)
                .max_depth(2)
                .into_iter()
                .filter_map(|e| e.ok())
            {
                if let Some(name) = entry.file_name().to_str() {
                    if name.ends_with(".pdf") || name.ends_with(".txt") {
                        items.push(LauncherItem {
                            id: format!("file={}", items.len()),
                            title: name.to_string(),
                            subtitle: Some("File".into()),
                            path: Some(entry.path().to_string_lossy().into()),
                            icon_path: None,
                            item_type: ItemType::File,
                            tags: vec![],
                        });
                    }
                }
            }
        }

        items
    }
}
