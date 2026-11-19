//! Client SSH intégré avec pseudo-console Windows native (ConPTY + `ssh.exe`),
//! authentification DPAPI sécurisée via `SSH_ASKPASS` et terminal VT100/ANSI GDI natif.

use crate::security::{SecretString, zeroize_string};
use crate::ui::{
    COL_PROTO_SSH, COL_TEXT_ON_ACCENT, GdiPalette, active_theme_colors, col_accent,
    col_accent_hover, col_bg_deep, col_bg_hover, col_bg_surface, col_border, col_status_err,
    col_status_ok, col_text_primary, draw_filled_badge, draw_horizontal_line, draw_rounded_box,
    draw_rounded_button, draw_status_dot, draw_text_line,
};
use std::collections::VecDeque;
use std::ffi::c_void;
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use windows::Win32::Foundation::{
    COLORREF, CloseHandle, HANDLE, HGLOBAL, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM,
};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, CreateFontW, CreateSolidBrush,
    DEFAULT_CHARSET, DEFAULT_PITCH, DeleteDC, DeleteObject, EndPaint, FF_MODERN,
    FONT_CLIP_PRECISION, FONT_OUTPUT_PRECISION, FONT_QUALITY, FillRect, GetTextExtentPoint32W,
    HFONT, InvalidateRect, PAINTSTRUCT, RDW_ALLCHILDREN, RDW_ERASE, RDW_INVALIDATE, RDW_UPDATENOW,
    RedrawWindow, SRCCOPY, SelectObject, SetBkMode, SetTextColor, TRANSPARENT, TextOutW,
};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_FLAGS_AND_ATTRIBUTES, FILE_GENERIC_READ,
    FILE_SHARE_NONE, FlushFileBuffers, OPEN_EXISTING, ReadFile, WriteFile,
};
use windows::Win32::System::Console::{
    COORD, ClosePseudoConsole, CreatePseudoConsole, HPCON, ResizePseudoConsole,
};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, GetClipboardData, OpenClipboard, SetClipboardData,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Memory::{
    GLOBAL_ALLOC_FLAGS, GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock,
};
use windows::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, CreatePipe, DisconnectNamedPipe, NAMED_PIPE_MODE,
};
use windows::Win32::System::Threading::{
    CREATE_UNICODE_ENVIRONMENT, CreateProcessW, DeleteProcThreadAttributeList,
    EXTENDED_STARTUPINFO_PRESENT, InitializeProcThreadAttributeList, LPPROC_THREAD_ATTRIBUTE_LIST,
    PROCESS_INFORMATION, STARTUPINFOEXW, STARTUPINFOW, TerminateProcess, UpdateProcThreadAttribute,
    WaitForSingleObject,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, ReleaseCapture, SetCapture, SetFocus, TME_LEAVE, TRACKMOUSEEVENT, TrackMouseEvent,
    VK_BACK, VK_CONTROL, VK_DELETE, VK_DOWN, VK_END, VK_ESCAPE, VK_F1, VK_F12, VK_HOME, VK_INSERT,
    VK_LEFT, VK_NEXT, VK_PRIOR, VK_RETURN, VK_RIGHT, VK_SHIFT, VK_TAB, VK_UP,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyMenu, DestroyWindow,
    GWLP_USERDATA, GetClientRect, GetCursorPos, GetWindowLongPtrW, IDC_ARROW, IDC_HAND, IDC_IBEAM,
    LoadCursorW, MF_SEPARATOR, MF_STRING, MoveWindow, PostMessageW, RegisterClassExW, SW_HIDE,
    SW_SHOW, SetCursor, SetForegroundWindow, SetWindowLongPtrW, ShowWindow, TPM_LEFTALIGN,
    TPM_RETURNCMD, TPM_TOPALIGN, TrackPopupMenu, WINDOW_EX_STYLE, WM_APP, WM_CAPTURECHANGED,
    WM_CHAR, WM_CONTEXTMENU, WM_DESTROY, WM_ERASEBKGND, WM_KEYDOWN, WM_LBUTTONDOWN, WM_LBUTTONUP,
    WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_PAINT, WM_RBUTTONUP, WM_SETCURSOR, WM_SIZE, WNDCLASSEXW,
    WS_CHILD, WS_CLIPCHILDREN,
};
use windows_core::{PCWSTR, PWSTR, w};

// =============================================================================
// MESSAGES & ÉVÉNEMENTS VERS LA FENÊTRE PRINCIPALE
// =============================================================================

const WM_MOUSELEAVE: u32 = 0x02A3;
const PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE: usize = 0x0002_0016;
const PIPE_ACCESS_OUTBOUND: u32 = 0x0000_0002;
const PIPE_TYPE_BYTE: u32 = 0x0000_0000;
const PIPE_WAIT: u32 = 0x0000_0000;
const CF_UNICODETEXT_FMT: u32 = 13;

pub const WM_SSH_EVENT: u32 = WM_APP + 130;
const WM_SSH_INTERNAL_REFRESH: u32 = WM_APP + 131;

pub const SSH_EVT_CONNECTING: usize = 1;
pub const SSH_EVT_CONNECTED: usize = 2;
pub const SSH_EVT_ERROR: usize = 3;
pub const SSH_EVT_DISCONNECTED: usize = 4;

pub fn pack_ssh_event_wparam(session_id: u32, event_code: usize) -> WPARAM {
    WPARAM(((session_id as usize) << 8) | (event_code & 0xFF))
}

pub fn unpack_ssh_event_wparam(wparam: WPARAM) -> (u32, usize) {
    let raw = wparam.0;
    ((raw >> 8) as u32, raw & 0xFF)
}

/// Point d'entrée auxiliaire silencieux `SSH_ASKPASS` pour `ssh.exe` :
/// lit le mot de passe depuis le pipe nommé éphémère et l'écrit sur `stdout` sans GUI.
pub fn handle_ssh_askpass_if_requested() -> Option<i32> {
    let pipe_name = std::env::var("MALVISION_SSH_ASKPASS_PIPE").ok()?;
    if pipe_name.trim().is_empty() {
        return None;
    }

    let wide_pipe: Vec<u16> = pipe_name.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        let h_pipe = CreateFileW(
            PCWSTR(wide_pipe.as_ptr()),
            FILE_GENERIC_READ.0,
            FILE_SHARE_NONE,
            None,
            OPEN_EXISTING,
            FILE_ATTRIBUTE_NORMAL,
            None,
        );
        let Ok(handle) = h_pipe else {
            return Some(1);
        };

        let mut buf = [0u8; 1024];
        let mut read_bytes = 0u32;
        let ok = ReadFile(handle, Some(&mut buf), Some(&mut read_bytes), None).is_ok();
        let _ = CloseHandle(handle);

        if ok && read_bytes > 0 {
            let mut stdout = std::io::stdout().lock();
            let _ = stdout.write_all(&buf[..read_bytes as usize]);
            let _ = stdout.flush();
        }
        for b in &mut buf {
            std::ptr::write_volatile(b, 0);
        }
        Some(if ok { 0 } else { 1 })
    }
}

/// Localise l'exécutable OpenSSH client natif de Windows (`ssh.exe`).
pub fn find_windows_ssh_exe() -> Result<PathBuf, String> {
    let sys_root = std::env::var_os("SystemRoot")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Windows"));
    let candidates = [
        sys_root.join(r"System32\OpenSSH\ssh.exe"),
        sys_root.join(r"Sysnative\OpenSSH\ssh.exe"),
        PathBuf::from(r"C:\Program Files\OpenSSH\ssh.exe"),
    ];
    for c in candidates {
        if c.is_file() {
            return Ok(c);
        }
    }
    Err(
        "Client OpenSSH Windows (ssh.exe) introuvable dans System32\\OpenSSH. \
         Activez la fonctionnalité facultative « Client OpenSSH » dans Windows 11."
            .to_string(),
    )
}

/// Construit les arguments sécurisés pour `ssh.exe` (sans aucun mot de passe en clair).
pub fn build_ssh_args(
    host: &str,
    port: u16,
    username: &str,
    identity_file: &str,
) -> Result<Vec<String>, String> {
    let clean_host = host.trim();
    if clean_host.is_empty()
        || clean_host
            .chars()
            .any(|c| c.is_whitespace() || matches!(c, '\r' | '\n' | '\0'))
    {
        return Err("Hôte SSH invalide.".to_string());
    }
    if port == 0 {
        return Err("Port SSH invalide.".to_string());
    }

    let mut args = vec![
        "-p".to_string(),
        port.to_string(),
        "-o".to_string(),
        "ServerAliveInterval=25".to_string(),
        "-o".to_string(),
        "ServerAliveCountMax=3".to_string(),
    ];

    let clean_key = identity_file.trim();
    if !clean_key.is_empty() {
        if clean_key.chars().any(|c| matches!(c, '\r' | '\n' | '\0')) {
            return Err("Chemin de clé privée SSH invalide.".to_string());
        }
        args.push("-i".to_string());
        args.push(clean_key.to_string());
    }

    let clean_user = username.trim();
    if !clean_user.is_empty() {
        if clean_user
            .chars()
            .any(|c| c.is_whitespace() || matches!(c, '\r' | '\n' | '\0'))
        {
            return Err("Nom d'utilisateur SSH invalide.".to_string());
        }
        args.push(format!("{clean_user}@{clean_host}"));
    } else {
        args.push(clean_host.to_string());
    }

    Ok(args)
}

