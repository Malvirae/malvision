//! Bindings COM et hébergement ActiveX pour `MsTscAx.dll` (Microsoft RDP Client Control).
//!
//! Utilise le conteneur ActiveX système `AtlAxWin` (`atl.dll` intégré à Windows) et interroge
//! directement les interfaces COM `IMsRdpClient9`, `IMsRdpClientAdvancedSettings8`,
//! `IMsRdpClientNonScriptable3`, `IMsRdpExtendedSettings` et `IMsTscAxEvents`.

use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use windows::Win32::Foundation::{
    E_NOINTERFACE, E_NOTIMPL, E_POINTER, HWND, LPARAM, S_OK, VARIANT_BOOL, VARIANT_TRUE, WPARAM,
};
use windows::Win32::System::Com::{
    DISPATCH_FLAGS, DISPPARAMS, EXCEPINFO, IConnectionPoint, IConnectionPointContainer, IDispatch,
    ITypeInfo,
};
use windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress, LoadLibraryW};
use windows::Win32::System::Ole::{IOleInPlaceActiveObject, IOleObject, OLEIVERB_UIACTIVATE};
use windows::Win32::System::Variant::{
    VARIANT, VARIANT_0, VARIANT_0_0, VARIANT_0_0_0, VT_BOOL, VT_BYREF, VT_I2, VT_I4, VT_UI4,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DestroyWindow, GetClientRect, MoveWindow, PostMessageW, WINDOW_EX_STYLE,
    WM_APP, WS_CHILD, WS_CLIPCHILDREN, WS_CLIPSIBLINGS, WS_VISIBLE,
};
use windows_core::{BSTR, GUID, HRESULT, IUnknown, Interface, PCWSTR, s, w};

/// Message Win32 personnalisé envoyé à la fenêtre principale lors d'un événement COM `IMsTscAxEvents`.
pub const WM_RDP_EVENT: u32 = WM_APP + 100;

/// Codes d'événements transmis dans les 16 bits de poids faible de `WPARAM` avec `WM_RDP_EVENT`
/// (les bits `WPARAM >> 16` contiennent le `session_id` interne).
pub const RDP_EVT_CONNECTING: usize = 1;
pub const RDP_EVT_CONNECTED: usize = 2;
pub const RDP_EVT_LOGIN_COMPLETE: usize = 3;
pub const RDP_EVT_DISCONNECTED: usize = 4;
pub const RDP_EVT_FATAL_ERROR: usize = 5;
pub const RDP_EVT_WARNING: usize = 6;
pub const RDP_EVT_LOGON_ERROR: usize = 7;
pub const RDP_EVT_AUTH_WARNING_DISPLAYED: usize = 8;
pub const RDP_EVT_AUTH_WARNING_DISMISSED: usize = 9;
pub const RDP_EVT_AUTO_RECONNECTING: usize = 10;

/// Encode `(session_id, event_code)` dans un `WPARAM`.
#[inline]
pub fn pack_rdp_event_wparam(session_id: u32, event_code: usize) -> WPARAM {
    WPARAM(((session_id as usize) << 16) | (event_code & 0xFFFF))
}

/// Décode `(session_id, event_code)` depuis le `WPARAM` de `WM_RDP_EVENT`.
#[inline]
pub fn unpack_rdp_event_wparam(wparam: WPARAM) -> (u32, usize) {
    let session_id = (wparam.0 >> 16) as u32;
    let event_code = wparam.0 & 0xFFFF;
    (session_id, event_code)
}

/// DIID de l'interface source d'événements `IMsTscAxEvents` (`{336d5562-efa8-482e-8cb3-c5c0fc7a7db6}`).
pub const DIID_IMS_TSC_AX_EVENTS: GUID = GUID::from_u128(0x336d5562_efa8_482e_8cb3_c5c0fc7a7db6);

/// IID de `IMsRdpClient9` (`{28904001-04b6-436c-a55b-0af1a0883dc9}`).
pub const IID_IMS_RDP_CLIENT_9: GUID = GUID::from_u128(0x28904001_04b6_436c_a55b_0af1a0883dc9);

/// IID de `IMsRdpClientAdvancedSettings8` (`{89acb528-2557-4d16-8625-226a30e97e9a}`).
pub const IID_IMS_RDP_CLIENT_ADVANCED_SETTINGS_8: GUID =
    GUID::from_u128(0x89acb528_2557_4d16_8625_226a30e97e9a);

/// IID de `IMsRdpClientNonScriptable3` (`{b3378d90-0728-45c7-8ed7-b6159fb92219}`).
pub const IID_IMS_RDP_CLIENT_NON_SCRIPTABLE_3: GUID =
    GUID::from_u128(0xb3378d90_0728_45c7_8ed7_b6159fb92219);

/// IID de `IMsRdpExtendedSettings` (`{302d8188-0052-4807-806a-362b628f9ac5}`).
pub const IID_IMS_RDP_EXTENDED_SETTINGS: GUID =
    GUID::from_u128(0x302d8188_0052_4807_806a_362b628f9ac5);

/// Liste ordonnée des identifiants ActiveX `MsTscAx` (NotSafeForScripting en priorité pour permettre
/// `IMsRdpClientNonScriptable3` et `ClearTextPassword`, du plus récent au plus ancien).
const RDP_CONTROL_IDENTIFIERS: &[PCWSTR] = &[
    w!("{3F859AA3-C2D4-4FAA-B0E4-FD0C9C4E5E3A}"), // MsRdpClient12NotSafeForScripting (v13)
    w!("{1DF7C823-B2D4-4B54-975A-F2AC5D7CF8B8}"), // MsRdpClient11NotSafeForScripting (v12)
    w!("{A0C63C30-F08D-4AB4-907C-34905D770C7D}"), // MsRdpClient10NotSafeForScripting (v11)
    w!("{8B918B82-7985-4C24-89DF-C33AD2BBFBCD}"), // MsRdpClient9NotSafeForScripting (v10)
    w!("{A3BC03A0-041D-42E3-AD22-882B7865C9C5}"), // MsRdpClient8NotSafeForScripting (v9)
    w!("MsTscAx.MsTscAx.13"),
    w!("MsTscAx.MsTscAx.12"),
    w!("MsTscAx.MsTscAx.11"),
    w!("MsTscAx.MsTscAx.10"),
    w!("MsTscAx.MsTscAx.9"),
];

type AtlAxWinInitFn = unsafe extern "system" fn() -> i32;
type AtlAxGetControlFn = unsafe extern "system" fn(HWND, *mut Option<IUnknown>) -> HRESULT;

use crate::settings::RdpKeyboardHookMode;

/// Paramètres de connexion RDP fournis par l'utilisateur ou le gestionnaire de connexions.
#[derive(Debug, Clone)]
pub struct RdpConnectionConfig {
    pub host: String,
    pub port: u16,
    pub domain: String,
    pub username: String,
    pub password: String,
    pub width: u32,
    pub height: u32,
    pub dynamic_resolution: bool,
    pub redirect_clipboard: bool,
    pub redirect_drives: bool,
    pub keyboard_hook_mode: RdpKeyboardHookMode,
}

