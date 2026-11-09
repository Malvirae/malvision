//! Gestion des préférences persistantes de l'application Malvision (Jalons 2 & 2C),
//! résolution du thème (Sombre / Clair / Système) et gestion du raccourci de démarrage
//! Windows (`Malvision.lnk` dans le dossier Startup de l'utilisateur courant via COM `IShellLinkW`).

use crate::storage::Storage;
use std::path::{Path, PathBuf};
use windows::Win32::Foundation::MAX_PATH;
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize,
    IPersistFile, STGM_READ,
};
use windows::Win32::System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_DWORD, RegGetValueW};
use windows::Win32::UI::Shell::{
    FOLDERID_Startup, IShellLinkW, KF_FLAG_DEFAULT, SHGetKnownFolderPath, ShellLink,
};
use windows_core::{Interface, PCWSTR, w};

const KEY_SIDEBAR_WIDTH: &str = "sidebar_width";
const KEY_THEME_MODE: &str = "theme_mode";
const KEY_CLOSE_TO_TRAY: &str = "close_to_tray";
const KEY_START_MINIMIZED_TO_TRAY: &str = "start_minimized_to_tray";
const KEY_CONFIRM_EXIT_ACTIVE: &str = "confirm_exit_active_sessions";
const KEY_RESTORE_RDP_SESSIONS: &str = "restore_rdp_sessions";
const KEY_START_WITH_WINDOWS: &str = "start_with_windows";
const KEY_SHOW_RECENT_CONNECTIONS: &str = "show_recent_connections";
const KEY_MAX_RECENT_CONNECTIONS: &str = "max_recent_connections";
const KEY_RDP_DEFAULT_CLIPBOARD: &str = "rdp_default_clipboard";
const KEY_RDP_DEFAULT_DYNAMIC_RES: &str = "rdp_default_dynamic_resolution";
const KEY_RDP_KEYBOARD_HOOK_MODE: &str = "rdp_keyboard_hook_mode";
const KEY_RESTORABLE_RDP_IDS: &str = "restorable_rdp_connection_ids";
const KEY_ANYDESK_PATH: &str = "anydesk_path";
const KEY_RUSTDESK_PATH: &str = "rustdesk_path";

pub const DEFAULT_SIDEBAR_WIDTH: i32 = 260;
pub const MIN_SIDEBAR_WIDTH: i32 = 200;
pub const MAX_SIDEBAR_WIDTH: i32 = 520;

pub const DEFAULT_MAX_RECENT_CONNECTIONS: usize = 10;
pub const MIN_MAX_RECENT_CONNECTIONS: usize = 1;
pub const MAX_MAX_RECENT_CONNECTIONS: usize = 50;

pub const STARTUP_SHORTCUT_FILENAME: &str = "Malvision.lnk";

/// Choix de thème d'apparence dans les Paramètres.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeMode {
    Dark,
    Light,
    System,
}

impl ThemeMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Dark => "dark",
            Self::Light => "light",
            Self::System => "system",
        }
    }

    pub fn from_db_str(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "light" => Self::Light,
            "system" => Self::System,
            _ => Self::Dark,
        }
    }

    #[allow(dead_code)]
    pub fn display_name(self) -> &'static str {
        match self {
            Self::Dark => "Sombre (par défaut)",
            Self::Light => "Clair",
            Self::System => "Système (suit Windows 11)",
        }
    }

    #[allow(dead_code)]
    pub fn to_combo_index(self) -> usize {
        match self {
            Self::Dark => 0,
            Self::Light => 1,
            Self::System => 2,
        }
    }

    #[allow(dead_code)]
    pub fn from_combo_index(idx: usize) -> Self {
        match idx {
            1 => Self::Light,
            2 => Self::System,
            _ => Self::Dark,
        }
    }

    /// Résout le thème effectif (`Dark` ou `Light`).
    pub fn resolve(self) -> ResolvedTheme {
        match self {
            Self::Dark => ResolvedTheme::Dark,
            Self::Light => ResolvedTheme::Light,
            Self::System => {
                if is_windows_light_theme_enabled() {
                    ResolvedTheme::Light
                } else {
                    ResolvedTheme::Dark
                }
            }
        }
    }
}

