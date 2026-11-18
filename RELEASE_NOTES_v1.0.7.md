# Malvision v1.0.7 — Gestionnaire d'identifiants partagés

## Nouveautés principales

- **Gestionnaire d'identifiants partagés (DPAPI)** :
  - Nouveau bouton **« Identifiants »** en bas de la barre latérale permettant de créer, modifier et supprimer des identifiants réutilisables (`nom`, `utilisateur` / `DOMAINE\utilisateur`, `domaine`, `mot de passe` chiffré via Windows DPAPI, `description`).
  - Sélecteur **« Identifiant partagé »** dans le formulaire de connexion pour lier un même identifiant à plusieurs connexions (`RDP`, `FTP`, `SSH`, `AnyDesk`, `RustDesk`).
  - Mise à jour centralisée : modifier un mot de passe dans le gestionnaire met immédiatement à jour toutes les connexions associées, tout en préservant les paramètres propres à chaque connexion (dossier initial FTP, clé privée SSH).
  - Migration automatique et sans perte de la base SQLite locale vers le **schéma v7** (`credentials` + `connections.credential_id`).

## Vérification
- **111 tests unitaires et d'intégration** passants (`cargo test`), `cargo fmt --check` et `cargo clippy --all-targets -- -D warnings` sans avertissement.
- Exécutable portable autonome : `Malvision.exe` (Windows 11 x64).