/// Analyse une chaîne d'hôte (`ip`, `dns`, `ip:port`, `[ipv6]:port`) et retourne `(host, port)`.
pub fn parse_host_and_port(input: &str) -> Result<(String, u16), String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("Veuillez saisir une adresse IP ou un nom DNS.".to_string());
    }

    if let Some(rest) = trimmed.strip_prefix('[') {
        if let Some((ipv6, after)) = rest.split_once(']') {
            if ipv6.is_empty() {
                return Err("Adresse IPv6 invalide.".to_string());
            }
            if after.is_empty() {
                return Ok((ipv6.to_string(), 3389));
            }
            if let Some(port_str) = after.strip_prefix(':') {
                let port = port_str
                    .parse::<u16>()
                    .map_err(|_| format!("Port RDP invalide : '{port_str}'."))?;
                if port == 0 {
                    return Err("Le port RDP doit être compris entre 1 et 65535.".to_string());
                }
                return Ok((ipv6.to_string(), port));
            }
            return Err("Format d'adresse IPv6 invalide.".to_string());
        }
        return Err("Crochet fermant ']' manquant dans l'adresse IPv6.".to_string());
    }

    // Si un seul ':' est présent, c'est un couple host:port
    if trimmed.matches(':').count() == 1
        && let Some((host_part, port_part)) = trimmed.rsplit_once(':')
    {
        let host_clean = host_part.trim();
        if host_clean.is_empty() {
            return Err("Nom d'hôte vide avant le numéro de port.".to_string());
        }
        let port = port_part
            .trim()
            .parse::<u16>()
            .map_err(|_| format!("Port RDP invalide : '{port_part}'."))?;
        if port == 0 {
            return Err("Le port RDP doit être compris entre 1 et 65535.".to_string());
        }
        return Ok((host_clean.to_string(), port));
    }

    Ok((trimmed.to_string(), 3389))
}

/// Sépare un identifiant `DOMAINE\utilisateur` ou `utilisateur@domaine` en `(domaine, utilisateur)`.
pub fn split_domain_username(input: &str) -> (String, String) {
    let trimmed = input.trim();
    if let Some((domain, user)) = trimmed.split_once('\\') {
        (domain.trim().to_string(), user.trim().to_string())
    } else {
        (String::new(), trimmed.to_string())
    }
}

/// Appelle une méthode dans la vtable COM à l'offset `byte_offset`.
///
/// # Safety
/// L'appelant doit vérifier que `unk` implémente l'interface correspondant à l'offset de vtable.
unsafe fn vtable_entry(unk: &IUnknown, byte_offset: usize) -> *const c_void {
    unsafe {
        let obj_ptr = unk.as_raw() as *const *const *const c_void;
        let vtbl = *obj_ptr;
        let index = byte_offset / std::mem::size_of::<usize>();
        *vtbl.add(index)
    }
}

/// Vtable COM brute pour `IDispatch` / `IMsTscAxEvents`.
///
/// `IConnectionPoint::Advise` de `MsTscAx.dll` appelle `QueryInterface` avec `DIID_IMsTscAxEvents`
/// (`{336d5562-efa8-482e-8cb3-c5c0fc7a7db6}`). La macro standard `#[implement(IDispatch)]` de
/// `windows-rs` rejette ce `DIID` avec `E_NOINTERFACE` car elle n'accepte que `IID_IDispatch` et
/// `IID_IUnknown`. Notre implémentation COM dédiée répond positivement aux trois GUIDs :
/// `IUnknown::IID`, `IDispatch::IID` et `DIID_IMS_TSC_AX_EVENTS`.
#[repr(C)]
struct RawDispatchVtbl {
    query_interface:
        unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> HRESULT,
    add_ref: unsafe extern "system" fn(*mut c_void) -> u32,
    release: unsafe extern "system" fn(*mut c_void) -> u32,
    get_type_info_count: unsafe extern "system" fn(*mut c_void, *mut u32) -> HRESULT,
    get_type_info: unsafe extern "system" fn(*mut c_void, u32, u32, *mut *mut ITypeInfo) -> HRESULT,
    get_ids_of_names: unsafe extern "system" fn(
        *mut c_void,
        *const GUID,
        *const PCWSTR,
        u32,
        u32,
        *mut i32,
    ) -> HRESULT,
    invoke: unsafe extern "system" fn(
        *mut c_void,
        i32,
        *const GUID,
        u32,
        DISPATCH_FLAGS,
        *const DISPPARAMS,
        *mut VARIANT,
        *mut EXCEPINFO,
        *mut u32,
    ) -> HRESULT,
}

#[repr(C)]
struct RdpEventSinkComObject {
    vtbl: *const RawDispatchVtbl,
    ref_count: AtomicU32,
    active: AtomicBool,
    notify_hwnd: HWND,
    session_id: u32,
}

static RDP_EVENT_SINK_VTBL: RawDispatchVtbl = RawDispatchVtbl {
    query_interface: sink_query_interface,
    add_ref: sink_add_ref,
    release: sink_release,
    get_type_info_count: sink_get_type_info_count,
    get_type_info: sink_get_type_info,
    get_ids_of_names: sink_get_ids_of_names,
    invoke: sink_invoke,
};

impl RdpEventSinkComObject {
    fn new_com_unknown(notify_hwnd: HWND, session_id: u32) -> IUnknown {
        let boxed = Box::new(Self {
            vtbl: &RDP_EVENT_SINK_VTBL,
            ref_count: AtomicU32::new(1),
            active: AtomicBool::new(true),
            notify_hwnd,
            session_id,
        });
        unsafe { IUnknown::from_raw(Box::into_raw(boxed).cast()) }
    }
}

unsafe extern "system" fn sink_query_interface(
    this: *mut c_void,
    riid: *const GUID,
    ppv: *mut *mut c_void,
) -> HRESULT {
    unsafe {
        if ppv.is_null() {
            return E_POINTER;
        }
        *ppv = std::ptr::null_mut();
        if riid.is_null() {
            return E_NOINTERFACE;
        }
        let iid = *riid;
        if iid == IUnknown::IID || iid == IDispatch::IID || iid == DIID_IMS_TSC_AX_EVENTS {
            *ppv = this;
            sink_add_ref(this);
            S_OK
        } else {
            E_NOINTERFACE
        }
    }
}

unsafe extern "system" fn sink_add_ref(this: *mut c_void) -> u32 {
    unsafe {
        let obj = &*(this as *const RdpEventSinkComObject);
        obj.ref_count.fetch_add(1, Ordering::Relaxed) + 1
    }
}

unsafe extern "system" fn sink_release(this: *mut c_void) -> u32 {
    unsafe {
        let obj = &*(this as *const RdpEventSinkComObject);
        let prev = obj.ref_count.fetch_sub(1, Ordering::Release);
        if prev == 1 {
            std::sync::atomic::fence(Ordering::Acquire);
            drop(Box::from_raw(this as *mut RdpEventSinkComObject));
            0
        } else {
            prev - 1
        }
    }
}

unsafe extern "system" fn sink_get_type_info_count(
    _this: *mut c_void,
    pctinfo: *mut u32,
) -> HRESULT {
    unsafe {
        if !pctinfo.is_null() {
            *pctinfo = 0;
        }
        S_OK
    }
}

unsafe extern "system" fn sink_get_type_info(
    _this: *mut c_void,
    _itinfo: u32,
    _lcid: u32,
    pptinfo: *mut *mut ITypeInfo,
) -> HRESULT {
    unsafe {
        if !pptinfo.is_null() {
            *pptinfo = std::ptr::null_mut();
        }
        E_NOTIMPL
    }
}

