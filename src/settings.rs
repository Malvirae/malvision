//! Gestion des préférences persistantes de l'interface Malvision (Jalon 2).

use crate::storage::Storage;

const KEY_SIDEBAR_WIDTH: &str = "sidebar_width";
pub const DEFAULT_SIDEBAR_WIDTH: i32 = 260;
pub const MIN_SIDEBAR_WIDTH: i32 = 200;
pub const MAX_SIDEBAR_WIDTH: i32 = 520;

#[derive(Debug, Clone)]
pub struct AppSettings {
    pub sidebar_width: i32,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            sidebar_width: DEFAULT_SIDEBAR_WIDTH,
        }
    }
}

impl AppSettings {
    pub fn load(storage: &Storage) -> Self {
        let sidebar_width = storage
            .get_setting(KEY_SIDEBAR_WIDTH)
            .ok()
            .flatten()
            .and_then(|v| v.parse::<i32>().ok())
            .map(|w| w.clamp(MIN_SIDEBAR_WIDTH, MAX_SIDEBAR_WIDTH))
            .unwrap_or(DEFAULT_SIDEBAR_WIDTH);

        Self { sidebar_width }
    }

    pub fn save_sidebar_width(&mut self, storage: &mut Storage, width: i32) {
        let clamped = width.clamp(MIN_SIDEBAR_WIDTH, MAX_SIDEBAR_WIDTH);
        self.sidebar_width = clamped;
        let _ = storage.set_setting(KEY_SIDEBAR_WIDTH, &clamped.to_string());
    }
}
