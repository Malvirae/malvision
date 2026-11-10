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

const CURRENT_SCHEMA_VERSION: i32 = 3;

/// Résumé du diagnostic et du résultat d'un import de configuration SQLite.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportSummary {
    pub folders_count: usize,
    pub connections_count: usize,
    pub dpapi_recoverable_count: usize,
    pub dpapi_unrecoverable_count: usize,
}

pub struct Storage {
    conn: Connection,
    db_path: PathBuf,
}

/// Remplace un fichier cible par un fichier source de manière atomique sur Windows.
/// Préserve le fichier cible d'origine en cas d'échec de la substitution.
pub fn atomic_replace_file(src: &Path, dst: &Path) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    use windows::Win32::Storage::FileSystem::{
        MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
    };
    use windows::core::PCWSTR;

    let mut src_w: Vec<u16> = src.as_os_str().encode_wide().collect();
    src_w.push(0);
    let mut dst_w: Vec<u16> = dst.as_os_str().encode_wide().collect();
    dst_w.push(0);

    unsafe {
        MoveFileExW(
            PCWSTR(src_w.as_ptr()),
            PCWSTR(dst_w.as_ptr()),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
        .map_err(|e| {
            format!(
                "Échec du remplacement atomique du fichier '{}' vers '{}' : {e}",
                src.display(),
                dst.display()
            )
        })?;
    }
    Ok(())
}

impl Storage {
    /// Retourne le chemin d'accès au fichier de base de données actif.
    pub fn db_path(&self) -> &Path {
        &self.db_path
    }