/// Thème effectif après résolution du mode `System`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolvedTheme {
    Dark,
    Light,
}

impl ResolvedTheme {
    #[allow(dead_code)]
    pub fn is_dark(self) -> bool {
        matches!(self, Self::Dark)
    }

    #[allow(dead_code)]
    pub fn colors(self) -> ThemeColors {
        match self {
            Self::Dark => ThemeColors::DARK,
            Self::Light => ThemeColors::LIGHT,
        }
    }
}

/// Palette de couleurs GDI (`COLORREF` `0x00BBGGRR`) complète pour un thème donné.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThemeColors {
    pub is_dark: bool,
    pub bg_deep: u32,
    pub bg_sidebar: u32,
    pub bg_surface: u32,
    pub bg_hover: u32,
    pub bg_selected: u32,
    pub bg_input: u32,
    pub border: u32,
    pub accent: u32,
    pub accent_hover: u32,
    pub text_primary: u32,
    pub text_muted: u32,
    pub status_ok: u32,
    pub status_warn: u32,
    pub status_err: u32,
}

impl ThemeColors {
    /// Palette sombre professionnelle Windows 11 (anthracite profond `#16181B` + accent bleu `#0078D4`).
    pub const DARK: Self = Self {
        is_dark: true,
        bg_deep: 0x001B1816,      // #16181B
        bg_sidebar: 0x0024201D,   // #1D2024
        bg_surface: 0x00302925,   // #252930
        bg_hover: 0x003D352F,     // #2F353D
        bg_selected: 0x004A3826,  // #26384A
        bg_input: 0x001F1C19,     // #191C1F
        border: 0x00453D37,       // #373D45
        accent: 0x00D47800,       // #0078D4
        accent_hover: 0x00E88A10, // #108AE8
        text_primary: 0x00F6F3F0, // #F0F3F6
        text_muted: 0x00A89D93,   // #939DA8
        status_ok: 0x0050AF4C,    // #4CAF50
        status_warn: 0x0000A8FF,  // #FFA800
        status_err: 0x005053EF,   // #EF5350
    };

    /// Palette claire professionnelle Windows 11 Fluent Light (fond ardoise clair `#F4F6F9`,
    /// cartes blanches `#FFFFFF`, texte charbon `#181E26`, accent bleu `#0067C0`).
    pub const LIGHT: Self = Self {
        is_dark: false,
        bg_deep: 0x00F9F6F4,      // #F4F6F9
        bg_sidebar: 0x00F2ECE8,   // #E8ECF2
        bg_surface: 0x00FFFFFF,   // #FFFFFF
        bg_hover: 0x00EDE4DC,     // #DCE4ED
        bg_selected: 0x00FAE8D4,  // #D4E8FA
        bg_input: 0x00FFFFFF,     // #FFFFFF
        border: 0x00D4C8BE,       // #BEC8D4
        accent: 0x00C06700,       // #0067C0
        accent_hover: 0x00D47800, // #0078D4
        text_primary: 0x00261E18, // #181E26
        text_muted: 0x006E5E52,   // #525E6E
        status_ok: 0x00387E1E,    // #1E7E38
        status_warn: 0x00007CD9,  // #D97C00
        status_err: 0x002F2FD3,   // #D32F2F
    };
}

/// Architecture préparatoire pour le mode des raccourcis clavier RDP (`KeyboardHookMode`),
/// prévu pour un jalon ultérieur (non exposé dans l'UI du Jalon 2C).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RdpKeyboardHookMode {
    OnLocal,
    OnRemote,
    InFullScreenOnly,
}

impl RdpKeyboardHookMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::OnLocal => "local",
            Self::OnRemote => "remote",
            Self::InFullScreenOnly => "fullscreen_only",
        }
    }

    pub fn from_db_str(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "local" => Self::OnLocal,
            "fullscreen_only" => Self::InFullScreenOnly,
            _ => Self::OnRemote,
        }
    }

    pub fn to_combo_index(self) -> usize {
        match self {
            Self::OnRemote => 0,
            Self::OnLocal => 1,
            Self::InFullScreenOnly => 2,
        }
    }

    pub fn from_combo_index(idx: usize) -> Self {
        match idx {
            1 => Self::OnLocal,
            2 => Self::InFullScreenOnly,
            _ => Self::OnRemote,
        }
    }
}

