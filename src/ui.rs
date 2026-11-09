//! Fenêtre principale native Win32 de Malvision (Jalon 1B — Prototype RDP intégré).

use crate::rdp::{
    RDP_EVT_AUTH_WARNING_DISMISSED, RDP_EVT_AUTH_WARNING_DISPLAYED, RDP_EVT_AUTO_RECONNECTING,
    RDP_EVT_CONNECTED, RDP_EVT_CONNECTING, RDP_EVT_DISCONNECTED, RDP_EVT_FATAL_ERROR,
    RDP_EVT_LOGIN_COMPLETE, RDP_EVT_LOGON_ERROR, RDP_EVT_WARNING, RdpActiveXHost,
    RdpConnectionConfig, WM_RDP_EVENT, parse_host_and_port, split_domain_username,
};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    COLOR_BTNFACE, CreateFontW, DeleteObject, FONT_CHARSET, FONT_CLIP_PRECISION,
    FONT_OUTPUT_PRECISION, FONT_QUALITY, HBRUSH, HFONT, InvalidateRect, UpdateWindow,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{EnableWindow, GetFocus, SetFocus, VK_RETURN};
use windows::Win32::UI::WindowsAndMessaging::{
    BS_DEFPUSHBUTTON, BS_PUSHBUTTON, CS_HREDRAW, CS_VREDRAW, CW_USEDEFAULT, CreateWindowExW,
    DefWindowProcW, DispatchMessageW, ES_AUTOHSCROLL, ES_PASSWORD, GWLP_USERDATA, GetClientRect,
    GetMessageW, GetWindowLongPtrW, GetWindowTextLengthW, GetWindowTextW, HMENU, IDC_ARROW,
    IsChild, IsDialogMessageW, IsIconic, KillTimer, LoadCursorW, MB_ICONERROR, MB_ICONWARNING,
    MB_OK, MINMAXINFO, MSG, MessageBoxW, MoveWindow, PostQuitMessage, RegisterClassExW,
    SIZE_MINIMIZED, SW_SHOW, SendMessageW, SetTimer, SetWindowLongPtrW, SetWindowTextW, ShowWindow,
    TranslateMessage, WINDOW_EX_STYLE, WINDOW_STYLE, WM_CLOSE, WM_COMMAND, WM_CREATE, WM_DESTROY,
    WM_GETMINMAXINFO, WM_KEYDOWN, WM_SETFONT, WM_SIZE, WM_SYSCOMMAND, WM_TIMER, WNDCLASSEXW,
    WS_BORDER, WS_CAPTION, WS_CHILD, WS_CLIPCHILDREN, WS_MAXIMIZEBOX, WS_MINIMIZEBOX,
    WS_OVERLAPPED, WS_OVERLAPPEDWINDOW, WS_SYSMENU, WS_TABSTOP, WS_THICKFRAME, WS_VISIBLE,
};
use windows_core::{PCWSTR, w};

const TOOLBAR_HEIGHT: i32 = 46;
const STATUSBAR_HEIGHT: i32 = 28;
const MIN_WINDOW_WIDTH: i32 = 720;
const MIN_WINDOW_HEIGHT: i32 = 480;

const IDC_EDIT_HOST: usize = 1001;
const IDC_EDIT_USER: usize = 1002;
const IDC_EDIT_PASS: usize = 1003;
const IDC_BTN_CONNECT: usize = 1004;
const IDC_BTN_DISCONNECT: usize = 1005;
const IDC_STATIC_STATUS: usize = 1006;

/// Timer de supervision de l'état réel de `IMsRdpClient::get_Connected` (en complément des événements COM).
const IDT_RDP_STATE_POLL: usize = 2001;
const RDP_STATE_POLL_INTERVAL_MS: u32 = 350;

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

/// État interne de suivi de la session côté UI, toujours validé avec `IMsRdpClient::get_Connected`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiRdpState {
    Disconnected,
    Connecting,
    Connected,
    LoggedIn,
}

