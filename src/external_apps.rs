//! Intégration et lancement sécurisé des applications clientes distantes externes
//! (AnyDesk et RustDesk) pour le Jalon 3 de Malvision.
//!
//! Exigences strictes :
//! - Utilisation exclusive de l'API Win32 `CreateProcessW` sans intermédiaire shell (`cmd.exe`, PowerShell).
//! - Encodage et échappement sécurisés des arguments contre toute tentative d'injection.
//! - Aucun mot de passe passé en ligne de commande.
//! - Détection automatique des exécutables installés (Program Files, AppData, Registre Windows).
//! - Validation préalable de l'existence et des droits d'exécution.
//! - Dialogue standard Windows de sélection de fichier (`IFileOpenDialog`).

use std::path::{Path, PathBuf};
use windows::Win32::Foundation::{CloseHandle, HWND, MAX_PATH};
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize,
};
use windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ, RegGetValueW,
};
use windows::Win32::System::Threading::{CreateProcessW, PROCESS_INFORMATION, STARTUPINFOW};
use windows::Win32::UI::Shell::{
    Common::COMDLG_FILTERSPEC, FOS_FILEMUSTEXIST, FOS_FORCEFILESYSTEM, FileOpenDialog, IFileDialog,
    IFileOpenDialog, SIGDN_FILESYSPATH,
};
use windows_core::{Interface, PCWSTR, PWSTR, w};

/// Détecte automatiquement le chemin de l'exécutable `AnyDesk.exe` sur le système.
pub fn detect_anydesk_path() -> Option<PathBuf> {
    // 1. Registre App Paths (HKLM puis HKCU)
    if let Some(path) = query_app_path_registry(
        HKEY_LOCAL_MACHINE,
        w!("SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\App Paths\\AnyDesk.exe"),
    ) && path.is_file()
    {
        return Some(path);
    }
    if let Some(path) = query_app_path_registry(
        HKEY_CURRENT_USER,
        w!("SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\App Paths\\AnyDesk.exe"),
    ) && path.is_file()
    {
        return Some(path);
    }

    // 2. Emplacements standards d'installation
    let mut candidates = Vec::new();

    if let Ok(pf86) = std::env::var("ProgramFiles(x86)") {
        candidates.push(PathBuf::from(pf86).join("AnyDesk").join("AnyDesk.exe"));
    }
    if let Ok(pf) = std::env::var("ProgramFiles") {
        candidates.push(PathBuf::from(pf).join("AnyDesk").join("AnyDesk.exe"));
    }
    if let Ok(appdata) = std::env::var("APPDATA") {
        candidates.push(PathBuf::from(appdata).join("AnyDesk").join("AnyDesk.exe"));
    }
    if let Ok(localappdata) = std::env::var("LOCALAPPDATA") {
        candidates.push(
            PathBuf::from(localappdata)
                .join("AnyDesk")
                .join("AnyDesk.exe"),
        );
    }

    // Disque système par défaut
    candidates.push(PathBuf::from(r"C:\Program Files (x86)\AnyDesk\AnyDesk.exe"));
    candidates.push(PathBuf::from(r"C:\Program Files\AnyDesk\AnyDesk.exe"));

    candidates.into_iter().find(|p| p.is_file())
}

/// Détecte automatiquement le chemin de l'exécutable `rustdesk.exe` sur le système.
pub fn detect_rustdesk_path() -> Option<PathBuf> {
    // 1. Registre App Paths
    for hkey in [HKEY_LOCAL_MACHINE, HKEY_CURRENT_USER] {
        for subkey in [
            w!("SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\App Paths\\rustdesk.exe"),
            w!("SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\App Paths\\RustDesk.exe"),
        ] {
            if let Some(path) = query_app_path_registry(hkey, subkey)
                && path.is_file()
            {
                return Some(path);
            }
        }
    }

    // 2. Emplacements standards
    let mut candidates = Vec::new();

    if let Ok(pf) = std::env::var("ProgramFiles") {
        candidates.push(PathBuf::from(&pf).join("RustDesk").join("rustdesk.exe"));
        candidates.push(PathBuf::from(pf).join("RustDesk").join("RustDesk.exe"));
    }
    if let Ok(pf86) = std::env::var("ProgramFiles(x86)") {
        candidates.push(PathBuf::from(&pf86).join("RustDesk").join("rustdesk.exe"));
        candidates.push(PathBuf::from(pf86).join("RustDesk").join("RustDesk.exe"));
    }
    if let Ok(appdata) = std::env::var("APPDATA") {
        candidates.push(
            PathBuf::from(&appdata)
                .join("RustDesk")
                .join("rustdesk.exe"),
        );
        candidates.push(PathBuf::from(appdata).join("RustDesk").join("RustDesk.exe"));
    }
    if let Ok(localappdata) = std::env::var("LOCALAPPDATA") {
        candidates.push(
            PathBuf::from(&localappdata)
                .join("RustDesk")
                .join("rustdesk.exe"),
        );
        candidates.push(
            PathBuf::from(localappdata)
                .join("RustDesk")
                .join("RustDesk.exe"),
        );
    }

    candidates.push(PathBuf::from(r"C:\Program Files\RustDesk\rustdesk.exe"));
    candidates.push(PathBuf::from(r"C:\Program Files\RustDesk\RustDesk.exe"));

    candidates.into_iter().find(|p| p.is_file())
}

