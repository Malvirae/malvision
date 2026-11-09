# Malvision

Application native **Windows 11 x64** de gestion de connexions distantes, écrite en **Rust** avec `windows-rs` (Win32 natif + COM/ActiveX + SQLite embarqué + DPAPI).

## Vision et fonctionnalités (Jalon 2)

- **Interface sombre moderne Windows 11** :
  - Thème anthracite profond (`#16181B`), barre de titre sombre DWM (`DWMWA_USE_IMMERSIVE_DARK_MODE`), typographie `Segoe UI`, rendu GDI double-bufferisé sans scintillement et prise en charge HiDPI Per-Monitor V2.
  - **Panneau latéral gauche redimensionnable** (260 px par défaut, largeur persistée entre `200 px` et `520 px`) comprenant l'en-tête Malvision, les boutons **+ Connexion** et **+ Dossier**, la recherche instantanée en temps réel, la section **Favoris** et l'arborescence multi-niveaux des dossiers et connexions.
  - **Arborescence hiérarchique (`TreeView`)** à profondeur arbitraire : développer/replier avec persistance de l'état, sélection, double-clic ou touche `Entrée` pour lancer une session, renommage rapide (`F2`), menu contextuel complet (*Se connecter*, *Modifier*, *Renommer*, *Dupliquer*, *Ajouter/Retirer des favoris*, *Déplacer vers un dossier*, *Supprimer avec confirmation*) et **glisser-déposer (Drag & Drop)** avec prévention stricte des cycles de dossiers.
  - **Zone centrale multi-sessions et tableau de bord** :
    - Lorsqu'aucune session RDP n'est active : écran d'accueil sobre présentant les accès rapides, les cartes de **Connexions favorites** et les **Connexions récemment utilisées**.
    - Lorsqu'une ou plusieurs sessions RDP sont ouvertes : **barre d'onglets** avec indicateur visuel d'état, sélection d'onglet, bouton de fermeture individuel (`×`) et bouton d'ouverture rapide (`+`).
    - Chaque onglet RDP possède son propre contrôle ActiveX `MsTscAx` et son propre cycle de vie COM (`RdpActiveXHost`). Changer d'onglet masque/affiche les hôtes (`SW_HIDE` / `SW_SHOW`) **sans détruire** les sessions inactives.
  - **Barre d'état inférieure** : affiche le statut réel de la session ou de la sélection, le protocole et l'hôte (`IP/DNS:port`), sans jamais exposer de données sensibles.
- **Protocoles supportés** :
  - **RDP** : session intégrée directement dans la fenêtre Malvision via `MsTscAx.dll` (`IMsRdpClient9`, `IMsRdpClientAdvancedSettings8`, `IMsRdpClientNonScriptable3`, `IMsTscAxEvents`).
  - **AnyDesk** : enregistrement, organisation et formulaire dédiés (lancement externe prévu au Jalon 3).
  - **RustDesk** : enregistrement, organisation et formulaire dédiés (lancement externe prévu au Jalon 3).
- **Distribution portable** : un unique exécutable `Malvision.exe` autonome, sans moteur web embarqué et sans aucune DLL tierce à distribuer.

## Architecture technique

- [`src/main.rs`](src/main.rs) : Point d'entrée Windows (`windows_subsystem = "windows"`), activation HiDPI Per-Monitor V2 (`SetProcessDpiAwarenessContext`), initialisation COM/OLE STA (`OleInitialize`), initialisation des contrôles communs (`InitCommonControlsEx`) et lancement de l'application.
- [`src/models.rs`](src/models.rs) : Modèles de données (`Protocol`, `Folder`, `ConnectionItem`, `ConnectionDraft`, `RecentConnectionEntry`) et validation stricte des entrées (hôte/IP/ID, plage de ports `1..=65535`, normalisation `DOMAINE\utilisateur`).
- [`src/security.rs`](src/security.rs) : Chiffrement et déchiffrement des mots de passe via **Windows DPAPI** (`CryptProtectData` / `CryptUnprotectData` sous contexte utilisateur courant avec entropie applicative dédiée) et conteneur `SecretString` avec effacement mémoire volatil (`write_volatile`) automatique sur `Drop`.
- [`src/storage.rs`](src/storage.rs) : Persistance locale **SQLite** embarquée (`rusqlite` avec fonctionnalité `bundled`) stockée dans `%LOCALAPPDATA%\Malvision\malvision.db` :
  - Tables : `folders`, `connections`, `recent_connections`, `settings`.
  - Migrations de schéma versionnées (`PRAGMA user_version = 1`), intégrité référentielle (`PRAGMA foreign_keys = ON`), journalisation `WAL`, vérification d'intégrité à l'ouverture (`PRAGMA quick_check`) sans écrasement silencieux en cas de corruption.
  - Détection et blocage des cycles hiérarchiques de dossiers (`would_create_folder_cycle`).
- [`src/settings.rs`](src/settings.rs) : Chargement et sauvegarde des préférences d'interface (largeur du panneau gauche).
- [`src/rdp.rs`](src/rdp.rs) : Hébergement ActiveX natif via `atl.dll` (`AtlAxWinInit` / `AtlAxWin` / `AtlAxGetControl`), configuration `IMsRdpClient9` / `IMsRdpClientAdvancedSettings8` / `IMsRdpClientNonScriptable3` (port personnalisé, NLA/CredSSP, résolution dynamique `SmartSizing` + `UpdateSessionDisplaySettings`, partage du presse-papiers `RedirectClipboard`), et sink d'événements COM `IMsTscAxEvents` (`RdpEventSinkComObject`) identifiant chaque onglet par `session_id`.
- [`src/sessions.rs`](src/sessions.rs) : Gestionnaire multi-sessions (`SessionManager`, `RdpSessionTab`) orchestrant l'ouverture, la commutation d'onglets sans destruction des sessions inactives, le redimensionnement, la supervision périodique de l'état réel (`get_Connected`) et la fermeture propre (`Disconnect` + `Unadvise` + `DestroyWindow`).
- [`src/ui.rs`](src/ui.rs) : Interface graphique sombre Win32/GDI, arborescence `TreeView` avec glisser-déposer, recherche dynamique, tableau de bord d'accueil, barre d'onglets multi-sessions et boîtes de dialogue modales (création/édition de connexion, création/renommage de dossier, déplacement).

## Stockage local et sécurité DPAPI

- **Emplacement de la base SQLite** : `%LOCALAPPDATA%\Malvision\malvision.db`
- **Protection des secrets (DPAPI)** :
  - Lorsqu'un mot de passe est enregistré avec l'option *« Enregistrer le mot de passe (chiffré via Windows DPAPI) »*, il est chiffré par `CryptProtectData` avec entropie applicative avant d'être écrit sous forme de `BLOB` dans la colonne `encrypted_password`.
  - Aucun mot de passe en clair n'est jamais stocké sur disque, ni affiché dans l'interface ou les journaux.
  - Si l'utilisateur décoche l'enregistrement du mot de passe lors de l'édition d'une connexion, le secret chiffré est immédiatement purgé de la base SQLite (`encrypted_password = NULL`).
  - **Note de portabilité DPAPI** : Les secrets chiffrés via Windows DPAPI sont liés au compte utilisateur Windows courant (et par défaut à la machine locale). Le fichier `malvision.db` copié vers une autre machine ou un autre compte utilisateur conservera toute l'arborescence et les paramètres de connexion, mais les mots de passe devront être resaisis.

## Compilation et validation

```powershell
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo build --release
```

Binaire généré : `target/release/Malvision.exe`
