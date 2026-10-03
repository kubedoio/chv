//! Versioned full-volume digest for migration finalize verification.
//!
//! Historically the only integrity mechanism on the migration data path was
//! the per-chunk CRC32, which says nothing about the *assembled*
//! destination volume: a corrupted, truncated, or externally modified
//! destination still finalized successfully (issue #392). At finalize the
//! sender now computes a SHA-256 digest over the whole source volume and
//! carries it in `FinalizeComplete.volume_checksum`; the receiver
//! re-computes the same digest over the destination volume and answers
//! `FinalizeAck{verified}` accordingly.
//!
//! # Wire format
//!
//! The digest is self-describing and versioned so a future algorithm
//! change is *detected*, never silently misinterpreted as a different
//! algorithm's digest:
//!
//! ```text
//! "sha256:" (7 ASCII bytes) || 32 raw big-endian digest bytes
//! ```
//!
//! A receiver that does not recognize the leading algorithm prefix fails
//! closed (`verified = false`, error naming the unsupported format) rather
//! than comparing apples to oranges.

use chv_stord_backends::{StorageBackend, DIRTY_TRACKING_BLOCK_SIZE};
use sha2::{Digest, Sha256};

/// Algorithm prefix of the current digest format (`"sha256:"`).
pub const SHA256_PREFIX: &[u8] = b"sha256:";

/// Length of a raw SHA-256 digest.
pub const SHA256_DIGEST_LEN: usize = 32;

/// A parsed (or freshly computed) full-volume digest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VolumeDigest {
    /// SHA-256 over the volume's bytes, in offset order.
    Sha256([u8; SHA256_DIGEST_LEN]),
}

impl VolumeDigest {
    /// Name of the digest algorithm (for content-free error messages).
    pub fn algorithm(&self) -> &'static str {
        match self {
            VolumeDigest::Sha256(_) => "sha256",
        }
    }

    /// Lowercase hex rendering of the raw digest (for logs and error
    /// messages; a hash is content-free by construction).
    pub fn hex(&self) -> String {
        match self {
            VolumeDigest::Sha256(digest) => digest.iter().map(|b| format!("{b:02x}")).collect(),
        }
    }

    /// `"<algo>:<hex>"` rendering used in mismatch error messages.
    pub fn display(&self) -> String {
        format!("{}:{}", self.algorithm(), self.hex())
    }

    /// Encode into the versioned wire format carried by
    /// `FinalizeComplete.volume_checksum`.
    pub fn to_wire(&self) -> Vec<u8> {
        match self {
            VolumeDigest::Sha256(digest) => {
                let mut wire = Vec::with_capacity(SHA256_PREFIX.len() + SHA256_DIGEST_LEN);
                wire.extend_from_slice(SHA256_PREFIX);
                wire.extend_from_slice(digest);
                wire
            }
        }
    }
}

/// Encode a raw SHA-256 digest into the versioned wire format.
pub fn encode_sha256_digest(digest: &[u8; SHA256_DIGEST_LEN]) -> Vec<u8> {
    VolumeDigest::Sha256(*digest).to_wire()
}

/// Parse a `FinalizeComplete.volume_checksum` payload.
///
/// Fails closed on anything that is not the exact expected format for a
/// *known* algorithm: empty payloads, unknown algorithm prefixes, or a
/// known prefix with the wrong payload length. The error message names
/// what was observed (prefix snippet and length only — never volume data)
/// so an unsupported future format is diagnosable at the sender.
pub fn parse_volume_digest(bytes: &[u8]) -> Result<VolumeDigest, String> {
    if bytes.is_empty() {
        return Err("sender sent no volume digest (empty volume_checksum)".to_string());
    }
    if let Some(raw) = bytes.strip_prefix(SHA256_PREFIX) {
        if raw.len() == SHA256_DIGEST_LEN {
            let mut digest = [0u8; SHA256_DIGEST_LEN];
            digest.copy_from_slice(raw);
            return Ok(VolumeDigest::Sha256(digest));
        }
        return Err(format!(
            "malformed sha256 volume digest: expected {} raw bytes after the prefix, got {}",
            SHA256_DIGEST_LEN,
            raw.len()
        ));
    }
    Err(format!(
        "unsupported volume digest format: {} bytes starting with '{}' (expected \"sha256:\" + {SHA256_DIGEST_LEN} raw bytes)",
        bytes.len(),
        describe_prefix(bytes),
    ))
}