fn query_app_path_registry(hkey: HKEY, subkey: PCWSTR) -> Option<PathBuf> {
    unsafe {
        let mut buf = [0u16; MAX_PATH as usize];
        let mut cb_data = (buf.len() * std::mem::size_of::<u16>()) as u32;
        let res = RegGetValueW(
            hkey,
            subkey,
            None,
            RRF_RT_REG_SZ,
            None,
            Some(buf.as_mut_ptr() as *mut _),
            Some(&mut cb_data),
        );
        if res.is_ok() {
            let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
            if len > 0 {
                let path_str = String::from_utf16_lossy(&buf[..len]);
                let cleaned = path_str.trim().trim_matches('"');
                return Some(PathBuf::from(cleaned));
            }
        }
        None
    }
}

/// Valide qu'un chemin spécifié pointe vers un fichier exécutable `.exe` valide et existant.
pub fn validate_executable_path(path: &Path) -> Result<PathBuf, String> {
    if !path.exists() {
        return Err(format!("Le fichier '{}' est introuvable.", path.display()));
    }
    if !path.is_file() {
        return Err(format!(
            "'{}' est un répertoire et non un fichier exécutable.",
            path.display()
        ));
    }
    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if ext != "exe" {
        return Err(format!(
            "Le fichier '{}' n'est pas un exécutable Windows valide (.exe requis).",
            path.display()
        ));
    }
    Ok(path.to_path_buf())
}

/// Nettoie et valide l'identifiant distant pour éviter toute injection dans la ligne de commande.
/// N'autorise que les caractères alphanumériques, espaces, tirets, underscores, points et arobases.
pub fn sanitize_remote_id(raw_id: &str) -> Result<String, String> {
    let trimmed = raw_id.trim();
    if trimmed.is_empty() {
        return Err("L'identifiant ou alias distant ne peut pas être vide.".to_string());
    }

    // Caractères dangereux strictement interdits dans les identifiants
    const FORBIDDEN_CHARS: &[char] = &[
        '"', '\'', '`', ';', '&', '|', '<', '>', '\n', '\r', '\t', '\\', '/', '^', '%', '$',
    ];
    if trimmed.chars().any(|c| FORBIDDEN_CHARS.contains(&c)) {
        return Err(format!(
            "L'identifiant distant '{trimmed}' contient des caractères interdits."
        ));
    }

    // N'autoriser que les caractères valides pour un ID AnyDesk/RustDesk
    if !trimmed
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '@' | ' '))
    {
        return Err(format!(
            "L'identifiant distant '{trimmed}' n'est pas au format attendu."
        ));
    }

    Ok(trimmed.to_string())
}

/// Construit la ligne de commande UTF-16 correctement échappée pour AnyDesk :
/// `"C:\Path\To\AnyDesk.exe" "<id>"`
pub fn build_anydesk_command_line(exe_path: &Path, remote_id: Option<&str>) -> Vec<u16> {
    let mut cmd = format!("\"{}\"", exe_path.display());
    if let Some(id) = remote_id
        && let Ok(safe_id) = sanitize_remote_id(id)
    {
        cmd.push_str(&format!(" \"{safe_id}\""));
    }
    let mut utf16: Vec<u16> = cmd.encode_utf16().collect();
    utf16.push(0);
    utf16
}

/// Construit la ligne de commande UTF-16 correctement échappée pour RustDesk :
/// `"C:\Path\To\rustdesk.exe" --connect "<id>"`
pub fn build_rustdesk_command_line(exe_path: &Path, remote_id: Option<&str>) -> Vec<u16> {
    let mut cmd = format!("\"{}\"", exe_path.display());
    if let Some(id) = remote_id
        && let Ok(safe_id) = sanitize_remote_id(id)
    {
        cmd.push_str(&format!(" --connect \"{safe_id}\""));
    }
    let mut utf16: Vec<u16> = cmd.encode_utf16().collect();
    utf16.push(0);
    utf16
}