/// Interroge le registre Windows 11 (`HKCU\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize\AppsUseLightTheme`)
/// pour savoir si le thème clair système est actif.
pub fn is_windows_light_theme_enabled() -> bool {
    unsafe {
        let mut data: u32 = 0;
        let mut cb_data: u32 = std::mem::size_of::<u32>() as u32;
        let status = RegGetValueW(
            HKEY_CURRENT_USER,
            w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize"),
            w!("AppsUseLightTheme"),
            RRF_RT_REG_DWORD,
            None,
            Some(&mut data as *mut u32 as *mut _),
            Some(&mut cb_data),
        );
        status.is_ok() && data == 1
    }
}

/// Préférences globales de l'application Malvision persistées dans la table SQLite `settings`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppSettings {
    pub sidebar_width: i32,
    pub theme_mode: ThemeMode,
    pub close_to_tray: bool,
    pub start_minimized_to_tray: bool,
    pub confirm_exit_active_sessions: bool,
    pub restore_rdp_sessions: bool,
    pub start_with_windows: bool,
    pub show_recent_connections: bool,
    pub max_recent_connections: usize,
    pub rdp_default_clipboard: bool,
    pub rdp_default_dynamic_resolution: bool,
    pub rdp_keyboard_hook_mode: RdpKeyboardHookMode,
    pub restorable_rdp_connection_ids: Vec<i64>,
    pub anydesk_path: String,
    pub rustdesk_path: String,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            sidebar_width: DEFAULT_SIDEBAR_WIDTH,
            theme_mode: ThemeMode::Dark,
            close_to_tray: false,
            start_minimized_to_tray: false,
            confirm_exit_active_sessions: true,
            restore_rdp_sessions: false,
            start_with_windows: false,
            show_recent_connections: true,
            max_recent_connections: DEFAULT_MAX_RECENT_CONNECTIONS,
            rdp_default_clipboard: true,
            rdp_default_dynamic_resolution: true,
            rdp_keyboard_hook_mode: RdpKeyboardHookMode::OnRemote,
            restorable_rdp_connection_ids: Vec::new(),
            anydesk_path: String::new(),
            rustdesk_path: String::new(),
        }
    }
}

fn parse_bool_setting(val: Option<String>, default: bool) -> bool {
    match val.as_deref().map(str::trim) {
        Some("1") | Some("true") | Some("TRUE") | Some("yes") => true,
        Some("0") | Some("false") | Some("FALSE") | Some("no") => false,
        _ => default,
    }
}

