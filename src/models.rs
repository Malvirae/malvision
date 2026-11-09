//! Modèles de données métier de Malvision (Jalon 2) : Protocoles, Dossiers, Connexions,
//! Paramètres RDP et entrées d'historique récent.

use crate::rdp::{parse_host_and_port, split_domain_username};
use crate::security::SecretString;

/// Protocoles de connexion distante pris en charge par Malvision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Protocol {
    Rdp,
    AnyDesk,
    RustDesk,
}

impl Protocol {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Rdp => "rdp",
            Self::AnyDesk => "anydesk",
            Self::RustDesk => "rustdesk",
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Self::Rdp => "RDP",
            Self::AnyDesk => "AnyDesk",
            Self::RustDesk => "RustDesk",
        }
    }

    pub fn badge_prefix(self) -> &'static str {
        match self {
            Self::Rdp => "[RDP]",
            Self::AnyDesk => "[ANY]",
            Self::RustDesk => "[RST]",
        }
    }

    pub fn from_db_str(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "rdp" => Ok(Self::Rdp),
            "anydesk" => Ok(Self::AnyDesk),
            "rustdesk" => Ok(Self::RustDesk),
            other => Err(format!("Protocole inconnu en base de données : '{other}'.")),
        }
    }
}

/// Dossier d'organisation dans l'arborescence (profondeur arbitraire via `parent_id`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Folder {
    pub id: i64,
    pub parent_id: Option<i64>,
    pub name: String,
    pub sort_order: i32,
    pub is_expanded: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

/// Enregistrement complet d'une connexion dans la base locale.
///
/// Ne contient jamais le mot de passe en clair : seul `has_saved_password` indique
/// la présence d'un blob DPAPI dans SQLite, déchiffré uniquement à la demande.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionItem {
    pub id: i64,
    pub folder_id: Option<i64>,
    pub name: String,
    pub protocol: Protocol,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub domain: String,
    pub description: String,
    pub is_favorite: bool,
    pub save_password: bool,
    pub has_saved_password: bool,
    pub dynamic_resolution: bool,
    pub redirect_clipboard: bool,
    pub sort_order: i32,
    pub created_at: i64,
    pub updated_at: i64,
    pub last_connected_at: Option<i64>,
}

impl ConnectionItem {
    /// Retourne une étiquette d'adresse lisible (ex. `192.168.1.10:3389` ou l'ID AnyDesk/RustDesk).
    pub fn endpoint_label(&self) -> String {
        match self.protocol {
            Protocol::Rdp => {
                if self.port == 3389 {
                    self.host.clone()
                } else {
                    format!("{}:{}", self.host, self.port)
                }
            }
            Protocol::AnyDesk | Protocol::RustDesk => self.host.clone(),
        }
    }
}

/// Données de création ou de mise à jour d'une connexion depuis le formulaire UI.
#[derive(Debug, Clone)]
pub struct ConnectionDraft {
    pub folder_id: Option<i64>,
    pub name: String,
    pub protocol: Protocol,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub domain: String,
    pub description: String,
    pub is_favorite: bool,
    pub save_password: bool,
    /// Mot de passe saisi ou modifié dans le formulaire (sera chiffré avec DPAPI si `save_password` est vrai,
    /// ou effacé s'il est vide et que `keep_existing_password` est faux).
    pub password: SecretString,
    /// Si vrai et que `password` est vide, conserve le blob DPAPI existant lors d'une édition.
    pub keep_existing_password: bool,
    pub dynamic_resolution: bool,
    pub redirect_clipboard: bool,
}