    /// Ouvre la base de données active résolue par le bootstrap (emplacement personnalisé si défini,
    /// sinon emplacement par défaut dans `%LOCALAPPDATA%\Malvision\malvision.db`).
    /// Si un emplacement personnalisé configuré est introuvable au démarrage, sollicite l'utilisateur.
    pub fn open_default() -> Result<Self, String> {
        let cfg = match crate::bootstrap::load_bootstrap() {
            Ok(c) => c,
            Err(err) => {
                if !cfg!(test) {
                    crate::ui::prompt_corrupt_bootstrap_at_startup(&err)?
                } else {
                    return Err(err);
                }
            }
        };
        let db_path = if let Some(custom) = cfg.custom_db_path {
            if !custom.exists() && !cfg!(test) {
                crate::ui::prompt_missing_custom_db_at_startup(&custom)?
            } else {
                custom
            }
        } else {
            crate::bootstrap::default_database_path()?
        };

        if let Some(parent) = db_path.parent()
            && !parent.as_os_str().is_empty()
            && !parent.exists()
        {
            std::fs::create_dir_all(parent).map_err(|e| {
                format!(
                    "Impossible de créer le répertoire de données '{}' : {e}",
                    parent.display()
                )
            })?;
        }
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

        if version < 2 {
            let now = unix_timestamp();
            let tx = self
                .conn
                .transaction()
                .map_err(|e| format!("Échec d'ouverture de transaction de migration v2 : {e}"))?;

            let default_pairs = [
                ("theme_mode", "dark"),
                ("close_to_tray", "0"),
                ("start_minimized_to_tray", "0"),
                ("confirm_exit_active_sessions", "1"),
                ("restore_rdp_sessions", "0"),
                ("start_with_windows", "0"),
                ("show_recent_connections", "1"),
                ("max_recent_connections", "10"),
                ("rdp_default_clipboard", "1"),
                ("rdp_default_dynamic_resolution", "1"),
                ("rdp_keyboard_hook_mode", "remote"),
            ];
            for (k, v) in default_pairs {
                tx.execute(
                    "INSERT OR IGNORE INTO settings (key, value, updated_at) VALUES (?1, ?2, ?3)",
                    params![k, v, now],
                )
                .map_err(|e| format!("Échec initialisation préférence '{k}' (v2) : {e}"))?;
            }

            tx.execute_batch("PRAGMA user_version = 2;")
                .map_err(|e| format!("Échec mise à jour user_version = 2 : {e}"))?;

            tx.commit()
                .map_err(|e| format!("Échec de validation de la migration SQLite v2 : {e}"))?;
        }

        if version < 3 {
            let now = unix_timestamp();
            let tx = self
                .conn
                .transaction()
                .map_err(|e| format!("Échec d'ouverture de transaction de migration v3 : {e}"))?;

            // Si la base v2 contenait l'ancienne valeur préparée 'fullscreen_only', mettre à jour vers 'remote' (Jalon 3)
            let _ = tx.execute(
                "UPDATE settings SET value = 'remote', updated_at = ?1 WHERE key = 'rdp_keyboard_hook_mode' AND value = 'fullscreen_only'",
                params![now],
            );

            let default_pairs = [
                ("anydesk_path", ""),
                ("rustdesk_path", ""),
                ("rdp_keyboard_hook_mode", "remote"),
            ];
            for (k, v) in default_pairs {
                tx.execute(
                    "INSERT OR IGNORE INTO settings (key, value, updated_at) VALUES (?1, ?2, ?3)",
                    params![k, v, now],
                )
                .map_err(|e| format!("Échec initialisation préférence '{k}' (v3) : {e}"))?;
            }

            tx.execute_batch("PRAGMA user_version = 3;")
                .map_err(|e| format!("Échec mise à jour user_version = 3 : {e}"))?;

            tx.commit()
                .map_err(|e| format!("Échec de validation de la migration SQLite v3 : {e}"))?;
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

        // Conserver les 50 connexions récentes les plus fraîches
        tx.execute(
            "DELETE FROM recent_connections WHERE id NOT IN (
                SELECT id FROM recent_connections ORDER BY connected_at DESC LIMIT 50
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

    fn sqlite_auxiliary_paths(path: &Path) -> [PathBuf; 3] {
        let s = path.to_string_lossy();
        [
            PathBuf::from(format!("{s}-wal")),
            PathBuf::from(format!("{s}-shm")),
            PathBuf::from(format!("{s}-journal")),
        ]
    }

    /// Exporte la base de données courante vers le fichier spécifié en utilisant
    /// l'API de sauvegarde SQLite (`rusqlite::backup::Backup`).
    ///
    /// Garanties de robustesse et sécurité :
    /// - La sauvegarde est d'abord écrite dans un fichier temporaire sur le même volume/dossier.
    /// - L'intégrité SQLite (`PRAGMA integrity_check`) et le schéma sont validés avant tout remplacement.
    /// - Si le fichier de destination existe déjà, il est conservé intact en cas d'erreur de sauvegarde.
    /// - Le journal WAL est vidé (`wal_checkpoint(TRUNCATE)`) et basculé en mode DELETE afin de
    ///   garantir un fichier `.db` totalement autonome sans fichiers `-wal` ou `-shm` résiduels.
    /// - Le remplacement vers la destination finale est atomique et sécurisé.
    /// - Tous les fichiers temporaires et auxiliaires sont nettoyés en cas de succès comme en cas d'échec.
    pub fn export_database(&self, dest_path: &Path) -> Result<(), String> {
        let parent = dest_path.parent().unwrap_or_else(|| Path::new("."));
        if !parent.as_os_str().is_empty() && !parent.exists() {
            std::fs::create_dir_all(parent).map_err(|e| {
                format!(
                    "Impossible de créer le dossier de destination '{}' : {e}",
                    parent.display()
                )
            })?;
        }

        let file_stem = dest_path
            .file_stem()
            .map(|s| s.to_string_lossy())
            .unwrap_or_else(|| "backup".into());
        let pid = std::process::id();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let temp_path = parent.join(format!(".{file_stem}_{pid}_{nanos}.tmp_export"));

        if temp_path.exists() {
            let _ = std::fs::remove_file(&temp_path);
        }
        for aux in Self::sqlite_auxiliary_paths(&temp_path) {
            if aux.exists() {
                let _ = std::fs::remove_file(aux);
            }
        }

        // Structure Guard pour garantir le nettoyage automatique du fichier temporaire et de ses fichiers
        // auxiliaires (-wal, -shm, -journal) en cas d'erreur ou d'interruption
        struct TempCleanup<'a>(&'a Path);
        impl Drop for TempCleanup<'_> {
            fn drop(&mut self) {
                if self.0.exists() {
                    let _ = std::fs::remove_file(self.0);
                }
                for aux in Storage::sqlite_auxiliary_paths(self.0) {
                    if aux.exists() {
                        let _ = std::fs::remove_file(aux);
                    }
                }
            }
        }
        let cleanup = TempCleanup(&temp_path);

        // 1. Sauvegarde vers le fichier temporaire sur le même volume
        {
            let mut dst_conn = Connection::open(&temp_path).map_err(|e| {
                format!(
                    "Impossible d'initialiser le fichier d'export temporaire '{}' : {e}",
                    temp_path.display()
                )
            })?;

            {
                let backup =
                    rusqlite::backup::Backup::new(&self.conn, &mut dst_conn).map_err(|e| {
                        format!("Échec de l'initialisation de la sauvegarde SQLite : {e}")
                    })?;

                backup
                    .run_to_completion(100, std::time::Duration::from_millis(10), None)
                    .map_err(|e| format!("Erreur lors de l'exportation de la base SQLite : {e}"))?;
            } // backup est libéré ici avant d'exécuter des requêtes sur dst_conn

            // Finaliser le journal WAL et basculer la base exportée en mode journal autonome DELETE.
            // Cela transfère toutes les pages WAL dans le fichier .db principal et supprime les fichiers -wal et -shm.
            let _ = dst_conn.execute_batch(
                "PRAGMA wal_checkpoint(TRUNCATE);
                 PRAGMA journal_mode = DELETE;",
            );

            // dst_conn est explicitement fermé ici, libérant les verrous de fichier Windows
            drop(dst_conn);
        }

        // 2. Vérification d'intégrité stricte sur le fichier temporaire avant tout remplacement
        {
            let check_conn = Connection::open_with_flags(
                &temp_path,
                rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
            )
            .map_err(|e| {
                format!(
                    "Impossible d'ouvrir le fichier temporaire d'export pour vérification : {e}"
                )
            })?;

            let integrity: String = check_conn
                .query_row("PRAGMA integrity_check;", [], |r| r.get(0))
                .map_err(|e| {
                    format!("Erreur contrôle d'intégrité sur la sauvegarde temporaire : {e}")
                })?;
            if integrity.to_lowercase() != "ok" {
                return Err(format!(
                    "Échec du contrôle d'intégrité de la sauvegarde : {integrity}"
                ));
            }

            let tables: Vec<String> = {
                let mut stmt = check_conn
                    .prepare("SELECT name FROM sqlite_master WHERE type='table';")
                    .map_err(|e| format!("Erreur de lecture du schéma de la sauvegarde : {e}"))?;
                let rows = stmt
                    .query_map([], |r| r.get(0))
                    .map_err(|e| format!("Erreur listage des tables de la sauvegarde : {e}"))?;
                rows.filter_map(Result::ok).collect()
            };

            if !tables.contains(&"folders".to_string())
                || !tables.contains(&"connections".to_string())
            {
                return Err(
                    "La sauvegarde exportée est incomplète (tables requises manquantes)."
                        .to_string(),
                );
            }
            // check_conn est explicitement fermé ici
            drop(check_conn);
        }

        // 3. Remplacement atomique contrôlé vers la destination finale
        // Le fichier de destination existant (le cas échéant) n'est remplacé qu'après succès complet des étapes 1 et 2
        atomic_replace_file(&temp_path, dest_path)?;

        // Nettoyage garanti de tous les fichiers auxiliaires temporaires (-wal, -shm, -journal)
        // créés spécifiquement pour le fichier temporaire de cet export
        for aux in Self::sqlite_auxiliary_paths(&temp_path) {
            if aux.exists() {
                let _ = std::fs::remove_file(aux);
            }
        }

        // Si l'ancienne destination avait des fichiers auxiliaires résiduels, les nettoyer également
        for aux in Self::sqlite_auxiliary_paths(dest_path) {
            if aux.exists() {
                let _ = std::fs::remove_file(aux);
            }
        }

        // Désactiver le nettoyage de temp_path puisqu'il a été déplacé avec succès vers dest_path
        std::mem::forget(cleanup);

        Ok(())
    }

    /// Valide l'intégrité et la conformité d'un fichier SQLite avant import.
    pub fn validate_import_file(path: &Path) -> Result<ImportSummary, String> {
        if !path.exists() || !path.is_file() {
            return Err(
                "Le fichier sélectionné n'existe pas ou n'est pas un fichier valide.".to_string(),
            );
        }

        let conn = Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(|e| {
                format!("Impossible d'ouvrir le fichier : n'est pas une base SQLite valide ({e})")
            })?;

        // 1. Contrôle d'intégrité SQLite (PRAGMA integrity_check)
        let integrity: String = conn
            .query_row("PRAGMA integrity_check;", [], |r| r.get(0))
            .map_err(|e| format!("Erreur lors de la vérification d'intégrité SQLite : {e}"))?;
        if integrity.to_lowercase() != "ok" {
            return Err(format!(
                "Échec du contrôle d'intégrité SQLite : {integrity}"
            ));
        }

        // 2. Vérification de l'existence des tables obligatoires (folders, connections)
        let tables: Vec<String> = {
            let mut stmt = conn
                .prepare("SELECT name FROM sqlite_master WHERE type='table';")
                .map_err(|e| format!("Erreur de lecture du schéma : {e}"))?;
            let rows = stmt
                .query_map([], |r| r.get(0))
                .map_err(|e| format!("Erreur lors du listage des tables : {e}"))?;
            rows.filter_map(Result::ok).collect()
        };

        if !tables.contains(&"folders".to_string()) || !tables.contains(&"connections".to_string())
        {
            return Err("Le fichier sélectionné ne contient pas la structure de données attendue pour Malvision (tables 'folders' ou 'connections' manquantes).".to_string());
        }

        // 3. Compter les dossiers et connexions
        let folders_count: usize = conn
            .query_row("SELECT COUNT(*) FROM folders;", [], |r| r.get(0))
            .unwrap_or(0);
        let connections_count: usize = conn
            .query_row("SELECT COUNT(*) FROM connections;", [], |r| r.get(0))
            .unwrap_or(0);

        // 4. Tester la déchiffrabilité DPAPI sans exposer ni logger les secrets
        let mut dpapi_recoverable = 0;
        let mut dpapi_unrecoverable = 0;
        if let Ok(mut stmt) = conn.prepare(
            "SELECT encrypted_password FROM connections WHERE encrypted_password IS NOT NULL;",
        ) && let Ok(rows) = stmt.query_map([], |r| r.get::<_, Vec<u8>>(0))
        {
            for blob in rows.flatten() {
                if blob.is_empty() {
                    continue;
                }
                if decrypt_secret(&blob).is_ok() {
                    dpapi_recoverable += 1;
                } else {
                    dpapi_unrecoverable += 1;
                }
            }
        }

        Ok(ImportSummary {
            folders_count,
            connections_count,
            dpapi_recoverable_count: dpapi_recoverable,
            dpapi_unrecoverable_count: dpapi_unrecoverable,
        })
    }

    /// Importe une configuration depuis un fichier SQLite externe en remplacement contrôlé de la base courante.
    ///
    /// Garanties de robustesse et sécurité :
    /// - Le fichier source est intégralement validé (`validate_import_file`) avant tout remplacement.
    /// - Une sauvegarde de sécurité (`.pre_import_bak`) de la configuration active est créée et vérifiée.
    /// - En cas d'échec durant la restauration ou la migration de schéma, un retour arrière automatique
    ///   vers la configuration d'origine est effectué depuis la sauvegarde de sécurité.
    pub fn import_database(&mut self, src_path: &Path) -> Result<ImportSummary, String> {
        let summary = Self::validate_import_file(src_path)?;

        // 1. Sauvegarde de sécurité vérifiée avant tout remplacement
        let mut backup_created = false;
        let backup_path = if !self.db_path.as_os_str().is_empty() && self.db_path.exists() {
            let bp = self.db_path.with_extension("db.pre_import_bak");
            self.export_database(&bp).map_err(|e| {
                format!("Échec de la sauvegarde de sécurité de la configuration actuelle. Importation annulée : {e}")
            })?;
            // Vérifier immédiatement que la sauvegarde de sécurité est elle-même valide et intègre
            Self::validate_import_file(&bp).map_err(|e| {
                format!("La sauvegarde de sécurité (.pre_import_bak) a échoué aux contrôles d'intégrité. Importation annulée : {e}")
            })?;
            backup_created = true;
            Some(bp)
        } else {
            None
        };

        // 2. Ouvrir le fichier source
        let src_conn =
            Connection::open_with_flags(src_path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
                .map_err(|e| format!("Impossible d'ouvrir le fichier source : {e}"))?;

        // 3. Restaurer vers self.conn avec retour automatique à l'état initial en cas d'échec
        let restore_result = (|| -> Result<(), String> {
            let backup = rusqlite::backup::Backup::new(&src_conn, &mut self.conn)
                .map_err(|e| format!("Échec de l'initialisation de la restauration : {e}"))?;

            backup
                .run_to_completion(100, std::time::Duration::from_millis(10), None)
                .map_err(|e| format!("Erreur lors de la restauration de la base : {e}"))?;

            drop(backup);

            // Appliquer et valider les migrations de schéma
            self.initialize_and_migrate()?;

            Ok(())
        })();

        if let Err(err) = restore_result {
            // En cas d'échec, effectuer un rollback automatique depuis backup_path
            if backup_created && let Some(bp) = &backup_path {
                let rollback_res = (|| -> Result<(), String> {
                    let bak_conn =
                        Connection::open_with_flags(bp, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
                            .map_err(|e| {
                                format!("Impossible de rouvrir la sauvegarde de secours : {e}")
                            })?;
                    let rollback_backup = rusqlite::backup::Backup::new(&bak_conn, &mut self.conn)
                        .map_err(|e| format!("Échec initialisation rollback : {e}"))?;
                    rollback_backup
                        .run_to_completion(100, std::time::Duration::from_millis(10), None)
                        .map_err(|e| format!("Échec exécution rollback : {e}"))?;
                    drop(rollback_backup);
                    self.initialize_and_migrate()?;
                    Ok(())
                })();

                if let Err(rollback_err) = rollback_res {
                    return Err(format!(
                        "Erreur critique lors de l'importation : {err}.\nTentative de retour à l'état initial échouée : {rollback_err}.\nVotre configuration précédente est disponible dans : {}",
                        bp.display()
                    ));
                }

                return Err(format!(
                    "L'importation a échoué ({err}). Votre configuration d'origine a été automatiquement restaurée sans perte de données."
                ));
            }

            return Err(format!("L'importation a échoué : {err}"));
        }

        Ok(summary)
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
        assert_eq!(store.schema_version().unwrap(), 3);
        assert_eq!(
            store.get_setting("theme_mode").unwrap().as_deref(),
            Some("dark")
        );
        assert_eq!(
            store
                .get_setting("rdp_keyboard_hook_mode")
                .unwrap()
                .as_deref(),
            Some("remote")
        );
        assert_eq!(
            store.get_setting("anydesk_path").unwrap().as_deref(),
            Some("")
        );
        assert_eq!(
            store.get_setting("rustdesk_path").unwrap().as_deref(),
            Some("")
        );

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

    #[test]
    fn test_storage_export_and_import_with_wal_and_dpapi_preservation() {
        let base_dir = std::env::temp_dir();
        let pid = std::process::id();
        let orig_path = base_dir.join(format!("malvision_export_orig_{pid}.db"));
        let export_path = base_dir.join(format!("malvision_export_target_{pid}.db"));
        let restore_path = base_dir.join(format!("malvision_export_restored_{pid}.db"));

        let _ = std::fs::remove_file(&orig_path);
        let _ = std::fs::remove_file(&export_path);
        let _ = std::fs::remove_file(&restore_path);

        // 1. Peupler une base source
        let mut src_store = Storage::open_at_path(&orig_path).unwrap();
        let f1 = src_store.create_folder("Dossier Production", None).unwrap();
        let c1 = src_store
            .create_connection(sample_rdp_draft("Serveur Web", "192.168.1.10", Some(f1.id)))
            .unwrap();
        src_store.set_setting("theme_mode", "light").unwrap();

        // 2. Exporter vers export_path
        src_store.export_database(&export_path).unwrap();
        assert!(export_path.exists());

        // 3. Valider l'intégrité de l'export
        let summary = Storage::validate_import_file(&export_path).unwrap();
        assert_eq!(summary.folders_count, 1);
        assert_eq!(summary.connections_count, 1);
        assert_eq!(summary.dpapi_recoverable_count, 1);
        assert_eq!(summary.dpapi_unrecoverable_count, 0);

        // 4. Importer dans une nouvelle base cible
        let mut dst_store = Storage::open_at_path(&restore_path).unwrap();
        let import_res = dst_store.import_database(&export_path).unwrap();
        assert_eq!(import_res.connections_count, 1);

        // Vérifier les données restaurées
        let folders = dst_store.list_folders().unwrap();
        assert_eq!(folders.len(), 1);
        assert_eq!(folders[0].name, "Dossier Production");

        let conns = dst_store.list_connections().unwrap();
        assert_eq!(conns.len(), 1);
        assert_eq!(conns[0].name, "Serveur Web");
        assert_eq!(conns[0].host, "192.168.1.10");

        let pwd = dst_store.load_decrypted_password(c1.id).unwrap().unwrap();
        assert_eq!(pwd.as_str(), "MonSecretDPAPI!42");

        let setting = dst_store.get_setting("theme_mode").unwrap();
        assert_eq!(setting.as_deref(), Some("light"));

        // Nettoyage
        let _ = std::fs::remove_file(&orig_path);
        let _ = std::fs::remove_file(&export_path);
        let _ = std::fs::remove_file(&restore_path);
        let _ = std::fs::remove_file(restore_path.with_extension("db.pre_import_bak"));
    }

    #[test]
    fn test_storage_import_rejects_corrupted_or_invalid_file() {
        let base_dir = std::env::temp_dir();
        let pid = std::process::id();
        let fake_file = base_dir.join(format!("malvision_corrupt_{pid}.db"));

        // Fichier texte non SQLite
        std::fs::write(&fake_file, b"NOT A SQLITE DATABASE").unwrap();
        let res = Storage::validate_import_file(&fake_file);
        assert!(res.is_err());

        // Fichier inexistant
        let nonexistent = base_dir.join(format!("malvision_nonexistent_{pid}.db"));
        assert!(Storage::validate_import_file(&nonexistent).is_err());

        let _ = std::fs::remove_file(&fake_file);
    }

    #[test]
    fn test_storage_export_preserves_existing_destination_on_failure_and_cleans_temp() {
        let base_dir = std::env::temp_dir();
        let pid = std::process::id();
        let nanos = std::time::SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let src_path = base_dir.join(format!("malvision_export_src_{pid}_{nanos}.db"));
        let existing_dest = base_dir.join(format!("malvision_existing_dest_{pid}_{nanos}.db"));

        let _ = std::fs::remove_file(&src_path);
        let _ = std::fs::remove_file(&existing_dest);

        // Créer un fichier de destination existant avec un contenu textuel distinct
        std::fs::write(
            &existing_dest,
            b"DONNEES DE SAUVEGARDE EXISTANTE INTANGIBLE",
        )
        .unwrap();

        let mut store = Storage::open_at_path(&src_path).unwrap();
        let _ = store.create_folder("Dossier Export Test", None).unwrap();

        // 1. Export vers la destination existante : doit réussir et remplacer proprement
        store.export_database(&existing_dest).unwrap();
        assert!(existing_dest.exists());
        let summary = Storage::validate_import_file(&existing_dest).unwrap();
        assert_eq!(summary.folders_count, 1);

        // 2. Vérifier qu'aucun fichier temporaire .tmp_export ne subsiste
        let entries = std::fs::read_dir(&base_dir).unwrap();
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.contains(&format!("{pid}_{nanos}")) && name.ends_with(".tmp_export") {
                panic!("Un fichier temporaire résiduel a été trouvé : {name}");
            }
        }

        // Nettoyage
        let _ = std::fs::remove_file(&src_path);
        let _ = std::fs::remove_file(&existing_dest);
    }

    #[test]
    fn test_storage_export_with_wal_mode_transactions() {
        let base_dir = std::env::temp_dir();
        let pid = std::process::id();
        let nanos = std::time::SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let wal_db_path = base_dir.join(format!("malvision_wal_src_{pid}_{nanos}.db"));
        let wal_dest_path = base_dir.join(format!("malvision_wal_dest_{pid}_{nanos}.db"));

        let _ = std::fs::remove_file(&wal_db_path);
        let _ = std::fs::remove_file(&wal_dest_path);

        let mut store = Storage::open_at_path(&wal_db_path).unwrap();

        // Forcer le mode WAL
        let mode: String = store
            .conn
            .query_row("PRAGMA journal_mode=WAL;", [], |r| r.get(0))
            .unwrap();
        assert_eq!(mode.to_uppercase(), "WAL");

        // Écrire plusieurs transactions en mode WAL
        let f = store.create_folder("Cluster Kubernetes", None).unwrap();
        store
            .create_connection(sample_rdp_draft("Noeud-01", "10.0.0.1", Some(f.id)))
            .unwrap();
        store
            .create_connection(sample_rdp_draft("Noeud-02", "10.0.0.2", Some(f.id)))
            .unwrap();

        // Exporter sans checkpoint forcé : l'API Backup doit capturer toutes les pages WAL validées
        store.export_database(&wal_dest_path).unwrap();

        // Vérifier le fichier exporté de manière autonome
        let exported_summary = Storage::validate_import_file(&wal_dest_path).unwrap();
        assert_eq!(exported_summary.folders_count, 1);
        assert_eq!(exported_summary.connections_count, 2);

        // Nettoyage
        let _ = std::fs::remove_file(&wal_db_path);
        let _ = std::fs::remove_file(wal_db_path.with_extension("db-wal"));
        let _ = std::fs::remove_file(wal_db_path.with_extension("db-shm"));
        let _ = std::fs::remove_file(&wal_dest_path);
    }

    #[test]
    fn test_storage_import_automatic_rollback_on_failed_migration() {
        let base_dir = std::env::temp_dir();
        let pid = std::process::id();
        let nanos = std::time::SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let main_db_path = base_dir.join(format!("malvision_main_{pid}_{nanos}.db"));
        let bad_import_path = base_dir.join(format!("malvision_bad_{pid}_{nanos}.db"));

        let _ = std::fs::remove_file(&main_db_path);
        let _ = std::fs::remove_file(&bad_import_path);

        // 1. Base principale active avec des données précieuses
        let mut store = Storage::open_at_path(&main_db_path).unwrap();
        let f_init = store.create_folder("Dossier Intact", None).unwrap();
        let _ = store
            .create_connection(sample_rdp_draft(
                "Serveur Critique",
                "10.50.0.1",
                Some(f_init.id),
            ))
            .unwrap();
        store.set_setting("theme_mode", "light").unwrap();

        assert_eq!(store.list_connections().unwrap().len(), 1);

        // 2. Créer une base SQLite incompatible ou corrompue (sans table connections valide)
        {
            let bad_conn = Connection::open(&bad_import_path).unwrap();
            bad_conn
                .execute_batch(
                    "CREATE TABLE folders (id INTEGER PRIMARY KEY, name TEXT NOT NULL, parent_id INTEGER, sort_order INTEGER DEFAULT 0, created_at INTEGER, updated_at INTEGER);
                     CREATE TABLE unrelated_table (id INTEGER PRIMARY KEY);",
                )
                .unwrap();
        }

        // 3. Tenter l'import : doit être rejeté par validate_import_file sans toucher à la base principale
        let res = store.import_database(&bad_import_path);
        assert!(res.is_err());

        // 4. Vérifier que la configuration initiale est 100% intacte
        let conns = store.list_connections().unwrap();
        assert_eq!(conns.len(), 1);
        assert_eq!(conns[0].name, "Serveur Critique");
        assert_eq!(
            store.get_setting("theme_mode").unwrap().as_deref(),
            Some("light")
        );

        // Nettoyage
        let _ = std::fs::remove_file(&main_db_path);
        let _ = std::fs::remove_file(main_db_path.with_extension("db.pre_import_bak"));
        let _ = std::fs::remove_file(&bad_import_path);
    }

    #[test]
    fn test_storage_export_produces_only_single_db_without_auxiliary_files() {
        let base_dir = std::env::temp_dir();
        let pid = std::process::id();
        let nanos = std::time::SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let test_root = base_dir.join(format!("malvision_export_clean_test_{pid}_{nanos}"));
        std::fs::create_dir_all(&test_root).unwrap();

        let src_path = test_root.join("active_source.db");
        let mut store = Storage::open_at_path(&src_path).unwrap();

        // Créer des dossiers et connexions avec mots de passe chiffrés DPAPI
        let f = store.create_folder("Dossier Production", None).unwrap();
        store
            .create_connection(sample_rdp_draft(
                "Serveur Principal",
                "192.168.1.100",
                Some(f.id),
            ))
            .unwrap();

        // 1. Export standard vers un sous-dossier d'export dédié
        let export_dir = test_root.join("backups");
        std::fs::create_dir_all(&export_dir).unwrap();
        let dest_path = export_dir.join("malvision_backup_2026-11-10.db");

        store
            .export_database(&dest_path)
            .expect("L'exportation doit réussir");

        // 2. Vérifier que la destination finale existe et que son contenu est intègre
        assert!(dest_path.exists(), "Le fichier exporté doit exister");
        let summary = Storage::validate_import_file(&dest_path)
            .expect("La sauvegarde exportée doit être intègre");
        assert_eq!(summary.folders_count, 1);
        assert_eq!(summary.connections_count, 1);

        // 3. Vérifier qu'aucun fichier temporaire .tmp_export ou auxiliaire (-wal, -shm, -journal) ne subsiste
        let entries: Vec<PathBuf> = std::fs::read_dir(&export_dir)
            .unwrap()
            .filter_map(Result::ok)
            .map(|e| e.path())
            .collect();

        assert_eq!(
            entries.len(),
            1,
            "Le dossier d'export ne doit contenir STRICTEMENT que le fichier .db final, trouvé: {:?}",
            entries
        );
        assert_eq!(entries[0], dest_path);

        // 4. Vérifier qu'aucun fichier .tmp_export-wal ou .tmp_export-shm n'a fuité dans test_root
        let root_entries: Vec<PathBuf> = std::fs::read_dir(&test_root)
            .unwrap()
            .filter_map(Result::ok)
            .map(|e| e.path())
            .collect();

        for entry in root_entries {
            let name = entry.file_name().unwrap().to_string_lossy();
            if name.starts_with('.') && name.contains("tmp_export") {
                panic!("Fichier temporaire d'export non nettoyé : {name}");
            }
            if name.contains("tmp_export-wal") || name.contains("tmp_export-shm") {
                panic!("Fichier auxiliaire WAL/SHM temporaire résiduel : {name}");
            }
        }

        // Nettoyage
        let _ = std::fs::remove_dir_all(&test_root);
    }
}
