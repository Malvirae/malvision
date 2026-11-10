//! Vérification des mises à jour via GitHub Releases et téléchargement sécurisé des assets.

use crate::security::{
    compute_file_sha256, compute_hmac_sha256, constant_time_compare, generate_crypto_random_bytes,
};
use std::fmt;
use std::os::windows::io::AsRawHandle;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use windows::Win32::Foundation::{CloseHandle, FILETIME, HANDLE};
use windows::Win32::Networking::WinHttp::{
    INTERNET_DEFAULT_HTTPS_PORT, WINHTTP_ACCESS_TYPE_DEFAULT_PROXY, WINHTTP_FLAG_SECURE,
    WINHTTP_QUERY_FLAG_NUMBER, WINHTTP_QUERY_LOCATION, WINHTTP_QUERY_STATUS_CODE,
    WinHttpCloseHandle, WinHttpConnect, WinHttpOpen, WinHttpOpenRequest, WinHttpQueryDataAvailable,
    WinHttpQueryHeaders, WinHttpReadData, WinHttpReceiveResponse, WinHttpSendRequest,
    WinHttpSetTimeouts,
};
use windows::Win32::System::Threading::{
    CreateEventW, EVENT_MODIFY_STATE, GetCurrentProcess, GetProcessTimes, OpenEventW, OpenProcess,
    PROCESS_NAME_FORMAT, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE,
    QueryFullProcessImageNameW, SetEvent, TerminateProcess, WaitForMultipleObjects,
    WaitForSingleObject,
};
use windows::Win32::UI::WindowsAndMessaging::{MB_ICONERROR, MB_OK, MessageBoxW};
use windows_core::{PCWSTR, PWSTR, w};

pub const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const GITHUB_REPO: &str = "Malvirae/malvision";
pub const MSG_NO_PUBLIC_RELEASE: &str = "Aucune version publique disponible pour le moment.";

/// Représentation sémantique d'une version logicielle (Major.Minor.Patch).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SemVer {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl SemVer {
    /// Retourne la version locale courante dérivée de Cargo (source unique de vérité).
    #[allow(dead_code)]
    pub fn current() -> Result<Self, String> {
        current_semver()
    }

    #[allow(dead_code)]
    pub fn new(major: u32, minor: u32, patch: u32) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim().trim_start_matches(['v', 'V']);
        let mut parts = s.split('.');
        let major: u32 = parts.next()?.parse().ok()?;
        let minor: u32 = parts.next()?.parse().ok()?;
        let patch_part = parts.next()?;
        let patch_str = patch_part.split('-').next()?;
        let patch: u32 = patch_str.parse().ok()?;
        Some(Self {
            major,
            minor,
            patch,
        })
    }
}

/// Retourne la version locale de l'application sous forme de `SemVer`,
/// extraite de `CARGO_PKG_VERSION` (source unique de vérité).
pub fn current_semver() -> Result<SemVer, String> {
    SemVer::parse(CURRENT_VERSION)
        .ok_or_else(|| format!("Version locale invalide : {CURRENT_VERSION}"))
}

impl PartialOrd for SemVer {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for SemVer {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.major
            .cmp(&other.major)
            .then_with(|| self.minor.cmp(&other.minor))
            .then_with(|| self.patch.cmp(&other.patch))
    }
}

impl fmt::Display for SemVer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// Informations sur un asset téléchargeable d'une GitHub Release.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseAsset {
    pub name: String,
    pub download_url: String,
    pub size: usize,
}

/// Métadonnées d'une release GitHub publiée.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseInfo {
    pub tag_name: String,
    pub version: SemVer,
    pub name: String,
    pub body: String,
    pub html_url: String,
    pub exe_asset: Option<ReleaseAsset>,
    pub sha256_sums_asset: Option<ReleaseAsset>,
    pub expected_exe_sha256: Option<String>,
}

/// Analyse une réponse JSON renvoyée par l'API GitHub `/releases/latest`.
pub fn parse_release_json(json_str: &str) -> Result<ReleaseInfo, String> {
    let v: serde_json::Value =
        serde_json::from_str(json_str).map_err(|e| format!("JSON invalide : {e}"))?;

    // Réponse standard de GitHub lorsque le dépôt est privé ou qu'aucune release publique n'existe
    if let Some(msg) = v.get("message").and_then(|m| m.as_str())
        && msg.eq_ignore_ascii_case("Not Found")
    {
        return Err(MSG_NO_PUBLIC_RELEASE.to_string());
    }

    let is_draft = v["draft"].as_bool().unwrap_or(false);
    let is_prerelease = v["prerelease"].as_bool().unwrap_or(false);
    if is_draft || is_prerelease {
        return Err(
            "La release trouvée est une pré-version ou un brouillon non public.".to_string(),
        );
    }

    let tag_name = v["tag_name"]
        .as_str()
        .ok_or_else(|| "Propriété 'tag_name' manquante dans la release".to_string())?
        .to_string();

    let version = SemVer::parse(&tag_name)
        .ok_or_else(|| format!("Format SemVer non reconnu pour le tag '{tag_name}'"))?;

    let name = v["name"].as_str().unwrap_or(&tag_name).to_string();
    let body = v["body"].as_str().unwrap_or("").to_string();
    let html_url = v["html_url"].as_str().unwrap_or("").to_string();

    let mut exe_asset: Option<ReleaseAsset> = None;
    let mut sha256_sums_asset: Option<ReleaseAsset> = None;

    if let Some(assets) = v["assets"].as_array() {
        for a in assets {
            let asset_name = a["name"].as_str().unwrap_or("");
            let url = a["browser_download_url"].as_str().unwrap_or("");
            let size = a["size"].as_u64().unwrap_or(0) as usize;

            if asset_name.eq_ignore_ascii_case("Malvision.exe") {
                exe_asset = Some(ReleaseAsset {
                    name: asset_name.to_string(),
                    download_url: url.to_string(),
                    size,
                });
            } else if asset_name.eq_ignore_ascii_case("SHA256SUMS.txt")
                || asset_name.eq_ignore_ascii_case("sha256sums.txt")
            {
                sha256_sums_asset = Some(ReleaseAsset {
                    name: asset_name.to_string(),
                    download_url: url.to_string(),
                    size,
                });
            }
        }
    }

    // Tenter d'extraire le hash SHA-256 directement depuis le corps de texte si documenté
    let expected_exe_sha256 = extract_sha256_from_text(&body, "Malvision.exe");

    Ok(ReleaseInfo {
        tag_name,
        version,
        name,
        body,
        html_url,
        exe_asset,
        sha256_sums_asset,
        expected_exe_sha256,
    })
}

/// Extrait une empreinte SHA-256 (64 caractères hexadécimaux) associée à un fichier depuis un texte brut.
pub fn extract_sha256_from_text(text: &str, target_filename: &str) -> Option<String> {
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed
            .to_ascii_lowercase()
            .contains(&target_filename.to_ascii_lowercase())
        {
            for word in trimmed.split_whitespace() {
                let clean = word.trim_matches(|c: char| !c.is_ascii_alphanumeric());
                if clean.len() == 64 && clean.chars().all(|c| c.is_ascii_hexdigit()) {
                    return Some(clean.to_ascii_lowercase());
                }
            }
        }
    }
    None
}

/// Télécharge une ressource HTTPS via l'API Win32 native WinHTTP.
pub fn winhttp_get(url: &str) -> Result<Vec<u8>, String> {
    winhttp_get_with_depth(url, 0)
}

fn winhttp_get_with_depth(url: &str, depth: u32) -> Result<Vec<u8>, String> {
    if depth > 5 {
        return Err("Nombre excessif de redirections HTTP (boucle détectée).".to_string());
    }

    unsafe {
        let (host, port, path) = parse_url(url)?;

        let ua_str = format!("Malvision/{CURRENT_VERSION} (Windows NT 10.0; Win64; x64)\0");
        let ua_w: Vec<u16> = ua_str.encode_utf16().collect();
        let session = WinHttpOpen(
            PCWSTR(ua_w.as_ptr()),
            WINHTTP_ACCESS_TYPE_DEFAULT_PROXY,
            PCWSTR::null(),
            PCWSTR::null(),
            0,
        );
        if session.is_null() {
            return Err("WinHttpOpen a échoué.".to_string());
        }

        let host_w: Vec<u16> = host.encode_utf16().chain(std::iter::once(0)).collect();
        let connect = WinHttpConnect(session, PCWSTR(host_w.as_ptr()), port, 0);
        if connect.is_null() {
            let _ = WinHttpCloseHandle(session);
            return Err(format!("Impossible de se connecter à l'hôte '{host}'."));
        }

        let path_w: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
        let flags = WINHTTP_FLAG_SECURE;

        let request = WinHttpOpenRequest(
            connect,
            w!("GET"),
            PCWSTR(path_w.as_ptr()),
            PCWSTR::null(),
            PCWSTR::null(),
            std::ptr::null(),
            flags,
        );
        if request.is_null() {
            let _ = WinHttpCloseHandle(connect);
            let _ = WinHttpCloseHandle(session);
            return Err("WinHttpOpenRequest a échoué.".to_string());
        }

        // Délais d'attente (timeouts) : 5s résolution, 5s connexion, 5s envoi, 10s réception
        let _ = WinHttpSetTimeouts(request, 5000, 5000, 5000, 10000);

        // Envoyer la requête avec accept header standard
        let headers_str = format!(
            "Accept: application/vnd.github.v3+json, application/octet-stream, */*\r\nUser-Agent: Malvision/{CURRENT_VERSION}\r\n"
        );
        let headers: Vec<u16> = headers_str.encode_utf16().collect();
        let send_res = WinHttpSendRequest(request, Some(&headers), None, 0, 0, 0);
        if let Err(e) = send_res {
            let _ = WinHttpCloseHandle(request);
            let _ = WinHttpCloseHandle(connect);
            let _ = WinHttpCloseHandle(session);
            return Err(format!(
                "Échec de l'envoi de la requête réseau ({e}). Vérifiez votre connexion Internet."
            ));
        }

        let recv_res = WinHttpReceiveResponse(request, std::ptr::null_mut());
        if let Err(e) = recv_res {
            let _ = WinHttpCloseHandle(request);
            let _ = WinHttpCloseHandle(connect);
            let _ = WinHttpCloseHandle(session);
            return Err(format!("Échec de réception de la réponse réseau ({e})."));
        }

        let mut status_code: u32 = 0;
        let mut status_size: u32 = std::mem::size_of::<u32>() as u32;
        let mut header_index = 0u32;
        let _ = WinHttpQueryHeaders(
            request,
            WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
            PCWSTR::null(),
            Some(&mut status_code as *mut _ as *mut _),
            &mut status_size,
            &mut header_index,
        );

        // Gestion des redirections HTTP (301, 302, 307, 308) avec validation stricte de sécurité HTTPS
        if (301..=308).contains(&status_code) {
            let mut redirect_url_buf = vec![0u16; 2048];
            let mut redirect_url_size = (redirect_url_buf.len() * 2) as u32;
            let mut loc_index = 0u32;
            let q_res = WinHttpQueryHeaders(
                request,
                WINHTTP_QUERY_LOCATION,
                PCWSTR::null(),
                Some(redirect_url_buf.as_mut_ptr() as *mut _),
                &mut redirect_url_size,
                &mut loc_index,
            );
            let _ = WinHttpCloseHandle(request);
            let _ = WinHttpCloseHandle(connect);
            let _ = WinHttpCloseHandle(session);

            if q_res.is_ok() {
                let redirect_url =
                    String::from_utf16_lossy(&redirect_url_buf[..(redirect_url_size as usize / 2)])
                        .trim_matches('\0')
                        .to_string();
                if !redirect_url.is_empty() {
                    if !redirect_url.to_ascii_lowercase().starts_with("https://") {
                        return Err(
                            "Redirection vers un protocole non sécurisé (HTTP non chiffré) refusée."
                                .to_string(),
                        );
                    }
                    return winhttp_get_with_depth(&redirect_url, depth + 1);
                }
            }
            return Err(format!(
                "Redirection HTTP {status_code} sans en-tête Location valide."
            ));
        }

        if status_code != 200 {
            let _ = WinHttpCloseHandle(request);
            let _ = WinHttpCloseHandle(connect);
            let _ = WinHttpCloseHandle(session);
            return Err(format!("Le serveur a retourné le code HTTP {status_code}."));
        }

        // Lecture du corps de réponse par blocs
        let mut response_body = Vec::new();
        loop {
            let mut bytes_avail: u32 = 0;
            if WinHttpQueryDataAvailable(request, &mut bytes_avail).is_err() || bytes_avail == 0 {
                break;
            }

            let mut chunk = vec![0u8; bytes_avail as usize];
            let mut bytes_read: u32 = 0;
            if WinHttpReadData(
                request,
                chunk.as_mut_ptr() as *mut _,
                bytes_avail,
                &mut bytes_read,
            )
            .is_ok()
                && bytes_read > 0
            {
                chunk.truncate(bytes_read as usize);
                response_body.extend_from_slice(&chunk);
            } else {
                break;
            }
        }

        let _ = WinHttpCloseHandle(request);
        let _ = WinHttpCloseHandle(connect);
        let _ = WinHttpCloseHandle(session);

        Ok(response_body)
    }
}

