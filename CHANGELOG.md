# Changelog — Malvision

Toutes les évolutions notables du projet **Malvision** sont documentées dans ce fichier.

## [1.0.4] — Mise à jour automatique avec remplacement et redémarrage (2026-11-10)

### Ajouté & Amélioré
- **Mise à jour automatique en place avec redémarrage transparent** :
  - Ajout du bouton proéminent **« Mettre à jour et redémarrer »** dans l'onglet Paramètres > Mises à jour lors de la détection d'une nouvelle version sur GitHub Releases.
  - Élimination définitive de l'erreur Windows `0x80070005 — Accès refusé` causée par la tentative d'écriture directe sur le binaire `Malvision.exe` verrouillé par le système d'exploitation.
  - Parcours utilisateur entièrement automatisé : téléchargement en arrière-plan, vérification SHA-256, fermeture propre, remplacement atomique, relance immédiate de Malvision et affichage de la nouvelle version.
  - Conservation d'un bouton secondaire discret « Téléchargement manuel (.exe)… » pour les utilisateurs souhaitant enregistrer le fichier manuellement via `IFileSaveDialog`.
- **Architecture relais éphémère sans installateur externe (Single Portable Executable)** :
  - Maintien rigoureux du principe d'un seul fichier exécutable portable distribué (`Malvision.exe`), sans installateur externe (InnoSetup, MSI, etc.), sans service d'arrière-plan permanent et sans tâche planifiée Windows.
  - Utilisation de `Malvision.exe` lui-même comme processus relais éphémère exécuté depuis `%LOCALAPPDATA%\Malvision\updates\malvision_updater.exe` avec le commutateur interne `--apply-update <auth_token>`.
  - Contournement immédiat du mutex d'instance unique et des initialisations lourdes (ActiveX, SQLite, interface Win32) dès l'interception de l'argument `--apply-update`.
- **Sécurité, Intégrité et Rollback automatique durcis** :
  - **Jeton d'authentification CSPRNG 256 bits** : génération d'octets cryptographiquement sûrs via `BCryptGenRandom(None, buf, BCRYPT_USE_SYSTEM_PREFERRED_RNG)` sans aucun fallback prévisible.
  - **Secret non exposé sur le disque** : le jeton d'authentification secret (`auth_token`) n'est jamais stocké dans le fichier manifeste sur le disque ; il transite exclusivement par argument mémoire vers le processus auxiliaire éphémère.
  - **Véritable HMAC-SHA-256 natif Windows CNG (RFC 2104 / FIPS 198-1)** : calcul d'un HMAC-SHA-256 conforme via `BCryptOpenAlgorithmProvider` avec `BCRYPT_ALG_HANDLE_HMAC_FLAG` sur l'ensemble des métadonnées critiques (`manifest_hmac`), vérifié en temps constant (`constant_time_compare`).
  - **Contrôle obligatoire du processus parent & Anti-recyclage de PID** : validation obligatoire du chemin d'image (`QueryFullProcessImageNameW`) rejetant toute erreur ou divergence, et vérification de l'horodatage exact de création du processus (`GetProcessTimes`), neutralisant toute exploitation d'un PID réutilisé.
  - **Règles de sécurité sur la cible** : restriction stricte aux fichiers `.exe` et interdiction absolue de cibler les répertoires système Windows (`SystemRoot`).
  - **Confirmation de santé explicite (Health Check)** : émission d'un signal de disponibilité explicite (événement Win32 `Local\Malvision_Health_<token>` et fichier drapeau) après initialisation complète de tous les composants critiques (COM/ActiveX, base de données, fenêtre principale).
  - **Rollback automatique vérifié cryptographiquement** : en cas de plantage, de fermeture prématurée ou de gel au démarrage du nouveau binaire, le processus enfant est terminé pour libérer les verrous Windows, l'ancienne version (`Malvision.exe.bak`) est restaurée en place avec contrôle strict de l'empreinte SHA-256 originale attendue, et l'application est relancée.
  - **Récupération différée par processus auxiliaire distinct & Authentification DPAPI (`pending_recovery.json`)** :
    - Élimination de toute tentative de remplacement d'un exécutable Windows en cours d'exécution : lorsqu'une récupération est requise au démarrage, Malvision délègue l'opération à `malvision_updater.exe --apply-recovery <parent_pid> <parent_creation_filetime>` et quitte immédiatement pour déverrouiller son image.
    - Le helper auxiliaire attend la libération effective du processus principal via `WaitForSingleObject` avant de restaurer le backup `.bak`.
    - Authentification absolue par Windows DPAPI (`CryptProtectData`) : la cible et l'empreinte originale sont extraites d'un blob chiffré non falsifiable, interdisant toute modification locale du JSON par un tiers.
    - Persistance de la directive : `pending_recovery.json` n'est supprimée qu'après confirmation cryptographique de la restauration.
    - Procédure de secours manuelle documentée dans un message d'erreur natif explicite en cas d'impossibilité technique de restauration automatique.
  - **Conservation du backup et nettoyage sécurisé** : le backup `.bak` est rigoureusement conservé jusqu'à la confirmation de santé du nouveau processus. Purge préventive sécurisée au démarrage normal.

