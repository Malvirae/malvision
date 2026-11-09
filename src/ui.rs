//! Fenêtre principale moderne de Malvision (Jalon 2 & 2B — Gestionnaire de connexions,
//! arborescence multi-niveaux custom-draw avec glisser-déposer, recherche, favoris,
//! formulaires modaux sombres DPI-aware et onglets RDP simultanés).

use crate::models::{ConnectionDraft, ConnectionItem, Folder, Protocol};
use crate::rdp::{WM_RDP_EVENT, unpack_rdp_event_wparam};
use crate::security::SecretString;
use crate::sessions::{SessionManager, SessionStatus};
use crate::settings::{AppSettings, MAX_SIDEBAR_WIDTH, MIN_SIDEBAR_WIDTH};
use crate::storage::Storage;
use std::collections::HashMap;
use std::ffi::c_void;
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Dwm::{DWMWA_USE_IMMERSIVE_DARK_MODE, DwmSetWindowAttribute};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, ClientToScreen, CreateCompatibleBitmap, CreateCompatibleDC, CreateFontW, CreatePen,
    CreateSolidBrush, DT_CENTER, DT_END_ELLIPSIS, DT_LEFT, DT_NOPREFIX, DT_SINGLELINE, DT_VCENTER,
    DeleteDC, DeleteObject, DrawTextW, EndPaint, FONT_CHARSET, FONT_CLIP_PRECISION,
    FONT_OUTPUT_PRECISION, FONT_QUALITY, FillRect, GetStockObject, HBRUSH, HDC, HFONT,
    InvalidateRect, LineTo, MoveToEx, NULL_BRUSH, PAINTSTRUCT, PS_SOLID, RoundRect, ScreenToClient,
    SelectObject, SetBkColor, SetBkMode, SetTextColor, TRANSPARENT, UpdateWindow,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Controls::{
    CDDS_ITEMPREPAINT, CDDS_PREPAINT, CDIS_HOT, CDIS_SELECTED, CDRF_NOTIFYITEMDRAW,
    CDRF_SKIPDEFAULT, DRAWITEMSTRUCT, HTREEITEM, MEASUREITEMSTRUCT, NM_CLICK, NM_CUSTOMDRAW,
    NM_DBLCLK, NM_RCLICK, NMHDR, NMTREEVIEWW, NMTVCUSTOMDRAW, ODS_FOCUS, ODS_SELECTED, ODT_BUTTON,
    ODT_COMBOBOX, TV_FIRST, TVE_EXPAND, TVE_TOGGLE, TVGN_CARET, TVGN_DROPHILITE, TVHITTESTINFO,
    TVHT_ONITEMBUTTON, TVI_LAST, TVI_ROOT, TVIF_HANDLE, TVIF_PARAM, TVIF_STATE, TVIF_TEXT,
    TVINSERTSTRUCTW, TVIS_EXPANDED, TVITEMEXW, TVITEMW, TVM_DELETEITEM, TVM_EXPAND, TVM_GETITEMW,
    TVM_HITTEST, TVM_INSERTITEMW, TVM_SELECTITEM, TVM_SETBKCOLOR, TVM_SETTEXTCOLOR, TVN_BEGINDRAGW,
    TVN_ITEMEXPANDEDW, TVN_SELCHANGEDW, TVS_FULLROWSELECT, TVS_HASBUTTONS, TVS_SHOWSELALWAYS,
    WC_BUTTONW, WC_COMBOBOXW, WC_EDITW, WC_TREEVIEWW, WM_MOUSELEAVE,
};
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    EnableWindow, GetFocus, GetKeyState, IsWindowEnabled, ReleaseCapture, SetCapture, SetFocus,
    TME_LEAVE, TRACKMOUSEEVENT, TrackMouseEvent, VK_ESCAPE, VK_F2, VK_RETURN, VK_SHIFT, VK_SPACE,
    VK_TAB,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AdjustWindowRectEx, AppendMenuW, BM_GETCHECK, BM_SETCHECK, BS_OWNERDRAW, CB_ADDSTRING,
    CB_GETCURSEL, CB_GETLBTEXT, CB_GETLBTEXTLEN, CB_SETCURSEL, CBS_DROPDOWNLIST, CBS_HASSTRINGS,
    CBS_OWNERDRAWFIXED, CS_DBLCLKS, CS_HREDRAW, CS_VREDRAW, CW_USEDEFAULT, CallWindowProcW,
    CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyMenu, DestroyWindow, DispatchMessageW,
    EN_CHANGE, EN_KILLFOCUS, EN_SETFOCUS, ES_AUTOHSCROLL, ES_PASSWORD, GWL_STYLE, GWLP_USERDATA,
    GWLP_WNDPROC, GetClientRect, GetCursorPos, GetDlgCtrlID, GetMessageW, GetParent,
    GetWindowLongPtrW, GetWindowRect, GetWindowTextLengthW, GetWindowTextW, HMENU, IDC_ARROW,
    IDC_HAND, IDC_SIZEWE, IDYES, IsChild, IsIconic, IsWindow, KillTimer, LoadCursorW, MB_ICONERROR,
    MB_ICONINFORMATION, MB_ICONQUESTION, MB_ICONWARNING, MB_OK, MB_YESNO, MF_SEPARATOR, MF_STRING,
    MINMAXINFO, MSG, MessageBoxW, MoveWindow, PostMessageW, PostQuitMessage, RegisterClassExW,
    SW_SHOW, SendMessageW, SetCursor, SetForegroundWindow, SetTimer, SetWindowLongPtrW,
    SetWindowTextW, ShowWindow, TPM_LEFTALIGN, TPM_RETURNCMD, TPM_TOPALIGN, TrackPopupMenu,
    TranslateMessage, WINDOW_EX_STYLE, WINDOW_STYLE, WM_APP, WM_CLOSE, WM_COMMAND, WM_CREATE,
    WM_CTLCOLORBTN, WM_CTLCOLOREDIT, WM_CTLCOLORLISTBOX, WM_CTLCOLORSTATIC, WM_DESTROY,
    WM_DRAWITEM, WM_ERASEBKGND, WM_GETMINMAXINFO, WM_KEYDOWN, WM_KEYUP, WM_KILLFOCUS,
    WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MEASUREITEM, WM_MOUSEMOVE, WM_NOTIFY, WM_PAINT, WM_SETCURSOR,
    WM_SETFOCUS, WM_SETFONT, WM_SIZE, WM_SYSCOMMAND, WM_TIMER, WNDCLASSEXW, WNDPROC, WS_CAPTION,
    WS_CHILD, WS_CLIPCHILDREN, WS_MAXIMIZEBOX, WS_MINIMIZEBOX, WS_OVERLAPPED, WS_OVERLAPPEDWINDOW,
    WS_POPUP, WS_SYSMENU, WS_TABSTOP, WS_THICKFRAME, WS_VISIBLE, WS_VSCROLL,
};
use windows_core::{PCWSTR, w};

