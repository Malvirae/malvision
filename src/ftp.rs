//! Client FTP intégré natif Windows (RFC 959 / RFC 3659) pour Malvision v1.0.5.
//!
//! Architecture :
//! - Moteur protocole FTP pur Rust sur `std::net::TcpStream` sans dépendance externe :
//!   - Authentification `USER`/`PASS` avec protection des mots de passe en mémoire (`SecretString`).
//!   - Navigation (`PWD`, `CWD`, `CDUP`), mode binaire (`TYPE I`), UTF-8 (`OPTS UTF8 ON`).
//!   - Mode passif `PASV` / `EPSV` avec repli automatique sur l'IP distante si le serveur annonce une IP privée/0.0.0.0.
//!   - Listing `MLSD` et `LIST` (formats Unix `ls -l` et Windows/DOS `<DIR>`).
//!   - Transferts (`RETR`, `STOR`) par flux avec barre de progression, annulation et fichier `.part` atomique.
//!   - Opérations complètes : `MKD`, `RMD`, `DELE`, `RNFR`/`RNTO`.
//!   - Protection stricte contre toute injection CRLF/NUL dans les arguments FTP.
//! - Interface native Win32 Fluent (`MalvisionFtpHostClass`) synchronisée avec les thèmes Sombre/Clair/Système
//!   et le scaling DPI de Malvision, avec barre d'outils, barre de chemin, explorateur double-bufferisé,
//!   menu contextuel, navigation clavier, glisser-déposer (`WM_DROPFILES`) et journal d'activité.

use crate::external_apps::{browse_for_open_file, browse_for_save_file};
use crate::security::SecretString;
use crate::ui::{
    COL_FOLDER_AMBER, COL_PROTO_FTP, COL_TEXT_ON_ACCENT, GdiPalette, active_theme_colors,
    col_accent, col_accent_hover, col_bg_deep, col_bg_hover, col_bg_input, col_bg_sidebar,
    col_bg_surface, col_border, col_status_err, col_status_ok, col_text_muted, col_text_primary,
    confirm_dialog, create_dark_edit, draw_filled_badge, draw_horizontal_line, draw_icon_folder,
    draw_rounded_box, draw_rounded_button, draw_text_line, get_window_text, run_text_prompt_dialog,
    set_window_text, show_error_dialog,
};
use std::collections::{BTreeSet, VecDeque};
use std::ffi::c_void;
use std::fs::File;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, BitBlt, ClientToScreen, CreateCompatibleBitmap, CreateCompatibleDC, CreatePen,
    CreateSolidBrush, DT_END_ELLIPSIS, DT_NOPREFIX, DT_RIGHT, DT_SINGLELINE, DT_VCENTER, DeleteDC,
    DeleteObject, DrawTextW, EndPaint, FillRect, HDC, InvalidateRect, LineTo, MoveToEx,
    PAINTSTRUCT, PS_SOLID, RDW_ALLCHILDREN, RDW_ERASE, RDW_INVALIDATE, RDW_UPDATENOW, RedrawWindow,
    SRCCOPY, SelectObject, SetBkColor, SetBkMode, SetTextColor, TRANSPARENT,
};
use windows::Win32::Storage::FileSystem::GetLogicalDrives;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Registry::{
    HKEY, HKEY_CLASSES_ROOT, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, RRF_RT_REG_EXPAND_SZ,
    RRF_RT_REG_SZ, RegCloseKey, RegEnumValueW, RegGetValueW, RegOpenKeyExW,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetFocus, GetKeyState, ReleaseCapture, SetCapture, SetFocus, TME_LEAVE, TRACKMOUSEEVENT,
    TrackMouseEvent, VK_BACK, VK_CONTROL, VK_DELETE, VK_DOWN, VK_END, VK_F2, VK_F5, VK_HOME,
    VK_RETURN, VK_SHIFT, VK_UP,
};
use windows::Win32::UI::Shell::{
    DragAcceptFiles, DragFinish, DragQueryFileW, HDROP, ShellExecuteW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CallWindowProcW, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyMenu,
    DestroyWindow, EN_KILLFOCUS, EN_SETFOCUS, GWLP_USERDATA, GWLP_WNDPROC, GetClientRect,
    GetCursorPos, GetDlgCtrlID, GetWindowLongPtrW, IDC_ARROW, IDC_HAND, IDC_SIZENS, IDC_SIZEWE,
    LoadCursorW, MF_POPUP, MF_SEPARATOR, MF_STRING, MoveWindow, PostMessageW, RegisterClassExW,
    SW_HIDE, SW_SHOW, SW_SHOWNORMAL, SetCursor, SetForegroundWindow, SetWindowLongPtrW, ShowWindow,
    TPM_LEFTALIGN, TPM_RETURNCMD, TPM_TOPALIGN, TrackPopupMenu, WINDOW_EX_STYLE, WM_APP,
    WM_CAPTURECHANGED, WM_COMMAND, WM_CONTEXTMENU, WM_CTLCOLOREDIT, WM_DESTROY, WM_DROPFILES,
    WM_ERASEBKGND, WM_KEYDOWN, WM_LBUTTONDBLCLK, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE,
    WM_MOUSEWHEEL, WM_PAINT, WM_RBUTTONUP, WM_SETCURSOR, WM_SIZE, WNDCLASSEXW, WNDPROC, WS_CHILD,
    WS_CLIPCHILDREN,
};
use windows_core::{PCWSTR, w};

// =============================================================================
// MESSAGES & ÉVÉNEMENTS VERS LA FENÊTRE PRINCIPALE
// =============================================================================

const WM_MOUSELEAVE: u32 = 0x02A3;

pub const WM_FTP_EVENT: u32 = WM_APP + 120;
pub const WM_FTP_REQUEST_CREDENTIALS: u32 = WM_APP + 121;
const WM_FTP_INTERNAL_REFRESH: u32 = WM_APP + 125;
const WM_FTP_OPEN_EDITOR: u32 = WM_APP + 126;
const WM_FTP_OPEN_EXTERNAL_EDITOR: u32 = WM_APP + 127;
const WM_FTP_EXTERNAL_FILE_CHANGED: u32 = WM_APP + 128;

const MAX_EDITABLE_FILE_BYTES: usize = 5 * 1024 * 1024;
const MAX_EXTERNAL_EDIT_FILE_BYTES: usize = 32 * 1024 * 1024;

pub const FTP_EVT_CONNECTING: usize = 1;
pub const FTP_EVT_CONNECTED: usize = 2;
pub const FTP_EVT_STATUS_TEXT: usize = 3;
pub const FTP_EVT_AUTH_FAILED: usize = 4;
pub const FTP_EVT_ERROR: usize = 5;
pub const FTP_EVT_DISCONNECTED: usize = 6;

#[inline]
pub fn pack_ftp_event_wparam(session_id: u32, event_code: usize) -> WPARAM {
    WPARAM(((session_id as usize) << 8) | (event_code & 0xFF))
}

#[inline]
pub fn unpack_ftp_event_wparam(wparam: WPARAM) -> (u32, usize) {
    let raw = wparam.0;
    let session_id = (raw >> 8) as u32;
    let event_code = raw & 0xFF;
    (session_id, event_code)
}

// =============================================================================
// MOTEUR PROTOCOLE FTP PUR RUST (RFC 959 / RFC 3659)
// =============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FtpEntryKind {
    ParentDir,
    Directory,
    File,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FtpEntry {
    pub name: String,
    pub kind: FtpEntryKind,
    pub size: Option<u64>,
    pub modified: String,
    pub permissions: String,
}

/// Valide qu'aucun caractère de contrôle CRLF ou NUL ne permet d'injecter des commandes FTP.
pub fn sanitize_ftp_arg(arg: &str) -> Result<(), String> {
    if arg.chars().any(|c| c == '\r' || c == '\n' || c == '\0') {
        return Err(
            "L'argument FTP contient des caractères de contrôle interdits (CR/LF/NUL).".to_string(),
        );
    }
    Ok(())
}

/// Formate lisiblement une taille de fichier en octets, Ko, Mo ou Go.
pub fn format_file_size(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;
    let b = bytes as f64;
    if b < KB {
        format!("{bytes} o")
    } else if b < MB {
        format!("{:.1} Ko", b / KB)
    } else if b < GB {
        format!("{:.1} Mo", b / MB)
    } else {
        format!("{:.2} Go", b / GB)
    }
}

/// Parse une réponse `227 Entering Passive Mode (h1,h2,h3,h4,p1,p2)` vers `(IpAddr, port)`.
pub fn parse_pasv_response(msg: &str, control_peer_ip: IpAddr) -> Result<SocketAddr, String> {
    let start = msg
        .find('(')
        .ok_or_else(|| format!("Réponse PASV invalide (parenthèse manquante) : {msg}"))?;
    let end = msg[start + 1..]
        .find(')')
        .map(|i| start + 1 + i)
        .ok_or_else(|| format!("Réponse PASV invalide (parenthèse fermante) : {msg}"))?;
    let inner = &msg[start + 1..end];
    let nums: Vec<u16> = inner
        .split(',')
        .map(|part| part.trim().parse::<u16>())
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| format!("Réponse PASV malformée : {msg}"))?;
    if nums.len() != 6 || nums.iter().any(|&n| n > 255) {
        return Err(format!("Réponse PASV hors plage IPv4/port : {msg}"));
    }

    let advertised = Ipv4Addr::new(nums[0] as u8, nums[1] as u8, nums[2] as u8, nums[3] as u8);
    let port = (nums[4] << 8) | nums[5];
    if port == 0 {
        return Err("Port de données PASV égal à 0.".to_string());
    }

    // Repli résilient si le serveur derrière un NAT annonce 0.0.0.0 ou une IP privée alors que
    // l'hôte de contrôle est public/distant.
    let target_ip = if advertised.is_unspecified()
        || (advertised.is_loopback() && !control_peer_ip.is_loopback())
        || (advertised.is_private()
            && match control_peer_ip {
                IpAddr::V4(v4) => !v4.is_private() && !v4.is_loopback(),
                IpAddr::V6(_) => true,
            }) {
        control_peer_ip
    } else {
        IpAddr::V4(advertised)
    };

    Ok(SocketAddr::new(target_ip, port))
}

/// Parse une réponse `229 Entering Extended Passive Mode (|||port|)`.
pub fn parse_epsv_response(msg: &str) -> Result<u16, String> {
    let start = msg
        .find('(')
        .ok_or_else(|| format!("Réponse EPSV invalide : {msg}"))?;
    let end = msg[start + 1..]
        .find(')')
        .map(|i| start + 1 + i)
        .ok_or_else(|| format!("Réponse EPSV invalide : {msg}"))?;
    let inner = &msg[start + 1..end];
    let delim = inner
        .chars()
        .next()
        .ok_or_else(|| format!("Délimiteur EPSV absent : {msg}"))?;
    let parts: Vec<&str> = inner.split(delim).collect();
    if parts.len() < 4 {
        return Err(format!("Format EPSV invalide : {msg}"));
    }
    let port = parts[3]
        .trim()
        .parse::<u16>()
        .map_err(|_| format!("Port EPSV invalide : {msg}"))?;
    if port == 0 {
        return Err("Port EPSV égal à 0.".to_string());
    }
    Ok(port)
}

/// Extrait le répertoire courant d'une réponse `257 "/chemin" ...` (en gérant les guillemets doublés).
pub fn parse_pwd_response(msg: &str) -> Option<String> {
    let first_q = msg.find('"')?;
    let rest = &msg[first_q + 1..];
    let mut out = String::new();
    let mut chars = rest.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '"' {
            if chars.peek() == Some(&'"') {
                chars.next();
                out.push('"');
            } else {
                return Some(if out.is_empty() { "/".to_string() } else { out });
            }
        } else {
            out.push(c);
        }
    }
    None
}

/// Parse une ligne `MLSD` (RFC 3659 : `type=dir;modify=20261116140000;size=123; nom`).
pub fn parse_mlsd_line(line: &str) -> Option<FtpEntry> {
    let trimmed = line.trim_end_matches(['\r', '\n']);
    let (facts_part, name_part) = trimmed.split_once(' ')?;
    let name = name_part.trim().to_string();
    if name.is_empty() || name == "." || name == ".." {
        return None;
    }

    let mut kind = FtpEntryKind::File;
    let mut size: Option<u64> = None;
    let mut modified = String::new();
    let mut perms = String::new();

    for fact in facts_part.split(';').filter(|f| !f.is_empty()) {
        if let Some((k, v)) = fact.split_once('=') {
            match k.to_ascii_lowercase().as_str() {
                "type" => {
                    let vl = v.to_ascii_lowercase();
                    if vl == "cdir" || vl == "pdir" {
                        return None;
                    }
                    if vl == "dir" {
                        kind = FtpEntryKind::Directory;
                    } else {
                        kind = FtpEntryKind::File;
                    }
                }
                "size" => {
                    size = v.parse::<u64>().ok();
                }
                "modify" => {
                    if v.len() >= 12 {
                        modified = format!(
                            "{}-{}-{} {}:{}",
                            &v[0..4],
                            &v[4..6],
                            &v[6..8],
                            &v[8..10],
                            &v[10..12]
                        );
                    } else {
                        modified = v.to_string();
                    }
                }
                "unix.mode" | "perm" => {
                    perms = v.to_string();
                }
                _ => {}
            }
        }
    }

    if kind == FtpEntryKind::Directory {
        size = None;
    }

    Some(FtpEntry {
        name,
        kind,
        size,
        modified,
        permissions: perms,
    })
}

/// Parse une ligne issue de `LIST` (format Unix `ls -l` ou format Windows/IIS MS-DOS).
pub fn parse_list_line(line: &str) -> Option<FtpEntry> {
    let trimmed = line.trim_end_matches(['\r', '\n']);
    if trimmed.is_empty() || trimmed.starts_with("total ") {
        return None;
    }

    // 1. Format Unix `drwxr-xr-x   2 user group   4096 Nov 16 14:00 nom`
    let first = trimmed.chars().next()?;
    if matches!(first, 'd' | '-' | 'l' | 'b' | 'c' | 'p' | 's') && trimmed.len() > 10 {
        let mut parts: Vec<(usize, &str)> = Vec::new();
        let mut in_word = false;
        let mut start = 0;
        for (idx, ch) in trimmed.char_indices() {
            if ch.is_whitespace() {
                if in_word {
                    parts.push((start, &trimmed[start..idx]));
                    in_word = false;
                }
            } else if !in_word {
                start = idx;
                in_word = true;
            }
        }
        if in_word {
            parts.push((start, &trimmed[start..]));
        }

        // Classiquement : perms(0) links(1) owner(2) group(3) size(4) month(5) day(6) time_or_year(7) name(8..)
        if parts.len() >= 9 {
            let perms = parts[0].1.to_string();
            let raw_name = trimmed[parts[8].0..].trim();
            let clean_name = if first == 'l' {
                raw_name.split(" -> ").next().unwrap_or(raw_name).trim()
            } else {
                raw_name
            };
            if clean_name == "." || clean_name == ".." || clean_name.is_empty() {
                return None;
            }

            let kind = if first == 'd' {
                FtpEntryKind::Directory
            } else {
                FtpEntryKind::File
            };
            let size = if kind == FtpEntryKind::Directory {
                None
            } else {
                parts[4].1.parse::<u64>().ok()
            };
            let modified = format!("{} {} {}", parts[5].1, parts[6].1, parts[7].1);

            return Some(FtpEntry {
                name: clean_name.to_string(),
                kind,
                size,
                modified,
                permissions: perms,
            });
        }
    }

    // 2. Format Windows / IIS : `11-16-26  02:00PM       <DIR>          MonDossier`
    // ou `11-16-26  02:00PM               12345 fichier.txt`
    let tokens: Vec<&str> = trimmed.split_whitespace().collect();
    if tokens.len() >= 4 && (tokens[0].contains('-') || tokens[0].contains('/')) {
        // Rechercher l'index du 3e token ou "<DIR>" / taille
        if let Some(pos) = trimmed.find("<DIR>") {
            let after = trimmed[pos + 5..].trim();
            if after == "." || after == ".." || after.is_empty() {
                return None;
            }
            return Some(FtpEntry {
                name: after.to_string(),
                kind: FtpEntryKind::Directory,
                size: None,
                modified: format!("{} {}", tokens[0], tokens[1]),
                permissions: "DIR".to_string(),
            });
        } else if let Ok(sz) = tokens[2].parse::<u64>()
            && let Some(sz_pos) = trimmed.find(tokens[2])
        {
            let after = trimmed[sz_pos + tokens[2].len()..].trim();
            if after == "." || after == ".." || after.is_empty() {
                return None;
            }
            return Some(FtpEntry {
                name: after.to_string(),
                kind: FtpEntryKind::File,
                size: Some(sz),
                modified: format!("{} {}", tokens[0], tokens[1]),
                permissions: String::new(),
            });
        }
    }

    None
}

/// Client FTP synchrone sur `TcpStream` (exécuté sur le thread de travail dédié à l'onglet).
pub struct FtpClient {
    stream: TcpStream,
    peer_ip: IpAddr,
    reader: BufReader<TcpStream>,
}

impl FtpClient {
    /// Établit la connexion TCP + authentification `USER`/`PASS` + `OPTS UTF8 ON` + `TYPE I`.
    /// Retourne `Err((is_auth_failure, message))` afin de distinguer un échec réseau d'un `530` d'identifiants.
    pub fn connect<F>(
        host: &str,
        port: u16,
        username: &str,
        password: &SecretString,
        mut log: F,
    ) -> Result<Self, (bool, String)>
    where
        F: FnMut(bool, &str),
    {
        sanitize_ftp_arg(host).map_err(|e| (false, e))?;
        sanitize_ftp_arg(username).map_err(|e| (false, e))?;
        sanitize_ftp_arg(password.as_str()).map_err(|e| (false, e))?;

        let addr_str = format!("{host}:{port}");
        let addrs: Vec<SocketAddr> = addr_str
            .to_socket_addrs()
            .map_err(|e| {
                (
                    false,
                    format!("Résolution DNS impossible pour '{addr_str}' : {e}"),
                )
            })?
            .collect();
        if addrs.is_empty() {
            return Err((false, format!("Aucune adresse trouvée pour '{addr_str}'.")));
        }

        let mut last_err = None;
        let mut connected_stream = None;
        for addr in addrs {
            match TcpStream::connect_timeout(&addr, Duration::from_secs(10)) {
                Ok(s) => {
                    connected_stream = Some(s);
                    break;
                }
                Err(e) => last_err = Some(e),
            }
        }

        let stream = connected_stream.ok_or_else(|| {
            let err_txt = last_err
                .map(|e| e.to_string())
                .unwrap_or_else(|| "Délai dépassé".to_string());
            (
                false,
                format!("Connexion TCP impossible vers {addr_str} : {err_txt}"),
            )
        })?;

        let _ = stream.set_read_timeout(Some(Duration::from_secs(30)));
        let _ = stream.set_write_timeout(Some(Duration::from_secs(30)));
        let peer_ip = stream
            .peer_addr()
            .map(|a| a.ip())
            .unwrap_or(IpAddr::V4(Ipv4Addr::LOCALHOST));

        let reader_stream = stream
            .try_clone()
            .map_err(|e| (false, format!("Erreur clonage socket FTP : {e}")))?;
        let mut client = Self {
            stream,
            peer_ip,
            reader: BufReader::new(reader_stream),
        };

        // Bannière d'accueil 220
        let (code, banner) = client.read_response().map_err(|e| (false, e))?;
        log(false, &format!("← {banner}"));
        if code / 100 != 2 {
            return Err((
                false,
                format!("Le serveur FTP a refusé la connexion ({banner})."),
            ));
        }

        let effective_user = if username.trim().is_empty() {
            "anonymous"
        } else {
            username.trim()
        };
        log(true, &format!("→ USER {effective_user}"));
        let (u_code, u_msg) = client
            .send_raw_command(&format!("USER {effective_user}"))
            .map_err(|e| (false, e))?;
        log(false, &format!("← {u_msg}"));

        if u_code == 331 || u_code == 332 {
            log(true, "→ PASS ********");
            let (p_code, p_msg) = client
                .send_raw_command(&format!("PASS {}", password.as_str()))
                .map_err(|e| (false, e))?;
            log(false, &format!("← {p_msg}"));
            if p_code == 530 || p_code == 430 {
                return Err((
                    true,
                    format!("Authentification FTP refusée ({p_msg}). Vérifiez vos identifiants."),
                ));
            }
            if p_code / 100 != 2 {
                return Err((
                    false,
                    format!("Échec d'ouverture de session FTP ({p_msg})."),
                ));
            }
        } else if u_code == 530 {
            return Err((true, format!("Utilisateur FTP refusé ({u_msg}).")));
        } else if u_code / 100 != 2 {
            return Err((false, format!("Réponse USER inattendue ({u_msg}).")));
        }

        // Activer UTF-8 si supporté (optionnel)
        if let Ok((c, m)) = client.send_raw_command("OPTS UTF8 ON")
            && c / 100 == 2
        {
            log(false, &format!("← {m}"));
        }

        // Passage en mode binaire
        log(true, "→ TYPE I");
        let (t_code, t_msg) = client.send_raw_command("TYPE I").map_err(|e| (false, e))?;
        log(false, &format!("← {t_msg}"));
        if t_code / 100 != 2 {
            return Err((
                false,
                format!("Échec passage en mode binaire TYPE I ({t_msg})."),
            ));
        }

        Ok(client)
    }

    fn read_response(&mut self) -> Result<(u16, String), String> {
        let mut first_line = String::new();
        let bytes = self
            .reader
            .read_line(&mut first_line)
            .map_err(|e| format!("Erreur lecture réponse FTP : {e}"))?;
        if bytes == 0 {
            return Err("Connexion fermée par le serveur FTP.".to_string());
        }
        let trimmed = first_line.trim_end_matches(['\r', '\n']);
        if trimmed.len() < 3 {
            return Err(format!("Réponse FTP invalide : '{trimmed}'"));
        }
        let code: u16 = trimmed[..3]
            .parse()
            .map_err(|_| format!("Code FTP invalide dans '{trimmed}'"))?;

        let mut full_msg = trimmed.to_string();
        if trimmed.as_bytes().get(3) == Some(&b'-') {
            let prefix = format!("{code:03} ");
            loop {
                let mut line = String::new();
                let n = self
                    .reader
                    .read_line(&mut line)
                    .map_err(|e| format!("Erreur lecture réponse FTP multi-lignes : {e}"))?;
                if n == 0 {
                    break;
                }
                let ltrim = line.trim_end_matches(['\r', '\n']);
                full_msg.push('\n');
                full_msg.push_str(ltrim);
                if ltrim.starts_with(&prefix) {
                    break;
                }
            }
        }
        Ok((code, full_msg))
    }

    fn send_raw_command(&mut self, cmd: &str) -> Result<(u16, String), String> {
        let wire = format!("{cmd}\r\n");
        self.stream
            .write_all(wire.as_bytes())
            .map_err(|e| format!("Erreur d'envoi commande FTP : {e}"))?;
        self.stream
            .flush()
            .map_err(|e| format!("Erreur flush socket FTP : {e}"))?;
        self.read_response()
    }

    pub fn noop(&mut self) -> Result<(), String> {
        let (code, msg) = self.send_raw_command("NOOP")?;
        if code / 100 != 2 {
            return Err(format!("NOOP refusé ({msg})"));
        }
        Ok(())
    }

    pub fn pwd(&mut self) -> Result<String, String> {
        let (code, msg) = self.send_raw_command("PWD")?;
        if code != 257 {
            return Err(format!("Échec PWD ({msg})"));
        }
        parse_pwd_response(&msg).ok_or_else(|| format!("Chemin PWD illisible : {msg}"))
    }

    pub fn cwd(&mut self, path: &str) -> Result<String, String> {
        sanitize_ftp_arg(path)?;
        let (code, msg) = self.send_raw_command(&format!("CWD {path}"))?;
        if code / 100 != 2 {
            return Err(format!("Impossible d'ouvrir le dossier '{path}' ({msg})"));
        }
        self.pwd()
    }

    pub fn cdup(&mut self) -> Result<String, String> {
        let (code, msg) = self.send_raw_command("CDUP")?;
        if code / 100 != 2 {
            // Certains serveurs n'implémentent pas CDUP -> repli CWD ..
            let (c2, m2) = self.send_raw_command("CWD ..")?;
            if c2 / 100 != 2 {
                return Err(format!(
                    "Impossible de remonter au dossier parent ({msg} / {m2})"
                ));
            }
        }
        self.pwd()
    }

    fn open_data_stream(&mut self) -> Result<TcpStream, String> {
        let (code, msg) = self.send_raw_command("PASV")?;
        let data_addr = if code == 227 {
            parse_pasv_response(&msg, self.peer_ip)?
        } else {
            // Repli EPSV (utile notamment en IPv6)
            let (ec, em) = self.send_raw_command("EPSV")?;
            if ec != 229 {
                return Err(format!("Échec mode passif PASV/EPSV ({msg} / {em})"));
            }
            let port = parse_epsv_response(&em)?;
            SocketAddr::new(self.peer_ip, port)
        };

        let ds = TcpStream::connect_timeout(&data_addr, Duration::from_secs(10)).map_err(|e| {
            format!("Connexion socket de données FTP ({data_addr}) impossible : {e}")
        })?;
        let _ = ds.set_read_timeout(Some(Duration::from_secs(30)));
        let _ = ds.set_write_timeout(Some(Duration::from_secs(30)));
        Ok(ds)
    }

    pub fn list(&mut self, current_dir: &str) -> Result<Vec<FtpEntry>, String> {
        let mut entries = Vec::new();
        let mut used_mlsd = false;

        if let Ok(mut data_stream) = self.open_data_stream() {
            let (code, _) = self.send_raw_command("MLSD")?;
            if code / 100 == 1 {
                used_mlsd = true;
                let mut buf = Vec::new();
                let _ = data_stream.read_to_end(&mut buf);
                drop(data_stream);
                let (end_code, end_msg) = self.read_response()?;
                if end_code / 100 != 2 {
                    return Err(format!("Erreur fin de listing MLSD ({end_msg})"));
                }
                let text = String::from_utf8_lossy(&buf);
                for line in text.lines() {
                    if let Some(entry) = parse_mlsd_line(line) {
                        entries.push(entry);
                    }
                }
            } else {
                drop(data_stream);
            }
        }

        if !used_mlsd {
            let mut data_stream = self.open_data_stream()?;
            let (code, msg) = self.send_raw_command("LIST")?;
            if code / 100 != 1 {
                return Err(format!("Échec commande LIST ({msg})"));
            }
            let mut buf = Vec::new();
            data_stream
                .read_to_end(&mut buf)
                .map_err(|e| format!("Erreur lecture LIST : {e}"))?;
            drop(data_stream);
            let (end_code, end_msg) = self.read_response()?;
            if end_code / 100 != 2 {
                return Err(format!("Erreur fin de listing LIST ({end_msg})"));
            }
            let text = String::from_utf8_lossy(&buf);
            for line in text.lines() {
                if let Some(entry) = parse_list_line(line) {
                    entries.push(entry);
                }
            }
        }

        // Dossiers d'abord (alphabétique insensible à la casse), puis fichiers
        entries.sort_by(|a, b| {
            let a_dir = a.kind == FtpEntryKind::Directory;
            let b_dir = b.kind == FtpEntryKind::Directory;
            b_dir
                .cmp(&a_dir)
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });

        if current_dir != "/" && !current_dir.is_empty() {
            entries.insert(
                0,
                FtpEntry {
                    name: "..".to_string(),
                    kind: FtpEntryKind::ParentDir,
                    size: None,
                    modified: String::new(),
                    permissions: String::new(),
                },
            );
        }

        Ok(entries)
    }