### Testé
- Suite de tests automatisés étendue à **90 tests (100 % passants)** :
  - `test_bytes_to_hex_and_hex_to_bytes` : conversion bidirectionnelle hexadécimale vérifiée.
  - `test_bcrypt_gen_random_bytes` : génération aléatoire CSPRNG sans motif prévisible.
  - `test_constant_time_compare` : comparaison en temps constant insensible aux attaques par canal auxiliaire.
  - `test_hmac_sha256_rfc4231_official_vectors` : validation avec les vecteurs de test officiels RFC 4231 Cas 1, 2 et 3.
  - `test_update_manifest_hmac_and_serialization` : validation JSON et calcul HMAC-SHA-256 du manifeste avec secret hors JSON.
  - `test_manifest_tampering_rejected` : rejet strict de toute altération de la cible, du hash, du hash original ou des PID.
  - `test_auth_token_expiration_logic` : contrôle de la fenêtre d'expiration de 5 minutes.
  - `test_target_exe_safety_rules` : validation de l'extension `.exe` et exclusion des dossiers système Windows.
  - `test_perform_rollback_and_verify_success_and_failure` : restauration vérifiée par SHA-256 du backup et gestion des erreurs.
  - `test_pending_recovery_persistence_and_cleanup` et `test_pending_recovery_negative_scenarios_rejected` : scellement DPAPI, persistance et rejet des directives altérées.
  - `test_health_token_registration_and_notification` : cycle d'enregistrement et d'émission du signal de santé.
  - `test_in_use_file_lock_reproduction_and_wait` : reproduction du verrouillage de fichier en cours d'exécution sous Windows et libération de handle.

## [1.0.3] — Correctif de comparaison des versions GitHub (2026-11-10)

### Corrigé
- **Moteur de mises à jour — Source unique de vérité `SemVer`** :
  - Suppression de la constante obsolète en dur `SemVer::CURRENT` dans `src/updater.rs` qui était restée fixée à `1.0.1`, provoquant la détection erronée d'une mise à jour disponible sur une version identique ou antérieure.
  - Implémentation de `current_semver() -> Result<SemVer, String>` et `SemVer::current()`, dérivant dynamiquement la version locale depuis `CARGO_PKG_VERSION` (source unique de vérité absolue).
  - Remplacement de toutes les comparaisons dans `src/ui.rs` (`spawn_startup_update_check` et dialogue Paramètres > Mises à jour).
  - Sécurisation stricte : en cas d'erreur de lecture de la version locale, aucun téléchargement n'est autorisé (`unwrap_or(false)`).
- **Réseau HTTP & User-Agent** :
  - Génération dynamique du User-Agent WinHTTP et des en-têtes HTTP de requêtes GitHub à partir de `CURRENT_VERSION` (`Malvision/1.0.3 (Windows NT 10.0; Win64; x64)`), éliminant toute chaîne de version résiduelle en dur.

### Testé
- Suite de tests automatisés étendue à **75 tests (100 % passants)** :
  - `test_current_semver_matches_cargo_pkg_version` : validation de la concordance exacte entre la version locale SemVer et `CARGO_PKG_VERSION`.
  - `test_semver_comparison_matrix_for_v103` : matrice complète de comparaison (`1.0.3 == 1.0.3` -> false, `1.0.4 > 1.0.3` -> true, `1.1.0 > 1.0.3` -> true, `2.0.0 > 1.0.3` -> true, `1.0.2 < 1.0.3` -> false, `1.0.1 < 1.0.3` -> false).
  - `test_invalid_local_version_never_authorizes_update` : garantie qu'un échec de parsing ne déclenche jamais de téléchargement.
  - `test_startup_and_manual_comparison_simulation` : simulation des réponses GitHub (version égale, antérieure et supérieure).