unsafe extern "system" fn sink_get_ids_of_names(
    _this: *mut c_void,
    _riid: *const GUID,
    _rgsznames: *const PCWSTR,
    _cnames: u32,
    _lcid: u32,
    _rgdispid: *mut i32,
) -> HRESULT {
    E_NOTIMPL
}

unsafe extern "system" fn sink_invoke(
    this: *mut c_void,
    dispidmember: i32,
    _riid: *const GUID,
    _lcid: u32,
    _wflags: DISPATCH_FLAGS,
    pdispparams: *const DISPPARAMS,
    _pvarresult: *mut VARIANT,
    _pexcepinfo: *mut EXCEPINFO,
    _puargerr: *mut u32,
) -> HRESULT {
    unsafe {
        let obj = &*(this as *const RdpEventSinkComObject);
        if !obj.active.load(Ordering::Acquire) {
            return S_OK;
        }
        let notify_hwnd = obj.notify_hwnd;
        let session_id = obj.session_id;

        let first_int_arg = || -> isize {
            if pdispparams.is_null() {
                return 0;
            }
            let dp = &*pdispparams;
            if dp.cArgs == 0 || dp.rgvarg.is_null() {
                return 0;
            }
            // Dans IDispatch::Invoke, les arguments sont passés en ordre inverse;
            // le 1er paramètre de la méthode est à l'indice `cArgs - 1`.
            let arg = &*dp.rgvarg.add((dp.cArgs - 1) as usize);
            let vt = arg.Anonymous.Anonymous.vt;
            if vt == VT_I4 || vt == VT_UI4 {
                arg.Anonymous.Anonymous.Anonymous.lVal as isize
            } else if vt == VT_I2 {
                arg.Anonymous.Anonymous.Anonymous.iVal as isize
            } else if (vt.0 & VT_BYREF.0) != 0 && (vt.0 & !VT_BYREF.0) == VT_I4.0 {
                let pl = arg.Anonymous.Anonymous.Anonymous.plVal;
                if !pl.is_null() { *pl as isize } else { 0 }
            } else {
                0
            }
        };

        match dispidmember {
            1 => {
                // OnConnecting (DISPID 1)
                let _ = PostMessageW(
                    Some(notify_hwnd),
                    WM_RDP_EVENT,
                    pack_rdp_event_wparam(session_id, RDP_EVT_CONNECTING),
                    LPARAM(0),
                );
            }
            2 => {
                // OnConnected (DISPID 2)
                let _ = PostMessageW(
                    Some(notify_hwnd),
                    WM_RDP_EVENT,
                    pack_rdp_event_wparam(session_id, RDP_EVT_CONNECTED),
                    LPARAM(0),
                );
            }
            3 => {
                // OnLoginComplete (DISPID 3)
                let _ = PostMessageW(
                    Some(notify_hwnd),
                    WM_RDP_EVENT,
                    pack_rdp_event_wparam(session_id, RDP_EVT_LOGIN_COMPLETE),
                    LPARAM(0),
                );
            }
            4 => {
                // OnDisconnected(discReason: LONG) (DISPID 4)
                let reason = first_int_arg();
                let _ = PostMessageW(
                    Some(notify_hwnd),
                    WM_RDP_EVENT,
                    pack_rdp_event_wparam(session_id, RDP_EVT_DISCONNECTED),
                    LPARAM(reason),
                );
            }
            10 => {
                // OnFatalError(errorCode: LONG) (DISPID 10)
                let code = first_int_arg();
                let _ = PostMessageW(
                    Some(notify_hwnd),
                    WM_RDP_EVENT,
                    pack_rdp_event_wparam(session_id, RDP_EVT_FATAL_ERROR),
                    LPARAM(code),
                );
            }
            11 => {
                // OnWarning(warningCode: LONG) (DISPID 11)
                let code = first_int_arg();
                let _ = PostMessageW(
                    Some(notify_hwnd),
                    WM_RDP_EVENT,
                    pack_rdp_event_wparam(session_id, RDP_EVT_WARNING),
                    LPARAM(code),
                );
            }
            15 => {
                // OnConfirmClose(pfAllowClose: *mut VARIANT_BOOL) (DISPID 15)
                if !pdispparams.is_null() {
                    let dp = &*pdispparams;
                    if dp.cArgs >= 1 && !dp.rgvarg.is_null() {
                        let arg = &*dp.rgvarg;
                        let pbool = arg.Anonymous.Anonymous.Anonymous.pboolVal;
                        if !pbool.is_null() {
                            *pbool = VARIANT_TRUE;
                        }
                    }
                }
            }
            18 => {
                // OnAuthenticationWarningDisplayed (DISPID 18)
                let _ = PostMessageW(
                    Some(notify_hwnd),
                    WM_RDP_EVENT,
                    pack_rdp_event_wparam(session_id, RDP_EVT_AUTH_WARNING_DISPLAYED),
                    LPARAM(0),
                );
            }
            19 => {
                // OnAuthenticationWarningDismissed (DISPID 19)
                let _ = PostMessageW(
                    Some(notify_hwnd),
                    WM_RDP_EVENT,
                    pack_rdp_event_wparam(session_id, RDP_EVT_AUTH_WARNING_DISMISSED),
                    LPARAM(0),
                );
            }
            22 => {
                // OnLogonError(lError: LONG) (DISPID 22)
                let code = first_int_arg();
                let _ = PostMessageW(
                    Some(notify_hwnd),
                    WM_RDP_EVENT,
                    pack_rdp_event_wparam(session_id, RDP_EVT_LOGON_ERROR),
                    LPARAM(code),
                );
            }
            17 | 34 => {
                // OnAutoReconnecting / OnAutoReconnecting2 (DISPID 17 / 34)
                let reason = first_int_arg();
                let _ = PostMessageW(
                    Some(notify_hwnd),
                    WM_RDP_EVENT,
                    pack_rdp_event_wparam(session_id, RDP_EVT_AUTO_RECONNECTING),
                    LPARAM(reason),
                );
            }
            _ => {}
        }

        S_OK
    }
}

/// Encapsule l'instance ActiveX `MsTscAx` hébergée dans la fenêtre parente.
pub struct RdpActiveXHost {
    session_id: u32,
    parent_hwnd: HWND,
    ax_hwnd: HWND,
    rdp_client: IUnknown,
    connection_point: Option<IConnectionPoint>,
    advise_cookie: u32,
    _event_sink: Option<IUnknown>,
    control_id: String,
    dynamic_resolution: bool,
}

