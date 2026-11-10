# Changelog — Malvision

Toutes les évolutions notables du projet **Malvision** sont documentées dans ce fichier.

## [Jalon 3D] — Correctifs UX avant V1.0.0 (Champ de recherche sans résidu par recréation dynamique et double-clic sur toute la ligne)

### Corrigé
- **Champ de recherche sans artefact lors du basculement de thème (Sombre <-> Clair)** :
  - Identification de la cause racine sur Windows 11 réel : les styles visuels Common Controls v6 et le gestionnaire DWM/UxTheme conservent un cache interne persistant de surface/bitmap d'arrière-plan associé au HWND du contrôle `EDIT`, rendant inopérants les rafraîchissements GDI seuls.
  - Implémentation de `recreate_search_edit()` dans `apply_theme()` et lors de la prévisualisation dans la boîte de dialogue des Paramètres : destruction complète de l'ancien HWND et recréation instantanée du contrôle `EDIT` natif avec les pinceaux et couleurs du thème nouvellement résolu (`#FFFFFF` en thème clair, `#191C1F` en thème sombre).
  - Préservation intégrale et transparente du texte saisi (`get_window_text` / `set_window_text`), de la plage de sélection de texte (`EM_GETSEL` / `EM_SETSEL`), de la police GDI, du texte indicatif (`EM_SETCUEBANNER`) et de l'état de focus (`GetFocus` / `SetFocus`).
  - Alignement parfait du conteneur arrondi à bordure GDI et de l'icône de recherche, sans aucun redémarrage nécessaire.
- **Double-clic interactif sur toute la largeur de ligne dans l'arborescence (TreeView)** :
  - Implémentation de `tree_hit_test_full_row()` résolvant les limitations du `TVM_HITTEST` standard (qui retourne `NULL` lorsque le clic est situé dans l'espace vide à droite du libellé `TVHT_ONITEMRIGHT` ou sur l'indentation).
  - Résolution directe via `hti.hItem` sur les indicateurs `TVHT_ONITEM`, `TVHT_ONITEMRIGHT` et `TVHT_ONITEMINDENT`, couplée à un sondage horizontal scanline le long de la coordonnée Y pour identifier avec certitude la rangée cliquée.
  - Double-clic réactif et ergonomique sur toute la largeur de ligne utile pour les trois protocoles (`RDP`, `AnyDesk`, `RustDesk`), y compris dans les sous-dossiers, les favoris et les résultats filtrés de recherche.
  - Préservation stricte du comportement natif des dossiers : un double-clic sur un dossier ou une racine renvoie `LRESULT(0)` pour déplier/replier l'arborescence ; les clics sur les boutons d'expansion `[+]`/`[-]` (`TVHT_ONITEMBUTTON`) sont transmis sans détournement.
  - Amélioration coordonnée du clic droit contextuel (`show_tree_context_menu`) et de la détection de cible de glisser-déposer (`tree_hit_test_screen_or_client`) pour une ergonomie homogène sur toute la largeur du panneau.

### Ajouté
- Test unitaire et d'intégration `test_ui_search_edit_theming_and_ctlcolorstatic` enrichi validant la recréation avec nouveau HWND, la préservation du texte ("srv-prod-01") et de la sélection (4..8) en cycles Dark -> Light -> Dark.
- Test unitaire et d'intégration `test_ui_unified_double_click_all_protocols_and_folders` enrichi testant `tree_hit_test_full_row` sur l'icône ($X=35$), le texte ($X=80$) et l'espace vide à droite de la rangée ($X=240$), ainsi que l'absence de faux positifs en espace vide sous les éléments ($Y=500$).
- Captures réelles d'écran Release validant l'absence de tout résidu sombre en thème clair (`jalon3d_01_dark_initial.png`, `jalon3d_02_preview_light.png`, `jalon3d_03_applied_light.png`, `jalon3d_04_switched_back_dark.png`).