// =============================================================================
// CONSTANTES VISUELLES & COULEURS (THÈME SOMBRE WINDOWS 11 ANTHRACITE)
// =============================================================================

const COL_BG_DEEP: u32 = 0x001B1816; // #16181B Anthracite profond
const COL_BG_SIDEBAR: u32 = 0x00231F1C; // #1C1F23 Panneau gauche & en-têtes/pieds de dialogues
const COL_BG_SURFACE: u32 = 0x002D2723; // #23272D Cartes / onglets inactifs / boutons secondaires
const COL_BG_HOVER: u32 = 0x003A322C; // #2C323A Survol / sélection
const COL_BG_INPUT: u32 = 0x0026211E; // #1E2126 Champs de saisie & ComboBox
const COL_BG_INPUT_DISABLED: u32 = 0x00191614; // #141619 Champ désactivé
const COL_ACCENT: u32 = 0x00D47800; // #0078D4 Bleu Windows 11
const COL_ACCENT_HOVER: u32 = 0x00E88A1A; // #1A8AE8 Bleu survol
const COL_ACCENT_PRESSED: u32 = 0x009E5A00; // #005A9E Bleu pressé
const COL_BORDER: u32 = 0x00453D36; // #363D45 Bordure subtile
const COL_BORDER_SUBTLE: u32 = 0x00302A25; // #252A30 Bordure désactivée
const COL_TEXT_PRIMARY: u32 = 0x00F6F3F0; // #F0F3F6 Blanc doux
const COL_TEXT_MUTED: u32 = 0x00A69B8E; // #8E9BA6 Gris secondaire
const COL_STATUS_OK: u32 = 0x0050B02E; // #2EB050 Vert connecté
const COL_STATUS_WARN: u32 = 0x00209CE8; // #E89C20 Orange connexion en cours
const COL_STATUS_ERR: u32 = 0x004545E0; // #E04545 Rouge erreur
const COL_GOLD_STAR: u32 = 0x0030B8F2; // #F2B830 Étoile favori dorée
const COL_FOLDER_AMBER: u32 = 0x003EA0E0; // #E0A03E Icône dossier ambre
const COL_PROTO_ANYDESK: u32 = 0x003943D8; // #D84339 Badge AnyDesk
const COL_PROTO_RUSTDESK: u32 = 0x008A9616; // #16968A Badge RustDesk

const SIDEBAR_HEADER_HEIGHT: i32 = 126;
const SPLITTER_WIDTH: i32 = 5;
const TABBAR_HEIGHT: i32 = 38;
const STATUSBAR_HEIGHT: i32 = 28;
const MIN_WINDOW_WIDTH: i32 = 900;
const MIN_WINDOW_HEIGHT: i32 = 560;

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

// Commandes du menu contextuel de l'arborescence
const CMD_CTX_CONNECT: u32 = 3001;
const CMD_CTX_EDIT: u32 = 3002;
const CMD_CTX_RENAME: u32 = 3003;
const CMD_CTX_DUPLICATE: u32 = 3004;
const CMD_CTX_TOGGLE_FAV: u32 = 3005;
const CMD_CTX_MOVE: u32 = 3006;
const CMD_CTX_DELETE: u32 = 3007;
const CMD_CTX_NEW_CONN_HERE: u32 = 3008;
const CMD_CTX_NEW_SUBFOLDER: u32 = 3009;

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
    TabSelect(u32),
    TabClose(u32),
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
                brush_bg_deep: CreateSolidBrush(COLORREF(COL_BG_DEEP)),
                brush_bg_sidebar: CreateSolidBrush(COLORREF(COL_BG_SIDEBAR)),
                brush_bg_surface: CreateSolidBrush(COLORREF(COL_BG_SURFACE)),
                brush_bg_hover: CreateSolidBrush(COLORREF(COL_BG_HOVER)),
                brush_bg_input: CreateSolidBrush(COLORREF(COL_BG_INPUT)),
                brush_accent: CreateSolidBrush(COLORREF(COL_ACCENT)),
                brush_accent_hover: CreateSolidBrush(COLORREF(COL_ACCENT_HOVER)),
            }
        }
    }

    unsafe fn create_for_window(hwnd: HWND) -> Self {
        unsafe { Self::create_for_dpi(DpiMetrics::for_window(hwnd)) }
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
}

