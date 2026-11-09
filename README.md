# Malvision

Application native Windows 11 x64 de gestion de connexions distantes, écrite en Rust avec `windows-rs` (Win32 + COM/ActiveX).

## Vision du projet

- **RDP** : connexion intégrée directement dans la fenêtre Malvision via le contrôle ActiveX officiel Microsoft (`MsTscAx.dll` / `IMsRdpClient9`).
- **AnyDesk** : lancement du client externe (jalon ultérieur).
- **RustDesk** : lancement du client externe (jalon ultérieur).
- **Distribution** : un unique exécutable portable `Malvision.exe` (~293 Ko en Release) sans dépendances DLL tierces ni installateur.

## Architecture technique (Jalon 1)

- [`src/main.rs`](src/main.rs) : Point d'entrée Windows (`windows_subsystem = "windows"`), activation HiDPI Per-Monitor V2 (`SetProcessDpiAwarenessContext`), initialisation COM/OLE STA (`OleInitialize`) et lancement de l'interface.
- [`src/rdp.rs`](src/rdp.rs) : Hébergement ActiveX natif via `atl.dll` (`AtlAxWinInit` / `AtlAxWin` / `AtlAxGetControl`) et interfaçage COM direct avec `MsTscAx.dll` :
  - `IMsRdpClient9` (`{28904001-04b6-436c-a55b-0af1a0883dc9}`) : configuration `Server`, `Domain`, `UserName`, `DesktopWidth`, `DesktopHeight`, `ColorDepth`, appels `Connect()`, `Disconnect()`, `UpdateSessionDisplaySettings()` et décodage des erreurs via `ExtendedDisconnectReason` + `GetErrorDescription()`.
  - `IMsRdpClientAdvancedSettings8` (`{89acb528-2557-4d16-8625-226a30e97e9a}`) : configuration `RDPPort`, `SmartSizing = true`, `EnableCredSspSupport = true` (NLA), `AuthenticationLevel = 2` (avertissement certificat standard Windows), `DisplayConnectionBar = false`, `ClearTextPassword`.
  - `IMsRdpClientNonScriptable3` (`{b3378d90-0728-45c7-8ed7-b6159fb92219}`) : attachement de la fenêtre parente (`UIParentWindowHandle`) pour les boîtes de dialogue système CredSSP/NLA et certificats, `NegotiateSecurityLayer = true`, `EnableCredSspSupport = true`, gestion en mémoire de `ClearTextPassword` / `ResetPassword`.
  - `IMsTscAxEvents` (`{336d5562-efa8-482e-8cb3-c5c0fc7a7db6}`) : sink COM `IDispatch` connecté via `IConnectionPointContainer` / `IConnectionPoint::Advise`, relayant en temps réel les événements (`OnConnecting`, `OnConnected`, `OnLoginComplete`, `OnDisconnected`, `OnFatalError`, `OnWarning`, `OnAuthenticationWarningDisplayed`, `OnAutoReconnecting2`) à la fenêtre Win32.
- [`src/ui.rs`](src/ui.rs) : Fenêtre principale Win32, barre de connexion (Hôte/IP:port, Utilisateur/Domaine, Mot de passe masqué effacé de l'UI dès l'envoi, boutons Connecter / Déconnecter), barre d'état, redimensionnement dynamique (`WM_SIZE`), et routage clavier/souris OLE (`IOleInPlaceActiveObject::TranslateAccelerator` + `OLEIVERB_UIACTIVATE`).

## Compilation et Validation

```powershell
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo build --release
```

Binaire généré : `target/release/Malvision.exe`

## Procédure de test manuel d'une session RDP réelle

1. Lancer `.\target\release\Malvision.exe`.
2. Vérifier dans la barre d'état inférieure que le contrôle ActiveX `MsTscAx.dll` est bien initialisé (ex. `MsTscAx.dll v10.0.26100, {1DF7C823-B2D4-4B54-975A-F2AC5D7CF8B8}`).
3. Dans **Hôte / IP**, saisir l'adresse IP ou le nom DNS d'une machine Windows distante autorisant RDP (ex. `192.168.1.50` ou `192.168.1.50:3389`).
4. Dans **Utilisateur**, saisir le compte distant (ex. ` Utilisateur` ou `DOMAINE\Utilisateur`).
5. Dans **Mot de passe**, saisir le mot de passe (ou laisser vide pour que la boîte de dialogue Windows Security / CredSSP native s'affiche).
6. Cliquer sur **Connecter** (ou appuyer sur `Entrée`) :
   - Si le certificat TLS du serveur n'est pas reconnu, la boîte de dialogue d'avertissement de certificat Windows officielle s'affiche, parentée à la fenêtre Malvision.
   - Une fois authentifié (NLA / CredSSP), le bureau distant s'affiche directement à l'intérieur de la fenêtre Malvision.
7. Tester le clavier et la souris dans la session distante, ainsi que le redimensionnement de la fenêtre Malvision.
8. Cliquer sur **Déconnecter** pour fermer proprement la session RDP sans fermer l'application.
