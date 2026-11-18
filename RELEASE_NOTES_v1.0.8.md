# Malvision v1.0.8 — Notes de version

**Date de publication :** 18 novembre 2026  
**Version :** `1.0.8`  
**Binaire officiel :** `Malvision.exe`  
**Architecture :** Windows 11 x64 (binaire portable autonome, sans dépendances externes)  
**Empreinte SHA-256 :** `a6f679877f083d72c0396b746eb95b476de8266dcc0cbb267a8cb9a191ac696d`  
**Taille :** 5 686 784 octets  

---

## Nouveautés de la V1.0.8

### 1. Onglet « Accueil » permanent dans la barre des connexions
- **Navigation libre entre Accueil et sessions actives** ([`src/sessions.rs`](src/sessions.rs), [`src/ui.rs`](src/ui.rs)) :
  - L'onglet **Accueil** reste désormais affiché en permanence tout à gauche de la barre d'onglets, même lorsque plusieurs connexions sont ouvertes.
  - Les nouvelles connexions s'ouvrent à la suite de l'onglet Accueil ; un clic sur **Accueil** masque temporairement les sessions actives (`SessionManager::select_home()`) sans les déconnecter, tout en conservant le défilement du tableau de bord.

### 2. Finitions de la fenêtre Paramètres
- **Onglet À propos** : description du logiciel actualisée (RDP `MsTscAx`, client FTP double-panneau, terminal SSH `ConPTY`, AnyDesk/RustDesk avec DPAPI, identifiants partagés et onglets multi-sessions) et dimensionnée sans troncature.
- **Onglet Avancé** : alignement des boutons (`Modifier l'emplacement…`, `Ouvrir le dossier`, `Revenir au dossier par défaut`) avec respect de la marge droite (`28 px`).

---

## Contrôle d'intégrité

| Fichier | Taille (octets) | Empreinte SHA-256 |
| :--- | :--- | :--- |
| `Malvision.exe` | `5 686 784` | `a6f679877f083d72c0396b746eb95b476de8266dcc0cbb267a8cb9a191ac696d` |

---

## Validation et Tests

- **111 tests unitaires et d'intégration réussis (`100 % passants`)** (`cargo test`).
- `cargo fmt --check` et `cargo clippy --all-targets -- -D warnings` sans erreur ni avertissement.
- Scan de secrets (`scan_secrets.py`) : `0` fuite détectée.
- Vérification des métadonnées PE de `target/release/Malvision.exe` : `FileVersion = 1.0.8.0`, `ProductVersion = 1.0.8`.
