# Malvision v1.0.5 — Notes de version

**Date de publication :** 16 novembre 2026  
**Version :** `1.0.5`  
**Binaire officiel :** `Malvision.exe`  
**Architecture :** Windows 11 x64 (binaire portable autonome, sans dépendances externes)  
**Empreinte SHA-256 :** `961c1cf3ccdf758a78edb313bff0182bc52296035557bc4b8b8be66dc64f301c`  
**Taille :** 5 548 032 octets  

---

## Nouveautés de la V1.0.5

### 1. Client FTP intégré avec identifiants chiffrés DPAPI
Malvision intègre un client FTP natif complet directement dans les onglets de session, en respectant intégralement les codes esthétiques Fluent de l'application (thèmes Sombre / Clair / Système, HiDPI, GDI double-buffering) :
- **Moteur FTP Rust pur (RFC 959 / RFC 3659)** sans dépendance externe :
  - Modes passifs `EPSV` et `PASV` avec repli automatique d'adresse IP pour environnements NAT.
  - Négociation `OPTS UTF8 ON`, mode binaire `TYPE I`, listage intelligent `MLSD` avec repli sur `LIST` (formats Unix et Windows/DOS).
  - Téléchargement atomique (écriture vers fichier `.part` avant renommage final), suivi de progression et annulation instantanée, protection stricte contre les injections CR/LF/NUL.
- **Interface Win32/GDI native par onglet (`FtpSessionHost`)** :
  - Barre d'outils Fluent (`↑ Parent`, `↻ Actualiser`, `⇧ Téléverser…`, `⇩ Télécharger…`, `+ Dossier`, `✎ Renommer`, `✕ Supprimer`, `Identifiants…`, `Reconnecter`).
  - Vue par défaut à **deux panneaux redimensionnables** façon FileZilla : explorateur local Windows à gauche (lecteurs `Poste de travail`, navigation, création/renommage/suppression) et explorateur FTP distant à droite avec séparateur vertical draggable.
  - Panneau inférieur d'activité et logs à **hauteur redimensionnable via glisser-déposer** (curseur `IDC_SIZENS`) adaptant dynamiquement le nombre de lignes visibles.
  - Barre d'adresse éditable sur chaque panneau (`LOCAL` / `FTP`), double-clic pour téléverser depuis le panneau gauche ou télécharger directement dans le dossier local courant depuis le panneau droit, menu contextuel, raccourcis clavier (`Entrée`, `Retour`, `F5`, `F2`, `Suppr`), glisser-déposer (`WM_DROPFILES`) et barre de progression temps réel.
  - Changement de thème Sombre/Clair répercuté à chaud sans relancer les sessions FTP actives.
- **Gestion complète des identifiants & Migration SQLite v5** :
  - Nouveau protocole `Protocol::Ftp` en base locale (`schema v5`) avec conservation transparente des données existantes.
  - Formulaire de connexion adaptatif (`FTP (Client intégré)` avec port par défaut `21`, utilisateur et dossier initial facultatif, et prise en charge des URLs `ftp://user@host:port/chemin`).
  - Saisie à la connexion via une fenêtre modale dédiée lorsqu'aucun identifiant n'est enregistré (ou lors d'un échec d'authentification `530`), avec option **« Enregistrer ces identifiants (chiffrés via Windows DPAPI) »**.

### 2. Correctif RustDesk : transmission automatique du mot de passe enregistré
- Résolution du problème obligeant à retaper le mot de passe RustDesk même lorsqu'il était enregistré dans la fiche de connexion.
- Déchiffrement du secret via Windows DPAPI au lancement d'une connexion `Protocol::RustDesk` et passage sécurisé via `--connect "<id>" --password "<mot_de_passe>"` avec échappement Windows `CommandLineToArgvW` et effacement mémoire volatil (`write_volatile`) du tampon UTF-16 après création du processus.

### 3. Correctif de stabilité GDI / Win32
- Prévention d'une exception fatale (`0xc000041d`) sous Windows lors du passage d'une chaîne vide à `DrawTextW` dans les assistants de rendu `draw_text_line`, `draw_text_multiline`, `draw_filled_badge` et `draw_text_right`.

---

## Contrôle d'intégrité

| Fichier | Taille (octets) | Empreinte SHA-256 |
| :--- | :--- | :--- |
| `Malvision.exe` | `5 548 032` | `961c1cf3ccdf758a78edb313bff0182bc52296035557bc4b8b8be66dc64f301c` |

---

## Validation et Tests

- **101 tests unitaires et d'intégration réussis (`100 % passants`)**, incluant les tests de protocole/client FTP contre serveur mock local, l'explorateur local Windows et le calcul des splitters, la migration SQLite v4→v5, l'interface Win32 du formulaire et de l'onglet FTP, ainsi que la transmission DPAPI `--password` pour RustDesk.
- `cargo fmt` et `cargo clippy --all-targets -- -D warnings` sans erreur ni avertissement.
- Vérification des métadonnées PE de `target/release/Malvision.exe` : `FileVersion = 1.0.5.0`, `ProductVersion = 1.0.5`.
