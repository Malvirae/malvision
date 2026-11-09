//! Gestionnaire de sessions RDP simultanées avec onglets (Jalon 2).
//!
//! Chaque session RDP ouverte possède :
//! - Son identifiant unique `session_id` (`u32`).
//! - Sa propre instance `RdpActiveXHost` (`AtlAxWin` + `IMsRdpClient9` + sink `IMsTscAxEvents`).
//! - Son état de connexion vérifié en temps réel (`SessionStatus`).
//! - Son cycle de vie COM strictement confiné au thread STA principal.
//!
//! Changement d'onglet :
//! - Masque les conteneurs `AtlAxWin` des sessions en arrière-plan (`SW_HIDE`) sans jamais les détruire.
//! - Affiche le conteneur `AtlAxWin` de l'onglet actif (`SW_SHOW`), le redimensionne à la zone centrale
//!   et active son focus OLE (`OLEIVERB_UIACTIVATE`).

use crate::models::{ConnectionItem, Protocol};
use crate::rdp::{
    RDP_EVT_AUTH_WARNING_DISMISSED, RDP_EVT_AUTH_WARNING_DISPLAYED, RDP_EVT_AUTO_RECONNECTING,
    RDP_EVT_CONNECTED, RDP_EVT_CONNECTING, RDP_EVT_DISCONNECTED, RDP_EVT_FATAL_ERROR,
    RDP_EVT_LOGIN_COMPLETE, RDP_EVT_LOGON_ERROR, RDP_EVT_WARNING, RdpActiveXHost,
    RdpConnectionConfig,
};
use crate::security::SecretString;
use windows::Win32::Foundation::{HWND, RECT};

/// État courant d'un onglet de session RDP.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionStatus {
    Connecting,
    Connected,
    LoggedIn,
    Disconnected,
    Error,
}

/// Représente une session RDP ouverte dans un onglet.
pub struct RdpSessionTab {
    pub session_id: u32,
    pub title: String,
    pub protocol: Protocol,
    pub host_label: String,
    pub status: SessionStatus,
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