/// Render the leading bytes of an unrecognized digest payload for an error
/// message: the part before the first `:` (capped at 16 bytes), falling
/// back to a lossy rendering for non-ASCII prefixes. Content-free by
/// construction (format metadata only).
fn describe_prefix(bytes: &[u8]) -> String {
    let end = bytes
        .iter()
        .position(|&b| b == b':')
        .unwrap_or(bytes.len())
        .min(16);
    String::from_utf8_lossy(&bytes[..end]).into_owned()
}

/// Compute the full-volume SHA-256 digest, streaming the volume through
/// the backend's `read_block` path (the same path used for bulk copy).
///
/// The volume is read in `DIRTY_TRACKING_BLOCK_SIZE` chunks and fed to the
/// hasher incrementally — the whole volume is never held in memory. A
/// short read from the backend is an error rather than a silently
/// mis-aligned digest.
pub async fn compute_volume_digest<B: StorageBackend>(
    backend: &B,
    volume_id: &str,
    handle: &str,
    size_bytes: u64,
) -> Result<VolumeDigest, String> {
    let mut hasher = Sha256::new();
    let mut offset: u64 = 0;
    while offset < size_bytes {
        let length = DIRTY_TRACKING_BLOCK_SIZE.min(size_bytes - offset);
        let data = backend
            .read_block(volume_id, handle, offset, length)
            .await
            .map_err(|e| format!("read_block failed at offset {offset}: {e}"))?;
        if data.len() != length as usize {
            return Err(format!(
                "short read at offset {offset}: expected {length} bytes, got {}",
                data.len()
            ));
        }
        hasher.update(&data);
        offset += length;
    }
    Ok(VolumeDigest::Sha256(hasher.finalize().into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_parse_round_trip() {
        let digest = [7u8; SHA256_DIGEST_LEN];
        let wire = encode_sha256_digest(&digest);
        assert_eq!(wire.len(), SHA256_PREFIX.len() + SHA256_DIGEST_LEN);
        assert_eq!(&wire[..SHA256_PREFIX.len()], SHA256_PREFIX);
        assert_eq!(parse_volume_digest(&wire), Ok(VolumeDigest::Sha256(digest)));
    }

    #[test]
    fn to_wire_matches_encode_sha256_digest() {
        let digest = [0xAB; SHA256_DIGEST_LEN];
        assert_eq!(
            VolumeDigest::Sha256(digest).to_wire(),
            encode_sha256_digest(&digest)
        );
    }

    #[test]
    fn parse_rejects_empty_payload() {
        let err = parse_volume_digest(&[]).unwrap_err();
        assert!(
            err.contains("no volume digest"),
            "error should name the empty payload: {err}"
        );
    }

    #[test]
    fn parse_rejects_unknown_algorithm_prefix() {
        // A future sender using, say, blake3 must be detected as an
        // unsupported format, not misinterpreted.
        let mut wire = b"blake3:".to_vec();
        wire.extend_from_slice(&[0u8; 32]);
        let err = parse_volume_digest(&wire).unwrap_err();
        assert!(
            err.contains("unsupported volume digest format"),
            "error should name the unsupported format: {err}"
        );
        assert!(
            err.contains("blake3"),
            "error should name the observed prefix: {err}"
        );
        assert!(
            err.contains("sha256"),
            "error should name the expected format: {err}"
        );
    }

    #[test]
    fn parse_rejects_bare_digest_without_prefix() {
        // 32 raw bytes with no algorithm prefix: unversioned, rejected.
        let err = parse_volume_digest(&[1u8; SHA256_DIGEST_LEN]).unwrap_err();
        assert!(err.contains("unsupported volume digest format"));
    }

    #[test]
    fn parse_rejects_wrong_payload_length() {
        let mut wire = SHA256_PREFIX.to_vec();
        wire.extend_from_slice(&[2u8; SHA256_DIGEST_LEN - 1]);
        let err = parse_volume_digest(&wire).unwrap_err();
        assert!(
            err.contains("malformed sha256 volume digest"),
            "error should name the malformed length: {err}"
        );
        assert!(
            err.contains("31"),
            "error should state the observed length: {err}"
        );
    }

    #[test]
    fn parse_rejects_trailing_garbage() {
        let mut wire = encode_sha256_digest(&[3u8; SHA256_DIGEST_LEN]);
        wire.push(0);
        assert!(parse_volume_digest(&wire).is_err());
    }

    #[test]
    fn display_is_content_free_and_prefixed() {
        let digest = VolumeDigest::Sha256([0x0A; SHA256_DIGEST_LEN]);
        let displayed = digest.display();
        assert!(displayed.starts_with("sha256:"));
        assert_eq!(displayed.len(), "sha256:".len() + 2 * SHA256_DIGEST_LEN);
        assert!(displayed.ends_with(&"0a".repeat(SHA256_DIGEST_LEN)));
    }

    #[test]
    fn describe_prefix_caps_and_stops_at_colon() {
        assert_eq!(describe_prefix(b"sha999:x"), "sha999");
        let long: Vec<u8> = std::iter::repeat_n(b'a', 40).collect();
        assert_eq!(describe_prefix(&long).len(), 16);
    }

    /// End-to-end over a real `LocalFileBackend`: the digest of a volume
    /// with known contents equals an independent SHA-256 of the file, and
    /// mutating one byte changes it (the property the finalize exchange
    /// relies on).
    #[tokio::test]
    async fn compute_volume_digest_matches_file_hash() {
        use chv_common::types::{BackendLocator, DevicePolicy};
        use chv_stord_backends::LocalFileBackend;

        let dir = tempfile::tempdir().unwrap();
        let image: Vec<u8> = (0..100_000u32).map(|i| (i % 251) as u8).collect();
        std::fs::write(dir.path().join("vol.img"), &image).unwrap();

        let backend = LocalFileBackend::new(dir.path().to_path_buf());
        let export = backend
            .open(
                "vol-digest",
                &BackendLocator {
                    backend_class: "local".to_string(),
                    locator: "vol.img".to_string(),
                    options: [("size_bytes".to_string(), image.len().to_string())]
                        .into_iter()
                        .collect(),
                },
                &DevicePolicy::default(),
            )
            .await
            .unwrap();

        let digest = compute_volume_digest(
            &backend,
            "vol-digest",
            &export.attachment_handle,
            image.len() as u64,
        )
        .await
        .unwrap();
        let mut expected = Sha256::new();
        expected.update(&image);
        assert_eq!(digest, VolumeDigest::Sha256(expected.finalize().into()));

        // One flipped byte must produce a different digest.
        let mut corrupted = image.clone();
        corrupted[123] ^= 0xFF;
        std::fs::write(dir.path().join("vol.img"), &corrupted).unwrap();
        let digest2 = compute_volume_digest(
            &backend,
            "vol-digest",
            &export.attachment_handle,
            image.len() as u64,
        )
        .await
        .unwrap();
        assert_ne!(digest, digest2);
    }

    #[tokio::test]
    async fn compute_volume_digest_empty_volume_is_well_defined() {
        use chv_common::types::{BackendLocator, DevicePolicy};
        use chv_stord_backends::LocalFileBackend;

        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("vol.img"), b"").unwrap();

        let backend = LocalFileBackend::new(dir.path().to_path_buf());
        let export = backend
            .open(
                "vol-empty",
                &BackendLocator {
                    backend_class: "local".to_string(),
                    locator: "vol.img".to_string(),
                    options: [("size_bytes".to_string(), "0".to_string())]
                        .into_iter()
                        .collect(),
                },
                &DevicePolicy::default(),
            )
            .await
            .unwrap();

        let digest = compute_volume_digest(&backend, "vol-empty", &export.attachment_handle, 0)
            .await
            .unwrap();
        // SHA-256 of the empty input.
        assert_eq!(digest, VolumeDigest::Sha256(Sha256::digest([]).into()));
    }
}
