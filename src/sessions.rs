//! Gestionnaire de sessions RDP simultanées avec onglets (Jalons 2 & 2C).
//!
//! Chaque session RDP ouverte possède :
//! - Son identifiant unique `session_id` (`u32`).
//! - Son identifiant de connexion SQLite éventuel `connection_id` (`Option<i64>`).
//! - Sa propre instance `RdpActiveXHost` (`AtlAxWin` + `IMsRdpClient9` + sink `IMsTscAxEvents`).
//! - Son état de connexion vérifié en temps réel (`SessionStatus`).
//! - Son cycle de vie COM strictement confiné au thread STA principal.
//!
//! Fonctionnalités Jalon 2C :
//! - Fermeture automatique de l'onglet lorsqu'une session RDP est réellement déconnectée
//!   (déconnexion volontaire, fermeture de session Windows distante, déconnexion depuis le bureau
//!   distant ou coupure réseau confirmée), avec libération propre des ressources COM/ActiveX.
//! - Maintien de l'onglet ouvert lors d'une interruption réseau transitoire avec reconnexion
//!   automatique en cours (`RDP_EVT_AUTO_RECONNECTING`).
//! - Sélection automatique d'un autre onglet RDP s'il en reste, ou retour à l'écran `Accueil`
//!   lorsque le dernier onglet se ferme.
//! - Protection contre les doubles destructions COM et la réentrance depuis les callbacks.

use crate::models::{ConnectionItem, Protocol};
use crate::rdp::{
    RDP_EVT_AUTH_WARNING_DISMISSED, RDP_EVT_AUTH_WARNING_DISPLAYED, RDP_EVT_AUTO_RECONNECTING,
    RDP_EVT_CONNECTED, RDP_EVT_CONNECTING, RDP_EVT_DISCONNECTED, RDP_EVT_FATAL_ERROR,
    RDP_EVT_LOGIN_COMPLETE, RDP_EVT_LOGON_ERROR, RDP_EVT_WARNING, RdpActiveXHost,
    RdpConnectionConfig,
};
use crate::security::SecretString;
use crate::settings::RdpKeyboardHookMode;
use windows::Win32::Foundation::{HWND, RECT};

/// État courant d'un onglet de session RDP.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionStatus {
    Connecting,
    Connected,
    LoggedIn,
    #[allow(dead_code)]
    Disconnected,
    #[allow(dead_code)]
    Error,
}

/// Résultat du traitement d'un événement RDP par `SessionManager`.
#[derive(Debug, Clone, Default)]
pub struct RdpEventOutcome {
    /// Identifiant de l'onglet fermé automatiquement suite à une déconnexion confirmée.
    #[allow(dead_code)]
    pub closed_session_id: Option<u32>,
    /// Message de statut à afficher dans la barre d'état.
    pub status_message: Option<String>,
    /// Alerte d'erreur éventuelle `(titre, message)` à présenter si la déconnexion est anormale.
    pub error_alert: Option<(String, String)>,
}

/// Représente une session RDP ouverte dans un onglet.
pub struct RdpSessionTab {
    pub session_id: u32,
    pub connection_id: Option<i64>,
    pub title: String,
    pub protocol: Protocol,
    pub host_label: String,
    pub status: SessionStatus,
    pub auto_reconnecting: bool,
    pub status_detail: String,
    pub last_error: Option<String>,
    pub host: RdpActiveXHost,
}

/// Gestionnaire central des onglets de sessions RDP actives/ouvertes.
pub struct SessionManager {
    next_session_id: u32,
    tabs: Vec<RdpSessionTab>,
    active_session_id: Option<u32>,
}

impl Default for SessionManager {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionManager {
    pub fn new() -> Self {
        Self {
            next_session_id: 1,
            tabs: Vec::new(),
            active_session_id: None,
        }
    }

    pub fn tabs(&self) -> &[RdpSessionTab] {
        &self.tabs
    }

    pub fn is_empty(&self) -> bool {
        self.tabs.is_empty()
    }

    pub fn active_session_id(&self) -> Option<u32> {
        self.active_session_id
    }

