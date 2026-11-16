# Malvision v1.0.6 — Notes de version

**Date de publication :** 16 novembre 2026  
**Version :** `1.0.6`  
**Binaire officiel :** `Malvision.exe`  
**Architecture :** Windows 11 x64 (binaire portable autonome, sans dépendances externes)  
**Empreinte SHA-256 :** `c69027ae74cafa3d1d4a8ba0a1389c39fea1e76c320e4a9e2d9d342c9440aa9f`  
**Taille :** 5 645 312 octets  

---

## Nouveautés de la V1.0.6

### 1. Connexion SSH intégrée (`Protocol::Ssh`) avec terminal ConPTY natif
- **Nouveau protocole dans le choix des connexions** (`SSH (Terminal intégré)`, badge `SSH`, port `22` par défaut, champ facultatif *Clé privée SSH*, prise en charge des URLs `ssh://utilisateur@hôte:port`).
- **Terminal intégré ConPTY + OpenSSH Windows** ([`src/ssh.rs`](src/ssh.rs)) :
  - Pseudo-console native Windows (`CreatePseudoConsole` / `ResizePseudoConsole`) hébergée dans un onglet Malvision avec rendu GDI double-bufferisé (`Consolas`, couleurs ANSI 16/256/TrueColor, écran alternatif `htop`/`vim`/`nano`, scrollback molette).
  - Barre d'outils Fluent (`Copier`, `Coller`, `Effacer`, `Reconnecter`), sélection souris, menu contextuel et raccourcis `Ctrl+Shift+C` / `Ctrl+Shift+V`.
  - **Mot de passe chiffré DPAPI** transmis de manière sécurisée via `SSH_ASKPASS` (`SSH_ASKPASS_REQUIRE=force`) et un pipe nommé éphémère à usage unique (`\\.\pipe\MalvisionAskPass_*`) sans jamais exposer le secret dans la ligne de commande ni sur disque.
- **Migration SQLite v5 -> v6** ajoutant `'ssh'` dans la contrainte `CHECK(protocol ...)` avec préservation intégrale des données existantes.

### 2. Améliorations ergonomiques du client FTP intégré
- **Suppression du doublon `..` (Dossier parent)** lors de la navigation dans l'arborescence distante.
- **Keep-alive `NOOP` automatique** (toutes les 20 s) et **reconnexion transparente dans le dernier dossier visité** (`last_remote_dir`) en cas de coupure serveur ou de clic sur *Reconnecter*.
- **Sélection multiple** (`Clic`, `Ctrl + Clic`, `Shift + Clic`, `Ctrl + A`) dans les deux panneaux (Local et FTP) pour les transferts et suppressions groupées.
- **Glisser-déposer direct entre les deux panneaux** (Local ↔ FTP) avec bandeau visuel de dépôt cible.

### 3. Finitions ergonomiques du formulaire de connexion
- Espacement sous le champ *Description* avant l'encadré *Sécurité DPAPI* (`+14 px`).
- Liste déroulante des protocoles agrandie pour afficher simultanément les 5 protocoles sans barre de défilement.
- Icône Malvision affichée dans la barre de titre des fenêtres *Nouvelle connexion distante* et *Modifier la connexion*.

---

## Contrôle d'intégrité

| Fichier | Taille (octets) | Empreinte SHA-256 |
| :--- | :--- | :--- |
| `Malvision.exe` | `5 645 312` | `c69027ae74cafa3d1d4a8ba0a1389c39fea1e76c320e4a9e2d9d342c9440aa9f` |

---

## Validation et Tests

- **107 tests unitaires et d'intégration réussis (`100 % passants`)** (`cargo test`).
- `cargo fmt --check` et `cargo clippy --all-targets -- -D warnings` sans erreur ni avertissement.
- Vérification des métadonnées PE de `target/release/Malvision.exe` : `FileVersion = 1.0.6.0`, `ProductVersion = 1.0.6`.