    pub fn size(&mut self, remote_name: &str) -> Option<u64> {
        sanitize_ftp_arg(remote_name).ok()?;
        let (code, msg) = self.send_raw_command(&format!("SIZE {remote_name}")).ok()?;
        if code == 213 {
            msg.split_whitespace().nth(1)?.parse::<u64>().ok()
        } else {
            None
        }
    }

    pub fn download_file<F>(
        &mut self,
        remote_name: &str,
        local_path: &Path,
        size_hint: Option<u64>,
        cancel: &AtomicBool,
        mut on_progress: F,
    ) -> Result<u64, String>
    where
        F: FnMut(u64, Option<u64>),
    {
        sanitize_ftp_arg(remote_name)?;
        let total = size_hint.or_else(|| self.size(remote_name));

        let mut data_stream = self.open_data_stream()?;
        let (code, msg) = self.send_raw_command(&format!("RETR {remote_name}"))?;
        if code / 100 != 1 {
            return Err(format!("Échec téléchargement RETR '{remote_name}' ({msg})"));
        }

        let part_path = local_path.with_extension("part");
        let mut out_file = File::create(&part_path).map_err(|e| {
            format!(
                "Impossible de créer le fichier local '{}' : {e}",
                local_path.display()
            )
        })?;

        let mut buf = [0u8; 32 * 1024];
        let mut transferred: u64 = 0;
        let mut read_err: Option<String> = None;

        loop {
            if cancel.load(Ordering::Relaxed) {
                read_err = Some("Téléchargement annulé par l'utilisateur.".to_string());
                break;
            }
            match data_stream.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    if let Err(e) = out_file.write_all(&buf[..n]) {
                        read_err = Some(format!("Erreur d'écriture locale : {e}"));
                        break;
                    }
                    transferred += n as u64;
                    on_progress(transferred, total);
                }
                Err(e) => {
                    read_err = Some(format!("Erreur réception données FTP : {e}"));
                    break;
                }
            }
        }

        drop(out_file);
        drop(data_stream);

        if let Some(err) = read_err {
            let _ = std::fs::remove_file(&part_path);
            let _ = self.read_response();
            return Err(err);
        }

        let (end_code, end_msg) = self.read_response()?;
        if end_code / 100 != 2 {
            let _ = std::fs::remove_file(&part_path);
            return Err(format!("Transfert RETR incomplet ({end_msg})"));
        }

        if local_path.exists() {
            let _ = std::fs::remove_file(local_path);
        }
        std::fs::rename(&part_path, local_path).map_err(|e| {
            let _ = std::fs::remove_file(&part_path);
            format!(
                "Impossible de finaliser le fichier téléchargé '{}' : {e}",
                local_path.display()
            )
        })?;

        Ok(transferred)
    }

    pub fn upload_file<F>(
        &mut self,
        local_path: &Path,
        remote_name: &str,
        cancel: &AtomicBool,
        mut on_progress: F,
    ) -> Result<u64, String>
    where
        F: FnMut(u64, Option<u64>),
    {
        sanitize_ftp_arg(remote_name)?;
        let mut in_file = File::open(local_path).map_err(|e| {
            format!(
                "Impossible d'ouvrir le fichier local '{}' : {e}",
                local_path.display()
            )
        })?;
        let total = in_file.metadata().ok().map(|m| m.len());

        let mut data_stream = self.open_data_stream()?;
        let (code, msg) = self.send_raw_command(&format!("STOR {remote_name}"))?;
        if code / 100 != 1 {
            return Err(format!("Échec téléversement STOR '{remote_name}' ({msg})"));
        }

        let mut buf = [0u8; 32 * 1024];
        let mut transferred: u64 = 0;
        let mut write_err: Option<String> = None;

        loop {
            if cancel.load(Ordering::Relaxed) {
                write_err = Some("Téléversement annulé par l'utilisateur.".to_string());
                break;
            }
            match in_file.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    if let Err(e) = data_stream.write_all(&buf[..n]) {
                        write_err = Some(format!("Erreur envoi données FTP : {e}"));
                        break;
                    }
                    transferred += n as u64;
                    on_progress(transferred, total);
                }
                Err(e) => {
                    write_err = Some(format!("Erreur lecture fichier local : {e}"));
                    break;
                }
            }
        }

        let _ = data_stream.flush();
        drop(data_stream);

        if let Some(err) = write_err {
            let _ = self.read_response();
            return Err(err);
        }

        let (end_code, end_msg) = self.read_response()?;
        if end_code / 100 != 2 {
            return Err(format!("Transfert STOR incomplet ({end_msg})"));
        }

        Ok(transferred)
    }

    pub fn download_bytes(
        &mut self,
        remote_name: &str,
        max_bytes: usize,
        cancel: &AtomicBool,
    ) -> Result<Vec<u8>, String> {
        sanitize_ftp_arg(remote_name)?;
        if let Some(size) = self.size(remote_name)
            && size > max_bytes as u64
        {
            return Err(format!(
                "Le fichier « {remote_name} » dépasse la limite d'édition ({} Mo max).",
                max_bytes / (1024 * 1024)
            ));
        }

        let mut data_stream = self.open_data_stream()?;
        let (code, msg) = self.send_raw_command(&format!("RETR {remote_name}"))?;
        if code / 100 != 1 {
            return Err(format!("Échec lecture RETR '{remote_name}' ({msg})"));
        }

        let mut out = Vec::new();
        let mut buf = [0u8; 32 * 1024];
        let mut read_err: Option<String> = None;

        loop {
            if cancel.load(Ordering::Relaxed) {
                read_err = Some("Lecture du fichier annulée par l'utilisateur.".to_string());
                break;
            }
            match data_stream.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    if out.len().saturating_add(n) > max_bytes {
                        read_err = Some(format!(
                            "Le fichier « {remote_name} » dépasse la limite d'édition ({} Mo max).",
                            max_bytes / (1024 * 1024)
                        ));
                        break;
                    }
                    out.extend_from_slice(&buf[..n]);
                }
                Err(e) => {
                    read_err = Some(format!("Erreur réception données FTP : {e}"));
                    break;
                }
            }
        }

        drop(data_stream);

        if let Some(err) = read_err {
            let _ = self.read_response();
            return Err(err);
        }

        let (end_code, end_msg) = self.read_response()?;
        if end_code / 100 != 2 {
            return Err(format!("Transfert RETR incomplet ({end_msg})"));
        }

        Ok(out)
    }

    pub fn upload_bytes(
        &mut self,
        data: &[u8],
        remote_name: &str,
        cancel: &AtomicBool,
    ) -> Result<u64, String> {
        sanitize_ftp_arg(remote_name)?;
        let mut data_stream = self.open_data_stream()?;
        let (code, msg) = self.send_raw_command(&format!("STOR {remote_name}"))?;
        if code / 100 != 1 {
            return Err(format!("Échec téléversement STOR '{remote_name}' ({msg})"));
        }

        let mut offset = 0usize;
        let mut write_err: Option<String> = None;
        while offset < data.len() {
            if cancel.load(Ordering::Relaxed) {
                write_err = Some("Téléversement annulé par l'utilisateur.".to_string());
                break;
            }
            let end = (offset + 32 * 1024).min(data.len());
            if let Err(e) = data_stream.write_all(&data[offset..end]) {
                write_err = Some(format!("Erreur envoi données FTP : {e}"));
                break;
            }
            offset = end;
        }

        let _ = data_stream.flush();
        drop(data_stream);

        if let Some(err) = write_err {
            let _ = self.read_response();
            return Err(err);
        }

        let (end_code, end_msg) = self.read_response()?;
        if end_code / 100 != 2 {
            return Err(format!("Transfert STOR incomplet ({end_msg})"));
        }

        Ok(data.len() as u64)
    }

    pub fn mkdir(&mut self, name: &str) -> Result<(), String> {
        sanitize_ftp_arg(name)?;
        let (code, msg) = self.send_raw_command(&format!("MKD {name}"))?;
        if code / 100 != 2 {
            return Err(format!("Impossible de créer le dossier '{name}' ({msg})"));
        }
        Ok(())
    }

    pub fn delete_file(&mut self, name: &str) -> Result<(), String> {
        sanitize_ftp_arg(name)?;
        let (code, msg) = self.send_raw_command(&format!("DELE {name}"))?;
        if code / 100 != 2 {
            return Err(format!(
                "Impossible de supprimer le fichier '{name}' ({msg})"
            ));
        }
        Ok(())
    }

    pub fn delete_dir(&mut self, name: &str) -> Result<(), String> {
        sanitize_ftp_arg(name)?;
        let (code, msg) = self.send_raw_command(&format!("RMD {name}"))?;
        if code / 100 != 2 {
            return Err(format!(
                "Impossible de supprimer le dossier '{name}' ({msg}). Vérifiez qu'il est vide."
            ));
        }
        Ok(())
    }

    pub fn rename(&mut self, old_name: &str, new_name: &str) -> Result<(), String> {
        sanitize_ftp_arg(old_name)?;
        sanitize_ftp_arg(new_name)?;
        let (c1, m1) = self.send_raw_command(&format!("RNFR {old_name}"))?;
        if c1 / 100 != 3 {
            return Err(format!("Impossible de renommer '{old_name}' ({m1})"));
        }
        let (c2, m2) = self.send_raw_command(&format!("RNTO {new_name}"))?;
        if c2 / 100 != 2 {
            return Err(format!("Échec renommage vers '{new_name}' ({m2})"));
        }
        Ok(())
    }

    pub fn quit(&mut self) {
        let _ = self.send_raw_command("QUIT");
    }
}

// =============================================================================
// INTERFACE GRAPHIQUE NATIVE WIN32 FLUENT POUR L'ONGLET FTP
// =============================================================================

#[derive(Debug, Clone)]
pub struct FtpLogLine {
    pub text: String,
    pub is_error: bool,
    pub is_success: bool,
}