impl RdpActiveXHost {
    /// Initialise `atl.dll` et crée la fenêtre conteneur `AtlAxWin` hébergeant le contrôle RDP Microsoft.
    pub fn create(
        session_id: u32,
        parent_hwnd: HWND,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
    ) -> Result<Self, String> {
        unsafe {
            let atl_mod = LoadLibraryW(w!("atl.dll"))
                .map_err(|e| format!("Impossible de charger atl.dll : {e}"))?;

            let init_proc = GetProcAddress(atl_mod, s!("AtlAxWinInit"))
                .ok_or_else(|| "Fonction AtlAxWinInit introuvable dans atl.dll.".to_string())?;
            let get_ctrl_proc = GetProcAddress(atl_mod, s!("AtlAxGetControl"))
                .ok_or_else(|| "Fonction AtlAxGetControl introuvable dans atl.dll.".to_string())?;

            let atl_ax_win_init: AtlAxWinInitFn = std::mem::transmute(init_proc);
            let atl_ax_get_control: AtlAxGetControlFn = std::mem::transmute(get_ctrl_proc);

            if atl_ax_win_init() == 0 {
                return Err("Échec de l'initialisation AtlAxWinInit().".to_string());
            }

            let hinstance =
                GetModuleHandleW(None).map_err(|e| format!("GetModuleHandleW a échoué : {e}"))?;

            let mut created_hwnd = HWND::default();
            let mut rdp_client_opt: Option<IUnknown> = None;
            let mut active_control_id = String::new();

            for &progid in RDP_CONTROL_IDENTIFIERS {
                let hwnd_res = CreateWindowExW(
                    WINDOW_EX_STYLE(0),
                    w!("AtlAxWin"),
                    progid,
                    WS_CHILD | WS_VISIBLE | WS_CLIPCHILDREN | WS_CLIPSIBLINGS,
                    x,
                    y,
                    width.max(100),
                    height.max(100),
                    Some(parent_hwnd),
                    None,
                    Some(hinstance.into()),
                    None,
                );

                if let Ok(hwnd) = hwnd_res
                    && !hwnd.is_invalid()
                {
                    let mut unk: Option<IUnknown> = None;
                    let hr = atl_ax_get_control(hwnd, &mut unk);
                    if hr.is_ok()
                        && let Some(control_unk) = unk
                    {
                        let mut client9_ptr: *mut c_void = std::ptr::null_mut();
                        let qi_hr = (Interface::vtable(&control_unk).QueryInterface)(
                            Interface::as_raw(&control_unk),
                            &IID_IMS_RDP_CLIENT_9,
                            &mut client9_ptr,
                        );
                        if qi_hr == S_OK && !client9_ptr.is_null() {
                            created_hwnd = hwnd;
                            rdp_client_opt = Some(IUnknown::from_raw(client9_ptr));
                            active_control_id = progid.to_string().unwrap_or_default();
                            break;
                        }
                    }
                    let _ = DestroyWindow(hwnd);
                }
            }

            let rdp_client = rdp_client_opt.ok_or_else(|| {
                "Impossible d'instancier le contrôle ActiveX RDP Microsoft (MsTscAx.dll / IMsRdpClient9).".to_string()
            })?;

            // Connecter le sink d'événements IMsTscAxEvents via IConnectionPointContainer
            let cpc = rdp_client
                .cast::<IConnectionPointContainer>()
                .map_err(|e| format!("IConnectionPointContainer indisponible sur MsTscAx : {e}"))?;
            let cp = cpc
                .FindConnectionPoint(&DIID_IMS_TSC_AX_EVENTS)
                .map_err(|e| format!("FindConnectionPoint(IMsTscAxEvents) a échoué : {e}"))?;

            let sink_unk = RdpEventSinkComObject::new_com_unknown(parent_hwnd, session_id);
            let advise_cookie = cp
                .Advise(&sink_unk)
                .map_err(|e| format!("IConnectionPoint::Advise(IMsTscAxEvents) a échoué : {e}"))?;

            let host = Self {
                session_id,
                parent_hwnd,
                ax_hwnd: created_hwnd,
                rdp_client,
                connection_point: Some(cp),
                advise_cookie,
                _event_sink: Some(sink_unk),
                control_id: active_control_id,
                dynamic_resolution: true,
            };

            host.configure_defaults()?;
            Ok(host)
        }
    }

    /// Retourne l'identifiant interne de la session RDP.
    pub fn session_id(&self) -> u32 {
        self.session_id
    }

    /// Affiche ou masque la fenêtre conteneur ActiveX (changement d'onglet sans détruire la session).
    pub fn set_visible(&self, visible: bool) {
        use windows::Win32::UI::WindowsAndMessaging::{SW_HIDE, SW_SHOW, ShowWindow};
        unsafe {
            let _ = ShowWindow(self.ax_hwnd, if visible { SW_SHOW } else { SW_HIDE });
        }
    }

    /// Retourne l'identifiant (CLSID ou ProgID) du contrôle ActiveX instancié.
    pub fn control_id(&self) -> &str {
        &self.control_id
    }

    /// Retourne le cookie d'abonnement `IConnectionPoint::Advise` (strictement non nul si abonné).
    pub fn advise_cookie(&self) -> u32 {
        self.advise_cookie
    }

    /// Retourne la chaîne de version du contrôle RDP (`IMsTscAx::get_Version`, oVft = 224).
    pub fn version_string(&self) -> String {
        unsafe {
            type GetVersionFn = unsafe extern "system" fn(*mut c_void, *mut BSTR) -> HRESULT;
            let func: GetVersionFn = std::mem::transmute(vtable_entry(&self.rdp_client, 224));
            let mut bstr = BSTR::new();
            if func(self.rdp_client.as_raw(), &mut bstr) == S_OK {
                bstr.to_string()
            } else {
                String::from("inconnue")
            }
        }
    }

    /// Retourne le `HWND` du conteneur ActiveX `AtlAxWin`.
    pub fn hwnd(&self) -> HWND {
        self.ax_hwnd
    }

    /// Active l'objet OLE pour que le contrôle RDP reçoive immédiatement les entrées clavier et souris.
    pub fn focus_control(&self) {
        unsafe {
            if let Ok(ole_obj) = self.rdp_client.cast::<IOleObject>() {
                let mut rect = Default::default();
                let _ = GetClientRect(self.ax_hwnd, &mut rect);
                let _ = ole_obj.DoVerb(
                    OLEIVERB_UIACTIVATE.0,
                    std::ptr::null_mut(),
                    None,
                    0,
                    self.ax_hwnd,
                    &rect,
                );
            }
        }
    }

    /// Traduit les accélérateurs clavier OLE du contrôle ActiveX RDP (Tab, flèches, raccourcis Windows).
    pub fn translate_accelerator(
        &self,
        msg: *const windows::Win32::UI::WindowsAndMessaging::MSG,
    ) -> bool {
        unsafe {
            if let Ok(active_obj) = self.rdp_client.cast::<IOleInPlaceActiveObject>() {
                active_obj.TranslateAccelerator(Some(msg)).is_ok()
            } else {
                false
            }
        }
    }