### Corrigé
- **Combinaisons spéciales RDP — Correction définitive des scan codes SendKeys (`IMsRdpClientNonScriptable::SendKeys`)** :
  - Audit approfondi par désassemblage de `mstscax.dll` et inspection du code de référence Microsoft RDCMan (`SendKeys.cs`) : le paramètre `plKeyData` attend directement les codes de balayage matériel PS/2 bruts (`0x1D` Ctrl, `0x38` Alt, `0x4F` Fin, `0x53` Suppr), et non un mot de 32 bits `lParam` synthétique shifté (dont le bit de répétition `1` était lu à tort comme le scan code de la touche Escape).
  - Normalisation conforme du tableau `pbArrayKeyUp` : `0` pour l'enfoncement (down), `1` pour le relâchement (up) avec séquence atomique ordonnée de 6 événements (Ctrl down, Alt down, Fin/Suppr down, Fin/Suppr up, Alt up, Ctrl up).
  - Positionnement systématique du focus Win32 (`SetFocus(self.ax_hwnd)` et fenêtre fille active `IHWindowClass`) avant injection, couplé à `IOleObject::DoVerb(OLEIVERB_UIACTIVATE)`.
  - Clarification et mise en valeur dans le menu d'actions de session : `Ctrl + Alt + Fin (Sécurité distante RDP)` présenté comme la séquence SAS officielle recommandée sur session distante, et `Ctrl + Alt + Suppr (Simulation)`. Rétroaction immédiate dans la barre d'état et boîte de dialogue d'erreur détaillée avec code HRESULT en cas d'échec COM.
- **Élimination définitive des résidus graphiques de la barre de recherche lors du changement de thème** :
  - Désactivation complète des styles visuels et caches UxTheme sur `search_edit` via `SetWindowTheme(search_edit, w!(""), w!(""))` garantissant que le contrôle d'édition natif obéit strictement aux messages GDI.
  - Sous-classement dédié `search_edit_wnd_proc` interceptant `WM_ERASEBKGND` pour peindre directement la couleur de fond dynamique `col_bg_input()` dans le rectangle client.
  - Transmission des messages `WM_THEMECHANGED` et `WM_SYSCOLORCHANGE` lors de `apply_theme()`, combinée à un rafraîchissement complet via `RedrawWindow` (`RDW_INVALIDATE | RDW_ERASE | RDW_FRAME | RDW_ALLCHILDREN | RDW_UPDATENOW`) sur le champ de recherche et la fenêtre parente.
  - Configuration de `SetBkMode(hdc, TRANSPARENT)` dans `WM_CTLCOLOREDIT` et `WM_CTLCOLORSTATIC` supprimant tout bloc opaque résiduel autour des caractères et du texte indicatif `EM_SETCUEBANNER`. Rendu vérifié parfait en transitions Sombre -> Clair -> Sombre, avec texte ou vide.
- **Double-clic unifié dans l'arborescence pour les trois protocoles (RDP, AnyDesk, RustDesk)** :
  - Remplacement de la lecture passive de `selected_node` par un hit-test Win32 explicite `TVM_HITTEST` au point du curseur lors de la notification `NM_DBLCLK`.
  - Identification immédiate du nœud cliqué via `node_kind_from_hitem` et sélection visuelle `TVM_SELECTITEM(TVGN_CARET)`.
  - Point d'entrée unique `launch_connection_by_id` : lancement transparent en onglet pour RDP, lancement d'AnyDesk avec transmission du mot de passe sur stdin (si configuré), et lancement de RustDesk.
  - Préservation du comportement natif des dossiers : retour `LRESULT(0)` lors du double-clic sur un dossier ou une racine pour développer/replier sans déclencher de connexion.
  - Correction du message de confirmation RustDesk dans la barre d'état.

### Ajouté
- Test unitaire `test_rdp_scan_codes_and_sequence_construction` validant les scan codes PS/2 bruts et la séquence atomique ordonnée de 6 touches.
- Test d'intégration `test_ui_unified_double_click_all_protocols_and_folders` validant le routage unifié du double-clic pour RDP, AnyDesk, RustDesk et le dépliage des dossiers.
- Test d'intégration complété `test_ui_search_edit_theming_and_ctlcolorstatic` validant les cycles complets de changement de thème Dark -> Light -> Dark sans résidu et l'interception de `WM_ERASEBKGND`.


## [Jalon 3B] — Correctifs pré-release V1.0.0 (Plein écran RDP, SendKeys, formulaires adaptatifs, transmission de secret AnyDesk et thème)