struct AppWindowState {
    hwnd: HWND,
    font: HFONT,
    edit_host: HWND,
    edit_user: HWND,
    edit_pass: HWND,
    btn_connect: HWND,
    btn_disconnect: HWND,
    status_label: HWND,
    rdp_host: Option<RdpActiveXHost>,
    last_target: String,
    ui_state: UiRdpState,
}

impl AppWindowState {
    unsafe fn initialize(hwnd: HWND) -> Self {
        unsafe {
            let hinstance = GetModuleHandleW(None).unwrap_or_default();

            let font = CreateFontW(
                -15,
                0,
                0,
                0,
                400,
                0,
                0,
                0,
                FONT_CHARSET(1),
                FONT_OUTPUT_PRECISION(0),
                FONT_CLIP_PRECISION(0),
                FONT_QUALITY(5), // CLEARTYPE_QUALITY
                0,
                w!("Segoe UI"),
            );

            let create_ctrl = |class: PCWSTR,
                               text: PCWSTR,
                               style: WINDOW_STYLE,
                               x: i32,
                               y: i32,
                               w: i32,
                               h: i32,
                               id: usize|
             -> HWND {
                let ctrl = CreateWindowExW(
                    WINDOW_EX_STYLE(0),
                    class,
                    text,
                    WS_CHILD | WS_VISIBLE | style,
                    x,
                    y,
                    w,
                    h,
                    Some(hwnd),
                    Some(HMENU(id as *mut _)),
                    Some(hinstance.into()),
                    None,
                )
                .unwrap_or_default();
                if !ctrl.is_invalid() && !font.is_invalid() {
                    let _ = SendMessageW(
                        ctrl,
                        WM_SETFONT,
                        Some(WPARAM(font.0 as usize)),
                        Some(LPARAM(1)),
                    );
                }
                ctrl
            };

            let _ = create_ctrl(
                w!("STATIC"),
                w!("Hôte / IP :"),
                WINDOW_STYLE(0),
                10,
                13,
                72,
                22,
                0,
            );
            let edit_host = create_ctrl(
                w!("EDIT"),
                w!(""),
                WS_BORDER | WS_TABSTOP | WINDOW_STYLE(ES_AUTOHSCROLL as u32),
                84,
                10,
                210,
                26,
                IDC_EDIT_HOST,
            );

            let _ = create_ctrl(
                w!("STATIC"),
                w!("Utilisateur :"),
                WINDOW_STYLE(0),
                306,
                13,
                78,
                22,
                0,
            );
            let edit_user = create_ctrl(
                w!("EDIT"),
                w!(""),
                WS_BORDER | WS_TABSTOP | WINDOW_STYLE(ES_AUTOHSCROLL as u32),
                386,
                10,
                175,
                26,
                IDC_EDIT_USER,
            );

            let _ = create_ctrl(
                w!("STATIC"),
                w!("Mot de passe :"),
                WINDOW_STYLE(0),
                573,
                13,
                92,
                22,
                0,
            );
            let edit_pass = create_ctrl(
                w!("EDIT"),
                w!(""),
                WS_BORDER | WS_TABSTOP | WINDOW_STYLE((ES_AUTOHSCROLL | ES_PASSWORD) as u32),
                668,
                10,
                165,
                26,
                IDC_EDIT_PASS,
            );

            let btn_connect = create_ctrl(
                w!("BUTTON"),
                w!("Connecter"),
                WS_TABSTOP | WINDOW_STYLE(BS_DEFPUSHBUTTON as u32),
                845,
                9,
                105,
                28,
                IDC_BTN_CONNECT,
            );

            let btn_disconnect = create_ctrl(
                w!("BUTTON"),
                w!("Déconnecter"),
                WS_TABSTOP | WINDOW_STYLE(BS_PUSHBUTTON as u32),
                958,
                9,
                110,
                28,
                IDC_BTN_DISCONNECT,
            );
            let _ = EnableWindow(btn_disconnect, false);

            let mut rc = RECT::default();
            let _ = GetClientRect(hwnd, &mut rc);
            let client_w = (rc.right - rc.left).max(640);
            let client_h = (rc.bottom - rc.top).max(480);
            let rdp_h = (client_h - TOOLBAR_HEIGHT - STATUSBAR_HEIGHT).max(200);

            let status_label = create_ctrl(
                w!("STATIC"),
                w!("Prêt — Contrôle RDP Microsoft (MsTscAx.dll) en cours d'initialisation..."),
                WINDOW_STYLE(0),
                10,
                TOOLBAR_HEIGHT + rdp_h + 5,
                client_w - 20,
                20,
                IDC_STATIC_STATUS,
            );

            let rdp_host = match RdpActiveXHost::create(hwnd, 0, TOOLBAR_HEIGHT, client_w, rdp_h) {
                Ok(host) => {
                    let ver = host.version_string();
                    let cid = host.control_id();
                    let cookie = host.advise_cookie();
                    set_window_text(
                        status_label,
                        &format!(
                            "Prêt — Contrôle ActiveX RDP Microsoft (MsTscAx.dll v{ver}, {cid}, sink=#{cookie}) hébergé."
                        ),
                    );
                    Some(host)
                }
                Err(err) => {
                    let msg = format!("Erreur d'initialisation ActiveX RDP : {err}");
                    set_window_text(status_label, &msg);
                    show_error_dialog(hwnd, "Erreur d'initialisation RDP", &msg);
                    None
                }
            };

            // Démarrer un timer léger de supervision pour synchroniser l'état avec IMsRdpClient::get_Connected()
            let _ = SetTimer(
                Some(hwnd),
                IDT_RDP_STATE_POLL,
                RDP_STATE_POLL_INTERVAL_MS,
                None,
            );

            let _ = SetFocus(Some(edit_host));

            Self {
                hwnd,
                font,
                edit_host,
                edit_user,
                edit_pass,
                btn_connect,
                btn_disconnect,
                status_label,
                rdp_host,
                last_target: String::new(),
                ui_state: UiRdpState::Disconnected,
            }
        }
    }