    /// Configure les options par défaut du contrôle RDP (NLA, SmartSizing, boîte d'authentification native).
    fn configure_defaults(&self) -> Result<(), String> {
        unsafe {
            // Paramétrer les options avancées (IMsRdpClientAdvancedSettings8)
            if let Some(adv) = self.advanced_settings8() {
                // EnableCredSspSupport (oVft = 1384) -> active NLA (Network Level Authentication)
                let _ = self.put_bool(&adv, 1384, true);
                // SmartSizing (oVft = 832) -> adapte l'affichage lors du redimensionnement
                let _ = self.put_bool(&adv, 832, true);
                // AuthenticationLevel (oVft = 1176) -> 2 = Avertir l'utilisateur si l'authentification serveur échoue (comportement standard Windows)
                let _ = self.put_u32(&adv, 1176, 2);
                // DisplayConnectionBar (oVft = 904) -> false (intégré dans la fenêtre Malvision)
                let _ = self.put_bool(&adv, 904, false);
                // ContainerHandledFullScreen (oVft = 136) -> 1 (géré par le conteneur)
                let _ = self.put_i32(&adv, 136, 1);
            }

            // Paramétrer IMsRdpClientNonScriptable3 (fenêtre parente pour les dialogues NLA/certificats)
            if let Some(non_scriptable) = self.non_scriptable3() {
                // put_UIParentWindowHandle (oVft = 120)
                type PutHwndFn = unsafe extern "system" fn(*mut c_void, *mut c_void) -> HRESULT;
                let func: PutHwndFn = std::mem::transmute(vtable_entry(&non_scriptable, 120));
                let _ = func(non_scriptable.as_raw(), self.parent_hwnd.0);

                // put_NegotiateSecurityLayer (oVft = 168) -> VARIANT_TRUE
                let _ = self.put_bool(&non_scriptable, 168, true);
                // put_EnableCredSspSupport (oVft = 184) -> VARIANT_TRUE
                let _ = self.put_bool(&non_scriptable, 184, true);
            }

            Ok(())
        }
    }

    /// Retourne l'état de connexion actuel (`0` = déconnecté, `1` = connecté, `2` = connexion en cours).
    pub fn connection_state(&self) -> i16 {
        unsafe {
            // get_Connected est à oVft = 136 sur IMsRdpClient9
            type GetConnectedFn = unsafe extern "system" fn(*mut c_void, *mut i16) -> HRESULT;
            let func: GetConnectedFn = std::mem::transmute(vtable_entry(&self.rdp_client, 136));
            let mut state: i16 = 0;
            if func(self.rdp_client.as_raw(), &mut state) == S_OK {
                state
            } else {
                0
            }
        }
    }

    /// Indique si une session est connectée (`1`) ou en cours de connexion (`2`).
    pub fn is_active(&self) -> bool {
        self.connection_state() != 0
    }

    /// Lance une connexion RDP vers l'hôte configuré.
    pub fn connect(&mut self, config: &RdpConnectionConfig) -> Result<(), String> {
        if self.is_active() {
            return Err("Une session RDP est déjà active ou en cours de connexion.".to_string());
        }

        self.dynamic_resolution = config.dynamic_resolution;

        unsafe {
            // 1. Server (oVft = 56)
            self.put_bstr(&self.rdp_client, 56, &config.host)
                .map_err(|hr| format!("Erreur lors de la configuration de l'hôte RDP ({hr:?})"))?;

            // 2. Domain (oVft = 72)
            self.put_bstr(&self.rdp_client, 72, &config.domain)
                .map_err(|hr| format!("Erreur lors de la configuration du domaine ({hr:?})"))?;

            // 3. UserName (oVft = 88)
            self.put_bstr(&self.rdp_client, 88, &config.username)
                .map_err(|hr| {
                    format!("Erreur lors de la configuration de l'utilisateur ({hr:?})")
                })?;

            // 4. DesktopWidth (oVft = 144) & DesktopHeight (oVft = 160) & ColorDepth (oVft = 296)
            let width = config.width.clamp(640, 4096) as i32;
            let height = config.height.clamp(480, 2160) as i32;
            let _ = self.put_i32(&self.rdp_client, 144, width);
            let _ = self.put_i32(&self.rdp_client, 160, height);
            let _ = self.put_i32(&self.rdp_client, 296, 32);

            // 5. Textes d'état intégrés au contrôle ActiveX
            // put_ConnectingText (oVft = 120), put_DisconnectedText (oVft = 104)
            let _ = self.put_bstr(
                &self.rdp_client,
                120,
                &format!(
                    "Connexion RDP en cours vers {}:{}...",
                    config.host, config.port
                ),
            );
            let _ = self.put_bstr(&self.rdp_client, 104, "Session RDP terminée.");

            // 6. Configuration avancée (Port RDP, NLA, SmartSizing, Presse-papiers, Mot de passe)
            if let Some(adv) = self.advanced_settings8() {
                // RDPPort (oVft = 280)
                let _ = self.put_i32(&adv, 280, i32::from(config.port));
                // SmartSizing (oVft = 832)
                let _ = self.put_bool(&adv, 832, config.dynamic_resolution);
                // RedirectClipboard (oVft = 1192)
                let _ = self.put_bool(&adv, 1192, config.redirect_clipboard);
                // RedirectDrives (oVft = 968) -> active le copier-coller de fichiers via le presse-papiers RDP
                let _ = self.put_bool(&adv, 968, config.redirect_drives);
                // EnableCredSspSupport (oVft = 1384)
                let _ = self.put_bool(&adv, 1384, true);
                // AuthenticationLevel (oVft = 1176) -> 2 (avertissement certificat standard Windows)
                let _ = self.put_u32(&adv, 1176, 2);

                if !config.password.is_empty() {
                    // ClearTextPassword sur IMsRdpClientAdvancedSettings (oVft = 896)
                    let _ = self.put_bstr(&adv, 896, &config.password);
                }
            }

            // 6b. Configuration de la capture des raccourcis clavier (KeyboardHookMode)
            if let Some(secured) = self.secured_settings2() {
                let mode_val: i32 = match config.keyboard_hook_mode {
                    RdpKeyboardHookMode::OnLocal => 0,
                    RdpKeyboardHookMode::OnRemote => 1,
                    RdpKeyboardHookMode::InFullScreenOnly => 2,
                };
                // put_KeyboardHookMode (oVft = 104)
                let _ = self.put_i32(&secured, 104, mode_val);
            }

            // 7. Configuration IMsRdpClientNonScriptable3 (mot de passe ou demande d'identifiants Windows)
            if let Some(non_scriptable) = self.non_scriptable3() {
                if !config.password.is_empty() {
                    // ClearTextPassword sur IMsTscNonScriptable (oVft = 24)
                    let _ = self.put_bstr(&non_scriptable, 24, &config.password);
                    // PromptForCredentials (oVft = 152) -> false (on fournit déjà le mot de passe, CredSSP demandera quand même si refusé)
                    let _ = self.put_bool(&non_scriptable, 152, false);
                } else {
                    // ResetPassword (oVft = 96)
                    type ResetPwdFn = unsafe extern "system" fn(*mut c_void) -> HRESULT;
                    let reset_fn: ResetPwdFn =
                        std::mem::transmute(vtable_entry(&non_scriptable, 96));
                    let _ = reset_fn(non_scriptable.as_raw());
                    // Si aucun mot de passe n'est saisi, laisser CredSSP / Windows afficher sa boîte d'identifiants native
                    let _ = self.put_bool(&non_scriptable, 152, true);
                }
            }

            // 8. Appel de Connect() (oVft = 264 sur IMsRdpClient9)
            type ConnectFn = unsafe extern "system" fn(*mut c_void) -> HRESULT;
            let connect_fn: ConnectFn = std::mem::transmute(vtable_entry(&self.rdp_client, 264));
            let hr = connect_fn(self.rdp_client.as_raw());
            if hr != S_OK {
                return Err(format!(
                    "L'appel IMsRdpClient::Connect() a échoué (HRESULT 0x{:08X}).",
                    hr.0 as u32
                ));
            }

            self.focus_control();
            Ok(())
        }
    }

