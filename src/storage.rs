//! Persistance locale SQLite de Malvision (`%LOCALAPPDATA%\Malvision\malvision.db`).
//!
//! Fonctionnalités prises en charge :
//! - Migrations de schéma versionnées (`PRAGMA user_version`).
//! - Contraintes d'intégrité référentielle (`PRAGMA foreign_keys = ON`).
//! - Transactions atomiques et mode WAL.
//! - CRUD complet des dossiers (`folders`) avec prévention stricte des cycles parents.
//! - CRUD complet des connexions (`connections`) avec chiffrement DPAPI des mots de passe.
//! - Recherche filtrée, gestion des favoris, duplication, déplacement, historique (`recent_connections`)
//!   et préférences (`settings`).

use crate::models::{ConnectionDraft, ConnectionItem, Folder, Protocol, RecentConnectionEntry};
use crate::security::{SecretString, decrypt_secret, encrypt_secret};
use rusqlite::{Connection, OptionalExtension, params};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const CURRENT_SCHEMA_VERSION: i32 = 1;

pub struct Storage {
    conn: Connection,
    db_path: PathBuf,
}

impl Storage {
    /// Ouvre ou crée la base de données dans `%LOCALAPPDATA%\Malvision\malvision.db`.
    pub fn open_default() -> Result<Self, String> {
        let dir = default_app_data_dir()?;
        std::fs::create_dir_all(&dir).map_err(|e| {
            format!(
                "Impossible de créer le répertoire de données '{}' : {e}",
                dir.display()
            )
        })?;
        let db_path = dir.join("malvision.db");
        Self::open_at_path(&db_path)
    }

    /// Ouvre ou crée la base de données à l'emplacement spécifié.
    pub fn open_at_path(path: &Path) -> Result<Self, String> {
        let conn = Connection::open(path).map_err(|e| {
            format!(
                "Impossible d'ouvrir la base SQLite '{}' : {e}",
                path.display()
            )
        })?;

        let mut storage = Self {
            conn,
            db_path: path.to_path_buf(),
        };
        storage.initialize_and_migrate()?;
        Ok(storage)
    }

    /// Crée une base SQLite en mémoire (utile pour les tests unitaires et d'intégration rapides).
    #[cfg(test)]
    pub fn open_in_memory() -> Result<Self, String> {
        let conn =
            Connection::open_in_memory().map_err(|e| format!("SQLite in-memory error : {e}"))?;
        let mut storage = Self {
            conn,
            db_path: PathBuf::from(":memory:"),
        };
        storage.initialize_and_migrate()?;
        Ok(storage)
    }

    fn initialize_and_migrate(&mut self) -> Result<(), String> {
        // Activer les clés étrangères et vérifier que le fichier est bien une base SQLite saine
        self.conn
            .execute_batch(
                "PRAGMA foreign_keys = ON;
                 PRAGMA busy_timeout = 5000;",
            )
            .map_err(|e| {
                format!(
                    "Le fichier de base de données '{}' est invalide ou corrompu : {e}",
                    self.db_path.display()
                )
            })?;

        let _ = self.conn.pragma_update(None, "journal_mode", "WAL");

        // Vérification d'intégrité rapide
        let integrity: String = self
            .conn
            .query_row("PRAGMA quick_check;", [], |row| row.get(0))
            .map_err(|e| {
                format!(
                    "Échec du contrôle d'intégrité SQLite sur '{}' : {e}",
                    self.db_path.display()
                )
            })?;
        if !integrity.eq_ignore_ascii_case("ok") {
            return Err(format!(
                "La base de données SQLite '{}' présente une corruption ({integrity}). Aucune donnée n'a été écrasée.",
                self.db_path.display()
            ));
        }

        let version: i32 = self
            .conn
            .query_row("PRAGMA user_version;", [], |row| row.get(0))
            .map_err(|e| format!("Impossible de lire user_version : {e}"))?;

        if version < 1 {
            let tx = self
                .conn
                .transaction()
                .map_err(|e| format!("Échec d'ouverture de transaction de migration : {e}"))?;

            tx.execute_batch(
                "CREATE TABLE IF NOT EXISTS folders (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    parent_id INTEGER NULL REFERENCES folders(id) ON DELETE CASCADE,
                    name TEXT NOT NULL CHECK(length(trim(name)) > 0),
                    sort_order INTEGER NOT NULL DEFAULT 0,
                    is_expanded INTEGER NOT NULL DEFAULT 1,
                    created_at INTEGER NOT NULL,
                    updated_at INTEGER NOT NULL
                );

                CREATE INDEX IF NOT EXISTS idx_folders_parent ON folders(parent_id, sort_order, name);

                CREATE TABLE IF NOT EXISTS connections (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    folder_id INTEGER NULL REFERENCES folders(id) ON DELETE SET NULL,
                    name TEXT NOT NULL CHECK(length(trim(name)) > 0),
                    protocol TEXT NOT NULL CHECK(protocol IN ('rdp', 'anydesk', 'rustdesk')),
                    host TEXT NOT NULL CHECK(length(trim(host)) > 0),
                    port INTEGER NOT NULL DEFAULT 3389 CHECK(port BETWEEN 1 AND 65535),
                    username TEXT NOT NULL DEFAULT '',
                    domain TEXT NOT NULL DEFAULT '',
                    description TEXT NOT NULL DEFAULT '',
                    is_favorite INTEGER NOT NULL DEFAULT 0,
                    save_password INTEGER NOT NULL DEFAULT 0,
                    encrypted_password BLOB NULL,
                    dynamic_resolution INTEGER NOT NULL DEFAULT 1,
                    redirect_clipboard INTEGER NOT NULL DEFAULT 1,
                    sort_order INTEGER NOT NULL DEFAULT 0,
                    created_at INTEGER NOT NULL,
                    updated_at INTEGER NOT NULL,
                    last_connected_at INTEGER NULL
                );

                CREATE INDEX IF NOT EXISTS idx_connections_folder ON connections(folder_id, sort_order, name);
                CREATE INDEX IF NOT EXISTS idx_connections_favorite ON connections(is_favorite, name);

                CREATE TABLE IF NOT EXISTS recent_connections (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    connection_id INTEGER NOT NULL REFERENCES connections(id) ON DELETE CASCADE,
                    connected_at INTEGER NOT NULL
                );

                CREATE INDEX IF NOT EXISTS idx_recent_connected_at ON recent_connections(connected_at DESC);

                CREATE TABLE IF NOT EXISTS settings (
                    key TEXT PRIMARY KEY NOT NULL,
                    value TEXT NOT NULL,
                    updated_at INTEGER NOT NULL
                );

                PRAGMA user_version = 1;",
            )
            .map_err(|e| format!("Échec de la migration SQLite v1 : {e}"))?;

            tx.commit()
                .map_err(|e| format!("Échec de validation de la migration SQLite v1 : {e}"))?;
        }