    fn on_size(&self, size_type: usize, width: i32, height: i32) {
        // Ne jamais redimensionner le contrôle ActiveX lorsque la fenêtre est minimisée (0x0)
        if size_type == SIZE_MINIMIZED as usize || width <= 0 || height <= 0 {
            return;
        }
        unsafe {
            ensure_main_window_frame_styles(self.hwnd);

            let rdp_y = TOOLBAR_HEIGHT;
            let rdp_h = (height - TOOLBAR_HEIGHT - STATUSBAR_HEIGHT).max(100);

            if let Some(ref host) = self.rdp_host {
                host.resize(0, rdp_y, width, rdp_h);
            }

            let status_y = TOOLBAR_HEIGHT + rdp_h + 5;
            let _ = MoveWindow(
                self.status_label,
                10,
                status_y,
                (width - 20).max(100),
                STATUSBAR_HEIGHT - 6,
                true,
            );
            let _ = InvalidateRect(Some(self.status_label), None, true);
        }
    }

    fn set_status(&self, message: &str) {
        unsafe {
            set_window_text(self.status_label, message);
            let _ = InvalidateRect(Some(self.status_label), None, true);
            let _ = UpdateWindow(self.status_label);
        }
    }

    fn update_controls_for_state(&self, active: bool) {
        unsafe {
            let _ = EnableWindow(self.edit_host, !active);
            let _ = EnableWindow(self.edit_user, !active);
            let _ = EnableWindow(self.edit_pass, !active);
            let _ = EnableWindow(self.btn_connect, !active);
            let _ = EnableWindow(self.btn_disconnect, active);
        }
    }

    fn target_display(&self) -> String {
        if self.last_target.is_empty() {
            "l'hôte distant".to_string()
        } else {
            self.last_target.clone()
        }
    }