    /// Déconnecte proprement la session RDP active.
    pub fn disconnect(&self) -> Result<(), String> {
        if !self.is_active() {
            return Ok(());
        }
        unsafe {
            // Disconnect() est à oVft = 272 sur IMsRdpClient9
            type DisconnectFn = unsafe extern "system" fn(*mut c_void) -> HRESULT;
            let disconnect_fn: DisconnectFn =
                std::mem::transmute(vtable_entry(&self.rdp_client, 272));
            let hr = disconnect_fn(self.rdp_client.as_raw());
            if hr != S_OK {
                return Err(format!(
                    "IMsRdpClient::Disconnect() a retourné HRESULT 0x{:08X}.",
                    hr.0 as u32
                ));
            }
            Ok(())
        }
    }

    /// Redimensionne le conteneur ActiveX RDP et met à jour dynamiquement la résolution de session si connectée.
    pub fn resize(&self, x: i32, y: i32, width: i32, height: i32) {
        let w = width.max(100);
        let h = height.max(100);
        unsafe {
            let _ = MoveWindow(self.ax_hwnd, x, y, w, h, true);

            let state = self.connection_state();
            if state == 1 && self.dynamic_resolution {
                // Connecté : tenter UpdateSessionDisplaySettings (oVft = 552 sur IMsRdpClient9)
                // pour adapter dynamiquement le bureau distant (avec SmartSizing en secours automatique)
                type UpdateDisplayFn = unsafe extern "system" fn(
                    *mut c_void,
                    u32,
                    u32,
                    u32,
                    u32,
                    u32,
                    u32,
                    u32,
                ) -> HRESULT;
                let update_fn: UpdateDisplayFn =
                    std::mem::transmute(vtable_entry(&self.rdp_client, 552));
                let uw = (w as u32).clamp(640, 4096);
                let uh = (h as u32).clamp(480, 2160);
                let _ = update_fn(self.rdp_client.as_raw(), uw, uh, uw, uh, 0, 100, 100);
            } else if state == 0 {
                // Déconnecté : mettre à jour DesktopWidth / DesktopHeight pour la prochaine connexion
                let _ = self.put_i32(&self.rdp_client, 144, w.clamp(640, 4096));
                let _ = self.put_i32(&self.rdp_client, 160, h.clamp(480, 2160));
            }
        }
    }

    /// Récupère le code `ExtendedDisconnectReason` et le message détaillé via `IMsRdpClient9::GetErrorDescription`.
    pub fn describe_disconnect(&self, disconnect_reason: i32) -> (u32, String) {
        unsafe {
            // get_ExtendedDisconnectReason (oVft = 328)
            type GetExtReasonFn = unsafe extern "system" fn(*mut c_void, *mut u32) -> HRESULT;
            let get_ext: GetExtReasonFn = std::mem::transmute(vtable_entry(&self.rdp_client, 328));
            let mut ext_reason: u32 = 0;
            let _ = get_ext(self.rdp_client.as_raw(), &mut ext_reason);

            // GetErrorDescription(disconnectReason: u32, extendedDisconnectReason: u32, *mut BSTR) (oVft = 432)
            type GetErrorDescFn =
                unsafe extern "system" fn(*mut c_void, u32, u32, *mut BSTR) -> HRESULT;
            let get_desc: GetErrorDescFn = std::mem::transmute(vtable_entry(&self.rdp_client, 432));
            let mut bstr_desc = BSTR::new();
            let hr = get_desc(
                self.rdp_client.as_raw(),
                disconnect_reason as u32,
                ext_reason,
                &mut bstr_desc,
            );

            let text = if hr == S_OK && !bstr_desc.is_empty() {
                bstr_desc.to_string().trim().replace(['\r', '\n'], " ")
            } else {
                fallback_disconnect_reason(disconnect_reason, ext_reason)
            };

            (ext_reason, text)
        }
    }

    /// Active ou désactive le mode Zoom / SmartSizing via `IMsRdpExtendedSettings` et `IMsRdpClientAdvancedSettings8`.
    #[allow(dead_code)]
    pub fn set_smart_sizing(&self, enabled: bool) {
        unsafe {
            if let Some(adv) = self.advanced_settings8() {
                let _ = self.put_bool(&adv, 832, enabled);
            }
            let mut ext_ptr: *mut c_void = std::ptr::null_mut();
            if (Interface::vtable(&self.rdp_client).QueryInterface)(
                Interface::as_raw(&self.rdp_client),
                &IID_IMS_RDP_EXTENDED_SETTINGS,
                &mut ext_ptr,
            ) == S_OK
                && !ext_ptr.is_null()
            {
                let ext = IUnknown::from_raw(ext_ptr);
                type PutPropFn =
                    unsafe extern "system" fn(*mut c_void, *const c_void, *mut VARIANT) -> HRESULT;
                let func: PutPropFn = std::mem::transmute(vtable_entry(&ext, 24));
                let prop_name = BSTR::from("DesktopScaleFactor");
                let mut var = VARIANT {
                    Anonymous: VARIANT_0 {
                        Anonymous: std::mem::ManuallyDrop::new(VARIANT_0_0 {
                            vt: VT_BOOL,
                            wReserved1: 0,
                            wReserved2: 0,
                            wReserved3: 0,
                            Anonymous: VARIANT_0_0_0 {
                                boolVal: if enabled {
                                    VARIANT_TRUE
                                } else {
                                    VARIANT_BOOL(0)
                                },
                            },
                        }),
                    },
                };
                let _ = func(ext.as_raw(), prop_name.as_ptr().cast(), &mut var);
            }
        }
    }

    unsafe fn advanced_settings8(&self) -> Option<IUnknown> {
        unsafe {
            // get_AdvancedSettings8 est à oVft = 472 sur IMsRdpClient9
            type GetAdv8Fn = unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HRESULT;
            let func: GetAdv8Fn = std::mem::transmute(vtable_entry(&self.rdp_client, 472));
            let mut ptr: *mut c_void = std::ptr::null_mut();
            if func(self.rdp_client.as_raw(), &mut ptr) == S_OK && !ptr.is_null() {
                return Some(IUnknown::from_raw(ptr));
            }

            // Repli par QueryInterface direct
            let mut qi_ptr: *mut c_void = std::ptr::null_mut();
            if (Interface::vtable(&self.rdp_client).QueryInterface)(
                Interface::as_raw(&self.rdp_client),
                &IID_IMS_RDP_CLIENT_ADVANCED_SETTINGS_8,
                &mut qi_ptr,
            ) == S_OK
                && !qi_ptr.is_null()
            {
                return Some(IUnknown::from_raw(qi_ptr));
            }
            None
        }
    }

    unsafe fn secured_settings2(&self) -> Option<IUnknown> {
        unsafe {
            // get_SecuredSettings2 est à oVft = 320 sur IMsRdpClient9
            type GetSecuredSettings2Fn =
                unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HRESULT;
            let func: GetSecuredSettings2Fn =
                std::mem::transmute(vtable_entry(&self.rdp_client, 320));
            let mut ptr: *mut c_void = std::ptr::null_mut();
            if func(self.rdp_client.as_raw(), &mut ptr) == S_OK && !ptr.is_null() {
                return Some(IUnknown::from_raw(ptr));
            }

            // Repli sur get_SecuredSettings à oVft = 240
            type GetSecuredSettingsFn =
                unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HRESULT;
            let func2: GetSecuredSettingsFn =
                std::mem::transmute(vtable_entry(&self.rdp_client, 240));
            let mut ptr2: *mut c_void = std::ptr::null_mut();
            if func2(self.rdp_client.as_raw(), &mut ptr2) == S_OK && !ptr2.is_null() {
                return Some(IUnknown::from_raw(ptr2));
            }
            None
        }
    }

