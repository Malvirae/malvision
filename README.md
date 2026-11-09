# Malvision

Application native **Windows 11 x64** de gestion de connexions distantes, écrite en **Rust** avec `windows-rs` (Win32 natif + COM/ActiveX + SQLite embarqué + DPAPI).

## Vision et fonctionnalités (Jalons 1 à 3)

- **Intégration externe native d'AnyDesk et RustDesk (Jalons 3 et 3B)** :
  - **Gestion centralisée dans Paramètres > Applications externes** :
    - Chemins configurables vers `AnyDesk.exe` et `rustdesk.exe`.
    - Bouton **Parcourir...** exploitant le sélecteur standard moderne Windows (`IFileOpenDialog` avec filtre `*.exe`).
    - Bouton **Détecter** scannant automatiquement le Registre Windows (`HKLM`/`HKCU` `App Paths`) et les dossiers standards (`Program Files`, `Program Files (x86)`, `APPDATA`, `LOCALAPPDATA`).
    - Bouton **Tester** exécutant l'application pour valider le binaire en toute sécurité.
    - Persistance SQLite dans la table `settings` (schéma v3).
  - **Formulaire de connexion adaptatif au protocole (Jalon 3B)** :
    - Le dialogue de création/édition adapte instantanément ses champs selon le protocole sélectionné (`RDP`, `AnyDesk`, `RustDesk`).
    - Pour AnyDesk et RustDesk : masquage propre des champs spécifiques RDP (Port, Utilisateur, Domaine, Résolution dynamique, Presse-papiers), élargissement du champ Identifiant distant sur toute la largeur, adaptation des libellés de mot de passe et affichage de notes contextuelles.
    - Préservation totale des valeurs saisies lors de basculements successifs entre protocoles.
  - **Lancement sécurisé des sessions distantes et gestion des secrets (Jalon 3B)** :
    - Exécution exclusive via l'API Win32 native `CreateProcessW` sans intermédiaire shell (`cmd.exe` ou PowerShell), protégeant contre toute injection d'arguments.
    - Assainissement strict des identifiants distants (`[a-zA-Z0-9_\-\.@\s]`), rejetant immédiatement tout caractère de contrôle, guillemet ou redirection.
    - **AnyDesk** : transmission sécurisée du mot de passe d'accès sans surveillance via un tube anonyme Win32 standard input (`--with-password`), sans mot de passe en ligne de commande ni dans les logs, avec fermeture immédiate du tube (`EOF`) et effacement mémoire (`write_volatile`).
    - **RustDesk** : lancement direct `--connect "<id>"`. L'audit CLI confirmant l'absence de transmission de secret distant sur stdin/CLI, le mot de passe est conservé en coffre DPAPI pour référence de l'utilisateur avec mention explicite dans l'interface que l'authentification reste gérée par le client RustDesk.
    - Aucun faux onglet RDP n'est ouvert pour les outils tiers.
- **RDP Avancé — Plein écran natif et actions de session (Jalons 3 et 3B)** :
  - **Plein écran RDP natif ActiveX (`IMsRdpClient9::put_FullScreen`)** :
    - Mode plein écran natif (`ContainerHandledFullScreen = 0`) avec barre de connexion supérieure (`DisplayConnectionBar = true`, `put_ConnectionBarText`).
    - Synchronisation d'état via les événements ActiveX `OnEnterFullScreenMode` et `OnLeaveFullScreenMode`.
    - Sortie fiable en plein écran via le raccourci Windows natif `Ctrl+Alt+Pause` ou la barre de connexion.
  - **Envoi précis des combinaisons spéciales (`IMsRdpClientNonScriptable::SendKeys`)** :
    - Conversion des touches en scan codes matériels PS/2 réels et encodage conforme des entiers 32 bits `lParam` (touches étendues, contexte Alt, transition up/down).
    - Envoi fonctionnel de **Ctrl + Alt + Fin** (séquence SAS officielle de session distante RDP) et de **Ctrl + Alt + Suppr** selon les stratégies système Windows.
