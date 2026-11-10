#![cfg_attr(not(test), windows_subsystem = "windows")]

mod bootstrap;
mod external_apps;
mod models;
mod rdp;
mod security;
mod sessions;
mod settings;
mod single_instance;
mod storage;
mod ui;
mod updater;

use windows::Win32::System::Com::CoUninitialize;
use windows::Win32::System::Ole::{OleInitialize, OleUninitialize};
use windows::Win32::UI::Controls::{
    ICC_STANDARD_CLASSES, ICC_TREEVIEW_CLASSES, INITCOMMONCONTROLSEX, InitCommonControlsEx,
};
use windows::Win32::UI::HiDpi::{
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetProcessDpiAwarenessContext,
};

fn main() {
    // 0. Interception prioritaire : mode auxiliaire de mise à jour différée (--apply-update)
    // Ce mode s'exécute sans interface, sans toucher à SQLite ni au mutex d'instance unique.
    let args: Vec<String> = std::env::args().collect();
    if args.len() >= 2 && args[1] == "--apply-update" {
        updater::run_apply_update_mode(&args[2..]);
        return;
    }

    // Réception du jeton de santé si Malvision a été relancé post-mise à jour
    if args.len() >= 3 && args[1] == "--health-check" {
        updater::set_pending_health_token(args[2].clone());
    }

    // 1. Appliquer une éventuelle restauration différée si le rollback précédent était bloqué
    updater::check_and_apply_pending_recovery();

    // 2. Nettoyage préventif des fichiers résiduels d'une éventuelle mise à jour précédente (.bak, helper)
    updater::cleanup_leftover_update_files();

    // 3. Contrôle d'instance unique pour la session Windows interactive
    // Si une instance de Malvision est déjà active (visible ou minimisée dans le System Tray),
    // elle est immédiatement réactivée et restaurée, et le processus secondaire quitte.
    let _instance_guard = if !cfg!(test) {
        match single_instance::check_single_instance() {
            single_instance::SingleInstanceResult::Primary(guard) => Some(guard),
            single_instance::SingleInstanceResult::SecondaryExited => return,
        }
    } else {
        None
    };

    unsafe {
        // Activer la prise en charge HiDPI Per-Monitor V2 sur Windows 11
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);

        // Initialiser OLE / COM en mode STA (requis pour l'hébergement ActiveX MsTscAx)
        if let Err(err) = OleInitialize(None) {
            eprintln!("Échec de OleInitialize : {err}");
            return;
        }

        let icc = INITCOMMONCONTROLSEX {
            dwSize: std::mem::size_of::<INITCOMMONCONTROLSEX>() as u32,
            dwICC: ICC_STANDARD_CLASSES | ICC_TREEVIEW_CLASSES,
        };
        let _ = InitCommonControlsEx(&icc);

        let result = ui::run_app();

        OleUninitialize();
        CoUninitialize();

        if let Err(err) = result {
            eprintln!("Erreur fatale Malvision : {err}");
        }
    }
}
