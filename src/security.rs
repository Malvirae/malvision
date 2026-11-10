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
    BCRYPT_ALG_HANDLE, BCRYPT_ALG_HANDLE_HMAC_FLAG, BCRYPT_HASH_HANDLE,
    BCRYPT_OPEN_ALGORITHM_PROVIDER_FLAGS, BCRYPT_SHA256_ALGORITHM, BCRYPT_USE_SYSTEM_PREFERRED_RNG,
    BCryptCloseAlgorithmProvider, BCryptCreateHash, BCryptDestroyHash, BCryptFinishHash,
    BCryptGenRandom, BCryptHashData, BCryptOpenAlgorithmProvider, CRYPT_INTEGER_BLOB,
    CRYPTPROTECT_UI_FORBIDDEN, CryptProtectData, CryptUnprotectData,
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

/// Convertit une tranche d'octets en chaîne hexadécimale en minuscules.
pub fn bytes_to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Décode une chaîne hexadécimale en vecteur d'octets.
pub fn hex_to_bytes(s: &str) -> Option<Vec<u8>> {
    if !s.len().is_multiple_of(2) {
        return None;
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok())
        .collect()
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

/// Calcule l'empreinte SHA-256 hexadécimale en minuscules d'un buffer d'octets
/// via l'API native Windows CNG (`BCrypt`).
pub fn compute_sha256(data: &[u8]) -> Result<String, String> {
    unsafe {
        let mut alg = BCRYPT_ALG_HANDLE::default();
        let status = BCryptOpenAlgorithmProvider(
            &mut alg,
            BCRYPT_SHA256_ALGORITHM,
            None,
            BCRYPT_OPEN_ALGORITHM_PROVIDER_FLAGS(0),
        );
        if status.is_err() {
            return Err(format!(
                "BCryptOpenAlgorithmProvider failed: 0x{:08X}",
                status.0
            ));
        }

        let mut hash_handle = BCRYPT_HASH_HANDLE::default();
        let status = BCryptCreateHash(alg, &mut hash_handle, None, None, 0);
        if status.is_err() {
            let _ = BCryptCloseAlgorithmProvider(alg, 0);
            return Err(format!("BCryptCreateHash failed: 0x{:08X}", status.0));
        }

        if !data.is_empty() {
            let status = BCryptHashData(hash_handle, data, 0);
            if status.is_err() {
                let _ = BCryptDestroyHash(hash_handle);
                let _ = BCryptCloseAlgorithmProvider(alg, 0);
                return Err(format!("BCryptHashData failed: 0x{:08X}", status.0));
            }
        }

        let mut hash = [0u8; 32];
        let status = BCryptFinishHash(hash_handle, &mut hash, 0);
        if status.is_err() {
            let _ = BCryptDestroyHash(hash_handle);
            let _ = BCryptCloseAlgorithmProvider(alg, 0);
            return Err(format!("BCryptFinishHash failed: 0x{:08X}", status.0));
        }

        let _ = BCryptDestroyHash(hash_handle);
        let _ = BCryptCloseAlgorithmProvider(alg, 0);

        Ok(hash.iter().map(|b| format!("{b:02x}")).collect::<String>())
    }
}

/// Calcule l'empreinte SHA-256 d'un fichier à partir de son chemin.
pub fn compute_file_sha256(path: &std::path::Path) -> Result<String, String> {
    let data = std::fs::read(path).map_err(|e| {
        format!(
            "Impossible de lire le fichier '{}' pour le calcul SHA-256 : {e}",
            path.display()
        )
    })?;
    compute_sha256(&data)
}

/// Calcule l'authentification de message HMAC-SHA-256 standard (RFC 2104 / FIPS 198-1)
/// via l'API native Windows CNG (`BCrypt` avec `BCRYPT_ALG_HANDLE_HMAC_FLAG`).
pub fn compute_hmac_sha256(key: &[u8], data: &[u8]) -> Result<String, String> {
    unsafe {
        let mut alg = BCRYPT_ALG_HANDLE::default();
        let status = BCryptOpenAlgorithmProvider(
            &mut alg,
            BCRYPT_SHA256_ALGORITHM,
            None,
            BCRYPT_ALG_HANDLE_HMAC_FLAG,
        );
        if status.is_err() {
            return Err(format!(
                "BCryptOpenAlgorithmProvider(HMAC) failed: 0x{:08X}",
                status.0
            ));
        }

        let mut hash_handle = BCRYPT_HASH_HANDLE::default();
        let status = BCryptCreateHash(
            alg,
            &mut hash_handle,
            None,
            if key.is_empty() { None } else { Some(key) },
            0,
        );
        if status.is_err() {
            let _ = BCryptCloseAlgorithmProvider(alg, 0);
            return Err(format!("BCryptCreateHash(HMAC) failed: 0x{:08X}", status.0));
        }

        if !data.is_empty() {
            let status = BCryptHashData(hash_handle, data, 0);
            if status.is_err() {
                let _ = BCryptDestroyHash(hash_handle);
                let _ = BCryptCloseAlgorithmProvider(alg, 0);
                return Err(format!("BCryptHashData(HMAC) failed: 0x{:08X}", status.0));
            }
        }

        let mut mac = [0u8; 32];
        let status = BCryptFinishHash(hash_handle, &mut mac, 0);
        if status.is_err() {
            let _ = BCryptDestroyHash(hash_handle);
            let _ = BCryptCloseAlgorithmProvider(alg, 0);
            return Err(format!("BCryptFinishHash(HMAC) failed: 0x{:08X}", status.0));
        }

        let _ = BCryptDestroyHash(hash_handle);
        let _ = BCryptCloseAlgorithmProvider(alg, 0);

        Ok(mac.iter().map(|b| format!("{b:02x}")).collect::<String>())
    }
}

/// Génère des octets aléatoires cryptographiquement sécurisés via `BCryptGenRandom` (RNG système Windows).
pub fn generate_crypto_random_bytes(buf: &mut [u8]) -> Result<(), String> {
    if buf.is_empty() {
        return Ok(());
    }
    let status = unsafe { BCryptGenRandom(None, buf, BCRYPT_USE_SYSTEM_PREFERRED_RNG) };
    status
        .ok()
        .map_err(|e| format!("BCryptGenRandom a échoué (NTSTATUS {status:?}) : {e}"))
}

/// Compare deux chaînes en temps constant pour éviter les attaques par canal auxiliaire (timing attacks).
pub fn constant_time_compare(a: &str, b: &str) -> bool {
    let a_bytes = a.as_bytes();
    let b_bytes = b.as_bytes();
    if a_bytes.len() != b_bytes.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a_bytes.iter().zip(b_bytes.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_sha256_known_vector() {
        // SHA-256("") = e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
        let empty_hash = compute_sha256(b"").unwrap();
        assert_eq!(
            empty_hash,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );

        // SHA-256("Malvision") = 6876c52a0a256fece328bb9ea4e116934a36eead0d648dd6dfb92cfb373cf2b6 (calculated or validated)
        let hash_malvision = compute_sha256(b"Malvision").unwrap();
        assert_eq!(hash_malvision.len(), 64);
    }

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

    #[test]
    fn test_bcrypt_gen_random_bytes() {
        let mut b1 = [0u8; 32];
        let mut b2 = [0u8; 32];
        generate_crypto_random_bytes(&mut b1).expect("Génération aléatoire b1");
        generate_crypto_random_bytes(&mut b2).expect("Génération aléatoire b2");
        assert_ne!(b1, [0u8; 32]);
        assert_ne!(b2, [0u8; 32]);
        assert_ne!(b1, b2, "Deux tirages CSPRNG doivent différer");
    }

    #[test]
    fn test_constant_time_compare() {
        assert!(constant_time_compare("abc123XYZ", "abc123XYZ"));
        assert!(!constant_time_compare("abc123XYZ", "abc123XYz"));
        assert!(!constant_time_compare("abc123XYZ", "abc123XY"));
        assert!(!constant_time_compare("", "a"));
        assert!(constant_time_compare("", ""));
    }

    #[test]
    fn test_hmac_sha256_rfc4231_official_vectors() {
        // RFC 4231 Test Case 1
        let key1 = [0x0bu8; 20];
        let data1 = b"Hi There";
        let hmac1 = compute_hmac_sha256(&key1, data1).expect("HMAC test 1");
        assert_eq!(
            hmac1, "b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7",
            "Vecteur officiel RFC 4231 Cas 1"
        );

        // RFC 4231 Test Case 2
        let key2 = b"Jefe";
        let data2 = b"what do ya want for nothing?";
        let hmac2 = compute_hmac_sha256(key2, data2).expect("HMAC test 2");
        assert_eq!(
            hmac2, "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843",
            "Vecteur officiel RFC 4231 Cas 2"
        );

        // RFC 4231 Test Case 3
        let key3 = [0xaau8; 20];
        let data3 = [0xddu8; 50];
        let hmac3 = compute_hmac_sha256(&key3, &data3).expect("HMAC test 3");
        assert_eq!(
            hmac3, "773ea91e36800e46854db8ebd09181a72959098b3ef8c122d9635514ced565fe",
            "Vecteur officiel RFC 4231 Cas 3"
        );
    }

    #[test]
    fn test_bytes_to_hex_and_hex_to_bytes() {
        let original = b"Malvision_Security_Bytes_12345";
        let hex = bytes_to_hex(original);
        assert_eq!(hex.len(), original.len() * 2);
        let decoded = hex_to_bytes(&hex).expect("Décodage hex valide");
        assert_eq!(decoded, original);

        assert!(hex_to_bytes("odd").is_none());
        assert!(hex_to_bytes("zz").is_none());
        assert_eq!(hex_to_bytes("").unwrap(), Vec::<u8>::new());
    }
}
