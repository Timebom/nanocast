use iced::window;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Platform {
    MacOS,
    Linux,
    Windows,
}

impl Platform {
    pub fn current() -> Self {
        #[cfg(target_os = "macos")]
        return Platform::MacOS;
        #[cfg(target_os = "linux")]
        return Platform::Linux;
        #[cfg(target_os = "windows")]
        return Platform::Windows;
        #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
        return Platform::Linux;
    }

    pub fn default_config(&self) -> engine::Config {
        let mut config = engine::Config::default();

        match self {
            Platform::MacOS => {
                config.index.applications_paths = vec![
                    "/Applications".to_string(),
                    "~/Applications".to_string(),
                    "/System/Applications".to_string(),
                ];
                config.index.file_paths = vec![
                    "~/Downloads".to_string(),
                    "~/Documents".to_string(),
                    "~/Desktop".to_string(),
                    "~/Pictures".to_string(),
                ];

                config.hotkey.modifiers = "Meta".to_string();
                config.window.blur = true;
            }
            Platform::Linux => {
                config.index.applications_paths = vec![
                    "/usr/share/applications".to_string(),
                    "/usr/local/share/applications".to_string(),
                    "~/.local/share/applications".to_string(),
                ];
                config.hotkey.modifiers = "Control".to_string();
            }
            Platform::Windows => {
                // TODO: Windows defaults later
            }
        }
        config
    }

    pub fn window_settings(&self, config: &engine::WindowConfig) -> window::Settings {
        let mut settings = window::Settings {
            size: iced::Size::new(config.width, config.height),
            decorations: false,
            transparent: true,
            level: window::Level::AlwaysOnTop,
            resizable: false,
            position: window::Position::Centered,
            ..Default::default()
        };

        #[cfg(target_os = "macos")]
        {
            settings.platform_specific = iced::window::settings::PlatformSpecific {
                titlebar_transparent: true,
                fullsize_content_view: true,
                ..Default::default()
            };
        }

        settings
    }

    pub fn open_command(&self) -> &'static str {
        match self {
            Platform::MacOS => "open",
            Platform::Linux => "xdg-open",
            Platform::Windows => "start",
        }
    }

    /// Returns icon path resolution strategy
    pub fn get_app_icon_path(&self, app_path: &str) -> Option<String> {
        match self {
            Platform::MacOS => {
                // Simple macOS .app icon lookup
                let path = std::path::Path::new(app_path);
                if path.extension().map_or(false, |e| e == "app") {
                    let resources = path.join("Contents/Resources");
                    let candidates = ["AppIcon.icns", "icon.icns"];

                    for candidate in candidates {
                        let icon = resources.join(candidate);
                        if icon.exists() {
                            return Some(icon.to_string_lossy().to_string());
                        }
                    }
                }
                None
            }
            _ => None, // Linux/Windows handled elsewhere
        }
    }
}