    /// Vérifie périodiquement l'état réel de `IMsRdpClient::get_Connected` (`0`=déconnecté, `1`=connecté, `2`=connexion en cours)
    /// pour garantir que la barre de statut reflète toujours l'état véritable du contrôle ActiveX.
    fn sync_real_rdp_state(&mut self) {
        unsafe {
            ensure_main_window_frame_styles(self.hwnd);
        }

        let Some(ref host) = self.rdp_host else {
            return;
        };

        let real_state = host.connection_state();
        let target = self.target_display();

        match real_state {
            1 => {
                // Le contrôle confirme que la session RDP est réellement établie (Connected == 1)
                if self.ui_state == UiRdpState::Connecting
                    || self.ui_state == UiRdpState::Disconnected
                {
                    self.ui_state = UiRdpState::Connected;
                    self.update_controls_for_state(true);
                    self.set_status(&format!("Connecté à {target} — Session RDP active."));
                }
            }
            2 => {
                // Connexion en cours (Connected == 2)
                if self.ui_state == UiRdpState::Disconnected {
                    self.ui_state = UiRdpState::Connecting;
                    self.update_controls_for_state(true);
                    self.set_status(&format!(
                        "Connexion en cours vers {target} (NLA / négociation TLS)..."
                    ));
                }
            }
            0 if self.ui_state != UiRdpState::Disconnected => {
                // Déconnecté (Connected == 0) alors que l'UI pensait être connectée ou en cours
                self.ui_state = UiRdpState::Disconnected;
                self.update_controls_for_state(false);
                self.set_status(&format!("Déconnecté de {target} — Prêt."));
            }
            _ => {}
        }
    }

    fn handle_connect(&mut self) {
        let Some(ref rdp_host) = self.rdp_host else {
            show_error_dialog(
                self.hwnd,
                "Contrôle RDP indisponible",
                "Le contrôle ActiveX MsTscAx n'est pas initialisé.",
            );
            return;
        };

        let raw_host = unsafe { get_window_text(self.edit_host) };
        let raw_user = unsafe { get_window_text(self.edit_user) };
        let mut raw_pass = unsafe { get_window_text(self.edit_pass) };

        let (host, port) = match parse_host_and_port(&raw_host) {
            Ok(parsed) => parsed,
            Err(err) => {
                self.set_status(&format!("Paramètre invalide : {err}"));
                show_warning_dialog(self.hwnd, "Hôte RDP invalide", &err);
                unsafe {
                    let _ = SetFocus(Some(self.edit_host));
                }
                clear_sensitive_string(&mut raw_pass);
                return;
            }
        };

        let (domain, username) = split_domain_username(&raw_user);

        let mut rc = RECT::default();
        unsafe {
            let _ = GetClientRect(self.hwnd, &mut rc);
        }
        let rdp_w = (rc.right - rc.left).max(640) as u32;
        let rdp_h = (rc.bottom - rc.top - TOOLBAR_HEIGHT - STATUSBAR_HEIGHT).max(480) as u32;

        let mut config = RdpConnectionConfig {
            host: host.clone(),
            port,
            domain,
            username,
            password: raw_pass.clone(),
            width: rdp_w,
            height: rdp_h,
        };

        // Effacer immédiatement le champ mot de passe de l'interface pour ne jamais le conserver visible/en mémoire UI
        clear_sensitive_string(&mut raw_pass);
        unsafe {
            set_window_text(self.edit_pass, "");
        }

        let target_label = format!("{host}:{port}");
        self.set_status(&format!(
            "Initialisation de la connexion RDP vers {target_label}..."
        ));

        let connect_result = rdp_host.connect(&config);
        clear_sensitive_string(&mut config.password);

        match connect_result {
            Ok(()) => {
                self.last_target = target_label.clone();
                self.ui_state = UiRdpState::Connecting;
                self.update_controls_for_state(true);
                self.set_status(&format!(
                    "Connexion en cours vers {target_label} (NLA / négociation TLS)..."
                ));
            }
            Err(err) => {
                self.ui_state = UiRdpState::Disconnected;
                self.update_controls_for_state(false);
                let msg = format!("Échec du lancement de la connexion vers {target_label} : {err}");
                self.set_status(&msg);
                show_error_dialog(self.hwnd, "Erreur de connexion RDP", &msg);
            }
        }
    }

