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
        let ciphertext = cipher
            .encrypt(nonce, plaintext.as_bytes())
            .expect("AES-256-GCM encryption should not fail");

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
