use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use sha2::{Digest, Sha256};
use thiserror::Error;
use tracing::warn;

/// Errors returned by [`CredentialEncryption::decrypt`].
///
/// Decrypt fails closed: callers MUST handle these errors and never substitute
/// the ciphertext back into a credential field. Returning the literal
/// `enc:hex...` to a downstream consumer (e.g. the S3 client) makes auth
/// failures look like phantom AWS errors and silently breaks backups.
#[derive(Debug, Error)]
pub enum DecryptError {
    /// The input had the `enc:` prefix but the payload could not be hex-decoded
    /// or was shorter than the AES-GCM nonce. The stored value is corrupt.
    #[error("malformed encrypted credential")]
    Malformed,
    /// AES-GCM authentication failed. The configured key does not match the
    /// key used to encrypt this value, or the ciphertext was tampered with.
    #[error("credential authentication failed (wrong key or tampered ciphertext)")]
    AuthFailed,
    /// The decrypted bytes were not valid UTF-8. Authentic per AES-GCM, but
    /// not a recoverable string credential.
    #[error("decrypted credential is not valid UTF-8")]
    InvalidUtf8,
    /// No encryption key is configured. Callers that have an `enc:`-prefixed
    /// value in the database hit this when the operator forgot to set
    /// `CHV_ENCRYPTION_KEY` (or rotated it away) — the value cannot be
    /// recovered and the credential field MUST be treated as missing.
    #[error("no encryption key configured; cannot decrypt encrypted credential")]
    KeyUnavailable,
}

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

        // SHA-256 always produces 32 bytes, which is the exact key size for
        // AES-256-GCM, so this constructor cannot fail in practice. We still
        // avoid `.expect()` in production code (ADR-008): on the impossible
        // failure path we fall back to a no-cipher state and log a clear
        // operator-visible warning instead of crashing the process.
        match Aes256Gcm::new_from_slice(&key_bytes) {
            Ok(cipher) => Self {
                cipher: Some(cipher),
            },
            Err(e) => {
                warn!(
                    error = %e,
                    "failed to construct AES-256-GCM cipher from key; \
                     credentials will be stored in plaintext"
                );
                Self { cipher: None }
            }
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

    /// Decrypts an `enc:`-prefixed credential.
    ///
    /// Fail-closed contract: any failure mode (wrong key, tampered ciphertext,
    /// malformed payload, missing key) returns a [`DecryptError`]. Callers
    /// MUST NOT fall back to the input string — doing so leaks ciphertext
    /// into downstream consumers as if it were a credential, which silently
    /// breaks S3 backups with opaque auth errors.
    ///
    /// Plaintext values without the `enc:` prefix are returned as-is to
    /// preserve backward compatibility with rows written before encryption
    /// was enabled.
    pub fn decrypt(&self, ciphertext: &str) -> Result<String, DecryptError> {
        // Plaintext (no `enc:` prefix) is a backward-compatibility case: the
        // row was written before encryption was enabled. Pass through unchanged.
        let Some(payload) = ciphertext.strip_prefix("enc:") else {
            metrics::counter!("chv_credential_decrypt_total", "outcome" => "ok_plaintext")
                .increment(1);
            return Ok(ciphertext.to_string());
        };

        // From here on, the value is supposed to be encrypted. Any failure is
        // a hard error — we never return the literal `enc:hex...` to a caller.

        let Some(cipher) = &self.cipher else {
            metrics::counter!("chv_credential_decrypt_total", "outcome" => "err_key_unavailable")
                .increment(1);
            return Err(DecryptError::KeyUnavailable);
        };

        let combined = match hex::decode(payload) {
            Ok(v) => v,
            Err(_) => {
                metrics::counter!("chv_credential_decrypt_total", "outcome" => "err_malformed")
                    .increment(1);
                return Err(DecryptError::Malformed);
            }
        };

        if combined.len() < 12 {
            metrics::counter!("chv_credential_decrypt_total", "outcome" => "err_malformed")
                .increment(1);
            return Err(DecryptError::Malformed);
        }

        let (nonce_bytes, encrypted) = combined.split_at(12);
        let nonce = Nonce::from_slice(nonce_bytes);
        let plain_bytes = match cipher.decrypt(nonce, encrypted) {
            Ok(v) => v,
            Err(_) => {
                metrics::counter!("chv_credential_decrypt_total", "outcome" => "err_auth")
                    .increment(1);
                return Err(DecryptError::AuthFailed);
            }
        };

        match String::from_utf8(plain_bytes) {
            Ok(s) => {
                metrics::counter!("chv_credential_decrypt_total", "outcome" => "ok").increment(1);
                Ok(s)
            }
            Err(_) => {
                metrics::counter!("chv_credential_decrypt_total", "outcome" => "err_invalid_utf8")
                    .increment(1);
                Err(DecryptError::InvalidUtf8)
            }
        }
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
        std::env::set_var("CHV_ENCRYPTION_KEY", "test-key-for-unit-tests-12345");

        let crypto = CredentialEncryption::new();
        let plaintext = "my-super-secret-key";

        let encrypted = crypto.encrypt(plaintext);
        assert!(encrypted.starts_with("enc:"));
        assert_ne!(encrypted, plaintext);

        let decrypted = crypto.decrypt(&encrypted).expect("roundtrip succeeds");
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_decrypt_plaintext_backward_compatible() {
        let _g = EnvGuard::lock();
        std::env::set_var("CHV_ENCRYPTION_KEY", "test-key-for-unit-tests-12345");

        let crypto = CredentialEncryption::new();
        let plaintext = "plain-old-value";

        // Decrypting a non-prefixed string returns it as-is for back-compat
        // with rows written before encryption was enabled.
        let decrypted = crypto
            .decrypt(plaintext)
            .expect("plaintext passthrough succeeds");
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_no_key_stores_plaintext() {
        let _g = EnvGuard::lock();
        std::env::remove_var("CHV_ENCRYPTION_KEY");
        std::env::remove_var("CHV_JWT_SECRET");

        let crypto = CredentialEncryption::new();
        let plaintext = "no-encryption-active";

        let encrypted = crypto.encrypt(plaintext);
        assert_eq!(encrypted, plaintext);

        let decrypted = crypto
            .decrypt(&encrypted)
            .expect("no-key plaintext passthrough succeeds");
        assert_eq!(decrypted, plaintext);
    }

    /// Fail-closed: tampered ciphertext MUST surface AuthFailed, not be
    /// returned verbatim. Returning the literal `enc:hex...` would let the
    /// caller pass the ciphertext into the S3 client as if it were a
    /// credential.
    #[test]
    fn tampered_ciphertext_returns_auth_failed() {
        let _g = EnvGuard::lock();
        std::env::set_var("CHV_ENCRYPTION_KEY", "tamper-test-key");

        let crypto = CredentialEncryption::new();
        let plaintext = "sensitive-credential";
        let encrypted = crypto.encrypt(plaintext);
        assert!(encrypted.starts_with("enc:"));

        // Flip one byte near the tail (auth tag region).
        let mut bytes: Vec<u8> = encrypted.as_bytes().to_vec();
        let tail = bytes.len() - 1;
        bytes[tail] = if bytes[tail] == b'a' { b'b' } else { b'a' };
        let tampered = String::from_utf8(bytes).expect("ASCII hex");

        let result = crypto.decrypt(&tampered);
        assert!(
            matches!(result, Err(DecryptError::AuthFailed)),
            "expected AuthFailed for tampered ciphertext, got {:?}",
            result
        );
    }

    /// C4 regression test: ciphertext encrypted with key A MUST surface
    /// AuthFailed when decrypted with key B. The previous fail-soft contract
    /// returned the ciphertext literal here, which then ended up written
    /// into `s3_access_key`/`s3_secret_key` and passed to the S3 client as
    /// a credential — silently breaking backups with opaque auth errors.
    #[test]
    fn wrong_key_returns_auth_failed() {
        let _g = EnvGuard::lock();

        // Encrypt with key A.
        std::env::set_var("CHV_ENCRYPTION_KEY", "key-A-for-encryption");
        let crypto_a = CredentialEncryption::new();
        let plaintext = "secret-under-key-A";
        let encrypted = crypto_a.encrypt(plaintext);
        assert!(encrypted.starts_with("enc:"));

        // Decrypt with key B.
        std::env::set_var("CHV_ENCRYPTION_KEY", "key-B-totally-different");
        let crypto_b = CredentialEncryption::new();
        let result = crypto_b.decrypt(&encrypted);

        assert!(
            matches!(result, Err(DecryptError::AuthFailed)),
            "expected AuthFailed under wrong key, got {:?}",
            result
        );
    }

    #[test]
    fn malformed_hex_after_enc_prefix_returns_malformed() {
        let _g = EnvGuard::lock();
        std::env::set_var("CHV_ENCRYPTION_KEY", "any-key");

        let crypto = CredentialEncryption::new();
        let result = crypto.decrypt("enc:not-valid-hex-zzz");

        assert!(
            matches!(result, Err(DecryptError::Malformed)),
            "expected Malformed for non-hex payload, got {:?}",
            result
        );
    }

    #[test]
    fn empty_ciphertext_after_prefix_returns_malformed() {
        let _g = EnvGuard::lock();
        std::env::set_var("CHV_ENCRYPTION_KEY", "any-key");

        let crypto = CredentialEncryption::new();
        // hex-decode of "" is Ok([]); length 0 is below the 12-byte nonce floor.
        let result = crypto.decrypt("enc:");

        assert!(
            matches!(result, Err(DecryptError::Malformed)),
            "expected Malformed for empty payload, got {:?}",
            result
        );
    }

    #[test]
    fn short_ciphertext_below_nonce_size_returns_malformed() {
        let _g = EnvGuard::lock();
        std::env::set_var("CHV_ENCRYPTION_KEY", "any-key");

        let crypto = CredentialEncryption::new();
        let result = crypto.decrypt("enc:0011");

        assert!(
            matches!(result, Err(DecryptError::Malformed)),
            "expected Malformed for under-nonce payload, got {:?}",
            result
        );
    }

    /// An `enc:`-prefixed value with no configured key cannot be recovered.
    /// We must return KeyUnavailable, not the ciphertext literal.
    #[test]
    fn enc_prefixed_with_no_key_returns_key_unavailable() {
        let _g = EnvGuard::lock();

        // Encrypt under a key…
        std::env::set_var("CHV_ENCRYPTION_KEY", "key-that-will-be-removed");
        let crypto_with_key = CredentialEncryption::new();
        let encrypted = crypto_with_key.encrypt("plaintext-under-key");
        assert!(encrypted.starts_with("enc:"));

        // …then drop the key (operator misconfig / rotation gap).
        std::env::remove_var("CHV_ENCRYPTION_KEY");
        std::env::remove_var("CHV_JWT_SECRET");
        let crypto_no_key = CredentialEncryption::new();
        let result = crypto_no_key.decrypt(&encrypted);

        assert!(
            matches!(result, Err(DecryptError::KeyUnavailable)),
            "expected KeyUnavailable when key is missing, got {:?}",
            result
        );
    }

    #[test]
    fn encrypt_then_decrypt_roundtrip_with_unicode() {
        let _g = EnvGuard::lock();
        std::env::set_var("CHV_ENCRYPTION_KEY", "unicode-roundtrip-key");

        let crypto = CredentialEncryption::new();
        let plaintext = "hello 🌍 — naïve café 日本語 🚀";

        let encrypted = crypto.encrypt(plaintext);
        assert!(encrypted.starts_with("enc:"));

        let decrypted = crypto.decrypt(&encrypted).expect("unicode roundtrip");
        assert_eq!(decrypted, plaintext);
    }
}
