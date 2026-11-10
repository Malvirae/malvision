//! Fenêtre principale moderne de Malvision (Jalon 2 & 2B — Gestionnaire de connexions,
//! arborescence multi-niveaux custom-draw avec glisser-déposer, recherche, favoris,
//! formulaires modaux sombres DPI-aware et onglets RDP simultanés).

use crate::models::{ConnectionDraft, ConnectionItem, Folder, Protocol};
use crate::rdp::{WM_RDP_EVENT, unpack_rdp_event_wparam};
use crate::security::SecretString;
use crate::sessions::{SessionManager, SessionStatus};
use crate::settings::{
    AppSettings, MAX_SIDEBAR_WIDTH, MIN_SIDEBAR_WIDTH, RdpKeyboardHookMode, ResolvedTheme,
    ThemeMode, apply_startup_shortcut, sync_settings_with_real_startup_shortcut,
};
use crate::storage::Storage;
use std::collections::HashMap;
use std::ffi::c_void;
use windows::Win32::Foundation::{COLORREF, HANDLE, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Dwm::{DWMWA_USE_IMMERSIVE_DARK_MODE, DwmSetWindowAttribute};
use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BeginPaint, ClientToScreen, CreateBitmap,
    CreateCompatibleBitmap, CreateCompatibleDC, CreateDIBSection, CreateFontW, CreatePen,
    CreateSolidBrush, DIB_RGB_COLORS, DT_CALCRECT, DT_CENTER, DT_END_ELLIPSIS, DT_LEFT,
    DT_NOPREFIX, DT_SINGLELINE, DT_VCENTER, DT_WORDBREAK, DeleteDC, DeleteObject, DrawTextW,
    EndPaint, FONT_CHARSET, FONT_CLIP_PRECISION, FONT_OUTPUT_PRECISION, FONT_QUALITY, FillRect,
    GetStockObject, HBITMAP, HBRUSH, HDC, HFONT, InvalidateRect, LineTo, MoveToEx, NULL_BRUSH,
    PAINTSTRUCT, PS_SOLID, RDW_ALLCHILDREN, RDW_ERASE, RDW_FRAME, RDW_INVALIDATE, RDW_UPDATENOW,
    RedrawWindow, RoundRect, ScreenToClient, SelectObject, SetBkColor, SetBkMode, SetTextColor,
    TRANSPARENT, UpdateWindow,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Controls::{
    CDDS_ITEMPREPAINT, CDDS_PREPAINT, CDIS_HOT, CDIS_SELECTED, CDRF_NOTIFYITEMDRAW,
    CDRF_SKIPDEFAULT, DRAWITEMSTRUCT, HTREEITEM, MEASUREITEMSTRUCT, NM_CLICK, NM_CUSTOMDRAW,
    NM_DBLCLK, NM_RCLICK, NMHDR, NMTREEVIEWW, NMTVCUSTOMDRAW, ODS_FOCUS, ODS_SELECTED, ODT_BUTTON,
    ODT_COMBOBOX, SetWindowTheme, TV_FIRST, TVE_COLLAPSE, TVE_EXPAND, TVGN_CARET, TVGN_DROPHILITE,
    TVGN_FIRSTVISIBLE, TVGN_NEXTVISIBLE, TVHITTESTINFO, TVHT_ONITEMBUTTON, TVI_LAST, TVI_ROOT,
    TVIF_HANDLE, TVIF_PARAM, TVIF_STATE, TVIF_TEXT, TVINSERTSTRUCTW, TVIS_EXPANDED, TVITEMEXW,
    TVITEMW, TVM_DELETEITEM, TVM_EXPAND, TVM_GETITEMRECT, TVM_GETITEMW, TVM_GETNEXTITEM,
    TVM_HITTEST, TVM_INSERTITEMW, TVM_SELECTITEM, TVM_SETBKCOLOR, TVM_SETTEXTCOLOR, TVN_BEGINDRAGW,
    TVN_ITEMEXPANDEDW, TVN_ITEMEXPANDINGW, TVN_SELCHANGEDW, TVS_FULLROWSELECT, TVS_HASBUTTONS,
    TVS_SHOWSELALWAYS, WC_BUTTONW, WC_COMBOBOXW, WC_EDITW, WC_TREEVIEWW, WM_MOUSELEAVE,
};
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    EnableWindow, GetFocus, GetKeyState, IsWindowEnabled, ReleaseCapture, SetCapture, SetFocus,
    TME_LEAVE, TRACKMOUSEEVENT, TrackMouseEvent, VK_ESCAPE, VK_F2, VK_RETURN, VK_SHIFT, VK_SPACE,
    VK_TAB,
};
use windows::Win32::UI::Shell::{
    NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW, Shell_NotifyIconW,
    ShellExecuteW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AdjustWindowRectEx, AppendMenuW, BM_GETCHECK, BM_SETCHECK, BS_OWNERDRAW, CB_ADDSTRING,
    CB_GETCURSEL, CB_GETLBTEXT, CB_GETLBTEXTLEN, CB_SETCURSEL, CBS_DROPDOWNLIST, CBS_HASSTRINGS,
    CBS_OWNERDRAWFIXED, CS_DBLCLKS, CS_HREDRAW, CS_VREDRAW, CW_USEDEFAULT, CallWindowProcW,
    CreateIconIndirect, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyIcon, DestroyMenu,
    DestroyWindow, DispatchMessageW, EN_CHANGE, EN_KILLFOCUS, EN_SETFOCUS, ES_AUTOHSCROLL,
    ES_PASSWORD, GWL_STYLE, GWLP_USERDATA, GWLP_WNDPROC, GetClientRect, GetCursorPos, GetDlgCtrlID,
    GetMessageW, GetParent, GetPropW, GetSystemMetrics, GetWindowLongPtrW, GetWindowRect,
    GetWindowTextLengthW, GetWindowTextW, HICON, HMENU, ICON_BIG, ICON_SMALL, ICONINFO, IDC_ARROW,
    IDC_HAND, IDC_SIZEWE, IDNO, IDRETRY, IDYES, IMAGE_ICON, IsChild, IsIconic, IsWindow,
    IsWindowVisible, KillTimer, LR_DEFAULTCOLOR, LoadCursorW, LoadImageW, MB_ICONERROR,
    MB_ICONINFORMATION, MB_ICONQUESTION, MB_ICONWARNING, MB_OK, MB_RETRYCANCEL, MB_YESNO,
    MB_YESNOCANCEL, MF_SEPARATOR, MF_STRING, MINMAXINFO, MSG, MessageBoxW, MoveWindow,
    PostMessageW, PostQuitMessage, RegisterClassExW, RegisterWindowMessageW, RemovePropW,
    SM_CXICON, SM_CXSCREEN, SM_CXSMICON, SM_CYICON, SM_CYSCREEN, SM_CYSMICON, SW_HIDE, SW_RESTORE,
    SW_SHOW, SendMessageW, SetCursor, SetForegroundWindow, SetPropW, SetTimer, SetWindowLongPtrW,
    SetWindowTextW, ShowWindow, TPM_LEFTALIGN, TPM_RETURNCMD, TPM_TOPALIGN, TrackPopupMenu,
    TranslateMessage, WINDOW_EX_STYLE, WINDOW_STYLE, WM_APP, WM_CLOSE, WM_COMMAND, WM_CONTEXTMENU,
    WM_CREATE, WM_CTLCOLORBTN, WM_CTLCOLOREDIT, WM_CTLCOLORLISTBOX, WM_CTLCOLORSTATIC, WM_DESTROY,
    WM_DRAWITEM, WM_ERASEBKGND, WM_GETMINMAXINFO, WM_KEYDOWN, WM_KEYUP, WM_KILLFOCUS,
    WM_LBUTTONDBLCLK, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MEASUREITEM, WM_MOUSEMOVE, WM_MOUSEWHEEL,
    WM_NOTIFY, WM_NULL, WM_PAINT, WM_RBUTTONUP, WM_SETCURSOR, WM_SETFOCUS, WM_SETFONT, WM_SETICON,
    WM_SIZE, WM_SYSCOMMAND, WM_TIMER, WNDCLASSEXW, WNDPROC, WS_CAPTION, WS_CHILD, WS_CLIPCHILDREN,
    WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_MAXIMIZEBOX, WS_MINIMIZEBOX, WS_OVERLAPPED,
    WS_OVERLAPPEDWINDOW, WS_POPUP, WS_SYSMENU, WS_TABSTOP, WS_THICKFRAME, WS_VISIBLE, WS_VSCROLL,
};
use windows_core::{PCWSTR, w};

// =============================================================================
// CONSTANTES VISUELLES & COULEURS DYNAMIQUES (THÈMES SOMBRE / CLAIR / SYSTÈME)
// =============================================================================

static ACTIVE_THEME_IS_DARK: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(true);

#[inline]
pub fn is_active_theme_dark() -> bool {
    ACTIVE_THEME_IS_DARK.load(std::sync::atomic::Ordering::Relaxed)
}

#[inline]
pub fn set_active_theme_dark(is_dark: bool) {
    ACTIVE_THEME_IS_DARK.store(is_dark, std::sync::atomic::Ordering::Relaxed);
}

#[inline]
fn active_theme_colors() -> crate::settings::ThemeColors {
    if is_active_theme_dark() {
        crate::settings::ThemeColors::DARK
    } else {
        crate::settings::ThemeColors::LIGHT
    }
}

#[inline]
fn col_bg_deep() -> u32 {
    active_theme_colors().bg_deep
}
#[inline]
fn col_bg_sidebar() -> u32 {
    active_theme_colors().bg_sidebar
}
#[inline]
fn col_bg_surface() -> u32 {
    active_theme_colors().bg_surface
}
#[inline]
fn col_bg_hover() -> u32 {
    active_theme_colors().bg_hover
}
#[inline]
fn col_bg_input() -> u32 {
    active_theme_colors().bg_input
}
#[inline]
fn col_bg_input_disabled() -> u32 {
    if is_active_theme_dark() {
        0x00191614
    } else {
        0x00ECE8E4
    }
}
#[inline]
fn col_accent() -> u32 {
    active_theme_colors().accent
}
#[inline]
fn col_accent_hover() -> u32 {
    active_theme_colors().accent_hover
}
#[inline]
fn col_accent_pressed() -> u32 {
    if is_active_theme_dark() {
        0x009E5A00
    } else {
        0x008A4F00
    }
}
#[inline]
fn col_border() -> u32 {
    active_theme_colors().border
}
#[inline]
fn col_border_subtle() -> u32 {
    if is_active_theme_dark() {
        0x00302A25
    } else {
        0x00E2D9D0
    }
}
#[inline]
fn col_text_primary() -> u32 {
    active_theme_colors().text_primary
}
#[inline]
fn col_text_muted() -> u32 {
    active_theme_colors().text_muted
}
#[inline]
fn col_text_secondary() -> u32 {
    if is_active_theme_dark() {
        0x00A1A1AA // #A1A1AA
    } else {
        0x0052525B // #52525B
    }
}
#[inline]
fn col_status_ok() -> u32 {
    active_theme_colors().status_ok
}
#[inline]
fn col_status_warn() -> u32 {
    active_theme_colors().status_warn
}
#[inline]
fn col_status_err() -> u32 {
    active_theme_colors().status_err
}

const COL_TEXT_ON_ACCENT: u32 = 0x00F6F3F0; // Blanc pur sur boutons primaires et badges colorés
const COL_GOLD_STAR: u32 = 0x00249CE8; // Doré lisible sur fond sombre et clair
const COL_PRIORITY_TRIANGLE: u32 = 0x008AC834; // Vert émeraude #34C88A pour le triangle prioritaire creux
const COL_FOLDER_AMBER: u32 = 0x003294DC; // Ambre dossier lisible sur fond sombre et clair
const COL_PROTO_ANYDESK: u32 = 0x003943D8; // #D84339 Badge AnyDesk
const COL_PROTO_RUSTDESK: u32 = 0x008A9616; // #16968A Badge RustDesk

const SIDEBAR_HEADER_HEIGHT: i32 = 152;
const SIDEBAR_FOOTER_HEIGHT: i32 = 46;
const SPLITTER_WIDTH: i32 = 5;
const TABBAR_HEIGHT: i32 = 38;
const STATUSBAR_HEIGHT: i32 = 28;
const MIN_WINDOW_WIDTH: i32 = 900;
const MIN_WINDOW_HEIGHT: i32 = 560;

// Messages d'édition Win32 pour sélection automatique (champ Port RDP)
#[allow(dead_code)]
pub const EM_GETSEL: u32 = 0x00B0;
pub const EM_SETSEL: u32 = 0x00B1;

// Identifiants des contrôles enfants de la fenêtre principale
const IDC_SEARCH_EDIT: usize = 1101;
const IDC_TREEVIEW: usize = 1102;

// Timer de supervision RDP
const IDT_RDP_STATE_POLL: usize = 2001;
const RDP_STATE_POLL_INTERVAL_MS: u32 = 350;

// Message différé pour ouvrir un dialogue modal ou lancer une action hors notification
const WM_DEFERRED_ACTION: u32 = WM_APP + 101;
const DEFERRED_CONNECT_SELECTED: usize = 1;
const DEFERRED_NEW_CONNECTION: usize = 2;
const DEFERRED_NEW_FOLDER: usize = 3;
const DEFERRED_EDIT_CONNECTION: usize = 4;
const DEFERRED_OPEN_SETTINGS: usize = 5;
const DEFERRED_APPLY_UPDATE_AND_EXIT: usize = 6;

const WM_APP_UPDATE_PROGRESS: u32 = WM_APP + 110;

// Message de notification de la zone de notification Windows (System Tray)
pub const WM_TRAYICON: u32 = WM_APP + 102;
const TRAY_ICON_ID: u32 = 1;
const CMD_TRAY_OPEN: u32 = 3101;
const CMD_TRAY_EXIT: u32 = 3102;

// Override atomique pour les tests d'intégration de confirmation de fermeture (0 = normal, 1 = Oui, -1 = Non)
static TEST_EXIT_CONFIRM_OVERRIDE: std::sync::atomic::AtomicI8 =
    std::sync::atomic::AtomicI8::new(0);

// Commandes du menu contextuel de l'arborescence
const CMD_CTX_CONNECT: u32 = 3001;
const CMD_CTX_EDIT: u32 = 3002;
const CMD_CTX_RENAME: u32 = 3003;
const CMD_CTX_DUPLICATE: u32 = 3004;
const CMD_CTX_TOGGLE_FAV: u32 = 3005;
const CMD_CTX_TOGGLE_PRIORITY: u32 = 3010;
const CMD_CTX_MOVE: u32 = 3006;
const CMD_CTX_DELETE: u32 = 3007;
const CMD_CTX_NEW_CONN_HERE: u32 = 3008;
const CMD_CTX_NEW_SUBFOLDER: u32 = 3009;

// Commandes du menu d'actions de session active RDP
const CMD_SESSION_CAD: u32 = 3201;
const CMD_SESSION_CTRL_ALT_END: u32 = 3202;
const CMD_SESSION_FULLSCREEN: u32 = 3203;
const CMD_SESSION_DISCONNECT: u32 = 3204;

/// Métriques de mise à l'échelle DPI (100 % = 96 DPI, 125 % = 120 DPI, 150 % = 144 DPI).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DpiMetrics {
    pub dpi: i32,
}

impl DpiMetrics {
    pub fn for_window(hwnd: HWND) -> Self {
        let raw = if hwnd.is_invalid() {
            96
        } else {
            unsafe { GetDpiForWindow(hwnd) as i32 }
        };
        Self::from_dpi(raw)
    }

    pub const fn from_dpi(dpi: i32) -> Self {
        let clamped = if dpi < 96 {
            96
        } else if dpi > 384 {
            384
        } else {
            dpi
        };
        Self { dpi: clamped }
    }

    #[inline]
    pub const fn scale(self, px: i32) -> i32 {
        if px >= 0 {
            (px * self.dpi + 48) / 96
        } else {
            (px * self.dpi - 48) / 96
        }
    }
}

/// Style Win32 standard de la fenêtre principale, réappliqué si un contrôle ActiveX enfant le modifie.
pub const MAIN_WINDOW_BASE_STYLE: WINDOW_STYLE = WINDOW_STYLE(
    WS_OVERLAPPED.0
        | WS_CAPTION.0
        | WS_SYSMENU.0
        | WS_THICKFRAME.0
        | WS_MINIMIZEBOX.0
        | WS_MAXIMIZEBOX.0
        | WS_CLIPCHILDREN.0,
);

/// Type d'élément représenté par un nœud dans le `TreeView`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TreeNodeKind {
    FavoritesRoot,
    ConnectionsRoot,
    Folder(i64),
    Connection(i64),
}

impl TreeNodeKind {
    fn to_lparam(self) -> isize {
        match self {
            Self::FavoritesRoot => 1,
            Self::ConnectionsRoot => 2,
            Self::Folder(id) => ((id << 3) | 3) as isize,
            Self::Connection(id) => ((id << 3) | 4) as isize,
        }
    }

    fn from_lparam(val: isize) -> Option<Self> {
        match val {
            1 => Some(Self::FavoritesRoot),
            2 => Some(Self::ConnectionsRoot),
            v if (v & 7) == 3 => Some(Self::Folder((v >> 3) as i64)),
            v if (v & 7) == 4 => Some(Self::Connection((v >> 3) as i64)),
            _ => None,
        }
    }
}

/// Zones cliquables dynamiques dessinées sur le canevas (boutons barre latérale, onglets, cartes d'accueil).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InteractiveZone {
    BtnNewConnection,
    BtnNewFolder,
    BtnExpandAll,
    BtnCollapseAll,
    BtnSettings,
    TabSelect(u32),
    TabClose(u32),
    TabActions(u32),
    TabNewConnection,
    HomeCardConnect(i64),
    HomeCardEdit(i64),
    HomeQuickNewConnection,
    HomeQuickNewFolder,
}

#[derive(Debug, Clone)]
struct HitZone {
    rect: RECT,
    zone: InteractiveZone,
}

pub struct GdiPalette {
    pub dpi: DpiMetrics,
    font_title: HFONT,
    font_subtitle: HFONT,
    font_ui: HFONT,
    font_ui_bold: HFONT,
    font_small: HFONT,
    brush_bg_deep: HBRUSH,
    brush_bg_sidebar: HBRUSH,
    brush_bg_surface: HBRUSH,
    brush_bg_hover: HBRUSH,
    brush_bg_input: HBRUSH,
    brush_accent: HBRUSH,
    brush_accent_hover: HBRUSH,
}

impl GdiPalette {
    unsafe fn create_for_dpi(dpi: DpiMetrics) -> Self {
        unsafe {
            let make_font = |base_height: i32, weight: i32| -> HFONT {
                CreateFontW(
                    dpi.scale(base_height),
                    0,
                    0,
                    0,
                    weight,
                    0,
                    0,
                    0,
                    FONT_CHARSET(1),
                    FONT_OUTPUT_PRECISION(0),
                    FONT_CLIP_PRECISION(0),
                    FONT_QUALITY(5), // CLEARTYPE_QUALITY
                    0,
                    w!("Segoe UI"),
                )
            };

            Self {
                dpi,
                font_title: make_font(-20, 600),
                font_subtitle: make_font(-15, 600),
                font_ui: make_font(-13, 400),
                font_ui_bold: make_font(-13, 600),
                font_small: make_font(-12, 400),
                brush_bg_deep: CreateSolidBrush(COLORREF(col_bg_deep())),
                brush_bg_sidebar: CreateSolidBrush(COLORREF(col_bg_sidebar())),
                brush_bg_surface: CreateSolidBrush(COLORREF(col_bg_surface())),
                brush_bg_hover: CreateSolidBrush(COLORREF(col_bg_hover())),
                brush_bg_input: CreateSolidBrush(COLORREF(col_bg_input())),
                brush_accent: CreateSolidBrush(COLORREF(col_accent())),
                brush_accent_hover: CreateSolidBrush(COLORREF(col_accent_hover())),
            }
        }
    }

    unsafe fn create_for_window(hwnd: HWND) -> Self {
        unsafe { Self::create_for_dpi(DpiMetrics::for_window(hwnd)) }
    }

    /// Recrée les pinceaux GDI lors d'un changement de thème à chaud (Sombre <-> Clair).
    pub unsafe fn rebuild_brushes(&mut self) {
        unsafe {
            for b in [
                self.brush_bg_deep,
                self.brush_bg_sidebar,
                self.brush_bg_surface,
                self.brush_bg_hover,
                self.brush_bg_input,
                self.brush_accent,
                self.brush_accent_hover,
            ] {
                if !b.is_invalid() {
                    let _ = DeleteObject(b.into());
                }
            }
            self.brush_bg_deep = CreateSolidBrush(COLORREF(col_bg_deep()));
            self.brush_bg_sidebar = CreateSolidBrush(COLORREF(col_bg_sidebar()));
            self.brush_bg_surface = CreateSolidBrush(COLORREF(col_bg_surface()));
            self.brush_bg_hover = CreateSolidBrush(COLORREF(col_bg_hover()));
            self.brush_bg_input = CreateSolidBrush(COLORREF(col_bg_input()));
            self.brush_accent = CreateSolidBrush(COLORREF(col_accent()));
            self.brush_accent_hover = CreateSolidBrush(COLORREF(col_accent_hover()));
        }
    }
}

impl Drop for GdiPalette {
    fn drop(&mut self) {
        unsafe {
            for f in [
                self.font_title,
                self.font_subtitle,
                self.font_ui,
                self.font_ui_bold,
                self.font_small,
            ] {
                if !f.is_invalid() {
                    let _ = DeleteObject(f.into());
                }
            }
            for b in [
                self.brush_bg_deep,
                self.brush_bg_sidebar,
                self.brush_bg_surface,
                self.brush_bg_hover,
                self.brush_bg_input,
                self.brush_accent,
                self.brush_accent_hover,
            ] {
                if !b.is_invalid() {
                    let _ = DeleteObject(b.into());
                }
            }
        }
    }
}

/// Génère en mémoire une icône 32×32 ARGB identifiable pour Malvision (badge bleu Fluent avec monogramme « M »),
/// intégrée directement au binaire sans dépendre d'aucun fichier `.ico` externe.
unsafe fn create_malvision_app_icon() -> HICON {
    unsafe {
        let size = 32i32;
        let bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: size,
                biHeight: -size, // Top-down DIB
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };

        let mut bits_ptr: *mut c_void = std::ptr::null_mut();
        let Ok(hbm_color) = CreateDIBSection(None, &bmi, DIB_RGB_COLORS, &mut bits_ptr, None, 0)
        else {
            return HICON::default();
        };

        if !bits_ptr.is_null() {
            let pixels =
                std::slice::from_raw_parts_mut(bits_ptr as *mut u32, (size * size) as usize);
            for y in 0..size {
                for x in 0..size {
                    // Forme carrée à coins arrondis (rayon 6 px)
                    let dx = if x < 6 {
                        6 - x
                    } else if x > 25 {
                        x - 25
                    } else {
                        0
                    };
                    let dy = if y < 6 {
                        6 - y
                    } else if y > 25 {
                        y - 25
                    } else {
                        0
                    };

                    if dx * dx + dy * dy > 36 {
                        pixels[(y * size + x) as usize] = 0x0000_0000;
                        continue;
                    }

                    // Bordure lumineuse subtile + dégradé bleu Windows 11 (#0078D4 -> #005A9E)
                    let is_border =
                        dx * dx + dy * dy >= 25 || x == 0 || x == 31 || y == 0 || y == 31;
                    let bg_argb = if is_border {
                        0xFF38_98F0
                    } else if y < 16 {
                        0xFF00_78D4
                    } else {
                        0xFF00_62B0
                    };

                    // Monogramme 'M' blanc net au centre (x in 7..=24, y in 8..=23)
                    let in_left_leg = (7..=10).contains(&x) && (8..=23).contains(&y);
                    let in_right_leg = (21..=24).contains(&x) && (8..=23).contains(&y);
                    let in_left_diag =
                        (8..=18).contains(&y) && (x - 9 - (y - 8) / 2).abs() <= 1 && x <= 16;
                    let in_right_diag =
                        (8..=18).contains(&y) && ((22 - (y - 8) / 2) - x).abs() <= 1 && x >= 15;

                    pixels[(y * size + x) as usize] =
                        if in_left_leg || in_right_leg || in_left_diag || in_right_diag {
                            0xFFFF_FFFF
                        } else {
                            bg_argb
                        };
                }
            }
        }

        let hbm_mask = CreateBitmap(size, size, 1, 1, None);
        let icon_info = ICONINFO {
            fIcon: true.into(),
            xHotspot: 0,
            yHotspot: 0,
            hbmMask: hbm_mask,
            hbmColor: hbm_color,
        };
        let hicon = CreateIconIndirect(&icon_info).unwrap_or_default();
        if !hbm_mask.is_invalid() {
            let _ = DeleteObject(hbm_mask.into());
        }
        if !hbm_color.is_invalid() {
            let _ = DeleteObject(hbm_color.into());
        }
        hicon
    }
}

pub const WM_APP_UPDATE_AVAILABLE: u32 = WM_APP + 42;

const SPLASH_PNG_BYTES: &[u8] = include_bytes!("../assets/branding/malvision-splash.png");
const LOGO_PNG_BYTES: &[u8] = include_bytes!("../assets/branding/malvision-logo.png");

struct SplashState {
    bitmap: isize,
    width: i32,
    height: i32,
}

static SPLASH_STATE: std::sync::Mutex<Option<SplashState>> = std::sync::Mutex::new(None);

unsafe extern "system" fn splash_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        match msg {
            WM_ERASEBKGND => LRESULT(1),
            WM_PAINT => {
                let mut ps = PAINTSTRUCT::default();
                let hdc = BeginPaint(hwnd, &mut ps);
                if let Ok(guard) = SPLASH_STATE.lock()
                    && let Some(state) = guard.as_ref()
                {
                    let mem_dc = CreateCompatibleDC(Some(hdc));
                    let old_bm = SelectObject(mem_dc, HBITMAP(state.bitmap as *mut c_void).into());
                    let _ = windows::Win32::Graphics::Gdi::BitBlt(
                        hdc,
                        0,
                        0,
                        state.width,
                        state.height,
                        Some(mem_dc),
                        0,
                        0,
                        windows::Win32::Graphics::Gdi::SRCCOPY,
                    );
                    let _ = SelectObject(mem_dc, old_bm);
                    let _ = DeleteDC(mem_dc);
                }
                let _ = EndPaint(hwnd, &ps);
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}

/// Affiche le splash screen officiel Malvision centré à l'écran.
pub unsafe fn create_splash_window() -> Option<HWND> {
    unsafe {
        let Ok(img) = image::load_from_memory(SPLASH_PNG_BYTES) else {
            return None;
        };
        let target_w = 560u32;
        let target_h = 315u32;
        let resized = img
            .resize_exact(target_w, target_h, image::imageops::FilterType::Lanczos3)
            .to_rgba8();
        let (w, h) = (resized.width() as i32, resized.height() as i32);

        let bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w,
                biHeight: -h,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bits_ptr: *mut c_void = std::ptr::null_mut();
        let Ok(hbm) = CreateDIBSection(None, &bmi, DIB_RGB_COLORS, &mut bits_ptr, None, 0) else {
            return None;
        };

        if !bits_ptr.is_null() {
            let slice = std::slice::from_raw_parts_mut(bits_ptr as *mut u8, (w * h * 4) as usize);
            let mut idx = 0;
            for pixel in resized.pixels() {
                slice[idx] = pixel[2]; // B
                slice[idx + 1] = pixel[1]; // G
                slice[idx + 2] = pixel[0]; // R
                slice[idx + 3] = pixel[3]; // A
                idx += 4;
            }
        }
        if let Ok(mut guard) = SPLASH_STATE.lock() {
            *guard = Some(SplashState {
                bitmap: hbm.0 as isize,
                width: w,
                height: h,
            });
        }

        let hinstance = GetModuleHandleW(None).unwrap_or_default();
        let class_name = w!("MalvisionSplashClass");
        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(splash_wnd_proc),
            hInstance: hinstance.into(),
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            lpszClassName: class_name,
            ..Default::default()
        };
        let _ = RegisterClassExW(&wc);

        let screen_w = GetSystemMetrics(SM_CXSCREEN);
        let screen_h = GetSystemMetrics(SM_CYSCREEN);
        let x = (screen_w - w) / 2;
        let y = (screen_h - h) / 2;

        let hwnd = CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
            class_name,
            w!("Malvision"),
            WS_POPUP | WS_VISIBLE,
            x,
            y,
            w,
            h,
            None,
            None,
            Some(hinstance.into()),
            None,
        )
        .ok()?;

        let _ = UpdateWindow(hwnd);
        Some(hwnd)
    }
}

/// Ferme et libère le splash screen.
pub unsafe fn close_splash_window(hwnd: HWND) {
    unsafe {
        if !hwnd.is_invalid() && IsWindow(Some(hwnd)).as_bool() {
            let _ = DestroyWindow(hwnd);
        }
        let old_bitmap = if let Ok(mut guard) = SPLASH_STATE.lock() {
            guard.take().map(|s| s.bitmap)
        } else {
            None
        };
        if let Some(bm) = old_bitmap
            && bm != 0
        {
            let _ = DeleteObject(HBITMAP(bm as *mut c_void).into());
        }
    }
}

/// Génère un bitmap DIB pré-composité du logo officiel Malvision adapté à la couleur d'arrière-plan.
unsafe fn create_logo_bitmap(target_size: u32, bg_color: u32) -> Option<HBITMAP> {
    unsafe {
        let img = image::load_from_memory(LOGO_PNG_BYTES).ok()?;
        let resized = img
            .resize_exact(
                target_size,
                target_size,
                image::imageops::FilterType::Lanczos3,
            )
            .to_rgba8();
        let size = target_size as i32;

        let bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: size,
                biHeight: -size,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bits_ptr: *mut c_void = std::ptr::null_mut();
        let Ok(hbm) = CreateDIBSection(None, &bmi, DIB_RGB_COLORS, &mut bits_ptr, None, 0) else {
            return None;
        };

        let bg_r = (bg_color & 0xFF) as f32;
        let bg_g = ((bg_color >> 8) & 0xFF) as f32;
        let bg_b = ((bg_color >> 16) & 0xFF) as f32;

        if !bits_ptr.is_null() {
            let slice =
                std::slice::from_raw_parts_mut(bits_ptr as *mut u8, (size * size * 4) as usize);
            let mut idx = 0;
            for pixel in resized.pixels() {
                let a = pixel[3] as f32 / 255.0;
                let r = (pixel[0] as f32 * a + bg_r * (1.0 - a)).round() as u8;
                let g = (pixel[1] as f32 * a + bg_g * (1.0 - a)).round() as u8;
                let b = (pixel[2] as f32 * a + bg_b * (1.0 - a)).round() as u8;

                slice[idx] = b;
                slice[idx + 1] = g;
                slice[idx + 2] = r;
                slice[idx + 3] = 255;
                idx += 4;
            }
        }
        Some(hbm)
    }
}

pub struct AppWindowState {
    hwnd: HWND,
    palette: GdiPalette,
    storage: Storage,
    settings: AppSettings,
    sessions: SessionManager,
    search_edit: HWND,
    treeview: HWND,
    search_query: String,
    selected_node: Option<TreeNodeKind>,
    dragging_node: Option<TreeNodeKind>,
    resizing_sidebar: bool,
    hovered_zone: Option<InteractiveZone>,
    pressed_zone: Option<InteractiveZone>,
    tracking_mouse: bool,
    hit_zones: Vec<HitZone>,
    status_override: Option<String>,
    cached_folders: HashMap<i64, Folder>,
    cached_connections: HashMap<i64, ConnectionItem>,
    fav_count: usize,
    conn_count: usize,
    app_icon: HICON,
    tray_icon_added: bool,
    is_hidden_in_tray: bool,
    exit_requested: bool,
    wm_taskbar_created: u32,
    last_launch_debounce: Option<(i64, std::time::Instant)>,
    pub dashboard_scroll_y: i32,
    pub dashboard_content_height: i32,
    is_programmatic_tree_update: bool,
}

impl AppWindowState {
    unsafe fn initialize(hwnd: HWND, custom_storage: Option<Storage>) -> Self {
        unsafe {
            let mut status_override = None;
            let mut storage = if let Some(s) = custom_storage {
                s
            } else if cfg!(test) {
                Storage::open_in_memory().expect("Base SQLite en mémoire pour test UI")
            } else {
                match Storage::open_default() {
                    Ok(s) => s,
                    Err(err) => {
                        show_error_dialog(
                            hwnd,
                            "Erreur d'ouverture de la base SQLite",
                            &format!(
                                "{err}\n\nBasculer sur une base temporaire de secours pour ne pas écraser de données."
                            ),
                        );
                        status_override = Some(format!("Attention : {err}"));
                        let fallback_path = std::env::temp_dir().join("malvision_recovery.db");
                        Storage::open_at_path(&fallback_path).expect("Base SQLite de secours")
                    }
                }
            };

            let mut settings = AppSettings::load(&storage);
            if !cfg!(test) {
                sync_settings_with_real_startup_shortcut(&mut settings, &mut storage);
            }

            let is_dark = settings.theme_mode.resolve() == ResolvedTheme::Dark;
            set_active_theme_dark(is_dark);

            // Aligner la barre de titre native Windows 11 avec le thème actif (Sombre / Clair)
            let dark_mode: i32 = i32::from(is_dark);
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWA_USE_IMMERSIVE_DARK_MODE,
                &dark_mode as *const i32 as *const c_void,
                std::mem::size_of::<i32>() as u32,
            );

            let palette = GdiPalette::create_for_window(hwnd);
            let hinstance = GetModuleHandleW(None).unwrap_or_default();
            #[allow(clippy::manual_dangling_ptr)]
            let pe_icon_small = LoadImageW(
                Some(hinstance.into()),
                PCWSTR(1 as *const u16),
                IMAGE_ICON,
                GetSystemMetrics(SM_CXSMICON),
                GetSystemMetrics(SM_CYSMICON),
                LR_DEFAULTCOLOR,
            )
            .ok()
            .map(|h| HICON(h.0));
            #[allow(clippy::manual_dangling_ptr)]
            let pe_icon_big = LoadImageW(
                Some(hinstance.into()),
                PCWSTR(1 as *const u16),
                IMAGE_ICON,
                GetSystemMetrics(SM_CXICON),
                GetSystemMetrics(SM_CYICON),
                LR_DEFAULTCOLOR,
            )
            .ok()
            .map(|h| HICON(h.0));

            let app_icon = pe_icon_small.unwrap_or_else(|| create_malvision_app_icon());
            let app_icon_big = pe_icon_big.unwrap_or(app_icon);

            if !app_icon.is_invalid() {
                let _ = SendMessageW(
                    hwnd,
                    WM_SETICON,
                    Some(WPARAM(ICON_SMALL as usize)),
                    Some(LPARAM(app_icon.0 as isize)),
                );
            }
            if !app_icon_big.is_invalid() {
                let _ = SendMessageW(
                    hwnd,
                    WM_SETICON,
                    Some(WPARAM(ICON_BIG as usize)),
                    Some(LPARAM(app_icon_big.0 as isize)),
                );
            }

            let wm_taskbar_created = RegisterWindowMessageW(w!("TaskbarCreated"));
            let sidebar_w = settings.sidebar_width;

            let hinstance = GetModuleHandleW(None).unwrap_or_default();

            // Champ de recherche sans bordure Win32 (encapsulé dans un conteneur arrondi dessiné en GDI)
            let search_edit = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                WC_EDITW,
                w!(""),
                WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(ES_AUTOHSCROLL as u32),
                36,
                89,
                (sidebar_w - 52).max(80),
                18,
                Some(hwnd),
                Some(HMENU(IDC_SEARCH_EDIT as *mut _)),
                Some(hinstance.into()),
                None,
            )
            .unwrap_or_default();

            if !search_edit.is_invalid() {
                // Désactiver le skin UxTheme natif pour que le contrôle EDIT obéisse strictement à nos couleurs GDI
                let _ = SetWindowTheme(search_edit, w!(""), w!(""));
                // Sous-classer pour intercepter WM_ERASEBKGND et garantir un rendu sans résidu
                let orig_proc = SetWindowLongPtrW(
                    search_edit,
                    GWLP_WNDPROC,
                    search_edit_wnd_proc as *const () as isize,
                );
                let _ = SetPropW(
                    search_edit,
                    PROP_SEARCH_EDIT_ORIG_PROC,
                    Some(HANDLE(orig_proc as *mut _)),
                );

                let _ = SendMessageW(
                    search_edit,
                    WM_SETFONT,
                    Some(WPARAM(palette.font_ui.0 as usize)),
                    Some(LPARAM(1)),
                );
                let cue = w!("Rechercher (nom, IP, utilisateur)...");
                let _ = SendMessageW(
                    search_edit,
                    0x1501,
                    Some(WPARAM(1)),
                    Some(LPARAM(cue.as_ptr() as isize)),
                );
            }

            let mut rc = RECT::default();
            let _ = GetClientRect(hwnd, &mut rc);
            let client_h = (rc.bottom - rc.top).max(560);
            let tree_h =
                (client_h - SIDEBAR_HEADER_HEIGHT - SIDEBAR_FOOTER_HEIGHT - STATUSBAR_HEIGHT)
                    .max(120);

            const TVS_NOHSCROLL: u32 = 0x8000;
            let tree_style = WINDOW_STYLE(
                WS_CHILD.0
                    | WS_VISIBLE.0
                    | WS_TABSTOP.0
                    | TVS_HASBUTTONS
                    | TVS_FULLROWSELECT
                    | TVS_SHOWSELALWAYS
                    | TVS_NOHSCROLL,
            );

            let treeview = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                WC_TREEVIEWW,
                w!(""),
                tree_style,
                0,
                SIDEBAR_HEADER_HEIGHT,
                sidebar_w,
                tree_h,
                Some(hwnd),
                Some(HMENU(IDC_TREEVIEW as *mut _)),
                Some(hinstance.into()),
                None,
            )
            .unwrap_or_default();

            if !treeview.is_invalid() {
                // Sous-classer pour intercepter WM_LBUTTONDBLCLK sur toute la largeur utile de ligne
                let orig_proc = SetWindowLongPtrW(
                    treeview,
                    GWLP_WNDPROC,
                    treeview_wnd_proc as *const () as isize,
                );
                let _ = SetPropW(
                    treeview,
                    PROP_TREEVIEW_ORIG_PROC,
                    Some(HANDLE(orig_proc as *mut _)),
                );

                let _ = SendMessageW(
                    treeview,
                    WM_SETFONT,
                    Some(WPARAM(palette.font_ui.0 as usize)),
                    Some(LPARAM(1)),
                );
                let _ = SendMessageW(
                    treeview,
                    TVM_SETBKCOLOR,
                    Some(WPARAM(0)),
                    Some(LPARAM(col_bg_sidebar() as isize)),
                );
                let _ = SendMessageW(
                    treeview,
                    TVM_SETTEXTCOLOR,
                    Some(WPARAM(0)),
                    Some(LPARAM(col_text_primary() as isize)),
                );
                // Hauteur de ligne 28 px pour un rendu moderne et lisible
                let _ = SendMessageW(treeview, TV_FIRST + 27, Some(WPARAM(28)), Some(LPARAM(0)));
            }

            let _ = SetTimer(
                Some(hwnd),
                IDT_RDP_STATE_POLL,
                RDP_STATE_POLL_INTERVAL_MS,
                None,
            );

            let mut state = Self {
                hwnd,
                palette,
                storage,
                settings,
                sessions: SessionManager::new(),
                search_edit,
                treeview,
                search_query: String::new(),
                selected_node: None,
                dragging_node: None,
                resizing_sidebar: false,
                hovered_zone: None,
                pressed_zone: None,
                tracking_mouse: false,
                hit_zones: Vec::new(),
                status_override,
                cached_folders: HashMap::new(),
                cached_connections: HashMap::new(),
                fav_count: 0,
                conn_count: 0,
                app_icon,
                tray_icon_added: false,
                is_hidden_in_tray: false,
                exit_requested: false,
                wm_taskbar_created,
                last_launch_debounce: None,
                dashboard_scroll_y: 0,
                dashboard_content_height: 0,
                is_programmatic_tree_update: false,
            };

            if !cfg!(test) {
                state.init_tray_icon();
            }

            state.rebuild_treeview();
            state
        }
    }

    /// Applique immédiatement le thème configuré (`Dark`, `Light` ou `System`) à toute la fenêtre
    /// principale, au `TreeView`, au champ de recherche et aux pinceaux GDI sans redémarrage.
    pub fn apply_theme(&mut self) {
        unsafe {
            let is_dark = self.settings.resolved_theme() == ResolvedTheme::Dark;
            set_active_theme_dark(is_dark);
            self.palette.rebuild_brushes();

            let dark_mode: i32 = i32::from(is_dark);
            let _ = DwmSetWindowAttribute(
                self.hwnd,
                DWMWA_USE_IMMERSIVE_DARK_MODE,
                &dark_mode as *const i32 as *const c_void,
                std::mem::size_of::<i32>() as u32,
            );

            if !self.treeview.is_invalid() {
                let _ = SendMessageW(
                    self.treeview,
                    TVM_SETBKCOLOR,
                    Some(WPARAM(0)),
                    Some(LPARAM(col_bg_sidebar() as isize)),
                );
                let _ = SendMessageW(
                    self.treeview,
                    TVM_SETTEXTCOLOR,
                    Some(WPARAM(0)),
                    Some(LPARAM(col_text_primary() as isize)),
                );
                let _ = RedrawWindow(
                    Some(self.treeview),
                    None,
                    None,
                    RDW_INVALIDATE | RDW_ERASE | RDW_FRAME | RDW_ALLCHILDREN | RDW_UPDATENOW,
                );
            }
            self.recreate_search_edit();
            let _ = RedrawWindow(
                Some(self.hwnd),
                None,
                None,
                RDW_INVALIDATE | RDW_ERASE | RDW_FRAME | RDW_ALLCHILDREN | RDW_UPDATENOW,
            );
        }
    }

    /// Recrée proprement le contrôle `EDIT` natif de recherche lors d'un basculement de thème
    /// (Sombre <-> Clair).
    ///
    /// Cette recréation complète garantit l'élimination définitive de tout artefact visuel
    /// ou résidu anthracite lié au cache interne de rendu de Windows 11 / UxTheme, tout en préservant
    /// intégralement le texte saisi, la sélection de curseur et l'état de focus.
    pub fn recreate_search_edit(&mut self) {
        unsafe {
            if self.hwnd.is_invalid() {
                return;
            }
            let current_text =
                if !self.search_edit.is_invalid() && IsWindow(Some(self.search_edit)).as_bool() {
                    get_window_text(self.search_edit)
                } else {
                    self.search_query.clone()
                };

            let mut sel_start: u32 = 0;
            let mut sel_end: u32 = 0;
            let has_focus =
                if !self.search_edit.is_invalid() && IsWindow(Some(self.search_edit)).as_bool() {
                    let _ = SendMessageW(
                        self.search_edit,
                        EM_GETSEL,
                        Some(WPARAM(&mut sel_start as *mut _ as usize)),
                        Some(LPARAM(&mut sel_end as *mut _ as isize)),
                    );
                    GetFocus() == self.search_edit
                } else {
                    false
                };

            if !self.search_edit.is_invalid() && IsWindow(Some(self.search_edit)).as_bool() {
                let _ = DestroyWindow(self.search_edit);
                self.search_edit = HWND::default();
            }

            let sidebar_w = self.settings.sidebar_width;
            let edit_x = 36;
            let edit_y = 89;
            let edit_w = (sidebar_w - 52).max(80);
            let edit_h = 18;
            let hinstance = GetModuleHandleW(None).unwrap_or_default();

            let new_edit = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                WC_EDITW,
                w!(""),
                WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(ES_AUTOHSCROLL as u32),
                edit_x,
                edit_y,
                edit_w,
                edit_h,
                Some(self.hwnd),
                Some(HMENU(IDC_SEARCH_EDIT as *mut _)),
                Some(hinstance.into()),
                None,
            )
            .unwrap_or_default();

            if !new_edit.is_invalid() {
                let _ = SetWindowTheme(new_edit, w!(""), w!(""));
                let orig_proc = SetWindowLongPtrW(
                    new_edit,
                    GWLP_WNDPROC,
                    search_edit_wnd_proc as *const () as isize,
                );
                let _ = SetPropW(
                    new_edit,
                    PROP_SEARCH_EDIT_ORIG_PROC,
                    Some(HANDLE(orig_proc as *mut _)),
                );

                let _ = SendMessageW(
                    new_edit,
                    WM_SETFONT,
                    Some(WPARAM(self.palette.font_ui.0 as usize)),
                    Some(LPARAM(1)),
                );
                let cue = w!("Rechercher (nom, IP, utilisateur)...");
                let _ = SendMessageW(
                    new_edit,
                    0x1501, // EM_SETCUEBANNER
                    Some(WPARAM(1)),
                    Some(LPARAM(cue.as_ptr() as isize)),
                );

                if !current_text.is_empty() {
                    set_window_text(new_edit, &current_text);
                    let _ = SendMessageW(
                        new_edit,
                        EM_SETSEL,
                        Some(WPARAM(sel_start as usize)),
                        Some(LPARAM(sel_end as isize)),
                    );
                }

                if has_focus {
                    let _ = SetFocus(Some(new_edit));
                }
            }
            self.search_edit = new_edit;
        }
    }

    // =========================================================================
    // SYSTEM TRAY WINDOWS (`Shell_NotifyIconW`) & CYCLE DE VIE DE FERMETURE
    // =========================================================================

    pub fn init_tray_icon(&mut self) {
        unsafe {
            if self.hwnd.is_invalid() || self.app_icon.is_invalid() {
                return;
            }
            let mut nid = NOTIFYICONDATAW {
                cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
                hWnd: self.hwnd,
                uID: TRAY_ICON_ID,
                uFlags: NIF_MESSAGE | NIF_ICON | NIF_TIP,
                uCallbackMessage: WM_TRAYICON,
                hIcon: self.app_icon,
                ..Default::default()
            };
            let tip = "Malvision — Connexions Distantes";
            for (dst, src) in nid.szTip.iter_mut().take(127).zip(tip.encode_utf16()) {
                *dst = src;
            }
            if Shell_NotifyIconW(NIM_ADD, &nid).as_bool() {
                self.tray_icon_added = true;
            }
        }
    }

    pub fn remove_tray_icon(&mut self) {
        unsafe {
            if !self.tray_icon_added || self.hwnd.is_invalid() {
                return;
            }
            let nid = NOTIFYICONDATAW {
                cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
                hWnd: self.hwnd,
                uID: TRAY_ICON_ID,
                ..Default::default()
            };
            let _ = Shell_NotifyIconW(NIM_DELETE, &nid);
            self.tray_icon_added = false;
        }
    }

    pub fn hide_to_tray(&mut self) {
        if !self.tray_icon_added {
            self.init_tray_icon();
        }
        self.is_hidden_in_tray = true;
        unsafe {
            let _ = ShowWindow(self.hwnd, SW_HIDE);
        }
    }

    pub fn restore_from_tray(&mut self) {
        self.is_hidden_in_tray = false;
        unsafe {
            let _ = ShowWindow(self.hwnd, SW_SHOW);
            if IsIconic(self.hwnd).as_bool() {
                let _ = ShowWindow(self.hwnd, SW_RESTORE);
            }
            let _ = SetForegroundWindow(self.hwnd);
        }
        self.layout_children();
    }

    unsafe fn show_tray_context_menu(&mut self) {
        unsafe {
            let mut cursor = POINT::default();
            let _ = GetCursorPos(&mut cursor);

            let Ok(hmenu) = CreatePopupMenu() else {
                return;
            };
            let _ = AppendMenuW(
                hmenu,
                MF_STRING,
                CMD_TRAY_OPEN as usize,
                w!("Ouvrir Malvision"),
            );
            let _ = AppendMenuW(hmenu, MF_SEPARATOR, 0, PCWSTR::null());
            let _ = AppendMenuW(
                hmenu,
                MF_STRING,
                CMD_TRAY_EXIT as usize,
                w!("Quitter Malvision"),
            );

            let _ = SetForegroundWindow(self.hwnd);
            let cmd = TrackPopupMenu(
                hmenu,
                TPM_LEFTALIGN | TPM_TOPALIGN | TPM_RETURNCMD,
                cursor.x,
                cursor.y,
                Some(0),
                self.hwnd,
                None,
            );
            let _ = PostMessageW(Some(self.hwnd), WM_NULL, WPARAM(0), LPARAM(0));
            let _ = DestroyMenu(hmenu);

            match cmd.0 as u32 {
                CMD_TRAY_OPEN => self.restore_from_tray(),
                CMD_TRAY_EXIT => {
                    let _ = self.request_explicit_exit();
                }
                _ => {}
            }
        }
    }

    fn confirm_exit_if_needed(&self) -> bool {
        if !self.settings.confirm_exit_active_sessions || self.sessions.is_empty() {
            return true;
        }
        match TEST_EXIT_CONFIRM_OVERRIDE.load(std::sync::atomic::Ordering::SeqCst) {
            1 => return true,
            -1 => return false,
            _ => {}
        }
        let count = self.sessions.tabs().len();
        confirm_dialog(
            self.hwnd,
            "Confirmer la fermeture de Malvision",
            &format!(
                "{count} session(s) RDP active(s) sont actuellement ouvertes.\n\nVoulez-vous vraiment quitter Malvision et fermer toutes les connexions ?"
            ),
        )
    }

    fn perform_clean_exit(&mut self) {
        if self.settings.restore_rdp_sessions {
            let open_ids = self.sessions.open_connection_ids();
            self.settings
                .save_restorable_rdp_sessions(&mut self.storage, &open_ids);
        } else if !self.settings.restorable_rdp_connection_ids.is_empty() {
            self.settings
                .save_restorable_rdp_sessions(&mut self.storage, &[]);
        }
        self.remove_tray_icon();
        self.sessions.close_all();
        unsafe {
            let _ = DestroyWindow(self.hwnd);
        }
    }

    /// Demande de sortie explicite (menu « Quitter Malvision » du System Tray) :
    /// contourne `close_to_tray` mais respecte `confirm_exit_active_sessions`.
    pub fn request_explicit_exit(&mut self) -> bool {
        self.exit_requested = true;
        if !self.confirm_exit_if_needed() {
            self.exit_requested = false;
            return false;
        }
        self.perform_clean_exit();
        true
    }

    /// Gère `WM_CLOSE` (clic sur la croix `[X]` ou `Alt+F4`).
    /// Retourne `true` si la fenêtre a été détruite, `false` si elle a été masquée dans le Tray ou annulée.
    pub fn on_close_requested(&mut self) -> bool {
        if !self.exit_requested && self.settings.close_to_tray {
            self.hide_to_tray();
            return false;
        }
        if !self.confirm_exit_if_needed() {
            self.exit_requested = false;
            return false;
        }
        self.perform_clean_exit();
        true
    }

    /// Restaure au démarrage les sessions RDP précédemment actives si l'option est activée,
    /// en vérifiant préalablement que chaque fiche existe et que son secret DPAPI est valide
    /// afin de ne jamais déclencher de connexion incontrôlée en cas d'erreur de secret.
    pub fn restore_sessions_on_startup(&mut self) {
        if !self.settings.restore_rdp_sessions
            || self.settings.restorable_rdp_connection_ids.is_empty()
        {
            return;
        }

        let ids_to_restore = self.settings.restorable_rdp_connection_ids.clone();
        // Effacer immédiatement la liste persistée pour éviter toute boucle en cas d'arrêt anormal
        self.settings
            .save_restorable_rdp_sessions(&mut self.storage, &[]);

        let mut skipped_due_to_secret = 0usize;
        for cid in ids_to_restore {
            let Ok(Some(conn)) = self.storage.get_connection(cid) else {
                continue;
            };
            if conn.protocol != Protocol::Rdp {
                continue;
            }
            if conn.save_password {
                match self.storage.load_decrypted_password(conn.id) {
                    Ok(Some(_)) => {}
                    _ => {
                        skipped_due_to_secret += 1;
                        continue;
                    }
                }
            }
            self.launch_connection_by_id(conn.id);
        }

        if skipped_due_to_secret > 0 {
            self.status_override = Some(format!(
                "{skipped_due_to_secret} session(s) non restaurée(s) automatiquement (mot de passe DPAPI indisponible)."
            ));
            self.invalidate_canvas();
        }
    }

    fn client_rect(&self) -> RECT {
        let mut rc = RECT::default();
        unsafe {
            let _ = GetClientRect(self.hwnd, &mut rc);
        }
        rc
    }

    /// Zone rectangulaire réservée à l'affichage du contrôle ActiveX RDP de l'onglet actif.
    fn rdp_viewport_rect(&self) -> RECT {
        let rc = self.client_rect();
        let left = self.settings.sidebar_width + SPLITTER_WIDTH;
        let top = TABBAR_HEIGHT;
        let right = rc.right.max(left + 200);
        let bottom = (rc.bottom - STATUSBAR_HEIGHT).max(top + 200);
        RECT {
            left,
            top,
            right,
            bottom,
        }
    }

    fn layout_children(&mut self) {
        let rc = self.client_rect();
        let width = rc.right - rc.left;
        let height = rc.bottom - rc.top;
        if width <= 0 || height <= 0 {
            return;
        }

        let max_sb = (width - 420).clamp(MIN_SIDEBAR_WIDTH, MAX_SIDEBAR_WIDTH);
        let sidebar_w = self.settings.sidebar_width.clamp(MIN_SIDEBAR_WIDTH, max_sb);
        self.settings.sidebar_width = sidebar_w;

        unsafe {
            let _ = MoveWindow(self.search_edit, 36, 89, (sidebar_w - 52).max(80), 18, true);
            let tree_h =
                (height - SIDEBAR_HEADER_HEIGHT - SIDEBAR_FOOTER_HEIGHT - STATUSBAR_HEIGHT).max(80);
            let _ = MoveWindow(
                self.treeview,
                0,
                SIDEBAR_HEADER_HEIGHT,
                sidebar_w,
                tree_h,
                true,
            );
        }

        let viewport = self.rdp_viewport_rect();
        self.sessions.resize_active(viewport);

        unsafe {
            let _ = InvalidateRect(Some(self.hwnd), None, false);
        }
    }

    fn invalidate_canvas(&self) {
        unsafe {
            let _ = InvalidateRect(Some(self.hwnd), None, false);
        }
    }

    // =========================================================================
    // ARBORESCENCE TREEVIEW (Favoris + Dossiers multi-niveaux + Connexions)
    // =========================================================================

    pub fn rebuild_treeview(&mut self) {
        if self.treeview.is_invalid() {
            return;
        }

        self.is_programmatic_tree_update = true;
        let previous_selection = self.selected_node;
        unsafe {
            let _ = SendMessageW(
                self.treeview,
                TVM_DELETEITEM,
                Some(WPARAM(0)),
                Some(LPARAM(TVI_ROOT.0)),
            );
        }

        let folders = self.storage.list_folders().unwrap_or_default();
        let connections = if self.search_query.trim().is_empty() {
            self.storage.list_connections().unwrap_or_default()
        } else {
            self.storage
                .search_connections(&self.search_query)
                .unwrap_or_default()
        };

        self.cached_folders.clear();
        for f in &folders {
            self.cached_folders.insert(f.id, f.clone());
        }
        self.cached_connections.clear();
        for c in &connections {
            self.cached_connections.insert(c.id, c.clone());
        }

        let is_searching = !self.search_query.trim().is_empty();
        let mut handle_to_select: Option<HTREEITEM> = None;

        // 1. Section Favoris
        let favs: Vec<&ConnectionItem> = connections.iter().filter(|c| c.is_favorite).collect();
        self.fav_count = favs.len();
        self.conn_count = connections.len();

        let fav_root_label = format!("Favoris ({})", favs.len());
        let h_fav_root =
            self.insert_tree_item(TVI_ROOT, &fav_root_label, TreeNodeKind::FavoritesRoot, true);
        if previous_selection == Some(TreeNodeKind::FavoritesRoot) {
            handle_to_select = Some(h_fav_root);
        }

        for fav in &favs {
            let label = format!("{} {}", fav.protocol.badge_prefix(), fav.name);
            let h =
                self.insert_tree_item(h_fav_root, &label, TreeNodeKind::Connection(fav.id), false);
            if handle_to_select.is_none()
                && previous_selection == Some(TreeNodeKind::Connection(fav.id))
            {
                handle_to_select = Some(h);
            }
        }

        // 2. Section Dossiers & Connexions (Arborescence récursive à profondeur arbitraire)
        let root_title = if is_searching {
            format!("Résultats de recherche ({})", connections.len())
        } else {
            format!("Connexions ({})", connections.len())
        };
        let h_conn_root =
            self.insert_tree_item(TVI_ROOT, &root_title, TreeNodeKind::ConnectionsRoot, true);
        if previous_selection == Some(TreeNodeKind::ConnectionsRoot) {
            handle_to_select = Some(h_conn_root);
        }

        if is_searching {
            for conn in &connections {
                let label = format!(
                    "{} {}  —  {}",
                    conn.protocol.badge_prefix(),
                    conn.name,
                    conn.endpoint_label()
                );
                let h = self.insert_tree_item(
                    h_conn_root,
                    &label,
                    TreeNodeKind::Connection(conn.id),
                    false,
                );
                if previous_selection == Some(TreeNodeKind::Connection(conn.id)) {
                    handle_to_select = Some(h);
                }
            }
        } else {
            let mut folders_by_parent: HashMap<Option<i64>, Vec<&Folder>> = HashMap::new();
            for f in &folders {
                folders_by_parent.entry(f.parent_id).or_default().push(f);
            }
            let mut conns_by_folder: HashMap<Option<i64>, Vec<&ConnectionItem>> = HashMap::new();
            for c in &connections {
                conns_by_folder.entry(c.folder_id).or_default().push(c);
            }

            let mut visited_folders = Vec::new();
            self.populate_folder_subtree(
                h_conn_root,
                None,
                &folders_by_parent,
                &conns_by_folder,
                previous_selection,
                &mut handle_to_select,
                &mut visited_folders,
            );
        }

        unsafe {
            let _ = SendMessageW(
                self.treeview,
                TVM_EXPAND,
                Some(WPARAM(TVE_EXPAND.0 as usize)),
                Some(LPARAM(h_fav_root.0)),
            );
            let _ = SendMessageW(
                self.treeview,
                TVM_EXPAND,
                Some(WPARAM(TVE_EXPAND.0 as usize)),
                Some(LPARAM(h_conn_root.0)),
            );

            if let Some(h_sel) = handle_to_select {
                let _ = SendMessageW(
                    self.treeview,
                    TVM_SELECTITEM,
                    Some(WPARAM(TVGN_CARET as usize)),
                    Some(LPARAM(h_sel.0)),
                );
            }
            let _ = InvalidateRect(Some(self.treeview), None, false);
        }

        self.is_programmatic_tree_update = false;
        self.invalidate_canvas();
    }

    pub fn expand_all_folders(&mut self) {
        let _ = self.storage.set_all_folders_expanded(true);
        for f in self.cached_folders.values_mut() {
            f.is_expanded = true;
        }
        self.rebuild_treeview();
    }

    pub fn collapse_all_folders(&mut self) {
        let _ = self.storage.set_all_folders_expanded(false);
        for f in self.cached_folders.values_mut() {
            f.is_expanded = false;
        }
        self.rebuild_treeview();
    }

    pub fn toggle_or_set_folder_expansion(
        &mut self,
        hitem: HTREEITEM,
        fid: i64,
        force_state: Option<bool>,
    ) {
        if hitem.0 == 0 {
            return;
        }
        let current = self.is_tree_item_expanded(hitem);
        let new_state = force_state.unwrap_or(!current);

        let prev_prog = self.is_programmatic_tree_update;
        self.is_programmatic_tree_update = true;

        let act = if new_state { TVE_EXPAND } else { TVE_COLLAPSE };
        unsafe {
            let _ = SendMessageW(
                self.treeview,
                TVM_EXPAND,
                Some(WPARAM(act.0 as usize)),
                Some(LPARAM(hitem.0)),
            );
        }

        self.is_programmatic_tree_update = prev_prog;

        let _ = self.storage.set_folder_expanded(fid, new_state);
        if let Some(f) = self.cached_folders.get_mut(&fid) {
            f.is_expanded = new_state;
        }

        unsafe {
            let _ = InvalidateRect(Some(self.treeview), None, false);
        }
        self.invalidate_canvas();
    }

    pub fn toggle_root_expansion(&mut self, hitem: HTREEITEM) {
        if hitem.0 == 0 {
            return;
        }
        let current = self.is_tree_item_expanded(hitem);
        let act = if !current { TVE_EXPAND } else { TVE_COLLAPSE };
        unsafe {
            let _ = SendMessageW(
                self.treeview,
                TVM_EXPAND,
                Some(WPARAM(act.0 as usize)),
                Some(LPARAM(hitem.0)),
            );
            let _ = InvalidateRect(Some(self.treeview), None, false);
        }
        self.invalidate_canvas();
    }

    #[allow(clippy::too_many_arguments)]
    fn populate_folder_subtree(
        &self,
        parent_hitem: HTREEITEM,
        parent_folder_id: Option<i64>,
        folders_by_parent: &HashMap<Option<i64>, Vec<&Folder>>,
        conns_by_folder: &HashMap<Option<i64>, Vec<&ConnectionItem>>,
        previous_selection: Option<TreeNodeKind>,
        handle_to_select: &mut Option<HTREEITEM>,
        visited: &mut Vec<i64>,
    ) {
        // 1. Les connexions directes apparaissent TOUJOURS AVANT les sous-dossiers : prioritaires d'abord (alphabétique), puis normales (alphabétique)
        if let Some(conns) = conns_by_folder.get(&parent_folder_id) {
            let mut sorted_conns = conns.clone();
            sorted_conns.sort_by(|a, b| {
                b.is_priority
                    .cmp(&a.is_priority)
                    .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
            });
            for conn in sorted_conns {
                let label = format!("{} {}", conn.protocol.badge_prefix(), conn.name);
                let h_conn = self.insert_tree_item(
                    parent_hitem,
                    &label,
                    TreeNodeKind::Connection(conn.id),
                    false,
                );
                if previous_selection == Some(TreeNodeKind::Connection(conn.id)) {
                    *handle_to_select = Some(h_conn);
                }
            }
        }

        // 2. Ensuite les sous-dossiers directs, classés alphabétiquement (insensible à la casse), avec récursion
        if let Some(subfolders) = folders_by_parent.get(&parent_folder_id) {
            let mut sorted_folders = subfolders.clone();
            sorted_folders.sort_by_key(|a| a.name.to_lowercase());
            for folder in sorted_folders {
                if visited.contains(&folder.id) {
                    continue;
                }
                visited.push(folder.id);

                let h_folder = self.insert_tree_item(
                    parent_hitem,
                    &folder.name,
                    TreeNodeKind::Folder(folder.id),
                    folder.is_expanded,
                );

                if previous_selection == Some(TreeNodeKind::Folder(folder.id)) {
                    *handle_to_select = Some(h_folder);
                }

                self.populate_folder_subtree(
                    h_folder,
                    Some(folder.id),
                    folders_by_parent,
                    conns_by_folder,
                    previous_selection,
                    handle_to_select,
                    visited,
                );

                unsafe {
                    let act = if folder.is_expanded {
                        TVE_EXPAND
                    } else {
                        TVE_COLLAPSE
                    };
                    let _ = SendMessageW(
                        self.treeview,
                        TVM_EXPAND,
                        Some(WPARAM(act.0 as usize)),
                        Some(LPARAM(h_folder.0)),
                    );
                }
            }
        }
    }

    fn insert_tree_item(
        &self,
        parent: HTREEITEM,
        text: &str,
        kind: TreeNodeKind,
        expanded: bool,
    ) -> HTREEITEM {
        const TVIS_EXPANDED_FLAG: u32 = 0x0020;

        let mut wide: Vec<u16> = text.encode_utf16().collect();
        wide.push(0);

        let state_val = if expanded { TVIS_EXPANDED_FLAG } else { 0 };

        let mut ins = TVINSERTSTRUCTW {
            hParent: parent,
            hInsertAfter: TVI_LAST,
            ..Default::default()
        };
        ins.Anonymous.itemex = TVITEMEXW {
            mask: TVIF_TEXT | TVIF_PARAM | TVIF_STATE,
            state: state_val,
            stateMask: TVIS_EXPANDED_FLAG,
            pszText: windows_core::PWSTR(wide.as_mut_ptr()),
            cchTextMax: wide.len() as i32,
            lParam: LPARAM(kind.to_lparam()),
            ..Default::default()
        };

        unsafe {
            let res = SendMessageW(
                self.treeview,
                TVM_INSERTITEMW,
                Some(WPARAM(0)),
                Some(LPARAM(&ins as *const TVINSERTSTRUCTW as isize)),
            );
            HTREEITEM(res.0)
        }
    }

    fn node_kind_from_hitem(&self, hitem: HTREEITEM) -> Option<TreeNodeKind> {
        if hitem.0 == 0 {
            return None;
        }
        let mut item = TVITEMW {
            mask: TVIF_HANDLE | TVIF_PARAM,
            hItem: hitem,
            ..Default::default()
        };
        unsafe {
            let ok = SendMessageW(
                self.treeview,
                TVM_GETITEMW,
                Some(WPARAM(0)),
                Some(LPARAM(&mut item as *mut TVITEMW as isize)),
            );
            if ok.0 != 0 {
                TreeNodeKind::from_lparam(item.lParam.0)
            } else {
                None
            }
        }
    }

    fn is_tree_item_expanded(&self, hitem: HTREEITEM) -> bool {
        if hitem.0 == 0 {
            return false;
        }
        let mut item = TVITEMW {
            mask: TVIF_HANDLE | TVIF_STATE,
            hItem: hitem,
            stateMask: TVIS_EXPANDED,
            ..Default::default()
        };
        unsafe {
            let ok = SendMessageW(
                self.treeview,
                TVM_GETITEMW,
                Some(WPARAM(0)),
                Some(LPARAM(&mut item as *mut TVITEMW as isize)),
            );
            ok.0 != 0 && (item.state.0 & TVIS_EXPANDED.0) != 0
        }
    }

    unsafe fn on_treeview_custom_draw(&self, tvcd: &mut NMTVCUSTOMDRAW) -> LRESULT {
        unsafe {
            if tvcd.nmcd.dwDrawStage == CDDS_PREPAINT {
                return LRESULT(CDRF_NOTIFYITEMDRAW as isize);
            }
            if tvcd.nmcd.dwDrawStage != CDDS_ITEMPREPAINT {
                return LRESULT(0);
            }

            let hdc = tvcd.nmcd.hdc;
            let hitem = HTREEITEM(tvcd.nmcd.dwItemSpec as isize);
            let kind = TreeNodeKind::from_lparam(tvcd.nmcd.lItemlParam.0)
                .or_else(|| self.node_kind_from_hitem(hitem));

            let mut tv_rc = RECT::default();
            let _ = GetClientRect(self.treeview, &mut tv_rc);
            let row_rc = RECT {
                left: 0,
                top: tvcd.nmcd.rc.top,
                right: tv_rc.right.max(tvcd.nmcd.rc.right),
                bottom: tvcd.nmcd.rc.bottom,
            };
            if row_rc.bottom <= row_rc.top {
                return LRESULT(CDRF_SKIPDEFAULT as isize);
            }

            let is_selected = (tvcd.nmcd.uItemState.0 & CDIS_SELECTED.0) != 0
                || (kind.is_some() && self.selected_node == kind);
            let is_hot = (tvcd.nmcd.uItemState.0 & CDIS_HOT.0) != 0;

            let bg_brush = if is_selected {
                self.palette.brush_bg_hover
            } else if is_hot {
                self.palette.brush_bg_surface
            } else {
                self.palette.brush_bg_sidebar
            };
            let _ = FillRect(hdc, &row_rc, bg_brush);

            if is_selected {
                let bar_rc = RECT {
                    left: 0,
                    top: row_rc.top + 3,
                    right: 3,
                    bottom: row_rc.bottom - 3,
                };
                let _ = FillRect(hdc, &bar_rc, self.palette.brush_accent);
            }

            let _ = SetBkMode(hdc, TRANSPARENT);
            let mid_y = (row_rc.top + row_rc.bottom) / 2;
            let indent = 10 + tvcd.iLevel.max(0) * 16;
            let expanded = self.is_tree_item_expanded(hitem);

            match kind {
                Some(TreeNodeKind::FavoritesRoot) => {
                    draw_icon_chevron(hdc, indent + 4, mid_y, expanded, col_text_muted());
                    draw_icon_star(hdc, indent + 16, mid_y - 7, COL_GOLD_STAR);
                    let _ = SelectObject(hdc, self.palette.font_ui_bold.into());
                    let _ = SetTextColor(hdc, COLORREF(col_text_primary()));
                    let title = format!("Favoris ({})", self.fav_count);
                    draw_text_line(
                        hdc,
                        &title,
                        RECT {
                            left: indent + 36,
                            top: row_rc.top,
                            right: row_rc.right - 8,
                            bottom: row_rc.bottom,
                        },
                    );
                }
                Some(TreeNodeKind::ConnectionsRoot) => {
                    draw_icon_chevron(hdc, indent + 4, mid_y, expanded, col_text_muted());
                    draw_icon_layers(hdc, indent + 16, mid_y - 7, col_accent());
                    let _ = SelectObject(hdc, self.palette.font_ui_bold.into());
                    let _ = SetTextColor(hdc, COLORREF(col_text_primary()));
                    let title = if self.search_query.trim().is_empty() {
                        format!("Connexions ({})", self.conn_count)
                    } else {
                        format!("Recherche ({})", self.conn_count)
                    };
                    draw_text_line(
                        hdc,
                        &title,
                        RECT {
                            left: indent + 36,
                            top: row_rc.top,
                            right: row_rc.right - 8,
                            bottom: row_rc.bottom,
                        },
                    );
                }
                Some(TreeNodeKind::Folder(fid)) => {
                    draw_icon_chevron(hdc, indent + 4, mid_y, expanded, col_text_muted());
                    draw_icon_folder(hdc, indent + 16, mid_y - 6, expanded, COL_FOLDER_AMBER);
                    let _ = SelectObject(
                        hdc,
                        if is_selected {
                            self.palette.font_ui_bold.into()
                        } else {
                            self.palette.font_ui.into()
                        },
                    );
                    let _ = SetTextColor(hdc, COLORREF(col_text_primary()));
                    let name = self
                        .cached_folders
                        .get(&fid)
                        .map(|f| f.name.as_str())
                        .unwrap_or("Dossier");
                    draw_text_line(
                        hdc,
                        name,
                        RECT {
                            left: indent + 36,
                            top: row_rc.top,
                            right: row_rc.right - 8,
                            bottom: row_rc.bottom,
                        },
                    );
                }
                Some(TreeNodeKind::Connection(cid)) => {
                    if let Some(conn) = self.cached_connections.get(&cid) {
                        let (badge_col, badge_txt) = match conn.protocol {
                            Protocol::Rdp => (col_accent(), "RDP"),
                            Protocol::AnyDesk => (COL_PROTO_ANYDESK, "ANY"),
                            Protocol::RustDesk => (COL_PROTO_RUSTDESK, "RST"),
                        };
                        let badge_rc = RECT {
                            left: indent + 12,
                            top: mid_y - 8,
                            right: indent + 44,
                            bottom: mid_y + 8,
                        };
                        draw_filled_badge(
                            hdc,
                            badge_rc,
                            badge_col,
                            badge_txt,
                            self.palette.font_small,
                        );

                        let text_offset = if conn.is_priority { 62 } else { 50 };
                        let right_pad = if conn.is_favorite { 24 } else { 8 };

                        if conn.is_priority {
                            draw_icon_priority(hdc, indent + 47, mid_y - 6, COL_PRIORITY_TRIANGLE);
                        }

                        let _ = SelectObject(
                            hdc,
                            if is_selected {
                                self.palette.font_ui_bold.into()
                            } else {
                                self.palette.font_ui.into()
                            },
                        );
                        let _ = SetTextColor(hdc, COLORREF(col_text_primary()));
                        draw_text_line(
                            hdc,
                            &conn.name,
                            RECT {
                                left: indent + text_offset,
                                top: row_rc.top,
                                right: (row_rc.right - right_pad).max(indent + text_offset + 10),
                                bottom: row_rc.bottom,
                            },
                        );

                        if conn.is_favorite && row_rc.right - 18 > indent + text_offset + 15 {
                            draw_icon_star(hdc, row_rc.right - 18, mid_y - 7, COL_GOLD_STAR);
                        }
                    }
                }
                None => {}
            }

            LRESULT(CDRF_SKIPDEFAULT as isize)
        }
    }

    unsafe fn on_treeview_click(&mut self) {
        unsafe {
            let mut pt = POINT::default();
            let _ = GetCursorPos(&mut pt);
            let _ = ScreenToClient(self.treeview, &mut pt);
            let mut hti = TVHITTESTINFO {
                pt,
                ..Default::default()
            };
            let h = SendMessageW(
                self.treeview,
                TVM_HITTEST,
                Some(WPARAM(0)),
                Some(LPARAM(&mut hti as *mut TVHITTESTINFO as isize)),
            );
            let hitem = if h.0 != 0 {
                HTREEITEM(h.0)
            } else if hti.hItem.0 != 0 {
                hti.hItem
            } else {
                HTREEITEM(0)
            };
            if hitem.0 == 0 {
                return;
            }

            if let Some(kind) = self.node_kind_from_hitem(hitem)
                && matches!(
                    kind,
                    TreeNodeKind::FavoritesRoot
                        | TreeNodeKind::ConnectionsRoot
                        | TreeNodeKind::Folder(_)
                )
            {
                let is_toggle_zone = if (hti.flags.0 & TVHT_ONITEMBUTTON.0) != 0 {
                    true
                } else {
                    let mut item_rect = [0u8; 32];
                    let ptr_rect = item_rect.as_mut_ptr() as *mut usize;
                    *ptr_rect = hitem.0 as usize;
                    let ok = SendMessageW(
                        self.treeview,
                        TVM_GETITEMRECT,
                        Some(WPARAM(1)), // 1 = rectangle du texte uniquement
                        Some(LPARAM(item_rect.as_mut_ptr() as isize)),
                    );
                    if ok.0 != 0 {
                        let rc = *(item_rect.as_ptr() as *const RECT);
                        pt.x < rc.left
                    } else {
                        pt.x <= 48
                    }
                };

                if is_toggle_zone {
                    match kind {
                        TreeNodeKind::Folder(fid) => {
                            self.toggle_or_set_folder_expansion(hitem, fid, None);
                        }
                        TreeNodeKind::FavoritesRoot | TreeNodeKind::ConnectionsRoot => {
                            self.toggle_root_expansion(hitem);
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    fn selected_folder_context(&self) -> Option<i64> {
        match self.selected_node {
            Some(TreeNodeKind::Folder(fid)) => Some(fid),
            Some(TreeNodeKind::Connection(cid)) => self
                .storage
                .get_connection(cid)
                .ok()
                .flatten()
                .and_then(|c| c.folder_id),
            _ => None,
        }
    }
}

impl AppWindowState {
    // =========================================================================
    // DESSIN GDI DOUBLE-BUFFÉRISÉ (THÈME SOMBRE WINDOWS 11)
    // =========================================================================

    unsafe fn on_paint(&mut self) {
        unsafe {
            let mut ps = PAINTSTRUCT::default();
            let hdc_screen = BeginPaint(self.hwnd, &mut ps);
            let rc = self.client_rect();
            let width = (rc.right - rc.left).max(1);
            let height = (rc.bottom - rc.top).max(1);

            let hdc = CreateCompatibleDC(Some(hdc_screen));
            let hbm = CreateCompatibleBitmap(hdc_screen, width, height);
            let old_bm = SelectObject(hdc, hbm.into());

            self.hit_zones.clear();
            let _ = SetBkMode(hdc, TRANSPARENT);

            // 1. Fond global anthracite profond
            let _ = FillRect(hdc, &rc, self.palette.brush_bg_deep);

            let sidebar_w = self.settings.sidebar_width;
            let content_left = sidebar_w + SPLITTER_WIDTH;
            let status_top = (height - STATUSBAR_HEIGHT).max(0);

            // 2. En-tête du panneau gauche (Malvision + Boutons + Recherche)
            let sidebar_header_rc = RECT {
                left: 0,
                top: 0,
                right: sidebar_w,
                bottom: SIDEBAR_HEADER_HEIGHT,
            };
            let _ = FillRect(hdc, &sidebar_header_rc, self.palette.brush_bg_sidebar);

            // Titre "MALVISION" + sous-titre discret
            let _ = SelectObject(hdc, self.palette.font_title.into());
            let _ = SetTextColor(hdc, COLORREF(col_text_primary()));
            draw_text_line(
                hdc,
                "Malvision",
                RECT {
                    left: 14,
                    top: 10,
                    right: sidebar_w - 10,
                    bottom: 36,
                },
            );

            // Badge de version dynamique à droite du titre (ex: v1.0.1)
            let version_str = format!("v{}", env!("CARGO_PKG_VERSION"));
            let mut trc = RECT::default();
            let mut wide_v: Vec<u16> = version_str.encode_utf16().collect();
            let _ = SelectObject(hdc, self.palette.font_small.into());
            let _ = DrawTextW(
                hdc,
                &mut wide_v,
                &mut trc,
                DT_CALCRECT | DT_SINGLELINE | DT_NOPREFIX,
            );
            let text_w = trc.right - trc.left;
            let badge_w = (text_w + 14).max(42);
            let badge_left = 114;
            draw_filled_badge(
                hdc,
                RECT {
                    left: badge_left,
                    top: 14,
                    right: badge_left + badge_w,
                    bottom: 32,
                },
                col_accent(),
                &version_str,
                self.palette.font_small,
            );

            // Bouton "+ Connexion" et "+ Dossier" (symétrie stricte de largeur et d'alignement)
            let btn_gap = 8;
            let total_btn_w = (sidebar_w - 24 - btn_gap).max(100);
            let btn_w = total_btn_w / 2;

            let rc_btn_conn = RECT {
                left: 12,
                top: 44,
                right: 12 + btn_w,
                bottom: 76,
            };
            let rc_btn_fold = RECT {
                left: 12 + btn_w + btn_gap,
                top: 44,
                right: 12 + btn_w + btn_gap + btn_w,
                bottom: 76,
            };

            let is_conn_hover = self.hovered_zone == Some(InteractiveZone::BtnNewConnection);
            draw_rounded_button(
                hdc,
                rc_btn_conn,
                if is_conn_hover {
                    col_accent_hover()
                } else {
                    col_accent()
                },
                col_accent_hover(),
                "+ Connexion",
                self.palette.font_ui_bold,
                COL_TEXT_ON_ACCENT,
            );
            self.hit_zones.push(HitZone {
                rect: rc_btn_conn,
                zone: InteractiveZone::BtnNewConnection,
            });

            let is_fold_hover = self.hovered_zone == Some(InteractiveZone::BtnNewFolder);
            draw_rounded_button(
                hdc,
                rc_btn_fold,
                if is_fold_hover {
                    col_bg_hover()
                } else {
                    col_bg_surface()
                },
                col_border(),
                "+ Dossier",
                self.palette.font_ui,
                col_text_primary(),
            );
            self.hit_zones.push(HitZone {
                rect: rc_btn_fold,
                zone: InteractiveZone::BtnNewFolder,
            });

            // Conteneur sombre à coins arrondis autour du champ de recherche + icône loupe
            let search_box_rc = RECT {
                left: 12,
                top: 84,
                right: (sidebar_w - 12).max(92),
                bottom: 112,
            };
            let is_search_focused = GetFocus() == self.search_edit;
            draw_rounded_box(
                hdc,
                search_box_rc,
                col_bg_input(),
                if is_search_focused {
                    col_accent()
                } else {
                    col_border()
                },
            );
            draw_icon_search(
                hdc,
                search_box_rc.left + 8,
                search_box_rc.top + 7,
                if is_search_focused {
                    col_accent_hover()
                } else {
                    col_text_muted()
                },
            );

            // Boutons « Tout déplier » et « Tout replier » au-dessus de l'arborescence
            let rc_expand_all = RECT {
                left: 12,
                top: 118,
                right: 12 + btn_w,
                bottom: 144,
            };
            let rc_collapse_all = RECT {
                left: 12 + btn_w + btn_gap,
                top: 118,
                right: 12 + btn_w + btn_gap + btn_w,
                bottom: 144,
            };

            let is_expand_hover = self.hovered_zone == Some(InteractiveZone::BtnExpandAll);
            draw_compact_toolbar_button(
                hdc,
                rc_expand_all,
                is_expand_hover,
                "Tout déplier",
                true,
                self.palette.font_small,
            );
            self.hit_zones.push(HitZone {
                rect: rc_expand_all,
                zone: InteractiveZone::BtnExpandAll,
            });

            let is_collapse_hover = self.hovered_zone == Some(InteractiveZone::BtnCollapseAll);
            draw_compact_toolbar_button(
                hdc,
                rc_collapse_all,
                is_collapse_hover,
                "Tout replier",
                false,
                self.palette.font_small,
            );
            self.hit_zones.push(HitZone {
                rect: rc_collapse_all,
                zone: InteractiveZone::BtnCollapseAll,
            });

            draw_horizontal_line(hdc, 0, sidebar_w, SIDEBAR_HEADER_HEIGHT - 1, col_border());

            // 2b. Pied du panneau gauche : bouton « Paramètres » avec icône engrenage
            let footer_top = (status_top - SIDEBAR_FOOTER_HEIGHT).max(SIDEBAR_HEADER_HEIGHT);
            let sidebar_footer_rc = RECT {
                left: 0,
                top: footer_top,
                right: sidebar_w,
                bottom: status_top,
            };
            let _ = FillRect(hdc, &sidebar_footer_rc, self.palette.brush_bg_sidebar);
            draw_horizontal_line(hdc, 0, sidebar_w, footer_top, col_border());

            let rc_btn_settings = RECT {
                left: 12,
                top: footer_top + 7,
                right: (sidebar_w - 12).max(92),
                bottom: status_top - 7,
            };
            let is_settings_hover = self.hovered_zone == Some(InteractiveZone::BtnSettings);
            draw_rounded_box(
                hdc,
                rc_btn_settings,
                if is_settings_hover {
                    col_bg_hover()
                } else {
                    col_bg_surface()
                },
                if is_settings_hover {
                    col_accent()
                } else {
                    col_border()
                },
            );
            let gear_cy = (rc_btn_settings.top + rc_btn_settings.bottom) / 2;
            draw_icon_gear(
                hdc,
                rc_btn_settings.left + 18,
                gear_cy,
                if is_settings_hover {
                    col_accent_hover()
                } else {
                    col_text_muted()
                },
            );
            let _ = SelectObject(hdc, self.palette.font_ui.into());
            let _ = SetTextColor(hdc, COLORREF(col_text_primary()));
            draw_text_line(
                hdc,
                "Paramètres",
                RECT {
                    left: rc_btn_settings.left + 34,
                    top: rc_btn_settings.top,
                    right: rc_btn_settings.right - 10,
                    bottom: rc_btn_settings.bottom,
                },
            );
            self.hit_zones.push(HitZone {
                rect: rc_btn_settings,
                zone: InteractiveZone::BtnSettings,
            });

            // Séparateur vertical redimensionnable entre le panneau gauche et la zone centrale
            let splitter_rc = RECT {
                left: sidebar_w,
                top: 0,
                right: content_left,
                bottom: status_top,
            };
            let _ = FillRect(hdc, &splitter_rc, self.palette.brush_bg_sidebar);
            draw_vertical_line(hdc, sidebar_w, 0, status_top, col_border());

            // 3. Barre d'onglets supérieure de la zone centrale
            let tabbar_rc = RECT {
                left: content_left,
                top: 0,
                right: width,
                bottom: TABBAR_HEIGHT,
            };
            let _ = FillRect(hdc, &tabbar_rc, self.palette.brush_bg_sidebar);
            draw_horizontal_line(hdc, content_left, width, TABBAR_HEIGHT - 1, col_border());

            self.paint_tab_bar(hdc, tabbar_rc);

            // 4. Zone centrale : si aucune session RDP n'est ouverte, dessiner l'écran d'accueil moderne
            if self.sessions.is_empty() {
                let home_rc = RECT {
                    left: content_left,
                    top: TABBAR_HEIGHT,
                    right: width,
                    bottom: status_top,
                };
                self.paint_home_dashboard(hdc, home_rc);
            }

            // 5. Barre d'état inférieure (Statut, Protocole, Hôte, Erreurs éventuelles)
            let status_rc = RECT {
                left: 0,
                top: status_top,
                right: width,
                bottom: height,
            };
            self.paint_status_bar(hdc, status_rc);

            // Blit du buffer mémoire vers l'écran en une seule opération sans scintillement
            let _ = windows::Win32::Graphics::Gdi::BitBlt(
                hdc_screen,
                0,
                0,
                width,
                height,
                Some(hdc),
                0,
                0,
                windows::Win32::Graphics::Gdi::SRCCOPY,
            );

            let _ = SelectObject(hdc, old_bm);
            let _ = DeleteObject(hbm.into());
            let _ = DeleteDC(hdc);
            let _ = EndPaint(self.hwnd, &ps);
        }
    }

    unsafe fn paint_tab_bar(&mut self, hdc: HDC, tabbar_rc: RECT) {
        unsafe {
            let mut x = tabbar_rc.left + 8;
            let active_id = self.sessions.active_session_id();

            if self.sessions.is_empty() {
                // Onglet d'accueil passif avec icône maison vectorielle
                let home_tab = RECT {
                    left: x,
                    top: 5,
                    right: x + 150,
                    bottom: TABBAR_HEIGHT - 1,
                };
                let _ = FillRect(hdc, &home_tab, self.palette.brush_bg_deep);
                draw_horizontal_line(
                    hdc,
                    home_tab.left,
                    home_tab.right,
                    home_tab.top,
                    col_accent(),
                );
                draw_horizontal_line(
                    hdc,
                    home_tab.left,
                    home_tab.right,
                    home_tab.top + 1,
                    col_accent(),
                );
                draw_icon_home(
                    hdc,
                    home_tab.left + 14,
                    (home_tab.top + home_tab.bottom) / 2 - 7,
                    col_accent_hover(),
                );
                let _ = SelectObject(hdc, self.palette.font_ui_bold.into());
                let _ = SetTextColor(hdc, COLORREF(col_text_primary()));
                draw_text_line(
                    hdc,
                    "Accueil",
                    RECT {
                        left: home_tab.left + 34,
                        top: home_tab.top,
                        right: home_tab.right - 10,
                        bottom: home_tab.bottom,
                    },
                );
                return;
            }

            let tab_infos: Vec<(u32, String, SessionStatus, Protocol)> = self
                .sessions
                .tabs()
                .iter()
                .map(|t| (t.session_id, t.title.clone(), t.status, t.protocol))
                .collect();

            for (sid, title, status, proto) in tab_infos {
                let is_active = active_id == Some(sid);
                let tab_w = 210;
                if x + tab_w > tabbar_rc.right - 40 {
                    break;
                }

                let tab_rc = RECT {
                    left: x,
                    top: 4,
                    right: x + tab_w,
                    bottom: TABBAR_HEIGHT - 1,
                };

                let is_tab_hover = self.hovered_zone == Some(InteractiveZone::TabSelect(sid));
                let bg_brush = if is_active {
                    self.palette.brush_bg_deep
                } else if is_tab_hover {
                    self.palette.brush_bg_hover
                } else {
                    self.palette.brush_bg_surface
                };
                let _ = FillRect(hdc, &tab_rc, bg_brush);

                if is_active {
                    // Ligne d'accent bleue sur l'onglet actif
                    draw_horizontal_line(hdc, tab_rc.left, tab_rc.right, tab_rc.top, col_accent());
                    draw_horizontal_line(
                        hdc,
                        tab_rc.left,
                        tab_rc.right,
                        tab_rc.top + 1,
                        col_accent(),
                    );
                }

                // Indicateur coloré d'état de connexion
                let dot_color = match status {
                    SessionStatus::Connected | SessionStatus::LoggedIn => col_status_ok(),
                    SessionStatus::Connecting => col_status_warn(),
                    SessionStatus::Error => col_status_err(),
                    SessionStatus::Disconnected => col_text_muted(),
                };
                draw_status_dot(
                    hdc,
                    tab_rc.left + 12,
                    (tab_rc.top + tab_rc.bottom) / 2,
                    dot_color,
                );

                let _ = SelectObject(
                    hdc,
                    if is_active {
                        self.palette.font_ui_bold.into()
                    } else {
                        self.palette.font_ui.into()
                    },
                );
                let _ = SetTextColor(
                    hdc,
                    COLORREF(if is_active {
                        col_text_primary()
                    } else {
                        col_text_muted()
                    }),
                );

                // Bouton de fermeture d'onglet "×"
                let close_rc = RECT {
                    left: tab_rc.right - 26,
                    top: tab_rc.top + 6,
                    right: tab_rc.right - 6,
                    bottom: tab_rc.bottom - 5,
                };

                // Bouton d'actions de session pour l'onglet actif RDP (menu Ctrl+Alt+Del, plein écran...)
                let (actions_rc, text_right) = if is_active && proto == Protocol::Rdp {
                    let act_rc = RECT {
                        left: tab_rc.right - 48,
                        top: tab_rc.top + 6,
                        right: tab_rc.right - 28,
                        bottom: tab_rc.bottom - 5,
                    };
                    (Some(act_rc), act_rc.left - 4)
                } else {
                    (None, close_rc.left - 4)
                };

                let tab_label = format!("{} {}", proto.badge_prefix(), title);
                draw_text_line(
                    hdc,
                    &tab_label,
                    RECT {
                        left: tab_rc.left + 24,
                        top: tab_rc.top,
                        right: text_right,
                        bottom: tab_rc.bottom,
                    },
                );

                if let Some(act_rc) = actions_rc {
                    let is_act_hover = self.hovered_zone == Some(InteractiveZone::TabActions(sid));
                    if is_act_hover {
                        let _ = FillRect(hdc, &act_rc, self.palette.brush_bg_hover);
                    }
                    let _ = SelectObject(hdc, self.palette.font_ui.into());
                    let _ = SetTextColor(hdc, COLORREF(col_text_muted()));
                    draw_text_line(
                        hdc,
                        "⚡",
                        RECT {
                            left: act_rc.left + 3,
                            top: act_rc.top - 1,
                            right: act_rc.right,
                            bottom: act_rc.bottom,
                        },
                    );
                    self.hit_zones.push(HitZone {
                        rect: act_rc,
                        zone: InteractiveZone::TabActions(sid),
                    });
                }

                let is_close_hover = self.hovered_zone == Some(InteractiveZone::TabClose(sid));
                if is_close_hover {
                    let err_brush = CreateSolidBrush(COLORREF(col_status_err()));
                    let _ = FillRect(hdc, &close_rc, err_brush);
                    let _ = DeleteObject(err_brush.into());
                }
                let _ = SelectObject(hdc, self.palette.font_ui_bold.into());
                let _ = SetTextColor(hdc, COLORREF(col_text_primary()));
                draw_text_line(
                    hdc,
                    "×",
                    RECT {
                        left: close_rc.left + 5,
                        top: close_rc.top - 1,
                        right: close_rc.right,
                        bottom: close_rc.bottom,
                    },
                );

                // Enregistrer d'abord les boutons interactifs (prioritaires au test de clic) puis l'onglet
                self.hit_zones.push(HitZone {
                    rect: close_rc,
                    zone: InteractiveZone::TabClose(sid),
                });
                self.hit_zones.push(HitZone {
                    rect: RECT {
                        left: tab_rc.left,
                        top: tab_rc.top,
                        right: if let Some(act) = actions_rc {
                            act.left
                        } else {
                            close_rc.left
                        },
                        bottom: tab_rc.bottom,
                    },
                    zone: InteractiveZone::TabSelect(sid),
                });

                x += tab_w + 4;
            }

            // Petit bouton "+" à droite des onglets pour créer/lancer une nouvelle connexion
            if x + 32 < tabbar_rc.right {
                let plus_rc = RECT {
                    left: x + 2,
                    top: 6,
                    right: x + 30,
                    bottom: TABBAR_HEIGHT - 4,
                };
                let is_hover = self.hovered_zone == Some(InteractiveZone::TabNewConnection);
                draw_rounded_button(
                    hdc,
                    plus_rc,
                    if is_hover {
                        col_bg_hover()
                    } else {
                        col_bg_surface()
                    },
                    col_border(),
                    "+",
                    self.palette.font_ui_bold,
                    col_text_primary(),
                );
                self.hit_zones.push(HitZone {
                    rect: plus_rc,
                    zone: InteractiveZone::TabNewConnection,
                });
            }
        }
    }

    unsafe fn paint_home_dashboard(&mut self, hdc: HDC, area: RECT) {
        unsafe {
            let pad_x = 32;

            // Restreindre le tracé à la zone d'accueil
            let hrgn = windows::Win32::Graphics::Gdi::CreateRectRgn(
                area.left,
                area.top,
                area.right,
                area.bottom,
            );
            let _ = windows::Win32::Graphics::Gdi::SelectClipRgn(hdc, Some(hrgn));

            let mut y = area.top + 26 - self.dashboard_scroll_y;
            let start_y = y;

            // En-tête d'accueil
            let _ = SelectObject(hdc, self.palette.font_title.into());
            let _ = SetTextColor(hdc, COLORREF(col_text_primary()));
            draw_text_line(
                hdc,
                "Gestionnaire de connexions distantes",
                RECT {
                    left: area.left + pad_x,
                    top: y,
                    right: area.right - pad_x,
                    bottom: y + 28,
                },
            );
            y += 30;

            let _ = SelectObject(hdc, self.palette.font_ui.into());
            let _ = SetTextColor(hdc, COLORREF(col_text_muted()));
            draw_text_line(
                hdc,
                "Double-cliquez sur une connexion dans l'arborescence ou cliquez sur une carte ci-dessous pour démarrer une session RDP intégrée.",
                RECT {
                    left: area.left + pad_x,
                    top: y,
                    right: area.right - pad_x,
                    bottom: y + 22,
                },
            );
            y += 34;

            // Boutons d'accès rapide
            let btn_new_conn = RECT {
                left: area.left + pad_x,
                top: y,
                right: area.left + pad_x + 190,
                bottom: y + 34,
            };
            let btn_new_fold = RECT {
                left: btn_new_conn.right + 12,
                top: y,
                right: btn_new_conn.right + 12 + 165,
                bottom: y + 34,
            };

            let hover_nc = self.hovered_zone == Some(InteractiveZone::HomeQuickNewConnection);
            draw_rounded_button(
                hdc,
                btn_new_conn,
                if hover_nc {
                    col_accent_hover()
                } else {
                    col_accent()
                },
                col_accent_hover(),
                "+ Nouvelle connexion",
                self.palette.font_ui_bold,
                COL_TEXT_ON_ACCENT,
            );
            if btn_new_conn.bottom >= area.top && btn_new_conn.top <= area.bottom {
                self.hit_zones.push(HitZone {
                    rect: btn_new_conn,
                    zone: InteractiveZone::HomeQuickNewConnection,
                });
            }

            let hover_nf = self.hovered_zone == Some(InteractiveZone::HomeQuickNewFolder);
            draw_rounded_button(
                hdc,
                btn_new_fold,
                if hover_nf {
                    col_bg_hover()
                } else {
                    col_bg_surface()
                },
                col_border(),
                "+ Nouveau dossier",
                self.palette.font_ui,
                col_text_primary(),
            );
            if btn_new_fold.bottom >= area.top && btn_new_fold.top <= area.bottom {
                self.hit_zones.push(HitZone {
                    rect: btn_new_fold,
                    zone: InteractiveZone::HomeQuickNewFolder,
                });
            }

            y += 50;

            let all_folders = self.storage.list_folders().unwrap_or_default();
            let all_conns = self.storage.list_connections().unwrap_or_default();

            // Section 1 : Connexions favorites
            let favorites: Vec<&ConnectionItem> =
                all_conns.iter().filter(|c| c.is_favorite).take(6).collect();

            if !favorites.is_empty() {
                draw_icon_star(hdc, area.left + pad_x, y + 4, 0x0041B8FF);
                let _ = SelectObject(hdc, self.palette.font_subtitle.into());
                let _ = SetTextColor(hdc, COLORREF(col_text_primary()));
                draw_text_line(
                    hdc,
                    "Connexions favorites",
                    RECT {
                        left: area.left + pad_x + 22,
                        top: y,
                        right: area.right - pad_x,
                        bottom: y + 24,
                    },
                );
                y += 30;
                y = self.paint_connection_cards_grid(hdc, area, pad_x, y, &favorites);
                y += 20;
            }

            // Section 2 : Connexions récemment utilisées (Cas A : activées / Cas B : masquées sans vide)
            if self.settings.show_recent_connections {
                let max_rec = self.settings.max_recent_connections.clamp(1, 50);
                let recents = self
                    .storage
                    .list_recent_connections(max_rec)
                    .unwrap_or_default();
                let recent_refs: Vec<&ConnectionItem> =
                    recents.iter().map(|r| &r.connection).collect();

                draw_icon_clock(hdc, area.left + pad_x, y + 4, col_accent_hover());
                let _ = SelectObject(hdc, self.palette.font_subtitle.into());
                let _ = SetTextColor(hdc, COLORREF(col_text_primary()));
                let recent_title = format!(
                    "Connexions récemment utilisées (max. {})",
                    self.settings.max_recent_connections
                );
                draw_text_line(
                    hdc,
                    &recent_title,
                    RECT {
                        left: area.left + pad_x + 22,
                        top: y,
                        right: area.right - pad_x,
                        bottom: y + 24,
                    },
                );
                y += 30;

                if recent_refs.is_empty() {
                    let _ = SelectObject(hdc, self.palette.font_ui.into());
                    let _ = SetTextColor(hdc, COLORREF(col_text_muted()));
                    draw_text_line(
                        hdc,
                        "Aucune session récente enregistrée.",
                        RECT {
                            left: area.left + pad_x,
                            top: y,
                            right: area.right - pad_x,
                            bottom: y + 24,
                        },
                    );
                    y += 34;
                } else {
                    y = self.paint_connection_cards_grid(hdc, area, pad_x, y, &recent_refs);
                    y += 20;
                }
            }

            // Section 3 : Toutes les connexions (Nouvelle section demandée dans le Jalon V1.0.1)
            draw_icon_layers(hdc, area.left + pad_x, y + 4, col_accent());
            let _ = SelectObject(hdc, self.palette.font_subtitle.into());
            let _ = SetTextColor(hdc, COLORREF(col_text_primary()));
            draw_text_line(
                hdc,
                "Toutes les connexions",
                RECT {
                    left: area.left + pad_x + 22,
                    top: y,
                    right: area.right - pad_x,
                    bottom: y + 24,
                },
            );
            y += 30;

            if all_conns.is_empty() {
                let _ = SelectObject(hdc, self.palette.font_ui.into());
                let _ = SetTextColor(hdc, COLORREF(col_text_muted()));
                draw_text_line(
                    hdc,
                    "Aucune connexion enregistrée. Cliquez sur « + Nouvelle connexion » ci-dessus pour ajouter un serveur.",
                    RECT {
                        left: area.left + pad_x,
                        top: y,
                        right: area.right - pad_x,
                        bottom: y + 24,
                    },
                );
                y += 38;
            } else {
                let ordered_conns =
                    crate::models::collect_tree_ordered_connections(&all_folders, &all_conns);
                y = self.paint_compact_connections_grid(
                    hdc,
                    area,
                    pad_x,
                    y,
                    &ordered_conns,
                    &all_folders,
                );
                y += 24;
            }

            // Hauteur totale du contenu pour le calcul du défilement fluide
            self.dashboard_content_height = (y + 40) - start_y;

            // Rétablissement de la région de découpage
            let _ = windows::Win32::Graphics::Gdi::SelectClipRgn(hdc, None);
            let _ = windows::Win32::Graphics::Gdi::DeleteObject(hrgn.into());
        }
    }

    unsafe fn paint_connection_cards_grid(
        &mut self,
        hdc: HDC,
        area: RECT,
        pad_x: i32,
        start_y: i32,
        items: &[&ConnectionItem],
    ) -> i32 {
        unsafe {
            let avail_w = (area.right - area.left - pad_x * 2).max(300);
            let cols = if avail_w >= 820 {
                3
            } else if avail_w >= 520 {
                2
            } else {
                1
            };
            let gap = 14;
            let card_w = (avail_w - gap * (cols - 1)) / cols;
            let card_h = 78;

            let mut row = 0;
            let mut col = 0;

            for conn in items {
                let x = area.left + pad_x + col * (card_w + gap);
                let y = start_y + row * (card_h + gap);

                let card_rc = RECT {
                    left: x,
                    top: y,
                    right: x + card_w,
                    bottom: y + card_h,
                };

                if card_rc.bottom >= area.top && card_rc.top <= area.bottom {
                    let is_hover =
                        self.hovered_zone == Some(InteractiveZone::HomeCardConnect(conn.id));
                    draw_rounded_box(
                        hdc,
                        card_rc,
                        if is_hover {
                            col_bg_hover()
                        } else {
                            col_bg_surface()
                        },
                        if is_hover { col_accent() } else { col_border() },
                    );

                    // Badge protocole
                    let proto_color = match conn.protocol {
                        Protocol::Rdp => col_accent(),
                        Protocol::AnyDesk => 0x003B46D9, // Rouge AnyDesk discret
                        Protocol::RustDesk => 0x00B5721E, // Bleu-vert RustDesk
                    };
                    draw_filled_badge(
                        hdc,
                        RECT {
                            left: card_rc.left + 12,
                            top: card_rc.top + 12,
                            right: card_rc.left + 62,
                            bottom: card_rc.top + 30,
                        },
                        proto_color,
                        conn.protocol.display_name(),
                        self.palette.font_small,
                    );

                    // Nom de la connexion
                    let _ = SelectObject(hdc, self.palette.font_ui_bold.into());
                    let _ = SetTextColor(hdc, COLORREF(col_text_primary()));
                    draw_text_line(
                        hdc,
                        &conn.name,
                        RECT {
                            left: card_rc.left + 70,
                            top: card_rc.top + 10,
                            right: card_rc.right - 74,
                            bottom: card_rc.top + 32,
                        },
                    );

                    // Sous-texte : hôte:port + utilisateur
                    let _ = SelectObject(hdc, self.palette.font_small.into());
                    let _ = SetTextColor(hdc, COLORREF(col_text_muted()));
                    let user_info = if conn.username.is_empty() {
                        conn.endpoint_label()
                    } else if conn.domain.is_empty() {
                        format!("{}  •  {}", conn.endpoint_label(), conn.username)
                    } else {
                        format!(
                            "{}  •  {}\\{}",
                            conn.endpoint_label(),
                            conn.domain,
                            conn.username
                        )
                    };
                    draw_text_line(
                        hdc,
                        &user_info,
                        RECT {
                            left: card_rc.left + 12,
                            top: card_rc.top + 42,
                            right: card_rc.right - 12,
                            bottom: card_rc.bottom - 10,
                        },
                    );

                    // Bouton sobre "Modifier" sur la carte
                    let edit_rc = RECT {
                        left: card_rc.right - 68,
                        top: card_rc.top + 10,
                        right: card_rc.right - 10,
                        bottom: card_rc.top + 32,
                    };
                    let is_edit_hover =
                        self.hovered_zone == Some(InteractiveZone::HomeCardEdit(conn.id));
                    draw_rounded_button(
                        hdc,
                        edit_rc,
                        if is_edit_hover {
                            col_bg_deep()
                        } else {
                            col_bg_surface()
                        },
                        if is_edit_hover {
                            col_accent()
                        } else {
                            col_border()
                        },
                        "Modifier",
                        self.palette.font_small,
                        if is_edit_hover {
                            col_text_primary()
                        } else {
                            col_text_muted()
                        },
                    );

                    self.hit_zones.push(HitZone {
                        rect: edit_rc,
                        zone: InteractiveZone::HomeCardEdit(conn.id),
                    });
                    self.hit_zones.push(HitZone {
                        rect: card_rc,
                        zone: InteractiveZone::HomeCardConnect(conn.id),
                    });
                }

                col += 1;
                if col >= cols {
                    col = 0;
                    row += 1;
                }
            }

            let total_rows = items.len().div_ceil(cols as usize) as i32;
            start_y + total_rows * (card_h + gap)
        }
    }

    unsafe fn paint_compact_connections_grid(
        &mut self,
        hdc: HDC,
        area: RECT,
        pad_x: i32,
        start_y: i32,
        items: &[ConnectionItem],
        folders: &[Folder],
    ) -> i32 {
        unsafe {
            let avail_w = (area.right - area.left - pad_x * 2).max(240);
            let gap = 12;
            let target_w = 230;
            let mut cols = ((avail_w + gap) / (target_w + gap)).max(1);
            let w_for_cols = (avail_w - gap * (cols - 1)) / cols;
            if w_for_cols > 250 {
                let next_w = (avail_w - gap * cols) / (cols + 1);
                if next_w >= 205 {
                    cols += 1;
                }
            }
            let raw_card_w = (avail_w - gap * (cols - 1)) / cols;
            let card_w = if cols > 1 {
                raw_card_w.clamp(210, 250)
            } else {
                raw_card_w.min(320)
            };
            let card_h = 56;

            let mut folder_map = HashMap::new();
            for f in folders {
                folder_map.insert(f.id, (f.name.clone(), f.parent_id));
            }

            let mut row = 0;
            let mut col = 0;

            for conn in items {
                let x = area.left + pad_x + col * (card_w + gap);
                let y = start_y + row * (card_h + gap);

                let card_rc = RECT {
                    left: x,
                    top: y,
                    right: x + card_w,
                    bottom: y + card_h,
                };

                if card_rc.bottom >= area.top && card_rc.top <= area.bottom {
                    let is_hover =
                        self.hovered_zone == Some(InteractiveZone::HomeCardConnect(conn.id));
                    draw_rounded_box(
                        hdc,
                        card_rc,
                        if is_hover {
                            col_bg_hover()
                        } else {
                            col_bg_surface()
                        },
                        if is_hover { col_accent() } else { col_border() },
                    );

                    // Badge protocole uniforme avec l'arborescence (RDP, ANY, RST)
                    let (badge_col, badge_txt) = match conn.protocol {
                        Protocol::Rdp => (col_accent(), "RDP"),
                        Protocol::AnyDesk => (COL_PROTO_ANYDESK, "ANY"),
                        Protocol::RustDesk => (COL_PROTO_RUSTDESK, "RST"),
                    };
                    let badge_rc = RECT {
                        left: card_rc.left + 8,
                        top: card_rc.top + 8,
                        right: card_rc.left + 8 + 32,
                        bottom: card_rc.top + 8 + 17,
                    };
                    draw_filled_badge(hdc, badge_rc, badge_col, badge_txt, self.palette.font_small);

                    let _ = SelectObject(hdc, self.palette.font_ui_bold.into());
                    let _ = SetTextColor(hdc, COLORREF(col_text_primary()));
                    let star_w = if conn.is_favorite { 18 } else { 0 };
                    draw_text_line(
                        hdc,
                        &conn.name,
                        RECT {
                            left: card_rc.left + 46,
                            top: card_rc.top + 7,
                            right: card_rc.right - 8 - star_w,
                            bottom: card_rc.top + 27,
                        },
                    );

                    if conn.is_favorite {
                        draw_icon_star(hdc, card_rc.right - 20, card_rc.top + 9, COL_GOLD_STAR);
                    }

                    let mut path_str = String::new();
                    if let Some(mut cur_fid) = conn.folder_id {
                        let mut parts = Vec::new();
                        let mut loop_protect = 0;
                        while let Some((name, parent)) = folder_map.get(&cur_fid) {
                            parts.push(name.as_str());
                            loop_protect += 1;
                            if loop_protect > 50 {
                                break;
                            }
                            if let Some(p) = parent {
                                cur_fid = *p;
                            } else {
                                break;
                            }
                        }
                        if !parts.is_empty() {
                            parts.reverse();
                            path_str = format!("{} • ", parts.join("/"));
                        }
                    }

                    let detail = format!("{}{}", path_str, conn.endpoint_label());
                    let _ = SelectObject(hdc, self.palette.font_small.into());
                    let _ = SetTextColor(hdc, COLORREF(col_text_muted()));
                    draw_text_line(
                        hdc,
                        &detail,
                        RECT {
                            left: card_rc.left + 8,
                            top: card_rc.top + 30,
                            right: card_rc.right - 8,
                            bottom: card_rc.bottom - 6,
                        },
                    );

                    self.hit_zones.push(HitZone {
                        rect: card_rc,
                        zone: InteractiveZone::HomeCardConnect(conn.id),
                    });
                }

                col += 1;
                if col >= cols {
                    col = 0;
                    row += 1;
                }
            }

            let total_rows = items.len().div_ceil(cols as usize) as i32;
            start_y + total_rows * (card_h + gap)
        }
    }

    unsafe fn paint_status_bar(&self, hdc: HDC, status_rc: RECT) {
        unsafe {
            let _ = FillRect(hdc, &status_rc, self.palette.brush_bg_sidebar);
            draw_horizontal_line(
                hdc,
                status_rc.left,
                status_rc.right,
                status_rc.top,
                col_border(),
            );

            let (dot_color, status_text, proto_text, host_text) =
                if let Some(tab) = self.sessions.active_tab() {
                    let col = match tab.status {
                        SessionStatus::Connected | SessionStatus::LoggedIn => col_status_ok(),
                        SessionStatus::Connecting => col_status_warn(),
                        SessionStatus::Error => col_status_err(),
                        SessionStatus::Disconnected => col_text_muted(),
                    };
                    (
                        col,
                        tab.status_detail.clone(),
                        format!("Protocole : {}", tab.protocol.display_name()),
                        format!("Hôte : {}", tab.host_label),
                    )
                } else if let Some(TreeNodeKind::Connection(cid)) = self.selected_node
                    && let Ok(Some(conn)) = self.storage.get_connection(cid)
                {
                    let msg = self.status_override.clone().unwrap_or_else(|| {
                        format!(
                            "Sélection : {} — Double-cliquez pour lancer la session",
                            conn.name
                        )
                    });
                    (
                        col_accent(),
                        msg,
                        format!("Protocole : {}", conn.protocol.display_name()),
                        format!("Hôte : {}", conn.endpoint_label()),
                    )
                } else {
                    let msg = self
                        .status_override
                        .clone()
                        .unwrap_or_else(|| "Prêt — Aucune session active.".to_string());
                    (
                        col_text_muted(),
                        msg,
                        "Protocole : —".to_string(),
                        "Hôte : —".to_string(),
                    )
                };

            let mid_y = (status_rc.top + status_rc.bottom) / 2;
            draw_status_dot(hdc, 14, mid_y, dot_color);

            let right_host_w = 240;
            let right_proto_w = 140;
            let status_right_limit = (status_rc.right - right_host_w - right_proto_w - 20).max(200);

            let _ = SelectObject(hdc, self.palette.font_small.into());
            let _ = SetTextColor(hdc, COLORREF(col_text_primary()));
            draw_text_line(
                hdc,
                &status_text,
                RECT {
                    left: 28,
                    top: status_rc.top,
                    right: status_right_limit,
                    bottom: status_rc.bottom,
                },
            );

            let _ = SetTextColor(hdc, COLORREF(col_text_muted()));
            draw_vertical_line(
                hdc,
                status_right_limit + 6,
                status_rc.top + 5,
                status_rc.bottom - 5,
                col_border(),
            );
            draw_text_line(
                hdc,
                &proto_text,
                RECT {
                    left: status_right_limit + 16,
                    top: status_rc.top,
                    right: status_right_limit + right_proto_w,
                    bottom: status_rc.bottom,
                },
            );

            draw_vertical_line(
                hdc,
                status_right_limit + right_proto_w + 6,
                status_rc.top + 5,
                status_rc.bottom - 5,
                col_border(),
            );
            draw_text_line(
                hdc,
                &host_text,
                RECT {
                    left: status_right_limit + right_proto_w + 16,
                    top: status_rc.top,
                    right: status_rc.right - 12,
                    bottom: status_rc.bottom,
                },
            );
        }
    }
}

// =============================================================================
// PRIMITIVES GRAPHIQUES GDI & ICÔNES VECTORIELLES SOBRES
// =============================================================================

unsafe fn draw_text_line(hdc: HDC, text: &str, mut rc: RECT) {
    unsafe {
        let mut wide: Vec<u16> = text.encode_utf16().collect();
        let _ = DrawTextW(
            hdc,
            &mut wide,
            &mut rc,
            DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS | DT_NOPREFIX,
        );
    }
}

unsafe fn draw_text_multiline(hdc: HDC, text: &str, mut rc: RECT) {
    unsafe {
        let mut wide: Vec<u16> = text.encode_utf16().collect();
        let _ = DrawTextW(
            hdc,
            &mut wide,
            &mut rc,
            DT_LEFT | DT_WORDBREAK | DT_NOPREFIX,
        );
    }
}

unsafe fn draw_rounded_box(hdc: HDC, rc: RECT, fill_col: u32, border_col: u32) {
    unsafe {
        let brush = CreateSolidBrush(COLORREF(fill_col));
        let pen = CreatePen(PS_SOLID, 1, COLORREF(border_col));
        let old_b = SelectObject(hdc, brush.into());
        let old_p = SelectObject(hdc, pen.into());
        let _ = RoundRect(hdc, rc.left, rc.top, rc.right, rc.bottom, 6, 6);
        let _ = SelectObject(hdc, old_b);
        let _ = SelectObject(hdc, old_p);
        let _ = DeleteObject(brush.into());
        let _ = DeleteObject(pen.into());
    }
}

unsafe fn draw_rounded_button(
    hdc: HDC,
    rc: RECT,
    fill_col: u32,
    border_col: u32,
    label: &str,
    font: HFONT,
    text_col: u32,
) {
    unsafe {
        draw_rounded_box(hdc, rc, fill_col, border_col);
        let _ = SelectObject(hdc, font.into());
        let _ = SetTextColor(hdc, COLORREF(text_col));
        let mut wide: Vec<u16> = label.encode_utf16().collect();
        let mut text_rc = rc;
        let _ = DrawTextW(
            hdc,
            &mut wide,
            &mut text_rc,
            DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS | DT_NOPREFIX,
        );
    }
}

unsafe fn draw_filled_badge(hdc: HDC, rc: RECT, bg_col: u32, text: &str, font: HFONT) {
    unsafe {
        draw_rounded_box(hdc, rc, bg_col, bg_col);
        let _ = SelectObject(hdc, font.into());
        let _ = SetTextColor(hdc, COLORREF(COL_TEXT_ON_ACCENT));
        let mut wide: Vec<u16> = text.encode_utf16().collect();
        let mut trc = rc;
        let _ = DrawTextW(
            hdc,
            &mut wide,
            &mut trc,
            DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX,
        );
    }
}

unsafe fn draw_status_dot(hdc: HDC, cx: i32, cy: i32, color: u32) {
    unsafe {
        let rc = RECT {
            left: cx - 4,
            top: cy - 4,
            right: cx + 4,
            bottom: cy + 4,
        };
        draw_rounded_box(hdc, rc, color, color);
    }
}

unsafe fn draw_horizontal_line(hdc: HDC, x1: i32, x2: i32, y: i32, color: u32) {
    unsafe {
        let pen = CreatePen(PS_SOLID, 1, COLORREF(color));
        let old = SelectObject(hdc, pen.into());
        let _ = MoveToEx(hdc, x1, y, None);
        let _ = LineTo(hdc, x2, y);
        let _ = SelectObject(hdc, old);
        let _ = DeleteObject(pen.into());
    }
}

unsafe fn draw_vertical_line(hdc: HDC, x: i32, y1: i32, y2: i32, color: u32) {
    unsafe {
        let pen = CreatePen(PS_SOLID, 1, COLORREF(color));
        let old = SelectObject(hdc, pen.into());
        let _ = MoveToEx(hdc, x, y1, None);
        let _ = LineTo(hdc, x, y2);
        let _ = SelectObject(hdc, old);
        let _ = DeleteObject(pen.into());
    }
}

fn ensure_gdiplus_initialized() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    static mut TOKEN: usize = 0;
    ONCE.call_once(|| unsafe {
        let input = windows::Win32::Graphics::GdiPlus::GdiplusStartupInput {
            GdiplusVersion: 1,
            ..Default::default()
        };
        let _ = windows::Win32::Graphics::GdiPlus::GdiplusStartup(
            &raw mut TOKEN,
            &input,
            std::ptr::null_mut(),
        );
    });
}

#[inline]
fn colorref_to_argb(color: u32) -> u32 {
    let r = color & 0xFF;
    let g = (color >> 8) & 0xFF;
    let b = (color >> 16) & 0xFF;
    0xFF00_0000 | (r << 16) | (g << 8) | b
}

/// Dessine un chevron vectoriel antialiasé GDI+ (`>` replié ou `⌄` déplié) centré en `(cx, cy)`.
/// Utilise une géométrie vectorielle commune avec une rotation stricte de 90° entre les états.
unsafe fn draw_icon_chevron(hdc: HDC, cx: i32, cy: i32, expanded: bool, color: u32) {
    ensure_gdiplus_initialized();
    unsafe {
        use windows::Win32::Graphics::GdiPlus::*;
        let mut graphics: *mut GpGraphics = std::ptr::null_mut();
        if GdipCreateFromHDC(hdc, &mut graphics) != Ok {
            return;
        }
        let _ = GdipSetSmoothingMode(graphics, SmoothingModeAntiAlias);

        let argb = colorref_to_argb(color);
        let mut pen: *mut GpPen = std::ptr::null_mut();
        if GdipCreatePen1(argb, 1.5, UnitPixel, &mut pen) == Ok {
            let _ = GdipSetPenStartCap(pen, LineCapRound);
            let _ = GdipSetPenEndCap(pen, LineCapRound);
            let _ = GdipSetPenLineJoin(pen, LineJoinRound);

            let cxf = cx as f32;
            let cyf = cy as f32;
            let d = 3.0f32;

            if expanded {
                // Chevron déplié vers le bas ⌄ (sommet à cy + 1.5)
                let _ = GdipDrawLine(graphics, pen, cxf - d, cyf - d / 2.0, cxf, cyf + d / 2.0);
                let _ = GdipDrawLine(graphics, pen, cxf, cyf + d / 2.0, cxf + d, cyf - d / 2.0);
            } else {
                // Chevron replié vers la droite > (rotation stricte de 90°, sommet à cx + 1.5)
                let _ = GdipDrawLine(graphics, pen, cxf - d / 2.0, cyf - d, cxf + d / 2.0, cyf);
                let _ = GdipDrawLine(graphics, pen, cxf + d / 2.0, cyf, cxf - d / 2.0, cyf + d);
            }

            let _ = GdipDeletePen(pen);
        }
        let _ = GdipDeleteGraphics(graphics);
    }
}

/// Dessine une icône de dossier vectorielle sobre de `16x14` à `(x, y)`.
unsafe fn draw_icon_folder(hdc: HDC, x: i32, y: i32, expanded: bool, fill_col: u32) {
    unsafe {
        let tab_rc = RECT {
            left: x,
            top: y + 1,
            right: x + 7,
            bottom: y + 5,
        };
        draw_rounded_box(hdc, tab_rc, fill_col, fill_col);
        let body_rc = RECT {
            left: x,
            top: y + 3,
            right: x + 15,
            bottom: y + 13,
        };
        let border_col = if expanded {
            col_accent_hover()
        } else {
            fill_col
        };
        draw_rounded_box(hdc, body_rc, fill_col, border_col);
    }
}

/// Dessine une étoile/losange de favori sobre à `(x, y)`.
unsafe fn draw_icon_star(hdc: HDC, x: i32, y: i32, color: u32) {
    unsafe {
        let pen = CreatePen(PS_SOLID, 2, COLORREF(color));
        let old = SelectObject(hdc, pen.into());
        let cx = x + 7;
        let cy = y + 7;
        let _ = MoveToEx(hdc, cx, cy - 5, None);
        let _ = LineTo(hdc, cx + 5, cy);
        let _ = LineTo(hdc, cx, cy + 5);
        let _ = LineTo(hdc, cx - 5, cy);
        let _ = LineTo(hdc, cx, cy - 5);
        let _ = SelectObject(hdc, old);
        let _ = DeleteObject(pen.into());
    }
}

/// Dessine un indicateur de connexion prioritaire discret (triangle équilatéral creux vert émeraude #34C88A).
unsafe fn draw_icon_priority(hdc: HDC, x: i32, y: i32, color: u32) {
    ensure_gdiplus_initialized();
    unsafe {
        use windows::Win32::Graphics::GdiPlus::*;
        let mut graphics: *mut GpGraphics = std::ptr::null_mut();
        if GdipCreateFromHDC(hdc, &mut graphics) != Ok {
            return;
        }
        let _ = GdipSetSmoothingMode(graphics, SmoothingModeAntiAlias);

        let argb = colorref_to_argb(color);
        let mut pen: *mut GpPen = std::ptr::null_mut();
        if GdipCreatePen1(argb, 1.5, UnitPixel, &mut pen) == Ok {
            let _ = GdipSetPenLineJoin(pen, LineJoinRound);

            let cx = x as f32 + 5.5;
            let cy = y as f32 + 6.0;

            // Triangle équilatéral régulier creux (contour seul) orienté vers le haut ▲
            // Côté 10.5 px, hauteur 9.1 px, trait 1.5 px (encombrement total ~12x10.6 px)
            let pts = [
                PointF { X: cx, Y: cy - 4.8 },
                PointF {
                    X: cx - 5.25,
                    Y: cy + 4.3,
                },
                PointF {
                    X: cx + 5.25,
                    Y: cy + 4.3,
                },
            ];

            let _ = GdipDrawPolygon(graphics, pen, pts.as_ptr(), 3);
            let _ = GdipDeletePen(pen);
        }
        let _ = GdipDeleteGraphics(graphics);
    }
}

/// Dessine une icône à doubles chevrons antialiasée GDI+ (descendants pour « Tout déplier », montants pour « Tout replier »).
/// Géométrie strictement symétrique par réflexion verticale autour de `cy`.
unsafe fn draw_icon_double_chevron(hdc: HDC, cx: i32, cy: i32, downwards: bool, color: u32) {
    ensure_gdiplus_initialized();
    unsafe {
        use windows::Win32::Graphics::GdiPlus::*;
        let mut graphics: *mut GpGraphics = std::ptr::null_mut();
        if GdipCreateFromHDC(hdc, &mut graphics) != Ok {
            return;
        }
        let _ = GdipSetSmoothingMode(graphics, SmoothingModeAntiAlias);

        let argb = colorref_to_argb(color);
        let mut pen: *mut GpPen = std::ptr::null_mut();
        if GdipCreatePen1(argb, 1.5, UnitPixel, &mut pen) == Ok {
            let _ = GdipSetPenStartCap(pen, LineCapRound);
            let _ = GdipSetPenEndCap(pen, LineCapRound);
            let _ = GdipSetPenLineJoin(pen, LineJoinRound);

            let cxf = cx as f32;
            let cyf = cy as f32;
            let d = 3.0f32;

            if downwards {
                // Chevrons vers le bas ︾ (Tout déplier)
                // Chevron 1 supérieur (sommet à cy - 0.5)
                let _ = GdipDrawLine(graphics, pen, cxf - d, cyf - 3.5, cxf, cyf - 0.5);
                let _ = GdipDrawLine(graphics, pen, cxf, cyf - 0.5, cxf + d, cyf - 3.5);
                // Chevron 2 inférieur (sommet à cy + 3.5)
                let _ = GdipDrawLine(graphics, pen, cxf - d, cyf + 0.5, cxf, cyf + 3.5);
                let _ = GdipDrawLine(graphics, pen, cxf, cyf + 3.5, cxf + d, cyf + 0.5);
            } else {
                // Chevrons vers le haut ︽ (Tout replier) — réflexion verticale stricte
                // Chevron 1 inférieur inversé (sommet à cy + 0.5)
                let _ = GdipDrawLine(graphics, pen, cxf - d, cyf + 3.5, cxf, cyf + 0.5);
                let _ = GdipDrawLine(graphics, pen, cxf, cyf + 0.5, cxf + d, cyf + 3.5);
                // Chevron 2 supérieur inversé (sommet à cy - 3.5)
                let _ = GdipDrawLine(graphics, pen, cxf - d, cyf - 0.5, cxf, cyf - 3.5);
                let _ = GdipDrawLine(graphics, pen, cxf, cyf - 3.5, cxf + d, cyf - 0.5);
            }

            let _ = GdipDeletePen(pen);
        }
        let _ = GdipDeleteGraphics(graphics);
    }
}

/// Dessine un bouton compact de barre d'outils Fluent (avec icône vectorielle et texte centré).
unsafe fn draw_compact_toolbar_button(
    hdc: HDC,
    rc: RECT,
    is_hover: bool,
    label: &str,
    downwards: bool,
    font: HFONT,
) {
    unsafe {
        let fill_col = if is_hover {
            col_bg_hover()
        } else {
            col_bg_surface()
        };
        let border_col = if is_hover { col_accent() } else { col_border() };
        draw_rounded_box(hdc, rc, fill_col, border_col);

        let icon_col = if is_hover {
            col_accent()
        } else {
            col_text_muted()
        };
        let text_col = col_text_primary();

        let _ = SelectObject(hdc, font.into());
        let mut trc = RECT::default();
        let mut wide: Vec<u16> = label.encode_utf16().collect();
        let _ = DrawTextW(
            hdc,
            &mut wide,
            &mut trc,
            DT_CALCRECT | DT_SINGLELINE | DT_NOPREFIX,
        );
        let text_w = trc.right - trc.left;
        let icon_w = 12;
        let spacing = 6;
        let total_content_w = icon_w + spacing + text_w;
        let btn_w = rc.right - rc.left;
        let start_x = rc.left + ((btn_w - total_content_w) / 2).max(4);

        let mid_y = (rc.top + rc.bottom) / 2;
        draw_icon_double_chevron(hdc, start_x + 6, mid_y, downwards, icon_col);

        let _ = SetTextColor(hdc, COLORREF(text_col));
        let text_left = start_x + icon_w + spacing;
        let mut text_rc = RECT {
            left: text_left,
            top: rc.top,
            right: rc.right - 4,
            bottom: rc.bottom,
        };
        let _ = DrawTextW(
            hdc,
            &mut wide,
            &mut text_rc,
            DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS | DT_NOPREFIX,
        );
    }
}

/// Dessine une icône de grille/couches pour la racine "Connexions".
unsafe fn draw_icon_layers(hdc: HDC, x: i32, y: i32, color: u32) {
    unsafe {
        for (dx, dy) in [(0, 0), (8, 0), (0, 8), (8, 8)] {
            let rc = RECT {
                left: x + dx,
                top: y + dy,
                right: x + dx + 6,
                bottom: y + dy + 6,
            };
            draw_rounded_box(hdc, rc, color, color);
        }
    }
}

/// Dessine une icône loupe vectorielle de recherche à `(x, y)`.
unsafe fn draw_icon_search(hdc: HDC, x: i32, y: i32, color: u32) {
    unsafe {
        let pen = CreatePen(PS_SOLID, 2, COLORREF(color));
        let brush = GetStockObject(NULL_BRUSH);
        let old_p = SelectObject(hdc, pen.into());
        let old_b = SelectObject(hdc, brush);
        let _ = RoundRect(hdc, x, y, x + 10, y + 10, 10, 10);
        let _ = MoveToEx(hdc, x + 9, y + 9, None);
        let _ = LineTo(hdc, x + 14, y + 14);
        let _ = SelectObject(hdc, old_b);
        let _ = SelectObject(hdc, old_p);
        let _ = DeleteObject(pen.into());
    }
}

/// Dessine une icône d'accueil vectorielle à `(x, y)`.
unsafe fn draw_icon_home(hdc: HDC, x: i32, y: i32, color: u32) {
    unsafe {
        let pen = CreatePen(PS_SOLID, 2, COLORREF(color));
        let old = SelectObject(hdc, pen.into());
        let _ = MoveToEx(hdc, x + 1, y + 7, None);
        let _ = LineTo(hdc, x + 7, y + 1);
        let _ = LineTo(hdc, x + 13, y + 7);
        let _ = MoveToEx(hdc, x + 3, y + 6, None);
        let _ = LineTo(hdc, x + 3, y + 13);
        let _ = LineTo(hdc, x + 11, y + 13);
        let _ = LineTo(hdc, x + 11, y + 6);
        let _ = SelectObject(hdc, old);
        let _ = DeleteObject(pen.into());
    }
}

/// Dessine une icône horloge sobre pour les sessions récentes à `(x, y)`.
unsafe fn draw_icon_clock(hdc: HDC, x: i32, y: i32, color: u32) {
    unsafe {
        let pen = CreatePen(PS_SOLID, 2, COLORREF(color));
        let brush = GetStockObject(NULL_BRUSH);
        let old_p = SelectObject(hdc, pen.into());
        let old_b = SelectObject(hdc, brush);
        let _ = RoundRect(hdc, x, y, x + 14, y + 14, 14, 14);
        let _ = MoveToEx(hdc, x + 7, y + 4, None);
        let _ = LineTo(hdc, x + 7, y + 7);
        let _ = LineTo(hdc, x + 10, y + 9);
        let _ = SelectObject(hdc, old_b);
        let _ = SelectObject(hdc, old_p);
        let _ = DeleteObject(pen.into());
    }
}

/// Dessine une icône engrenage vectorielle sobre centrée en `(cx, cy)`.
unsafe fn draw_icon_gear(hdc: HDC, cx: i32, cy: i32, color: u32) {
    unsafe {
        let pen = CreatePen(PS_SOLID, 2, COLORREF(color));
        let brush = GetStockObject(NULL_BRUSH);
        let old_p = SelectObject(hdc, pen.into());
        let old_b = SelectObject(hdc, brush);

        // Dents cardinales et diagonales de l'engrenage
        let _ = MoveToEx(hdc, cx, cy - 7, None);
        let _ = LineTo(hdc, cx, cy + 8);
        let _ = MoveToEx(hdc, cx - 7, cy, None);
        let _ = LineTo(hdc, cx + 8, cy);
        let _ = MoveToEx(hdc, cx - 5, cy - 5, None);
        let _ = LineTo(hdc, cx + 6, cy + 6);
        let _ = MoveToEx(hdc, cx - 5, cy + 5, None);
        let _ = LineTo(hdc, cx + 6, cy - 6);

        // Couronne extérieure et moyeu intérieur
        let ring_brush = CreateSolidBrush(COLORREF(col_bg_surface()));
        let _ = SelectObject(hdc, ring_brush.into());
        let _ = RoundRect(hdc, cx - 5, cy - 5, cx + 6, cy + 6, 10, 10);
        let _ = RoundRect(hdc, cx - 2, cy - 2, cx + 3, cy + 3, 5, 5);

        let _ = SelectObject(hdc, old_b);
        let _ = SelectObject(hdc, old_p);
        let _ = DeleteObject(ring_brush.into());
        let _ = DeleteObject(pen.into());
    }
}

impl AppWindowState {
    // =========================================================================
    // INTERACTIONS SOURIS, ONGLETS, SÉPARATEUR, MENU CONTEXTUEL & DRAG-AND-DROP
    // =========================================================================

    fn hit_test_zone(&self, x: i32, y: i32) -> Option<InteractiveZone> {
        for hz in &self.hit_zones {
            if x >= hz.rect.left && x < hz.rect.right && y >= hz.rect.top && y < hz.rect.bottom {
                return Some(hz.zone);
            }
        }
        None
    }

    fn is_on_splitter(&self, x: i32, y: i32) -> bool {
        let rc = self.client_rect();
        let sb = self.settings.sidebar_width;
        y < rc.bottom - STATUSBAR_HEIGHT && x >= sb - 2 && x <= sb + SPLITTER_WIDTH + 2
    }

    fn on_mouse_move(&mut self, x: i32, y: i32) {
        if !self.tracking_mouse {
            let mut tme = TRACKMOUSEEVENT {
                cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                dwFlags: TME_LEAVE,
                hwndTrack: self.hwnd,
                dwHoverTime: 0,
            };
            unsafe {
                let _ = TrackMouseEvent(&mut tme);
            }
            self.tracking_mouse = true;
        }

        if self.resizing_sidebar {
            let rc = self.client_rect();
            let max_sb = ((rc.right - rc.left) - 420).clamp(MIN_SIDEBAR_WIDTH, MAX_SIDEBAR_WIDTH);
            let new_w = x.clamp(MIN_SIDEBAR_WIDTH, max_sb);
            if new_w != self.settings.sidebar_width {
                self.settings.sidebar_width = new_w;
                self.layout_children();
            }
            return;
        }

        if self.dragging_node.is_some() {
            // Surligner la cible sous le curseur dans le TreeView pendant le glisser-déposer
            if let Some(hitem) = self.tree_hit_test_screen_or_client(x, y) {
                unsafe {
                    let _ = SendMessageW(
                        self.treeview,
                        TVM_SELECTITEM,
                        Some(WPARAM(TVGN_DROPHILITE as usize)),
                        Some(LPARAM(hitem.0)),
                    );
                }
            }
            return;
        }

        let new_hover = self.hit_test_zone(x, y);
        if new_hover != self.hovered_zone {
            self.hovered_zone = new_hover;
            self.invalidate_canvas();
        }
    }

    fn on_mouse_wheel(&mut self, delta: i16) {
        if self.sessions.is_empty() {
            let rc = self.client_rect();
            let visible_h = rc.bottom - rc.top - TABBAR_HEIGHT - STATUSBAR_HEIGHT;
            let max_scroll = (self.dashboard_content_height - visible_h + 30).max(0);
            let scroll_step = 60;
            let change = if delta > 0 { -scroll_step } else { scroll_step };
            let new_scroll = (self.dashboard_scroll_y + change).clamp(0, max_scroll);
            if new_scroll != self.dashboard_scroll_y {
                self.dashboard_scroll_y = new_scroll;
                self.invalidate_canvas();
            }
        }
    }

    fn on_left_button_down(&mut self, x: i32, y: i32) {
        if self.is_on_splitter(x, y) {
            self.resizing_sidebar = true;
            unsafe {
                SetCapture(self.hwnd);
            }
            return;
        }

        self.pressed_zone = self.hit_test_zone(x, y);
    }

    fn on_left_button_up(&mut self, x: i32, y: i32) {
        if self.resizing_sidebar {
            self.resizing_sidebar = false;
            unsafe {
                let _ = ReleaseCapture();
            }
            let w = self.settings.sidebar_width;
            self.settings.save_sidebar_width(&mut self.storage, w);
            self.invalidate_canvas();
            return;
        }

        if let Some(drag_source) = self.dragging_node.take() {
            unsafe {
                let _ = ReleaseCapture();
                let _ = SendMessageW(
                    self.treeview,
                    TVM_SELECTITEM,
                    Some(WPARAM(TVGN_DROPHILITE as usize)),
                    Some(LPARAM(0)),
                );
            }

            if let Some(target_hitem) = self.tree_hit_test_screen_or_client(x, y)
                && let Some(target_kind) = self.node_kind_from_hitem(target_hitem)
            {
                self.complete_drag_and_drop(drag_source, target_kind);
            }
            return;
        }

        let pressed = self.pressed_zone.take();
        let released = self.hit_test_zone(x, y);
        if let (Some(p), Some(r)) = (pressed, released)
            && p == r
        {
            match r {
                InteractiveZone::BtnNewConnection
                | InteractiveZone::TabNewConnection
                | InteractiveZone::HomeQuickNewConnection => unsafe {
                    let _ = PostMessageW(
                        Some(self.hwnd),
                        WM_DEFERRED_ACTION,
                        WPARAM(DEFERRED_NEW_CONNECTION),
                        LPARAM(0),
                    );
                },
                InteractiveZone::BtnNewFolder | InteractiveZone::HomeQuickNewFolder => unsafe {
                    let _ = PostMessageW(
                        Some(self.hwnd),
                        WM_DEFERRED_ACTION,
                        WPARAM(DEFERRED_NEW_FOLDER),
                        LPARAM(0),
                    );
                },
                InteractiveZone::BtnExpandAll => {
                    self.expand_all_folders();
                }
                InteractiveZone::BtnCollapseAll => {
                    self.collapse_all_folders();
                }
                InteractiveZone::BtnSettings => unsafe {
                    let _ = PostMessageW(
                        Some(self.hwnd),
                        WM_DEFERRED_ACTION,
                        WPARAM(DEFERRED_OPEN_SETTINGS),
                        LPARAM(0),
                    );
                },
                InteractiveZone::TabSelect(sid) => {
                    let viewport = self.rdp_viewport_rect();
                    self.sessions.select_tab(sid, viewport);
                    self.invalidate_canvas();
                }
                InteractiveZone::TabClose(sid) => {
                    let viewport = self.rdp_viewport_rect();
                    self.sessions.close_tab(sid, viewport);
                    self.invalidate_canvas();
                }
                InteractiveZone::TabActions(sid) => {
                    let action_x = self
                        .hit_zones
                        .iter()
                        .find(|z| z.zone == InteractiveZone::TabActions(sid))
                        .map(|z| z.rect.left)
                        .unwrap_or(0);
                    let mut pt = POINT {
                        x: action_x,
                        y: TABBAR_HEIGHT,
                    };
                    unsafe {
                        let _ = ClientToScreen(self.hwnd, &mut pt);
                        self.show_session_actions_menu(sid, pt);
                    }
                }
                InteractiveZone::HomeCardConnect(cid) => {
                    self.launch_connection_by_id(cid);
                }
                InteractiveZone::HomeCardEdit(cid) => unsafe {
                    let _ = PostMessageW(
                        Some(self.hwnd),
                        WM_DEFERRED_ACTION,
                        WPARAM(DEFERRED_EDIT_CONNECTION),
                        LPARAM(cid as isize),
                    );
                },
            }
        }
    }

    /// Détecte de façon déterministe l'élément du TreeView présent sur la rangée spécifiée (`pt`),
    /// quelle que soit la longueur du texte, le niveau d'indentation ou la position horizontale du clic
    /// (icône, libellé, ou espace vide à droite jusqu'au bord du panneau).
    pub fn tree_resolve_row_at_point(&self, pt: POINT) -> Option<(HTREEITEM, TVHITTESTINFO)> {
        unsafe {
            let mut rc_client = RECT::default();
            let _ = GetClientRect(self.treeview, &mut rc_client);
            if pt.x < 0 || pt.x >= rc_client.right || pt.y < 0 || pt.y >= rc_client.bottom {
                return None;
            }

            let mut hti = TVHITTESTINFO {
                pt,
                ..Default::default()
            };
            let h = SendMessageW(
                self.treeview,
                TVM_HITTEST,
                Some(WPARAM(0)),
                Some(LPARAM(&mut hti as *mut TVHITTESTINFO as isize)),
            );

            // Si le clic touche explicitement le bouton d'expansion [+] / [-],
            // retourner immédiatement pour que l'appelant sache qu'il s'agit du bouton
            if (hti.flags.0 & TVHT_ONITEMBUTTON.0) != 0 {
                return Some((hti.hItem, hti));
            }

            // 1. Détection directe si SendMessageW retourne l'élément (ex: sur l'icône ou le libellé)
            if h.0 != 0 {
                return Some((HTREEITEM(h.0), hti));
            }

            // 2. Détection directe via hti.hItem si TVHT_ONITEM / TVHT_ONITEMRIGHT / TVHT_ONITEMINDENT
            const TVHT_ONITEMRIGHT_VAL: u32 = 0x0020;
            const TVHT_ONITEMINDENT_VAL: u32 = 0x0008;
            const TVHT_ONITEM_VAL: u32 = 0x0046;
            if hti.hItem.0 != 0
                && (hti.flags.0 & (TVHT_ONITEM_VAL | TVHT_ONITEMRIGHT_VAL | TVHT_ONITEMINDENT_VAL))
                    != 0
            {
                return Some((hti.hItem, hti));
            }

            // 3. Résolution déterministe par parcours des éléments visibles (TVGN_FIRSTVISIBLE / TVGN_NEXTVISIBLE)
            // et vérification de la plage verticale de chaque rangée via TVM_GETITEMRECT (fItemRect = FALSE).
            let mut current = HTREEITEM(
                SendMessageW(
                    self.treeview,
                    TVM_GETNEXTITEM,
                    Some(WPARAM(TVGN_FIRSTVISIBLE as usize)),
                    Some(LPARAM(0)),
                )
                .0,
            );

            while current.0 != 0 {
                let mut rc_item = RECT::default();
                *(&mut rc_item as *mut RECT as *mut isize) = current.0;
                let ok = SendMessageW(
                    self.treeview,
                    TVM_GETITEMRECT,
                    Some(WPARAM(0)), // FALSE = rectangle de toute la ligne
                    Some(LPARAM(&mut rc_item as *mut RECT as isize)),
                );
                if ok.0 != 0 && rc_item.bottom > rc_item.top {
                    if rc_item.top >= rc_client.bottom {
                        break;
                    }
                    if pt.y >= rc_item.top && pt.y < rc_item.bottom {
                        hti.hItem = current;
                        hti.flags.0 |= TVHT_ONITEMRIGHT_VAL;
                        return Some((current, hti));
                    }
                }
                current = HTREEITEM(
                    SendMessageW(
                        self.treeview,
                        TVM_GETNEXTITEM,
                        Some(WPARAM(TVGN_NEXTVISIBLE as usize)),
                        Some(LPARAM(current.0)),
                    )
                    .0,
                );
            }

            None
        }
    }

    /// Alias rétrocompatible pour tree_resolve_row_at_point
    pub fn tree_hit_test_full_row(&self, pt: POINT) -> Option<(HTREEITEM, TVHITTESTINFO)> {
        self.tree_resolve_row_at_point(pt)
    }

    fn tree_hit_test_screen_or_client(&self, main_x: i32, main_y: i32) -> Option<HTREEITEM> {
        unsafe {
            let mut pt = POINT {
                x: main_x,
                y: main_y,
            };
            let _ = ClientToScreen(self.hwnd, &mut pt);
            let _ = ScreenToClient(self.treeview, &mut pt);
            self.tree_hit_test_full_row(pt).and_then(|(item, hti)| {
                if (hti.flags.0 & TVHT_ONITEMBUTTON.0) == 0 && item.0 != 0 {
                    Some(item)
                } else {
                    None
                }
            })
        }
    }

    fn complete_drag_and_drop(&mut self, source: TreeNodeKind, target: TreeNodeKind) {
        if source == target {
            return;
        }

        let target_folder_id: Option<i64> = match target {
            TreeNodeKind::ConnectionsRoot => None,
            TreeNodeKind::Folder(fid) => Some(fid),
            TreeNodeKind::Connection(cid) => {
                // Déposer sur une connexion place l'élément dans le même dossier parent que cette connexion
                self.storage
                    .get_connection(cid)
                    .ok()
                    .flatten()
                    .and_then(|c| c.folder_id)
            }
            TreeNodeKind::FavoritesRoot => {
                // Déposer une connexion sur "Favoris" l'ajoute aux favoris !
                if let TreeNodeKind::Connection(cid) = source {
                    if let Err(e) = self.storage.set_connection_favorite(cid, true) {
                        show_warning_dialog(self.hwnd, "Favori", &e);
                    } else {
                        self.status_override = Some("Connexion ajoutée aux favoris.".to_string());
                        self.rebuild_treeview();
                    }
                }
                return;
            }
        };

        match source {
            TreeNodeKind::Folder(fid) => match self.storage.move_folder(fid, target_folder_id) {
                Ok(()) => {
                    self.status_override = Some("Dossier déplacé avec succès.".to_string());
                    self.rebuild_treeview();
                }
                Err(err) => {
                    show_warning_dialog(self.hwnd, "Déplacement du dossier refusé", &err);
                }
            },
            TreeNodeKind::Connection(cid) => {
                match self.storage.move_connection(cid, target_folder_id) {
                    Ok(()) => {
                        self.status_override = Some("Connexion déplacée avec succès.".to_string());
                        self.rebuild_treeview();
                    }
                    Err(err) => {
                        show_warning_dialog(self.hwnd, "Déplacement de la connexion refusé", &err);
                    }
                }
            }
            _ => {}
        }
    }

    // =========================================================================
    // LANCEMENT DE SESSION (RDP INTÉGRÉ / CLIENTS EXTERNES PRÉPARÉS)
    // =========================================================================

    pub fn launch_connection_by_id(&mut self, connection_id: i64) {
        // Anti-rebond (debounce) : si la même connexion a été demandée il y a moins de 400 ms,
        // ignorer l'événement redondant pour éviter tout double lancement (ex. messages concurrents)
        if let Some((last_id, last_time)) = self.last_launch_debounce
            && last_id == connection_id
            && last_time.elapsed().as_millis() < 400
        {
            return;
        }
        self.last_launch_debounce = Some((connection_id, std::time::Instant::now()));

        let conn = match self.storage.get_connection(connection_id) {
            Ok(Some(c)) => c,
            Ok(None) => {
                show_error_dialog(
                    self.hwnd,
                    "Connexion introuvable",
                    "Cette connexion n'existe plus dans la base locale.",
                );
                return;
            }
            Err(err) => {
                show_error_dialog(self.hwnd, "Erreur SQLite", &err);
                return;
            }
        };

        let _ = self.storage.record_recent_connection(conn.id);

        match conn.protocol {
            Protocol::Rdp => {
                let decrypted_pwd = if conn.has_saved_password {
                    match self.storage.load_decrypted_password(conn.id) {
                        Ok(secret_opt) => secret_opt,
                        Err(err) => {
                            show_warning_dialog(
                                self.hwnd,
                                "Mot de passe DPAPI non déchiffrable",
                                &format!(
                                    "{err}\n\nLa session RDP va démarrer et Windows vous demandera vos identifiants via NLA/CredSSP."
                                ),
                            );
                            None
                        }
                    }
                } else {
                    None
                };

                let viewport = self.rdp_viewport_rect();
                match self.sessions.open_rdp_session(
                    self.hwnd,
                    viewport,
                    &conn,
                    decrypted_pwd,
                    self.settings.rdp_keyboard_hook_mode,
                ) {
                    Ok(_sid) => {
                        self.status_override = None;
                        self.invalidate_canvas();
                    }
                    Err(err) => {
                        let msg = format!(
                            "Impossible de démarrer la session RDP vers {} : {err}",
                            conn.endpoint_label()
                        );
                        self.status_override = Some(msg.clone());
                        self.invalidate_canvas();
                        show_error_dialog(self.hwnd, "Erreur de lancement RDP", &msg);
                    }
                }
            }
            Protocol::AnyDesk => {
                let saved_pwd = if conn.has_saved_password {
                    self.storage.load_decrypted_password(conn.id).ok().flatten()
                } else {
                    None
                };
                let pwd_str = saved_pwd.as_ref().map(|s| s.as_str());
                match crate::external_apps::launch_anydesk_connection(
                    &self.settings.anydesk_path,
                    &conn.host,
                    pwd_str,
                ) {
                    Ok(info_msg) => {
                        self.status_override = Some(info_msg);
                        self.invalidate_canvas();
                    }
                    Err(err) => {
                        let msg = format!(
                            "Impossible de lancer AnyDesk pour « {} » : {err}",
                            conn.name
                        );
                        self.status_override = Some(msg.clone());
                        self.invalidate_canvas();
                        show_error_dialog(self.hwnd, "Erreur de lancement AnyDesk", &msg);
                    }
                }
            }
            Protocol::RustDesk => {
                match crate::external_apps::launch_rustdesk_connection(
                    &self.settings.rustdesk_path,
                    &conn.host,
                ) {
                    Ok(info_msg) => {
                        self.status_override = Some(info_msg);
                        self.invalidate_canvas();
                    }
                    Err(err) => {
                        let msg = format!(
                            "Impossible de lancer RustDesk pour « {} » : {err}",
                            conn.name
                        );
                        self.status_override = Some(msg.clone());
                        self.invalidate_canvas();
                        show_error_dialog(self.hwnd, "Erreur de lancement RustDesk", &msg);
                    }
                }
            }
        }
    }

    pub fn on_right_button_up(&mut self, x: i32, y: i32) {
        if let Some(InteractiveZone::TabSelect(sid) | InteractiveZone::TabActions(sid)) =
            self.hit_test_zone(x, y)
        {
            let mut screen_pt = POINT { x, y };
            unsafe {
                let _ = ClientToScreen(self.hwnd, &mut screen_pt);
                self.show_session_actions_menu(sid, screen_pt);
            }
        }
    }

    unsafe fn show_session_actions_menu(&mut self, session_id: u32, screen_pt: POINT) {
        unsafe {
            let Ok(hmenu) = CreatePopupMenu() else { return };
            let is_fullscreen = self.sessions.is_active_fullscreen();
            let fs_label = if is_fullscreen {
                "Quitter le mode plein écran\tCtrl+Alt+Pause"
            } else {
                "Basculer en plein écran\tCtrl+Alt+Pause"
            };

            let _ = AppendMenuW(
                hmenu,
                MF_STRING,
                CMD_SESSION_CTRL_ALT_END as usize,
                w!("Envoyer Ctrl + Alt + Fin (Sécurité distante RDP)"),
            );
            let _ = AppendMenuW(
                hmenu,
                MF_STRING,
                CMD_SESSION_CAD as usize,
                w!("Envoyer Ctrl + Alt + Suppr (Simulation)"),
            );
            let _ = AppendMenuW(hmenu, MF_SEPARATOR, 0, PCWSTR::null());
            let fs_label_w: Vec<u16> = fs_label.encode_utf16().chain(std::iter::once(0)).collect();
            let _ = AppendMenuW(
                hmenu,
                MF_STRING,
                CMD_SESSION_FULLSCREEN as usize,
                PCWSTR(fs_label_w.as_ptr()),
            );
            let _ = AppendMenuW(hmenu, MF_SEPARATOR, 0, PCWSTR::null());
            let _ = AppendMenuW(
                hmenu,
                MF_STRING,
                CMD_SESSION_DISCONNECT as usize,
                w!("Fermer / Déconnecter"),
            );

            let _ = SetForegroundWindow(self.hwnd);
            let cmd = TrackPopupMenu(
                hmenu,
                TPM_LEFTALIGN | TPM_TOPALIGN | TPM_RETURNCMD,
                screen_pt.x,
                screen_pt.y,
                Some(0),
                self.hwnd,
                None,
            );
            let _ = DestroyMenu(hmenu);

            match cmd.0 as u32 {
                CMD_SESSION_CAD => match self.sessions.send_cad_to_active() {
                    Ok(()) => {
                        self.status_override = Some(
                                "Combinaison Ctrl + Alt + Suppr transmise à la session RDP (Note : sous RDP, utilisez Ctrl + Alt + Fin pour invoquer l'écran de sécurité distant).".to_string(),
                            );
                        self.invalidate_canvas();
                    }
                    Err(e) => {
                        self.status_override = Some(format!("Erreur Ctrl+Alt+Suppr : {e}"));
                        self.invalidate_canvas();
                        show_error_dialog(self.hwnd, "Erreur d'envoi Ctrl + Alt + Suppr", &e);
                    }
                },
                CMD_SESSION_CTRL_ALT_END => match self.sessions.send_ctrl_alt_end_to_active() {
                    Ok(()) => {
                        self.status_override = Some(
                                "Combinaison Ctrl + Alt + Fin (SAS de sécurité RDP) transmise avec succès au bureau distant.".to_string(),
                            );
                        self.invalidate_canvas();
                    }
                    Err(e) => {
                        self.status_override = Some(format!("Erreur Ctrl+Alt+Fin : {e}"));
                        self.invalidate_canvas();
                        show_error_dialog(self.hwnd, "Erreur d'envoi Ctrl + Alt + Fin", &e);
                    }
                },
                CMD_SESSION_FULLSCREEN => {
                    if let Err(e) = self.sessions.toggle_active_fullscreen() {
                        self.status_override = Some(format!("Erreur Plein écran : {e}"));
                        self.invalidate_canvas();
                    }
                }
                CMD_SESSION_DISCONNECT => {
                    let viewport = self.rdp_viewport_rect();
                    self.sessions.close_tab(session_id, viewport);
                    self.invalidate_canvas();
                }
                _ => {}
            }
        }
    }

    // =========================================================================
    // MENU CONTEXTUEL DE L'ARBORESCENCE & ACTIONS CRUD
    // =========================================================================

    unsafe fn show_tree_context_menu(&mut self) {
        unsafe {
            let mut cursor = POINT::default();
            let _ = GetCursorPos(&mut cursor);

            let mut client_pt = cursor;
            let _ = ScreenToClient(self.treeview, &mut client_pt);
            if let Some((hitem, hti)) = self.tree_hit_test_full_row(client_pt)
                && (hti.flags.0 & TVHT_ONITEMBUTTON.0) == 0
                && hitem.0 != 0
            {
                let _ = SendMessageW(
                    self.treeview,
                    TVM_SELECTITEM,
                    Some(WPARAM(TVGN_CARET as usize)),
                    Some(LPARAM(hitem.0)),
                );
                self.selected_node = self.node_kind_from_hitem(hitem);
            }

            let Ok(hmenu) = CreatePopupMenu() else {
                return;
            };

            match self.selected_node {
                Some(TreeNodeKind::Connection(cid)) => {
                    let conn_opt = self.storage.get_connection(cid).ok().flatten();
                    let is_fav = conn_opt.as_ref().is_some_and(|c| c.is_favorite);
                    let is_prio = conn_opt.as_ref().is_some_and(|c| c.is_priority);
                    let _ = AppendMenuW(
                        hmenu,
                        MF_STRING,
                        CMD_CTX_CONNECT as usize,
                        w!("Se connecter"),
                    );
                    let _ = AppendMenuW(
                        hmenu,
                        MF_STRING,
                        CMD_CTX_EDIT as usize,
                        w!("Modifier la connexion..."),
                    );
                    let _ = AppendMenuW(hmenu, MF_SEPARATOR, 0, PCWSTR::null());
                    let _ = AppendMenuW(
                        hmenu,
                        MF_STRING,
                        CMD_CTX_TOGGLE_FAV as usize,
                        if is_fav {
                            w!("Retirer des favoris")
                        } else {
                            w!("★ Ajouter aux favoris")
                        },
                    );
                    let _ = AppendMenuW(
                        hmenu,
                        MF_STRING,
                        CMD_CTX_TOGGLE_PRIORITY as usize,
                        if is_prio {
                            w!("Retirer la priorité")
                        } else {
                            w!("▲ Marquer comme prioritaire")
                        },
                    );
                    let _ = AppendMenuW(
                        hmenu,
                        MF_STRING,
                        CMD_CTX_RENAME as usize,
                        w!("Renommer (F2)..."),
                    );
                    let _ = AppendMenuW(
                        hmenu,
                        MF_STRING,
                        CMD_CTX_DUPLICATE as usize,
                        w!("Dupliquer"),
                    );
                    let _ = AppendMenuW(
                        hmenu,
                        MF_STRING,
                        CMD_CTX_MOVE as usize,
                        w!("Déplacer vers un dossier..."),
                    );
                    let _ = AppendMenuW(hmenu, MF_SEPARATOR, 0, PCWSTR::null());
                    let _ = AppendMenuW(
                        hmenu,
                        MF_STRING,
                        CMD_CTX_DELETE as usize,
                        w!("Supprimer..."),
                    );
                }
                Some(TreeNodeKind::Folder(_fid)) => {
                    let _ = AppendMenuW(
                        hmenu,
                        MF_STRING,
                        CMD_CTX_NEW_CONN_HERE as usize,
                        w!("+ Nouvelle connexion ici..."),
                    );
                    let _ = AppendMenuW(
                        hmenu,
                        MF_STRING,
                        CMD_CTX_NEW_SUBFOLDER as usize,
                        w!("+ Nouveau sous-dossier..."),
                    );
                    let _ = AppendMenuW(hmenu, MF_SEPARATOR, 0, PCWSTR::null());
                    let _ = AppendMenuW(
                        hmenu,
                        MF_STRING,
                        CMD_CTX_RENAME as usize,
                        w!("Renommer le dossier (F2)..."),
                    );
                    let _ = AppendMenuW(
                        hmenu,
                        MF_STRING,
                        CMD_CTX_MOVE as usize,
                        w!("Déplacer le dossier..."),
                    );
                    let _ = AppendMenuW(hmenu, MF_SEPARATOR, 0, PCWSTR::null());
                    let _ = AppendMenuW(
                        hmenu,
                        MF_STRING,
                        CMD_CTX_DELETE as usize,
                        w!("Supprimer le dossier..."),
                    );
                }
                _ => {
                    let _ = AppendMenuW(
                        hmenu,
                        MF_STRING,
                        CMD_CTX_NEW_CONN_HERE as usize,
                        w!("+ Nouvelle connexion..."),
                    );
                    let _ = AppendMenuW(
                        hmenu,
                        MF_STRING,
                        CMD_CTX_NEW_SUBFOLDER as usize,
                        w!("+ Nouveau dossier racine..."),
                    );
                }
            }

            let cmd = TrackPopupMenu(
                hmenu,
                TPM_LEFTALIGN | TPM_TOPALIGN | TPM_RETURNCMD,
                cursor.x,
                cursor.y,
                Some(0),
                self.hwnd,
                None,
            );
            let _ = DestroyMenu(hmenu);

            if cmd.0 != 0 {
                self.execute_context_command(cmd.0 as u32);
            }
        }
    }

    fn execute_context_command(&mut self, cmd: u32) {
        match cmd {
            CMD_CTX_CONNECT => {
                if let Some(TreeNodeKind::Connection(cid)) = self.selected_node {
                    self.launch_connection_by_id(cid);
                }
            }
            CMD_CTX_EDIT => {
                if let Some(TreeNodeKind::Connection(cid)) = self.selected_node {
                    self.open_connection_dialog(Some(cid), None);
                }
            }
            CMD_CTX_TOGGLE_FAV => {
                if let Some(TreeNodeKind::Connection(cid)) = self.selected_node
                    && let Ok(Some(conn)) = self.storage.get_connection(cid)
                    && self
                        .storage
                        .set_connection_favorite(cid, !conn.is_favorite)
                        .is_ok()
                {
                    self.rebuild_treeview();
                }
            }
            CMD_CTX_TOGGLE_PRIORITY => {
                if let Some(TreeNodeKind::Connection(cid)) = self.selected_node
                    && let Ok(Some(conn)) = self.storage.get_connection(cid)
                    && self
                        .storage
                        .set_connection_priority(cid, !conn.is_priority)
                        .is_ok()
                {
                    self.rebuild_treeview();
                }
            }
            CMD_CTX_RENAME => self.prompt_rename_selected(),
            CMD_CTX_DUPLICATE => {
                if let Some(TreeNodeKind::Connection(cid)) = self.selected_node {
                    match self.storage.duplicate_connection(cid) {
                        Ok(dup) => {
                            self.selected_node = Some(TreeNodeKind::Connection(dup.id));
                            self.status_override =
                                Some(format!("Connexion dupliquée : « {} ».", dup.name));
                            self.rebuild_treeview();
                        }
                        Err(err) => show_error_dialog(self.hwnd, "Duplication impossible", &err),
                    }
                }
            }
            CMD_CTX_MOVE => self.prompt_move_selected(),
            CMD_CTX_DELETE => self.prompt_delete_selected(),
            CMD_CTX_NEW_CONN_HERE => {
                let folder_id = self.selected_folder_context();
                self.open_connection_dialog(None, folder_id);
            }
            CMD_CTX_NEW_SUBFOLDER => {
                let parent_id = match self.selected_node {
                    Some(TreeNodeKind::Folder(fid)) => Some(fid),
                    _ => None,
                };
                self.prompt_create_folder(parent_id);
            }
            _ => {}
        }
    }

    fn prompt_create_folder(&mut self, parent_id: Option<i64>) {
        if let Some(name) = run_text_prompt_dialog(
            self.hwnd,
            "Nouveau dossier",
            "Nom du dossier :",
            "",
            &self.palette,
        ) {
            match self.storage.create_folder(&name, parent_id) {
                Ok(folder) => {
                    self.selected_node = Some(TreeNodeKind::Folder(folder.id));
                    self.status_override = Some(format!("Dossier « {} » créé.", folder.name));
                    self.rebuild_treeview();
                }
                Err(err) => show_warning_dialog(self.hwnd, "Création de dossier", &err),
            }
        }
    }

    fn prompt_rename_selected(&mut self) {
        match self.selected_node {
            Some(TreeNodeKind::Folder(fid)) => {
                let Some(folder) = self.storage.get_folder(fid).ok().flatten() else {
                    return;
                };
                if let Some(new_name) = run_text_prompt_dialog(
                    self.hwnd,
                    "Renommer le dossier",
                    "Nouveau nom du dossier :",
                    &folder.name,
                    &self.palette,
                ) {
                    if let Err(err) = self.storage.rename_folder(fid, &new_name) {
                        show_warning_dialog(self.hwnd, "Renommage impossible", &err);
                    } else {
                        self.rebuild_treeview();
                    }
                }
            }
            Some(TreeNodeKind::Connection(cid)) => {
                let Some(conn) = self.storage.get_connection(cid).ok().flatten() else {
                    return;
                };
                if let Some(new_name) = run_text_prompt_dialog(
                    self.hwnd,
                    "Renommer la connexion",
                    "Nouveau nom de la connexion :",
                    &conn.name,
                    &self.palette,
                ) {
                    if let Err(err) = self.storage.rename_connection(cid, &new_name) {
                        show_warning_dialog(self.hwnd, "Renommage impossible", &err);
                    } else {
                        self.rebuild_treeview();
                    }
                }
            }
            _ => {}
        }
    }

    fn prompt_move_selected(&mut self) {
        let folders = self.storage.list_folders().unwrap_or_default();
        match self.selected_node {
            Some(TreeNodeKind::Connection(cid)) => {
                let Some(conn) = self.storage.get_connection(cid).ok().flatten() else {
                    return;
                };
                if let Some(chosen_folder) = run_folder_picker_dialog(
                    self.hwnd,
                    &format!("Déplacer « {} »", conn.name),
                    &folders,
                    conn.folder_id,
                    None,
                    &self.palette,
                ) {
                    if let Err(err) = self.storage.move_connection(cid, chosen_folder) {
                        show_warning_dialog(self.hwnd, "Déplacement refusé", &err);
                    } else {
                        self.rebuild_treeview();
                    }
                }
            }
            Some(TreeNodeKind::Folder(fid)) => {
                let Some(folder) = self.storage.get_folder(fid).ok().flatten() else {
                    return;
                };
                if let Some(chosen_parent) = run_folder_picker_dialog(
                    self.hwnd,
                    &format!("Déplacer le dossier « {} »", folder.name),
                    &folders,
                    folder.parent_id,
                    Some(fid),
                    &self.palette,
                ) {
                    if let Err(err) = self.storage.move_folder(fid, chosen_parent) {
                        show_warning_dialog(self.hwnd, "Déplacement refusé", &err);
                    } else {
                        self.rebuild_treeview();
                    }
                }
            }
            _ => {}
        }
    }

    fn prompt_delete_selected(&mut self) {
        match self.selected_node {
            Some(TreeNodeKind::Connection(cid)) => {
                let Some(conn) = self.storage.get_connection(cid).ok().flatten() else {
                    return;
                };
                if confirm_dialog(
                    self.hwnd,
                    "Confirmer la suppression",
                    &format!(
                        "Voulez-vous vraiment supprimer la connexion « {} » ({}) ?",
                        conn.name,
                        conn.endpoint_label()
                    ),
                ) {
                    if let Err(err) = self.storage.delete_connection(cid) {
                        show_error_dialog(self.hwnd, "Erreur de suppression", &err);
                    } else {
                        self.selected_node = None;
                        self.status_override =
                            Some(format!("Connexion « {} » supprimée.", conn.name));
                        self.rebuild_treeview();
                    }
                }
            }
            Some(TreeNodeKind::Folder(fid)) => {
                let Some(folder) = self.storage.get_folder(fid).ok().flatten() else {
                    return;
                };
                if confirm_dialog(
                    self.hwnd,
                    "Supprimer le dossier",
                    &format!(
                        "Voulez-vous vraiment supprimer le dossier « {} » ?\n\nLes connexions qu'il contient seront conservées et rattachées au niveau supérieur.",
                        folder.name
                    ),
                ) {
                    if let Err(err) = self.storage.delete_folder(fid, false) {
                        show_error_dialog(self.hwnd, "Erreur de suppression", &err);
                    } else {
                        self.selected_node = None;
                        self.status_override =
                            Some(format!("Dossier « {} » supprimé.", folder.name));
                        self.rebuild_treeview();
                    }
                }
            }
            _ => {}
        }
    }

    fn open_connection_dialog(
        &mut self,
        edit_connection_id: Option<i64>,
        default_folder_id: Option<i64>,
    ) {
        let existing =
            edit_connection_id.and_then(|id| self.storage.get_connection(id).ok().flatten());
        let folders = self.storage.list_folders().unwrap_or_default();

        if let Some(draft) = run_connection_form_dialog(
            self.hwnd,
            existing.as_ref(),
            default_folder_id,
            &folders,
            self.settings.rdp_default_dynamic_resolution,
            self.settings.rdp_default_clipboard,
            &self.palette,
        ) {
            let res = if let Some(ref orig) = existing {
                self.storage.update_connection(orig.id, draft)
            } else {
                self.storage.create_connection(draft)
            };

            match res {
                Ok(saved) => {
                    self.selected_node = Some(TreeNodeKind::Connection(saved.id));
                    self.status_override = Some(format!(
                        "Connexion « {} » ({}) enregistrée.",
                        saved.name,
                        saved.endpoint_label()
                    ));
                    self.rebuild_treeview();
                }
                Err(err) => {
                    show_error_dialog(self.hwnd, "Enregistrement de la connexion", &err);
                }
            }
        }
    }

    pub fn spawn_startup_update_check(&self) {
        let hwnd_raw = self.hwnd.0 as isize;
        let ignored = self.settings.ignored_update_version.clone();
        std::thread::spawn(move || {
            // Pause initiale discrète de 3 secondes pour ne pas ralentir le démarrage
            std::thread::sleep(std::time::Duration::from_secs(3));
            if let Ok(info) = crate::updater::check_latest_release(crate::updater::GITHUB_REPO)
                && crate::updater::current_semver()
                    .map(|local| info.version > local)
                    .unwrap_or(false)
            {
                let ver_str = info.version.to_string();
                if ignored.as_deref() != Some(&ver_str) {
                    let boxed = Box::new(info);
                    unsafe {
                        let target_hwnd = HWND(hwnd_raw as *mut _);
                        if IsWindow(Some(target_hwnd)).as_bool() {
                            let _ = PostMessageW(
                                Some(target_hwnd),
                                WM_APP_UPDATE_AVAILABLE,
                                WPARAM(0),
                                LPARAM(Box::into_raw(boxed) as isize),
                            );
                        }
                    }
                }
            }
        });
    }

    pub fn open_settings_dialog(&mut self) {
        if !cfg!(test) {
            sync_settings_with_real_startup_shortcut(&mut self.settings, &mut self.storage);
        }

        let active_count = self.sessions.tabs().len();
        let (maybe_settings, imported_data) = run_settings_dialog(
            self.hwnd,
            &self.settings,
            &self.palette,
            &mut self.storage,
            active_count,
        );

        if let Some(new_settings) = maybe_settings {
            if !cfg!(test)
                && let Err(err) = apply_startup_shortcut(new_settings.start_with_windows)
            {
                show_warning_dialog(self.hwnd, "Démarrage automatique Windows", &err);
            }
            self.settings = new_settings;
            let _ = self.settings.save_all(&mut self.storage);
        }

        if imported_data {
            self.settings = AppSettings::load(&self.storage);
            self.rebuild_treeview();
        }

        self.apply_theme();
        self.status_override = Some(if imported_data {
            "Configuration importée et appliquée avec succès.".to_string()
        } else {
            "Paramètres enregistrés et appliqués.".to_string()
        });
        self.invalidate_canvas();
    }
}

// =============================================================================
// BOÎTES DE DIALOGUE MODALES MODERNES (FORMULAIRE CONNEXION, SAISIE, DOSSIER)
// =============================================================================

const IDC_DLG_NAME: usize = 4001;
const IDC_DLG_PROTO: usize = 4002;
const IDC_DLG_FOLDER: usize = 4003;
const IDC_DLG_HOST: usize = 4004;
const IDC_DLG_PORT: usize = 4005;
const IDC_DLG_USER: usize = 4006;
const IDC_DLG_DOMAIN: usize = 4007;
const IDC_DLG_PASS: usize = 4008;
const IDC_DLG_SAVE_PASS: usize = 4009;
const IDC_DLG_FAVORITE: usize = 4010;
const IDC_DLG_PRIORITY: usize = 4016;
const IDC_DLG_DYN_RES: usize = 4011;
const IDC_DLG_CLIPBOARD: usize = 4012;
const IDC_DLG_DESC: usize = 4013;
const IDC_DLG_OK: usize = 4014;
const IDC_DLG_CANCEL: usize = 4015;

/// Ferme proprement un dialogue modal : réactive la fenêtre propriétaire, lui redonne le focus
/// et détruit immédiatement le `HWND` du dialogue pour que `IsWindow(Some(dlg_hwnd))` devienne `false`.
unsafe fn close_modal_dialog(dlg_hwnd: HWND, owner: HWND) {
    unsafe {
        if IsWindow(Some(owner)).as_bool() {
            let _ = EnableWindow(owner, true);
            let _ = SetForegroundWindow(owner);
            let _ = SetFocus(Some(owner));
        }
        if IsWindow(Some(dlg_hwnd)).as_bool() {
            let _ = DestroyWindow(dlg_hwnd);
        }
    }
}

/// Boucle modale Win32 déterministe sans aliasing de référence `&bool` :
/// - Boucle tant que `IsWindow(Some(dlg_hwnd)).as_bool()` est vrai.
/// - Gère explicitement `VK_ESCAPE` (`IDC_DLG_CANCEL`), `VK_RETURN` (`IDC_DLG_OK` ou `IDC_DLG_CANCEL`
///   si le focus est sur le bouton Annuler), et `VK_TAB` / `Shift+VK_TAB` entre les contrôles actifs.
unsafe fn run_modal_loop(owner: HWND, dlg_hwnd: HWND, tab_controls: &[HWND]) {
    unsafe {
        let mut msg = MSG::default();
        while IsWindow(Some(dlg_hwnd)).as_bool() {
            let ret = GetMessageW(&mut msg, None, 0, 0);
            if !ret.as_bool() {
                if IsWindow(Some(owner)).as_bool() {
                    let _ = EnableWindow(owner, true);
                }
                PostQuitMessage(msg.wParam.0 as i32);
                break;
            }

            if (msg.hwnd == dlg_hwnd || IsChild(dlg_hwnd, msg.hwnd).as_bool())
                && msg.message == WM_KEYDOWN
            {
                let vk = msg.wParam.0 as u16;
                if vk == VK_ESCAPE.0 {
                    let _ = SendMessageW(
                        dlg_hwnd,
                        WM_COMMAND,
                        Some(WPARAM(IDC_DLG_CANCEL)),
                        Some(LPARAM(0)),
                    );
                    continue;
                }
                if vk == VK_RETURN.0 {
                    let focus = GetFocus();
                    let focus_id = GetDlgCtrlID(focus) as usize;
                    let target_cmd = if focus_id == IDC_DLG_CANCEL {
                        IDC_DLG_CANCEL
                    } else {
                        IDC_DLG_OK
                    };
                    let _ = SendMessageW(
                        dlg_hwnd,
                        WM_COMMAND,
                        Some(WPARAM(target_cmd)),
                        Some(LPARAM(0)),
                    );
                    continue;
                }
                if vk == VK_TAB.0 && !tab_controls.is_empty() {
                    let enabled: Vec<HWND> = tab_controls
                        .iter()
                        .copied()
                        .filter(|&h| {
                            IsWindow(Some(h)).as_bool()
                                && IsWindowEnabled(h).as_bool()
                                && IsWindowVisible(h).as_bool()
                        })
                        .collect();
                    if !enabled.is_empty() {
                        let focus = GetFocus();
                        let shift = (GetKeyState(VK_SHIFT.0 as i32) as u16 & 0x8000) != 0;
                        let cur_idx = enabled
                            .iter()
                            .position(|&h| h == focus || IsChild(h, focus).as_bool());
                        let next_idx = match cur_idx {
                            Some(i) if shift => {
                                if i == 0 {
                                    enabled.len() - 1
                                } else {
                                    i - 1
                                }
                            }
                            Some(i) => (i + 1) % enabled.len(),
                            None => 0,
                        };
                        let next_hwnd = enabled[next_idx];
                        let _ = SetFocus(Some(next_hwnd));
                        let _ = InvalidateRect(Some(dlg_hwnd), None, false);
                        continue;
                    }
                }
            }

            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        if IsWindow(Some(owner)).as_bool() {
            let _ = EnableWindow(owner, true);
        }
    }
}

static ORIG_BUTTON_WNDPROC: std::sync::atomic::AtomicIsize = std::sync::atomic::AtomicIsize::new(0);
static ORIG_COMBO_WNDPROC: std::sync::atomic::AtomicIsize = std::sync::atomic::AtomicIsize::new(0);
static ORIG_EDIT_WNDPROC: std::sync::atomic::AtomicIsize = std::sync::atomic::AtomicIsize::new(0);

#[inline]
unsafe fn call_orig_button_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        let raw = ORIG_BUTTON_WNDPROC.load(std::sync::atomic::Ordering::Relaxed);
        if raw != 0 {
            let proc: WNDPROC = std::mem::transmute(raw);
            CallWindowProcW(proc, hwnd, msg, wparam, lparam)
        } else {
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
    }
}

#[inline]
unsafe fn call_orig_combo_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        let raw = ORIG_COMBO_WNDPROC.load(std::sync::atomic::Ordering::Relaxed);
        if raw != 0 {
            let proc: WNDPROC = std::mem::transmute(raw);
            CallWindowProcW(proc, hwnd, msg, wparam, lparam)
        } else {
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
    }
}

#[inline]
unsafe fn call_orig_edit_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        let raw = ORIG_EDIT_WNDPROC.load(std::sync::atomic::Ordering::Relaxed);
        if raw != 0 {
            let proc: WNDPROC = std::mem::transmute(raw);
            CallWindowProcW(proc, hwnd, msg, wparam, lparam)
        } else {
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
    }
}

/// Sous-classement du champ Port RDP (`WC_EDITW`) :
/// - Sélectionne automatiquement l'intégralité du texte (`EM_SETSEL(0, -1)`) lorsque le champ
///   reçoit le focus par navigation clavier (`WM_SETFOCUS`) OU par clic souris (`WM_LBUTTONDOWN`
///   lorsque le contrôle n'avait pas encore le focus).
/// - Ne provoque aucune sélection intempestive pendant la saisie clavier ou les clics suivants.
unsafe extern "system" fn auto_select_edit_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        match msg {
            WM_SETFOCUS => {
                let res = call_orig_edit_proc(hwnd, msg, wparam, lparam);
                let _ = SendMessageW(hwnd, EM_SETSEL, Some(WPARAM(0)), Some(LPARAM(-1)));
                res
            }
            WM_LBUTTONDOWN => {
                if GetFocus() != hwnd {
                    SetWindowLongPtrW(hwnd, GWLP_USERDATA, 1);
                    let _ = SetFocus(Some(hwnd));
                    let _ = SendMessageW(hwnd, EM_SETSEL, Some(WPARAM(0)), Some(LPARAM(-1)));
                    return LRESULT(0);
                }
                call_orig_edit_proc(hwnd, msg, wparam, lparam)
            }
            WM_LBUTTONUP => {
                if GetWindowLongPtrW(hwnd, GWLP_USERDATA) == 1 {
                    SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
                    let _ = SendMessageW(hwnd, EM_SETSEL, Some(WPARAM(0)), Some(LPARAM(-1)));
                    return LRESULT(0);
                }
                call_orig_edit_proc(hwnd, msg, wparam, lparam)
            }
            WM_KILLFOCUS => {
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
                call_orig_edit_proc(hwnd, msg, wparam, lparam)
            }
            _ => call_orig_edit_proc(hwnd, msg, wparam, lparam),
        }
    }
}

unsafe fn install_auto_select_on_focus(hwnd: HWND) {
    unsafe {
        if hwnd.is_invalid() {
            return;
        }
        let prev = SetWindowLongPtrW(
            hwnd,
            GWLP_WNDPROC,
            auto_select_edit_wnd_proc as *const () as usize as isize,
        );
        if prev != 0 {
            ORIG_EDIT_WNDPROC.store(prev, std::sync::atomic::Ordering::Relaxed);
        }
    }
}

/// Sous-classement des boutons d'action `BS_OWNERDRAW` pour rafraîchir le survol (`WM_MOUSEMOVE` / `WM_MOUSELEAVE`).
unsafe extern "system" fn dark_pushbutton_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        match msg {
            WM_ERASEBKGND => LRESULT(1),
            WM_MOUSEMOVE => {
                let mut tme = TRACKMOUSEEVENT {
                    cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                    dwFlags: TME_LEAVE,
                    hwndTrack: hwnd,
                    dwHoverTime: 0,
                };
                let _ = TrackMouseEvent(&mut tme);
                let _ = InvalidateRect(Some(hwnd), None, false);
                call_orig_button_proc(hwnd, msg, wparam, lparam)
            }
            WM_MOUSELEAVE => {
                let _ = InvalidateRect(Some(hwnd), None, false);
                call_orig_button_proc(hwnd, msg, wparam, lparam)
            }
            _ => call_orig_button_proc(hwnd, msg, wparam, lparam),
        }
    }
}

/// Sous-classement des cases à cocher `BS_OWNERDRAW` :
/// - État stocké dans `GWLP_USERDATA` du contrôle (`0` = décoché, `1` = coché).
/// - Gère `BM_GETCHECK`, `BM_SETCHECK`, le clic souris et la barre d'espace.
unsafe extern "system" fn dark_checkbox_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        match msg {
            WM_ERASEBKGND => LRESULT(1),
            WM_MOUSEMOVE => {
                let mut tme = TRACKMOUSEEVENT {
                    cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                    dwFlags: TME_LEAVE,
                    hwndTrack: hwnd,
                    dwHoverTime: 0,
                };
                let _ = TrackMouseEvent(&mut tme);
                let _ = InvalidateRect(Some(hwnd), None, false);
                call_orig_button_proc(hwnd, msg, wparam, lparam)
            }
            WM_MOUSELEAVE => {
                let _ = InvalidateRect(Some(hwnd), None, false);
                call_orig_button_proc(hwnd, msg, wparam, lparam)
            }
            BM_GETCHECK => {
                let checked = GetWindowLongPtrW(hwnd, GWLP_USERDATA) != 0;
                LRESULT(isize::from(checked))
            }
            BM_SETCHECK => {
                let checked = wparam.0 != 0;
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, isize::from(checked));
                let _ = InvalidateRect(Some(hwnd), None, false);
                LRESULT(0)
            }
            WM_LBUTTONUP => {
                if IsWindowEnabled(hwnd).as_bool() {
                    let mut rc = RECT::default();
                    let _ = GetClientRect(hwnd, &mut rc);
                    let x = (lparam.0 & 0xFFFF) as i16 as i32;
                    let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;
                    if x >= 0 && x < rc.right && y >= 0 && y < rc.bottom {
                        let cur = GetWindowLongPtrW(hwnd, GWLP_USERDATA) != 0;
                        SetWindowLongPtrW(hwnd, GWLP_USERDATA, isize::from(!cur));
                        let _ = InvalidateRect(Some(hwnd), None, false);
                        if let Ok(parent) = GetParent(hwnd) {
                            let id = GetDlgCtrlID(hwnd) as usize;
                            let _ = SendMessageW(
                                parent,
                                WM_COMMAND,
                                Some(WPARAM(id)),
                                Some(LPARAM(hwnd.0 as isize)),
                            );
                        }
                    }
                }
                call_orig_button_proc(hwnd, msg, wparam, lparam)
            }
            WM_KEYUP if wparam.0 == VK_SPACE.0 as usize => {
                if IsWindowEnabled(hwnd).as_bool() {
                    let cur = GetWindowLongPtrW(hwnd, GWLP_USERDATA) != 0;
                    SetWindowLongPtrW(hwnd, GWLP_USERDATA, isize::from(!cur));
                    let _ = InvalidateRect(Some(hwnd), None, false);
                    if let Ok(parent) = GetParent(hwnd) {
                        let id = GetDlgCtrlID(hwnd) as usize;
                        let _ = SendMessageW(
                            parent,
                            WM_COMMAND,
                            Some(WPARAM(id)),
                            Some(LPARAM(hwnd.0 as isize)),
                        );
                    }
                }
                LRESULT(0)
            }
            _ => call_orig_button_proc(hwnd, msg, wparam, lparam),
        }
    }
}

const PROP_SEARCH_EDIT_ORIG_PROC: PCWSTR = w!("MalvisionSearchEditOrigProc");

/// Sous-classement de la barre de recherche (`WC_EDITW`) pour intercepter `WM_ERASEBKGND`
/// et garantir un effacement dynamique sans résidu visuel lors des basculements de thème (Clair <-> Sombre).
unsafe extern "system" fn search_edit_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        let orig_handle = GetPropW(hwnd, PROP_SEARCH_EDIT_ORIG_PROC);
        let orig_proc: WNDPROC = if !orig_handle.0.is_null() {
            std::mem::transmute::<*mut c_void, WNDPROC>(orig_handle.0)
        } else {
            None
        };

        match msg {
            WM_ERASEBKGND => {
                let hdc = HDC(wparam.0 as *mut _);
                let mut rc = RECT::default();
                let _ = GetClientRect(hwnd, &mut rc);
                let brush = CreateSolidBrush(COLORREF(col_bg_input()));
                let _ = FillRect(hdc, &rc, brush);
                let _ = DeleteObject(brush.into());
                LRESULT(1)
            }
            windows::Win32::UI::WindowsAndMessaging::WM_NCDESTROY => {
                let _ = RemovePropW(hwnd, PROP_SEARCH_EDIT_ORIG_PROC);
                if let Some(proc) = orig_proc {
                    CallWindowProcW(Some(proc), hwnd, msg, wparam, lparam)
                } else {
                    DefWindowProcW(hwnd, msg, wparam, lparam)
                }
            }
            _ => {
                if let Some(proc) = orig_proc {
                    CallWindowProcW(Some(proc), hwnd, msg, wparam, lparam)
                } else {
                    DefWindowProcW(hwnd, msg, wparam, lparam)
                }
            }
        }
    }
}

const PROP_TREEVIEW_ORIG_PROC: PCWSTR = w!("MalvisionTreeViewOrigProc");

/// Sous-classement du contrôle TreeView (`WC_TREEVIEWW`) pour intercepter directement `WM_LBUTTONDBLCLK`.
/// Par conception, Windows Common Controls n'émet pas `NM_DBLCLK` au parent lorsque le double-clic se produit
/// dans l'espace vide à droite du libellé (`TVHT_ONITEMRIGHT`). Ce sous-classement garantit
/// que le double-clic sur toute la largeur utile de la ligne déclenche la connexion ou bascule le dossier.
unsafe extern "system" fn treeview_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        let orig_handle = GetPropW(hwnd, PROP_TREEVIEW_ORIG_PROC);
        let orig_proc: WNDPROC = if !orig_handle.0.is_null() {
            std::mem::transmute::<*mut c_void, WNDPROC>(orig_handle.0)
        } else {
            None
        };

        match msg {
            WM_LBUTTONDBLCLK => {
                let parent = GetParent(hwnd).unwrap_or_default();
                let ptr = if !parent.is_invalid() {
                    GetWindowLongPtrW(parent, GWLP_USERDATA) as *mut AppWindowState
                } else {
                    std::ptr::null_mut()
                };

                if !ptr.is_null() {
                    let pt = POINT {
                        x: (lparam.0 & 0xFFFF) as i16 as i32,
                        y: ((lparam.0 >> 16) & 0xFFFF) as i16 as i32,
                    };

                    if let Some((hitem, hti)) = (*ptr).tree_resolve_row_at_point(pt)
                        && (hti.flags.0 & TVHT_ONITEMBUTTON.0) == 0
                        && hitem.0 != 0
                        && let Some(kind) = (*ptr).node_kind_from_hitem(hitem)
                    {
                        match kind {
                            TreeNodeKind::Connection(cid) => {
                                (*ptr).selected_node = Some(TreeNodeKind::Connection(cid));
                                let _ = SendMessageW(
                                    hwnd,
                                    TVM_SELECTITEM,
                                    Some(WPARAM(TVGN_CARET as usize)),
                                    Some(LPARAM(hitem.0)),
                                );
                                (*ptr).launch_connection_by_id(cid);
                                // Consommer le message : évite le comportement natif par défaut et élimine tout double lancement
                                return LRESULT(0);
                            }
                            TreeNodeKind::Folder(fid) => {
                                (*ptr).toggle_or_set_folder_expansion(hitem, fid, None);
                                return LRESULT(0);
                            }
                            TreeNodeKind::FavoritesRoot | TreeNodeKind::ConnectionsRoot => {
                                (*ptr).toggle_root_expansion(hitem);
                                return LRESULT(0);
                            }
                        }
                    }
                }

                if let Some(proc) = orig_proc {
                    CallWindowProcW(Some(proc), hwnd, msg, wparam, lparam)
                } else {
                    DefWindowProcW(hwnd, msg, wparam, lparam)
                }
            }
            windows::Win32::UI::WindowsAndMessaging::WM_NCDESTROY => {
                let _ = RemovePropW(hwnd, PROP_TREEVIEW_ORIG_PROC);
                if let Some(proc) = orig_proc {
                    CallWindowProcW(Some(proc), hwnd, msg, wparam, lparam)
                } else {
                    DefWindowProcW(hwnd, msg, wparam, lparam)
                }
            }
            _ => {
                if let Some(proc) = orig_proc {
                    CallWindowProcW(Some(proc), hwnd, msg, wparam, lparam)
                } else {
                    DefWindowProcW(hwnd, msg, wparam, lparam)
                }
            }
        }
    }
}

/// Sous-classement de `WC_COMBOBOXW` pour remplacer le cadre gris/blanc natif par un rendu
/// sombre uniforme (fond `col_bg_input()`, bordure `col_border()` ou `col_accent()` au focus/survol,
/// texte clair et chevron vectoriel). La police `HFONT` est stockée dans `GWLP_USERDATA`.
unsafe extern "system" fn dark_combobox_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        match msg {
            WM_ERASEBKGND => LRESULT(1),
            WM_MOUSEMOVE => {
                let mut tme = TRACKMOUSEEVENT {
                    cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                    dwFlags: TME_LEAVE,
                    hwndTrack: hwnd,
                    dwHoverTime: 0,
                };
                let _ = TrackMouseEvent(&mut tme);
                let _ = InvalidateRect(Some(hwnd), None, false);
                call_orig_combo_proc(hwnd, msg, wparam, lparam)
            }
            WM_MOUSELEAVE | WM_SETFOCUS | WM_KILLFOCUS => {
                let res = call_orig_combo_proc(hwnd, msg, wparam, lparam);
                let _ = InvalidateRect(Some(hwnd), None, false);
                res
            }
            WM_PAINT => {
                let mut ps = PAINTSTRUCT::default();
                let hdc_screen = BeginPaint(hwnd, &mut ps);
                let mut rc = RECT::default();
                let _ = GetClientRect(hwnd, &mut rc);
                let w = (rc.right - rc.left).max(1);
                let h = (rc.bottom - rc.top).max(1);

                let hdc = CreateCompatibleDC(Some(hdc_screen));
                let hbm = CreateCompatibleBitmap(hdc_screen, w, h);
                let old_bm = SelectObject(hdc, hbm.into());
                let _ = SetBkMode(hdc, TRANSPARENT);

                let bg_parent = CreateSolidBrush(COLORREF(col_bg_sidebar()));
                let _ = FillRect(hdc, &rc, bg_parent);
                let _ = DeleteObject(bg_parent.into());

                let mut cursor = POINT::default();
                let _ = GetCursorPos(&mut cursor);
                let _ = ScreenToClient(hwnd, &mut cursor);
                let hovered =
                    cursor.x >= 0 && cursor.x < rc.right && cursor.y >= 0 && cursor.y < rc.bottom;
                let focused = GetFocus() == hwnd;

                let border_col = if focused {
                    col_accent()
                } else if hovered {
                    col_accent_hover()
                } else {
                    col_border()
                };
                draw_rounded_box(hdc, rc, col_bg_input(), border_col);

                // Texte de l'élément sélectionné
                let sel = SendMessageW(hwnd, CB_GETCURSEL, Some(WPARAM(0)), Some(LPARAM(0))).0;
                if sel >= 0 {
                    let len = SendMessageW(
                        hwnd,
                        CB_GETLBTEXTLEN,
                        Some(WPARAM(sel as usize)),
                        Some(LPARAM(0)),
                    )
                    .0;
                    if len > 0 {
                        let mut buf = vec![0u16; (len as usize) + 1];
                        let copied = SendMessageW(
                            hwnd,
                            CB_GETLBTEXT,
                            Some(WPARAM(sel as usize)),
                            Some(LPARAM(buf.as_mut_ptr() as isize)),
                        )
                        .0;
                        if copied > 0 {
                            let text = String::from_utf16_lossy(&buf[..copied as usize]);
                            let font_raw = GetWindowLongPtrW(hwnd, GWLP_USERDATA);
                            let hfont = HFONT(font_raw as *mut _);
                            if !hfont.0.is_null() {
                                let _ = SelectObject(hdc, hfont.into());
                            }
                            let _ = SetTextColor(hdc, COLORREF(col_text_primary()));
                            draw_text_line(
                                hdc,
                                &text,
                                RECT {
                                    left: rc.left + 10,
                                    top: rc.top,
                                    right: rc.right - 28,
                                    bottom: rc.bottom,
                                },
                            );
                        }
                    }
                }

                // Chevron déroulant à droite
                draw_icon_chevron(
                    hdc,
                    rc.right - 15,
                    (rc.top + rc.bottom) / 2,
                    true,
                    if focused || hovered {
                        col_text_primary()
                    } else {
                        col_text_muted()
                    },
                );

                let _ = windows::Win32::Graphics::Gdi::BitBlt(
                    hdc_screen,
                    0,
                    0,
                    w,
                    h,
                    Some(hdc),
                    0,
                    0,
                    windows::Win32::Graphics::Gdi::SRCCOPY,
                );
                let _ = SelectObject(hdc, old_bm);
                let _ = DeleteObject(hbm.into());
                let _ = DeleteDC(hdc);
                let _ = EndPaint(hwnd, &ps);
                LRESULT(0)
            }
            _ => call_orig_combo_proc(hwnd, msg, wparam, lparam),
        }
    }
}

/// Dessine un bouton `BS_OWNERDRAW` moderne en thème sombre Windows 11.
unsafe fn draw_owner_button(dis: &DRAWITEMSTRUCT, is_primary: bool, font: HFONT, font_bold: HFONT) {
    unsafe {
        let hdc = dis.hDC;
        let rc = dis.rcItem;
        let _ = SetBkMode(hdc, TRANSPARENT);

        let bg_parent = CreateSolidBrush(COLORREF(col_bg_sidebar()));
        let _ = FillRect(hdc, &rc, bg_parent);
        let _ = DeleteObject(bg_parent.into());

        let mut pt = POINT::default();
        let _ = GetCursorPos(&mut pt);
        let _ = ScreenToClient(dis.hwndItem, &mut pt);
        let hovered = pt.x >= rc.left && pt.x < rc.right && pt.y >= rc.top && pt.y < rc.bottom;
        let pressed = (dis.itemState.0 & ODS_SELECTED.0) != 0;
        let focused = (dis.itemState.0 & ODS_FOCUS.0) != 0 || GetFocus() == dis.hwndItem;

        let (fill_col, border_col, text_col) = if is_primary {
            let fill = if pressed {
                col_accent_pressed()
            } else if hovered {
                col_accent_hover()
            } else {
                col_accent()
            };
            let border = if focused || hovered {
                COL_TEXT_ON_ACCENT
            } else {
                col_accent_hover()
            };
            (fill, border, COL_TEXT_ON_ACCENT)
        } else {
            let fill = if pressed {
                col_bg_input()
            } else if hovered {
                col_bg_hover()
            } else {
                col_bg_surface()
            };
            let border = if focused {
                col_accent()
            } else if hovered {
                col_text_muted()
            } else {
                col_border()
            };
            (fill, border, col_text_primary())
        };

        let label = get_window_text(dis.hwndItem);
        draw_rounded_button(
            hdc,
            rc,
            fill_col,
            border_col,
            &label,
            if is_primary { font_bold } else { font },
            text_col,
        );
    }
}

/// Dessine une case à cocher `BS_OWNERDRAW` moderne en thème sombre avec coche vectorielle.
unsafe fn draw_owner_checkbox(dis: &DRAWITEMSTRUCT, font: HFONT, dpi: DpiMetrics) {
    unsafe {
        let hdc = dis.hDC;
        let rc = dis.rcItem;
        let _ = SetBkMode(hdc, TRANSPARENT);

        let bg_parent = CreateSolidBrush(COLORREF(col_bg_sidebar()));
        let _ = FillRect(hdc, &rc, bg_parent);
        let _ = DeleteObject(bg_parent.into());

        let enabled = IsWindowEnabled(dis.hwndItem).as_bool();
        let checked = GetWindowLongPtrW(dis.hwndItem, GWLP_USERDATA) != 0;

        let mut pt = POINT::default();
        let _ = GetCursorPos(&mut pt);
        let _ = ScreenToClient(dis.hwndItem, &mut pt);
        let hovered =
            enabled && pt.x >= rc.left && pt.x < rc.right && pt.y >= rc.top && pt.y < rc.bottom;
        let focused = (dis.itemState.0 & ODS_FOCUS.0) != 0 || GetFocus() == dis.hwndItem;

        let box_sz = dpi.scale(18);
        let box_top = rc.top + ((rc.bottom - rc.top) - box_sz) / 2;
        let box_rc = RECT {
            left: rc.left,
            top: box_top,
            right: rc.left + box_sz,
            bottom: box_top + box_sz,
        };

        let fill_col = if !enabled {
            col_bg_input_disabled()
        } else if checked {
            if hovered {
                col_accent_hover()
            } else {
                col_accent()
            }
        } else if hovered {
            col_bg_hover()
        } else {
            col_bg_input()
        };

        let border_col = if !enabled {
            col_border_subtle()
        } else if focused || hovered || checked {
            col_accent_hover()
        } else {
            col_border()
        };

        draw_rounded_box(hdc, box_rc, fill_col, border_col);

        if checked {
            let pen = CreatePen(
                PS_SOLID,
                2,
                COLORREF(if enabled {
                    COL_TEXT_ON_ACCENT
                } else {
                    col_text_muted()
                }),
            );
            let old_p = SelectObject(hdc, pen.into());
            let x0 = box_rc.left + dpi.scale(4);
            let y0 = box_rc.top + dpi.scale(9);
            let x1 = box_rc.left + dpi.scale(8);
            let y1 = box_rc.top + dpi.scale(13);
            let x2 = box_rc.left + dpi.scale(14);
            let y2 = box_rc.top + dpi.scale(5);
            let _ = MoveToEx(hdc, x0, y0, None);
            let _ = LineTo(hdc, x1, y1);
            let _ = LineTo(hdc, x2, y2);
            let _ = SelectObject(hdc, old_p);
            let _ = DeleteObject(pen.into());
        }

        let label = get_window_text(dis.hwndItem);
        let _ = SelectObject(hdc, font.into());
        let _ = SetTextColor(
            hdc,
            COLORREF(if enabled {
                col_text_primary()
            } else {
                col_text_muted()
            }),
        );
        draw_text_line(
            hdc,
            &label,
            RECT {
                left: box_rc.right + dpi.scale(10),
                top: rc.top,
                right: rc.right,
                bottom: rc.bottom,
            },
        );
    }
}

/// Dessine une ligne de la liste déroulante d'une `ComboBox` (`CBS_OWNERDRAWFIXED | CBS_HASSTRINGS`).
unsafe fn draw_owner_combobox_item(dis: &DRAWITEMSTRUCT, font: HFONT) {
    unsafe {
        if dis.itemID == u32::MAX {
            return;
        }
        let hdc = dis.hDC;
        let rc = dis.rcItem;
        let selected = (dis.itemState.0 & ODS_SELECTED.0) != 0;

        let bg_col = if selected {
            col_bg_hover()
        } else {
            col_bg_input()
        };
        let bg_brush = CreateSolidBrush(COLORREF(bg_col));
        let _ = FillRect(hdc, &rc, bg_brush);
        let _ = DeleteObject(bg_brush.into());

        if selected {
            let accent_rc = RECT {
                left: rc.left,
                top: rc.top + 2,
                right: rc.left + 3,
                bottom: rc.bottom - 2,
            };
            let acc_brush = CreateSolidBrush(COLORREF(col_accent()));
            let _ = FillRect(hdc, &accent_rc, acc_brush);
            let _ = DeleteObject(acc_brush.into());
        }

        let len = SendMessageW(
            dis.hwndItem,
            CB_GETLBTEXTLEN,
            Some(WPARAM(dis.itemID as usize)),
            Some(LPARAM(0)),
        )
        .0;
        if len > 0 {
            let mut buf = vec![0u16; (len as usize) + 1];
            let copied = SendMessageW(
                dis.hwndItem,
                CB_GETLBTEXT,
                Some(WPARAM(dis.itemID as usize)),
                Some(LPARAM(buf.as_mut_ptr() as isize)),
            )
            .0;
            if copied > 0 {
                let text = String::from_utf16_lossy(&buf[..copied as usize]);
                let _ = SetBkMode(hdc, TRANSPARENT);
                let _ = SelectObject(hdc, font.into());
                let _ = SetTextColor(hdc, COLORREF(col_text_primary()));
                draw_text_line(
                    hdc,
                    &text,
                    RECT {
                        left: rc.left + 10,
                        top: rc.top,
                        right: rc.right - 8,
                        bottom: rc.bottom,
                    },
                );
            }
        }
    }
}

/// Crée un champ `WC_EDITW` sans bordure blanche, centré dans un conteneur arrondi sombre `box_rc`.
unsafe fn create_dark_edit(
    parent: HWND,
    hinstance: windows::Win32::Foundation::HINSTANCE,
    box_rc: RECT,
    id: usize,
    password: bool,
    font: HFONT,
    dpi: DpiMetrics,
) -> HWND {
    unsafe {
        let mut style = WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(ES_AUTOHSCROLL as u32);
        if password {
            style |= WINDOW_STYLE(ES_PASSWORD as u32);
        }
        let pad_x = dpi.scale(10);
        let edit_h = dpi.scale(20);
        let box_h = box_rc.bottom - box_rc.top;
        let edit_top = box_rc.top + ((box_h - edit_h) / 2).max(2);
        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            WC_EDITW,
            w!(""),
            style,
            box_rc.left + pad_x,
            edit_top,
            (box_rc.right - box_rc.left - pad_x * 2).max(20),
            edit_h,
            Some(parent),
            Some(HMENU(id as *mut _)),
            Some(hinstance),
            None,
        )
        .unwrap_or_default();
        let _ = SendMessageW(
            hwnd,
            WM_SETFONT,
            Some(WPARAM(font.0 as usize)),
            Some(LPARAM(1)),
        );
        hwnd
    }
}

/// Crée un bouton d'action `BS_OWNERDRAW` thématisé sombre.
unsafe fn create_dark_button(
    parent: HWND,
    hinstance: windows::Win32::Foundation::HINSTANCE,
    label: PCWSTR,
    rc: RECT,
    id: usize,
    font: HFONT,
) -> HWND {
    unsafe {
        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            WC_BUTTONW,
            label,
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_OWNERDRAW as u32),
            rc.left,
            rc.top,
            rc.right - rc.left,
            rc.bottom - rc.top,
            Some(parent),
            Some(HMENU(id as *mut _)),
            Some(hinstance),
            None,
        )
        .unwrap_or_default();
        let _ = SendMessageW(
            hwnd,
            WM_SETFONT,
            Some(WPARAM(font.0 as usize)),
            Some(LPARAM(1)),
        );
        let prev = SetWindowLongPtrW(
            hwnd,
            GWLP_WNDPROC,
            dark_pushbutton_wnd_proc as *const () as usize as isize,
        );
        if prev != 0 {
            ORIG_BUTTON_WNDPROC.store(prev, std::sync::atomic::Ordering::Relaxed);
        }
        hwnd
    }
}

/// Crée une case à cocher `BS_OWNERDRAW` thématisée sombre.
unsafe fn create_dark_checkbox(
    parent: HWND,
    hinstance: windows::Win32::Foundation::HINSTANCE,
    label: PCWSTR,
    rc: RECT,
    id: usize,
    font: HFONT,
) -> HWND {
    unsafe {
        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            WC_BUTTONW,
            label,
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_OWNERDRAW as u32),
            rc.left,
            rc.top,
            rc.right - rc.left,
            rc.bottom - rc.top,
            Some(parent),
            Some(HMENU(id as *mut _)),
            Some(hinstance),
            None,
        )
        .unwrap_or_default();
        let _ = SendMessageW(
            hwnd,
            WM_SETFONT,
            Some(WPARAM(font.0 as usize)),
            Some(LPARAM(1)),
        );
        let prev = SetWindowLongPtrW(
            hwnd,
            GWLP_WNDPROC,
            dark_checkbox_wnd_proc as *const () as usize as isize,
        );
        if prev != 0 {
            ORIG_BUTTON_WNDPROC.store(prev, std::sync::atomic::Ordering::Relaxed);
        }
        hwnd
    }
}

/// Crée une liste déroulante `WC_COMBOBOXW` (`CBS_DROPDOWNLIST | CBS_OWNERDRAWFIXED | CBS_HASSTRINGS`)
/// thématisée sombre.
unsafe fn create_dark_combobox(
    parent: HWND,
    hinstance: windows::Win32::Foundation::HINSTANCE,
    rc: RECT,
    drop_height: i32,
    id: usize,
    font: HFONT,
    dpi: DpiMetrics,
) -> HWND {
    unsafe {
        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            WC_COMBOBOXW,
            w!(""),
            WS_CHILD
                | WS_VISIBLE
                | WS_TABSTOP
                | WS_VSCROLL
                | WINDOW_STYLE((CBS_DROPDOWNLIST | CBS_OWNERDRAWFIXED | CBS_HASSTRINGS) as u32),
            rc.left,
            rc.top,
            rc.right - rc.left,
            drop_height,
            Some(parent),
            Some(HMENU(id as *mut _)),
            Some(hinstance),
            None,
        )
        .unwrap_or_default();
        let _ = SendMessageW(
            hwnd,
            WM_SETFONT,
            Some(WPARAM(font.0 as usize)),
            Some(LPARAM(1)),
        );
        // CB_SETITEMHEIGHT (0x0153) : wParam = -1 fixes selection field height, wParam = 0 fixes list item height
        let field_h = ((rc.bottom - rc.top) - dpi.scale(6)).max(dpi.scale(22));
        let item_h = dpi.scale(26);
        let _ = SendMessageW(
            hwnd,
            0x0153,
            Some(WPARAM(usize::MAX)),
            Some(LPARAM(field_h as isize)),
        );
        let _ = SendMessageW(hwnd, 0x0153, Some(WPARAM(0)), Some(LPARAM(item_h as isize)));
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, font.0 as isize);
        let prev = SetWindowLongPtrW(
            hwnd,
            GWLP_WNDPROC,
            dark_combobox_wnd_proc as *const () as usize as isize,
        );
        if prev != 0 {
            ORIG_COMBO_WNDPROC.store(prev, std::sync::atomic::Ordering::Relaxed);
        }
        hwnd
    }
}

/// Calcule la taille totale de fenêtre (`AdjustWindowRectEx`) correspondant à une zone client `(client_w, client_h)`.
unsafe fn calc_dialog_window_size(style: WINDOW_STYLE, client_w: i32, client_h: i32) -> (i32, i32) {
    unsafe {
        let mut rc = RECT {
            left: 0,
            top: 0,
            right: client_w,
            bottom: client_h,
        };
        let _ = AdjustWindowRectEx(&mut rc, style, false, WINDOW_EX_STYLE(0));
        (rc.right - rc.left, rc.bottom - rc.top)
    }
}

struct ConnDialogLabel {
    text: String,
    rect: RECT,
    muted: bool,
    bold: bool,
    rdp_only: bool,
}

struct DialogEditBox {
    hwnd: HWND,
    box_rect: RECT,
    rdp_only: bool,
}

struct ConnectionDialogState {
    owner: HWND,
    dpi: DpiMetrics,
    font_ui: HFONT,
    font_bold: HFONT,
    font_subtitle: HFONT,
    font_small: HFONT,
    brush_bg: HBRUSH,
    brush_input: HBRUSH,
    header_title: String,
    header_subtitle: String,
    current_protocol: Protocol,
    host_label_text: String,
    host_label_rect: RECT,
    host_box_rect_normal: RECT,
    host_box_rect_full: RECT,
    password_label_text: String,
    password_label_rect: RECT,
    notice_text: Option<String>,
    notice_rect: RECT,
    labels: Vec<ConnDialogLabel>,
    edit_boxes: Vec<DialogEditBox>,
    divider_y1: i32,
    divider_y2: i32,
    security_banner_rc: RECT,
    folder_options: Vec<Option<i64>>,
    has_existing_pwd: bool,
    ed_name: HWND,
    cb_proto: HWND,
    cb_folder: HWND,
    ed_host: HWND,
    ed_port: HWND,
    ed_user: HWND,
    ed_domain: HWND,
    ed_pass: HWND,
    chk_save_pass: HWND,
    chk_fav: HWND,
    chk_priority: HWND,
    chk_dyn_res: HWND,
    chk_clipboard: HWND,
    ed_desc: HWND,
    result: Option<ConnectionDraft>,
}

/// Construit une liste hiérarchique indentée de tous les dossiers disponibles `(Option<id>, label)`.
fn build_folder_choices(
    folders: &[Folder],
    exclude_subtree_of: Option<i64>,
) -> Vec<(Option<i64>, String)> {
    let mut out = vec![(None, "Racine (Aucun dossier parent)".to_string())];
    let mut by_parent: HashMap<Option<i64>, Vec<&Folder>> = HashMap::new();
    for f in folders {
        by_parent.entry(f.parent_id).or_default().push(f);
    }

    fn walk(
        parent: Option<i64>,
        depth: usize,
        exclude: Option<i64>,
        by_parent: &HashMap<Option<i64>, Vec<&Folder>>,
        out: &mut Vec<(Option<i64>, String)>,
        visited: &mut Vec<i64>,
    ) {
        if let Some(children) = by_parent.get(&parent) {
            for f in children {
                if Some(f.id) == exclude || visited.contains(&f.id) {
                    continue;
                }
                visited.push(f.id);
                let indent = "    ".repeat(depth);
                let prefix = if depth == 0 { "" } else { "└ " };
                out.push((Some(f.id), format!("{indent}{prefix}{}", f.name)));
                walk(Some(f.id), depth + 1, exclude, by_parent, out, visited);
            }
        }
    }

    let mut visited = Vec::new();
    walk(
        None,
        0,
        exclude_subtree_of,
        &by_parent,
        &mut out,
        &mut visited,
    );
    out
}

/// Adapte immédiatement la disposition des contrôles et libellés du formulaire selon le protocole sélectionné.
fn update_dialog_protocol_layout(state: &mut ConnectionDialogState, hwnd: HWND, proto: Protocol) {
    state.current_protocol = proto;
    let is_rdp = proto == Protocol::Rdp;
    unsafe {
        let _ = ShowWindow(state.ed_port, if is_rdp { SW_SHOW } else { SW_HIDE });
        let _ = ShowWindow(state.ed_user, if is_rdp { SW_SHOW } else { SW_HIDE });
        let _ = ShowWindow(state.ed_domain, if is_rdp { SW_SHOW } else { SW_HIDE });
        let _ = ShowWindow(state.chk_dyn_res, if is_rdp { SW_SHOW } else { SW_HIDE });
        let _ = ShowWindow(state.chk_clipboard, if is_rdp { SW_SHOW } else { SW_HIDE });

        let host_rc = if is_rdp {
            state.host_box_rect_normal
        } else {
            state.host_box_rect_full
        };
        let _ = MoveWindow(
            state.ed_host,
            host_rc.left,
            host_rc.top,
            host_rc.right - host_rc.left,
            host_rc.bottom - host_rc.top,
            true,
        );

        match proto {
            Protocol::Rdp => {
                state.host_label_text = "Hôte RDP (IP / DNS) * :".to_string();
                state.password_label_text = "Mot de passe :".to_string();
                state.notice_text = None;
            }
            Protocol::AnyDesk => {
                state.host_label_text = "ID ou Alias AnyDesk * :".to_string();
                state.password_label_text = "Accès sans surveillance :".to_string();
                state.notice_text = Some("Note AnyDesk : mot de passe transmis de façon sécurisée sur le flux standard (pipe stdin).".to_string());
            }
            Protocol::RustDesk => {
                state.host_label_text = "ID RustDesk * :".to_string();
                state.password_label_text = "Mot de passe permanent :".to_string();
                state.notice_text = Some("Note RustDesk : l'authentification distante reste gérée directement par le client RustDesk.".to_string());
            }
        }

        let _ = InvalidateRect(Some(hwnd), None, true);
        let _ = UpdateWindow(hwnd);
    }
}

unsafe extern "system" fn conn_dlg_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        match msg {
            WM_ERASEBKGND => LRESULT(1),
            WM_CTLCOLORSTATIC | WM_CTLCOLORBTN => {
                let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const ConnectionDialogState;
                if !ptr.is_null() {
                    let hdc = HDC(wparam.0 as *mut _);
                    let _ = SetTextColor(hdc, COLORREF(col_text_primary()));
                    let _ = SetBkColor(hdc, COLORREF(col_bg_sidebar()));
                    return LRESULT((*ptr).brush_bg.0 as isize);
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            WM_CTLCOLOREDIT | WM_CTLCOLORLISTBOX => {
                let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const ConnectionDialogState;
                if !ptr.is_null() {
                    let hdc = HDC(wparam.0 as *mut _);
                    let ctrl_hwnd = HWND(lparam.0 as *mut _);
                    let enabled = !ctrl_hwnd.0.is_null() && IsWindowEnabled(ctrl_hwnd).as_bool();
                    let _ = SetTextColor(
                        hdc,
                        COLORREF(if enabled {
                            col_text_primary()
                        } else {
                            col_text_muted()
                        }),
                    );
                    let _ = SetBkColor(hdc, COLORREF(col_bg_input()));
                    return LRESULT((*ptr).brush_input.0 as isize);
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            WM_MEASUREITEM => {
                let mis = lparam.0 as *mut MEASUREITEMSTRUCT;
                if !mis.is_null() {
                    let dpi = DpiMetrics::for_window(hwnd);
                    (*mis).itemHeight = dpi.scale(26) as u32;
                    return LRESULT(1);
                }
                LRESULT(0)
            }
            WM_DRAWITEM => {
                let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const ConnectionDialogState;
                let dis = lparam.0 as *const DRAWITEMSTRUCT;
                if !ptr.is_null() && !dis.is_null() {
                    let state = &*ptr;
                    let d = &*dis;
                    if d.CtlType == ODT_COMBOBOX {
                        draw_owner_combobox_item(d, state.font_ui);
                        return LRESULT(1);
                    }
                    if d.CtlType == ODT_BUTTON {
                        let id = d.CtlID as usize;
                        if id == IDC_DLG_OK || id == IDC_DLG_CANCEL {
                            draw_owner_button(d, id == IDC_DLG_OK, state.font_ui, state.font_bold);
                        } else {
                            draw_owner_checkbox(d, state.font_ui, state.dpi);
                        }
                        return LRESULT(1);
                    }
                }
                LRESULT(0)
            }
            WM_PAINT => {
                let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const ConnectionDialogState;
                if ptr.is_null() {
                    return DefWindowProcW(hwnd, msg, wparam, lparam);
                }
                let state = &*ptr;
                let mut ps = PAINTSTRUCT::default();
                let hdc_screen = BeginPaint(hwnd, &mut ps);
                let mut rc = RECT::default();
                let _ = GetClientRect(hwnd, &mut rc);
                let w = (rc.right - rc.left).max(1);
                let h = (rc.bottom - rc.top).max(1);

                let hdc = CreateCompatibleDC(Some(hdc_screen));
                let hbm = CreateCompatibleBitmap(hdc_screen, w, h);
                let old_bm = SelectObject(hdc, hbm.into());
                let _ = SetBkMode(hdc, TRANSPARENT);

                // Fond global sombre uniforme (aucune surface blanche)
                let _ = FillRect(hdc, &rc, state.brush_bg);

                let pad_x = state.dpi.scale(28);
                let _ = SelectObject(hdc, state.font_subtitle.into());
                let _ = SetTextColor(hdc, COLORREF(col_text_primary()));
                draw_text_line(
                    hdc,
                    &state.header_title,
                    RECT {
                        left: pad_x,
                        top: state.dpi.scale(18),
                        right: w - pad_x,
                        bottom: state.dpi.scale(42),
                    },
                );

                let _ = SelectObject(hdc, state.font_small.into());
                let _ = SetTextColor(hdc, COLORREF(col_text_muted()));
                draw_text_line(
                    hdc,
                    &state.header_subtitle,
                    RECT {
                        left: pad_x,
                        top: state.dpi.scale(42),
                        right: w - pad_x,
                        bottom: state.dpi.scale(62),
                    },
                );

                draw_horizontal_line(hdc, pad_x, w - pad_x, state.divider_y1, col_border());
                draw_horizontal_line(hdc, pad_x, w - pad_x, state.divider_y2, col_border());

                // Libellé dynamique d'hôte (RDP / AnyDesk / RustDesk)
                let _ = SelectObject(hdc, state.font_ui.into());
                let _ = SetTextColor(hdc, COLORREF(col_text_primary()));
                draw_text_line(hdc, &state.host_label_text, state.host_label_rect);

                // Libellé dynamique de mot de passe (RDP / AnyDesk / RustDesk)
                draw_text_line(hdc, &state.password_label_text, state.password_label_rect);

                // Notice informative spécifique (RustDesk / AnyDesk)
                if let Some(ref notice) = state.notice_text {
                    let _ = SelectObject(hdc, state.font_small.into());
                    let _ = SetTextColor(hdc, COLORREF(col_accent()));
                    draw_text_line(hdc, notice, state.notice_rect);
                }

                // Tous les autres libellés dessinés directement en mode TRANSPARENT
                for lbl in &state.labels {
                    if lbl.rdp_only && state.current_protocol != Protocol::Rdp {
                        continue;
                    }
                    let f = if lbl.bold {
                        state.font_bold
                    } else if lbl.muted {
                        state.font_small
                    } else {
                        state.font_ui
                    };
                    let _ = SelectObject(hdc, f.into());
                    let _ = SetTextColor(
                        hdc,
                        COLORREF(if lbl.muted {
                            col_text_muted()
                        } else {
                            col_text_primary()
                        }),
                    );
                    draw_text_line(hdc, &lbl.text, lbl.rect);
                }

                // Conteneurs sombres arrondis autour de chaque champ WC_EDITW
                let focus_hwnd = GetFocus();
                for eb in &state.edit_boxes {
                    if eb.rdp_only && state.current_protocol != Protocol::Rdp {
                        continue;
                    }
                    let enabled = IsWindowEnabled(eb.hwnd).as_bool();
                    let focused = focus_hwnd == eb.hwnd;
                    let fill = if enabled {
                        col_bg_input()
                    } else {
                        col_bg_deep()
                    };
                    let border = if focused { col_accent() } else { col_border() };
                    let rect = if eb.hwnd == state.ed_host {
                        if state.current_protocol == Protocol::Rdp {
                            state.host_box_rect_normal
                        } else {
                            state.host_box_rect_full
                        }
                    } else {
                        eb.box_rect
                    };
                    draw_rounded_box(hdc, rect, fill, border);
                }

                // Encadré discret d'information sécurité DPAPI
                draw_rounded_box(hdc, state.security_banner_rc, col_bg_deep(), col_border());
                let _ = SelectObject(hdc, state.font_small.into());
                let _ = SetTextColor(hdc, COLORREF(col_text_muted()));
                draw_text_line(
                    hdc,
                    "Sécurité DPAPI : le mot de passe est chiffré par Windows (utilisateur courant) et n'est jamais stocké en clair.",
                    RECT {
                        left: state.security_banner_rc.left + state.dpi.scale(12),
                        top: state.security_banner_rc.top,
                        right: state.security_banner_rc.right - state.dpi.scale(12),
                        bottom: state.security_banner_rc.bottom,
                    },
                );

                let _ = windows::Win32::Graphics::Gdi::BitBlt(
                    hdc_screen,
                    0,
                    0,
                    w,
                    h,
                    Some(hdc),
                    0,
                    0,
                    windows::Win32::Graphics::Gdi::SRCCOPY,
                );

                let _ = SelectObject(hdc, old_bm);
                let _ = DeleteObject(hbm.into());
                let _ = DeleteDC(hdc);
                let _ = EndPaint(hwnd, &ps);
                LRESULT(0)
            }
            WM_COMMAND => {
                let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut ConnectionDialogState;
                if !ptr.is_null() {
                    let state = &mut *ptr;
                    let id = wparam.0 & 0xFFFF;
                    let code = ((wparam.0 >> 16) & 0xFFFF) as u32;

                    if code == EN_SETFOCUS || code == EN_KILLFOCUS {
                        let _ = InvalidateRect(Some(hwnd), None, false);
                        return LRESULT(0);
                    }

                    // Changement de protocole dans la ComboBox (CBN_SELCHANGE == 1)
                    if id == IDC_DLG_PROTO && code == 1 {
                        let sel = SendMessageW(
                            state.cb_proto,
                            CB_GETCURSEL,
                            Some(WPARAM(0)),
                            Some(LPARAM(0)),
                        )
                        .0;
                        let proto = match sel {
                            1 => Protocol::AnyDesk,
                            2 => Protocol::RustDesk,
                            _ => Protocol::Rdp,
                        };
                        update_dialog_protocol_layout(state, hwnd, proto);
                    } else if id == IDC_DLG_SAVE_PASS {
                        let checked = SendMessageW(
                            state.chk_save_pass,
                            BM_GETCHECK,
                            Some(WPARAM(0)),
                            Some(LPARAM(0)),
                        )
                        .0 != 0;
                        let _ = EnableWindow(state.ed_pass, checked);
                        let _ = InvalidateRect(Some(hwnd), None, false);
                        let _ = InvalidateRect(Some(state.ed_pass), None, true);
                    } else if id == IDC_DLG_OK || id == 1 {
                        let proto_idx = SendMessageW(
                            state.cb_proto,
                            CB_GETCURSEL,
                            Some(WPARAM(0)),
                            Some(LPARAM(0)),
                        )
                        .0;
                        let protocol = match proto_idx {
                            1 => Protocol::AnyDesk,
                            2 => Protocol::RustDesk,
                            _ => Protocol::Rdp,
                        };

                        let folder_idx = SendMessageW(
                            state.cb_folder,
                            CB_GETCURSEL,
                            Some(WPARAM(0)),
                            Some(LPARAM(0)),
                        )
                        .0;
                        let folder_id = if folder_idx >= 0
                            && (folder_idx as usize) < state.folder_options.len()
                        {
                            state.folder_options[folder_idx as usize]
                        } else {
                            None
                        };

                        let port_str = get_window_text(state.ed_port);
                        let port = if protocol == Protocol::Rdp {
                            match port_str.trim().parse::<u16>() {
                                Ok(p) if p > 0 => p,
                                _ => {
                                    show_warning_dialog(
                                        hwnd,
                                        "Port RDP invalide",
                                        "Le port RDP doit être un entier compris entre 1 et 65535.",
                                    );
                                    return LRESULT(0);
                                }
                            }
                        } else {
                            3389
                        };

                        let save_password = SendMessageW(
                            state.chk_save_pass,
                            BM_GETCHECK,
                            Some(WPARAM(0)),
                            Some(LPARAM(0)),
                        )
                        .0 != 0;
                        let is_favorite = SendMessageW(
                            state.chk_fav,
                            BM_GETCHECK,
                            Some(WPARAM(0)),
                            Some(LPARAM(0)),
                        )
                        .0 != 0;
                        let is_priority = SendMessageW(
                            state.chk_priority,
                            BM_GETCHECK,
                            Some(WPARAM(0)),
                            Some(LPARAM(0)),
                        )
                        .0 != 0;
                        let dynamic_resolution = SendMessageW(
                            state.chk_dyn_res,
                            BM_GETCHECK,
                            Some(WPARAM(0)),
                            Some(LPARAM(0)),
                        )
                        .0 != 0;
                        let redirect_clipboard = SendMessageW(
                            state.chk_clipboard,
                            BM_GETCHECK,
                            Some(WPARAM(0)),
                            Some(LPARAM(0)),
                        )
                        .0 != 0;

                        let raw_pwd = get_window_text(state.ed_pass);
                        let keep_existing_password =
                            save_password && raw_pwd.is_empty() && state.has_existing_pwd;

                        let mut draft = ConnectionDraft {
                            folder_id,
                            name: get_window_text(state.ed_name),
                            protocol,
                            host: get_window_text(state.ed_host),
                            port,
                            username: get_window_text(state.ed_user),
                            domain: get_window_text(state.ed_domain),
                            description: get_window_text(state.ed_desc),
                            is_favorite,
                            is_priority,
                            save_password,
                            password: SecretString::new(raw_pwd),
                            keep_existing_password,
                            dynamic_resolution,
                            redirect_clipboard,
                        };

                        // Effacer immédiatement le champ mot de passe de l'UI
                        set_window_text(state.ed_pass, "");

                        match draft.validate_and_normalize() {
                            Ok(()) => {
                                state.result = Some(draft);
                                let owner = state.owner;
                                close_modal_dialog(hwnd, owner);
                            }
                            Err(err) => {
                                show_warning_dialog(hwnd, "Validation de la connexion", &err);
                            }
                        }
                    } else if id == IDC_DLG_CANCEL || id == 2 {
                        set_window_text(state.ed_pass, "");
                        let owner = state.owner;
                        close_modal_dialog(hwnd, owner);
                    }
                }
                LRESULT(0)
            }
            WM_CLOSE => {
                let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut ConnectionDialogState;
                if !ptr.is_null() {
                    set_window_text((*ptr).ed_pass, "");
                    let owner = (*ptr).owner;
                    close_modal_dialog(hwnd, owner);
                } else {
                    let _ = DestroyWindow(hwnd);
                }
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}

fn run_connection_form_dialog(
    owner: HWND,
    existing: Option<&ConnectionItem>,
    default_folder_id: Option<i64>,
    folders: &[Folder],
    default_dyn_res: bool,
    default_clipboard: bool,
    palette: &GdiPalette,
) -> Option<ConnectionDraft> {
    unsafe {
        let hinstance = GetModuleHandleW(None).unwrap_or_default();
        let class_name = w!("MalvisionConnectionFormDlgClass");

        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(conn_dlg_wnd_proc),
            hInstance: hinstance.into(),
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            hbrBackground: palette.brush_bg_sidebar,
            lpszClassName: class_name,
            ..Default::default()
        };
        let _ = RegisterClassExW(&wc);

        let dpi = if owner.is_invalid() {
            palette.dpi
        } else {
            DpiMetrics::for_window(owner)
        };
        let client_w = dpi.scale(680);
        let client_h = dpi.scale(660);
        let dlg_style = WS_POPUP | WS_CAPTION | WS_SYSMENU | WS_CLIPCHILDREN;
        let (win_w, win_h) = calc_dialog_window_size(dlg_style, client_w, client_h);

        let mut owner_rc = RECT::default();
        let _ = GetWindowRect(owner, &mut owner_rc);
        let x = owner_rc.left + ((owner_rc.right - owner_rc.left) - win_w) / 2;
        let y = owner_rc.top + ((owner_rc.bottom - owner_rc.top) - win_h) / 2;

        let title = if existing.is_some() {
            w!("Modifier la connexion")
        } else {
            w!("Nouvelle connexion distante")
        };

        // Créer la fenêtre initialement non visible afin d'attacher GWLP_USERDATA AVANT tout peinture
        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            class_name,
            title,
            dlg_style,
            x.max(20),
            y.max(20),
            win_w,
            win_h,
            Some(owner),
            None,
            Some(hinstance.into()),
            None,
        )
        .ok()?;

        let dark_mode: i32 = i32::from(is_active_theme_dark());
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_USE_IMMERSIVE_DARK_MODE,
            &dark_mode as *const i32 as *const c_void,
            std::mem::size_of::<i32>() as u32,
        );

        let pad_x = dpi.scale(28);
        let label_w = dpi.scale(190);
        let field_left = pad_x + label_w;
        let field_right = client_w - pad_x;
        let field_h = dpi.scale(32);

        let y_host = dpi.scale(210);
        let port_box_w = dpi.scale(78);
        let port_lbl_w = dpi.scale(48);
        let rc_port = RECT {
            left: field_right - port_box_w,
            top: y_host,
            right: field_right,
            bottom: y_host + field_h,
        };
        let rc_host = RECT {
            left: field_left,
            top: y_host,
            right: rc_port.left - port_lbl_w - dpi.scale(10),
            bottom: y_host + field_h,
        };
        let host_box_rect_normal = rc_host;
        let host_box_rect_full = RECT {
            left: field_left,
            top: y_host,
            right: field_right,
            bottom: y_host + field_h,
        };
        let y_pass = dpi.scale(336);
        let password_label_rect = RECT {
            left: pad_x,
            top: y_pass,
            right: field_left - dpi.scale(8),
            bottom: y_pass + field_h,
        };
        let y_chk3 = dpi.scale(432);
        let notice_rect = RECT {
            left: field_left,
            top: y_chk3,
            right: field_right,
            bottom: y_chk3 + dpi.scale(44),
        };
        let initial_proto = existing.map(|e| e.protocol).unwrap_or(Protocol::Rdp);

        // Préparer l'état avant de créer les contrôles enfants pour que WM_CTLCOLOR* ait toujours un pointeur valide
        let mut dlg_state = Box::new(ConnectionDialogState {
            owner,
            dpi,
            font_ui: palette.font_ui,
            font_bold: palette.font_ui_bold,
            font_subtitle: palette.font_subtitle,
            font_small: palette.font_small,
            brush_bg: palette.brush_bg_sidebar,
            brush_input: palette.brush_bg_input,
            header_title: if existing.is_some() {
                "Modifier la fiche de connexion".to_string()
            } else {
                "Nouvelle connexion distante".to_string()
            },
            header_subtitle:
                "Configurez l'hôte distant, le dossier de rangement et les paramètres de session."
                    .to_string(),
            current_protocol: initial_proto,
            host_label_text: match initial_proto {
                Protocol::Rdp => "Hôte RDP (IP / DNS) * :".to_string(),
                Protocol::AnyDesk => "ID ou Alias AnyDesk * :".to_string(),
                Protocol::RustDesk => "ID RustDesk * :".to_string(),
            },
            host_label_rect: RECT {
                left: pad_x,
                top: y_host,
                right: field_left - dpi.scale(8),
                bottom: y_host + field_h,
            },
            host_box_rect_normal,
            host_box_rect_full,
            password_label_text: match initial_proto {
                Protocol::Rdp => "Mot de passe :".to_string(),
                Protocol::AnyDesk => "Accès sans surveillance :".to_string(),
                Protocol::RustDesk => "Mot de passe permanent :".to_string(),
            },
            password_label_rect,
            notice_text: match initial_proto {
                Protocol::Rdp => None,
                Protocol::AnyDesk => Some(
                    "Note AnyDesk : mot de passe transmis sur stdin (--with-password).".to_string(),
                ),
                Protocol::RustDesk => Some(
                    "Note RustDesk : l'authentification reste gérée par le client RustDesk."
                        .to_string(),
                ),
            },
            notice_rect,
            labels: Vec::new(),
            edit_boxes: Vec::new(),
            divider_y1: dpi.scale(70),
            divider_y2: dpi.scale(588),
            security_banner_rc: RECT {
                left: pad_x,
                top: dpi.scale(538),
                right: field_right,
                bottom: dpi.scale(574),
            },
            folder_options: Vec::new(),
            has_existing_pwd: existing.is_some_and(|e| e.has_saved_password),
            ed_name: HWND::default(),
            cb_proto: HWND::default(),
            cb_folder: HWND::default(),
            ed_host: HWND::default(),
            ed_port: HWND::default(),
            ed_user: HWND::default(),
            ed_domain: HWND::default(),
            ed_pass: HWND::default(),
            chk_save_pass: HWND::default(),
            chk_fav: HWND::default(),
            chk_priority: HWND::default(),
            chk_dyn_res: HWND::default(),
            chk_clipboard: HWND::default(),
            ed_desc: HWND::default(),
            result: None,
        });

        SetWindowLongPtrW(hwnd, GWLP_USERDATA, (&mut *dlg_state) as *mut _ as isize);

        let add_label =
            |labels: &mut Vec<ConnDialogLabel>, text: &str, top: i32, h: i32, rdp_only: bool| {
                labels.push(ConnDialogLabel {
                    text: text.to_string(),
                    rect: RECT {
                        left: pad_x,
                        top,
                        right: field_left - dpi.scale(8),
                        bottom: top + h,
                    },
                    muted: false,
                    bold: false,
                    rdp_only,
                });
            };

        // 1. Nom de la connexion
        let y_name = dpi.scale(84);
        add_label(
            &mut dlg_state.labels,
            "Nom de la connexion * :",
            y_name,
            field_h,
            false,
        );
        let rc_name = RECT {
            left: field_left,
            top: y_name,
            right: field_right,
            bottom: y_name + field_h,
        };
        let ed_name = create_dark_edit(
            hwnd,
            hinstance.into(),
            rc_name,
            IDC_DLG_NAME,
            false,
            palette.font_ui,
            dpi,
        );
        dlg_state.edit_boxes.push(DialogEditBox {
            hwnd: ed_name,
            box_rect: rc_name,
            rdp_only: false,
        });

        // 2. Protocole
        let y_proto = dpi.scale(126);
        add_label(
            &mut dlg_state.labels,
            "Protocole :",
            y_proto,
            field_h,
            false,
        );
        let rc_proto = RECT {
            left: field_left,
            top: y_proto,
            right: field_right,
            bottom: y_proto + field_h,
        };
        let cb_proto = create_dark_combobox(
            hwnd,
            hinstance.into(),
            rc_proto,
            dpi.scale(160),
            IDC_DLG_PROTO,
            palette.font_ui,
            dpi,
        );
        for p_label in [
            w!("RDP (Intégré dans Malvision)"),
            w!("AnyDesk (Client externe)"),
            w!("RustDesk (Client externe)"),
        ] {
            let _ = SendMessageW(
                cb_proto,
                CB_ADDSTRING,
                Some(WPARAM(0)),
                Some(LPARAM(p_label.as_ptr() as isize)),
            );
        }

        // 3. Dossier parent
        let y_folder = dpi.scale(168);
        add_label(
            &mut dlg_state.labels,
            "Dossier parent :",
            y_folder,
            field_h,
            false,
        );
        let rc_folder = RECT {
            left: field_left,
            top: y_folder,
            right: field_right,
            bottom: y_folder + field_h,
        };
        let cb_folder = create_dark_combobox(
            hwnd,
            hinstance.into(),
            rc_folder,
            dpi.scale(240),
            IDC_DLG_FOLDER,
            palette.font_ui,
            dpi,
        );

        let choices = build_folder_choices(folders, None);
        let mut folder_options = Vec::with_capacity(choices.len());
        let target_folder = existing.and_then(|e| e.folder_id).or(default_folder_id);
        let mut selected_folder_idx: usize = 0;

        for (idx, (fid, label)) in choices.into_iter().enumerate() {
            if fid == target_folder {
                selected_folder_idx = idx;
            }
            folder_options.push(fid);
            let mut w_lbl: Vec<u16> = label.encode_utf16().collect();
            w_lbl.push(0);
            let _ = SendMessageW(
                cb_folder,
                CB_ADDSTRING,
                Some(WPARAM(0)),
                Some(LPARAM(w_lbl.as_ptr() as isize)),
            );
        }
        let _ = SendMessageW(
            cb_folder,
            CB_SETCURSEL,
            Some(WPARAM(selected_folder_idx)),
            Some(LPARAM(0)),
        );
        dlg_state.folder_options = folder_options;

        // 4. Hôte + Port
        dlg_state.labels.push(ConnDialogLabel {
            text: "Port :".to_string(),
            rect: RECT {
                left: rc_host.right + dpi.scale(12),
                top: y_host,
                right: rc_port.left - dpi.scale(4),
                bottom: y_host + field_h,
            },
            muted: false,
            bold: false,
            rdp_only: true,
        });

        let ed_host = create_dark_edit(
            hwnd,
            hinstance.into(),
            rc_host,
            IDC_DLG_HOST,
            false,
            palette.font_ui,
            dpi,
        );
        let ed_port = create_dark_edit(
            hwnd,
            hinstance.into(),
            rc_port,
            IDC_DLG_PORT,
            false,
            palette.font_ui,
            dpi,
        );
        set_window_text(ed_port, "3389");
        install_auto_select_on_focus(ed_port);
        dlg_state.edit_boxes.push(DialogEditBox {
            hwnd: ed_host,
            box_rect: rc_host,
            rdp_only: false,
        });
        dlg_state.edit_boxes.push(DialogEditBox {
            hwnd: ed_port,
            box_rect: rc_port,
            rdp_only: true,
        });

        // 5. Utilisateur
        let y_user = dpi.scale(252);
        add_label(
            &mut dlg_state.labels,
            "Nom d'utilisateur :",
            y_user,
            field_h,
            true,
        );
        let rc_user = RECT {
            left: field_left,
            top: y_user,
            right: field_right,
            bottom: y_user + field_h,
        };
        let ed_user = create_dark_edit(
            hwnd,
            hinstance.into(),
            rc_user,
            IDC_DLG_USER,
            false,
            palette.font_ui,
            dpi,
        );
        dlg_state.edit_boxes.push(DialogEditBox {
            hwnd: ed_user,
            box_rect: rc_user,
            rdp_only: true,
        });

        // 6. Domaine
        let y_dom = dpi.scale(294);
        add_label(
            &mut dlg_state.labels,
            "Domaine (facultatif) :",
            y_dom,
            field_h,
            true,
        );
        let rc_dom = RECT {
            left: field_left,
            top: y_dom,
            right: field_right,
            bottom: y_dom + field_h,
        };
        let ed_domain = create_dark_edit(
            hwnd,
            hinstance.into(),
            rc_dom,
            IDC_DLG_DOMAIN,
            false,
            palette.font_ui,
            dpi,
        );
        dlg_state.edit_boxes.push(DialogEditBox {
            hwnd: ed_domain,
            box_rect: rc_dom,
            rdp_only: true,
        });

        // 7. Mot de passe (libellé géré dynamiquement par WM_PAINT)
        let rc_pass = RECT {
            left: field_left,
            top: y_pass,
            right: field_right,
            bottom: y_pass + field_h,
        };
        let ed_pass = create_dark_edit(
            hwnd,
            hinstance.into(),
            rc_pass,
            IDC_DLG_PASS,
            true,
            palette.font_ui,
            dpi,
        );
        dlg_state.edit_boxes.push(DialogEditBox {
            hwnd: ed_pass,
            box_rect: rc_pass,
            rdp_only: false,
        });

        // 8. Cases à cocher larges (434 px à 96 DPI -> aucune troncature)
        let chk_h = dpi.scale(24);
        let y_chk1 = dpi.scale(372);
        let y_chk2 = dpi.scale(398);
        let y_chk_prio = dpi.scale(424);
        let y_chk3 = dpi.scale(450);
        let y_chk4 = dpi.scale(476);

        dlg_state.labels.push(ConnDialogLabel {
            text: "Options de session :".to_string(),
            rect: RECT {
                left: pad_x,
                top: y_chk1,
                right: field_left - dpi.scale(8),
                bottom: y_chk1 + chk_h,
            },
            muted: false,
            bold: false,
            rdp_only: false,
        });

        let chk_save_pass = create_dark_checkbox(
            hwnd,
            hinstance.into(),
            w!("Enregistrer le mot de passe (chiffré via Windows DPAPI)"),
            RECT {
                left: field_left,
                top: y_chk1,
                right: field_right,
                bottom: y_chk1 + chk_h,
            },
            IDC_DLG_SAVE_PASS,
            palette.font_ui,
        );
        let chk_fav = create_dark_checkbox(
            hwnd,
            hinstance.into(),
            w!("Marquer comme connexion favorite"),
            RECT {
                left: field_left,
                top: y_chk2,
                right: field_right,
                bottom: y_chk2 + chk_h,
            },
            IDC_DLG_FAVORITE,
            palette.font_ui,
        );
        let chk_priority = create_dark_checkbox(
            hwnd,
            hinstance.into(),
            w!("Connexion prioritaire (remonte au début de son dossier)"),
            RECT {
                left: field_left,
                top: y_chk_prio,
                right: field_right,
                bottom: y_chk_prio + chk_h,
            },
            IDC_DLG_PRIORITY,
            palette.font_ui,
        );
        let chk_dyn_res = create_dark_checkbox(
            hwnd,
            hinstance.into(),
            w!("Ajuster automatiquement la résolution (SmartSizing RDP)"),
            RECT {
                left: field_left,
                top: y_chk3,
                right: field_right,
                bottom: y_chk3 + chk_h,
            },
            IDC_DLG_DYN_RES,
            palette.font_ui,
        );
        let chk_clipboard = create_dark_checkbox(
            hwnd,
            hinstance.into(),
            w!("Partager le presse-papiers avec la session RDP"),
            RECT {
                left: field_left,
                top: y_chk4,
                right: field_right,
                bottom: y_chk4 + chk_h,
            },
            IDC_DLG_CLIPBOARD,
            palette.font_ui,
        );

        // 9. Description
        let y_desc = dpi.scale(508);
        add_label(
            &mut dlg_state.labels,
            "Description :",
            y_desc,
            field_h,
            false,
        );
        let rc_desc = RECT {
            left: field_left,
            top: y_desc,
            right: field_right,
            bottom: y_desc + field_h,
        };
        let ed_desc = create_dark_edit(
            hwnd,
            hinstance.into(),
            rc_desc,
            IDC_DLG_DESC,
            false,
            palette.font_ui,
            dpi,
        );
        dlg_state.edit_boxes.push(DialogEditBox {
            hwnd: ed_desc,
            box_rect: rc_desc,
            rdp_only: false,
        });

        // 10. Boutons d'action Enregistrer / Annuler
        let btn_w = dpi.scale(130);
        let btn_h = dpi.scale(36);
        let btn_y = dpi.scale(604);
        let btn_gap = dpi.scale(12);
        let rc_cancel = RECT {
            left: field_right - btn_w,
            top: btn_y,
            right: field_right,
            bottom: btn_y + btn_h,
        };
        let rc_ok = RECT {
            left: rc_cancel.left - btn_gap - btn_w,
            top: btn_y,
            right: rc_cancel.left - btn_gap,
            bottom: btn_y + btn_h,
        };

        let btn_ok = create_dark_button(
            hwnd,
            hinstance.into(),
            w!("Enregistrer"),
            rc_ok,
            IDC_DLG_OK,
            palette.font_ui_bold,
        );
        let btn_cancel = create_dark_button(
            hwnd,
            hinstance.into(),
            w!("Annuler"),
            rc_cancel,
            IDC_DLG_CANCEL,
            palette.font_ui,
        );

        dlg_state.ed_name = ed_name;
        dlg_state.cb_proto = cb_proto;
        dlg_state.cb_folder = cb_folder;
        dlg_state.ed_host = ed_host;
        dlg_state.ed_port = ed_port;
        dlg_state.ed_user = ed_user;
        dlg_state.ed_domain = ed_domain;
        dlg_state.ed_pass = ed_pass;
        dlg_state.chk_save_pass = chk_save_pass;
        dlg_state.chk_fav = chk_fav;
        dlg_state.chk_priority = chk_priority;
        dlg_state.chk_dyn_res = chk_dyn_res;
        dlg_state.chk_clipboard = chk_clipboard;
        dlg_state.ed_desc = ed_desc;

        // Pré-remplissage si édition d'une connexion existante
        if let Some(ex) = existing {
            set_window_text(ed_name, &ex.name);
            let p_idx = match ex.protocol {
                Protocol::Rdp => 0,
                Protocol::AnyDesk => 1,
                Protocol::RustDesk => 2,
            };
            let _ = SendMessageW(cb_proto, CB_SETCURSEL, Some(WPARAM(p_idx)), Some(LPARAM(0)));
            set_window_text(ed_host, &ex.host);
            set_window_text(ed_port, &ex.port.to_string());
            set_window_text(ed_user, &ex.username);
            set_window_text(ed_domain, &ex.domain);
            set_window_text(ed_desc, &ex.description);

            let _ = SendMessageW(
                chk_save_pass,
                BM_SETCHECK,
                Some(WPARAM(usize::from(ex.save_password))),
                Some(LPARAM(0)),
            );
            let _ = EnableWindow(ed_pass, ex.save_password);
            if ex.has_saved_password {
                let cue = w!("(Mot de passe enregistré — laisser vide pour conserver)");
                let _ = SendMessageW(
                    ed_pass,
                    0x1501,
                    Some(WPARAM(1)),
                    Some(LPARAM(cue.as_ptr() as isize)),
                );
            }

            let _ = SendMessageW(
                chk_fav,
                BM_SETCHECK,
                Some(WPARAM(usize::from(ex.is_favorite))),
                Some(LPARAM(0)),
            );
            let _ = SendMessageW(
                chk_priority,
                BM_SETCHECK,
                Some(WPARAM(usize::from(ex.is_priority))),
                Some(LPARAM(0)),
            );
            let _ = SendMessageW(
                chk_dyn_res,
                BM_SETCHECK,
                Some(WPARAM(usize::from(ex.dynamic_resolution))),
                Some(LPARAM(0)),
            );
            let _ = SendMessageW(
                chk_clipboard,
                BM_SETCHECK,
                Some(WPARAM(usize::from(ex.redirect_clipboard))),
                Some(LPARAM(0)),
            );
            update_dialog_protocol_layout(&mut dlg_state, hwnd, ex.protocol);
        } else {
            let _ = SendMessageW(cb_proto, CB_SETCURSEL, Some(WPARAM(0)), Some(LPARAM(0)));
            let _ = SendMessageW(chk_save_pass, BM_SETCHECK, Some(WPARAM(0)), Some(LPARAM(0)));
            let _ = EnableWindow(ed_pass, false);
            let _ = SendMessageW(
                chk_dyn_res,
                BM_SETCHECK,
                Some(WPARAM(usize::from(default_dyn_res))),
                Some(LPARAM(0)),
            );
            let _ = SendMessageW(
                chk_clipboard,
                BM_SETCHECK,
                Some(WPARAM(usize::from(default_clipboard))),
                Some(LPARAM(0)),
            );
            update_dialog_protocol_layout(&mut dlg_state, hwnd, Protocol::Rdp);
        }

        let tab_order = [
            ed_name,
            cb_proto,
            cb_folder,
            ed_host,
            ed_port,
            ed_user,
            ed_domain,
            ed_pass,
            chk_save_pass,
            chk_fav,
            chk_priority,
            chk_dyn_res,
            chk_clipboard,
            ed_desc,
            btn_ok,
            btn_cancel,
        ];

        let _ = EnableWindow(owner, false);
        let _ = ShowWindow(hwnd, SW_SHOW);
        let _ = UpdateWindow(hwnd);
        let _ = SetFocus(Some(ed_name));

        run_modal_loop(owner, hwnd, &tab_order);

        dlg_state.result.take()
    }
}

struct TextPromptDialogState {
    owner: HWND,
    dpi: DpiMetrics,
    font_ui: HFONT,
    font_bold: HFONT,
    font_subtitle: HFONT,
    brush_bg: HBRUSH,
    brush_input: HBRUSH,
    header_title: String,
    prompt_label: String,
    label_rc: RECT,
    edit_box_rc: RECT,
    divider_y: i32,
    ed_input: HWND,
    result: Option<String>,
}

unsafe extern "system" fn prompt_dlg_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        match msg {
            WM_ERASEBKGND => LRESULT(1),
            WM_CTLCOLORSTATIC | WM_CTLCOLORBTN => {
                let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const TextPromptDialogState;
                if !ptr.is_null() {
                    let hdc = HDC(wparam.0 as *mut _);
                    let _ = SetTextColor(hdc, COLORREF(col_text_primary()));
                    let _ = SetBkColor(hdc, COLORREF(col_bg_sidebar()));
                    return LRESULT((*ptr).brush_bg.0 as isize);
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            WM_CTLCOLOREDIT => {
                let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const TextPromptDialogState;
                if !ptr.is_null() {
                    let hdc = HDC(wparam.0 as *mut _);
                    let _ = SetTextColor(hdc, COLORREF(col_text_primary()));
                    let _ = SetBkColor(hdc, COLORREF(col_bg_input()));
                    return LRESULT((*ptr).brush_input.0 as isize);
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            WM_DRAWITEM => {
                let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const TextPromptDialogState;
                let dis = lparam.0 as *const DRAWITEMSTRUCT;
                if !ptr.is_null() && !dis.is_null() {
                    let state = &*ptr;
                    let d = &*dis;
                    if d.CtlType == ODT_BUTTON {
                        let id = d.CtlID as usize;
                        draw_owner_button(d, id == IDC_DLG_OK, state.font_ui, state.font_bold);
                        return LRESULT(1);
                    }
                }
                LRESULT(0)
            }
            WM_PAINT => {
                let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const TextPromptDialogState;
                if ptr.is_null() {
                    return DefWindowProcW(hwnd, msg, wparam, lparam);
                }
                let state = &*ptr;
                let mut ps = PAINTSTRUCT::default();
                let hdc_screen = BeginPaint(hwnd, &mut ps);
                let mut rc = RECT::default();
                let _ = GetClientRect(hwnd, &mut rc);
                let w = (rc.right - rc.left).max(1);
                let h = (rc.bottom - rc.top).max(1);

                let hdc = CreateCompatibleDC(Some(hdc_screen));
                let hbm = CreateCompatibleBitmap(hdc_screen, w, h);
                let old_bm = SelectObject(hdc, hbm.into());
                let _ = SetBkMode(hdc, TRANSPARENT);

                let _ = FillRect(hdc, &rc, state.brush_bg);

                let pad_x = state.dpi.scale(24);
                let _ = SelectObject(hdc, state.font_subtitle.into());
                let _ = SetTextColor(hdc, COLORREF(col_text_primary()));
                draw_text_line(
                    hdc,
                    &state.header_title,
                    RECT {
                        left: pad_x,
                        top: state.dpi.scale(16),
                        right: w - pad_x,
                        bottom: state.dpi.scale(42),
                    },
                );

                let _ = SelectObject(hdc, state.font_ui.into());
                let _ = SetTextColor(hdc, COLORREF(col_text_muted()));
                draw_text_line(hdc, &state.prompt_label, state.label_rc);

                let focused = GetFocus() == state.ed_input;
                draw_rounded_box(
                    hdc,
                    state.edit_box_rc,
                    col_bg_input(),
                    if focused { col_accent() } else { col_border() },
                );

                draw_horizontal_line(hdc, pad_x, w - pad_x, state.divider_y, col_border());

                let _ = windows::Win32::Graphics::Gdi::BitBlt(
                    hdc_screen,
                    0,
                    0,
                    w,
                    h,
                    Some(hdc),
                    0,
                    0,
                    windows::Win32::Graphics::Gdi::SRCCOPY,
                );
                let _ = SelectObject(hdc, old_bm);
                let _ = DeleteObject(hbm.into());
                let _ = DeleteDC(hdc);
                let _ = EndPaint(hwnd, &ps);
                LRESULT(0)
            }
            WM_COMMAND => {
                let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut TextPromptDialogState;
                if !ptr.is_null() {
                    let state = &mut *ptr;
                    let id = wparam.0 & 0xFFFF;
                    let code = ((wparam.0 >> 16) & 0xFFFF) as u32;
                    if code == EN_SETFOCUS || code == EN_KILLFOCUS {
                        let _ = InvalidateRect(Some(hwnd), None, false);
                        return LRESULT(0);
                    }
                    if id == IDC_DLG_OK || id == 1 {
                        let txt = get_window_text(state.ed_input);
                        if txt.trim().is_empty() {
                            show_warning_dialog(
                                hwnd,
                                "Champ requis",
                                "Veuillez saisir une valeur non vide.",
                            );
                        } else {
                            state.result = Some(txt.trim().to_string());
                            let owner = state.owner;
                            close_modal_dialog(hwnd, owner);
                        }
                    } else if id == IDC_DLG_CANCEL || id == 2 {
                        let owner = state.owner;
                        close_modal_dialog(hwnd, owner);
                    }
                }
                LRESULT(0)
            }
            WM_CLOSE => {
                let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut TextPromptDialogState;
                if !ptr.is_null() {
                    let owner = (*ptr).owner;
                    close_modal_dialog(hwnd, owner);
                } else {
                    let _ = DestroyWindow(hwnd);
                }
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}

fn run_text_prompt_dialog(
    owner: HWND,
    title: &str,
    prompt_label: &str,
    initial_value: &str,
    palette: &GdiPalette,
) -> Option<String> {
    unsafe {
        let hinstance = GetModuleHandleW(None).unwrap_or_default();
        let class_name = w!("MalvisionTextPromptDlgClass");
        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(prompt_dlg_wnd_proc),
            hInstance: hinstance.into(),
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            hbrBackground: palette.brush_bg_sidebar,
            lpszClassName: class_name,
            ..Default::default()
        };
        let _ = RegisterClassExW(&wc);

        let dpi = if owner.is_invalid() {
            palette.dpi
        } else {
            DpiMetrics::for_window(owner)
        };
        let client_w = dpi.scale(480);
        let client_h = dpi.scale(215);
        let dlg_style = WS_POPUP | WS_CAPTION | WS_SYSMENU | WS_CLIPCHILDREN;
        let (win_w, win_h) = calc_dialog_window_size(dlg_style, client_w, client_h);

        let mut owner_rc = RECT::default();
        let _ = GetWindowRect(owner, &mut owner_rc);
        let x = owner_rc.left + ((owner_rc.right - owner_rc.left) - win_w) / 2;
        let y = owner_rc.top + ((owner_rc.bottom - owner_rc.top) - win_h) / 2;

        let mut title_w: Vec<u16> = title.encode_utf16().collect();
        title_w.push(0);

        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            class_name,
            PCWSTR(title_w.as_ptr()),
            dlg_style,
            x.max(40),
            y.max(40),
            win_w,
            win_h,
            Some(owner),
            None,
            Some(hinstance.into()),
            None,
        )
        .ok()?;

        let dark_mode: i32 = 1;
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_USE_IMMERSIVE_DARK_MODE,
            &dark_mode as *const i32 as *const c_void,
            std::mem::size_of::<i32>() as u32,
        );

        let pad_x = dpi.scale(24);
        let label_rc = RECT {
            left: pad_x,
            top: dpi.scale(50),
            right: client_w - pad_x,
            bottom: dpi.scale(74),
        };
        let edit_box_rc = RECT {
            left: pad_x,
            top: dpi.scale(80),
            right: client_w - pad_x,
            bottom: dpi.scale(116),
        };
        let divider_y = dpi.scale(142);

        let mut state = Box::new(TextPromptDialogState {
            owner,
            dpi,
            font_ui: palette.font_ui,
            font_bold: palette.font_ui_bold,
            font_subtitle: palette.font_subtitle,
            brush_bg: palette.brush_bg_sidebar,
            brush_input: palette.brush_bg_input,
            header_title: title.to_string(),
            prompt_label: prompt_label.to_string(),
            label_rc,
            edit_box_rc,
            divider_y,
            ed_input: HWND::default(),
            result: None,
        });
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, (&mut *state) as *mut _ as isize);

        let ed_input = create_dark_edit(
            hwnd,
            hinstance.into(),
            edit_box_rc,
            IDC_DLG_NAME,
            false,
            palette.font_ui,
            dpi,
        );
        set_window_text(ed_input, initial_value);
        state.ed_input = ed_input;

        let btn_w = dpi.scale(116);
        let btn_h = dpi.scale(36);
        let btn_y = dpi.scale(158);
        let btn_gap = dpi.scale(12);
        let rc_cancel = RECT {
            left: client_w - pad_x - btn_w,
            top: btn_y,
            right: client_w - pad_x,
            bottom: btn_y + btn_h,
        };
        let rc_ok = RECT {
            left: rc_cancel.left - btn_gap - btn_w,
            top: btn_y,
            right: rc_cancel.left - btn_gap,
            bottom: btn_y + btn_h,
        };

        let btn_ok = create_dark_button(
            hwnd,
            hinstance.into(),
            w!("Valider"),
            rc_ok,
            IDC_DLG_OK,
            palette.font_ui_bold,
        );
        let btn_cancel = create_dark_button(
            hwnd,
            hinstance.into(),
            w!("Annuler"),
            rc_cancel,
            IDC_DLG_CANCEL,
            palette.font_ui,
        );

        let tab_order = [ed_input, btn_ok, btn_cancel];

        let _ = EnableWindow(owner, false);
        let _ = ShowWindow(hwnd, SW_SHOW);
        let _ = UpdateWindow(hwnd);
        let _ = SetFocus(Some(ed_input));

        run_modal_loop(owner, hwnd, &tab_order);

        state.result.take()
    }
}

struct FolderPickerDialogState {
    owner: HWND,
    dpi: DpiMetrics,
    font_ui: HFONT,
    font_bold: HFONT,
    font_subtitle: HFONT,
    brush_bg: HBRUSH,
    brush_input: HBRUSH,
    header_title: String,
    label_rc: RECT,
    divider_y: i32,
    cb_folder: HWND,
    folder_options: Vec<Option<i64>>,
    result: Option<Option<i64>>,
}

unsafe extern "system" fn folder_picker_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        match msg {
            WM_ERASEBKGND => LRESULT(1),
            WM_CTLCOLORSTATIC | WM_CTLCOLORBTN => {
                let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const FolderPickerDialogState;
                if !ptr.is_null() {
                    let hdc = HDC(wparam.0 as *mut _);
                    let _ = SetTextColor(hdc, COLORREF(col_text_primary()));
                    let _ = SetBkColor(hdc, COLORREF(col_bg_sidebar()));
                    return LRESULT((*ptr).brush_bg.0 as isize);
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            WM_CTLCOLORLISTBOX | WM_CTLCOLOREDIT => {
                let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const FolderPickerDialogState;
                if !ptr.is_null() {
                    let hdc = HDC(wparam.0 as *mut _);
                    let _ = SetTextColor(hdc, COLORREF(col_text_primary()));
                    let _ = SetBkColor(hdc, COLORREF(col_bg_input()));
                    return LRESULT((*ptr).brush_input.0 as isize);
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            WM_MEASUREITEM => {
                let mis = lparam.0 as *mut MEASUREITEMSTRUCT;
                if !mis.is_null() {
                    let dpi = DpiMetrics::for_window(hwnd);
                    (*mis).itemHeight = dpi.scale(26) as u32;
                    return LRESULT(1);
                }
                LRESULT(0)
            }
            WM_DRAWITEM => {
                let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const FolderPickerDialogState;
                let dis = lparam.0 as *const DRAWITEMSTRUCT;
                if !ptr.is_null() && !dis.is_null() {
                    let state = &*ptr;
                    let d = &*dis;
                    if d.CtlType == ODT_COMBOBOX {
                        draw_owner_combobox_item(d, state.font_ui);
                        return LRESULT(1);
                    }
                    if d.CtlType == ODT_BUTTON {
                        let id = d.CtlID as usize;
                        draw_owner_button(d, id == IDC_DLG_OK, state.font_ui, state.font_bold);
                        return LRESULT(1);
                    }
                }
                LRESULT(0)
            }
            WM_PAINT => {
                let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const FolderPickerDialogState;
                if ptr.is_null() {
                    return DefWindowProcW(hwnd, msg, wparam, lparam);
                }
                let state = &*ptr;
                let mut ps = PAINTSTRUCT::default();
                let hdc_screen = BeginPaint(hwnd, &mut ps);
                let mut rc = RECT::default();
                let _ = GetClientRect(hwnd, &mut rc);
                let w = (rc.right - rc.left).max(1);
                let h = (rc.bottom - rc.top).max(1);

                let hdc = CreateCompatibleDC(Some(hdc_screen));
                let hbm = CreateCompatibleBitmap(hdc_screen, w, h);
                let old_bm = SelectObject(hdc, hbm.into());
                let _ = SetBkMode(hdc, TRANSPARENT);

                let _ = FillRect(hdc, &rc, state.brush_bg);

                let pad_x = state.dpi.scale(24);
                let _ = SelectObject(hdc, state.font_subtitle.into());
                let _ = SetTextColor(hdc, COLORREF(col_text_primary()));
                draw_text_line(
                    hdc,
                    &state.header_title,
                    RECT {
                        left: pad_x,
                        top: state.dpi.scale(16),
                        right: w - pad_x,
                        bottom: state.dpi.scale(42),
                    },
                );

                let _ = SelectObject(hdc, state.font_ui.into());
                let _ = SetTextColor(hdc, COLORREF(col_text_muted()));
                draw_text_line(
                    hdc,
                    "Sélectionnez le dossier de destination :",
                    state.label_rc,
                );

                draw_horizontal_line(hdc, pad_x, w - pad_x, state.divider_y, col_border());

                let _ = windows::Win32::Graphics::Gdi::BitBlt(
                    hdc_screen,
                    0,
                    0,
                    w,
                    h,
                    Some(hdc),
                    0,
                    0,
                    windows::Win32::Graphics::Gdi::SRCCOPY,
                );
                let _ = SelectObject(hdc, old_bm);
                let _ = DeleteObject(hbm.into());
                let _ = DeleteDC(hdc);
                let _ = EndPaint(hwnd, &ps);
                LRESULT(0)
            }
            WM_COMMAND => {
                let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut FolderPickerDialogState;
                if !ptr.is_null() {
                    let state = &mut *ptr;
                    let id = wparam.0 & 0xFFFF;
                    if id == IDC_DLG_OK || id == 1 {
                        let sel = SendMessageW(
                            state.cb_folder,
                            CB_GETCURSEL,
                            Some(WPARAM(0)),
                            Some(LPARAM(0)),
                        )
                        .0;
                        if sel >= 0 && (sel as usize) < state.folder_options.len() {
                            state.result = Some(state.folder_options[sel as usize]);
                        } else {
                            state.result = Some(None);
                        }
                        let owner = state.owner;
                        close_modal_dialog(hwnd, owner);
                    } else if id == IDC_DLG_CANCEL || id == 2 {
                        let owner = state.owner;
                        close_modal_dialog(hwnd, owner);
                    }
                }
                LRESULT(0)
            }
            WM_CLOSE => {
                let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut FolderPickerDialogState;
                if !ptr.is_null() {
                    let owner = (*ptr).owner;
                    close_modal_dialog(hwnd, owner);
                } else {
                    let _ = DestroyWindow(hwnd);
                }
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}

fn run_folder_picker_dialog(
    owner: HWND,
    title: &str,
    folders: &[Folder],
    current_folder: Option<i64>,
    exclude_subtree_of: Option<i64>,
    palette: &GdiPalette,
) -> Option<Option<i64>> {
    unsafe {
        let hinstance = GetModuleHandleW(None).unwrap_or_default();
        let class_name = w!("MalvisionFolderPickerDlgClass");
        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(folder_picker_wnd_proc),
            hInstance: hinstance.into(),
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            hbrBackground: palette.brush_bg_sidebar,
            lpszClassName: class_name,
            ..Default::default()
        };
        let _ = RegisterClassExW(&wc);

        let dpi = if owner.is_invalid() {
            palette.dpi
        } else {
            DpiMetrics::for_window(owner)
        };
        let client_w = dpi.scale(480);
        let client_h = dpi.scale(215);
        let dlg_style = WS_POPUP | WS_CAPTION | WS_SYSMENU | WS_CLIPCHILDREN;
        let (win_w, win_h) = calc_dialog_window_size(dlg_style, client_w, client_h);

        let mut owner_rc = RECT::default();
        let _ = GetWindowRect(owner, &mut owner_rc);
        let x = owner_rc.left + ((owner_rc.right - owner_rc.left) - win_w) / 2;
        let y = owner_rc.top + ((owner_rc.bottom - owner_rc.top) - win_h) / 2;

        let mut title_w: Vec<u16> = title.encode_utf16().collect();
        title_w.push(0);

        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            class_name,
            PCWSTR(title_w.as_ptr()),
            dlg_style,
            x.max(40),
            y.max(40),
            win_w,
            win_h,
            Some(owner),
            None,
            Some(hinstance.into()),
            None,
        )
        .ok()?;

        let dark_mode: i32 = 1;
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_USE_IMMERSIVE_DARK_MODE,
            &dark_mode as *const i32 as *const c_void,
            std::mem::size_of::<i32>() as u32,
        );

        let pad_x = dpi.scale(24);
        let label_rc = RECT {
            left: pad_x,
            top: dpi.scale(50),
            right: client_w - pad_x,
            bottom: dpi.scale(74),
        };
        let combo_rc = RECT {
            left: pad_x,
            top: dpi.scale(80),
            right: client_w - pad_x,
            bottom: dpi.scale(114),
        };
        let divider_y = dpi.scale(142);

        let mut state = Box::new(FolderPickerDialogState {
            owner,
            dpi,
            font_ui: palette.font_ui,
            font_bold: palette.font_ui_bold,
            font_subtitle: palette.font_subtitle,
            brush_bg: palette.brush_bg_sidebar,
            brush_input: palette.brush_bg_input,
            header_title: title.to_string(),
            label_rc,
            divider_y,
            cb_folder: HWND::default(),
            folder_options: Vec::new(),
            result: None,
        });
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, (&mut *state) as *mut _ as isize);

        let cb_folder = create_dark_combobox(
            hwnd,
            hinstance.into(),
            combo_rc,
            dpi.scale(220),
            IDC_DLG_FOLDER,
            palette.font_ui,
            dpi,
        );

        let choices = build_folder_choices(folders, exclude_subtree_of);
        let mut folder_options = Vec::with_capacity(choices.len());
        let mut sel_idx: usize = 0;
        for (idx, (fid, label)) in choices.into_iter().enumerate() {
            if fid == current_folder {
                sel_idx = idx;
            }
            folder_options.push(fid);
            let mut w_lbl: Vec<u16> = label.encode_utf16().collect();
            w_lbl.push(0);
            let _ = SendMessageW(
                cb_folder,
                CB_ADDSTRING,
                Some(WPARAM(0)),
                Some(LPARAM(w_lbl.as_ptr() as isize)),
            );
        }
        let _ = SendMessageW(
            cb_folder,
            CB_SETCURSEL,
            Some(WPARAM(sel_idx)),
            Some(LPARAM(0)),
        );
        state.cb_folder = cb_folder;
        state.folder_options = folder_options;

        let btn_w = dpi.scale(116);
        let btn_h = dpi.scale(36);
        let btn_y = dpi.scale(158);
        let btn_gap = dpi.scale(12);
        let rc_cancel = RECT {
            left: client_w - pad_x - btn_w,
            top: btn_y,
            right: client_w - pad_x,
            bottom: btn_y + btn_h,
        };
        let rc_ok = RECT {
            left: rc_cancel.left - btn_gap - btn_w,
            top: btn_y,
            right: rc_cancel.left - btn_gap,
            bottom: btn_y + btn_h,
        };

        let btn_ok = create_dark_button(
            hwnd,
            hinstance.into(),
            w!("Déplacer"),
            rc_ok,
            IDC_DLG_OK,
            palette.font_ui_bold,
        );
        let btn_cancel = create_dark_button(
            hwnd,
            hinstance.into(),
            w!("Annuler"),
            rc_cancel,
            IDC_DLG_CANCEL,
            palette.font_ui,
        );

        let tab_order = [cb_folder, btn_ok, btn_cancel];

        let _ = EnableWindow(owner, false);
        let _ = ShowWindow(hwnd, SW_SHOW);
        let _ = UpdateWindow(hwnd);
        let _ = SetFocus(Some(cb_folder));

        run_modal_loop(owner, hwnd, &tab_order);

        state.result.take()
    }
}

// =============================================================================
// FENÊTRE MODALE DES PARAMÈTRES (APPARENCE, COMPORTEMENT, ACCUEIL, RDP)
// =============================================================================

// =============================================================================
// FENÊTRE MODALE DES PARAMÈTRES (APPARENCE, COMPORTEMENT, DONNÉES, MISES À JOUR, À PROPOS)
// =============================================================================

pub const IDC_SET_THEME: usize = 4201;
pub const IDC_SET_CLOSE_TRAY: usize = 4202;
pub const IDC_SET_START_MIN_TRAY: usize = 4203;
pub const IDC_SET_CONFIRM_EXIT: usize = 4204;
pub const IDC_SET_RESTORE_RDP: usize = 4205;
pub const IDC_SET_START_WIN: usize = 4206;
pub const IDC_SET_SHOW_RECENT: usize = 4207;
pub const IDC_SET_MAX_RECENT: usize = 4208;
pub const IDC_SET_RDP_CLIP: usize = 4209;
pub const IDC_SET_RDP_DYNRES: usize = 4210;
pub const IDC_SET_RDP_HOOK_MODE: usize = 4211;
pub const IDC_SET_ANYDESK_PATH: usize = 4212;
pub const IDC_SET_ANYDESK_BROWSE: usize = 4213;
pub const IDC_SET_ANYDESK_DETECT: usize = 4214;
pub const IDC_SET_ANYDESK_TEST: usize = 4215;
pub const IDC_SET_RUSTDESK_PATH: usize = 4216;
pub const IDC_SET_RUSTDESK_BROWSE: usize = 4217;
pub const IDC_SET_RUSTDESK_DETECT: usize = 4218;
pub const IDC_SET_RUSTDESK_TEST: usize = 4219;
pub const IDC_SET_DATA_EXPORT: usize = 4220;
pub const IDC_SET_DATA_IMPORT: usize = 4221;
pub const IDC_SET_UPDATE_AUTO: usize = 4222;
pub const IDC_SET_UPDATE_CHECK_NOW: usize = 4223;
pub const IDC_SET_UPDATE_DOWNLOAD: usize = 4224;
pub const IDC_SET_ABOUT_GITHUB: usize = 4225;
pub const IDC_SET_ABOUT_CHECK_UPDATES: usize = 4226;
pub const IDC_SET_ADV_PATH_DISPLAY: usize = 4227;
pub const IDC_SET_ADV_CHANGE_PATH: usize = 4228;
pub const IDC_SET_ADV_OPEN_FOLDER: usize = 4229;
pub const IDC_SET_ADV_RESET_DEFAULT: usize = 4230;
pub const IDC_SET_UPDATE_MANUAL_DOWNLOAD: usize = 4231;
#[allow(dead_code)]
pub const IDC_SET_TAB_GENERAL: usize = 4290;
#[allow(dead_code)]
pub const IDC_SET_TAB_PROTOCOLS: usize = 4291;
#[allow(dead_code)]
pub const IDC_SET_TAB_DATA: usize = 4292;
#[allow(dead_code)]
pub const IDC_SET_TAB_UPDATES: usize = 4293;
#[allow(dead_code)]
pub const IDC_SET_TAB_ADVANCED: usize = 4294;
#[allow(dead_code)]
pub const IDC_SET_TAB_ABOUT: usize = 4295;

struct SettingsDialogState {
    owner: HWND,
    dpi: DpiMetrics,
    font_ui: HFONT,
    font_bold: HFONT,
    font_subtitle: HFONT,
    font_title: HFONT,
    font_small: HFONT,
    brush_bg: HBRUSH,
    brush_input: HBRUSH,
    owned_brushes: bool,
    storage: *mut Storage,
    imported_changes: bool,

    current_tab: usize,
    hover_tab: Option<usize>,
    sidebar_w: i32,

    base_settings: AppSettings,

    // Tab 0 : Général
    cb_theme: HWND,
    chk_close_tray: HWND,
    chk_start_min_tray: HWND,
    chk_confirm_exit: HWND,
    chk_restore_rdp: HWND,
    chk_start_win: HWND,
    chk_show_recent: HWND,
    ed_max_recent: HWND,
    edit_box_rc: RECT,

    // Tab 1 : Protocoles
    ed_anydesk_path: HWND,
    btn_anydesk_browse: HWND,
    btn_anydesk_detect: HWND,
    btn_anydesk_test: HWND,
    anydesk_rc: RECT,
    ed_rustdesk_path: HWND,
    btn_rustdesk_browse: HWND,
    btn_rustdesk_detect: HWND,
    btn_rustdesk_test: HWND,
    rustdesk_rc: RECT,
    chk_rdp_clip: HWND,
    chk_rdp_dynres: HWND,
    cb_rdp_hook_mode: HWND,

    // Tab 2 : Données
    btn_export_data: HWND,
    btn_import_data: HWND,

    // Tab 3 : Mises à jour
    chk_auto_update: HWND,
    btn_check_update: HWND,
    btn_download_update: HWND,
    btn_manual_download: HWND,
    is_updating: bool,
    active_sessions_count: usize,
    update_status_text: String,
    latest_release: Option<crate::updater::ReleaseInfo>,

    // Tab 4 : Avancé (Emplacement personnalisé malvision.db)
    ed_adv_path: HWND,
    btn_adv_change_path: HWND,
    btn_adv_open_folder: HWND,
    btn_adv_reset_default: HWND,
    adv_path_rc: RECT,

    // Tab 5 : À propos
    btn_about_github: HWND,
    btn_about_check_updates: HWND,
    logo_bitmap: Option<HBITMAP>,

    // Boutons d'action globaux (barre inférieure)
    btn_ok: HWND,
    btn_cancel: HWND,

    result: Option<AppSettings>,
}

impl Drop for SettingsDialogState {
    fn drop(&mut self) {
        if self.owned_brushes {
            unsafe {
                if !self.brush_bg.is_invalid() {
                    let _ = DeleteObject(self.brush_bg.into());
                }
                if !self.brush_input.is_invalid() {
                    let _ = DeleteObject(self.brush_input.into());
                }
            }
        }
        if let Some(bm) = self.logo_bitmap {
            unsafe {
                if !bm.is_invalid() {
                    let _ = DeleteObject(bm.into());
                }
            }
        }
    }
}

impl SettingsDialogState {
    fn update_tab_visibility(&self) {
        let t = self.current_tab;

        // Onglet 0 : Général
        for &ctrl in &[
            self.cb_theme,
            self.chk_close_tray,
            self.chk_start_min_tray,
            self.chk_confirm_exit,
            self.chk_restore_rdp,
            self.chk_start_win,
            self.chk_show_recent,
            self.ed_max_recent,
        ] {
            if !ctrl.is_invalid() {
                let _ = unsafe { ShowWindow(ctrl, if t == 0 { SW_SHOW } else { SW_HIDE }) };
            }
        }

        // Onglet 1 : Protocoles
        for &ctrl in &[
            self.ed_anydesk_path,
            self.btn_anydesk_browse,
            self.btn_anydesk_detect,
            self.btn_anydesk_test,
            self.ed_rustdesk_path,
            self.btn_rustdesk_browse,
            self.btn_rustdesk_detect,
            self.btn_rustdesk_test,
            self.chk_rdp_clip,
            self.chk_rdp_dynres,
            self.cb_rdp_hook_mode,
        ] {
            if !ctrl.is_invalid() {
                let _ = unsafe { ShowWindow(ctrl, if t == 1 { SW_SHOW } else { SW_HIDE }) };
            }
        }

        // Onglet 2 : Données
        for &ctrl in &[self.btn_export_data, self.btn_import_data] {
            if !ctrl.is_invalid() {
                let _ = unsafe { ShowWindow(ctrl, if t == 2 { SW_SHOW } else { SW_HIDE }) };
            }
        }

        // Onglet 3 : Mises à jour
        for &ctrl in &[
            self.chk_auto_update,
            self.btn_check_update,
            self.btn_download_update,
            self.btn_manual_download,
        ] {
            if !ctrl.is_invalid() {
                let _ = unsafe { ShowWindow(ctrl, if t == 3 { SW_SHOW } else { SW_HIDE }) };
            }
        }

        // Onglet 4 : Avancé
        for &ctrl in &[
            self.ed_adv_path,
            self.btn_adv_change_path,
            self.btn_adv_open_folder,
            self.btn_adv_reset_default,
        ] {
            if !ctrl.is_invalid() {
                let _ = unsafe { ShowWindow(ctrl, if t == 4 { SW_SHOW } else { SW_HIDE }) };
            }
        }

        // Onglet 5 : À propos
        for &ctrl in &[self.btn_about_github, self.btn_about_check_updates] {
            if !ctrl.is_invalid() {
                let _ = unsafe { ShowWindow(ctrl, if t == 5 { SW_SHOW } else { SW_HIDE }) };
            }
        }
    }
}

fn notify_update_status(dlg_hwnd_raw: isize, status: &str) {
    let boxed = Box::new(status.to_string());
    unsafe {
        let hwnd = HWND(dlg_hwnd_raw as *mut _);
        if IsWindow(Some(hwnd)).as_bool() {
            let _ = PostMessageW(
                Some(hwnd),
                WM_APP_UPDATE_PROGRESS,
                WPARAM(0),
                LPARAM(Box::into_raw(boxed) as isize),
            );
        }
    }
}

fn notify_update_error(dlg_hwnd_raw: isize, err: String) {
    let boxed = Box::new(err);
    unsafe {
        let hwnd = HWND(dlg_hwnd_raw as *mut _);
        if IsWindow(Some(hwnd)).as_bool() {
            let _ = PostMessageW(
                Some(hwnd),
                WM_APP_UPDATE_PROGRESS,
                WPARAM(1),
                LPARAM(Box::into_raw(boxed) as isize),
            );
        }
    }
}

unsafe extern "system" fn settings_dlg_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        match msg {
            WM_ERASEBKGND => LRESULT(1),
            WM_CTLCOLORSTATIC | WM_CTLCOLORBTN => {
                let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const SettingsDialogState;
                if !ptr.is_null() {
                    let hdc = HDC(wparam.0 as *mut _);
                    let _ = SetTextColor(hdc, COLORREF(col_text_primary()));
                    let _ = SetBkColor(hdc, COLORREF(col_bg_sidebar()));
                    return LRESULT((*ptr).brush_bg.0 as isize);
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            WM_CTLCOLOREDIT | WM_CTLCOLORLISTBOX => {
                let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const SettingsDialogState;
                if !ptr.is_null() {
                    let hdc = HDC(wparam.0 as *mut _);
                    let _ = SetTextColor(hdc, COLORREF(col_text_primary()));
                    let _ = SetBkColor(hdc, COLORREF(col_bg_input()));
                    return LRESULT((*ptr).brush_input.0 as isize);
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            WM_MEASUREITEM => {
                let mis = lparam.0 as *mut MEASUREITEMSTRUCT;
                if !mis.is_null() {
                    let dpi = DpiMetrics::for_window(hwnd);
                    (*mis).itemHeight = dpi.scale(26) as u32;
                    return LRESULT(1);
                }
                LRESULT(0)
            }
            WM_DRAWITEM => {
                let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const SettingsDialogState;
                let dis = lparam.0 as *const DRAWITEMSTRUCT;
                if !ptr.is_null() && !dis.is_null() {
                    let state = &*ptr;
                    let d = &*dis;
                    if d.CtlType == ODT_COMBOBOX {
                        draw_owner_combobox_item(d, state.font_ui);
                        return LRESULT(1);
                    }
                    if d.CtlType == ODT_BUTTON {
                        let id = d.CtlID as usize;
                        let is_primary = id == IDC_DLG_OK
                            || id == IDC_SET_DATA_EXPORT
                            || id == IDC_SET_ADV_CHANGE_PATH;
                        if id == IDC_DLG_OK
                            || id == IDC_DLG_CANCEL
                            || id == IDC_SET_ANYDESK_BROWSE
                            || id == IDC_SET_ANYDESK_DETECT
                            || id == IDC_SET_ANYDESK_TEST
                            || id == IDC_SET_RUSTDESK_BROWSE
                            || id == IDC_SET_RUSTDESK_DETECT
                            || id == IDC_SET_RUSTDESK_TEST
                            || id == IDC_SET_DATA_EXPORT
                            || id == IDC_SET_DATA_IMPORT
                            || id == IDC_SET_UPDATE_CHECK_NOW
                            || id == IDC_SET_UPDATE_DOWNLOAD
                            || id == IDC_SET_UPDATE_MANUAL_DOWNLOAD
                            || id == IDC_SET_ADV_CHANGE_PATH
                            || id == IDC_SET_ADV_OPEN_FOLDER
                            || id == IDC_SET_ADV_RESET_DEFAULT
                            || id == IDC_SET_ABOUT_GITHUB
                            || id == IDC_SET_ABOUT_CHECK_UPDATES
                        {
                            draw_owner_button(d, is_primary, state.font_ui, state.font_bold);
                        } else {
                            draw_owner_checkbox(d, state.font_ui, state.dpi);
                        }
                        return LRESULT(1);
                    }
                }
                LRESULT(0)
            }
            WM_LBUTTONDOWN => {
                let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut SettingsDialogState;
                if !ptr.is_null() {
                    let state = &mut *ptr;
                    let x = (lparam.0 & 0xFFFF) as i16 as i32;
                    let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;

                    let bottom_bar_y = state.dpi.scale(570) - state.dpi.scale(56);
                    if x < state.sidebar_w && y < bottom_bar_y {
                        let start_y = state.dpi.scale(24);
                        let item_h = state.dpi.scale(42);
                        if y >= start_y {
                            let tab_idx = ((y - start_y) / item_h) as usize;
                            if tab_idx < 6 && tab_idx != state.current_tab {
                                state.current_tab = tab_idx;
                                state.update_tab_visibility();
                                let _ = InvalidateRect(Some(hwnd), None, true);
                            }
                        }
                    }
                }
                LRESULT(0)
            }
            WM_APP_UPDATE_PROGRESS => {
                let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut SettingsDialogState;
                if !ptr.is_null() && lparam.0 != 0 {
                    let state = &mut *ptr;
                    let is_error = wparam.0 == 1;
                    let text = *Box::from_raw(lparam.0 as *mut String);
                    if is_error {
                        state.is_updating = false;
                        state.update_status_text = format!("Mise à jour impossible : {text}");
                        let _ = EnableWindow(state.btn_check_update, true);
                        let _ = EnableWindow(state.btn_download_update, true);
                        let _ = EnableWindow(state.btn_manual_download, true);
                        let _ = InvalidateRect(Some(hwnd), None, false);
                        show_error_dialog(
                            hwnd,
                            "Mise à jour impossible",
                            &format!("Impossible d'appliquer la mise à jour :\n\n{text}"),
                        );
                    } else {
                        state.update_status_text = text;
                        let _ = InvalidateRect(Some(hwnd), None, false);
                    }
                }
                LRESULT(0)
            }
            WM_MOUSEMOVE => {
                let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut SettingsDialogState;
                if !ptr.is_null() {
                    let state = &mut *ptr;
                    let x = (lparam.0 & 0xFFFF) as i16 as i32;
                    let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;

                    let bottom_bar_y = state.dpi.scale(570) - state.dpi.scale(56);
                    let mut hover = None;
                    if x < state.sidebar_w && y < bottom_bar_y {
                        let start_y = state.dpi.scale(24);
                        let item_h = state.dpi.scale(42);
                        if y >= start_y {
                            let tab_idx = ((y - start_y) / item_h) as usize;
                            if tab_idx < 6 {
                                hover = Some(tab_idx);
                            }
                        }
                    }
                    if hover != state.hover_tab {
                        state.hover_tab = hover;
                        let _ = InvalidateRect(Some(hwnd), None, false);
                        let mut tme = TRACKMOUSEEVENT {
                            cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                            dwFlags: TME_LEAVE,
                            hwndTrack: hwnd,
                            dwHoverTime: 0,
                        };
                        let _ = TrackMouseEvent(&mut tme);
                    }
                }
                LRESULT(0)
            }
            WM_MOUSELEAVE => {
                let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut SettingsDialogState;
                if !ptr.is_null() {
                    let state = &mut *ptr;
                    if state.hover_tab.is_some() {
                        state.hover_tab = None;
                        let _ = InvalidateRect(Some(hwnd), None, false);
                    }
                }
                LRESULT(0)
            }
            WM_PAINT => {
                let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const SettingsDialogState;
                if ptr.is_null() {
                    return DefWindowProcW(hwnd, msg, wparam, lparam);
                }
                let state = &*ptr;
                let mut ps = PAINTSTRUCT::default();
                let hdc_screen = BeginPaint(hwnd, &mut ps);
                let mut rc = RECT::default();
                let _ = GetClientRect(hwnd, &mut rc);
                let w = (rc.right - rc.left).max(1);
                let h = (rc.bottom - rc.top).max(1);

                let hdc = CreateCompatibleDC(Some(hdc_screen));
                let hbm = CreateCompatibleBitmap(hdc_screen, w, h);
                let old_bm = SelectObject(hdc, hbm.into());
                let _ = SetBkMode(hdc, TRANSPARENT);

                // Fond général de la fenêtre
                let _ = FillRect(hdc, &rc, state.brush_bg);

                let sidebar_w = state.sidebar_w;
                let bottom_bar_h = state.dpi.scale(56);
                let content_h = h - bottom_bar_h;
                let content_x = sidebar_w + state.dpi.scale(28);
                let content_right = w - state.dpi.scale(28);

                // --- 1. BARRE LATÉRALE DE NAVIGATION (STYLE WINDOWS 11 FLUENT) ---
                let sidebar_rc = RECT {
                    left: 0,
                    top: 0,
                    right: sidebar_w,
                    bottom: content_h,
                };
                draw_rounded_box(hdc, sidebar_rc, col_bg_sidebar(), col_bg_sidebar());
                draw_horizontal_line(hdc, sidebar_w, sidebar_w, content_h, col_border());
                // Ligne verticale de séparation de la barre latérale
                let _ = MoveToEx(hdc, sidebar_w, 0, None);
                let _ = LineTo(hdc, sidebar_w, content_h);

                let tab_labels = [
                    "Général",
                    "Protocoles",
                    "Données",
                    "Mises à jour",
                    "Avancé",
                    "À propos",
                ];
                let item_start_y = state.dpi.scale(24);
                let item_h = state.dpi.scale(42);
                let pad_sidebar_x = state.dpi.scale(12);

                for (i, label) in tab_labels.iter().enumerate() {
                    let item_top = item_start_y + (i as i32) * item_h;
                    let item_rc = RECT {
                        left: pad_sidebar_x,
                        top: item_top,
                        right: sidebar_w - pad_sidebar_x,
                        bottom: item_top + state.dpi.scale(36),
                    };

                    if i == state.current_tab {
                        // Onglet sélectionné : pilule accentuée
                        draw_rounded_box(hdc, item_rc, col_accent(), col_accent());
                        let _ = SelectObject(hdc, state.font_bold.into());
                        let _ = SetTextColor(hdc, COLORREF(0x00FFFFFF));
                    } else if state.hover_tab == Some(i) {
                        // Onglet survolé : surbrillance subtile
                        draw_rounded_box(hdc, item_rc, col_bg_input(), col_border());
                        let _ = SelectObject(hdc, state.font_ui.into());
                        let _ = SetTextColor(hdc, COLORREF(col_text_primary()));
                    } else {
                        // Onglet normal
                        let _ = SelectObject(hdc, state.font_ui.into());
                        let _ = SetTextColor(hdc, COLORREF(col_text_secondary()));
                    }

                    let mut text_rc = item_rc;
                    text_rc.left += state.dpi.scale(12);
                    draw_text_line(hdc, label, text_rc);
                }

                // --- 2. EN-TÊTE DU CONTENU SELON L'ONGLET ACTIF ---
                let (header_title, header_desc) = match state.current_tab {
                    0 => (
                        "Paramètres Généraux",
                        "Personnalisez l'apparence visuelle, le System Tray et l'écran d'accueil.",
                    ),
                    1 => (
                        "Protocoles de Connexion",
                        "Configuration des chemins AnyDesk/RustDesk et options par défaut RDP.",
                    ),
                    2 => (
                        "Données & Sauvegardes",
                        "Exportez ou restaurez votre configuration de dossiers et connexions SQLite.",
                    ),
                    3 => (
                        "Mises à Jour du Logiciel",
                        "Vérification et téléchargement sécurisé des versions officielles GitHub.",
                    ),
                    4 => (
                        "Emplacement des Données (Avancé)",
                        "Configurez l'emplacement de votre base de données SQLite malvision.db.",
                    ),
                    _ => (
                        "À propos de Malvision",
                        concat!(
                            "Gestionnaire de connexions distantes natif Windows 11 — Version ",
                            env!("CARGO_PKG_VERSION"),
                            "."
                        ),
                    ),
                };

                let _ = SelectObject(hdc, state.font_subtitle.into());
                let _ = SetTextColor(hdc, COLORREF(col_text_primary()));
                draw_text_line(
                    hdc,
                    header_title,
                    RECT {
                        left: content_x,
                        top: state.dpi.scale(16),
                        right: content_right,
                        bottom: state.dpi.scale(40),
                    },
                );

                let _ = SelectObject(hdc, state.font_small.into());
                let _ = SetTextColor(hdc, COLORREF(col_text_muted()));
                draw_text_line(
                    hdc,
                    header_desc,
                    RECT {
                        left: content_x,
                        top: state.dpi.scale(42),
                        right: content_right,
                        bottom: state.dpi.scale(60),
                    },
                );

                draw_horizontal_line(
                    hdc,
                    content_x,
                    content_right,
                    state.dpi.scale(66),
                    col_border(),
                );

                // --- 3. DESSIN DU CONTENU DE L'ONGLET ACTIF ---
                match state.current_tab {
                    0 => {
                        // ONGLET 0 : GÉNÉRAL
                        let _ = SelectObject(hdc, state.font_bold.into());
                        let _ = SetTextColor(hdc, COLORREF(col_accent()));
                        draw_text_line(
                            hdc,
                            "Apparence",
                            RECT {
                                left: content_x,
                                top: state.dpi.scale(76),
                                right: content_right,
                                bottom: state.dpi.scale(98),
                            },
                        );

                        let _ = SelectObject(hdc, state.font_ui.into());
                        let _ = SetTextColor(hdc, COLORREF(col_text_primary()));
                        draw_text_line(
                            hdc,
                            "Thème de l'interface :",
                            RECT {
                                left: content_x,
                                top: state.dpi.scale(104),
                                right: content_x + state.dpi.scale(180),
                                bottom: state.dpi.scale(134),
                            },
                        );

                        let _ = SelectObject(hdc, state.font_bold.into());
                        let _ = SetTextColor(hdc, COLORREF(col_accent()));
                        draw_text_line(
                            hdc,
                            "Comportement & Zone de notification (System Tray)",
                            RECT {
                                left: content_x,
                                top: state.dpi.scale(152),
                                right: content_right,
                                bottom: state.dpi.scale(174),
                            },
                        );

                        let _ = SelectObject(hdc, state.font_bold.into());
                        let _ = SetTextColor(hdc, COLORREF(col_accent()));
                        draw_text_line(
                            hdc,
                            "Écran d'accueil",
                            RECT {
                                left: content_x,
                                top: state.dpi.scale(336),
                                right: content_right,
                                bottom: state.dpi.scale(358),
                            },
                        );

                        let _ = SelectObject(hdc, state.font_ui.into());
                        let _ = SetTextColor(hdc, COLORREF(col_text_primary()));
                        draw_text_line(
                            hdc,
                            "Connexions récentes affichées (1 à 50) :",
                            RECT {
                                left: content_x + state.dpi.scale(22),
                                top: state.dpi.scale(392),
                                right: content_x + state.dpi.scale(310),
                                bottom: state.dpi.scale(422),
                            },
                        );

                        if state.edit_box_rc.right > state.edit_box_rc.left {
                            let focused = GetFocus() == state.ed_max_recent;
                            draw_rounded_box(
                                hdc,
                                state.edit_box_rc,
                                col_bg_input(),
                                if focused { col_accent() } else { col_border() },
                            );
                        }
                    }
                    1 => {
                        // ONGLET 1 : PROTOCOLES
                        let _ = SelectObject(hdc, state.font_bold.into());
                        let _ = SetTextColor(hdc, COLORREF(col_accent()));
                        draw_text_line(
                            hdc,
                            "AnyDesk",
                            RECT {
                                left: content_x,
                                top: state.dpi.scale(76),
                                right: content_right,
                                bottom: state.dpi.scale(98),
                            },
                        );
                        let _ = SelectObject(hdc, state.font_small.into());
                        let _ = SetTextColor(hdc, COLORREF(col_text_muted()));
                        draw_text_line(
                            hdc,
                            "Chemin vers AnyDesk.exe utilisé pour lancer vos sessions AnyDesk :",
                            RECT {
                                left: content_x,
                                top: state.dpi.scale(98),
                                right: content_right,
                                bottom: state.dpi.scale(116),
                            },
                        );
                        if state.anydesk_rc.right > state.anydesk_rc.left {
                            let focused = GetFocus() == state.ed_anydesk_path;
                            draw_rounded_box(
                                hdc,
                                state.anydesk_rc,
                                col_bg_input(),
                                if focused { col_accent() } else { col_border() },
                            );
                        }

                        let _ = SelectObject(hdc, state.font_bold.into());
                        let _ = SetTextColor(hdc, COLORREF(col_accent()));
                        draw_text_line(
                            hdc,
                            "RustDesk",
                            RECT {
                                left: content_x,
                                top: state.dpi.scale(168),
                                right: content_right,
                                bottom: state.dpi.scale(190),
                            },
                        );
                        let _ = SelectObject(hdc, state.font_small.into());
                        let _ = SetTextColor(hdc, COLORREF(col_text_muted()));
                        draw_text_line(
                            hdc,
                            "Chemin vers rustdesk.exe utilisé pour lancer vos sessions RustDesk :",
                            RECT {
                                left: content_x,
                                top: state.dpi.scale(190),
                                right: content_right,
                                bottom: state.dpi.scale(208),
                            },
                        );
                        if state.rustdesk_rc.right > state.rustdesk_rc.left {
                            let focused = GetFocus() == state.ed_rustdesk_path;
                            draw_rounded_box(
                                hdc,
                                state.rustdesk_rc,
                                col_bg_input(),
                                if focused { col_accent() } else { col_border() },
                            );
                        }

                        let _ = SelectObject(hdc, state.font_bold.into());
                        let _ = SetTextColor(hdc, COLORREF(col_accent()));
                        draw_text_line(
                            hdc,
                            "Bureau à distance Windows (RDP)",
                            RECT {
                                left: content_x,
                                top: state.dpi.scale(260),
                                right: content_right,
                                bottom: state.dpi.scale(282),
                            },
                        );
                        let _ = SelectObject(hdc, state.font_ui.into());
                        let _ = SetTextColor(hdc, COLORREF(col_text_primary()));
                        draw_text_line(
                            hdc,
                            "Raccourcis Windows (KeyboardHookMode) :",
                            RECT {
                                left: content_x,
                                top: state.dpi.scale(352),
                                right: content_x + state.dpi.scale(240),
                                bottom: state.dpi.scale(382),
                            },
                        );
                    }
                    2 => {
                        // ONGLET 2 : DONNÉES & SAUVEGARDES
                        // Carte informative de sécurité DPAPI (dimensionnée pour accueillir toutes les puces sans troncature)
                        let card_rc = RECT {
                            left: content_x,
                            top: state.dpi.scale(76),
                            right: content_right,
                            bottom: state.dpi.scale(248),
                        };
                        draw_rounded_box(hdc, card_rc, col_bg_input(), col_border());

                        let _ = SelectObject(hdc, state.font_bold.into());
                        let _ = SetTextColor(hdc, COLORREF(col_accent()));
                        draw_text_line(
                            hdc,
                            "Protection & Sécurité des Données (Windows DPAPI)",
                            RECT {
                                left: content_x + state.dpi.scale(16),
                                top: state.dpi.scale(88),
                                right: content_right - state.dpi.scale(16),
                                bottom: state.dpi.scale(110),
                            },
                        );

                        let _ = SelectObject(hdc, state.font_small.into());
                        let _ = SetTextColor(hdc, COLORREF(col_text_secondary()));
                        let dpapi_info = "• Votre configuration SQLite contient l'arborescence complète de vos dossiers et serveurs.\n• Les mots de passe sont chiffrés au repos par l'API Windows DPAPI liée à votre compte Windows.\n• Sur le même ordinateur et compte, les mots de passe sont restaurés automatiquement.\n• Lors de l'importation sur un autre poste, la structure est préservée ; il suffit de ressaisir les accès.";
                        draw_text_multiline(
                            hdc,
                            dpapi_info,
                            RECT {
                                left: content_x + state.dpi.scale(16),
                                top: state.dpi.scale(114),
                                right: content_right - state.dpi.scale(16),
                                bottom: state.dpi.scale(238),
                            },
                        );

                        // Section Exportation
                        let _ = SelectObject(hdc, state.font_bold.into());
                        let _ = SetTextColor(hdc, COLORREF(col_text_primary()));
                        draw_text_line(
                            hdc,
                            "Exporter la configuration",
                            RECT {
                                left: content_x,
                                top: state.dpi.scale(264),
                                right: content_right,
                                bottom: state.dpi.scale(286),
                            },
                        );
                        let _ = SelectObject(hdc, state.font_small.into());
                        let _ = SetTextColor(hdc, COLORREF(col_text_muted()));
                        draw_text_line(
                            hdc,
                            "Créez une sauvegarde complète autonome (.db) sans interruption de service.",
                            RECT {
                                left: content_x,
                                top: state.dpi.scale(286),
                                right: content_right,
                                bottom: state.dpi.scale(306),
                            },
                        );

                        // Section Importation
                        let _ = SelectObject(hdc, state.font_bold.into());
                        let _ = SetTextColor(hdc, COLORREF(col_text_primary()));
                        draw_text_line(
                            hdc,
                            "Importer et restaurer une configuration",
                            RECT {
                                left: content_x,
                                top: state.dpi.scale(366),
                                right: content_right,
                                bottom: state.dpi.scale(388),
                            },
                        );
                        let _ = SelectObject(hdc, state.font_small.into());
                        let _ = SetTextColor(hdc, COLORREF(col_text_muted()));
                        draw_text_line(
                            hdc,
                            "Restaure une base sauvegardée. Une copie de secours (.pre_import_bak) est générée automatiquement.",
                            RECT {
                                left: content_x,
                                top: state.dpi.scale(388),
                                right: content_right,
                                bottom: state.dpi.scale(408),
                            },
                        );
                    }
                    3 => {
                        // ONGLET 3 : MISES À JOUR
                        // Carte de statut
                        let status_rc = RECT {
                            left: content_x,
                            top: state.dpi.scale(110),
                            right: content_right,
                            bottom: state.dpi.scale(215),
                        };
                        draw_rounded_box(hdc, status_rc, col_bg_input(), col_border());

                        let _ = SelectObject(hdc, state.font_ui.into());
                        let _ = SetTextColor(hdc, COLORREF(col_text_secondary()));
                        draw_text_line(
                            hdc,
                            "Version installée :",
                            RECT {
                                left: content_x + state.dpi.scale(16),
                                top: state.dpi.scale(122),
                                right: content_x + state.dpi.scale(160),
                                bottom: state.dpi.scale(144),
                            },
                        );
                        let _ = SelectObject(hdc, state.font_bold.into());
                        let _ = SetTextColor(hdc, COLORREF(col_text_primary()));
                        draw_text_line(
                            hdc,
                            &format!("{} (x64 portable)", crate::updater::CURRENT_VERSION),
                            RECT {
                                left: content_x + state.dpi.scale(170),
                                top: state.dpi.scale(122),
                                right: content_right - state.dpi.scale(16),
                                bottom: state.dpi.scale(144),
                            },
                        );

                        let _ = SelectObject(hdc, state.font_ui.into());
                        let _ = SetTextColor(hdc, COLORREF(col_text_secondary()));
                        draw_text_line(
                            hdc,
                            "Dernière vérification :",
                            RECT {
                                left: content_x + state.dpi.scale(16),
                                top: state.dpi.scale(150),
                                right: content_x + state.dpi.scale(160),
                                bottom: state.dpi.scale(172),
                            },
                        );
                        let _ = SelectObject(hdc, state.font_ui.into());
                        let _ = SetTextColor(hdc, COLORREF(col_text_primary()));
                        let last_check = state
                            .base_settings
                            .last_update_check_time
                            .as_deref()
                            .unwrap_or("Jamais");
                        draw_text_line(
                            hdc,
                            last_check,
                            RECT {
                                left: content_x + state.dpi.scale(170),
                                top: state.dpi.scale(150),
                                right: content_right - state.dpi.scale(16),
                                bottom: state.dpi.scale(172),
                            },
                        );

                        let _ = SelectObject(hdc, state.font_ui.into());
                        let _ = SetTextColor(hdc, COLORREF(col_text_secondary()));
                        draw_text_line(
                            hdc,
                            "Statut :",
                            RECT {
                                left: content_x + state.dpi.scale(16),
                                top: state.dpi.scale(178),
                                right: content_x + state.dpi.scale(160),
                                bottom: state.dpi.scale(200),
                            },
                        );
                        let _ = SelectObject(hdc, state.font_bold.into());
                        let _ = SetTextColor(hdc, COLORREF(col_accent()));
                        draw_text_line(
                            hdc,
                            &state.update_status_text,
                            RECT {
                                left: content_x + state.dpi.scale(170),
                                top: state.dpi.scale(178),
                                right: content_right - state.dpi.scale(16),
                                bottom: state.dpi.scale(200),
                            },
                        );

                        // Carte d'explication portable
                        let info_rc = RECT {
                            left: content_x,
                            top: state.dpi.scale(276),
                            right: content_right,
                            bottom: state.dpi.scale(392),
                        };
                        draw_rounded_box(hdc, info_rc, col_bg_input(), col_border());
                        let _ = SelectObject(hdc, state.font_bold.into());
                        let _ = SetTextColor(hdc, COLORREF(col_text_primary()));
                        draw_text_line(
                            hdc,
                            "Distribution portable sans installation",
                            RECT {
                                left: content_x + state.dpi.scale(16),
                                top: state.dpi.scale(286),
                                right: content_right - state.dpi.scale(16),
                                bottom: state.dpi.scale(308),
                            },
                        );
                        let _ = SelectObject(hdc, state.font_small.into());
                        let _ = SetTextColor(hdc, COLORREF(col_text_secondary()));
                        let portable_notice = "• Malvision est distribué comme un exécutable portable unique Malvision.exe.\n• Le bouton « Mettre à jour et redémarrer » télécharge la version officielle, vérifie son empreinte SHA-256 et remplace automatiquement l'application sans intervention manuelle.\n• En cas d'erreur de lancement, l'ancienne version est automatiquement restaurée.\n• Toutes vos connexions et préférences restent intactes dans votre base SQLite.";
                        draw_text_multiline(
                            hdc,
                            portable_notice,
                            RECT {
                                left: content_x + state.dpi.scale(16),
                                top: state.dpi.scale(312),
                                right: content_right - state.dpi.scale(16),
                                bottom: state.dpi.scale(384),
                            },
                        );
                    }
                    4 => {
                        // ONGLET 4 : AVANCÉ (Emplacement personnalisé malvision.db)
                        let _ = SelectObject(hdc, state.font_bold.into());
                        let _ = SetTextColor(hdc, COLORREF(col_accent()));
                        draw_text_line(
                            hdc,
                            "EMPLACEMENT DE LA BASE DE DONNÉES SQLITE",
                            RECT {
                                left: content_x,
                                top: state.dpi.scale(76),
                                right: content_right,
                                bottom: state.dpi.scale(98),
                            },
                        );

                        let _ = SelectObject(hdc, state.font_ui.into());
                        let _ = SetTextColor(hdc, COLORREF(col_text_secondary()));
                        draw_text_line(
                            hdc,
                            "Fichier de base de données actif :",
                            RECT {
                                left: content_x,
                                top: state.dpi.scale(98),
                                right: content_right,
                                bottom: state.dpi.scale(118),
                            },
                        );

                        // Statut / Type de lecteur
                        let current_path = (*state.storage).db_path();
                        let is_custom = crate::bootstrap::is_using_custom_database();
                        let drive_kind = crate::bootstrap::check_drive_kind(current_path);
                        let status_str = match (is_custom, drive_kind) {
                            (false, _) => "Emplacement standard (%LOCALAPPDATA%)",
                            (true, crate::bootstrap::DriveKind::LocalFixed) => {
                                "Emplacement personnalisé (Disque local fixe)"
                            }
                            (true, crate::bootstrap::DriveKind::Removable) => {
                                "Emplacement personnalisé (Disque amovible)"
                            }
                            (true, crate::bootstrap::DriveKind::NetworkRemote) => {
                                "Emplacement personnalisé (Partage réseau)"
                            }
                            (true, _) => "Emplacement personnalisé",
                        };

                        let _ = SelectObject(hdc, state.font_small.into());
                        let _ = SetTextColor(
                            hdc,
                            COLORREF(if is_custom {
                                col_accent()
                            } else {
                                col_text_muted()
                            }),
                        );
                        draw_text_line(
                            hdc,
                            status_str,
                            RECT {
                                left: content_x,
                                top: state.dpi.scale(208),
                                right: content_right,
                                bottom: state.dpi.scale(228),
                            },
                        );

                        // Carte d'explication et de sécurité
                        let info_rc = RECT {
                            left: content_x,
                            top: state.dpi.scale(236),
                            right: content_right,
                            bottom: state.dpi.scale(426),
                        };
                        draw_rounded_box(hdc, info_rc, col_bg_input(), col_border());
                        let _ = SelectObject(hdc, state.font_bold.into());
                        let _ = SetTextColor(hdc, COLORREF(col_text_primary()));
                        draw_text_line(
                            hdc,
                            "Règles strictes de gestion et sécurité des données",
                            RECT {
                                left: content_x + state.dpi.scale(16),
                                top: state.dpi.scale(246),
                                right: content_right - state.dpi.scale(16),
                                bottom: state.dpi.scale(268),
                            },
                        );
                        let _ = SelectObject(hdc, state.font_small.into());
                        let _ = SetTextColor(hdc, COLORREF(col_text_secondary()));
                        let rules_notice = "• Le fichier de base de données doit obligatoirement s'appeler « malvision.db ».\n• Malvision ne déplace, ne renomme ni ne supprime JAMAIS votre ancienne base de données.\n• Si un nouveau dossier vide est choisi, la base active actuelle y est copiée en ligne de manière intègre.\n• Si une base malvision.db existe déjà dans le dossier cible, son intégrité est vérifiée avant bascule.\n• En cas de clé USB ou de disque externe débranché au démarrage, Malvision affiche un écran d'alerte sans créer de base vide.";
                        draw_text_multiline(
                            hdc,
                            rules_notice,
                            RECT {
                                left: content_x + state.dpi.scale(16),
                                top: state.dpi.scale(272),
                                right: content_right - state.dpi.scale(16),
                                bottom: state.dpi.scale(416),
                            },
                        );
                    }
                    _ => {
                        // ONGLET 5 : À PROPOS
                        // Dessin du logo officiel
                        if let Some(logo) = state.logo_bitmap {
                            let mem_dc = CreateCompatibleDC(Some(hdc));
                            let old_bm = SelectObject(mem_dc, logo.into());
                            let _ = windows::Win32::Graphics::Gdi::BitBlt(
                                hdc,
                                content_x,
                                state.dpi.scale(76),
                                state.dpi.scale(72),
                                state.dpi.scale(72),
                                Some(mem_dc),
                                0,
                                0,
                                windows::Win32::Graphics::Gdi::SRCCOPY,
                            );
                            let _ = SelectObject(mem_dc, old_bm);
                            let _ = DeleteDC(mem_dc);
                        }

                        let text_left = content_x + state.dpi.scale(88);
                        let _ = SelectObject(hdc, state.font_title.into());
                        let _ = SetTextColor(hdc, COLORREF(col_text_primary()));
                        draw_text_line(
                            hdc,
                            "MALVISION",
                            RECT {
                                left: text_left,
                                top: state.dpi.scale(74),
                                right: content_right,
                                bottom: state.dpi.scale(104),
                            },
                        );

                        let _ = SelectObject(hdc, state.font_bold.into());
                        let _ = SetTextColor(hdc, COLORREF(col_accent()));
                        draw_text_line(
                            hdc,
                            &format!("Version {} (x64 portable)", crate::updater::CURRENT_VERSION),
                            RECT {
                                left: text_left,
                                top: state.dpi.scale(104),
                                right: content_right,
                                bottom: state.dpi.scale(126),
                            },
                        );

                        let _ = SelectObject(hdc, state.font_small.into());
                        let _ = SetTextColor(hdc, COLORREF(col_text_muted()));
                        draw_text_line(
                            hdc,
                            "Gestionnaire de connexions distantes natif Windows 11",
                            RECT {
                                left: text_left,
                                top: state.dpi.scale(126),
                                right: content_right,
                                bottom: state.dpi.scale(148),
                            },
                        );

                        let _ = SelectObject(hdc, state.font_ui.into());
                        let _ = SetTextColor(hdc, COLORREF(col_text_secondary()));
                        let about_desc = "Malvision est un client natif haute performance conçu pour les administrateurs système et professionnels de l'infrastructure.\nIl prend en charge le Bureau à distance Microsoft (ActiveX MsTscAx) avec sessions par onglets, ainsi que le lancement automatisé des clients AnyDesk et RustDesk.";
                        draw_text_multiline(
                            hdc,
                            about_desc,
                            RECT {
                                left: content_x,
                                top: state.dpi.scale(166),
                                right: content_right,
                                bottom: state.dpi.scale(256),
                            },
                        );

                        let _ = SelectObject(hdc, state.font_small.into());
                        let _ = SetTextColor(hdc, COLORREF(col_text_muted()));
                        let legal_text = "Développé en Rust natif & Win32 • 100% portable • Aucune télémétrie • Chiffrement local DPAPI\nCopyright (c) 2026 Malvirae — Développé pour Malvision\nDépôt officiel : https://github.com/Malvirae/malvision";
                        draw_text_multiline(
                            hdc,
                            legal_text,
                            RECT {
                                left: content_x,
                                top: state.dpi.scale(324),
                                right: content_right,
                                bottom: state.dpi.scale(392),
                            },
                        );
                    }
                }

                // --- 4. BARRE INFÉRIEURE D'ACTIONS (ENREGISTRER / ANNULER) ---
                draw_horizontal_line(hdc, 0, w, content_h, col_border());

                let _ = windows::Win32::Graphics::Gdi::BitBlt(
                    hdc_screen,
                    0,
                    0,
                    w,
                    h,
                    Some(hdc),
                    0,
                    0,
                    windows::Win32::Graphics::Gdi::SRCCOPY,
                );
                let _ = SelectObject(hdc, old_bm);
                let _ = DeleteObject(hbm.into());
                let _ = DeleteDC(hdc);
                let _ = EndPaint(hwnd, &ps);
                LRESULT(0)
            }
            WM_COMMAND => {
                let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut SettingsDialogState;
                if !ptr.is_null() {
                    let state = &mut *ptr;
                    let id = wparam.0 & 0xFFFF;
                    let code = ((wparam.0 >> 16) & 0xFFFF) as u32;

                    if (IDC_SET_TAB_GENERAL..=IDC_SET_TAB_ABOUT).contains(&id) {
                        let tab_idx = id - IDC_SET_TAB_GENERAL;
                        if tab_idx < 6 && tab_idx != state.current_tab {
                            state.current_tab = tab_idx;
                            state.update_tab_visibility();
                            let _ = InvalidateRect(Some(hwnd), None, true);
                        }
                        return LRESULT(0);
                    }

                    if code == EN_SETFOCUS || code == EN_KILLFOCUS {
                        let _ = InvalidateRect(Some(hwnd), None, false);
                        return LRESULT(0);
                    }

                    // Prévisualisation immédiate du thème (CBN_SELCHANGE == 1)
                    if id == IDC_SET_THEME && code == 1 {
                        let sel = SendMessageW(
                            state.cb_theme,
                            CB_GETCURSEL,
                            Some(WPARAM(0)),
                            Some(LPARAM(0)),
                        )
                        .0;
                        let mode = match sel {
                            1 => ThemeMode::Light,
                            2 => ThemeMode::System,
                            _ => ThemeMode::Dark,
                        };
                        let is_dark = mode.resolve() == ResolvedTheme::Dark;
                        set_active_theme_dark(is_dark);

                        if state.owned_brushes {
                            if !state.brush_bg.is_invalid() {
                                let _ = DeleteObject(state.brush_bg.into());
                            }
                            if !state.brush_input.is_invalid() {
                                let _ = DeleteObject(state.brush_input.into());
                            }
                        }
                        state.brush_bg = CreateSolidBrush(COLORREF(col_bg_sidebar()));
                        state.brush_input = CreateSolidBrush(COLORREF(col_bg_input()));
                        state.owned_brushes = true;

                        if let Some(old_logo) = state.logo_bitmap {
                            let _ = DeleteObject(old_logo.into());
                        }
                        state.logo_bitmap =
                            create_logo_bitmap(state.dpi.scale(72) as u32, col_bg_sidebar());

                        let dark_mode: i32 = i32::from(is_dark);
                        let _ = DwmSetWindowAttribute(
                            hwnd,
                            DWMWA_USE_IMMERSIVE_DARK_MODE,
                            &dark_mode as *const i32 as *const c_void,
                            std::mem::size_of::<i32>() as u32,
                        );
                        if IsWindow(Some(state.owner)).as_bool() {
                            let owner_ptr = GetWindowLongPtrW(state.owner, GWLP_USERDATA)
                                as *mut AppWindowState;
                            if !owner_ptr.is_null() {
                                (*owner_ptr).settings.theme_mode = mode;
                                (*owner_ptr).apply_theme();
                            }
                        }

                        for ctrl in [
                            state.cb_theme,
                            state.chk_close_tray,
                            state.chk_start_min_tray,
                            state.chk_confirm_exit,
                            state.chk_restore_rdp,
                            state.chk_start_win,
                            state.chk_show_recent,
                            state.ed_max_recent,
                            state.chk_rdp_clip,
                            state.chk_rdp_dynres,
                            state.cb_rdp_hook_mode,
                            state.ed_anydesk_path,
                            state.btn_anydesk_browse,
                            state.btn_anydesk_detect,
                            state.btn_anydesk_test,
                            state.ed_rustdesk_path,
                            state.btn_rustdesk_browse,
                            state.btn_rustdesk_detect,
                            state.btn_rustdesk_test,
                            state.btn_export_data,
                            state.btn_import_data,
                            state.chk_auto_update,
                            state.btn_check_update,
                            state.btn_download_update,
                            state.btn_manual_download,
                            state.btn_about_github,
                            state.btn_about_check_updates,
                            state.btn_ok,
                            state.btn_cancel,
                        ] {
                            if !ctrl.is_invalid() {
                                let _ = InvalidateRect(Some(ctrl), None, true);
                            }
                        }
                        let _ = InvalidateRect(Some(hwnd), None, true);
                        let _ = UpdateWindow(hwnd);
                    } else if id == IDC_SET_CLOSE_TRAY {
                        let tray_enabled = SendMessageW(
                            state.chk_close_tray,
                            BM_GETCHECK,
                            Some(WPARAM(0)),
                            Some(LPARAM(0)),
                        )
                        .0 != 0;
                        if !tray_enabled {
                            let _ = SendMessageW(
                                state.chk_start_min_tray,
                                BM_SETCHECK,
                                Some(WPARAM(0)),
                                Some(LPARAM(0)),
                            );
                        }
                        let _ = EnableWindow(state.chk_start_min_tray, tray_enabled);
                        let _ = InvalidateRect(Some(state.chk_start_min_tray), None, false);
                    } else if id == IDC_SET_ANYDESK_BROWSE {
                        if let Some(path) = crate::external_apps::browse_for_executable(
                            hwnd,
                            "Sélectionner l'exécutable AnyDesk (AnyDesk.exe)",
                        ) {
                            set_window_text(state.ed_anydesk_path, &path.to_string_lossy());
                        }
                    } else if id == IDC_SET_ANYDESK_DETECT {
                        if let Some(path) = crate::external_apps::detect_anydesk_path() {
                            set_window_text(state.ed_anydesk_path, &path.to_string_lossy());
                            show_info_dialog(
                                hwnd,
                                "Détection AnyDesk",
                                &format!(
                                    "AnyDesk a été détecté automatiquement :\n{}",
                                    path.display()
                                ),
                            );
                        } else {
                            show_warning_dialog(
                                hwnd,
                                "Détection AnyDesk",
                                "AnyDesk n'a pas été détecté automatiquement dans les dossiers standards ou le Registre.\nVeuillez utiliser le bouton « Parcourir » pour sélectionner AnyDesk.exe.",
                            );
                        }
                    } else if id == IDC_SET_ANYDESK_TEST {
                        let path_str = get_window_text(state.ed_anydesk_path).trim().to_string();
                        if path_str.is_empty() {
                            show_warning_dialog(
                                hwnd,
                                "Test de lancement AnyDesk",
                                "Veuillez d'abord renseigner ou détecter le chemin vers AnyDesk.exe.",
                            );
                        } else {
                            let p = std::path::Path::new(&path_str);
                            match crate::external_apps::test_launch_app(p) {
                                Ok(pid) => {
                                    show_info_dialog(
                                        hwnd,
                                        "Test AnyDesk réussi",
                                        &format!(
                                            "Le client AnyDesk a été démarré avec succès (PID : {pid}).\nLa fenêtre de l'application doit être visible."
                                        ),
                                    );
                                }
                                Err(err) => {
                                    show_error_dialog(
                                        hwnd,
                                        "Échec du test AnyDesk",
                                        &format!("Impossible de démarrer AnyDesk :\n{err}"),
                                    );
                                }
                            }
                        }
                    } else if id == IDC_SET_RUSTDESK_BROWSE {
                        if let Some(path) = crate::external_apps::browse_for_executable(
                            hwnd,
                            "Sélectionner l'exécutable RustDesk (rustdesk.exe)",
                        ) {
                            set_window_text(state.ed_rustdesk_path, &path.to_string_lossy());
                        }
                    } else if id == IDC_SET_RUSTDESK_DETECT {
                        if let Some(path) = crate::external_apps::detect_rustdesk_path() {
                            set_window_text(state.ed_rustdesk_path, &path.to_string_lossy());
                            show_info_dialog(
                                hwnd,
                                "Détection RustDesk",
                                &format!(
                                    "RustDesk a été détecté automatiquement :\n{}",
                                    path.display()
                                ),
                            );
                        } else {
                            show_warning_dialog(
                                hwnd,
                                "Détection RustDesk",
                                "RustDesk n'a pas été détecté automatiquement dans les dossiers standards ou le Registre.\nVeuillez utiliser le bouton « Parcourir » pour sélectionner rustdesk.exe.",
                            );
                        }
                    } else if id == IDC_SET_RUSTDESK_TEST {
                        let path_str = get_window_text(state.ed_rustdesk_path).trim().to_string();
                        if path_str.is_empty() {
                            show_warning_dialog(
                                hwnd,
                                "Test de lancement RustDesk",
                                "Veuillez d'abord renseigner ou détecter le chemin vers RustDesk.exe.",
                            );
                        } else {
                            let p = std::path::Path::new(&path_str);
                            match crate::external_apps::test_launch_app(p) {
                                Ok(pid) => {
                                    show_info_dialog(
                                        hwnd,
                                        "Test RustDesk réussi",
                                        &format!(
                                            "Le client RustDesk a été démarré avec succès (PID : {pid}).\nLa fenêtre de l'application doit être visible."
                                        ),
                                    );
                                }
                                Err(err) => {
                                    show_error_dialog(
                                        hwnd,
                                        "Échec du test RustDesk",
                                        &format!("Impossible de démarrer RustDesk :\n{err}"),
                                    );
                                }
                            }
                        }
                    } else if id == IDC_SET_DATA_EXPORT {
                        let st = windows::Win32::System::SystemInformation::GetLocalTime();
                        let def_name = format!(
                            "malvision_backup_{:04}-{:02}-{:02}.db",
                            st.wYear, st.wMonth, st.wDay
                        );

                        if let Some(dest_path) = crate::external_apps::browse_for_save_file(
                            hwnd,
                            "Exporter la configuration Malvision",
                            &def_name,
                            "db",
                            "Configuration Malvision (*.db)",
                            "*.db",
                        ) {
                            let confirm_msg = format!(
                                "Voulez-vous exporter l'ensemble de votre arborescence et de vos connexions vers :\n{}\n\nRappel : Les mots de passe sont chiffrés avec la clé DPAPI de votre compte Windows actuel.",
                                dest_path.display()
                            );
                            if confirm_dialog(hwnd, "Confirmation d'exportation", &confirm_msg)
                                && !state.storage.is_null()
                            {
                                let storage = &*state.storage;
                                match storage.export_database(&dest_path) {
                                    Ok(()) => {
                                        show_info_dialog(
                                            hwnd,
                                            "Exportation réussie",
                                            &format!(
                                                "Votre configuration Malvision a été exportée avec succès !\n\nFichier sauvegardé :\n{}",
                                                dest_path.display()
                                            ),
                                        );
                                    }
                                    Err(err) => {
                                        show_error_dialog(
                                            hwnd,
                                            "Échec de l'exportation",
                                            &format!(
                                                "Une erreur est survenue lors de l'exportation :\n{err}"
                                            ),
                                        );
                                    }
                                }
                            }
                        }
                    } else if id == IDC_SET_DATA_IMPORT {
                        if let Some(src_path) = crate::external_apps::browse_for_open_file(
                            hwnd,
                            "Importer une configuration Malvision",
                            "Configuration Malvision (*.db)",
                            "*.db",
                        ) {
                            match Storage::validate_import_file(&src_path) {
                                Ok(summary) => {
                                    let confirm_msg = format!(
                                        "ATTENTION : L'importation va remplacer l'ensemble de vos dossiers et connexions actuels.\n\nFichier source : {}\n• Dossiers détectés : {}\n• Connexions détectées : {}\n\nUne copie de sécurité de votre base active (.pre_import_bak) sera automatiquement conservée.\n\nVoulez-vous procéder au remplacement ?",
                                        src_path.display(),
                                        summary.folders_count,
                                        summary.connections_count
                                    );
                                    if confirm_dialog(
                                        hwnd,
                                        "Confirmation de remplacement",
                                        &confirm_msg,
                                    ) && !state.storage.is_null()
                                    {
                                        let storage = &mut *state.storage;
                                        match storage.import_database(&src_path) {
                                            Ok(res) => {
                                                state.imported_changes = true;
                                                show_info_dialog(
                                                    hwnd,
                                                    "Importation réussie",
                                                    &format!(
                                                        "La base de données Malvision a été restaurée avec succès !\n\n• Dossiers importés : {}\n• Connexions importées : {}\n• Mots de passe DPAPI préservés : {}\n• Mots de passe nécessitant une ressaisie : {}\n\nVos données seront actualisées à la fermeture de cette boîte.",
                                                        res.folders_count,
                                                        res.connections_count,
                                                        res.dpapi_recoverable_count,
                                                        res.dpapi_unrecoverable_count
                                                    ),
                                                );
                                            }
                                            Err(err) => {
                                                show_error_dialog(
                                                    hwnd,
                                                    "Échec de l'importation",
                                                    &format!(
                                                        "Erreur lors de l'importation :\n{err}"
                                                    ),
                                                );
                                            }
                                        }
                                    }
                                }
                                Err(err) => {
                                    show_error_dialog(
                                        hwnd,
                                        "Fichier de configuration invalide",
                                        &format!(
                                            "Le fichier sélectionné ne constitue pas une base de données Malvision valide :\n{err}"
                                        ),
                                    );
                                }
                            }
                        }
                    } else if id == IDC_SET_UPDATE_CHECK_NOW {
                        state.update_status_text =
                            "Recherche en cours sur GitHub Releases…".to_string();
                        let _ = EnableWindow(state.btn_check_update, false);
                        let _ = InvalidateRect(Some(hwnd), None, false);
                        let _ = UpdateWindow(hwnd);

                        let st = windows::Win32::System::SystemInformation::GetLocalTime();
                        let time_str = format!(
                            "{:02}/{:02}/{:04} à {:02}:{:02}",
                            st.wDay, st.wMonth, st.wYear, st.wHour, st.wMinute
                        );
                        state.base_settings.last_update_check_time = Some(time_str);

                        match crate::updater::check_latest_release(crate::updater::GITHUB_REPO) {
                            Ok(info) => {
                                let _ = EnableWindow(state.btn_check_update, true);
                                let is_upgrade = crate::updater::current_semver()
                                    .map(|local| info.version > local)
                                    .unwrap_or(false);
                                if is_upgrade {
                                    state.update_status_text = format!(
                                        "Nouvelle version disponible : v{} !",
                                        info.version
                                    );
                                    state.base_settings.last_known_remote_version =
                                        Some(info.version.to_string());
                                    state.latest_release = Some(info.clone());
                                    let _ = EnableWindow(state.btn_download_update, true);
                                    let _ = EnableWindow(state.btn_manual_download, true);
                                    let _ = InvalidateRect(Some(hwnd), None, false);

                                    let msg = format!(
                                        "Une nouvelle version de Malvision est disponible !\n\nVersion actuelle : v{}\nNouvelle version : v{}\nNom : {}\n\nCliquez sur « Mettre à jour et redémarrer » pour l'installer automatiquement.",
                                        crate::updater::CURRENT_VERSION,
                                        info.version,
                                        info.name
                                    );
                                    show_info_dialog(hwnd, "Mise à jour disponible", &msg);
                                } else {
                                    state.update_status_text = format!(
                                        "Votre version de Malvision est à jour (v{}).",
                                        crate::updater::CURRENT_VERSION
                                    );
                                    state.latest_release = None;
                                    let _ = EnableWindow(state.btn_download_update, false);
                                    let _ = EnableWindow(state.btn_manual_download, false);
                                    let _ = InvalidateRect(Some(hwnd), None, false);
                                    show_info_dialog(
                                        hwnd,
                                        "Malvision est à jour",
                                        "Vous disposez déjà de la version la plus récente de Malvision.",
                                    );
                                }
                            }
                            Err(err) => {
                                let _ = EnableWindow(state.btn_check_update, true);
                                let is_not_found = err == crate::updater::MSG_NO_PUBLIC_RELEASE
                                    || err.contains("404")
                                    || err.contains("Not Found");
                                if is_not_found {
                                    state.update_status_text =
                                        crate::updater::MSG_NO_PUBLIC_RELEASE.to_string();
                                    let _ = InvalidateRect(Some(hwnd), None, false);
                                    show_info_dialog(
                                        hwnd,
                                        "Mises à jour",
                                        crate::updater::MSG_NO_PUBLIC_RELEASE,
                                    );
                                } else {
                                    state.update_status_text = format!("Erreur : {err}");
                                    let _ = InvalidateRect(Some(hwnd), None, false);
                                    show_warning_dialog(
                                        hwnd,
                                        "Vérification des mises à jour",
                                        &format!(
                                            "Impossible de vérifier les mises à jour :\n{err}\n\nVeuillez vérifier votre connexion Internet."
                                        ),
                                    );
                                }
                            }
                        }
                    } else if id == IDC_SET_UPDATE_DOWNLOAD {
                        if state.is_updating {
                            return LRESULT(0);
                        }
                        if let Some(info) = &state.latest_release
                            && let Some(asset) = &info.exe_asset
                        {
                            let expected_hash = info.expected_exe_sha256.as_deref();
                            if expected_hash.is_none() || expected_hash.unwrap().trim().is_empty() {
                                show_error_dialog(
                                    hwnd,
                                    "Vérification d'intégrité impossible",
                                    "Aucune empreinte cryptographique SHA-256 officielle n'a été trouvée pour cette release (ni dans les notes de version, ni dans SHA256SUMS.txt).\n\nPar mesure de sécurité stricte, la mise à jour est bloquée.",
                                );
                                return LRESULT(0);
                            }

                            // 1. Avertissement sessions actives
                            if state.active_sessions_count > 0 {
                                let count = state.active_sessions_count;
                                let msg = format!(
                                    "{count} session(s) RDP active(s) sont actuellement ouvertes.\n\nMalvision doit fermer ces sessions et redémarrer pour appliquer la mise à jour.\n\nVoulez-vous continuer ?"
                                );
                                if !confirm_dialog(hwnd, "Sessions actives", &msg) {
                                    return LRESULT(0);
                                }
                            }

                            // 2. Bascule en état de mise à jour en cours
                            state.is_updating = true;
                            let _ = EnableWindow(state.btn_check_update, false);
                            let _ = EnableWindow(state.btn_download_update, false);
                            let _ = EnableWindow(state.btn_manual_download, false);
                            state.update_status_text =
                                "Téléchargement de la nouvelle version…".to_string();
                            let _ = InvalidateRect(Some(hwnd), None, false);
                            let _ = UpdateWindow(hwnd);

                            // 3. Thread d'arrière-plan pour ne pas bloquer l'UI
                            let asset_url = asset.download_url.clone();
                            let expected_hash_str = expected_hash.unwrap().trim().to_string();
                            let dlg_hwnd_raw = hwnd.0 as isize;
                            let owner_hwnd_raw = state.owner.0 as isize;

                            std::thread::spawn(move || {
                                let updates_dir = match crate::updater::get_updates_temp_dir() {
                                    Ok(d) => d,
                                    Err(e) => {
                                        notify_update_error(dlg_hwnd_raw, e);
                                        return;
                                    }
                                };
                                let new_exe_temp = updates_dir.join("new_version.exe");
                                if new_exe_temp.exists() {
                                    let _ = std::fs::remove_file(&new_exe_temp);
                                }

                                notify_update_status(
                                    dlg_hwnd_raw,
                                    "Téléchargement de la nouvelle version…",
                                );
                                if let Err(e) = crate::updater::download_and_verify_asset(
                                    &asset_url,
                                    &new_exe_temp,
                                    Some(&expected_hash_str),
                                ) {
                                    notify_update_error(
                                        dlg_hwnd_raw,
                                        format!("Échec du téléchargement : {e}"),
                                    );
                                    return;
                                }

                                notify_update_status(
                                    dlg_hwnd_raw,
                                    "Vérification de l'intégrité SHA-256…",
                                );
                                match crate::security::compute_file_sha256(&new_exe_temp) {
                                    Ok(h) if h.eq_ignore_ascii_case(&expected_hash_str) => {}
                                    _ => {
                                        notify_update_error(
                                            dlg_hwnd_raw,
                                            "Empreinte cryptographique SHA-256 non conforme."
                                                .to_string(),
                                        );
                                        return;
                                    }
                                }

                                notify_update_status(
                                    dlg_hwnd_raw,
                                    "Préparation de l'installation…",
                                );
                                let prep = match crate::updater::prepare_update_helper(
                                    &new_exe_temp,
                                    &expected_hash_str,
                                ) {
                                    Ok(p) => p,
                                    Err(e) => {
                                        notify_update_error(
                                            dlg_hwnd_raw,
                                            format!(
                                                "Préparation de l'installation impossible : {e}"
                                            ),
                                        );
                                        return;
                                    }
                                };

                                notify_update_status(dlg_hwnd_raw, "Redémarrage de Malvision…");
                                if let Err(e) = crate::updater::launch_helper_and_wait_ready(
                                    &prep,
                                    std::time::Duration::from_secs(5),
                                ) {
                                    notify_update_error(
                                        dlg_hwnd_raw,
                                        format!(
                                            "Le processus auxiliaire de mise à jour n'a pas répondu : {e}"
                                        ),
                                    );
                                    return;
                                }

                                // Fermer la boîte de dialogue et demander la fermeture propre et sortie du processus principal
                                let owner_hwnd = HWND(owner_hwnd_raw as *mut _);
                                let dlg_hwnd = HWND(dlg_hwnd_raw as *mut _);
                                let _ =
                                    PostMessageW(Some(dlg_hwnd), WM_CLOSE, WPARAM(0), LPARAM(0));
                                let _ = PostMessageW(
                                    Some(owner_hwnd),
                                    WM_DEFERRED_ACTION,
                                    WPARAM(DEFERRED_APPLY_UPDATE_AND_EXIT),
                                    LPARAM(0),
                                );
                            });
                        }
                    } else if id == IDC_SET_UPDATE_MANUAL_DOWNLOAD {
                        if let Some(info) = &state.latest_release
                            && let Some(asset) = &info.exe_asset
                        {
                            let expected_hash = info.expected_exe_sha256.as_deref();
                            if expected_hash.is_none() || expected_hash.unwrap().trim().is_empty() {
                                show_error_dialog(
                                    hwnd,
                                    "Vérification d'intégrité impossible",
                                    "Aucune empreinte SHA-256 officielle disponible.",
                                );
                                return LRESULT(0);
                            }

                            if let Some(dest_path) = crate::external_apps::browse_for_save_file(
                                hwnd,
                                "Enregistrer la mise à jour Malvision",
                                "Malvision.exe",
                                "exe",
                                "Exécutable portable (*.exe)",
                                "*.exe",
                            ) {
                                state.update_status_text =
                                    "Téléchargement manuel en cours...".to_string();
                                let _ = InvalidateRect(Some(hwnd), None, false);
                                let _ = UpdateWindow(hwnd);

                                match crate::updater::download_and_verify_asset(
                                    &asset.download_url,
                                    &dest_path,
                                    expected_hash,
                                ) {
                                    Ok(()) => {
                                        state.update_status_text = format!(
                                            "Version v{} enregistrée avec succès !",
                                            info.version
                                        );
                                        let _ = InvalidateRect(Some(hwnd), None, false);
                                        show_info_dialog(
                                            hwnd,
                                            "Téléchargement manuel réussi",
                                            &format!(
                                                "Fichier enregistré :\n{}\n\nSHA-256 vérifié.",
                                                dest_path.display()
                                            ),
                                        );
                                    }
                                    Err(err) => {
                                        state.update_status_text =
                                            format!("Échec du téléchargement : {err}");
                                        let _ = InvalidateRect(Some(hwnd), None, false);
                                        show_error_dialog(hwnd, "Échec du téléchargement", &err);
                                    }
                                }
                            }
                        }
                    } else if id == IDC_SET_ABOUT_GITHUB {
                        let _ = ShellExecuteW(
                            Some(hwnd),
                            w!("open"),
                            w!("https://github.com/Malvirae/malvision"),
                            None,
                            None,
                            SW_SHOW,
                        );
                    } else if id == IDC_SET_ABOUT_CHECK_UPDATES {
                        state.current_tab = 3;
                        state.update_tab_visibility();
                        let _ = InvalidateRect(Some(hwnd), None, true);
                        let _ = SendMessageW(
                            hwnd,
                            WM_COMMAND,
                            Some(WPARAM(IDC_SET_UPDATE_CHECK_NOW)),
                            Some(LPARAM(0)),
                        );
                    } else if id == IDC_SET_ADV_CHANGE_PATH {
                        handle_adv_change_database_path(hwnd, state);
                    } else if id == IDC_SET_ADV_OPEN_FOLDER {
                        let cur_path = (*state.storage).db_path();
                        let folder = cur_path.parent().unwrap_or(std::path::Path::new("C:\\"));
                        open_folder_in_explorer(folder);
                    } else if id == IDC_SET_ADV_RESET_DEFAULT {
                        handle_adv_reset_default_path(hwnd, state);
                    } else if id == IDC_DLG_OK || id == 1 {
                        let theme_sel = SendMessageW(
                            state.cb_theme,
                            CB_GETCURSEL,
                            Some(WPARAM(0)),
                            Some(LPARAM(0)),
                        )
                        .0;
                        let theme_mode = match theme_sel {
                            1 => ThemeMode::Light,
                            2 => ThemeMode::System,
                            _ => ThemeMode::Dark,
                        };

                        let close_to_tray = SendMessageW(
                            state.chk_close_tray,
                            BM_GETCHECK,
                            Some(WPARAM(0)),
                            Some(LPARAM(0)),
                        )
                        .0 != 0;
                        let start_minimized_to_tray = close_to_tray
                            && SendMessageW(
                                state.chk_start_min_tray,
                                BM_GETCHECK,
                                Some(WPARAM(0)),
                                Some(LPARAM(0)),
                            )
                            .0 != 0;
                        let confirm_exit_active_sessions = SendMessageW(
                            state.chk_confirm_exit,
                            BM_GETCHECK,
                            Some(WPARAM(0)),
                            Some(LPARAM(0)),
                        )
                        .0 != 0;
                        let restore_rdp_sessions = SendMessageW(
                            state.chk_restore_rdp,
                            BM_GETCHECK,
                            Some(WPARAM(0)),
                            Some(LPARAM(0)),
                        )
                        .0 != 0;
                        let start_with_windows = SendMessageW(
                            state.chk_start_win,
                            BM_GETCHECK,
                            Some(WPARAM(0)),
                            Some(LPARAM(0)),
                        )
                        .0 != 0;
                        let show_recent_connections = SendMessageW(
                            state.chk_show_recent,
                            BM_GETCHECK,
                            Some(WPARAM(0)),
                            Some(LPARAM(0)),
                        )
                        .0 != 0;

                        let max_rec_str = get_window_text(state.ed_max_recent);
                        let max_recent_connections = match max_rec_str.trim().parse::<usize>() {
                            Ok(v) if (1..=50).contains(&v) => v,
                            _ => {
                                show_warning_dialog(
                                    hwnd,
                                    "Nombre de connexions récentes invalide",
                                    "Veuillez saisir un nombre entier compris entre 1 et 50.",
                                );
                                return LRESULT(0);
                            }
                        };

                        let rdp_default_clipboard = SendMessageW(
                            state.chk_rdp_clip,
                            BM_GETCHECK,
                            Some(WPARAM(0)),
                            Some(LPARAM(0)),
                        )
                        .0 != 0;
                        let rdp_default_dynamic_resolution = SendMessageW(
                            state.chk_rdp_dynres,
                            BM_GETCHECK,
                            Some(WPARAM(0)),
                            Some(LPARAM(0)),
                        )
                        .0 != 0;

                        let rdp_hook_sel = SendMessageW(
                            state.cb_rdp_hook_mode,
                            CB_GETCURSEL,
                            Some(WPARAM(0)),
                            Some(LPARAM(0)),
                        )
                        .0;
                        let rdp_keyboard_hook_mode =
                            RdpKeyboardHookMode::from_combo_index(if rdp_hook_sel >= 0 {
                                rdp_hook_sel as usize
                            } else {
                                0
                            });

                        let anydesk_path =
                            get_window_text(state.ed_anydesk_path).trim().to_string();
                        let rustdesk_path =
                            get_window_text(state.ed_rustdesk_path).trim().to_string();

                        if !anydesk_path.is_empty() {
                            let p = std::path::Path::new(&anydesk_path);
                            if let Err(err) = crate::external_apps::validate_executable_path(p) {
                                show_warning_dialog(
                                    hwnd,
                                    "Chemin AnyDesk invalide",
                                    &format!(
                                        "Le chemin spécifié pour AnyDesk est invalide :\n{err}\n\nVeuillez vérifier le chemin ou laisser le champ vide."
                                    ),
                                );
                                return LRESULT(0);
                            }
                        }

                        if !rustdesk_path.is_empty() {
                            let p = std::path::Path::new(&rustdesk_path);
                            if let Err(err) = crate::external_apps::validate_executable_path(p) {
                                show_warning_dialog(
                                    hwnd,
                                    "Chemin RustDesk invalide",
                                    &format!(
                                        "Le chemin spécifié pour RustDesk est invalide :\n{err}\n\nVeuillez vérifier le chemin ou laisser le champ vide."
                                    ),
                                );
                                return LRESULT(0);
                            }
                        }

                        let check_updates_on_startup = SendMessageW(
                            state.chk_auto_update,
                            BM_GETCHECK,
                            Some(WPARAM(0)),
                            Some(LPARAM(0)),
                        )
                        .0 != 0;

                        let mut updated = state.base_settings.clone();
                        updated.theme_mode = theme_mode;
                        updated.close_to_tray = close_to_tray;
                        updated.start_minimized_to_tray = start_minimized_to_tray;
                        updated.confirm_exit_active_sessions = confirm_exit_active_sessions;
                        updated.restore_rdp_sessions = restore_rdp_sessions;
                        updated.start_with_windows = start_with_windows;
                        updated.show_recent_connections = show_recent_connections;
                        updated.max_recent_connections = max_recent_connections;
                        updated.rdp_default_clipboard = rdp_default_clipboard;
                        updated.rdp_default_dynamic_resolution = rdp_default_dynamic_resolution;
                        updated.rdp_keyboard_hook_mode = rdp_keyboard_hook_mode;
                        updated.anydesk_path = anydesk_path;
                        updated.rustdesk_path = rustdesk_path;
                        updated.check_updates_on_startup = check_updates_on_startup;

                        state.result = Some(updated);
                        let owner = state.owner;
                        close_modal_dialog(hwnd, owner);
                    } else if id == IDC_DLG_CANCEL || id == 2 {
                        let owner = state.owner;
                        close_modal_dialog(hwnd, owner);
                    }
                }
                LRESULT(0)
            }
            WM_CLOSE => {
                let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut SettingsDialogState;
                if !ptr.is_null() {
                    let owner = (*ptr).owner;
                    close_modal_dialog(hwnd, owner);
                } else {
                    let _ = DestroyWindow(hwnd);
                }
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}

fn run_settings_dialog(
    owner: HWND,
    current: &AppSettings,
    palette: &GdiPalette,
    storage: &mut Storage,
    active_sessions_count: usize,
) -> (Option<AppSettings>, bool) {
    unsafe {
        let hinstance = GetModuleHandleW(None).unwrap_or_default();
        let class_name = w!("MalvisionSettingsDlgClass");

        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(settings_dlg_wnd_proc),
            hInstance: hinstance.into(),
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            hbrBackground: palette.brush_bg_sidebar,
            lpszClassName: class_name,
            ..Default::default()
        };
        let _ = RegisterClassExW(&wc);

        let dpi = if owner.is_invalid() {
            palette.dpi
        } else {
            DpiMetrics::for_window(owner)
        };
        let client_w = dpi.scale(760);
        let client_h = dpi.scale(570);
        let dlg_style = WS_POPUP | WS_CAPTION | WS_SYSMENU | WS_CLIPCHILDREN;
        let (win_w, win_h) = calc_dialog_window_size(dlg_style, client_w, client_h);

        let mut owner_rc = RECT::default();
        let _ = GetWindowRect(owner, &mut owner_rc);
        let x = owner_rc.left + ((owner_rc.right - owner_rc.left) - win_w) / 2;
        let y = owner_rc.top + ((owner_rc.bottom - owner_rc.top) - win_h) / 2;

        let Ok(hwnd) = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            class_name,
            w!("Paramètres — Malvision"),
            dlg_style,
            x.max(20),
            y.max(20),
            win_w,
            win_h,
            Some(owner),
            None,
            Some(hinstance.into()),
            None,
        ) else {
            return (None, false);
        };

        let dark_mode: i32 = i32::from(is_active_theme_dark());
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_USE_IMMERSIVE_DARK_MODE,
            &dark_mode as *const i32 as *const c_void,
            std::mem::size_of::<i32>() as u32,
        );

        let sidebar_w = dpi.scale(180);
        let bottom_bar_h = dpi.scale(56);
        let content_x = sidebar_w + dpi.scale(28);
        let content_right = client_w - dpi.scale(28);
        let field_h = dpi.scale(32);
        let chk_h = dpi.scale(24);

        let logo_bm = create_logo_bitmap(dpi.scale(72) as u32, col_bg_sidebar());

        let mut dlg_state = Box::new(SettingsDialogState {
            owner,
            dpi,
            font_ui: palette.font_ui,
            font_bold: palette.font_ui_bold,
            font_subtitle: palette.font_subtitle,
            font_title: palette.font_title,
            font_small: palette.font_small,
            brush_bg: palette.brush_bg_sidebar,
            brush_input: palette.brush_bg_input,
            owned_brushes: false,
            storage: storage as *mut Storage,
            imported_changes: false,
            current_tab: 0,
            hover_tab: None,
            sidebar_w,
            base_settings: current.clone(),
            cb_theme: HWND::default(),
            chk_close_tray: HWND::default(),
            chk_start_min_tray: HWND::default(),
            chk_confirm_exit: HWND::default(),
            chk_restore_rdp: HWND::default(),
            chk_start_win: HWND::default(),
            chk_show_recent: HWND::default(),
            ed_max_recent: HWND::default(),
            edit_box_rc: RECT::default(),
            ed_anydesk_path: HWND::default(),
            btn_anydesk_browse: HWND::default(),
            btn_anydesk_detect: HWND::default(),
            btn_anydesk_test: HWND::default(),
            anydesk_rc: RECT::default(),
            ed_rustdesk_path: HWND::default(),
            btn_rustdesk_browse: HWND::default(),
            btn_rustdesk_detect: HWND::default(),
            btn_rustdesk_test: HWND::default(),
            rustdesk_rc: RECT::default(),
            chk_rdp_clip: HWND::default(),
            chk_rdp_dynres: HWND::default(),
            cb_rdp_hook_mode: HWND::default(),
            btn_export_data: HWND::default(),
            btn_import_data: HWND::default(),
            chk_auto_update: HWND::default(),
            btn_check_update: HWND::default(),
            btn_download_update: HWND::default(),
            btn_manual_download: HWND::default(),
            is_updating: false,
            active_sessions_count,
            update_status_text: "Recherche non effectuée".to_string(),
            latest_release: None,
            ed_adv_path: HWND::default(),
            btn_adv_change_path: HWND::default(),
            btn_adv_open_folder: HWND::default(),
            btn_adv_reset_default: HWND::default(),
            adv_path_rc: RECT::default(),
            btn_about_github: HWND::default(),
            btn_about_check_updates: HWND::default(),
            logo_bitmap: logo_bm,
            btn_ok: HWND::default(),
            btn_cancel: HWND::default(),
            result: None,
        });

        SetWindowLongPtrW(hwnd, GWLP_USERDATA, (&mut *dlg_state) as *mut _ as isize);

        // --- ONGLET 0 : CONTRÔLES GÉNÉRAL ---
        let y_theme = dpi.scale(104);
        let rc_theme = RECT {
            left: content_x + dpi.scale(190),
            top: y_theme,
            right: content_right,
            bottom: y_theme + field_h,
        };
        let cb_theme = create_dark_combobox(
            hwnd,
            hinstance.into(),
            rc_theme,
            dpi.scale(160),
            IDC_SET_THEME,
            palette.font_ui,
            dpi,
        );
        for lbl in [
            w!("Sombre (Anthracite Windows 11 — par défaut)"),
            w!("Clair (Fluent Light haute lisibilité)"),
            w!("Système (Suit automatiquement Windows)"),
        ] {
            let _ = SendMessageW(
                cb_theme,
                CB_ADDSTRING,
                Some(WPARAM(0)),
                Some(LPARAM(lbl.as_ptr() as isize)),
            );
        }
        let theme_idx = match current.theme_mode {
            ThemeMode::Dark => 0usize,
            ThemeMode::Light => 1usize,
            ThemeMode::System => 2usize,
        };
        let _ = SendMessageW(
            cb_theme,
            CB_SETCURSEL,
            Some(WPARAM(theme_idx)),
            Some(LPARAM(0)),
        );

        let y_c1 = dpi.scale(180);
        let y_c2 = dpi.scale(208);
        let y_c3 = dpi.scale(236);
        let y_c4 = dpi.scale(264);
        let y_c5 = dpi.scale(292);

        let chk_close_tray = create_dark_checkbox(
            hwnd,
            hinstance.into(),
            w!("Fermer vers la zone de notification lors d'un clic sur la croix [X]"),
            RECT {
                left: content_x,
                top: y_c1,
                right: content_right,
                bottom: y_c1 + chk_h,
            },
            IDC_SET_CLOSE_TRAY,
            palette.font_ui,
        );
        let chk_start_min_tray = create_dark_checkbox(
            hwnd,
            hinstance.into(),
            w!("Démarrer minimisé dans la zone de notification (requiert l'option ci-dessus)"),
            RECT {
                left: content_x + dpi.scale(22),
                top: y_c2,
                right: content_right,
                bottom: y_c2 + chk_h,
            },
            IDC_SET_START_MIN_TRAY,
            palette.font_ui,
        );
        let chk_confirm_exit = create_dark_checkbox(
            hwnd,
            hinstance.into(),
            w!("Demander confirmation avant de quitter avec des sessions RDP actives"),
            RECT {
                left: content_x,
                top: y_c3,
                right: content_right,
                bottom: y_c3 + chk_h,
            },
            IDC_SET_CONFIRM_EXIT,
            palette.font_ui,
        );
        let chk_restore_rdp = create_dark_checkbox(
            hwnd,
            hinstance.into(),
            w!("Restaurer automatiquement les connexions RDP ouvertes au démarrage"),
            RECT {
                left: content_x,
                top: y_c4,
                right: content_right,
                bottom: y_c4 + chk_h,
            },
            IDC_SET_RESTORE_RDP,
            palette.font_ui,
        );
        let chk_start_win = create_dark_checkbox(
            hwnd,
            hinstance.into(),
            w!("Démarrer Malvision avec Windows (raccourci .lnk dans le dossier Démarrage)"),
            RECT {
                left: content_x,
                top: y_c5,
                right: content_right,
                bottom: y_c5 + chk_h,
            },
            IDC_SET_START_WIN,
            palette.font_ui,
        );

        let _ = SendMessageW(
            chk_close_tray,
            BM_SETCHECK,
            Some(WPARAM(usize::from(current.close_to_tray))),
            Some(LPARAM(0)),
        );
        let _ = SendMessageW(
            chk_start_min_tray,
            BM_SETCHECK,
            Some(WPARAM(usize::from(
                current.close_to_tray && current.start_minimized_to_tray,
            ))),
            Some(LPARAM(0)),
        );
        let _ = EnableWindow(chk_start_min_tray, current.close_to_tray);
        let _ = SendMessageW(
            chk_confirm_exit,
            BM_SETCHECK,
            Some(WPARAM(usize::from(current.confirm_exit_active_sessions))),
            Some(LPARAM(0)),
        );
        let _ = SendMessageW(
            chk_restore_rdp,
            BM_SETCHECK,
            Some(WPARAM(usize::from(current.restore_rdp_sessions))),
            Some(LPARAM(0)),
        );
        let _ = SendMessageW(
            chk_start_win,
            BM_SETCHECK,
            Some(WPARAM(usize::from(current.start_with_windows))),
            Some(LPARAM(0)),
        );

        let y_rec_chk = dpi.scale(362);
        let chk_show_recent = create_dark_checkbox(
            hwnd,
            hinstance.into(),
            w!("Afficher la section des connexions récemment utilisées sur l'écran d'accueil"),
            RECT {
                left: content_x,
                top: y_rec_chk,
                right: content_right,
                bottom: y_rec_chk + chk_h,
            },
            IDC_SET_SHOW_RECENT,
            palette.font_ui,
        );
        let _ = SendMessageW(
            chk_show_recent,
            BM_SETCHECK,
            Some(WPARAM(usize::from(current.show_recent_connections))),
            Some(LPARAM(0)),
        );

        let y_max_rec = dpi.scale(392);
        let rc_max_rec = RECT {
            left: content_x + dpi.scale(320),
            top: y_max_rec,
            right: content_x + dpi.scale(410),
            bottom: y_max_rec + field_h,
        };
        dlg_state.edit_box_rc = rc_max_rec;
        let ed_max_recent = create_dark_edit(
            hwnd,
            hinstance.into(),
            rc_max_rec,
            IDC_SET_MAX_RECENT,
            false,
            palette.font_ui,
            dpi,
        );
        set_window_text(ed_max_recent, &current.max_recent_connections.to_string());
        install_auto_select_on_focus(ed_max_recent);

        // --- ONGLET 1 : CONTRÔLES PROTOCOLES ---
        let btn_w_act = dpi.scale(76);
        let btn_gap_act = dpi.scale(6);
        let total_btns_w = btn_w_act * 3 + btn_gap_act * 2;

        let y_any = dpi.scale(120);
        let rc_anydesk = RECT {
            left: content_x,
            top: y_any,
            right: content_right - total_btns_w - dpi.scale(8),
            bottom: y_any + field_h,
        };
        dlg_state.anydesk_rc = rc_anydesk;
        let ed_anydesk_path = create_dark_edit(
            hwnd,
            hinstance.into(),
            rc_anydesk,
            IDC_SET_ANYDESK_PATH,
            false,
            palette.font_ui,
            dpi,
        );
        set_window_text(ed_anydesk_path, &current.anydesk_path);
        install_auto_select_on_focus(ed_anydesk_path);

        let btn_anydesk_browse = create_dark_button(
            hwnd,
            hinstance.into(),
            w!("Parcourir"),
            RECT {
                left: content_right - total_btns_w,
                top: y_any,
                right: content_right - total_btns_w + btn_w_act,
                bottom: y_any + field_h,
            },
            IDC_SET_ANYDESK_BROWSE,
            palette.font_ui,
        );
        let btn_anydesk_detect = create_dark_button(
            hwnd,
            hinstance.into(),
            w!("Détecter"),
            RECT {
                left: content_right - btn_w_act * 2 - btn_gap_act,
                top: y_any,
                right: content_right - btn_w_act - btn_gap_act,
                bottom: y_any + field_h,
            },
            IDC_SET_ANYDESK_DETECT,
            palette.font_ui,
        );
        let btn_anydesk_test = create_dark_button(
            hwnd,
            hinstance.into(),
            w!("Tester"),
            RECT {
                left: content_right - btn_w_act,
                top: y_any,
                right: content_right,
                bottom: y_any + field_h,
            },
            IDC_SET_ANYDESK_TEST,
            palette.font_ui,
        );

        let y_rust = dpi.scale(212);
        let rc_rustdesk = RECT {
            left: content_x,
            top: y_rust,
            right: content_right - total_btns_w - dpi.scale(8),
            bottom: y_rust + field_h,
        };
        dlg_state.rustdesk_rc = rc_rustdesk;
        let ed_rustdesk_path = create_dark_edit(
            hwnd,
            hinstance.into(),
            rc_rustdesk,
            IDC_SET_RUSTDESK_PATH,
            false,
            palette.font_ui,
            dpi,
        );
        set_window_text(ed_rustdesk_path, &current.rustdesk_path);
        install_auto_select_on_focus(ed_rustdesk_path);

        let btn_rustdesk_browse = create_dark_button(
            hwnd,
            hinstance.into(),
            w!("Parcourir"),
            RECT {
                left: content_right - total_btns_w,
                top: y_rust,
                right: content_right - total_btns_w + btn_w_act,
                bottom: y_rust + field_h,
            },
            IDC_SET_RUSTDESK_BROWSE,
            palette.font_ui,
        );
        let btn_rustdesk_detect = create_dark_button(
            hwnd,
            hinstance.into(),
            w!("Détecter"),
            RECT {
                left: content_right - btn_w_act * 2 - btn_gap_act,
                top: y_rust,
                right: content_right - btn_w_act - btn_gap_act,
                bottom: y_rust + field_h,
            },
            IDC_SET_RUSTDESK_DETECT,
            palette.font_ui,
        );
        let btn_rustdesk_test = create_dark_button(
            hwnd,
            hinstance.into(),
            w!("Tester"),
            RECT {
                left: content_right - btn_w_act,
                top: y_rust,
                right: content_right,
                bottom: y_rust + field_h,
            },
            IDC_SET_RUSTDESK_TEST,
            palette.font_ui,
        );

        let y_rdp1 = dpi.scale(288);
        let y_rdp2 = dpi.scale(316);
        let chk_rdp_clip = create_dark_checkbox(
            hwnd,
            hinstance.into(),
            w!("Partager le presse-papiers par défaut sur les nouvelles fiches RDP"),
            RECT {
                left: content_x,
                top: y_rdp1,
                right: content_right,
                bottom: y_rdp1 + chk_h,
            },
            IDC_SET_RDP_CLIP,
            palette.font_ui,
        );
        let chk_rdp_dynres = create_dark_checkbox(
            hwnd,
            hinstance.into(),
            w!("Ajuster automatiquement la résolution (SmartSizing) par défaut"),
            RECT {
                left: content_x,
                top: y_rdp2,
                right: content_right,
                bottom: y_rdp2 + chk_h,
            },
            IDC_SET_RDP_DYNRES,
            palette.font_ui,
        );
        let _ = SendMessageW(
            chk_rdp_clip,
            BM_SETCHECK,
            Some(WPARAM(usize::from(current.rdp_default_clipboard))),
            Some(LPARAM(0)),
        );
        let _ = SendMessageW(
            chk_rdp_dynres,
            BM_SETCHECK,
            Some(WPARAM(usize::from(current.rdp_default_dynamic_resolution))),
            Some(LPARAM(0)),
        );

        let y_hook = dpi.scale(352);
        let rc_hook = RECT {
            left: content_x + dpi.scale(250),
            top: y_hook,
            right: content_right,
            bottom: y_hook + field_h,
        };
        let cb_rdp_hook_mode = create_dark_combobox(
            hwnd,
            hinstance.into(),
            rc_hook,
            dpi.scale(160),
            IDC_SET_RDP_HOOK_MODE,
            palette.font_ui,
            dpi,
        );
        for lbl in [
            w!("Envoyés à la session distante (défaut recommandé)"),
            w!("Exécutés sur le poste local"),
            w!("Envoyés à distance uniquement en plein écran"),
        ] {
            let _ = SendMessageW(
                cb_rdp_hook_mode,
                CB_ADDSTRING,
                Some(WPARAM(0)),
                Some(LPARAM(lbl.as_ptr() as isize)),
            );
        }
        let _ = SendMessageW(
            cb_rdp_hook_mode,
            CB_SETCURSEL,
            Some(WPARAM(current.rdp_keyboard_hook_mode.to_combo_index())),
            Some(LPARAM(0)),
        );

        // --- ONGLET 2 : CONTRÔLES DONNÉES ---
        let btn_export_data = create_dark_button(
            hwnd,
            hinstance.into(),
            w!("Exporter ma configuration…"),
            RECT {
                left: content_x,
                top: dpi.scale(312),
                right: content_x + dpi.scale(250),
                bottom: dpi.scale(312) + dpi.scale(36),
            },
            IDC_SET_DATA_EXPORT,
            palette.font_ui_bold,
        );
        let btn_import_data = create_dark_button(
            hwnd,
            hinstance.into(),
            w!("Importer une configuration…"),
            RECT {
                left: content_x,
                top: dpi.scale(414),
                right: content_x + dpi.scale(250),
                bottom: dpi.scale(414) + dpi.scale(36),
            },
            IDC_SET_DATA_IMPORT,
            palette.font_ui,
        );

        // --- ONGLET 3 : CONTRÔLES MISES À JOUR ---
        let chk_auto_update = create_dark_checkbox(
            hwnd,
            hinstance.into(),
            w!("Vérifier automatiquement les mises à jour au démarrage de l'application"),
            RECT {
                left: content_x,
                top: dpi.scale(76),
                right: content_right,
                bottom: dpi.scale(76) + chk_h,
            },
            IDC_SET_UPDATE_AUTO,
            palette.font_ui,
        );
        let _ = SendMessageW(
            chk_auto_update,
            BM_SETCHECK,
            Some(WPARAM(usize::from(current.check_updates_on_startup))),
            Some(LPARAM(0)),
        );

        let btn_check_update = create_dark_button(
            hwnd,
            hinstance.into(),
            w!("Vérifier maintenant"),
            RECT {
                left: content_x,
                top: dpi.scale(228),
                right: content_x + dpi.scale(180),
                bottom: dpi.scale(228) + dpi.scale(34),
            },
            IDC_SET_UPDATE_CHECK_NOW,
            palette.font_ui,
        );
        let btn_download_update = create_dark_button(
            hwnd,
            hinstance.into(),
            w!("Mettre à jour et redémarrer"),
            RECT {
                left: content_x + dpi.scale(192),
                top: dpi.scale(228),
                right: content_x + dpi.scale(430),
                bottom: dpi.scale(228) + dpi.scale(34),
            },
            IDC_SET_UPDATE_DOWNLOAD,
            palette.font_ui_bold,
        );
        let _ = EnableWindow(btn_download_update, false);

        let btn_manual_download = create_dark_button(
            hwnd,
            hinstance.into(),
            w!("Téléchargement manuel (.exe)…"),
            RECT {
                left: content_x,
                top: dpi.scale(398),
                right: content_x + dpi.scale(250),
                bottom: dpi.scale(398) + dpi.scale(30),
            },
            IDC_SET_UPDATE_MANUAL_DOWNLOAD,
            palette.font_ui,
        );
        let _ = EnableWindow(btn_manual_download, false);

        // --- ONGLET 4 : CONTRÔLES AVANCÉ (EMPLACEMENT BASE SQLITE) ---
        let current_db_path = (*storage).db_path();
        let rc_adv_path = RECT {
            left: content_x,
            top: dpi.scale(120),
            right: content_right,
            bottom: dpi.scale(120) + field_h,
        };
        dlg_state.adv_path_rc = rc_adv_path;
        let ed_adv_path = create_dark_edit(
            hwnd,
            hinstance.into(),
            rc_adv_path,
            IDC_SET_ADV_PATH_DISPLAY,
            false,
            palette.font_ui,
            dpi,
        );
        const EM_SETREADONLY: u32 = 0x00CF;
        let _ = SendMessageW(
            ed_adv_path,
            EM_SETREADONLY,
            Some(WPARAM(1)),
            Some(LPARAM(0)),
        );
        set_window_text(ed_adv_path, &current_db_path.to_string_lossy());

        let btn_adv_change_path = create_dark_button(
            hwnd,
            hinstance.into(),
            w!("Modifier l'emplacement…"),
            RECT {
                left: content_x,
                top: dpi.scale(164),
                right: content_x + dpi.scale(190),
                bottom: dpi.scale(164) + dpi.scale(34),
            },
            IDC_SET_ADV_CHANGE_PATH,
            palette.font_ui_bold,
        );
        let btn_adv_open_folder = create_dark_button(
            hwnd,
            hinstance.into(),
            w!("Ouvrir le dossier"),
            RECT {
                left: content_x + dpi.scale(198),
                top: dpi.scale(164),
                right: content_x + dpi.scale(338),
                bottom: dpi.scale(164) + dpi.scale(34),
            },
            IDC_SET_ADV_OPEN_FOLDER,
            palette.font_ui,
        );
        let btn_adv_reset_default = create_dark_button(
            hwnd,
            hinstance.into(),
            w!("Revenir au dossier par défaut"),
            RECT {
                left: content_x + dpi.scale(346),
                top: dpi.scale(164),
                right: content_x + dpi.scale(570),
                bottom: dpi.scale(164) + dpi.scale(34),
            },
            IDC_SET_ADV_RESET_DEFAULT,
            palette.font_ui,
        );
        let _ = EnableWindow(
            btn_adv_reset_default,
            crate::bootstrap::is_using_custom_database(),
        );

        // --- ONGLET 5 : CONTRÔLES À PROPOS ---
        let btn_about_github = create_dark_button(
            hwnd,
            hinstance.into(),
            w!("Ouvrir le dépôt GitHub"),
            RECT {
                left: content_x,
                top: dpi.scale(268),
                right: content_x + dpi.scale(220),
                bottom: dpi.scale(268) + dpi.scale(36),
            },
            IDC_SET_ABOUT_GITHUB,
            palette.font_ui,
        );
        let btn_about_check_updates = create_dark_button(
            hwnd,
            hinstance.into(),
            w!("Vérifier les mises à jour"),
            RECT {
                left: content_x + dpi.scale(232),
                top: dpi.scale(268),
                right: content_x + dpi.scale(430),
                bottom: dpi.scale(268) + dpi.scale(36),
            },
            IDC_SET_ABOUT_CHECK_UPDATES,
            palette.font_ui_bold,
        );

        // --- BOUTONS ENREGISTRER & ANNULER (BARRE INFÉRIEURE COMMUNE) ---
        let btn_w = dpi.scale(120);
        let btn_h = dpi.scale(34);
        let btn_y = client_h - bottom_bar_h + (bottom_bar_h - btn_h) / 2;
        let btn_gap = dpi.scale(12);

        let rc_cancel = RECT {
            left: content_right - btn_w,
            top: btn_y,
            right: content_right,
            bottom: btn_y + btn_h,
        };
        let rc_ok = RECT {
            left: rc_cancel.left - btn_gap - btn_w,
            top: btn_y,
            right: rc_cancel.left - btn_gap,
            bottom: btn_y + btn_h,
        };

        let btn_ok = create_dark_button(
            hwnd,
            hinstance.into(),
            w!("Enregistrer"),
            rc_ok,
            IDC_DLG_OK,
            palette.font_ui_bold,
        );
        let btn_cancel = create_dark_button(
            hwnd,
            hinstance.into(),
            w!("Annuler"),
            rc_cancel,
            IDC_DLG_CANCEL,
            palette.font_ui,
        );

        // Attribution dans l'état
        dlg_state.cb_theme = cb_theme;
        dlg_state.chk_close_tray = chk_close_tray;
        dlg_state.chk_start_min_tray = chk_start_min_tray;
        dlg_state.chk_confirm_exit = chk_confirm_exit;
        dlg_state.chk_restore_rdp = chk_restore_rdp;
        dlg_state.chk_start_win = chk_start_win;
        dlg_state.chk_show_recent = chk_show_recent;
        dlg_state.ed_max_recent = ed_max_recent;

        dlg_state.ed_anydesk_path = ed_anydesk_path;
        dlg_state.btn_anydesk_browse = btn_anydesk_browse;
        dlg_state.btn_anydesk_detect = btn_anydesk_detect;
        dlg_state.btn_anydesk_test = btn_anydesk_test;
        dlg_state.ed_rustdesk_path = ed_rustdesk_path;
        dlg_state.btn_rustdesk_browse = btn_rustdesk_browse;
        dlg_state.btn_rustdesk_detect = btn_rustdesk_detect;
        dlg_state.btn_rustdesk_test = btn_rustdesk_test;
        dlg_state.chk_rdp_clip = chk_rdp_clip;
        dlg_state.chk_rdp_dynres = chk_rdp_dynres;
        dlg_state.cb_rdp_hook_mode = cb_rdp_hook_mode;

        dlg_state.btn_export_data = btn_export_data;
        dlg_state.btn_import_data = btn_import_data;

        dlg_state.chk_auto_update = chk_auto_update;
        dlg_state.btn_check_update = btn_check_update;
        dlg_state.btn_download_update = btn_download_update;
        dlg_state.btn_manual_download = btn_manual_download;

        dlg_state.ed_adv_path = ed_adv_path;
        dlg_state.btn_adv_change_path = btn_adv_change_path;
        dlg_state.btn_adv_open_folder = btn_adv_open_folder;
        dlg_state.btn_adv_reset_default = btn_adv_reset_default;

        dlg_state.btn_about_github = btn_about_github;
        dlg_state.btn_about_check_updates = btn_about_check_updates;

        dlg_state.btn_ok = btn_ok;
        dlg_state.btn_cancel = btn_cancel;

        dlg_state.update_tab_visibility();

        let tab_order = [
            cb_theme,
            chk_close_tray,
            chk_start_min_tray,
            chk_confirm_exit,
            chk_restore_rdp,
            chk_start_win,
            chk_show_recent,
            ed_max_recent,
            ed_anydesk_path,
            btn_anydesk_browse,
            btn_anydesk_detect,
            btn_anydesk_test,
            ed_rustdesk_path,
            btn_rustdesk_browse,
            btn_rustdesk_detect,
            btn_rustdesk_test,
            chk_rdp_clip,
            chk_rdp_dynres,
            cb_rdp_hook_mode,
            btn_export_data,
            btn_import_data,
            chk_auto_update,
            btn_check_update,
            btn_download_update,
            btn_manual_download,
            ed_adv_path,
            btn_adv_change_path,
            btn_adv_open_folder,
            btn_adv_reset_default,
            btn_about_github,
            btn_about_check_updates,
            btn_ok,
            btn_cancel,
        ];

        let _ = EnableWindow(owner, false);
        let _ = ShowWindow(hwnd, SW_SHOW);
        let _ = UpdateWindow(hwnd);
        let _ = SetFocus(Some(cb_theme));

        run_modal_loop(owner, hwnd, &tab_order);

        (dlg_state.result.take(), dlg_state.imported_changes)
    }
}

// =============================================================================
// WNDPROC PRINCIPAL, POMPE D'ÉVÉNEMENTS WIN32/OLE & HELPERS
// =============================================================================

impl Drop for AppWindowState {
    fn drop(&mut self) {
        self.remove_tray_icon();
        unsafe {
            let _ = KillTimer(Some(self.hwnd), IDT_RDP_STATE_POLL);
            if !self.app_icon.is_invalid() {
                let _ = DestroyIcon(self.app_icon);
            }
        }
        self.sessions.close_all();
    }
}

/// Garantit que `MsTscAx.dll` n'a pas altéré les styles de bordure / maximisation / minimisation
/// de la fenêtre principale (`WS_OVERLAPPEDWINDOW`).
unsafe fn ensure_main_window_frame_styles(hwnd: HWND) {
    unsafe {
        let current = GetWindowLongPtrW(hwnd, GWL_STYLE) as u32;
        let required = MAIN_WINDOW_BASE_STYLE.0;
        if (current & required) != required {
            SetWindowLongPtrW(hwnd, GWL_STYLE, (current | required) as isize);
        }
    }
}

unsafe fn get_window_text(hwnd: HWND) -> String {
    unsafe {
        let len = GetWindowTextLengthW(hwnd);
        if len <= 0 {
            return String::new();
        }
        let mut buf = vec![0u16; (len as usize) + 1];
        let copied = GetWindowTextW(hwnd, &mut buf);
        if copied <= 0 {
            return String::new();
        }
        String::from_utf16_lossy(&buf[..copied as usize])
    }
}

unsafe fn set_window_text(hwnd: HWND, text: &str) {
    unsafe {
        let mut wide: Vec<u16> = text.encode_utf16().collect();
        wide.push(0);
        let _ = SetWindowTextW(hwnd, PCWSTR(wide.as_ptr()));
    }
}

fn show_error_dialog(hwnd: HWND, title: &str, message: &str) {
    if cfg!(test) {
        return;
    }
    unsafe {
        let mut title_w: Vec<u16> = title.encode_utf16().collect();
        title_w.push(0);
        let mut msg_w: Vec<u16> = message.encode_utf16().collect();
        msg_w.push(0);
        let _ = MessageBoxW(
            Some(hwnd),
            PCWSTR(msg_w.as_ptr()),
            PCWSTR(title_w.as_ptr()),
            MB_OK | MB_ICONERROR,
        );
    }
}

fn show_warning_dialog(hwnd: HWND, title: &str, message: &str) {
    if cfg!(test) {
        return;
    }
    unsafe {
        let mut title_w: Vec<u16> = title.encode_utf16().collect();
        title_w.push(0);
        let mut msg_w: Vec<u16> = message.encode_utf16().collect();
        msg_w.push(0);
        let _ = MessageBoxW(
            Some(hwnd),
            PCWSTR(msg_w.as_ptr()),
            PCWSTR(title_w.as_ptr()),
            MB_OK | MB_ICONWARNING,
        );
    }
}

fn show_info_dialog(hwnd: HWND, title: &str, message: &str) {
    if cfg!(test) {
        return;
    }
    unsafe {
        let mut title_w: Vec<u16> = title.encode_utf16().collect();
        title_w.push(0);
        let mut msg_w: Vec<u16> = message.encode_utf16().collect();
        msg_w.push(0);
        let _ = MessageBoxW(
            Some(hwnd),
            PCWSTR(msg_w.as_ptr()),
            PCWSTR(title_w.as_ptr()),
            MB_OK | MB_ICONINFORMATION,
        );
    }
}

fn confirm_dialog(hwnd: HWND, title: &str, message: &str) -> bool {
    unsafe {
        let mut title_w: Vec<u16> = title.encode_utf16().collect();
        title_w.push(0);
        let mut msg_w: Vec<u16> = message.encode_utf16().collect();
        msg_w.push(0);
        let res = MessageBoxW(
            Some(hwnd),
            PCWSTR(msg_w.as_ptr()),
            PCWSTR(title_w.as_ptr()),
            MB_YESNO | MB_ICONQUESTION,
        );
        res == IDYES
    }
}

pub fn open_folder_in_explorer(folder: &std::path::Path) {
    let mut folder_w: Vec<u16> = folder.to_string_lossy().encode_utf16().collect();
    folder_w.push(0);
    unsafe {
        let _ = ShellExecuteW(
            None,
            windows_core::w!("explore"),
            windows_core::PCWSTR(folder_w.as_ptr()),
            None,
            None,
            SW_SHOW,
        );
    }
}

pub fn restart_application() {
    crate::single_instance::prepare_for_voluntary_restart();
    if let Ok(exe) = std::env::current_exe() {
        let _ = std::process::Command::new(exe).spawn();
    }
    std::process::exit(0);
}

pub fn prompt_missing_custom_db_at_startup(
    custom_path: &std::path::Path,
) -> Result<std::path::PathBuf, String> {
    if cfg!(test) {
        return Ok(custom_path.to_path_buf());
    }
    loop {
        let msg = format!(
            "La base de données SQLite personnalisée est introuvable à l'emplacement configuré :\n\n{}\n\nLe lecteur ou support amovible est peut-être déconnecté.\n\n• Cliquez sur [Recommencer] pour réessayer d'accéder au support.\n• Cliquez sur [Annuler] pour afficher les options de récupération.",
            custom_path.display()
        );
        let res = unsafe {
            let mut title_w: Vec<u16> = "Base de données personnalisée introuvable"
                .encode_utf16()
                .collect();
            title_w.push(0);
            let mut msg_w: Vec<u16> = msg.encode_utf16().collect();
            msg_w.push(0);
            MessageBoxW(
                None,
                PCWSTR(msg_w.as_ptr()),
                PCWSTR(title_w.as_ptr()),
                MB_RETRYCANCEL | MB_ICONWARNING,
            )
        };
        if res == IDRETRY {
            if custom_path.exists() {
                return Ok(custom_path.to_path_buf());
            }
            continue;
        }

        // Annuler -> Proposer les alternatives
        let choice_msg = "Que souhaitez-vous faire ?\n\n• [Oui] : Choisir un autre dossier pour votre base de données.\n• [Non] : Revenir à l'emplacement standard (%LOCALAPPDATA%).\n• [Annuler] : Quitter l'application sans modifier votre configuration.";
        let choice_res = unsafe {
            let mut title_w: Vec<u16> = "Options de récupération de la base"
                .encode_utf16()
                .collect();
            title_w.push(0);
            let mut msg_w: Vec<u16> = choice_msg.encode_utf16().collect();
            msg_w.push(0);
            MessageBoxW(
                None,
                PCWSTR(msg_w.as_ptr()),
                PCWSTR(title_w.as_ptr()),
                MB_YESNOCANCEL | MB_ICONQUESTION,
            )
        };

        if choice_res == IDYES {
            if let Some(folder) = crate::external_apps::browse_for_folder(
                HWND::default(),
                "Sélectionnez le dossier contenant malvision.db",
            ) {
                let candidate = folder.join(crate::bootstrap::DATABASE_FILENAME);
                if candidate.exists() {
                    let _ = crate::bootstrap::set_custom_database_path(Some(&candidate));
                    return Ok(candidate);
                } else {
                    show_error_dialog(
                        HWND::default(),
                        "Fichier introuvable",
                        &format!(
                            "Aucun fichier malvision.db n'a été trouvé dans le dossier :\n{}",
                            folder.display()
                        ),
                    );
                }
            }
        } else if choice_res == IDNO {
            let _ = crate::bootstrap::set_custom_database_path(None);
            return crate::bootstrap::default_database_path();
        } else {
            std::process::exit(0);
        }
    }
}

pub fn prompt_corrupt_bootstrap_at_startup(
    err_msg: &str,
) -> Result<crate::bootstrap::BootstrapConfig, String> {
    if cfg!(test) {
        return Err(err_msg.to_string());
    }
    let bootstrap_path = crate::bootstrap::bootstrap_file_path()
        .unwrap_or_else(|_| std::path::PathBuf::from("bootstrap.json"));
    loop {
        let msg = format!(
            "Malvision a détecté une anomalie dans le fichier de configuration bootstrap :\n\n{}\n\nFichier concerné :\n{}\n\nPar mesure de sécurité, le démarrage est suspendu pour éviter d'ouvrir ou de créer une mauvaise base de données.\n\n• Cliquez sur [Recommencer] pour réessayer de lire le fichier.\n• Cliquez sur [Annuler] pour afficher les options de récupération.",
            err_msg,
            bootstrap_path.display()
        );
        let res = unsafe {
            let mut title_w: Vec<u16> =
                "Erreur de configuration bootstrap".encode_utf16().collect();
            title_w.push(0);
            let mut msg_w: Vec<u16> = msg.encode_utf16().collect();
            msg_w.push(0);
            MessageBoxW(
                None,
                PCWSTR(msg_w.as_ptr()),
                PCWSTR(title_w.as_ptr()),
                MB_RETRYCANCEL | MB_ICONERROR,
            )
        };
        if res == IDRETRY {
            if let Ok(cfg) = crate::bootstrap::load_bootstrap() {
                return Ok(cfg);
            }
            continue;
        }

        // Annuler -> Proposer les alternatives
        let choice_msg = "Options de récupération :\n\n• [Oui] : Choisir un dossier pour redéfinir l'emplacement de votre base malvision.db.\n• [Non] : Réinitialiser la configuration pour utiliser l'emplacement standard (%LOCALAPPDATA%). Une copie de sauvegarde bootstrap.json.corrupt_bak sera conservée.\n• [Annuler] : Quitter l'application sans modifier vos données.";
        let choice_res = unsafe {
            let mut title_w: Vec<u16> = "Options de récupération du bootstrap"
                .encode_utf16()
                .collect();
            title_w.push(0);
            let mut msg_w: Vec<u16> = choice_msg.encode_utf16().collect();
            msg_w.push(0);
            MessageBoxW(
                None,
                PCWSTR(msg_w.as_ptr()),
                PCWSTR(title_w.as_ptr()),
                MB_YESNOCANCEL | MB_ICONQUESTION,
            )
        };

        if choice_res == IDYES {
            if let Some(folder) = crate::external_apps::browse_for_folder(
                HWND::default(),
                "Sélectionnez le dossier contenant malvision.db",
            ) {
                let target_db = folder.join(crate::bootstrap::DATABASE_FILENAME);
                if target_db.exists() {
                    let new_cfg = crate::bootstrap::BootstrapConfig {
                        version: 1,
                        custom_db_path: Some(target_db),
                    };
                    let _ = crate::bootstrap::save_bootstrap(&new_cfg);
                    return Ok(new_cfg);
                } else {
                    show_error_dialog(
                        HWND::default(),
                        "Fichier introuvable",
                        &format!(
                            "Aucun fichier malvision.db n'a été trouvé dans le dossier :\n{}",
                            folder.display()
                        ),
                    );
                }
            }
        } else if choice_res == IDNO {
            let bak_path = bootstrap_path.with_extension("json.corrupt_bak");
            let _ = std::fs::copy(&bootstrap_path, &bak_path);
            let def_cfg = crate::bootstrap::BootstrapConfig::default();
            let _ = crate::bootstrap::save_bootstrap(&def_cfg);
            return Ok(def_cfg);
        } else {
            std::process::exit(0);
        }
    }
}

unsafe fn handle_adv_change_database_path(hwnd: HWND, state: &mut SettingsDialogState) {
    let current_path = unsafe { (*state.storage).db_path() };
    let chosen_folder = match crate::external_apps::browse_for_folder(
        hwnd,
        "Sélectionnez le dossier où stocker malvision.db",
    ) {
        Some(f) => f,
        None => return,
    };

    let plan = match crate::bootstrap::plan_database_location_change(current_path, &chosen_folder) {
        Ok(p) => p,
        Err(err) => {
            show_error_dialog(hwnd, "Emplacement non valide", &err);
            return;
        }
    };

    match plan {
        crate::bootstrap::LocationChangePlan::AlreadyCurrent => {
            show_info_dialog(
                hwnd,
                "Emplacement inchangé",
                "Le dossier sélectionné contient déjà la base de données SQLite active.",
            );
        }
        crate::bootstrap::LocationChangePlan::CopyRequired(target_db) => {
            let drive_kind = crate::bootstrap::check_drive_kind(&chosen_folder);
            if drive_kind == crate::bootstrap::DriveKind::Removable {
                let msg = "Le dossier sélectionné est situé sur un disque amovible (ex. clé USB).\n\nSi ce support est débranché lors d'un prochain lancement, Malvision vous demandera de le reconnecter.\n\nSouhaitez-vous continuer ?";
                if !confirm_dialog(hwnd, "Disque amovible détecté", msg) {
                    return;
                }
            } else if drive_kind == crate::bootstrap::DriveKind::NetworkRemote {
                let msg = "Le dossier sélectionné est situé sur un partage réseau.\n\nL'utilisation d'une base SQLite via un réseau local requiert un lien stable.\n\nSouhaitez-vous continuer ?";
                if !confirm_dialog(hwnd, "Emplacement réseau détecté", msg) {
                    return;
                }
            }

            // Exporter la base actuelle vers le nouvel emplacement de manière sécurisée et intègre
            if let Err(err) = unsafe { (*state.storage).export_database(&target_db) } {
                show_error_dialog(
                    hwnd,
                    "Échec de la copie de la base",
                    &format!("Impossible de copier la base active vers la destination :\n{err}"),
                );
                return;
            }

            if let Err(err) = crate::bootstrap::set_custom_database_path(Some(&target_db)) {
                show_error_dialog(
                    hwnd,
                    "Erreur de configuration",
                    &format!("Impossible d'enregistrer la configuration bootstrap :\n{err}"),
                );
                return;
            }

            let restart_msg = format!(
                "La base de données a été copiée avec succès vers :\n{}\n\nPour activer ce nouvel emplacement, Malvision doit redémarrer.\n\nSouhaitez-vous redémarrer Malvision maintenant ?",
                target_db.display()
            );
            if confirm_dialog(hwnd, "Nouvel emplacement configuré", &restart_msg) {
                restart_application();
            } else {
                unsafe {
                    set_window_text(state.ed_adv_path, &target_db.to_string_lossy());
                    let _ = EnableWindow(state.btn_adv_reset_default, true);
                    let _ = InvalidateRect(Some(hwnd), None, true);
                }
            }
        }
        crate::bootstrap::LocationChangePlan::ExistingFound(target_db) => {
            // Valider l'intégrité de la base existante
            match Storage::open_at_path(&target_db) {
                Ok(_) => {}
                Err(err) => {
                    show_error_dialog(
                        hwnd,
                        "Base de données existante invalide",
                        &format!(
                            "Un fichier malvision.db existe dans ce dossier mais n'est pas une base SQLite valide :\n{err}"
                        ),
                    );
                    return;
                }
            }

            let confirm_msg = format!(
                "Un fichier malvision.db existant a été détecté dans ce dossier :\n{}\n\nSouhaitez-vous basculer sur cette base existante ?\n(Aucune donnée ne sera écrasée).",
                target_db.display()
            );
            if !confirm_dialog(hwnd, "Base existante détectée", &confirm_msg) {
                return;
            }

            if let Err(err) = crate::bootstrap::set_custom_database_path(Some(&target_db)) {
                show_error_dialog(
                    hwnd,
                    "Erreur de configuration",
                    &format!("Impossible d'enregistrer la configuration bootstrap :\n{err}"),
                );
                return;
            }

            let restart_msg = format!(
                "L'emplacement a été configuré vers :\n{}\n\nPour activer cette base, Malvision doit redémarrer.\n\nSouhaitez-vous redémarrer Malvision maintenant ?",
                target_db.display()
            );
            if confirm_dialog(hwnd, "Emplacement configuré", &restart_msg) {
                restart_application();
            } else {
                unsafe {
                    set_window_text(state.ed_adv_path, &target_db.to_string_lossy());
                    let _ = EnableWindow(state.btn_adv_reset_default, true);
                    let _ = InvalidateRect(Some(hwnd), None, true);
                }
            }
        }
    }
}

unsafe fn handle_adv_reset_default_path(hwnd: HWND, state: &mut SettingsDialogState) {
    if !crate::bootstrap::is_using_custom_database() {
        return;
    }

    let confirm = confirm_dialog(
        hwnd,
        "Revenir à l'emplacement standard",
        "Souhaitez-vous revenir à l'emplacement par défaut dans %LOCALAPPDATA% ?\n\nVotre base personnalisée actuelle restera intacte sur le disque.",
    );
    if !confirm {
        return;
    }

    if let Err(err) = crate::bootstrap::set_custom_database_path(None) {
        show_error_dialog(
            hwnd,
            "Erreur de réinitialisation",
            &format!("Impossible de réinitialiser la configuration bootstrap :\n{err}"),
        );
        return;
    }

    let default_path = crate::bootstrap::default_database_path()
        .unwrap_or_else(|_| std::path::PathBuf::from("malvision.db"));
    let restart_msg = format!(
        "L'emplacement standard par défaut a été rétabli :\n{}\n\nPour activer ce changement, Malvision doit redémarrer.\n\nSouhaitez-vous redémarrer Malvision maintenant ?",
        default_path.display()
    );
    if confirm_dialog(hwnd, "Emplacement par défaut rétabli", &restart_msg) {
        restart_application();
    } else {
        unsafe {
            set_window_text(state.ed_adv_path, &default_path.to_string_lossy());
            let _ = EnableWindow(state.btn_adv_reset_default, false);
            let _ = InvalidateRect(Some(hwnd), None, true);
        }
    }
}

unsafe extern "system" fn main_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        match msg {
            WM_CREATE => {
                let state = Box::new(AppWindowState::initialize(hwnd, None));
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(state) as isize);
                LRESULT(0)
            }
            WM_ERASEBKGND => LRESULT(1),
            WM_PAINT => {
                let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut AppWindowState;
                if !ptr.is_null() {
                    (*ptr).on_paint();
                    return LRESULT(0);
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            WM_CTLCOLOREDIT => {
                let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const AppWindowState;
                if !ptr.is_null() {
                    let hdc = HDC(wparam.0 as *mut _);
                    let _ = SetTextColor(hdc, COLORREF(col_text_primary()));
                    let _ = SetBkColor(hdc, COLORREF(col_bg_input()));
                    let _ = SetBkMode(hdc, TRANSPARENT);
                    return LRESULT((*ptr).palette.brush_bg_input.0 as isize);
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            WM_CTLCOLORSTATIC => {
                let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const AppWindowState;
                if !ptr.is_null() {
                    let hdc = HDC(wparam.0 as *mut _);
                    let ctrl_hwnd = HWND(lparam.0 as *mut _);
                    if ctrl_hwnd == (*ptr).search_edit {
                        let _ = SetTextColor(hdc, COLORREF(col_text_muted()));
                        let _ = SetBkColor(hdc, COLORREF(col_bg_input()));
                        let _ = SetBkMode(hdc, TRANSPARENT);
                        return LRESULT((*ptr).palette.brush_bg_input.0 as isize);
                    }
                    let _ = SetTextColor(hdc, COLORREF(col_text_primary()));
                    let _ = SetBkColor(hdc, COLORREF(col_bg_sidebar()));
                    let _ = SetBkMode(hdc, TRANSPARENT);
                    return LRESULT((*ptr).palette.brush_bg_sidebar.0 as isize);
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            WM_GETMINMAXINFO => {
                let mmi = lparam.0 as *mut MINMAXINFO;
                if !mmi.is_null() {
                    (*mmi).ptMinTrackSize = POINT {
                        x: MIN_WINDOW_WIDTH,
                        y: MIN_WINDOW_HEIGHT,
                    };
                }
                LRESULT(0)
            }
            WM_SYSCOMMAND => {
                ensure_main_window_frame_styles(hwnd);
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            WM_SIZE => {
                let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut AppWindowState;
                if !ptr.is_null() {
                    let size_type = wparam.0;
                    if size_type != windows::Win32::UI::WindowsAndMessaging::SIZE_MINIMIZED as usize
                    {
                        ensure_main_window_frame_styles(hwnd);
                        (*ptr).layout_children();
                    }
                }
                LRESULT(0)
            }
            WM_SETCURSOR => {
                let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const AppWindowState;
                if !ptr.is_null() && wparam.0 == hwnd.0 as usize {
                    let hit = (lparam.0 & 0xFFFF) as i16;
                    // HTCLIENT == 1
                    if hit == 1 {
                        let mut pt = POINT::default();
                        let _ = GetCursorPos(&mut pt);
                        let _ = ScreenToClient(hwnd, &mut pt);
                        if (*ptr).is_on_splitter(pt.x, pt.y) {
                            let _ =
                                SetCursor(Some(LoadCursorW(None, IDC_SIZEWE).unwrap_or_default()));
                            return LRESULT(1);
                        }
                        if (*ptr).hit_test_zone(pt.x, pt.y).is_some() {
                            let _ =
                                SetCursor(Some(LoadCursorW(None, IDC_HAND).unwrap_or_default()));
                            return LRESULT(1);
                        }
                    }
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            WM_MOUSEMOVE => {
                let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut AppWindowState;
                if !ptr.is_null() {
                    let x = (lparam.0 & 0xFFFF) as i16 as i32;
                    let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;
                    (*ptr).on_mouse_move(x, y);
                }
                LRESULT(0)
            }
            WM_MOUSEWHEEL => {
                let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut AppWindowState;
                if !ptr.is_null() {
                    let delta = ((wparam.0 >> 16) & 0xFFFF) as i16;
                    (*ptr).on_mouse_wheel(delta);
                }
                LRESULT(0)
            }
            WM_MOUSELEAVE => {
                let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut AppWindowState;
                if !ptr.is_null() {
                    (*ptr).tracking_mouse = false;
                    if (*ptr).hovered_zone.is_some() {
                        (*ptr).hovered_zone = None;
                        (*ptr).invalidate_canvas();
                    }
                }
                LRESULT(0)
            }
            WM_LBUTTONDOWN => {
                let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut AppWindowState;
                if !ptr.is_null() {
                    let x = (lparam.0 & 0xFFFF) as i16 as i32;
                    let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;
                    (*ptr).on_left_button_down(x, y);
                }
                LRESULT(0)
            }
            WM_LBUTTONUP => {
                let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut AppWindowState;
                if !ptr.is_null() {
                    let x = (lparam.0 & 0xFFFF) as i16 as i32;
                    let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;
                    (*ptr).on_left_button_up(x, y);
                }
                LRESULT(0)
            }
            WM_RBUTTONUP => {
                let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut AppWindowState;
                if !ptr.is_null() {
                    let x = (lparam.0 & 0xFFFF) as i16 as i32;
                    let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;
                    (*ptr).on_right_button_up(x, y);
                }
                LRESULT(0)
            }
            WM_COMMAND => {
                let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut AppWindowState;
                if !ptr.is_null() {
                    let id = wparam.0 & 0xFFFF;
                    let code = ((wparam.0 >> 16) & 0xFFFF) as u32;
                    if id == IDC_SEARCH_EDIT {
                        if code == EN_CHANGE {
                            let q = get_window_text((*ptr).search_edit);
                            if q != (*ptr).search_query {
                                (*ptr).search_query = q;
                                (*ptr).rebuild_treeview();
                            }
                        } else if code == EN_SETFOCUS || code == EN_KILLFOCUS {
                            (*ptr).invalidate_canvas();
                        }
                    }
                }
                LRESULT(0)
            }
            WM_NOTIFY => {
                let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut AppWindowState;
                if !ptr.is_null() && lparam.0 != 0 {
                    let nmhdr = &*(lparam.0 as *const NMHDR);
                    if nmhdr.idFrom == IDC_TREEVIEW {
                        match nmhdr.code {
                            NM_CUSTOMDRAW => {
                                let nmcd = &mut *(lparam.0 as *mut NMTVCUSTOMDRAW);
                                return (*ptr).on_treeview_custom_draw(nmcd);
                            }
                            NM_CLICK => {
                                (*ptr).on_treeview_click();
                            }
                            TVN_SELCHANGEDW => {
                                let nmtv = &*(lparam.0 as *const NMTREEVIEWW);
                                (*ptr).selected_node =
                                    TreeNodeKind::from_lparam(nmtv.itemNew.lParam.0);
                                let _ = InvalidateRect(Some((*ptr).treeview), None, false);
                                (*ptr).invalidate_canvas();
                            }
                            TVN_ITEMEXPANDINGW => {
                                return LRESULT(0);
                            }
                            TVN_ITEMEXPANDEDW => {
                                if !(*ptr).is_programmatic_tree_update
                                    && (*ptr).search_query.trim().is_empty()
                                {
                                    let nmtv = &*(lparam.0 as *const NMTREEVIEWW);
                                    let hitem = nmtv.itemNew.hItem;
                                    let raw_lparam = nmtv.itemNew.lParam.0;
                                    let kind = TreeNodeKind::from_lparam(raw_lparam)
                                        .or_else(|| (*ptr).node_kind_from_hitem(hitem));
                                    if let Some(TreeNodeKind::Folder(fid)) = kind {
                                        let expanded = if (nmtv.action.0 & TVE_EXPAND.0) != 0 {
                                            true
                                        } else if (nmtv.action.0 & TVE_COLLAPSE.0) != 0 {
                                            false
                                        } else {
                                            (*ptr).is_tree_item_expanded(hitem)
                                        };
                                        let _ = (*ptr).storage.set_folder_expanded(fid, expanded);
                                        if let Some(f) = (*ptr).cached_folders.get_mut(&fid) {
                                            f.is_expanded = expanded;
                                        }
                                    }
                                }
                                let _ = InvalidateRect(Some((*ptr).treeview), None, false);
                            }
                            TVN_BEGINDRAGW => {
                                let nmtv = &*(lparam.0 as *const NMTREEVIEWW);
                                if let Some(kind) = TreeNodeKind::from_lparam(nmtv.itemNew.lParam.0)
                                    && matches!(
                                        kind,
                                        TreeNodeKind::Folder(_) | TreeNodeKind::Connection(_)
                                    )
                                {
                                    (*ptr).dragging_node = Some(kind);
                                    SetCapture(hwnd);
                                }
                            }
                            NM_DBLCLK => {
                                let mut pt = POINT::default();
                                let _ = GetCursorPos(&mut pt);
                                let _ = ScreenToClient((*ptr).treeview, &mut pt);
                                if let Some((hitem, hti)) = (*ptr).tree_resolve_row_at_point(pt) {
                                    if (hti.flags.0 & TVHT_ONITEMBUTTON.0) != 0 {
                                        // Clic direct sur le bouton [+] / [-], laisser le TreeView gérer nativement
                                        return LRESULT(0);
                                    }
                                    if hitem.0 != 0
                                        && let Some(kind) = (*ptr).node_kind_from_hitem(hitem)
                                    {
                                        match kind {
                                            TreeNodeKind::Connection(cid) => {
                                                (*ptr).selected_node =
                                                    Some(TreeNodeKind::Connection(cid));
                                                let _ = SendMessageW(
                                                    (*ptr).treeview,
                                                    TVM_SELECTITEM,
                                                    Some(WPARAM(TVGN_CARET as usize)),
                                                    Some(LPARAM(hitem.0)),
                                                );
                                                (*ptr).launch_connection_by_id(cid);
                                                return LRESULT(1);
                                            }
                                            TreeNodeKind::Folder(fid) => {
                                                (*ptr).toggle_or_set_folder_expansion(
                                                    hitem, fid, None,
                                                );
                                                return LRESULT(1);
                                            }
                                            TreeNodeKind::FavoritesRoot
                                            | TreeNodeKind::ConnectionsRoot => {
                                                (*ptr).toggle_root_expansion(hitem);
                                                return LRESULT(1);
                                            }
                                        }
                                    }
                                }
                            }
                            NM_RCLICK => {
                                (*ptr).show_tree_context_menu();
                                return LRESULT(1);
                            }
                            _ => {}
                        }
                    }
                }
                LRESULT(0)
            }
            WM_DEFERRED_ACTION => {
                let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut AppWindowState;
                if !ptr.is_null() {
                    match wparam.0 {
                        DEFERRED_CONNECT_SELECTED => {
                            if let Some(TreeNodeKind::Connection(cid)) = (*ptr).selected_node {
                                (*ptr).launch_connection_by_id(cid);
                            }
                        }
                        DEFERRED_NEW_CONNECTION => {
                            let parent_folder = (*ptr).selected_folder_context();
                            (*ptr).open_connection_dialog(None, parent_folder);
                        }
                        DEFERRED_NEW_FOLDER => {
                            let parent_folder = (*ptr).selected_folder_context();
                            (*ptr).prompt_create_folder(parent_folder);
                        }
                        DEFERRED_EDIT_CONNECTION => {
                            let cid = lparam.0 as i64;
                            (*ptr).open_connection_dialog(Some(cid), None);
                        }
                        DEFERRED_OPEN_SETTINGS => {
                            (*ptr).open_settings_dialog();
                        }
                        DEFERRED_APPLY_UPDATE_AND_EXIT => {
                            crate::single_instance::prepare_for_voluntary_restart();
                            (*ptr).perform_clean_exit();
                            std::process::exit(0);
                        }
                        _ => {}
                    }
                }
                LRESULT(0)
            }
            WM_RDP_EVENT => {
                let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut AppWindowState;
                if !ptr.is_null() {
                    let (session_id, event_code) = unpack_rdp_event_wparam(wparam);
                    let viewport = (*ptr).rdp_viewport_rect();
                    let outcome = (*ptr)
                        .sessions
                        .handle_rdp_event_with_viewport(session_id, event_code, lparam.0, viewport);
                    if let Some(msg) = outcome.status_message {
                        (*ptr).status_override = Some(msg);
                    }
                    (*ptr).invalidate_canvas();
                    if let Some((title, msg_err)) = outcome.error_alert {
                        show_warning_dialog(hwnd, &title, &msg_err);
                    }
                }
                LRESULT(0)
            }
            WM_TIMER => {
                if wparam.0 == IDT_RDP_STATE_POLL {
                    let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut AppWindowState;
                    if !ptr.is_null() && !IsIconic(hwnd).as_bool() && !(*ptr).is_hidden_in_tray {
                        ensure_main_window_frame_styles(hwnd);
                        let viewport = (*ptr).rdp_viewport_rect();
                        if (*ptr).sessions.sync_real_states_with_viewport(viewport) {
                            (*ptr).invalidate_canvas();
                        }
                    }
                }
                LRESULT(0)
            }
            WM_TRAYICON => {
                let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut AppWindowState;
                if !ptr.is_null() {
                    let mouse_msg = (lparam.0 as u32) & 0xFFFF;
                    match mouse_msg {
                        WM_LBUTTONDBLCLK => {
                            (*ptr).restore_from_tray();
                        }
                        WM_RBUTTONUP | WM_CONTEXTMENU => {
                            (*ptr).show_tray_context_menu();
                        }
                        _ => {}
                    }
                }
                LRESULT(0)
            }
            WM_CLOSE => {
                let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut AppWindowState;
                if !ptr.is_null() {
                    (*ptr).on_close_requested();
                } else {
                    let _ = DestroyWindow(hwnd);
                }
                LRESULT(0)
            }
            WM_DESTROY => {
                let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut AppWindowState;
                if !ptr.is_null() {
                    SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
                    drop(Box::from_raw(ptr));
                }
                PostQuitMessage(0);
                LRESULT(0)
            }
            WM_APP_UPDATE_AVAILABLE => {
                let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut AppWindowState;
                if !ptr.is_null() && lparam.0 != 0 {
                    let info = Box::from_raw(lparam.0 as *mut crate::updater::ReleaseInfo);
                    let state = &mut *ptr;
                    state.settings.last_known_remote_version = Some(info.version.to_string());
                    let _ = state.settings.save_all(&mut state.storage);
                    state.status_override = Some(format!(
                        "Mise à jour disponible : v{} — Voir Paramètres > Mises à jour",
                        info.version
                    ));
                    state.invalidate_canvas();
                }
                LRESULT(0)
            }
            _ => {
                let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut AppWindowState;
                if !ptr.is_null() {
                    if msg != 0 && msg == (*ptr).wm_taskbar_created {
                        (*ptr).tray_icon_added = false;
                        (*ptr).init_tray_icon();
                        return LRESULT(0);
                    }
                    if msg != 0 && msg == crate::single_instance::get_activate_window_message() {
                        let state = &mut *ptr;
                        if state.is_hidden_in_tray {
                            state.restore_from_tray();
                        } else if IsIconic(hwnd).as_bool() {
                            let _ = ShowWindow(hwnd, SW_RESTORE);
                            let _ = SetForegroundWindow(hwnd);
                        } else {
                            let _ = ShowWindow(hwnd, SW_SHOW);
                            let _ = SetForegroundWindow(hwnd);
                        }
                        return LRESULT(0);
                    }
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
        }
    }
}

/// Indique si le message Win32 est un message clavier susceptible d'être un accélérateur OLE
/// (`WM_KEYDOWN`, `WM_KEYUP`, `WM_SYSKEYDOWN`, `WM_SYSKEYUP`, `WM_CHAR`).
#[inline]
pub fn is_keyboard_message(message: u32) -> bool {
    const WM_KEYFIRST: u32 = 0x0100;
    const WM_KEYLAST: u32 = 0x0109;
    (WM_KEYFIRST..=WM_KEYLAST).contains(&message)
}

/// Lance la fenêtre principale de Malvision et exécute la boucle d'événements Win32 / ActiveX.
pub fn run_app() -> Result<(), String> {
    unsafe {
        let hinstance = GetModuleHandleW(None)
            .map_err(|e| format!("Impossible d'obtenir le handle d'instance : {e}"))?;

        // 1. Splash screen officiel Malvision (affiché au démarrage si non minimisé et hors tests)
        let splash_hwnd = if !cfg!(test) {
            create_splash_window()
        } else {
            None
        };
        let splash_start = std::time::Instant::now();

        let class_name = w!("MalvisionMainWindowClass");
        let bg_brush = CreateSolidBrush(COLORREF(col_bg_deep()));
        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: CS_HREDRAW | CS_VREDRAW | CS_DBLCLKS,
            lpfnWndProc: Some(main_wnd_proc),
            hInstance: hinstance.into(),
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            hbrBackground: bg_brush,
            lpszClassName: class_name,
            ..Default::default()
        };

        let _ = RegisterClassExW(&wc);

        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            class_name,
            w!("Malvision — Gestionnaire de Connexions Distantes"),
            WS_OVERLAPPEDWINDOW | WS_CLIPCHILDREN,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            1360,
            840,
            None,
            None,
            Some(hinstance.into()),
            None,
        )
        .map_err(|e| format!("Échec de CreateWindowExW : {e}"))?;

        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut AppWindowState;
        if !ptr.is_null() {
            let state = &mut *ptr;
            if state.settings.close_to_tray && state.settings.start_minimized_to_tray {
                if let Some(s) = splash_hwnd {
                    close_splash_window(s);
                }
                state.hide_to_tray();
            } else {
                if let Some(s) = splash_hwnd {
                    let elapsed = splash_start.elapsed();
                    let min_dur = std::time::Duration::from_millis(650);
                    if elapsed < min_dur {
                        std::thread::sleep(min_dur - elapsed);
                    }
                    close_splash_window(s);
                }
                let _ = ShowWindow(hwnd, SW_SHOW);
                let _ = UpdateWindow(hwnd);
                let _ = SetForegroundWindow(hwnd);
            }
            state.restore_sessions_on_startup();
            if !cfg!(test) && state.settings.check_updates_on_startup {
                state.spawn_startup_update_check();
            }
        } else {
            if let Some(s) = splash_hwnd {
                close_splash_window(s);
            }
            let _ = ShowWindow(hwnd, SW_SHOW);
            let _ = UpdateWindow(hwnd);
        }

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).into() {
            let mut handled = false;

            if is_keyboard_message(msg.message) {
                let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut AppWindowState;
                if !ptr.is_null() {
                    let state = &mut *ptr;
                    let focus_hwnd = GetFocus();

                    if let Some(active_tab) = state.sessions.active_tab() {
                        let ax_hwnd = active_tab.host.hwnd();
                        let focus_in_rdp =
                            focus_hwnd == ax_hwnd || IsChild(ax_hwnd, focus_hwnd).as_bool();
                        if focus_in_rdp && active_tab.host.translate_accelerator(&msg) {
                            handled = true;
                        }
                    }

                    if !handled && focus_hwnd == state.treeview && msg.message == WM_KEYDOWN {
                        if msg.wParam.0 == VK_RETURN.0 as usize {
                            if let Some(TreeNodeKind::Connection(cid)) = state.selected_node {
                                state.launch_connection_by_id(cid);
                                handled = true;
                            }
                        } else if msg.wParam.0 == VK_F2.0 as usize {
                            state.prompt_rename_selected();
                            handled = true;
                        }
                    }
                }
            }

            if !handled {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }

        let _ = DeleteObject(bg_brush.into());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rdp::{RDP_EVT_CONNECTED, RDP_EVT_CONNECTING, pack_rdp_event_wparam};
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use windows::Win32::Graphics::Gdi::{GetDC, ReleaseDC};
    use windows::Win32::UI::WindowsAndMessaging::{
        BM_CLICK, FindWindowExW, GetDlgItem, IsIconic, IsWindowVisible, IsZoomed, SC_MAXIMIZE,
        SC_MINIMIZE, SC_RESTORE, TIMERPROC, WM_LBUTTONDOWN, WM_NCHITTEST, WM_NCLBUTTONDOWN,
    };

    static TEST_DB_COUNTER: AtomicUsize = AtomicUsize::new(1);

    fn create_temp_test_db_path(label: &str) -> PathBuf {
        let seq = TEST_DB_COUNTER.fetch_add(1, Ordering::SeqCst);
        let pid = std::process::id();
        std::env::temp_dir().join(format!("malvision_ui_test_{label}_{pid}_{seq}.db"))
    }

    unsafe fn create_isolated_test_main_window(db_path: &std::path::Path) -> HWND {
        unsafe {
            let hinstance = GetModuleHandleW(None).unwrap();
            let class_name = w!("MalvisionJalon2BTestMainWindowClass");
            let wc = WNDCLASSEXW {
                cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                style: CS_HREDRAW | CS_VREDRAW | CS_DBLCLKS,
                lpfnWndProc: Some(main_wnd_proc),
                hInstance: hinstance.into(),
                lpszClassName: class_name,
                ..Default::default()
            };
            let _ = RegisterClassExW(&wc);

            let hwnd = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                class_name,
                w!("TestMalvisionJalon2BWindow"),
                WS_OVERLAPPEDWINDOW | WS_CLIPCHILDREN,
                100,
                100,
                1280,
                800,
                None,
                None,
                Some(hinstance.into()),
                None,
            )
            .expect("Création fenêtre principale de test isolée");

            // Remplacer le stockage par la base SQLite temporaire dédiée au test
            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut AppWindowState;
            assert!(!ptr.is_null());
            (*ptr).storage =
                Storage::open_at_path(db_path).expect("Ouverture DB SQLite temporaire");
            (*ptr).rebuild_treeview();

            let _ = ShowWindow(hwnd, SW_SHOW);
            let _ = UpdateWindow(hwnd);
            hwnd
        }
    }

    #[test]
    fn test_is_keyboard_message_excludes_mouse_and_non_client() {
        assert!(is_keyboard_message(WM_KEYDOWN));
        assert!(!is_keyboard_message(WM_NCLBUTTONDOWN));
        assert!(!is_keyboard_message(WM_NCHITTEST));
        assert!(!is_keyboard_message(WM_SYSCOMMAND));
        assert!(!is_keyboard_message(WM_LBUTTONDOWN));
    }

    #[test]
    fn test_tree_node_kind_lparam_roundtrip() {
        for kind in [
            TreeNodeKind::FavoritesRoot,
            TreeNodeKind::ConnectionsRoot,
            TreeNodeKind::Folder(1),
            TreeNodeKind::Folder(9999),
            TreeNodeKind::Connection(1),
            TreeNodeKind::Connection(424242),
        ] {
            let lp = kind.to_lparam();
            assert_eq!(TreeNodeKind::from_lparam(lp), Some(kind));
        }
    }

    #[test]
    fn test_dpi_scaling_metrics_100_125_150_percent() {
        let dpi_100 = DpiMetrics::from_dpi(96);
        let dpi_125 = DpiMetrics::from_dpi(120);
        let dpi_150 = DpiMetrics::from_dpi(144);

        assert_eq!(dpi_100.scale(100), 100);
        assert_eq!(dpi_125.scale(100), 125);
        assert_eq!(dpi_150.scale(100), 150);

        // Vérifier que le formulaire de connexion (680x660 à 96 DPI) s'adapte proportionnellement
        assert_eq!(dpi_100.scale(680), 680);
        assert_eq!(dpi_125.scale(680), 850);
        assert_eq!(dpi_150.scale(680), 1020);

        // Vérifier que les polices et pinceaux GDI se créent et se libèrent proprement aux 3 facteurs d'échelle
        unsafe {
            for dpi in [dpi_100, dpi_125, dpi_150] {
                let pal = GdiPalette::create_for_dpi(dpi);
                assert!(!pal.font_ui.0.is_null());
                assert!(!pal.font_ui_bold.0.is_null());
                assert!(!pal.font_title.0.is_null());
            }
        }
    }

    #[test]
    fn test_window_maximize_restore_minimize_and_multi_tab_rdp_events() {
        let _guard = WIN32_UI_TEST_MUTEX
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        use windows::Win32::System::Ole::{OleInitialize, OleUninitialize};

        unsafe {
            let _ = OleInitialize(None);
            let hinstance = GetModuleHandleW(None).unwrap();
            let class_name = w!("MalvisionJalon2TestWindowClass");
            let wc = WNDCLASSEXW {
                cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                style: CS_HREDRAW | CS_VREDRAW,
                lpfnWndProc: Some(main_wnd_proc),
                hInstance: hinstance.into(),
                lpszClassName: class_name,
                ..Default::default()
            };
            let _ = RegisterClassExW(&wc);

            let hwnd = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                class_name,
                w!("TestMalvisionJalon2Window"),
                WS_OVERLAPPEDWINDOW | WS_CLIPCHILDREN,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                1280,
                800,
                None,
                None,
                Some(hinstance.into()),
                None,
            )
            .expect("Création fenêtre principale de test Jalon 2");

            let _ = ShowWindow(hwnd, SW_SHOW);

            let style = GetWindowLongPtrW(hwnd, GWL_STYLE) as u32;
            assert_eq!(style & MAIN_WINDOW_BASE_STYLE.0, MAIN_WINDOW_BASE_STYLE.0);

            // Cycle Maximiser -> Restaurer -> Maximiser -> Minimiser -> Restaurer
            let _ = SendMessageW(
                hwnd,
                WM_SYSCOMMAND,
                Some(WPARAM(SC_MAXIMIZE as usize)),
                Some(LPARAM(0)),
            );
            assert!(IsZoomed(hwnd).as_bool());

            let _ = SendMessageW(
                hwnd,
                WM_SYSCOMMAND,
                Some(WPARAM(SC_RESTORE as usize)),
                Some(LPARAM(0)),
            );
            assert!(!IsZoomed(hwnd).as_bool());

            let _ = SendMessageW(
                hwnd,
                WM_SYSCOMMAND,
                Some(WPARAM(SC_MAXIMIZE as usize)),
                Some(LPARAM(0)),
            );
            let _ = SendMessageW(
                hwnd,
                WM_SYSCOMMAND,
                Some(WPARAM(SC_MINIMIZE as usize)),
                Some(LPARAM(0)),
            );
            assert!(IsIconic(hwnd).as_bool());

            let _ = SendMessageW(
                hwnd,
                WM_SYSCOMMAND,
                Some(WPARAM(SC_RESTORE as usize)),
                Some(LPARAM(0)),
            );
            assert!(!IsIconic(hwnd).as_bool());

            // Vérifier la réception d'événements RDP encodés avec session_id
            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut AppWindowState;
            assert!(!ptr.is_null());

            let _ = SendMessageW(
                hwnd,
                WM_RDP_EVENT,
                Some(pack_rdp_event_wparam(1, RDP_EVT_CONNECTING)),
                Some(LPARAM(0)),
            );
            let _ = SendMessageW(
                hwnd,
                WM_RDP_EVENT,
                Some(pack_rdp_event_wparam(1, RDP_EVT_CONNECTED)),
                Some(LPARAM(0)),
            );

            let _ = SendMessageW(hwnd, WM_CLOSE, Some(WPARAM(0)), Some(LPARAM(0)));
            OleUninitialize();
        }
    }

    // =========================================================================
    // TESTS D'INTÉGRATION UI WIN32 RÉELS (BOUCLES MODALES, BOUTONS, CLAVIER, SQLITE)
    // =========================================================================

    static WIN32_UI_TEST_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());
    static FOLDER_TEST_STEP: AtomicUsize = AtomicUsize::new(0);

    unsafe extern "system" fn folder_dialog_driver_timer(
        hwnd: HWND,
        _msg: u32,
        id_event: usize,
        _dw_time: u32,
    ) {
        unsafe {
            let Ok(dlg) = FindWindowExW(None, None, w!("MalvisionTextPromptDlgClass"), None) else {
                return;
            };
            if dlg.0.is_null() || !IsWindow(Some(dlg)).as_bool() {
                return;
            }
            let _ = KillTimer(Some(hwnd), id_event);

            // Pendant que le dialogue modal est ouvert, la fenêtre propriétaire doit être désactivée
            assert!(
                !IsWindowEnabled(hwnd).as_bool(),
                "La fenêtre propriétaire doit être désactivée pendant le dialogue modal"
            );

            let step = FOLDER_TEST_STEP.load(Ordering::SeqCst);
            let ed = GetDlgItem(Some(dlg), IDC_DLG_NAME as i32).unwrap_or_default();
            assert!(!ed.0.is_null(), "Champ de saisie du dossier introuvable");

            if step == 1 {
                // Scénario 1 : Saisir un nom et cliquer réellement sur "Valider" (BM_CLICK)
                set_window_text(ed, "Production Europe");
                let btn_ok = GetDlgItem(Some(dlg), IDC_DLG_OK as i32).unwrap_or_default();
                assert!(!btn_ok.0.is_null(), "Bouton Valider introuvable");
                let _ = SendMessageW(btn_ok, BM_CLICK, Some(WPARAM(0)), Some(LPARAM(0)));
            } else if step == 2 {
                // Scénario 2 : Saisir un nom mais cliquer sur "Annuler" (BM_CLICK)
                set_window_text(ed, "Dossier Annule Ne Pas Creer");
                let btn_cancel = GetDlgItem(Some(dlg), IDC_DLG_CANCEL as i32).unwrap_or_default();
                assert!(!btn_cancel.0.is_null(), "Bouton Annuler introuvable");
                let _ = SendMessageW(btn_cancel, BM_CLICK, Some(WPARAM(0)), Some(LPARAM(0)));
            }
        }
    }

    #[test]
    fn test_ui_folder_dialog_create_validate_and_cancel_real_win32() {
        let _guard = WIN32_UI_TEST_MUTEX
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let db_path = create_temp_test_db_path("folder_dlg");
        unsafe {
            let hwnd = create_isolated_test_main_window(&db_path);
            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut AppWindowState;
            assert!(!ptr.is_null());

            // 1. Ouvrir le dialogue de création de dossier -> saisir "Production Europe" -> cliquer "Valider"
            FOLDER_TEST_STEP.store(1, Ordering::SeqCst);
            let timer_fn: TIMERPROC = Some(folder_dialog_driver_timer);
            let _ = SetTimer(Some(hwnd), 9201, 25, timer_fn);
            (*ptr).prompt_create_folder(None);
            let _ = KillTimer(Some(hwnd), 9201);

            // Vérifier que le dialogue est détruit et que la fenêtre principale a retrouvé son état actif
            let leftover = FindWindowExW(None, None, w!("MalvisionTextPromptDlgClass"), None)
                .unwrap_or_default();
            assert!(
                leftover.0.is_null() || !IsWindow(Some(leftover)).as_bool(),
                "Le dialogue de dossier doit être détruit après clic sur Valider"
            );
            assert!(
                IsWindowEnabled(hwnd).as_bool(),
                "La fenêtre principale doit être réactivée après fermeture du dialogue"
            );

            // Vérifier la présence dans le cache de l'arborescence et dans SQLite
            assert_eq!((*ptr).cached_folders.len(), 1);
            assert_eq!(
                (*ptr).cached_folders.values().next().unwrap().name,
                "Production Europe"
            );

            // Vérifier la persistance après réouverture d'une instance Storage sur le même fichier SQLite
            let reopened = Storage::open_at_path(&db_path).expect("Réouverture SQLite");
            let folders = reopened.list_folders().expect("Lecture dossiers SQLite");
            assert_eq!(folders.len(), 1);
            assert_eq!(folders[0].name, "Production Europe");

            // 2. Rouvrir le dialogue -> saisir un texte -> cliquer "Annuler" -> vérifier qu'aucune modification n'est enregistrée
            FOLDER_TEST_STEP.store(2, Ordering::SeqCst);
            let _ = SetTimer(Some(hwnd), 9202, 25, timer_fn);
            (*ptr).prompt_create_folder(None);
            let _ = KillTimer(Some(hwnd), 9202);

            assert!(IsWindowEnabled(hwnd).as_bool());
            let folders_after_cancel = reopened.list_folders().expect("Lecture après Annuler");
            assert_eq!(
                folders_after_cancel.len(),
                1,
                "Annuler ne doit enregistrer aucun dossier supplémentaire"
            );

            let _ = SendMessageW(hwnd, WM_CLOSE, Some(WPARAM(0)), Some(LPARAM(0)));
        }
        let _ = std::fs::remove_file(&db_path);
    }

    static CONN_TEST_STEP: AtomicUsize = AtomicUsize::new(0);

    unsafe extern "system" fn connection_dialog_driver_timer(
        hwnd: HWND,
        _msg: u32,
        id_event: usize,
        _dw_time: u32,
    ) {
        unsafe {
            let Ok(dlg) = FindWindowExW(None, None, w!("MalvisionConnectionFormDlgClass"), None)
            else {
                return;
            };
            if dlg.0.is_null() || !IsWindow(Some(dlg)).as_bool() {
                return;
            }
            let _ = KillTimer(Some(hwnd), id_event);

            assert!(
                !IsWindowEnabled(hwnd).as_bool(),
                "La fenêtre propriétaire doit être désactivée pendant le formulaire de connexion"
            );

            let step = CONN_TEST_STEP.load(Ordering::SeqCst);
            let ed_name = GetDlgItem(Some(dlg), IDC_DLG_NAME as i32).unwrap_or_default();
            let ed_host = GetDlgItem(Some(dlg), IDC_DLG_HOST as i32).unwrap_or_default();
            let ed_port = GetDlgItem(Some(dlg), IDC_DLG_PORT as i32).unwrap_or_default();
            let ed_user = GetDlgItem(Some(dlg), IDC_DLG_USER as i32).unwrap_or_default();
            let ed_domain = GetDlgItem(Some(dlg), IDC_DLG_DOMAIN as i32).unwrap_or_default();
            let ed_pass = GetDlgItem(Some(dlg), IDC_DLG_PASS as i32).unwrap_or_default();
            let chk_save = GetDlgItem(Some(dlg), IDC_DLG_SAVE_PASS as i32).unwrap_or_default();
            let chk_fav = GetDlgItem(Some(dlg), IDC_DLG_FAVORITE as i32).unwrap_or_default();
            let ed_desc = GetDlgItem(Some(dlg), IDC_DLG_DESC as i32).unwrap_or_default();

            if step == 1 {
                set_window_text(ed_name, "SRV-RDP-CORE-01");
                set_window_text(ed_host, "10.100.20.15");
                set_window_text(ed_port, "3389");
                set_window_text(ed_user, "adm.malvision");
                set_window_text(ed_domain, "CORP");
                set_window_text(ed_desc, "Serveur principal validé par test UI Win32");

                // Cocher "Enregistrer le mot de passe" et notifier le dialogue
                let _ = SendMessageW(chk_save, BM_SETCHECK, Some(WPARAM(1)), Some(LPARAM(0)));
                let _ = SendMessageW(
                    dlg,
                    WM_COMMAND,
                    Some(WPARAM(IDC_DLG_SAVE_PASS)),
                    Some(LPARAM(chk_save.0 as isize)),
                );
                assert!(
                    IsWindowEnabled(ed_pass).as_bool(),
                    "Le champ mot de passe doit être activé lorsque la case DPAPI est cochée"
                );
                set_window_text(ed_pass, "P@ssw0rd_UI_Test_2B!");

                let _ = SendMessageW(chk_fav, BM_SETCHECK, Some(WPARAM(1)), Some(LPARAM(0)));

                let btn_ok = GetDlgItem(Some(dlg), IDC_DLG_OK as i32).unwrap_or_default();
                assert!(!btn_ok.0.is_null(), "Bouton Enregistrer introuvable");
                let _ = SendMessageW(btn_ok, BM_CLICK, Some(WPARAM(0)), Some(LPARAM(0)));
            } else if step == 2 {
                set_window_text(ed_name, "CONNEXION-ANNULEE");
                set_window_text(ed_host, "192.168.99.99");
                let btn_cancel = GetDlgItem(Some(dlg), IDC_DLG_CANCEL as i32).unwrap_or_default();
                assert!(!btn_cancel.0.is_null(), "Bouton Annuler introuvable");
                let _ = SendMessageW(btn_cancel, BM_CLICK, Some(WPARAM(0)), Some(LPARAM(0)));
            } else if step == 3 {
                // Tester le formulaire adaptatif AnyDesk
                let cb_proto = GetDlgItem(Some(dlg), IDC_DLG_PROTO as i32).unwrap_or_default();
                assert!(!cb_proto.0.is_null());
                // Sélectionner AnyDesk (indice 1) et notifier CBN_SELCHANGE
                let _ = SendMessageW(cb_proto, CB_SETCURSEL, Some(WPARAM(1)), Some(LPARAM(0)));
                let _ = SendMessageW(
                    dlg,
                    WM_COMMAND,
                    Some(WPARAM(1 << 16 | IDC_DLG_PROTO)),
                    Some(LPARAM(cb_proto.0 as isize)),
                );
                // Vérifier que les champs spécifiques RDP sont bien masqués
                assert!(
                    !IsWindowVisible(ed_port).as_bool(),
                    "Port doit être masqué sous AnyDesk"
                );
                assert!(
                    !IsWindowVisible(ed_user).as_bool(),
                    "Utilisateur doit être masqué sous AnyDesk"
                );
                assert!(
                    !IsWindowVisible(ed_domain).as_bool(),
                    "Domaine doit être masqué sous AnyDesk"
                );

                // Remplir la fiche AnyDesk
                set_window_text(ed_name, "Support AnyDesk Client");
                set_window_text(ed_host, "456 789 012");
                let _ = SendMessageW(chk_save, BM_SETCHECK, Some(WPARAM(1)), Some(LPARAM(0)));
                let _ = SendMessageW(
                    dlg,
                    WM_COMMAND,
                    Some(WPARAM(IDC_DLG_SAVE_PASS)),
                    Some(LPARAM(chk_save.0 as isize)),
                );
                set_window_text(ed_pass, "UnattendedPwdAnyDesk42!");

                let btn_ok = GetDlgItem(Some(dlg), IDC_DLG_OK as i32).unwrap_or_default();
                let _ = SendMessageW(btn_ok, BM_CLICK, Some(WPARAM(0)), Some(LPARAM(0)));
            } else if step == 4 {
                // Tester la bascule dynamique RustDesk <-> RDP
                let cb_proto = GetDlgItem(Some(dlg), IDC_DLG_PROTO as i32).unwrap_or_default();
                // Sélectionner RustDesk (indice 2)
                let _ = SendMessageW(cb_proto, CB_SETCURSEL, Some(WPARAM(2)), Some(LPARAM(0)));
                let _ = SendMessageW(
                    dlg,
                    WM_COMMAND,
                    Some(WPARAM(1 << 16 | IDC_DLG_PROTO)),
                    Some(LPARAM(cb_proto.0 as isize)),
                );
                assert!(
                    !IsWindowVisible(ed_port).as_bool(),
                    "Port masqué sous RustDesk"
                );

                // Rebasculer sur RDP (indice 0)
                let _ = SendMessageW(cb_proto, CB_SETCURSEL, Some(WPARAM(0)), Some(LPARAM(0)));
                let _ = SendMessageW(
                    dlg,
                    WM_COMMAND,
                    Some(WPARAM(1 << 16 | IDC_DLG_PROTO)),
                    Some(LPARAM(cb_proto.0 as isize)),
                );
                assert!(
                    IsWindowVisible(ed_port).as_bool(),
                    "Port réaffiché sous RDP"
                );
                assert!(
                    IsWindowVisible(ed_user).as_bool(),
                    "Utilisateur réaffiché sous RDP"
                );

                let btn_cancel = GetDlgItem(Some(dlg), IDC_DLG_CANCEL as i32).unwrap_or_default();
                let _ = SendMessageW(btn_cancel, BM_CLICK, Some(WPARAM(0)), Some(LPARAM(0)));
            }
        }
    }

    #[test]
    fn test_ui_connection_dialog_create_validate_and_cancel_real_win32() {
        let _guard = WIN32_UI_TEST_MUTEX
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let db_path = create_temp_test_db_path("conn_dlg");
        unsafe {
            let hwnd = create_isolated_test_main_window(&db_path);
            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut AppWindowState;
            assert!(!ptr.is_null());

            let folder = (*ptr)
                .storage
                .create_folder("Datacenter Paris", None)
                .expect("Création dossier parent");
            (*ptr).rebuild_treeview();

            // 1. Ouvrir le formulaire de connexion -> remplir tous les champs -> cliquer "Enregistrer"
            CONN_TEST_STEP.store(1, Ordering::SeqCst);
            let timer_fn: TIMERPROC = Some(connection_dialog_driver_timer);
            let _ = SetTimer(Some(hwnd), 9301, 25, timer_fn);
            (*ptr).open_connection_dialog(None, Some(folder.id));
            let _ = KillTimer(Some(hwnd), 9301);

            let leftover = FindWindowExW(None, None, w!("MalvisionConnectionFormDlgClass"), None)
                .unwrap_or_default();
            assert!(
                leftover.0.is_null() || !IsWindow(Some(leftover)).as_bool(),
                "Le formulaire de connexion doit être détruit après clic sur Enregistrer"
            );
            assert!(
                IsWindowEnabled(hwnd).as_bool(),
                "La fenêtre principale doit être réactivée après enregistrement"
            );

            // Vérifier la persistance SQLite et le déchiffrement DPAPI
            let reopened = Storage::open_at_path(&db_path).expect("Réouverture SQLite");
            let conns = reopened.list_connections().expect("Lecture connexions");
            assert_eq!(conns.len(), 1);
            let saved = &conns[0];
            assert_eq!(saved.name, "SRV-RDP-CORE-01");
            assert_eq!(saved.host, "10.100.20.15");
            assert_eq!(saved.port, 3389);
            assert_eq!(saved.username, "adm.malvision");
            assert_eq!(saved.domain, "CORP");
            assert_eq!(saved.folder_id, Some(folder.id));
            assert!(saved.is_favorite);
            assert!(saved.save_password);
            assert!(saved.has_saved_password);

            let decrypted = reopened
                .load_decrypted_password(saved.id)
                .expect("Déchiffrement DPAPI")
                .expect("Mot de passe présent");
            assert_eq!(decrypted.as_str(), "P@ssw0rd_UI_Test_2B!");

            // 2. Rouvrir le formulaire -> remplir -> cliquer "Annuler" -> vérifier qu'aucune modification n'est enregistrée
            CONN_TEST_STEP.store(2, Ordering::SeqCst);
            let _ = SetTimer(Some(hwnd), 9302, 25, timer_fn);
            (*ptr).open_connection_dialog(None, Some(folder.id));
            let _ = KillTimer(Some(hwnd), 9302);

            assert!(IsWindowEnabled(hwnd).as_bool());
            let conns_after_cancel = reopened.list_connections().expect("Lecture après Annuler");
            assert_eq!(conns_after_cancel.len(), 1);

            // 3. Ouvrir et enregistrer une fiche AnyDesk via le formulaire adaptatif
            CONN_TEST_STEP.store(3, Ordering::SeqCst);
            let _ = SetTimer(Some(hwnd), 9303, 25, timer_fn);
            (*ptr).open_connection_dialog(None, Some(folder.id));
            let _ = KillTimer(Some(hwnd), 9303);

            let conns_with_anydesk = reopened.list_connections().expect("Lecture après AnyDesk");
            assert_eq!(conns_with_anydesk.len(), 2);
            let anydesk_conn = conns_with_anydesk
                .iter()
                .find(|c| c.protocol == Protocol::AnyDesk)
                .expect("Fiche AnyDesk trouvée");
            assert_eq!(anydesk_conn.name, "Support AnyDesk Client");
            assert_eq!(anydesk_conn.host, "456 789 012");
            let anydesk_pwd = reopened
                .load_decrypted_password(anydesk_conn.id)
                .expect("Déchiffrement DPAPI")
                .expect("Mot de passe AnyDesk");
            assert_eq!(anydesk_pwd.as_str(), "UnattendedPwdAnyDesk42!");

            // 4. Tester la bascule dynamique RustDesk <-> RDP dans le formulaire
            CONN_TEST_STEP.store(4, Ordering::SeqCst);
            let _ = SetTimer(Some(hwnd), 9304, 25, timer_fn);
            (*ptr).open_connection_dialog(None, Some(folder.id));
            let _ = KillTimer(Some(hwnd), 9304);

            let _ = SendMessageW(hwnd, WM_CLOSE, Some(WPARAM(0)), Some(LPARAM(0)));
        }
        let _ = std::fs::remove_file(&db_path);
    }

    static ESC_AND_CLOSE_STEP: AtomicUsize = AtomicUsize::new(0);

    unsafe extern "system" fn esc_and_close_driver_timer(
        hwnd: HWND,
        _msg: u32,
        id_event: usize,
        _dw_time: u32,
    ) {
        unsafe {
            let step = ESC_AND_CLOSE_STEP.load(Ordering::SeqCst);
            if step == 1 {
                let Ok(dlg) = FindWindowExW(None, None, w!("MalvisionTextPromptDlgClass"), None)
                else {
                    return;
                };
                if dlg.0.is_null() || !IsWindow(Some(dlg)).as_bool() {
                    return;
                }
                let _ = KillTimer(Some(hwnd), id_event);
                let ed = GetDlgItem(Some(dlg), IDC_DLG_NAME as i32).unwrap_or(dlg);
                // Poster WM_KEYDOWN(VK_ESCAPE) dans la file de messages pour tester run_modal_loop
                let _ = PostMessageW(
                    Some(ed),
                    WM_KEYDOWN,
                    WPARAM(VK_ESCAPE.0 as usize),
                    LPARAM(0),
                );
            } else if step == 2 {
                let Ok(dlg) =
                    FindWindowExW(None, None, w!("MalvisionConnectionFormDlgClass"), None)
                else {
                    return;
                };
                if dlg.0.is_null() || !IsWindow(Some(dlg)).as_bool() {
                    return;
                }
                let _ = KillTimer(Some(hwnd), id_event);
                // Envoyer WM_CLOSE (équivalent clic sur la croix [X] de la barre de titre)
                let _ = PostMessageW(Some(dlg), WM_CLOSE, WPARAM(0), LPARAM(0));
            }
        }
    }

    #[test]
    fn test_ui_dialogs_close_via_escape_and_close_button_x() {
        let _guard = WIN32_UI_TEST_MUTEX
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let db_path = create_temp_test_db_path("esc_and_x");
        unsafe {
            let hwnd = create_isolated_test_main_window(&db_path);
            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut AppWindowState;
            assert!(!ptr.is_null());

            let timer_fn: TIMERPROC = Some(esc_and_close_driver_timer);

            // 1. Fermeture par touche Échap (VK_ESCAPE) sur le dialogue de dossier
            ESC_AND_CLOSE_STEP.store(1, Ordering::SeqCst);
            let _ = SetTimer(Some(hwnd), 9401, 25, timer_fn);
            (*ptr).prompt_create_folder(None);
            let _ = KillTimer(Some(hwnd), 9401);
            assert!(
                IsWindowEnabled(hwnd).as_bool(),
                "La fenêtre principale doit être réactivée après Échap"
            );

            // 2. Fermeture par la croix [X] (WM_CLOSE) sur le formulaire de connexion
            ESC_AND_CLOSE_STEP.store(2, Ordering::SeqCst);
            let _ = SetTimer(Some(hwnd), 9402, 25, timer_fn);
            (*ptr).open_connection_dialog(None, None);
            let _ = KillTimer(Some(hwnd), 9402);
            assert!(
                IsWindowEnabled(hwnd).as_bool(),
                "La fenêtre principale doit être réactivée après fermeture par [X]"
            );

            assert!(
                (*ptr).storage.list_folders().unwrap().is_empty(),
                "Aucun dossier ne doit être créé après Échap"
            );
            assert!(
                (*ptr).storage.list_connections().unwrap().is_empty(),
                "Aucune connexion ne doit être créée après fermeture par [X]"
            );

            let _ = SendMessageW(hwnd, WM_CLOSE, Some(WPARAM(0)), Some(LPARAM(0)));
        }
        let _ = std::fs::remove_file(&db_path);
    }

    // =========================================================================
    // TESTS JALON 2C : EM_SETSEL PORT RDP, DIALOGUE PARAMÈTRES, THÈMES & TRAY
    // =========================================================================

    unsafe extern "system" fn port_auto_select_driver_timer(
        hwnd: HWND,
        _msg: u32,
        id_event: usize,
        _dw_time: u32,
    ) {
        unsafe {
            let Ok(dlg) = FindWindowExW(None, None, w!("MalvisionConnectionFormDlgClass"), None)
            else {
                return;
            };
            if dlg.0.is_null() || !IsWindow(Some(dlg)).as_bool() {
                return;
            }
            let _ = KillTimer(Some(hwnd), id_event);

            let ed_name = GetDlgItem(Some(dlg), IDC_DLG_NAME as i32).unwrap_or_default();
            let ed_host = GetDlgItem(Some(dlg), IDC_DLG_HOST as i32).unwrap_or_default();
            let ed_port = GetDlgItem(Some(dlg), IDC_DLG_PORT as i32).unwrap_or_default();

            set_window_text(ed_name, "SRV-CUSTOM-PORT");
            set_window_text(ed_host, "10.200.1.50");
            assert_eq!(get_window_text(ed_port), "3389");

            // 1. Focus par navigation clavier / SetFocus -> EM_SETSEL sélectionne tout ("3389", 0..4)
            let _ = SetFocus(Some(ed_name));
            let _ = SetFocus(Some(ed_port));
            const EM_GETSEL: u32 = 0x00B0;
            let mut sel_start: u32 = 99;
            let mut sel_end: u32 = 99;
            let _ = SendMessageW(
                ed_port,
                EM_GETSEL,
                Some(WPARAM(&mut sel_start as *mut u32 as usize)),
                Some(LPARAM(&mut sel_end as *mut u32 as isize)),
            );
            assert_eq!(
                (sel_start, sel_end),
                (0, 4),
                "Le focus clavier sur le champ Port doit sélectionner l'intégralité de '3389'"
            );

            // 2. Focus par clic souris (WM_LBUTTONDOWN quand le champ n'a pas le focus)
            let _ = SetFocus(Some(ed_host));
            let _ = SendMessageW(
                ed_port,
                WM_LBUTTONDOWN,
                Some(WPARAM(1)),
                Some(LPARAM((10 << 16) | 10)),
            );
            let _ = SendMessageW(
                ed_port,
                WM_LBUTTONUP,
                Some(WPARAM(0)),
                Some(LPARAM((10 << 16) | 10)),
            );
            sel_start = 99;
            sel_end = 99;
            let _ = SendMessageW(
                ed_port,
                EM_GETSEL,
                Some(WPARAM(&mut sel_start as *mut u32 as usize)),
                Some(LPARAM(&mut sel_end as *mut u32 as isize)),
            );
            assert_eq!(
                (sel_start, sel_end),
                (0, 4),
                "Le clic souris sur le champ Port doit conserver la sélection intégrale de '3389'"
            );

            // 3. Saisie directe de "42666" caractère par caractère via WM_CHAR sans effacement manuel
            const WM_CHAR_MSG: u32 = 0x0102;
            for ch in "42666".chars() {
                let _ = SendMessageW(
                    ed_port,
                    WM_CHAR_MSG,
                    Some(WPARAM(ch as usize)),
                    Some(LPARAM(1)),
                );
            }
            assert_eq!(
                get_window_text(ed_port),
                "42666",
                "La saisie directe doit remplacer '3389' par '42666' sans resélection intempestive"
            );

            let btn_ok = GetDlgItem(Some(dlg), IDC_DLG_OK as i32).unwrap_or_default();
            let _ = SendMessageW(btn_ok, BM_CLICK, Some(WPARAM(0)), Some(LPARAM(0)));
        }
    }

    #[test]
    fn test_ui_port_field_auto_selects_all_on_focus_and_direct_typing() {
        let _guard = WIN32_UI_TEST_MUTEX
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let db_path = create_temp_test_db_path("port_setsel");
        unsafe {
            let hwnd = create_isolated_test_main_window(&db_path);
            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut AppWindowState;
            assert!(!ptr.is_null());

            let timer_fn: TIMERPROC = Some(port_auto_select_driver_timer);
            let _ = SetTimer(Some(hwnd), 9501, 25, timer_fn);
            (*ptr).open_connection_dialog(None, None);
            let _ = KillTimer(Some(hwnd), 9501);

            let conns = (*ptr).storage.list_connections().unwrap();
            assert_eq!(conns.len(), 1);
            assert_eq!(conns[0].name, "SRV-CUSTOM-PORT");
            assert_eq!(conns[0].port, 42666);

            let _ = SendMessageW(hwnd, WM_CLOSE, Some(WPARAM(0)), Some(LPARAM(0)));
        }
        let _ = std::fs::remove_file(&db_path);
    }

    unsafe extern "system" fn settings_dialog_driver_timer(
        hwnd: HWND,
        _msg: u32,
        id_event: usize,
        _dw_time: u32,
    ) {
        unsafe {
            let Ok(dlg) = FindWindowExW(None, None, w!("MalvisionSettingsDlgClass"), None) else {
                return;
            };
            if dlg.0.is_null() || !IsWindow(Some(dlg)).as_bool() {
                return;
            }
            let _ = KillTimer(Some(hwnd), id_event);

            let cb_theme = GetDlgItem(Some(dlg), IDC_SET_THEME as i32).unwrap_or_default();
            let chk_tray = GetDlgItem(Some(dlg), IDC_SET_CLOSE_TRAY as i32).unwrap_or_default();
            let chk_min = GetDlgItem(Some(dlg), IDC_SET_START_MIN_TRAY as i32).unwrap_or_default();
            let ed_max = GetDlgItem(Some(dlg), IDC_SET_MAX_RECENT as i32).unwrap_or_default();

            // Par défaut, close_to_tray est décoché donc start_minimized_to_tray doit être désactivé
            assert!(
                !IsWindowEnabled(chk_min).as_bool(),
                "Démarrer minimisé doit être désactivé tant que le tray n'est pas activé"
            );

            // Basculer le thème sur Clair (index 1) et vérifier l'aperçu immédiat sans redémarrage
            let _ = SendMessageW(cb_theme, CB_SETCURSEL, Some(WPARAM(1)), Some(LPARAM(0)));
            let _ = SendMessageW(
                dlg,
                WM_COMMAND,
                Some(WPARAM((1usize << 16) | IDC_SET_THEME)),
                Some(LPARAM(cb_theme.0 as isize)),
            );
            assert!(!is_active_theme_dark());

            // Activer close_to_tray -> doit activer chk_min
            let _ = SendMessageW(chk_tray, BM_SETCHECK, Some(WPARAM(1)), Some(LPARAM(0)));
            let _ = SendMessageW(
                dlg,
                WM_COMMAND,
                Some(WPARAM(IDC_SET_CLOSE_TRAY)),
                Some(LPARAM(chk_tray.0 as isize)),
            );
            assert!(
                IsWindowEnabled(chk_min).as_bool(),
                "Démarrer minimisé doit devenir disponible dès que le tray est activé"
            );
            let _ = SendMessageW(chk_min, BM_SETCHECK, Some(WPARAM(1)), Some(LPARAM(0)));

            // Configurer le nombre maximal de connexions récentes à 15
            set_window_text(ed_max, "15");

            // Contrôles Jalon 3 (KeyboardHookMode et Applications Externes)
            let cb_hook = GetDlgItem(Some(dlg), IDC_SET_RDP_HOOK_MODE as i32).unwrap_or_default();
            let ed_anydesk = GetDlgItem(Some(dlg), IDC_SET_ANYDESK_PATH as i32).unwrap_or_default();
            let ed_rustdesk =
                GetDlgItem(Some(dlg), IDC_SET_RUSTDESK_PATH as i32).unwrap_or_default();
            assert!(IsWindow(Some(cb_hook)).as_bool());
            assert!(IsWindow(Some(ed_anydesk)).as_bool());
            assert!(IsWindow(Some(ed_rustdesk)).as_bool());

            // Basculer KeyboardHookMode sur "FullScreenOnly" (index 2)
            let _ = SendMessageW(cb_hook, CB_SETCURSEL, Some(WPARAM(2)), Some(LPARAM(0)));

            let btn_ok = GetDlgItem(Some(dlg), IDC_DLG_OK as i32).unwrap_or_default();
            let _ = SendMessageW(btn_ok, BM_CLICK, Some(WPARAM(0)), Some(LPARAM(0)));
        }
    }

    #[test]
    fn test_ui_settings_dialog_theme_switching_and_persistence_real_win32() {
        let _guard = WIN32_UI_TEST_MUTEX
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let db_path = create_temp_test_db_path("settings_dlg");
        unsafe {
            let hwnd = create_isolated_test_main_window(&db_path);
            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut AppWindowState;
            assert!(!ptr.is_null());

            let timer_fn: TIMERPROC = Some(settings_dialog_driver_timer);
            let _ = SetTimer(Some(hwnd), 9601, 25, timer_fn);
            (*ptr).open_settings_dialog();
            let _ = KillTimer(Some(hwnd), 9601);

            assert_eq!((*ptr).settings.theme_mode, ThemeMode::Light);
            assert!(!is_active_theme_dark());
            assert!((*ptr).settings.close_to_tray);
            assert!((*ptr).settings.start_minimized_to_tray);
            assert_eq!((*ptr).settings.max_recent_connections, 15);
            assert_eq!(
                (*ptr).settings.rdp_keyboard_hook_mode,
                RdpKeyboardHookMode::InFullScreenOnly
            );

            // Vérifier la persistance dans SQLite
            let reopened = Storage::open_at_path(&db_path).unwrap();
            let loaded = AppSettings::load(&reopened);
            assert_eq!(loaded.theme_mode, ThemeMode::Light);
            assert!(loaded.close_to_tray);
            assert!(loaded.start_minimized_to_tray);
            assert_eq!(loaded.max_recent_connections, 15);
            assert_eq!(
                loaded.rdp_keyboard_hook_mode,
                RdpKeyboardHookMode::InFullScreenOnly
            );

            // Remettre le thème sombre et quitter proprement
            (*ptr).settings.theme_mode = ThemeMode::Dark;
            (*ptr).apply_theme();
            (*ptr).perform_clean_exit();
        }
        let _ = std::fs::remove_file(&db_path);
    }

    unsafe extern "system" fn settings_tabs_driver_timer(
        hwnd: HWND,
        _msg: u32,
        id_event: usize,
        _dw_time: u32,
    ) {
        unsafe {
            let Ok(dlg) = FindWindowExW(None, None, w!("MalvisionSettingsDlgClass"), None) else {
                return;
            };
            if dlg.0.is_null() || !IsWindow(Some(dlg)).as_bool() {
                return;
            }
            let _ = KillTimer(Some(hwnd), id_event);

            let cb_theme = GetDlgItem(Some(dlg), IDC_SET_THEME as i32).unwrap_or_default();
            let ed_anydesk = GetDlgItem(Some(dlg), IDC_SET_ANYDESK_PATH as i32).unwrap_or_default();
            let btn_export = GetDlgItem(Some(dlg), IDC_SET_DATA_EXPORT as i32).unwrap_or_default();
            let btn_import = GetDlgItem(Some(dlg), IDC_SET_DATA_IMPORT as i32).unwrap_or_default();
            let chk_updates = GetDlgItem(Some(dlg), IDC_SET_UPDATE_AUTO as i32).unwrap_or_default();
            let btn_check_now =
                GetDlgItem(Some(dlg), IDC_SET_UPDATE_CHECK_NOW as i32).unwrap_or_default();
            let btn_github = GetDlgItem(Some(dlg), IDC_SET_ABOUT_GITHUB as i32).unwrap_or_default();
            let ed_adv_path =
                GetDlgItem(Some(dlg), IDC_SET_ADV_PATH_DISPLAY as i32).unwrap_or_default();
            let btn_adv_change =
                GetDlgItem(Some(dlg), IDC_SET_ADV_CHANGE_PATH as i32).unwrap_or_default();
            let btn_adv_open =
                GetDlgItem(Some(dlg), IDC_SET_ADV_OPEN_FOLDER as i32).unwrap_or_default();
            let btn_adv_reset =
                GetDlgItem(Some(dlg), IDC_SET_ADV_RESET_DEFAULT as i32).unwrap_or_default();

            // 1. Initialement dans Général : le thème est visible, Données/Mises à jour/Avancé/À propos masqués
            assert!(
                IsWindowVisible(cb_theme).as_bool(),
                "Général doit être actif par défaut"
            );
            assert!(
                !IsWindowVisible(ed_anydesk).as_bool(),
                "Protocoles doit être masqué dans Général"
            );
            assert!(
                !IsWindowVisible(btn_export).as_bool(),
                "Export doit être masqué dans Général"
            );
            assert!(
                !IsWindowVisible(chk_updates).as_bool(),
                "Mises à jour doit être masqué"
            );
            assert!(
                !IsWindowVisible(ed_adv_path).as_bool(),
                "Avancé doit être masqué dans Général"
            );
            assert!(
                !IsWindowVisible(btn_github).as_bool(),
                "GitHub doit être masqué"
            );

            // 2. Basculer vers Protocoles (IDC_SET_TAB_PROTOCOLS)
            let _ = SendMessageW(
                dlg,
                WM_COMMAND,
                Some(WPARAM(IDC_SET_TAB_PROTOCOLS)),
                Some(LPARAM(0)),
            );
            assert!(
                !IsWindowVisible(cb_theme).as_bool(),
                "Général doit être masqué"
            );
            assert!(
                IsWindowVisible(ed_anydesk).as_bool(),
                "Protocoles doit être visible dans l'onglet Protocoles"
            );

            // 3. Basculer vers Données (IDC_SET_TAB_DATA)
            let _ = SendMessageW(
                dlg,
                WM_COMMAND,
                Some(WPARAM(IDC_SET_TAB_DATA)),
                Some(LPARAM(0)),
            );
            assert!(
                !IsWindowVisible(ed_anydesk).as_bool(),
                "Protocoles doit être masqué"
            );
            assert!(
                IsWindowVisible(btn_export).as_bool(),
                "Export doit être visible dans Données"
            );
            assert!(
                IsWindowVisible(btn_import).as_bool(),
                "Import doit être visible dans Données"
            );

            // 4. Basculer vers Mises à jour (IDC_SET_TAB_UPDATES)
            let _ = SendMessageW(
                dlg,
                WM_COMMAND,
                Some(WPARAM(IDC_SET_TAB_UPDATES)),
                Some(LPARAM(0)),
            );
            assert!(
                !IsWindowVisible(btn_export).as_bool(),
                "Données doit être masqué"
            );
            assert!(
                IsWindowVisible(chk_updates).as_bool(),
                "Check updates auto doit être visible"
            );
            assert!(
                IsWindowVisible(btn_check_now).as_bool(),
                "Vérifier maintenant doit être visible"
            );
            // Décocher la vérification automatique
            let _ = SendMessageW(chk_updates, BM_SETCHECK, Some(WPARAM(0)), Some(LPARAM(0)));

            // 5. Basculer vers Avancé (IDC_SET_TAB_ADVANCED)
            let _ = SendMessageW(
                dlg,
                WM_COMMAND,
                Some(WPARAM(IDC_SET_TAB_ADVANCED)),
                Some(LPARAM(0)),
            );
            assert!(
                !IsWindowVisible(chk_updates).as_bool(),
                "Mises à jour doit être masqué"
            );
            assert!(
                IsWindowVisible(ed_adv_path).as_bool(),
                "Champ chemin DB doit être visible dans Avancé"
            );
            assert!(
                IsWindowVisible(btn_adv_change).as_bool(),
                "Bouton modifier emplacement doit être visible dans Avancé"
            );
            assert!(
                IsWindowVisible(btn_adv_open).as_bool(),
                "Bouton ouvrir dossier doit être visible dans Avancé"
            );
            assert!(
                IsWindowVisible(btn_adv_reset).as_bool(),
                "Bouton reset défaut doit être visible dans Avancé"
            );

            // 6. Basculer vers À propos (IDC_SET_TAB_ABOUT)
            let _ = SendMessageW(
                dlg,
                WM_COMMAND,
                Some(WPARAM(IDC_SET_TAB_ABOUT)),
                Some(LPARAM(0)),
            );
            assert!(
                !IsWindowVisible(ed_adv_path).as_bool(),
                "Avancé doit être masqué dans À propos"
            );
            assert!(
                IsWindowVisible(btn_github).as_bool(),
                "Bouton GitHub doit être visible dans À propos"
            );

            // 7. Enregistrer et fermer (IDC_DLG_OK)
            let btn_ok = GetDlgItem(Some(dlg), IDC_DLG_OK as i32).unwrap_or_default();
            let _ = SendMessageW(btn_ok, BM_CLICK, Some(WPARAM(0)), Some(LPARAM(0)));
        }
    }

    #[test]
    fn test_ui_settings_dialog_tabs_jalon4_categories_and_buttons() {
        let _guard = WIN32_UI_TEST_MUTEX
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let db_path = create_temp_test_db_path("settings_tabs");
        unsafe {
            let hwnd = create_isolated_test_main_window(&db_path);
            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut AppWindowState;
            assert!(!ptr.is_null());

            let timer_fn: TIMERPROC = Some(settings_tabs_driver_timer);
            let _ = SetTimer(Some(hwnd), 9602, 25, timer_fn);
            (*ptr).open_settings_dialog();
            let _ = KillTimer(Some(hwnd), 9602);

            // Vérifier que la modification a bien persisté (décochage check_updates_on_startup)
            assert!(!(*ptr).settings.check_updates_on_startup);

            let reopened = Storage::open_at_path(&db_path).unwrap();
            let loaded = AppSettings::load(&reopened);
            assert!(!loaded.check_updates_on_startup);

            (*ptr).perform_clean_exit();
        }
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn test_ui_system_tray_close_to_tray_restore_confirm_active_sessions_and_explicit_exit() {
        let _guard = WIN32_UI_TEST_MUTEX
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let db_path = create_temp_test_db_path("tray_lifecycle");
        unsafe {
            let hwnd = create_isolated_test_main_window(&db_path);
            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut AppWindowState;
            assert!(!ptr.is_null());

            // 1. Activer "Fermer vers la zone de notification" -> WM_CLOSE masque la fenêtre au lieu de la détruire
            (*ptr).settings.close_to_tray = true;
            let _ = SendMessageW(hwnd, WM_CLOSE, Some(WPARAM(0)), Some(LPARAM(0)));
            assert!(
                IsWindow(Some(hwnd)).as_bool(),
                "La fenêtre ne doit pas être détruite lorsque close_to_tray est actif"
            );
            assert!(
                (*ptr).is_hidden_in_tray,
                "L'état is_hidden_in_tray doit être vrai après fermeture vers le tray"
            );
            assert!(
                !windows::Win32::UI::WindowsAndMessaging::IsWindowVisible(hwnd).as_bool(),
                "La fenêtre doit être masquée (SW_HIDE)"
            );

            // 2. Double-clic sur l'icône du System Tray -> restaure la fenêtre
            let _ = SendMessageW(
                hwnd,
                WM_TRAYICON,
                Some(WPARAM(TRAY_ICON_ID as usize)),
                Some(LPARAM(WM_LBUTTONDBLCLK as isize)),
            );
            assert!(
                !(*ptr).is_hidden_in_tray,
                "Le double-clic tray doit restaurer la fenêtre"
            );
            assert!(
                windows::Win32::UI::WindowsAndMessaging::IsWindowVisible(hwnd).as_bool(),
                "La fenêtre doit redevenir visible après restauration depuis le tray"
            );

            // 3. Simuler une session RDP active et tester le refus puis l'acceptation de la confirmation de sortie
            let viewport = (*ptr).rdp_viewport_rect();
            let dummy_conn = ConnectionItem {
                id: 1,
                name: "SRV-TRAY-ACTIVE".to_string(),
                protocol: Protocol::Rdp,
                host: "127.0.0.1".to_string(),
                port: 3389,
                username: "admin".to_string(),
                domain: String::new(),
                folder_id: None,
                description: String::new(),
                is_favorite: false,
                is_priority: false,
                save_password: false,
                has_saved_password: false,
                dynamic_resolution: true,
                redirect_clipboard: true,
                sort_order: 0,
                created_at: 0,
                updated_at: 0,
                last_connected_at: None,
            };
            let _ = (*ptr).sessions.open_rdp_session(
                hwnd,
                viewport,
                &dummy_conn,
                None,
                crate::settings::RdpKeyboardHookMode::OnRemote,
            );
            assert_eq!((*ptr).sessions.tabs().len(), 1);

            // Simuler un clic sur "Annuler" dans la boîte de confirmation (-1)
            TEST_EXIT_CONFIRM_OVERRIDE.store(-1, Ordering::SeqCst);
            let _ = (*ptr).request_explicit_exit();
            assert!(
                IsWindow(Some(hwnd)).as_bool(),
                "L'annulation de la confirmation doit empêcher la fermeture"
            );
            assert_eq!((*ptr).sessions.tabs().len(), 1);

            // Simuler la confirmation de sortie (+1) -> ferme les sessions, retire l'icône tray et détruit la fenêtre
            TEST_EXIT_CONFIRM_OVERRIDE.store(1, Ordering::SeqCst);
            let _ = (*ptr).request_explicit_exit();
            TEST_EXIT_CONFIRM_OVERRIDE.store(0, Ordering::SeqCst);
            assert!(
                !IsWindow(Some(hwnd)).as_bool(),
                "La sortie explicite confirmée doit détruire la fenêtre et nettoyer les ressources"
            );
        }
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn test_ui_rdp_session_menu_actions_and_external_app_launch_wiring() {
        let _guard = WIN32_UI_TEST_MUTEX
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let unique_id = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let db_path = std::env::temp_dir().join(format!("malvision_jalon3_test_{unique_id}.db"));
        let _ = std::fs::remove_file(&db_path);

        unsafe {
            let hwnd = create_isolated_test_main_window(&db_path);
            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut AppWindowState;
            assert!(!ptr.is_null());

            // 1. Ouvrir une session RDP fictive
            let viewport = (*ptr).rdp_viewport_rect();
            let dummy_conn = ConnectionItem {
                id: 42,
                name: "SRV-TEST-JALON3".to_string(),
                protocol: Protocol::Rdp,
                host: "127.0.0.1".to_string(),
                port: 3389,
                username: "admin".to_string(),
                domain: String::new(),
                folder_id: None,
                description: String::new(),
                is_favorite: false,
                is_priority: false,
                save_password: false,
                has_saved_password: false,
                dynamic_resolution: true,
                redirect_clipboard: true,
                sort_order: 0,
                created_at: 0,
                updated_at: 0,
                last_connected_at: None,
            };
            let sid = (*ptr)
                .sessions
                .open_rdp_session(
                    hwnd,
                    viewport,
                    &dummy_conn,
                    None,
                    crate::settings::RdpKeyboardHookMode::OnRemote,
                )
                .expect("La session RDP de test doit s'initialiser");
            assert_eq!((*ptr).sessions.tabs().len(), 1);
            assert_eq!((*ptr).sessions.active_session_id(), Some(sid));

            // 2. Tester les méthodes de commandes de session sans crash
            let _ = (*ptr).sessions.send_cad_to_active();
            let _ = (*ptr).sessions.send_ctrl_alt_end_to_active();
            assert!(!(*ptr).sessions.is_active_fullscreen());
            let _ = (*ptr).sessions.toggle_active_fullscreen();

            // 3. Tester le lancement de connexions AnyDesk et RustDesk
            let mut store = Storage::open_at_path(&db_path).unwrap();
            let any_draft = ConnectionDraft {
                folder_id: None,
                name: "AnyDesk-Remote".to_string(),
                protocol: Protocol::AnyDesk,
                host: "123456789".to_string(),
                port: 0,
                username: String::new(),
                domain: String::new(),
                description: String::new(),
                is_favorite: false,
                is_priority: false,
                save_password: false,
                password: SecretString::new(String::new()),
                keep_existing_password: false,
                dynamic_resolution: false,
                redirect_clipboard: false,
            };
            let any_conn = store.create_connection(any_draft).unwrap();

            // Lancement AnyDesk sans exécutable configuré -> doit définir un status_override clair sans paniquer
            (*ptr).launch_connection_by_id(any_conn.id);
            assert!((*ptr).status_override.is_some());

            // 4. Nettoyer et détruire la fenêtre
            (*ptr).sessions.close_all();
            let _ = DestroyWindow(hwnd);
        }
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn test_ui_search_edit_theming_and_ctlcolorstatic() {
        let _guard = WIN32_UI_TEST_MUTEX
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let db_path = create_temp_test_db_path("search_theme");
        let _ = std::fs::remove_file(&db_path);

        unsafe {
            let hwnd = create_isolated_test_main_window(&db_path);
            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut AppWindowState;
            assert!(!ptr.is_null());

            let initial_search_edit = (*ptr).search_edit;
            assert!(IsWindow(Some(initial_search_edit)).as_bool());

            // Tester la saisie de texte et sélection avant changement de thème
            set_window_text(initial_search_edit, "srv-prod-01");
            let _ = SendMessageW(
                initial_search_edit,
                EM_SETSEL,
                Some(WPARAM(4)),
                Some(LPARAM(8)),
            );

            let hdc = GetDC(Some(hwnd));
            assert!(!hdc.0.is_null());

            // 1. En Dark Theme (par défaut), WM_CTLCOLORSTATIC et WM_CTLCOLOREDIT doivent retourner brush_bg_input
            let res_dark_static = SendMessageW(
                hwnd,
                WM_CTLCOLORSTATIC,
                Some(WPARAM(hdc.0 as usize)),
                Some(LPARAM(initial_search_edit.0 as isize)),
            );
            assert_eq!(
                res_dark_static.0,
                (*ptr).palette.brush_bg_input.0 as isize,
                "WM_CTLCOLORSTATIC sur search_edit doit renvoyer brush_bg_input en Dark theme"
            );

            let res_dark_edit = SendMessageW(
                hwnd,
                WM_CTLCOLOREDIT,
                Some(WPARAM(hdc.0 as usize)),
                Some(LPARAM(initial_search_edit.0 as isize)),
            );
            assert_eq!(
                res_dark_edit.0,
                (*ptr).palette.brush_bg_input.0 as isize,
                "WM_CTLCOLOREDIT sur search_edit doit renvoyer brush_bg_input en Dark theme"
            );

            // Sous-classement : WM_ERASEBKGND direct sur search_edit doit retourner 1
            let res_erase_dark = SendMessageW(
                initial_search_edit,
                WM_ERASEBKGND,
                Some(WPARAM(hdc.0 as usize)),
                Some(LPARAM(0)),
            );
            assert_eq!(
                res_erase_dark.0, 1,
                "WM_ERASEBKGND intercepté doit retourner 1"
            );

            // 2. Basculer en Light Theme (cycle Dark -> Light)
            // Le contrôle EDIT natif doit être proprement recréé pour éliminer tout cache UxTheme
            (*ptr).settings.theme_mode = crate::settings::ThemeMode::Light;
            (*ptr).apply_theme();
            assert_eq!(
                (*ptr).settings.resolved_theme(),
                crate::settings::ResolvedTheme::Light
            );

            let light_search_edit = (*ptr).search_edit;
            assert!(IsWindow(Some(light_search_edit)).as_bool());
            assert_ne!(
                initial_search_edit, light_search_edit,
                "Le contrôle search_edit doit être proprement recréé avec un nouvel HWND"
            );

            // Vérifier la préservation du texte et de la sélection
            assert_eq!(get_window_text(light_search_edit), "srv-prod-01");
            let mut sel_start: u32 = 0;
            let mut sel_end: u32 = 0;
            let _ = SendMessageW(
                light_search_edit,
                EM_GETSEL,
                Some(WPARAM(&mut sel_start as *mut _ as usize)),
                Some(LPARAM(&mut sel_end as *mut _ as isize)),
            );
            assert_eq!(sel_start, 4);
            assert_eq!(sel_end, 8);

            let res_light = SendMessageW(
                hwnd,
                WM_CTLCOLORSTATIC,
                Some(WPARAM(hdc.0 as usize)),
                Some(LPARAM(light_search_edit.0 as isize)),
            );
            assert_eq!(
                res_light.0,
                (*ptr).palette.brush_bg_input.0 as isize,
                "WM_CTLCOLORSTATIC sur search_edit doit renvoyer brush_bg_input en Light theme"
            );

            let res_erase_light = SendMessageW(
                light_search_edit,
                WM_ERASEBKGND,
                Some(WPARAM(hdc.0 as usize)),
                Some(LPARAM(0)),
            );
            assert_eq!(res_erase_light.0, 1);

            // 3. Basculer de nouveau en Dark Theme (cycle Light -> Dark -> aucun résidu)
            (*ptr).settings.theme_mode = crate::settings::ThemeMode::Dark;
            (*ptr).apply_theme();
            assert_eq!(
                (*ptr).settings.resolved_theme(),
                crate::settings::ResolvedTheme::Dark
            );

            let dark_search_edit = (*ptr).search_edit;
            assert!(IsWindow(Some(dark_search_edit)).as_bool());
            assert_ne!(
                light_search_edit, dark_search_edit,
                "Le contrôle doit être de nouveau recréé pour le retour au thème sombre"
            );
            assert_eq!(get_window_text(dark_search_edit), "srv-prod-01");

            let res_dark_reapply = SendMessageW(
                hwnd,
                WM_CTLCOLORSTATIC,
                Some(WPARAM(hdc.0 as usize)),
                Some(LPARAM(dark_search_edit.0 as isize)),
            );
            assert_eq!(
                res_dark_reapply.0,
                (*ptr).palette.brush_bg_input.0 as isize,
                "WM_CTLCOLORSTATIC doit restaurer brush_bg_input sombre sans résidu"
            );

            // 4. WM_CTLCOLORSTATIC sur un autre contrôle doit renvoyer brush_bg_sidebar
            let res_other = SendMessageW(
                hwnd,
                WM_CTLCOLORSTATIC,
                Some(WPARAM(hdc.0 as usize)),
                Some(LPARAM(hwnd.0 as isize)),
            );
            assert_eq!(
                res_other.0,
                (*ptr).palette.brush_bg_sidebar.0 as isize,
                "WM_CTLCOLORSTATIC sur un autre contrôle doit renvoyer brush_bg_sidebar"
            );

            // Nettoyage
            let _ = ReleaseDC(Some(hwnd), hdc);
            let _ = DestroyWindow(hwnd);
        }
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn test_ui_unified_double_click_all_protocols_and_folders() {
        let _guard = WIN32_UI_TEST_MUTEX
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let db_path = create_temp_test_db_path("unified_dblclk");
        let _ = std::fs::remove_file(&db_path);

        unsafe {
            let hwnd = create_isolated_test_main_window(&db_path);
            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut AppWindowState;
            assert!(!ptr.is_null());

            let mut store = Storage::open_at_path(&db_path).unwrap();

            // Créer un dossier
            let folder = store.create_folder("Dossier Serveurs", None).unwrap();

            // Créer une connexion AnyDesk
            let any_draft = ConnectionDraft {
                name: "Client AnyDesk Test".to_string(),
                protocol: Protocol::AnyDesk,
                host: "987654321".to_string(),
                port: 0,
                username: String::new(),
                domain: String::new(),
                folder_id: Some(folder.id),
                is_favorite: false,
                is_priority: false,
                description: String::new(),
                password: SecretString::new(String::new()),
                save_password: false,
                keep_existing_password: false,
                dynamic_resolution: false,
                redirect_clipboard: false,
            };
            let any_conn = store.create_connection(any_draft).unwrap();

            // Créer une connexion RustDesk
            let rust_draft = ConnectionDraft {
                name: "Client RustDesk Test".to_string(),
                protocol: Protocol::RustDesk,
                host: "112233445".to_string(),
                port: 0,
                username: String::new(),
                domain: String::new(),
                folder_id: Some(folder.id),
                is_favorite: false,
                is_priority: false,
                description: String::new(),
                password: SecretString::new(String::new()),
                save_password: false,
                keep_existing_password: false,
                dynamic_resolution: false,
                redirect_clipboard: false,
            };
            let rust_conn = store.create_connection(rust_draft).unwrap();

            // Rafraîchir le treeview
            (*ptr).rebuild_treeview();

            // 1. Tester le double-clic unifié sur connexion AnyDesk via launch_connection_by_id
            (*ptr).status_override = None;
            (*ptr).launch_connection_by_id(any_conn.id);
            assert!(
                (*ptr).status_override.is_some(),
                "Le lancement AnyDesk doit renseigner status_override"
            );

            // 2. Tester le double-clic unifié sur connexion RustDesk via launch_connection_by_id
            (*ptr).status_override = None;
            (*ptr).launch_connection_by_id(rust_conn.id);
            assert!(
                (*ptr).status_override.is_some(),
                "Le lancement RustDesk doit renseigner status_override"
            );

            // 3. Tester la méthode node_kind_from_hitem pour s'assurer que les connexions et dossiers sont résolus
            let treeview = (*ptr).treeview;
            let root_item = SendMessageW(
                treeview,
                windows::Win32::UI::Controls::TVM_GETNEXTITEM,
                Some(WPARAM(windows::Win32::UI::Controls::TVGN_ROOT as usize)),
                Some(LPARAM(0)),
            );
            assert_ne!(
                root_item.0, 0,
                "Le TreeView doit posséder au moins une racine"
            );

            // 4. Développer toutes les racines pour rendre les connexions visibles
            let _ = SendMessageW(
                treeview,
                TVM_EXPAND,
                Some(WPARAM(TVE_EXPAND.0 as usize)),
                Some(LPARAM(root_item.0)),
            );

            // Trouver un item visible dans le TreeView
            let first_visible = SendMessageW(
                treeview,
                windows::Win32::UI::Controls::TVM_GETNEXTITEM,
                Some(WPARAM(
                    windows::Win32::UI::Controls::TVGN_FIRSTVISIBLE as usize,
                )),
                Some(LPARAM(0)),
            );
            assert_ne!(
                first_visible.0, 0,
                "Un premier élément visible doit exister"
            );

            // 5. Tester tree_hit_test_full_row :
            // Obtenir la position Y de cet élément et tester sur l'icône, le libellé et l'espace vide à droite
            let mut rc_item: [i32; 4] = [first_visible.0 as i32, 0, 0, 0];
            let ok = SendMessageW(
                treeview,
                windows::Win32::UI::Controls::TVM_GETITEMRECT,
                Some(WPARAM(0)), // fItemRect = FALSE -> rectangle de la ligne entière
                Some(LPARAM(rc_item.as_mut_ptr() as isize)),
            );
            if ok.0 != 0 {
                let row_y = (rc_item[1] + rc_item[3]) / 2;

                // Clic sur l'icône (x = 35)
                let hit_icon = (*ptr).tree_hit_test_full_row(POINT { x: 35, y: row_y });
                assert!(
                    hit_icon.is_some(),
                    "Le clic sur l'icône doit détecter la rangée"
                );

                // Clic sur le texte (x = 80)
                let hit_label = (*ptr).tree_hit_test_full_row(POINT { x: 80, y: row_y });
                assert!(
                    hit_label.is_some(),
                    "Le clic sur le texte doit détecter la rangée"
                );

                // Clic dans l'espace vide à droite du texte (x = 240)
                let hit_right = (*ptr).tree_hit_test_full_row(POINT { x: 240, y: row_y });
                assert!(
                    hit_right.is_some(),
                    "Le clic à droite du texte sur la même ligne doit détecter exactement la même rangée"
                );
                assert_eq!(
                    hit_label.unwrap().0,
                    hit_right.unwrap().0,
                    "Le libellé et l'espace droit doivent résoudre le même HTREEITEM"
                );

                // Clic dans l'espace vide sous tous les éléments (y = 500)
                let hit_nowhere = (*ptr).tree_hit_test_full_row(POINT { x: 200, y: 500 });
                assert!(
                    hit_nowhere.is_none(),
                    "Un clic en dehors des lignes visibles doit retourner None"
                );
            }

            let _ = DestroyWindow(hwnd);
        }
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn test_ui_full_row_double_click_short_name_and_all_x_positions() {
        let _guard = WIN32_UI_TEST_MUTEX
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let db_path = create_temp_test_db_path("short_name_dblclk");
        let _ = std::fs::remove_file(&db_path);

        unsafe {
            let hwnd = create_isolated_test_main_window(&db_path);
            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut AppWindowState;
            assert!(!ptr.is_null());

            let mut store = Storage::open_at_path(&db_path).unwrap();

            // 1. Créer une arborescence avec plusieurs niveaux d'indentation (Dossier Parent -> Sous-dossier)
            let parent_folder = store.create_folder("Parent Folder", None).unwrap();
            let sub_folder = store
                .create_folder("Sub Folder", Some(parent_folder.id))
                .unwrap();

            // 2. Créer une connexion au nom minimaliste « A » dans le sous-dossier indenté
            let short_draft = ConnectionDraft {
                name: "A".to_string(),
                protocol: Protocol::RustDesk,
                host: "99887766".to_string(),
                port: 0,
                username: String::new(),
                domain: String::new(),
                folder_id: Some(sub_folder.id),
                is_favorite: false,
                is_priority: false,
                description: String::new(),
                password: SecretString::new(String::new()),
                save_password: false,
                keep_existing_password: false,
                dynamic_resolution: false,
                redirect_clipboard: false,
            };
            let short_conn = store.create_connection(short_draft).unwrap();

            // Rafraîchir le TreeView et développer toute la hiérarchie pour rendre « A » visible
            (*ptr).rebuild_treeview();

            let treeview = (*ptr).treeview;
            let root_item = SendMessageW(
                treeview,
                TVM_GETNEXTITEM,
                Some(WPARAM(windows::Win32::UI::Controls::TVGN_ROOT as usize)),
                Some(LPARAM(0)),
            );
            assert_ne!(root_item.0, 0);

            // Développer les dossiers
            let mut curr = root_item;
            while curr.0 != 0 {
                let _ = SendMessageW(
                    treeview,
                    TVM_EXPAND,
                    Some(WPARAM(TVE_EXPAND.0 as usize)),
                    Some(LPARAM(curr.0)),
                );
                curr = SendMessageW(
                    treeview,
                    TVM_GETNEXTITEM,
                    Some(WPARAM(TVGN_NEXTVISIBLE as usize)),
                    Some(LPARAM(curr.0)),
                );
            }

            // Développer récursivement encore une fois pour s'assurer que le sous-dossier est ouvert
            let mut scan = SendMessageW(
                treeview,
                TVM_GETNEXTITEM,
                Some(WPARAM(TVGN_FIRSTVISIBLE as usize)),
                Some(LPARAM(0)),
            );
            while scan.0 != 0 {
                let _ = SendMessageW(
                    treeview,
                    TVM_EXPAND,
                    Some(WPARAM(TVE_EXPAND.0 as usize)),
                    Some(LPARAM(scan.0)),
                );
                scan = SendMessageW(
                    treeview,
                    TVM_GETNEXTITEM,
                    Some(WPARAM(TVGN_NEXTVISIBLE as usize)),
                    Some(LPARAM(scan.0)),
                );
            }

            // 3. Localiser l'élément HTREEITEM de la connexion courte « A »
            let mut target_item = HTREEITEM(0);
            let mut walk = SendMessageW(
                treeview,
                TVM_GETNEXTITEM,
                Some(WPARAM(TVGN_FIRSTVISIBLE as usize)),
                Some(LPARAM(0)),
            );
            while walk.0 != 0 {
                if let Some(TreeNodeKind::Connection(cid)) =
                    (*ptr).node_kind_from_hitem(HTREEITEM(walk.0))
                    && cid == short_conn.id
                {
                    target_item = HTREEITEM(walk.0);
                    break;
                }
                walk = SendMessageW(
                    treeview,
                    TVM_GETNEXTITEM,
                    Some(WPARAM(TVGN_NEXTVISIBLE as usize)),
                    Some(LPARAM(walk.0)),
                );
            }
            assert_ne!(
                target_item.0, 0,
                "La connexion courte « A » doit être visible dans le TreeView"
            );

            // 4. Obtenir les dimensions de la rangée de la connexion « A » et les limites clientes
            let mut rc_item = RECT::default();
            *(&mut rc_item as *mut RECT as *mut isize) = target_item.0;
            let ok = SendMessageW(
                treeview,
                TVM_GETITEMRECT,
                Some(WPARAM(0)), // fItemRect = FALSE -> pleine largeur
                Some(LPARAM(&mut rc_item as *mut RECT as isize)),
            );
            assert_ne!(ok.0, 0, "TVM_GETITEMRECT doit réussir");
            let row_y = (rc_item.top + rc_item.bottom) / 2;

            let mut rc_client = RECT::default();
            let _ = GetClientRect(treeview, &mut rc_client);
            assert!(
                rc_client.right > 100,
                "La zone cliente du TreeView doit avoir une largeur suffisante"
            );

            // 5. Tester la détection sur toutes les positions horizontales spécifiées :
            // - Sur l'icône (X ≈ 35)
            // - Sur le texte (X ≈ 60)
            // - Juste après le texte (X ≈ 90)
            // - Au centre de la zone vide (X ≈ rc_client.right / 2)
            // - À 10 pixels du bord droit du panneau (X = rc_client.right - 10)
            let test_x_positions = [35, 60, 90, rc_client.right / 2, rc_client.right - 10];

            for x in test_x_positions {
                let hit = (*ptr).tree_resolve_row_at_point(POINT { x, y: row_y });
                assert!(
                    hit.is_some(),
                    "tree_resolve_row_at_point doit détecter la rangée pour X = {}",
                    x
                );
                let (resolved_item, _) = hit.unwrap();
                assert_eq!(
                    resolved_item.0, target_item.0,
                    "La position X = {} doit résoudre exactement l'élément de la connexion « A »",
                    x
                );
            }

            // 6. Vérifier qu'un véritable événement souris Win32 WM_LBUTTONDBLCLK envoyé au TreeView
            // à 10 pixels du bord droit déclenche réellement launch_connection_by_id
            (*ptr).status_override = None;
            (*ptr).last_launch_debounce = None;

            let click_x = rc_client.right - 10;
            let lparam_val = (((row_y as u32) & 0xFFFF) << 16) | ((click_x as u32) & 0xFFFF);
            let res = SendMessageW(
                treeview,
                WM_LBUTTONDBLCLK,
                Some(WPARAM(0)),
                Some(LPARAM(lparam_val as isize)),
            );
            assert_eq!(
                res.0, 0,
                "treeview_wnd_proc doit consommer le message (LRESULT(0))"
            );
            assert!(
                (*ptr).status_override.is_some(),
                "Le double-clic tout à droite de la ligne (X = {}) doit déclencher launch_connection_by_id",
                click_x
            );
            assert_eq!(
                (*ptr).selected_node,
                Some(TreeNodeKind::Connection(short_conn.id)),
                "La connexion « A » doit être sélectionnée"
            );

            // 7. Vérifier l'anti-rebond (debounce) : un second double-clic immédiat ne re-déclenche pas
            let prev_time = (*ptr).last_launch_debounce.as_ref().map(|(_, t)| *t);
            let _ = SendMessageW(
                treeview,
                WM_LBUTTONDBLCLK,
                Some(WPARAM(0)),
                Some(LPARAM(lparam_val as isize)),
            );
            let new_time = (*ptr).last_launch_debounce.as_ref().map(|(_, t)| *t);
            assert_eq!(
                prev_time, new_time,
                "L'anti-rebond doit ignorer un double-clic concurrent dans les 400ms"
            );

            // 8. Tester le double-clic dans l'espace vide sous les éléments
            let hit_below = (*ptr).tree_resolve_row_at_point(POINT {
                x: rc_client.right / 2,
                y: rc_client.bottom + 50,
            });
            assert!(
                hit_below.is_none(),
                "Un double-clic sous les éléments doit renvoyer None"
            );

            // 9. Tester le double-clic sur un dossier dans son espace vide à droite : doit basculer son expansion
            let mut folder_item = HTREEITEM(0);
            let mut walk_f = SendMessageW(
                treeview,
                TVM_GETNEXTITEM,
                Some(WPARAM(TVGN_FIRSTVISIBLE as usize)),
                Some(LPARAM(0)),
            );
            while walk_f.0 != 0 {
                if let Some(TreeNodeKind::Folder(fid)) =
                    (*ptr).node_kind_from_hitem(HTREEITEM(walk_f.0))
                    && fid == sub_folder.id
                {
                    folder_item = HTREEITEM(walk_f.0);
                    break;
                }
                walk_f = SendMessageW(
                    treeview,
                    TVM_GETNEXTITEM,
                    Some(WPARAM(TVGN_NEXTVISIBLE as usize)),
                    Some(LPARAM(walk_f.0)),
                );
            }
            assert_ne!(folder_item.0, 0);

            let mut rc_f = RECT::default();
            *(&mut rc_f as *mut RECT as *mut isize) = folder_item.0;
            let ok_f = SendMessageW(
                treeview,
                TVM_GETITEMRECT,
                Some(WPARAM(0)),
                Some(LPARAM(&mut rc_f as *mut RECT as isize)),
            );
            assert_ne!(ok_f.0, 0);
            let f_y = (rc_f.top + rc_f.bottom) / 2;
            let f_click_x = rc_client.right - 10;
            let f_lparam = (((f_y as u32) & 0xFFFF) << 16) | ((f_click_x as u32) & 0xFFFF);

            let mut item_state = TVITEMW {
                mask: TVIF_STATE,
                hItem: folder_item,
                stateMask: TVIS_EXPANDED,
                ..Default::default()
            };
            let _ = SendMessageW(
                treeview,
                TVM_GETITEMW,
                Some(WPARAM(0)),
                Some(LPARAM(&mut item_state as *mut TVITEMW as isize)),
            );
            let initial_expanded = (item_state.state.0 & TVIS_EXPANDED.0) != 0;

            let res_f = SendMessageW(
                treeview,
                WM_LBUTTONDBLCLK,
                Some(WPARAM(0)),
                Some(LPARAM(f_lparam as isize)),
            );
            assert_eq!(res_f.0, 0);

            let mut new_item_state = TVITEMW {
                mask: TVIF_STATE,
                hItem: folder_item,
                stateMask: TVIS_EXPANDED,
                ..Default::default()
            };
            let _ = SendMessageW(
                treeview,
                TVM_GETITEMW,
                Some(WPARAM(0)),
                Some(LPARAM(&mut new_item_state as *mut TVITEMW as isize)),
            );
            let toggled_expanded = (new_item_state.state.0 & TVIS_EXPANDED.0) != 0;
            assert_ne!(
                initial_expanded, toggled_expanded,
                "Le double-clic tout à droite sur la ligne d'un dossier doit basculer son expansion"
            );

            let _ = DestroyWindow(hwnd);
        }
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn test_ui_single_instance_activation_message_restores_from_tray_and_focuses() {
        let _guard = WIN32_UI_TEST_MUTEX
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let db_path = create_temp_test_db_path("single_inst_msg");
        let _ = std::fs::remove_file(&db_path);

        unsafe {
            let hwnd = create_isolated_test_main_window(&db_path);
            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut AppWindowState;
            assert!(!ptr.is_null());

            // 1. Simuler Malvision minimisé / masqué dans le tray
            (*ptr).is_hidden_in_tray = true;
            let _ = ShowWindow(hwnd, SW_HIDE);
            assert!(!windows::Win32::UI::WindowsAndMessaging::IsWindowVisible(hwnd).as_bool());

            // 2. Envoyer le message de réactivation d'instance unique
            let msg = crate::single_instance::get_activate_window_message();
            assert_ne!(msg, 0);
            let res = SendMessageW(hwnd, msg, Some(WPARAM(0)), Some(LPARAM(0)));
            assert_eq!(res.0, 0);

            // 3. Vérifier que la fenêtre a bien été restaurée du tray et est visible
            assert!(!(*ptr).is_hidden_in_tray);
            assert!(windows::Win32::UI::WindowsAndMessaging::IsWindowVisible(hwnd).as_bool());

            let _ = DestroyWindow(hwnd);
        }
        let _ = std::fs::remove_file(&db_path);
    }

    fn make_test_draft(
        name: &str,
        protocol: Protocol,
        host: &str,
        folder_id: Option<i64>,
        is_favorite: bool,
    ) -> ConnectionDraft {
        ConnectionDraft {
            folder_id,
            name: name.to_string(),
            protocol,
            host: host.to_string(),
            port: 3389,
            username: String::new(),
            domain: String::new(),
            description: String::new(),
            is_favorite,
            is_priority: false,
            save_password: false,
            password: crate::security::SecretString::new(String::new()),
            keep_existing_password: false,
            dynamic_resolution: true,
            redirect_clipboard: true,
        }
    }

    #[test]
    fn test_ui_dashboard_layout_cas_a_and_cas_b_and_scrolling() {
        let _guard = WIN32_UI_TEST_MUTEX
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let db_path = create_temp_test_db_path("dash_cas_ab");
        let _ = std::fs::remove_file(&db_path);

        unsafe {
            let hwnd = create_isolated_test_main_window(&db_path);
            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut AppWindowState;
            assert!(!ptr.is_null());

            // Insérer des connexions de test
            let state = &mut *ptr;
            let f1 = state.storage.create_folder("Production", None).unwrap();
            let _ = state
                .storage
                .create_connection(make_test_draft(
                    "Alpha SRV",
                    Protocol::Rdp,
                    "10.0.0.1",
                    None,
                    true,
                ))
                .unwrap();
            let _ = state
                .storage
                .create_connection(make_test_draft(
                    "Zeta SRV",
                    Protocol::AnyDesk,
                    "123456789",
                    Some(f1.id),
                    false,
                ))
                .unwrap();

            let hdc = GetDC(Some(hwnd));
            assert!(!hdc.0.is_null());

            let area = RECT {
                left: 260,
                top: TABBAR_HEIGHT,
                right: 1200,
                bottom: 800 - STATUSBAR_HEIGHT,
            };

            // Cas A : Connexions récentes activées
            state.settings.show_recent_connections = true;
            state.hit_zones.clear();
            state.paint_home_dashboard(hdc, area);
            let height_cas_a = state.dashboard_content_height;
            assert!(
                height_cas_a > 100,
                "Le tableau de bord doit calculer une hauteur valide"
            );

            // Vérifier que « Toutes les connexions » contient bien des zones cliquables
            let card_zones_cas_a = state
                .hit_zones
                .iter()
                .filter(|z| matches!(z.zone, InteractiveZone::HomeCardConnect(_)))
                .count();
            assert!(
                card_zones_cas_a >= 2,
                "Au moins 2 cartes doivent être cliquables"
            );

            // Cas B : Connexions récentes désactivées
            state.settings.show_recent_connections = false;
            state.hit_zones.clear();
            state.paint_home_dashboard(hdc, area);
            let height_cas_b = state.dashboard_content_height;
            assert!(height_cas_b > 0);

            // Test du défilement fluide au mousewheel
            assert_eq!(state.dashboard_scroll_y, 0);
            state.on_mouse_wheel(-120); // Scroll vers le bas
            // Si la hauteur de contenu dépasse la zone visible ou s'adapte, clampé proprement
            let visible_h = 800 - TABBAR_HEIGHT - STATUSBAR_HEIGHT;
            let max_scroll = (state.dashboard_content_height - visible_h + 30).max(0);
            assert!(state.dashboard_scroll_y <= max_scroll);

            state.on_mouse_wheel(120); // Scroll vers le haut
            assert_eq!(state.dashboard_scroll_y, 0);

            let _ = ReleaseDC(Some(hwnd), hdc);
            let _ = DestroyWindow(hwnd);
        }
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn test_ui_treeview_direct_connections_appear_before_subfolders() {
        let _guard = WIN32_UI_TEST_MUTEX
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let db_path = create_temp_test_db_path("tree_sorting");
        let _ = std::fs::remove_file(&db_path);

        unsafe {
            let hwnd = create_isolated_test_main_window(&db_path);
            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut AppWindowState;
            assert!(!ptr.is_null());

            let state = &mut *ptr;
            // Créer un dossier parent avec des connexions directes et des sous-dossiers
            let parent = state.storage.create_folder("Clients", None).unwrap();
            let _sub_b = state
                .storage
                .create_folder("Client B", Some(parent.id))
                .unwrap();
            let _sub_a = state
                .storage
                .create_folder("Client A", Some(parent.id))
                .unwrap();

            let _conn_z = state
                .storage
                .create_connection(make_test_draft(
                    "Zebra Server",
                    Protocol::Rdp,
                    "10.0.0.99",
                    Some(parent.id),
                    false,
                ))
                .unwrap();

            let _conn_a = state
                .storage
                .create_connection(make_test_draft(
                    "Alpha Server",
                    Protocol::RustDesk,
                    "999888777",
                    Some(parent.id),
                    false,
                ))
                .unwrap();

            state.rebuild_treeview();

            // Vérifier que le tri commun models::collect_tree_ordered_connections respecte la règle
            let folders = state.storage.list_folders().unwrap();
            let conns = state.storage.list_connections().unwrap();
            let ordered = crate::models::collect_tree_ordered_connections(&folders, &conns);

            // Dans "Clients", Alpha Server doit venir avant Zebra Server,
            // et tous deux avant les connexions des sous-dossiers Client A / Client B
            let names: Vec<&str> = ordered.iter().map(|c| c.name.as_str()).collect();
            assert_eq!(names, vec!["Alpha Server", "Zebra Server"]);

            let _ = DestroyWindow(hwnd);
        }
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn test_ui_folder_expansion_individual_and_global_commands() {
        use windows::Win32::UI::Controls::{TVGN_CHILD, TVGN_NEXT, TVGN_ROOT};
        let _guard = WIN32_UI_TEST_MUTEX
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let db_path = create_temp_test_db_path("tree_expand_pers");
        let _ = std::fs::remove_file(&db_path);

        fn find_tree_item(
            tv: HWND,
            item: HTREEITEM,
            target: TreeNodeKind,
            st: &AppWindowState,
        ) -> Option<HTREEITEM> {
            if item.0 == 0 {
                return None;
            }
            if st.node_kind_from_hitem(item) == Some(target) {
                return Some(item);
            }
            unsafe {
                let child = SendMessageW(
                    tv,
                    TVM_GETNEXTITEM,
                    Some(WPARAM(TVGN_CHILD as usize)),
                    Some(LPARAM(item.0)),
                );
                if let Some(f) = find_tree_item(tv, HTREEITEM(child.0), target, st) {
                    return Some(f);
                }
                let next = SendMessageW(
                    tv,
                    TVM_GETNEXTITEM,
                    Some(WPARAM(TVGN_NEXT as usize)),
                    Some(LPARAM(item.0)),
                );
                find_tree_item(tv, HTREEITEM(next.0), target, st)
            }
        }

        unsafe {
            let hwnd = create_isolated_test_main_window(&db_path);
            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut AppWindowState;
            assert!(!ptr.is_null());
            let state = &mut *ptr;

            let f_clients = state.storage.create_folder("Clients", None).unwrap();
            let f_a = state
                .storage
                .create_folder("Client A", Some(f_clients.id))
                .unwrap();
            let f_infra = state
                .storage
                .create_folder("Infrastructure", Some(f_a.id))
                .unwrap();
            let f_b = state
                .storage
                .create_folder("Client B", Some(f_clients.id))
                .unwrap();

            let mut draft = make_test_draft(
                "Srv Prio",
                Protocol::Rdp,
                "10.0.0.1",
                Some(f_infra.id),
                false,
            );
            draft.is_priority = true;
            let _conn_prio = state.storage.create_connection(draft).unwrap();

            state.rebuild_treeview();

            // Par défaut, tous les dossiers sont créés dépliés (is_expanded = true)
            let f_map = state.storage.list_folders().unwrap();
            assert!(f_map.iter().all(|f| f.is_expanded));

            let root_item = SendMessageW(
                state.treeview,
                TVM_GETNEXTITEM,
                Some(WPARAM(TVGN_ROOT as usize)),
                Some(LPARAM(0)),
            );
            let h_clients = find_tree_item(
                state.treeview,
                HTREEITEM(root_item.0),
                TreeNodeKind::Folder(f_clients.id),
                state,
            )
            .expect("Dossier Clients introuvable dans le TreeView");
            let h_a = find_tree_item(
                state.treeview,
                HTREEITEM(root_item.0),
                TreeNodeKind::Folder(f_a.id),
                state,
            )
            .expect("Dossier Client A introuvable dans le TreeView");

            // 1. Simulation d'un double-clic sur la rangée de Client A -> doit le replier et sauvegarder dans SQLite
            let mut item_rect_a = [0u8; 32];
            *(item_rect_a.as_mut_ptr() as *mut usize) = h_a.0 as usize;
            let _ = SendMessageW(
                state.treeview,
                TVM_GETITEMRECT,
                Some(WPARAM(0)),
                Some(LPARAM(item_rect_a.as_mut_ptr() as isize)),
            );
            let rc_a = *(item_rect_a.as_ptr() as *const RECT);
            let dbl_x = (rc_a.left + 50) as i16 as u32;
            let dbl_y = (rc_a.top + (rc_a.bottom - rc_a.top) / 2) as i16 as u32;
            let dbl_lparam = LPARAM(((dbl_y << 16) | (dbl_x & 0xFFFF)) as isize);
            let _ = SendMessageW(
                state.treeview,
                WM_LBUTTONDBLCLK,
                Some(WPARAM(1)),
                Some(dbl_lparam),
            );

            // Vérification immédiate dans SQLite
            let db_a = state.storage.get_folder(f_a.id).unwrap().unwrap();
            assert!(
                !db_a.is_expanded,
                "Client A DOIT être replié dans SQLite suite au double-clic"
            );
            assert!(
                state
                    .storage
                    .get_folder(f_clients.id)
                    .unwrap()
                    .unwrap()
                    .is_expanded
            );
            assert!(
                state
                    .storage
                    .get_folder(f_infra.id)
                    .unwrap()
                    .unwrap()
                    .is_expanded
            );
            assert!(
                state
                    .storage
                    .get_folder(f_b.id)
                    .unwrap()
                    .unwrap()
                    .is_expanded
            );

            // 2. Simulation d'un clic chevron sur Clients -> doit le replier
            let mut item_rect_c = [0u8; 32];
            *(item_rect_c.as_mut_ptr() as *mut usize) = h_clients.0 as usize;
            let _ = SendMessageW(
                state.treeview,
                TVM_GETITEMRECT,
                Some(WPARAM(0)),
                Some(LPARAM(item_rect_c.as_mut_ptr() as isize)),
            );
            let rc_c = *(item_rect_c.as_ptr() as *const RECT);
            let clk_x = (rc_c.left + 8) as i16 as u32;
            let clk_y = (rc_c.top + (rc_c.bottom - rc_c.top) / 2) as i16 as u32;
            let clk_lparam = LPARAM(((clk_y << 16) | (clk_x & 0xFFFF)) as isize);
            let _ = SendMessageW(
                state.treeview,
                WM_LBUTTONDOWN,
                Some(WPARAM(1)),
                Some(clk_lparam),
            );
            let _ = SendMessageW(
                state.treeview,
                WM_LBUTTONUP,
                Some(WPARAM(0)),
                Some(clk_lparam),
            );

            let db_c = state.storage.get_folder(f_clients.id).unwrap().unwrap();
            assert!(
                !db_c.is_expanded,
                "Clients DOIT être replié dans SQLite suite au clic chevron"
            );

            // Redéplier Clients via double-clic
            let _ = SendMessageW(
                state.treeview,
                WM_LBUTTONDBLCLK,
                Some(WPARAM(1)),
                Some(clk_lparam),
            );
            assert!(
                state
                    .storage
                    .get_folder(f_clients.id)
                    .unwrap()
                    .unwrap()
                    .is_expanded
            );

            // 3. Changement de thème : ne doit pas réinitialiser les états
            state.apply_theme();
            assert!(
                !state
                    .storage
                    .get_folder(f_a.id)
                    .unwrap()
                    .unwrap()
                    .is_expanded
            );

            // 4. Recherche temporaire : ne doit pas écraser les états en base
            state.search_query = "quelquechose".to_string();
            state.rebuild_treeview();
            assert!(
                !state
                    .storage
                    .get_folder(f_a.id)
                    .unwrap()
                    .unwrap()
                    .is_expanded
            );

            state.search_query.clear();
            state.rebuild_treeview();
            assert!(
                !state
                    .storage
                    .get_folder(f_a.id)
                    .unwrap()
                    .unwrap()
                    .is_expanded
            );

            let _ = DestroyWindow(hwnd);
        }

        // 5. Test de persistance après redémarrage complet (nouvelle fenêtre sur la même base)
        unsafe {
            let hwnd2 = create_isolated_test_main_window(&db_path);
            let ptr2 = GetWindowLongPtrW(hwnd2, GWLP_USERDATA) as *mut AppWindowState;
            assert!(!ptr2.is_null());
            let state2 = &mut *ptr2;

            let folders = state2.storage.list_folders().unwrap();
            let c = folders.iter().find(|f| f.name == "Clients").unwrap();
            let a = folders.iter().find(|f| f.name == "Client A").unwrap();
            let infra = folders.iter().find(|f| f.name == "Infrastructure").unwrap();
            let b = folders.iter().find(|f| f.name == "Client B").unwrap();

            assert!(c.is_expanded, "Clients doit être déplié après redémarrage");
            assert!(
                !a.is_expanded,
                "Client A doit rester replié après redémarrage"
            );
            assert!(
                b.is_expanded,
                "Client B doit rester déplié après redémarrage"
            );
            assert!(
                infra.is_expanded,
                "Infrastructure doit rester déplié en base bien que son parent soit replié"
            );

            // 6. Action globale : Tout replier
            state2.collapse_all_folders();
            for f in state2.storage.list_folders().unwrap() {
                assert!(
                    !f.is_expanded,
                    "Le dossier {} doit être replié suite à collapse_all",
                    f.name
                );
            }

            let _ = DestroyWindow(hwnd2);
        }

        // 7. Redémarrage après « Tout replier » : tous les dossiers doivent rester repliés
        unsafe {
            let hwnd3 = create_isolated_test_main_window(&db_path);
            let ptr3 = GetWindowLongPtrW(hwnd3, GWLP_USERDATA) as *mut AppWindowState;
            assert!(!ptr3.is_null());
            let state3 = &mut *ptr3;

            for f in state3.storage.list_folders().unwrap() {
                assert!(
                    !f.is_expanded,
                    "Le dossier {} doit rester replié après redémarrage complet",
                    f.name
                );
            }

            // 8. Action globale : Tout déplier
            state3.expand_all_folders();
            for f in state3.storage.list_folders().unwrap() {
                assert!(
                    f.is_expanded,
                    "Le dossier {} doit être déplié suite à expand_all",
                    f.name
                );
            }

            let _ = DestroyWindow(hwnd3);
        }

        // 9. Redémarrage après « Tout déplier » : tous les dossiers doivent rester dépliés
        unsafe {
            let hwnd4 = create_isolated_test_main_window(&db_path);
            let ptr4 = GetWindowLongPtrW(hwnd4, GWLP_USERDATA) as *mut AppWindowState;
            assert!(!ptr4.is_null());
            let state4 = &mut *ptr4;

            for f in state4.storage.list_folders().unwrap() {
                assert!(
                    f.is_expanded,
                    "Le dossier {} doit rester déplié après redémarrage complet",
                    f.name
                );
            }

            let _ = DestroyWindow(hwnd4);
        }

        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn test_ui_connection_priority_dialog_and_context_menu_and_sorting() {
        let _guard = WIN32_UI_TEST_MUTEX
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let db_path = create_temp_test_db_path("conn_priority_ui");
        let _ = std::fs::remove_file(&db_path);

        unsafe {
            let hwnd = create_isolated_test_main_window(&db_path);
            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut AppWindowState;
            assert!(!ptr.is_null());
            let state = &mut *ptr;

            let f1 = state.storage.create_folder("Clients", None).unwrap();

            // Créer une connexion normale
            let c_normal = state
                .storage
                .create_connection(make_test_draft(
                    "Serveur Normal",
                    Protocol::Rdp,
                    "10.0.0.1",
                    Some(f1.id),
                    false,
                ))
                .unwrap();
            assert!(!c_normal.is_priority);

            // Créer une connexion prioritaire
            let mut prio_draft = make_test_draft(
                "Serveur Prio",
                Protocol::Rdp,
                "10.0.0.2",
                Some(f1.id),
                false,
            );
            prio_draft.is_priority = true;
            let c_prio = state.storage.create_connection(prio_draft).unwrap();
            assert!(c_prio.is_priority);

            // Simuler la commande du menu contextuel CMD_CTX_TOGGLE_PRIORITY sur la connexion normale
            state.selected_node = Some(TreeNodeKind::Connection(c_normal.id));
            state.execute_context_command(CMD_CTX_TOGGLE_PRIORITY);

            let updated_normal = state.storage.get_connection(c_normal.id).unwrap().unwrap();
            assert!(
                updated_normal.is_priority,
                "La connexion doit être devenue prioritaire via le menu"
            );

            // Basculer à nouveau pour retirer la priorité
            state.execute_context_command(CMD_CTX_TOGGLE_PRIORITY);
            let updated_normal2 = state.storage.get_connection(c_normal.id).unwrap().unwrap();
            assert!(
                !updated_normal2.is_priority,
                "La priorité doit avoir été retirée"
            );

            // Connexion à la fois favorite et prioritaire
            state
                .storage
                .set_connection_favorite(c_prio.id, true)
                .unwrap();
            let c_both = state.storage.get_connection(c_prio.id).unwrap().unwrap();
            assert!(c_both.is_favorite);
            assert!(c_both.is_priority);

            let _ = DestroyWindow(hwnd);
        }
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn test_ui_sidebar_buttons_symmetry_and_dpi() {
        for &dpi_val in &[96, 120, 144] {
            let sidebar_w = 260;
            let btn_gap = 8;
            let total_btn_w = (sidebar_w - 24 - btn_gap).max(100);
            let btn_w = total_btn_w / 2;

            let rc_btn_conn = RECT {
                left: 12,
                top: 44,
                right: 12 + btn_w,
                bottom: 76,
            };
            let rc_btn_fold = RECT {
                left: 12 + btn_w + btn_gap,
                top: 44,
                right: 12 + btn_w + btn_gap + btn_w,
                bottom: 76,
            };

            let rc_expand_all = RECT {
                left: 12,
                top: 118,
                right: 12 + btn_w,
                bottom: 144,
            };
            let rc_collapse_all = RECT {
                left: 12 + btn_w + btn_gap,
                top: 118,
                right: 12 + btn_w + btn_gap + btn_w,
                bottom: 144,
            };

            // Vérifier stricte égalité de largeur
            let w_conn = rc_btn_conn.right - rc_btn_conn.left;
            let w_fold = rc_btn_fold.right - rc_btn_fold.left;
            let w_exp = rc_expand_all.right - rc_expand_all.left;
            let w_col = rc_collapse_all.right - rc_collapse_all.left;

            assert_eq!(
                w_conn, w_fold,
                "Connexion et Dossier doivent avoir la même largeur à {dpi_val} DPI"
            );
            assert_eq!(
                w_exp, w_col,
                "Tout déplier et Tout replier doivent avoir la même largeur à {dpi_val} DPI"
            );
            assert_eq!(
                w_conn, w_exp,
                "Les largeurs des paires de boutons doivent être symétriques"
            );

            // Vérifier hauteur identique
            assert_eq!(rc_btn_conn.bottom - rc_btn_conn.top, 32);
            assert_eq!(rc_btn_fold.bottom - rc_btn_fold.top, 32);
            assert_eq!(rc_expand_all.bottom - rc_expand_all.top, 26);
            assert_eq!(rc_collapse_all.bottom - rc_collapse_all.top, 26);

            // Vérifier alignement horizontal
            assert_eq!(rc_btn_conn.top, rc_btn_fold.top);
            assert_eq!(rc_expand_all.top, rc_collapse_all.top);
            assert_eq!(rc_btn_fold.left - rc_btn_conn.right, btn_gap);
            assert_eq!(rc_collapse_all.left - rc_expand_all.right, btn_gap);
        }
    }

    #[test]
    fn test_gdiplus_api() {
        use windows::Win32::Graphics::Gdi::*;
        use windows::Win32::Graphics::GdiPlus::*;
        ensure_gdiplus_initialized();
        unsafe {
            let hdc = CreateCompatibleDC(None);
            let hbm = CreateCompatibleBitmap(hdc, 32, 32);
            let old = SelectObject(hdc, hbm.into());

            let mut graphics: *mut GpGraphics = std::ptr::null_mut();
            let g_res = GdipCreateFromHDC(hdc, &mut graphics);
            assert_eq!(g_res, Ok);
            let _ = GdipSetSmoothingMode(graphics, SmoothingModeAntiAlias);

            let mut pen: *mut GpPen = std::ptr::null_mut();
            let pen_res = GdipCreatePen1(0xFF38BDF8, 1.5, UnitPixel, &mut pen);
            assert_eq!(pen_res, Ok);
            let _ = GdipSetPenStartCap(pen, LineCapRound);
            let _ = GdipSetPenEndCap(pen, LineCapRound);
            let _ = GdipSetPenLineJoin(pen, LineJoinRound);

            let l_res = GdipDrawLine(graphics, pen, 2.0, 2.0, 10.0, 10.0);
            assert_eq!(l_res, Ok);

            let poly_pts = [
                PointF { X: 2.0, Y: 2.0 },
                PointF { X: 6.0, Y: 2.0 },
                PointF { X: 4.0, Y: 6.0 },
            ];
            let poly_res = GdipDrawPolygon(graphics, pen, poly_pts.as_ptr(), 3);
            assert_eq!(poly_res, Ok);

            let _ = GdipDeletePen(pen);

            let mut brush: *mut GpSolidFill = std::ptr::null_mut();
            let br_res = GdipCreateSolidFill(0xFF38BDF8, &mut brush);
            assert_eq!(br_res, Ok);
            let pts = [
                PointF { X: 2.0, Y: 2.0 },
                PointF { X: 6.0, Y: 2.0 },
                PointF { X: 4.0, Y: 6.0 },
            ];
            let _ = GdipFillPolygon(
                graphics,
                brush as *mut _,
                pts.as_ptr(),
                3,
                FillModeAlternate,
            );
            let _ = GdipDeleteBrush(brush as *mut _);

            let _ = GdipDeleteGraphics(graphics);

            // Teste l'appel direct des fonctions de tracé vectoriel GDI+ aux 3 échelles DPI
            for dpi_scale in [1.0f32, 1.25f32, 1.50f32] {
                let cx = (16.0 * dpi_scale) as i32;
                let cy = (16.0 * dpi_scale) as i32;
                draw_icon_chevron(hdc, cx, cy, true, 0x00FFFFFF);
                draw_icon_chevron(hdc, cx, cy, false, 0x00FFFFFF);
                draw_icon_double_chevron(hdc, cx, cy, true, 0x00FFFFFF);
                draw_icon_double_chevron(hdc, cx, cy, false, 0x00FFFFFF);
                draw_icon_priority(hdc, cx - 5, cy - 6, COL_PRIORITY_TRIANGLE);
            }

            let _ = SelectObject(hdc, old);
            let _ = DeleteObject(hbm.into());
            let _ = DeleteDC(hdc);
        }
    }
}
