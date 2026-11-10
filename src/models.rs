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

/// Retourne la liste ordonnée de toutes les connexions selon la hiérarchie logique de l'arborescence :
/// 1. Connexions situées à la racine, classées alphabétiquement (insensible à la casse).
/// 2. Connexions directes des dossiers classés alphabétiquement.
/// 3. Connexions des sous-dossiers parcourus récursivement selon le même ordre.
///
/// Dans chaque dossier :
/// - Les connexions directes apparaissent toujours AVANT les sous-dossiers.
/// - Les sous-dossiers sont ensuite parcourus dans l'ordre alphabétique.
pub fn collect_tree_ordered_connections(
    folders: &[Folder],
    connections: &[ConnectionItem],
) -> Vec<ConnectionItem> {
    use std::collections::HashMap;

    let mut folders_by_parent: HashMap<Option<i64>, Vec<&Folder>> = HashMap::new();
    for f in folders {
        folders_by_parent.entry(f.parent_id).or_default().push(f);
    }
    // Trier les sous-dossiers de chaque parent par ordre alphabétique insensible à la casse
    for list in folders_by_parent.values_mut() {
        list.sort_by_key(|a| a.name.to_lowercase());
    }

    let mut conns_by_folder: HashMap<Option<i64>, Vec<&ConnectionItem>> = HashMap::new();
    for c in connections {
        conns_by_folder.entry(c.folder_id).or_default().push(c);
    }
    // Trier les connexions de chaque dossier par ordre alphabétique insensible à la casse
    for list in conns_by_folder.values_mut() {
        list.sort_by_key(|a| a.name.to_lowercase());
    }

    let mut result = Vec::new();
    let mut visited_folders = Vec::new();

    fn traverse(
        parent_id: Option<i64>,
        folders_by_parent: &HashMap<Option<i64>, Vec<&Folder>>,
        conns_by_folder: &HashMap<Option<i64>, Vec<&ConnectionItem>>,
        visited: &mut Vec<i64>,
        out: &mut Vec<ConnectionItem>,
    ) {
        // 1. D'abord les connexions directes du dossier en cours
        if let Some(conns) = conns_by_folder.get(&parent_id) {
            for c in conns {
                out.push((*c).clone());
            }
        }
        // 2. Ensuite les sous-dossiers récursivement
        if let Some(subfolders) = folders_by_parent.get(&parent_id) {
            for f in subfolders {
                if visited.contains(&f.id) {
                    continue;
                }
                visited.push(f.id);
                traverse(Some(f.id), folders_by_parent, conns_by_folder, visited, out);
            }
        }
    }

    traverse(
        None,
        &folders_by_parent,
        &conns_by_folder,
        &mut visited_folders,
        &mut result,
    );

    result
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

    #[test]
    fn test_collect_tree_ordered_connections_rules() {
        let folders = vec![
            Folder {
                id: 1,
                parent_id: None,
                name: "Clients".to_string(),
                sort_order: 0,
                is_expanded: true,
                created_at: 0,
                updated_at: 0,
            },
            Folder {
                id: 2,
                parent_id: Some(1),
                name: "Client B".to_string(),
                sort_order: 0,
                is_expanded: true,
                created_at: 0,
                updated_at: 0,
            },
            Folder {
                id: 3,
                parent_id: Some(1),
                name: "client a".to_string(), // minuscules pour tester le case-insensitive
                sort_order: 0,
                is_expanded: true,
                created_at: 0,
                updated_at: 0,
            },
            Folder {
                id: 4,
                parent_id: None,
                name: "Administration".to_string(),
                sort_order: 0,
                is_expanded: true,
                created_at: 0,
                updated_at: 0,
            },
        ];

        let connections = vec![
            // Connexion racine
            ConnectionItem {
                id: 10,
                folder_id: None,
                name: "Z-Root-Last".to_string(),
                protocol: Protocol::Rdp,
                host: "10.0.0.1".to_string(),
                port: 3389,
                username: "user".to_string(),
                domain: String::new(),
                description: String::new(),
                is_favorite: false,
                save_password: false,
                has_saved_password: false,
                dynamic_resolution: true,
                redirect_clipboard: true,
                sort_order: 0,
                created_at: 0,
                updated_at: 0,
                last_connected_at: None,
            },
            ConnectionItem {
                id: 11,
                folder_id: None,
                name: "A-Root-First".to_string(),
                protocol: Protocol::Rdp,
                host: "10.0.0.2".to_string(),
                port: 3389,
                username: "user".to_string(),
                domain: String::new(),
                description: String::new(),
                is_favorite: false,
                save_password: false,
                has_saved_password: false,
                dynamic_resolution: true,
                redirect_clipboard: true,
                sort_order: 0,
                created_at: 0,
                updated_at: 0,
                last_connected_at: None,
            },
            // Dans dossier 1 ("Clients") : connexions directes
            ConnectionItem {
                id: 20,
                folder_id: Some(1),
                name: "Serveur RDS".to_string(),
                protocol: Protocol::Rdp,
                host: "10.0.1.1".to_string(),
                port: 3389,
                username: "user".to_string(),
                domain: String::new(),
                description: String::new(),
                is_favorite: false,
                save_password: false,
                has_saved_password: false,
                dynamic_resolution: true,
                redirect_clipboard: true,
                sort_order: 0,
                created_at: 0,
                updated_at: 0,
                last_connected_at: None,
            },
            ConnectionItem {
                id: 21,
                folder_id: Some(1),
                name: "serveur ad".to_string(),
                protocol: Protocol::Rdp,
                host: "10.0.1.2".to_string(),
                port: 3389,
                username: "user".to_string(),
                domain: String::new(),
                description: String::new(),
                is_favorite: false,
                save_password: false,
                has_saved_password: false,
                dynamic_resolution: true,
                redirect_clipboard: true,
                sort_order: 0,
                created_at: 0,
                updated_at: 0,
                last_connected_at: None,
            },
            // Dans dossier 3 ("client a")
            ConnectionItem {
                id: 30,
                folder_id: Some(3),
                name: "Machine Client A".to_string(),
                protocol: Protocol::AnyDesk,
                host: "123456789".to_string(),
                port: 0,
                username: String::new(),
                domain: String::new(),
                description: String::new(),
                is_favorite: false,
                save_password: false,
                has_saved_password: false,
                dynamic_resolution: true,
                redirect_clipboard: true,
                sort_order: 0,
                created_at: 0,
                updated_at: 0,
                last_connected_at: None,
            },
            // Dans dossier 2 ("Client B")
            ConnectionItem {
                id: 40,
                folder_id: Some(2),
                name: "Machine Client B".to_string(),
                protocol: Protocol::RustDesk,
                host: "987654321".to_string(),
                port: 0,
                username: String::new(),
                domain: String::new(),
                description: String::new(),
                is_favorite: false,
                save_password: false,
                has_saved_password: false,
                dynamic_resolution: true,
                redirect_clipboard: true,
                sort_order: 0,
                created_at: 0,
                updated_at: 0,
                last_connected_at: None,
            },
            // Dans dossier 4 ("Administration")
            ConnectionItem {
                id: 50,
                folder_id: Some(4),
                name: "Passerelle Bastion".to_string(),
                protocol: Protocol::Rdp,
                host: "10.0.99.1".to_string(),
                port: 3389,
                username: "admin".to_string(),
                domain: String::new(),
                description: String::new(),
                is_favorite: true,
                save_password: false,
                has_saved_password: false,
                dynamic_resolution: true,
                redirect_clipboard: true,
                sort_order: 0,
                created_at: 0,
                updated_at: 0,
                last_connected_at: None,
            },
        ];

        let ordered = collect_tree_ordered_connections(&folders, &connections);
        let names: Vec<&str> = ordered.iter().map(|c| c.name.as_str()).collect();

        // Ordre attendu :
        // 1. Racine directes (alphabétique) : "A-Root-First", "Z-Root-Last"
        // 2. Dossier racine "Administration" (car "Administration" < "Clients") : "Passerelle Bastion"
        // 3. Dossier racine "Clients" :
        //    a. Connexions directes d'abord (alphabétique) : "serveur ad", "Serveur RDS"
        //    b. Sous-dossier "client a" (car "client a" < "Client B") : "Machine Client A"
        //    c. Sous-dossier "Client B" : "Machine Client B"
        assert_eq!(
            names,
            vec![
                "A-Root-First",
                "Z-Root-Last",
                "Passerelle Bastion",
                "serveur ad",
                "Serveur RDS",
                "Machine Client A",
                "Machine Client B",
            ]
        );
    }
}
