# Malvision v1.0.4 — Notes de version

**Date de publication :** 10 novembre 2026  
**Version :** `1.0.4`  
**Binaire officiel :** `Malvision.exe`  
**Architecture :** Windows 11 x64 (binaire portable autonome, sans dépendances externes)  
**Empreinte SHA-256 :** `3e0b40c5d622d5c0d41fa828204d4a5a2d78bc69cb9ef8fcef6aee26b304b337`  
**Taille :** 5 276 160 octets  

---

## Nouveautés majeures de la V1.0.4

### 1. Mise à jour automatique en place avec remplacement et redémarrage
Malvision résout définitivement la contrainte historique du verrouillage d'image Windows (`0x80070005 — Accès refusé`) lors du téléchargement d'une nouvelle version dans le dossier de l'exécutable actif.
- **Bouton « Mettre à jour et redémarrer »** : accessible en un clic dans Paramètres > Mises à jour dès qu'une version supérieure est disponible.
- **Parcours 100 % automatisé** : téléchargement en tâche de fond, contrôle cryptographique SHA-256, fermeture ordonnée, remplacement atomique et relance immédiate de Malvision.
- **Téléchargement manuel conservé** : pour les utilisateurs préférant choisir manuellement l'emplacement de sauvegarde, un bouton discret « Téléchargement manuel (.exe)… » reste disponible.

### 2. Architecture relais éphémère et Sécurité cryptographique durcie (Single Portable Executable garanti)
- **Aucun installateur externe, aucun service d'arrière-plan, aucune tâche planifiée** : Malvision reste un seul et unique exécutable portable.
- **Véritable HMAC-SHA-256 natif Windows CNG (RFC 2104 / FIPS 198-1)** : scellement d'intégrité du manifeste via `BCryptOpenAlgorithmProvider` avec `BCRYPT_ALG_HANDLE_HMAC_FLAG`.
- **Secret non exposé sur le disque** : le jeton d'authentification secret (`auth_token`) de 256 bits généré par `BCryptGenRandom` n'est jamais écrit dans le fichier manifeste JSON sur le disque ; il est transmis exclusivement par argument mémoire au processus auxiliaire éphémère.
- **Vérification obligatoire du processus parent & anti-recyclage de PID** : contrôle obligatoire et strict du chemin d'image (`QueryFullProcessImageNameW`) et validation de l'horodatage de création du processus (`GetProcessTimes`), interdisant tout détournement de processus ou réutilisation de PID.
- **Restauration et Rollback cryptographiquement vérifiés** : toute restauration de l'ancienne version (`Malvision.exe.bak`) contrôle obligatoirement l'empreinte SHA-256 d'origine avant et après la restauration.
- **Sécurisation absolue de la récupération différée (`pending_recovery.json`)** :
  - Validation stricte contre l'exécutable courant (`std::env::current_exe()`).
  - Dérivation stricte du chemin de sauvegarde (aucun chemin arbitraire accepté).
  - Contrôle d'exclusion des répertoires système (`SystemRoot`) et rejet catégorique des liens symboliques ou jonctions.
  - Vérification cryptographique de l'empreinte du backup avant toute opération sur les fichiers.
- **Confirmation explicite de santé (Health Check)** :
  - Surveillance active par événement Win32 dédié (`Local\Malvision_Health_<token>`) et drapeau fichier.
  - Le binaire précédent (`Malvision.exe.bak`) est conservé jusqu'à validation complète de l'initialisation de l'application (DPI, single-instance, COM/ActiveX, SQLite, création de fenêtre).
  - En cas de crash ou d'anomalie au démarrage, le relais termine le processus si nécessaire pour libérer le verrou, restaure l'ancienne version avec contrôle d'empreinte et la relance.
- **Protection des sessions distantes** : détection et avertissement clair si des sessions RDP sont en cours avec demande de confirmation explicite avant fermeture. Les sessions AnyDesk et RustDesk externes ne sont jamais interrompues.
- **Nettoyage automatique sécurisé** : purge des fichiers temporaires uniquement après confirmation de bon fonctionnement.

---

## Contrôle d'intégrité officiel

| Fichier | Taille (octets) | Empreinte SHA-256 |
| :--- | :--- | :--- |
| `Malvision.exe` | `5 276 160` | `3e0b40c5d622d5c0d41fa828204d4a5a2d78bc69cb9ef8fcef6aee26b304b337` |

---

## Validation et Tests

- **88 tests unitaires et d'intégration réussis (100 % passants)** incluant les vecteurs officiels RFC 4231 pour HMAC-SHA-256.
- **Clippy et rustfmt stricts** sans avertissements (`-D warnings`).
- **Validation réelle sous Windows 11 x64** : tests de qualification nominaux, rejet des manifestes altérés et vérification réelle du rollback après crash.