/// Lance un processus Windows via l'API Win32 native `CreateProcessW`.
///
/// Ne fait aucun appel shell (`cmd.exe` / PowerShell) et ne bloque pas l'UI.
pub fn launch_external_process(exe_path: &Path, cmd_line: &[u16]) -> Result<u32, String> {
    let valid_exe = validate_executable_path(exe_path)?;

    unsafe {
        let startup_info = STARTUPINFOW {
            cb: std::mem::size_of::<STARTUPINFOW>() as u32,
            ..Default::default()
        };
        let mut process_info = PROCESS_INFORMATION::default();

        let mut app_name_w: Vec<u16> = valid_exe.as_os_str().encode_wide().collect();
        app_name_w.push(0);

        // CreateProcessW requiert un buffer mutable pour lpCommandLine
        let mut cmd_buf = cmd_line.to_vec();

        let working_dir_w: Option<Vec<u16>> = valid_exe.parent().map(|dir| {
            let mut w: Vec<u16> = dir.as_os_str().encode_wide().collect();
            w.push(0);
            w
        });
        let lp_working_dir = working_dir_w
            .as_ref()
            .map(|w| PCWSTR(w.as_ptr()))
            .unwrap_or(PCWSTR::null());

        let res = CreateProcessW(
            PCWSTR(app_name_w.as_ptr()),
            Some(PWSTR(cmd_buf.as_mut_ptr())),
            None,
            None,
            false,
            Default::default(),
            None,
            lp_working_dir,
            &startup_info,
            &mut process_info,
        );

        if let Err(err) = res {
            return Err(format!(
                "Échec de CreateProcessW pour '{}' : {}",
                valid_exe.display(),
                err.message()
            ));
        }

        let pid = process_info.dwProcessId;
        let _ = CloseHandle(process_info.hThread);
        let _ = CloseHandle(process_info.hProcess);

        Ok(pid)
    }
}

/// Teste le lancement simple d'un exécutable externe (sans connexion) pour vérifier que le chemin fonctionne.
pub fn test_launch_app(exe_path: &Path) -> Result<String, String> {
    let valid_exe = validate_executable_path(exe_path)?;
    let cmd_line = build_anydesk_command_line(&valid_exe, None);
    let pid = launch_external_process(&valid_exe, &cmd_line)?;
    Ok(format!(
        "Lancement réussi de '{}' (Process ID : {}).",
        valid_exe
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("client"),
        pid
    ))
}

/// Lance une connexion AnyDesk vers un identifiant ou alias distant.
pub fn launch_anydesk_connection(configured_path: &str, remote_id: &str) -> Result<String, String> {
    let exe_path = if configured_path.trim().is_empty() {
        detect_anydesk_path().ok_or_else(|| {
            "Le chemin de AnyDesk.exe n'est pas configuré et aucune installation standard n'a été détectée. Veuillez renseigner le chemin dans Paramètres > Applications externes.".to_string()
        })?
    } else {
        validate_executable_path(Path::new(configured_path.trim()))?
    };

    let safe_id = sanitize_remote_id(remote_id)?;
    let cmd_line = build_anydesk_command_line(&exe_path, Some(&safe_id));
    let pid = launch_external_process(&exe_path, &cmd_line)?;

    Ok(format!(
        "Processus AnyDesk lancé (PID {pid}) vers l'identifiant '{safe_id}'. La fenêtre AnyDesk prend en charge la négociation et l'authentification."
    ))
}

/// Lance une connexion RustDesk vers un identifiant ou alias distant.
pub fn launch_rustdesk_connection(
    configured_path: &str,
    remote_id: &str,
) -> Result<String, String> {
    let exe_path = if configured_path.trim().is_empty() {
        detect_rustdesk_path().ok_or_else(|| {
            "Le chemin de RustDesk.exe n'est pas configuré et aucune installation standard n'a été détectée. Veuillez renseigner le chemin dans Paramètres > Applications externes.".to_string()
        })?
    } else {
        validate_executable_path(Path::new(configured_path.trim()))?
    };

    let safe_id = sanitize_remote_id(remote_id)?;
    let cmd_line = build_rustdesk_command_line(&exe_path, Some(&safe_id));
    let pid = launch_external_process(&exe_path, &cmd_line)?;

    Ok(format!(
        "Processus RustDesk lancé (PID {pid}) vers l'identifiant '{safe_id}'. La fenêtre RustDesk prend en charge la négociation et l'authentification."
    ))
}

