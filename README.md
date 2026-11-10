# Malvision

Application native **Windows 11 x64** de gestion de connexions distantes, écrite en **Rust** avec `windows-rs` (Win32 natif + COM/ActiveX + SQLite embarqué + DPAPI).

**Version actuelle :** `1.0.1` (V1.0.1 Stable Portable)

---

## Fonctionnalités principales (V1.0.1)

- **Instance unique avec réactivation intelligente (V1.0.1)** :
  - Mutex Win32 nommé à portée de session (`Local\Malvision_SingleInstance_Mutex`).
  - Toute tentative de seconde instance réactive immédiatement l'instance déjà ouverte : restauration depuis l'état minimisé ou le System Tray, mise au premier plan et focus garanti (`AllowSetForegroundWindow`).
  - Le processus secondaire s'arrête instantanément sans ouvrir de base SQLite, sans créer de fenêtre ni dupliquer d'icône dans la zone de notification.
  - Libération contrôlée lors des redémarrages volontaires de l'application.
- **Emplacement personnalisé de la base de données SQLite (V1.0.1)** :
  - Nouvel onglet **Avancé** dans le dialogue des Paramètres (6 catégories au total).
  - Déplacement assisté de `malvision.db` vers n'importe quel dossier choisi par l'utilisateur via un sélecteur moderne `IFileOpenDialog`.
  - Boutons d'accès direct « Ouvrir le dossier » dans l'Explorateur et « Revenir au dossier par défaut ».
  - Détection et avertissement clair pour les disques amovibles (`DRIVE_REMOVABLE`) et partages réseau (`DRIVE_REMOTE`).
  - Sauvegarde sécurisée sans suppression ni renommage de l'ancienne base : copie intègre avec `Storage::export_database()` et checkpoint WAL complet.
  - Persistance dans `%LOCALAPPDATA%\Malvision\bootstrap.json`.
  - Résilience au démarrage : dialogue interactif en cas de disque déconnecté (Réessayer, Choisir un autre dossier, Revenir au défaut, Quitter).
- **Tri perfectionné de l'arborescence des connexions (V1.0.1)** :
  - Règle stricte et intuitive : **dans chaque dossier et à la racine, les connexions directes apparaissent toujours avant les sous-dossiers**.
  - Tri alphabétique insensible à la casse (A à Z) appliqué récursivement.
  - Préservation intégrale des favoris, icônes de protocoles (RDP, AnyDesk, RustDesk) et de l'état développé/replié des dossiers.
- **En-tête & Identité : Badge de version dynamique (V1.0.1)** :
  - Remplacement du badge `Win11` par le badge de version dynamique `v1.0.1` à droite du titre « Malvision ».
  - Dimensionnement dynamique sans risque de troncature, cohérent sur tous les thèmes et facteurs d'échelle DPI.
- **Tableau de bord : « Toutes les connexions » & Défilement fluide (V1.0.1)** :
  - Section complète affichant l'intégralité des connexions sous forme de cartes compactes Fluent (`hauteur = 56px`, largeur 210 à 250px) en grille adaptative (4 à 5 cartes par ligne sur écran large sans étirement excessif).
  - Badges de protocoles strictement uniformisés avec l'arborescence : `RDP` (bleu), `ANY` (rouge), `RST` (turquoise).
  - Informations synthétiques : nom en gras avec gestion des noms longs par ellipsis, étoile dorée de favori, chemin du dossier et hôte / identifiant distant.
  - Défilement vertical fluide à la molette (`WM_MOUSEWHEEL`) sur toute la page d'accueil avec hit-testing précis des cartes.
  - **Repositionnement dynamique intelligent** :
    - *Cas A (Connexions récentes activées)* : enchaînement harmonieux Actions rapides -> Favoris -> Récentes -> Toutes les connexions.
    - *Cas B (Connexions récentes désactivées)* : section récente complètement masquée sans espace résiduel ; la section « Toutes les connexions » remonte directement sous les favoris / actions rapides.
  - Lancement immédiat par clic sur n'importe quelle carte.
- **Centre de Paramètres moderne à 6 catégories (V1.0.1)** :
  - Volet de navigation latéral style Fluent avec 6 onglets dédiés :
    1. **Général** : sélection du thème (Sombre, Clair, Système), comportement System Tray, démarrage avec Windows, connexions récentes.
    2. **Protocoles** : chemins d'accès vers AnyDesk et RustDesk (avec détection automatique et test), options RDP par défaut (Presse-papiers, Dynamic Resolution, KeyboardHookMode).
    3. **Données** : exportation transactionnelle de la configuration SQLite (compatible WAL, préservation DPAPI), importation avec contrôle d'intégrité et sauvegarde de sécurité `.pre_import_bak`.
    4. **Mises à jour** : vérification automatique ou manuelle via l'API GitHub Releases, téléchargement sécurisé avec vérification SHA-256 et instructions de remplacement portable.
    5. **Avancé** : gestion de l'emplacement personnalisé de la base de données SQLite `malvision.db`.
    6. **À propos** : logo officiel, version installée, description, lien direct vers le dépôt GitHub et raccourci de vérification des versions.
- **Sauvegarde et Restauration SQLite (Jalon 4)** :
  - **Exportation WAL-safe** : utilisation de l'API de sauvegarde en ligne SQLite (`rusqlite::backup::Backup`), sans verrouillage ni déconnexion des sessions RDP actives.
  - **Protection DPAPI** : conservation intégrale des mots de passe chiffrés par Windows DPAPI. Avertissement explicite sur la récupération liée au compte Windows.
  - **Importation sécurisée** : contrôle strict d'intégrité SQLite (`PRAGMA integrity_check`), validation du schéma de tables, sauvegarde automatique préalable de la base active (`.pre_import_bak`), test non destructif de déchiffrement DPAPI et actualisation instantanée de l'interface.
