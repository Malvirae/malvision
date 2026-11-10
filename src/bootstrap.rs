//! Gestion du bootstrap et de l'emplacement personnalisable de la base SQLite `malvision.db`.
//!
//! Ce module permet à l'utilisateur de choisir un dossier personnalisé pour stocker sa base SQLite,
//! par exemple sur un disque secondaire `D:\Applications\Malvision\malvision.db`.
//!
//! Principes stricts de robustesse et de sécurité :
//! - Le fichier de bootstrap est stocké dans `%LOCALAPPDATA%\Malvision\bootstrap.json`.
//! - Il ne contient aucun mot de passe ni secret (uniquement la version et le chemin absolu).
//! - L'ancienne base n'est JAMAIS supprimée, déplacée ou renommée.
//! - Si un nouveau dossier vide est choisi, une sauvegarde SQLite en ligne intègre est copiée.
//! - Si une base existe déjà dans le dossier cible, elle n'est jamais écrasée automatiquement.
//! - Si la base personnalisée est absente au démarrage (clé USB ou disque externe débranché),
//!   l'application ne crée jamais silencieusement une base vide et ne bascule pas silencieusement.
//!   Elle affiche un diagnostic explicite permettant de réessayer, de choisir un autre dossier
//!   ou de revenir à la base par défaut.

use crate::storage::default_app_data_dir;
use std::path::{Path, PathBuf};
use windows::Win32::Storage::FileSystem::GetDriveTypeW;
use windows_core::PCWSTR;

/// Constantes standard Win32 retournées par `GetDriveTypeW`.
const DRIVE_REMOVABLE: u32 = 2;
const DRIVE_FIXED: u32 = 3;
const DRIVE_REMOTE: u32 = 4;

/// Nom fixe et obligatoire du fichier de base de données Malvision.
pub const DATABASE_FILENAME: &str = "malvision.db";

/// Nom du fichier de bootstrap persistant dans `%LOCALAPPDATA%\Malvision`.
pub const BOOTSTRAP_FILENAME: &str = "bootstrap.json";

/// Configuration de bootstrap lue avant l'initialisation de SQLite.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BootstrapConfig {
    pub version: u32,
    pub custom_db_path: Option<PathBuf>,
}

impl Default for BootstrapConfig {
    fn default() -> Self {
        Self {
            version: 1,
            custom_db_path: None,
        }
    }
}

/// Retourne le chemin du fichier de configuration bootstrap (`%LOCALAPPDATA%\Malvision\bootstrap.json`).
pub fn bootstrap_file_path() -> Result<PathBuf, String> {
    let app_dir = default_app_data_dir()?;
    Ok(app_dir.join(BOOTSTRAP_FILENAME))
}

/// Retourne le chemin de la base de données standard par défaut (`%LOCALAPPDATA%\Malvision\malvision.db`).
pub fn default_database_path() -> Result<PathBuf, String> {
    let app_dir = default_app_data_dir()?;
    Ok(app_dir.join(DATABASE_FILENAME))
}

/// Charge la configuration de bootstrap depuis le disque.
/// En cas d'absence de fichier ou de format JSON invalide, retourne la configuration par défaut.
pub fn load_bootstrap() -> BootstrapConfig {
    let path = match bootstrap_file_path() {
        Ok(p) => p,
        Err(_) => return BootstrapConfig::default(),
    };

    if !path.exists() {
        return BootstrapConfig::default();
    }

    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return BootstrapConfig::default(),
    };

    parse_bootstrap_json(&content)
}

pub fn parse_bootstrap_json(content: &str) -> BootstrapConfig {
    let val: serde_json::Value = match serde_json::from_str(content) {
        Ok(v) => v,
        Err(_) => return BootstrapConfig::default(),
    };

    let version = val.get("version").and_then(|v| v.as_u64()).unwrap_or(1) as u32;
    let custom_db_path = val
        .get("custom_db_path")
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty())
        .map(PathBuf::from);

    BootstrapConfig {
        version,
        custom_db_path,
    }
}

