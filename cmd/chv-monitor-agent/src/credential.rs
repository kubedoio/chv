//! The on-disk credential: the only secret the agent holds. Written
//! with owner-only permissions immediately after enrollment or
//! rotation, before the credential is ever used.

use crate::wire::EnrollResponse;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, thiserror::Error)]
pub enum CredentialError {
    #[error("failed to read credential: {0}")]
    Io(#[from] std::io::Error),
    #[error("failed to parse credential: {0}")]
    Parse(#[from] serde_json::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredCredential {
    pub agent_id: String,
    pub vm_id: String,
    pub certificate_pem: String,
    pub private_key_pem: String,
    pub credential_epoch: u64,
    pub expires_at_ms: i64,
}

impl StoredCredential {
    /// The identity epoch string the ingestion contract requires on
    /// every sample (counter continuity across rotation).
    pub fn identity_epoch(&self) -> String {
        format!("agent-credential-generation-{}", self.credential_epoch)
    }

    pub fn from_enroll(resp: &EnrollResponse, private_key_pem: String) -> Self {
        Self {
            agent_id: resp.agent_id.clone(),
            vm_id: resp.vm_id.clone(),
            certificate_pem: resp.certificate_pem.clone(),
            private_key_pem,
            credential_epoch: resp.credential_epoch,
            expires_at_ms: resp.expires_at_ms,
        }
    }

    pub fn apply_rotation(
        &mut self,
        certificate_pem: String,
        credential_epoch: u64,
        expires_at_ms: i64,
    ) {
        self.certificate_pem = certificate_pem;
        self.credential_epoch = credential_epoch;
        self.expires_at_ms = expires_at_ms;
    }

    pub fn load(path: &Path) -> Result<Option<Self>, CredentialError> {
        match std::fs::read_to_string(path) {
            Ok(text) => Ok(Some(serde_json::from_str(&text)?)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Persist with owner-only permissions from the moment the file
    /// exists (created with mode 0600 — never a umask-default file
    /// that is later tightened; the private key must not be readable
    /// by anyone else even transiently).
    pub fn store(&self, path: &Path) -> Result<(), CredentialError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_vec_pretty(self)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            use std::os::unix::fs::PermissionsExt;
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .mode(0o600)
                .open(path)?;
            std::io::Write::write_all(&mut file, &json)?;
            // New files are created 0600 above; this tighten only
            // matters for a pre-existing file written by an older
            // agent (write-then-chmod) that this store overwrites.
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
        }
        #[cfg(not(unix))]
        {
            std::fs::write(path, &json)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> StoredCredential {
        StoredCredential {
            agent_id: "a1".into(),
            vm_id: "vm-1".into(),
            certificate_pem: "cert".into(),
            private_key_pem: "key".into(),
            credential_epoch: 1,
            expires_at_ms: 123,
        }
    }

    #[test]
    fn round_trip_and_identity_epoch() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cred.json");
        let c = sample();
        c.store(&path).unwrap();
        let loaded = StoredCredential::load(&path).unwrap().unwrap();
        assert_eq!(loaded.agent_id, "a1");
        assert_eq!(loaded.identity_epoch(), "agent-credential-generation-1");

        let mut c = loaded;
        c.apply_rotation("cert2".into(), 2, 456);
        assert_eq!(c.identity_epoch(), "agent-credential-generation-2");
        c.store(&path).unwrap();
        assert_eq!(
            StoredCredential::load(&path)
                .unwrap()
                .unwrap()
                .credential_epoch,
            2
        );
    }

    #[test]
    fn missing_file_is_none() {
        let dir = tempfile::tempdir().unwrap();
        assert!(StoredCredential::load(&dir.path().join("nope.json"))
            .unwrap()
            .is_none());
    }

    #[cfg(unix)]
    #[test]
    fn credential_file_is_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cred.json");
        sample().store(&path).unwrap();
        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600, "credential must be 0600");
    }
}