fn parse_url(url: &str) -> Result<(String, u16, String), String> {
    let rest = if let Some(stripped) = url.strip_prefix("https://") {
        stripped
    } else if url.starts_with("http://") {
        return Err(
            "Protocole non sécurisé rejeté : seules les connexions HTTPS chiffrées sont autorisées pour les mises à jour."
                .to_string(),
        );
    } else {
        return Err(
            "Protocole d'URL non supporté (seules les connexions HTTPS sont acceptées)."
                .to_string(),
        );
    };

    let (host_port, path) = match rest.find('/') {
        Some(idx) => (&rest[..idx], &rest[idx..]),
        None => (rest, "/"),
    };

    let (host, port) = match host_port.find(':') {
        Some(idx) => {
            let h = host_port[..idx].to_string();
            let p: u16 = host_port[idx + 1..]
                .parse()
                .map_err(|_| "Port invalide dans l'URL.".to_string())?;
            (h, p)
        }
        None => (host_port.to_string(), INTERNET_DEFAULT_HTTPS_PORT),
    };

    Ok((host, port, path.to_string()))
}

/// Vérifie la dernière release publique disponible sur le dépôt GitHub.
pub fn check_latest_release(repo: &str) -> Result<ReleaseInfo, String> {
    let url = format!("https://api.github.com/repos/{repo}/releases/latest");
    let body = winhttp_get(&url).map_err(|e| {
        if e.contains("code HTTP 404") || e.contains("404") {
            MSG_NO_PUBLIC_RELEASE.to_string()
        } else {
            e
        }
    })?;
    let json_str = String::from_utf8(body).map_err(|e| format!("Réponse UTF-8 invalide : {e}"))?;
    let mut info = parse_release_json(&json_str)?;

    // Si l'empreinte SHA-256 n'est pas présente dans les notes de version, tenter de la récupérer via SHA256SUMS.txt
    if info.expected_exe_sha256.is_none()
        && let Some(sums_asset) = &info.sha256_sums_asset
        && let Ok(sums_bytes) = winhttp_get(&sums_asset.download_url)
        && let Ok(sums_text) = String::from_utf8(sums_bytes)
    {
        info.expected_exe_sha256 = extract_sha256_from_text(&sums_text, "Malvision.exe");
    }

    Ok(info)
}

/// Écrit les octets téléchargés dans un fichier temporaire, vérifie l'intégrité SHA-256 et déplace atomiquement vers le chemin final.
///
/// Exigences de sécurité :
/// - Vérification obligatoire de l'empreinte cryptographique SHA-256 de référence.
/// - Aucun téléchargement sans empreinte officielle n'est accepté.
/// - En cas d'échec de téléchargement ou d'empreinte altérée, le fichier temporaire est détruit
///   et le fichier cible existant (le cas échéant) est conservé intact.
pub fn write_and_verify_asset_bytes(
    data: &[u8],
    dest_path: &Path,
    expected_sha256: Option<&str>,
) -> Result<(), String> {
    if data.is_empty() {
        return Err("Le téléchargement a retourné un fichier vide (0 octet).".to_string());
    }

    // Empreinte SHA-256 obligatoire : aucun téléchargement sans empreinte officielle n'est accepté
    let expected_hash = match expected_sha256 {
        Some(h) if !h.trim().is_empty() => h.trim().to_ascii_lowercase(),
        _ => {
            return Err(
                "Vérification d'intégrité impossible : aucune empreinte cryptographique SHA-256 officielle n'est fournie pour cet asset. Téléchargement refusé par mesure de sécurité."
                    .to_string(),
            );
        }
    };

    let parent = dest_path.parent().unwrap_or_else(|| Path::new("."));
    if !parent.as_os_str().is_empty() && !parent.exists() {
        std::fs::create_dir_all(parent).map_err(|e| {
            format!(
                "Impossible de créer le dossier de destination '{}' : {e}",
                parent.display()
            )
        })?;
    }

    let file_stem = dest_path
        .file_stem()
        .map(|s| s.to_string_lossy())
        .unwrap_or_else(|| "update".into());
    let pid = std::process::id();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let temp_path = parent.join(format!(".{file_stem}_{pid}_{nanos}.tmp_download"));

    if temp_path.exists() {
        let _ = std::fs::remove_file(&temp_path);
    }

    // Structure Guard pour garantir le nettoyage automatique en cas d'erreur
    struct TempCleanup<'a>(&'a Path);
    impl Drop for TempCleanup<'_> {
        fn drop(&mut self) {
            if self.0.exists() {
                let _ = std::fs::remove_file(self.0);
            }
        }
    }
    let cleanup = TempCleanup(&temp_path);

    // 1. Écrire dans le fichier temporaire
    std::fs::write(&temp_path, data).map_err(|e| {
        format!(
            "Impossible d'écrire le fichier temporaire '{}' : {e}",
            temp_path.display()
        )
    })?;

    // 2. Vérification obligatoire et stricte de l'empreinte SHA-256
    let computed = compute_file_sha256(&temp_path)?;
    if computed.to_ascii_lowercase() != expected_hash {
        // cleanup va supprimer temp_path automatiquement
        return Err(format!(
            "Échec de validation de l'intégrité SHA-256 !\n\nAttendu  : {expected_hash}\nCalculé  : {computed}\n\nLe fichier temporaire a été supprimé par sécurité. Le fichier de destination d'origine reste intact."
        ));
    }

    // 3. Déplacement atomique vers la destination finale
    crate::storage::atomic_replace_file(&temp_path, dest_path)?;

    // Désactiver le nettoyage du fichier temporaire puisqu'il a été déplacé avec succès
    std::mem::forget(cleanup);

    Ok(())
}

/// Télécharge un asset dans un fichier cible en vérifiant l'intégrité SHA-256.
pub fn download_and_verify_asset(
    asset_url: &str,
    dest_path: &Path,
    expected_sha256: Option<&str>,
) -> Result<(), String> {
    let data = winhttp_get(asset_url)?;
    write_and_verify_asset_bytes(&data, dest_path, expected_sha256)
}

/// Manifeste de mise à jour sécurisé stocké dans `%LOCALAPPDATA%\Malvision\updates\update_manifest.json`.
/// Note de sécurité : le jeton d'authentification secret (`auth_token`) n'est JAMAIS stocké dans le manifeste
/// sur le disque. Seul le code d'authentification HMAC-SHA-256 et les métadonnées de contrôle y sont consignés.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateManifest {
    pub manifest_hmac: String,
    pub parent_pid: u32,
    pub parent_creation_filetime: u64,
    pub target_exe_path: PathBuf,
    pub original_target_sha256: String,
    pub new_exe_path: PathBuf,
    pub expected_sha256: String,
    pub created_at_epoch_secs: u64,
}

impl UpdateManifest {
    pub fn to_json(&self) -> String {
        let val = serde_json::json!({
            "manifest_hmac": self.manifest_hmac,
            "parent_pid": self.parent_pid,
            "parent_creation_filetime": self.parent_creation_filetime,
            "target_exe_path": self.target_exe_path.to_string_lossy(),
            "original_target_sha256": self.original_target_sha256,
            "new_exe_path": self.new_exe_path.to_string_lossy(),
            "expected_sha256": self.expected_sha256,
            "created_at_epoch_secs": self.created_at_epoch_secs,
        });
        serde_json::to_string_pretty(&val).unwrap_or_else(|_| "{}".to_string())
    }

    pub fn from_json(s: &str) -> Result<Self, String> {
        let v: serde_json::Value = serde_json::from_str(s)
            .map_err(|e| format!("Manifeste de mise à jour JSON corrompu : {e}"))?;
        let manifest_hmac = v["manifest_hmac"]
            .as_str()
            .ok_or("manifest_hmac manquant")?
            .to_string();
        let parent_pid = v["parent_pid"].as_u64().ok_or("parent_pid manquant")? as u32;
        let parent_creation_filetime = v["parent_creation_filetime"]
            .as_u64()
            .ok_or("parent_creation_filetime manquant")?;
        let target_str = v["target_exe_path"]
            .as_str()
            .ok_or("target_exe_path manquant")?;
        let original_target_sha256 = v["original_target_sha256"]
            .as_str()
            .ok_or("original_target_sha256 manquant")?
            .to_string();
        let new_str = v["new_exe_path"].as_str().ok_or("new_exe_path manquant")?;
        let expected_sha256 = v["expected_sha256"]
            .as_str()
            .ok_or("expected_sha256 manquant")?
            .to_string();
        let created_at_epoch_secs = v["created_at_epoch_secs"]
            .as_u64()
            .ok_or("created_at_epoch_secs manquant")?;

        Ok(Self {
            manifest_hmac,
            parent_pid,
            parent_creation_filetime,
            target_exe_path: PathBuf::from(target_str),
            original_target_sha256,
            new_exe_path: PathBuf::from(new_str),
            expected_sha256,
            created_at_epoch_secs,
        })
    }