        if version > CURRENT_SCHEMA_VERSION {
            return Err(format!(
                "La base de données (schéma v{version}) est plus récente que cette version de Malvision (schéma v{CURRENT_SCHEMA_VERSION})."
            ));
        }

        Ok(())
    }

    #[cfg(test)]
    pub fn schema_version(&self) -> Result<i32, String> {
        self.conn
            .query_row("PRAGMA user_version;", [], |row| row.get(0))
            .map_err(|e| format!("Erreur lecture user_version : {e}"))
    }

    // =========================================================================
    // DOSSIERS (CRUD, Arborescence, Déplacement, Prévention des cycles)
    // =========================================================================

    pub fn create_folder(&mut self, name: &str, parent_id: Option<i64>) -> Result<Folder, String> {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return Err("Le nom du dossier ne peut pas être vide.".to_string());
        }
        if trimmed.len() > 120 {
            return Err("Le nom du dossier ne doit pas dépasser 120 caractères.".to_string());
        }

        if let Some(pid) = parent_id
            && !self.folder_exists(pid)?
        {
            return Err(format!("Le dossier parent #{pid} n'existe pas."));
        }

        let now = unix_timestamp();
        let next_order: i32 = self
            .conn
            .query_row(
                "SELECT COALESCE(MAX(sort_order), -1) + 1 FROM folders WHERE parent_id IS ?1",
                params![parent_id],
                |row| row.get(0),
            )
            .unwrap_or(0);

        self.conn
            .execute(
                "INSERT INTO folders (parent_id, name, sort_order, is_expanded, created_at, updated_at)
                 VALUES (?1, ?2, ?3, 1, ?4, ?4)",
                params![parent_id, trimmed, next_order, now],
            )
            .map_err(|e| format!("Erreur lors de la création du dossier : {e}"))?;

        let id = self.conn.last_insert_rowid();
        self.get_folder(id)?
            .ok_or_else(|| "Dossier créé introuvable.".to_string())
    }

    pub fn get_folder(&self, folder_id: i64) -> Result<Option<Folder>, String> {
        self.conn
            .query_row(
                "SELECT id, parent_id, name, sort_order, is_expanded, created_at, updated_at
                 FROM folders WHERE id = ?1",
                params![folder_id],
                |row| {
                    Ok(Folder {
                        id: row.get(0)?,
                        parent_id: row.get(1)?,
                        name: row.get(2)?,
                        sort_order: row.get(3)?,
                        is_expanded: row.get::<_, i32>(4)? != 0,
                        created_at: row.get(5)?,
                        updated_at: row.get(6)?,
                    })
                },
            )
            .optional()
            .map_err(|e| format!("Erreur lors de la lecture du dossier #{folder_id} : {e}"))
    }

    pub fn list_folders(&self) -> Result<Vec<Folder>, String> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, parent_id, name, sort_order, is_expanded, created_at, updated_at
                 FROM folders
                 ORDER BY sort_order ASC, name COLLATE NOCASE ASC, id ASC",
            )
            .map_err(|e| format!("Erreur préparation requête list_folders : {e}"))?;

        let rows = stmt
            .query_map([], |row| {
                Ok(Folder {
                    id: row.get(0)?,
                    parent_id: row.get(1)?,
                    name: row.get(2)?,
                    sort_order: row.get(3)?,
                    is_expanded: row.get::<_, i32>(4)? != 0,
                    created_at: row.get(5)?,
                    updated_at: row.get(6)?,
                })
            })
            .map_err(|e| format!("Erreur exécution list_folders : {e}"))?;

        let mut out = Vec::new();
        for r in rows {
            out.push(r.map_err(|e| format!("Erreur décodage dossier : {e}"))?);
        }
        Ok(out)
    }

    pub fn rename_folder(&mut self, folder_id: i64, new_name: &str) -> Result<(), String> {
        let trimmed = new_name.trim();
        if trimmed.is_empty() {
            return Err("Le nom du dossier ne peut pas être vide.".to_string());
        }
        if trimmed.len() > 120 {
            return Err("Le nom du dossier ne doit pas dépasser 120 caractères.".to_string());
        }
        let now = unix_timestamp();
        let affected = self
            .conn
            .execute(
                "UPDATE folders SET name = ?1, updated_at = ?2 WHERE id = ?3",
                params![trimmed, now, folder_id],
            )
            .map_err(|e| format!("Erreur lors du renommage du dossier : {e}"))?;
        if affected == 0 {
            return Err(format!("Le dossier #{folder_id} est introuvable."));
        }
        Ok(())
    }

    pub fn set_folder_expanded(&mut self, folder_id: i64, expanded: bool) -> Result<(), String> {
        let val = i32::from(expanded);
        self.conn
            .execute(
                "UPDATE folders SET is_expanded = ?1 WHERE id = ?2",
                params![val, folder_id],
            )
            .map_err(|e| format!("Erreur mise à jour état dossier : {e}"))?;
        Ok(())
    }

    /// Vérifie si `candidate_parent_id` créerait un cycle si `folder_id` était déplacé dedans.
    pub fn would_create_folder_cycle(
        &self,
        folder_id: i64,
        candidate_parent_id: Option<i64>,
    ) -> Result<bool, String> {
        let Some(mut current_id) = candidate_parent_id else {
            return Ok(false);
        };
        if current_id == folder_id {
            return Ok(true);
        }

        let mut visited = Vec::with_capacity(16);
        loop {
            if current_id == folder_id {
                return Ok(true);
            }
            if visited.contains(&current_id) {
                // Protection défensive au cas où la base contiendrait déjà une boucle
                return Ok(true);
            }
            visited.push(current_id);

            let parent_opt: Option<Option<i64>> = self
                .conn
                .query_row(
                    "SELECT parent_id FROM folders WHERE id = ?1",
                    params![current_id],
                    |row| row.get(0),
                )
                .optional()
                .map_err(|e| format!("Erreur vérification cycle dossier : {e}"))?;

            match parent_opt {
                Some(Some(next_parent)) => current_id = next_parent,
                Some(None) => return Ok(false),
                None => return Err(format!("Dossier parent #{current_id} introuvable.")),
            }
        }
    }

    /// Déplace un dossier vers un nouveau dossier parent (ou à la racine si `None`),
    /// en empêchant strictement tout cycle dans l'arborescence.
    pub fn move_folder(
        &mut self,
        folder_id: i64,
        new_parent_id: Option<i64>,
    ) -> Result<(), String> {
        if !self.folder_exists(folder_id)? {
            return Err(format!("Le dossier #{folder_id} n'existe pas."));
        }
        if let Some(pid) = new_parent_id
            && !self.folder_exists(pid)?
        {
            return Err(format!("Le dossier cible #{pid} n'existe pas."));
        }
        if self.would_create_folder_cycle(folder_id, new_parent_id)? {
            return Err(
                "Déplacement impossible : un dossier ne peut pas être déplacé dans lui-même ou dans l'un de ses sous-dossiers."
                    .to_string(),
            );
        }

        let now = unix_timestamp();
        self.conn
            .execute(
                "UPDATE folders SET parent_id = ?1, updated_at = ?2 WHERE id = ?3",
                params![new_parent_id, now, folder_id],
            )
            .map_err(|e| format!("Erreur lors du déplacement du dossier : {e}"))?;
        Ok(())
    }

    /// Supprime un dossier et tous ses sous-dossiers (les connexions contenues peuvent soit être
    /// supprimées en cascade si `delete_connections` est vrai, soit remontées au dossier parent).
    pub fn delete_folder(
        &mut self,
        folder_id: i64,
        delete_connections: bool,
    ) -> Result<(), String> {
        let folder = self
            .get_folder(folder_id)?
            .ok_or_else(|| format!("Le dossier #{folder_id} n'existe pas."))?;

        let tx = self
            .conn
            .transaction()
            .map_err(|e| format!("Transaction suppression dossier : {e}"))?;

        if delete_connections {
            // Supprimer les connexions présentes dans ce dossier et tous ses sous-dossiers descendants
            tx.execute(
                "WITH RECURSIVE subtree(id) AS (
                    SELECT ?1
                    UNION ALL
                    SELECT f.id FROM folders f JOIN subtree s ON f.parent_id = s.id
                 )
                 DELETE FROM connections WHERE folder_id IN (SELECT id FROM subtree)",
                params![folder_id],
            )
            .map_err(|e| format!("Erreur suppression des connexions du dossier : {e}"))?;
        } else {
            // Rattacher les connexions du sous-arbre au parent du dossier supprimé
            tx.execute(
                "WITH RECURSIVE subtree(id) AS (
                    SELECT ?1
                    UNION ALL
                    SELECT f.id FROM folders f JOIN subtree s ON f.parent_id = s.id
                 )
                 UPDATE connections SET folder_id = ?2 WHERE folder_id IN (SELECT id FROM subtree)",
                params![folder_id, folder.parent_id],
            )
            .map_err(|e| format!("Erreur déplacement des connexions du dossier : {e}"))?;
        }

        tx.execute("DELETE FROM folders WHERE id = ?1", params![folder_id])
            .map_err(|e| format!("Erreur suppression du dossier : {e}"))?;

        tx.commit()
            .map_err(|e| format!("Validation suppression dossier : {e}"))?;
        Ok(())
    }

    fn folder_exists(&self, folder_id: i64) -> Result<bool, String> {
        let count: i64 = self
            .conn
            .query_row(
                "SELECT COUNT(1) FROM folders WHERE id = ?1",
                params![folder_id],
                |row| row.get(0),
            )
            .map_err(|e| format!("Erreur vérification existence dossier : {e}"))?;
        Ok(count > 0)
    }

    // =========================================================================
    // CONNEXIONS (CRUD, DPAPI, Duplication, Déplacement, Recherche, Favoris)
    // =========================================================================

    pub fn create_connection(
        &mut self,
        mut draft: ConnectionDraft,
    ) -> Result<ConnectionItem, String> {
        draft.validate_and_normalize()?;

        if let Some(fid) = draft.folder_id
            && !self.folder_exists(fid)?
        {
            return Err(format!("Le dossier parent #{fid} n'existe pas."));
        }

        let encrypted_blob: Option<Vec<u8>> = if draft.save_password && !draft.password.is_empty() {
            Some(encrypt_secret(draft.password.as_str())?)
        } else {
            None
        };
        draft.password.clear();

        let save_pwd_flag = i32::from(draft.save_password && encrypted_blob.is_some());
        let now = unix_timestamp();
        let next_order: i32 = self
            .conn
            .query_row(
                "SELECT COALESCE(MAX(sort_order), -1) + 1 FROM connections WHERE folder_id IS ?1",
                params![draft.folder_id],
                |row| row.get(0),
            )
            .unwrap_or(0);

        self.conn
            .execute(
                "INSERT INTO connections (
                    folder_id, name, protocol, host, port, username, domain, description,
                    is_favorite, save_password, encrypted_password, dynamic_resolution,
                    redirect_clipboard, sort_order, created_at, updated_at
                 ) VALUES (
                    ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8,
                    ?9, ?10, ?11, ?12,
                    ?13, ?14, ?15, ?15
                 )",
                params![
                    draft.folder_id,
                    draft.name,
                    draft.protocol.as_str(),
                    draft.host,
                    i32::from(draft.port),
                    draft.username,
                    draft.domain,
                    draft.description,
                    i32::from(draft.is_favorite),
                    save_pwd_flag,
                    encrypted_blob,
                    i32::from(draft.dynamic_resolution),
                    i32::from(draft.redirect_clipboard),
                    next_order,
                    now,
                ],
            )
            .map_err(|e| format!("Erreur lors de la création de la connexion : {e}"))?;

        let id = self.conn.last_insert_rowid();
        self.get_connection(id)?
            .ok_or_else(|| "Connexion créée introuvable.".to_string())
    }

    pub fn update_connection(
        &mut self,
        connection_id: i64,
        mut draft: ConnectionDraft,
    ) -> Result<ConnectionItem, String> {
        draft.validate_and_normalize()?;

        if !self.connection_exists(connection_id)? {
            return Err(format!("La connexion #{connection_id} n'existe pas."));
        }
        if let Some(fid) = draft.folder_id
            && !self.folder_exists(fid)?
        {
            return Err(format!("Le dossier parent #{fid} n'existe pas."));
        }

        let now = unix_timestamp();
        let tx = self
            .conn
            .transaction()
            .map_err(|e| format!("Transaction mise à jour connexion : {e}"))?;

        if !draft.save_password {
            // L'utilisateur a décoché « Enregistrer le mot de passe » : supprimer le secret DPAPI stocké
            tx.execute(
                "UPDATE connections SET
                    folder_id = ?1, name = ?2, protocol = ?3, host = ?4, port = ?5,
                    username = ?6, domain = ?7, description = ?8, is_favorite = ?9,
                    save_password = 0, encrypted_password = NULL,
                    dynamic_resolution = ?10, redirect_clipboard = ?11, updated_at = ?12
                 WHERE id = ?13",
                params![
                    draft.folder_id,
                    draft.name,
                    draft.protocol.as_str(),
                    draft.host,
                    i32::from(draft.port),
                    draft.username,
                    draft.domain,
                    draft.description,
                    i32::from(draft.is_favorite),
                    i32::from(draft.dynamic_resolution),
                    i32::from(draft.redirect_clipboard),
                    now,
                    connection_id,
                ],
            )
            .map_err(|e| format!("Erreur mise à jour connexion : {e}"))?;
        } else if !draft.password.is_empty() {
            // Nouveau mot de passe fourni : chiffrer via DPAPI et remplacer
            let encrypted_blob = encrypt_secret(draft.password.as_str())?;
            draft.password.clear();
            tx.execute(
                "UPDATE connections SET
                    folder_id = ?1, name = ?2, protocol = ?3, host = ?4, port = ?5,
                    username = ?6, domain = ?7, description = ?8, is_favorite = ?9,
                    save_password = 1, encrypted_password = ?10,
                    dynamic_resolution = ?11, redirect_clipboard = ?12, updated_at = ?13
                 WHERE id = ?14",
                params![
                    draft.folder_id,
                    draft.name,
                    draft.protocol.as_str(),
                    draft.host,
                    i32::from(draft.port),
                    draft.username,
                    draft.domain,
                    draft.description,
                    i32::from(draft.is_favorite),
                    encrypted_blob,
                    i32::from(draft.dynamic_resolution),
                    i32::from(draft.redirect_clipboard),
                    now,
                    connection_id,
                ],
            )
            .map_err(|e| format!("Erreur mise à jour connexion : {e}"))?;
        } else if draft.keep_existing_password {
            // Conserver le mot de passe chiffré déjà stocké
            tx.execute(
                "UPDATE connections SET
                    folder_id = ?1, name = ?2, protocol = ?3, host = ?4, port = ?5,
                    username = ?6, domain = ?7, description = ?8, is_favorite = ?9,
                    dynamic_resolution = ?10, redirect_clipboard = ?11, updated_at = ?12
                 WHERE id = ?13",
                params![
                    draft.folder_id,
                    draft.name,
                    draft.protocol.as_str(),
                    draft.host,
                    i32::from(draft.port),
                    draft.username,
                    draft.domain,
                    draft.description,
                    i32::from(draft.is_favorite),
                    i32::from(draft.dynamic_resolution),
                    i32::from(draft.redirect_clipboard),
                    now,
                    connection_id,
                ],
            )
            .map_err(|e| format!("Erreur mise à jour connexion : {e}"))?;
        } else {
            // Aucun mot de passe saisi et pas de conservation demandée : vider le secret
            tx.execute(
                "UPDATE connections SET
                    folder_id = ?1, name = ?2, protocol = ?3, host = ?4, port = ?5,
                    username = ?6, domain = ?7, description = ?8, is_favorite = ?9,
                    save_password = 0, encrypted_password = NULL,
                    dynamic_resolution = ?10, redirect_clipboard = ?11, updated_at = ?12
                 WHERE id = ?13",
                params![
                    draft.folder_id,
                    draft.name,
                    draft.protocol.as_str(),
                    draft.host,
                    i32::from(draft.port),
                    draft.username,
                    draft.domain,
                    draft.description,
                    i32::from(draft.is_favorite),
                    i32::from(draft.dynamic_resolution),
                    i32::from(draft.redirect_clipboard),
                    now,
                    connection_id,
                ],
            )
            .map_err(|e| format!("Erreur mise à jour connexion : {e}"))?;
        }

        draft.password.clear();
        tx.commit()
            .map_err(|e| format!("Validation mise à jour connexion : {e}"))?;

        self.get_connection(connection_id)?
            .ok_or_else(|| "Connexion mise à jour introuvable.".to_string())
    }

    pub fn get_connection(&self, connection_id: i64) -> Result<Option<ConnectionItem>, String> {
        self.conn
            .query_row(
                "SELECT id, folder_id, name, protocol, host, port, username, domain, description,
                        is_favorite, save_password,
                        (encrypted_password IS NOT NULL AND length(encrypted_password) > 0) AS has_pwd,
                        dynamic_resolution, redirect_clipboard, sort_order,
                        created_at, updated_at, last_connected_at
                 FROM connections WHERE id = ?1",
                params![connection_id],
                Self::map_connection_row,
            )
            .optional()
            .map_err(|e| format!("Erreur lecture connexion #{connection_id} : {e}"))
    }

    pub fn list_connections(&self) -> Result<Vec<ConnectionItem>, String> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, folder_id, name, protocol, host, port, username, domain, description,
                        is_favorite, save_password,
                        (encrypted_password IS NOT NULL AND length(encrypted_password) > 0) AS has_pwd,
                        dynamic_resolution, redirect_clipboard, sort_order,
                        created_at, updated_at, last_connected_at
                 FROM connections
                 ORDER BY sort_order ASC, name COLLATE NOCASE ASC, id ASC",
            )
            .map_err(|e| format!("Erreur préparation list_connections : {e}"))?;

        let rows = stmt
            .query_map([], Self::map_connection_row)
            .map_err(|e| format!("Erreur exécution list_connections : {e}"))?;

        let mut out = Vec::new();
        for r in rows {
            out.push(r.map_err(|e| format!("Erreur décodage connexion : {e}"))?);
        }
        Ok(out)
    }

    /// Recherche des connexions par nom, hôte, identifiant, utilisateur, domaine ou description.
    pub fn search_connections(&self, query: &str) -> Result<Vec<ConnectionItem>, String> {
        let trimmed = query.trim();
        if trimmed.is_empty() {
            return self.list_connections();
        }
        let pattern = format!("%{}%", trimmed.to_lowercase());
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, folder_id, name, protocol, host, port, username, domain, description,
                        is_favorite, save_password,
                        (encrypted_password IS NOT NULL AND length(encrypted_password) > 0) AS has_pwd,
                        dynamic_resolution, redirect_clipboard, sort_order,
                        created_at, updated_at, last_connected_at
                 FROM connections
                 WHERE lower(name) LIKE ?1
                    OR lower(host) LIKE ?1
                    OR lower(username) LIKE ?1
                    OR lower(domain) LIKE ?1
                    OR lower(description) LIKE ?1
                    OR lower(protocol) LIKE ?1
                 ORDER BY is_favorite DESC, name COLLATE NOCASE ASC, id ASC",
            )
            .map_err(|e| format!("Erreur préparation search_connections : {e}"))?;

        let rows = stmt
            .query_map(params![pattern], Self::map_connection_row)
            .map_err(|e| format!("Erreur exécution search_connections : {e}"))?;

        let mut out = Vec::new();
        for r in rows {
            out.push(r.map_err(|e| format!("Erreur décodage connexion : {e}"))?);
        }
        Ok(out)
    }

    /// Déchiffre à la demande le mot de passe DPAPI associé à `connection_id`.
    pub fn load_decrypted_password(
        &self,
        connection_id: i64,
    ) -> Result<Option<SecretString>, String> {
        let blob_opt: Option<Option<Vec<u8>>> = self
            .conn
            .query_row(
                "SELECT encrypted_password FROM connections WHERE id = ?1",
                params![connection_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| format!("Erreur lecture du secret chiffré : {e}"))?;

        match blob_opt.flatten() {
            Some(blob) if !blob.is_empty() => {
                let secret = decrypt_secret(&blob)?;
                Ok(Some(secret))
            }
            _ => Ok(None),
        }
    }

    pub fn rename_connection(&mut self, connection_id: i64, new_name: &str) -> Result<(), String> {
        let trimmed = new_name.trim();
        if trimmed.is_empty() {
            return Err("Le nom de la connexion ne peut pas être vide.".to_string());
        }
        if trimmed.len() > 160 {
            return Err("Le nom de la connexion ne doit pas dépasser 160 caractères.".to_string());
        }
        let now = unix_timestamp();
        let affected = self
            .conn
            .execute(
                "UPDATE connections SET name = ?1, updated_at = ?2 WHERE id = ?3",
                params![trimmed, now, connection_id],
            )
            .map_err(|e| format!("Erreur renommage connexion : {e}"))?;
        if affected == 0 {
            return Err(format!("La connexion #{connection_id} n'existe pas."));
        }
        Ok(())
    }

    pub fn set_connection_favorite(
        &mut self,
        connection_id: i64,
        favorite: bool,
    ) -> Result<(), String> {
        let now = unix_timestamp();
        let affected = self
            .conn
            .execute(
                "UPDATE connections SET is_favorite = ?1, updated_at = ?2 WHERE id = ?3",
                params![i32::from(favorite), now, connection_id],
            )
            .map_err(|e| format!("Erreur mise à jour favori : {e}"))?;
        if affected == 0 {
            return Err(format!("La connexion #{connection_id} n'existe pas."));
        }
        Ok(())
    }

    pub fn move_connection(
        &mut self,
        connection_id: i64,
        target_folder_id: Option<i64>,
    ) -> Result<(), String> {
        if !self.connection_exists(connection_id)? {
            return Err(format!("La connexion #{connection_id} n'existe pas."));
        }
        if let Some(fid) = target_folder_id
            && !self.folder_exists(fid)?
        {
            return Err(format!("Le dossier cible #{fid} n'existe pas."));
        }
        let now = unix_timestamp();
        self.conn
            .execute(
                "UPDATE connections SET folder_id = ?1, updated_at = ?2 WHERE id = ?3",
                params![target_folder_id, now, connection_id],
            )
            .map_err(|e| format!("Erreur déplacement connexion : {e}"))?;
        Ok(())
    }

    /// Duplique une connexion existante (en copiant également son blob DPAPI si présent).
    pub fn duplicate_connection(&mut self, connection_id: i64) -> Result<ConnectionItem, String> {
        let orig = self
            .get_connection(connection_id)?
            .ok_or_else(|| format!("La connexion #{connection_id} n'existe pas."))?;

        let blob: Option<Vec<u8>> = self
            .conn
            .query_row(
                "SELECT encrypted_password FROM connections WHERE id = ?1",
                params![connection_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| format!("Erreur lecture blob connexion : {e}"))?
            .flatten();

        let new_name = format!("{} (Copie)", orig.name);
        let now = unix_timestamp();

        self.conn
            .execute(
                "INSERT INTO connections (
                    folder_id, name, protocol, host, port, username, domain, description,
                    is_favorite, save_password, encrypted_password, dynamic_resolution,
                    redirect_clipboard, sort_order, created_at, updated_at
                 ) VALUES (
                    ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8,
                    ?9, ?10, ?11, ?12,
                    ?13, ?14, ?15, ?15
                 )",
                params![
                    orig.folder_id,
                    new_name,
                    orig.protocol.as_str(),
                    orig.host,
                    i32::from(orig.port),
                    orig.username,
                    orig.domain,
                    orig.description,
                    i32::from(orig.is_favorite),
                    i32::from(orig.save_password),
                    blob,
                    i32::from(orig.dynamic_resolution),
                    i32::from(orig.redirect_clipboard),
                    orig.sort_order + 1,
                    now,
                ],
            )
            .map_err(|e| format!("Erreur lors de la duplication de la connexion : {e}"))?;

        let new_id = self.conn.last_insert_rowid();
        self.get_connection(new_id)?
            .ok_or_else(|| "Connexion dupliquée introuvable.".to_string())
    }

    pub fn delete_connection(&mut self, connection_id: i64) -> Result<(), String> {
        let affected = self
            .conn
            .execute(
                "DELETE FROM connections WHERE id = ?1",
                params![connection_id],
            )
            .map_err(|e| format!("Erreur suppression connexion : {e}"))?;
        if affected == 0 {
            return Err(format!("La connexion #{connection_id} n'existe pas."));
        }
        Ok(())
    }

    /// Enregistre l'utilisation d'une connexion dans `recent_connections` et met à jour `last_connected_at`.
    pub fn record_recent_connection(&mut self, connection_id: i64) -> Result<(), String> {
        if !self.connection_exists(connection_id)? {
            return Err(format!("La connexion #{connection_id} n'existe pas."));
        }
        let now = unix_timestamp();
        let tx = self
            .conn
            .transaction()
            .map_err(|e| format!("Transaction recent_connections : {e}"))?;

        tx.execute(
            "UPDATE connections SET last_connected_at = ?1 WHERE id = ?2",
            params![now, connection_id],
        )
        .map_err(|e| format!("Erreur mise à jour last_connected_at : {e}"))?;

        // Remplacer toute entrée précédente de cette connexion pour garder une liste dédupliquée
        tx.execute(
            "DELETE FROM recent_connections WHERE connection_id = ?1",
            params![connection_id],
        )
        .map_err(|e| format!("Erreur nettoyage recent_connections : {e}"))?;

        tx.execute(
            "INSERT INTO recent_connections (connection_id, connected_at) VALUES (?1, ?2)",
            params![connection_id, now],
        )
        .map_err(|e| format!("Erreur insertion recent_connections : {e}"))?;

        // Conserver les 25 connexions récentes les plus fraîches
        tx.execute(
            "DELETE FROM recent_connections WHERE id NOT IN (
                SELECT id FROM recent_connections ORDER BY connected_at DESC LIMIT 25
             )",
            [],
        )
        .map_err(|e| format!("Erreur purge recent_connections : {e}"))?;

        tx.commit()
            .map_err(|e| format!("Validation recent_connections : {e}"))?;
        Ok(())
    }

    pub fn list_recent_connections(
        &self,
        limit: usize,
    ) -> Result<Vec<RecentConnectionEntry>, String> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT c.id, c.folder_id, c.name, c.protocol, c.host, c.port, c.username, c.domain,
                        c.description, c.is_favorite, c.save_password,
                        (c.encrypted_password IS NOT NULL AND length(c.encrypted_password) > 0) AS has_pwd,
                        c.dynamic_resolution, c.redirect_clipboard, c.sort_order,
                        c.created_at, c.updated_at, c.last_connected_at,
                        r.connected_at
                 FROM recent_connections r
                 JOIN connections c ON c.id = r.connection_id
                 ORDER BY r.connected_at DESC, r.id DESC
                 LIMIT ?1",
            )
            .map_err(|e| format!("Erreur préparation list_recent_connections : {e}"))?;

        let rows = stmt
            .query_map(params![limit as i64], |row| {
                let conn_item = Self::map_connection_row(row)?;
                let connected_at: i64 = row.get(18)?;
                Ok(RecentConnectionEntry {
                    connection: conn_item,
                    connected_at,
                })
            })
            .map_err(|e| format!("Erreur exécution list_recent_connections : {e}"))?;

        let mut out = Vec::new();
        for r in rows {
            out.push(r.map_err(|e| format!("Erreur décodage recent_connection : {e}"))?);
        }
        Ok(out)
    }

    // =========================================================================
    // PRÉFÉRENCES (Table settings)
    // =========================================================================

    pub fn set_setting(&mut self, key: &str, value: &str) -> Result<(), String> {
        let now = unix_timestamp();
        self.conn
            .execute(
                "INSERT INTO settings (key, value, updated_at) VALUES (?1, ?2, ?3)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
                params![key, value, now],
            )
            .map_err(|e| format!("Erreur sauvegarde préférence '{key}' : {e}"))?;
        Ok(())
    }

    pub fn get_setting(&self, key: &str) -> Result<Option<String>, String> {
        self.conn
            .query_row(
                "SELECT value FROM settings WHERE key = ?1",
                params![key],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| format!("Erreur lecture préférence '{key}' : {e}"))
    }

    fn connection_exists(&self, connection_id: i64) -> Result<bool, String> {
        let count: i64 = self
            .conn
            .query_row(
                "SELECT COUNT(1) FROM connections WHERE id = ?1",
                params![connection_id],
                |row| row.get(0),
            )
            .map_err(|e| format!("Erreur vérification existence connexion : {e}"))?;
        Ok(count > 0)
    }

    fn map_connection_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ConnectionItem> {
        let proto_str: String = row.get(3)?;
        let protocol = Protocol::from_db_str(&proto_str).unwrap_or(Protocol::Rdp);
        let port_i32: i32 = row.get(5)?;
        Ok(ConnectionItem {
            id: row.get(0)?,
            folder_id: row.get(1)?,
            name: row.get(2)?,
            protocol,
            host: row.get(4)?,
            port: u16::try_from(port_i32).unwrap_or(3389),
            username: row.get(6)?,
            domain: row.get(7)?,
            description: row.get(8)?,
            is_favorite: row.get::<_, i32>(9)? != 0,
            save_password: row.get::<_, i32>(10)? != 0,
            has_saved_password: row.get::<_, i32>(11)? != 0,
            dynamic_resolution: row.get::<_, i32>(12)? != 0,
            redirect_clipboard: row.get::<_, i32>(13)? != 0,
            sort_order: row.get(14)?,
            created_at: row.get(15)?,
            updated_at: row.get(16)?,
            last_connected_at: row.get(17)?,
        })
    }
}

