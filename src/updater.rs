//! Vérification des mises à jour via GitHub Releases et téléchargement sécurisé des assets.

use crate::security::compute_file_sha256;
use std::fmt;
use std::path::Path;
use windows::Win32::Networking::WinHttp::{
    INTERNET_DEFAULT_HTTPS_PORT, WINHTTP_ACCESS_TYPE_DEFAULT_PROXY, WINHTTP_FLAG_SECURE,
    WINHTTP_OPEN_REQUEST_FLAGS, WINHTTP_QUERY_FLAG_NUMBER, WINHTTP_QUERY_LOCATION,
    WINHTTP_QUERY_STATUS_CODE, WinHttpCloseHandle, WinHttpConnect, WinHttpOpen, WinHttpOpenRequest,
    WinHttpQueryDataAvailable, WinHttpQueryHeaders, WinHttpReadData, WinHttpReceiveResponse,
    WinHttpSendRequest, WinHttpSetTimeouts,
};
use windows_core::{PCWSTR, w};

pub const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const GITHUB_REPO: &str = "Malvirae/malvision";

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
        patch: 0,
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

/// Télécharge une ressource HTTP/HTTPS via l'API Win32 native WinHTTP.
pub fn winhttp_get(url: &str) -> Result<Vec<u8>, String> {
    unsafe {
        let (host, port, path, is_https) = parse_url(url)?;

        let user_agent = w!("Malvision/1.0.0 (Windows NT 10.0; Win64; x64)");
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
        let flags = if is_https {
            WINHTTP_FLAG_SECURE
        } else {
            WINHTTP_OPEN_REQUEST_FLAGS(0)
        };

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
        let headers: Vec<u16> = "Accept: application/vnd.github.v3+json, application/octet-stream, */*\r\nUser-Agent: Malvision/1.0.0\r\n"
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

        // Gestion des redirections HTTP (301, 302, 307, 308)
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
                    return winhttp_get(&redirect_url);
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

fn parse_url(url: &str) -> Result<(String, u16, String, bool), String> {
    let (is_https, rest) = if let Some(stripped) = url.strip_prefix("https://") {
        (true, stripped)
    } else if let Some(stripped) = url.strip_prefix("http://") {
        (false, stripped)
    } else {
        return Err(
            "Protocole d'URL non supporté (seuls http et https sont acceptés).".to_string(),
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
        None => {
            let port = if is_https {
                INTERNET_DEFAULT_HTTPS_PORT
            } else {
                80
            };
            (host_port.to_string(), port)
        }
    };

    Ok((host, port, path.to_string(), is_https))
}

/// Vérifie la dernière release publique disponible sur le dépôt GitHub.
pub fn check_latest_release(repo: &str) -> Result<ReleaseInfo, String> {
    let url = format!("https://api.github.com/repos/{repo}/releases/latest");
    let body = winhttp_get(&url)?;
    let json_str = String::from_utf8(body).map_err(|e| format!("Réponse UTF-8 invalide : {e}"))?;
    parse_release_json(&json_str)
}

/// Écrit les octets téléchargés dans un fichier temporaire, vérifie l'intégrité SHA-256 et déplace atomiquement vers le chemin final.
pub fn write_and_verify_asset_bytes(
    data: &[u8],
    dest_path: &Path,
    expected_sha256: Option<&str>,
) -> Result<(), String> {
    if data.is_empty() {
        return Err("Le téléchargement a retourné un fichier vide (0 octet).".to_string());
    }

    let temp_path = dest_path.with_extension("malvision_download_tmp");
    if temp_path.exists() {
        let _ = std::fs::remove_file(&temp_path);
    }

    // Écrire dans le fichier temporaire
    std::fs::write(&temp_path, data).map_err(|e| {
        format!(
            "Impossible d'écrire le fichier temporaire '{}' : {e}",
            temp_path.display()
        )
    })?;

    // Vérifier l'empreinte SHA-256 si une valeur de référence est fournie
    if let Some(expected_hash) = expected_sha256 {
        let expected_clean = expected_hash.trim().to_ascii_lowercase();
        if !expected_clean.is_empty() {
            let computed = compute_file_sha256(&temp_path)?;
            if computed.to_ascii_lowercase() != expected_clean {
                let _ = std::fs::remove_file(&temp_path);
                return Err(format!(
                    "Échec de validation de l'intégrité SHA-256 !\n\nAttendu  : {expected_clean}\nCalculé  : {computed}\n\nLe fichier a été supprimé par sécurité."
                ));
            }
        }
    }

    // Déplacement atomique vers la destination finale
    if dest_path.exists() {
        let _ = std::fs::remove_file(dest_path);
    }
    std::fs::rename(&temp_path, dest_path).map_err(|e| {
        let _ = std::fs::remove_file(&temp_path);
        format!(
            "Impossible de finaliser l'installation vers '{}' : {e}",
            dest_path.display()
        )
    })?;

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
        let target_file = base_dir.join(format!("malvision_download_test_{pid}.exe"));
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

        // 2. Échec avec hash altéré
        let corrupt_hash = "0000000000000000000000000000000000000000000000000000000000000000";
        let target_file2 = base_dir.join(format!("malvision_download_corrupt_{pid}.exe"));
        let _ = std::fs::remove_file(&target_file2);

        let res_err = write_and_verify_asset_bytes(data, &target_file2, Some(corrupt_hash));
        assert!(res_err.is_err(), "Le hash altéré doit être rejeté");
        assert!(
            !target_file2.exists(),
            "Le fichier cible ne doit pas être créé en cas de hash invalide"
        );

        // 3. Échec avec payload vide
        let res_empty = write_and_verify_asset_bytes(b"", &target_file2, None);
        assert!(res_empty.is_err());

        let _ = std::fs::remove_file(&target_file);
        let _ = std::fs::remove_file(&target_file2);
    }
}
