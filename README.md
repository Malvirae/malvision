# Malvision

Application native **Windows 11 x64** de gestion de connexions distantes, écrite en **Rust** avec `windows-rs` (Win32 natif + COM/ActiveX + SQLite embarqué + DPAPI).

## Vision et fonctionnalités (Jalons 2, 2B & 2C)

- **Thèmes visuels dynamiques Windows 11 (Sombre / Clair / Système)** :
  - **Thème sombre** par défaut : anthracite profond (`#16181B`), barre de titre sombre DWM (`DWMWA_USE_IMMERSIVE_DARK_MODE`), typographie `Segoe UI`, rendu GDI double-bufferisé sans scintillement et prise en charge HiDPI Per-Monitor V2 (`DpiMetrics` 100 % / 125 % / 150 %).
  - **Thème clair** haute lisibilité : fond blanc pur (`#FFFFFF`), panneau latéral gris-bleu Fluent (`#F3F5F9`), bordures douces et contrastes optimisés, sans inversion brutale.
  - **Thème système** : synchronisation automatique et dynamique avec les préférences Windows 11 (`AppsUseLightTheme`).
  - Application immédiate du thème sans redémarrage requis sur l'ensemble de l'interface (panneau principal, TreeView, dialogues, formulaires, menus).
- **Centre de Paramètres complet & persistant** :
  - Bouton **Paramètres** élégant avec icône vectorielle d'engrenage au bas du volet latéral gauche.
  - Fenêtre de réglages dédiée articulée en 4 sections :
    - **Apparence** : sélection du mode de thème (Sombre / Clair / Système).
    - **Comportement & System Tray** : fermeture vers la zone de notification, démarrage minimisé, confirmation avant fermeture en présence de sessions actives, restauration des sessions RDP au lancement.
    - **Démarrage avec Windows** : activation/désactivation immédiate d'un raccourci utilisateur `.lnk` (`Malvision.lnk`) dans le dossier Startup (`FOLDERID_Startup`, `IShellLinkW` + `IPersistFile`), sans droits administrateur, avec détection de déplacement et réparation automatique de la cible vers l'exécutable réel.
    - **Accueil** : affichage optionnel et nombre maximal paramétrable (1 à 50, défaut 10) des connexions récentes.
    - **Options RDP par défaut** : partage du presse-papiers et ajustement automatique de la résolution (SmartSizing).
- **Intégration Windows System Tray (`Shell_NotifyIconW`)** :
  - Véritable icône Malvision intégrée en mémoire (générée dynamiquement via GDI DIBSection sans dépendance à un fichier `.ico` externe).
  - Double-clic pour restaurer et amener Malvision au premier plan.
  - Menu contextuel standard (*Ouvrir Malvision*, *Quitter Malvision*).
  - Prise en charge du message `TaskbarCreated` pour restaurer automatiquement l'icône en cas de redémarrage d'Explorer.
  - Masquage transparent de la fenêtre lors du clic sur la croix `[X]` si l'option est activée, maintenant actives les sessions RDP en arrière-plan.
- **Cycle de vie et qualité d'expérience RDP** :
  - **Fermeture automatique des onglets déconnectés** : déconnexion volontaire, fermeture de session Windows distante ou rupture réseau confirmée ferment immédiatement l'onglet, libèrent proprement les ressources ActiveX et reviennent à un autre onglet ou à l'Accueil.
  - **Maintien pendant la reconnexion automatique** : aucune fermeture intempestive en phase de reconnexion réseau (`AutoReconnecting`).
  - **Sécurité COM et anti-réentrance** : désactivation atomique des récepteurs d'événements COM (`deactivate_events`) avant libération des contrôles ActiveX, évitant tout crash ou double libération.
  - **Sélection automatique du port RDP** : au focus souris ou clavier sur le champ Port (`3389` par défaut), sélection intégrale du texte (`EM_SETSEL(0, -1)`), permettant une saisie immédiate (ex. `42666`) sans devoir effacer au préalable.
