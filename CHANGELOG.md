# Changelog — Malvision

Toutes les évolutions notables du projet **Malvision** sont documentées dans ce fichier.

## [Jalon 2C] — Finitions UX, Thèmes Sombre/Clair/Système, Paramètres persistants, System Tray et gestion des sessions RDP

### Ajouté
- **Sélection automatique du port RDP (`EM_SETSEL(0, -1)`)** :
  - Sous-classement du champ d'édition Port dans le formulaire de connexion.
  - À la réception du focus clavier (`Tab`, `Shift+Tab`) ou du clic souris, l'intégralité du port par défaut (`3389`) est automatiquement sélectionnée.
  - Saisie directe de n'importe quel port personnalisé (ex. `42666`) sans devoir effacer manuellement, sans désélection intempestive en cours de frappe, avec validation `1..=65535`.
- **Fermeture automatique des onglets RDP réellement déconnectés** :
  - Déconnexion volontaire, fermeture de session Windows distante ou rupture réseau confirmée entraînent la fermeture automatique de l'onglet et la libération complète des ressources ActiveX associées.
  - Sélection automatique d'un autre onglet restant ou retour propre à l'onglet d'accueil lorsqu'il n'y a plus de session active.
  - Maintien intentionnel de l'onglet ouvert lors des phases de reconnexion automatique (`AutoReconnecting` / `get_Connected() == 2`).
  - Désactivation atomique des récepteurs d'événements COM (`deactivate_events`) pour éliminer tout risque de double libération ou de réentrance lors de la destruction des onglets.
- **Bouton et boîte de dialogue des Paramètres** :
  - Bouton **Paramètres** avec icône vectorielle d'engrenage au bas du volet latéral gauche.
  - Fenêtre modale des Paramètres (`MalvisionSettingsDlgClass`) organisée en 4 sections :
    - **Apparence** : Choix du thème (`Sombre`, `Clair`, `Système (suit Windows 11)`) avec aperçu immédiat sans redémarrage.
    - **Comportement & System Tray** : *Fermer vers le tray*, *Démarrer minimisé* (activable uniquement si le tray est actif), *Confirmer la fermeture avec sessions actives*, *Restaurer les sessions RDP au démarrage*.
    - **Démarrage avec Windows** : Option créant/supprimant instantanément le raccourci utilisateur `.lnk` dans le dossier Startup, avec synchronisation et réparation automatique de cible.
    - **Accueil** : Affichage et nombre maximal de connexions récentes (1 à 50, défaut 10).
    - **Options RDP par défaut** : Presse-papiers et résolution dynamique (SmartSizing).
- **Intégration Windows System Tray (`Shell_NotifyIconW`)** :
  - Icône Malvision générée dynamiquement en mémoire (aucune dépendance à un fichier `.ico` externe).
  - Double-clic restaurant la fenêtre et lui redonnant le focus.
  - Menu contextuel standard (*Ouvrir Malvision*, *Quitter Malvision*).
  - Masquage transparent vers le tray lors du clic sur la croix `[X]` lorsque l'option est activée (les sessions restent actives).
  - Prise en charge du message `TaskbarCreated` pour restaurer automatiquement l'icône lors d'un redémarrage de `explorer.exe`.
  - Demande de confirmation avant fermeture si des sessions RDP sont en cours.
- **Thème clair haute lisibilité Windows 11 Fluent** :
  - Palette complète dédiée pour le thème clair (`#FFFFFF`, `#F3F5F9`, bordures douces `#D1D5DB`, contrastes optimisés).
  - Adaptation instantanée de tous les composants graphiques : panneau principal, arborescence TreeView, barre d'onglets, tableau de bord, contrôles personnalisés (boutons, boîtes combinées, cases à cocher) et dialogues modaux.
- **Migration et persistance SQLite versionnée (schéma v2)** :
  - Migration non destructive `PRAGMA user_version = 2` initialisant et persistant les réglages dans la table `settings`.
- **Tests unitaires et d'intégration UI Win32 dédiés** :
  - `test_ui_port_field_auto_selects_all_on_focus_and_direct_typing` : validation de la sélection `EM_SETSEL` au focus clavier et souris et saisie directe de `42666`.
  - `test_ui_settings_dialog_theme_switching_and_persistence_real_win32` : validation du basculement de thème et de la persistance SQLite.
  - `test_ui_system_tray_close_to_tray_restore_confirm_active_sessions_and_explicit_exit` : validation du cycle de vie du System Tray, masquage vers le tray, restauration, confirmation de fermeture avec session active et fermeture explicite.
  - `test_auto_close_disconnected_tab_preserves_other_sessions_and_returns_to_home` : validation de la fermeture d'onglet déconnecté et préservation des autres sessions.
  - `test_startup_shortcut_lnk_create_read_repair_and_remove` : validation de la manipulation du raccourci `.lnk` via `IShellLinkW`.

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
