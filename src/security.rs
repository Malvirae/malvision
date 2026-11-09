//! Protection des secrets (mots de passe) via Windows DPAPI (`CryptProtectData` / `CryptUnprotectData`)
//! avec portée utilisateur courant (`CurrentUser`).
//!
//! # Sécurité
//! - Aucun mot de passe n'est jamais stocké en clair dans SQLite ni écrit dans les logs.
//! - Le chiffrement est lié au compte Windows de l'utilisateur courant sur la machine locale :
//!   un blob chiffré ne peut pas être déchiffré par un autre utilisateur ou sur une autre machine.
//! - Les buffers contenant un secret en clair sont effacés en mémoire de manière best-effort
//!   (`write_volatile`) dès qu'ils ne sont plus nécessaires.

use std::ffi::c_void;
use windows::Win32::Foundation::{HLOCAL, LocalFree};
use windows::Win32::Security::Cryptography::{
    CRYPT_INTEGER_BLOB, CRYPTPROTECT_UI_FORBIDDEN, CryptProtectData, CryptUnprotectData,
};
use windows_core::PCWSTR;

/// Entropie applicative additionnelle liée à Malvision pour isoler les blobs DPAPI
/// des autres applications de la session utilisateur.
const MALVISION_DPAPI_ENTROPY: &[u8] = b"Malvision::Credentials::v1::DPAPI";

/// Chaîne sensible en mémoire dont le contenu est automatiquement mis à zéro lors du `Drop`.
#[derive(Clone)]
pub struct SecretString {
    inner: String,
}

impl SecretString {
    pub fn new(value: String) -> Self {
        Self { inner: value }
    }

    pub fn as_str(&self) -> &str {
        &self.inner
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    pub fn clear(&mut self) {
        zeroize_string(&mut self.inner);
    }
}

impl std::fmt::Debug for SecretString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SecretString([REDACTED])")
    }
}

impl Drop for SecretString {
    fn drop(&mut self) {
        zeroize_string(&mut self.inner);
    }
}

/// Efface de manière best-effort le contenu d'un `String` en mémoire vive.
pub fn zeroize_string(s: &mut String) {
    unsafe {
        for b in s.as_bytes_mut() {
            std::ptr::write_volatile(b, 0);
        }
    }
    s.clear();
}

/// Efface de manière best-effort le contenu d'un `Vec<u8>` en mémoire vive.
pub fn zeroize_bytes(buf: &mut Vec<u8>) {
    for b in buf.iter_mut() {
        unsafe {
            std::ptr::write_volatile(b, 0);
        }
    }
    buf.clear();
}

/// Chiffre une chaîne secrète avec Windows DPAPI (portée utilisateur courant, sans UI).
pub fn encrypt_secret(plaintext: &str) -> Result<Vec<u8>, String> {
    if plaintext.is_empty() {
        return Ok(Vec::new());
    }

    let mut plain_bytes = plaintext.as_bytes().to_vec();
    let in_blob = CRYPT_INTEGER_BLOB {
        cbData: plain_bytes.len() as u32,
        pbData: plain_bytes.as_mut_ptr(),
    };

    let mut entropy = MALVISION_DPAPI_ENTROPY.to_vec();
    let entropy_blob = CRYPT_INTEGER_BLOB {
        cbData: entropy.len() as u32,
        pbData: entropy.as_mut_ptr(),
    };

    let mut out_blob = CRYPT_INTEGER_BLOB::default();

    let res = unsafe {
        CryptProtectData(
            &in_blob,
            PCWSTR::null(),
            Some(&entropy_blob),
            None,
            None,
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut out_blob,
        )
    };

    zeroize_bytes(&mut plain_bytes);
    zeroize_bytes(&mut entropy);

    res.map_err(|_| {
        "Échec du chiffrement DPAPI du secret (contexte utilisateur Windows).".to_string()
    })?;

    if out_blob.pbData.is_null() || out_blob.cbData == 0 {
        return Err("DPAPI a retourné un bloc chiffré vide.".to_string());
    }

    let encrypted = unsafe {
        let slice = std::slice::from_raw_parts(out_blob.pbData, out_blob.cbData as usize);
        let vec = slice.to_vec();
        let _ = LocalFree(Some(HLOCAL(out_blob.pbData as *mut c_void)));
        vec
    };

    Ok(encrypted)
}