    /// Envoie une séquence de touches au contrôle RDP via l'API officielle `IMsRdpClientNonScriptable::SendKeys`.
    pub fn send_keys(
        &self,
        keys: &[(
            windows::Win32::UI::Input::KeyboardAndMouse::VIRTUAL_KEY,
            bool,
        )],
    ) -> Result<(), String> {
        if !self.is_active() {
            return Err("Aucune session RDP active pour envoyer les touches.".to_string());
        }
        unsafe {
            if let Some(non_scriptable) = self.non_scriptable3() {
                // SendKeys est à oVft = 112 sur IMsRdpClientNonScriptable
                type SendKeysFn =
                    unsafe extern "system" fn(*mut c_void, i32, *const i16, *const i32) -> HRESULT;
                let func: SendKeysFn = std::mem::transmute(vtable_entry(&non_scriptable, 112));
                let num_keys = keys.len() as i32;
                let mut up_array: Vec<i16> = Vec::with_capacity(keys.len());
                let mut key_data: Vec<i32> = Vec::with_capacity(keys.len());
                for (vk, is_up) in keys {
                    up_array.push(if *is_up { -1 } else { 0 }); // VARIANT_TRUE / VARIANT_FALSE
                    key_data.push(vk.0 as i32);
                }
                let hr = func(
                    non_scriptable.as_raw(),
                    num_keys,
                    up_array.as_ptr(),
                    key_data.as_ptr(),
                );
                if hr == S_OK {
                    Ok(())
                } else {
                    Err(format!(
                        "IMsRdpClientNonScriptable::SendKeys a échoué (HRESULT 0x{:08X}).",
                        hr.0 as u32
                    ))
                }
            } else {
                Err("Interface IMsRdpClientNonScriptable3 indisponible.".to_string())
            }
        }
    }

    /// Envoie la combinaison Ctrl + Alt + Suppr à la session distante via `SendKeys`.
    pub fn send_cad(&self) -> Result<(), String> {
        use windows::Win32::UI::Input::KeyboardAndMouse::{VK_CONTROL, VK_DELETE, VK_MENU};
        let sequence = [
            (VK_CONTROL, false),
            (VK_MENU, false),
            (VK_DELETE, false),
            (VK_DELETE, true),
            (VK_MENU, true),
            (VK_CONTROL, true),
        ];
        self.send_keys(&sequence)
    }

    /// Envoie la combinaison Ctrl + Alt + Fin (équivalent standard RDP de SAS pour invoquer l'écran de sécurité distant).
    pub fn send_ctrl_alt_end(&self) -> Result<(), String> {
        use windows::Win32::UI::Input::KeyboardAndMouse::{VK_CONTROL, VK_END, VK_MENU};
        let sequence = [
            (VK_CONTROL, false),
            (VK_MENU, false),
            (VK_END, false),
            (VK_END, true),
            (VK_MENU, true),
            (VK_CONTROL, true),
        ];
        self.send_keys(&sequence)
    }

    /// Bascule la session RDP en mode plein écran ou fenêtré (`put_FullScreen` à oVft = 336 sur `IMsRdpClient9`).
    pub fn set_full_screen(&self, full_screen: bool) -> Result<(), String> {
        unsafe {
            type PutFullScreenFn = unsafe extern "system" fn(*mut c_void, i16) -> HRESULT;
            let func: PutFullScreenFn = std::mem::transmute(vtable_entry(&self.rdp_client, 336));
            let val: i16 = if full_screen { -1 } else { 0 };
            let hr = func(self.rdp_client.as_raw(), val);
            if hr == S_OK {
                Ok(())
            } else {
                Err(format!(
                    "put_FullScreen({full_screen}) a retourné HRESULT 0x{:08X}.",
                    hr.0 as u32
                ))
            }
        }
    }

    /// Indique si la session est actuellement en mode plein écran (`get_FullScreen` à oVft = 344 sur `IMsRdpClient9`).
    pub fn is_full_screen(&self) -> bool {
        unsafe {
            type GetFullScreenFn = unsafe extern "system" fn(*mut c_void, *mut i16) -> HRESULT;
            let func: GetFullScreenFn = std::mem::transmute(vtable_entry(&self.rdp_client, 344));
            let mut val: i16 = 0;
            if func(self.rdp_client.as_raw(), &mut val) == S_OK {
                val != 0
            } else {
                false
            }
        }
    }

    unsafe fn non_scriptable3(&self) -> Option<IUnknown> {
        unsafe {
            let mut ptr: *mut c_void = std::ptr::null_mut();
            if (Interface::vtable(&self.rdp_client).QueryInterface)(
                Interface::as_raw(&self.rdp_client),
                &IID_IMS_RDP_CLIENT_NON_SCRIPTABLE_3,
                &mut ptr,
            ) == S_OK
                && !ptr.is_null()
            {
                Some(IUnknown::from_raw(ptr))
            } else {
                None
            }
        }
    }

    unsafe fn put_bstr(
        &self,
        target: &IUnknown,
        offset: usize,
        value: &str,
    ) -> Result<(), HRESULT> {
        unsafe {
            type PutBstrFn = unsafe extern "system" fn(*mut c_void, *const c_void) -> HRESULT;
            let func: PutBstrFn = std::mem::transmute(vtable_entry(target, offset));
            let bstr = BSTR::from(value);
            let hr = func(target.as_raw(), bstr.as_ptr().cast());
            if hr == S_OK { Ok(()) } else { Err(hr) }
        }
    }

    unsafe fn put_i32(&self, target: &IUnknown, offset: usize, value: i32) -> Result<(), HRESULT> {
        unsafe {
            type PutI32Fn = unsafe extern "system" fn(*mut c_void, i32) -> HRESULT;
            let func: PutI32Fn = std::mem::transmute(vtable_entry(target, offset));
            let hr = func(target.as_raw(), value);
            if hr == S_OK { Ok(()) } else { Err(hr) }
        }
    }

    unsafe fn put_u32(&self, target: &IUnknown, offset: usize, value: u32) -> Result<(), HRESULT> {
        unsafe {
            type PutU32Fn = unsafe extern "system" fn(*mut c_void, u32) -> HRESULT;
            let func: PutU32Fn = std::mem::transmute(vtable_entry(target, offset));
            let hr = func(target.as_raw(), value);
            if hr == S_OK { Ok(()) } else { Err(hr) }
        }
    }

    unsafe fn put_bool(
        &self,
        target: &IUnknown,
        offset: usize,
        value: bool,
    ) -> Result<(), HRESULT> {
        unsafe {
            type PutBoolFn = unsafe extern "system" fn(*mut c_void, VARIANT_BOOL) -> HRESULT;
            let func: PutBoolFn = std::mem::transmute(vtable_entry(target, offset));
            let vbool = if value { VARIANT_TRUE } else { VARIANT_BOOL(0) };
            let hr = func(target.as_raw(), vbool);
            if hr == S_OK { Ok(()) } else { Err(hr) }
        }
    }