impl AppWindowState {
    unsafe fn initialize(hwnd: HWND, custom_storage: Option<Storage>) -> Self {
        unsafe {
            // Activer la barre de titre sombre native de Windows 11 (Immersive Dark Mode)
            let dark_mode: i32 = 1;
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWA_USE_IMMERSIVE_DARK_MODE,
                &dark_mode as *const i32 as *const c_void,
                std::mem::size_of::<i32>() as u32,
            );

            let palette = GdiPalette::create_for_window(hwnd);

            let mut status_override = None;
            let storage = if let Some(s) = custom_storage {
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

            let settings = AppSettings::load(&storage);
            let sidebar_w = settings.sidebar_width;

            let hinstance = GetModuleHandleW(None).unwrap_or_default();

            // Champ de recherche sans bordure Win32 (encapsulé dans un conteneur arrondi sombre dessiné en GDI)
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
            let tree_h = (client_h - SIDEBAR_HEADER_HEIGHT - STATUSBAR_HEIGHT).max(150);

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
                    Some(LPARAM(COL_BG_SIDEBAR as isize)),
                );
                let _ = SendMessageW(
                    treeview,
                    TVM_SETTEXTCOLOR,
                    Some(WPARAM(0)),
                    Some(LPARAM(COL_TEXT_PRIMARY as isize)),
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
            };

            state.rebuild_treeview();
            state
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
            let tree_h = (height - SIDEBAR_HEADER_HEIGHT - STATUSBAR_HEIGHT).max(100);
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
        if let Some(subfolders) = folders_by_parent.get(&parent_folder_id) {
            for folder in subfolders {
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
            }
        }

        if let Some(conns) = conns_by_folder.get(&parent_folder_id) {
            for conn in conns {
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
                    draw_icon_chevron(hdc, indent + 4, mid_y, expanded, COL_TEXT_MUTED);
                    draw_icon_star(hdc, indent + 16, mid_y - 7, COL_GOLD_STAR);
                    let _ = SelectObject(hdc, self.palette.font_ui_bold.into());
                    let _ = SetTextColor(hdc, COLORREF(COL_TEXT_PRIMARY));
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
                    draw_icon_chevron(hdc, indent + 4, mid_y, expanded, COL_TEXT_MUTED);
                    draw_icon_layers(hdc, indent + 16, mid_y - 7, COL_ACCENT);
                    let _ = SelectObject(hdc, self.palette.font_ui_bold.into());
                    let _ = SetTextColor(hdc, COLORREF(COL_TEXT_PRIMARY));
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
                    draw_icon_chevron(hdc, indent + 4, mid_y, expanded, COL_TEXT_MUTED);
                    draw_icon_folder(hdc, indent + 16, mid_y - 6, expanded, COL_FOLDER_AMBER);
                    let _ = SelectObject(
                        hdc,
                        if is_selected {
                            self.palette.font_ui_bold.into()
                        } else {
                            self.palette.font_ui.into()
                        },
                    );
                    let _ = SetTextColor(hdc, COLORREF(COL_TEXT_PRIMARY));
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
                            Protocol::Rdp => (COL_ACCENT, "RDP"),
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

                        let right_pad = if conn.is_favorite { 24 } else { 8 };
                        let _ = SelectObject(
                            hdc,
                            if is_selected {
                                self.palette.font_ui_bold.into()
                            } else {
                                self.palette.font_ui.into()
                            },
                        );
                        let _ = SetTextColor(hdc, COLORREF(COL_TEXT_PRIMARY));
                        draw_text_line(
                            hdc,
                            &conn.name,
                            RECT {
                                left: indent + 50,
                                top: row_rc.top,
                                right: (row_rc.right - right_pad).max(indent + 60),
                                bottom: row_rc.bottom,
                            },
                        );

                        if conn.is_favorite && row_rc.right - 18 > indent + 65 {
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
            let hitem = HTREEITEM(h.0);
            if hitem.0 == 0 {
                return;
            }
            if (hti.flags.0 & TVHT_ONITEMBUTTON.0) != 0 {
                return;
            }
            if let Some(kind) = self.node_kind_from_hitem(hitem)
                && matches!(
                    kind,
                    TreeNodeKind::FavoritesRoot
                        | TreeNodeKind::ConnectionsRoot
                        | TreeNodeKind::Folder(_)
                )
                && pt.x <= 48
            {
                let _ = SendMessageW(
                    self.treeview,
                    TVM_EXPAND,
                    Some(WPARAM(TVE_TOGGLE.0 as usize)),
                    Some(LPARAM(hitem.0)),
                );
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
            let _ = SetTextColor(hdc, COLORREF(COL_TEXT_PRIMARY));
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

            // Point d'accentuation bleu à côté du titre
            draw_filled_badge(
                hdc,
                RECT {
                    left: 114,
                    top: 14,
                    right: 158,
                    bottom: 32,
                },
                COL_ACCENT,
                "Win11",
                self.palette.font_small,
            );

            // Bouton "+ Connexion" et "+ Dossier"
            let btn_gap = 8;
            let total_btn_w = (sidebar_w - 24 - btn_gap).max(120);
            let btn_conn_w = (total_btn_w * 58) / 100;
            let btn_fold_w = total_btn_w - btn_conn_w;

            let rc_btn_conn = RECT {
                left: 12,
                top: 44,
                right: 12 + btn_conn_w,
                bottom: 76,
            };
            let rc_btn_fold = RECT {
                left: rc_btn_conn.right + btn_gap,
                top: 44,
                right: rc_btn_conn.right + btn_gap + btn_fold_w,
                bottom: 76,
            };

            let is_conn_hover = self.hovered_zone == Some(InteractiveZone::BtnNewConnection);
            draw_rounded_button(
                hdc,
                rc_btn_conn,
                if is_conn_hover {
                    COL_ACCENT_HOVER
                } else {
                    COL_ACCENT
                },
                COL_ACCENT_HOVER,
                "+ Connexion",
                self.palette.font_ui_bold,
                COL_TEXT_PRIMARY,
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
                    COL_BG_HOVER
                } else {
                    COL_BG_SURFACE
                },
                COL_BORDER,
                "+ Dossier",
                self.palette.font_ui,
                COL_TEXT_PRIMARY,
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
                COL_BG_INPUT,
                if is_search_focused {
                    COL_ACCENT
                } else {
                    COL_BORDER
                },
            );
            draw_icon_search(
                hdc,
                search_box_rc.left + 8,
                search_box_rc.top + 7,
                if is_search_focused {
                    COL_ACCENT_HOVER
                } else {
                    COL_TEXT_MUTED
                },
            );
            draw_horizontal_line(hdc, 0, sidebar_w, SIDEBAR_HEADER_HEIGHT - 1, COL_BORDER);

            // Séparateur vertical redimensionnable entre le panneau gauche et la zone centrale
            let splitter_rc = RECT {
                left: sidebar_w,
                top: 0,
                right: content_left,
                bottom: status_top,
            };
            let _ = FillRect(hdc, &splitter_rc, self.palette.brush_bg_sidebar);
            draw_vertical_line(hdc, sidebar_w, 0, status_top, COL_BORDER);

            // 3. Barre d'onglets supérieure de la zone centrale
            let tabbar_rc = RECT {
                left: content_left,
                top: 0,
                right: width,
                bottom: TABBAR_HEIGHT,
            };
            let _ = FillRect(hdc, &tabbar_rc, self.palette.brush_bg_sidebar);
            draw_horizontal_line(hdc, content_left, width, TABBAR_HEIGHT - 1, COL_BORDER);

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
                draw_horizontal_line(hdc, home_tab.left, home_tab.right, home_tab.top, COL_ACCENT);
                draw_horizontal_line(
                    hdc,
                    home_tab.left,
                    home_tab.right,
                    home_tab.top + 1,
                    COL_ACCENT,
                );
                draw_icon_home(
                    hdc,
                    home_tab.left + 14,
                    (home_tab.top + home_tab.bottom) / 2 - 7,
                    COL_ACCENT_HOVER,
                );
                let _ = SelectObject(hdc, self.palette.font_ui_bold.into());
                let _ = SetTextColor(hdc, COLORREF(COL_TEXT_PRIMARY));
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
                    draw_horizontal_line(hdc, tab_rc.left, tab_rc.right, tab_rc.top, COL_ACCENT);
                    draw_horizontal_line(
                        hdc,
                        tab_rc.left,
                        tab_rc.right,
                        tab_rc.top + 1,
                        COL_ACCENT,
                    );
                }

                // Indicateur coloré d'état de connexion
                let dot_color = match status {
                    SessionStatus::Connected | SessionStatus::LoggedIn => COL_STATUS_OK,
                    SessionStatus::Connecting => COL_STATUS_WARN,
                    SessionStatus::Error => COL_STATUS_ERR,
                    SessionStatus::Disconnected => COL_TEXT_MUTED,
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
                        COL_TEXT_PRIMARY
                    } else {
                        COL_TEXT_MUTED
                    }),
                );

                let tab_label = format!("{} {}", proto.badge_prefix(), title);
                draw_text_line(
                    hdc,
                    &tab_label,
                    RECT {
                        left: tab_rc.left + 24,
                        top: tab_rc.top,
                        right: tab_rc.right - 30,
                        bottom: tab_rc.bottom,
                    },
                );

                // Bouton de fermeture d'onglet "×"
                let close_rc = RECT {
                    left: tab_rc.right - 26,
                    top: tab_rc.top + 6,
                    right: tab_rc.right - 6,
                    bottom: tab_rc.bottom - 5,
                };
                let is_close_hover = self.hovered_zone == Some(InteractiveZone::TabClose(sid));
                if is_close_hover {
                    let err_brush = CreateSolidBrush(COLORREF(COL_STATUS_ERR));
                    let _ = FillRect(hdc, &close_rc, err_brush);
                    let _ = DeleteObject(err_brush.into());
                }
                let _ = SelectObject(hdc, self.palette.font_ui_bold.into());
                let _ = SetTextColor(hdc, COLORREF(COL_TEXT_PRIMARY));
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

                // Enregistrer d'abord le bouton Close (prioritaire au test de clic) puis l'onglet
                self.hit_zones.push(HitZone {
                    rect: close_rc,
                    zone: InteractiveZone::TabClose(sid),
                });
                self.hit_zones.push(HitZone {
                    rect: RECT {
                        left: tab_rc.left,
                        top: tab_rc.top,
                        right: close_rc.left,
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
                        COL_BG_HOVER
                    } else {
                        COL_BG_SURFACE
                    },
                    COL_BORDER,
                    "+",
                    self.palette.font_ui_bold,
                    COL_TEXT_PRIMARY,
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
            let mut y = area.top + 26;

            // En-tête d'accueil
            let _ = SelectObject(hdc, self.palette.font_title.into());
            let _ = SetTextColor(hdc, COLORREF(COL_TEXT_PRIMARY));
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
            let _ = SetTextColor(hdc, COLORREF(COL_TEXT_MUTED));
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
                    COL_ACCENT_HOVER
                } else {
                    COL_ACCENT
                },
                COL_ACCENT_HOVER,
                "+ Nouvelle connexion",
                self.palette.font_ui_bold,
                COL_TEXT_PRIMARY,
            );
            self.hit_zones.push(HitZone {
                rect: btn_new_conn,
                zone: InteractiveZone::HomeQuickNewConnection,
            });

            let hover_nf = self.hovered_zone == Some(InteractiveZone::HomeQuickNewFolder);
            draw_rounded_button(
                hdc,
                btn_new_fold,
                if hover_nf {
                    COL_BG_HOVER
                } else {
                    COL_BG_SURFACE
                },
                COL_BORDER,
                "+ Nouveau dossier",
                self.palette.font_ui,
                COL_TEXT_PRIMARY,
            );
            self.hit_zones.push(HitZone {
                rect: btn_new_fold,
                zone: InteractiveZone::HomeQuickNewFolder,
            });

            y += 50;

            // Section 1 : Connexions favorites
            let all_conns = self.storage.list_connections().unwrap_or_default();
            let favorites: Vec<&ConnectionItem> =
                all_conns.iter().filter(|c| c.is_favorite).take(6).collect();

            draw_icon_star(hdc, area.left + pad_x, y + 4, 0x0041B8FF);
            let _ = SelectObject(hdc, self.palette.font_subtitle.into());
            let _ = SetTextColor(hdc, COLORREF(COL_TEXT_PRIMARY));
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

            if favorites.is_empty() {
                let _ = SelectObject(hdc, self.palette.font_ui.into());
                let _ = SetTextColor(hdc, COLORREF(COL_TEXT_MUTED));
                draw_text_line(
                    hdc,
                    "Aucune connexion marquée en favori. Cochez « Favori » lors de la création ou via le clic droit.",
                    RECT {
                        left: area.left + pad_x,
                        top: y,
                        right: area.right - pad_x,
                        bottom: y + 24,
                    },
                );
                y += 38;
            } else {
                y = self.paint_connection_cards_grid(hdc, area, pad_x, y, &favorites);
                y += 18;
            }

            // Section 2 : Connexions récemment utilisées
            let recents = self.storage.list_recent_connections(6).unwrap_or_default();
            let recent_refs: Vec<&ConnectionItem> = recents.iter().map(|r| &r.connection).collect();

            if y + 90 < area.bottom {
                draw_icon_clock(hdc, area.left + pad_x, y + 4, COL_ACCENT_HOVER);
                let _ = SelectObject(hdc, self.palette.font_subtitle.into());
                let _ = SetTextColor(hdc, COLORREF(COL_TEXT_PRIMARY));
                draw_text_line(
                    hdc,
                    "Connexions récemment utilisées",
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
                    let _ = SetTextColor(hdc, COLORREF(COL_TEXT_MUTED));
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
                } else {
                    let _ = self.paint_connection_cards_grid(hdc, area, pad_x, y, &recent_refs);
                }
            }
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
                if y + card_h > area.bottom - 10 {
                    break;
                }

                let card_rc = RECT {
                    left: x,
                    top: y,
                    right: x + card_w,
                    bottom: y + card_h,
                };

                let is_hover = self.hovered_zone == Some(InteractiveZone::HomeCardConnect(conn.id));
                draw_rounded_box(
                    hdc,
                    card_rc,
                    if is_hover {
                        COL_BG_HOVER
                    } else {
                        COL_BG_SURFACE
                    },
                    if is_hover { COL_ACCENT } else { COL_BORDER },
                );

                // Badge protocole
                let proto_color = match conn.protocol {
                    Protocol::Rdp => COL_ACCENT,
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
                let _ = SetTextColor(hdc, COLORREF(COL_TEXT_PRIMARY));
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
                let _ = SetTextColor(hdc, COLORREF(COL_TEXT_MUTED));
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
                        COL_BG_DEEP
                    } else {
                        COL_BG_SURFACE
                    },
                    if is_edit_hover {
                        COL_ACCENT
                    } else {
                        COL_BORDER
                    },
                    "Modifier",
                    self.palette.font_small,
                    if is_edit_hover {
                        COL_TEXT_PRIMARY
                    } else {
                        COL_TEXT_MUTED
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
                COL_BORDER,
            );

            let (dot_color, status_text, proto_text, host_text) =
                if let Some(tab) = self.sessions.active_tab() {
                    let col = match tab.status {
                        SessionStatus::Connected | SessionStatus::LoggedIn => COL_STATUS_OK,
                        SessionStatus::Connecting => COL_STATUS_WARN,
                        SessionStatus::Error => COL_STATUS_ERR,
                        SessionStatus::Disconnected => COL_TEXT_MUTED,
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
                        COL_ACCENT,
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
                        COL_TEXT_MUTED,
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
            let _ = SetTextColor(hdc, COLORREF(COL_TEXT_PRIMARY));
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

            let _ = SetTextColor(hdc, COLORREF(COL_TEXT_MUTED));
            draw_vertical_line(
                hdc,
                status_right_limit + 6,
                status_rc.top + 5,
                status_rc.bottom - 5,
                COL_BORDER,
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
                COL_BORDER,
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
        let _ = SetTextColor(hdc, COLORREF(COL_TEXT_PRIMARY));
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

/// Dessine un chevron vectoriel (`>` replié ou `v` déplié) centré en `(cx, cy)`.
unsafe fn draw_icon_chevron(hdc: HDC, cx: i32, cy: i32, expanded: bool, color: u32) {
    unsafe {
        let pen = CreatePen(PS_SOLID, 2, COLORREF(color));
        let old = SelectObject(hdc, pen.into());
        if expanded {
            let _ = MoveToEx(hdc, cx - 4, cy - 2, None);
            let _ = LineTo(hdc, cx, cy + 2);
            let _ = LineTo(hdc, cx + 5, cy - 3);
        } else {
            let _ = MoveToEx(hdc, cx - 2, cy - 4, None);
            let _ = LineTo(hdc, cx + 2, cy);
            let _ = LineTo(hdc, cx - 3, cy + 5);
        }
        let _ = SelectObject(hdc, old);
        let _ = DeleteObject(pen.into());
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
        let border_col = if expanded { COL_ACCENT_HOVER } else { fill_col };
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

    fn tree_hit_test_screen_or_client(&self, main_x: i32, main_y: i32) -> Option<HTREEITEM> {
        unsafe {
            let mut pt = POINT {
                x: main_x,
                y: main_y,
            };
            let _ = ClientToScreen(self.hwnd, &mut pt);
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
            let hitem = HTREEITEM(h.0);
            if hitem.0 != 0 { Some(hitem) } else { None }
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
                match self
                    .sessions
                    .open_rdp_session(self.hwnd, viewport, &conn, decrypted_pwd)
                {
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
            Protocol::AnyDesk | Protocol::RustDesk => {
                let proto_name = conn.protocol.display_name();
                let info = format!(
                    "La fiche {} « {} » (identifiant : {}) est enregistrée et prête.\n\nLe lancement automatisé du client externe {} fait partie du Jalon 3.",
                    proto_name, conn.name, conn.host, proto_name
                );
                self.status_override = Some(format!(
                    "{proto_name} ({}) enregistré — Lancement client externe prévu au Jalon 3.",
                    conn.host
                ));
                self.invalidate_canvas();
                show_info_dialog(self.hwnd, &format!("Connexion {proto_name}"), &info);
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
            let mut hti = TVHITTESTINFO {
                pt: client_pt,
                ..Default::default()
            };
            let h = SendMessageW(
                self.treeview,
                TVM_HITTEST,
                Some(WPARAM(0)),
                Some(LPARAM(&mut hti as *mut TVHITTESTINFO as isize)),
            );
            let hitem = HTREEITEM(h.0);
            if hitem.0 != 0 {
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
                    let is_fav = self
                        .storage
                        .get_connection(cid)
                        .ok()
                        .flatten()
                        .is_some_and(|c| c.is_favorite);
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
                        .filter(|&h| IsWindow(Some(h)).as_bool() && IsWindowEnabled(h).as_bool())
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

/// Sous-classement de `WC_COMBOBOXW` pour remplacer le cadre gris/blanc natif par un rendu
/// sombre uniforme (fond `COL_BG_INPUT`, bordure `COL_BORDER` ou `COL_ACCENT` au focus/survol,
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

                let bg_parent = CreateSolidBrush(COLORREF(COL_BG_SIDEBAR));
                let _ = FillRect(hdc, &rc, bg_parent);
                let _ = DeleteObject(bg_parent.into());

                let mut cursor = POINT::default();
                let _ = GetCursorPos(&mut cursor);
                let _ = ScreenToClient(hwnd, &mut cursor);
                let hovered =
                    cursor.x >= 0 && cursor.x < rc.right && cursor.y >= 0 && cursor.y < rc.bottom;
                let focused = GetFocus() == hwnd;

                let border_col = if focused {
                    COL_ACCENT
                } else if hovered {
                    COL_ACCENT_HOVER
                } else {
                    COL_BORDER
                };
                draw_rounded_box(hdc, rc, COL_BG_INPUT, border_col);

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
                            let _ = SetTextColor(hdc, COLORREF(COL_TEXT_PRIMARY));
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
                        COL_TEXT_PRIMARY
                    } else {
                        COL_TEXT_MUTED
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

        let bg_parent = CreateSolidBrush(COLORREF(COL_BG_SIDEBAR));
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
                COL_ACCENT_PRESSED
            } else if hovered {
                COL_ACCENT_HOVER
            } else {
                COL_ACCENT
            };
            let border = if focused || hovered {
                COL_TEXT_PRIMARY
            } else {
                COL_ACCENT_HOVER
            };
            (fill, border, COL_TEXT_PRIMARY)
        } else {
            let fill = if pressed {
                COL_BG_INPUT
            } else if hovered {
                COL_BG_HOVER
            } else {
                COL_BG_SURFACE
            };
            let border = if focused {
                COL_ACCENT
            } else if hovered {
                COL_TEXT_MUTED
            } else {
                COL_BORDER
            };
            (fill, border, COL_TEXT_PRIMARY)
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

        let bg_parent = CreateSolidBrush(COLORREF(COL_BG_SIDEBAR));
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
            COL_BG_INPUT_DISABLED
        } else if checked {
            if hovered {
                COL_ACCENT_HOVER
            } else {
                COL_ACCENT
            }
        } else if hovered {
            COL_BG_HOVER
        } else {
            COL_BG_INPUT
        };

        let border_col = if !enabled {
            COL_BORDER_SUBTLE
        } else if focused || hovered || checked {
            COL_ACCENT_HOVER
        } else {
            COL_BORDER
        };

        draw_rounded_box(hdc, box_rc, fill_col, border_col);

        if checked {
            let pen = CreatePen(
                PS_SOLID,
                2,
                COLORREF(if enabled {
                    COL_TEXT_PRIMARY
                } else {
                    COL_TEXT_MUTED
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
                COL_TEXT_PRIMARY
            } else {
                COL_TEXT_MUTED
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

        let bg_col = if selected { COL_BG_HOVER } else { COL_BG_INPUT };
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
            let acc_brush = CreateSolidBrush(COLORREF(COL_ACCENT));
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
                let _ = SetTextColor(hdc, COLORREF(COL_TEXT_PRIMARY));
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

struct DialogLabel {
    text: String,
    rect: RECT,
    muted: bool,
    bold: bool,
}

struct DialogEditBox {
    hwnd: HWND,
    box_rect: RECT,
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
    host_label_text: String,
    host_label_rect: RECT,
    labels: Vec<DialogLabel>,
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
                    let _ = SetTextColor(hdc, COLORREF(COL_TEXT_PRIMARY));
                    let _ = SetBkColor(hdc, COLORREF(COL_BG_SIDEBAR));
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
                            COL_TEXT_PRIMARY
                        } else {
                            COL_TEXT_MUTED
                        }),
                    );
                    let _ = SetBkColor(hdc, COLORREF(COL_BG_INPUT));
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
                let _ = SetTextColor(hdc, COLORREF(COL_TEXT_PRIMARY));
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
                let _ = SetTextColor(hdc, COLORREF(COL_TEXT_MUTED));
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

                draw_horizontal_line(hdc, pad_x, w - pad_x, state.divider_y1, COL_BORDER);
                draw_horizontal_line(hdc, pad_x, w - pad_x, state.divider_y2, COL_BORDER);

                // Libellé dynamique d'hôte (RDP / AnyDesk / RustDesk)
                let _ = SelectObject(hdc, state.font_ui.into());
                let _ = SetTextColor(hdc, COLORREF(COL_TEXT_PRIMARY));
                draw_text_line(hdc, &state.host_label_text, state.host_label_rect);

                // Tous les autres libellés dessinés directement en mode TRANSPARENT
                for lbl in &state.labels {
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
                            COL_TEXT_MUTED
                        } else {
                            COL_TEXT_PRIMARY
                        }),
                    );
                    draw_text_line(hdc, &lbl.text, lbl.rect);
                }

                // Conteneurs sombres arrondis autour de chaque champ WC_EDITW
                let focus_hwnd = GetFocus();
                for eb in &state.edit_boxes {
                    let enabled = IsWindowEnabled(eb.hwnd).as_bool();
                    let focused = focus_hwnd == eb.hwnd;
                    let fill = if enabled { COL_BG_INPUT } else { COL_BG_DEEP };
                    let border = if focused { COL_ACCENT } else { COL_BORDER };
                    draw_rounded_box(hdc, eb.box_rect, fill, border);
                }

                // Encadré discret d'information sécurité DPAPI
                draw_rounded_box(hdc, state.security_banner_rc, COL_BG_DEEP, COL_BORDER);
                let _ = SelectObject(hdc, state.font_small.into());
                let _ = SetTextColor(hdc, COLORREF(COL_TEXT_MUTED));
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
                        let is_rdp = sel <= 0;
                        let _ = EnableWindow(state.ed_port, is_rdp);
                        let _ = EnableWindow(state.ed_domain, is_rdp);
                        let _ = EnableWindow(state.chk_dyn_res, is_rdp);
                        let _ = EnableWindow(state.chk_clipboard, is_rdp);
                        state.host_label_text = if is_rdp {
                            "Hôte RDP (IP / DNS) * :".to_string()
                        } else if sel == 1 {
                            "ID / Alias AnyDesk * :".to_string()
                        } else {
                            "ID / Adresse RustDesk * :".to_string()
                        };
                        let _ = InvalidateRect(Some(hwnd), None, false);
                        let _ = InvalidateRect(Some(state.chk_dyn_res), None, false);
                        let _ = InvalidateRect(Some(state.chk_clipboard), None, false);
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

        let dark_mode: i32 = 1;
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
            host_label_text: "Hôte RDP (IP / DNS) * :".to_string(),
            host_label_rect: RECT {
                left: pad_x,
                top: dpi.scale(210),
                right: field_left - dpi.scale(8),
                bottom: dpi.scale(210) + field_h,
            },
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
            chk_dyn_res: HWND::default(),
            chk_clipboard: HWND::default(),
            ed_desc: HWND::default(),
            result: None,
        });

        SetWindowLongPtrW(hwnd, GWLP_USERDATA, (&mut *dlg_state) as *mut _ as isize);

        let add_label = |labels: &mut Vec<DialogLabel>, text: &str, top: i32, h: i32| {
            labels.push(DialogLabel {
                text: text.to_string(),
                rect: RECT {
                    left: pad_x,
                    top,
                    right: field_left - dpi.scale(8),
                    bottom: top + h,
                },
                muted: false,
                bold: false,
            });
        };

        // 1. Nom de la connexion
        let y_name = dpi.scale(84);
        add_label(
            &mut dlg_state.labels,
            "Nom de la connexion * :",
            y_name,
            field_h,
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
        });

        // 2. Protocole
        let y_proto = dpi.scale(126);
        add_label(&mut dlg_state.labels, "Protocole :", y_proto, field_h);
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
        add_label(&mut dlg_state.labels, "Dossier parent :", y_folder, field_h);
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
        dlg_state.labels.push(DialogLabel {
            text: "Port :".to_string(),
            rect: RECT {
                left: rc_host.right + dpi.scale(12),
                top: y_host,
                right: rc_port.left - dpi.scale(4),
                bottom: y_host + field_h,
            },
            muted: false,
            bold: false,
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
        dlg_state.edit_boxes.push(DialogEditBox {
            hwnd: ed_host,
            box_rect: rc_host,
        });
        dlg_state.edit_boxes.push(DialogEditBox {
            hwnd: ed_port,
            box_rect: rc_port,
        });

        // 5. Utilisateur
        let y_user = dpi.scale(252);
        add_label(
            &mut dlg_state.labels,
            "Nom d'utilisateur :",
            y_user,
            field_h,
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
        });

        // 6. Domaine
        let y_dom = dpi.scale(294);
        add_label(
            &mut dlg_state.labels,
            "Domaine (facultatif) :",
            y_dom,
            field_h,
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
        });

        // 7. Mot de passe
        let y_pass = dpi.scale(336);
        add_label(&mut dlg_state.labels, "Mot de passe :", y_pass, field_h);
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
        });

        // 8. Cases à cocher larges (434 px à 96 DPI -> aucune troncature)
        let chk_h = dpi.scale(24);
        let y_chk1 = dpi.scale(376);
        let y_chk2 = dpi.scale(404);
        let y_chk3 = dpi.scale(432);
        let y_chk4 = dpi.scale(460);

        dlg_state.labels.push(DialogLabel {
            text: "Options de session :".to_string(),
            rect: RECT {
                left: pad_x,
                top: y_chk1,
                right: field_left - dpi.scale(8),
                bottom: y_chk1 + chk_h,
            },
            muted: false,
            bold: false,
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
        let y_desc = dpi.scale(494);
        add_label(&mut dlg_state.labels, "Description :", y_desc, field_h);
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
            dlg_state.host_label_text = match ex.protocol {
                Protocol::Rdp => "Hôte RDP (IP / DNS) * :".to_string(),
                Protocol::AnyDesk => "ID / Alias AnyDesk * :".to_string(),
                Protocol::RustDesk => "ID / Adresse RustDesk * :".to_string(),
            };
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

            let is_rdp = ex.protocol == Protocol::Rdp;
            let _ = EnableWindow(ed_port, is_rdp);
            let _ = EnableWindow(ed_domain, is_rdp);
            let _ = EnableWindow(chk_dyn_res, is_rdp);
            let _ = EnableWindow(chk_clipboard, is_rdp);
        } else {
            let _ = SendMessageW(cb_proto, CB_SETCURSEL, Some(WPARAM(0)), Some(LPARAM(0)));
            let _ = SendMessageW(chk_save_pass, BM_SETCHECK, Some(WPARAM(0)), Some(LPARAM(0)));
            let _ = EnableWindow(ed_pass, false);
            let _ = SendMessageW(chk_dyn_res, BM_SETCHECK, Some(WPARAM(1)), Some(LPARAM(0)));
            let _ = SendMessageW(chk_clipboard, BM_SETCHECK, Some(WPARAM(1)), Some(LPARAM(0)));
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
                    let _ = SetTextColor(hdc, COLORREF(COL_TEXT_PRIMARY));
                    let _ = SetBkColor(hdc, COLORREF(COL_BG_SIDEBAR));
                    return LRESULT((*ptr).brush_bg.0 as isize);
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            WM_CTLCOLOREDIT => {
                let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const TextPromptDialogState;
                if !ptr.is_null() {
                    let hdc = HDC(wparam.0 as *mut _);
                    let _ = SetTextColor(hdc, COLORREF(COL_TEXT_PRIMARY));
                    let _ = SetBkColor(hdc, COLORREF(COL_BG_INPUT));
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
                let _ = SetTextColor(hdc, COLORREF(COL_TEXT_PRIMARY));
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
                let _ = SetTextColor(hdc, COLORREF(COL_TEXT_MUTED));
                draw_text_line(hdc, &state.prompt_label, state.label_rc);

                let focused = GetFocus() == state.ed_input;
                draw_rounded_box(
                    hdc,
                    state.edit_box_rc,
                    COL_BG_INPUT,
                    if focused { COL_ACCENT } else { COL_BORDER },
                );

                draw_horizontal_line(hdc, pad_x, w - pad_x, state.divider_y, COL_BORDER);

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
                    let _ = SetTextColor(hdc, COLORREF(COL_TEXT_PRIMARY));
                    let _ = SetBkColor(hdc, COLORREF(COL_BG_SIDEBAR));
                    return LRESULT((*ptr).brush_bg.0 as isize);
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            WM_CTLCOLORLISTBOX | WM_CTLCOLOREDIT => {
                let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const FolderPickerDialogState;
                if !ptr.is_null() {
                    let hdc = HDC(wparam.0 as *mut _);
                    let _ = SetTextColor(hdc, COLORREF(COL_TEXT_PRIMARY));
                    let _ = SetBkColor(hdc, COLORREF(COL_BG_INPUT));
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
                let _ = SetTextColor(hdc, COLORREF(COL_TEXT_PRIMARY));
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
                let _ = SetTextColor(hdc, COLORREF(COL_TEXT_MUTED));
                draw_text_line(
                    hdc,
                    "Sélectionnez le dossier de destination :",
                    state.label_rc,
                );

                draw_horizontal_line(hdc, pad_x, w - pad_x, state.divider_y, COL_BORDER);

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
// WNDPROC PRINCIPAL, POMPE D'ÉVÉNEMENTS WIN32/OLE & HELPERS
// =============================================================================

impl Drop for AppWindowState {
    fn drop(&mut self) {
        unsafe {
            let _ = KillTimer(Some(self.hwnd), IDT_RDP_STATE_POLL);
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
                    let _ = SetTextColor(hdc, COLORREF(COL_TEXT_PRIMARY));
                    let _ = SetBkColor(hdc, COLORREF(COL_BG_INPUT));
                    return LRESULT((*ptr).palette.brush_bg_input.0 as isize);
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
                            TVN_ITEMEXPANDEDW => {
                                let nmtv = &*(lparam.0 as *const NMTREEVIEWW);
                                if let Some(TreeNodeKind::Folder(fid)) =
                                    TreeNodeKind::from_lparam(nmtv.itemNew.lParam.0)
                                {
                                    let expanded = (nmtv.action.0 & TVE_EXPAND.0) != 0;
                                    let _ = (*ptr).storage.set_folder_expanded(fid, expanded);
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
                                if let Some(TreeNodeKind::Connection(_)) = (*ptr).selected_node {
                                    let _ = PostMessageW(
                                        Some(hwnd),
                                        WM_DEFERRED_ACTION,
                                        WPARAM(DEFERRED_CONNECT_SELECTED),
                                        LPARAM(0),
                                    );
                                    return LRESULT(1);
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
                        _ => {}
                    }
                }
                LRESULT(0)
            }
            WM_RDP_EVENT => {
                let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut AppWindowState;
                if !ptr.is_null() {
                    let (session_id, event_code) = unpack_rdp_event_wparam(wparam);
                    let alert = (*ptr)
                        .sessions
                        .handle_rdp_event(session_id, event_code, lparam.0);
                    (*ptr).invalidate_canvas();
                    if let Some((title, msg_err)) = alert {
                        show_warning_dialog(hwnd, &title, &msg_err);
                    }
                }
                LRESULT(0)
            }
            WM_TIMER => {
                if wparam.0 == IDT_RDP_STATE_POLL {
                    let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut AppWindowState;
                    if !ptr.is_null() && !IsIconic(hwnd).as_bool() {
                        ensure_main_window_frame_styles(hwnd);
                        if (*ptr).sessions.sync_real_states() {
                            (*ptr).invalidate_canvas();
                        }
                    }
                }
                LRESULT(0)
            }
            WM_CLOSE => {
                let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut AppWindowState;
                if !ptr.is_null() {
                    (*ptr).sessions.close_all();
                }
                let _ = DestroyWindow(hwnd);
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
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
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

        let class_name = w!("MalvisionMainWindowClass");
        let bg_brush = CreateSolidBrush(COLORREF(COL_BG_DEEP));
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

        let _ = ShowWindow(hwnd, SW_SHOW);
        let _ = UpdateWindow(hwnd);

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
    use windows::Win32::UI::WindowsAndMessaging::{
        BM_CLICK, FindWindowExW, GetDlgItem, IsIconic, IsZoomed, SC_MAXIMIZE, SC_MINIMIZE,
        SC_RESTORE, TIMERPROC, WM_LBUTTONDOWN, WM_NCHITTEST, WM_NCLBUTTONDOWN,
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
}