#[derive(Debug, Clone)]
pub struct FtpTransferState {
    pub label: String,
    pub transferred: u64,
    pub total: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct FtpSharedState {
    pub is_connected: bool,
    pub is_connecting: bool,
    pub is_busy: bool,
    pub auth_failed: bool,
    pub current_dir: String,
    pub host_label: String,
    pub username: String,
    pub entries: Vec<FtpEntry>,
    pub transfer: Option<FtpTransferState>,
    pub logs: Vec<FtpLogLine>,
    pub status_detail: String,
    pub error_banner: Option<String>,
    pub path_dirty: bool,
    pub pending_edit_file: Option<(String, String, Vec<u8>)>,
    pending_external_launch: Option<PendingExternalLaunch>,
    pending_external_changes: VecDeque<PendingExternalChange>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ExternalEditorApp {
    display_name: String,
    exe_path: PathBuf,
}

#[derive(Debug, Clone)]
enum ExternalEditTarget {
    App(ExternalEditorApp),
    SystemChooser,
}

#[derive(Debug, Clone)]
struct PendingExternalLaunch {
    remote_dir: String,
    remote_name: String,
    local_path: PathBuf,
    initial_bytes: Vec<u8>,
    target: ExternalEditTarget,
}

#[derive(Debug, Clone)]
struct PendingExternalChange {
    remote_dir: String,
    remote_name: String,
    new_bytes: Vec<u8>,
}

enum FtpWorkerCommand {
    Connect,
    ReconnectWithCredentials {
        username: String,
        password: SecretString,
    },
    Refresh,
    Cwd(String),
    Cdup,
    Download {
        remote_name: String,
        local_path: PathBuf,
        size_hint: Option<u64>,
    },
    Upload {
        local_path: PathBuf,
        remote_name: String,
    },
    Mkdir(String),
    Rename {
        old_name: String,
        new_name: String,
    },
    Delete {
        name: String,
        is_dir: bool,
    },
    FetchForEdit {
        remote_name: String,
        size_hint: Option<u64>,
    },
    FetchForExternalEdit {
        remote_name: String,
        size_hint: Option<u64>,
        target: ExternalEditTarget,
    },
    SaveEditedFile {
        remote_dir: String,
        remote_name: String,
        content: Vec<u8>,
    },
    Quit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FtpPane {
    Local,
    Remote,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FtpUiZone {
    BtnCdup,
    BtnRefresh,
    BtnUpload,
    BtnDownload,
    BtnMkdir,
    BtnRename,
    BtnDelete,
    BtnCredentials,
    BtnReconnect,
    BtnLocalGoPath,
    BtnGoPath,
    BtnCancelTransfer,
    BtnErrorCredentials,
    BtnErrorReconnect,
    PaneSplitter,
    ActivitySplitter,
    LocalRow(usize),
    Row(usize),
}

#[derive(Debug, Clone)]
struct FtpHitZone {
    rect: RECT,
    zone: FtpUiZone,
}

const IDC_FTP_PATH_EDIT: usize = 5101;
const IDC_FTP_LOCAL_PATH_EDIT: usize = 5102;
const CMD_FTP_CTX_OPEN: u32 = 5201;
const CMD_FTP_CTX_DOWNLOAD: u32 = 5202;
const CMD_FTP_CTX_DOWNLOAD_AS: u32 = 5203;
const CMD_FTP_CTX_UPLOAD: u32 = 5204;
const CMD_FTP_CTX_MKDIR: u32 = 5205;
const CMD_FTP_CTX_RENAME: u32 = 5206;
const CMD_FTP_CTX_DELETE: u32 = 5207;
const CMD_FTP_CTX_REFRESH: u32 = 5208;
const CMD_FTP_CTX_EDIT: u32 = 5209;
const CMD_FTP_CTX_EDIT_WITH_OTHER: u32 = 5210;
const CMD_FTP_CTX_EDIT_WITH_BASE: u32 = 5220;
const MAX_EXTERNAL_EDITORS_MENU: usize = 20;

struct FtpWndState {
    session_id: u32,
    container_hwnd: HWND,
    main_hwnd: HWND,
    palette: GdiPalette,
    shared: Arc<Mutex<FtpSharedState>>,
    cmd_tx: mpsc::Sender<FtpWorkerCommand>,
    cancel_transfer: Arc<AtomicBool>,
    ed_local_path: HWND,
    ed_path: HWND,
    local_path_box_rc: RECT,
    path_box_rc: RECT,
    local_list_area_rc: RECT,
    list_area_rc: RECT,
    pane_splitter_rc: RECT,
    activity_splitter_rc: RECT,
    split_ratio_permille: i32,
    activity_height: i32,
    dragging_pane_splitter: bool,
    dragging_activity_splitter: bool,
    active_pane: FtpPane,
    local_dir: PathBuf,
    local_showing_drives: bool,
    local_entries: Vec<FtpEntry>,
    local_error: Option<String>,
    local_path_dirty: bool,
    local_selected: BTreeSet<usize>,
    local_anchor_idx: Option<usize>,
    local_scroll_offset: i32,
    row_height: i32,
    remote_selected: BTreeSet<usize>,
    remote_anchor_idx: Option<usize>,
    scroll_offset: i32,
    drag_candidate: Option<(FtpPane, usize, POINT, bool)>,
    dragging_transfer: Option<FtpPane>,
    drag_hover_target: Option<FtpPane>,
    last_transfer_active: bool,
    hovered_zone: Option<FtpUiZone>,
    hit_zones: Vec<FtpHitZone>,
    external_watch_stop: Arc<AtomicBool>,
    external_temp_dirs: Vec<PathBuf>,
}

const PROP_FTP_OLD_EDIT_PROC: PCWSTR = w!("MalvisionFtpPathOldProc");
const PROP_FTP_HOST_HWND: PCWSTR = w!("MalvisionFtpPathHostHwnd");

unsafe extern "system" fn ftp_path_edit_subclass_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        if msg == WM_KEYDOWN && wparam.0 == VK_RETURN.0 as usize {
            let host_ptr =
                windows::Win32::UI::WindowsAndMessaging::GetPropW(hwnd, PROP_FTP_HOST_HWND);
            if !host_ptr.0.is_null() {
                let host_hwnd = HWND(host_ptr.0);
                let ctrl_id = GetDlgCtrlID(hwnd).max(0) as usize;
                let _ = PostMessageW(
                    Some(host_hwnd),
                    WM_COMMAND,
                    WPARAM(ctrl_id | (0x1000 << 16)),
                    LPARAM(hwnd.0 as isize),
                );
            }
            return LRESULT(0);
        }

        let old_raw =
            windows::Win32::UI::WindowsAndMessaging::GetPropW(hwnd, PROP_FTP_OLD_EDIT_PROC).0
                as usize;
        if old_raw != 0 {
            let old_proc: WNDPROC = Some(std::mem::transmute::<
                usize,
                unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT,
            >(old_raw));
            CallWindowProcW(old_proc, hwnd, msg, wparam, lparam)
        } else {
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
    }
}

fn current_time_hms() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let tod = secs % 86400;
    let h = tod / 3600;
    let m = (tod % 3600) / 60;
    let s = tod % 60;
    format!("{h:02}:{m:02}:{s:02}")
}

fn format_unix_time_hm(secs: u64) -> String {
    let days = (secs / 86400) as i64;
    let tod = secs % 86400;
    let hh = tod / 3600;
    let mm = (tod % 3600) / 60;
    // Algorithme civil Howard Hinnant (UTC)
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = y + i64::from(m <= 2);
    format!("{year:04}-{m:02}-{d:02} {hh:02}:{mm:02}")
}

fn default_local_dir() -> PathBuf {
    for key in ["USERPROFILE", "HOME"] {
        if let Some(v) = std::env::var_os(key) {
            let p = PathBuf::from(v);
            if p.is_dir() {
                return p;
            }
        }
    }
    let fallback = PathBuf::from(r"C:\");
    if fallback.is_dir() {
        fallback
    } else {
        std::env::current_dir().unwrap_or(fallback)
    }
}

fn list_windows_drives() -> Vec<FtpEntry> {
    let mask = unsafe { GetLogicalDrives() };
    let mut out = Vec::new();
    for idx in 0..26u32 {
        if (mask & (1u32 << idx)) != 0 {
            let letter = (b'A' + idx as u8) as char;
            out.push(FtpEntry {
                name: format!("{letter}:\\"),
                kind: FtpEntryKind::Directory,
                size: None,
                modified: "Lecteur local".to_string(),
                permissions: String::new(),
            });
        }
    }
    out
}

fn read_local_directory(dir: &Path) -> Result<Vec<FtpEntry>, String> {
    let rd = std::fs::read_dir(dir)
        .map_err(|e| format!("Impossible d'ouvrir « {} » : {e}", dir.display()))?;
    let mut dirs = Vec::new();
    let mut files = Vec::new();

    for item in rd.flatten() {
        let name = item.file_name().to_string_lossy().to_string();
        if name.is_empty() {
            continue;
        }
        let Ok(meta) = item.metadata() else {
            continue;
        };
        let is_dir = meta.is_dir();
        let modified = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| format_unix_time_hm(d.as_secs()))
            .unwrap_or_default();
        let entry = FtpEntry {
            name,
            kind: if is_dir {
                FtpEntryKind::Directory
            } else {
                FtpEntryKind::File
            },
            size: if is_dir { None } else { Some(meta.len()) },
            modified,
            permissions: String::new(),
        };
        if is_dir {
            dirs.push(entry);
        } else {
            files.push(entry);
        }
    }

    dirs.sort_by_key(|a| a.name.to_lowercase());
    files.sort_by_key(|a| a.name.to_lowercase());

    let mut entries = Vec::with_capacity(1 + dirs.len() + files.len());
    entries.push(FtpEntry {
        name: "..".to_string(),
        kind: FtpEntryKind::ParentDir,
        size: None,
        modified: String::new(),
        permissions: String::new(),
    });
    entries.extend(dirs);
    entries.extend(files);
    Ok(entries)
}

fn clamp_split_x(total_w: i32, ratio_permille: i32, min_pane_w: i32) -> i32 {
    let min_w = min_pane_w.min((total_w / 3).max(60));
    let max_w = (total_w - min_w).max(min_w);
    ((total_w * ratio_permille) / 1000).clamp(min_w, max_w)
}

fn clamp_activity_height(total_h: i32, desired_h: i32, min_h: i32, fixed_top_h: i32) -> i32 {
    let max_h = (total_h - fixed_top_h).max(min_h);
    desired_h.clamp(min_h, max_h)
}

fn refresh_local_entries(state: &mut FtpWndState) {
    if state.local_showing_drives {
        state.local_entries = list_windows_drives();
        state.local_error = None;
    } else {
        match read_local_directory(&state.local_dir) {
            Ok(list) => {
                state.local_entries = list;
                state.local_error = None;
            }
            Err(err) => {
                state.local_entries = vec![FtpEntry {
                    name: "..".to_string(),
                    kind: FtpEntryKind::ParentDir,
                    size: None,
                    modified: String::new(),
                    permissions: String::new(),
                }];
                state.local_error = Some(err);
            }
        }
    }
    let len = state.local_entries.len();
    state.local_selected.retain(|&i| i < len);
    if state.local_anchor_idx.is_some_and(|i| i >= len) {
        state.local_anchor_idx = None;
    }
    state.local_path_dirty = true;
}

fn apply_row_selection(
    selected: &mut BTreeSet<usize>,
    anchor: &mut Option<usize>,
    idx: usize,
    ctrl: bool,
    shift: bool,
    entries: &[FtpEntry],
) {
    if idx >= entries.len() {
        return;
    }
    if shift {
        let start = anchor.unwrap_or(idx).min(entries.len().saturating_sub(1));
        selected.clear();
        let (lo, hi) = if start <= idx {
            (start, idx)
        } else {
            (idx, start)
        };
        for (i, entry) in entries.iter().enumerate().take(hi + 1).skip(lo) {
            if lo == hi || entry.kind != FtpEntryKind::ParentDir {
                selected.insert(i);
            }
        }
        if selected.is_empty() {
            selected.insert(idx);
        }
        if anchor.is_none() {
            *anchor = Some(idx);
        }
    } else if ctrl {
        if selected.contains(&idx) {
            selected.remove(&idx);
        } else {
            selected.insert(idx);
            *anchor = Some(idx);
        }
    } else {
        selected.clear();
        selected.insert(idx);
        *anchor = Some(idx);
    }
}

fn is_mod_key_down(vk: u16) -> bool {
    unsafe { (GetKeyState(i32::from(vk)) as u16 & 0x8000) != 0 }
}

fn local_navigate_to(state: &mut FtpWndState, target: PathBuf) {
    if target.is_dir() {
        state.local_dir = target;
        state.local_showing_drives = false;
        state.local_selected.clear();
        state.local_anchor_idx = None;
        state.local_scroll_offset = 0;
        refresh_local_entries(state);
    } else {
        show_error_dialog(
            state.main_hwnd,
            "Dossier local introuvable",
            &format!("Le répertoire « {} » n'existe pas.", target.display()),
        );
    }
}

fn local_navigate_up(state: &mut FtpWndState) {
    if state.local_showing_drives {
        return;
    }
    if let Some(parent) = state.local_dir.parent() {
        state.local_dir = parent.to_path_buf();
    } else {
        state.local_showing_drives = true;
    }
    state.local_selected.clear();
    state.local_anchor_idx = None;
    state.local_scroll_offset = 0;
    refresh_local_entries(state);
}

fn push_ftp_log(
    shared: &Arc<Mutex<FtpSharedState>>,
    text: String,
    is_error: bool,
    is_success: bool,
) {
    if let Ok(mut st) = shared.lock() {
        let ts = current_time_hms();
        st.logs.push(FtpLogLine {
            text: format!("[{ts}] {text}"),
            is_error,
            is_success,
        });
        if st.logs.len() > 80 {
            let remove_count = st.logs.len() - 80;
            st.logs.drain(0..remove_count);
        }
    }
}

fn post_notify(
    container_hwnd_raw: usize,
    main_hwnd_raw: usize,
    session_id: u32,
    event_code: usize,
) {
    let container_hwnd = HWND(container_hwnd_raw as *mut c_void);
    let main_hwnd = HWND(main_hwnd_raw as *mut c_void);
    unsafe {
        if !container_hwnd.is_invalid() {
            let _ = PostMessageW(
                Some(container_hwnd),
                WM_FTP_INTERNAL_REFRESH,
                WPARAM(0),
                LPARAM(0),
            );
        }
        if !main_hwnd.is_invalid() {
            let _ = PostMessageW(
                Some(main_hwnd),
                WM_FTP_EVENT,
                pack_ftp_event_wparam(session_id, event_code),
                LPARAM(0),
            );
        }
    }
}

fn is_connection_lost_error(err: &str) -> bool {
    let lower = err.to_ascii_lowercase();
    lower.contains("421")
        || lower.contains("connexion ftp fermée")
        || lower.contains("erreur d'envoi commande ftp")
        || lower.contains("erreur flush socket ftp")
        || lower.contains("erreur de lecture ftp")
        || lower.contains("broken pipe")
        || lower.contains("connection reset")
        || lower.contains("connection aborted")
        || lower.contains("10053")
        || lower.contains("10054")
        || lower.contains("timed out")
        || lower.contains("unexpected end of file")
}

#[allow(clippy::too_many_arguments)]
fn spawn_ftp_worker(
    session_id: u32,
    container_hwnd_raw: usize,
    main_hwnd_raw: usize,
    host: String,
    port: u16,
    initial_user: String,
    initial_pass: SecretString,
    initial_remote_dir: String,
    shared: Arc<Mutex<FtpSharedState>>,
    cancel_transfer: Arc<AtomicBool>,
    cmd_rx: mpsc::Receiver<FtpWorkerCommand>,
) {
    std::thread::spawn(move || {
        let mut active_user = if initial_user.trim().is_empty() {
            "anonymous".to_string()
        } else {
            initial_user.trim().to_string()
        };
        let mut active_pass = initial_pass;
        let mut client: Option<FtpClient> = None;
        let mut last_remote_dir: Option<String> = if initial_remote_dir.trim().is_empty() {
            None
        } else {
            Some(initial_remote_dir.trim().to_string())
        };

        // FtpClient::list() insère déjà une unique entrée ".." (ParentDir) si cur_dir != "/".
        let refresh_dir = |c: &mut FtpClient,
                           cur_dir: &mut String,
                           last_dir: &mut Option<String>|
         -> Result<Vec<FtpEntry>, String> {
            if cur_dir.trim().is_empty() {
                *cur_dir = c.pwd().unwrap_or_else(|_| "/".to_string());
            }
            let entries = c.list(cur_dir)?;
            *last_dir = Some(cur_dir.clone());
            Ok(entries)
        };

        let connect_and_resume = |client_slot: &mut Option<FtpClient>,
                                  last_dir: &mut Option<String>,
                                  user: &str,
                                  pass: &SecretString,
                                  silent_keepalive: bool|
         -> bool {
            if let Some(mut old) = client_slot.take() {
                old.quit();
            }
            if !silent_keepalive {
                if let Ok(mut st) = shared.lock() {
                    st.is_connecting = true;
                    st.is_connected = false;
                    st.is_busy = true;
                    st.auth_failed = false;
                    st.error_banner = None;
                    st.status_detail = format!("Connexion FTP vers {host}:{port}...");
                }
                post_notify(
                    container_hwnd_raw,
                    main_hwnd_raw,
                    session_id,
                    FTP_EVT_CONNECTING,
                );
            }

            let log_shared = Arc::clone(&shared);
            match FtpClient::connect(&host, port, user, pass, |is_err, line| {
                if !silent_keepalive || is_err {
                    push_ftp_log(&log_shared, line.to_string(), is_err, false);
                }
            }) {
                Ok(mut c) => {
                    let mut target_dir = if let Some(ref resume_dir) = *last_dir {
                        match c.cwd(resume_dir) {
                            Ok(d) => d,
                            Err(err) => {
                                push_ftp_log(
                                    &shared,
                                    format!(
                                        "Dossier précédent « {resume_dir} » indisponible : {err}"
                                    ),
                                    true,
                                    false,
                                );
                                c.pwd().unwrap_or_else(|_| "/".to_string())
                            }
                        }
                    } else {
                        c.pwd().unwrap_or_else(|_| "/".to_string())
                    };

                    let entries = match refresh_dir(&mut c, &mut target_dir, last_dir) {
                        Ok(e) => e,
                        Err(err) => {
                            push_ftp_log(&shared, err, true, false);
                            Vec::new()
                        }
                    };
                    *client_slot = Some(c);

                    if let Ok(mut st) = shared.lock() {
                        st.is_connecting = false;
                        st.is_connected = true;
                        st.is_busy = false;
                        st.auth_failed = false;
                        st.error_banner = None;
                        st.current_dir = target_dir.clone();
                        st.path_dirty = true;
                        st.entries = entries;
                        st.status_detail =
                            format!("Connecté FTP ({user}@{host}:{port}) — {target_dir}");
                    }
                    push_ftp_log(
                        &shared,
                        if silent_keepalive {
                            format!("Session FTP reconnectée automatiquement dans {target_dir}")
                        } else {
                            format!("Session FTP ouverte dans {target_dir}")
                        },
                        false,
                        true,
                    );
                    post_notify(
                        container_hwnd_raw,
                        main_hwnd_raw,
                        session_id,
                        FTP_EVT_CONNECTED,
                    );
                    true
                }
                Err((auth_err, clean_err)) => {
                    if let Ok(mut st) = shared.lock() {
                        st.is_connecting = false;
                        st.is_connected = false;
                        st.is_busy = false;
                        st.auth_failed = auth_err;
                        st.error_banner = Some(clean_err.clone());
                        st.status_detail = format!("Erreur FTP : {clean_err}");
                    }
                    push_ftp_log(&shared, clean_err, true, false);
                    post_notify(
                        container_hwnd_raw,
                        main_hwnd_raw,
                        session_id,
                        if auth_err {
                            FTP_EVT_AUTH_FAILED
                        } else {
                            FTP_EVT_ERROR
                        },
                    );
                    false
                }
            }
        };

        loop {
            match cmd_rx.recv_timeout(Duration::from_secs(20)) {
                Ok(cmd) => match cmd {
                    FtpWorkerCommand::Connect => {
                        connect_and_resume(
                            &mut client,
                            &mut last_remote_dir,
                            &active_user,
                            &active_pass,
                            false,
                        );
                    }
                    FtpWorkerCommand::ReconnectWithCredentials { username, password } => {
                        active_user = if username.trim().is_empty() {
                            "anonymous".to_string()
                        } else {
                            username.trim().to_string()
                        };
                        active_pass = password;
                        if let Ok(mut st) = shared.lock() {
                            st.username = active_user.clone();
                        }
                        connect_and_resume(
                            &mut client,
                            &mut last_remote_dir,
                            &active_user,
                            &active_pass,
                            false,
                        );
                    }
                    FtpWorkerCommand::Refresh => {
                        if client.is_none()
                            && !connect_and_resume(
                                &mut client,
                                &mut last_remote_dir,
                                &active_user,
                                &active_pass,
                                true,
                            )
                        {
                            continue;
                        }
                        if let Some(ref mut c) = client {
                            let mut cur = last_remote_dir.clone().unwrap_or_else(|| {
                                shared
                                    .lock()
                                    .map(|s| s.current_dir.clone())
                                    .unwrap_or_else(|_| "/".to_string())
                            });
                            let res = refresh_dir(c, &mut cur, &mut last_remote_dir);
                            match res {
                                Ok(entries) => {
                                    if let Ok(mut st) = shared.lock() {
                                        st.current_dir = cur.clone();
                                        st.entries = entries;
                                        st.status_detail = format!(
                                            "Connecté FTP ({active_user}@{host}:{port}) — {cur}"
                                        );
                                    }
                                }
                                Err(err) if is_connection_lost_error(&err) => {
                                    connect_and_resume(
                                        &mut client,
                                        &mut last_remote_dir,
                                        &active_user,
                                        &active_pass,
                                        true,
                                    );
                                }
                                Err(err) => push_ftp_log(&shared, err, true, false),
                            }
                            post_notify(
                                container_hwnd_raw,
                                main_hwnd_raw,
                                session_id,
                                FTP_EVT_STATUS_TEXT,
                            );
                        }
                    }
                    FtpWorkerCommand::Cwd(target) => {
                        if client.is_none()
                            && !connect_and_resume(
                                &mut client,
                                &mut last_remote_dir,
                                &active_user,
                                &active_pass,
                                true,
                            )
                        {
                            continue;
                        }
                        let mut needs_reconnect = false;
                        if let Some(ref mut c) = client {
                            match c.cwd(&target) {
                                Ok(mut new_dir) => {
                                    match refresh_dir(c, &mut new_dir, &mut last_remote_dir) {
                                        Ok(entries) => {
                                            if let Ok(mut st) = shared.lock() {
                                                st.current_dir = new_dir.clone();
                                                st.path_dirty = true;
                                                st.entries = entries;
                                                st.status_detail = format!(
                                                    "Connecté FTP ({active_user}@{host}:{port}) — {new_dir}"
                                                );
                                            }
                                        }
                                        Err(err) if is_connection_lost_error(&err) => {
                                            needs_reconnect = true;
                                        }
                                        Err(err) => push_ftp_log(&shared, err, true, false),
                                    }
                                }
                                Err(err) if is_connection_lost_error(&err) => {
                                    needs_reconnect = true;
                                }
                                Err(err) => push_ftp_log(&shared, err, true, false),
                            }
                        }
                        if needs_reconnect
                            && connect_and_resume(
                                &mut client,
                                &mut last_remote_dir,
                                &active_user,
                                &active_pass,
                                true,
                            )
                            && let Some(ref mut c) = client
                            && let Ok(mut new_dir) = c.cwd(&target)
                            && let Ok(entries) = refresh_dir(c, &mut new_dir, &mut last_remote_dir)
                            && let Ok(mut st) = shared.lock()
                        {
                            st.current_dir = new_dir.clone();
                            st.path_dirty = true;
                            st.entries = entries;
                            st.status_detail =
                                format!("Connecté FTP ({active_user}@{host}:{port}) — {new_dir}");
                        }
                        post_notify(
                            container_hwnd_raw,
                            main_hwnd_raw,
                            session_id,
                            FTP_EVT_STATUS_TEXT,
                        );
                    }
                    FtpWorkerCommand::Cdup => {
                        if client.is_none()
                            && !connect_and_resume(
                                &mut client,
                                &mut last_remote_dir,
                                &active_user,
                                &active_pass,
                                true,
                            )
                        {
                            continue;
                        }
                        let mut needs_reconnect = false;
                        if let Some(ref mut c) = client {
                            match c.cdup() {
                                Ok(mut new_dir) => {
                                    match refresh_dir(c, &mut new_dir, &mut last_remote_dir) {
                                        Ok(entries) => {
                                            if let Ok(mut st) = shared.lock() {
                                                st.current_dir = new_dir.clone();
                                                st.path_dirty = true;
                                                st.entries = entries;
                                                st.status_detail = format!(
                                                    "Connecté FTP ({active_user}@{host}:{port}) — {new_dir}"
                                                );
                                            }
                                        }
                                        Err(err) if is_connection_lost_error(&err) => {
                                            needs_reconnect = true;
                                        }
                                        Err(err) => push_ftp_log(&shared, err, true, false),
                                    }
                                }
                                Err(err) if is_connection_lost_error(&err) => {
                                    needs_reconnect = true;
                                }
                                Err(err) => push_ftp_log(&shared, err, true, false),
                            }
                        }
                        if needs_reconnect
                            && connect_and_resume(
                                &mut client,
                                &mut last_remote_dir,
                                &active_user,
                                &active_pass,
                                true,
                            )
                            && let Some(ref mut c) = client
                            && let Ok(mut new_dir) = c.cdup()
                            && let Ok(entries) = refresh_dir(c, &mut new_dir, &mut last_remote_dir)
                            && let Ok(mut st) = shared.lock()
                        {
                            st.current_dir = new_dir.clone();
                            st.path_dirty = true;
                            st.entries = entries;
                            st.status_detail =
                                format!("Connecté FTP ({active_user}@{host}:{port}) — {new_dir}");
                        }
                        post_notify(
                            container_hwnd_raw,
                            main_hwnd_raw,
                            session_id,
                            FTP_EVT_STATUS_TEXT,
                        );
                    }
                    FtpWorkerCommand::Download {
                        remote_name,
                        local_path,
                        size_hint,
                    } => {
                        if client.is_none()
                            && !connect_and_resume(
                                &mut client,
                                &mut last_remote_dir,
                                &active_user,
                                &active_pass,
                                true,
                            )
                        {
                            continue;
                        }
                        if let Some(ref mut c) = client {
                            cancel_transfer.store(false, Ordering::Relaxed);
                            let known_total = size_hint.or_else(|| c.size(&remote_name));
                            if let Ok(mut st) = shared.lock() {
                                st.transfer = Some(FtpTransferState {
                                    label: format!("Téléchargement : {remote_name}"),
                                    transferred: 0,
                                    total: known_total,
                                });
                                st.status_detail = format!("Téléchargement de {remote_name}...");
                            }
                            post_notify(
                                container_hwnd_raw,
                                main_hwnd_raw,
                                session_id,
                                FTP_EVT_STATUS_TEXT,
                            );

                            let mut last_ui_tick = std::time::Instant::now();
                            let res = c.download_file(
                                &remote_name,
                                &local_path,
                                known_total,
                                &cancel_transfer,
                                |done, tot| {
                                    if let Ok(mut st) = shared.lock() {
                                        st.transfer = Some(FtpTransferState {
                                            label: format!("Téléchargement : {remote_name}"),
                                            transferred: done,
                                            total: tot,
                                        });
                                    }
                                    if last_ui_tick.elapsed() >= Duration::from_millis(90) {
                                        last_ui_tick = std::time::Instant::now();
                                        post_notify(
                                            container_hwnd_raw,
                                            main_hwnd_raw,
                                            session_id,
                                            FTP_EVT_STATUS_TEXT,
                                        );
                                    }
                                },
                            );

                            if let Ok(mut st) = shared.lock() {
                                st.transfer = None;
                                st.status_detail = match &res {
                                    Ok(n) => format!(
                                        "Téléchargement terminé : {remote_name} ({})",
                                        format_file_size(*n)
                                    ),
                                    Err(e) => e.clone(),
                                };
                            }
                            match res {
                                Ok(n) => push_ftp_log(
                                    &shared,
                                    format!(
                                        "Téléchargé '{remote_name}' ({}) -> {}",
                                        format_file_size(n),
                                        local_path.display()
                                    ),
                                    false,
                                    true,
                                ),
                                Err(err) => {
                                    let lost = is_connection_lost_error(&err);
                                    push_ftp_log(&shared, err, true, false);
                                    if lost {
                                        connect_and_resume(
                                            &mut client,
                                            &mut last_remote_dir,
                                            &active_user,
                                            &active_pass,
                                            true,
                                        );
                                    }
                                }
                            }
                            post_notify(
                                container_hwnd_raw,
                                main_hwnd_raw,
                                session_id,
                                FTP_EVT_STATUS_TEXT,
                            );
                        }
                    }
                    FtpWorkerCommand::Upload {
                        local_path,
                        remote_name,
                    } => {
                        if client.is_none()
                            && !connect_and_resume(
                                &mut client,
                                &mut last_remote_dir,
                                &active_user,
                                &active_pass,
                                true,
                            )
                        {
                            continue;
                        }
                        if let Some(ref mut c) = client {
                            cancel_transfer.store(false, Ordering::Relaxed);
                            let total = std::fs::metadata(&local_path).ok().map(|m| m.len());
                            if let Ok(mut st) = shared.lock() {
                                st.transfer = Some(FtpTransferState {
                                    label: format!("Téléversement : {remote_name}"),
                                    transferred: 0,
                                    total,
                                });
                                st.status_detail = format!("Téléversement de {remote_name}...");
                            }
                            post_notify(
                                container_hwnd_raw,
                                main_hwnd_raw,
                                session_id,
                                FTP_EVT_STATUS_TEXT,
                            );

                            let mut last_ui_tick = std::time::Instant::now();
                            let res = c.upload_file(
                                &local_path,
                                &remote_name,
                                &cancel_transfer,
                                |done, tot| {
                                    if let Ok(mut st) = shared.lock() {
                                        st.transfer = Some(FtpTransferState {
                                            label: format!("Téléversement : {remote_name}"),
                                            transferred: done,
                                            total: tot,
                                        });
                                    }
                                    if last_ui_tick.elapsed() >= Duration::from_millis(90) {
                                        last_ui_tick = std::time::Instant::now();
                                        post_notify(
                                            container_hwnd_raw,
                                            main_hwnd_raw,
                                            session_id,
                                            FTP_EVT_STATUS_TEXT,
                                        );
                                    }
                                },
                            );

                            let mut cur = last_remote_dir.clone().unwrap_or_else(|| {
                                shared
                                    .lock()
                                    .map(|s| s.current_dir.clone())
                                    .unwrap_or_else(|_| "/".to_string())
                            });
                            let refreshed = refresh_dir(c, &mut cur, &mut last_remote_dir).ok();

                            if let Ok(mut st) = shared.lock() {
                                st.transfer = None;
                                if let Some(entries) = refreshed {
                                    st.entries = entries;
                                }
                                st.status_detail = match &res {
                                    Ok(n) => format!(
                                        "Téléversement terminé : {remote_name} ({})",
                                        format_file_size(*n)
                                    ),
                                    Err(e) => e.clone(),
                                };
                            }
                            match res {
                                Ok(n) => push_ftp_log(
                                    &shared,
                                    format!("Téléversé '{remote_name}' ({})", format_file_size(n)),
                                    false,
                                    true,
                                ),
                                Err(err) => {
                                    let lost = is_connection_lost_error(&err);
                                    push_ftp_log(&shared, err, true, false);
                                    if lost {
                                        connect_and_resume(
                                            &mut client,
                                            &mut last_remote_dir,
                                            &active_user,
                                            &active_pass,
                                            true,
                                        );
                                    }
                                }
                            }
                            post_notify(
                                container_hwnd_raw,
                                main_hwnd_raw,
                                session_id,
                                FTP_EVT_STATUS_TEXT,
                            );
                        }
                    }
                    FtpWorkerCommand::Mkdir(dir_name) => {
                        if client.is_none()
                            && !connect_and_resume(
                                &mut client,
                                &mut last_remote_dir,
                                &active_user,
                                &active_pass,
                                true,
                            )
                        {
                            continue;
                        }
                        if let Some(ref mut c) = client {
                            match c.mkdir(&dir_name) {
                                Ok(()) => {
                                    push_ftp_log(
                                        &shared,
                                        format!("Dossier créé : {dir_name}"),
                                        false,
                                        true,
                                    );
                                    let mut cur =
                                        last_remote_dir.clone().unwrap_or_else(|| "/".to_string());
                                    if let Ok(entries) =
                                        refresh_dir(c, &mut cur, &mut last_remote_dir)
                                        && let Ok(mut st) = shared.lock()
                                    {
                                        st.entries = entries;
                                    }
                                }
                                Err(err) => push_ftp_log(&shared, err, true, false),
                            }
                            post_notify(
                                container_hwnd_raw,
                                main_hwnd_raw,
                                session_id,
                                FTP_EVT_STATUS_TEXT,
                            );
                        }
                    }
                    FtpWorkerCommand::Rename { old_name, new_name } => {
                        if client.is_none()
                            && !connect_and_resume(
                                &mut client,
                                &mut last_remote_dir,
                                &active_user,
                                &active_pass,
                                true,
                            )
                        {
                            continue;
                        }
                        if let Some(ref mut c) = client {
                            match c.rename(&old_name, &new_name) {
                                Ok(()) => {
                                    push_ftp_log(
                                        &shared,
                                        format!("Renommé : {old_name} -> {new_name}"),
                                        false,
                                        true,
                                    );
                                    let mut cur =
                                        last_remote_dir.clone().unwrap_or_else(|| "/".to_string());
                                    if let Ok(entries) =
                                        refresh_dir(c, &mut cur, &mut last_remote_dir)
                                        && let Ok(mut st) = shared.lock()
                                    {
                                        st.entries = entries;
                                    }
                                }
                                Err(err) => push_ftp_log(&shared, err, true, false),
                            }
                            post_notify(
                                container_hwnd_raw,
                                main_hwnd_raw,
                                session_id,
                                FTP_EVT_STATUS_TEXT,
                            );
                        }
                    }
                    FtpWorkerCommand::Delete { name, is_dir } => {
                        if client.is_none()
                            && !connect_and_resume(
                                &mut client,
                                &mut last_remote_dir,
                                &active_user,
                                &active_pass,
                                true,
                            )
                        {
                            continue;
                        }
                        if let Some(ref mut c) = client {
                            let res = if is_dir {
                                c.delete_dir(&name)
                            } else {
                                c.delete_file(&name)
                            };
                            match res {
                                Ok(()) => {
                                    push_ftp_log(
                                        &shared,
                                        format!("Supprimé : {name}"),
                                        false,
                                        true,
                                    );
                                    let mut cur =
                                        last_remote_dir.clone().unwrap_or_else(|| "/".to_string());
                                    if let Ok(entries) =
                                        refresh_dir(c, &mut cur, &mut last_remote_dir)
                                        && let Ok(mut st) = shared.lock()
                                    {
                                        st.entries = entries;
                                    }
                                }
                                Err(err) => push_ftp_log(&shared, err, true, false),
                            }
                            post_notify(
                                container_hwnd_raw,
                                main_hwnd_raw,
                                session_id,
                                FTP_EVT_STATUS_TEXT,
                            );
                        }
                    }
                    FtpWorkerCommand::FetchForEdit {
                        remote_name,
                        size_hint,
                    } => {
                        if let Some(sz) = size_hint
                            && sz > MAX_EDITABLE_FILE_BYTES as u64
                        {
                            let msg = format!(
                                "Le fichier « {remote_name} » est trop volumineux pour l'éditeur intégré ({} Mo max).",
                                MAX_EDITABLE_FILE_BYTES / (1024 * 1024)
                            );
                            if let Ok(mut st) = shared.lock() {
                                st.error_banner = Some(msg.clone());
                                st.status_detail = msg.clone();
                            }
                            push_ftp_log(&shared, msg, true, false);
                            post_notify(
                                container_hwnd_raw,
                                main_hwnd_raw,
                                session_id,
                                FTP_EVT_STATUS_TEXT,
                            );
                            continue;
                        }
                        if client.is_none()
                            && !connect_and_resume(
                                &mut client,
                                &mut last_remote_dir,
                                &active_user,
                                &active_pass,
                                true,
                            )
                        {
                            continue;
                        }
                        if let Some(ref mut c) = client {
                            cancel_transfer.store(false, Ordering::Relaxed);
                            if let Ok(mut st) = shared.lock() {
                                st.error_banner = None;
                                st.status_detail =
                                    format!("Ouverture de « {remote_name} » dans l'éditeur...");
                            }
                            post_notify(
                                container_hwnd_raw,
                                main_hwnd_raw,
                                session_id,
                                FTP_EVT_STATUS_TEXT,
                            );
                            match c.download_bytes(
                                &remote_name,
                                MAX_EDITABLE_FILE_BYTES,
                                &cancel_transfer,
                            ) {
                                Ok(bytes) => {
                                    let remote_dir =
                                        last_remote_dir.clone().unwrap_or_else(|| "/".to_string());
                                    if let Ok(mut st) = shared.lock() {
                                        st.pending_edit_file =
                                            Some((remote_dir, remote_name.clone(), bytes));
                                        st.status_detail = format!("Édition de « {remote_name} ».");
                                    }
                                    push_ftp_log(
                                        &shared,
                                        format!("Fichier « {remote_name} » chargé pour édition."),
                                        false,
                                        true,
                                    );
                                    unsafe {
                                        let _ = PostMessageW(
                                            Some(HWND(container_hwnd_raw as *mut c_void)),
                                            WM_FTP_OPEN_EDITOR,
                                            WPARAM(0),
                                            LPARAM(0),
                                        );
                                    }
                                }
                                Err(err) => {
                                    if let Ok(mut st) = shared.lock() {
                                        st.error_banner = Some(err.clone());
                                        st.status_detail = err.clone();
                                    }
                                    push_ftp_log(&shared, err, true, false);
                                }
                            }
                            post_notify(
                                container_hwnd_raw,
                                main_hwnd_raw,
                                session_id,
                                FTP_EVT_STATUS_TEXT,
                            );
                        }
                    }
                    FtpWorkerCommand::FetchForExternalEdit {
                        remote_name,
                        size_hint,
                        target,
                    } => {
                        if let Some(sz) = size_hint
                            && sz > MAX_EXTERNAL_EDIT_FILE_BYTES as u64
                        {
                            let msg = format!(
                                "Le fichier « {remote_name} » dépasse la limite d'édition externe ({} Mo max).",
                                MAX_EXTERNAL_EDIT_FILE_BYTES / (1024 * 1024)
                            );
                            if let Ok(mut st) = shared.lock() {
                                st.error_banner = Some(msg.clone());
                                st.status_detail = msg.clone();
                            }
                            push_ftp_log(&shared, msg, true, false);
                            post_notify(
                                container_hwnd_raw,
                                main_hwnd_raw,
                                session_id,
                                FTP_EVT_STATUS_TEXT,
                            );
                            continue;
                        }
                        if client.is_none()
                            && !connect_and_resume(
                                &mut client,
                                &mut last_remote_dir,
                                &active_user,
                                &active_pass,
                                true,
                            )
                        {
                            continue;
                        }
                        if let Some(ref mut c) = client {
                            cancel_transfer.store(false, Ordering::Relaxed);
                            if let Ok(mut st) = shared.lock() {
                                st.error_banner = None;
                                st.status_detail = format!(
                                    "Téléchargement de « {remote_name} » pour l'éditeur externe..."
                                );
                            }
                            post_notify(
                                container_hwnd_raw,
                                main_hwnd_raw,
                                session_id,
                                FTP_EVT_STATUS_TEXT,
                            );
                            match c.download_bytes(
                                &remote_name,
                                MAX_EXTERNAL_EDIT_FILE_BYTES,
                                &cancel_transfer,
                            ) {
                                Ok(bytes) => {
                                    let safe_name: String = remote_name
                                        .chars()
                                        .map(|ch| {
                                            if matches!(
                                                ch,
                                                '/' | '\\'
                                                    | ':'
                                                    | '*'
                                                    | '?'
                                                    | '"'
                                                    | '<'
                                                    | '>'
                                                    | '|'
                                            ) {
                                                '_'
                                            } else {
                                                ch
                                            }
                                        })
                                        .collect();
                                    let unique_ns = SystemTime::now()
                                        .duration_since(UNIX_EPOCH)
                                        .map(|d| d.as_millis())
                                        .unwrap_or(0);
                                    let temp_dir = std::env::temp_dir()
                                        .join("Malvision")
                                        .join("ftp_edit")
                                        .join(format!("s{session_id}_{unique_ns}"));
                                    if let Err(err) = std::fs::create_dir_all(&temp_dir) {
                                        let msg = format!(
                                            "Impossible de préparer le dossier temporaire : {err}"
                                        );
                                        if let Ok(mut st) = shared.lock() {
                                            st.error_banner = Some(msg.clone());
                                            st.status_detail = msg.clone();
                                        }
                                        push_ftp_log(&shared, msg, true, false);
                                    } else {
                                        let local_path = temp_dir.join(if safe_name.is_empty() {
                                            "fichier.txt"
                                        } else {
                                            safe_name.as_str()
                                        });
                                        if let Err(err) = std::fs::write(&local_path, &bytes) {
                                            let msg = format!(
                                                "Impossible d'écrire le fichier temporaire : {err}"
                                            );
                                            if let Ok(mut st) = shared.lock() {
                                                st.error_banner = Some(msg.clone());
                                                st.status_detail = msg.clone();
                                            }
                                            push_ftp_log(&shared, msg, true, false);
                                        } else {
                                            let remote_dir = last_remote_dir
                                                .clone()
                                                .unwrap_or_else(|| "/".to_string());
                                            if let Ok(mut st) = shared.lock() {
                                                st.pending_external_launch =
                                                    Some(PendingExternalLaunch {
                                                        remote_dir,
                                                        remote_name: remote_name.clone(),
                                                        local_path,
                                                        initial_bytes: bytes,
                                                        target,
                                                    });
                                                st.status_detail = format!(
                                                    "Ouverture externe de « {remote_name} »."
                                                );
                                            }
                                            push_ftp_log(
                                                &shared,
                                                format!(
                                                    "Fichier « {remote_name} » prêt pour édition externe."
                                                ),
                                                false,
                                                true,
                                            );
                                            unsafe {
                                                let _ = PostMessageW(
                                                    Some(HWND(container_hwnd_raw as *mut c_void)),
                                                    WM_FTP_OPEN_EXTERNAL_EDITOR,
                                                    WPARAM(0),
                                                    LPARAM(0),
                                                );
                                            }
                                        }
                                    }
                                }
                                Err(err) => {
                                    if let Ok(mut st) = shared.lock() {
                                        st.error_banner = Some(err.clone());
                                        st.status_detail = err.clone();
                                    }
                                    push_ftp_log(&shared, err, true, false);
                                }
                            }
                            post_notify(
                                container_hwnd_raw,
                                main_hwnd_raw,
                                session_id,
                                FTP_EVT_STATUS_TEXT,
                            );
                        }
                    }
                    FtpWorkerCommand::SaveEditedFile {
                        remote_dir,
                        remote_name,
                        content,
                    } => {
                        if client.is_none()
                            && !connect_and_resume(
                                &mut client,
                                &mut last_remote_dir,
                                &active_user,
                                &active_pass,
                                true,
                            )
                        {
                            continue;
                        }
                        if let Some(ref mut c) = client {
                            cancel_transfer.store(false, Ordering::Relaxed);
                            let prev_dir =
                                last_remote_dir.clone().unwrap_or_else(|| "/".to_string());
                            if remote_dir != prev_dir
                                && let Err(err) = c.cwd(&remote_dir)
                            {
                                if let Ok(mut st) = shared.lock() {
                                    st.error_banner = Some(err.clone());
                                    st.status_detail = err.clone();
                                }
                                push_ftp_log(&shared, err, true, false);
                                post_notify(
                                    container_hwnd_raw,
                                    main_hwnd_raw,
                                    session_id,
                                    FTP_EVT_STATUS_TEXT,
                                );
                                continue;
                            }
                            if let Ok(mut st) = shared.lock() {
                                st.error_banner = None;
                                st.status_detail =
                                    format!("Enregistrement de « {remote_name} » sur le FTP...");
                            }
                            post_notify(
                                container_hwnd_raw,
                                main_hwnd_raw,
                                session_id,
                                FTP_EVT_STATUS_TEXT,
                            );
                            match c.upload_bytes(&content, &remote_name, &cancel_transfer) {
                                Ok(bytes_written) => {
                                    if let Ok(mut st) = shared.lock() {
                                        st.status_detail = format!(
                                            "« {remote_name} » enregistré ({bytes_written} octets)."
                                        );
                                    }
                                    push_ftp_log(
                                        &shared,
                                        format!(
                                            "Fichier « {remote_name} » enregistré ({bytes_written} octets)."
                                        ),
                                        false,
                                        true,
                                    );
                                    if remote_dir != prev_dir {
                                        let _ = c.cwd(&prev_dir);
                                    }
                                    let mut cur =
                                        last_remote_dir.clone().unwrap_or_else(|| "/".to_string());
                                    if let Ok(entries) =
                                        refresh_dir(c, &mut cur, &mut last_remote_dir)
                                        && let Ok(mut st) = shared.lock()
                                    {
                                        st.entries = entries;
                                    }
                                }
                                Err(err) => {
                                    if remote_dir != prev_dir {
                                        let _ = c.cwd(&prev_dir);
                                    }
                                    if let Ok(mut st) = shared.lock() {
                                        st.error_banner = Some(err.clone());
                                        st.status_detail = err.clone();
                                    }
                                    push_ftp_log(&shared, err, true, false);
                                }
                            }
                            post_notify(
                                container_hwnd_raw,
                                main_hwnd_raw,
                                session_id,
                                FTP_EVT_STATUS_TEXT,
                            );
                        }
                    }
                    FtpWorkerCommand::Quit => {
                        if let Some(mut c) = client.take() {
                            c.quit();
                        }
                        break;
                    }
                },
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    // Keep-alive NOOP périodique pour éviter la déconnexion rapide par le serveur FTP
                    let mut keepalive_lost = false;
                    if let Some(ref mut c) = client
                        && c.noop().is_err()
                    {
                        keepalive_lost = true;
                    }
                    if keepalive_lost {
                        connect_and_resume(
                            &mut client,
                            &mut last_remote_dir,
                            &active_user,
                            &active_pass,
                            true,
                        );
                    }
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    if let Some(mut c) = client.take() {
                        c.quit();
                    }
                    break;
                }
            }
        }
        active_pass.clear();
    });
}
/// Dessine une flèche vectorielle "remonter au parent", un dossier ou un fichier sobre GDI.
unsafe fn draw_icon_ftp_entry(hdc: HDC, x: i32, y: i32, kind: FtpEntryKind) {
    unsafe {
        match kind {
            FtpEntryKind::ParentDir => {
                let pen = CreatePen(PS_SOLID, 2, COLORREF(col_accent_hover()));
                let old = SelectObject(hdc, pen.into());
                let cx = x + 7;
                let cy = y + 7;
                let _ = MoveToEx(hdc, cx, cy + 4, None);
                let _ = LineTo(hdc, cx, cy - 4);
                let _ = MoveToEx(hdc, cx - 4, cy, None);
                let _ = LineTo(hdc, cx, cy - 4);
                let _ = LineTo(hdc, cx + 4, cy);
                let _ = SelectObject(hdc, old);
                let _ = DeleteObject(pen.into());
            }
            FtpEntryKind::Directory => {
                draw_icon_folder(hdc, x, y, false, COL_FOLDER_AMBER);
            }
            FtpEntryKind::File => {
                let doc_rc = RECT {
                    left: x + 2,
                    top: y,
                    right: x + 13,
                    bottom: y + 14,
                };
                draw_rounded_box(hdc, doc_rc, col_bg_surface(), col_text_muted());
                draw_horizontal_line(hdc, x + 5, x + 10, y + 5, col_text_muted());
                draw_horizontal_line(hdc, x + 5, x + 10, y + 9, col_text_muted());
            }
        }
    }
}

unsafe fn draw_text_right(hdc: HDC, text: &str, mut rc: RECT) {
    if text.is_empty() {
        return;
    }
    unsafe {
        let mut wide: Vec<u16> = text.encode_utf16().collect();
        let _ = DrawTextW(
            hdc,
            &mut wide,
            &mut rc,
            DT_RIGHT | DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS | DT_NOPREFIX,
        );
    }
}

unsafe fn ftp_layout_controls(state: &mut FtpWndState, w: i32, h: i32) {
    unsafe {
        let dpi = state.palette.dpi;
        let toolbar_h = dpi.scale(44);
        let path_h = dpi.scale(38);
        let header_h = dpi.scale(28);
        let pad_x = dpi.scale(10);
        let btn_go_w = dpi.scale(50);

        let split_x = clamp_split_x(w, state.split_ratio_permille, dpi.scale(200));
        let left_r = split_x - 2;
        let right_l = split_x + 3;

        let fixed_top_h = toolbar_h + path_h + header_h + dpi.scale(60);
        let act_h = clamp_activity_height(h, state.activity_height, dpi.scale(54), fixed_top_h);
        let activity_top = h - act_h;

        state.pane_splitter_rc = RECT {
            left: split_x - 3,
            top: toolbar_h,
            right: split_x + 4,
            bottom: activity_top,
        };
        state.activity_splitter_rc = RECT {
            left: 0,
            top: activity_top - 3,
            right: w,
            bottom: activity_top + 4,
        };

        let edit_top = toolbar_h + dpi.scale(5);
        let edit_bottom = toolbar_h + path_h - dpi.scale(5);

        // Barre d'adresse locale (panneau gauche)
        let local_badge_w = dpi.scale(48);
        let local_edit_l = pad_x + local_badge_w + dpi.scale(8);
        let local_edit_r =
            (left_r - pad_x - btn_go_w - dpi.scale(6)).max(local_edit_l + dpi.scale(60));
        state.local_path_box_rc = RECT {
            left: local_edit_l,
            top: edit_top,
            right: local_edit_r,
            bottom: edit_bottom,
        };
        if !state.ed_local_path.is_invalid() {
            let _ = MoveWindow(
                state.ed_local_path,
                local_edit_l + dpi.scale(8),
                edit_top + dpi.scale(4),
                (local_edit_r - local_edit_l - dpi.scale(16)).max(40),
                (edit_bottom - edit_top - dpi.scale(8)).max(16),
                true,
            );
        }

        // Barre d'adresse FTP (panneau droit)
        let ftp_badge_w = dpi.scale(38);
        let right_w = w - right_l;
        let info_w = if right_w >= dpi.scale(420) {
            dpi.scale(140)
        } else {
            0
        };
        let info_gap = if info_w > 0 { dpi.scale(8) } else { 0 };
        let remote_edit_l = right_l + pad_x + ftp_badge_w + dpi.scale(8) + info_w + info_gap;
        let remote_edit_r =
            (w - pad_x - btn_go_w - dpi.scale(6)).max(remote_edit_l + dpi.scale(60));
        state.path_box_rc = RECT {
            left: remote_edit_l,
            top: edit_top,
            right: remote_edit_r,
            bottom: edit_bottom,
        };
        if !state.ed_path.is_invalid() {
            let _ = MoveWindow(
                state.ed_path,
                remote_edit_l + dpi.scale(8),
                edit_top + dpi.scale(4),
                (remote_edit_r - remote_edit_l - dpi.scale(16)).max(40),
                (edit_bottom - edit_top - dpi.scale(8)).max(16),
                true,
            );
        }
    }
}

fn selected_remote_entries(state: &FtpWndState) -> Vec<FtpEntry> {
    let Ok(st) = state.shared.lock() else {
        return Vec::new();
    };
    state
        .remote_selected
        .iter()
        .filter_map(|&idx| st.entries.get(idx).cloned())
        .collect()
}

fn selected_remote_entry(state: &FtpWndState) -> Option<FtpEntry> {
    let idx = state
        .remote_anchor_idx
        .filter(|i| state.remote_selected.contains(i))
        .or_else(|| state.remote_selected.iter().next().copied())?;
    let st = state.shared.lock().ok()?;
    st.entries.get(idx).cloned()
}

fn selected_local_entries(state: &FtpWndState) -> Vec<FtpEntry> {
    state
        .local_selected
        .iter()
        .filter_map(|&idx| state.local_entries.get(idx).cloned())
        .collect()
}

fn selected_local_entry(state: &FtpWndState) -> Option<FtpEntry> {
    let idx = state
        .local_anchor_idx
        .filter(|i| state.local_selected.contains(i))
        .or_else(|| state.local_selected.iter().next().copied())?;
    state.local_entries.get(idx).cloned()
}

fn trigger_ftp_upload_file_picker(state: &FtpWndState) {
    if let Some(local_path) = browse_for_open_file(
        state.container_hwnd,
        "Sélectionner un fichier à téléverser sur le serveur FTP",
        "Tous les fichiers (*.*)",
        "*.*",
    ) {
        let remote_name = local_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("fichier")
            .to_string();
        let _ = state.cmd_tx.send(FtpWorkerCommand::Upload {
            local_path,
            remote_name,
        });
    }
}

fn trigger_ftp_upload_selected_only(state: &FtpWndState) -> usize {
    if state.local_showing_drives {
        return 0;
    }
    let files: Vec<FtpEntry> = selected_local_entries(state)
        .into_iter()
        .filter(|e| e.kind == FtpEntryKind::File)
        .collect();
    let mut queued = 0usize;
    for entry in files {
        let local_path = state.local_dir.join(&entry.name);
        if local_path.is_file() {
            let _ = state.cmd_tx.send(FtpWorkerCommand::Upload {
                local_path,
                remote_name: entry.name,
            });
            queued += 1;
        }
    }
    if queued == 0 && !state.local_selected.is_empty() {
        push_ftp_log(
            &state.shared,
            "Sélectionnez un ou plusieurs fichiers locaux à téléverser.".to_string(),
            true,
            false,
        );
        unsafe {
            let _ = InvalidateRect(Some(state.container_hwnd), None, false);
        }
    }
    queued
}

fn trigger_ftp_upload(state: &FtpWndState) {
    if trigger_ftp_upload_selected_only(state) == 0 && state.local_selected.is_empty() {
        trigger_ftp_upload_file_picker(state);
    }
}

fn trigger_ftp_download_to_local_or_prompt(state: &FtpWndState, force_save_as: bool) {
    let files: Vec<FtpEntry> = selected_remote_entries(state)
        .into_iter()
        .filter(|e| e.kind == FtpEntryKind::File)
        .collect();
    if files.is_empty() {
        if !state.remote_selected.is_empty() {
            push_ftp_log(
                &state.shared,
                "Sélectionnez un ou plusieurs fichiers distants à télécharger.".to_string(),
                true,
                false,
            );
            unsafe {
                let _ = InvalidateRect(Some(state.container_hwnd), None, false);
            }
        }
        return;
    }

    if !force_save_as && !state.local_showing_drives && state.local_dir.is_dir() {
        let existing_count = files
            .iter()
            .filter(|e| state.local_dir.join(&e.name).exists())
            .count();
        if existing_count > 0 {
            let msg = if files.len() == 1 {
                format!(
                    "Le fichier « {} » existe déjà dans « {} ».\nVoulez-vous le remplacer ?",
                    files[0].name,
                    state.local_dir.display()
                )
            } else {
                format!(
                    "{existing_count} fichier(s) sur {} existent déjà dans « {} ».\nVoulez-vous les remplacer ?",
                    files.len(),
                    state.local_dir.display()
                )
            };
            if !confirm_dialog(
                state.main_hwnd,
                "Remplacer le(s) fichier(s) local(aux) ?",
                &msg,
            ) {
                return;
            }
        }
        for entry in files {
            let dest_path = state.local_dir.join(&entry.name);
            let _ = state.cmd_tx.send(FtpWorkerCommand::Download {
                remote_name: entry.name,
                local_path: dest_path,
                size_hint: entry.size,
            });
        }
        return;
    }

    let first = &files[0];
    let ext = Path::new(&first.name)
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("");
    if let Some(dest_path) = browse_for_save_file(
        state.container_hwnd,
        &format!("Enregistrer '{}' sous", first.name),
        &first.name,
        ext,
        "Tous les fichiers (*.*)",
        "*.*",
    ) {
        let _ = state.cmd_tx.send(FtpWorkerCommand::Download {
            remote_name: first.name.clone(),
            local_path: dest_path,
            size_hint: first.size,
        });
    }
}

fn trigger_ftp_mkdir(state: &mut FtpWndState) {
    if state.active_pane == FtpPane::Local {
        if state.local_showing_drives {
            return;
        }
        if let Some(dir_name) = run_text_prompt_dialog(
            state.main_hwnd,
            "Nouveau dossier local",
            "Nom du dossier à créer :",
            "",
            &state.palette,
        ) {
            let target = state.local_dir.join(dir_name.trim());
            if let Err(err) = std::fs::create_dir(&target) {
                show_error_dialog(
                    state.main_hwnd,
                    "Erreur dossier local",
                    &format!("Impossible de créer « {} » : {err}", target.display()),
                );
            } else {
                refresh_local_entries(state);
                unsafe {
                    let _ = InvalidateRect(Some(state.container_hwnd), None, false);
                }
            }
        }
        return;
    }

    if let Some(dir_name) = run_text_prompt_dialog(
        state.main_hwnd,
        "Nouveau dossier FTP",
        "Nom du dossier à créer :",
        "",
        &state.palette,
    ) {
        let _ = state.cmd_tx.send(FtpWorkerCommand::Mkdir(dir_name));
    }
}

fn trigger_ftp_rename(state: &mut FtpWndState) {
    if state.active_pane == FtpPane::Local {
        if state.local_showing_drives {
            return;
        }
        let Some(entry) = selected_local_entry(state) else {
            return;
        };
        if entry.kind == FtpEntryKind::ParentDir {
            return;
        }
        if let Some(new_name) = run_text_prompt_dialog(
            state.main_hwnd,
            "Renommer l'élément local",
            "Nouveau nom :",
            &entry.name,
            &state.palette,
        ) && new_name != entry.name
        {
            let src = state.local_dir.join(&entry.name);
            let dst = state.local_dir.join(new_name.trim());
            if let Err(err) = std::fs::rename(&src, &dst) {
                show_error_dialog(
                    state.main_hwnd,
                    "Erreur renommage local",
                    &format!("Impossible de renommer : {err}"),
                );
            } else {
                refresh_local_entries(state);
                unsafe {
                    let _ = InvalidateRect(Some(state.container_hwnd), None, false);
                }
            }
        }
        return;
    }

    let Some(entry) = selected_remote_entry(state) else {
        return;
    };
    if entry.kind == FtpEntryKind::ParentDir {
        return;
    }
    if let Some(new_name) = run_text_prompt_dialog(
        state.main_hwnd,
        "Renommer l'élément distant",
        "Nouveau nom :",
        &entry.name,
        &state.palette,
    ) && new_name != entry.name
    {
        let _ = state.cmd_tx.send(FtpWorkerCommand::Rename {
            old_name: entry.name,
            new_name,
        });
    }
}

fn trigger_ftp_delete(state: &mut FtpWndState) {
    if state.active_pane == FtpPane::Local {
        if state.local_showing_drives {
            return;
        }
        let items: Vec<FtpEntry> = selected_local_entries(state)
            .into_iter()
            .filter(|e| e.kind != FtpEntryKind::ParentDir)
            .collect();
        if items.is_empty() {
            return;
        }
        let prompt = if items.len() == 1 {
            let kind_lbl = if items[0].kind == FtpEntryKind::Directory {
                "le dossier local (et son contenu)"
            } else {
                "le fichier local"
            };
            format!(
                "Voulez-vous vraiment supprimer {kind_lbl} « {} » ?",
                items[0].name
            )
        } else {
            format!(
                "Voulez-vous vraiment supprimer ces {} éléments locaux ?",
                items.len()
            )
        };
        if confirm_dialog(
            state.main_hwnd,
            "Confirmation de suppression locale",
            &prompt,
        ) {
            for entry in items {
                let target = state.local_dir.join(&entry.name);
                let res = if entry.kind == FtpEntryKind::Directory {
                    std::fs::remove_dir_all(&target)
                } else {
                    std::fs::remove_file(&target)
                };
                if let Err(err) = res {
                    show_error_dialog(
                        state.main_hwnd,
                        "Erreur suppression locale",
                        &format!("Impossible de supprimer « {} » : {err}", entry.name),
                    );
                }
            }
            refresh_local_entries(state);
            unsafe {
                let _ = InvalidateRect(Some(state.container_hwnd), None, false);
            }
        }
        return;
    }

    let items: Vec<FtpEntry> = selected_remote_entries(state)
        .into_iter()
        .filter(|e| e.kind != FtpEntryKind::ParentDir)
        .collect();
    if items.is_empty() {
        return;
    }
    let prompt = if items.len() == 1 {
        let kind_lbl = if items[0].kind == FtpEntryKind::Directory {
            "le dossier distant"
        } else {
            "le fichier distant"
        };
        format!(
            "Voulez-vous vraiment supprimer {kind_lbl} « {} » ?",
            items[0].name
        )
    } else {
        format!(
            "Voulez-vous vraiment supprimer ces {} éléments distants ?",
            items.len()
        )
    };
    if confirm_dialog(state.main_hwnd, "Confirmation de suppression FTP", &prompt) {
        for entry in items {
            let _ = state.cmd_tx.send(FtpWorkerCommand::Delete {
                name: entry.name,
                is_dir: entry.kind == FtpEntryKind::Directory,
            });
        }
    }
}

fn activate_remote_entry(state: &FtpWndState, idx: usize) {
    let entry = {
        let Ok(st) = state.shared.lock() else { return };
        st.entries.get(idx).cloned()
    };
    let Some(e) = entry else { return };
    match e.kind {
        FtpEntryKind::ParentDir => {
            let _ = state.cmd_tx.send(FtpWorkerCommand::Cdup);
        }
        FtpEntryKind::Directory => {
            let _ = state.cmd_tx.send(FtpWorkerCommand::Cwd(e.name));
        }
        FtpEntryKind::File => {
            trigger_ftp_download_to_local_or_prompt(state, false);
        }
    }
}

fn activate_local_entry(state: &mut FtpWndState, idx: usize) {
    let Some(e) = state.local_entries.get(idx).cloned() else {
        return;
    };
    match e.kind {
        FtpEntryKind::ParentDir => local_navigate_up(state),
        FtpEntryKind::Directory => {
            let next = if state.local_showing_drives {
                PathBuf::from(&e.name)
            } else {
                state.local_dir.join(&e.name)
            };
            local_navigate_to(state, next);
        }
        FtpEntryKind::File => {
            let is_conn = state.shared.lock().is_ok_and(|s| s.is_connected);
            if is_conn {
                let local_path = state.local_dir.join(&e.name);
                if local_path.is_file() {
                    let _ = state.cmd_tx.send(FtpWorkerCommand::Upload {
                        local_path,
                        remote_name: e.name,
                    });
                }
            }
        }
    }
    unsafe {
        let _ = InvalidateRect(Some(state.container_hwnd), None, false);
    }
}

unsafe fn show_ftp_context_menu(state: &mut FtpWndState, screen_pt: POINT) {
    unsafe {
        let Ok(hmenu) = CreatePopupMenu() else { return };
        if state.active_pane == FtpPane::Local {
            let sel_all = selected_local_entries(state);
            let sel_items: Vec<&FtpEntry> = sel_all
                .iter()
                .filter(|e| e.kind != FtpEntryKind::ParentDir && !state.local_showing_drives)
                .collect();
            let file_count = sel_items
                .iter()
                .filter(|e| e.kind == FtpEntryKind::File)
                .count();
            let sel_first = selected_local_entry(state);

            if file_count > 1 {
                let lbl: Vec<u16> = format!("Téléverser {file_count} fichiers vers le serveur FTP")
                    .encode_utf16()
                    .chain(std::iter::once(0))
                    .collect();
                let _ = AppendMenuW(
                    hmenu,
                    MF_STRING,
                    CMD_FTP_CTX_UPLOAD as usize,
                    PCWSTR(lbl.as_ptr()),
                );
            } else if file_count == 1 {
                let _ = AppendMenuW(
                    hmenu,
                    MF_STRING,
                    CMD_FTP_CTX_UPLOAD as usize,
                    w!("Téléverser vers le serveur FTP"),
                );
            } else if sel_first.is_some() {
                let _ = AppendMenuW(
                    hmenu,
                    MF_STRING,
                    CMD_FTP_CTX_OPEN as usize,
                    w!("Ouvrir le dossier"),
                );
            }
            if !state.local_showing_drives {
                let _ = AppendMenuW(hmenu, MF_SEPARATOR, 0, PCWSTR::null());
                let _ = AppendMenuW(
                    hmenu,
                    MF_STRING,
                    CMD_FTP_CTX_MKDIR as usize,
                    w!("Nouveau dossier local…"),
                );
                if sel_items.len() == 1 {
                    let _ = AppendMenuW(
                        hmenu,
                        MF_STRING,
                        CMD_FTP_CTX_RENAME as usize,
                        w!("Renommer…"),
                    );
                }
                if !sel_items.is_empty() {
                    let _ = AppendMenuW(
                        hmenu,
                        MF_STRING,
                        CMD_FTP_CTX_DELETE as usize,
                        w!("Supprimer…"),
                    );
                }
            }
            let _ = AppendMenuW(hmenu, MF_SEPARATOR, 0, PCWSTR::null());
            let _ = AppendMenuW(
                hmenu,
                MF_STRING,
                CMD_FTP_CTX_REFRESH as usize,
                w!("Actualiser\tF5"),
            );

            let _ = SetForegroundWindow(state.container_hwnd);
            let cmd = TrackPopupMenu(
                hmenu,
                TPM_LEFTALIGN | TPM_TOPALIGN | TPM_RETURNCMD,
                screen_pt.x,
                screen_pt.y,
                Some(0),
                state.container_hwnd,
                None,
            );
            let _ = DestroyMenu(hmenu);

            match cmd.0 as u32 {
                CMD_FTP_CTX_OPEN => {
                    if let Some(idx) = state.local_selected.iter().next().copied() {
                        activate_local_entry(state, idx);
                    }
                }
                CMD_FTP_CTX_UPLOAD => trigger_ftp_upload(state),
                CMD_FTP_CTX_MKDIR => trigger_ftp_mkdir(state),
                CMD_FTP_CTX_RENAME => trigger_ftp_rename(state),
                CMD_FTP_CTX_DELETE => trigger_ftp_delete(state),
                CMD_FTP_CTX_REFRESH => {
                    refresh_local_entries(state);
                    let _ = InvalidateRect(Some(state.container_hwnd), None, false);
                }
                _ => {}
            }
            return;
        }

        let sel_all = selected_remote_entries(state);
        let sel_items: Vec<&FtpEntry> = sel_all
            .iter()
            .filter(|e| e.kind != FtpEntryKind::ParentDir)
            .collect();
        let file_count = sel_items
            .iter()
            .filter(|e| e.kind == FtpEntryKind::File)
            .count();
        let sel_first = selected_remote_entry(state);

        let mut discovered_editors: Vec<ExternalEditorApp> = Vec::new();
        if file_count > 1 {
            let lbl: Vec<u16> = format!("Télécharger {file_count} fichiers vers le dossier local")
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();
            let _ = AppendMenuW(
                hmenu,
                MF_STRING,
                CMD_FTP_CTX_DOWNLOAD as usize,
                PCWSTR(lbl.as_ptr()),
            );
        } else if file_count == 1 {
            let _ = AppendMenuW(hmenu, MF_STRING, CMD_FTP_CTX_EDIT as usize, w!("Éditer"));
            if let Some(first_file) = sel_items.iter().find(|e| e.kind == FtpEntryKind::File) {
                discovered_editors = discover_external_editors(&first_file.name);
            }
            if let Ok(hsub) = CreatePopupMenu() {
                for (idx, app) in discovered_editors
                    .iter()
                    .take(MAX_EXTERNAL_EDITORS_MENU)
                    .enumerate()
                {
                    let app_w: Vec<u16> = app
                        .display_name
                        .encode_utf16()
                        .chain(std::iter::once(0))
                        .collect();
                    let _ = AppendMenuW(
                        hsub,
                        MF_STRING,
                        (CMD_FTP_CTX_EDIT_WITH_BASE + idx as u32) as usize,
                        PCWSTR(app_w.as_ptr()),
                    );
                }
                if !discovered_editors.is_empty() {
                    let _ = AppendMenuW(hsub, MF_SEPARATOR, 0, PCWSTR::null());
                }
                let _ = AppendMenuW(
                    hsub,
                    MF_STRING,
                    CMD_FTP_CTX_EDIT_WITH_OTHER as usize,
                    w!("Choisir une autre application…"),
                );
                let _ = AppendMenuW(hmenu, MF_POPUP, hsub.0 as usize, w!("Éditer avec"));
            }
            let _ = AppendMenuW(
                hmenu,
                MF_STRING,
                CMD_FTP_CTX_DOWNLOAD as usize,
                w!("Télécharger vers le dossier local"),
            );
            let _ = AppendMenuW(
                hmenu,
                MF_STRING,
                CMD_FTP_CTX_DOWNLOAD_AS as usize,
                w!("Télécharger sous…"),
            );
        } else if sel_first.is_some() {
            let _ = AppendMenuW(
                hmenu,
                MF_STRING,
                CMD_FTP_CTX_OPEN as usize,
                w!("Ouvrir le dossier"),
            );
        }
        let _ = AppendMenuW(
            hmenu,
            MF_STRING,
            CMD_FTP_CTX_UPLOAD as usize,
            w!("Téléverser un fichier ici…"),
        );
        let _ = AppendMenuW(hmenu, MF_SEPARATOR, 0, PCWSTR::null());
        let _ = AppendMenuW(
            hmenu,
            MF_STRING,
            CMD_FTP_CTX_MKDIR as usize,
            w!("Nouveau dossier…"),
        );
        if sel_items.len() == 1 {
            let _ = AppendMenuW(
                hmenu,
                MF_STRING,
                CMD_FTP_CTX_RENAME as usize,
                w!("Renommer…"),
            );
        }
        if !sel_items.is_empty() {
            let _ = AppendMenuW(
                hmenu,
                MF_STRING,
                CMD_FTP_CTX_DELETE as usize,
                w!("Supprimer…"),
            );
        }
        let _ = AppendMenuW(hmenu, MF_SEPARATOR, 0, PCWSTR::null());
        let _ = AppendMenuW(
            hmenu,
            MF_STRING,
            CMD_FTP_CTX_REFRESH as usize,
            w!("Actualiser\tF5"),
        );

        let _ = SetForegroundWindow(state.container_hwnd);
        let cmd = TrackPopupMenu(
            hmenu,
            TPM_LEFTALIGN | TPM_TOPALIGN | TPM_RETURNCMD,
            screen_pt.x,
            screen_pt.y,
            Some(0),
            state.container_hwnd,
            None,
        );
        let _ = DestroyMenu(hmenu);

        match cmd.0 as u32 {
            CMD_FTP_CTX_OPEN => {
                if let Some(idx) = state.remote_selected.iter().next().copied() {
                    activate_remote_entry(state, idx);
                }
            }
            CMD_FTP_CTX_EDIT => trigger_ftp_edit(state),
            CMD_FTP_CTX_EDIT_WITH_OTHER => {
                trigger_ftp_edit_with(state, ExternalEditTarget::SystemChooser)
            }
            c if (CMD_FTP_CTX_EDIT_WITH_BASE
                ..CMD_FTP_CTX_EDIT_WITH_BASE + MAX_EXTERNAL_EDITORS_MENU as u32)
                .contains(&c) =>
            {
                let idx = (c - CMD_FTP_CTX_EDIT_WITH_BASE) as usize;
                if let Some(app) = discovered_editors.get(idx) {
                    trigger_ftp_edit_with(state, ExternalEditTarget::App(app.clone()));
                }
            }
            CMD_FTP_CTX_DOWNLOAD => trigger_ftp_download_to_local_or_prompt(state, false),
            CMD_FTP_CTX_DOWNLOAD_AS => trigger_ftp_download_to_local_or_prompt(state, true),
            CMD_FTP_CTX_UPLOAD => trigger_ftp_upload(state),
            CMD_FTP_CTX_MKDIR => trigger_ftp_mkdir(state),
            CMD_FTP_CTX_RENAME => trigger_ftp_rename(state),
            CMD_FTP_CTX_DELETE => trigger_ftp_delete(state),
            CMD_FTP_CTX_REFRESH => {
                let _ = state.cmd_tx.send(FtpWorkerCommand::Refresh);
            }
            _ => {}
        }
    }
}

fn trigger_ftp_edit(state: &FtpWndState) {
    let files: Vec<FtpEntry> = selected_remote_entries(state)
        .into_iter()
        .filter(|e| e.kind == FtpEntryKind::File)
        .collect();
    if files.len() != 1 {
        return;
    }
    let entry = &files[0];
    let _ = state.cmd_tx.send(FtpWorkerCommand::FetchForEdit {
        remote_name: entry.name.clone(),
        size_hint: entry.size,
    });
}

fn trigger_ftp_edit_with(state: &FtpWndState, target: ExternalEditTarget) {
    let files: Vec<FtpEntry> = selected_remote_entries(state)
        .into_iter()
        .filter(|e| e.kind == FtpEntryKind::File)
        .collect();
    if files.len() != 1 {
        return;
    }
    let entry = &files[0];
    let _ = state.cmd_tx.send(FtpWorkerCommand::FetchForExternalEdit {
        remote_name: entry.name.clone(),
        size_hint: entry.size,
        target,
    });
}

fn reg_read_string(root: HKEY, subkey: &str, value_name: Option<&str>) -> Option<String> {
    let subkey_w: Vec<u16> = subkey.encode_utf16().chain(std::iter::once(0)).collect();
    let val_w: Option<Vec<u16>> =
        value_name.map(|v| v.encode_utf16().chain(std::iter::once(0)).collect());
    unsafe {
        let mut hkey = HKEY::default();
        if RegOpenKeyExW(
            root,
            PCWSTR(subkey_w.as_ptr()),
            Some(0),
            KEY_READ,
            &mut hkey,
        )
        .is_err()
        {
            return None;
        }
        let val_ptr = val_w
            .as_ref()
            .map(|v| PCWSTR(v.as_ptr()))
            .unwrap_or(PCWSTR::null());
        let mut size_bytes: u32 = 0;
        let ok = RegGetValueW(
            hkey,
            PCWSTR::null(),
            val_ptr,
            RRF_RT_REG_SZ | RRF_RT_REG_EXPAND_SZ,
            None,
            None,
            Some(&mut size_bytes),
        )
        .is_ok();
        let res = if ok && size_bytes >= 2 {
            let mut buf = vec![0u16; (size_bytes as usize) / 2 + 1];
            let mut actual = size_bytes;
            if RegGetValueW(
                hkey,
                PCWSTR::null(),
                val_ptr,
                RRF_RT_REG_SZ | RRF_RT_REG_EXPAND_SZ,
                None,
                Some(buf.as_mut_ptr() as *mut c_void),
                Some(&mut actual),
            )
            .is_ok()
            {
                let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
                Some(String::from_utf16_lossy(&buf[..len]))
            } else {
                None
            }
        } else {
            None
        };
        let _ = RegCloseKey(hkey);
        res
    }
}

fn reg_enum_string_values(root: HKEY, subkey: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let subkey_w: Vec<u16> = subkey.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        let mut hkey = HKEY::default();
        if RegOpenKeyExW(
            root,
            PCWSTR(subkey_w.as_ptr()),
            Some(0),
            KEY_READ,
            &mut hkey,
        )
        .is_err()
        {
            return out;
        }
        let mut idx = 0u32;
        loop {
            let mut name_buf = [0u16; 256];
            let mut name_len = name_buf.len() as u32;
            let mut data_buf = [0u8; 1024];
            let mut data_len = data_buf.len() as u32;
            let mut val_type = 0u32;
            let err = RegEnumValueW(
                hkey,
                idx,
                Some(windows::core::PWSTR(name_buf.as_mut_ptr())),
                &mut name_len,
                None,
                Some(&mut val_type),
                Some(data_buf.as_mut_ptr()),
                Some(&mut data_len),
            );
            if err.is_err() {
                break;
            }
            let name = String::from_utf16_lossy(&name_buf[..name_len as usize]);
            let val = if (val_type == 1 || val_type == 2) && data_len >= 2 {
                let u16_slice = std::slice::from_raw_parts(
                    data_buf.as_ptr() as *const u16,
                    (data_len as usize) / 2,
                );
                let end = u16_slice
                    .iter()
                    .position(|&c| c == 0)
                    .unwrap_or(u16_slice.len());
                String::from_utf16_lossy(&u16_slice[..end])
            } else {
                String::new()
            };
            out.push((name, val));
            idx += 1;
        }
        let _ = RegCloseKey(hkey);
    }
    out
}