    pub fn active_tab(&self) -> Option<&RdpSessionTab> {
        let active_id = self.active_session_id?;
        self.tabs.iter().find(|t| t.session_id == active_id)
    }

    pub fn active_tab_mut(&mut self) -> Option<&mut RdpSessionTab> {
        let active_id = self.active_session_id?;
        self.tabs.iter_mut().find(|t| t.session_id == active_id)
    }

    /// Retourne la liste des identifiants SQLite `connection_id` des sessions RDP actuellement ouvertes.
    pub fn open_connection_ids(&self) -> Vec<i64> {
        let mut ids = Vec::new();
        for tab in &self.tabs {
            if let Some(cid) = tab.connection_id
                && !ids.contains(&cid)
            {
                ids.push(cid);
            }
        }
        ids
    }

    /// Crée et démarre un nouvel onglet de session RDP intégrée dans `parent_hwnd`.
    pub fn open_rdp_session(
        &mut self,
        parent_hwnd: HWND,
        viewport: RECT,
        conn: &ConnectionItem,
        decrypted_password: Option<SecretString>,
        keyboard_hook_mode: RdpKeyboardHookMode,
    ) -> Result<u32, String> {
        let session_id = self.next_session_id;
        self.next_session_id = self.next_session_id.wrapping_add(1).max(1);

        let x = viewport.left;
        let y = viewport.top;
        let width = (viewport.right - viewport.left).max(640);
        let height = (viewport.bottom - viewport.top).max(480);

        let mut host = RdpActiveXHost::create(session_id, parent_hwnd, x, y, width, height)?;

        let pwd_str = decrypted_password
            .as_ref()
            .map(|s| s.as_str().to_string())
            .unwrap_or_default();

        let mut cfg = RdpConnectionConfig {
            host: conn.host.clone(),
            port: conn.port,
            domain: conn.domain.clone(),
            username: conn.username.clone(),
            password: pwd_str,
            width: width as u32,
            height: height as u32,
            dynamic_resolution: conn.dynamic_resolution,
            redirect_clipboard: conn.redirect_clipboard,
            redirect_drives: conn.redirect_clipboard,
            keyboard_hook_mode,
        };

        let connect_res = host.connect(&cfg);
        crate::security::zeroize_string(&mut cfg.password);
        drop(decrypted_password);

        connect_res?;

        let host_label = conn.endpoint_label();
        let detail = format!(
            "Connexion en cours vers {host_label} (MsTscAx v{}, {}, sink=#{})...",
            host.version_string(),
            host.control_id(),
            host.advise_cookie()
        );

        let tab = RdpSessionTab {
            session_id: host.session_id(),
            connection_id: Some(conn.id),
            title: conn.name.clone(),
            protocol: conn.protocol,
            host_label,
            status: SessionStatus::Connecting,
            auto_reconnecting: false,
            status_detail: detail,
            last_error: None,
            host,
        };

        self.tabs.push(tab);
        self.select_tab(session_id, viewport);
        Ok(session_id)
    }

    /// Sélectionne un onglet de session RDP : masque les autres conteneurs sans les détruire,
    /// affiche et redimensionne l'onglet actif, puis lui transfère le focus clavier/souris OLE.
    pub fn select_tab(&mut self, session_id: u32, viewport: RECT) -> bool {
        if !self.tabs.iter().any(|t| t.session_id == session_id) {
            return false;
        }
        self.active_session_id = Some(session_id);

        let x = viewport.left;
        let y = viewport.top;
        let w = (viewport.right - viewport.left).max(100);
        let h = (viewport.bottom - viewport.top).max(100);

        for tab in &mut self.tabs {
            if tab.session_id == session_id {
                tab.host.set_visible(true);
                tab.host.resize(x, y, w, h);
                tab.host.focus_control();
            } else {
                tab.host.set_visible(false);
            }
        }
        true
    }

