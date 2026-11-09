# Changelog — Malvision

Toutes les évolutions notables du projet **Malvision** sont documentées dans ce fichier.

## [Jalon 2B] — Correctifs bloquants des dialogues modaux et finition UI sombre Windows 11

### Corrigé
- **Bug critique — Dialogues modaux inutilisables en Release (`run_modal_loop`, `close_modal_dialog`)** :
  - Suppression de l'aliasing UB (`&bool` `noalias readonly` lu en parallèle d'un pointeur mutable `GWLP_USERDATA`) que LLVM optimisait en constante `true` sous `opt-level = 3` + `lto = true`.
  - Fermeture effective des fenêtres modales par `DestroyWindow(dlg_hwnd)` avec réactivation systématique (`EnableWindow(owner, true)`) et restauration du focus clavier (`SetForegroundWindow` + `SetFocus`) sur la fenêtre propriétaire **avant** destruction du dialogue.
  - Prise en charge complète des boutons (`Valider`, `Enregistrer`, `Déplacer`, `Annuler`), des touches clavier (`Entrée`, `Échap`, `Tab` / `Maj+Tab`) et de la croix système `[X]` (`WM_CLOSE`).
- **Audit visuel et homogénéisation intégrale du thème sombre** :
  - **Suppression totale des fonds blancs sur les libellés** : remplacement de tous les contrôles `STATIC` Win32 par un rendu GDI direct (`SetBkMode(TRANSPARENT)`) en double-buffering (`CreateCompatibleDC` / `CreateCompatibleBitmap`) et initialisation de `GWLP_USERDATA` avant création des contrôles enfants.
  - **Boutons sombres personnalisés (`BS_OWNERDRAW` + sous-classement)** : rendu propriétaire via `WM_DRAWITEM` (`draw_owner_button`) avec coins arrondis, états normal / survol / pressé / focus, bouton primaire bleu (`#0078D4`) et bouton secondaire anthracite (`#252930`).
  - **Listes déroulantes sombres (`CBS_OWNERDRAWFIXED | CBS_HASSTRINGS` + sous-classement)** : cadre sombre arrondi, flèche chevron vectorielle, éléments de liste personnalisés (`WM_MEASUREITEM` + `WM_DRAWITEM`) et liste déroulante sombre (`WM_CTLCOLORLISTBOX`).
  - **Cases à cocher sombres non tronquées (`BS_OWNERDRAW` + sous-classement)** : case vectorielle sombre/bleue avec coche dessinée en GDI et largeur étendue (`412 px` à 96 DPI) éliminant toute troncature horizontale.
  - **Formulaire de connexion agrandi et aéré** : dimensions clientes portées à `680 × 660` (96 DPI), mise à l'échelle dynamique HiDPI (`DpiMetrics` pour 100 %, 125 %, 150 %) et encadré d'information DPAPI dédié.
  - **Arborescence `TreeView` modernisée (`NM_CUSTOMDRAW` + icônes vectorielles GDI)** : sélection pleine largeur avec barre d'accentuation bleue à gauche, chevrons vectoriels, icônes vectorielles (dossier, étoile, couches) et badges de protocole arrondis (`RDP`, `ANY`, `RST`), sans émojis disparates.

### Ajouté
- **Tests d'intégration UI Win32 réels (`src/ui.rs`)** :
  - `test_ui_folder_dialog_create_validate_and_cancel_real_win32` : test de bout en bout (ouverture du dialogue, saisie, clic `BM_CLICK` sur `Valider`, fermeture, persistance SQLite isolée, réouverture et clic sur `Annuler`).
  - `test_ui_connection_dialog_create_validate_and_cancel_real_win32` : test de bout en bout du formulaire de connexion RDP avec chiffrement/déchiffrement DPAPI.
  - `test_ui_dialogs_close_via_escape_and_close_button_x` : test de fermeture par touche `Échap` (`VK_ESCAPE`) et par croix `[X]` (`WM_CLOSE`).
  - `test_dpi_scaling_metrics_100_125_150_percent` : vérification des métriques HiDPI à 96, 120 et 144 DPI.