- **Vérification et mise à jour GitHub Releases (Jalon 4)** :
  - Interrogation asynchrone non bloquante de l'API publique GitHub (`GET /repos/Malvirae/malvision/releases/latest`) via WinHTTP natif en HTTPS.
  - Comparaison sémantique SemVer (`CURRENT = 1.0.0`), exclusion des pré-versions (`prerelease`) et brouillons (`draft`).
  - Téléchargement sécurisé de l'asset `Malvision.exe` avec contrôle strict de l'empreinte SHA-256 officielle (extraite de `SHA256SUMS.txt` ou du descriptif de release).
- **Double-clic pleine largeur sur tous les protocoles (Jalons 3C & 3D)** :
  - Lancement immédiat de n'importe quelle connexion par double-clic sur l'icône, le libellé ou l'espace vide à droite du texte (`tree_hit_test_full_row`), pour les sessions RDP (onglet intégré), AnyDesk (avec transmission DPAPI sur stdin si configuré) et RustDesk.
  - Expansion / repli fiable des dossiers sans déclenchement intempestif de connexion.
- **Thématisation dynamique sans résidu (Jalons 2C, 3C & 3D)** :
  - Basculement instantané Sombre <-> Clair <-> Système sans redémarrage, y compris en prévisualisation directe dans le dialogue Paramètres.
  - Champ de recherche reconstruit dynamiquement éliminant tout cache UxTheme/DWM persistant sur Windows 11.
- **Intégration externe native d'AnyDesk et RustDesk (Jalons 3, 3B et 3C)** :
  - Détection automatique dans le Registre Windows (`HKLM`/`HKCU` `App Paths`) et les dossiers standards (`Program Files`, `Program Files (x86)`, `APPDATA`, `LOCALAPPDATA`).
  - Formulaire de connexion adaptatif masquant automatiquement les champs non applicables (Port, Utilisateur, Domaine pour AnyDesk/RustDesk).
  - Lancement via `CreateProcessW` natif Win32 sans shell, assainissement strict des ID distants, transmission sécurisée du mot de passe AnyDesk par tube anonyme standard input (`--with-password`).
- **RDP Avancé — Plein écran natif, raccourcis et actions de session (Jalons 3 et 3B)** :
  - Plein écran officiel ActiveX Microsoft (`IMsRdpClient9::put_FullScreen`) avec barre de connexion supérieure rétractable.
  - Envoi précis des combinaisons spéciales (`SendKeys` matériel PS/2) : **Ctrl + Alt + Fin** (SAS RDP) et **Ctrl + Alt + Suppr**.
  - Raccourcis Windows (`KeyboardHookMode`) paramétrables (Sur le poste distant, En plein écran uniquement, Sur le poste local).
  - Redirection du presse-papiers et copie/collage natif de fichiers entre l'hôte et la session distante via la redirection de disques.
  - Fermeture automatique des onglets déconnectés sans affecter les sessions en cours de reconnexion automatique.
- **Distribution 100 % portable** : un unique exécutable `Malvision.exe` autonome d'environ 5 Mo, sans installation obligatoire, sans dépendances externes lourdes (ni Electron ni WebView2).
  - Lancement immédiat et fluide de n'importe quelle connexion par double-clic sur l'icône, le libellé ou l'espace vide à droite du texte (`tree_hit_test_full_row`), pour les sessions RDP (onglet intégré), AnyDesk (avec transmission DPAPI sur stdin si configuré) et RustDesk.
  - Résolution robuste par hit-test de rangée (`TVHT_ONITEM`, `TVHT_ONITEMRIGHT`, `TVHT_ONITEMINDENT` et sondage horizontal scanline) sans dépendre de l'état de sélection antérieur.
  - Le double-clic sur un dossier ou une racine assure son développement/repli sans lancer de fausse connexion, et les boutons d'expansion `[+]`/`[-]` fonctionnent sans interférence.
- **Thématisation dynamique sans résidu (Jalons 2C, 3C & 3D)** :
  - Basculement instantané Sombre <-> Clair <-> Système sans redémarrage, y compris en prévisualisation directe dans le dialogue Paramètres.
  - Champ de recherche reconstruit dynamiquement via `recreate_search_edit()` éliminant tout cache UxTheme/DWM persistant sur Windows 11 tout en préservant le texte, la sélection, le focus et le cue banner.
- **Intégration externe native d'AnyDesk et RustDesk (Jalons 3, 3B et 3C)** :
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
  - **Envoi précis des combinaisons spéciales (`IMsRdpClientNonScriptable::SendKeys`, Jalon 3C)** :
    - Injection directe des scan codes matériels PS/2 réels (`0x1D` Ctrl, `0x38` Alt, `0x4F` Fin, `0x53` Suppr) et gestion précise des transitions key-down / key-up conforme au modèle de référence Microsoft RDCMan.
    - Focus Win32 et OLE systématique garantissant la réception des touches par la session distante active.
    - Séquence officielle **Ctrl + Alt + Fin** (SAS RDP) recommandée pour accéder à l'écran de sécurité distant, et **Ctrl + Alt + Suppr** avec rétroaction d'état et dialogue explicatif d'erreur.
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
  - **Finition de rendu et zéro résidu (Jalon 3C)** : sous-classement dédié `search_edit_wnd_proc` interceptant `WM_ERASEBKGND`, neutralisation des styles UxTheme avec `SetWindowTheme`, mode de fond transparent et rafraîchissement complet `RedrawWindow` garantissant des transitions impeccables (Sombre <-> Clair) sans redémarrage.
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