    /// Ferme un onglet de session RDP : désabonne immédiatement le sink COM (`deactivate_events`),
    /// déconnecte proprement la session si nécessaire et détruit son conteneur ActiveX sans affecter
    /// les autres sessions. Sélectionne un autre onglet RDP s'il en reste, ou revient à `Accueil` (`None`).
    pub fn close_tab(&mut self, session_id: u32, viewport: RECT) -> bool {
        let Some(idx) = self.tabs.iter().position(|t| t.session_id == session_id) else {
            return false;
        };

        // Retirer l'onglet de `self.tabs` AVANT tout appel COM pour garantir qu'aucun événement
        // réentrant ou déjà en file d'attente ne puisse tenter une double destruction.
        let mut removed = self.tabs.remove(idx);
        removed.host.deactivate_events();
        if removed.host.is_active() {
            let _ = removed.host.disconnect();
        }
        drop(removed);

        if self.active_session_id == Some(session_id) {
            if self.tabs.is_empty() {
                self.active_session_id = None;
            } else {
                let next_idx = idx.min(self.tabs.len() - 1);
                let next_id = self.tabs[next_idx].session_id;
                self.select_tab(next_id, viewport);
            }
        }
        true
    }

    /// Ferme proprement toutes les sessions RDP ouvertes (ex. fermeture de l'application).
    pub fn close_all(&mut self) {
        let drained: Vec<RdpSessionTab> = std::mem::take(&mut self.tabs);
        self.active_session_id = None;
        for mut tab in drained {
            tab.host.deactivate_events();
            if tab.host.is_active() {
                let _ = tab.host.disconnect();
            }
            drop(tab);
        }
    }

    /// Redimensionne l'onglet actif lorsque la zone centrale change de taille (`WM_SIZE` ou déplacement du séparateur).
    pub fn resize_active(&mut self, viewport: RECT) {
        let x = viewport.left;
        let y = viewport.top;
        let w = (viewport.right - viewport.left).max(100);
        let h = (viewport.bottom - viewport.top).max(100);

        if let Some(active) = self.active_tab_mut() {
            active.host.resize(x, y, w, h);
        }
    }

    /// Envoie Ctrl + Alt + Suppr à la session RDP active via l'API officielle SendKeys.
    pub fn send_cad_to_active(&self) -> Result<(), String> {
        let Some(tab) = self.active_tab() else {
            return Err("Aucun onglet RDP actif pour envoyer Ctrl + Alt + Suppr.".to_string());
        };
        tab.host.send_cad()
    }

    /// Envoie Ctrl + Alt + Fin à la session RDP active (invoque l'écran de sécurité distant sous RDP).
    pub fn send_ctrl_alt_end_to_active(&self) -> Result<(), String> {
        let Some(tab) = self.active_tab() else {
            return Err("Aucun onglet RDP actif pour envoyer Ctrl + Alt + Fin.".to_string());
        };
        tab.host.send_ctrl_alt_end()
    }

    /// Bascule la session RDP active entre mode plein écran et fenêtré.
    pub fn toggle_active_fullscreen(&mut self) -> Result<bool, String> {
        let Some(tab) = self.active_tab() else {
            return Err("Aucun onglet RDP actif pour basculer le plein écran.".to_string());
        };
        let new_state = !tab.host.is_full_screen();
        tab.host.set_full_screen(new_state)?;
        Ok(new_state)
    }

    /// Indique si la session RDP active est actuellement en plein écran.
    pub fn is_active_fullscreen(&self) -> bool {
        self.active_tab()
            .map(|t| t.host.is_full_screen())
            .unwrap_or(false)
    }

