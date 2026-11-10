# Malvision v1.0.4 — Notes de version

**Date de publication :** 10 novembre 2026  
**Version :** `1.0.4`  
**Binaire officiel :** `Malvision.exe`  
**Architecture :** Windows 11 x64 (binaire portable autonome, sans dépendances externes)  
**Empreinte SHA-256 :** `318da16e833cd2e0237a6031ba96694d2e5fb8786be367aa2f09639ce0fee05c`  
**Taille :** 5 267 968 octets  

---

## Nouveautés majeures de la V1.0.4

### 1. Mise à jour automatique en place avec remplacement et redémarrage
Malvision résout définitivement la contrainte historique du verrouillage d'image Windows (`0x80070005 — Accès refusé`) lors du téléchargement d'une nouvelle version dans le dossier de l'exécutable actif.
- **Bouton « Mettre à jour et redémarrer »** : accessible en un clic dans Paramètres > Mises à jour dès qu'une version supérieure est disponible.
- **Parcours 100 % automatisé** : téléchargement en tâche de fond, contrôle cryptographique SHA-256, fermeture ordonnée, remplacement atomique et relance immédiate de Malvision.
- **Téléchargement manuel conservé** : pour les utilisateurs préférant choisir manuellement l'emplacement de sauvegarde, un bouton discret « Téléchargement manuel (.exe)… » reste disponible.

### 2. Architecture relais éphémère et Sécurité durcie (Single Portable Executable garanti)
- **Aucun installateur externe, aucun service d'arrière-plan, aucune tâche planifiée** : Malvision reste un seul et unique exécutable portable.
- **Jeton CSPRNG 256 bits et intégrité HMAC** : génération du jeton d'authentification par le RNG cryptographique Windows (`BCryptGenRandom`) sans aucun fallback prévisible, et scellement d'intégrité du manifeste par HMAC SHA-256.
- **Vérification obligatoire du processus parent & anti-recyclage de PID** : contrôle obligatoire et strict du chemin d'image (`QueryFullProcessImageNameW`) et validation de l'horodatage de création du processus (`GetProcessTimes`), interdisant tout détournement de processus ou réutilisation de PID.
- **Règles de sécurité sur la cible** : extension `.exe` obligatoire et interdiction stricte de cibler les répertoires système Windows (`SystemRoot`).
- **Confirmation explicite de santé (Health Check)** :
  - Surveillance active par événement Win32 dédié (`Local\Malvision_Health_<token>`) et drapeau fichier.
  - Le binaire précédent (`Malvision.exe.bak`) est conservé jusqu'à validation complète de l'initialisation de l'application (DPI, single-instance, COM/ActiveX, SQLite, création de fenêtre).
  - En cas de crash ou d'anomalie au démarrage, le relais termine le processus si nécessaire pour libérer le verrou, restaure l'ancienne version, vérifie son empreinte et la relance.
  - Enregistrement d'une directive de récupération différée (`pending_recovery.json`) si un verrou temporaire empêchait le rollback immédiat.
- **Protection des sessions distantes** : détection et avertissement clair si des sessions RDP sont en cours avec demande de confirmation explicite avant fermeture. Les sessions AnyDesk et RustDesk externes ne sont jamais interrompues.
- **Nettoyage automatique sécurisé** : purge des fichiers temporaires uniquement après confirmation de bon fonctionnement.

---

## Contrôle d'intégrité officiel

| Fichier | Taille (octets) | Empreinte SHA-256 |
| :--- | :--- | :--- |
| `Malvision.exe` | `5 267 968` | `318da16e833cd2e0237a6031ba96694d2e5fb8786be367aa2f09639ce0fee05c` |

---

## Validation et Tests

- **87 tests unitaires et d'intégration réussis (100 % passants)**.
- **Clippy et rustfmt stricts** sans avertissements.
- **Validation Windows 11 x64** avec reproduction réelle de scénarios nominaux, d'altération de manifeste et de crash avec rollback vérifié.