fn extract_exe_path_from_command(raw: &str) -> Option<PathBuf> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    let candidate = if let Some(rest) = trimmed.strip_prefix('"') {
        rest.split('"').next().unwrap_or("").trim()
    } else if let Some(idx) = trimmed.to_ascii_lowercase().find(".exe") {
        trimmed[..idx + 4].trim()
    } else {
        trimmed.split_whitespace().next().unwrap_or("")
    };
    if candidate.is_empty() {
        return None;
    }
    let path = PathBuf::from(candidate);
    if path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("exe"))
    {
        Some(path)
    } else {
        None
    }
}

fn resolve_exe_via_app_paths(exe_name: &str) -> Option<PathBuf> {
    let key = format!("SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\App Paths\\{exe_name}");
    for root in [HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE] {
        if let Some(val) = reg_read_string(root, &key, None)
            && let Some(p) = extract_exe_path_from_command(&val)
            && p.is_file()
        {
            return Some(p);
        }
    }
    None
}

fn push_unique_editor(
    out: &mut Vec<ExternalEditorApp>,
    seen_paths: &mut std::collections::HashSet<String>,
    display_name: &str,
    exe_path: PathBuf,
) {
    if !exe_path.is_file() {
        return;
    }
    let lower_name = exe_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if lower_name == "malvision.exe" || lower_name == "rundll32.exe" || lower_name == "cmd.exe" {
        return;
    }
    let norm = exe_path.to_string_lossy().to_ascii_lowercase();
    if seen_paths.insert(norm) {
        out.push(ExternalEditorApp {
            display_name: display_name.to_string(),
            exe_path,
        });
    }
}