impl AppSettings {
    /// Charge toutes les préférences depuis la table `settings` de SQLite avec valeurs par défaut sûres.
    pub fn load(storage: &Storage) -> Self {
        let defaults = Self::default();

        let sidebar_width = storage
            .get_setting(KEY_SIDEBAR_WIDTH)
            .ok()
            .flatten()
            .and_then(|v| v.parse::<i32>().ok())
            .map(|w| w.clamp(MIN_SIDEBAR_WIDTH, MAX_SIDEBAR_WIDTH))
            .unwrap_or(defaults.sidebar_width);

        let theme_mode = storage
            .get_setting(KEY_THEME_MODE)
            .ok()
            .flatten()
            .map(|s| ThemeMode::from_db_str(&s))
            .unwrap_or(defaults.theme_mode);

        let close_to_tray = parse_bool_setting(
            storage.get_setting(KEY_CLOSE_TO_TRAY).ok().flatten(),
            defaults.close_to_tray,
        );

        let raw_start_min = parse_bool_setting(
            storage
                .get_setting(KEY_START_MINIMIZED_TO_TRAY)
                .ok()
                .flatten(),
            defaults.start_minimized_to_tray,
        );
        // "Démarrer minimisé dans la zone de notification" n'est actif que si le tray est activé
        let start_minimized_to_tray = close_to_tray && raw_start_min;

        let confirm_exit_active_sessions = parse_bool_setting(
            storage.get_setting(KEY_CONFIRM_EXIT_ACTIVE).ok().flatten(),
            defaults.confirm_exit_active_sessions,
        );

        let restore_rdp_sessions = parse_bool_setting(
            storage.get_setting(KEY_RESTORE_RDP_SESSIONS).ok().flatten(),
            defaults.restore_rdp_sessions,
        );

        let start_with_windows = parse_bool_setting(
            storage.get_setting(KEY_START_WITH_WINDOWS).ok().flatten(),
            defaults.start_with_windows,
        );

        let show_recent_connections = parse_bool_setting(
            storage
                .get_setting(KEY_SHOW_RECENT_CONNECTIONS)
                .ok()
                .flatten(),
            defaults.show_recent_connections,
        );

        let max_recent_connections = storage
            .get_setting(KEY_MAX_RECENT_CONNECTIONS)
            .ok()
            .flatten()
            .and_then(|v| v.trim().parse::<usize>().ok())
            .map(|n| n.clamp(MIN_MAX_RECENT_CONNECTIONS, MAX_MAX_RECENT_CONNECTIONS))
            .unwrap_or(defaults.max_recent_connections);

        let rdp_default_clipboard = parse_bool_setting(
            storage
                .get_setting(KEY_RDP_DEFAULT_CLIPBOARD)
                .ok()
                .flatten(),
            defaults.rdp_default_clipboard,
        );

        let rdp_default_dynamic_resolution = parse_bool_setting(
            storage
                .get_setting(KEY_RDP_DEFAULT_DYNAMIC_RES)
                .ok()
                .flatten(),
            defaults.rdp_default_dynamic_resolution,
        );

        let rdp_keyboard_hook_mode = storage
            .get_setting(KEY_RDP_KEYBOARD_HOOK_MODE)
            .ok()
            .flatten()
            .map(|s| RdpKeyboardHookMode::from_db_str(&s))
            .unwrap_or(defaults.rdp_keyboard_hook_mode);

        let restorable_rdp_connection_ids = storage
            .get_setting(KEY_RESTORABLE_RDP_IDS)
            .ok()
            .flatten()
            .map(|s| {
                s.split(',')
                    .filter_map(|part| part.trim().parse::<i64>().ok())
                    .collect::<Vec<i64>>()
            })
            .unwrap_or_default();

        let anydesk_path = storage
            .get_setting(KEY_ANYDESK_PATH)
            .ok()
            .flatten()
            .unwrap_or_default();

        let rustdesk_path = storage
            .get_setting(KEY_RUSTDESK_PATH)
            .ok()
            .flatten()
            .unwrap_or_default();

        Self {
            sidebar_width,
            theme_mode,
            close_to_tray,
            start_minimized_to_tray,
            confirm_exit_active_sessions,
            restore_rdp_sessions,
            start_with_windows,
            show_recent_connections,
            max_recent_connections,
            rdp_default_clipboard,
            rdp_default_dynamic_resolution,
            rdp_keyboard_hook_mode,
            restorable_rdp_connection_ids,
            anydesk_path,
            rustdesk_path,
        }
    }