## [1.0.2] — Ergonomie avancée de l'arborescence (2026-11-10)

### Ajouté & Amélioré
- **Boutons « Tout déplier » et « Tout replier »** :
  - Deux boutons compacts de barre d'outils au style Fluent positionnés au-dessus de l'arborescence, immédiatement sous le champ de recherche.
  - Icônes vectorielles GDI précises à doubles chevrons : double chevron descendant pour « Tout déplier », double chevron montant pour « Tout replier ».
  - Dépliage et repliage récursifs instantanés de l'ensemble des dossiers et sous-dossiers, fluides même avec des arborescences de plusieurs centaines de dossiers.
  - Mise à jour atomique de l'état persistant SQLite (`folders.is_expanded`) via `Storage::set_all_folders_expanded()`.
  - Préservation intégrale des connexions, favoris et sessions RDP actives.
- **Persistance individuelle de l'état développé / replié des dossiers** :
  - Mémorisation de l'état de chaque dossier par identifiant SQLite stable (`folder_id`), indépendant du nom ou de l'ordre d'affichage.
  - Prise en charge universelle de la persistance quel que soit le mode d'interaction utilisateur : clic sur chevron ou icône de dossier (avec détection dynamique de la zone d'expansion à toute profondeur), double-clic sur toute la rangée utile (`treeview_wnd_proc`), navigation au clavier (`TVN_ITEMEXPANDEDW`), ou boutons d'outils globaux.
  - Restauration exacte des dossiers développés ou repliés à chaque redémarrage complet de Malvision.
  - Préservation des états lors des rafraîchissements de l'arborescence, des renommages, déplacements de connexions et changements de thème (Sombre, Clair, Système).
  - Découplage complet de la recherche : le dépliage temporaire pour afficher les résultats filtrés ne modifie pas les préférences enregistrées en base.
  - Suppression des écritures SQLite redondantes lors des rafraîchissements programmatiques (`is_programmatic_tree_update`).
- **Connexions prioritaires** :
  - Nouvelle propriété booléenne `is_priority` gérée en base SQLite (migration automatique vers le schéma version 4 avec index optimisé `idx_connections_priority`).
  - Règle de tri stricte appliquée au sein de chaque dossier et à la racine :
    1. Connexions prioritaires classées par ordre alphabétique (A à Z).
    2. Connexions normales classées par ordre alphabétique (A à Z).
    3. Sous-dossiers classés par ordre alphabétique (A à Z).
  - Indicateur visuel distinctif : flèche triangulaire montante (`▲`) ambre (`#2080E0`) dessinée sur le nœud de connexion dans l'arborescence, évitant toute confusion visuelle avec l'étoile dorée des favoris.
  - Case à cocher dédiée « Marquer comme prioritaire » intégrée dans les boîtes de dialogue de création et d'édition de connexion.
  - Action directe dans le menu contextuel clic droit de l'arborescence : « Marquer comme prioritaire » / « Retirer la priorité » permettant la bascule immédiate en un clic.
  - Prise en charge complète dans les duplications, exports, sauvegardes et répercussion dans la liste ordonnée du tableau de bord « Toutes les connexions ».
- **Symétrie parfaite des boutons d'action de la barre latérale** :
  - Largeurs dynamiques rigoureusement identiques pour les boutons `+ Connexion` et `+ Dossier` grâce à la formule `(largeur_disponible - espacement) / 2`.
  - Même hauteur (32 px logiques), même alignement vertical, texte parfaitement centré et états de survol/clic harmonisés.
  - Rendu impeccable à 100 %, 125 % et 150 % HiDPI, ainsi que lors du redimensionnement dynamique de la barre latérale.
- **Identité & Versioning** :
  - Montée de version globale `1.0.2` : `Cargo.toml`, métadonnées PE Win32 (`build.rs`), User-Agent du vérificateur de mises à jour, et affichage dynamique du badge d'en-tête (`v1.0.2`).

