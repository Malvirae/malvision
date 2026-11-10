# Malvision v1.0.2 — Ergonomie avancée de l'arborescence (Distribution Portable)

**Malvision** est un gestionnaire de connexions distantes moderne, ultra-léger et sécurisé, développé en **Rust natif** exclusivement pour **Windows 11 x64**.

Cette version **1.0.2** apporte des améliorations ergonomiques majeures demandées pour la gestion d'arborescences volumineuses et complexes : le contrôle global de dépliage/repliage de l'arborescence, la mémorisation persistante de l'état de chaque dossier, la gestion fine des connexions prioritaires au sein de chaque dossier, et la parfaite symétrie visuelle des boutons d'action de la barre latérale.

---

## 🚀 Nouveautés & Évolutions clés

### 📂 Contrôle global : Boutons « Tout déplier » et « Tout replier »
- **Boutons compacts intégrés** : Positionnés élégamment sous la zone de recherche et au-dessus de l'arborescence, ces deux boutons Fluent permettent de contrôler en un clic l'affichage de l'ensemble de la hiérarchie.
- **Pictogrammes vectoriels Fluent antialiasés** : Rendu vectoriel ultra-net via GDI+ avec antialiasing intégral, traits d'épaisseur uniforme et extrémités arrondies. Deux chevrons descendants pour « Tout déplier », deux chevrons montants pour « Tout replier », avec une symétrie verticale rigoureuse et un alignement optique centré.
- **Exécution instantanée et atomique** : L'action récursive parcourt l'ensemble des nœuds et synchronise immédiatement l'état persistant dans la base SQLite (`Storage::set_all_folders_expanded`).
- **Préservation intégrale** : Ne modifie aucunement les favoris, les métadonnées de dossiers ni les sessions en cours.

### 🌿 Chevrons vectoriels de l'arborescence des dossiers
- **Géométrie vectorielle commune à 90°** : Les chevrons d'expansion des dossiers utilisent désormais un tracé vectoriel unique tourné de 90° entre l'état replié (`>`) et l'état déplié (`⌄`).
- **Netteté et antialiasing GDI+** : Rendu parfaitement lisse sans aucun effet de pixelisation ni escalier, avec des extrémités arrondies et un centrage vertical parfait sur chaque ligne à 100 %, 125 % et 150 % DPI.

### 💾 Persistance individuelle de l'état développé / replié des dossiers
- **Mémorisation par identifiant stable** : Chaque dossier enregistre son propre état d'expansion (`is_expanded`) indexé par son identifiant SQLite unique, insensible aux renommages et à l'ordre d'affichage.
- **Restauration fidèle au démarrage** : À la fermeture et à la réouverture de Malvision, l'utilisateur retrouve exactement la même arborescence avec les dossiers dépliés ou repliés tels qu'il les a laissés.
- **Résilience absolue** :
  - Les états sont scrupuleusement préservés lors des rafraîchissements, déplacements et changements de thèmes (Sombre, Clair, Système).
  - La recherche n'altère pas les préférences : le filtrage dynamique peut déplier temporairement les branches contenant des résultats sans écraser l'état persistant enregistré en base.
  - Les rafraîchissements programmatiques n'effectuent aucune écriture SQLite superflue.

### ⭐ Connexions prioritaires au sein de chaque dossier
- **Épinglage en tête de dossier** : Une connexion peut désormais être marquée comme « Prioritaire ». Elle se positionne automatiquement au sommet de son dossier parent.
- **Hiérarchie de tri stricte et universelle** :
  1. **Connexions prioritaires** classées par ordre alphabétique (A à Z).
  2. **Connexions normales** classées par ordre alphabétique (A à Z).
  3. **Sous-dossiers** classés par ordre alphabétique (A à Z).
- **Triangle creux vert émeraude antialiasé** : Remplacement de l'ancienne flèche par un triangle équilatéral régulier creux (contour seul) orienté vers le haut (`▲`) de couleur vert émeraude `#34C88A`, parfaitement aligné avec le texte et distinct de l'étoile dorée des favoris.
- **Contrôle ergonomique rapide** :
  - Case à cocher « Marquer comme prioritaire » intégrée dans les formulaires de création et de modification de connexion.
  - Action instantanée dans le menu contextuel clic droit : « Marquer comme prioritaire » / « Retirer la priorité ».
- **Intégration complète** : Le statut prioritaire est préservé lors de la duplication, des exports JSON, des sauvegardes automatiques et se reflète harmonieusement dans la liste ordonnée du tableau de bord.

### 📐 Symétrie parfaite des boutons de la barre latérale
- **Répartition équilibrée de l'espace** : Les boutons d'action rapide `+ Connexion` et `+ Dossier` partagent désormais exactement la même largeur calculée dynamiquement par `(largeur_disponible - espacement) / 2`.
- **Alignement et typographie harmonisés** : Hauteur identique (32 px logiques), texte rigoureusement centré, espacement de 8 px, et retour visuel cohérent au survol et au clic.
- **Haute résolution (HiDPI)** : Calculs scalés avec précision garantissant une netteté et un alignement parfaits à 100 %, 125 % et 150 % d'échelle d'affichage Windows.

### 🏷️ Identité & Versioning
- Badge de version dans l'en-tête mis à jour automatiquement en `v1.0.2`.
- Métadonnées exécutables PE Win32 estampillées `1.0.2.0`.
- User-Agent de contrôle des mises à jour configuré sur `Malvision/1.0.2`.

---

## 📦 Téléchargement & Sommes de contrôle

| Fichier | Taille | Empreinte SHA-256 |
| :--- | :--- | :--- |
| **`Malvision.exe`** | 5 208 064 octets (~5.2 Mo) | `463544f040aec4febbf50df412cd6a629dc5f203356e86e335d52223fa81447e` |

---

## 🛠️ Consignes d'utilisation & Mise à niveau portable

### 1. Application Portable autonome
Malvision est un binaire autonome unique (`Malvision.exe`), ne nécessitant aucun installateur ni dépendance externe. Pour mettre à jour une installation existante :
1. Fermez Malvision s'il est en cours d'exécution.
2. Remplacez simplement votre ancien exécutable `Malvision.exe` par la nouvelle version `1.0.2`.
3. Lancez la nouvelle version. La migration de votre base de données SQLite vers le schéma version 4 est automatique, immédiate et sécurisée.

### 2. Emplacement des données
- Base de données principale : `%LOCALAPPDATA%\Malvision\malvision.db` (ou votre emplacement personnalisé configuré dans l'onglet *Paramètres > Avancé*).
- Vos identifiants enregistrés restent chiffrés localement par l'API DPAPI de Windows liée à votre session utilisateur.