    /// Persiste l'intégralité des préférences dans la table `settings` de SQLite.
    pub fn save_all(&mut self, storage: &mut Storage) -> Result<(), String> {
        self.sidebar_width = self
            .sidebar_width
            .clamp(MIN_SIDEBAR_WIDTH, MAX_SIDEBAR_WIDTH);
        self.max_recent_connections = self
            .max_recent_connections
            .clamp(MIN_MAX_RECENT_CONNECTIONS, MAX_MAX_RECENT_CONNECTIONS);
        if !self.close_to_tray {
            self.start_minimized_to_tray = false;
        }

        storage.set_setting(KEY_SIDEBAR_WIDTH, &self.sidebar_width.to_string())?;
        storage.set_setting(KEY_THEME_MODE, self.theme_mode.as_str())?;
        storage.set_setting(
            KEY_CLOSE_TO_TRAY,
            if self.close_to_tray { "1" } else { "0" },
        )?;
        storage.set_setting(
            KEY_START_MINIMIZED_TO_TRAY,
            if self.start_minimized_to_tray {
                "1"
            } else {
                "0"
            },
        )?;
        storage.set_setting(
            KEY_CONFIRM_EXIT_ACTIVE,
            if self.confirm_exit_active_sessions {
                "1"
            } else {
                "0"
            },
        )?;
        storage.set_setting(
            KEY_RESTORE_RDP_SESSIONS,
            if self.restore_rdp_sessions { "1" } else { "0" },
        )?;
        storage.set_setting(
            KEY_START_WITH_WINDOWS,
            if self.start_with_windows { "1" } else { "0" },
        )?;
        storage.set_setting(
            KEY_SHOW_RECENT_CONNECTIONS,
            if self.show_recent_connections {
                "1"
            } else {
                "0"
            },
        )?;
        storage.set_setting(
            KEY_MAX_RECENT_CONNECTIONS,
            &self.max_recent_connections.to_string(),
        )?;
        storage.set_setting(
            KEY_RDP_DEFAULT_CLIPBOARD,
            if self.rdp_default_clipboard { "1" } else { "0" },
        )?;
        storage.set_setting(
            KEY_RDP_DEFAULT_DYNAMIC_RES,
            if self.rdp_default_dynamic_resolution {
                "1"
            } else {
                "0"
            },
        )?;
        storage.set_setting(
            KEY_RDP_KEYBOARD_HOOK_MODE,
            self.rdp_keyboard_hook_mode.as_str(),
        )?;
        storage.set_setting(KEY_ANYDESK_PATH, &self.anydesk_path)?;
        storage.set_setting(KEY_RUSTDESK_PATH, &self.rustdesk_path)?;
        Ok(())
    }

    pub fn save_sidebar_width(&mut self, storage: &mut Storage, width: i32) {
        let clamped = width.clamp(MIN_SIDEBAR_WIDTH, MAX_SIDEBAR_WIDTH);
        self.sidebar_width = clamped;
        let _ = storage.set_setting(KEY_SIDEBAR_WIDTH, &clamped.to_string());
    }

    /// Enregistre la liste des identifiants de connexions RDP ouvertes en vue d'une restauration au démarrage.
    pub fn save_restorable_rdp_sessions(&mut self, storage: &mut Storage, connection_ids: &[i64]) {
        let mut dedup = Vec::new();
        for &cid in connection_ids {
            if !dedup.contains(&cid) {
                dedup.push(cid);
            }
        }
        self.restorable_rdp_connection_ids = dedup;
        let serialized = self
            .restorable_rdp_connection_ids
            .iter()
            .map(i64::to_string)
            .collect::<Vec<_>>()
            .join(",");
        let _ = storage.set_setting(KEY_RESTORABLE_RDP_IDS, &serialized);
    }

    #[allow(dead_code)]
    pub fn resolved_theme(&self) -> ResolvedTheme {
        self.theme_mode.resolve()
    }
}

// =============================================================================
// DÉMARRAGE AUTOMATIQUE AVEC WINDOWS (.lnk dans le dossier Startup utilisateur)
// =============================================================================

/// Retourne le chemin absolu du raccourci `Malvision.lnk` dans le dossier Startup de l'utilisateur courant.
pub fn default_startup_shortcut_path() -> Result<PathBuf, String> {
    unsafe {
        if let Ok(raw_path) = SHGetKnownFolderPath(&FOLDERID_Startup, KF_FLAG_DEFAULT, None) {
            let folder_str = raw_path.to_string().ok();
            CoTaskMemFree(Some(raw_path.0 as *const _));
            if let Some(dir) = folder_str
                && !dir.is_empty()
            {
                return Ok(PathBuf::from(dir).join(STARTUP_SHORTCUT_FILENAME));
            }
        }
    }

    if let Some(appdata) = std::env::var_os("APPDATA") {
        return Ok(PathBuf::from(appdata)
            .join("Microsoft")
            .join("Windows")
            .join("Start Menu")
            .join("Programs")
            .join("Startup")
            .join(STARTUP_SHORTCUT_FILENAME));
    }

    Err("Impossible de déterminer le dossier Startup de l'utilisateur courant.".to_string())
}

