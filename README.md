# Malvision

Application native Windows 11 x64 de gestion de connexions distantes, écrite en Rust avec `windows-rs` (Win32 + COM/ActiveX).

## Vision du projet

- **RDP** : connexion intégrée directement dans la fenêtre Malvision via le contrôle ActiveX officiel Microsoft (`MsTscAx.dll` / `IMsRdpClient9`).
- **AnyDesk** : lancement du client externe (jalon ultérieur).
- **RustDesk** : lancement du client externe (jalon ultérieur).
- **Distribution** : un unique exécutable portable `Malvision.exe` sans dépendances DLL tierces ni installateur.

## Jalon 1 — Initialisation et prototype RDP intégré

Ce jalon démontre l'hébergement natif d'une véritable session RDP Microsoft à l'intérieur d'une fenêtre Win32 écrite en Rust :

- Création d'un conteneur ActiveX natif (`AtlAxWin` via `atl.dll` système) hébergeant `MsTscAx.MsTscAx` (avec repli ordonné des versions 13 à 9 de `MsTscAx.dll`).
- Barre d'outils native Win32 permettant de renseigner :
  - Hôte (`IP`, `DNS` ou `hôte:port`)
  - Utilisateur (`DOMAINE\utilisateur` ou `utilisateur`)
  - Mot de passe (saisi en mémoire uniquement via `IMsTscNonScriptable::put_ClearTextPassword` / `IMsRdpClientAdvancedSettings::put_ClearTextPassword`, jamais persisté sur disque)
- Lancement et arrêt propres de la session RDP (`Connect` / `Disconnect`).
- Support complet du clavier et de la souris dans la session distante (transfert du focus OLE et pompe de messages `IsDialogMessageW` adaptée).
- Redimensionnement dynamique de la surface RDP avec la fenêtre (`UpdateSessionDisplaySettings` + `SmartSizing`).
- Préservation intégrale des mécanismes de sécurité Windows : NLA (`EnableCredSspSupport = true`), boîte de dialogue d'identifiants/certificats Windows parentée à la fenêtre (`UIParentWindowHandle`, `AuthenticationLevel = 2`).
- Écoute des événements COM `IMsTscAxEvents` (`IDispatch` sink via `IConnectionPoint`) et affichage clair des états et erreurs de connexion (`GetErrorDescription`, `ExtendedDisconnectReason`).

## Compilation

```powershell
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo build --release
```

Binaire généré : `target/release/Malvision.exe`