## [Jalon 2] — Interface moderne, Gestionnaire de connexions multi-onglets, SQLite & DPAPI

### Ajouté
- **Interface sombre moderne Windows 11** :
  - Thème anthracite profond (`#16181B`), barre de titre sombre DWM (`DWMWA_USE_IMMERSIVE_DARK_MODE`), typographie `Segoe UI` et rendu double-bufferisé sans scintillement.
  - **Panneau latéral redimensionnable** (260 px par défaut, largeur sauvegardée dans SQLite) avec en-tête, boutons **+ Connexion** et **+ Dossier**, champ de recherche instantanée, section **Favoris** et arborescence multi-niveaux.
  - **Arborescence hiérarchique (`TreeView`)** à profondeur arbitraire : expansion/repli persistés, sélection, double-clic pour connecter, menu contextuel complet (*Se connecter*, *Modifier*, *Renommer*, *Dupliquer*, *Favoris*, *Déplacer*, *Supprimer avec confirmation*) et **glisser-déposer (Drag & Drop)** avec prévention des cycles.
  - **Écran d'accueil central** : accès rapide à la création de connexion/dossier, cartes des **Connexions favorites** et des **Connexions récemment utilisées**, badges distincts par protocole (`RDP`, `AnyDesk`, `RustDesk`).
  - **Gestionnaire multi-sessions RDP par onglets (`src/sessions.rs`)** : ouverture simultanée de plusieurs sessions RDP ayant chacune son propre contrôle ActiveX `MsTscAx` et son propre cycle de vie COM (`session_id` dédié dans les messages `WM_RDP_EVENT`), commutation d'onglets sans détruire les sessions inactives, bouton de fermeture par onglet et indicateur visuel d'état.
  - **Formulaires modaux sombres** : création/édition de connexion (Nom, Protocole RDP/AnyDesk/RustDesk, Dossier parent, Hôte/IP/ID, Port, Utilisateur, Domaine, Mot de passe chiffré DPAPI, Favori, Description, Résolution dynamique, Partage du presse-papiers), création/renommage de dossier et déplacement hiérarchique.
- **Persistance locale SQLite (`src/storage.rs`)** :
  - Base de données locale `%LOCALAPPDATA%\Malvision\malvision.db` embarquée statiquement (`rusqlite` `bundled`).
  - Tables `folders`, `connections`, `recent_connections` et `settings` avec migration versionnée (`PRAGMA user_version = 1`), clés étrangères (`ON`), mode `WAL`, contrôle d'intégrité (`PRAGMA quick_check`) et détection récursive des cycles de dossiers (`would_create_folder_cycle`).
- **Sécurité Windows DPAPI (`src/security.rs`)** :
  - Chiffrement/déchiffrement des mots de passe enregistrés via `CryptProtectData` / `CryptUnprotectData` (portée utilisateur courant + entropie applicative) et effacement mémoire volatil (`SecretString`).

## [Jalon 1B] — Corrections du statut RDP et de la gestion de fenêtre

### Corrigé
- **Anomalie 1 (Statut de connexion RDP)** : prise en charge de `DIID_IMsTscAxEvents` (`{336d5562-efa8-482e-8cb3-c5c0fc7a7db6}`) dans `RdpEventSinkComObject::QueryInterface` et ajout d'une supervision périodique de l'état réel `IMsRdpClient::get_Connected()`.
- **Anomalie 2 (Fenêtre bloquée après maximisation)** : filtrage strict des messages clavier (`is_keyboard_message`) avant `TranslateAccelerator` / `IsDialogMessageW` et préservation active du style `WS_OVERLAPPEDWINDOW`.

## [Jalon 1] — Initialisation et prototype RDP intégré

### Ajouté
- Initialisation du dépôt Rust Windows 11 x64 (`windows-rs`).
- Intégration native du contrôle ActiveX Microsoft RDP (`MsTscAx.dll` / `IMsRdpClient9`) via conteneur `AtlAxWin`, authentification NLA/CredSSP, redimensionnement dynamique et décodage des erreurs de déconnexion.