### Corrigé
- **Plein écran RDP natif ActiveX (`IMsRdpClient9::put_FullScreen`)** :
  - Configuration de `ContainerHandledFullScreen = 0` dans les propriétés non-scriptables pour déléguer la gestion plein écran au conteneur natif Microsoft (`TscShellContainerClass`).
  - Activation de la barre de connexion (`DisplayConnectionBar = true`, `PinConnectionBar = false`) et initialisation systématique de `IMsRdpClientNonScriptable3::put_ConnectionBarText` (offset de vtable 280) avant l'appel à `put_FullScreen`, conformément aux spécifications Microsoft.
  - Câblage des événements ActiveX `OnEnterFullScreenMode` (DISPID 5 / `RDP_EVT_ENTER_FULLSCREEN`) et `OnLeaveFullScreenMode` (DISPID 6 / `RDP_EVT_LEAVE_FULLSCREEN`) pour synchroniser fidèlement l'état UI du bouton plein écran.
  - Sortie fiable du mode plein écran par le raccourci natif Windows `Ctrl+Alt+Pause` ou par le bouton de réduction/restauration de la barre de connexion supérieure.
- **Envoi des combinaisons spéciales RDP (`IMsRdpClientNonScriptable::SendKeys`)** :
  - Résolution de l'absence d'effet de `SendKeys` (offset de vtable 112) : conversion des codes de touches virtuelles (`VK_*`) en codes de balayage matériel PS/2 (`MapVirtualKeyW(MAPVK_VK_TO_VSC_EX)`) et construction conforme des entiers 32 bits `lParam` (bits 16-23 : scan code, bit 24 : touche étendue, bit 29 : contexte Alt, bit 30 : état précédent, bit 31 : transition de relâchement).
  - Clarification technique et documentation : sous Windows NT, la combinaison `Ctrl + Alt + Suppr` est une séquence SAS protégée par le noyau (`Winlogon`). Sur une session RDP, la séquence officielle prévue par Microsoft est `Ctrl + Alt + Fin`. Les deux séquences sont désormais correctement transmises via `SendKeys` avec leurs scan codes matériels respectifs.