    /// Traite un événement COM `IMsTscAxEvents` reçu pour `session_id` et ferme automatiquement
    /// l'onglet si la déconnexion est réellement confirmée.
    pub fn handle_rdp_event_with_viewport(
        &mut self,
        session_id: u32,
        event_code: usize,
        param: isize,
        viewport: RECT,
    ) -> RdpEventOutcome {
        let Some(tab) = self.tabs.iter_mut().find(|t| t.session_id == session_id) else {
            // Si un callback COM tardif arrive après la fermeture de l'onglet, l'ignorer proprement.
            return RdpEventOutcome::default();
        };
        let target = tab.host_label.clone();
        let title = tab.title.clone();
        let real_conn_state = tab.host.connection_state();

        match event_code {
            RDP_EVT_CONNECTING => {
                tab.status = SessionStatus::Connecting;
                tab.auto_reconnecting = false;
                tab.last_error = None;
                let msg =
                    format!("Connexion RDP en cours vers {target} (NLA / négociation TLS)...");
                tab.status_detail = msg.clone();
                RdpEventOutcome {
                    closed_session_id: None,
                    status_message: Some(msg),
                    error_alert: None,
                }
            }
            RDP_EVT_CONNECTED => {
                // Ne jamais afficher Connecté sans confirmation réelle de get_Connected() == 1
                if real_conn_state == 1 {
                    tab.status = SessionStatus::Connected;
                    tab.auto_reconnecting = false;
                    tab.last_error = None;
                    let msg = format!("Connecté à {target} — Ouverture de session en cours...");
                    tab.status_detail = msg.clone();
                    if self.active_session_id == Some(session_id) {
                        tab.host.focus_control();
                    }
                    RdpEventOutcome {
                        closed_session_id: None,
                        status_message: Some(msg),
                        error_alert: None,
                    }
                } else {
                    RdpEventOutcome::default()
                }
            }
            RDP_EVT_LOGIN_COMPLETE => {
                if real_conn_state == 1 {
                    tab.status = SessionStatus::LoggedIn;
                    tab.auto_reconnecting = false;
                    tab.last_error = None;
                    let msg = format!("Connecté à {target} — Session RDP authentifiée et prête.");
                    tab.status_detail = msg.clone();
                    if self.active_session_id == Some(session_id) {
                        tab.host.focus_control();
                    }
                    RdpEventOutcome {
                        closed_session_id: None,
                        status_message: Some(msg),
                        error_alert: None,
                    }
                } else {
                    RdpEventOutcome::default()
                }
            }
            RDP_EVT_AUTH_WARNING_DISPLAYED => {
                let msg = format!(
                    "Avertissement de certificat / sécurité Windows affiché pour {target}."
                );
                tab.status_detail = msg.clone();
                RdpEventOutcome {
                    closed_session_id: None,
                    status_message: Some(msg),
                    error_alert: None,
                }
            }
            RDP_EVT_AUTH_WARNING_DISMISSED => {
                let msg = format!(
                    "Dialogue de sécurité Windows validé pour {target}, poursuite de la connexion..."
                );
                tab.status_detail = msg.clone();
                RdpEventOutcome {
                    closed_session_id: None,
                    status_message: Some(msg),
                    error_alert: None,
                }
            }
            RDP_EVT_AUTO_RECONNECTING => {
                // Interruption réseau transitoire avec reconnexion automatique en cours :
                // Ne PAS fermer l'onglet.
                tab.status = SessionStatus::Connecting;
                tab.auto_reconnecting = true;
                let msg =
                    format!("Reconnexion automatique RDP en cours vers {target} (code={param})...");
                tab.status_detail = msg.clone();
                RdpEventOutcome {
                    closed_session_id: None,
                    status_message: Some(msg),
                    error_alert: None,
                }
            }
            RDP_EVT_LOGON_ERROR => {
                let code = param as i32;
                if code != -2 {
                    let msg = format!(
                        "Notification d'ouverture de session Windows sur {target} (code={code})."
                    );
                    tab.status_detail = msg.clone();
                    RdpEventOutcome {
                        closed_session_id: None,
                        status_message: Some(msg),
                        error_alert: None,
                    }
                } else {
                    RdpEventOutcome::default()
                }
            }
            RDP_EVT_WARNING => {
                let code = param as i32;
                let msg = format!("Avertissement RDP reçu de {target} (code={code}).");
                tab.status_detail = msg.clone();
                RdpEventOutcome {
                    closed_session_id: None,
                    status_message: Some(msg),
                    error_alert: None,
                }
            }
            RDP_EVT_FATAL_ERROR => {
                let code = param as i32;
                let msg = format!(
                    "Erreur fatale du contrôle RDP sur « {title} » ({target}, code={code})."
                );
                self.close_tab(session_id, viewport);
                RdpEventOutcome {
                    closed_session_id: Some(session_id),
                    status_message: Some(msg.clone()),
                    error_alert: Some(("Erreur fatale RDP".to_string(), msg)),
                }
            }
            RDP_EVT_DISCONNECTED => {
                // Si une reconnexion automatique est encore activement en cours dans le contrôle
                // ActiveX (get_Connected() == 2), ne pas fermer prématurément l'onglet.
                if real_conn_state == 2 {
                    return RdpEventOutcome::default();
                }

                let disc_reason = param as i32;
                let (ext_reason, desc) = tab.host.describe_disconnect(disc_reason);
                // Déconnexion volontaire, déconnexion depuis le bureau distant ou fermeture de session Windows distante :
                // disc_reason 1 (locale), 2 (par le serveur/logoff), 3 (initiée par le serveur)
                // avec ext_reason 0 (aucune erreur), 5 (remplacée), 11 (déconnectée par l'utilisateur) ou 12 (logoff distant).
                let is_normal_close =
                    matches!(disc_reason, 1..=3) && matches!(ext_reason, 0 | 5 | 11 | 12);

                // Fermer automatiquement l'onglet correspondant et libérer ses ressources COM/ActiveX
                self.close_tab(session_id, viewport);

                if is_normal_close {
                    let msg = format!("Session « {title} » ({target}) terminée ({desc}).");
                    RdpEventOutcome {
                        closed_session_id: Some(session_id),
                        status_message: Some(msg),
                        error_alert: None,
                    }
                } else {
                    let full_err = format!(
                        "Déconnexion de « {title} » ({target}) [code={disc_reason}, étendu={ext_reason}] : {desc}"
                    );
                    RdpEventOutcome {
                        closed_session_id: Some(session_id),
                        status_message: Some(full_err.clone()),
                        error_alert: Some(("Session RDP interrompue".to_string(), full_err)),
                    }
                }
            }
            _ => RdpEventOutcome::default(),
        }
    }