impl ConnectionDraft {
    /// Valide les champs du formulaire et normalise l'hôte / port.
    pub fn validate_and_normalize(&mut self) -> Result<(), String> {
        self.name = self.name.trim().to_string();
        if self.name.is_empty() {
            return Err("Le nom de la connexion ne peut pas être vide.".to_string());
        }
        if self.name.len() > 160 {
            return Err("Le nom de la connexion ne doit pas dépasser 160 caractères.".to_string());
        }

        self.host = self.host.trim().to_string();
        if self.host.is_empty() {
            return Err(match self.protocol {
                Protocol::Rdp => {
                    "L'adresse hôte RDP (IPv4, IPv6 ou DNS) ne peut pas être vide.".to_string()
                }
                Protocol::AnyDesk => {
                    "L'identifiant ou alias AnyDesk ne peut pas être vide.".to_string()
                }
                Protocol::RustDesk => {
                    "L'identifiant ou adresse RustDesk ne peut pas être vide.".to_string()
                }
            });
        }

        match self.protocol {
            Protocol::Rdp => {
                if self.port == 0 {
                    return Err("Le port RDP doit être compris entre 1 et 65535.".to_string());
                }
                // Si l'utilisateur a saisi `hôte:port` dans le champ hôte et laissé le port à 3389,
                // extraire proprement l'hôte et le port.
                let (parsed_host, parsed_port) = parse_host_and_port(&self.host)?;
                if parsed_host.contains(char::is_whitespace) {
                    return Err("L'adresse d'hôte RDP ne doit pas contenir d'espaces.".to_string());
                }
                self.host = parsed_host;
                if parsed_port != 3389 && self.port == 3389 {
                    self.port = parsed_port;
                }
            }
            Protocol::AnyDesk | Protocol::RustDesk => {
                if self.port == 0 {
                    self.port = 3389;
                }
            }
        }

        self.username = self.username.trim().to_string();
        self.domain = self.domain.trim().to_string();
        self.description = self.description.trim().to_string();

        // Si l'utilisateur a saisi `DOMAINE\utilisateur` dans le champ utilisateur et laissé domaine vide
        if self.domain.is_empty() && self.username.contains('\\') {
            let (d, u) = split_domain_username(&self.username);
            self.domain = d;
            self.username = u;
        }

        if !self.save_password {
            self.password.clear();
            self.keep_existing_password = false;
        }

        Ok(())
    }
}

/// Entrée d'historique des connexions récemment lancées.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecentConnectionEntry {
    pub connection: ConnectionItem,
    pub connected_at: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_connection_draft_rdp() {
        let mut draft = ConnectionDraft {
            folder_id: None,
            name: "  Serveur Production  ".to_string(),
            protocol: Protocol::Rdp,
            host: "10.20.30.40:3390".to_string(),
            port: 3389,
            username: "CORP\\admin".to_string(),
            domain: String::new(),
            description: " Contrôleur ".to_string(),
            is_favorite: true,
            save_password: false,
            password: SecretString::new("MonMotDePasse".to_string()),
            keep_existing_password: false,
            dynamic_resolution: true,
            redirect_clipboard: true,
        };

        draft.validate_and_normalize().expect("Validation valide");
        assert_eq!(draft.name, "Serveur Production");
        assert_eq!(draft.host, "10.20.30.40");
        assert_eq!(draft.port, 3390);
        assert_eq!(draft.domain, "CORP");
        assert_eq!(draft.username, "admin");
        // Comme save_password == false, le mot de passe en mémoire doit avoir été purgé
        assert!(draft.password.is_empty());
    }

    #[test]
    fn test_reject_invalid_connection_draft() {
        let mut empty_name = ConnectionDraft {
            folder_id: None,
            name: "   ".to_string(),
            protocol: Protocol::Rdp,
            host: "192.168.1.1".to_string(),
            port: 3389,
            username: String::new(),
            domain: String::new(),
            description: String::new(),
            is_favorite: false,
            save_password: false,
            password: SecretString::new(String::new()),
            keep_existing_password: false,
            dynamic_resolution: true,
            redirect_clipboard: true,
        };
        assert!(empty_name.validate_and_normalize().is_err());

        empty_name.name = "Valide".to_string();
        empty_name.host = "hote avec espace".to_string();
        assert!(empty_name.validate_and_normalize().is_err());

        empty_name.host = "192.168.1.1".to_string();
        empty_name.port = 0;
        assert!(empty_name.validate_and_normalize().is_err());
    }
}