/// Crée ou met à jour un raccourci Windows `.lnk` à l'emplacement `lnk_path` pointant vers `target_exe`.
/// Utilise l'interface COM native `IShellLinkW` + `IPersistFile` sans nécessiter de privilèges administrateur.
pub fn create_or_update_startup_shortcut_at(
    lnk_path: &Path,
    target_exe: &Path,
) -> Result<(), String> {
    if let Some(parent) = lnk_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            format!(
                "Impossible de préparer le dossier Startup '{}' : {e}",
                parent.display()
            )
        })?;
    }

    unsafe {
        let co_init = CoInitializeEx(None, windows::Win32::System::Com::COINIT_APARTMENTTHREADED);
        let result = (|| -> Result<(), String> {
            let shell_link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)
                .map_err(|e| format!("Échec CoCreateInstance(ShellLink) : {e}"))?;

            let mut exe_w: Vec<u16> = target_exe.as_os_str().encode_wide().collect();
            exe_w.push(0);
            shell_link
                .SetPath(PCWSTR(exe_w.as_ptr()))
                .map_err(|e| format!("Échec IShellLinkW::SetPath : {e}"))?;

            if let Some(work_dir) = target_exe.parent() {
                let mut dir_w: Vec<u16> = work_dir.as_os_str().encode_wide().collect();
                dir_w.push(0);
                let _ = shell_link.SetWorkingDirectory(PCWSTR(dir_w.as_ptr()));
            }

            let _ =
                shell_link.SetDescription(w!("Malvision — Gestionnaire de Connexions Distantes"));

            let persist_file: IPersistFile = shell_link
                .cast()
                .map_err(|e| format!("Échec QueryInterface(IPersistFile) : {e}"))?;

            let mut lnk_w: Vec<u16> = lnk_path.as_os_str().encode_wide().collect();
            lnk_w.push(0);
            persist_file
                .Save(PCWSTR(lnk_w.as_ptr()), true)
                .map_err(|e| {
                    format!(
                        "Échec d'enregistrement du raccourci '{}' : {e}",
                        lnk_path.display()
                    )
                })?;

            Ok(())
        })();

        if co_init.is_ok() {
            CoUninitialize();
        }
        result
    }
}

/// Lit la cible réelle (`GetPath`) d'un fichier raccourci `.lnk` existant via `IShellLinkW`.
pub fn read_startup_shortcut_target_at(lnk_path: &Path) -> Option<PathBuf> {
    if !lnk_path.exists() {
        return None;
    }

    unsafe {
        let co_init = CoInitializeEx(None, windows::Win32::System::Com::COINIT_APARTMENTTHREADED);
        let target = (|| -> Option<PathBuf> {
            let shell_link: IShellLinkW =
                CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).ok()?;
            let persist_file: IPersistFile = shell_link.cast().ok()?;

            let mut lnk_w: Vec<u16> = lnk_path.as_os_str().encode_wide().collect();
            lnk_w.push(0);
            persist_file.Load(PCWSTR(lnk_w.as_ptr()), STGM_READ).ok()?;

            let mut buf = [0u16; MAX_PATH as usize];
            shell_link.GetPath(&mut buf, std::ptr::null_mut(), 0).ok()?;

            let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
            if len == 0 {
                return None;
            }
            let path_str = String::from_utf16_lossy(&buf[..len]);
            Some(PathBuf::from(path_str))
        })();

        if co_init.is_ok() {
            CoUninitialize();
        }
        target
    }
}

/// Supprime le raccourci `.lnk` s'il existe.
pub fn remove_startup_shortcut_at(lnk_path: &Path) -> Result<(), String> {
    if lnk_path.exists() {
        std::fs::remove_file(lnk_path).map_err(|e| {
            format!(
                "Impossible de supprimer le raccourci de démarrage '{}' : {e}",
                lnk_path.display()
            )
        })?;
    }
    Ok(())
}

/// Applique immédiatement l'activation ou la désactivation du raccourci de démarrage `.lnk`.
pub fn apply_startup_shortcut_at(
    lnk_path: &Path,
    target_exe: &Path,
    enable: bool,
) -> Result<(), String> {
    if enable {
        create_or_update_startup_shortcut_at(lnk_path, target_exe)
    } else {
        remove_startup_shortcut_at(lnk_path)
    }
}

