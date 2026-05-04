use anyhow;
use global_hotkey::{
    GlobalHotKeyEvent,
    GlobalHotKeyManager
};
use global_hotkey::hotkey::{
    Code,
    HotKey,
    Modifiers
};
use engine::{
    Config,
    HotkeyConfig,
};
use std::sync::mpsc::{
    Receiver,
    channel,
};

pub struct HotkeyHandler {
    manager: GlobalHotKeyManager,
    receiver: Receiver<GlobalHotKeyEvent>,
}

impl HotkeyHandler {
    pub fn new(config: &Config) -> anyhow::Result<Self> {
        let manager = GlobalHotKeyManager::new()?;
        let hotkey = Self::parse_hotkey(&config.hotkey)?;
        manager.register(hotkey)?;
        println!("Global hotkey registered: {:?}", config.hotkey);

        let (tx, receiver) = channel();
        GlobalHotKeyEvent::set_event_handler(Some(move |event| {
            let _ = tx.send(event);
        }));
        Ok(Self { manager, receiver })
    }

    fn parse_hotkey(hk: &engine::HotkeyConfig) -> anyhow::Result<HotKey> {
        let modifiers = match hk.modifiers.to_lowercase().as_str() {
            "control" | "ctrl" => Modifiers::CONTROL,
            "meta" | "cmd" | "super" => Modifiers::META,
            "alt" => Modifiers::ALT,
            "shift" => Modifiers::SHIFT,
            _ => Modifiers::CONTROL,
        };

        let code = match hk.key.to_lowercase().as_str() {
            "space" => Code::Space,
            "enter" => Code::Enter,
            "k" => Code::KeyK,
            "j" => Code::KeyJ,
            "slash" | "/" => Code::Slash,
            _ => Code::Space,
        };

        Ok(HotKey::new(Some(modifiers), code))
    }

    pub fn try_recv(&self) -> Option<GlobalHotKeyEvent> {
        self.receiver.try_recv().ok()
    }

    #[allow(dead_code)]
    pub fn update_hotkey(&mut self, config: &HotkeyConfig) -> anyhow::Result<()> {
        let new_hotkey = Self::parse_hotkey(config)?;
        self.manager.register(new_hotkey)?;
        Ok(())
    }
}
