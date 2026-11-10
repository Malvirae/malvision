# Malvision v1.0.3 — Correctif de Comparaison des Versions GitHub (Distribution Portable)

**Malvision** est un gestionnaire de connexions distantes moderne, ultra-léger et sécurisé, développé en **Rust natif** exclusivement pour **Windows 11 x64**.

Cette version **1.0.3** corrige une anomalie dans le moteur de détection et de comparaison des mises à jour distantes GitHub, et centralise la gestion de version selon le principe de source unique de vérité.

---

## 🚀 Évolutions & Correctifs clés

### 🔄 Moteur de mises à jour : Source unique de vérité `SemVer`
- **Suppression des constantes codées en dur** : Élimination de la constante obsolète `SemVer::CURRENT` dans `src/updater.rs` qui restait fixée à `1.0.1`, provoquant la notification erronée d'une mise à jour disponible lorsqu'une version `1.0.2` interrogeait le dépôt GitHub.
- **Centralisation via Cargo** : Création de la fonction centralisée `crate::updater::current_semver() -> Result<SemVer, String>` et de `SemVer::current()`, dérivant directement la version locale depuis la macro `env!("CARGO_PKG_VERSION")`.
- **Comparaison rigoureuse et exacte** :
  - `1.0.3 == 1.0.3` : L'application reconnaît qu'elle est parfaitement à jour (`is_update_available: false`).
  - `1.0.2 < 1.0.3` : Les versions distantes antérieures ne déclenchent aucune alerte (`is_update_available: false`).
  - `1.0.4 > 1.0.3` : Seule une version distante strictement supérieure déclenche la notification et propose le téléchargement (`is_update_available: true`).
- **Sécurisation défensive** : En cas d'erreur hypothétique lors de la lecture ou de l'analyse de la version locale, le moteur adopte un comportement strictement défensif (`unwrap_or(false)`) et n'autorise jamais de téléchargement automatique.

### 🌐 Réseau WinHTTP & User-Agent dynamique
- **Génération dynamique du User-Agent** : Formatage automatique de la chaîne d'en-tête HTTP WinHTTP à partir de la version courante (`Malvision/1.0.3 (Windows NT 10.0; Win64; x64)`), garantissant l'absence totale de chaînes de version codées en dur dans les couches réseau.

### 🏷️ Identité & Versioning
- Métadonnées exécutables PE Win32 estampillées `1.0.3.0` (`FileVersion`) et `1.0.3` (`ProductVersion`).
- Badge de version dans l'en-tête principal affichant dynamiquement `v1.0.3`.
- Dialogue « À propos » et onglet « Mises à jour » reflétant fidèlement `v1.0.3`.

---

## 🔒 Intégrité binaire & Sécurité

- **Exécutable portable unique autonome** : `target/release/Malvision.exe` (sans runtime externe ni dépendance tierce requise).
- **Contrôles cryptographiques stricts** : Vérification systématique des empreintes SHA-256 lors de tout téléchargement de mise à jour.
- **Protection des secrets** : Chiffrement intégral DPAPI pour les mots de passe et données sensibles.

---

## 📦 Procédure de mise à niveau

1. Téléchargez le fichier `Malvision.exe` depuis la section **Releases** du dépôt GitHub :
   `https://github.com/Malvirae/malvision/releases/tag/v1.0.3`
2. Remplacez simplement votre ancien exécutable `Malvision.exe` par la nouvelle version `1.0.3`.
3. Vos paramètres, vos dossiers, vos connexions et vos préférences restent 100 % intacts dans la base locale SQLite.