fn discover_external_editors(remote_name: &str) -> Vec<ExternalEditorApp> {
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();

    let local_app_data = std::env::var_os("LOCALAPPDATA").map(PathBuf::from);
    let program_files = std::env::var_os("ProgramFiles").map(PathBuf::from);
    let program_files_x86 = std::env::var_os("ProgramFiles(x86)").map(PathBuf::from);
    let system_root = std::env::var_os("SystemRoot")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Windows"));

    // 1. Éditeurs de code / texte courants détectés sur le poste
    let known: &[(&str, &str, &[&str])] = &[
        (
            "Visual Studio Code",
            "Code.exe",
            &[
                r"Programs\Microsoft VS Code\Code.exe",
                r"Microsoft VS Code\Code.exe",
            ],
        ),
        (
            "VS Code Insiders",
            "Code - Insiders.exe",
            &[
                r"Programs\Microsoft VS Code Insiders\Code - Insiders.exe",
                r"Microsoft VS Code Insiders\Code - Insiders.exe",
            ],
        ),
        (
            "Cursor",
            "Cursor.exe",
            &[r"Programs\cursor\Cursor.exe", r"Cursor\Cursor.exe"],
        ),
        (
            "Windsurf",
            "Windsurf.exe",
            &[r"Programs\Windsurf\Windsurf.exe", r"Windsurf\Windsurf.exe"],
        ),
        ("Zed", "Zed.exe", &[r"Programs\Zed\Zed.exe", r"Zed\Zed.exe"]),
        ("Notepad++", "notepad++.exe", &[r"Notepad++\notepad++.exe"]),
        (
            "Sublime Text",
            "sublime_text.exe",
            &[
                r"Sublime Text\sublime_text.exe",
                r"Sublime Text 3\sublime_text.exe",
            ],
        ),
        (
            "VSCodium",
            "VSCodium.exe",
            &[r"Programs\VSCodium\VSCodium.exe", r"VSCodium\VSCodium.exe"],
        ),
    ];

    for (label, exe_name, rel_paths) in known {
        if let Some(p) = resolve_exe_via_app_paths(exe_name) {
            push_unique_editor(&mut out, &mut seen, label, p);
            continue;
        }
        let roots = [
            local_app_data.as_ref(),
            program_files.as_ref(),
            program_files_x86.as_ref(),
        ];
        'search_known: for root_opt in roots.into_iter().flatten() {
            for rel in *rel_paths {
                let cand = root_opt.join(rel);
                if cand.is_file() {
                    push_unique_editor(&mut out, &mut seen, label, cand);
                    break 'search_known;
                }
            }
        }
    }

    // 2. Bloc-notes Windows (toujours disponible)
    push_unique_editor(
        &mut out,
        &mut seen,
        "Bloc-notes (Notepad)",
        system_root.join("System32").join("notepad.exe"),
    );

    // 3. Applications associées à l'extension du fichier dans le Registre Windows
    if let Some(ext) = Path::new(remote_name)
        .extension()
        .and_then(|e| e.to_str())
        .filter(|e| !e.is_empty())
    {
        let dot_ext = format!(".{}", ext.to_ascii_lowercase());
        let user_owl = format!(
            "Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\FileExts\\{dot_ext}\\OpenWithList"
        );
        let mut exe_candidates: Vec<String> = Vec::new();
        for (name, val) in reg_enum_string_values(HKEY_CURRENT_USER, &user_owl) {
            if !name.eq_ignore_ascii_case("MRUList") && val.to_ascii_lowercase().ends_with(".exe") {
                exe_candidates.push(val);
            }
        }
        for exe_name in exe_candidates {
            if out.len() >= MAX_EXTERNAL_EDITORS_MENU {
                break;
            }
            let cmd_key = format!("Applications\\{exe_name}\\shell\\open\\command");
            let exe_path = reg_read_string(HKEY_CLASSES_ROOT, &cmd_key, None)
                .and_then(|c| extract_exe_path_from_command(&c))
                .or_else(|| resolve_exe_via_app_paths(&exe_name));
            if let Some(p) = exe_path {
                let app_key = format!("Applications\\{exe_name}");
                let friendly =
                    reg_read_string(HKEY_CLASSES_ROOT, &app_key, Some("FriendlyAppName"))
                        .filter(|s| !s.trim().is_empty() && !s.starts_with('@'))
                        .unwrap_or_else(|| {
                            p.file_stem()
                                .and_then(|s| s.to_str())
                                .unwrap_or(&exe_name)
                                .to_string()
                        });
                push_unique_editor(&mut out, &mut seen, &friendly, p);
            }
        }

        let user_progids = format!(
            "Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\FileExts\\{dot_ext}\\OpenWithProgids"
        );
        let mut progids: Vec<String> = reg_enum_string_values(HKEY_CURRENT_USER, &user_progids)
            .into_iter()
            .map(|(k, _)| k)
            .collect();
        let cr_progids = format!("{dot_ext}\\OpenWithProgids");
        progids.extend(
            reg_enum_string_values(HKEY_CLASSES_ROOT, &cr_progids)
                .into_iter()
                .map(|(k, _)| k),
        );
        for progid in progids {
            if out.len() >= MAX_EXTERNAL_EDITORS_MENU {
                break;
            }
            let cmd_key = format!("{progid}\\shell\\open\\command");
            if let Some(cmd) = reg_read_string(HKEY_CLASSES_ROOT, &cmd_key, None)
                && let Some(p) = extract_exe_path_from_command(&cmd)
            {
                let friendly = reg_read_string(HKEY_CLASSES_ROOT, &progid, None)
                    .filter(|s| !s.trim().is_empty() && !s.starts_with('@'))
                    .unwrap_or_else(|| {
                        p.file_stem()
                            .and_then(|s| s.to_str())
                            .unwrap_or("Application")
                            .to_string()
                    });
                push_unique_editor(&mut out, &mut seen, &friendly, p);
            }
        }
    }

    out.truncate(MAX_EXTERNAL_EDITORS_MENU);
    out
}

fn launch_external_editor(
    owner: HWND,
    local_path: &Path,
    target: &ExternalEditTarget,
) -> Result<(), String> {
    let path_str = local_path.to_string_lossy().to_string();
    let (verb_w, file_w, params_w): (Vec<u16>, Vec<u16>, Option<Vec<u16>>) = match target {
        ExternalEditTarget::App(app) => {
            let exe_str = app.exe_path.to_string_lossy().to_string();
            let quoted_file = format!("\"{path_str}\"");
            (
                "open\0".encode_utf16().collect(),
                exe_str.encode_utf16().chain(std::iter::once(0)).collect(),
                Some(
                    quoted_file
                        .encode_utf16()
                        .chain(std::iter::once(0))
                        .collect(),
                ),
            )
        }
        ExternalEditTarget::SystemChooser => (
            "openas\0".encode_utf16().collect(),
            path_str.encode_utf16().chain(std::iter::once(0)).collect(),
            None,
        ),
    };

    let hinst = unsafe {
        ShellExecuteW(
            Some(owner),
            PCWSTR(verb_w.as_ptr()),
            PCWSTR(file_w.as_ptr()),
            params_w
                .as_ref()
                .map(|p| PCWSTR(p.as_ptr()))
                .unwrap_or(PCWSTR::null()),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        )
    };
    if (hinst.0 as isize) <= 32 {
        return Err(format!(
            "Impossible de lancer l'éditeur externe (code ShellExecute {}).",
            hinst.0 as isize
        ));
    }
    Ok(())
}

fn spawn_external_edit_watcher(
    container_hwnd_raw: usize,
    shared: Arc<Mutex<FtpSharedState>>,
    stop_flag: Arc<AtomicBool>,
    remote_dir: String,
    remote_name: String,
    local_path: PathBuf,
    initial_bytes: Vec<u8>,
) {
    std::thread::spawn(move || {
        let mut last_bytes = initial_bytes;
        let mut last_modified = std::fs::metadata(&local_path)
            .ok()
            .and_then(|m| m.modified().ok());

        while !stop_flag.load(Ordering::Relaxed) {
            std::thread::sleep(Duration::from_millis(600));
            if stop_flag.load(Ordering::Relaxed) || !local_path.exists() {
                break;
            }
            let cur_modified = std::fs::metadata(&local_path)
                .ok()
                .and_then(|m| m.modified().ok());
            if cur_modified.is_some() && cur_modified != last_modified {
                // Laisse le temps à l'éditeur (écriture atomique) de relâcher le fichier
                std::thread::sleep(Duration::from_millis(160));
                let mut read_bytes = None;
                for _ in 0..3 {
                    if let Ok(b) = std::fs::read(&local_path) {
                        read_bytes = Some(b);
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(120));
                }
                last_modified = std::fs::metadata(&local_path)
                    .ok()
                    .and_then(|m| m.modified().ok());
                if let Some(new_bytes) = read_bytes
                    && new_bytes != last_bytes
                {
                    last_bytes = new_bytes.clone();
                    if let Ok(mut st) = shared.lock() {
                        st.pending_external_changes
                            .push_back(PendingExternalChange {
                                remote_dir: remote_dir.clone(),
                                remote_name: remote_name.clone(),
                                new_bytes,
                            });
                    }
                    unsafe {
                        let _ = PostMessageW(
                            Some(HWND(container_hwnd_raw as *mut c_void)),
                            WM_FTP_EXTERNAL_FILE_CHANGED,
                            WPARAM(0),
                            LPARAM(0),
                        );
                    }
                }
            }
        }
    });
}

fn ftp_hit_test(state: &FtpWndState, x: i32, y: i32) -> Option<FtpUiZone> {
    let pt = POINT { x, y };
    for hz in &state.hit_zones {
        if pt.x >= hz.rect.left
            && pt.x < hz.rect.right
            && pt.y >= hz.rect.top
            && pt.y < hz.rect.bottom
        {
            return Some(hz.zone);
        }
    }
    None
}

#[allow(clippy::too_many_arguments)]
unsafe fn paint_file_list_pane(
    hdc: HDC,
    palette: &GdiPalette,
    theme: &crate::settings::ThemeColors,
    pane_l: i32,
    pane_r: i32,
    header_top: i32,
    header_h: i32,
    list_bottom: i32,
    row_h: i32,
    entries: &[FtpEntry],
    selected_indices: &BTreeSet<usize>,
    scroll_offset: &mut i32,
    hovered_zone: Option<FtpUiZone>,
    is_active_pane: bool,
    is_local: bool,
    empty_msg: &str,
    hit_zones: &mut Vec<FtpHitZone>,
) {
    unsafe {
        let dpi = palette.dpi;
        let pad_x = dpi.scale(10);
        let pane_w = (pane_r - pane_l).max(60);

        let header_rc = RECT {
            left: pane_l,
            top: header_top,
            right: pane_r,
            bottom: header_top + header_h,
        };
        let _ = FillRect(hdc, &header_rc, palette.brush_bg_surface);
        draw_horizontal_line(
            hdc,
            pane_l,
            pane_r,
            header_rc.bottom - 1,
            if is_active_pane {
                col_accent()
            } else {
                col_border()
            },
        );

        let col_name_l = pane_l + pad_x + dpi.scale(24);
        let col_type_l = pane_l + (pane_w * 50) / 100;
        let col_type_r = pane_l + (pane_w * 64) / 100;
        let col_size_r = pane_l + (pane_w * 80) / 100;
        let col_mod_l = col_size_r + dpi.scale(10);

        let _ = SelectObject(hdc, palette.font_small.into());
        let _ = SetTextColor(hdc, COLORREF(col_text_muted()));
        draw_text_line(
            hdc,
            "Nom",
            RECT {
                left: col_name_l,
                top: header_rc.top,
                right: col_type_l - dpi.scale(6),
                bottom: header_rc.bottom,
            },
        );
        draw_text_line(
            hdc,
            "Type",
            RECT {
                left: col_type_l,
                top: header_rc.top,
                right: col_type_r,
                bottom: header_rc.bottom,
            },
        );
        draw_text_right(
            hdc,
            "Taille",
            RECT {
                left: col_type_r + dpi.scale(2),
                top: header_rc.top,
                right: col_size_r,
                bottom: header_rc.bottom,
            },
        );
        draw_text_line(
            hdc,
            if is_local {
                "Modifié"
            } else {
                "Modifié / Droits"
            },
            RECT {
                left: col_mod_l,
                top: header_rc.top,
                right: pane_r - pad_x,
                bottom: header_rc.bottom,
            },
        );

        let list_top = header_rc.bottom;
        let view_h = (list_bottom - list_top).max(1);
        if entries.is_empty() {
            let _ = SelectObject(hdc, palette.font_ui.into());
            let _ = SetTextColor(hdc, COLORREF(col_text_muted()));
            draw_text_line(
                hdc,
                empty_msg,
                RECT {
                    left: pane_l + pad_x * 2,
                    top: list_top + dpi.scale(22),
                    right: pane_r - pad_x * 2,
                    bottom: list_top + dpi.scale(50),
                },
            );
            return;
        }

        let total_h = entries.len() as i32 * row_h;
        let max_scroll = (total_h - view_h).max(0);
        *scroll_offset = (*scroll_offset).clamp(0, max_scroll);

        for (idx, entry) in entries.iter().enumerate() {
            let ry = list_top + idx as i32 * row_h - *scroll_offset;
            if ry + row_h <= list_top || ry >= list_bottom {
                continue;
            }
            let vis_top = ry.max(list_top);
            let vis_bottom = (ry + row_h).min(list_bottom);
            let row_rc = RECT {
                left: pane_l,
                top: vis_top,
                right: pane_r - dpi.scale(9),
                bottom: vis_bottom,
            };
            let zone = if is_local {
                FtpUiZone::LocalRow(idx)
            } else {
                FtpUiZone::Row(idx)
            };

            let is_sel = selected_indices.contains(&idx);
            let is_hover = hovered_zone == Some(zone);
            if is_sel {
                let sel_brush = CreateSolidBrush(COLORREF(theme.bg_selected));
                let _ = FillRect(hdc, &row_rc, sel_brush);
                let _ = DeleteObject(sel_brush.into());
                let bar_rc = RECT {
                    left: pane_l,
                    top: vis_top,
                    right: pane_l + dpi.scale(3),
                    bottom: vis_bottom,
                };
                let _ = FillRect(hdc, &bar_rc, palette.brush_accent);
            } else if is_hover {
                let _ = FillRect(hdc, &row_rc, palette.brush_bg_hover);
            }

            if vis_bottom - vis_top >= row_h / 2 {
                let icon_y = ry + (row_h - 14) / 2;
                draw_icon_ftp_entry(hdc, pane_l + pad_x, icon_y, entry.kind);

                let _ = SelectObject(
                    hdc,
                    if entry.kind != FtpEntryKind::File || is_sel {
                        palette.font_ui_bold.into()
                    } else {
                        palette.font_ui.into()
                    },
                );
                let _ = SetTextColor(hdc, COLORREF(col_text_primary()));
                draw_text_line(
                    hdc,
                    &entry.name,
                    RECT {
                        left: col_name_l,
                        top: vis_top,
                        right: col_type_l - dpi.scale(6),
                        bottom: vis_bottom,
                    },
                );

                let _ = SelectObject(hdc, palette.font_small.into());
                let _ = SetTextColor(hdc, COLORREF(col_text_muted()));
                let kind_txt = match entry.kind {
                    FtpEntryKind::ParentDir => "Dossier parent",
                    FtpEntryKind::Directory => "Dossier",
                    FtpEntryKind::File => "Fichier",
                };
                draw_text_line(
                    hdc,
                    kind_txt,
                    RECT {
                        left: col_type_l,
                        top: vis_top,
                        right: col_type_r,
                        bottom: vis_bottom,
                    },
                );

                let size_txt = entry
                    .size
                    .map(format_file_size)
                    .unwrap_or_else(|| "—".to_string());
                draw_text_right(
                    hdc,
                    &size_txt,
                    RECT {
                        left: col_type_r + dpi.scale(2),
                        top: vis_top,
                        right: col_size_r,
                        bottom: vis_bottom,
                    },
                );

                let mod_txt = if entry.modified.is_empty() {
                    &entry.permissions
                } else {
                    &entry.modified
                };
                draw_text_line(
                    hdc,
                    mod_txt,
                    RECT {
                        left: col_mod_l,
                        top: vis_top,
                        right: pane_r - pad_x,
                        bottom: vis_bottom,
                    },
                );
            }

            hit_zones.push(FtpHitZone { rect: row_rc, zone });
        }

        if total_h > view_h {
            let thumb_h = ((view_h * view_h) / total_h).max(dpi.scale(26));
            let thumb_top = list_top + ((*scroll_offset * (view_h - thumb_h)) / max_scroll.max(1));
            let thumb_rc = RECT {
                left: pane_r - dpi.scale(7),
                top: thumb_top,
                right: pane_r - dpi.scale(2),
                bottom: thumb_top + thumb_h,
            };
            draw_rounded_box(hdc, thumb_rc, col_border(), col_border());
        }
    }
}

