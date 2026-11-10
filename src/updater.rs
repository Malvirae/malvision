//! Vérification des mises à jour via GitHub Releases et téléchargement sécurisé des assets.

use crate::security::compute_file_sha256;
use std::fmt;
use std::path::Path;
use windows::Win32::Networking::WinHttp::{
    INTERNET_DEFAULT_HTTPS_PORT, WINHTTP_ACCESS_TYPE_DEFAULT_PROXY, WINHTTP_FLAG_SECURE,
    WINHTTP_QUERY_FLAG_NUMBER, WINHTTP_QUERY_LOCATION, WINHTTP_QUERY_STATUS_CODE,
    WinHttpCloseHandle, WinHttpConnect, WinHttpOpen, WinHttpOpenRequest, WinHttpQueryDataAvailable,
    WinHttpQueryHeaders, WinHttpReadData, WinHttpReceiveResponse, WinHttpSendRequest,
    WinHttpSetTimeouts,
};
use windows_core::{PCWSTR, w};

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
    pub const CURRENT: Self = Self {
        major: 1,
        minor: 0,
        patch: 1,
    };

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

        let user_agent = w!("Malvision/1.0.2 (Windows NT 10.0; Win64; x64)");
        let session = WinHttpOpen(
            user_agent,
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
        let headers: Vec<u16> = "Accept: application/vnd.github.v3+json, application/octet-stream, */*\r\nUser-Agent: Malvision/1.0.2\r\n"
            .encode_utf16()
            .collect();
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
