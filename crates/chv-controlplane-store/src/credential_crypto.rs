use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use sha2::{Digest, Sha256};
use tracing::warn;

#[derive(Clone)]
pub struct CredentialEncryption {
    cipher: Option<Aes256Gcm>,
}

impl CredentialEncryption {
    pub fn new() -> Self {
        let key_str = std::env::var("CHV_ENCRYPTION_KEY")
            .or_else(|_| std::env::var("CHV_JWT_SECRET"))
            .unwrap_or_else(|_| {
                warn!(
                    "Neither CHV_ENCRYPTION_KEY nor CHV_JWT_SECRET is set; \
                     S3 credentials will be stored in plaintext"
                );
                String::new()
            });

        if key_str.is_empty() {
            return Self { cipher: None };
        }

        let mut hasher = Sha256::new();
        hasher.update(key_str.as_bytes());
        let key_bytes = hasher.finalize();

        let cipher = Aes256Gcm::new_from_slice(&key_bytes)
            .expect("SHA-256 produces a valid 32-byte key for AES-256-GCM");
        Self {
            cipher: Some(cipher),
        }
    }

    pub fn encrypt(&self, plaintext: &str) -> String {
        let Some(cipher) = &self.cipher else {
            return plaintext.to_string();
        };

        let nonce_bytes: [u8; 12] = rand::random();
        let nonce = Nonce::from_slice(&nonce_bytes);
        let ciphertext = match cipher.encrypt(nonce, plaintext.as_bytes()) {
            Ok(ct) => ct,
            Err(e) => {
                tracing::warn!(error = %e, "AES-256-GCM encryption failed; storing plaintext");
                return plaintext.to_string();
            }
        };

        let mut combined = Vec::with_capacity(nonce_bytes.len() + ciphertext.len());
        combined.extend_from_slice(&nonce_bytes);
        combined.extend_from_slice(&ciphertext);

        format!("enc:{}", hex::encode(combined))
    }