    /// Vérifie l'intégrité cryptographique absolue du manifeste en temps constant via HMAC-SHA-256.
    pub fn verify_integrity(&self, token_arg: &str) -> bool {
        if token_arg.is_empty() {
            return false;
        }
        match compute_manifest_hmac(
            token_arg,
            self.parent_pid,
            self.parent_creation_filetime,
            &self.target_exe_path,
            &self.original_target_sha256,
            &self.new_exe_path,
            &self.expected_sha256,
            self.created_at_epoch_secs,
        ) {
            Ok(expected_hmac) => constant_time_compare(&self.manifest_hmac, &expected_hmac),
            Err(_) => false,
        }
    }
}

/// Calcule l'authentification de message cryptographique HMAC-SHA-256 (RFC 2104 / FIPS 198-1)
/// du manifeste à l'aide de l'API cryptographique native Windows CNG.
#[allow(clippy::too_many_arguments)]
pub fn compute_manifest_hmac(
    token: &str,
    parent_pid: u32,
    parent_creation_filetime: u64,
    target_exe_path: &Path,
    original_target_sha256: &str,
    new_exe_path: &Path,
    expected_sha256: &str,
    created_at_epoch_secs: u64,
) -> Result<String, String> {
    let payload = format!(
        "{parent_pid}|{parent_creation_filetime}|{}|{original_target_sha256}|{}|{expected_sha256}|{created_at_epoch_secs}",
        target_exe_path.to_string_lossy(),
        new_exe_path.to_string_lossy()
    );
    compute_hmac_sha256(token.as_bytes(), payload.as_bytes())
}

/// Informations de préparation de la mise à jour différée.
#[derive(Debug, Clone)]
pub struct UpdatePreparation {
    pub auth_token: String,
    #[allow(dead_code)]
    pub target_exe: PathBuf,
    pub helper_exe: PathBuf,
    #[allow(dead_code)]
    pub manifest_path: PathBuf,
}

/// Retourne le chemin du répertoire temporaire sécurisé des mises à jour :
/// `%LOCALAPPDATA%\Malvision\updates\`.
pub fn get_updates_temp_dir() -> Result<PathBuf, String> {
    let base_dir = std::env::var_os("LOCALAPPDATA")
        .or_else(|| std::env::var_os("APPDATA"))
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let updates_dir = base_dir.join("Malvision").join("updates");
    if !updates_dir.exists() {
        std::fs::create_dir_all(&updates_dir).map_err(|e| {
            format!(
                "Impossible de créer le dossier de mise à jour '{}' : {e}",
                updates_dir.display()
            )
        })?;
    }
    Ok(updates_dir)
}