/// Déchiffre un blob DPAPI uniquement au moment où le secret est nécessaire (ex. ouverture de session RDP).
pub fn decrypt_secret(ciphertext: &[u8]) -> Result<SecretString, String> {
    if ciphertext.is_empty() {
        return Ok(SecretString::new(String::new()));
    }

    let mut cipher_copy = ciphertext.to_vec();
    let in_blob = CRYPT_INTEGER_BLOB {
        cbData: cipher_copy.len() as u32,
        pbData: cipher_copy.as_mut_ptr(),
    };

    let mut entropy = MALVISION_DPAPI_ENTROPY.to_vec();
    let entropy_blob = CRYPT_INTEGER_BLOB {
        cbData: entropy.len() as u32,
        pbData: entropy.as_mut_ptr(),
    };

    let mut out_blob = CRYPT_INTEGER_BLOB::default();

    let res = unsafe {
        CryptUnprotectData(
            &in_blob,
            None,
            Some(&entropy_blob),
            None,
            None,
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut out_blob,
        )
    };

    zeroize_bytes(&mut cipher_copy);
    zeroize_bytes(&mut entropy);

    res.map_err(|_| {
        "Impossible de déchiffrer le mot de passe via Windows DPAPI (secret lié à un autre profil utilisateur ou données altérées).".to_string()
    })?;

    if out_blob.pbData.is_null() {
        return Ok(SecretString::new(String::new()));
    }

    let result = unsafe {
        let slice = std::slice::from_raw_parts_mut(out_blob.pbData, out_blob.cbData as usize);
        let decoded = String::from_utf8(slice.to_vec())
            .map_err(|_| "Le secret déchiffré n'est pas une chaîne UTF-8 valide.".to_string());
        for b in slice.iter_mut() {
            std::ptr::write_volatile(b, 0);
        }
        let _ = LocalFree(Some(HLOCAL(out_blob.pbData as *mut c_void)));
        decoded
    }?;

    Ok(SecretString::new(result))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dpapi_encrypt_decrypt_roundtrip() {
        let original = "P@ssw0rd!TrèsComplexe#2026";
        let encrypted = encrypt_secret(original).expect("Chiffrement DPAPI");
        assert!(!encrypted.is_empty());
        // Vérifier que le texte en clair n'apparaît pas dans le binaire chiffré
        assert!(
            !encrypted
                .windows(original.len())
                .any(|w| w == original.as_bytes()),
            "Le mot de passe ne doit jamais être en clair dans le blob DPAPI"
        );

        let decrypted = decrypt_secret(&encrypted).expect("Déchiffrement DPAPI");
        assert_eq!(decrypted.as_str(), original);

        // Vérifier que Debug ne divulgue jamais le secret
        let dbg_str = format!("{decrypted:?}");
        assert!(!dbg_str.contains("P@ssw0rd"));
        assert!(dbg_str.contains("REDACTED"));
    }

    #[test]
    fn test_dpapi_rejects_corrupted_ciphertext() {
        let mut encrypted = encrypt_secret("SecretTest").expect("Chiffrement");
        let last = encrypted.len() - 1;
        encrypted[last] ^= 0xFF;
        let res = decrypt_secret(&encrypted);
        assert!(
            res.is_err(),
            "Un blob DPAPI altéré doit être rejeté explicitement"
        );
    }

    #[test]
    fn test_dpapi_empty_secret() {
        let encrypted = encrypt_secret("").unwrap();
        assert!(encrypted.is_empty());
        let decrypted = decrypt_secret(&encrypted).unwrap();
        assert!(decrypted.is_empty());
    }
}