    pub fn decrypt(&self, ciphertext: &str) -> String {
        let Some(cipher) = &self.cipher else {
            return ciphertext.to_string();
        };

        let Some(payload) = ciphertext.strip_prefix("enc:") else {
            return ciphertext.to_string();
        };

        let combined = match hex::decode(payload) {
            Ok(v) => v,
            Err(_) => return ciphertext.to_string(),
        };

        if combined.len() < 12 {
            return ciphertext.to_string();
        }

        let (nonce_bytes, encrypted) = combined.split_at(12);
        let nonce = Nonce::from_slice(nonce_bytes);
        let plain_bytes = match cipher.decrypt(nonce, encrypted) {
            Ok(v) => v,
            Err(_) => return ciphertext.to_string(),
        };

        String::from_utf8_lossy(&plain_bytes).into_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// Serializes all tests in this module so they don't race on the
    /// process-global `CHV_ENCRYPTION_KEY` and `CHV_JWT_SECRET` env vars.
    /// Tests that mutate these env vars MUST acquire this lock.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    /// RAII guard that snapshots and restores both env vars across a test.
    struct EnvGuard {
        _lock: std::sync::MutexGuard<'static, ()>,
        prev_enc: Option<String>,
        prev_jwt: Option<String>,
    }

    impl EnvGuard {
        fn lock() -> Self {
            // poisoned mutex is fine — we only use it for serialization
            let lock = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
            let prev_enc = std::env::var("CHV_ENCRYPTION_KEY").ok();
            let prev_jwt = std::env::var("CHV_JWT_SECRET").ok();
            Self {
                _lock: lock,
                prev_enc,
                prev_jwt,
            }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            match &self.prev_enc {
                Some(v) => std::env::set_var("CHV_ENCRYPTION_KEY", v),
                None => std::env::remove_var("CHV_ENCRYPTION_KEY"),
            }
            match &self.prev_jwt {
                Some(v) => std::env::set_var("CHV_JWT_SECRET", v),
                None => std::env::remove_var("CHV_JWT_SECRET"),
            }
        }
    }

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let _g = EnvGuard::lock();
        // Set a test encryption key
        std::env::set_var("CHV_ENCRYPTION_KEY", "test-key-for-unit-tests-12345");

        let crypto = CredentialEncryption::new();
        let plaintext = "my-super-secret-key";

        let encrypted = crypto.encrypt(plaintext);
        assert!(encrypted.starts_with("enc:"));
        assert_ne!(encrypted, plaintext);

        let decrypted = crypto.decrypt(&encrypted);
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_decrypt_plaintext_backward_compatible() {
        let _g = EnvGuard::lock();
        std::env::set_var("CHV_ENCRYPTION_KEY", "test-key-for-unit-tests-12345");

        let crypto = CredentialEncryption::new();
        let plaintext = "plain-old-value";

        // Decrypting a non-prefixed string returns it as-is
        let decrypted = crypto.decrypt(plaintext);
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_no_key_stores_plaintext() {
        let _g = EnvGuard::lock();
        // Remove any existing key
        std::env::remove_var("CHV_ENCRYPTION_KEY");
        std::env::remove_var("CHV_JWT_SECRET");

        let crypto = CredentialEncryption::new();
        let plaintext = "no-encryption-active";

        let encrypted = crypto.encrypt(plaintext);
        assert_eq!(encrypted, plaintext);

        let decrypted = crypto.decrypt(&encrypted);
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn tampered_ciphertext_returns_original_string() {
        let _g = EnvGuard::lock();
        std::env::set_var("CHV_ENCRYPTION_KEY", "tamper-test-key");

        let crypto = CredentialEncryption::new();
        let plaintext = "sensitive-credential";
        let encrypted = crypto.encrypt(plaintext);
        assert!(encrypted.starts_with("enc:"));

        // Flip one byte in the hex-encoded ciphertext (after the "enc:" prefix).
        // We mutate a hex char near the tail to land inside the AES-GCM auth tag /
        // ciphertext region rather than the nonce, but either way the auth tag
        // must reject this.
        let mut bytes: Vec<u8> = encrypted.as_bytes().to_vec();
        let tail = bytes.len() - 1;
        bytes[tail] = if bytes[tail] == b'a' { b'b' } else { b'a' };
        let tampered = String::from_utf8(bytes).expect("ASCII hex");

        let decrypted = crypto.decrypt(&tampered);
        // Fail-soft contract: tampered ciphertext is returned verbatim, NOT
        // the original plaintext (proves AES-GCM auth tag detected tampering).
        assert_eq!(decrypted, tampered);
        assert_ne!(decrypted, plaintext);
    }

    #[test]
    fn wrong_key_returns_original_string() {
        let _g = EnvGuard::lock();

        // Encrypt with key A.
        std::env::set_var("CHV_ENCRYPTION_KEY", "key-A-for-encryption");
        let crypto_a = CredentialEncryption::new();
        let plaintext = "secret-under-key-A";
        let encrypted = crypto_a.encrypt(plaintext);
        assert!(encrypted.starts_with("enc:"));

        // Decrypt with key B — auth check must fail and decrypt() returns input.
        std::env::set_var("CHV_ENCRYPTION_KEY", "key-B-totally-different");
        let crypto_b = CredentialEncryption::new();
        let decrypted = crypto_b.decrypt(&encrypted);

        assert_eq!(decrypted, encrypted);
        assert_ne!(decrypted, plaintext);
    }

    #[test]
    fn malformed_hex_after_enc_prefix_returns_input() {
        let _g = EnvGuard::lock();
        std::env::set_var("CHV_ENCRYPTION_KEY", "any-key");

        let crypto = CredentialEncryption::new();
        let input = "enc:not-valid-hex-zzz";

        let decrypted = crypto.decrypt(input);
        assert_eq!(decrypted, input);
    }

    #[test]
    fn empty_ciphertext_after_prefix_returns_input() {
        let _g = EnvGuard::lock();
        std::env::set_var("CHV_ENCRYPTION_KEY", "any-key");

        let crypto = CredentialEncryption::new();
        let input = "enc:";

        let decrypted = crypto.decrypt(input);
        assert_eq!(decrypted, input);
    }

    #[test]
    fn short_ciphertext_below_nonce_size_returns_input() {
        let _g = EnvGuard::lock();
        std::env::set_var("CHV_ENCRYPTION_KEY", "any-key");

        let crypto = CredentialEncryption::new();
        // 2 bytes after hex decode — well below the 12-byte nonce floor.
        let input = "enc:0011";

        let decrypted = crypto.decrypt(input);
        assert_eq!(decrypted, input);
    }

    #[test]
    fn encrypt_then_decrypt_roundtrip_with_unicode() {
        let _g = EnvGuard::lock();
        std::env::set_var("CHV_ENCRYPTION_KEY", "unicode-roundtrip-key");

        let crypto = CredentialEncryption::new();
        let plaintext = "hello 🌍 — naïve café 日本語 🚀";

        let encrypted = crypto.encrypt(plaintext);
        assert!(encrypted.starts_with("enc:"));

        let decrypted = crypto.decrypt(&encrypted);
        assert_eq!(decrypted, plaintext);
    }
}