fn quote_win32_arg(arg: &str) -> String {
    if !arg.is_empty() && !arg.chars().any(|c| matches!(c, ' ' | '\t' | '"')) {
        return arg.to_string();
    }
    let mut out = String::with_capacity(arg.len() + 2);
    out.push('"');
    let mut backslashes = 0usize;
    for ch in arg.chars() {
        if ch == '\\' {
            backslashes += 1;
        } else if ch == '"' {
            out.push_str(&"\\".repeat(backslashes * 2 + 1));
            backslashes = 0;
            out.push('"');
        } else {
            out.push_str(&"\\".repeat(backslashes));
            backslashes = 0;
            out.push(ch);
        }
    }
    out.push_str(&"\\".repeat(backslashes * 2));
    out.push('"');
    out
}

// =============================================================================
// PARSEUR & GRILLE TERMINAL VT100 / ANSI
// =============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VtCell {
    pub ch: char,
    pub fg: u32,
    pub bg: u32,
    pub bold: bool,
}

impl VtCell {
    fn blank(fg: u32, bg: u32) -> Self {
        Self {
            ch: ' ',
            fg,
            bg,
            bold: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VtParseState {
    Ground,
    Escape,
    Csi,
    Osc,
    OscEsc,
}

pub struct VtTerminal {
    pub cols: usize,
    pub rows: usize,
    pub default_fg: u32,
    pub default_bg: u32,
    cells: Vec<VtCell>,
    alt_cells: Option<Vec<VtCell>>,
    scrollback: VecDeque<Vec<VtCell>>,
    pub view_scroll: usize,
    pub cursor_x: usize,
    pub cursor_y: usize,
    pub cursor_visible: bool,
    saved_cursor: (usize, usize),
    scroll_top: usize,
    scroll_bottom: usize,
    cur_fg: u32,
    cur_bg: u32,
    cur_bold: bool,
    cur_reverse: bool,
    state: VtParseState,
    csi_buf: String,
    utf8_buf: Vec<u8>,
    pub title: String,
}

const ANSI_16_BGR: [u32; 16] = [
    0x001E1C1A, // 0 Black
    0x005252E0, // 1 Red
    0x007EC84E, // 2 Green
    0x004BB5E6, // 3 Yellow
    0x00E09A3E, // 4 Blue
    0x00C66BC4, // 5 Magenta
    0x00C8B838, // 6 Cyan
    0x00D8D4D0, // 7 White
    0x0078726C, // 8 Bright Black
    0x007878F8, // 9 Bright Red
    0x009CE46E, // 10 Bright Green
    0x0072D0F8, // 11 Bright Yellow
    0x00F8B862, // 12 Bright Blue
    0x00E48AE0, // 13 Bright Magenta
    0x00E8DC58, // 14 Bright Cyan
    0x00FAF8F6, // 15 Bright White
];

fn rgb_to_colorref(r: u8, g: u8, b: u8) -> u32 {
    u32::from(r) | (u32::from(g) << 8) | (u32::from(b) << 16)
}

fn xterm_256_to_colorref(idx: u8) -> u32 {
    if idx < 16 {
        return ANSI_16_BGR[idx as usize];
    }
    if idx >= 232 {
        let gray = 8 + (idx - 232) * 10;
        return rgb_to_colorref(gray, gray, gray);
    }
    let n = idx - 16;
    let r = n / 36;
    let g = (n % 36) / 6;
    let b = n % 6;
    let map = |v: u8| if v == 0 { 0 } else { 55 + v * 40 };
    rgb_to_colorref(map(r), map(g), map(b))
}

impl VtTerminal {
    pub fn new(cols: usize, rows: usize, default_fg: u32, default_bg: u32) -> Self {
        let c = cols.max(20);
        let r = rows.max(5);
        Self {
            cols: c,
            rows: r,
            default_fg,
            default_bg,
            cells: vec![VtCell::blank(default_fg, default_bg); c * r],
            alt_cells: None,
            scrollback: VecDeque::with_capacity(1500),
            view_scroll: 0,
            cursor_x: 0,
            cursor_y: 0,
            cursor_visible: true,
            saved_cursor: (0, 0),
            scroll_top: 0,
            scroll_bottom: r - 1,
            cur_fg: default_fg,
            cur_bg: default_bg,
            cur_bold: false,
            cur_reverse: false,
            state: VtParseState::Ground,
            csi_buf: String::new(),
            utf8_buf: Vec::with_capacity(4),
            title: String::new(),
        }
    }

    pub fn update_theme_colors(&mut self, new_fg: u32, new_bg: u32) {
        let old_fg = self.default_fg;
        let old_bg = self.default_bg;
        self.default_fg = new_fg;
        self.default_bg = new_bg;
        if self.cur_fg == old_fg {
            self.cur_fg = new_fg;
        }
        if self.cur_bg == old_bg {
            self.cur_bg = new_bg;
        }
        for cell in &mut self.cells {
            if cell.fg == old_fg {
                cell.fg = new_fg;
            }
            if cell.bg == old_bg {
                cell.bg = new_bg;
            }
        }
        for line in &mut self.scrollback {
            for cell in line {
                if cell.fg == old_fg {
                    cell.fg = new_fg;
                }
                if cell.bg == old_bg {
                    cell.bg = new_bg;
                }
            }
        }
    }

    pub fn resize(&mut self, new_cols: usize, new_rows: usize) {
        let nc = new_cols.max(20);
        let nr = new_rows.max(5);
        if nc == self.cols && nr == self.rows {
            return;
        }
        let mut next = vec![VtCell::blank(self.default_fg, self.default_bg); nc * nr];
        let copy_rows = self.rows.min(nr);
        let copy_cols = self.cols.min(nc);
        for y in 0..copy_rows {
            for x in 0..copy_cols {
                next[y * nc + x] = self.cells[y * self.cols + x];
            }
        }
        self.cells = next;
        self.cols = nc;
        self.rows = nr;
        self.cursor_x = self.cursor_x.min(nc - 1);
        self.cursor_y = self.cursor_y.min(nr - 1);
        self.scroll_top = 0;
        self.scroll_bottom = nr - 1;
    }

    pub fn clear_all(&mut self) {
        self.cells
            .fill(VtCell::blank(self.default_fg, self.default_bg));
        self.scrollback.clear();
        self.view_scroll = 0;
        self.cursor_x = 0;
        self.cursor_y = 0;
    }

    pub fn total_scrollback(&self) -> usize {
        if self.alt_cells.is_some() {
            0
        } else {
            self.scrollback.len()
        }
    }

    pub fn visible_cell(&self, screen_y: usize, x: usize) -> VtCell {
        if x >= self.cols || screen_y >= self.rows {
            return VtCell::blank(self.default_fg, self.default_bg);
        }
        let sb_len = if self.alt_cells.is_some() {
            0
        } else {
            self.scrollback.len()
        };
        let scroll = self.view_scroll.min(sb_len);
        if scroll == 0 {
            return self.cells[screen_y * self.cols + x];
        }
        let virtual_idx = sb_len.saturating_sub(scroll) + screen_y;
        if virtual_idx < sb_len {
            self.scrollback[virtual_idx]
                .get(x)
                .copied()
                .unwrap_or_else(|| VtCell::blank(self.default_fg, self.default_bg))
        } else {
            let row = virtual_idx - sb_len;
            if row < self.rows {
                self.cells[row * self.cols + x]
            } else {
                VtCell::blank(self.default_fg, self.default_bg)
            }
        }
    }

    pub fn feed_bytes(&mut self, data: &[u8]) {
        for &b in data {
            if self.state == VtParseState::Ground {
                if !self.utf8_buf.is_empty() {
                    if (b & 0xC0) == 0x80 {
                        self.utf8_buf.push(b);
                        if let Ok(s) = std::str::from_utf8(&self.utf8_buf) {
                            let chars: Vec<char> = s.chars().collect();
                            self.utf8_buf.clear();
                            for ch in chars {
                                self.put_char(ch);
                            }
                        } else if self.utf8_buf.len() >= 4 {
                            self.utf8_buf.clear();
                        }
                        continue;
                    } else {
                        self.utf8_buf.clear();
                    }
                }
                if b >= 0x80 {
                    self.utf8_buf.push(b);
                    continue;
                }
            }
            self.process_byte(b);
        }
    }

    fn effective_colors(&self) -> (u32, u32) {
        if self.cur_reverse {
            (self.cur_bg, self.cur_fg)
        } else {
            (self.cur_fg, self.cur_bg)
        }
    }

    fn blank_cell(&self) -> VtCell {
        let (_, bg) = self.effective_colors();
        VtCell {
            ch: ' ',
            fg: self.default_fg,
            bg,
            bold: false,
        }
    }

    fn scroll_up_region(&mut self, lines: usize) {
        let top = self.scroll_top.min(self.rows.saturating_sub(1));
        let bot = self.scroll_bottom.min(self.rows.saturating_sub(1));
        if top > bot {
            return;
        }
        let n = lines.min(bot - top + 1);
        for _ in 0..n {
            if top == 0 && self.alt_cells.is_none() {
                let line = self.cells[0..self.cols].to_vec();
                if self.scrollback.len() >= 1500 {
                    self.scrollback.pop_front();
                }
                self.scrollback.push_back(line);
            }
            for y in top..bot {
                let dst = y * self.cols;
                let src = (y + 1) * self.cols;
                self.cells.copy_within(src..src + self.cols, dst);
            }
            let b = self.blank_cell();
            self.cells[bot * self.cols..(bot + 1) * self.cols].fill(b);
        }
    }

    fn scroll_down_region(&mut self, lines: usize) {
        let top = self.scroll_top.min(self.rows.saturating_sub(1));
        let bot = self.scroll_bottom.min(self.rows.saturating_sub(1));
        if top > bot {
            return;
        }
        let n = lines.min(bot - top + 1);
        for _ in 0..n {
            for y in (top + 1..=bot).rev() {
                let dst = y * self.cols;
                let src = (y - 1) * self.cols;
                self.cells.copy_within(src..src + self.cols, dst);
            }
            let b = self.blank_cell();
            self.cells[top * self.cols..(top + 1) * self.cols].fill(b);
        }
    }

    fn newline(&mut self) {
        if self.cursor_y >= self.scroll_bottom {
            self.scroll_up_region(1);
        } else if self.cursor_y + 1 < self.rows {
            self.cursor_y += 1;
        }
    }

    fn put_char(&mut self, ch: char) {
        if self.cursor_x >= self.cols {
            self.cursor_x = 0;
            self.newline();
        }
        let (fg, bg) = self.effective_colors();
        let idx = self.cursor_y * self.cols + self.cursor_x;
        if idx < self.cells.len() {
            self.cells[idx] = VtCell {
                ch,
                fg,
                bg,
                bold: self.cur_bold,
            };
        }
        self.cursor_x += 1;
        if self.cursor_x > self.cols {
            self.cursor_x = self.cols;
        }
    }

    fn process_byte(&mut self, b: u8) {
        match self.state {
            VtParseState::Ground => match b {
                0x1B => self.state = VtParseState::Escape,
                b'\r' => self.cursor_x = 0,
                b'\n' | 0x0B | 0x0C => self.newline(),
                0x08 => self.cursor_x = self.cursor_x.saturating_sub(1),
                b'\t' => {
                    let next = ((self.cursor_x / 8) + 1) * 8;
                    self.cursor_x = next.min(self.cols - 1);
                }
                0x00..=0x1F | 0x7F => {}
                c => self.put_char(c as char),
            },
            VtParseState::Escape => match b {
                b'[' => {
                    self.csi_buf.clear();
                    self.state = VtParseState::Csi;
                }
                b']' => {
                    self.csi_buf.clear();
                    self.state = VtParseState::Osc;
                }
                b'7' => {
                    self.saved_cursor = (self.cursor_x, self.cursor_y);
                    self.state = VtParseState::Ground;
                }
                b'8' => {
                    self.cursor_x = self.saved_cursor.0.min(self.cols - 1);
                    self.cursor_y = self.saved_cursor.1.min(self.rows - 1);
                    self.state = VtParseState::Ground;
                }
                b'M' => {
                    if self.cursor_y <= self.scroll_top {
                        self.scroll_down_region(1);
                    } else {
                        self.cursor_y = self.cursor_y.saturating_sub(1);
                    }
                    self.state = VtParseState::Ground;
                }
                b'(' | b')' | b'*' | b'+' => {
                    // Charset designation: ignore next char simply by returning on next byte
                    self.state = VtParseState::Ground;
                }
                _ => self.state = VtParseState::Ground,
            },
            VtParseState::Osc => {
                if b == 0x07 {
                    if let Some((code, val)) = self.csi_buf.split_once(';')
                        && (code == "0" || code == "2")
                    {
                        self.title = val.to_string();
                    }
                    self.state = VtParseState::Ground;
                } else if b == 0x1B {
                    self.state = VtParseState::OscEsc;
                } else if self.csi_buf.len() < 256 {
                    self.csi_buf.push(b as char);
                }
            }
            VtParseState::OscEsc => {
                if b == b'\\'
                    && let Some((code, val)) = self.csi_buf.split_once(';')
                    && (code == "0" || code == "2")
                {
                    self.title = val.to_string();
                }
                self.state = VtParseState::Ground;
            }
            VtParseState::Csi => {
                if (0x40..=0x7E).contains(&b) {
                    let seq = self.csi_buf.clone();
                    self.state = VtParseState::Ground;
                    self.exec_csi(&seq, b as char);
                } else if self.csi_buf.len() < 128 {
                    self.csi_buf.push(b as char);
                }
            }
        }
    }

    fn exec_csi(&mut self, raw_params: &str, cmd: char) {
        if let Some(priv_params) = raw_params.strip_prefix('?') {
            let nums: Vec<usize> = priv_params
                .split(';')
                .filter_map(|s| s.parse::<usize>().ok())
                .collect();
            let enable = cmd == 'h';
            for n in nums {
                match n {
                    25 => self.cursor_visible = enable,
                    1049 | 1047 | 47 => {
                        if enable && self.alt_cells.is_none() {
                            self.alt_cells = Some(self.cells.clone());
                            self.cells
                                .fill(VtCell::blank(self.default_fg, self.default_bg));
                            self.cursor_x = 0;
                            self.cursor_y = 0;
                            self.view_scroll = 0;
                        } else if !enable && let Some(prev) = self.alt_cells.take() {
                            if prev.len() == self.cells.len() {
                                self.cells = prev;
                            }
                            self.view_scroll = 0;
                        }
                    }
                    _ => {}
                }
            }
            return;
        }

        let params: Vec<usize> = if raw_params.is_empty() {
            Vec::new()
        } else {
            raw_params
                .split(';')
                .map(|s| s.parse::<usize>().unwrap_or(0))
                .collect()
        };
        let p = |i: usize, def: usize| -> usize {
            params.get(i).copied().filter(|&v| v > 0).unwrap_or(def)
        };

        match cmd {
            'A' => self.cursor_y = self.cursor_y.saturating_sub(p(0, 1)),
            'B' => self.cursor_y = (self.cursor_y + p(0, 1)).min(self.rows - 1),
            'C' => self.cursor_x = (self.cursor_x + p(0, 1)).min(self.cols - 1),
            'D' => self.cursor_x = self.cursor_x.saturating_sub(p(0, 1)),
            'E' => {
                self.cursor_y = (self.cursor_y + p(0, 1)).min(self.rows - 1);
                self.cursor_x = 0;
            }
            'F' => {
                self.cursor_y = self.cursor_y.saturating_sub(p(0, 1));
                self.cursor_x = 0;
            }
            'G' | '`' => self.cursor_x = (p(0, 1).saturating_sub(1)).min(self.cols - 1),
            'd' => self.cursor_y = (p(0, 1).saturating_sub(1)).min(self.rows - 1),
            'H' | 'f' => {
                self.cursor_y = (p(0, 1).saturating_sub(1)).min(self.rows - 1);
                self.cursor_x = (p(1, 1).saturating_sub(1)).min(self.cols - 1);
            }
            'J' => {
                let mode = params.first().copied().unwrap_or(0);
                let b = self.blank_cell();
                let pos = self.cursor_y * self.cols + self.cursor_x.min(self.cols - 1);
                match mode {
                    0 => {
                        if pos < self.cells.len() {
                            self.cells[pos..].fill(b);
                        }
                    }
                    1 => {
                        let end = (pos + 1).min(self.cells.len());
                        self.cells[..end].fill(b);
                    }
                    2 | 3 => self.cells.fill(b),
                    _ => {}
                }
            }
            'K' => {
                let mode = params.first().copied().unwrap_or(0);
                let b = self.blank_cell();
                let row_start = self.cursor_y * self.cols;
                let row_end = row_start + self.cols;
                let cur = row_start + self.cursor_x.min(self.cols - 1);
                match mode {
                    0 => self.cells[cur..row_end].fill(b),
                    1 => self.cells[row_start..=(cur.min(row_end - 1))].fill(b),
                    2 => self.cells[row_start..row_end].fill(b),
                    _ => {}
                }
            }
            'L' => {
                let n = p(0, 1);
                let saved_top = self.scroll_top;
                self.scroll_top = self.cursor_y;
                self.scroll_down_region(n);
                self.scroll_top = saved_top;
            }
            'M' => {
                let n = p(0, 1);
                let saved_top = self.scroll_top;
                self.scroll_top = self.cursor_y;
                self.scroll_up_region(n);
                self.scroll_top = saved_top;
            }
            'P' => {
                let n = p(0, 1).min(self.cols - self.cursor_x.min(self.cols - 1));
                let row_start = self.cursor_y * self.cols;
                let cx = self.cursor_x.min(self.cols - 1);
                let b = self.blank_cell();
                for x in cx..(self.cols - n) {
                    self.cells[row_start + x] = self.cells[row_start + x + n];
                }
                for x in (self.cols - n)..self.cols {
                    self.cells[row_start + x] = b;
                }
            }
            '@' => {
                let n = p(0, 1).min(self.cols - self.cursor_x.min(self.cols - 1));
                let row_start = self.cursor_y * self.cols;
                let cx = self.cursor_x.min(self.cols - 1);
                let b = self.blank_cell();
                for x in ((cx + n)..self.cols).rev() {
                    self.cells[row_start + x] = self.cells[row_start + x - n];
                }
                for x in cx..(cx + n).min(self.cols) {
                    self.cells[row_start + x] = b;
                }
            }
            'S' => self.scroll_up_region(p(0, 1)),
            'T' => self.scroll_down_region(p(0, 1)),
            'r' => {
                let top = p(0, 1).saturating_sub(1).min(self.rows - 1);
                let bot = p(1, self.rows).saturating_sub(1).min(self.rows - 1);
                if top < bot {
                    self.scroll_top = top;
                    self.scroll_bottom = bot;
                } else {
                    self.scroll_top = 0;
                    self.scroll_bottom = self.rows - 1;
                }
                self.cursor_x = 0;
                self.cursor_y = 0;
            }
            's' => self.saved_cursor = (self.cursor_x, self.cursor_y),
            'u' => {
                self.cursor_x = self.saved_cursor.0.min(self.cols - 1);
                self.cursor_y = self.saved_cursor.1.min(self.rows - 1);
            }
            'm' => self.apply_sgr(&params),
            _ => {}
        }
    }

    fn apply_sgr(&mut self, params: &[usize]) {
        if params.is_empty() {
            self.cur_fg = self.default_fg;
            self.cur_bg = self.default_bg;
            self.cur_bold = false;
            self.cur_reverse = false;
            return;
        }
        let mut i = 0;
        while i < params.len() {
            match params[i] {
                0 => {
                    self.cur_fg = self.default_fg;
                    self.cur_bg = self.default_bg;
                    self.cur_bold = false;
                    self.cur_reverse = false;
                }
                1 => self.cur_bold = true,
                22 => self.cur_bold = false,
                7 => self.cur_reverse = true,
                27 => self.cur_reverse = false,
                30..=37 => {
                    let idx = params[i] - 30 + if self.cur_bold { 8 } else { 0 };
                    self.cur_fg = ANSI_16_BGR[idx];
                }
                39 => self.cur_fg = self.default_fg,
                40..=47 => self.cur_bg = ANSI_16_BGR[params[i] - 40],
                49 => self.cur_bg = self.default_bg,
                90..=97 => self.cur_fg = ANSI_16_BGR[params[i] - 90 + 8],
                100..=107 => self.cur_bg = ANSI_16_BGR[params[i] - 100 + 8],
                38 | 48 => {
                    let is_fg = params[i] == 38;
                    if i + 2 < params.len() && params[i + 1] == 5 {
                        let col = xterm_256_to_colorref(params[i + 2] as u8);
                        if is_fg {
                            self.cur_fg = col;
                        } else {
                            self.cur_bg = col;
                        }
                        i += 2;
                    } else if i + 4 < params.len() && params[i + 1] == 2 {
                        let col = rgb_to_colorref(
                            params[i + 2] as u8,
                            params[i + 3] as u8,
                            params[i + 4] as u8,
                        );
                        if is_fg {
                            self.cur_fg = col;
                        } else {
                            self.cur_bg = col;
                        }
                        i += 4;
                    }
                }
                _ => {}
            }
            i += 1;
        }
    }
}

// =============================================================================
// HÉBERGEMENT CONPTY + INTERFACE GDI NATIF PAR ONGLET
// =============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SshUiZone {
    BtnCopy,
    BtnPaste,
    BtnClear,
    BtnReconnect,
    TermArea,
}

#[derive(Debug, Clone)]
struct SshHitZone {
    rect: RECT,
    zone: SshUiZone,
}

const CMD_SSH_CTX_COPY: u32 = 5401;
const CMD_SSH_CTX_PASTE: u32 = 5402;
const CMD_SSH_CTX_CLEAR: u32 = 5403;
const CMD_SSH_CTX_RECONNECT: u32 = 5404;

struct SshSharedRuntime {
    term: VtTerminal,
    is_connected: bool,
    status_detail: String,
    pty_in_write: usize,
    hpc: usize,
    h_process: usize,
    stop_flag: Arc<AtomicBool>,
}

struct SshWndState {
    session_id: u32,
    container_hwnd: HWND,
    main_hwnd: HWND,
    palette: GdiPalette,
    mono_font: HFONT,
    char_w: i32,
    char_h: i32,
    host: String,
    port: u16,
    username: String,
    identity_file: String,
    password: Mutex<SecretString>,
    runtime: Arc<Mutex<SshSharedRuntime>>,
    term_rect: RECT,
    sel_anchor: Option<(usize, usize)>,
    sel_end: Option<(usize, usize)>,
    selecting: bool,
    hovered_zone: Option<SshUiZone>,
    hit_zones: Vec<SshHitZone>,
}

unsafe fn create_mono_font_and_measure(hwnd: HWND, dpi_scale_h: i32) -> (HFONT, i32, i32) {
    unsafe {
        let font = CreateFontW(
            dpi_scale_h,
            0,
            0,
            0,
            400,
            0,
            0,
            0,
            DEFAULT_CHARSET,
            FONT_OUTPUT_PRECISION(0),
            FONT_CLIP_PRECISION(0),
            FONT_QUALITY(5), // CLEARTYPE_QUALITY
            (DEFAULT_PITCH.0 | FF_MODERN.0) as u32,
            w!("Consolas"),
        );
        let hdc = windows::Win32::Graphics::Gdi::GetDC(Some(hwnd));
        let old = SelectObject(hdc, font.into());
        let mut sz = windows::Win32::Foundation::SIZE::default();
        let sample: Vec<u16> = "W".encode_utf16().collect();
        let _ = GetTextExtentPoint32W(hdc, &sample, &mut sz);
        let _ = SelectObject(hdc, old);
        let _ = windows::Win32::Graphics::Gdi::ReleaseDC(Some(hwnd), hdc);
        (font, sz.cx.max(7), sz.cy.max(14))
    }
}

fn close_runtime_handles(rt: &mut SshSharedRuntime) {
    rt.stop_flag.store(true, Ordering::Relaxed);
    unsafe {
        if rt.hpc != 0 {
            ClosePseudoConsole(HPCON(rt.hpc as isize));
            rt.hpc = 0;
        }
        if rt.pty_in_write != 0 {
            let _ = CloseHandle(HANDLE(rt.pty_in_write as *mut c_void));
            rt.pty_in_write = 0;
        }
        if rt.h_process != 0 {
            let hp = HANDLE(rt.h_process as *mut c_void);
            let _ = TerminateProcess(hp, 0);
            let _ = CloseHandle(hp);
            rt.h_process = 0;
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn spawn_ssh_process(
    session_id: u32,
    container_hwnd: HWND,
    main_hwnd: HWND,
    host: &str,
    port: u16,
    username: &str,
    identity_file: &str,
    password: &SecretString,
    runtime: &Arc<Mutex<SshSharedRuntime>>,
) {
    let (cols, rows) = {
        let Ok(mut rt) = runtime.lock() else {
            return;
        };
        close_runtime_handles(&mut rt);
        rt.stop_flag = Arc::new(AtomicBool::new(false));
        rt.is_connected = true;
        rt.status_detail = format!("Connexion SSH vers {host}:{port}...");
        (rt.term.cols as i16, rt.term.rows as i16)
    };

    let ssh_exe = match find_windows_ssh_exe() {
        Ok(p) => p,
        Err(err) => {
            if let Ok(mut rt) = runtime.lock() {
                rt.is_connected = false;
                rt.status_detail = err.clone();
                rt.term
                    .feed_bytes(format!("\r\n\x1b[31m[Erreur] {err}\x1b[0m\r\n").as_bytes());
            }
            unsafe {
                let _ = PostMessageW(
                    Some(container_hwnd),
                    WM_SSH_INTERNAL_REFRESH,
                    WPARAM(0),
                    LPARAM(0),
                );
                let _ = PostMessageW(
                    Some(main_hwnd),
                    WM_SSH_EVENT,
                    pack_ssh_event_wparam(session_id, SSH_EVT_ERROR),
                    LPARAM(0),
                );
            }
            return;
        }
    };

    let args = match build_ssh_args(host, port, username, identity_file) {
        Ok(a) => a,
        Err(err) => {
            if let Ok(mut rt) = runtime.lock() {
                rt.is_connected = false;
                rt.status_detail = err.clone();
                rt.term
                    .feed_bytes(format!("\r\n\x1b[31m[Erreur] {err}\x1b[0m\r\n").as_bytes());
            }
            return;
        }
    };

    let mut cmd_str = quote_win32_arg(&ssh_exe.to_string_lossy());
    for a in &args {
        cmd_str.push(' ');
        cmd_str.push_str(&quote_win32_arg(a));
    }

    // Préparer un pipe nommé éphémère SSH_ASKPASS si un mot de passe DPAPI est enregistré
    let mut askpass_pipe_name: Option<String> = None;
    let mut rand = [0u8; 16];
    if !password.is_empty() && crate::security::generate_crypto_random_bytes(&mut rand).is_ok() {
        let hex: String = rand.iter().map(|b| format!("{b:02x}")).collect();
        let pipe_name = format!(r"\\.\pipe\MalvisionAskPass_{}_{}", std::process::id(), hex);
        let wide_pipe: Vec<u16> = pipe_name.encode_utf16().chain(std::iter::once(0)).collect();
        let secret_copy = format!("{}\n", password.as_str());
        let h_pipe = unsafe {
            CreateNamedPipeW(
                PCWSTR(wide_pipe.as_ptr()),
                FILE_FLAGS_AND_ATTRIBUTES(PIPE_ACCESS_OUTBOUND),
                NAMED_PIPE_MODE(PIPE_TYPE_BYTE | PIPE_WAIT),
                1,
                1024,
                0,
                5000,
                None,
            )
        };
        if !h_pipe.is_invalid() {
            let pipe_raw = h_pipe.0 as usize;
            askpass_pipe_name = Some(pipe_name);
            std::thread::spawn(move || unsafe {
                let hp = HANDLE(pipe_raw as *mut c_void);
                let mut sec = secret_copy;
                let _ = ConnectNamedPipe(hp, None);
                let mut written = 0u32;
                let _ = WriteFile(hp, Some(sec.as_bytes()), Some(&mut written), None);
                let _ = FlushFileBuffers(hp);
                let _ = DisconnectNamedPipe(hp);
                let _ = CloseHandle(hp);
                zeroize_string(&mut sec);
            });
        }
    }

    // Construire l'environnement enfant Unicode
    let mut env_pairs: Vec<(String, String)> = std::env::vars().collect();
    env_pairs.retain(|(k, _)| {
        !k.eq_ignore_ascii_case("MALVISION_SSH_ASKPASS_PIPE")
            && !k.eq_ignore_ascii_case("SSH_ASKPASS")
            && !k.eq_ignore_ascii_case("SSH_ASKPASS_REQUIRE")
    });
    if let Some(ref pipe_name) = askpass_pipe_name
        && let Ok(exe_path) = std::env::current_exe()
    {
        env_pairs.push((
            "SSH_ASKPASS".to_string(),
            exe_path.to_string_lossy().to_string(),
        ));
        env_pairs.push(("SSH_ASKPASS_REQUIRE".to_string(), "force".to_string()));
        env_pairs.push(("DISPLAY".to_string(), "malvision:0".to_string()));
        env_pairs.push(("MALVISION_SSH_ASKPASS_PIPE".to_string(), pipe_name.clone()));
    }
    let mut env_w: Vec<u16> = Vec::new();
    for (k, v) in env_pairs {
        env_w.extend(format!("{k}={v}").encode_utf16());
        env_w.push(0);
    }
    env_w.push(0);

    unsafe {
        let mut in_read = HANDLE::default();
        let mut in_write = HANDLE::default();
        let mut out_read = HANDLE::default();
        let mut out_write = HANDLE::default();

        if CreatePipe(&mut in_read, &mut in_write, None, 0).is_err()
            || CreatePipe(&mut out_read, &mut out_write, None, 0).is_err()
        {
            return;
        }

        let hpc_res = CreatePseudoConsole(COORD { X: cols, Y: rows }, in_read, out_write, 0);
        let Ok(hpc) = hpc_res else {
            let _ = CloseHandle(in_read);
            let _ = CloseHandle(in_write);
            let _ = CloseHandle(out_read);
            let _ = CloseHandle(out_write);
            return;
        };

        // Fermer immédiatement les extrémités détenues par ConPTY
        let _ = CloseHandle(in_read);
        let _ = CloseHandle(out_write);

        let mut attr_size = 0usize;
        let _ = InitializeProcThreadAttributeList(None, 1, None, &mut attr_size);
        let mut attr_buf = vec![0u8; attr_size];
        let attr_list = LPPROC_THREAD_ATTRIBUTE_LIST(attr_buf.as_mut_ptr() as *mut c_void);
        if InitializeProcThreadAttributeList(Some(attr_list), 1, None, &mut attr_size).is_err()
            || UpdateProcThreadAttribute(
                attr_list,
                0,
                PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE,
                Some(hpc.0 as *const c_void),
                std::mem::size_of::<HPCON>(),
                None,
                None,
            )
            .is_err()
        {
            ClosePseudoConsole(hpc);
            let _ = CloseHandle(in_write);
            let _ = CloseHandle(out_read);
            return;
        }

        let si = STARTUPINFOEXW {
            StartupInfo: STARTUPINFOW {
                cb: std::mem::size_of::<STARTUPINFOEXW>() as u32,
                ..Default::default()
            },
            lpAttributeList: attr_list,
        };
        let mut pi = PROCESS_INFORMATION::default();
        let mut cmd_w: Vec<u16> = cmd_str.encode_utf16().chain(std::iter::once(0)).collect();

        let created = CreateProcessW(
            None,
            Some(PWSTR(cmd_w.as_mut_ptr())),
            None,
            None,
            false,
            EXTENDED_STARTUPINFO_PRESENT | CREATE_UNICODE_ENVIRONMENT,
            Some(env_w.as_ptr() as *const c_void),
            None,
            &si.StartupInfo,
            &mut pi,
        );
        DeleteProcThreadAttributeList(attr_list);

        if created.is_err() {
            ClosePseudoConsole(hpc);
            let _ = CloseHandle(in_write);
            let _ = CloseHandle(out_read);
            return;
        }
        let _ = CloseHandle(pi.hThread);

        let stop_flag = if let Ok(mut rt) = runtime.lock() {
            rt.pty_in_write = in_write.0 as usize;
            rt.hpc = hpc.0 as usize;
            rt.h_process = pi.hProcess.0 as usize;
            rt.is_connected = true;
            rt.status_detail = format!("Connecté SSH ({host}:{port})");
            Arc::clone(&rt.stop_flag)
        } else {
            return;
        };

        let _ = PostMessageW(
            Some(main_hwnd),
            WM_SSH_EVENT,
            pack_ssh_event_wparam(session_id, SSH_EVT_CONNECTED),
            LPARAM(0),
        );

        // Thread de lecture PTY -> VtTerminal
        let rt_clone = Arc::clone(runtime);
        let cont_raw = container_hwnd.0 as usize;
        let main_raw = main_hwnd.0 as usize;
        let out_read_raw = out_read.0 as usize;
        let proc_raw = pi.hProcess.0 as usize;

        std::thread::spawn(move || {
            let hr = HANDLE(out_read_raw as *mut c_void);
            let mut buf = [0u8; 4096];
            loop {
                if stop_flag.load(Ordering::Relaxed) {
                    break;
                }
                let mut n = 0u32;
                let ok = ReadFile(hr, Some(&mut buf), Some(&mut n), None).is_ok();
                if !ok || n == 0 {
                    break;
                }
                if let Ok(mut rt) = rt_clone.lock() {
                    rt.term.feed_bytes(&buf[..n as usize]);
                }
                let ch = HWND(cont_raw as *mut c_void);
                if !ch.is_invalid() {
                    let _ = PostMessageW(Some(ch), WM_SSH_INTERNAL_REFRESH, WPARAM(0), LPARAM(0));
                }
            }
            let _ = CloseHandle(hr);

            let hp = HANDLE(proc_raw as *mut c_void);
            let _ = WaitForSingleObject(hp, 500);
            if !stop_flag.load(Ordering::Relaxed) {
                if let Ok(mut rt) = rt_clone.lock() {
                    rt.is_connected = false;
                    rt.status_detail = "Session SSH fermée.".to_string();
                    rt.term
                        .feed_bytes("\r\n\x1b[33m[Session SSH terminée]\x1b[0m\r\n".as_bytes());
                }
                let ch = HWND(cont_raw as *mut c_void);
                let mh = HWND(main_raw as *mut c_void);
                if !ch.is_invalid() {
                    let _ = PostMessageW(Some(ch), WM_SSH_INTERNAL_REFRESH, WPARAM(0), LPARAM(0));
                }
                if !mh.is_invalid() {
                    let _ = PostMessageW(
                        Some(mh),
                        WM_SSH_EVENT,
                        pack_ssh_event_wparam(session_id, SSH_EVT_DISCONNECTED),
                        LPARAM(0),
                    );
                }
            }
        });
    }
}

fn send_bytes_to_pty(state: &SshWndState, bytes: &[u8]) {
    if bytes.is_empty() {
        return;
    }
    let handle_raw = state.runtime.lock().map(|rt| rt.pty_in_write).unwrap_or(0);
    if handle_raw == 0 {
        return;
    }
    unsafe {
        let hw = HANDLE(handle_raw as *mut c_void);
        let mut written = 0u32;
        let _ = WriteFile(hw, Some(bytes), Some(&mut written), None);
    }
}

fn selected_terminal_text(state: &SshWndState) -> Option<String> {
    let (a, b) = (state.sel_anchor?, state.sel_end?);
    if a == b {
        return None;
    }
    let (start, end) = if (a.1, a.0) <= (b.1, b.0) {
        (a, b)
    } else {
        (b, a)
    };
    let rt = state.runtime.lock().ok()?;
    let mut out = String::new();
    for y in start.1..=end.1.min(rt.term.rows.saturating_sub(1)) {
        let x0 = if y == start.1 { start.0 } else { 0 };
        let x1 = if y == end.1 {
            end.0.min(rt.term.cols.saturating_sub(1))
        } else {
            rt.term.cols.saturating_sub(1)
        };
        let mut line = String::new();
        for x in x0..=x1 {
            line.push(rt.term.visible_cell(y, x).ch);
        }
        out.push_str(line.trim_end());
        if y < end.1 {
            out.push_str("\r\n");
        }
    }
    if out.is_empty() { None } else { Some(out) }
}

unsafe fn copy_text_to_clipboard(hwnd: HWND, text: &str) {
    unsafe {
        if OpenClipboard(Some(hwnd)).is_err() {
            return;
        }
        let _ = EmptyClipboard();
        let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
        let byte_len = wide.len() * std::mem::size_of::<u16>();
        if let Ok(hmem) = GlobalAlloc(GLOBAL_ALLOC_FLAGS(GMEM_MOVEABLE.0), byte_len) {
            let ptr = GlobalLock(hmem) as *mut u16;
            if !ptr.is_null() {
                std::ptr::copy_nonoverlapping(wide.as_ptr(), ptr, wide.len());
                let _ = GlobalUnlock(hmem);
                let _ = SetClipboardData(CF_UNICODETEXT_FMT, Some(HANDLE(hmem.0)));
            }
        }
        let _ = CloseClipboard();
    }
}

unsafe fn paste_from_clipboard(state: &SshWndState) {
    unsafe {
        if OpenClipboard(Some(state.container_hwnd)).is_err() {
            return;
        }
        if let Ok(h) = GetClipboardData(CF_UNICODETEXT_FMT) {
            let ptr = GlobalLock(HGLOBAL(h.0)) as *const u16;
            if !ptr.is_null() {
                let mut len = 0usize;
                while *ptr.add(len) != 0 && len < 65536 {
                    len += 1;
                }
                let slice = std::slice::from_raw_parts(ptr, len);
                let txt = String::from_utf16_lossy(slice);
                let _ = GlobalUnlock(HGLOBAL(h.0));
                send_bytes_to_pty(state, txt.as_bytes());
            }
        }
        let _ = CloseClipboard();
    }
}

fn is_key_down(vk: u16) -> bool {
    unsafe { (GetKeyState(i32::from(vk)) as u16 & 0x8000) != 0 }
}

fn ssh_hit_test(state: &SshWndState, x: i32, y: i32) -> Option<SshUiZone> {
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

unsafe fn paint_ssh_host(state: &mut SshWndState) {
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
        let toolbar_h = dpi.scale(42);
        let pad_x = dpi.scale(12);
        let theme = active_theme_colors();

        // 1. Barre supérieure Fluent SSH
        let bar_rc = RECT {
            left: 0,
            top: 0,
            right: w,
            bottom: toolbar_h,
        };
        let _ = FillRect(hdc, &bar_rc, state.palette.brush_bg_sidebar);
        draw_horizontal_line(hdc, 0, w, toolbar_h - 1, col_border());

        let badge_rc = RECT {
            left: pad_x,
            top: dpi.scale(11),
            right: pad_x + dpi.scale(40),
            bottom: toolbar_h - dpi.scale(11),
        };
        draw_filled_badge(
            hdc,
            badge_rc,
            COL_PROTO_SSH,
            "SSH",
            state.palette.font_small,
        );

        let is_conn = state
            .runtime
            .lock()
            .map(|r| r.is_connected)
            .unwrap_or(false);
        draw_status_dot(
            hdc,
            badge_rc.right + dpi.scale(12),
            toolbar_h / 2,
            if is_conn {
                col_status_ok()
            } else {
                col_status_err()
            },
        );

        let endpoint_txt = if state.username.is_empty() {
            format!("{}:{}", state.host, state.port)
        } else {
            format!("{}@{}:{}", state.username, state.host, state.port)
        };
        let _ = SelectObject(hdc, state.palette.font_ui_bold.into());
        let _ = SetTextColor(hdc, COLORREF(col_text_primary()));
        draw_text_line(
            hdc,
            &endpoint_txt,
            RECT {
                left: badge_rc.right + dpi.scale(22),
                top: 0,
                right: (w - dpi.scale(370)).max(badge_rc.right + dpi.scale(80)),
                bottom: toolbar_h,
            },
        );

        // Boutons à droite : Copier | Coller | Effacer | Reconnecter
        let btn_h = dpi.scale(28);
        let btn_top = (toolbar_h - btn_h) / 2;
        let gap = dpi.scale(6);
        let btns = [
            ("Reconnecter", SshUiZone::BtnReconnect, dpi.scale(96), true),
            ("Effacer", SshUiZone::BtnClear, dpi.scale(68), false),
            ("Coller", SshUiZone::BtnPaste, dpi.scale(64), false),
            ("Copier", SshUiZone::BtnCopy, dpi.scale(64), false),
        ];
        let mut rx = w - pad_x;
        for (lbl, zone, bw, primary) in btns {
            let brc = RECT {
                left: rx - bw,
                top: btn_top,
                right: rx,
                bottom: btn_top + btn_h,
            };
            let hover = state.hovered_zone == Some(zone);
            let (fill, border, tcol) = if primary {
                let bg = if hover {
                    col_accent_hover()
                } else {
                    col_accent()
                };
                (bg, bg, COL_TEXT_ON_ACCENT)
            } else if hover {
                (col_bg_hover(), col_accent(), col_text_primary())
            } else {
                (col_bg_surface(), col_border(), col_text_primary())
            };
            draw_rounded_button(hdc, brc, fill, border, lbl, state.palette.font_ui, tcol);
            state.hit_zones.push(FtpOrSsh_noop(brc, zone));
            rx = brc.left - gap;
        }

        // 2. Zone Terminal GDI
        let term_pad = dpi.scale(10);
        state.term_rect = RECT {
            left: 0,
            top: toolbar_h,
            right: w,
            bottom: h,
        };
        let _ = FillRect(hdc, &state.term_rect, state.palette.brush_bg_deep);
        state.hit_zones.push(SshHitZone {
            rect: state.term_rect,
            zone: SshUiZone::TermArea,
        });

        let norm_sel = match (state.sel_anchor, state.sel_end) {
            (Some(a), Some(b)) if a != b => {
                if (a.1, a.0) <= (b.1, b.0) {
                    Some((a, b))
                } else {
                    Some((b, a))
                }
            }
            _ => None,
        };

        let _ = SelectObject(hdc, state.mono_font.into());
        if let Ok(rt) = state.runtime.lock() {
            let start_x = term_pad;
            let start_y = toolbar_h + dpi.scale(6);
            let default_bg = rt.term.default_bg;

            for y in 0..rt.term.rows {
                let py = start_y + (y as i32) * state.char_h;
                if py + state.char_h > h {
                    break;
                }
                for x in 0..rt.term.cols {
                    let px = start_x + (x as i32) * state.char_w;
                    if px >= w - term_pad {
                        break;
                    }
                    let cell = rt.term.visible_cell(y, x);
                    let in_sel =
                        norm_sel.is_some_and(|(s, e)| (y, x) >= (s.1, s.0) && (y, x) <= (e.1, e.0));
                    let cell_bg = if in_sel { theme.bg_selected } else { cell.bg };
                    if cell_bg != default_bg {
                        let b = CreateSolidBrush(COLORREF(cell_bg));
                        let crc = RECT {
                            left: px,
                            top: py,
                            right: px + state.char_w,
                            bottom: py + state.char_h,
                        };
                        let _ = FillRect(hdc, &crc, b);
                        let _ = DeleteObject(b.into());
                    }
                    if cell.ch != ' ' {
                        let _ = SetTextColor(hdc, COLORREF(cell.fg));
                        let u = [cell.ch as u16];
                        let _ = TextOutW(hdc, px, py, &u);
                    }
                }
            }

            // Curseur terminal
            if rt.term.cursor_visible && rt.term.view_scroll == 0 && is_conn {
                let cx = start_x + (rt.term.cursor_x.min(rt.term.cols - 1) as i32) * state.char_w;
                let cy = start_y + (rt.term.cursor_y.min(rt.term.rows - 1) as i32) * state.char_h;
                let cur_rc = RECT {
                    left: cx,
                    top: cy,
                    right: cx + state.char_w,
                    bottom: cy + state.char_h,
                };
                draw_rounded_box(hdc, cur_rc, col_accent(), col_accent_hover());
            }
        }

        let _ = BitBlt(hdc_screen, 0, 0, w, h, Some(hdc), 0, 0, SRCCOPY);
        let _ = SelectObject(hdc, old_bm);
        let _ = DeleteObject(hbm.into());
        let _ = DeleteDC(hdc);
        let _ = EndPaint(state.container_hwnd, &ps);
    }
}

#[inline]
#[allow(non_snake_case)]
fn FtpOrSsh_noop(rect: RECT, zone: SshUiZone) -> SshHitZone {
    SshHitZone { rect, zone }
}

unsafe fn resize_terminal_to_window(state: &mut SshWndState, w: i32, h: i32) {
    let dpi = state.palette.dpi;
    let toolbar_h = dpi.scale(42);
    let term_pad = dpi.scale(10);
    let usable_w = (w - term_pad * 2).max(state.char_w * 20);
    let usable_h = (h - toolbar_h - dpi.scale(12)).max(state.char_h * 5);
    let cols = (usable_w / state.char_w.max(1)).max(20) as usize;
    let rows = (usable_h / state.char_h.max(1)).max(5) as usize;

    if let Ok(mut rt) = state.runtime.lock() {
        rt.term.resize(cols, rows);
        if rt.hpc != 0 {
            unsafe {
                let _ = ResizePseudoConsole(
                    HPCON(rt.hpc as isize),
                    COORD {
                        X: cols as i16,
                        Y: rows as i16,
                    },
                );
            }
        }
    }
}

/// Évite le double envoi des touches déjà transmises dans `WM_KEYDOWN` (`Entrée`, `Retour arrière`, `Tab`, `Échap`).
fn should_skip_ssh_wm_char(ch: char, ctrl_down: bool) -> bool {
    match ch {
        '\x08' | '\t' | '\x1b' => true,
        '\r' | '\n' if !ctrl_down => true,
        _ => false,
    }
}

unsafe extern "system" fn ssh_host_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut SshWndState;
        match msg {
            WM_ERASEBKGND => LRESULT(1),
            WM_SIZE => {
                if !ptr.is_null() {
                    let state = &mut *ptr;
                    let w = (lparam.0 & 0xFFFF) as i16 as i32;
                    let h = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;
                    resize_terminal_to_window(state, w, h);
                    let _ = InvalidateRect(Some(hwnd), None, false);
                }
                LRESULT(0)
            }
            WM_SSH_INTERNAL_REFRESH => {
                let _ = InvalidateRect(Some(hwnd), None, false);
                LRESULT(0)
            }
            WM_PAINT => {
                if !ptr.is_null() {
                    paint_ssh_host(&mut *ptr);
                    return LRESULT(0);
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            WM_SETCURSOR => {
                if !ptr.is_null() {
                    let state = &*ptr;
                    let id = match state.hovered_zone {
                        Some(SshUiZone::TermArea) => Some(IDC_IBEAM),
                        Some(_) => Some(IDC_HAND),
                        None => None,
                    };
                    if let Some(c) = id {
                        let _ = SetCursor(Some(LoadCursorW(None, c).unwrap_or_default()));
                        return LRESULT(1);
                    }
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            WM_MOUSEMOVE => {
                if !ptr.is_null() {
                    let state = &mut *ptr;
                    let x = (lparam.0 & 0xFFFF) as i16 as i32;
                    let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;
                    if state.selecting {
                        let dpi = state.palette.dpi;
                        let start_x = dpi.scale(10);
                        let start_y = dpi.scale(42) + dpi.scale(6);
                        let col = ((x - start_x).max(0) / state.char_w.max(1)) as usize;
                        let row = ((y - start_y).max(0) / state.char_h.max(1)) as usize;
                        state.sel_end = Some((col, row));
                        let _ = InvalidateRect(Some(hwnd), None, false);
                        return LRESULT(0);
                    }
                    let mut tme = TRACKMOUSEEVENT {
                        cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                        dwFlags: TME_LEAVE,
                        hwndTrack: hwnd,
                        dwHoverTime: 0,
                    };
                    let _ = TrackMouseEvent(&mut tme);
                    let nh = ssh_hit_test(state, x, y);
                    if nh != state.hovered_zone {
                        state.hovered_zone = nh;
                        let _ = InvalidateRect(Some(hwnd), None, false);
                    }
                }
                LRESULT(0)
            }
            WM_MOUSELEAVE => {
                if !ptr.is_null() && (*ptr).hovered_zone.take().is_some() {
                    let _ = InvalidateRect(Some(hwnd), None, false);
                }
                LRESULT(0)
            }
            WM_LBUTTONUP | WM_CAPTURECHANGED => {
                if !ptr.is_null() && (*ptr).selecting {
                    (*ptr).selecting = false;
                    if msg == WM_LBUTTONUP {
                        let _ = ReleaseCapture();
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
                    match ssh_hit_test(state, x, y) {
                        Some(SshUiZone::BtnCopy) => {
                            if let Some(txt) = selected_terminal_text(state) {
                                copy_text_to_clipboard(hwnd, &txt);
                            }
                        }
                        Some(SshUiZone::BtnPaste) => paste_from_clipboard(state),
                        Some(SshUiZone::BtnClear) => {
                            if let Ok(mut rt) = state.runtime.lock() {
                                rt.term.clear_all();
                            }
                            let _ = InvalidateRect(Some(hwnd), None, false);
                        }
                        Some(SshUiZone::BtnReconnect) => {
                            if let Ok(mut rt) = state.runtime.lock() {
                                rt.term.clear_all();
                            }
                            let pwd = state
                                .password
                                .lock()
                                .map(|p| SecretString::new(p.as_str().to_string()))
                                .unwrap_or_else(|_| SecretString::new(String::new()));
                            spawn_ssh_process(
                                state.session_id,
                                hwnd,
                                state.main_hwnd,
                                &state.host,
                                state.port,
                                &state.username,
                                &state.identity_file,
                                &pwd,
                                &state.runtime,
                            );
                        }
                        Some(SshUiZone::TermArea) => {
                            let dpi = state.palette.dpi;
                            let start_x = dpi.scale(10);
                            let start_y = dpi.scale(42) + dpi.scale(6);
                            let col = ((x - start_x).max(0) / state.char_w.max(1)) as usize;
                            let row = ((y - start_y).max(0) / state.char_h.max(1)) as usize;
                            state.sel_anchor = Some((col, row));
                            state.sel_end = Some((col, row));
                            state.selecting = true;
                            SetCapture(hwnd);
                            let _ = InvalidateRect(Some(hwnd), None, false);
                        }
                        None => {}
                    }
                }
                LRESULT(0)
            }
            WM_RBUTTONUP | WM_CONTEXTMENU => {
                if !ptr.is_null() {
                    let state = &mut *ptr;
                    let mut pt = POINT::default();
                    let _ = GetCursorPos(&mut pt);
                    if let Ok(hmenu) = CreatePopupMenu() {
                        let _ = AppendMenuW(
                            hmenu,
                            MF_STRING,
                            CMD_SSH_CTX_COPY as usize,
                            w!("Copier la sélection\tCtrl+Shift+C"),
                        );
                        let _ = AppendMenuW(
                            hmenu,
                            MF_STRING,
                            CMD_SSH_CTX_PASTE as usize,
                            w!("Coller\tCtrl+Shift+V"),
                        );
                        let _ = AppendMenuW(hmenu, MF_SEPARATOR, 0, PCWSTR::null());
                        let _ = AppendMenuW(
                            hmenu,
                            MF_STRING,
                            CMD_SSH_CTX_CLEAR as usize,
                            w!("Effacer l'écran"),
                        );
                        let _ = AppendMenuW(
                            hmenu,
                            MF_STRING,
                            CMD_SSH_CTX_RECONNECT as usize,
                            w!("Reconnecter"),
                        );
                        let _ = SetForegroundWindow(hwnd);
                        let cmd = TrackPopupMenu(
                            hmenu,
                            TPM_LEFTALIGN | TPM_TOPALIGN | TPM_RETURNCMD,
                            pt.x,
                            pt.y,
                            Some(0),
                            hwnd,
                            None,
                        );
                        let _ = DestroyMenu(hmenu);
                        match cmd.0 as u32 {
                            CMD_SSH_CTX_COPY => {
                                if let Some(txt) = selected_terminal_text(state) {
                                    copy_text_to_clipboard(hwnd, &txt);
                                }
                            }
                            CMD_SSH_CTX_PASTE => paste_from_clipboard(state),
                            CMD_SSH_CTX_CLEAR => {
                                if let Ok(mut rt) = state.runtime.lock() {
                                    rt.term.clear_all();
                                }
                                let _ = InvalidateRect(Some(hwnd), None, false);
                            }
                            CMD_SSH_CTX_RECONNECT => {
                                let pwd = state
                                    .password
                                    .lock()
                                    .map(|p| SecretString::new(p.as_str().to_string()))
                                    .unwrap_or_else(|_| SecretString::new(String::new()));
                                spawn_ssh_process(
                                    state.session_id,
                                    hwnd,
                                    state.main_hwnd,
                                    &state.host,
                                    state.port,
                                    &state.username,
                                    &state.identity_file,
                                    &pwd,
                                    &state.runtime,
                                );
                            }
                            _ => {}
                        }
                    }
                }
                LRESULT(0)
            }
            WM_MOUSEWHEEL => {
                if !ptr.is_null() {
                    let state = &mut *ptr;
                    let delta = ((wparam.0 >> 16) & 0xFFFF) as i16 as i32;
                    if let Ok(mut rt) = state.runtime.lock() {
                        let max_sb = rt.term.total_scrollback();
                        if delta > 0 {
                            rt.term.view_scroll = (rt.term.view_scroll + 3).min(max_sb);
                        } else {
                            rt.term.view_scroll = rt.term.view_scroll.saturating_sub(3);
                        }
                    }
                    let _ = InvalidateRect(Some(hwnd), None, false);
                }
                LRESULT(0)
            }
            WM_CHAR => {
                if !ptr.is_null() {
                    let state = &mut *ptr;
                    if let Some(ch) = char::from_u32(wparam.0 as u32) {
                        if should_skip_ssh_wm_char(ch, is_key_down(VK_CONTROL.0)) {
                            return LRESULT(0);
                        }
                        if let Ok(mut rt) = state.runtime.lock() {
                            rt.term.view_scroll = 0;
                        }
                        let mut buf = [0u8; 4];
                        let s = ch.encode_utf8(&mut buf);
                        send_bytes_to_pty(state, s.as_bytes());
                    }
                }
                LRESULT(0)
            }
            WM_KEYDOWN => {
                if !ptr.is_null() {
                    let state = &mut *ptr;
                    let vk = wparam.0 as u16;
                    let ctrl = is_key_down(VK_CONTROL.0);
                    let shift = is_key_down(VK_SHIFT.0);
                    if ctrl && shift && vk == b'C' as u16 {
                        if let Some(txt) = selected_terminal_text(state) {
                            copy_text_to_clipboard(hwnd, &txt);
                        }
                        return LRESULT(0);
                    }
                    if ctrl && shift && vk == b'V' as u16 {
                        paste_from_clipboard(state);
                        return LRESULT(0);
                    }
                    let seq: Option<&[u8]> = match vk {
                        x if x == VK_UP.0 => Some(b"\x1b[A"),
                        x if x == VK_DOWN.0 => Some(b"\x1b[B"),
                        x if x == VK_RIGHT.0 => Some(b"\x1b[C"),
                        x if x == VK_LEFT.0 => Some(b"\x1b[D"),
                        x if x == VK_HOME.0 => Some(b"\x1b[H"),
                        x if x == VK_END.0 => Some(b"\x1b[F"),
                        x if x == VK_INSERT.0 => Some(b"\x1b[2~"),
                        x if x == VK_DELETE.0 => Some(b"\x1b[3~"),
                        x if x == VK_PRIOR.0 => Some(b"\x1b[5~"),
                        x if x == VK_NEXT.0 => Some(b"\x1b[6~"),
                        x if x == VK_BACK.0 => Some(b"\x7f"),
                        x if x == VK_TAB.0 => Some(b"\t"),
                        x if x == VK_RETURN.0 => Some(b"\r"),
                        x if x == VK_ESCAPE.0 => Some(b"\x1b"),
                        x if (VK_F1.0..=VK_F12.0).contains(&x) => match x - VK_F1.0 {
                            0 => Some(b"\x1bOP"),
                            1 => Some(b"\x1bOQ"),
                            2 => Some(b"\x1bOR"),
                            3 => Some(b"\x1bOS"),
                            4 => Some(b"\x1b[15~"),
                            5 => Some(b"\x1b[17~"),
                            6 => Some(b"\x1b[18~"),
                            7 => Some(b"\x1b[19~"),
                            8 => Some(b"\x1b[20~"),
                            9 => Some(b"\x1b[21~"),
                            10 => Some(b"\x1b[23~"),
                            11 => Some(b"\x1b[24~"),
                            _ => None,
                        },
                        _ => None,
                    };
                    if let Some(bytes) = seq {
                        send_bytes_to_pty(state, bytes);
                        return LRESULT(0);
                    }
                }
                LRESULT(0)
            }
            WM_DESTROY => {
                if !ptr.is_null() {
                    SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
                    let st = Box::from_raw(ptr);
                    if let Ok(mut rt) = st.runtime.lock() {
                        close_runtime_handles(&mut rt);
                    }
                    if let Ok(mut pwd) = st.password.lock() {
                        pwd.clear();
                    }
                    if !st.mono_font.is_invalid() {
                        let _ = DeleteObject(st.mono_font.into());
                    }
                }
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}

/// Contrôleur Win32 d'un onglet SSH intégré hébergé dans la fenêtre principale.
pub struct SshSessionHost {
    session_id: u32,
    container_hwnd: HWND,
    runtime: Arc<Mutex<SshSharedRuntime>>,
}

impl SshSessionHost {
    #[allow(clippy::too_many_arguments)]
    pub fn create(
        session_id: u32,
        parent_hwnd: HWND,
        host: String,
        port: u16,
        username: String,
        password: SecretString,
        identity_file: String,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
    ) -> Result<Self, String> {
        unsafe {
            let hinstance = GetModuleHandleW(None).unwrap_or_default();
            let class_name = w!("MalvisionSshHostClass");
            let wc = WNDCLASSEXW {
                cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                style: windows::Win32::UI::WindowsAndMessaging::CS_DBLCLKS
                    | windows::Win32::UI::WindowsAndMessaging::CS_HREDRAW
                    | windows::Win32::UI::WindowsAndMessaging::CS_VREDRAW,
                lpfnWndProc: Some(ssh_host_wnd_proc),
                hInstance: hinstance.into(),
                hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
                lpszClassName: class_name,
                ..Default::default()
            };
            let _ = RegisterClassExW(&wc);

            let container_hwnd = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                class_name,
                w!("Malvision SSH"),
                WS_CHILD | WS_CLIPCHILDREN,
                x,
                y,
                width.max(240),
                height.max(160),
                Some(parent_hwnd),
                None,
                Some(hinstance.into()),
                None,
            )
            .map_err(|e| format!("Échec création conteneur SSH Win32 : {e}"))?;

            let palette = GdiPalette::create_for_window(container_hwnd);
            let dpi = palette.dpi;
            let (mono_font, char_w, char_h) =
                create_mono_font_and_measure(container_hwnd, -dpi.scale(14));

            let term_cols = ((width - dpi.scale(20)).max(160) / char_w).max(20) as usize;
            let term_rows = ((height - dpi.scale(54)).max(80) / char_h).max(5) as usize;

            let runtime = Arc::new(Mutex::new(SshSharedRuntime {
                term: VtTerminal::new(term_cols, term_rows, col_text_primary(), col_bg_deep()),
                is_connected: false,
                status_detail: format!("Connexion SSH vers {host}:{port}..."),
                pty_in_write: 0,
                hpc: 0,
                h_process: 0,
                stop_flag: Arc::new(AtomicBool::new(false)),
            }));

            let pwd_copy = SecretString::new(password.as_str().to_string());
            let state = Box::new(SshWndState {
                session_id,
                container_hwnd,
                main_hwnd: parent_hwnd,
                palette,
                mono_font,
                char_w,
                char_h,
                host: host.clone(),
                port,
                username: username.clone(),
                identity_file: identity_file.clone(),
                password: Mutex::new(pwd_copy),
                runtime: Arc::clone(&runtime),
                term_rect: RECT::default(),
                sel_anchor: None,
                sel_end: None,
                selecting: false,
                hovered_zone: None,
                hit_zones: Vec::new(),
            });
            SetWindowLongPtrW(container_hwnd, GWLP_USERDATA, Box::into_raw(state) as isize);

            spawn_ssh_process(
                session_id,
                container_hwnd,
                parent_hwnd,
                &host,
                port,
                &username,
                &identity_file,
                &password,
                &runtime,
            );

            Ok(Self {
                session_id,
                container_hwnd,
                runtime,
            })
        }
    }

    pub fn session_id(&self) -> u32 {
        self.session_id
    }

    pub fn latest_status_detail(&self) -> Option<String> {
        self.runtime.lock().ok().map(|r| r.status_detail.clone())
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
                    width.max(160),
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
            let ptr = GetWindowLongPtrW(self.container_hwnd, GWLP_USERDATA) as *mut SshWndState;
            if !ptr.is_null() {
                (*ptr).palette.rebuild_brushes();
            }
            if let Ok(mut rt) = self.runtime.lock() {
                rt.term
                    .update_theme_colors(col_text_primary(), col_bg_deep());
            }
            let _ = RedrawWindow(
                Some(self.container_hwnd),
                None,
                None,
                RDW_INVALIDATE | RDW_ERASE | RDW_ALLCHILDREN | RDW_UPDATENOW,
            );
        }
    }

    pub fn disconnect(&self) {
        if let Ok(mut rt) = self.runtime.lock() {
            close_runtime_handles(&mut rt);
        }
    }
}

impl Drop for SshSessionHost {
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

    #[test]
    fn test_build_ssh_args_and_validation() {
        let args = build_ssh_args("10.0.0.5", 2222, "admin", "~/.ssh/id_ed25519").unwrap();
        assert_eq!(
            args,
            vec![
                "-p",
                "2222",
                "-o",
                "ServerAliveInterval=25",
                "-o",
                "ServerAliveCountMax=3",
                "-i",
                "~/.ssh/id_ed25519",
                "admin@10.0.0.5",
            ]
        );
        assert!(build_ssh_args("", 22, "admin", "").is_err());
        assert!(build_ssh_args("10.0.0.5", 0, "admin", "").is_err());
        assert!(build_ssh_args("10.0.0.5\nrm", 22, "admin", "").is_err());
    }

    #[test]
    fn test_vt_terminal_parsing_colors_and_cursor() {
        let mut vt = VtTerminal::new(40, 10, 0x00FFFFFF, 0x00000000);
        vt.feed_bytes(b"\x1b[32mOK\x1b[0m\r\nLigne 2");
        assert_eq!(vt.visible_cell(0, 0).ch, 'O');
        assert_eq!(vt.visible_cell(0, 0).fg, ANSI_16_BGR[2]);
        assert_eq!(vt.visible_cell(0, 1).ch, 'K');
        assert_eq!(vt.visible_cell(1, 0).ch, 'L');
        assert_eq!(vt.cursor_y, 1);
        assert_eq!(vt.cursor_x, 7);

        // Effacer la ligne et revenir au début
        vt.feed_bytes(b"\r\x1b[2K");
        assert_eq!(vt.visible_cell(1, 0).ch, ' ');
    }

    #[test]
    fn test_should_skip_ssh_wm_char_prevents_double_enter() {
        assert!(should_skip_ssh_wm_char('\r', false));
        assert!(should_skip_ssh_wm_char('\n', false));
        assert!(should_skip_ssh_wm_char('\x08', false));
        assert!(should_skip_ssh_wm_char('\t', false));
        assert!(should_skip_ssh_wm_char('\x1b', false));
        assert!(!should_skip_ssh_wm_char('a', false));
        assert!(!should_skip_ssh_wm_char('\x03', true)); // Ctrl+C
    }
}