    /// Crée et démarre un nouvel onglet de session RDP intégrée dans `parent_hwnd`.
    pub fn open_rdp_session(
        &mut self,
        parent_hwnd: HWND,
        viewport: RECT,
        conn: &ConnectionItem,
        decrypted_password: Option<SecretString>,
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
            title: conn.name.clone(),
            protocol: conn.protocol,
            host_label,
            status: SessionStatus::Connecting,
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

    /// Ferme un onglet de session RDP : déconnecte proprement la session et détruit son conteneur ActiveX.
    pub fn close_tab(&mut self, session_id: u32, viewport: RECT) -> bool {
        let Some(idx) = self.tabs.iter().position(|t| t.session_id == session_id) else {
            return false;
        };

        let removed = self.tabs.remove(idx);
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
        for tab in self.tabs.drain(..) {
            if tab.host.is_active() {
                let _ = tab.host.disconnect();
            }
            drop(tab);
        }
        self.active_session_id = None;
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

    /// Traite un événement COM `IMsTscAxEvents` reçu pour `session_id`.
    /// Retourne `Some((title, error_message))` si une erreur nécessitant une alerte utilisateur est survenue.
    pub fn handle_rdp_event(
        &mut self,
        session_id: u32,
        event_code: usize,
        param: isize,
    ) -> Option<(String, String)> {
        let tab = self.tabs.iter_mut().find(|t| t.session_id == session_id)?;
        let target = tab.host_label.clone();
        let real_conn_state = tab.host.connection_state();

        match event_code {
            RDP_EVT_CONNECTING => {
                tab.status = SessionStatus::Connecting;
                tab.last_error = None;
                tab.status_detail =
                    format!("Connexion RDP en cours vers {target} (NLA / négociation TLS)...");
                None
            }
            RDP_EVT_CONNECTED => {
                // Ne jamais afficher Connecté sans confirmation réelle de get_Connected() == 1
                if real_conn_state == 1 {
                    tab.status = SessionStatus::Connected;
                    tab.last_error = None;
                    tab.status_detail =
                        format!("Connecté à {target} — Ouverture de session en cours...");
                    if self.active_session_id == Some(session_id) {
                        tab.host.focus_control();
                    }
                }
                None
            }
            RDP_EVT_LOGIN_COMPLETE => {
                if real_conn_state == 1 {
                    tab.status = SessionStatus::LoggedIn;
                    tab.last_error = None;
                    tab.status_detail =
                        format!("Connecté à {target} — Session RDP authentifiée et prête.");
                    if self.active_session_id == Some(session_id) {
                        tab.host.focus_control();
                    }
                }
                None
            }
            RDP_EVT_AUTH_WARNING_DISPLAYED => {
                tab.status_detail = format!(
                    "Avertissement de certificat / sécurité Windows affiché pour {target}."
                );
                None
            }
            RDP_EVT_AUTH_WARNING_DISMISSED => {
                tab.status_detail = format!(
                    "Dialogue de sécurité Windows validé pour {target}, poursuite de la connexion..."
                );
                None
            }
            RDP_EVT_AUTO_RECONNECTING => {
                tab.status = SessionStatus::Connecting;
                tab.status_detail =
                    format!("Reconnexion automatique RDP en cours vers {target} (code={param})...");
                None
            }
            RDP_EVT_LOGON_ERROR => {
                let code = param as i32;
                if code != -2 {
                    tab.status_detail = format!(
                        "Notification d'ouverture de session Windows sur {target} (code={code})."
                    );
                }
                None
            }
            RDP_EVT_WARNING => {
                let code = param as i32;
                tab.status_detail = format!("Avertissement RDP reçu de {target} (code={code}).");
                None
            }
            RDP_EVT_FATAL_ERROR => {
                let code = param as i32;
                tab.status = SessionStatus::Error;
                let msg =
                    format!("Erreur fatale du contrôle RDP sur {target} (code d'erreur={code}).");
                tab.last_error = Some(msg.clone());
                tab.status_detail = msg.clone();
                Some(("Erreur fatale RDP".to_string(), msg))
            }
            RDP_EVT_DISCONNECTED => {
                let disc_reason = param as i32;
                let (ext_reason, desc) = tab.host.describe_disconnect(disc_reason);
                let is_normal_close = matches!(disc_reason, 1..=3) && matches!(ext_reason, 0 | 12);

                if is_normal_close {
                    tab.status = SessionStatus::Disconnected;
                    tab.last_error = None;
                    tab.status_detail = format!("Déconnecté de {target} ({desc})");
                    None
                } else {
                    tab.status = SessionStatus::Error;
                    let full_err = format!(
                        "Déconnexion de {target} [code={disc_reason}, étendu={ext_reason}] : {desc}"
                    );
                    tab.last_error = Some(full_err.clone());
                    tab.status_detail = full_err.clone();
                    Some(("Session RDP interrompue".to_string(), full_err))
                }
            }
            _ => None,
        }
    }

    /// Synchronise périodiquement l'état de tous les onglets avec `IMsRdpClient::get_Connected()`.
    /// Retourne `true` si l'état d'au moins un onglet a changé (pour rafraîchir la barre d'onglets et la barre d'état).
    pub fn sync_real_states(&mut self) -> bool {
        let mut changed = false;
        for tab in &mut self.tabs {
            let real_state = tab.host.connection_state();
            let target = &tab.host_label;
            match real_state {
                1 => {
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
                0 => {
                    if matches!(
                        tab.status,
                        SessionStatus::Connecting
                            | SessionStatus::Connected
                            | SessionStatus::LoggedIn
                    ) {
                        tab.status = SessionStatus::Disconnected;
                        tab.status_detail = format!("Déconnecté de {target}.");
                        changed = true;
                    }
                }
                _ => {}
            }
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
                title: "SRV-01".to_string(),
                protocol: Protocol::Rdp,
                host_label: "10.0.0.1".to_string(),
                status: SessionStatus::Disconnected,
                status_detail: "Prêt".to_string(),
                last_error: None,
                host: host1,
            });
            mgr.tabs.push(RdpSessionTab {
                session_id: 2,
                title: "SRV-02".to_string(),
                protocol: Protocol::Rdp,
                host_label: "10.0.0.2".to_string(),
                status: SessionStatus::Disconnected,
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
}
