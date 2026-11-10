//! Gestion de l'instance unique de Malvision pour la session utilisateur Windows.
//!
//! Ce module garantit qu'une seule instance de Malvision ne s'exécute par session Windows interactive.
//! En cas de tentative de lancement d'une deuxième instance (raccourci barre des tâches, Menu Démarrer,
//! bureau, ou démarrage automatique) :
//! - La deuxième instance ne crée aucune fenêtre ni connexion SQLite.
//! - Elle signale à l'instance principale de se réactiver, de se restaurer (depuis le System Tray ou
//!   son état minimisé), et de reprendre le focus.
//! - Le processus secondaire quitte immédiatement et silencieusement avec un code de succès.

use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use windows::Win32::Foundation::{
    CloseHandle, ERROR_ALREADY_EXISTS, GetLastError, HANDLE, HWND, LPARAM, WAIT_ABANDONED,
    WAIT_OBJECT_0, WAIT_TIMEOUT, WPARAM,
};
use windows::Win32::System::Threading::{CreateMutexW, ReleaseMutex, WaitForSingleObject};
use windows::Win32::UI::WindowsAndMessaging::{
    AllowSetForegroundWindow, FindWindowW, GetWindowThreadProcessId, IsWindow, PostMessageW,
    RegisterWindowMessageW,
};
use windows_core::w;

/// Nom du message Win32 enregistré pour réactiver l'instance existante.
const ACTIVATE_MSG_NAME: &windows_core::PCWSTR = &w!("Malvision_ActivateInstance_Message");

/// Nom du mutex Win32 de session (portée locale à la session Windows de l'utilisateur).
const MUTEX_NAME: &windows_core::PCWSTR = &w!("Local\\Malvision_SingleInstance_Mutex");

static VOLUNTARY_RESTART_FLAG: AtomicBool = AtomicBool::new(false);
static GLOBAL_MUTEX_HANDLE: OnceLock<usize> = OnceLock::new();

/// Retourne l'identifiant numérique du message Win32 enregistré pour l'activation.
pub fn get_activate_window_message() -> u32 {
    static MSG_ID: OnceLock<u32> = OnceLock::new();
    *MSG_ID.get_or_init(|| unsafe { RegisterWindowMessageW(*ACTIVATE_MSG_NAME) })
}

/// Résultat de la vérification d'instance unique.
pub enum SingleInstanceResult {
    /// Première instance principale. Doit conserver le guard pour maintenir le verrou pendant son exécution.
    Primary(SingleInstanceGuard),
    /// Une instance principale existait déjà et a été notifiée/réactivée. Le processus secondaire doit quitter.
    SecondaryExited,
}

/// Gardien RAII du mutex de session unique. Libère et ferme le mutex lors de la destruction.
pub struct SingleInstanceGuard {
    handle: HANDLE,
}

impl Drop for SingleInstanceGuard {
    fn drop(&mut self) {
        if !self.handle.is_invalid() {
            unsafe {
                let _ = ReleaseMutex(self.handle);
                let _ = CloseHandle(self.handle);
            }
        }
    }
}

/// Vérifie si une instance de Malvision est déjà active dans la session Windows.
///
/// Si aucune instance n'est active :
/// - Acquiert le mutex nommé et retourne `SingleInstanceResult::Primary`.
///
/// Si une instance est déjà active :
/// - Tente de localiser sa fenêtre principale pendant un maximum de 2,5 secondes.
/// - Autorise l'instance principale à prendre le premier plan (`AllowSetForegroundWindow`).
/// - Envoie le message `MALVISION_ACTIVATE_INSTANCE` pour la restaurer (du tray ou minimisée) et la focaliser.
/// - Retourne `SingleInstanceResult::SecondaryExited`.
pub fn check_single_instance() -> SingleInstanceResult {
    unsafe {
        let handle = match CreateMutexW(None, true, *MUTEX_NAME) {
            Ok(h) => h,
            Err(_) => {
                // En cas d'impossibilité de créer le mutex, autoriser le démarrage par mesure de résilience
                return SingleInstanceResult::Primary(SingleInstanceGuard {
                    handle: HANDLE::default(),
                });
            }
        };

        let last_err = GetLastError();

        if last_err != ERROR_ALREADY_EXISTS {
            // Nous sommes la première instance et possédons le mutex
            let _ = GLOBAL_MUTEX_HANDLE.set(handle.0 as usize);
            return SingleInstanceResult::Primary(SingleInstanceGuard { handle });
        }

        // Le mutex existe déjà. Vérifier si le propriétaire précédent est toujours vivant ou s'il a crashé
        let wait_res = WaitForSingleObject(handle, 0);
        if wait_res == WAIT_ABANDONED || wait_res == WAIT_OBJECT_0 {
            // Le processus précédent s'est terminé brutalement sans libérer le mutex.
            // Le système d'exploitation transfère la propriété à ce thread.
            let _ = GLOBAL_MUTEX_HANDLE.set(handle.0 as usize);
            return SingleInstanceResult::Primary(SingleInstanceGuard { handle });
        }

        if wait_res == WAIT_TIMEOUT {
            // L'instance précédente est bien vivante et détient le verrou.
            // Rechercher la fenêtre principale pendant un court délai pour lui envoyer le signal de réveil.
            let msg = get_activate_window_message();
            let mut primary_hwnd = HWND::default();

            for _ in 0..25 {
                let h = FindWindowW(w!("MalvisionMainWindowClass"), None).unwrap_or_default();
                if !h.is_invalid() && IsWindow(Some(h)).as_bool() {
                    primary_hwnd = h;
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(100));
            }

            if !primary_hwnd.is_invalid() {
                let mut pid = 0u32;
                let _ = GetWindowThreadProcessId(primary_hwnd, Some(&mut pid));
                if pid != 0 {
                    let _ = AllowSetForegroundWindow(pid);
                }
                let _ = PostMessageW(Some(primary_hwnd), msg, WPARAM(0), LPARAM(0));
            }

            let _ = CloseHandle(handle);
            return SingleInstanceResult::SecondaryExited;
        }

        // Cas par défaut : démarrer
        SingleInstanceResult::Primary(SingleInstanceGuard { handle })
    }
}

/// Marque le redémarrage volontaire de Malvision (ex: changement d'emplacement SQLite)
/// et libère immédiatement le mutex d'instance unique afin que la nouvelle instance puisse démarrer sans blocage.
pub fn prepare_for_voluntary_restart() {
    VOLUNTARY_RESTART_FLAG.store(true, Ordering::SeqCst);
    if let Some(&raw_h) = GLOBAL_MUTEX_HANDLE.get() {
        let handle = HANDLE(raw_h as *mut _);
        if !handle.is_invalid() {
            unsafe {
                let _ = ReleaseMutex(handle);
                let _ = CloseHandle(handle);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_activate_message_registration() {
        let msg = get_activate_window_message();
        assert!(msg >= 0xC000, "Le message enregistré doit être >= 0xC000");
        assert_eq!(msg, get_activate_window_message());
    }

    #[test]
    fn test_single_instance_guard_drop_clean() {
        let guard = SingleInstanceGuard {
            handle: HANDLE::default(),
        };
        drop(guard);
    }
}
