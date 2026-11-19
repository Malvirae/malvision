//! Modèles de données métier de Malvision (Jalon 2) : Protocoles, Dossiers, Connexions,
//! Paramètres RDP et entrées d'historique récent.

use crate::rdp::{parse_host_and_port, split_domain_username};
use crate::security::SecretString;

/// Protocoles de connexion distante pris en charge par Malvision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Protocol {
    Rdp,
    Ftp,
    Ssh,
    AnyDesk,
    RustDesk,
}

impl Protocol {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Rdp => "rdp",
            Self::Ftp => "ftp",
            Self::Ssh => "ssh",
            Self::AnyDesk => "anydesk",
            Self::RustDesk => "rustdesk",
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Self::Rdp => "RDP",
            Self::Ftp => "FTP",
            Self::Ssh => "SSH",
            Self::AnyDesk => "AnyDesk",
            Self::RustDesk => "RustDesk",
        }
    }

    pub fn badge_prefix(self) -> &'static str {
        match self {
            Self::Rdp => "[RDP]",
            Self::Ftp => "[FTP]",
            Self::Ssh => "[SSH]",
            Self::AnyDesk => "[ANY]",
            Self::RustDesk => "[RST]",
        }
    }

    pub fn from_db_str(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "rdp" => Ok(Self::Rdp),
            "ftp" => Ok(Self::Ftp),
            "ssh" => Ok(Self::Ssh),
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
/// Identifiant partagé réutilisable par plusieurs connexions (RDP, FTP, SSH, AnyDesk, RustDesk).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CredentialItem {
    pub id: i64,
    pub name: String,
    pub username: String,
    pub domain: String,
    pub has_saved_password: bool,
    pub description: String,
    pub created_at: i64,
    pub updated_at: i64,
}

impl CredentialItem {
    /// Libellé compact d'affichage (`DOMAINE\utilisateur` ou `utilisateur`).
    pub fn account_label(&self) -> String {
        if !self.domain.is_empty() && !self.username.is_empty() {
            format!("{}\\{}", self.domain, self.username)
        } else if !self.username.is_empty() {
            self.username.clone()
        } else if !self.domain.is_empty() {
            self.domain.clone()
        } else {
            "(sans nom d'utilisateur)".to_string()
        }
    }
}

/// Données saisies pour créer ou modifier un identifiant partagé.
#[derive(Debug, Clone)]
pub struct CredentialDraft {
    pub name: String,
    pub username: String,
    pub domain: String,
    pub description: String,
    pub password: SecretString,
    /// Si true lors d'une mise à jour et `password` vide, conserve le mot de passe chiffré existant.
    pub keep_existing_password: bool,
}