### Testé
- Suite de tests automatisés étendue à **70 tests (100 % passants)** :
  - `test_priority_connections_sorting_and_rules` : validation du tri prioritaire devant les connexions normales puis les sous-dossiers.
  - `test_folder_expansion_individual_and_global` : validation du stockage et de l'expansion individuelle/globale en SQLite.
  - `test_set_connection_priority_and_duplicate` : validation de la bascule de priorité et de sa duplication.
  - `test_migration_v4_preserves_data` : validation de la migration transactionnelle du schéma v3 vers v4 sans perte de données.
  - `test_ui_folder_expansion_individual_and_global_commands` : validation UI des commandes tout déplier / tout replier.
  - `test_ui_connection_priority_dialog_and_context_menu_and_sorting` : validation du dialogue, du menu contextuel et du tri UI.
  - `test_ui_sidebar_buttons_symmetry_and_dpi` : validation mathématique de la symétrie des boutons à 100 %, 125 % et 150 % DPI.

## [1.0.1] — Version Corrective et Améliorations Ergonomiques (2026-11-10)

### Ajouté & Amélioré
- **Instance unique avec réactivation et focus garanti** :
  - Mutex Win32 nommé à portée de session (`Local\Malvision_SingleInstance_Mutex`).
  - Détection immédiate au lancement (barre des tâches, menu Démarrer, raccourcis, démarrage auto, ou EXE direct).
  - Réactivation fiable de l'instance principale, restauration depuis l'état minimisé ou depuis la zone de notification (System Tray) avec transfert d'autorisation de focus (`AllowSetForegroundWindow`) et message fenêtré enregistré (`RegisterWindowMessageW`).
  - Quitter immédiat du processus secondaire sans initialisation de base SQLite, sans création de fenêtre ni doublon d'icône tray.
  - Libération atomique du verrou lors des redémarrages volontaires (changement d'emplacement de base).
- **Emplacement personnalisable de la base de données SQLite (`malvision.db`)** :
  - Nouvel onglet **Avancé** dans la boîte de dialogue des Paramètres (volet latéral étendu à 6 catégories).
  - Contrôle du chemin actif avec boutons « Modifier l'emplacement… » (sélecteur de dossier Win32 `IFileOpenDialog` moderne), « Ouvrir le dossier » dans l'Explorateur et « Revenir au dossier par défaut ».
  - Détection du type de support (`DRIVE_REMOTE`, `DRIVE_REMOVABLE`, `DRIVE_FIXED`) avec avertissement explicite pour les disques amovibles ou réseaux.
  - Sauvegarde sécurisée sans altération de la base d'origine (jamais supprimée, déplacée ni renommée). Copie intègre par `Storage::export_database()` avec validation SQLite (`PRAGMA integrity_check`) et checkpoint WAL.
  - Persistance dans `%LOCALAPPDATA%\Malvision\bootstrap.json`.
  - Robustesse au démarrage : dialogue interactif en cas de disque personnalisé déconnecté (Réessayer, Choisir un autre dossier, Revenir au défaut, Quitter).
- **Tri perfectionné de l'arborescence des connexions** :
  - Dans chaque dossier et à la racine : **connexions directes toujours affichées avant les sous-dossiers**.
  - Tri alphabétique insensible à la casse (A à Z) appliqué récursivement dans toute la hiérarchie.
  - Préservation intégrale des favoris, icônes de protocoles, expansion/repli des nœuds et double-clic pleine largeur.
- **En-tête : Badge de version dynamique de l'application** :
  - Remplacement du badge statique `Win11` par le numéro de version de l'application (`v1.0.1`) à droite du titre « Malvision ».
  - Largeur dynamique calculée via GDI (`DT_CALCRECT`) garantissant une adaptation automatique sans troncature pour les futures versions (`v1.0.2`, `v1.1.0`, etc.).
  - Préservation du style officiel : fond bleu accent, texte blanc, coins arrondis, parfaitement intégré aux thèmes sombre, clair et système.
- **Tableau de bord : Section « Toutes les connexions », Grille adaptative & Badges uniformisés** :
  - Nouvelle section affichant toutes les connexions ordonnée selon la hiérarchie logique de l'arborescence.
  - Cartes compactes ajustées en largeur (210 à 250 pixels logiques, espacement de 12px) permettant d'aligner idéalement 4 à 5 cartes par ligne sur une fenêtre large, sans étirement excessif.
  - Badges de protocole strictement uniformisés avec l'arborescence : `RDP` (bleu), `ANY` (rouge `#D84339`), `RST` (turquoise `#16968A`).
  - Prise en charge complète du défilement à la molette (`WM_MOUSEWHEEL`) avec conservation de la zone visible et hit-testing précis des cartes.
  - **Repositionnement dynamique intelligent** :
    - *Cas A (Connexions récentes activées)* : enchaînement classique sous les favoris et récentes.
    - *Cas B (Connexions récentes désactivées)* : masquage intégral de la section récente sans aucun espace résiduel, « Toutes les connexions » remontant directement sous les favoris / actions rapides.
  - Clic direct sur une carte pour lancer instantanément la session correspondante.
- **Sécurisation absolue du bootstrap SQLite (`bootstrap.json`)** :
  - Règle stricte : si le fichier `bootstrap.json` est absent, l'emplacement par défaut (`%LOCALAPPDATA%\Malvision\malvision.db`) est utilisé.
  - Si le fichier `bootstrap.json` existe mais est illisible, invalide, ou contient un chemin non absolu / nom de fichier incorrect, Malvision interrompt immédiatement le démarrage avec une alerte explicite sans jamais basculer silencieusement sur une autre base ni créer de base vide.
  - Dialogue de récupération contrôlée proposant de réessayer, de resélectionner le dossier ou de réinitialiser la configuration avec sauvegarde automatique `bootstrap.json.corrupt_bak`.

### Testé
- Suite de tests portée à **63 tests automatisés (100 % passants)** :
  - `test_bootstrap_corrupted_json_and_invalid_inputs` (rejet de syntaxes corrompues, versions non supportées, chemins relatifs ou types invalides).
  - `test_single_instance_guard_drop_clean` & `test_activate_message_registration` (instance unique Win32).
  - `test_bootstrap_defaults_and_roundtrip` & `test_validate_directory_writable_success_and_plan`.
  - `test_collect_tree_ordered_connections_rules` (tri connexions avant dossiers).
  - `test_ui_single_instance_activation_message_restores_from_tray_and_focuses`.
  - `test_ui_dashboard_layout_cas_a_and_cas_b_and_scrolling`.
  - `test_ui_treeview_direct_connections_appear_before_subfolders`.
  - `test_ui_settings_dialog_tabs_jalon4_categories_and_buttons` mis à jour pour 6 onglets.

## [1.0.0] — Jalon 4C : Dernières Finitions Prépublication V1.0.0 (2026-11-10)

### Interface & Ergonomie
- **Cadre DPAPI intégralement visible (Paramètres > Données)** :
  - Ajustement vertical de la carte informative Windows DPAPI (extension de la hauteur à 172px scaled) offrant un dégagement confortable pour les 4 puces explicatives complètes sans aucun rognage de la dernière ligne.
  - Recalage proportionné et aéré de l'ensemble des sections et boutons d'export et d'import sur la grille visuelle (boutons à Y: 312px et Y: 414px), vérifié sur les facteurs d'échelle 100 %, 125 % et 150 % HiDPI.

### Sécurité & Robustesse SQLite
- **Élimination complète des fichiers auxiliaires d'exportation** :
  - Exécution de `PRAGMA wal_checkpoint(TRUNCATE)` et `PRAGMA journal_mode = DELETE` sur la base d'export temporaire avant fermeture, convertissant la sauvegarde en fichier `.db` totalement autonome et indépendant de tout journal externe.
  - Fermeture explicite des connexions SQLite temporaires avant toute manipulation de fichiers.
  - Nettoyage sécurisé et garanti de l'ensemble des fichiers auxiliaires résiduels (`.tmp_export-wal`, `.tmp_export-shm`, `.tmp_export-journal`) en cas de succès comme en cas d'erreur.
  - Garantie stricte d'absence d'impact sur la base de données active en cours d'utilisation.

### Mises à jour
- **Traitement ergonomique du code HTTP 404 GitHub Releases** :
  - Prise en charge explicite du retour HTTP 404 lorsque le dépôt est privé ou qu'aucune release publique n'est encore publiée.
  - Affichage d'un message informatif direct : « Aucune version publique disponible pour le moment. » sans le présenter à tort comme une défaillance de la connexion Internet.
  - Préservation intégrale de l'URL officielle GitHub et du comportement standard lors de la future publication publique de la V1.0.0.

### Testé
- Suite de tests portée à **54 tests automatisés (100 % passants)** :
  - Ajout de `test_storage_export_produces_only_single_db_without_auxiliary_files` (vérification stricte de la production d'un fichier `.db` unique sans aucun fichier `.tmp_export-wal` ou `.tmp_export-shm` résiduel).
  - Ajout de `test_parse_release_json_github_not_found` (validation du message utilisateur sur réponse HTTP 404 / `Not Found`).

## [1.0.0-rc2] — Jalon 4B : Sécurisation Finale de Prépublication V1.0.0 (2026-11-10)

### Sécurité & Robustesse
- **Exportation SQLite durcie et sans risque de perte** :
  - Sauvegarde désormais écrite dans un fichier temporaire sur le même volume/répertoire (`.{dest_stem}_{pid}_{nanos}.tmp_export`).
  - Contrôle d'intégrité SQLite strict (`PRAGMA integrity_check`) et validation du schéma effectués sur le fichier temporaire avant toute substitution.
  - Conservation absolue du fichier de destination existant en cas d'erreur ou d'interruption.
  - Remplacement atomique et synchrone vers la destination via l'API Win32 native `MoveFileExW` (`MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH`).
  - Nettoyage automatique des fichiers temporaires via un gardien RAII `Drop`.
- **Importation SQLite avec rollback automatique** :
  - Validation préalable de la base source avant toute modification.
  - Création d'une sauvegarde de sécurité de la base active (`.pre_import_bak`) immédiatement vérifiée avec `validate_import_file`.
  - En cas d'échec durant la restauration ou la migration de schéma, un retour arrière automatique (rollback) est déclenché depuis `.pre_import_bak`, préservant intégralement la configuration d'origine de l'utilisateur.
- **Mises à jour sécurisées et non bloquantes** :
  - Rejet strict des protocoles et redirections HTTP en clair (seul HTTPS est autorisé, avec limite à 5 redirections).
  - Vérification obligatoire et inconditionnelle de l'empreinte SHA-256 de référence : aucun téléchargement n'est accepté sans empreinte officielle valide.
  - Téléchargement vers un fichier temporaire, avec conservation du fichier existant en cas de hash invalide ou de panne réseau.
  - Récupération automatique de l'empreinte depuis l'asset `SHA256SUMS.txt` si non présente dans le corps de la release.
  - Amélioration de la réactivité visuelle de la boîte des Paramètres (désactivation et rétroaction d'état claire sur les boutons de vérification et de téléchargement).
- **Audit de sécurité prépublication** :
  - Audit automatisé complet de l'ensemble des 12 commits de l'historique Git et des 118 objets : 0 token, 0 clé privée, 0 mot de passe, 0 base SQLite tracked.
  - Retrait des mentions prématurées de licence MIT : conservation stricte de `Copyright (c) 2026 Malvirae` en attente du choix explicite de licence par l'utilisateur.

### Testé
- Suite de tests portée à **52 tests automatisés (100 % passants)** :
  - `test_storage_export_preserves_existing_destination_on_failure_and_cleans_temp`
  - `test_storage_export_with_wal_mode_transactions`
  - `test_storage_import_automatic_rollback_on_failed_migration`
  - `test_parse_release_json_with_sha256sums_txt_asset`
  - Tests d'intégrité SHA-256 avec rejet obligatoire sans hash et préservation du fichier existant.

## [1.0.0-rc1] — Jalon 4 : Finalisation, Identité Graphique, Sauvegarde et Mises à Jour (2026-11-10)

### Ajouté
- **Identité graphique officielle & Ressources Windows PE** :
  - Intégration du monogramme officiel géométrique « M » bicolore (Bleu Windows `#0078D4` et Cyan `#38BDF8`).
  - Production et compilation via `build.rs` et `windres` d'une icône PE multirésolution complète (16×16, 24×24, 32×32, 48×48, 64×64, 128×128, 256×256) intégrée au binaire sans fichiers `.ico` externes.
  - L'icône officielle s'affiche dans l'Explorateur Windows, la barre des tâches, Alt+Tab, la fenêtre principale et la zone de notification Windows (System Tray).
  - Écran de démarrage (Splash screen) officiel non bloquant affiché pendant ~650 ms lors du démarrage normal (automatiquement contourné si l'application est démarrée minimisée dans le tray).
  - Métadonnées de version PE complètes et rigoureuses : `ProductName: MALVISION`, `FileDescription: Malvision Remote Connection Manager`, `FileVersion: 1.0.0.0`, `ProductVersion: 1.0.0`, `InternalName: Malvision`, `OriginalFilename: Malvision.exe`.
- **Centre de Paramètres moderne à 5 catégories avec volet latéral Fluent** :
  - Réorganisation ergonomique sous forme de 5 onglets accessibles par clic souris dans la barre latérale gauche :
    - **Général** : Thèmes (Sombre/Clair/Système), comportement de fermeture vers le tray, démarrage minimisé, connexions récentes, confirmation de fermeture.
    - **Protocoles** : Détection automatique, configuration et tests de lancement pour AnyDesk et RustDesk, options RDP par défaut (presse-papiers, dynamic resolution, keyboard hook mode).
    - **Données** : Sauvegardes SQLite et intégrité des secrets DPAPI.
    - **Mises à jour** : Gestion du canal de mise à jour GitHub Releases, état de la dernière vérification, téléchargement sécurisé et notice portable.
    - **À propos** : Rendu dynamique du monogramme officiel, numéro de version 1.0.0, informations légales et lien direct vers le dépôt GitHub.
  - Navigation au clavier accessible : la touche Tabule (`Tab`) cycle exclusivement sur les contrôles visibles de la catégorie sélectionnée.
- **Sauvegarde et Restauration transactionnelles SQLite (WAL & DPAPI)** :
  - Module d'exportation `export_database` exploitant l'API de sauvegarde en ligne SQLite `rusqlite::backup::Backup`, garantissant une copie cohérente sans interrompre les sessions distantes ni bloquer le journal WAL.
  - Sauvegarde préservant les mots de passe chiffrés par Windows DPAPI, permettant une restauration instantanée sans ressaisie des secrets sur la même machine/compte utilisateur.
  - Module de validation d'import `validate_import_file` effectuant un contrôle d'intégrité SQLite (`PRAGMA integrity_check`), la validation du schéma relationnel et un comptage non destructif des secrets DPAPI déchiffrables et non déchiffrables.
  - Restauration contrôlée `import_database` créant automatiquement une copie de sécurité `.pre_import_bak` avant remplacement, puis réinitialisant et actualisant en direct l'arborescence graphique.
- **Vérification et téléchargement des mises à jour via GitHub Releases** :
  - Client asynchrone WinHTTP natif en HTTPS interrogeant l'API publique `GET /repos/Malvirae/malvision/releases/latest` sans nécessiter de jeton d'authentification.
  - Comparaison sémantique SemVer (`CURRENT = 1.0.0`) filtrant les versions antérieures, égales, ainsi que les versions identifiées comme brouillons (`draft`) ou pré-versions (`prerelease`).
  - Vérification automatique en tâche de fond au démarrage (différée de 3 secondes pour ne pas ralentir le lancement) avec notification discrète dans la barre d'état.
  - Bouton « Télécharger la mise à jour… » invitant à choisir l'emplacement de destination, téléchargeant l'asset binaire avec fichier temporaire `.tmp` et validant obligatoirement l'empreinte cryptographique SHA-256 officielle (extraite de `SHA256SUMS.txt` ou des Release Notes).
  - Calcul cryptographique SHA-256 natif Windows CNG BCrypt (`BCRYPT_SHA256_ALGORITHM`) sans crate externe.
- **Préparation de la distribution portable V1.0.0** :
  - Binaire unique `Malvision.exe` d'environ 5 Mo, totalement autonome et prêt au déploiement sans installation.
  - Fichier de sommes de contrôle `SHA256SUMS.txt` et modèle de Release Notes `RELEASE_NOTES_v1.0.0.md`.
  - Durcissement de `.gitignore` excluant les bases de données SQLite, journaux WAL/SHM, fichiers temporaires et sauvegardes.

### Testé
- Suite de tests automatisés portée à 48 tests réussis (unitaires et intégration Win32 réels) :
  - `test_storage_export_and_import_with_wal_and_dpapi_preservation`
  - `test_storage_import_rejects_corrupted_or_invalid_file`
  - `test_semver_parsing_and_comparisons`
  - `test_parse_release_json_valid_and_ignores_prerelease`
  - `test_extract_sha256_from_text`
  - `test_write_and_verify_asset_bytes_sha256_success_and_tamper_detection`
  - `test_compute_sha256_known_vector`
  - `test_ui_settings_dialog_tabs_jalon4_categories_and_buttons`


### Corrigé
- **Résolution de la cause racine du double-clic dans l'espace vide à droite du libellé** :
  - Identification de l'architecture interne des Common Controls Windows (`WC_TREEVIEWW`) : lorsque l'utilisateur double-clique dans l'espace vide à droite du libellé (`TVHT_ONITEMRIGHT`), la procédure de fenêtre native du TreeView intercepte `WM_LBUTTONDBLCLK`, constate que le point n'est ni sur l'icône ni sur le texte, et abandonne l'événement **sans émettre la notification `NM_DBLCLK`** à la fenêtre parente. Le parent ne recevait donc jamais le message.
  - Mise en place d'un sous-classement Win32 dédié du TreeView (`treeview_wnd_proc` via `GWLP_WNDPROC` et propriété `MalvisionTreeViewOrigProc`) interceptant en amont le message `WM_LBUTTONDBLCLK` avant qu'il ne soit ignoré par le contrôle natif.
  - Résolution déterministe de rangée sans sondage empirique (`tree_resolve_row_at_point`) :
    - Vérification stricte des limites clientes (`GetClientRect`).
    - Parcours des éléments visibles avec `TVGN_FIRSTVISIBLE` puis `TVGN_NEXTVISIBLE`.
    - Récupération de la boîte englobante de chaque rangée via `TVM_GETITEMRECT` avec `fItemRect = FALSE` (ligne entière).
    - Détection immédiate de l'élément dont la plage verticale `[rc_item.top, rc_item.bottom[` contient `pt.y`.
    - Fonctionne sans aucune coordonnée X codée en dur, quelle que soit la longueur du texte (ex. connexion courte « A »), la largeur du panneau, la profondeur du sous-dossier ou la position horizontale du clic.
  - Préservation des interactions natives :
    - Clics sur les boutons d'expansion `[+]`/`[-]` (`TVHT_ONITEMBUTTON`) transmis directement à la procédure d'origine.
    - Double-clic sur un dossier ou une racine : basculement immédiat de l'expansion via `TVM_EXPAND(TVE_TOGGLE)`.
    - Double-clic sur une connexion (`RDP`, `AnyDesk`, `RustDesk`) : sélection visuelle `TVM_SELECTITEM(TVGN_CARET)` et exécution unique de `launch_connection_by_id`.
    - Clics en dehors de toute rangée visible (espace vide sous la liste) : ignorés sans effet secondaire.
  - Élimination garantie des doubles lancements :
    - Le sous-classement consomme `WM_LBUTTONDBLCLK` en retournant `LRESULT(0)` sans appeler la procédure native, empêchant l'émission ultérieure de `NM_DBLCLK`.
    - Protection complémentaire par anti-rebond (debounce 400 ms) dans `launch_connection_by_id` neutralisant tout message concurrent.

### Ajouté
- Test unitaire et d'intégration Win32 réel `test_ui_full_row_double_click_short_name_and_all_x_positions` (portant le total à 40 tests automatisés) :
  - Connexion au libellé minimaliste « A » (protocole RustDesk) imbriquée dans une arborescence à sous-dossiers (`Parent Folder` -> `Sub Folder`).
  - Validation du hit-test sur 5 positions horizontales distinctes : sur l'icône ($X=35$), sur le texte ($X=60$), juste après le texte ($X=90$), au centre de la zone vide ($X=\text{width}/2$) et à 10 pixels du bord droit du panneau ($X=\text{width}-10$).
  - Validation de l'émission effective d'un message `WM_LBUTTONDBLCLK` à 10 pixels du bord droit déclenchant le lancement de la connexion et renseignant `status_override`.
  - Validation de l'anti-rebond lors de double-clics successifs rapprochés.
  - Validation du basculement d'expansion d'un dossier par double-clic tout à droite de sa ligne.
  - Validation du retour `None` et de l'absence d'action en cas de clic dans l'espace vide sous la liste ($Y=\text{height}+50$).

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