unsafe fn paint_ftp_host(state: &mut FtpWndState) {
    unsafe {
        let mut ps = PAINTSTRUCT::default();
        let hdc_screen = BeginPaint(state.container_hwnd, &mut ps);
        let mut rc = RECT::default();
        let _ = GetClientRect(state.container_hwnd, &mut rc);
        let w = (rc.right - rc.left).max(1);
        let h = (rc.bottom - rc.top).max(1);

        let hdc = CreateCompatibleDC(Some(hdc_screen));
        let hbm = CreateCompatibleBitmap(hdc_screen, w, h);
        let old_bm = SelectObject(hdc, hbm.into());
        let _ = SetBkMode(hdc, TRANSPARENT);

        state.hit_zones.clear();

        let dpi = state.palette.dpi;
        let toolbar_h = dpi.scale(44);
        let path_h = dpi.scale(38);
        let header_h = dpi.scale(28);
        let pad_x = dpi.scale(10);
        let theme = active_theme_colors();

        let fixed_top_h = toolbar_h + path_h + header_h + dpi.scale(60);
        let act_h = clamp_activity_height(h, state.activity_height, dpi.scale(54), fixed_top_h);
        let activity_top = h - act_h;
        let split_x = clamp_split_x(w, state.split_ratio_permille, dpi.scale(200));
        let left_r = split_x - 2;
        let right_l = split_x + 3;

        state.pane_splitter_rc = RECT {
            left: split_x - 3,
            top: toolbar_h,
            right: split_x + 4,
            bottom: activity_top,
        };
        state.activity_splitter_rc = RECT {
            left: 0,
            top: activity_top - 3,
            right: w,
            bottom: activity_top + 4,
        };

        // Synchroniser la barre d'adresse locale
        if state.local_path_dirty && !state.ed_local_path.is_invalid() {
            let local_txt = if state.local_showing_drives {
                "Poste de travail".to_string()
            } else {
                state.local_dir.to_string_lossy().to_string()
            };
            set_window_text(state.ed_local_path, &local_txt);
            state.local_path_dirty = false;
        }

        // Synchroniser la barre d'adresse distante et lire le snapshot FTP
        let snapshot = if let Ok(mut st) = state.shared.lock() {
            if st.path_dirty && !state.ed_path.is_invalid() {
                set_window_text(state.ed_path, &st.current_dir);
                st.path_dirty = false;
            }
            st.clone()
        } else {
            let _ = SelectObject(hdc, old_bm);
            let _ = DeleteObject(hbm.into());
            let _ = DeleteDC(hdc);
            let _ = EndPaint(state.container_hwnd, &ps);
            return;
        };

        // Si un transfert vient de se terminer, actualiser le panneau local automatiquement
        let transfer_active_now = snapshot.transfer.is_some();
        if state.last_transfer_active && !transfer_active_now {
            refresh_local_entries(state);
        }
        state.last_transfer_active = transfer_active_now;

        // Fond global
        let _ = FillRect(hdc, &rc, state.palette.brush_bg_deep);

        // 1. Barre d'outils supérieure (Surface sidebar)
        let toolbar_rc = RECT {
            left: 0,
            top: 0,
            right: w,
            bottom: toolbar_h,
        };
        let _ = FillRect(hdc, &toolbar_rc, state.palette.brush_bg_sidebar);
        draw_horizontal_line(hdc, 0, w, toolbar_h - 1, col_border());

        let sel_local_list = selected_local_entries(state);
        let sel_remote_list: Vec<FtpEntry> = state
            .remote_selected
            .iter()
            .filter_map(|&i| snapshot.entries.get(i).cloned())
            .collect();
        let can_cdup = if state.active_pane == FtpPane::Local {
            !state.local_showing_drives
        } else {
            snapshot.is_connected
        };
        let can_upload = snapshot.is_connected;
        let can_download =
            sel_remote_list.iter().any(|e| e.kind == FtpEntryKind::File) && snapshot.is_connected;
        let can_mkdir = if state.active_pane == FtpPane::Local {
            !state.local_showing_drives
        } else {
            snapshot.is_connected
        };
        let active_items: Vec<&FtpEntry> = if state.active_pane == FtpPane::Local {
            sel_local_list
                .iter()
                .filter(|e| e.kind != FtpEntryKind::ParentDir && !state.local_showing_drives)
                .collect()
        } else {
            sel_remote_list
                .iter()
                .filter(|e| e.kind != FtpEntryKind::ParentDir)
                .collect()
        };
        let can_rename = active_items.len() == 1
            && (state.active_pane == FtpPane::Local || snapshot.is_connected);
        let can_delete = !active_items.is_empty()
            && (state.active_pane == FtpPane::Local || snapshot.is_connected);

        let btn_top = dpi.scale(7);
        let btn_h = dpi.scale(30);
        let gap = dpi.scale(6);
        let mut bx = pad_x;

        let toolbar_buttons = [
            (
                "↑ Parent",
                FtpUiZone::BtnCdup,
                dpi.scale(76),
                false,
                can_cdup,
            ),
            (
                "↻ Actualiser",
                FtpUiZone::BtnRefresh,
                dpi.scale(92),
                false,
                true,
            ),
            (
                "⇧ Téléverser",
                FtpUiZone::BtnUpload,
                dpi.scale(104),
                true,
                can_upload,
            ),
            (
                "⇩ Télécharger",
                FtpUiZone::BtnDownload,
                dpi.scale(108),
                false,
                can_download,
            ),
            (
                "+ Dossier",
                FtpUiZone::BtnMkdir,
                dpi.scale(84),
                false,
                can_mkdir,
            ),
            (
                "✎ Renommer",
                FtpUiZone::BtnRename,
                dpi.scale(94),
                false,
                can_rename,
            ),
            (
                "✕ Supprimer",
                FtpUiZone::BtnDelete,
                dpi.scale(94),
                false,
                can_delete,
            ),
        ];

        for (label, zone, bw, primary, enabled) in toolbar_buttons {
            if bx + bw > w - dpi.scale(210) {
                break;
            }
            let brc = RECT {
                left: bx,
                top: btn_top,
                right: bx + bw,
                bottom: btn_top + btn_h,
            };
            let is_hover = enabled && state.hovered_zone == Some(zone);
            let (fill, border, txt_col) = if !enabled {
                (col_bg_sidebar(), col_border(), col_text_muted())
            } else if primary {
                let bg = if is_hover {
                    col_accent_hover()
                } else {
                    col_accent()
                };
                (bg, bg, COL_TEXT_ON_ACCENT)
            } else if is_hover {
                (col_bg_hover(), col_accent(), col_text_primary())
            } else {
                (col_bg_surface(), col_border(), col_text_primary())
            };
            draw_rounded_button(
                hdc,
                brc,
                fill,
                border,
                label,
                if primary {
                    state.palette.font_ui_bold
                } else {
                    state.palette.font_ui
                },
                txt_col,
            );
            if enabled {
                state.hit_zones.push(FtpHitZone { rect: brc, zone });
            }
            bx += bw + gap;
        }

        // Boutons à droite : Identifiants… et Reconnecter
        let recon_w = dpi.scale(96);
        let cred_w = dpi.scale(102);
        let recon_rc = RECT {
            left: w - pad_x - recon_w,
            top: btn_top,
            right: w - pad_x,
            bottom: btn_top + btn_h,
        };
        let cred_rc = RECT {
            left: recon_rc.left - gap - cred_w,
            top: btn_top,
            right: recon_rc.left - gap,
            bottom: btn_top + btn_h,
        };

        for (label, brc, zone) in [
            ("Identifiants…", cred_rc, FtpUiZone::BtnCredentials),
            ("Reconnecter", recon_rc, FtpUiZone::BtnReconnect),
        ] {
            if brc.left > bx + dpi.scale(4) {
                let is_hover = state.hovered_zone == Some(zone);
                draw_rounded_button(
                    hdc,
                    brc,
                    if is_hover {
                        col_bg_hover()
                    } else {
                        col_bg_surface()
                    },
                    if is_hover { col_accent() } else { col_border() },
                    label,
                    state.palette.font_ui,
                    col_text_primary(),
                );
                state.hit_zones.push(FtpHitZone { rect: brc, zone });
            }
        }

        // 2. Barres de chemin (gauche = Local, droite = FTP)
        let path_top = toolbar_h;
        let path_bottom = toolbar_h + path_h;
        draw_horizontal_line(hdc, 0, w, path_bottom - 1, col_border());

        // Badge LOCAL (gauche)
        let local_badge_rc = RECT {
            left: pad_x,
            top: path_top + dpi.scale(10),
            right: pad_x + dpi.scale(48),
            bottom: path_bottom - dpi.scale(10),
        };
        draw_filled_badge(
            hdc,
            local_badge_rc,
            if state.active_pane == FtpPane::Local {
                col_accent()
            } else {
                COL_FOLDER_AMBER
            },
            "LOCAL",
            state.palette.font_small,
        );

        let local_focused = !state.ed_local_path.is_invalid() && GetFocus() == state.ed_local_path;
        draw_rounded_box(
            hdc,
            state.local_path_box_rc,
            col_bg_input(),
            if local_focused {
                col_accent()
            } else {
                col_border()
            },
        );
        let local_go_rc = RECT {
            left: state.local_path_box_rc.right + dpi.scale(6),
            top: path_top + dpi.scale(5),
            right: left_r - pad_x,
            bottom: path_bottom - dpi.scale(5),
        };
        let is_lgo_hover = state.hovered_zone == Some(FtpUiZone::BtnLocalGoPath);
        draw_rounded_button(
            hdc,
            local_go_rc,
            if is_lgo_hover {
                col_bg_hover()
            } else {
                col_bg_surface()
            },
            if is_lgo_hover {
                col_accent()
            } else {
                col_border()
            },
            "Aller",
            state.palette.font_ui,
            col_text_primary(),
        );
        state.hit_zones.push(FtpHitZone {
            rect: local_go_rc,
            zone: FtpUiZone::BtnLocalGoPath,
        });

        // Badge FTP + hôte (droite)
        let remote_badge_rc = RECT {
            left: right_l + pad_x,
            top: path_top + dpi.scale(10),
            right: right_l + pad_x + dpi.scale(38),
            bottom: path_bottom - dpi.scale(10),
        };
        draw_filled_badge(
            hdc,
            remote_badge_rc,
            if state.active_pane == FtpPane::Remote {
                col_accent()
            } else {
                COL_PROTO_FTP
            },
            "FTP",
            state.palette.font_small,
        );

        let info_left = remote_badge_rc.right + dpi.scale(8);
        let info_right = state.path_box_rc.left - dpi.scale(8);
        if info_right > info_left + dpi.scale(40) {
            let server_info = format!("{}@{}", snapshot.username, snapshot.host_label);
            let _ = SelectObject(hdc, state.palette.font_small.into());
            let _ = SetTextColor(hdc, COLORREF(col_text_muted()));
            draw_text_line(
                hdc,
                &server_info,
                RECT {
                    left: info_left,
                    top: path_top,
                    right: info_right,
                    bottom: path_bottom,
                },
            );
        }

        let focused = !state.ed_path.is_invalid() && GetFocus() == state.ed_path;
        draw_rounded_box(
            hdc,
            state.path_box_rc,
            col_bg_input(),
            if focused { col_accent() } else { col_border() },
        );

        let go_rc = RECT {
            left: state.path_box_rc.right + dpi.scale(6),
            top: path_top + dpi.scale(5),
            right: w - pad_x,
            bottom: path_bottom - dpi.scale(5),
        };
        let is_go_hover = state.hovered_zone == Some(FtpUiZone::BtnGoPath);
        draw_rounded_button(
            hdc,
            go_rc,
            if is_go_hover {
                col_bg_hover()
            } else {
                col_bg_surface()
            },
            if is_go_hover {
                col_accent()
            } else {
                col_border()
            },
            "Aller",
            state.palette.font_ui,
            col_text_primary(),
        );
        state.hit_zones.push(FtpHitZone {
            rect: go_rc,
            zone: FtpUiZone::BtnGoPath,
        });

        // 3. Double panneau d'exploration (gauche = local, droite = FTP)
        let row_h = dpi.scale(28);
        state.row_height = row_h;
        state.local_list_area_rc = RECT {
            left: 0,
            top: path_bottom + header_h,
            right: left_r,
            bottom: activity_top,
        };
        state.list_area_rc = RECT {
            left: right_l,
            top: path_bottom + header_h,
            right: w,
            bottom: activity_top,
        };

        // Panneau gauche : explorateur local
        let local_empty = state
            .local_error
            .clone()
            .unwrap_or_else(|| "Ce dossier local est vide.".to_string());
        let local_entries_snapshot = state.local_entries.clone();
        let mut local_scroll = state.local_scroll_offset;
        paint_file_list_pane(
            hdc,
            &state.palette,
            &theme,
            0,
            left_r,
            path_bottom,
            header_h,
            activity_top,
            row_h,
            &local_entries_snapshot,
            &state.local_selected,
            &mut local_scroll,
            state.hovered_zone,
            state.active_pane == FtpPane::Local,
            true,
            &local_empty,
            &mut state.hit_zones,
        );
        state.local_scroll_offset = local_scroll;

        // Panneau droit : explorateur FTP distant (ou état connexion / erreur)
        if snapshot.is_connecting {
            let hdr_rc = RECT {
                left: right_l,
                top: path_bottom,
                right: w,
                bottom: path_bottom + header_h,
            };
            let _ = FillRect(hdc, &hdr_rc, state.palette.brush_bg_surface);
            draw_horizontal_line(hdc, right_l, w, hdr_rc.bottom - 1, col_border());
            let _ = SelectObject(hdc, state.palette.font_ui.into());
            let _ = SetTextColor(hdc, COLORREF(col_text_muted()));
            draw_text_line(
                hdc,
                &format!("Connexion FTP en cours vers {}...", snapshot.host_label),
                RECT {
                    left: right_l + pad_x * 2,
                    top: state.list_area_rc.top + dpi.scale(28),
                    right: w - pad_x * 2,
                    bottom: state.list_area_rc.top + dpi.scale(56),
                },
            );
        } else if let Some(ref err_msg) = snapshot.error_banner
            && !snapshot.is_connected
        {
            let hdr_rc = RECT {
                left: right_l,
                top: path_bottom,
                right: w,
                bottom: path_bottom + header_h,
            };
            let _ = FillRect(hdc, &hdr_rc, state.palette.brush_bg_surface);
            draw_horizontal_line(hdc, right_l, w, hdr_rc.bottom - 1, col_border());

            let right_w = (w - right_l).max(160);
            let card_w = dpi.scale(420).min(right_w - pad_x * 2).max(140);
            let card_h = dpi.scale(148);
            let card_l = right_l + (right_w - card_w) / 2;
            let card_t = state.list_area_rc.top + dpi.scale(28);
            let card_rc = RECT {
                left: card_l,
                top: card_t,
                right: card_l + card_w,
                bottom: card_t + card_h,
            };
            draw_rounded_box(hdc, card_rc, col_bg_surface(), col_status_err());

            let _ = SelectObject(hdc, state.palette.font_ui_bold.into());
            let _ = SetTextColor(hdc, COLORREF(col_status_err()));
            draw_text_line(
                hdc,
                if snapshot.auth_failed {
                    "Authentification FTP refusée"
                } else {
                    "Connexion FTP interrompue ou impossible"
                },
                RECT {
                    left: card_rc.left + dpi.scale(16),
                    top: card_rc.top + dpi.scale(14),
                    right: card_rc.right - dpi.scale(16),
                    bottom: card_rc.top + dpi.scale(38),
                },
            );

            let _ = SelectObject(hdc, state.palette.font_ui.into());
            let _ = SetTextColor(hdc, COLORREF(col_text_primary()));
            draw_text_line(
                hdc,
                err_msg,
                RECT {
                    left: card_rc.left + dpi.scale(16),
                    top: card_rc.top + dpi.scale(44),
                    right: card_rc.right - dpi.scale(16),
                    bottom: card_rc.top + dpi.scale(74),
                },
            );

            let bw = dpi
                .scale(152)
                .min((card_w - dpi.scale(44)) / 2 + dpi.scale(28));
            let bh = dpi.scale(32);
            let b1_rc = RECT {
                left: card_rc.left + dpi.scale(16),
                top: card_rc.bottom - bh - dpi.scale(16),
                right: card_rc.left + dpi.scale(16) + bw,
                bottom: card_rc.bottom - dpi.scale(16),
            };
            let h1 = state.hovered_zone == Some(FtpUiZone::BtnErrorCredentials);
            draw_rounded_button(
                hdc,
                b1_rc,
                if h1 { col_accent_hover() } else { col_accent() },
                col_accent(),
                "Identifiants…",
                state.palette.font_ui_bold,
                COL_TEXT_ON_ACCENT,
            );
            state.hit_zones.push(FtpHitZone {
                rect: b1_rc,
                zone: FtpUiZone::BtnErrorCredentials,
            });

            let b2_rc = RECT {
                left: b1_rc.right + dpi.scale(10),
                top: b1_rc.top,
                right: (b1_rc.right + dpi.scale(10) + dpi.scale(116))
                    .min(card_rc.right - dpi.scale(12)),
                bottom: b1_rc.bottom,
            };
            let h2 = state.hovered_zone == Some(FtpUiZone::BtnErrorReconnect);
            draw_rounded_button(
                hdc,
                b2_rc,
                if h2 { col_bg_hover() } else { col_bg_deep() },
                if h2 { col_accent() } else { col_border() },
                "Réessayer",
                state.palette.font_ui,
                col_text_primary(),
            );
            state.hit_zones.push(FtpHitZone {
                rect: b2_rc,
                zone: FtpUiZone::BtnErrorReconnect,
            });
        } else {
            let mut remote_scroll = state.scroll_offset;
            paint_file_list_pane(
                hdc,
                &state.palette,
                &theme,
                right_l,
                w,
                path_bottom,
                header_h,
                activity_top,
                row_h,
                &snapshot.entries,
                &state.remote_selected,
                &mut remote_scroll,
                state.hovered_zone,
                state.active_pane == FtpPane::Remote,
                false,
                "Ce dossier distant est vide — Glissez-déposez un fichier ou double-cliquez à gauche.",
                &mut state.hit_zones,
            );
            state.scroll_offset = remote_scroll;
        }

        // Indicateur visuel de dépôt inter-panneaux (drag & drop façon FileZilla)
        if let (Some(src), Some(target)) = (state.dragging_transfer, state.drag_hover_target) {
            let (drop_rc, msg) = if src == FtpPane::Local && target == FtpPane::Remote {
                let count = selected_local_entries(state)
                    .into_iter()
                    .filter(|e| e.kind == FtpEntryKind::File)
                    .count();
                (
                    state.list_area_rc,
                    format!(
                        "Déposer ici pour téléverser {count} fichier(s) vers {}",
                        snapshot.current_dir
                    ),
                )
            } else {
                let count = selected_remote_entries(state)
                    .into_iter()
                    .filter(|e| e.kind == FtpEntryKind::File)
                    .count();
                (
                    state.local_list_area_rc,
                    format!(
                        "Déposer ici pour télécharger {count} fichier(s) vers {}",
                        state.local_dir.display()
                    ),
                )
            };
            let banner_rc = RECT {
                left: drop_rc.left + dpi.scale(14),
                top: (drop_rc.bottom - dpi.scale(42)).max(drop_rc.top + 8),
                right: drop_rc.right - dpi.scale(14),
                bottom: drop_rc.bottom - dpi.scale(10),
            };
            draw_rounded_box(hdc, banner_rc, col_accent(), col_accent_hover());
            let _ = SelectObject(hdc, state.palette.font_ui_bold.into());
            let _ = SetTextColor(hdc, COLORREF(COL_TEXT_ON_ACCENT));
            draw_text_line(
                hdc,
                &msg,
                RECT {
                    left: banner_rc.left + dpi.scale(12),
                    top: banner_rc.top,
                    right: banner_rc.right - dpi.scale(12),
                    bottom: banner_rc.bottom,
                },
            );
        }

        // Séparateur vertical draggable entre Local et FTP
        let vsplit_rc = RECT {
            left: left_r,
            top: toolbar_h,
            right: right_l,
            bottom: activity_top,
        };
        let v_active =
            state.dragging_pane_splitter || state.hovered_zone == Some(FtpUiZone::PaneSplitter);
        let v_brush =
            CreateSolidBrush(COLORREF(if v_active { col_accent() } else { col_border() }));
        let _ = FillRect(hdc, &vsplit_rc, v_brush);
        let _ = DeleteObject(v_brush.into());
        let grip_mid_y = (toolbar_h + activity_top) / 2;
        let v_grip = RECT {
            left: split_x - 1,
            top: grip_mid_y - dpi.scale(16),
            right: split_x + 2,
            bottom: grip_mid_y + dpi.scale(16),
        };
        draw_rounded_box(
            hdc,
            v_grip,
            if v_active {
                COL_TEXT_ON_ACCENT
            } else {
                col_text_muted()
            },
            if v_active {
                COL_TEXT_ON_ACCENT
            } else {
                col_text_muted()
            },
        );
        state.hit_zones.push(FtpHitZone {
            rect: state.pane_splitter_rc,
            zone: FtpUiZone::PaneSplitter,
        });

        // 4. Bandeau inférieur d'activité & logs (hauteur redimensionnable via drag)
        let act_rc = RECT {
            left: 0,
            top: activity_top,
            right: w,
            bottom: h,
        };
        let _ = FillRect(hdc, &act_rc, state.palette.brush_bg_sidebar);

        let h_active = state.dragging_activity_splitter
            || state.hovered_zone == Some(FtpUiZone::ActivitySplitter);
        draw_horizontal_line(
            hdc,
            0,
            w,
            activity_top,
            if h_active { col_accent() } else { col_border() },
        );
        if h_active {
            draw_horizontal_line(hdc, 0, w, activity_top + 1, col_accent());
        }
        // Poignée discrète centrée pour indiquer le redimensionnement vertical
        let h_grip = RECT {
            left: w / 2 - dpi.scale(18),
            top: activity_top + dpi.scale(2),
            right: w / 2 + dpi.scale(18),
            bottom: activity_top + dpi.scale(4),
        };
        draw_rounded_box(
            hdc,
            h_grip,
            if h_active {
                col_accent()
            } else {
                col_text_muted()
            },
            if h_active {
                col_accent()
            } else {
                col_text_muted()
            },
        );
        state.hit_zones.push(FtpHitZone {
            rect: state.activity_splitter_rc,
            zone: FtpUiZone::ActivitySplitter,
        });

        let act_head_y = activity_top + dpi.scale(7);
        let _ = SelectObject(hdc, state.palette.font_small.into());
        let _ = SetTextColor(hdc, COLORREF(col_text_primary()));
        draw_text_line(
            hdc,
            "Activité & transferts FTP",
            RECT {
                left: pad_x,
                top: act_head_y,
                right: pad_x + dpi.scale(175),
                bottom: act_head_y + dpi.scale(18),
            },
        );

        if let Some(ref tr) = snapshot.transfer {
            let cancel_w = dpi.scale(74);
            let cancel_rc = RECT {
                left: w - pad_x - cancel_w,
                top: act_head_y - dpi.scale(1),
                right: w - pad_x,
                bottom: act_head_y + dpi.scale(19),
            };
            let hc = state.hovered_zone == Some(FtpUiZone::BtnCancelTransfer);
            draw_rounded_button(
                hdc,
                cancel_rc,
                if hc {
                    col_status_err()
                } else {
                    col_bg_surface()
                },
                col_status_err(),
                "Annuler",
                state.palette.font_small,
                if hc {
                    COL_TEXT_ON_ACCENT
                } else {
                    col_text_primary()
                },
            );
            state.hit_zones.push(FtpHitZone {
                rect: cancel_rc,
                zone: FtpUiZone::BtnCancelTransfer,
            });

            let bar_right = cancel_rc.left - dpi.scale(10);
            let bar_left = (bar_right - dpi.scale(180)).max(pad_x + dpi.scale(180));
            if bar_right > bar_left + 40 {
                let track_rc = RECT {
                    left: bar_left,
                    top: act_head_y + dpi.scale(4),
                    right: bar_right,
                    bottom: act_head_y + dpi.scale(14),
                };
                draw_rounded_box(hdc, track_rc, col_bg_deep(), col_border());

                let pct = tr
                    .total
                    .filter(|&t| t > 0)
                    .map(|t| ((tr.transferred as f64 / t as f64).clamp(0.0, 1.0) * 100.0) as i32);
                let fill_w = match pct {
                    Some(p) => ((track_rc.right - track_rc.left) * p) / 100,
                    None => (track_rc.right - track_rc.left) / 3,
                };
                if fill_w > 2 {
                    let fill_rc = RECT {
                        left: track_rc.left,
                        top: track_rc.top,
                        right: (track_rc.left + fill_w).min(track_rc.right),
                        bottom: track_rc.bottom,
                    };
                    draw_rounded_box(hdc, fill_rc, col_accent(), col_accent());
                }

                let tr_lbl = match (pct, tr.total) {
                    (Some(p), Some(tot)) => format!(
                        "{} — {}% ({} / {})",
                        tr.label,
                        p,
                        format_file_size(tr.transferred),
                        format_file_size(tot)
                    ),
                    _ => format!("{} ({})", tr.label, format_file_size(tr.transferred)),
                };
                let _ = SetTextColor(hdc, COLORREF(col_accent_hover()));
                draw_text_right(
                    hdc,
                    &tr_lbl,
                    RECT {
                        left: pad_x + dpi.scale(175),
                        top: act_head_y,
                        right: bar_left - dpi.scale(10),
                        bottom: act_head_y + dpi.scale(18),
                    },
                );
            }
        }

        // Lignes du journal (s'adaptent automatiquement à la hauteur redimensionnée)
        let log_start_y = act_head_y + dpi.scale(22);
        let line_h = dpi.scale(16);
        let max_lines = ((h - log_start_y - dpi.scale(4)) / line_h).max(1) as usize;
        let skip = snapshot.logs.len().saturating_sub(max_lines);
        for (i, line) in snapshot.logs.iter().skip(skip).enumerate() {
            let ly = log_start_y + i as i32 * line_h;
            let txt_col = if line.is_error {
                col_status_err()
            } else if line.is_success {
                col_status_ok()
            } else {
                col_text_muted()
            };
            let _ = SetTextColor(hdc, COLORREF(txt_col));
            draw_text_line(
                hdc,
                &line.text,
                RECT {
                    left: pad_x,
                    top: ly,
                    right: w - pad_x,
                    bottom: ly + line_h,
                },
            );
        }

        let _ = BitBlt(hdc_screen, 0, 0, w, h, Some(hdc), 0, 0, SRCCOPY);
        let _ = SelectObject(hdc, old_bm);
        let _ = DeleteObject(hbm.into());
        let _ = DeleteDC(hdc);
        let _ = EndPaint(state.container_hwnd, &ps);
    }
}