impl CredentialDraft {
    /// Nettoie et valide les champs avant persistance.
    pub fn validate_and_normalize(&mut self) -> Result<(), String> {
        self.name = self.name.trim().to_string();
        if self.name.is_empty() {
            return Err("Le nom de l'identifiant ne peut pas être vide.".to_string());
        }
        self.username = self.username.trim().to_string();
        self.domain = self.domain.trim().to_string();
        self.description = self.description.trim().to_string();

        if self.domain.is_empty() && self.username.contains('\\') {
            let (d, u) = split_domain_username(&self.username);
            self.domain = d;
            self.username = u;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionItem {
    pub id: i64,
    pub folder_id: Option<i64>,
    pub credential_id: Option<i64>,
    pub name: String,
    pub protocol: Protocol,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub domain: String,
    pub description: String,
    pub is_favorite: bool,
    pub is_priority: bool,
    pub save_password: bool,
    pub has_saved_password: bool,
    pub dynamic_resolution: bool,
    pub redirect_clipboard: bool,
    pub sort_order: i32,
    pub created_at: i64,
    pub updated_at: i64,
    pub last_connected_at: Option<i64>,
    pub wol_host: String,
    pub wol_mac: String,
    pub wol_port: u16,
}

impl ConnectionItem {
    /// Indique si le Wake-on-LAN est configuré sur cette connexion (cible + MAC non vides).
    pub fn has_wol_configured(&self) -> bool {
        !self.wol_host.trim().is_empty() && !self.wol_mac.trim().is_empty()
    }
    /// Retourne une étiquette d'adresse lisible (ex. `192.168.1.10:3389`, `ftp.serveur.fr:2121` ou l'ID AnyDesk/RustDesk).
    pub fn endpoint_label(&self) -> String {
        match self.protocol {
            Protocol::Rdp => {
                if self.port == 3389 {
                    self.host.clone()
                } else {
                    format!("{}:{}", self.host, self.port)
                }
            }
            Protocol::Ftp => {
                if self.port == 21 {
                    self.host.clone()
                } else {
                    format!("{}:{}", self.host, self.port)
                }
            }
            Protocol::Ssh => {
                if self.port == 22 {
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
    pub credential_id: Option<i64>,
    pub name: String,
    pub protocol: Protocol,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub domain: String,
    pub description: String,
    pub is_favorite: bool,
    pub is_priority: bool,
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
                Protocol::Ftp => {
                    "L'adresse hôte FTP (IPv4, IPv6 ou DNS) ne peut pas être vide.".to_string()
                }
                Protocol::Ssh => {
                    "L'adresse hôte SSH (IPv4, IPv6 ou DNS) ne peut pas être vide.".to_string()
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
            Protocol::Ftp => {
                if self.port == 0 {
                    return Err("Le port FTP doit être compris entre 1 et 65535.".to_string());
                }
                let (parsed_host, parsed_port, initial_path) = parse_ftp_target(&self.host)?;
                if parsed_host.contains(char::is_whitespace) {
                    return Err("L'adresse d'hôte FTP ne doit pas contenir d'espaces.".to_string());
                }
                self.host = parsed_host;
                if parsed_port != 21 && self.port == 21 {
                    self.port = parsed_port;
                }
                if self.domain.trim().is_empty()
                    && let Some(p) = initial_path
                {
                    self.domain = p;
                }
            }
            Protocol::Ssh => {
                if self.port == 0 {
                    return Err("Le port SSH doit être compris entre 1 et 65535.".to_string());
                }
                let (parsed_user, parsed_host, parsed_port) = parse_ssh_target(&self.host)?;
                if parsed_host.contains(char::is_whitespace) {
                    return Err("L'adresse d'hôte SSH ne doit pas contenir d'espaces.".to_string());
                }
                self.host = parsed_host;
                if parsed_port != 22 && self.port == 22 {
                    self.port = parsed_port;
                }
                if self.username.trim().is_empty()
                    && let Some(u) = parsed_user
                {
                    self.username = u;
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

        // Si un identifiant partagé est lié, on délègue l'authentification à l'identifiant
        // (pour FTP/SSH on conserve `domain` qui stocke le répertoire initial / la clé privée).
        if self.credential_id.is_some() {
            self.username.clear();
            if self.protocol == Protocol::Rdp {
                self.domain.clear();
            }
            self.save_password = false;
            self.password.clear();
            self.keep_existing_password = false;
        } else if self.protocol == Protocol::Rdp
            && self.domain.is_empty()
            && self.username.contains('\\')
        {
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

/// Analyse une cible FTP saisie (`ftp://hôte:port/chemin`, `hôte:port`, `[::1]:2121`, `ftp.exemple.fr`)
/// en extrayant `(hôte, port, chemin_initial_éventuel)` avec port `21` par défaut.
pub fn parse_ftp_target(input: &str) -> Result<(String, u16, Option<String>), String> {
    let mut s = input.trim();
    if s.len() >= 6 && s[..6].eq_ignore_ascii_case("ftp://") {
        s = &s[6..];
    }
    let (host_port, initial_path) = if let Some((hp, path)) = s.split_once('/') {
        let p = format!("/{}", path.trim_start_matches('/'));
        (hp.trim(), if p == "/" { None } else { Some(p) })
    } else {
        (s, None)
    };

    if host_port.is_empty() {
        return Err("L'adresse hôte FTP (IPv4, IPv6 ou DNS) ne peut pas être vide.".to_string());
    }

    if let Some(rest) = host_port.strip_prefix('[') {
        let Some((ipv6, after)) = rest.split_once(']') else {
            return Err("Crochet fermant ']' manquant dans l'adresse IPv6 FTP.".to_string());
        };
        if ipv6.is_empty() {
            return Err("Adresse IPv6 FTP invalide.".to_string());
        }
        if after.is_empty() {
            return Ok((ipv6.to_string(), 21, initial_path));
        }
        if let Some(port_str) = after.strip_prefix(':') {
            let port = port_str
                .parse::<u16>()
                .map_err(|_| format!("Port FTP invalide : '{port_str}'."))?;
            if port == 0 {
                return Err("Le port FTP doit être compris entre 1 et 65535.".to_string());
            }
            return Ok((ipv6.to_string(), port, initial_path));
        }
        return Err("Format d'adresse IPv6 FTP invalide.".to_string());
    }

    if host_port.matches(':').count() == 1
        && let Some((h, port_str)) = host_port.rsplit_once(':')
    {
        if h.trim().is_empty() {
            return Err("L'adresse hôte FTP ne peut pas être vide.".to_string());
        }
        let port = port_str
            .trim()
            .parse::<u16>()
            .map_err(|_| format!("Port FTP invalide : '{port_str}'."))?;
        if port == 0 {
            return Err("Le port FTP doit être compris entre 1 et 65535.".to_string());
        }
        return Ok((h.trim().to_string(), port, initial_path));
    }

    Ok((host_port.to_string(), 21, initial_path))
}

/// Analyse une cible SSH saisie (`ssh://utilisateur@hôte:port`, `utilisateur@hôte:port`, `[::1]:2222`, `srv.exemple.fr`)
/// en extrayant `(utilisateur_éventuel, hôte, port)` avec port `22` par défaut.
pub fn parse_ssh_target(input: &str) -> Result<(Option<String>, String, u16), String> {
    let mut s = input.trim();
    if s.len() >= 6 && s[..6].eq_ignore_ascii_case("ssh://") {
        s = &s[6..];
    }
    s = s.trim_end_matches('/');

    let (parsed_user, host_port) = if let Some((u, hp)) = s.split_once('@') {
        let cu = u.trim();
        (
            if cu.is_empty() {
                None
            } else {
                Some(cu.to_string())
            },
            hp.trim(),
        )
    } else {
        (None, s)
    };

    if host_port.is_empty() {
        return Err("L'adresse hôte SSH (IPv4, IPv6 ou DNS) ne peut pas être vide.".to_string());
    }

    if let Some(rest) = host_port.strip_prefix('[') {
        let Some((ipv6, after)) = rest.split_once(']') else {
            return Err("Crochet fermant ']' manquant dans l'adresse IPv6 SSH.".to_string());
        };
        if ipv6.is_empty() {
            return Err("Adresse IPv6 SSH invalide.".to_string());
        }
        if after.is_empty() {
            return Ok((parsed_user, ipv6.to_string(), 22));
        }
        if let Some(port_str) = after.strip_prefix(':') {
            let port = port_str
                .parse::<u16>()
                .map_err(|_| format!("Port SSH invalide : '{port_str}'."))?;
            if port == 0 {
                return Err("Le port SSH doit être compris entre 1 et 65535.".to_string());
            }
            return Ok((parsed_user, ipv6.to_string(), port));
        }
        return Err("Format d'adresse IPv6 SSH invalide.".to_string());
    }

    if host_port.matches(':').count() == 1
        && let Some((h, port_str)) = host_port.rsplit_once(':')
    {
        if h.trim().is_empty() {
            return Err("L'adresse hôte SSH ne peut pas être vide.".to_string());
        }
        let port = port_str
            .trim()
            .parse::<u16>()
            .map_err(|_| format!("Port SSH invalide : '{port_str}'."))?;
        if port == 0 {
            return Err("Le port SSH doit être compris entre 1 et 65535.".to_string());
        }
        return Ok((parsed_user, h.trim().to_string(), port));
    }

    Ok((parsed_user, host_port.to_string(), 22))
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
    // Trier les connexions de chaque dossier : prioritaires d'abord (alphabétique), puis normales (alphabétique)
    for list in conns_by_folder.values_mut() {
        list.sort_by(|a, b| {
            b.is_priority
                .cmp(&a.is_priority)
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });
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

/// Parse et valide une adresse MAC (`AA:BB:CC:DD:EE:FF`, `AA-BB-CC-DD-EE-FF`, `AABB.CCDD.EEFF` ou `AABBCCDDEEFF`).
pub fn normalize_mac_address(raw: &str) -> Result<[u8; 6], String> {
    let hex_only: String = raw
        .chars()
        .filter(|c| !matches!(c, ':' | '-' | '.' | ' ' | '\t'))
        .collect();
    if hex_only.len() != 12 || !hex_only.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(
            "Adresse MAC invalide. Format attendu : AA:BB:CC:DD:EE:FF (ou AA-BB-CC-DD-EE-FF)."
                .to_string(),
        );
    }
    let mut mac = [0u8; 6];
    for (i, slot) in mac.iter_mut().enumerate() {
        let pair = &hex_only[i * 2..i * 2 + 2];
        *slot = u8::from_str_radix(pair, 16)
            .map_err(|_| "Adresse MAC invalide : octets hexadécimaux incorrects.".to_string())?;
    }
    Ok(mac)
}

pub fn format_mac_address(mac: [u8; 6]) -> String {
    format!(
        "{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
        mac[0], mac[1], mac[2], mac[3], mac[4], mac[5]
    )
}

/// Construit un Magic Packet Wake-on-LAN standard de 102 octets (`6 x 0xFF` + `16 x MAC`).
pub fn build_magic_packet(mac: [u8; 6]) -> [u8; 102] {
    let mut packet = [0xFFu8; 102];
    for i in 0..16 {
        let offset = 6 + i * 6;
        packet[offset..offset + 6].copy_from_slice(&mac);
    }
    packet
}

/// Résout une cible IP ou FQDN et envoie le Magic Packet Wake-on-LAN en UDP sur le port donné.
pub fn send_wake_on_lan(host: &str, mac_str: &str, port: u16) -> Result<String, String> {
    let clean_host = host.trim();
    if clean_host.is_empty() {
        return Err("L'adresse IP ou le FQDN Wake-on-LAN est obligatoire.".to_string());
    }
    if port == 0 {
        return Err("Le port UDP Wake-on-LAN doit être compris entre 1 et 65535.".to_string());
    }
    let mac = normalize_mac_address(mac_str)?;
    let packet = build_magic_packet(mac);

    let addrs: Vec<std::net::SocketAddr> =
        std::net::ToSocketAddrs::to_socket_addrs(&(clean_host, port))
            .map_err(|e| format!("Impossible de résoudre « {clean_host} » : {e}"))?
            .collect();
    if addrs.is_empty() {
        return Err(format!(
            "Aucune adresse réseau trouvée pour « {clean_host} »."
        ));
    }

    let mut sent_to: Vec<String> = Vec::new();
    let mut last_err: Option<String> = None;
    for target in addrs {
        let bind_addr = if target.is_ipv4() {
            "0.0.0.0:0"
        } else {
            "[::]:0"
        };
        match std::net::UdpSocket::bind(bind_addr) {
            Ok(sock) => {
                let _ = sock.set_broadcast(true);
                match sock.send_to(&packet, target) {
                    Ok(_) => sent_to.push(target.to_string()),
                    Err(e) => last_err = Some(format!("Échec envoi UDP vers {target} : {e}")),
                }
            }
            Err(e) => last_err = Some(format!("Impossible d'ouvrir le socket UDP : {e}")),
        }
    }

    if sent_to.is_empty() {
        return Err(
            last_err.unwrap_or_else(|| "Échec de l'envoi du paquet Wake-on-LAN.".to_string())
        );
    }

    Ok(format!(
        "Paquet Wake-on-LAN envoyé à {} ({}) pour la MAC {}.",
        clean_host,
        sent_to.join(", "),
        format_mac_address(mac)
    ))
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
            is_priority: false,
            save_password: false,
            password: SecretString::new("MonMotDePasse".to_string()),
            keep_existing_password: false,
            credential_id: None,
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
            is_priority: false,
            save_password: false,
            password: SecretString::new(String::new()),
            keep_existing_password: false,
            credential_id: None,
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
                is_priority: false,
                save_password: false,
                has_saved_password: false,
                dynamic_resolution: true,
                redirect_clipboard: true,
                sort_order: 0,
                created_at: 0,
                updated_at: 0,
                last_connected_at: None,
                credential_id: None,
                wol_host: String::new(),
                wol_mac: String::new(),
                wol_port: 9,
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
                is_priority: false,
                save_password: false,
                has_saved_password: false,
                dynamic_resolution: true,
                redirect_clipboard: true,
                sort_order: 0,
                created_at: 0,
                updated_at: 0,
                last_connected_at: None,
                credential_id: None,
                wol_host: String::new(),
                wol_mac: String::new(),
                wol_port: 9,
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
                is_priority: false,
                save_password: false,
                has_saved_password: false,
                dynamic_resolution: true,
                redirect_clipboard: true,
                sort_order: 0,
                created_at: 0,
                updated_at: 0,
                last_connected_at: None,
                credential_id: None,
                wol_host: String::new(),
                wol_mac: String::new(),
                wol_port: 9,
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
                is_priority: false,
                save_password: false,
                has_saved_password: false,
                dynamic_resolution: true,
                redirect_clipboard: true,
                sort_order: 0,
                created_at: 0,
                updated_at: 0,
                last_connected_at: None,
                credential_id: None,
                wol_host: String::new(),
                wol_mac: String::new(),
                wol_port: 9,
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
                is_priority: false,
                save_password: false,
                has_saved_password: false,
                dynamic_resolution: true,
                redirect_clipboard: true,
                sort_order: 0,
                created_at: 0,
                updated_at: 0,
                last_connected_at: None,
                credential_id: None,
                wol_host: String::new(),
                wol_mac: String::new(),
                wol_port: 9,
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
                is_priority: false,
                save_password: false,
                has_saved_password: false,
                dynamic_resolution: true,
                redirect_clipboard: true,
                sort_order: 0,
                created_at: 0,
                updated_at: 0,
                last_connected_at: None,
                credential_id: None,
                wol_host: String::new(),
                wol_mac: String::new(),
                wol_port: 9,
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
                is_priority: false,
                save_password: false,
                has_saved_password: false,
                dynamic_resolution: true,
                redirect_clipboard: true,
                sort_order: 0,
                created_at: 0,
                updated_at: 0,
                last_connected_at: None,
                credential_id: None,
                wol_host: String::new(),
                wol_mac: String::new(),
                wol_port: 9,
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

    #[test]
    fn test_priority_connections_sorting_and_rules() {
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
                name: "Infrastructure".to_string(),
                sort_order: 0,
                is_expanded: false,
                created_at: 0,
                updated_at: 0,
            },
        ];

        let make_conn = |id, fid, name: &str, is_fav, is_prio| ConnectionItem {
            id,
            folder_id: fid,
            name: name.to_string(),
            protocol: Protocol::Rdp,
            host: "10.0.0.1".to_string(),
            port: 3389,
            username: "admin".to_string(),
            domain: String::new(),
            description: String::new(),
            is_favorite: is_fav,
            is_priority: is_prio,
            save_password: false,
            has_saved_password: false,
            dynamic_resolution: true,
            redirect_clipboard: true,
            sort_order: 0,
            created_at: 0,
            updated_at: 0,
            last_connected_at: None,
            credential_id: None,
            wol_host: String::new(),
            wol_mac: String::new(),
            wol_port: 9,
        };

        let connections = vec![
            // Dans dossier 1 ("Clients") :
            // Normales
            make_conn(1, Some(1), "Serveur sauvegarde", false, false),
            make_conn(2, Some(1), "Serveur fichiers", true, false), // Favori mais non prioritaire
            // Prioritaires
            make_conn(3, Some(1), "Serveur RDS", false, true),
            make_conn(4, Some(1), "Serveur AD", true, true), // Favori ET prioritaire
            // Dans sous-dossier 2 ("Infrastructure")
            make_conn(5, Some(2), "Switch Core", false, false),
            make_conn(6, Some(2), "Routeur BGP", false, true), // Prioritaire dans son sous-dossier
            // Racine
            make_conn(7, None, "Z-Normale", false, false),
            make_conn(8, None, "A-Normale", false, false),
            make_conn(9, None, "M-Prioritaire", false, true),
        ];

        let ordered = collect_tree_ordered_connections(&folders, &connections);
        let names: Vec<&str> = ordered.iter().map(|c| c.name.as_str()).collect();

        // Ordre attendu :
        // 1. Racine :
        //    a. Prioritaires (alphabétique) : "M-Prioritaire"
        //    b. Normales (alphabétique) : "A-Normale", "Z-Normale"
        // 2. Dossier "Clients" :
        //    a. Connexions prioritaires directes (alphabétique) : "Serveur AD", "Serveur RDS"
        //    b. Connexions normales directes (alphabétique) : "Serveur fichiers", "Serveur sauvegarde"
        //    c. Sous-dossier "Infrastructure" :
        //       - Prioritaire : "Routeur BGP"
        //       - Normale : "Switch Core"
        assert_eq!(
            names,
            vec![
                "M-Prioritaire",
                "A-Normale",
                "Z-Normale",
                "Serveur AD",
                "Serveur RDS",
                "Serveur fichiers",
                "Serveur sauvegarde",
                "Routeur BGP",
                "Switch Core",
            ]
        );
    }

    #[test]
    fn test_validate_connection_draft_ftp() {
        let mut draft = ConnectionDraft {
            folder_id: None,
            name: "Serveur Web Prod".to_string(),
            protocol: Protocol::Ftp,
            host: "ftp://files.example.com:2121/public_html".to_string(),
            port: 21,
            username: "deploy".to_string(),
            domain: String::new(),
            description: "Déploiement web".to_string(),
            is_favorite: true,
            is_priority: false,
            save_password: true,
            password: SecretString::new("S3cretFTP!".to_string()),
            keep_existing_password: false,
            credential_id: None,
            dynamic_resolution: false,
            redirect_clipboard: false,
        };
        draft.validate_and_normalize().expect("Validation FTP OK");
        assert_eq!(draft.host, "files.example.com");
        assert_eq!(draft.port, 2121);
        assert_eq!(draft.domain, "/public_html");
        assert_eq!(draft.protocol.as_str(), "ftp");
        assert_eq!(draft.protocol.badge_prefix(), "[FTP]");
    }

    #[test]
    fn test_validate_connection_draft_ssh() {
        let mut draft = ConnectionDraft {
            folder_id: None,
            name: "Serveur Debian Prod".to_string(),
            protocol: Protocol::Ssh,
            host: "ssh://admin@ssh.example.com:2222".to_string(),
            port: 22,
            username: String::new(),
            domain: "~/.ssh/id_ed25519".to_string(),
            description: "Terminal SSH".to_string(),
            is_favorite: false,
            is_priority: false,
            save_password: true,
            password: SecretString::new("S3cretSSH!".to_string()),
            keep_existing_password: false,
            credential_id: None,
            dynamic_resolution: false,
            redirect_clipboard: false,
        };
        draft.validate_and_normalize().expect("Validation SSH OK");
        assert_eq!(draft.host, "ssh.example.com");
        assert_eq!(draft.port, 2222);
        assert_eq!(draft.username, "admin");
        assert_eq!(draft.domain, "~/.ssh/id_ed25519");
        assert_eq!(draft.protocol.as_str(), "ssh");
        assert_eq!(draft.protocol.badge_prefix(), "[SSH]");
    }

    #[test]
    fn test_validate_credential_draft_splits_domain_and_trims() {
        let mut draft = CredentialDraft {
            name: "  Admin Domaine  ".to_string(),
            username: "  CORP\\jdupont  ".to_string(),
            domain: String::new(),
            description: "  Compte partagé infra  ".to_string(),
            password: SecretString::new("S3cretPass!".to_string()),
            keep_existing_password: false,
        };
        draft
            .validate_and_normalize()
            .expect("Validation identifiant OK");
        assert_eq!(draft.name, "Admin Domaine");
        assert_eq!(draft.domain, "CORP");
        assert_eq!(draft.username, "jdupont");
        assert_eq!(draft.description, "Compte partagé infra");
        assert_eq!(draft.password.as_str(), "S3cretPass!");
    }

    #[test]
    fn test_reject_empty_credential_name() {
        let mut draft = CredentialDraft {
            name: "   ".to_string(),
            username: "admin".to_string(),
            domain: String::new(),
            description: String::new(),
            password: SecretString::new(String::new()),
            keep_existing_password: false,
        };
        assert!(draft.validate_and_normalize().is_err());
    }

    #[test]
    fn test_wol_mac_normalization_magic_packet_and_udp_send() {
        let mac = normalize_mac_address("aa-bb-cc-dd-ee-ff").expect("MAC valide");
        assert_eq!(mac, [0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF]);
        assert_eq!(format_mac_address(mac), "AA:BB:CC:DD:EE:FF");
        assert_eq!(
            normalize_mac_address("0011.2233.4455").unwrap(),
            [0x00, 0x11, 0x22, 0x33, 0x44, 0x55]
        );
        assert!(normalize_mac_address("ZZ:BB:CC:DD:EE:FF").is_err());
        assert!(normalize_mac_address("AA:BB:CC").is_err());

        let pkt = build_magic_packet(mac);
        assert_eq!(pkt.len(), 102);
        assert_eq!(&pkt[0..6], &[0xFF; 6]);
        for i in 0..16 {
            assert_eq!(&pkt[6 + i * 6..12 + i * 6], &mac);
        }

        let sock = std::net::UdpSocket::bind("127.0.0.1:0").expect("Bind UDP test");
        sock.set_read_timeout(Some(std::time::Duration::from_secs(2)))
            .unwrap();
        let port = sock.local_addr().unwrap().port();
        let msg = send_wake_on_lan("127.0.0.1", "AA:BB:CC:DD:EE:FF", port).expect("Envoi WoL");
        assert!(msg.contains("AA:BB:CC:DD:EE:FF"));

        let mut buf = [0u8; 128];
        let (n, _) = sock.recv_from(&mut buf).expect("Réception Magic Packet");
        assert_eq!(n, 102);
        assert_eq!(&buf[..102], &pkt);
    }
}