- **RDP Avancé — Raccourcis Windows (`KeyboardHookMode`)** :
  - Propriété COM Microsoft `IMsRdpClientSecuredSettings2::put_KeyboardHookMode` configurée avant connexion :
    - `0` : Raccourcis exécutés sur le poste local.
    - `1` : Raccourcis envoyés à la session distante (**défaut recommandé**).
    - `2` : Raccourcis envoyés à distance uniquement en plein écran.
  - Fonctionnement vérifié avec `Win+R`, `Win+E`, `Win+D`, `Alt+Tab`, `Alt+F4`, `Ctrl+C` et `Ctrl+V`.
- **RDP Avancé — Menu d'actions de session active** :
  - Menu contextuel accessible via le bouton d'action `⚡` sur l'onglet actif RDP ainsi que par clic droit sur l'onglet :
    - 🔒 **Envoyer Ctrl + Alt + Suppr** : injection via `SendKeys` avec scan codes PS/2.
    - 🛡️ **Envoyer Ctrl + Alt + Fin** : séquence SAS distante RDP officielle via `SendKeys`.
    - 🔲 **Basculer / Quitter le mode plein écran** : pilotage COM officiel avec barre de connexion.
    - ❌ **Fermer / Déconnecter la session** : fermeture propre avec confirmation et libération ActiveX.
- **RDP Avancé — Redirection presse-papiers et copie de fichiers** :
  - Redirection du presse-papiers texte/images (`RedirectClipboard`).
  - Activation de la redirection de disques (`IMsRdpClientAdvancedSettings8::put_RedirectDrives`) liée au partage du presse-papiers, permettant la copie/collage natif de fichiers entre l'hôte et la session distante via l'Explorateur Windows.
- **Thèmes visuels dynamiques Windows 11 (Sombre / Clair / Système)** :
  - **Thème sombre** par défaut : anthracite profond (`#16181B`), barre de titre sombre DWM (`DWMWA_USE_IMMERSIVE_DARK_MODE`), typographie `Segoe UI`, rendu GDI double-bufferisé sans scintillement et prise en charge HiDPI Per-Monitor V2 (`DpiMetrics` 100 % / 125 % / 150 %).
  - **Thème clair** haute lisibilité : fond blanc pur (`#FFFFFF`), panneau latéral gris-bleu Fluent (`#F3F5F9`), bordures douces et contrastes optimisés, sans inversion brutale.
  - **Thème système** : synchronisation automatique et dynamique avec les préférences Windows 11 (`AppsUseLightTheme`).
  - **Finition de rendu (Jalon 3B)** : traitement de `WM_CTLCOLORSTATIC` sur le champ de recherche pour éliminer tout artefact lors du basculement dynamique de thème.
- **Centre de Paramètres complet & persistant** :
  - Bouton **Paramètres** avec icône vectorielle d'engrenage au bas du volet latéral gauche.
  - Fenêtre de réglages dédiée articulée en 5 sections :
    - **Apparence** : sélection du mode de thème (Sombre / Clair / Système).
    - **Comportement & System Tray** : fermeture vers la zone de notification, démarrage minimisé, confirmation avant fermeture en présence de sessions actives, restauration des sessions RDP au lancement.
    - **Démarrage avec Windows** : activation/désactivation immédiate d'un raccourci utilisateur `.lnk` (`Malvision.lnk`) dans le dossier Startup (`FOLDERID_Startup`, `IShellLinkW` + `IPersistFile`), sans droits administrateur, avec détection de déplacement et réparation automatique de la cible vers l'exécutable réel.
    - **Accueil** : affichage optionnel et nombre maximal paramétrable (1 à 50, défaut 10) des connexions récentes.
    - **Options RDP par défaut** : presse-papiers, SmartSizing et mode des raccourcis Windows (`KeyboardHookMode`).
    - **Applications externes** : configuration, détection et tests d'exécutables pour AnyDesk et RustDesk.
- **Intégration Windows System Tray (`Shell_NotifyIconW`)** :
  - Icône Malvision générée dynamiquement en mémoire (aucune dépendance externe `.ico`).
  - Double-clic pour restaurer et amener Malvision au premier plan.
  - Prise en charge du message `TaskbarCreated` pour restaurer l'icône lors du redémarrage d'Explorer.
  - Masquage transparent de la fenêtre lors du clic sur la croix `[X]` si activé, conservant actives les sessions distantes.