/// Ouvre le dialogue standard Windows de sélection de fichier (`IFileOpenDialog`)
/// filtré sur les fichiers exécutables (`*.exe`).
pub fn browse_for_executable(parent_hwnd: HWND, title: &str) -> Option<PathBuf> {
    unsafe {
        let co_init = CoInitializeEx(None, windows::Win32::System::Com::COINIT_APARTMENTTHREADED);

        let result = (|| -> Option<PathBuf> {
            let dialog: IFileOpenDialog =
                CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER).ok()?;

            let mut title_w: Vec<u16> = title.encode_utf16().collect();
            title_w.push(0);
            let _ = dialog.SetTitle(PCWSTR(title_w.as_ptr()));

            let _ = dialog.SetOptions(FOS_FILEMUSTEXIST | FOS_FORCEFILESYSTEM);

            let filter_name = w!("Exécutables Windows (*.exe)");
            let filter_spec = w!("*.exe");
            let filters = [COMDLG_FILTERSPEC {
                pszName: filter_name,
                pszSpec: filter_spec,
            }];
            let _ = dialog
                .cast::<IFileDialog>()
                .map(|d| d.SetFileTypes(&filters));

            if dialog.Show(Some(parent_hwnd)).is_err() {
                return None; // Annulé par l'utilisateur
            }

            let item = dialog.GetResult().ok()?;
            let display_name_raw = item.GetDisplayName(SIGDN_FILESYSPATH).ok()?;
            let path_str = display_name_raw.to_string().ok()?;
            CoTaskMemFree(Some(display_name_raw.0 as *const _));

            Some(PathBuf::from(path_str))
        })();

        if co_init.is_ok() {
            CoUninitialize();
        }
        result
    }
}

use std::os::windows::ffi::OsStrExt;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_remote_id_accepts_valid_ids() {
        assert_eq!(sanitize_remote_id("123 456 789").unwrap(), "123 456 789");
        assert_eq!(sanitize_remote_id("987654321").unwrap(), "987654321");
        assert_eq!(
            sanitize_remote_id("pc-support@ad").unwrap(),
            "pc-support@ad"
        );
        assert_eq!(
            sanitize_remote_id("server_01.corp").unwrap(),
            "server_01.corp"
        );
    }

    #[test]
    fn test_sanitize_remote_id_rejects_empty_and_injections() {
        assert!(sanitize_remote_id("   ").is_err());
        assert!(sanitize_remote_id("12345; calc.exe").is_err());
        assert!(sanitize_remote_id("12345 & powershell").is_err());
        assert!(sanitize_remote_id("12345 | notepad").is_err());
        assert!(sanitize_remote_id("12345\" --bad-arg").is_err());
        assert!(sanitize_remote_id("12345'").is_err());
        assert!(sanitize_remote_id("12345\nwhoami").is_err());
        assert!(sanitize_remote_id("`dir`").is_err());
    }

    #[test]
    fn test_build_anydesk_command_line_encoding() {
        let exe = Path::new(r"C:\Program Files (x86)\AnyDesk\AnyDesk.exe");
        let cmd = build_anydesk_command_line(exe, Some("456 789 012"));
        let decoded = String::from_utf16_lossy(&cmd[..cmd.len() - 1]);
        assert_eq!(
            decoded,
            r#""C:\Program Files (x86)\AnyDesk\AnyDesk.exe" "456 789 012""#
        );

        let cmd_no_id = build_anydesk_command_line(exe, None);
        let decoded_no_id = String::from_utf16_lossy(&cmd_no_id[..cmd_no_id.len() - 1]);
        assert_eq!(
            decoded_no_id,
            r#""C:\Program Files (x86)\AnyDesk\AnyDesk.exe""#
        );
    }

    #[test]
    fn test_build_rustdesk_command_line_encoding() {
        let exe = Path::new(r"C:\Program Files\RustDesk\rustdesk.exe");
        let cmd = build_rustdesk_command_line(exe, Some("987654321"));
        let decoded = String::from_utf16_lossy(&cmd[..cmd.len() - 1]);
        assert_eq!(
            decoded,
            r#""C:\Program Files\RustDesk\rustdesk.exe" --connect "987654321""#
        );
    }

    #[test]
    fn test_validate_executable_path_checks_existence_and_extension() {
        let not_found = Path::new(r"C:\NonExistentDirectory\NonExistentApp.exe");
        assert!(validate_executable_path(not_found).is_err());

        let temp_dir = std::env::temp_dir();
        assert!(validate_executable_path(&temp_dir).is_err());

        // Créer un faux fichier non .exe
        let txt_file = temp_dir.join("test_malvision_fake.txt");
        let _ = std::fs::write(&txt_file, "hello");
        assert!(validate_executable_path(&txt_file).is_err());
        let _ = std::fs::remove_file(&txt_file);
    }
}