unsafe extern "system" fn ftp_host_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut FtpWndState;
        match msg {
            WM_ERASEBKGND => LRESULT(1),
            WM_CTLCOLOREDIT => {
                if !ptr.is_null() {
                    let state = &*ptr;
                    let hdc = HDC(wparam.0 as *mut _);
                    let _ = SetTextColor(hdc, COLORREF(col_text_primary()));
                    let _ = SetBkColor(hdc, COLORREF(col_bg_input()));
                    return LRESULT(state.palette.brush_bg_input.0 as isize);
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            WM_SIZE => {
                if !ptr.is_null() {
                    let state = &mut *ptr;
                    let w = (lparam.0 & 0xFFFF) as i16 as i32;
                    let h = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;
                    ftp_layout_controls(state, w, h);
                    let _ = InvalidateRect(Some(hwnd), None, false);
                }
                LRESULT(0)
            }
            WM_FTP_INTERNAL_REFRESH => {
                let _ = InvalidateRect(Some(hwnd), None, false);
                LRESULT(0)
            }
            WM_FTP_OPEN_EDITOR => {
                if !ptr.is_null() {
                    let state = &*ptr;
                    let pending = state
                        .shared
                        .lock()
                        .ok()
                        .and_then(|mut st| st.pending_edit_file.take());
                    if let Some((remote_dir, remote_name, raw_bytes)) = pending
                        && let Some(edited_bytes) = crate::ui::run_ftp_file_editor_dialog(
                            state.main_hwnd,
                            &remote_dir,
                            &remote_name,
                            &raw_bytes,
                            &state.palette,
                        )
                    {
                        let _ = state.cmd_tx.send(FtpWorkerCommand::SaveEditedFile {
                            remote_dir,
                            remote_name,
                            content: edited_bytes,
                        });
                    }
                }
                LRESULT(0)
            }
            WM_FTP_OPEN_EXTERNAL_EDITOR => {
                if !ptr.is_null() {
                    let state = &mut *ptr;
                    let pending = state
                        .shared
                        .lock()
                        .ok()
                        .and_then(|mut st| st.pending_external_launch.take());
                    if let Some(launch) = pending {
                        if let Some(parent_dir) = launch.local_path.parent() {
                            state.external_temp_dirs.push(parent_dir.to_path_buf());
                        }
                        if let Err(err) = launch_external_editor(
                            state.main_hwnd,
                            &launch.local_path,
                            &launch.target,
                        ) {
                            show_error_dialog(state.main_hwnd, "Éditeur externe FTP", &err);
                        } else {
                            spawn_external_edit_watcher(
                                state.container_hwnd.0 as usize,
                                Arc::clone(&state.shared),
                                Arc::clone(&state.external_watch_stop),
                                launch.remote_dir,
                                launch.remote_name,
                                launch.local_path,
                                launch.initial_bytes,
                            );
                        }
                    }
                }
                LRESULT(0)
            }
            WM_FTP_EXTERNAL_FILE_CHANGED => {
                if !ptr.is_null() {
                    let state = &*ptr;
                    loop {
                        let next_change = state
                            .shared
                            .lock()
                            .ok()
                            .and_then(|mut st| st.pending_external_changes.pop_front());
                        let Some(change) = next_change else {
                            break;
                        };
                        let prompt = format!(
                            "Le fichier « {} » a été modifié dans l'éditeur externe.\n\nVoulez-vous téléverser les modifications sur le serveur FTP ({}) ?",
                            change.remote_name, change.remote_dir
                        );
                        if confirm_dialog(state.main_hwnd, "Fichier modifié localement", &prompt) {
                            let _ = state.cmd_tx.send(FtpWorkerCommand::SaveEditedFile {
                                remote_dir: change.remote_dir,
                                remote_name: change.remote_name,
                                content: change.new_bytes,
                            });
                        }
                    }
                }
                LRESULT(0)
            }
            WM_PAINT => {
                if !ptr.is_null() {
                    paint_ftp_host(&mut *ptr);
                    return LRESULT(0);
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            WM_SETCURSOR => {
                if !ptr.is_null() {
                    let state = &*ptr;
                    if state.dragging_transfer.is_some() {
                        let cursor_id = if state.drag_hover_target.is_some() {
                            IDC_HAND
                        } else {
                            IDC_ARROW
                        };
                        let _ = SetCursor(Some(LoadCursorW(None, cursor_id).unwrap_or_default()));
                        return LRESULT(1);
                    }
                    if state.dragging_pane_splitter
                        || state.hovered_zone == Some(FtpUiZone::PaneSplitter)
                    {
                        let _ = SetCursor(Some(LoadCursorW(None, IDC_SIZEWE).unwrap_or_default()));
                        return LRESULT(1);
                    }
                    if state.dragging_activity_splitter
                        || state.hovered_zone == Some(FtpUiZone::ActivitySplitter)
                    {
                        let _ = SetCursor(Some(LoadCursorW(None, IDC_SIZENS).unwrap_or_default()));
                        return LRESULT(1);
                    }
                    if let Some(zone) = state.hovered_zone {
                        match zone {
                            FtpUiZone::Row(_) | FtpUiZone::LocalRow(_) => {}
                            _ => {
                                let _ = SetCursor(Some(
                                    LoadCursorW(None, IDC_HAND).unwrap_or_default(),
                                ));
                                return LRESULT(1);
                            }
                        }
                    }
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            WM_MOUSEMOVE => {
                if !ptr.is_null() {
                    let state = &mut *ptr;
                    let x = (lparam.0 & 0xFFFF) as i16 as i32;
                    let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;

                    if state.dragging_pane_splitter || state.dragging_activity_splitter {
                        let mut rc = RECT::default();
                        let _ = GetClientRect(hwnd, &mut rc);
                        let w = (rc.right - rc.left).max(200);
                        let h = (rc.bottom - rc.top).max(150);
                        let dpi = state.palette.dpi;

                        if state.dragging_pane_splitter {
                            let sx = clamp_split_x(w, (x * 1000) / w.max(1), dpi.scale(200));
                            state.split_ratio_permille = ((sx * 1000) / w.max(1)).clamp(200, 800);
                            ftp_layout_controls(state, w, h);
                        }
                        if state.dragging_activity_splitter {
                            let fixed_top_h = dpi.scale(44 + 38 + 28 + 60);
                            state.activity_height =
                                clamp_activity_height(h, h - y, dpi.scale(54), fixed_top_h);
                        }
                        let _ = InvalidateRect(Some(hwnd), None, false);
                        return LRESULT(0);
                    }

                    if let Some((src_pane, _idx, start_pt, _)) = state.drag_candidate {
                        let dist = (x - start_pt.x).abs() + (y - start_pt.y).abs();
                        if dist >= 6 {
                            let has_files = match src_pane {
                                FtpPane::Local => {
                                    !state.local_showing_drives
                                        && selected_local_entries(state)
                                            .iter()
                                            .any(|e| e.kind == FtpEntryKind::File)
                                }
                                FtpPane::Remote => selected_remote_entries(state)
                                    .iter()
                                    .any(|e| e.kind == FtpEntryKind::File),
                            };
                            if has_files {
                                state.dragging_transfer = Some(src_pane);
                            }
                            state.drag_candidate = None;
                        }
                    }

                    if let Some(src_pane) = state.dragging_transfer {
                        let toolbar_h = state.palette.dpi.scale(44);
                        let in_body = y > toolbar_h && y < state.activity_splitter_rc.top;
                        let new_target = if in_body {
                            match src_pane {
                                FtpPane::Local
                                    if x >= state.pane_splitter_rc.right
                                        && state.shared.lock().is_ok_and(|s| s.is_connected) =>
                                {
                                    Some(FtpPane::Remote)
                                }
                                FtpPane::Remote
                                    if x < state.pane_splitter_rc.left
                                        && !state.local_showing_drives
                                        && state.local_dir.is_dir() =>
                                {
                                    Some(FtpPane::Local)
                                }
                                _ => None,
                            }
                        } else {
                            None
                        };
                        if new_target != state.drag_hover_target {
                            state.drag_hover_target = new_target;
                            let _ = InvalidateRect(Some(hwnd), None, false);
                        }
                        let cursor_id = if state.drag_hover_target.is_some() {
                            IDC_HAND
                        } else {
                            IDC_ARROW
                        };
                        let _ = SetCursor(Some(LoadCursorW(None, cursor_id).unwrap_or_default()));
                        return LRESULT(0);
                    }

                    let mut tme = TRACKMOUSEEVENT {
                        cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                        dwFlags: TME_LEAVE,
                        hwndTrack: hwnd,
                        dwHoverTime: 0,
                    };
                    let _ = TrackMouseEvent(&mut tme);
                    let new_hover = ftp_hit_test(state, x, y);
                    if new_hover != state.hovered_zone {
                        state.hovered_zone = new_hover;
                        let _ = InvalidateRect(Some(hwnd), None, false);
                    }
                }
                LRESULT(0)
            }
            WM_MOUSELEAVE => {
                if !ptr.is_null() {
                    let state = &mut *ptr;
                    if state.hovered_zone.take().is_some() {
                        let _ = InvalidateRect(Some(hwnd), None, false);
                    }
                }
                LRESULT(0)
            }
            WM_MOUSEWHEEL => {
                if !ptr.is_null() {
                    let state = &mut *ptr;
                    let delta = ((wparam.0 >> 16) & 0xFFFF) as i16 as i32;
                    let step = state.row_height.max(24) * 2;
                    let mut pt = POINT {
                        x: (lparam.0 & 0xFFFF) as i16 as i32,
                        y: ((lparam.0 >> 16) & 0xFFFF) as i16 as i32,
                    };
                    let _ = windows::Win32::Graphics::Gdi::ScreenToClient(hwnd, &mut pt);
                    let in_local = pt.x < state.pane_splitter_rc.left;
                    if in_local {
                        if delta < 0 {
                            state.local_scroll_offset =
                                state.local_scroll_offset.saturating_add(step);
                        } else {
                            state.local_scroll_offset = (state.local_scroll_offset - step).max(0);
                        }
                    } else if delta < 0 {
                        state.scroll_offset = state.scroll_offset.saturating_add(step);
                    } else {
                        state.scroll_offset = (state.scroll_offset - step).max(0);
                    }
                    let _ = InvalidateRect(Some(hwnd), None, false);
                }
                LRESULT(0)
            }
            WM_LBUTTONUP | WM_CAPTURECHANGED => {
                if !ptr.is_null() {
                    let state = &mut *ptr;
                    let mut changed = false;
                    if state.dragging_pane_splitter || state.dragging_activity_splitter {
                        state.dragging_pane_splitter = false;
                        state.dragging_activity_splitter = false;
                        changed = true;
                    }
                    if let Some(src_pane) = state.dragging_transfer.take() {
                        let target = state.drag_hover_target.take();
                        if msg == WM_LBUTTONUP {
                            if src_pane == FtpPane::Local && target == Some(FtpPane::Remote) {
                                trigger_ftp_upload_selected_only(state);
                            } else if src_pane == FtpPane::Remote && target == Some(FtpPane::Local)
                            {
                                trigger_ftp_download_to_local_or_prompt(state, false);
                            }
                        }
                        changed = true;
                    } else if let Some((pane, idx, _pt, was_plain_reclick)) =
                        state.drag_candidate.take()
                        && msg == WM_LBUTTONUP
                        && was_plain_reclick
                    {
                        match pane {
                            FtpPane::Local => {
                                state.local_selected.clear();
                                state.local_selected.insert(idx);
                                state.local_anchor_idx = Some(idx);
                            }
                            FtpPane::Remote => {
                                state.remote_selected.clear();
                                state.remote_selected.insert(idx);
                                state.remote_anchor_idx = Some(idx);
                            }
                        }
                        changed = true;
                    }
                    if msg == WM_LBUTTONUP {
                        let _ = ReleaseCapture();
                    }
                    if changed {
                        let _ = InvalidateRect(Some(hwnd), None, false);
                    }
                }
                LRESULT(0)
            }
            WM_LBUTTONDOWN => {
                if !ptr.is_null() {
                    let state = &mut *ptr;
                    let _ = SetFocus(Some(hwnd));
                    let x = (lparam.0 & 0xFFFF) as i16 as i32;
                    let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;
                    let ctrl = is_mod_key_down(VK_CONTROL.0);
                    let shift = is_mod_key_down(VK_SHIFT.0);
                    if let Some(zone) = ftp_hit_test(state, x, y) {
                        match zone {
                            FtpUiZone::PaneSplitter => {
                                state.dragging_pane_splitter = true;
                                SetCapture(hwnd);
                                let _ = InvalidateRect(Some(hwnd), None, false);
                            }
                            FtpUiZone::ActivitySplitter => {
                                state.dragging_activity_splitter = true;
                                SetCapture(hwnd);
                                let _ = InvalidateRect(Some(hwnd), None, false);
                            }
                            FtpUiZone::BtnCdup => {
                                if state.active_pane == FtpPane::Local {
                                    local_navigate_up(state);
                                    let _ = InvalidateRect(Some(hwnd), None, false);
                                } else {
                                    let _ = state.cmd_tx.send(FtpWorkerCommand::Cdup);
                                }
                            }
                            FtpUiZone::BtnRefresh => {
                                refresh_local_entries(state);
                                let _ = state.cmd_tx.send(FtpWorkerCommand::Refresh);
                                let _ = InvalidateRect(Some(hwnd), None, false);
                            }
                            FtpUiZone::BtnUpload => trigger_ftp_upload(state),
                            FtpUiZone::BtnDownload => {
                                trigger_ftp_download_to_local_or_prompt(state, false);
                            }
                            FtpUiZone::BtnMkdir => trigger_ftp_mkdir(state),
                            FtpUiZone::BtnRename => trigger_ftp_rename(state),
                            FtpUiZone::BtnDelete => trigger_ftp_delete(state),
                            FtpUiZone::BtnCredentials | FtpUiZone::BtnErrorCredentials => {
                                let _ = PostMessageW(
                                    Some(state.main_hwnd),
                                    WM_FTP_REQUEST_CREDENTIALS,
                                    WPARAM(state.session_id as usize),
                                    LPARAM(0),
                                );
                            }
                            FtpUiZone::BtnReconnect | FtpUiZone::BtnErrorReconnect => {
                                let _ = state.cmd_tx.send(FtpWorkerCommand::Connect);
                            }
                            FtpUiZone::BtnLocalGoPath => {
                                state.active_pane = FtpPane::Local;
                                let p = get_window_text(state.ed_local_path);
                                let trimmed = p.trim();
                                if trimmed.eq_ignore_ascii_case("Poste de travail")
                                    || trimmed.is_empty()
                                {
                                    state.local_showing_drives = true;
                                    refresh_local_entries(state);
                                    let _ = InvalidateRect(Some(hwnd), None, false);
                                } else {
                                    local_navigate_to(state, PathBuf::from(trimmed));
                                    let _ = InvalidateRect(Some(hwnd), None, false);
                                }
                            }
                            FtpUiZone::BtnGoPath => {
                                state.active_pane = FtpPane::Remote;
                                let p = get_window_text(state.ed_path);
                                let trimmed = p.trim().to_string();
                                if !trimmed.is_empty() {
                                    let _ = state.cmd_tx.send(FtpWorkerCommand::Cwd(trimmed));
                                }
                            }
                            FtpUiZone::BtnCancelTransfer => {
                                state.cancel_transfer.store(true, Ordering::Relaxed);
                            }
                            FtpUiZone::LocalRow(idx) => {
                                state.active_pane = FtpPane::Local;
                                let already_sel = state.local_selected.contains(&idx);
                                if !ctrl && !shift && already_sel && state.local_selected.len() > 1
                                {
                                    state.drag_candidate =
                                        Some((FtpPane::Local, idx, POINT { x, y }, true));
                                } else {
                                    apply_row_selection(
                                        &mut state.local_selected,
                                        &mut state.local_anchor_idx,
                                        idx,
                                        ctrl,
                                        shift,
                                        &state.local_entries,
                                    );
                                    state.drag_candidate =
                                        Some((FtpPane::Local, idx, POINT { x, y }, false));
                                }
                                SetCapture(hwnd);
                                let _ = InvalidateRect(Some(hwnd), None, false);
                            }
                            FtpUiZone::Row(idx) => {
                                state.active_pane = FtpPane::Remote;
                                let already_sel = state.remote_selected.contains(&idx);
                                if !ctrl && !shift && already_sel && state.remote_selected.len() > 1
                                {
                                    state.drag_candidate =
                                        Some((FtpPane::Remote, idx, POINT { x, y }, true));
                                } else {
                                    if let Ok(st) = state.shared.lock() {
                                        apply_row_selection(
                                            &mut state.remote_selected,
                                            &mut state.remote_anchor_idx,
                                            idx,
                                            ctrl,
                                            shift,
                                            &st.entries,
                                        );
                                    }
                                    state.drag_candidate =
                                        Some((FtpPane::Remote, idx, POINT { x, y }, false));
                                }
                                SetCapture(hwnd);
                                let _ = InvalidateRect(Some(hwnd), None, false);
                            }
                        }
                    } else if y > state.palette.dpi.scale(44) && y < state.activity_splitter_rc.top
                    {
                        state.active_pane = if x < state.pane_splitter_rc.left {
                            FtpPane::Local
                        } else {
                            FtpPane::Remote
                        };
                        let _ = InvalidateRect(Some(hwnd), None, false);
                    }
                }
                LRESULT(0)
            }
            WM_LBUTTONDBLCLK => {
                if !ptr.is_null() {
                    let state = &mut *ptr;
                    state.drag_candidate = None;
                    state.dragging_transfer = None;
                    state.drag_hover_target = None;
                    let x = (lparam.0 & 0xFFFF) as i16 as i32;
                    let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;
                    match ftp_hit_test(state, x, y) {
                        Some(FtpUiZone::LocalRow(idx)) => {
                            state.active_pane = FtpPane::Local;
                            state.local_selected.clear();
                            state.local_selected.insert(idx);
                            state.local_anchor_idx = Some(idx);
                            activate_local_entry(state, idx);
                        }
                        Some(FtpUiZone::Row(idx)) => {
                            state.active_pane = FtpPane::Remote;
                            state.remote_selected.clear();
                            state.remote_selected.insert(idx);
                            state.remote_anchor_idx = Some(idx);
                            activate_remote_entry(state, idx);
                        }
                        _ => {}
                    }
                }
                LRESULT(0)
            }
            WM_RBUTTONUP => {
                if !ptr.is_null() {
                    let state = &mut *ptr;
                    let x = (lparam.0 & 0xFFFF) as i16 as i32;
                    let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;
                    match ftp_hit_test(state, x, y) {
                        Some(FtpUiZone::LocalRow(idx)) => {
                            state.active_pane = FtpPane::Local;
                            if !state.local_selected.contains(&idx) {
                                state.local_selected.clear();
                                state.local_selected.insert(idx);
                                state.local_anchor_idx = Some(idx);
                            }
                            let _ = InvalidateRect(Some(hwnd), None, false);
                        }
                        Some(FtpUiZone::Row(idx)) => {
                            state.active_pane = FtpPane::Remote;
                            if !state.remote_selected.contains(&idx) {
                                state.remote_selected.clear();
                                state.remote_selected.insert(idx);
                                state.remote_anchor_idx = Some(idx);
                            }
                            let _ = InvalidateRect(Some(hwnd), None, false);
                        }
                        _ => {
                            state.active_pane = if x < state.pane_splitter_rc.left {
                                FtpPane::Local
                            } else {
                                FtpPane::Remote
                            };
                        }
                    }
                    let mut pt = POINT { x, y };
                    let _ = ClientToScreen(hwnd, &mut pt);
                    show_ftp_context_menu(state, pt);
                }
                LRESULT(0)
            }
            WM_CONTEXTMENU => {
                if !ptr.is_null() {
                    let state = &mut *ptr;
                    let mut pt = POINT::default();
                    let _ = GetCursorPos(&mut pt);
                    show_ftp_context_menu(state, pt);
                }
                LRESULT(0)
            }
            WM_KEYDOWN => {
                if !ptr.is_null() {
                    let state = &mut *ptr;
                    let vk = wparam.0 as u16;
                    let ctrl = is_mod_key_down(VK_CONTROL.0);
                    if ctrl && vk == b'A' as u16 {
                        if state.active_pane == FtpPane::Local {
                            state.local_selected.clear();
                            for (i, e) in state.local_entries.iter().enumerate() {
                                if e.kind != FtpEntryKind::ParentDir {
                                    state.local_selected.insert(i);
                                }
                            }
                        } else if let Ok(st) = state.shared.lock() {
                            state.remote_selected.clear();
                            for (i, e) in st.entries.iter().enumerate() {
                                if e.kind != FtpEntryKind::ParentDir {
                                    state.remote_selected.insert(i);
                                }
                            }
                        }
                        let _ = InvalidateRect(Some(hwnd), None, false);
                        return LRESULT(0);
                    }
                    if state.active_pane == FtpPane::Local {
                        let count = state.local_entries.len();
                        match vk {
                            x if x == VK_UP.0 && count > 0 => {
                                let next = state
                                    .local_anchor_idx
                                    .map(|i| i.saturating_sub(1))
                                    .unwrap_or(0);
                                state.local_selected.clear();
                                state.local_selected.insert(next);
                                state.local_anchor_idx = Some(next);
                                let _ = InvalidateRect(Some(hwnd), None, false);
                            }
                            x if x == VK_DOWN.0 && count > 0 => {
                                let next = state
                                    .local_anchor_idx
                                    .map(|i| (i + 1).min(count - 1))
                                    .unwrap_or(0);
                                state.local_selected.clear();
                                state.local_selected.insert(next);
                                state.local_anchor_idx = Some(next);
                                let _ = InvalidateRect(Some(hwnd), None, false);
                            }
                            x if x == VK_HOME.0 && count > 0 => {
                                state.local_selected.clear();
                                state.local_selected.insert(0);
                                state.local_anchor_idx = Some(0);
                                state.local_scroll_offset = 0;
                                let _ = InvalidateRect(Some(hwnd), None, false);
                            }
                            x if x == VK_END.0 && count > 0 => {
                                state.local_selected.clear();
                                state.local_selected.insert(count - 1);
                                state.local_anchor_idx = Some(count - 1);
                                let _ = InvalidateRect(Some(hwnd), None, false);
                            }
                            x if x == VK_RETURN.0 => {
                                if let Some(idx) = state.local_anchor_idx {
                                    activate_local_entry(state, idx);
                                }
                            }
                            x if x == VK_BACK.0 => {
                                local_navigate_up(state);
                                let _ = InvalidateRect(Some(hwnd), None, false);
                            }
                            x if x == VK_F5.0 => {
                                refresh_local_entries(state);
                                let _ = InvalidateRect(Some(hwnd), None, false);
                            }
                            x if x == VK_F2.0 => trigger_ftp_rename(state),
                            x if x == VK_DELETE.0 => trigger_ftp_delete(state),
                            _ => {}
                        }
                    } else {
                        let count = state.shared.lock().map(|s| s.entries.len()).unwrap_or(0);
                        match vk {
                            x if x == VK_UP.0 && count > 0 => {
                                let next = state
                                    .remote_anchor_idx
                                    .map(|i| i.saturating_sub(1))
                                    .unwrap_or(0);
                                state.remote_selected.clear();
                                state.remote_selected.insert(next);
                                state.remote_anchor_idx = Some(next);
                                let _ = InvalidateRect(Some(hwnd), None, false);
                            }
                            x if x == VK_DOWN.0 && count > 0 => {
                                let next = state
                                    .remote_anchor_idx
                                    .map(|i| (i + 1).min(count - 1))
                                    .unwrap_or(0);
                                state.remote_selected.clear();
                                state.remote_selected.insert(next);
                                state.remote_anchor_idx = Some(next);
                                let _ = InvalidateRect(Some(hwnd), None, false);
                            }
                            x if x == VK_HOME.0 && count > 0 => {
                                state.remote_selected.clear();
                                state.remote_selected.insert(0);
                                state.remote_anchor_idx = Some(0);
                                state.scroll_offset = 0;
                                let _ = InvalidateRect(Some(hwnd), None, false);
                            }
                            x if x == VK_END.0 && count > 0 => {
                                state.remote_selected.clear();
                                state.remote_selected.insert(count - 1);
                                state.remote_anchor_idx = Some(count - 1);
                                let _ = InvalidateRect(Some(hwnd), None, false);
                            }
                            x if x == VK_RETURN.0 => {
                                if let Some(idx) = state.remote_anchor_idx {
                                    activate_remote_entry(state, idx);
                                }
                            }
                            x if x == VK_BACK.0 => {
                                let _ = state.cmd_tx.send(FtpWorkerCommand::Cdup);
                            }
                            x if x == VK_F5.0 => {
                                let _ = state.cmd_tx.send(FtpWorkerCommand::Refresh);
                            }
                            x if x == VK_F2.0 => trigger_ftp_rename(state),
                            x if x == VK_DELETE.0 => trigger_ftp_delete(state),
                            _ => {}
                        }
                    }
                }
                LRESULT(0)
            }
            WM_COMMAND => {
                if !ptr.is_null() {
                    let state = &mut *ptr;
                    let id = wparam.0 & 0xFFFF;
                    let code = ((wparam.0 >> 16) & 0xFFFF) as u32;
                    if id == IDC_FTP_PATH_EDIT || id == IDC_FTP_LOCAL_PATH_EDIT {
                        if code == EN_SETFOCUS || code == EN_KILLFOCUS {
                            if code == EN_SETFOCUS {
                                state.active_pane = if id == IDC_FTP_LOCAL_PATH_EDIT {
                                    FtpPane::Local
                                } else {
                                    FtpPane::Remote
                                };
                            }
                            let _ = InvalidateRect(Some(hwnd), None, false);
                        } else if code == 0x1000 {
                            if id == IDC_FTP_LOCAL_PATH_EDIT {
                                state.active_pane = FtpPane::Local;
                                let p = get_window_text(state.ed_local_path);
                                let trimmed = p.trim();
                                if trimmed.eq_ignore_ascii_case("Poste de travail")
                                    || trimmed.is_empty()
                                {
                                    state.local_showing_drives = true;
                                    refresh_local_entries(state);
                                    let _ = InvalidateRect(Some(hwnd), None, false);
                                } else {
                                    local_navigate_to(state, PathBuf::from(trimmed));
                                    let _ = InvalidateRect(Some(hwnd), None, false);
                                }
                            } else {
                                state.active_pane = FtpPane::Remote;
                                let p = get_window_text(state.ed_path);
                                let trimmed = p.trim().to_string();
                                if !trimmed.is_empty() {
                                    let _ = state.cmd_tx.send(FtpWorkerCommand::Cwd(trimmed));
                                }
                            }
                            let _ = SetFocus(Some(hwnd));
                        }
                    }
                }
                LRESULT(0)
            }
            WM_DROPFILES => {
                if !ptr.is_null() {
                    let state = &*ptr;
                    let hdrop = HDROP(wparam.0 as *mut c_void);
                    let count = DragQueryFileW(hdrop, 0xFFFF_FFFF, None);
                    for i in 0..count {
                        let len = DragQueryFileW(hdrop, i, None);
                        if len > 0 {
                            let mut buf = vec![0u16; (len + 1) as usize];
                            let _ = DragQueryFileW(hdrop, i, Some(&mut buf));
                            let path_str = String::from_utf16_lossy(&buf[..len as usize]);
                            let p = PathBuf::from(path_str);
                            if p.is_file() {
                                let remote_name = p
                                    .file_name()
                                    .and_then(|n| n.to_str())
                                    .unwrap_or("fichier")
                                    .to_string();
                                let _ = state.cmd_tx.send(FtpWorkerCommand::Upload {
                                    local_path: p,
                                    remote_name,
                                });
                            }
                        } else {
                            show_error_dialog(
                                state.main_hwnd,
                                "Glisser-déposer FTP",
                                "Seuls les fichiers individuels peuvent être téléversés par glisser-déposer.",
                            );
                        }
                    }
                    DragFinish(hdrop);
                }
                LRESULT(0)
            }
            WM_DESTROY => {
                if !ptr.is_null() {
                    SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
                    let state = Box::from_raw(ptr);
                    state.external_watch_stop.store(true, Ordering::Relaxed);
                    for dir in &state.external_temp_dirs {
                        let _ = std::fs::remove_dir_all(dir);
                    }
                    drop(state);
                }
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}

/// Contrôleur Win32 d'un onglet FTP intégré hébergé dans la fenêtre principale.
pub struct FtpSessionHost {
    session_id: u32,
    container_hwnd: HWND,
    shared: Arc<Mutex<FtpSharedState>>,
    cmd_tx: mpsc::Sender<FtpWorkerCommand>,
    cancel_transfer: Arc<AtomicBool>,
}

impl FtpSessionHost {
    #[allow(clippy::too_many_arguments)]
    pub fn create(
        session_id: u32,
        parent_hwnd: HWND,
        host: String,
        port: u16,
        username: String,
        password: SecretString,
        initial_remote_dir: String,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
    ) -> Result<Self, String> {
        unsafe {
            let hinstance = GetModuleHandleW(None).unwrap_or_default();
            let class_name = w!("MalvisionFtpHostClass");
            let wc = WNDCLASSEXW {
                cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                style: windows::Win32::UI::WindowsAndMessaging::CS_DBLCLKS
                    | windows::Win32::UI::WindowsAndMessaging::CS_HREDRAW
                    | windows::Win32::UI::WindowsAndMessaging::CS_VREDRAW,
                lpfnWndProc: Some(ftp_host_wnd_proc),
                hInstance: hinstance.into(),
                hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
                lpszClassName: class_name,
                ..Default::default()
            };
            let _ = RegisterClassExW(&wc);

            let container_hwnd = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                class_name,
                w!("Malvision FTP"),
                WS_CHILD | WS_CLIPCHILDREN,
                x,
                y,
                width.max(200),
                height.max(150),
                Some(parent_hwnd),
                None,
                Some(hinstance.into()),
                None,
            )
            .map_err(|e| format!("Échec création conteneur FTP Win32 : {e}"))?;

            DragAcceptFiles(container_hwnd, true);

            let palette = GdiPalette::create_for_window(container_hwnd);
            let dpi = palette.dpi;

            let host_label = if port == 21 {
                host.clone()
            } else {
                format!("{host}:{port}")
            };

            let shared = Arc::new(Mutex::new(FtpSharedState {
                is_connected: false,
                is_connecting: true,
                is_busy: true,
                auth_failed: false,
                current_dir: if initial_remote_dir.trim().is_empty() {
                    "/".to_string()
                } else {
                    initial_remote_dir.trim().to_string()
                },
                host_label,
                username: if username.trim().is_empty() {
                    "anonymous".to_string()
                } else {
                    username.trim().to_string()
                },
                entries: Vec::new(),
                transfer: None,
                logs: Vec::new(),
                status_detail: format!("Connexion FTP en cours vers {host}:{port}..."),
                error_banner: None,
                path_dirty: true,
                pending_edit_file: None,
                pending_external_launch: None,
                pending_external_changes: VecDeque::new(),
            }));

            let subclass_edit = |edit_hwnd: HWND| {
                let old_proc = SetWindowLongPtrW(
                    edit_hwnd,
                    GWLP_WNDPROC,
                    ftp_path_edit_subclass_proc as *const () as usize as isize,
                );
                let _ = windows::Win32::UI::WindowsAndMessaging::SetPropW(
                    edit_hwnd,
                    PROP_FTP_OLD_EDIT_PROC,
                    Some(windows::Win32::Foundation::HANDLE(old_proc as *mut c_void)),
                );
                let _ = windows::Win32::UI::WindowsAndMessaging::SetPropW(
                    edit_hwnd,
                    PROP_FTP_HOST_HWND,
                    Some(windows::Win32::Foundation::HANDLE(container_hwnd.0)),
                );
            };

            let ed_local_path = create_dark_edit(
                container_hwnd,
                hinstance.into(),
                RECT {
                    left: dpi.scale(68),
                    top: dpi.scale(49),
                    right: dpi.scale(240),
                    bottom: dpi.scale(77),
                },
                IDC_FTP_LOCAL_PATH_EDIT,
                false,
                palette.font_ui,
                dpi,
            );
            subclass_edit(ed_local_path);

            let ed_path = create_dark_edit(
                container_hwnd,
                hinstance.into(),
                RECT {
                    left: dpi.scale(420),
                    top: dpi.scale(49),
                    right: (width - dpi.scale(70)).max(dpi.scale(520)),
                    bottom: dpi.scale(77),
                },
                IDC_FTP_PATH_EDIT,
                false,
                palette.font_ui,
                dpi,
            );
            subclass_edit(ed_path);

            let (cmd_tx, cmd_rx) = mpsc::channel();
            let cancel_transfer = Arc::new(AtomicBool::new(false));
            let local_dir = default_local_dir();

            let mut state = Box::new(FtpWndState {
                session_id,
                container_hwnd,
                main_hwnd: parent_hwnd,
                palette,
                shared: Arc::clone(&shared),
                cmd_tx: cmd_tx.clone(),
                cancel_transfer: Arc::clone(&cancel_transfer),
                ed_local_path,
                ed_path,
                local_path_box_rc: RECT::default(),
                path_box_rc: RECT::default(),
                local_list_area_rc: RECT::default(),
                list_area_rc: RECT::default(),
                pane_splitter_rc: RECT::default(),
                activity_splitter_rc: RECT::default(),
                split_ratio_permille: 500,
                activity_height: dpi.scale(104),
                dragging_pane_splitter: false,
                dragging_activity_splitter: false,
                active_pane: FtpPane::Remote,
                local_dir,
                local_showing_drives: false,
                local_entries: Vec::new(),
                local_error: None,
                local_path_dirty: true,
                local_selected: BTreeSet::new(),
                local_anchor_idx: None,
                local_scroll_offset: 0,
                row_height: dpi.scale(28),
                remote_selected: BTreeSet::new(),
                remote_anchor_idx: None,
                scroll_offset: 0,
                drag_candidate: None,
                dragging_transfer: None,
                drag_hover_target: None,
                last_transfer_active: false,
                hovered_zone: None,
                hit_zones: Vec::new(),
                external_watch_stop: Arc::new(AtomicBool::new(false)),
                external_temp_dirs: Vec::new(),
            });
            refresh_local_entries(&mut state);
            ftp_layout_controls(&mut state, width.max(200), height.max(150));

            SetWindowLongPtrW(container_hwnd, GWLP_USERDATA, Box::into_raw(state) as isize);

            spawn_ftp_worker(
                session_id,
                container_hwnd.0 as usize,
                parent_hwnd.0 as usize,
                host,
                port,
                username,
                password,
                initial_remote_dir,
                Arc::clone(&shared),
                Arc::clone(&cancel_transfer),
                cmd_rx,
            );

            let _ = cmd_tx.send(FtpWorkerCommand::Connect);

            Ok(Self {
                session_id,
                container_hwnd,
                shared,
                cmd_tx,
                cancel_transfer,
            })
        }
    }
    pub fn session_id(&self) -> u32 {
        self.session_id
    }

    pub fn latest_status_detail(&self) -> Option<String> {
        self.shared.lock().ok().map(|s| s.status_detail.clone())
    }

    pub fn set_visible(&self, visible: bool) {
        if !self.container_hwnd.is_invalid() {
            unsafe {
                let _ = ShowWindow(self.container_hwnd, if visible { SW_SHOW } else { SW_HIDE });
            }
        }
    }

    pub fn resize(&self, x: i32, y: i32, width: i32, height: i32) {
        if !self.container_hwnd.is_invalid() {
            unsafe {
                let _ = MoveWindow(
                    self.container_hwnd,
                    x,
                    y,
                    width.max(120),
                    height.max(120),
                    true,
                );
            }
        }
    }

    pub fn focus_control(&self) {
        if !self.container_hwnd.is_invalid() {
            unsafe {
                let _ = SetFocus(Some(self.container_hwnd));
            }
        }
    }

    pub fn refresh_theme(&self) {
        if self.container_hwnd.is_invalid() {
            return;
        }
        unsafe {
            let ptr = GetWindowLongPtrW(self.container_hwnd, GWLP_USERDATA) as *mut FtpWndState;
            if !ptr.is_null() {
                (*ptr).palette.rebuild_brushes();
            }
            let _ = RedrawWindow(
                Some(self.container_hwnd),
                None,
                None,
                RDW_INVALIDATE | RDW_ERASE | RDW_ALLCHILDREN | RDW_UPDATENOW,
            );
        }
    }

    pub fn reconnect_with_credentials(&self, username: String, password: SecretString) {
        let _ = self
            .cmd_tx
            .send(FtpWorkerCommand::ReconnectWithCredentials { username, password });
    }

    pub fn disconnect(&self) {
        self.cancel_transfer.store(true, Ordering::Relaxed);
        let _ = self.cmd_tx.send(FtpWorkerCommand::Quit);
    }
}

impl Drop for FtpSessionHost {
    fn drop(&mut self) {
        self.disconnect();
        if !self.container_hwnd.is_invalid() {
            unsafe {
                let _ = DestroyWindow(self.container_hwnd);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    #[test]
    fn test_sanitize_ftp_arg_rejects_crlf_and_nul() {
        assert!(sanitize_ftp_arg("/var/www/html").is_ok());
        assert!(sanitize_ftp_arg("fichier (final).pdf").is_ok());
        assert!(sanitize_ftp_arg("user\r\nQUIT").is_err());
        assert!(sanitize_ftp_arg("file\nDELE secret.txt").is_err());
        assert!(sanitize_ftp_arg("bad\0path").is_err());
    }

    #[test]
    fn test_parse_pasv_and_epsv_responses() {
        let peer = IpAddr::V4(Ipv4Addr::new(198, 51, 100, 10));
        let addr1 = parse_pasv_response("227 Entering Passive Mode (198,51,100,10,195,80).", peer)
            .expect("PASV standard");
        assert_eq!(addr1.ip(), IpAddr::V4(Ipv4Addr::new(198, 51, 100, 10)));
        assert_eq!(addr1.port(), (195 << 8) | 80);

        // Si le serveur derrière NAT annonce 0.0.0.0 ou une IP privée sur un pair public,
        // repli automatique vers l'IP de contrôle réelle :
        let addr_nat = parse_pasv_response("227 Entering Passive Mode (10,0,0,5,100,1).", peer)
            .expect("PASV NAT fallback");
        assert_eq!(addr_nat.ip(), peer);
        assert_eq!(addr_nat.port(), (100 << 8) | 1);

        let epsv_port = parse_epsv_response("229 Entering Extended Passive Mode (|||51234|)")
            .expect("EPSV valid");
        assert_eq!(epsv_port, 51234);
    }

    #[test]
    fn test_parse_pwd_mlsd_and_list_formats() {
        assert_eq!(
            parse_pwd_response(r#"257 "/var/www/mon site" is the current directory."#),
            Some("/var/www/mon site".to_string())
        );

        let mlsd_dir = parse_mlsd_line("type=dir;modify=20261116143000; public_html").unwrap();
        assert_eq!(mlsd_dir.name, "public_html");
        assert_eq!(mlsd_dir.kind, FtpEntryKind::Directory);
        assert_eq!(mlsd_dir.modified, "2026-11-16 14:30");

        let mlsd_file =
            parse_mlsd_line("type=file;size=4096;modify=20261116150000; index.html").unwrap();
        assert_eq!(mlsd_file.name, "index.html");
        assert_eq!(mlsd_file.kind, FtpEntryKind::File);
        assert_eq!(mlsd_file.size, Some(4096));

        let unix_dir = parse_list_line("drwxr-xr-x 2 deploy www 4096 Nov 16 14:00 logs").unwrap();
        assert_eq!(unix_dir.name, "logs");
        assert_eq!(unix_dir.kind, FtpEntryKind::Directory);

        let unix_file =
            parse_list_line("-rw-r--r-- 1 deploy www 1048576 Nov 16 14:05 Mon Rapport.pdf")
                .unwrap();
        assert_eq!(unix_file.name, "Mon Rapport.pdf");
        assert_eq!(unix_file.kind, FtpEntryKind::File);
        assert_eq!(unix_file.size, Some(1_048_576));

        let win_dir =
            parse_list_line("11-16-26  02:00PM       <DIR>          Sauvegardes").unwrap();
        assert_eq!(win_dir.name, "Sauvegardes");
        assert_eq!(win_dir.kind, FtpEntryKind::Directory);

        let win_file = parse_list_line("11-16-26  02:05PM               20480 data.zip").unwrap();
        assert_eq!(win_file.name, "data.zip");
        assert_eq!(win_file.kind, FtpEntryKind::File);
        assert_eq!(win_file.size, Some(20480));
    }

    #[test]
    fn test_format_file_size_units() {
        assert_eq!(format_file_size(512), "512 o");
        assert_eq!(format_file_size(2048), "2.0 Ko");
        assert_eq!(format_file_size(5 * 1024 * 1024), "5.0 Mo");
    }

    #[test]
    fn test_ftp_client_end_to_end_against_local_mock_server() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("Bind mock FTP");
        let port = listener.local_addr().unwrap().port();

        let server_thread = std::thread::spawn(move || {
            let (mut ctrl, _) = listener.accept().unwrap();
            let mut ctrl_reader = BufReader::new(ctrl.try_clone().unwrap());
            ctrl.write_all(b"220 Malvision Mock FTP Ready\r\n").unwrap();

            let mut cur_dir = "/".to_string();
            let mut uploaded_data: Vec<u8> = Vec::new();
            let mut pending_data_listener: Option<TcpListener> = None;

            loop {
                let mut line = String::new();
                if ctrl_reader.read_line(&mut line).unwrap_or(0) == 0 {
                    break;
                }
                let cmd_line = line.trim_end_matches(['\r', '\n']);
                let (cmd, arg) = cmd_line.split_once(' ').unwrap_or((cmd_line, ""));
                match cmd.to_ascii_uppercase().as_str() {
                    "USER" => {
                        ctrl.write_all(b"331 Password required\r\n").unwrap();
                    }
                    "PASS" => {
                        if arg == "secret123" {
                            ctrl.write_all(b"230 User logged in\r\n").unwrap();
                        } else {
                            ctrl.write_all(b"530 Login incorrect\r\n").unwrap();
                        }
                    }
                    "OPTS" => ctrl.write_all(b"200 UTF8 mode enabled\r\n").unwrap(),
                    "TYPE" => ctrl.write_all(b"200 Type set to I\r\n").unwrap(),
                    "PWD" => {
                        let resp = format!("257 \"{cur_dir}\" is current directory\r\n");
                        ctrl.write_all(resp.as_bytes()).unwrap();
                    }
                    "CWD" => {
                        cur_dir = arg.to_string();
                        ctrl.write_all(b"250 CWD command successful\r\n").unwrap();
                    }
                    "MKD" => {
                        ctrl.write_all(b"257 Directory created\r\n").unwrap();
                    }
                    "PASV" => {
                        let dl = TcpListener::bind("127.0.0.1:0").unwrap();
                        let dport = dl.local_addr().unwrap().port();
                        pending_data_listener = Some(dl);
                        let resp = format!(
                            "227 Entering Passive Mode (127,0,0,1,{},{}).\r\n",
                            dport >> 8,
                            dport & 0xFF
                        );
                        ctrl.write_all(resp.as_bytes()).unwrap();
                    }
                    "MLSD" => {
                        ctrl.write_all(b"500 Unknown command\r\n").unwrap();
                    }
                    "LIST" => {
                        let dl = pending_data_listener.take().unwrap();
                        ctrl.write_all(b"150 Opening ASCII mode data connection\r\n")
                            .unwrap();
                        let (mut ds, _) = dl.accept().unwrap();
                        ds.write_all(
                            b"drwxr-xr-x 2 user group 4096 Nov 16 14:00 uploads\r\n\
                              -rw-r--r-- 1 user group 15 Nov 16 14:01 hello.txt\r\n",
                        )
                        .unwrap();
                        drop(ds);
                        ctrl.write_all(b"226 Transfer complete\r\n").unwrap();
                    }
                    "STOR" => {
                        let dl = pending_data_listener.take().unwrap();
                        ctrl.write_all(b"150 Opening BINARY data connection\r\n")
                            .unwrap();
                        let (mut ds, _) = dl.accept().unwrap();
                        uploaded_data.clear();
                        ds.read_to_end(&mut uploaded_data).unwrap();
                        drop(ds);
                        ctrl.write_all(b"226 Transfer complete\r\n").unwrap();
                    }
                    "RETR" => {
                        let dl = pending_data_listener.take().unwrap();
                        ctrl.write_all(b"150 Opening BINARY data connection\r\n")
                            .unwrap();
                        let (mut ds, _) = dl.accept().unwrap();
                        ds.write_all(&uploaded_data).unwrap();
                        drop(ds);
                        ctrl.write_all(b"226 Transfer complete\r\n").unwrap();
                    }
                    "RNFR" => ctrl
                        .write_all(b"350 Ready for destination name\r\n")
                        .unwrap(),
                    "RNTO" => ctrl.write_all(b"250 Rename successful\r\n").unwrap(),
                    "DELE" => ctrl.write_all(b"250 File deleted\r\n").unwrap(),
                    "QUIT" => {
                        ctrl.write_all(b"221 Goodbye\r\n").unwrap();
                        break;
                    }
                    _ => ctrl.write_all(b"502 Command not implemented\r\n").unwrap(),
                }
            }
        });

        let secret = SecretString::new("secret123".to_string());
        let mut logs = Vec::new();
        let mut client =
            FtpClient::connect("127.0.0.1", port, "malvision_user", &secret, |_, l| {
                logs.push(l.to_string())
            })
            .expect("Connexion mock FTP réussie");

        assert!(logs.iter().all(|l| !l.contains("secret123")));
        let pwd = client.pwd().unwrap();
        assert_eq!(pwd, "/");

        let list = client.list(&pwd).unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].name, "uploads");
        assert_eq!(list[0].kind, FtpEntryKind::Directory);
        assert_eq!(list[1].name, "hello.txt");
        assert_eq!(list[1].kind, FtpEntryKind::File);

        assert_eq!(client.cwd("/uploads").unwrap(), "/uploads");
        client.mkdir("sub").unwrap();

        let temp_dir =
            std::env::temp_dir().join(format!("malvision_ftp_test_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);
        let src = temp_dir.join("up.bin");
        let dst = temp_dir.join("down.bin");
        let payload = b"MALVISION_FTP_PAYLOAD_2026_OK";
        std::fs::write(&src, payload).unwrap();

        let cancel = AtomicBool::new(false);
        let up_bytes = client
            .upload_file(&src, "up.bin", &cancel, |_, _| {})
            .unwrap();
        assert_eq!(up_bytes, payload.len() as u64);

        let dn_bytes = client
            .download_file("up.bin", &dst, Some(up_bytes), &cancel, |_, _| {})
            .unwrap();
        assert_eq!(dn_bytes, payload.len() as u64);
        assert_eq!(std::fs::read(&dst).unwrap(), payload);

        client.rename("up.bin", "renamed.bin").unwrap();
        client.delete_file("renamed.bin").unwrap();
        client.quit();

        let _ = std::fs::remove_dir_all(&temp_dir);
        let _ = server_thread.join();
    }

    #[test]
    fn test_local_explorer_directory_and_drives_listing() {
        let drives = list_windows_drives();
        assert!(
            !drives.is_empty(),
            "Au moins un lecteur Windows doit être détecté"
        );
        assert!(drives.iter().all(|d| d.kind == FtpEntryKind::Directory));

        let temp_dir =
            std::env::temp_dir().join(format!("malvision_local_exp_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(temp_dir.join("z_folder")).unwrap();
        std::fs::create_dir_all(temp_dir.join("a_folder")).unwrap();
        std::fs::write(temp_dir.join("b_file.txt"), b"hello").unwrap();

        let entries = read_local_directory(&temp_dir).unwrap();
        assert_eq!(entries[0].kind, FtpEntryKind::ParentDir);
        assert_eq!(entries[0].name, "..");
        assert_eq!(entries[1].name, "a_folder");
        assert_eq!(entries[1].kind, FtpEntryKind::Directory);
        assert_eq!(entries[2].name, "z_folder");
        assert_eq!(entries[2].kind, FtpEntryKind::Directory);
        assert_eq!(entries[3].name, "b_file.txt");
        assert_eq!(entries[3].kind, FtpEntryKind::File);
        assert_eq!(entries[3].size, Some(5));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_ftp_splitter_clamping_math() {
        // Largeur 1000px : 50% -> 500, bornes respectées
        assert_eq!(clamp_split_x(1000, 500, 200), 500);
        assert_eq!(clamp_split_x(1000, 50, 200), 200);
        assert_eq!(clamp_split_x(1000, 950, 200), 800);

        // Hauteur 700px, entête fixe 170px -> logs bornés entre 54 et 530
        assert_eq!(clamp_activity_height(700, 180, 54, 170), 180);
        assert_eq!(clamp_activity_height(700, 10, 54, 170), 54);
        assert_eq!(clamp_activity_height(700, 680, 54, 170), 530);
    }

    #[test]
    fn test_ftp_noop_and_single_parent_dir_and_lost_error_detection() {
        assert!(is_connection_lost_error("421 Idle timeout"));
        assert!(is_connection_lost_error(
            "Connexion FTP fermée par le serveur"
        ));
        assert!(!is_connection_lost_error("550 Permission denied"));
    }

    #[test]
    fn test_ftp_multi_selection_ctrl_and_shift() {
        let entries = vec![
            FtpEntry {
                name: "..".to_string(),
                kind: FtpEntryKind::ParentDir,
                size: None,
                modified: String::new(),
                permissions: String::new(),
            },
            FtpEntry {
                name: "dir1".to_string(),
                kind: FtpEntryKind::Directory,
                size: None,
                modified: String::new(),
                permissions: String::new(),
            },
            FtpEntry {
                name: "a.txt".to_string(),
                kind: FtpEntryKind::File,
                size: Some(10),
                modified: String::new(),
                permissions: String::new(),
            },
            FtpEntry {
                name: "b.txt".to_string(),
                kind: FtpEntryKind::File,
                size: Some(20),
                modified: String::new(),
                permissions: String::new(),
            },
        ];

        let mut sel = BTreeSet::new();
        let mut anchor = None;

        // Clic simple sur 1
        apply_row_selection(&mut sel, &mut anchor, 1, false, false, &entries);
        assert_eq!(sel.iter().copied().collect::<Vec<_>>(), vec![1]);

        // Shift+clic sur 3 -> plage 1..=3
        apply_row_selection(&mut sel, &mut anchor, 3, false, true, &entries);
        assert_eq!(sel.iter().copied().collect::<Vec<_>>(), vec![1, 2, 3]);

        // Ctrl+clic sur 2 -> retire 2
        apply_row_selection(&mut sel, &mut anchor, 2, true, false, &entries);
        assert_eq!(sel.iter().copied().collect::<Vec<_>>(), vec![1, 3]);

        // Ctrl+clic sur 2 -> rajoute 2
        apply_row_selection(&mut sel, &mut anchor, 2, true, false, &entries);
        assert_eq!(sel.iter().copied().collect::<Vec<_>>(), vec![1, 2, 3]);
    }

    #[test]
    fn test_ftp_client_download_and_upload_bytes_for_editor() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("Bind mock FTP");
        let port = listener.local_addr().unwrap().port();

        let server_thread = std::thread::spawn(move || {
            let (mut ctrl, _) = listener.accept().unwrap();
            let mut ctrl_reader = BufReader::new(ctrl.try_clone().unwrap());
            ctrl.write_all(b"220 Mock FTP Ready\r\n").unwrap();

            let mut file_content: Vec<u8> = b"hello=world\n".to_vec();
            let mut pending_data_listener: Option<TcpListener> = None;

            loop {
                let mut line = String::new();
                if ctrl_reader.read_line(&mut line).unwrap_or(0) == 0 {
                    break;
                }
                let cmd_line = line.trim_end_matches(['\r', '\n']);
                let (cmd, _arg) = cmd_line.split_once(' ').unwrap_or((cmd_line, ""));
                match cmd.to_ascii_uppercase().as_str() {
                    "USER" => ctrl.write_all(b"331 Password required\r\n").unwrap(),
                    "PASS" => ctrl.write_all(b"230 Logged in\r\n").unwrap(),
                    "OPTS" | "TYPE" => ctrl.write_all(b"200 OK\r\n").unwrap(),
                    "SIZE" => {
                        let resp = format!("213 {}\r\n", file_content.len());
                        ctrl.write_all(resp.as_bytes()).unwrap();
                    }
                    "PASV" => {
                        let dl = TcpListener::bind("127.0.0.1:0").unwrap();
                        let dport = dl.local_addr().unwrap().port();
                        pending_data_listener = Some(dl);
                        let resp = format!(
                            "227 Entering Passive Mode (127,0,0,1,{},{}).\r\n",
                            dport >> 8,
                            dport & 0xFF
                        );
                        ctrl.write_all(resp.as_bytes()).unwrap();
                    }
                    "RETR" => {
                        let dl = pending_data_listener.take().unwrap();
                        ctrl.write_all(b"150 Opening data connection\r\n").unwrap();
                        let (mut ds, _) = dl.accept().unwrap();
                        ds.write_all(&file_content).unwrap();
                        drop(ds);
                        ctrl.write_all(b"226 Transfer complete\r\n").unwrap();
                    }
                    "STOR" => {
                        let dl = pending_data_listener.take().unwrap();
                        ctrl.write_all(b"150 Ready for STOR\r\n").unwrap();
                        let (mut ds, _) = dl.accept().unwrap();
                        let mut buf = Vec::new();
                        ds.read_to_end(&mut buf).unwrap();
                        file_content = buf;
                        ctrl.write_all(b"226 Transfer complete\r\n").unwrap();
                    }
                    "QUIT" => {
                        ctrl.write_all(b"221 Bye\r\n").unwrap();
                        break;
                    }
                    _ => ctrl.write_all(b"500 Unknown\r\n").unwrap(),
                }
            }
            file_content
        });

        let pass = SecretString::new("pass".to_string());
        let mut client = FtpClient::connect("127.0.0.1", port, "user", &pass, |_, _| {})
            .expect("Connect mock FTP");
        let cancel = AtomicBool::new(false);

        // Trop volumineux -> refusé avant RETR
        assert!(client.download_bytes("config.ini", 4, &cancel).is_err());

        // Lecture en mémoire -> édition -> écrasement sur le serveur
        let initial = client
            .download_bytes("config.ini", MAX_EDITABLE_FILE_BYTES, &cancel)
            .expect("download_bytes");
        assert_eq!(initial, b"hello=world\n");

        let written = client
            .upload_bytes(b"hello=malvision\n", "config.ini", &cancel)
            .expect("upload_bytes");
        assert_eq!(written, 16);

        let reread = client
            .download_bytes("config.ini", MAX_EDITABLE_FILE_BYTES, &cancel)
            .expect("re-download");
        assert_eq!(reread, b"hello=malvision\n");

        client.quit();
        let final_server_bytes = server_thread.join().unwrap();
        assert_eq!(final_server_bytes, b"hello=malvision\n");
    }

    #[test]
    fn test_extract_exe_path_and_discover_external_editors() {
        assert_eq!(
            extract_exe_path_from_command(r#""C:\Program Files\Notepad++\notepad++.exe" "%1""#),
            Some(PathBuf::from(r"C:\Program Files\Notepad++\notepad++.exe"))
        );
        assert_eq!(
            extract_exe_path_from_command(r"C:\Windows\System32\notepad.exe %1"),
            Some(PathBuf::from(r"C:\Windows\System32\notepad.exe"))
        );
        assert_eq!(extract_exe_path_from_command(""), None);

        let editors = discover_external_editors("index.php");
        assert!(
            editors
                .iter()
                .any(|e| e.display_name.contains("Bloc-notes")),
            "Bloc-notes doit toujours être détecté sur Windows"
        );
    }
}