pub fn serialize_bootstrap_json(config: &BootstrapConfig) -> String {
    let custom_str = config
        .custom_db_path
        .as_ref()
        .map(|p| serde_json::Value::String(p.to_string_lossy().to_string()))
        .unwrap_or(serde_json::Value::Null);

    let obj = serde_json::json!({
        "version": config.version,
        "custom_db_path": custom_str
    });

    serde_json::to_string_pretty(&obj).unwrap_or_else(|_| "{}".to_string())
}

/// Sauvegarde la configuration de bootstrap sur le disque.
pub fn save_bootstrap(config: &BootstrapConfig) -> Result<(), String> {
    let path = bootstrap_file_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            format!(
                "Impossible de créer le dossier de bootstrap '{}' : {e}",
                parent.display()
            )
        })?;
    }

    let json_text = serialize_bootstrap_json(config);

    // Écriture atomique via fichier temporaire
    let temp_path = path.with_extension("tmp_bootstrap");
    std::fs::write(&temp_path, json_text.as_bytes())
        .map_err(|e| format!("Impossible d'écrire le fichier temporaire de bootstrap : {e}"))?;

    crate::storage::atomic_replace_file(&temp_path, &path)
        .map_err(|e| format!("Impossible de finaliser le fichier de bootstrap : {e}"))?;

    Ok(())
}

/// Résout le chemin absolu de la base de données actuellement configurée.
/// Si un emplacement personnalisé est défini, retourne ce chemin.
/// Sinon, retourne l'emplacement par défaut dans `%LOCALAPPDATA%`.
#[allow(dead_code)]
pub fn resolve_active_database_path() -> Result<PathBuf, String> {
    let cfg = load_bootstrap();
    if let Some(custom) = cfg.custom_db_path {
        Ok(custom)
    } else {
        default_database_path()
    }
}

/// Définit un nouvel emplacement personnalisé de base de données (ou `None` pour revenir au défaut).
pub fn set_custom_database_path(path: Option<&Path>) -> Result<(), String> {
    let mut cfg = load_bootstrap();
    cfg.custom_db_path = path.map(|p| p.to_path_buf());
    save_bootstrap(&cfg)
}

/// Indique si Malvision utilise actuellement un emplacement personnalisé différent du dossier par défaut.
pub fn is_using_custom_database() -> bool {
    let cfg = load_bootstrap();
    cfg.custom_db_path.is_some()
}

/// Vérifie si un dossier cible existe et si les permissions d'écriture y sont accordées.
pub fn validate_directory_writable(dir: &Path) -> Result<(), String> {
    if !dir.exists() {
        std::fs::create_dir_all(dir).map_err(|e| {
            format!(
                "Impossible de créer le dossier cible '{}' : {e}",
                dir.display()
            )
        })?;
    }

    // Tester l'écriture et la suppression d'un fichier témoin éphémère
    let pid = std::process::id();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let test_file = dir.join(format!(".malvision_perm_test_{pid}_{nanos}.tmp"));

    std::fs::write(&test_file, b"OK").map_err(|e| {
        format!(
            "Permissions insuffisantes : impossible d'écrire dans le dossier '{}' : {e}",
            dir.display()
        )
    })?;

    let _ = std::fs::remove_file(&test_file);
    Ok(())
}

/// Diagnostic de l'emplacement cible : détecte si le chemin se trouve sur un partage réseau (UNC),
/// un lecteur réseau mappé ou un support amovible.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DriveKind {
    LocalFixed,
    Removable,
    NetworkRemote,
    Unknown,
}

pub fn check_drive_kind(path: &Path) -> DriveKind {
    let path_str = path.to_string_lossy();
    if path_str.starts_with(r"\\") || path_str.starts_with("//") {
        return DriveKind::NetworkRemote;
    }

    // Extraire la racine du lecteur (ex: "C:\\")
    let root = if let Some(first) = path.components().next() {
        let mut r = first.as_os_str().to_string_lossy().to_string();
        if !r.ends_with('\\') && !r.ends_with('/') {
            r.push('\\');
        }
        r
    } else {
        return DriveKind::Unknown;
    };

    let mut root_w: Vec<u16> = root.encode_utf16().collect();
    root_w.push(0);

    let drive_type = unsafe { GetDriveTypeW(PCWSTR(root_w.as_ptr())) };
    match drive_type {
        t if t == DRIVE_REMOTE => DriveKind::NetworkRemote,
        t if t == DRIVE_REMOVABLE => DriveKind::Removable,
        t if t == DRIVE_FIXED => DriveKind::LocalFixed,
        _ => DriveKind::Unknown,
    }
}