/// Applique l'activation ou la désactivation du raccourci `.lnk` dans le dossier Startup
/// de l'utilisateur courant, pointant vers l'exécutable `Malvision.exe` courant.
pub fn apply_startup_shortcut(enable: bool) -> Result<(), String> {
    let lnk_path = default_startup_shortcut_path()?;
    let target_exe = std::env::current_exe()
        .map_err(|e| format!("Impossible de résoudre le chemin de Malvision.exe : {e}"))?;
    apply_startup_shortcut_at(&lnk_path, &target_exe, enable)
}

/// Synchronise l'état de l'option « Démarrer Malvision avec Windows » avec l'existence et la cible
/// réelle du raccourci `.lnk` :
/// - Si le raccourci existe mais pointe vers un ancien emplacement (exécutable déplacé), met à jour
///   automatiquement la cible du `.lnk` vers `current_exe` et retourne `true`.
/// - Si le raccourci existe et pointe déjà vers `current_exe`, retourne `true`.
/// - Si le raccourci n'existe pas, retourne `false`.
pub fn inspect_and_repair_startup_shortcut_at(lnk_path: &Path, current_exe: &Path) -> bool {
    let Some(existing_target) = read_startup_shortcut_target_at(lnk_path) else {
        return false;
    };

    let same_target = paths_refer_to_same_exe(&existing_target, current_exe);
    if !same_target {
        let _ = create_or_update_startup_shortcut_at(lnk_path, current_exe);
    }
    true
}