/// Retourne le répertoire `%LOCALAPPDATA%\Malvision`.
pub fn default_app_data_dir() -> Result<PathBuf, String> {
    if let Some(local_app_data) = std::env::var_os("LOCALAPPDATA") {
        let mut p = PathBuf::from(local_app_data);
        p.push("Malvision");
        return Ok(p);
    }
    if let Some(user_profile) = std::env::var_os("USERPROFILE") {
        let mut p = PathBuf::from(user_profile);
        p.push("AppData");
        p.push("Local");
        p.push("Malvision");
        return Ok(p);
    }
    Err("Variable d'environnement LOCALAPPDATA introuvable.".to_string())
}

fn unix_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_rdp_draft(name: &str, host: &str, folder_id: Option<i64>) -> ConnectionDraft {
        ConnectionDraft {
            folder_id,
            name: name.to_string(),
            protocol: Protocol::Rdp,
            host: host.to_string(),
            port: 3389,
            username: "Administrateur".to_string(),
            domain: "LAB".to_string(),
            description: "Serveur de test".to_string(),
            is_favorite: false,
            save_password: true,
            password: SecretString::new("MonSecretDPAPI!42".to_string()),
            keep_existing_password: false,
            dynamic_resolution: true,
            redirect_clipboard: true,
        }
    }

    #[test]
    fn test_schema_migration_and_settings() {
        let mut store = Storage::open_in_memory().unwrap();
        assert_eq!(store.schema_version().unwrap(), 1);

        store.set_setting("sidebar_width", "280").unwrap();
        assert_eq!(
            store.get_setting("sidebar_width").unwrap().as_deref(),
            Some("280")
        );
        store.set_setting("sidebar_width", "310").unwrap();
        assert_eq!(
            store.get_setting("sidebar_width").unwrap().as_deref(),
            Some("310")
        );
    }

    #[test]
    fn test_folders_crud_subfolders_and_cycle_prevention() {
        let mut store = Storage::open_in_memory().unwrap();

        let root = store.create_folder("Clients", None).unwrap();
        let child = store.create_folder("Europe", Some(root.id)).unwrap();
        let grandchild = store.create_folder("France", Some(child.id)).unwrap();

        assert_eq!(store.list_folders().unwrap().len(), 3);

        // Renommage
        store
            .rename_folder(grandchild.id, "France - Paris")
            .unwrap();
        assert_eq!(
            store.get_folder(grandchild.id).unwrap().unwrap().name,
            "France - Paris"
        );

        // Prévention de cycle : impossible de déplacer `root` dans lui-même
        assert!(store.move_folder(root.id, Some(root.id)).is_err());
        // Impossible de déplacer `root` dans son descendant `grandchild`
        assert!(store.move_folder(root.id, Some(grandchild.id)).is_err());

        // Déplacement valide : `grandchild` directement sous `root`
        store.move_folder(grandchild.id, Some(root.id)).unwrap();
        assert_eq!(
            store.get_folder(grandchild.id).unwrap().unwrap().parent_id,
            Some(root.id)
        );
    }

    #[test]
    fn test_connections_crud_dpapi_search_favorites_and_recents() {
        let mut store = Storage::open_in_memory().unwrap();
        let folder = store.create_folder("Production", None).unwrap();

        let conn = store
            .create_connection(sample_rdp_draft("SRV-SQL-01", "10.0.1.15", Some(folder.id)))
            .unwrap();
        assert!(conn.has_saved_password);
        assert!(conn.save_password);

        // Vérifier qu'aucun mot de passe en clair n'est stocké dans la colonne BLOB de SQLite
        let raw_blob: Vec<u8> = store
            .conn
            .query_row(
                "SELECT encrypted_password FROM connections WHERE id = ?1",
                params![conn.id],
                |row| row.get(0),
            )
            .unwrap();
        assert!(
            !raw_blob
                .windows(b"MonSecretDPAPI!42".len())
                .any(|w| w == b"MonSecretDPAPI!42")
        );

        // Déchiffrement DPAPI à la demande
        let dec = store
            .load_decrypted_password(conn.id)
            .unwrap()
            .expect("Doit avoir un mot de passe");
        assert_eq!(dec.as_str(), "MonSecretDPAPI!42");

        // Duplication
        let dup = store.duplicate_connection(conn.id).unwrap();
        assert_ne!(dup.id, conn.id);
        assert_eq!(dup.name, "SRV-SQL-01 (Copie)");
        assert!(dup.has_saved_password);

        // Favoris et recherche
        store.set_connection_favorite(conn.id, true).unwrap();
        let results = store.search_connections("sql-01").unwrap();
        assert_eq!(results.len(), 2);
        assert!(results[0].is_favorite);

        // Historique récent
        store.record_recent_connection(conn.id).unwrap();
        let recents = store.list_recent_connections(10).unwrap();
        assert_eq!(recents.len(), 1);
        assert_eq!(recents[0].connection.id, conn.id);

        // Suppression du mot de passe via update_connection (save_password = false)
        let mut update_draft = sample_rdp_draft("SRV-SQL-01", "10.0.1.15", Some(folder.id));
        update_draft.save_password = false;
        let updated = store.update_connection(conn.id, update_draft).unwrap();
        assert!(!updated.has_saved_password);
        assert!(store.load_decrypted_password(conn.id).unwrap().is_none());
    }

    #[test]
    fn test_persistence_across_reopen() {
        let mut temp_path = std::env::temp_dir();
        temp_path.push(format!("malvision_test_{}.db", std::process::id()));
        let _ = std::fs::remove_file(&temp_path);

        {
            let mut store = Storage::open_at_path(&temp_path).unwrap();
            let f = store.create_folder("Infra", None).unwrap();
            let _ = store
                .create_connection(sample_rdp_draft("Bastion", "192.168.100.1", Some(f.id)))
                .unwrap();
        }

        {
            let store = Storage::open_at_path(&temp_path).unwrap();
            let folders = store.list_folders().unwrap();
            let conns = store.list_connections().unwrap();
            assert_eq!(folders.len(), 1);
            assert_eq!(folders[0].name, "Infra");
            assert_eq!(conns.len(), 1);
            assert_eq!(conns[0].name, "Bastion");
            let pwd = store.load_decrypted_password(conns[0].id).unwrap().unwrap();
            assert_eq!(pwd.as_str(), "MonSecretDPAPI!42");
        }

        let _ = std::fs::remove_file(&temp_path);
    }
}
