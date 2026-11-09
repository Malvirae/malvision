#![cfg_attr(not(test), windows_subsystem = "windows")]

mod rdp;
mod ui;

use windows::Win32::System::Com::CoUninitialize;
use windows::Win32::System::Ole::{OleInitialize, OleUninitialize};
use windows::Win32::UI::Controls::{
    ICC_STANDARD_CLASSES, INITCOMMONCONTROLSEX, InitCommonControlsEx,
};
use windows::Win32::UI::HiDpi::{
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetProcessDpiAwarenessContext,
};

fn main() {
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
            dwICC: ICC_STANDARD_CLASSES,
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