fn paths_refer_to_same_exe(a: &Path, b: &Path) -> bool {
    let norm_a = a.to_string_lossy().replace('/', "\\").to_lowercase();
    let norm_b = b.to_string_lossy().replace('/', "\\").to_lowercase();
    norm_a.trim_start_matches(r"\\?\") == norm_b.trim_start_matches(r"\\?\")
}

/// Synchronise le réglage `start_with_windows` de `AppSettings` avec l'état réel du raccourci
/// dans le dossier Startup de l'utilisateur courant (hors mode test).
pub fn sync_settings_with_real_startup_shortcut(settings: &mut AppSettings, storage: &mut Storage) {
    if cfg!(test) {
        return;
    }
    let Ok(lnk_path) = default_startup_shortcut_path() else {
        return;
    };
    let Ok(current_exe) = std::env::current_exe() else {
        return;
    };

    let shortcut_present = inspect_and_repair_startup_shortcut_at(&lnk_path, &current_exe);
    if settings.start_with_windows != shortcut_present {
        settings.start_with_windows = shortcut_present;
        let _ = storage.set_setting(
            KEY_START_WITH_WINDOWS,
            if shortcut_present { "1" } else { "0" },
        );
    }
}

use std::os::windows::ffi::OsStrExt;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_settings_persistence_defaults_and_theme_resolution() {
        let mut store = Storage::open_in_memory().unwrap();
        let mut settings = AppSettings::load(&store);

        // Vérifier toutes les valeurs par défaut exigées au Jalon 2C
        assert_eq!(settings.theme_mode, ThemeMode::Dark);
        assert_eq!(settings.resolved_theme(), ResolvedTheme::Dark);
        assert!(!settings.close_to_tray);
        assert!(!settings.start_minimized_to_tray);
        assert!(settings.confirm_exit_active_sessions);
        assert!(!settings.restore_rdp_sessions);
        assert!(!settings.start_with_windows);
        assert!(settings.show_recent_connections);
        assert_eq!(settings.max_recent_connections, 10);
        assert!(settings.rdp_default_clipboard);
        assert!(settings.rdp_default_dynamic_resolution);
        assert_eq!(
            settings.rdp_keyboard_hook_mode,
            RdpKeyboardHookMode::OnRemote
        );
        assert_eq!(settings.anydesk_path, "");
        assert_eq!(settings.rustdesk_path, "");

        // Modifier toutes les préférences et sauvegarder
        settings.theme_mode = ThemeMode::Light;
        settings.close_to_tray = true;
        settings.start_minimized_to_tray = true;
        settings.confirm_exit_active_sessions = false;
        settings.restore_rdp_sessions = true;
        settings.start_with_windows = true;
        settings.show_recent_connections = false;
        settings.max_recent_connections = 25;
        settings.rdp_default_clipboard = false;
        settings.rdp_default_dynamic_resolution = false;
        settings.rdp_keyboard_hook_mode = RdpKeyboardHookMode::OnLocal;
        settings.anydesk_path = r"C:\Program Files (x86)\AnyDesk\AnyDesk.exe".to_string();
        settings.rustdesk_path = r"C:\Program Files\RustDesk\rustdesk.exe".to_string();
        settings.save_all(&mut store).unwrap();
        settings.save_restorable_rdp_sessions(&mut store, &[10, 20, 10]);

        let reloaded = AppSettings::load(&store);
        assert_eq!(reloaded.theme_mode, ThemeMode::Light);
        assert_eq!(
            reloaded.rdp_keyboard_hook_mode,
            RdpKeyboardHookMode::OnLocal
        );
        assert_eq!(
            reloaded.anydesk_path,
            r"C:\Program Files (x86)\AnyDesk\AnyDesk.exe"
        );
        assert_eq!(
            reloaded.rustdesk_path,
            r"C:\Program Files\RustDesk\rustdesk.exe"
        );
        assert_eq!(reloaded.resolved_theme(), ResolvedTheme::Light);
        assert!(reloaded.close_to_tray);
        assert!(reloaded.start_minimized_to_tray);
        assert!(!reloaded.confirm_exit_active_sessions);
        assert!(reloaded.restore_rdp_sessions);
        assert!(reloaded.start_with_windows);
        assert!(!reloaded.show_recent_connections);
        assert_eq!(reloaded.max_recent_connections, 25);
        assert!(!reloaded.rdp_default_clipboard);
        assert!(!reloaded.rdp_default_dynamic_resolution);
        assert_eq!(reloaded.restorable_rdp_connection_ids, vec![10, 20]);

        // Si close_to_tray est désactivé, start_minimized_to_tray doit être forcé à false
        let mut s2 = reloaded;
        s2.close_to_tray = false;
        s2.save_all(&mut store).unwrap();
        let reloaded2 = AppSettings::load(&store);
        assert!(!reloaded2.close_to_tray);
        assert!(!reloaded2.start_minimized_to_tray);
    }

    #[test]
    fn test_startup_shortcut_lnk_create_read_repair_and_remove() {
        let temp_dir =
            std::env::temp_dir().join(format!("malvision_startup_lnk_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();

        let lnk_path = temp_dir.join(STARTUP_SHORTCUT_FILENAME);
        let exe_v1 = temp_dir.join("folder_a").join("Malvision.exe");
        let exe_v2 = temp_dir.join("folder_b").join("Malvision.exe");
        std::fs::create_dir_all(exe_v1.parent().unwrap()).unwrap();
        std::fs::create_dir_all(exe_v2.parent().unwrap()).unwrap();
        std::fs::write(&exe_v1, b"stub_exe_1").unwrap();
        std::fs::write(&exe_v2, b"stub_exe_2").unwrap();

        // 1. Aucun raccourci au départ
        assert!(!inspect_and_repair_startup_shortcut_at(&lnk_path, &exe_v1));

        // 2. Activer le démarrage automatique -> crée Malvision.lnk pointant vers exe_v1
        apply_startup_shortcut_at(&lnk_path, &exe_v1, true).unwrap();
        assert!(lnk_path.exists());
        let read_target = read_startup_shortcut_target_at(&lnk_path).unwrap();
        assert!(paths_refer_to_same_exe(&read_target, &exe_v1));

        // 3. Simuler le déplacement de l'exécutable portable vers exe_v2 :
        // inspect_and_repair_startup_shortcut_at doit réparer la cible du même fichier Malvision.lnk
        // sans jamais créer un second raccourci.
        assert!(inspect_and_repair_startup_shortcut_at(&lnk_path, &exe_v2));
        let repaired_target = read_startup_shortcut_target_at(&lnk_path).unwrap();
        assert!(paths_refer_to_same_exe(&repaired_target, &exe_v2));
        let entries_count = std::fs::read_dir(&temp_dir)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "lnk"))
            .count();
        assert_eq!(entries_count, 1, "Un seul raccourci .lnk doit exister");

        // 4. Désactiver le démarrage automatique -> supprime Malvision.lnk
        apply_startup_shortcut_at(&lnk_path, &exe_v2, false).unwrap();
        assert!(!lnk_path.exists());
        assert!(!inspect_and_repair_startup_shortcut_at(&lnk_path, &exe_v2));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
