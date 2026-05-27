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
}
