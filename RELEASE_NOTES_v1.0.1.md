# Malvision v1.0.1 — Version Corrective et Améliorations Ergonomiques (Distribution Portable)

**Malvision** est un gestionnaire de connexions distantes moderne, léger et sécurisé, développé en **Rust natif** exclusivement pour **Windows 11 x64**.

Cette version **1.0.1** apporte l'instance unique pour éviter les doublons de processus, la personnalisation complète de l'emplacement de la base SQLite (`malvision.db`), le tri perfectionné de l'arborescence (connexions directes avant sous-dossiers), et l'affichage complet de toutes les connexions sur le tableau de bord avec défilement vertical fluide.

---

## 🚀 Nouveautés & Évolutions clés

### 🔒 Instance unique & Réactivation intelligente
- **Portée session utilisateur** : Protection via mutex Win32 nommé (`Local\Malvision_SingleInstance_Mutex`).
- **Comportement anti-duplication** : Toute tentative de lancement d'une seconde instance (barre des tâches, menu Démarrer, raccourci Bureau, démarrage automatique, ou binaire direct) détecte immédiatement l'instance déjà en cours.
- **Réactivation et focus garanti** : Si l'application principale est minimisée dans le System Tray ou en barre des tâches, elle est restaurée, affichée et portée au premier plan (`AllowSetForegroundWindow` + `RegisterWindowMessageW`).
- **Arrêt immédiat du processus secondaire** : Sortie instantanée sans ouvrir SQLite, sans recréer d'interface ni dupliquer l'icône dans la zone de notification.
- **Redémarrage volontaire pris en charge** : Libération propre du mutex lors du changement d'emplacement de la base de données.

### 💾 Emplacement personnalisable de la base SQLite (`malvision.db`)
- **Nouvel onglet « Avancé » dans les Paramètres** : Visualisation du chemin actif de `malvision.db`, bouton « Modifier l'emplacement… » (sélecteur de dossier moderne `IFileOpenDialog`), bouton « Ouvrir le dossier » dans l'Explorateur et bouton « Revenir au dossier par défaut ».
- **Nom de fichier invariant** : Le fichier s'intitule obligatoirement `malvision.db` au sein du dossier choisi.
- **Sauvegarde et intégrité absolues** :
  - La base d'origine n'est jamais supprimée, déplacée ni renommée.
  - Si le dossier cible est vide : copie en ligne transactionnelle via `Storage::export_database()` avec validation d'intégrité SQLite (`PRAGMA integrity_check`) et checkpoint WAL complet.
  - Si une base `malvision.db` existe déjà dans le dossier cible : validation préalable du schéma et confirmation utilisateur avant bascule.
- **Contrôle des volumes amovibles et réseau** : Avertissement préventif clair si le dossier sélectionné réside sur un lecteur réseau ou une clé USB amovible.
- **Persistance légère** : Mémorisation du chemin personnalisé dans `%LOCALAPPDATA%\Malvision\bootstrap.json`.
- **Résilience au démarrage** : Si le volume personnalisé est déconnecté ou inaccessible au démarrage, Malvision n'écrase rien silencieusement : une boîte de dialogue interactive permet de *Réessayer*, *Choisir un autre dossier*, *Revenir à l'emplacement standard*, ou *Quitter sans rien altérer*.

### 📂 Tri perfectionné de l'arborescence
- **Connexions directes avant les sous-dossiers** : Dans chaque dossier ainsi qu'à la racine, toutes les connexions directes s'affichent en premier, suivies des sous-dossiers.
- **Tri alphabétique insensible à la casse (A à Z)** appliqué uniformément pour les connexions comme pour les dossiers.
- **Règle récursive et préservation d'état** : Maintien intégral des favoris, icônes de protocoles (RDP, AnyDesk, RustDesk), recherche dynamique et état développé/replié des dossiers.

### 📊 Tableau de bord : Section « Toutes les connexions » & Défilement fluide
- **Section complète sous les connexions récentes** : Affichage de l'intégralité des connexions sous forme de cartes compactes Fluent (`hauteur = 56px`) organisées en grille adaptative (1 à 4 colonnes selon la largeur).
- **Informations claires en un coup d'œil** : Badge protocole coloré, nom en gras, étoile dorée pour les favoris, chemin du dossier et hôte / identifiant distant.
- **Défilement vertical fluide** : Prise en charge de la molette souris (`WM_MOUSEWHEEL`) sur la vue d'accueil avec barres et limites de défilement dynamiques.
- **Repositionnement dynamique intelligent** :
  - *Cas A (Connexions récentes activées)* : Enchaînement naturel Actions rapides -> Connexions favorites -> Connexions récentes -> Toutes les connexions.
  - *Cas B (Connexions récentes désactivées)* : La section des connexions récentes est totalement masquée sans espace vide résiduel ; « Toutes les connexions » remonte automatiquement sous les favoris / actions rapides.
- **Lancement direct en un clic** : Clic sur une carte déclenche immédiatement la session correspondante.

---

## 📦 Téléchargement & Sommes de contrôle

| Fichier | Taille | Empreinte SHA-256 |
| :--- | :--- | :--- |
| **`Malvision.exe`** | 5 187 584 octets (~5.0 Mo) | `d9bf73c9027ccd16ed021c2d32dd40cf114cce361e254f19a7c290cad8fa7b35` |

---

## 🛠️ Consignes d'utilisation & Mise à niveau portable

1. Quittez complètement Malvision si une version antérieure est ouverte (via *Fichier > Quitter* ou depuis le System Tray).
2. Remplacez simplement votre ancien fichier `Malvision.exe` par le nouvel exécutable `Malvision.exe` v1.0.1.
3. Relancez l'application : l'ensemble de vos connexions, dossiers, favoris, secrets DPAPI et paramètres sont automatiquement préservés.