- **Gestionnaire d'arborescence et multi-sessions** :
  - Panneau latéral redimensionnable avec recherche instantanée, favoris, arborescence `TreeView` multi-niveaux à rendu propriétaire `NM_CUSTOMDRAW`, badges de protocoles vectoriels (`RDP`, `ANY`, `RST`) et glisser-déposer anti-cycle.
  - Sessions RDP multiples fonctionnant simultanément dans des onglets dédiés.
  - Chiffrement matériel local des mots de passe par Windows DPAPI (`CryptProtectData`).
- **Distribution portable** : un unique exécutable `Malvision.exe` autonome d'environ 2,4 Mo, sans moteur web lourd (ni Electron ni WebView2).

## Architecture technique

- [`src/main.rs`](src/main.rs) : Point d'entrée Windows (`windows_subsystem = "windows"`), activation HiDPI Per-Monitor V2 (`SetProcessDpiAwarenessContext`), initialisation COM/OLE STA (`OleInitialize`), initialisation des contrôles communs (`InitCommonControlsEx`) et lancement de l'application.
- [`src/models.rs`](src/models.rs) : Modèles de données (`Protocol`, `Folder`, `ConnectionItem`, `ConnectionDraft`, `RecentConnectionEntry`) et validation stricte des entrées (hôte/IP/ID, plage de ports `1..=65535`, normalisation `DOMAINE\utilisateur`).
- [`src/security.rs`](src/security.rs) : Chiffrement et déchiffrement des mots de passe via **Windows DPAPI** (`CryptProtectData` / `CryptUnprotectData` sous contexte utilisateur courant avec entropie applicative dédiée) et conteneur `SecretString` avec effacement mémoire volatil (`write_volatile`) automatique sur `Drop`.
- [`src/storage.rs`](src/storage.rs) : Persistance locale **SQLite** embarquée (`rusqlite` avec fonctionnalité `bundled`) stockée dans `%LOCALAPPDATA%\Malvision\malvision.db` :
  - Tables : `folders`, `connections`, `recent_connections`, `settings`.
  - Migrations de schéma versionnées (`PRAGMA user_version = 2`), initialisation des préférences utilisateur, intégrité référentielle (`PRAGMA foreign_keys = ON`), journalisation `WAL`, vérification d'intégrité à l'ouverture (`PRAGMA quick_check`) sans écrasement silencieux en cas de corruption.
  - Détection et blocage des cycles hiérarchiques de dossiers (`would_create_folder_cycle`).
- [`src/settings.rs`](src/settings.rs) : Modèle et persistance de toutes les préférences de l'application (largeur du volet, mode de thème, comportement du System Tray, confirmation de fermeture, restauration de session, intégration Startup Windows via `IShellLinkW`, nombre de connexions récentes, options RDP par défaut).
- [`src/rdp.rs`](src/rdp.rs) : Hébergement ActiveX natif via `atl.dll` (`AtlAxWinInit` / `AtlAxWin` / `AtlAxGetControl`), configuration `IMsRdpClient9` / `IMsRdpClientAdvancedSettings8` / `IMsRdpClientNonScriptable3` (port personnalisé, NLA/CredSSP, résolution dynamique `SmartSizing` + `UpdateSessionDisplaySettings`, partage du presse-papiers `RedirectClipboard`), et sink d'événements COM `IMsTscAxEvents` (`RdpEventSinkComObject`) avec désactivation atomique `deactivate_events` prévenant toute réentrance à la fermeture.
- [`src/sessions.rs`](src/sessions.rs) : Gestionnaire multi-sessions (`SessionManager`, `RdpSessionTab`) orchestrant l'ouverture, la commutation d'onglets sans destruction des sessions inactives, le redimensionnement, la supervision périodique de l'état réel (`get_Connected`), la fermeture automatique des onglets déconnectés et la libération propre des ressources COM (`Disconnect` + `Unadvise` + `DestroyWindow`).
- [`src/ui.rs`](src/ui.rs) : Interface graphique Win32/GDI avec thématisation dynamique intégrale (Sombre / Clair / Système), arborescence `TreeView` avec glisser-déposer, recherche dynamique, tableau de bord d'accueil, barre d'onglets multi-sessions, boîte de dialogue des Paramètres, intégration System Tray (`Shell_NotifyIconW`), sous-classement pour la sélection automatique du port (`EM_SETSEL`) et boîtes de dialogue modales sécurisées.

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
