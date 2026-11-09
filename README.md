# Malvision

Application native Windows 11 x64 de gestion de connexions distantes, écrite en Rust avec `windows-rs` (Win32 + COM/ActiveX).

## Vision du projet

- **RDP** : connexion intégrée directement dans la fenêtre Malvision via le contrôle ActiveX officiel Microsoft (`MsTscAx.dll` / `IMsRdpClient9`).
- **AnyDesk** : lancement du client externe (jalon ultérieur).
- **RustDesk** : lancement du client externe (jalon ultérieur).
- **Distribution** : un unique exécutable portable `Malvision.exe` (~295 Ko en Release) sans dépendances DLL tierces ni installateur.

## Architecture technique (Jalon 1 & Jalon 1B)

- [`src/main.rs`](src/main.rs) : Point d'entrée Windows (`windows_subsystem = "windows"`), activation HiDPI Per-Monitor V2 (`SetProcessDpiAwarenessContext`), initialisation COM/OLE STA (`OleInitialize`) et lancement de l'interface.
- [`src/rdp.rs`](src/rdp.rs) : Hébergement ActiveX natif via `atl.dll` (`AtlAxWinInit` / `AtlAxWin` / `AtlAxGetControl`) et interfaçage COM direct avec `MsTscAx.dll` :
  - `IMsRdpClient9` (`{28904001-04b6-436c-a55b-0af1a0883dc9}`) : configuration `Server`, `Domain`, `UserName`, `DesktopWidth`, `DesktopHeight`, `ColorDepth`, appels `Connect()`, `Disconnect()`, `get_Connected()`, `UpdateSessionDisplaySettings()` et décodage des erreurs via `ExtendedDisconnectReason` + `GetErrorDescription()`.
  - `IMsRdpClientAdvancedSettings8` (`{89acb528-2557-4d16-8625-226a30e97e9a}`) : configuration `RDPPort`, `SmartSizing = true`, `EnableCredSspSupport = true` (NLA), `AuthenticationLevel = 2` (avertissement certificat standard Windows), `DisplayConnectionBar = false`, `ClearTextPassword`.
  - `IMsRdpClientNonScriptable3` (`{b3378d90-0728-45c7-8ed7-b6159fb92219}`) : attachement de la fenêtre parente (`UIParentWindowHandle`) pour les boîtes de dialogue système CredSSP/NLA et certificats, `NegotiateSecurityLayer = true`, `EnableCredSspSupport = true`, gestion en mémoire de `ClearTextPassword` / `ResetPassword`.
  - `IMsTscAxEvents` (`{336d5562-efa8-482e-8cb3-c5c0fc7a7db6}`) : sink COM dédié (`RdpEventSinkComObject`) répondant à `QueryInterface` pour `DIID_IMsTscAxEvents`, `IDispatch` et `IUnknown`, connecté via `IConnectionPoint::Advise` et relayant en temps réel les événements (`OnConnecting`, `OnConnected`, `OnLoginComplete`, `OnDisconnected`, `OnFatalError`, `OnWarning`, `OnAuthenticationWarningDisplayed`, `OnAutoReconnecting2`) à la fenêtre Win32.
- [`src/ui.rs`](src/ui.rs) :
  - Fenêtre principale Win32, barre de connexion (Hôte/IP:port, Utilisateur/Domaine, Mot de passe masqué effacé de l'UI dès l'envoi, boutons Connecter / Déconnecter), barre d'état, redimensionnement dynamique (`WM_SIZE`, `WM_GETMINMAXINFO`).
  - Double validation de l'état de connexion : écoute événementielle `IMsTscAxEvents` + supervision périodique de `IMsRdpClient::get_Connected()` (`WM_TIMER`) garantissant que l'état « Connecté » n'est affiché que lorsque le contrôle ActiveX confirme `Connected == 1`.
  - Préservation des styles de cadre `WS_OVERLAPPEDWINDOW` (`WS_THICKFRAME | WS_MINIMIZEBOX | WS_MAXIMIZEBOX | WS_SYSMENU`) et filtrage strict de la pompe de messages (`is_keyboard_message`) pour que `IOleInPlaceActiveObject::TranslateAccelerator` et `IsDialogMessageW` n'interceptent jamais les clics souris sur les boutons Minimiser / Maximiser / Restaurer de la barre de titre.

## Compilation et Validation

```powershell
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo build --release
```

Binaire généré : `target/release/Malvision.exe`

## Procédure de vérification manuelle (Session RDP réelle)

1. Lancer `.\target\release\Malvision.exe`.
2. Vérifier dans la barre d'état que le contrôle ActiveX et son sink d'événements sont initialisés (`sink=#1`).
3. Saisir un hôte RDP valide, un utilisateur et un mot de passe, puis cliquer sur **Connecter**.
4. Vérifier que la barre d'état passe à **« Connecté à <hôte:port> — ... »** dès que le bureau distant est établi.
5. Maximiser la fenêtre Malvision, puis la **Restaurer** et la **Minimiser** / **Restaurer** pendant la session active.
6. Cliquer sur **Déconnecter** (puis tester une **Reconnexion**).