    fn handle_disconnect(&mut self) {
        if let Some(ref rdp_host) = self.rdp_host {
            if let Err(err) = rdp_host.disconnect() {
                let msg = format!("Erreur lors de la déconnexion : {err}");
                self.set_status(&msg);
                show_error_dialog(self.hwnd, "Erreur de déconnexion RDP", &msg);
            } else {
                self.set_status("Déconnexion RDP en cours...");
            }
        }
    }

    fn handle_rdp_event(&mut self, event_code: usize, param: isize) {
        let target = self.target_display();
        let real_conn_state = self
            .rdp_host
            .as_ref()
            .map_or(0, RdpActiveXHost::connection_state);

        match event_code {
            RDP_EVT_CONNECTING => {
                self.ui_state = UiRdpState::Connecting;
                self.update_controls_for_state(true);
                self.set_status(&format!(
                    "Connexion RDP en cours vers {target} (NLA / négociation TLS)..."
                ));
            }
            RDP_EVT_CONNECTED => {
                // Toujours vérifier la confirmation réelle du contrôle (get_Connected() == 1)
                if real_conn_state == 1 {
                    self.ui_state = UiRdpState::Connected;
                    self.update_controls_for_state(true);
                    self.set_status(&format!(
                        "Connecté à {target} — Ouverture de session en cours..."
                    ));
                    if let Some(ref host) = self.rdp_host {
                        host.focus_control();
                    }
                }
            }
            RDP_EVT_LOGIN_COMPLETE => {
                if real_conn_state == 1 {
                    self.ui_state = UiRdpState::LoggedIn;
                    self.update_controls_for_state(true);
                    self.set_status(&format!(
                        "Connecté à {target} — Session RDP authentifiée et prête."
                    ));
                    if let Some(ref host) = self.rdp_host {
                        host.focus_control();
                    }
                }
            }
            RDP_EVT_AUTH_WARNING_DISPLAYED => {
                self.set_status(&format!(
                    "Avertissement de certificat / sécurité Windows affiché pour {target}. Veuillez répondre à la boîte de dialogue système."
                ));
            }
            RDP_EVT_AUTH_WARNING_DISMISSED => {
                self.set_status(&format!(
                    "Dialogue de sécurité Windows validé pour {target}, poursuite de la connexion..."
                ));
            }
            RDP_EVT_AUTO_RECONNECTING => {
                self.ui_state = UiRdpState::Connecting;
                self.set_status(&format!(
                    "Reconnexion automatique RDP en cours vers {target} (code={param})..."
                ));
            }
            RDP_EVT_LOGON_ERROR => {
                let code = param as i32;
                // -2 (0xFFFFFFFE) signifie LOGON_FAILED_OTHER / continuation normale d'ouverture de session Windows
                if code != -2 {
                    self.set_status(&format!(
                        "Notification d'ouverture de session Windows sur {target} (code={code})."
                    ));
                }
            }
            RDP_EVT_WARNING => {
                let code = param as i32;
                self.set_status(&format!(
                    "Avertissement RDP reçu de {target} (code={code})."
                ));
            }
            RDP_EVT_FATAL_ERROR => {
                let code = param as i32;
                self.ui_state = UiRdpState::Disconnected;
                self.update_controls_for_state(false);
                let msg =
                    format!("Erreur fatale du contrôle RDP sur {target} (code d'erreur={code}).");
                self.set_status(&msg);
                show_error_dialog(self.hwnd, "Erreur fatale RDP", &msg);
            }
            RDP_EVT_DISCONNECTED => {
                let disc_reason = param as i32;
                self.ui_state = UiRdpState::Disconnected;
                self.update_controls_for_state(false);

                let (ext_reason, desc) = if let Some(ref host) = self.rdp_host {
                    host.describe_disconnect(disc_reason)
                } else {
                    (0, String::new())
                };

                // Les raisons 1..=3 avec ext_reason == 0 ou 12 (logoff utilisateur) sont des fermetures normales
                let is_normal_close = matches!(disc_reason, 1..=3) && matches!(ext_reason, 0 | 12);
                if is_normal_close {
                    self.set_status(&format!("Déconnecté de {target} ({desc})"));
                } else {
                    let full_err = format!(
                        "Déconnexion de {target} [code={disc_reason}, étendu={ext_reason}] : {desc}"
                    );
                    self.set_status(&full_err);
                    show_warning_dialog(self.hwnd, "Session RDP interrompue", &full_err);
                }
            }
            _ => {}
        }
    }
}

