# Malvision v1.0.0 — Version Initiale Officielle (Distribution Portable)

**Malvision** est un gestionnaire de connexions distantes moderne, léger et sécurisé, développé en **Rust natif** exclusivement pour **Windows 11 x64**.

Il regroupe dans une interface unifiée et réactive vos connexions **RDP**, **AnyDesk** et **RustDesk**, avec chiffrement local matériel des secrets via l'API **Windows DPAPI**.

---

## 🚀 Nouveautés & Fonctionnalités clés

### 🎨 Identité visuelle & Expérience Windows 11 Fluent
- **Monogramme officiel « M »** géométrique bicolore (Bleu Windows `#0078D4` et Cyan `#38BDF8`).
- **Icône multirésolution intégrée au binaire** (16×16 à 256×256) pour l'Explorateur Windows, la barre des tâches, Alt+Tab, la fenêtre principale et la zone de notification (System Tray).
- **Écran de démarrage officiel (Splash screen)** rapide et non bloquant (~650 ms), masqué automatiquement lors du démarrage minimisé.
- **Thématisation dynamique sans résidu** : basculement instantané Sombre / Clair / Système sans redémarrage, avec prise en charge complète du Per-Monitor V2 HiDPI (100 %, 125 %, 150 %).
- **Double-clic pleine largeur** : ouverture immédiate des connexions sur l'ensemble de la rangée (icône, texte ou espace vide) sans dépendre de l'état de sélection antérieur.

### ⚙️ Centre de Paramètres à 5 Catégories
- **Général** : Personnalisation du thème visuel, minimisation vers la zone de notification, démarrage avec Windows (raccourci utilisateur `.lnk` auto-réparant sans droits admin), affichage des connexions récentes.
- **Protocoles** : Chemins configurables vers AnyDesk et RustDesk avec détection automatique dans le Registre Windows et tests de lancement en un clic. Options RDP par défaut (Presse-papiers, Dynamic Resolution, KeyboardHookMode).
- **Données** : Sauvegardes SQLite en ligne (WAL-safe) et intégrité des secrets DPAPI.
- **Mises à jour** : Vérification asynchrone via GitHub Releases et téléchargement avec vérification cryptographique.
- **À propos** : Rendu dynamique du logo officiel, version installée et lien direct vers le dépôt GitHub.

### 💾 Sauvegarde & Restauration SQLite (WAL & DPAPI)
- **Exportation en ligne non bloquante** : Sauvegarde instantanée sans déconnecter vos sessions RDP ni verrouiller la base.
- **Préservation des secrets** : Les mots de passe chiffrés par DPAPI sont conservés dans la sauvegarde pour une restauration immédiate sur votre compte Windows.
- **Importation sécurisée** : Vérification stricte d'intégrité SQLite (`PRAGMA integrity_check`), validation du schéma et copie de sécurité automatique `.pre_import_bak` avant tout remplacement.

### 🔄 Mises à jour intégrées via GitHub Releases
- Interrogation réseau asynchrone HTTPS (WinHTTP natif) respectant le format SemVer.
- Téléchargement sécurisé avec fichier temporaire et contrôle cryptographique SHA-256 obligatoire avant validation.
- Respect absolu de la vie privée : aucune télémétrie, aucun tracker, aucun compte requis.

### 🛡️ RDP Avancé & Intégrations Externes
- **RDP ActiveX Microsoft natif** (`IMsRdpClient9`) : Plein écran officiel avec barre rétractable, injection matérielle PS/2 de **Ctrl+Alt+Fin** (SAS RDP) et **Ctrl+Alt+Suppr**, redirection bidirectionnelle du presse-papiers et des fichiers.
- **AnyDesk** : Transmission sécurisée du mot de passe sur flux standard input anonyme (`--with-password`), aucun secret sur la ligne de commande ni dans les logs.
- **RustDesk** : Lancement sécurisé via `CreateProcessW` sans intermédiaire shell.

---

## 📦 Téléchargement & Sommes de contrôle

| Fichier | Taille | Empreinte SHA-256 |
| :--- | :--- | :--- |
| **`Malvision.exe`** | ~5.0 Mo | `ebb34180a524ae433a787e25d1411c5e392cf1c021c3272dab4dfeae847693de` |

---

## 🛠️ Consignes d'utilisation portable

Malvision est distribué sous la forme d'un **exécutable portable unique**. Aucune installation n'est requise.

1. Téléchargez `Malvision.exe`.
2. Placez l'exécutable dans le dossier de votre choix (par exemple `C:\Outils\Malvision\` ou sur une clé USB sécurisée).
3. Double-cliquez sur `Malvision.exe` pour lancer l'application.
4. Vos données de configuration et vos préférences sont conservées localement dans `%LOCALAPPDATA%\Malvision\malvision.db`.

### Procédure de mise à jour future
1. Quittez complètement Malvision (via *Fichier > Quitter* ou depuis l'icône du System Tray).
2. Remplacez simplement votre ancien fichier `Malvision.exe` par le nouvel exécutable téléchargé.
3. Relancez l'application : l'ensemble de vos dossiers, favoris, connexions et paramètres restent intacts.