- **Formulaire de connexion adaptatif selon le protocole** :
  - Basculement dynamique instantané lors de la sélection du protocole (`RDP`, `AnyDesk`, `RustDesk`) dans le dialogue de création/édition de connexion.
  - Masquage automatique des contrôles non pertinents (Port, Utilisateur, Domaine, Résolution dynamique, Presse-papiers) pour AnyDesk et RustDesk.
  - Élargissement dynamique du champ Hôte / ID / Alias sur toute la largeur disponible pour améliorer la lisibilité des adresses AnyDesk et RustDesk.
  - Adaptation contextuelle du libellé du mot de passe (« Mot de passe d'accès sans surveillance » pour AnyDesk, « Mot de passe permanent » pour RustDesk) et affichage d'une note informative contextuelle.
  - Conservation sans altération de toutes les valeurs saisies lors de basculements successifs entre protocoles.
- **Transmission sécurisée du mot de passe AnyDesk sur stdin (`--with-password`)** :
  - Implémentation du lancement AnyDesk via un tube de communication anonyme Win32 (`CreatePipe`) relié au `hStdInput` du processus (`STARTF_USESTDHANDLES`).
  - Lancement sans passerelle shell (`cmd.exe`/PowerShell) via `CreateProcessW` avec argument officiel `--with-password`.
  - Écriture immédiate du mot de passe déchiffré via DPAPI suivie d'un retour à la ligne, fermeture immédiate du descripteur d'écriture (`CloseHandle`) pour envoyer `EOF`, et effacement mémoire immédiat du tampon sensible (`write_volatile`).
  - Aucun mot de passe dans la ligne de commande ni dans les journaux d'événements Windows.
- **Audit et sécurisation de l'authentification RustDesk** :
  - Audit de la CLI RustDesk confirmant l'absence de mécanisme d'injection stdin pour les sessions distantes (le paramètre `--password` configure le mot de passe local de la machine et ne permet pas d'authentifier une session distante).
  - Préservation du mot de passe en coffre DPAPI pour référence de l'utilisateur avec affichage d'une note explicite dans le formulaire indiquant que l'authentification reste assurée par l'invite du client RustDesk.
- **Artefact graphique de la barre de recherche lors du basculement dynamique de thème** :
  - Traitement du message `WM_CTLCOLORSTATIC` dans la procédure de fenêtre principale `main_wnd_proc` pour retourner le pinceau de fond d'entrée `palette.brush_bg_input` lorsque le contrôle concerné est `search_edit` (déclenché lors du rendu de la bannière indicatrice `EM_SETCUEBANNER`).
  - Appel explicite à `UpdateWindow(self.search_edit)` dans `apply_theme()` garantissant le rafraîchissement immédiat de la zone de saisie.

### Ajouté
- Test unitaire `test_rdp_scan_code_and_lparam_construction` validant le calcul des scan codes PS/2 et l'encodage des `lParam` 32 bits pour `SendKeys`.
- Test d'intégration Win32 `test_ui_search_edit_theming_and_ctlcolorstatic` validant le comportement des pinceaux sur `search_edit` en thème Sombre et Clair.
- Tests d'intégration complétés dans `test_ui_connection_dialog_create_validate_and_cancel_real_win32` validant le masquage et réaffichage dynamique des champs selon le protocole.

## [Jalon 3] — Intégrations AnyDesk et RustDesk, RDP avancé et actions de session

### Ajouté
- **Intégration d'AnyDesk** :
  - Section *Paramètres > Applications externes* avec champ de chemin vers `AnyDesk.exe`.
  - Bouton **Parcourir...** avec dialogue de sélection de fichier Win32 standard (`IFileOpenDialog` et filtre `*.exe`).
  - Bouton **Détecter** recherchant automatiquement `AnyDesk.exe` dans le Registre Windows (`HKLM`/`HKCU` `SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths`) et dans les dossiers standards (`Program Files`, `Program Files (x86)`, `APPDATA`, `LOCALAPPDATA`).
  - Bouton **Tester** démarrant le client pour vérifier le binaire.
  - Lancement des sessions AnyDesk depuis l'arborescence, les favoris ou l'accueil via l'API Win32 `CreateProcessW` sans intermédiaire shell ni concaténation, avec assainissement strict des ID distants et délégation totale de l'authentification et de la fenêtre à AnyDesk.
- **Intégration de RustDesk** :
  - Section *Paramètres > Applications externes* avec champ de chemin vers `rustdesk.exe`.
  - Bouton **Parcourir...**, **Détecter** et **Tester** dédiés à RustDesk.
  - Lancement direct de la session avec syntaxe officielle `--connect <id>` via `CreateProcessW`.
  - Rejet immédiat de toute tentative d'injection d'arguments dans l'identifiant distant.
- **Raccourcis clavier RDP (`KeyboardHookMode`)** :
  - Paramètre configurable dans *Paramètres > Options RDP par défaut* :
    - `0` : Raccourcis exécutés sur le poste local.
    - `1` : Raccourcis envoyés à la session distante (**défaut recommandé**).
    - `2` : Raccourcis envoyés à distance uniquement en plein écran.
  - Programmation COM sur `IMsRdpClientSecuredSettings2::put_KeyboardHookMode` (vtable offset 104) avant connexion.
  - Prise en charge des combinaisons Windows (`Win+R`, `Win+E`, `Win+D`, `Alt+Tab`, `Alt+F4`, `Ctrl+C`, `Ctrl+V`).
- **Menu d'actions de session active RDP** :
  - Bouton d'action contextuel `⚡` intégré sur l'onglet RDP actif et ouverture par clic droit sur l'onglet.
  - Actions disponibles :
    - 🔒 **Envoyer Ctrl + Alt + Suppr** via `IMsRdpClientNonScriptable::SendKeys` (vtable offset 112).
    - 🛡️ **Envoyer Ctrl + Alt + Fin** (équivalent natif RDP) via `IMsRdpClientNonScriptable::SendKeys`.
    - 🔲 **Basculer en plein écran / Quitter le plein écran** via `IMsRdpClient9::put_FullScreen` / `get_FullScreen`.
    - ❌ **Fermer / Déconnecter la session** avec libération propre des ressources COM.
- **Redirection de disques et partage de fichiers RDP** :
  - Activation de `put_RedirectDrives` (vtable offset 968) sur `IMsRdpClientAdvancedSettings8` couplée au partage de presse-papiers (`RedirectClipboard`), autorisant le copier/coller natif de fichiers entre l'hôte et la session distante.
- **Migration de base de données SQLite (schéma v3)** :
  - Ajout des clés `anydesk_path`, `rustdesk_path` et mise à jour de `rdp_keyboard_hook_mode` vers `remote`.
- **Tests unitaires et d'intégration automatisés (36 tests)** :
  - `test_sanitize_remote_id_accepts_valid_ids` et `test_sanitize_remote_id_rejects_empty_and_injections`.
  - `test_validate_executable_path_checks_existence_and_extension`.
  - `test_build_anydesk_command_line_encoding` et `test_build_rustdesk_command_line_encoding`.
  - `test_ui_rdp_session_menu_actions_and_external_app_launch_wiring` : validation des actions de session et du routage sans crash.

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