/// Plan d'action lors de la sélection d'un nouveau dossier d'emplacement de données.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LocationChangePlan {
    /// Le dossier sélectionné est identique à l'emplacement actuellement utilisé.
    AlreadyCurrent,
    /// Aucun fichier `malvision.db` n'existe dans le dossier cible. Une copie SQLite en ligne doit être effectuée.
    CopyRequired(PathBuf),
    /// Un fichier `malvision.db` existe déjà dans le dossier cible. Il doit être validé avant proposition d'utilisation.
    ExistingFound(PathBuf),
}

/// Analyse et prépare le changement d'emplacement de la base de données.
pub fn plan_database_location_change(
    current_db: &Path,
    target_folder: &Path,
) -> Result<LocationChangePlan, String> {
    validate_directory_writable(target_folder)?;

    let target_db = target_folder.join(DATABASE_FILENAME);

    // Comparaison canonique des chemins pour éviter les doublons avec barres obliques différentes
    let cur_can = current_db
        .canonicalize()
        .unwrap_or_else(|_| current_db.to_path_buf());
    let tgt_can = target_db
        .canonicalize()
        .unwrap_or_else(|_| target_db.clone());

    if cur_can == tgt_can {
        return Ok(LocationChangePlan::AlreadyCurrent);
    }

    if target_db.exists() {
        Ok(LocationChangePlan::ExistingFound(target_db))
    } else {
        Ok(LocationChangePlan::CopyRequired(target_db))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bootstrap_defaults_and_roundtrip() {
        let dir = std::env::temp_dir();
        let pid = std::process::id();
        let test_file = dir.join(format!("malvision_test_bootstrap_{pid}.json"));

        let _ = std::fs::remove_file(&test_file);

        let cfg = BootstrapConfig {
            version: 1,
            custom_db_path: Some(PathBuf::from(r"D:\Data\malvision.db")),
        };

        let json_text = serialize_bootstrap_json(&cfg);
        std::fs::write(&test_file, json_text).unwrap();

        let content = std::fs::read_to_string(&test_file).unwrap();
        let loaded = parse_bootstrap_json(&content);
        assert_eq!(loaded.version, 1);
        assert_eq!(
            loaded.custom_db_path,
            Some(PathBuf::from(r"D:\Data\malvision.db"))
        );

        let _ = std::fs::remove_file(&test_file);
    }

    #[test]
    fn test_validate_directory_writable_success_and_plan() {
        let dir = std::env::temp_dir();
        let pid = std::process::id();
        let test_folder = dir.join(format!("malvision_plan_test_{pid}"));
        std::fs::create_dir_all(&test_folder).unwrap();

        assert!(validate_directory_writable(&test_folder).is_ok());

        let current_db = test_folder.join("current.db");
        std::fs::write(&current_db, b"sqlite").unwrap();

        let other_folder = dir.join(format!("malvision_plan_other_{pid}"));
        std::fs::create_dir_all(&other_folder).unwrap();

        let plan = plan_database_location_change(&current_db, &other_folder).unwrap();
        match plan {
            LocationChangePlan::CopyRequired(p) => {
                assert_eq!(p, other_folder.join(DATABASE_FILENAME));
            }
            _ => panic!("Le dossier cible étant vide, CopyRequired était attendu"),
        }

        // Créer un malvision.db factice dans other_folder
        let existing_target = other_folder.join(DATABASE_FILENAME);
        std::fs::write(&existing_target, b"existing").unwrap();

        let plan2 = plan_database_location_change(&current_db, &other_folder).unwrap();
        match plan2 {
            LocationChangePlan::ExistingFound(p) => {
                assert_eq!(p, existing_target);
            }
            _ => panic!("ExistingFound était attendu"),
        }

        let _ = std::fs::remove_dir_all(&test_folder);
        let _ = std::fs::remove_dir_all(&other_folder);
    }
}