    /// Synchronise périodiquement l'état de tous les onglets avec `IMsRdpClient::get_Connected()`.
    /// Si une session précédemment connectée (`Connected` / `LoggedIn`) est passée à l'état
    /// réellement déconnecté (`0`) sans reconnexion automatique en cours, ferme automatiquement
    /// son onglet.
    /// Retourne `true` si l'état d'au moins un onglet a changé ou si un onglet a été fermé.
    pub fn sync_real_states_with_viewport(&mut self, viewport: RECT) -> bool {
        let mut changed = false;
        let mut to_close = Vec::new();

        for tab in &mut self.tabs {
            let real_state = tab.host.connection_state();
            let target = &tab.host_label;
            match real_state {
                1 => {
                    tab.auto_reconnecting = false;
                    if matches!(
                        tab.status,
                        SessionStatus::Connecting
                            | SessionStatus::Disconnected
                            | SessionStatus::Error
                    ) {
                        tab.status = SessionStatus::Connected;
                        tab.last_error = None;
                        tab.status_detail = format!("Connecté à {target} — Session RDP active.");
                        changed = true;
                    }
                }
                2 => {
                    if matches!(tab.status, SessionStatus::Disconnected) {
                        tab.status = SessionStatus::Connecting;
                        tab.status_detail =
                            format!("Connexion en cours vers {target} (NLA / négociation TLS)...");
                        changed = true;
                    }
                }
                0 if matches!(
                    tab.status,
                    SessionStatus::Connected | SessionStatus::LoggedIn
                ) && !tab.auto_reconnecting =>
                {
                    to_close.push(tab.session_id);
                    changed = true;
                }
                _ => {}
            }
        }

        for sid in to_close {
            self.close_tab(sid, viewport);
        }

        changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::System::Ole::{OleInitialize, OleUninitialize};
    use windows::Win32::UI::WindowsAndMessaging::{
        CW_USEDEFAULT, CreateWindowExW, DestroyWindow, WINDOW_EX_STYLE, WS_OVERLAPPEDWINDOW,
    };
    use windows_core::w;

    #[test]
    fn test_session_manager_multi_tab_lifecycle_without_destroying_inactive() {
        unsafe {
            let _ = OleInitialize(None);
            let hinstance = GetModuleHandleW(None).unwrap();
            let parent = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                w!("STATIC"),
                w!("TestSessionParent"),
                WS_OVERLAPPEDWINDOW,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                1024,
                768,
                None,
                None,
                Some(hinstance.into()),
                None,
            )
            .expect("Fenêtre parente de test");

            let viewport = RECT {
                left: 260,
                top: 40,
                right: 1024,
                bottom: 740,
            };

            let mut mgr = SessionManager::new();
            assert!(mgr.is_empty());

            // Créer deux onglets ActiveX indépendants (sans lancer Connect réseau)
            let host1 = RdpActiveXHost::create(1, parent, 260, 40, 764, 700).unwrap();
            let host2 = RdpActiveXHost::create(2, parent, 260, 40, 764, 700).unwrap();
            let hwnd1 = host1.hwnd();
            let hwnd2 = host2.hwnd();

            mgr.tabs.push(RdpSessionTab {
                session_id: 1,
                connection_id: Some(101),
                title: "SRV-01".to_string(),
                protocol: Protocol::Rdp,
                host_label: "10.0.0.1".to_string(),
                status: SessionStatus::Disconnected,
                auto_reconnecting: false,
                status_detail: "Prêt".to_string(),
                last_error: None,
                host: host1,
            });
            mgr.tabs.push(RdpSessionTab {
                session_id: 2,
                connection_id: Some(102),
                title: "SRV-02".to_string(),
                protocol: Protocol::Rdp,
                host_label: "10.0.0.2".to_string(),
                status: SessionStatus::Disconnected,
                auto_reconnecting: false,
                status_detail: "Prêt".to_string(),
                last_error: None,
                host: host2,
            });

            // Sélectionner l'onglet 1 puis l'onglet 2 : les deux HWNDs ActiveX doivent rester valides
            assert!(mgr.select_tab(1, viewport));
            assert_eq!(mgr.active_session_id(), Some(1));
            assert!(mgr.select_tab(2, viewport));
            assert_eq!(mgr.active_session_id(), Some(2));
            assert_eq!(mgr.tabs().len(), 2);
            assert!(!hwnd1.is_invalid());
            assert!(!hwnd2.is_invalid());

            // Fermer l'onglet 2 : l'onglet 1 redevient automatiquement l'onglet actif
            assert!(mgr.close_tab(2, viewport));
            assert_eq!(mgr.active_session_id(), Some(1));
            assert_eq!(mgr.tabs().len(), 1);

            mgr.close_all();
            assert!(mgr.is_empty());

            let _ = DestroyWindow(parent);
            OleUninitialize();
        }
    }