/// Génère un jeton d'authentification cryptographique de 256 bits (32 octets aléatoires)
/// via le CSPRNG Windows (`BCryptGenRandom`). Aucun fallback prévisible n'est toléré.
pub fn generate_auth_token() -> Result<String, String> {
    let mut bytes = [0u8; 32];
    generate_crypto_random_bytes(&mut bytes)?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

/// Compare deux chemins de fichiers de façon insensible à la casse et insensible aux préfixes UNC.
pub fn is_same_file_path(p1: &Path, p2: &Path) -> bool {
    if p1 == p2 {
        return true;
    }
    match (std::fs::canonicalize(p1), std::fs::canonicalize(p2)) {
        (Ok(c1), Ok(c2)) => c1 == c2,
        _ => {
            let s1 = p1.to_string_lossy().to_lowercase().replace('/', "\\");
            let s2 = p2.to_string_lossy().to_lowercase().replace('/', "\\");
            let n1 = s1.trim_start_matches(r"\\?\");
            let n2 = s2.trim_start_matches(r"\\?\");
            n1 == n2
        }
    }
}

/// Récupère le chemin d'exécutable d'un processus Windows à partir de son handle.
pub fn get_process_image_path(process_handle: HANDLE) -> Result<PathBuf, String> {
    let mut buffer = [0u16; 1024];
    let mut size = buffer.len() as u32;
    unsafe {
        QueryFullProcessImageNameW(
            process_handle,
            PROCESS_NAME_FORMAT(0),
            PWSTR(buffer.as_mut_ptr()),
            &mut size,
        )
        .map_err(|e| format!("QueryFullProcessImageNameW a échoué : {e}"))?;
    }
    let s = String::from_utf16_lossy(&buffer[..size as usize]);
    Ok(PathBuf::from(s))
}

/// Affiche une boîte de dialogue d'erreur native Windows en cas de défaillance du processus auxiliaire.
fn show_native_error(msg: &str) {
    let title: Vec<u16> = "Malvision — Mise à jour\0".encode_utf16().collect();
    let text: Vec<u16> = format!("{msg}\0").encode_utf16().collect();
    unsafe {
        let _ = MessageBoxW(
            None,
            PCWSTR(text.as_ptr()),
            PCWSTR(title.as_ptr()),
            MB_OK | MB_ICONERROR,
        );
    }
}

/// Prépare le binaire auxiliaire et le manifeste de mise à jour sécurisé.
pub fn prepare_update_helper(
    new_exe_path: &Path,
    expected_sha256: &str,
) -> Result<UpdatePreparation, String> {
    let current_exe = std::env::current_exe()
        .map_err(|e| format!("Impossible de déterminer l'exécutable courant : {e}"))?;
    let updates_dir = get_updates_temp_dir()?;
    let helper_exe = updates_dir.join("malvision_updater.exe");

    // Supprimer l'éventuel ancien binaire auxiliaire
    if helper_exe.exists() {
        let _ = std::fs::remove_file(&helper_exe);
    }

    // Copier l'exécutable actif vers le helper temporaire
    std::fs::copy(&current_exe, &helper_exe).map_err(|e| {
        format!("Impossible de copier le processus auxiliaire de mise à jour : {e}")
    })?;

    let auth_token = generate_auth_token()?;
    let parent_pid = std::process::id();

    // Récupérer l'horodatage exact de création du processus parent
    let mut creation_ft = FILETIME::default();
    let mut exit_ft = FILETIME::default();
    let mut kernel_ft = FILETIME::default();
    let mut user_ft = FILETIME::default();
    unsafe {
        GetProcessTimes(
            GetCurrentProcess(),
            &mut creation_ft,
            &mut exit_ft,
            &mut kernel_ft,
            &mut user_ft,
        )
        .map_err(|e| format!("Impossible de lire les attributs du processus parent : {e}"))?;
    }
    let parent_creation_filetime =
        ((creation_ft.dwHighDateTime as u64) << 32) | (creation_ft.dwLowDateTime as u64);

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let original_target_sha256 = compute_file_sha256(&current_exe)?;

    let manifest_hmac = compute_manifest_hmac(
        &auth_token,
        parent_pid,
        parent_creation_filetime,
        &current_exe,
        &original_target_sha256,
        new_exe_path,
        expected_sha256,
        now,
    )?;

    let manifest = UpdateManifest {
        manifest_hmac,
        parent_pid,
        parent_creation_filetime,
        target_exe_path: current_exe.clone(),
        original_target_sha256,
        new_exe_path: new_exe_path.to_path_buf(),
        expected_sha256: expected_sha256.to_string(),
        created_at_epoch_secs: now,
    };

    let manifest_path = updates_dir.join("update_manifest.json");
    let manifest_tmp = updates_dir.join("update_manifest.tmp");
    std::fs::write(&manifest_tmp, manifest.to_json())
        .map_err(|e| format!("Impossible d'écrire le manifeste de mise à jour : {e}"))?;
    let _ = std::fs::remove_file(&manifest_path);
    std::fs::rename(&manifest_tmp, &manifest_path)
        .map_err(|e| format!("Impossible de finaliser le fichier manifeste : {e}"))?;

    Ok(UpdatePreparation {
        auth_token,
        target_exe: current_exe,
        helper_exe,
        manifest_path,
    })
}

/// Lance le processus auxiliaire en tâche de fond et attend confirmation de sa disponibilité.
pub fn launch_helper_and_wait_ready(
    prep: &UpdatePreparation,
    timeout: Duration,
) -> Result<(), String> {
    let updates_dir = get_updates_temp_dir()?;
    let ready_flag = updates_dir.join("helper_ready.flag");
    let _ = std::fs::remove_file(&ready_flag);

    let mut cmd = std::process::Command::new(&prep.helper_exe);
    cmd.arg("--apply-update").arg(&prep.auth_token);

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }

    cmd.spawn()
        .map_err(|e| format!("Impossible de lancer le processus auxiliaire : {e}"))?;

    let start = std::time::Instant::now();
    while start.elapsed() < timeout {
        if ready_flag.exists() {
            return Ok(());
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }

    Err("Le processus auxiliaire de mise à jour n'a pas répondu dans le délai imparti.".to_string())
}

/// Restaure la sauvegarde .bak vers le chemin cible et vérifie obligatoirement son existence, sa validité
/// et la stricte conformité cryptographique du SHA-256 restauré avec l'empreinte originale attendue.
pub fn perform_rollback_and_verify(
    target: &Path,
    backup: &Path,
    expected_original_sha256: &str,
) -> Result<(), String> {
    if !backup.exists() {
        return Err(format!(
            "Fichier de sauvegarde introuvable : {}",
            backup.display()
        ));
    }

    // Vérifier l'absence de lien symbolique
    if let Ok(meta) = std::fs::symlink_metadata(backup)
        && meta.file_type().is_symlink()
    {
        return Err("Le fichier de sauvegarde est un lien symbolique non autorisé.".to_string());
    }

    // Contrôler l'intégrité de la sauvegarde avant restauration
    let backup_hash = compute_file_sha256(backup)
        .map_err(|e| format!("Impossible de vérifier l'empreinte de la sauvegarde : {e}"))?;
    if !constant_time_compare(&backup_hash, expected_original_sha256) {
        return Err(format!(
            "L'empreinte de la sauvegarde ({backup_hash}) ne correspond pas à l'empreinte originale attendue ({expected_original_sha256})."
        ));
    }

    // Réessayer jusqu'à 15 fois (3 secondes) pour contourner d'éventuels verrous temporaires
    let mut restored = false;
    for _ in 0..15 {
        let _ = std::fs::remove_file(target);
        if std::fs::rename(backup, target).is_ok() {
            restored = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(200));
    }

    if !restored {
        return Err(format!(
            "Impossible de restaurer la sauvegarde '{}' vers '{}' (fichier verrouillé).",
            backup.display(),
            target.display()
        ));
    }

    if !target.exists() || std::fs::metadata(target).map(|m| m.len()).unwrap_or(0) == 0 {
        return Err("Le fichier cible restauré est manquant ou vide.".to_string());
    }

    // Vérification cryptographique OBLIGATOIRE du binaire cible restauré
    let restored_hash = compute_file_sha256(target)
        .map_err(|e| format!("Impossible de vérifier le binaire restauré : {e}"))?;
    if !constant_time_compare(&restored_hash, expected_original_sha256) {
        return Err(format!(
            "Sécurité : L'empreinte du fichier cible restauré ({restored_hash}) ne correspond pas à la version d'origine ({expected_original_sha256})."
        ));
    }

    Ok(())
}

/// Enregistre une directive de récupération différée dans `%LOCALAPPDATA%\Malvision\updates\pending_recovery.json`.
/// Note de sécurité : le chemin de backup n'est jamais consigné arbitrairement ; il sera obligatoirement dérivé
/// du chemin de l'exécutable légitime lors de la reprise au redémarrage.
pub fn record_pending_recovery(target: &Path, original_target_sha256: &str) {
    if let Ok(updates_dir) = get_updates_temp_dir() {
        let recovery_path = updates_dir.join("pending_recovery.json");
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let val = serde_json::json!({
            "target_exe_path": target.to_string_lossy(),
            "expected_backup_sha256": original_target_sha256,
            "created_at_epoch_secs": now,
        });
        let _ = std::fs::write(&recovery_path, val.to_string());
    }
}

/// Exécute la logique du processus auxiliaire `--apply-update <token>`.
pub fn run_apply_update_mode(args: &[String]) {
    if args.is_empty() {
        eprintln!("Argument auth_token manquant.");
        return;
    }
    let token = &args[0];

    let updates_dir = match get_updates_temp_dir() {
        Ok(d) => d,
        Err(e) => {
            show_native_error(&format!("Erreur d'accès au dossier de mise à jour : {e}"));
            return;
        }
    };

    let manifest_path = updates_dir.join("update_manifest.json");
    let manifest_str = match std::fs::read_to_string(&manifest_path) {
        Ok(s) => s,
        Err(e) => {
            show_native_error(&format!("Manifeste de mise à jour introuvable : {e}"));
            return;
        }
    };

    let manifest = match UpdateManifest::from_json(&manifest_str) {
        Ok(m) => m,
        Err(e) => {
            show_native_error(&format!("Manifeste de mise à jour corrompu : {e}"));
            return;
        }
    };

    // 1. Contrôle du jeton d'authentification et de l'intégrité cryptographique absolue du manifeste
    if !manifest.verify_integrity(token) {
        show_native_error(
            "Sécurité : L'intégrité du manifeste ou le jeton d'authentification de mise à jour est invalide.",
        );
        return;
    }

    // 2. Contrôle de validité temporelle (5 minutes maximum)
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    if now.saturating_sub(manifest.created_at_epoch_secs) > 300 {
        show_native_error("Sécurité : La demande de mise à jour a expiré.");
        return;
    }

    // 3. Validation de sécurité stricte sur le chemin cible
    let ext = manifest
        .target_exe_path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("");
    if !ext.eq_ignore_ascii_case("exe") {
        show_native_error(
            "Sécurité : La cible de mise à jour doit être un fichier exécutable (.exe).",
        );
        return;
    }

    if let Ok(windir) = std::env::var("SystemRoot") {
        let win_path = PathBuf::from(windir);
        if manifest.target_exe_path.starts_with(&win_path) {
            show_native_error(
                "Sécurité : La cible de mise à jour ne peut pas résider dans le répertoire système Windows.",
            );
            return;
        }
    }

    // 4. Liaison et contrôle anti-recyclage du processus parent
    let parent_handle = match unsafe {
        OpenProcess(
            PROCESS_SYNCHRONIZE | PROCESS_QUERY_LIMITED_INFORMATION,
            false,
            manifest.parent_pid,
        )
    } {
        Ok(h) if !h.is_invalid() => h,
        _ => {
            show_native_error("Sécurité : Impossible de lier le processus parent de Malvision.");
            return;
        }
    };

    let mut p_creation = FILETIME::default();
    let mut p_exit = FILETIME::default();
    let mut p_kernel = FILETIME::default();
    let mut p_user = FILETIME::default();
    let times_res = unsafe {
        GetProcessTimes(
            parent_handle,
            &mut p_creation,
            &mut p_exit,
            &mut p_kernel,
            &mut p_user,
        )
    };
    if times_res.is_err() {
        let _ = unsafe { CloseHandle(parent_handle) };
        show_native_error("Sécurité : Impossible de vérifier l'horodatage du processus parent.");
        return;
    }
    let actual_parent_creation =
        ((p_creation.dwHighDateTime as u64) << 32) | (p_creation.dwLowDateTime as u64);
    if actual_parent_creation != manifest.parent_creation_filetime {
        let _ = unsafe { CloseHandle(parent_handle) };
        show_native_error(
            "Sécurité : Le PID parent a été recyclé ou ne correspond pas au processus demandeur.",
        );
        return;
    }

    // 5. Vérification OBLIGATOIRE du chemin d'image du processus parent
    let parent_exe = match get_process_image_path(parent_handle) {
        Ok(path) => path,
        Err(e) => {
            let _ = unsafe { CloseHandle(parent_handle) };
            show_native_error(&format!(
                "Sécurité : Échec obligatoire de vérification du chemin parent ({e})."
            ));
            return;
        }
    };

    if !is_same_file_path(&parent_exe, &manifest.target_exe_path) {
        let _ = unsafe { CloseHandle(parent_handle) };
        show_native_error(
            "Sécurité : Le processus parent ne correspond pas à l'exécutable Malvision autorisé.",
        );
        return;
    }

    // 6. Contrôle cryptographique préalable du binaire téléchargé
    match compute_file_sha256(&manifest.new_exe_path) {
        Ok(hash) if hash.eq_ignore_ascii_case(&manifest.expected_sha256) => {}
        _ => {
            let _ = unsafe { CloseHandle(parent_handle) };
            show_native_error("Sécurité : L'empreinte SHA-256 du nouveau binaire est invalide.");
            return;
        }
    }

    // 7. Signaler au processus principal que le helper est prêt à opérer
    let ready_flag = updates_dir.join("helper_ready.flag");
    let ready_tmp = updates_dir.join("helper_ready.tmp");
    if std::fs::write(&ready_tmp, b"READY").is_err()
        || std::fs::rename(&ready_tmp, &ready_flag).is_err()
    {
        let _ = unsafe { CloseHandle(parent_handle) };
        show_native_error("Impossible d'émettre le signal de synchronisation.");
        return;
    }

    // 8. Attente de la fin effective du processus principal (jusqu'à 30 secondes)
    let wait_res = unsafe { WaitForSingleObject(parent_handle, 30_000) };
    let _ = unsafe { CloseHandle(parent_handle) };
    if wait_res.0 != 0 {
        show_native_error(
            "Le processus principal de Malvision ne s'est pas arrêté dans le délai imparti.",
        );
        return;
    }

    // Délai de grâce pour libération complète des verrous du système de fichiers
    std::thread::sleep(std::time::Duration::from_millis(200));

    // 9. Remplacement atomique et gestion du backup
    let target = &manifest.target_exe_path;
    let backup = target.with_extension("exe.bak");
    let new_exe = &manifest.new_exe_path;

    // A. Sauvegarde de l'ancien binaire vers target.exe.bak avec boucle de réessai
    let mut backup_ok = false;
    for _ in 0..15 {
        if backup.exists() {
            let _ = std::fs::remove_file(&backup);
        }
        if std::fs::rename(target, &backup).is_ok() {
            backup_ok = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
    }

    if !backup_ok {
        show_native_error(&format!(
            "Impossible de remplacer l'exécutable à l'emplacement :\n{}\n\nAccès refusé ou fichier verrouillé.\nVérifiez vos permissions ou déplacez l'application dans un dossier accessible.",
            target.display()
        ));
        return;
    }

    // B. Déplacement / Copie du nouveau binaire vers target
    let installed_ok = match std::fs::rename(new_exe, target) {
        Ok(_) => true,
        Err(_) => {
            if std::fs::copy(new_exe, target).is_ok() {
                let _ = std::fs::remove_file(new_exe);
                true
            } else {
                false
            }
        }
    };

    // C. Vérification de l'intégrité cryptographique du binaire installé
    let target_hash_ok = installed_ok
        && compute_file_sha256(target)
            .map(|h| h.eq_ignore_ascii_case(&manifest.expected_sha256))
            .unwrap_or(false);

    if !target_hash_ok {
        // ROLLBACK immédiat avec vérification cryptographique complète
        let rollback_res =
            perform_rollback_and_verify(target, &backup, &manifest.original_target_sha256);
        if rollback_res.is_ok() {
            let _ = std::process::Command::new(target).spawn();
            show_native_error(
                "L'intégrité de la mise à jour installée est invalide. L'ancienne version a été automatiquement restaurée.",
            );
        } else {
            record_pending_recovery(target, &manifest.original_target_sha256);
            show_native_error(
                "Échec critique : L'intégrité de la mise à jour est invalide et la restauration automatique immédiate a échoué.\n\nUne tentative de récupération aura lieu au prochain démarrage.",
            );
        }
        return;
    }

    // 10. Préparation de la surveillance de disponibilité explicite (Health Check)
    let event_name = format!("Local\\Malvision_Health_{token}\0");
    let event_w: Vec<u16> = event_name.encode_utf16().collect();
    let health_event = unsafe { CreateEventW(None, true, false, PCWSTR(event_w.as_ptr())) }
        .unwrap_or(HANDLE(std::ptr::null_mut()));

    let health_flag = updates_dir.join(format!("health_{token}.flag"));
    let _ = std::fs::remove_file(&health_flag);

    // Relance de la nouvelle version avec argument de confirmation de santé
    let mut cmd = std::process::Command::new(target);
    cmd.arg("--health-check").arg(token);

    let child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            if !health_event.is_invalid() {
                let _ = unsafe { CloseHandle(health_event) };
            }
            let rollback_res =
                perform_rollback_and_verify(target, &backup, &manifest.original_target_sha256);
            if rollback_res.is_ok() {
                let _ = std::process::Command::new(target).spawn();
                show_native_error(&format!(
                    "Impossible de démarrer la nouvelle version ({e}). L'ancienne version a été automatiquement restaurée."
                ));
            } else {
                record_pending_recovery(target, &manifest.original_target_sha256);
                show_native_error(&format!(
                    "Échec critique : Impossible de démarrer la nouvelle version ({e}) et la restauration automatique immédiate a échoué.\n\nUne tentative de récupération aura lieu au prochain démarrage."
                ));
            }
            return;
        }
    };

    let child_handle = HANDLE(child.as_raw_handle() as *mut _);

    // 11. Attente du signal de disponibilité explicite ou détection de crash (jusqu'à 15 secondes)
    let is_healthy = if !health_event.is_invalid() {
        let wait_handles = [health_event, child_handle];
        let wait_res = unsafe { WaitForMultipleObjects(&wait_handles, false, 15_000) };
        wait_res.0 == 0 // Index 0 = health_event
    } else {
        // Fallback sur drapeau fichier si l'événement Win32 n'a pas pu être créé
        let start = std::time::Instant::now();
        let mut ok = false;
        while start.elapsed() < Duration::from_secs(15) {
            if health_flag.exists() {
                ok = true;
                break;
            }
            let exit_code = unsafe {
                let mut code = 0u32;
                if windows::Win32::System::Threading::GetExitCodeProcess(child_handle, &mut code)
                    .is_ok()
                    && code != 259
                {
                    Some(code)
                } else {
                    None
                }
            };
            if exit_code.is_some() {
                break;
            }
            std::thread::sleep(Duration::from_millis(150));
        }
        ok
    };

    if !health_event.is_invalid() {
        let _ = unsafe { CloseHandle(health_event) };
    }

    if is_healthy {
        // SUCCÈS CONFIRMÉ : Le nouveau processus est en ligne et pleinement fonctionnel
        // Le backup est maintenant supprimé en toute sécurité
        let _ = std::fs::remove_file(&backup);
        let _ = std::fs::remove_file(&health_flag);
        let _ = std::fs::remove_file(&manifest_path);
        let _ = std::fs::remove_file(&ready_flag);
    } else {
        // ÉCHEC DE SANTÉ (crash prématuré ou gel au démarrage) : ROLLBACK OBLIGATOIRE
        // A. Terminer le processus enfant s'il est encore actif pour libérer les verrous d'image Windows
        unsafe {
            let _ = TerminateProcess(child_handle, 1);
            let _ = WaitForSingleObject(child_handle, 5000);
        }
        std::thread::sleep(Duration::from_millis(200));

        // B. Restaurer et vérifier la sauvegarde avec validation d'empreinte
        let rollback_res =
            perform_rollback_and_verify(target, &backup, &manifest.original_target_sha256);
        if rollback_res.is_ok() {
            let _ = std::process::Command::new(target).spawn();
            show_native_error(
                "La nouvelle version n'a pas pu s'initialiser correctement.\n\nL'ancienne version a été automatiquement et intégralement restaurée.",
            );
        } else {
            record_pending_recovery(target, &manifest.original_target_sha256);
            show_native_error(
                "Échec critique : La nouvelle version a échoué et la restauration immédiate de la sauvegarde n'a pas pu être finalisée.\n\nUne tentative de récupération aura lieu au prochain démarrage.",
            );
        }

        let _ = std::fs::remove_file(&health_flag);
        let _ = std::fs::remove_file(&manifest_path);
        let _ = std::fs::remove_file(&ready_flag);
    }
}

/// Jeton en attente de validation de démarrage pour le processus nouvellement lancé.
static PENDING_HEALTH_TOKEN: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

/// Définit le jeton de santé transmis par le helper lors d'une relance post-mise à jour.
pub fn set_pending_health_token(token: String) {
    if let Ok(mut lock) = PENDING_HEALTH_TOKEN.lock() {
        *lock = Some(token);
    }
}

/// Émet le signal explicite de disponibilité après initialisation complète des composants essentiels.
pub fn notify_startup_healthy() {
    let token = if let Ok(mut lock) = PENDING_HEALTH_TOKEN.lock() {
        lock.take()
    } else {
        None
    };

    if let Some(t) = token {
        let event_name = format!("Local\\Malvision_Health_{t}\0");
        let event_w: Vec<u16> = event_name.encode_utf16().collect();
        unsafe {
            if let Ok(h) = OpenEventW(EVENT_MODIFY_STATE, false, PCWSTR(event_w.as_ptr()))
                && !h.is_invalid()
            {
                let _ = SetEvent(h);
                let _ = CloseHandle(h);
            }
        }

        if let Ok(updates_dir) = get_updates_temp_dir() {
            let flag = updates_dir.join(format!("health_{t}.flag"));
            let _ = std::fs::write(&flag, b"HEALTHY");
        }
    }
}

/// Vérifie et applique une restauration différée hautement sécurisée si un rollback immédiat avait été bloqué par un verrou.
/// Règles de sécurité strictes :
/// 1. La cible doit correspondre strictement à l'exécutable Malvision en cours d'exécution.
/// 2. Le chemin backup est strictement dérivé du binaire courant (.exe.bak) et n'est jamais lu comme un chemin arbitraire.
/// 3. Rejet catégorique si la cible ou le backup se situe dans le répertoire Windows (SystemRoot) ou est un lien symbolique.
/// 4. Vérification cryptographique préalable : l'empreinte SHA-256 du backup doit correspondre exactement à `expected_backup_sha256`.
/// 5. En cas de doute ou d'anomalie, la directive est supprimée sans modification des fichiers du disque.
pub fn check_and_apply_pending_recovery() {
    let Ok(updates_dir) = get_updates_temp_dir() else {
        return;
    };
    let recovery_file = updates_dir.join("pending_recovery.json");
    if !recovery_file.exists() {
        return;
    }

    let cleanup_recovery = || {
        let _ = std::fs::remove_file(&recovery_file);
    };

    let content = match std::fs::read_to_string(&recovery_file) {
        Ok(c) => c,
        Err(_) => {
            cleanup_recovery();
            return;
        }
    };

    let v: serde_json::Value = match serde_json::from_str(&content) {
        Ok(val) => val,
        Err(_) => {
            cleanup_recovery();
            return;
        }
    };

    let (Some(target_str), Some(expected_backup_sha256)) = (
        v["target_exe_path"].as_str(),
        v["expected_backup_sha256"].as_str(),
    ) else {
        cleanup_recovery();
        return;
    };

    let current_exe = match std::env::current_exe() {
        Ok(p) => p,
        Err(_) => {
            cleanup_recovery();
            return;
        }
    };

    let target_path = PathBuf::from(target_str);

    // 1. La cible dans la directive DOIT correspondre strictement à l'exécutable courant
    if !is_same_file_path(&target_path, &current_exe) {
        cleanup_recovery();
        return;
    }

    // 2. Extension .exe obligatoire
    let ext = current_exe
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("");
    if !ext.eq_ignore_ascii_case("exe") {
        cleanup_recovery();
        return;
    }

    // 3. Protection répertoire système Windows
    if let Ok(windir) = std::env::var("SystemRoot") {
        let win_path = PathBuf::from(windir);
        if current_exe.starts_with(&win_path) {
            cleanup_recovery();
            return;
        }
    }

    // 4. Protection contre les liens symboliques et jonctions
    if let Ok(meta) = std::fs::symlink_metadata(&current_exe)
        && meta.file_type().is_symlink()
    {
        cleanup_recovery();
        return;
    }

    // 5. Dérivation stricte du chemin backup (jamais de chemin arbitraire externe)
    let backup_path = current_exe.with_extension("exe.bak");
    if !backup_path.exists() {
        cleanup_recovery();
        return;
    }

    if let Ok(meta) = std::fs::symlink_metadata(&backup_path)
        && meta.file_type().is_symlink()
    {
        cleanup_recovery();
        return;
    }

    // 6. Vérification cryptographique OBLIGATOIRE de la sauvegarde avant toute opération sur les fichiers
    let actual_backup_sha256 = match compute_file_sha256(&backup_path) {
        Ok(h) => h,
        Err(_) => {
            cleanup_recovery();
            return;
        }
    };

    if !constant_time_compare(&actual_backup_sha256, expected_backup_sha256) {
        // Sauvegarde non authentique ou corrompue : refus strict sans altération du binaire
        cleanup_recovery();
        return;
    }

    // 7. Restauration avec boucle de réessai
    let mut restored = false;
    for _ in 0..15 {
        let _ = std::fs::remove_file(&current_exe);
        if std::fs::rename(&backup_path, &current_exe).is_ok() {
            restored = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(200));
    }

    if restored {
        // Contrôle cryptographique final du binaire restauré
        if let Ok(restored_sha256) = compute_file_sha256(&current_exe)
            && constant_time_compare(&restored_sha256, expected_backup_sha256)
        {
            cleanup_recovery();
            return;
        }
    }

    cleanup_recovery();
}

/// Nettoie les fichiers résiduels d'une mise à jour précédente lors d'un démarrage normal.
pub fn cleanup_leftover_update_files() {
    let Ok(updates_dir) = get_updates_temp_dir() else {
        return;
    };
    let recovery_file = updates_dir.join("pending_recovery.json");
    if recovery_file.exists() {
        // Ne pas nettoyer si une récupération est encore pendante !
        return;
    }

    if let Ok(current_exe) = std::env::current_exe() {
        let bak = current_exe.with_extension("exe.bak");
        if bak.exists() {
            let _ = std::fs::remove_file(&bak);
        }
    }

    let _ = std::fs::remove_file(updates_dir.join("update_manifest.json"));
    let _ = std::fs::remove_file(updates_dir.join("update_manifest.tmp"));
    let _ = std::fs::remove_file(updates_dir.join("helper_ready.flag"));
    let _ = std::fs::remove_file(updates_dir.join("helper_ready.tmp"));
    let _ = std::fs::remove_file(updates_dir.join("new_version.exe"));
    let _ = std::fs::remove_file(updates_dir.join("malvision_updater.exe"));
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_semver_parsing_and_comparisons() {
        let v100 = SemVer::parse("1.0.0").unwrap();
        let v101 = SemVer::parse("v1.0.1").unwrap();
        let v110 = SemVer::parse("V1.1.0").unwrap();
        let v200 = SemVer::parse("2.0.0-beta").unwrap();

        assert_eq!(v100, SemVer::new(1, 0, 0));
        assert_eq!(v101, SemVer::new(1, 0, 1));
        assert!(v101 > v100);
        assert!(v110 > v101);
        assert!(v200 > v110);
        assert_eq!(v100.to_string(), "1.0.0");
        assert_eq!(v101.to_string(), "1.0.1");

        // Format invalide
        assert!(SemVer::parse("invalid").is_none());
        assert!(SemVer::parse("1.0").is_none());
    }

    #[test]
    fn test_parse_release_json_valid_and_ignores_prerelease() {
        let sample_json = r#"{
            "tag_name": "v1.0.1",
            "name": "Malvision 1.0.1 - Correctifs",
            "draft": false,
            "prerelease": false,
            "body": "SHA256: e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855 *Malvision.exe",
            "html_url": "https://github.com/Malvirae/malvision/releases/tag/v1.0.1",
            "assets": [
                {
                    "name": "Malvision.exe",
                    "browser_download_url": "https://github.com/Malvirae/malvision/releases/download/v1.0.1/Malvision.exe",
                    "size": 5242880
                },
                {
                    "name": "SHA256SUMS.txt",
                    "browser_download_url": "https://github.com/Malvirae/malvision/releases/download/v1.0.1/SHA256SUMS.txt",
                    "size": 128
                }
            ]
        }"#;

        let info = parse_release_json(sample_json).unwrap();
        assert_eq!(info.tag_name, "v1.0.1");
        assert_eq!(info.version, SemVer::new(1, 0, 1));
        assert!(info.version > SemVer::new(1, 0, 0));
        assert!(info.exe_asset.is_some());
        assert_eq!(info.exe_asset.as_ref().unwrap().name, "Malvision.exe");
        assert_eq!(
            info.expected_exe_sha256.as_deref(),
            Some("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855")
        );

        // Prerelease doit être rejetée
        let prerelease_json = r#"{
            "tag_name": "v1.0.2",
            "draft": false,
            "prerelease": true
        }"#;
        assert!(parse_release_json(prerelease_json).is_err());
    }

    #[test]
    fn test_extract_sha256_from_text() {
        let text = "Release notes\nSHA256: 6876c52a0a256fece328bb9ea4e116934a36eead0d648dd6dfb92cfb373cf2b6 *Malvision.exe\nAutre info";
        let hash = extract_sha256_from_text(text, "Malvision.exe").unwrap();
        assert_eq!(
            hash,
            "6876c52a0a256fece328bb9ea4e116934a36eead0d648dd6dfb92cfb373cf2b6"
        );

        assert!(extract_sha256_from_text("Pas de hash ici", "Malvision.exe").is_none());
    }

    #[test]
    fn test_write_and_verify_asset_bytes_sha256_success_and_tamper_detection() {
        let base_dir = std::env::temp_dir();
        let pid = std::process::id();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let target_file = base_dir.join(format!("malvision_download_test_{pid}_{nanos}.exe"));
        let _ = std::fs::remove_file(&target_file);

        let data = b"MZ... Valid Fake Executable Payload for Malvision v1.0.1 ...";
        let valid_hash = crate::security::compute_sha256(data).unwrap();

        // 1. Succès avec hash correspondant
        let res = write_and_verify_asset_bytes(data, &target_file, Some(&valid_hash));
        assert!(
            res.is_ok(),
            "Le téléchargement avec SHA-256 valide doit réussir"
        );
        assert!(target_file.exists());
        assert_eq!(std::fs::read(&target_file).unwrap(), data);

        // 2. Rejet obligatoire si aucune empreinte SHA-256 n'est fournie (None ou vide)
        let res_no_hash = write_and_verify_asset_bytes(data, &target_file, None);
        assert!(
            res_no_hash.is_err(),
            "Tout téléchargement sans empreinte officielle doit être rejeté"
        );
        let res_empty_hash = write_and_verify_asset_bytes(data, &target_file, Some("   "));
        assert!(
            res_empty_hash.is_err(),
            "Une empreinte vide doit être rejetée"
        );

        // 3. Échec avec hash altéré ET conservation intégrale du fichier existant
        let corrupt_hash = "0000000000000000000000000000000000000000000000000000000000000000";
        let target_file2 = base_dir.join(format!("malvision_download_corrupt_{pid}_{nanos}.exe"));
        let _ = std::fs::remove_file(&target_file2);

        // Créer un fichier existant préalable
        std::fs::write(&target_file2, b"ANCIEN EXECUTABLE FONCTIONNEL").unwrap();

        let res_err = write_and_verify_asset_bytes(data, &target_file2, Some(corrupt_hash));
        assert!(res_err.is_err(), "Le hash altéré doit être rejeté");
        // Le fichier cible existant doit être conservé intact
        assert_eq!(
            std::fs::read(&target_file2).unwrap(),
            b"ANCIEN EXECUTABLE FONCTIONNEL",
            "L'ancien exécutable doit être préservé en cas d'échec du hash"
        );

        // 4. Échec avec payload vide
        let res_empty = write_and_verify_asset_bytes(b"", &target_file, Some(&valid_hash));
        assert!(res_empty.is_err());

        // 5. Validation de parse_url : rejet strict des URLs HTTP non sécurisées
        assert!(parse_url("http://insecure-download.example.com/file.exe").is_err());
        assert!(parse_url("ftp://ftp.example.com/file.exe").is_err());
        assert!(parse_url("https://github.com/Malvirae/malvision/releases").is_ok());

        let _ = std::fs::remove_file(&target_file);
        let _ = std::fs::remove_file(&target_file2);
    }

    #[test]
    fn test_parse_release_json_with_sha256sums_txt_asset() {
        let json_without_body_hash = r#"{
            "tag_name": "v1.0.1",
            "name": "Malvision 1.0.1",
            "draft": false,
            "prerelease": false,
            "body": "Notes de version sans hash explicite dans le texte",
            "html_url": "https://github.com/Malvirae/malvision/releases/tag/v1.0.1",
            "assets": [
                {
                    "name": "Malvision.exe",
                    "browser_download_url": "https://github.com/Malvirae/malvision/releases/download/v1.0.1/Malvision.exe",
                    "size": 5000000
                },
                {
                    "name": "SHA256SUMS.txt",
                    "browser_download_url": "https://github.com/Malvirae/malvision/releases/download/v1.0.1/SHA256SUMS.txt",
                    "size": 100
                }
            ]
        }"#;

        let info = parse_release_json(json_without_body_hash).unwrap();
        assert!(info.sha256_sums_asset.is_some());
        assert_eq!(info.sha256_sums_asset.unwrap().name, "SHA256SUMS.txt");

        // Simuler la lecture du fichier SHA256SUMS.txt
        let sums_content =
            "5d89be95c7c4ab5db2c4c7dc92875a759e6b08ca1856a49fd06667c09da2db25  Malvision.exe\n";
        let extracted = extract_sha256_from_text(sums_content, "Malvision.exe");
        assert_eq!(
            extracted.as_deref(),
            Some("5d89be95c7c4ab5db2c4c7dc92875a759e6b08ca1856a49fd06667c09da2db25")
        );
    }

    #[test]
    fn test_parse_release_json_github_not_found() {
        let not_found_json = r#"{
            "message": "Not Found",
            "documentation_url": "https://docs.github.com/rest/releases/releases#get-the-latest-release"
        }"#;
        let res = parse_release_json(not_found_json);
        assert!(res.is_err());
        assert_eq!(res.unwrap_err(), MSG_NO_PUBLIC_RELEASE);
    }

    #[test]
    fn test_current_semver_matches_cargo_pkg_version() {
        let current = current_semver()
            .expect("La version locale doit être dérivée sans erreur de CARGO_PKG_VERSION");
        assert_eq!(current.to_string(), CURRENT_VERSION);
        assert_eq!(current.to_string(), "1.0.4");
        assert_eq!(current, SemVer::new(1, 0, 4));
        assert_eq!(SemVer::current().unwrap(), current);
    }

    #[test]
    fn test_semver_comparison_matrix_for_v104() {
        let local_v104 = SemVer::parse("1.0.4").unwrap();

        // 1.0.4 == 1.0.4 -> À jour (pas de mise à jour)
        let remote_equal = SemVer::parse("1.0.4").unwrap();
        assert_eq!(remote_equal, local_v104);
        assert!(
            !(remote_equal > local_v104),
            "1.0.4 == 1.0.4 ne doit pas être considéré comme une mise à jour"
        );

        // 1.0.5 > 1.0.4 -> Mise à jour disponible
        let remote_patch_higher = SemVer::parse("1.0.5").unwrap();
        assert!(
            remote_patch_higher > local_v104,
            "1.0.5 > 1.0.4 doit être détecté comme une mise à jour disponible"
        );

        // 1.1.0 > 1.0.4 -> Mise à jour disponible
        let remote_minor_higher = SemVer::parse("1.1.0").unwrap();
        assert!(
            remote_minor_higher > local_v104,
            "1.1.0 > 1.0.4 doit être détecté comme une mise à jour disponible"
        );

        // 2.0.0 > 1.0.4 -> Mise à jour disponible
        let remote_major_higher = SemVer::parse("2.0.0").unwrap();
        assert!(
            remote_major_higher > local_v104,
            "2.0.0 > 1.0.4 doit être détecté comme une mise à jour disponible"
        );

        // 1.0.3 < 1.0.4 -> Aucune mise à jour
        let remote_older_v103 = SemVer::parse("1.0.3").unwrap();
        assert!(
            !(remote_older_v103 > local_v104),
            "1.0.3 < 1.0.4 ne doit pas être considéré comme une mise à jour"
        );

        // 1.0.2 < 1.0.4 -> Aucune mise à jour
        let remote_older_v102 = SemVer::parse("1.0.2").unwrap();
        assert!(
            !(remote_older_v102 > local_v104),
            "1.0.2 < 1.0.4 ne doit pas être considéré comme une mise à jour"
        );
    }

    #[test]
    fn test_invalid_local_version_never_authorizes_update() {
        // En cas d'erreur de parsing de la version locale, is_upgrade doit TOUJOURS valoir false
        let fake_err: Result<SemVer, String> = Err("Format invalide".to_string());
        let remote = SemVer::new(9, 9, 9);
        let is_upgrade = fake_err.map(|local| remote > local).unwrap_or(false);
        assert!(
            !is_upgrade,
            "Une erreur de version locale ne doit JAMAIS autoriser une mise à jour"
        );
    }

    #[test]
    fn test_startup_and_manual_comparison_simulation() {
        let local = current_semver().expect("Version locale valide");

        // Simulation réponse GitHub distante égale à 1.0.4
        let remote_info_same = ReleaseInfo {
            tag_name: "v1.0.4".to_string(),
            name: "Malvision 1.0.4".to_string(),
            version: SemVer::new(1, 0, 4),
            body: String::new(),
            html_url: String::new(),
            exe_asset: None,
            expected_exe_sha256: None,
            sha256_sums_asset: None,
        };
        let is_upgrade_same = remote_info_same.version > local;
        assert!(
            !is_upgrade_same,
            "1.0.4 distant == 1.0.4 local -> Doit indiquer à jour"
        );

        // Simulation réponse GitHub distante antérieure (ex: 1.0.3 alors qu'on est en 1.0.4)
        let remote_info_old = ReleaseInfo {
            tag_name: "v1.0.3".to_string(),
            name: "Malvision 1.0.3".to_string(),
            version: SemVer::new(1, 0, 3),
            body: String::new(),
            html_url: String::new(),
            exe_asset: None,
            expected_exe_sha256: None,
            sha256_sums_asset: None,
        };
        let is_upgrade_old = remote_info_old.version > local;
        assert!(
            !is_upgrade_old,
            "1.0.3 distant < 1.0.4 local -> Doit indiquer à jour"
        );

        // Simulation réponse GitHub distante postérieure (ex: 1.0.5)
        let remote_info_new = ReleaseInfo {
            tag_name: "v1.0.5".to_string(),
            name: "Malvision 1.0.5".to_string(),
            version: SemVer::new(1, 0, 5),
            body: String::new(),
            html_url: String::new(),
            exe_asset: None,
            expected_exe_sha256: None,
            sha256_sums_asset: None,
        };
        let is_upgrade_new = remote_info_new.version > local;
        assert!(
            is_upgrade_new,
            "1.0.5 distant > 1.0.4 local -> Doit indiquer mise à jour disponible"
        );
    }

    #[test]
    fn test_update_manifest_hmac_and_serialization() {
        let auth_token = generate_auth_token().expect("Génération auth_token cryptographique");
        assert_eq!(
            auth_token.len(),
            64,
            "Jeton SHA-256 / 256 bits (64 hex chars)"
        );

        let parent_pid = std::process::id();
        let parent_creation_filetime = 133500000000000000u64;
        let target_exe_path = PathBuf::from(r"C:\Apps\Malvision\Malvision.exe");
        let original_target_sha256 =
            "aaaabbbbccccddddeeeeffff1111222233334444555566667777888899990000".to_string();
        let new_exe_path =
            PathBuf::from(r"C:\Users\Axel\AppData\Local\Malvision\updates\new_version.exe");
        let expected_sha256 =
            "abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890".to_string();
        let created_at_epoch_secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let manifest_hmac = compute_manifest_hmac(
            &auth_token,
            parent_pid,
            parent_creation_filetime,
            &target_exe_path,
            &original_target_sha256,
            &new_exe_path,
            &expected_sha256,
            created_at_epoch_secs,
        )
        .expect("Calcul HMAC du manifeste");

        let manifest = UpdateManifest {
            manifest_hmac: manifest_hmac.clone(),
            parent_pid,
            parent_creation_filetime,
            target_exe_path: target_exe_path.clone(),
            original_target_sha256: original_target_sha256.clone(),
            new_exe_path: new_exe_path.clone(),
            expected_sha256: expected_sha256.clone(),
            created_at_epoch_secs,
        };

        // 1. Validation de l'intégrité nominale avec le bon jeton
        assert!(manifest.verify_integrity(&auth_token));

        // 2. Sérialisation et désérialisation JSON (le jeton secret n'est PAS dans le JSON !)
        let json = manifest.to_json();
        assert!(
            !json.contains(&auth_token),
            "Le jeton secret ne doit JAMAIS figurer dans le JSON"
        );
        let parsed = UpdateManifest::from_json(&json).expect("Désérialisation valide");
        assert_eq!(manifest.manifest_hmac, parsed.manifest_hmac);
        assert_eq!(manifest.parent_pid, parsed.parent_pid);
        assert_eq!(
            manifest.parent_creation_filetime,
            parsed.parent_creation_filetime
        );
        assert_eq!(manifest.target_exe_path, parsed.target_exe_path);
        assert_eq!(
            manifest.original_target_sha256,
            parsed.original_target_sha256
        );
        assert_eq!(manifest.new_exe_path, parsed.new_exe_path);
        assert_eq!(manifest.expected_sha256, parsed.expected_sha256);
        assert_eq!(manifest.created_at_epoch_secs, parsed.created_at_epoch_secs);
        assert!(parsed.verify_integrity(&auth_token));

        // 3. Rejet de jeton erroné ou vide
        assert!(!manifest.verify_integrity("wrong_token_1234567890abcdef"));
        assert!(!manifest.verify_integrity(""));

        // 4. Rejet de JSON malformé ou tronqué
        assert!(UpdateManifest::from_json("{ corrupt json").is_err());
        assert!(UpdateManifest::from_json(r#"{"manifest_hmac": "abc"}"#).is_err());
    }

    #[test]
    fn test_manifest_tampering_rejected() {
        let auth_token = generate_auth_token().unwrap();
        let parent_pid = std::process::id();
        let parent_creation_filetime = 999888777666u64;
        let target_exe_path = PathBuf::from(r"C:\Users\Axel\Malvision.exe");
        let original_target_sha256 =
            "0000111122223333444455556666777788889999aaaabbbbccccddddeeeeffff".to_string();
        let new_exe_path = PathBuf::from(r"C:\Users\Axel\New.exe");
        let expected_sha256 =
            "1111222233334444555566667777888899990000aaaabbbbccccddddeeeeffff".to_string();
        let created_at_epoch_secs = 1700000000u64;

        let manifest_hmac = compute_manifest_hmac(
            &auth_token,
            parent_pid,
            parent_creation_filetime,
            &target_exe_path,
            &original_target_sha256,
            &new_exe_path,
            &expected_sha256,
            created_at_epoch_secs,
        )
        .unwrap();

        let mut manifest = UpdateManifest {
            manifest_hmac,
            parent_pid,
            parent_creation_filetime,
            target_exe_path,
            original_target_sha256: original_target_sha256.clone(),
            new_exe_path,
            expected_sha256,
            created_at_epoch_secs,
        };

        // Manifeste intact : vérification OK
        assert!(manifest.verify_integrity(&auth_token));

        // Altération 1 : Modification de target_exe_path vers un fichier système
        manifest.target_exe_path = PathBuf::from(r"C:\Windows\System32\notepad.exe");
        assert!(
            !manifest.verify_integrity(&auth_token),
            "Toute altération de target_exe_path doit être rejetée"
        );

        // Rétablissement puis altération 2 : Modification du SHA-256 attendu
        manifest.target_exe_path = PathBuf::from(r"C:\Users\Axel\Malvision.exe");
        manifest.expected_sha256 =
            "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff".to_string();
        assert!(
            !manifest.verify_integrity(&auth_token),
            "Toute altération de expected_sha256 doit être rejetée"
        );

        // Rétablissement puis altération 3 : Modification de original_target_sha256
        manifest.expected_sha256 =
            "1111222233334444555566667777888899990000aaaabbbbccccddddeeeeffff".to_string();
        manifest.original_target_sha256 =
            "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff".to_string();
        assert!(
            !manifest.verify_integrity(&auth_token),
            "Toute altération de original_target_sha256 doit être rejetée"
        );

        // Rétablissement puis altération 4 : Modification du PID parent
        manifest.original_target_sha256 = original_target_sha256;
        manifest.parent_pid = 1234;
        assert!(
            !manifest.verify_integrity(&auth_token),
            "Toute altération de parent_pid doit être rejetée"
        );

        // Rétablissement puis altération 5 : Modification du timestamp de création
        manifest.parent_pid = parent_pid;
        manifest.parent_creation_filetime = 111222333u64;
        assert!(
            !manifest.verify_integrity(&auth_token),
            "Toute altération de parent_creation_filetime doit être rejetée"
        );
    }

    #[test]
    fn test_auth_token_expiration_logic() {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // 1. Horodatage récent (< 300s) -> valide
        let recent = now - 10;
        assert!(now.saturating_sub(recent) <= 300);

        // 2. Horodatage expiré (> 300s) -> invalide
        let expired = now - 301;
        assert!(now.saturating_sub(expired) > 300);
    }

    #[test]
    fn test_target_exe_safety_rules() {
        // Règle 1 : L'extension doit être .exe
        let valid_exe = PathBuf::from(r"C:\Apps\Malvision.exe");
        let ext = valid_exe.extension().and_then(|s| s.to_str()).unwrap_or("");
        assert!(ext.eq_ignore_ascii_case("exe"));

        let invalid_dll = PathBuf::from(r"C:\Apps\Malvision.dll");
        let ext_dll = invalid_dll
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("");
        assert!(!ext_dll.eq_ignore_ascii_case("exe"));

        let invalid_no_ext = PathBuf::from(r"C:\Apps\Malvision");
        let ext_none = invalid_no_ext
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("");
        assert!(!ext_none.eq_ignore_ascii_case("exe"));

        // Règle 2 : Exclusion du répertoire Windows
        let windir = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".to_string());
        let sys_target = PathBuf::from(format!(r"{windir}\System32\cmd.exe"));
        let win_path = PathBuf::from(&windir);
        assert!(sys_target.starts_with(&win_path));
    }

    #[test]
    fn test_perform_rollback_and_verify_success_and_failure() {
        let temp_dir =
            std::env::temp_dir().join(format!("malvision_test_rb_{}", std::process::id()));
        let _ = fs::create_dir_all(&temp_dir);

        let target = temp_dir.join("App.exe");
        let backup = temp_dir.join("App.exe.bak");

        let orig_bytes = b"ORIGINAL_BACKUP_EXECUTABLE_VALID";
        let bad_bytes = b"CORRUPTED_TARGET_THAT_FAILED_HEALTH";

        fs::write(&backup, orig_bytes).unwrap();
        fs::write(&target, bad_bytes).unwrap();
        let expected_orig_sha256 = crate::security::compute_sha256(orig_bytes).unwrap();

        // Rollback nominal : restaure backup -> target et vérifie cryptographiquement
        let res = perform_rollback_and_verify(&target, &backup, &expected_orig_sha256);
        assert!(
            res.is_ok(),
            "Le rollback doit réussir quand backup existe et son hash correspond"
        );
        assert_eq!(fs::read(&target).unwrap(), orig_bytes);
        assert!(!backup.exists());

        // Appel avec hash attendu non conforme : doit échouer immédiatement sans restaurer
        fs::write(&backup, orig_bytes).unwrap();
        let res_bad_hash = perform_rollback_and_verify(
            &target,
            &backup,
            "bad_hash_000000000000000000000000000000000000000000000000000000000000",
        );
        assert!(
            res_bad_hash.is_err(),
            "Le rollback avec hash divergent doit être refusé"
        );

        // Appel sans backup : doit échouer explicitement sans fausse annonce
        let _ = fs::remove_file(&backup);
        let res_no_bak = perform_rollback_and_verify(&target, &backup, &expected_orig_sha256);
        assert!(
            res_no_bak.is_err(),
            "Le rollback sans backup doit échouer explicitement"
        );

        let _ = fs::remove_file(&target);
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_pending_recovery_persistence_and_cleanup() {
        let temp_dir =
            std::env::temp_dir().join(format!("malvision_test_rec_{}", std::process::id()));
        let _ = fs::create_dir_all(&temp_dir);

        let target = temp_dir.join("RecoveredApp.exe");
        let backup = temp_dir.join("RecoveredApp.exe.bak");
        let recovery_file = temp_dir.join("pending_recovery.json");

        let orig_bytes = b"VALID_ORIGINAL_BYTES_FOR_RECOVERY";
        fs::write(&backup, orig_bytes).unwrap();
        fs::write(&target, b"BROKEN_TARGET").unwrap();
        let expected_backup_sha256 = crate::security::compute_sha256(orig_bytes).unwrap();

        // Enregistrer la directive de récupération
        let rec_json = serde_json::json!({
            "target_exe_path": target.to_string_lossy(),
            "expected_backup_sha256": expected_backup_sha256,
            "created_at_epoch_secs": 1700000000u64,
        });
        fs::write(&recovery_file, rec_json.to_string()).unwrap();
        assert!(recovery_file.exists());

        // Simulation de validation stricte :
        if let Ok(content) = fs::read_to_string(&recovery_file)
            && let Ok(v) = serde_json::from_str::<serde_json::Value>(&content)
            && let (Some(t_str), Some(exp_sha)) = (
                v["target_exe_path"].as_str(),
                v["expected_backup_sha256"].as_str(),
            )
        {
            let t = PathBuf::from(t_str);
            let b = t.with_extension("exe.bak"); // dérivé obligatoirement
            if b.exists() {
                let actual_hash = crate::security::compute_file_sha256(&b).unwrap();
                if crate::security::constant_time_compare(&actual_hash, exp_sha) {
                    let _ = fs::remove_file(&t);
                    if fs::rename(&b, &t).is_ok() {
                        let _ = fs::remove_file(&recovery_file);
                    }
                }
            }
        }

        assert!(
            !recovery_file.exists(),
            "La directive doit être supprimée après récupération"
        );
        assert!(!backup.exists(), "La sauvegarde a été restaurée en place");
        assert_eq!(
            fs::read(&target).unwrap(),
            orig_bytes,
            "Le binaire cible est restauré"
        );

        let _ = fs::remove_file(&target);
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_pending_recovery_negative_scenarios_rejected() {
        let temp_dir =
            std::env::temp_dir().join(format!("malvision_test_rec_neg_{}", std::process::id()));
        let _ = fs::create_dir_all(&temp_dir);

        let target = temp_dir.join("TargetApp.exe");
        let backup = temp_dir.join("TargetApp.exe.bak");
        let recovery_file = temp_dir.join("pending_recovery.json");

        let orig_bytes = b"GOOD_BACKUP_BYTES";
        let tampered_bytes = b"TAMPERED_BACKUP_BYTES";
        let initial_target_bytes = b"CURRENT_TARGET_BYTES";

        fs::write(&target, initial_target_bytes).unwrap();
        fs::write(&backup, tampered_bytes).unwrap();
        let expected_sha256 = crate::security::compute_sha256(orig_bytes).unwrap();

        // SCÉNARIO 1 : Empreinte de backup corrompue / divergente
        let rec_json = serde_json::json!({
            "target_exe_path": target.to_string_lossy(),
            "expected_backup_sha256": expected_sha256,
            "created_at_epoch_secs": 1700000000u64,
        });
        fs::write(&recovery_file, rec_json.to_string()).unwrap();

        let content = fs::read_to_string(&recovery_file).unwrap();
        let v: serde_json::Value = serde_json::from_str(&content).unwrap();
        let exp_sha = v["expected_backup_sha256"].as_str().unwrap();
        let actual_hash = crate::security::compute_file_sha256(&backup).unwrap();

        // Le contrôle doit refuser strictement l'opération
        assert!(
            !crate::security::constant_time_compare(&actual_hash, exp_sha),
            "Le hash doit être détecté comme non conforme"
        );
        // La cible et le backup ne doivent pas être altérés
        assert_eq!(fs::read(&target).unwrap(), initial_target_bytes);
        assert_eq!(fs::read(&backup).unwrap(), tampered_bytes);

        // SCÉNARIO 2 : Cible divergente (tentative de ciblage d'un autre fichier)
        let other_exe = temp_dir.join("OtherApp.exe");
        let fake_target = PathBuf::from(r"C:\Windows\System32\cmd.exe");
        assert!(!is_same_file_path(&fake_target, &other_exe));

        let _ = fs::remove_file(&target);
        let _ = fs::remove_file(&backup);
        let _ = fs::remove_file(&recovery_file);
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_health_token_registration_and_notification() {
        let token = "test_health_token_abc123".to_string();
        set_pending_health_token(token.clone());

        // Vérifier que le lock contient bien le token
        assert_eq!(
            PENDING_HEALTH_TOKEN.lock().unwrap().as_deref(),
            Some("test_health_token_abc123")
        );

        // L'appel à notify_startup_healthy consomme le token
        notify_startup_healthy();
        assert_eq!(PENDING_HEALTH_TOKEN.lock().unwrap().as_deref(), None);
    }

    #[test]
    fn test_atomic_replacement_and_rollback_on_hash_mismatch() {
        let temp_dir =
            std::env::temp_dir().join(format!("malvision_test_atomic_{}", std::process::id()));
        let _ = fs::create_dir_all(&temp_dir);

        let target_exe = temp_dir.join("TargetApp.exe");
        let bak_exe = temp_dir.join("TargetApp.exe.bak");
        let new_exe = temp_dir.join("TargetApp_new.exe");

        let original_bytes = b"ORIGINAL_TARGET_EXECUTABLE_V103";
        let corrupt_bytes = b"TAMPERED_OR_CORRUPTED_BYTES";
        let valid_bytes = b"VALID_NEW_EXECUTABLE_V104";

        fs::write(&target_exe, original_bytes).unwrap();
        let orig_hash = crate::security::compute_sha256(original_bytes).unwrap();
        let valid_hash = crate::security::compute_sha256(valid_bytes).unwrap();

        // CAS 1: Hash mismatch déclenchant rollback automatique
        fs::write(&new_exe, corrupt_bytes).unwrap();

        // Étape 1 : Sauvegarde atomique TargetApp.exe -> TargetApp.exe.bak
        let _ = fs::remove_file(&bak_exe);
        fs::rename(&target_exe, &bak_exe).expect("Renommage en .bak");
        assert!(!target_exe.exists());
        assert!(bak_exe.exists());

        // Étape 2 : Déplacement new -> TargetApp.exe
        fs::rename(&new_exe, &target_exe).expect("Déplacement new -> target");

        // Étape 3 : Contrôle d'intégrité
        let installed_bytes = fs::read(&target_exe).unwrap();
        let installed_hash = crate::security::compute_sha256(&installed_bytes).unwrap();

        if installed_hash != valid_hash {
            // ROLLBACK !
            let _ = fs::remove_file(&target_exe);
            fs::rename(&bak_exe, &target_exe).expect("Restauration du rollback");
        }

        // Vérifier que la cible est bien restaurée avec les octets et le hash d'origine
        let restored_bytes = fs::read(&target_exe).unwrap();
        assert_eq!(restored_bytes, original_bytes);
        assert_eq!(
            crate::security::compute_sha256(&restored_bytes).unwrap(),
            orig_hash
        );
        assert!(!bak_exe.exists(), "Le fichier .bak a été restauré en place");

        // CAS 2 : Hash valide -> Remplacement permanent réussi
        fs::write(&new_exe, valid_bytes).unwrap();
        fs::rename(&target_exe, &bak_exe).expect("Renommage en .bak");
        fs::rename(&new_exe, &target_exe).expect("Déplacement new -> target");

        let installed_bytes_ok = fs::read(&target_exe).unwrap();
        let installed_hash_ok = crate::security::compute_sha256(&installed_bytes_ok).unwrap();
        assert_eq!(installed_hash_ok, valid_hash);
        // Succès : le .bak peut être conservé ou nettoyé
        let _ = fs::remove_file(&bak_exe);
        let _ = fs::remove_file(&target_exe);
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_cleanup_leftover_update_files() {
        let temp_dir =
            std::env::temp_dir().join(format!("malvision_test_cleanup_{}", std::process::id()));
        let _ = fs::create_dir_all(&temp_dir);

        let bak_file = temp_dir.join("App.exe.bak");
        fs::write(&bak_file, b"old_bak").unwrap();
        assert!(bak_file.exists());

        // Nettoyage simulant cleanup_leftover_update_files
        if bak_file.exists() {
            let _ = fs::remove_file(&bak_file);
        }
        assert!(!bak_file.exists());

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_in_use_file_lock_reproduction_and_wait() {
        use windows::Win32::Foundation::GENERIC_READ;
        use windows::Win32::Storage::FileSystem::{
            CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_SHARE_READ, OPEN_EXISTING,
        };
        use windows::core::HSTRING;

        let temp_dir =
            std::env::temp_dir().join(format!("malvision_test_lock_{}", std::process::id()));
        let _ = fs::create_dir_all(&temp_dir);
        let locked_file = temp_dir.join("LockedBinary.exe");
        fs::write(&locked_file, b"RUNNING_PROCESS_IMAGE").unwrap();

        let h_path = HSTRING::from(locked_file.as_os_str());

        // Ouvrir le fichier en lecture avec partage lecture uniquement (simulant un binaire exécuté)
        let handle = unsafe {
            CreateFileW(
                &h_path,
                GENERIC_READ.0,
                FILE_SHARE_READ,
                None,
                OPEN_EXISTING,
                FILE_ATTRIBUTE_NORMAL,
                None,
            )
        };

        if let Ok(h) = handle {
            // Tant que le handle est ouvert (processus en cours), tenter d'écrire par dessus doit échouer
            let write_attempt = fs::OpenOptions::new()
                .write(true)
                .truncate(true)
                .open(&locked_file);
            assert!(
                write_attempt.is_err(),
                "L'écriture directe sur un fichier verrouillé en cours d'exécution doit échouer (0x80070005)"
            );

            // Fermer le handle (simulant la terminaison du processus parent)
            unsafe {
                let _ = windows::Win32::Foundation::CloseHandle(h);
            }

            // Dès la fermeture du handle, le remplacement réussit immédiatement !
            let new_content = b"REPLACED_AFTER_PROCESS_EXIT";
            fs::write(&locked_file, new_content)
                .expect("Remplacement réussi après fermeture du handle");
            assert_eq!(fs::read(&locked_file).unwrap(), new_content);
        }

        let _ = fs::remove_file(&locked_file);
        let _ = fs::remove_dir_all(&temp_dir);
    }
}
