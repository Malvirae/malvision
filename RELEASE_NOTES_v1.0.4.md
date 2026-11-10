# Malvision v1.0.4 — Notes de version

**Date de publication :** 10 novembre 2026  
**Version :** `1.0.4`  
**Binaire officiel :** `Malvision.exe`  
**Architecture :** Windows 11 x64 (binaire portable autonome, sans dépendances externes)  
**Empreinte SHA-256 :** `b988bd627ed4baa624340134cf4a6e2b8b7aa6f902cb0dfe2d834c66199fed98`  
**Taille :** 5 246 976 octets  

---

## Nouveautés majeures de la V1.0.4

### 1. Mise à jour automatique en place avec remplacement et redémarrage
Malvision résout définitivement la contrainte historique du verrouillage d'image Windows (`0x80070005 — Accès refusé`) lors du téléchargement d'une nouvelle version dans le dossier de l'exécutable actif.
- **Bouton « Mettre à jour et redémarrer »** : accessible en un clic dans Paramètres > Mises à jour dès qu'une version supérieure est disponible.
- **Parcours 100 % automatisé** : téléchargement en tâche de fond, contrôle cryptographique SHA-256, fermeture ordonnée, remplacement atomique et relance immédiate de Malvision.
- **Téléchargement manuel conservé** : pour les utilisateurs préférant choisir manuellement l'emplacement de sauvegarde, un bouton discret « Téléchargement manuel (.exe)… » reste disponible.

### 2. Architecture relais éphémère (Single Portable Executable garanti)
- **Aucun installateur externe, aucun service d'arrière-plan, aucune tâche planifiée** : Malvision reste un seul et unique exécutable portable.
- **Processus relais éphémère** : Malvision utilise une copie temporaire de son propre binaire exécutée avec le commutateur silencieux `--apply-update <auth_token>`, court-circuitant le mutex d'instance unique et attendant la fin du processus principal.
- **Remplacement atomique et Rollback sécurisé** :
  - Sauvegarde préalable de l'exécutable actif sous `Malvision.exe.bak`.
  - Contrôle d'intégrité SHA-256 avant et après mise en place du binaire cible.
  - Surveillance active du démarrage : si la nouvelle version ne parvient pas à démarrer ou subit un crash dans les 1,5 seconde, le relais restaure instantanément et automatiquement la version précédente (`Malvision.exe.bak`).
- **Protection des sessions distantes** : détection et avertissement clair si des sessions RDP sont en cours avec demande de confirmation explicite avant fermeture. Les sessions AnyDesk et RustDesk externes ne sont jamais interrompues.
- **Nettoyage automatique** : purge transparente des résidus `.bak` et temporaires lors de tout démarrage normal.

---

## Contrôle d'intégrité officiel

| Fichier | Taille (octets) | Empreinte SHA-256 |
| :--- | :--- | :--- |
| `Malvision.exe` | `5 246 976` | `b988bd627ed4baa624340134cf4a6e2b8b7aa6f902cb0dfe2d834c66199fed98` |

---

## Validation et Tests

- **79 tests unitaires et d'intégration réussis (100 % passants)**.
- **Clippy et rustfmt stricts** sans avertissements.
- **Validation Windows 11 x64** avec reproduction réelle du verrouillage d'image et validation du remplacement après arrêt du processus.