    /// Désactive immédiatement le sink d'événements COM (`IMsTscAxEvents`) et se désabonne
    /// (`IConnectionPoint::Unadvise`) avant toute opération de déconnexion ou de destruction,
    /// évitant ainsi toute réentrance ou double destruction depuis un callback COM tardif.
    pub fn deactivate_events(&mut self) {
        unsafe {
            if let Some(sink_unk) = &self._event_sink {
                let raw = sink_unk.as_raw() as *const RdpEventSinkComObject;
                if !raw.is_null() {
                    (*raw).active.store(false, Ordering::Release);
                }
            }
            if let Some(cp) = self.connection_point.take()
                && self.advise_cookie != 0
            {
                let _ = cp.Unadvise(self.advise_cookie);
                self.advise_cookie = 0;
            }
            self._event_sink = None;
        }
    }
}

impl Drop for RdpActiveXHost {
    fn drop(&mut self) {
        self.deactivate_events();
        if self.is_active() {
            let _ = self.disconnect();
        }
        unsafe {
            if !self.ax_hwnd.is_invalid() {
                let hwnd = self.ax_hwnd;
                self.ax_hwnd = HWND::default();
                let _ = DestroyWindow(hwnd);
            }
        }
    }
}

/// Traduit les codes de déconnexion RDP courants lorsque `GetErrorDescription` est vide.
pub fn fallback_disconnect_reason(reason: i32, ext_reason: u32) -> String {
    match reason {
        0 => "Aucune information supplémentaire.".to_string(),
        1 => "Déconnexion initiée localement par l'application.".to_string(),
        2 => "Déconnexion normale par l'utilisateur ou le serveur distant.".to_string(),
        3 => "Déconnexion initiée par le serveur distant.".to_string(),
        260 => "Échec de la résolution DNS ou nom d'hôte introuvable.".to_string(),
        516 => "Impossible d'établir la connexion TCP vers le serveur RDP (hôte injoignable ou port fermé).".to_string(),
        520 => "Hôte introuvable ou inaccessible sur le réseau.".to_string(),
        1288 => "La résolution du nom d'hôte a échoué.".to_string(),
        2052 => "Adresse IP ou nom d'hôte incorrect.".to_string(),
        2055 => "Échec de l'authentification NLA / CredSSP (identifiants refusés ou politique de sécurité).".to_string(),
        2308 => "Connexion interrompue (socket fermé par l'hôte distant).".to_string(),
        2825 => "Le serveur distant exige Network Level Authentication (NLA) ou les identifiants sont invalides.".to_string(),
        _ => format!("Fin de session RDP (code={reason}, code étendu={ext_reason})."),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_host_and_port_defaults() {
        assert_eq!(
            parse_host_and_port("192.168.1.50").unwrap(),
            ("192.168.1.50".to_string(), 3389)
        );
        assert_eq!(
            parse_host_and_port("  srv-prod.local  ").unwrap(),
            ("srv-prod.local".to_string(), 3389)
        );
    }

    #[test]
    fn test_parse_host_and_port_custom_port() {
        assert_eq!(
            parse_host_and_port("10.0.0.12:3390").unwrap(),
            ("10.0.0.12".to_string(), 3390)
        );
        assert_eq!(
            parse_host_and_port("[fe80::1]:13389").unwrap(),
            ("fe80::1".to_string(), 13389)
        );
    }

    #[test]
    fn test_parse_host_and_port_invalid() {
        assert!(parse_host_and_port("   ").is_err());
        assert!(parse_host_and_port("192.168.1.1:0").is_err());
        assert!(parse_host_and_port("192.168.1.1:99999").is_err());
        assert!(parse_host_and_port(":3389").is_err());
    }

    #[test]
    fn test_split_domain_username() {
        assert_eq!(
            split_domain_username("CORP\\Administrateur"),
            ("CORP".to_string(), "Administrateur".to_string())
        );
        assert_eq!(
            split_domain_username("admin"),
            (String::new(), "admin".to_string())
        );
        assert_eq!(
            split_domain_username("admin@corp.local"),
            (String::new(), "admin@corp.local".to_string())
        );
    }

    #[test]
    fn test_fallback_disconnect_reason() {
        let msg = fallback_disconnect_reason(516, 0);
        assert!(msg.contains("TCP"));
        let local = fallback_disconnect_reason(1, 0);
        assert!(local.contains("localement"));
    }

    #[test]
    fn test_sink_responds_to_diid_ims_tsc_ax_events() {
        unsafe {
            let sink = RdpEventSinkComObject::new_com_unknown(HWND::default(), 7);
            let mut out_ptr: *mut c_void = std::ptr::null_mut();

            // Vérifier que QueryInterface accepte DIID_IMS_TSC_AX_EVENTS
            let hr = (Interface::vtable(&sink).QueryInterface)(
                Interface::as_raw(&sink),
                &DIID_IMS_TSC_AX_EVENTS,
                &mut out_ptr,
            );
            assert_eq!(hr, S_OK);
            assert!(!out_ptr.is_null());
            let _ = IUnknown::from_raw(out_ptr);

            // Vérifier que QueryInterface accepte IDispatch::IID
            let mut disp_ptr: *mut c_void = std::ptr::null_mut();
            let hr_disp = (Interface::vtable(&sink).QueryInterface)(
                Interface::as_raw(&sink),
                &IDispatch::IID,
                &mut disp_ptr,
            );
            assert_eq!(hr_disp, S_OK);
            assert!(!disp_ptr.is_null());
            let _ = IUnknown::from_raw(disp_ptr);
        }
    }

    #[test]
    fn test_activex_rdp_host_instantiation_and_advise() {
        use windows::Win32::System::Ole::{OleInitialize, OleUninitialize};
        use windows::Win32::UI::WindowsAndMessaging::{CW_USEDEFAULT, WS_OVERLAPPEDWINDOW};

        unsafe {
            let _ = OleInitialize(None);
            let hinstance = GetModuleHandleW(None).unwrap();
            let parent = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                w!("STATIC"),
                w!("TestParent"),
                WS_OVERLAPPEDWINDOW,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                800,
                600,
                None,
                None,
                Some(hinstance.into()),
                None,
            )
            .expect("Création fenêtre parente de test");

            let host = RdpActiveXHost::create(1, parent, 0, 0, 800, 600)
                .expect("Instanciation du contrôle ActiveX MsTscAx.dll");

            assert!(!host.hwnd().is_invalid());
            assert_eq!(host.session_id(), 1);
            // Le cookie d'abonnement IConnectionPoint::Advise(IMsTscAxEvents) DOIT être non nul
            assert_ne!(
                host.advise_cookie(),
                0,
                "Le sink d'événements IMsTscAxEvents doit être abonné avec succès"
            );
            assert_eq!(host.connection_state(), 0);
            assert!(!host.is_active());
            assert!(!host.version_string().is_empty());
            assert!(!host.control_id().is_empty());

            let (_ext, err_desc) = host.describe_disconnect(516);
            assert!(!err_desc.is_empty());

            drop(host);
            let _ = DestroyWindow(parent);
            OleUninitialize();
        }
    }
}