- **Cycle de vie et qualité d'expérience RDP** :
  - **Fermeture automatique des onglets déconnectés** : déconnexion volontaire, fermeture de session Windows distante ou rupture réseau confirmée ferment immédiatement l'onglet, libèrent proprement les ressources ActiveX et reviennent à un autre onglet ou à l'Accueil.
  - **Maintien pendant la reconnexion automatique** : aucune fermeture intempestive en phase de reconnexion réseau (`AutoReconnecting`).
  - **Sécurité COM et anti-réentrance** : désactivation atomique des récepteurs d'événements COM (`deactivate_events`) avant libération des contrôles ActiveX, évitant tout crash ou double libération.
  - **Sélection automatique du port RDP** : au focus souris ou clavier sur le champ Port (`3389` par défaut), sélection intégrale du texte (`EM_SETSEL(0, -1)`), permettant une saisie immédiate (ex. `42666`) sans devoir effacer au préalable.
- **Gestionnaire d'arborescence et multi-sessions** :
  - Panneau latéral redimensionnable avec recherche instantanée, favoris, arborescence `TreeView` multi-niveaux à rendu propriétaire `NM_CUSTOMDRAW`, badges de protocoles vectoriels (`RDP`, `ANY`, `RST`) et glisser-déposer anti-cycle.
  - Sessions RDP multiples fonctionnant simultanément dans des onglets dédiés.
  - Chiffrement matériel local des mots de passe par Windows DPAPI (`CryptProtectData`).
- **Distribution portable** : un unique exécutable `Malvision.exe` autonome d'environ 2,4 Mo, sans runtime web lourd (ni Electron ni WebView2).

## Architecture technique

- [`src/main.rs`](src/main.rs) : Point d'entrée Windows (`windows_subsystem = "windows"`), activation HiDPI Per-Monitor V2, initialisation COM/OLE STA (`OleInitialize`), contrôles communs et boucle principale.
- [`src/external_apps.rs`](src/external_apps.rs) : Module complet d'intégration pour AnyDesk et RustDesk (détection via Registre et dossiers standards, validation de chemins, assainissement strict des ID distants, génération de lignes de commande UTF-16 sécurisées, création de processus via `CreateProcessW` sans shell, sélecteur de fichier Win32 `IFileOpenDialog`).
- [`src/models.rs`](src/models.rs) : Modèles de données (`Protocol`, `Folder`, `ConnectionItem`, `ConnectionDraft`, `RecentConnectionEntry`) et validation stricte des entrées.
- [`src/security.rs`](src/security.rs) : Chiffrement/déchiffrement des mots de passe via Windows DPAPI (`CryptProtectData` / `CryptUnprotectData`) et conteneur mémoire sécurisé `SecretString` à effacement volatil sur `Drop`.
- [`src/storage.rs`](src/storage.rs) : Persistance locale SQLite embarquée (`malvision.db`) avec schéma v3, tables `folders`, `connections`, `recent_connections`, `settings`, journalisation WAL et intégrité référentielle.
- [`src/settings.rs`](src/settings.rs) : Modèle et persistance de l'ensemble des réglages applicatifs (chemins AnyDesk/RustDesk, `RdpKeyboardHookMode`, System Tray, thèmes, Startup shortcut).
- [`src/rdp.rs`](src/rdp.rs) : Hébergement ActiveX natif (`AtlAxWin`), interfaces Microsoft RDP `IMsRdpClient9`, `IMsRdpClientAdvancedSettings8`, `IMsRdpClientSecuredSettings2`, `IMsRdpClientNonScriptable` (`SendKeys` pour Ctrl+Alt+Del et Ctrl+Alt+Fin, `put_FullScreen`, `put_KeyboardHookMode`, `put_RedirectDrives`, `put_RedirectClipboard`), sink d'événements COM avec désactivation atomique.
- [`src/sessions.rs`](src/sessions.rs) : Gestionnaire multi-sessions (`SessionManager`, `RdpSessionTab`) orchestrant l'ouverture, la commutation d'onglets, la transmission des raccourcis spéciaux de session et la fermeture automatique des onglets déconnectés.
- [`src/ui.rs`](src/ui.rs) : Interface graphique Win32/GDI avec thématisation dynamique intégrale (Sombre / Clair / Système), arborescence `TreeView`, barre d'onglets avec bouton d'action contextuel `⚡`, menu de session popup, boîte de dialogue des Paramètres (avec sections RDP et Applications externes) et dialogue modal moderne.

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
