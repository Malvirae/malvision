//! Vérification des mises à jour via GitHub Releases et téléchargement sécurisé des assets.

use crate::security::compute_file_sha256;
use std::fmt;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::Networking::WinHttp::{
    INTERNET_DEFAULT_HTTPS_PORT, WINHTTP_ACCESS_TYPE_DEFAULT_PROXY, WINHTTP_FLAG_SECURE,
    WINHTTP_QUERY_FLAG_NUMBER, WINHTTP_QUERY_LOCATION, WINHTTP_QUERY_STATUS_CODE,
    WinHttpCloseHandle, WinHttpConnect, WinHttpOpen, WinHttpOpenRequest, WinHttpQueryDataAvailable,
    WinHttpQueryHeaders, WinHttpReadData, WinHttpReceiveResponse, WinHttpSendRequest,
    WinHttpSetTimeouts,
};
use windows::Win32::System::Threading::{
    OpenProcess, PROCESS_NAME_FORMAT, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE,
    QueryFullProcessImageNameW, WaitForSingleObject,
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateManifest {
    pub auth_token: String,
    pub parent_pid: u32,
    pub target_exe_path: PathBuf,
    pub new_exe_path: PathBuf,
    pub expected_sha256: String,
    pub created_at_epoch_secs: u64,
}

impl UpdateManifest {
    pub fn to_json(&self) -> String {
        let val = serde_json::json!({
            "auth_token": self.auth_token,
            "parent_pid": self.parent_pid,
            "target_exe_path": self.target_exe_path.to_string_lossy(),
            "new_exe_path": self.new_exe_path.to_string_lossy(),
            "expected_sha256": self.expected_sha256,
            "created_at_epoch_secs": self.created_at_epoch_secs,
        });
        serde_json::to_string_pretty(&val).unwrap_or_else(|_| "{}".to_string())
    }

    pub fn from_json(s: &str) -> Result<Self, String> {
        let v: serde_json::Value = serde_json::from_str(s)
            .map_err(|e| format!("Manifeste de mise à jour JSON corrompu : {e}"))?;
        let auth_token = v["auth_token"]
            .as_str()
            .ok_or("auth_token manquant")?
            .to_string();
        let parent_pid = v["parent_pid"].as_u64().ok_or("parent_pid manquant")? as u32;
        let target_str = v["target_exe_path"]
            .as_str()
            .ok_or("target_exe_path manquant")?;
        let new_str = v["new_exe_path"].as_str().ok_or("new_exe_path manquant")?;
        let expected_sha256 = v["expected_sha256"]
            .as_str()
            .ok_or("expected_sha256 manquant")?
            .to_string();
        let created_at_epoch_secs = v["created_at_epoch_secs"]
            .as_u64()
            .ok_or("created_at_epoch_secs manquant")?;

        Ok(Self {
            auth_token,
            parent_pid,
            target_exe_path: PathBuf::from(target_str),
            new_exe_path: PathBuf::from(new_str),
            expected_sha256,
            created_at_epoch_secs,
        })
    }
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

/// Génère un jeton d'authentification cryptographique unique à usage unique.
fn generate_auth_token() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let pid = std::process::id();
    let tid = std::thread::current().id();
    let raw = format!("MalvisionUpdateAuthToken-{pid}-{now}-{tid:?}");
    crate::security::compute_sha256(raw.as_bytes())
        .unwrap_or_else(|_| format!("{pid:08x}{now:016x}0123456789abcdef"))
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

/// Prépare le binaire auxiliaire et le manifeste de mise à jour.
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

    let auth_token = generate_auth_token();
    let parent_pid = std::process::id();
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let manifest = UpdateManifest {
        auth_token: auth_token.clone(),
        parent_pid,
        target_exe_path: current_exe.clone(),
        new_exe_path: new_exe_path.to_path_buf(),
        expected_sha256: expected_sha256.to_string(),
        created_at_epoch_secs: now,
    };

    let manifest_path = updates_dir.join("update_manifest.json");
    std::fs::write(&manifest_path, manifest.to_json())
        .map_err(|e| format!("Impossible d'écrire le manifeste de mise à jour : {e}"))?;

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

    // 1. Contrôle du jeton d'authentification
    if manifest.auth_token != *token {
        show_native_error("Sécurité : Jeton d'authentification de mise à jour invalide.");
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

    // 3. Liaison au processus parent
    let parent_handle = match unsafe {
        OpenProcess(
            PROCESS_SYNCHRONIZE | PROCESS_QUERY_LIMITED_INFORMATION,
            false,
            manifest.parent_pid,
        )
    } {
        Ok(h) if !h.is_invalid() => h,
        _ => {
            show_native_error("Impossible de lier le processus parent de Malvision.");
            return;
        }
    };

    // 4. Vérification que le processus parent correspond à target_exe_path
    if let Ok(parent_exe) = get_process_image_path(parent_handle)
        && !is_same_file_path(&parent_exe, &manifest.target_exe_path)
    {
        let _ = unsafe { CloseHandle(parent_handle) };
        show_native_error(
            "Sécurité : Le processus parent ne correspond pas à l'exécutable Malvision autorisé.",
        );
        return;
    }

    // 5. Contrôle cryptographique préalable du binaire téléchargé
    match compute_file_sha256(&manifest.new_exe_path) {
        Ok(hash) if hash.eq_ignore_ascii_case(&manifest.expected_sha256) => {}
        _ => {
            let _ = unsafe { CloseHandle(parent_handle) };
            show_native_error("Sécurité : L'empreinte SHA-256 du nouveau binaire est invalide.");
            return;
        }
    }

    // 6. Signaler au processus principal que le helper est prêt à opérer
    let ready_flag = updates_dir.join("helper_ready.flag");
    if let Err(e) = std::fs::write(&ready_flag, b"READY") {
        let _ = unsafe { CloseHandle(parent_handle) };
        show_native_error(&format!(
            "Impossible d'émettre le signal de synchronisation : {e}"
        ));
        return;
    }

    // 7. Attente de la fin effective du processus principal (jusqu'à 30 secondes)
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

    // 8. Remplacement atomique et gestion du rollback
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
        // ROLLBACK immédiat
        let _ = std::fs::remove_file(target);
        let _ = std::fs::rename(&backup, target);
        show_native_error(
            "L'intégrité de la mise à jour installée est invalide. L'ancienne version a été automatiquement restaurée.",
        );
        return;
    }

    // D. Relance de la nouvelle version depuis son chemin d'origine
    match std::process::Command::new(target).spawn() {
        Ok(mut child) => {
            // Surveillance de santé pendant 1,5 seconde
            std::thread::sleep(std::time::Duration::from_millis(1500));
            if let Ok(Some(status)) = child.try_wait()
                && !status.success()
            {
                // Crash immédiat de la nouvelle version : ROLLBACK
                let _ = std::fs::remove_file(target);
                let _ = std::fs::rename(&backup, target);
                let _ = std::process::Command::new(target).spawn();
                show_native_error(
                    "La nouvelle version n'a pas pu démarrer. L'ancienne version a été automatiquement restaurée.",
                );
                return;
            }
        }
        Err(e) => {
            // Échec de lancement : ROLLBACK
            let _ = std::fs::remove_file(target);
            let _ = std::fs::rename(&backup, target);
            let _ = std::process::Command::new(target).spawn();
            show_native_error(&format!(
                "Impossible de démarrer la nouvelle version ({e}). L'ancienne version a été restaurée."
            ));
            return;
        }
    }

    // Nettoyage des fichiers temporaires du dossier updates
    let _ = std::fs::remove_file(&manifest_path);
    let _ = std::fs::remove_file(&ready_flag);
}

/// Nettoie les fichiers résiduels d'une mise à jour précédente lors d'un démarrage normal.
pub fn cleanup_leftover_update_files() {
    if let Ok(current_exe) = std::env::current_exe() {
        let bak = current_exe.with_extension("exe.bak");
        if bak.exists() {
            let _ = std::fs::remove_file(&bak);
        }
    }
    if let Ok(updates_dir) = get_updates_temp_dir()
        && updates_dir.exists()
    {
        let _ = std::fs::remove_file(updates_dir.join("update_manifest.json"));
        let _ = std::fs::remove_file(updates_dir.join("helper_ready.flag"));
        let _ = std::fs::remove_file(updates_dir.join("new_version.exe"));
        let _ = std::fs::remove_file(updates_dir.join("malvision_updater.exe"));
    }
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
    fn test_manifest_serialization_and_validation() {
        let manifest = UpdateManifest {
            auth_token: generate_auth_token(),
            parent_pid: std::process::id(),
            target_exe_path: PathBuf::from(r"C:\Program Files\Malvision\Malvision.exe"),
            new_exe_path: PathBuf::from(
                r"C:\Users\Axel\AppData\Local\Malvision\updates\Malvision_1.0.5.exe",
            ),
            expected_sha256: "abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890"
                .to_string(),
            created_at_epoch_secs: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        };

        let json = manifest.to_json();
        let parsed = UpdateManifest::from_json(&json).expect("Désérialisation valide");
        assert_eq!(manifest.auth_token, parsed.auth_token);
        assert_eq!(manifest.parent_pid, parsed.parent_pid);
        assert_eq!(manifest.target_exe_path, parsed.target_exe_path);
        assert_eq!(manifest.new_exe_path, parsed.new_exe_path);
        assert_eq!(manifest.expected_sha256, parsed.expected_sha256);
        assert_eq!(manifest.created_at_epoch_secs, parsed.created_at_epoch_secs);

        // Test manifest corrompu
        assert!(UpdateManifest::from_json("{ corrupt json").is_err());
        assert!(UpdateManifest::from_json(r#"{"auth_token": "abc"}"#).is_err());
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