impl Drop for AppWindowState {
    fn drop(&mut self) {
        unsafe {
            let _ = KillTimer(Some(self.hwnd), IDT_RDP_STATE_POLL);
        }
        // Libérer le contrôle ActiveX avant la police GDI
        self.rdp_host = None;
        unsafe {
            if !self.font.is_invalid() {
                let _ = DeleteObject(self.font.into());
            }
        }
    }
}

/// Garantit que `MsTscAx.dll` n'a pas altéré les styles de bordure / maximisation / minimisation
/// de la fenêtre principale (`WS_OVERLAPPEDWINDOW`).
unsafe fn ensure_main_window_frame_styles(hwnd: HWND) {
    unsafe {
        use windows::Win32::UI::WindowsAndMessaging::{
            GWL_STYLE, GetWindowLongPtrW, SetWindowLongPtrW,
        };
        let current = GetWindowLongPtrW(hwnd, GWL_STYLE) as u32;
        let required = MAIN_WINDOW_BASE_STYLE.0;
        if (current & required) != required {
            SetWindowLongPtrW(hwnd, GWL_STYLE, (current | required) as isize);
        }
    }
}

fn clear_sensitive_string(s: &mut String) {
    unsafe {
        for b in s.as_bytes_mut() {
            std::ptr::write_volatile(b, 0);
        }
    }
    s.clear();
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

unsafe extern "system" fn main_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        match msg {
            WM_CREATE => {
                let state = Box::new(AppWindowState::initialize(hwnd));
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(state) as isize);
                LRESULT(0)
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
                    let width = (lparam.0 & 0xFFFF) as i16 as i32;
                    let height = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;
                    (*ptr).on_size(wparam.0, width, height);
                }
                LRESULT(0)
            }
            WM_TIMER => {
                if wparam.0 == IDT_RDP_STATE_POLL {
                    let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut AppWindowState;
                    if !ptr.is_null() && !IsIconic(hwnd).as_bool() {
                        (*ptr).sync_real_rdp_state();
                    }
                }
                LRESULT(0)
            }
            WM_COMMAND => {
                let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut AppWindowState;
                if !ptr.is_null() {
                    let control_id = wparam.0 & 0xFFFF;
                    let notify_code = (wparam.0 >> 16) & 0xFFFF;
                    if notify_code == 0 {
                        match control_id {
                            IDC_BTN_CONNECT => (*ptr).handle_connect(),
                            IDC_BTN_DISCONNECT => (*ptr).handle_disconnect(),
                            _ => {}
                        }
                    }
                }
                LRESULT(0)
            }
            WM_RDP_EVENT => {
                let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut AppWindowState;
                if !ptr.is_null() {
                    (*ptr).handle_rdp_event(wparam.0, lparam.0);
                }
                LRESULT(0)
            }
            WM_CLOSE => {
                let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut AppWindowState;
                if !ptr.is_null()
                    && let Some(ref host) = (*ptr).rdp_host
                    && host.is_active()
                {
                    let _ = host.disconnect();
                }
                let _ = windows::Win32::UI::WindowsAndMessaging::DestroyWindow(hwnd);
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
///
/// Ne jamais passer les messages souris (`WM_NCLBUTTONDOWN`, `WM_NCHITTEST`, `WM_SYSCOMMAND`, etc.)
/// à `IOleInPlaceActiveObject::TranslateAccelerator` ou `IsDialogMessageW` : sinon le contrôle ActiveX
/// ou le gestionnaire de boîte de dialogue consomme ou bloque les clics sur les boutons Minimiser / Restaurer
/// de la barre de titre lorsque la fenêtre est maximisée.
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
        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(main_wnd_proc),
            hInstance: hinstance.into(),
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            hbrBackground: HBRUSH((COLOR_BTNFACE.0 + 1) as *mut _),
            lpszClassName: class_name,
            ..Default::default()
        };

        let _ = RegisterClassExW(&wc);

        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            class_name,
            w!("Malvision — Prototype RDP Intégré (MsTscAx.dll)"),
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
        .map_err(|e| format!("Échec de CreateWindowExW : {e}"))?;

        let _ = ShowWindow(hwnd, SW_SHOW);
        let _ = UpdateWindow(hwnd);

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).into() {
            let mut handled = false;

            // Ne filtrer que les messages clavier destinés aux contrôles enfants;
            // laisser tous les messages souris / non-client (barre de titre, boutons min/max/restore,
            // redimensionnement, WM_SYSCOMMAND) aller directement à DispatchMessageW.
            if is_keyboard_message(msg.message) {
                let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut AppWindowState;
                if !ptr.is_null() {
                    let state = &mut *ptr;
                    let focus_hwnd = GetFocus();

                    if let Some(ref rdp_host) = state.rdp_host {
                        let ax_hwnd = rdp_host.hwnd();
                        let focus_in_rdp =
                            focus_hwnd == ax_hwnd || IsChild(ax_hwnd, focus_hwnd).as_bool();

                        if focus_in_rdp {
                            if rdp_host.translate_accelerator(&msg) {
                                handled = true;
                            }
                        } else if msg.message == WM_KEYDOWN
                            && msg.wParam.0 == VK_RETURN.0 as usize
                            && (focus_hwnd == state.edit_host
                                || focus_hwnd == state.edit_user
                                || focus_hwnd == state.edit_pass)
                        {
                            state.handle_connect();
                            handled = true;
                        } else if IsChild(hwnd, focus_hwnd).as_bool()
                            && IsDialogMessageW(hwnd, &msg).as_bool()
                        {
                            handled = true;
                        }
                    } else if IsChild(hwnd, focus_hwnd).as_bool()
                        && IsDialogMessageW(hwnd, &msg).as_bool()
                    {
                        handled = true;
                    }
                }
            }

            if !handled {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::UI::WindowsAndMessaging::{
        GWL_STYLE, GetWindowLongPtrW, IsIconic, IsZoomed, SC_MAXIMIZE, SC_MINIMIZE, SC_RESTORE,
        WM_LBUTTONDOWN, WM_NCHITTEST, WM_NCLBUTTONDOWN,
    };

    #[test]
    fn test_is_keyboard_message_excludes_mouse_and_non_client() {
        assert!(is_keyboard_message(WM_KEYDOWN));
        assert!(!is_keyboard_message(WM_NCLBUTTONDOWN));
        assert!(!is_keyboard_message(WM_NCHITTEST));
        assert!(!is_keyboard_message(WM_SYSCOMMAND));
        assert!(!is_keyboard_message(WM_LBUTTONDOWN));
    }

    #[test]
    fn test_window_maximize_restore_minimize_cycle() {
        use windows::Win32::System::Ole::{OleInitialize, OleUninitialize};

        unsafe {
            let _ = OleInitialize(None);
            let hinstance = GetModuleHandleW(None).unwrap();
            let class_name = w!("MalvisionTestWindowLifecycleClass");
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
                w!("TestMalvisionWindow"),
                WS_OVERLAPPEDWINDOW | WS_CLIPCHILDREN,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                1024,
                768,
                None,
                None,
                Some(hinstance.into()),
                None,
            )
            .expect("Création fenêtre principale de test");

            let _ = ShowWindow(hwnd, SW_SHOW);

            // Vérifier les styles initiaux (WS_OVERLAPPEDWINDOW complet)
            let style = GetWindowLongPtrW(hwnd, GWL_STYLE) as u32;
            assert_eq!(style & MAIN_WINDOW_BASE_STYLE.0, MAIN_WINDOW_BASE_STYLE.0);
            assert!(!IsZoomed(hwnd).as_bool());
            assert!(!IsIconic(hwnd).as_bool());

            // 1. Maximiser via WM_SYSCOMMAND (SC_MAXIMIZE)
            let _ = SendMessageW(
                hwnd,
                WM_SYSCOMMAND,
                Some(WPARAM(SC_MAXIMIZE as usize)),
                Some(LPARAM(0)),
            );
            assert!(
                IsZoomed(hwnd).as_bool(),
                "La fenêtre doit être maximisée après SC_MAXIMIZE"
            );

            // 2. Restaurer depuis l'état maximisé via WM_SYSCOMMAND (SC_RESTORE)
            let _ = SendMessageW(
                hwnd,
                WM_SYSCOMMAND,
                Some(WPARAM(SC_RESTORE as usize)),
                Some(LPARAM(0)),
            );
            assert!(
                !IsZoomed(hwnd).as_bool(),
                "La fenêtre doit être restaurée après SC_RESTORE"
            );
            assert!(!IsIconic(hwnd).as_bool());

            // 3. Maximiser à nouveau puis Minimiser directement depuis l'état maximisé (SC_MINIMIZE)
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
                Some(WPARAM(SC_MINIMIZE as usize)),
                Some(LPARAM(0)),
            );
            assert!(
                IsIconic(hwnd).as_bool(),
                "La fenêtre maximisée doit pouvoir être minimisée avec SC_MINIMIZE"
            );

            // 4. Restaurer depuis l'état minimisé
            let _ = SendMessageW(
                hwnd,
                WM_SYSCOMMAND,
                Some(WPARAM(SC_RESTORE as usize)),
                Some(LPARAM(0)),
            );
            assert!(!IsIconic(hwnd).as_bool());

            // 5. Simuler les événements RDP (Connecting -> Connected -> LoginComplete -> Disconnected)
            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut AppWindowState;
            assert!(!ptr.is_null());
            (*ptr).last_target = "192.168.1.100:3389".to_string();

            let _ = SendMessageW(
                hwnd,
                WM_RDP_EVENT,
                Some(WPARAM(RDP_EVT_CONNECTING)),
                Some(LPARAM(0)),
            );
            assert_eq!((*ptr).ui_state, UiRdpState::Connecting);
            assert!(get_window_text((*ptr).status_label).contains("Connexion RDP en cours"));

            // Sans connexion réelle (get_Connected() == 0), RDP_EVT_CONNECTED ne doit PAS afficher Connecté à tort
            let _ = SendMessageW(
                hwnd,
                WM_RDP_EVENT,
                Some(WPARAM(RDP_EVT_CONNECTED)),
                Some(LPARAM(0)),
            );
            assert_eq!(
                (*ptr).ui_state,
                UiRdpState::Connecting,
                "Ne jamais afficher un état connecté sans confirmation réelle de get_Connected() == 1"
            );

            // Fermeture propre
            let _ = SendMessageW(hwnd, WM_CLOSE, Some(WPARAM(0)), Some(LPARAM(0)));
            OleUninitialize();
        }
    }
}
