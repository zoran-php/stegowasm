use aes_gcm::{
    Aes256Gcm, Key, Nonce,
    aead::{Aead, KeyInit},
};
use getrandom::getrandom;
use pbkdf2::pbkdf2_hmac;
use sha2::Sha256;

const SALT_LEN: usize = 16;
const NONCE_LEN: usize = 12;
const KEY_LEN: usize = 32;
const PBKDF2_ITERATIONS: u32 = 600_000;

fn derive_key_from_password(password: &str, salt: &[u8]) -> [u8; KEY_LEN] {
    let mut key = [0u8; KEY_LEN];
    pbkdf2_hmac::<Sha256>(password.as_bytes(), salt, PBKDF2_ITERATIONS, &mut key);
    key
}

pub fn encrypt_bytes(plain: &[u8], password: &str) -> Result<Vec<u8>, String> {
    let mut salt = [0u8; SALT_LEN];
    let mut nonce_bytes = [0u8; NONCE_LEN];

    getrandom(&mut salt).map_err(|e| format!("Failed to generate salt: {e}"))?;
    getrandom(&mut nonce_bytes).map_err(|e| format!("Failed to generate nonce: {e}"))?;

    let key_bytes = derive_key_from_password(password, &salt);
    let key = Key::<Aes256Gcm>::from_slice(&key_bytes);
    let cipher = Aes256Gcm::new(key);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, plain)
        .map_err(|_| "AES-GCM encryption failed".to_string())?;

    let mut out = Vec::with_capacity(SALT_LEN + NONCE_LEN + ciphertext.len());
    out.extend_from_slice(&salt);
    out.extend_from_slice(&nonce_bytes);
    out.extend_from_slice(&ciphertext);
    Ok(out)
}

pub fn decrypt_bytes(encrypted: &[u8], password: &str) -> Result<Vec<u8>, String> {
    if encrypted.len() < SALT_LEN + NONCE_LEN + 16 {
        return Err("Encrypted payload too short".to_string());
    }

    let salt = &encrypted[..SALT_LEN];
    let nonce_bytes = &encrypted[SALT_LEN..SALT_LEN + NONCE_LEN];
    let ciphertext = &encrypted[SALT_LEN + NONCE_LEN..];

    let key_bytes = derive_key_from_password(password, salt);
    let key = Key::<Aes256Gcm>::from_slice(&key_bytes);
    let cipher = Aes256Gcm::new(key);
    let nonce = Nonce::from_slice(nonce_bytes);

    cipher
        .decrypt(nonce, ciphertext)
        .map_err(|_| "Decrypt failed: wrong password or corrupted data".to_string())
}

#[cfg(test)]
mod tests {
    use super::{NONCE_LEN, SALT_LEN, decrypt_bytes, encrypt_bytes};

    #[test]
    fn encrypts_and_decrypts_round_trip() {
        let plain = b"top secret";
        let encrypted = encrypt_bytes(plain, "correct horse battery staple")
            .expect("encryption should succeed");
        let decrypted = decrypt_bytes(&encrypted, "correct horse battery staple")
            .expect("decryption should succeed");

        assert_eq!(decrypted, plain);
        assert!(encrypted.len() > plain.len());
    }

    #[test]
    fn rejects_wrong_password() {
        let encrypted = encrypt_bytes(b"top secret", "right").expect("encryption should succeed");
        let err = decrypt_bytes(&encrypted, "wrong").expect_err("password should be rejected");

        assert!(err.contains("wrong password"));
    }

    #[test]
    fn rejects_too_short_encrypted_payload() {
        let too_short = vec![0u8; SALT_LEN + NONCE_LEN + 15];
        let err = decrypt_bytes(&too_short, "password").expect_err("payload should be rejected");

        assert!(err.contains("too short"));
    }
}