    #[test]
    fn test_auto_close_disconnected_tab_preserves_other_sessions_and_returns_to_home() {
        unsafe {
            let _ = OleInitialize(None);
            let hinstance = GetModuleHandleW(None).unwrap();
            let parent = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                w!("STATIC"),
                w!("TestAutoCloseParent"),
                WS_OVERLAPPEDWINDOW,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                1024,
                768,
                None,
                None,
                Some(hinstance.into()),
                None,
            )
            .expect("Fenêtre parente de test");

            let viewport = RECT {
                left: 260,
                top: 40,
                right: 1024,
                bottom: 740,
            };

            let mut mgr = SessionManager::new();
            let host1 = RdpActiveXHost::create(10, parent, 260, 40, 764, 700).unwrap();
            let host2 = RdpActiveXHost::create(20, parent, 260, 40, 764, 700).unwrap();
            let host3 = RdpActiveXHost::create(30, parent, 260, 40, 764, 700).unwrap();

            mgr.tabs.push(RdpSessionTab {
                session_id: 10,
                connection_id: Some(1),
                title: "SRV-DC-01".to_string(),
                protocol: Protocol::Rdp,
                host_label: "10.0.0.10:3389".to_string(),
                status: SessionStatus::LoggedIn,
                auto_reconnecting: false,
                status_detail: "Connecté".to_string(),
                last_error: None,
                host: host1,
            });
            mgr.tabs.push(RdpSessionTab {
                session_id: 20,
                connection_id: Some(2),
                title: "SRV-SQL-02".to_string(),
                protocol: Protocol::Rdp,
                host_label: "10.0.0.20:3389".to_string(),
                status: SessionStatus::LoggedIn,
                auto_reconnecting: false,
                status_detail: "Connecté".to_string(),
                last_error: None,
                host: host2,
            });
            mgr.tabs.push(RdpSessionTab {
                session_id: 30,
                connection_id: Some(3),
                title: "SRV-APP-03".to_string(),
                protocol: Protocol::Rdp,
                host_label: "10.0.0.30:3389".to_string(),
                status: SessionStatus::LoggedIn,
                auto_reconnecting: false,
                status_detail: "Connecté".to_string(),
                last_error: None,
                host: host3,
            });
            mgr.select_tab(20, viewport);
            assert_eq!(mgr.active_session_id(), Some(20));

            // 1. Interruption réseau transitoire avec reconnexion automatique (RDP_EVT_AUTO_RECONNECTING) :
            // l'onglet 20 NE DOIT PAS se fermer.
            let reconnect_outcome =
                mgr.handle_rdp_event_with_viewport(20, RDP_EVT_AUTO_RECONNECTING, 1, viewport);
            assert_eq!(reconnect_outcome.closed_session_id, None);
            assert_eq!(mgr.tabs().len(), 3);
            assert!(
                mgr.tabs()
                    .iter()
                    .any(|t| t.session_id == 20 && t.auto_reconnecting)
            );

            // 2. Déconnexion volontaire / logoff distant sur l'onglet 20 (disc_reason = 2) :
            // l'onglet 20 se ferme automatiquement, les sessions 10 et 30 restent actives,
            // et l'onglet 30 devient l'onglet actif.
            let disc_20 = mgr.handle_rdp_event_with_viewport(20, RDP_EVT_DISCONNECTED, 2, viewport);
            assert_eq!(disc_20.closed_session_id, Some(20));
            assert!(disc_20.error_alert.is_none());
            assert_eq!(mgr.tabs().len(), 2);
            assert_eq!(mgr.open_connection_ids(), vec![1, 3]);
            assert_eq!(mgr.active_session_id(), Some(30));

            // 3. Si un callback COM tardif arrive pour l'onglet 20 déjà fermé :
            // aucune panique ni double destruction.
            let late_cb = mgr.handle_rdp_event_with_viewport(20, RDP_EVT_DISCONNECTED, 1, viewport);
            assert_eq!(late_cb.closed_session_id, None);
            assert_eq!(mgr.tabs().len(), 2);

            // 4. Déconnexion réseau confirmée sur l'onglet 10 (disc_reason = 2308) :
            // l'onglet 10 se ferme, seul l'onglet 30 reste.
            let disc_10 =
                mgr.handle_rdp_event_with_viewport(10, RDP_EVT_DISCONNECTED, 2308, viewport);
            assert_eq!(disc_10.closed_session_id, Some(10));
            assert!(disc_10.error_alert.is_some());
            assert_eq!(mgr.tabs().len(), 1);
            assert_eq!(mgr.active_session_id(), Some(30));

            // 5. Fermeture de la dernière session (onglet 30) :
            // l'onglet 30 se ferme et Malvision revient automatiquement à l'onglet Accueil (active_session_id == None).
            let disc_30 = mgr.handle_rdp_event_with_viewport(30, RDP_EVT_DISCONNECTED, 1, viewport);
            assert_eq!(disc_30.closed_session_id, Some(30));
            assert!(mgr.is_empty());
            assert_eq!(mgr.active_session_id(), None);

            let _ = DestroyWindow(parent);
            OleUninitialize();
        }
    }
}
