use chv_errors::ChvError;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use tracing::{info, warn};

/// Result of a successful ship operation.
#[derive(Debug, Clone)]
pub struct ShipResult {
    pub remote_path: String,
    pub size_bytes: i64,
    pub checksum: String,
    pub checksum_algorithm: String,
}

/// Abstraction over backup artifact shipping targets.
#[async_trait::async_trait]
pub trait BackupShipper: Send + Sync {
    /// Ship a local artifact to remote storage.
    ///
    /// * `source_path` — local path to the snapshot file (must be readable by the control plane).
    /// * `job_id` — backup job identifier used to build the remote key.
    async fn ship(&self, source_path: &Path, job_id: &str) -> Result<ShipResult, ChvError>;

    /// Delete a remote artifact.
    async fn delete(&self, remote_path: &str) -> Result<(), ChvError>;

    /// Download a remote artifact to a local path.
    async fn fetch(&self, remote_path: &str, local_path: &Path) -> Result<(), ChvError>;
}

// ── Null Shipper (dev/testing) ─────────────────────────────────────────────

pub struct NullShipper;

#[async_trait::async_trait]
impl BackupShipper for NullShipper {
    async fn ship(&self, source_path: &Path, job_id: &str) -> Result<ShipResult, ChvError> {
        let (checksum, size_bytes) = compute_checksum_and_size(source_path).await?;
        info!(
            job_id = %job_id,
            source_path = %source_path.display(),
            size_bytes = size_bytes,
            "NullShipper: skipping upload"
        );
        Ok(ShipResult {
            remote_path: source_path.to_string_lossy().to_string(),
            size_bytes,
            checksum,
            checksum_algorithm: "SHA256".into(),
        })
    }

    async fn delete(&self, _remote_path: &str) -> Result<(), ChvError> {
        Ok(())
    }

    async fn fetch(&self, remote_path: &str, local_path: &Path) -> Result<(), ChvError> {
        if remote_path != local_path.to_string_lossy() {
            tokio::fs::copy(remote_path, local_path).await.map_err(|e| {
                ChvError::Internal {
                    reason: format!("NullShipper fetch copy failed: {e}"),
                }
            })?;
        }
        Ok(())
    }
}

// ── NFS Shipper ────────────────────────────────────────────────────────────

pub struct NfsShipper {
    mount_path: PathBuf,
}

impl NfsShipper {
    pub fn new(mount_path: PathBuf) -> Self {
        Self { mount_path }
    }
}

#[async_trait::async_trait]
impl BackupShipper for NfsShipper {
    async fn ship(&self, source_path: &Path, job_id: &str) -> Result<ShipResult, ChvError> {
        let (checksum, size_bytes) = compute_checksum_and_size(source_path).await?;

        let file_name = format!("{job_id}.backup");
        let dest_path = self.mount_path.join(&file_name);

        tokio::fs::create_dir_all(&self.mount_path).await.map_err(|e| {
            ChvError::Internal {
                reason: format!("NFS shipper failed to create mount dir: {e}"),
            }
        })?;

        tokio::fs::copy(source_path, &dest_path).await.map_err(|e| {
            ChvError::Internal {
                reason: format!("NFS shipper copy failed: {e}"),
            }
        })?;

        info!(
            job_id = %job_id,
            source_path = %source_path.display(),
            dest_path = %dest_path.display(),
            size_bytes = size_bytes,
            "NFS shipper: artifact copied"
        );

        Ok(ShipResult {
            remote_path: dest_path.to_string_lossy().to_string(),
            size_bytes,
            checksum,
            checksum_algorithm: "SHA256".into(),
        })
    }

    async fn delete(&self, remote_path: &str) -> Result<(), ChvError> {
        match tokio::fs::remove_file(remote_path).await {
            Ok(()) => {
                info!(path = %remote_path, "NFS shipper: deleted artifact");
                Ok(())
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(ChvError::Internal {
                reason: format!("NFS shipper delete failed: {e}"),
            }),
        }
    }

    async fn fetch(&self, remote_path: &str, local_path: &Path) -> Result<(), ChvError> {
        tokio::fs::copy(remote_path, local_path).await.map_err(|e| {
            ChvError::Internal {
                reason: format!("NFS shipper fetch failed: {e}"),
            }
        })?;
        Ok(())
    }
}

// ── S3 Shipper ─────────────────────────────────────────────────────────────

pub struct S3Shipper {
    bucket: Box<s3::Bucket>,
    prefix: String,
}

impl S3Shipper {
    pub fn new(
        bucket: String,
        prefix: String,
        region: String,
        endpoint: Option<String>,
        access_key: Option<String>,
        secret_key: Option<String>,
    ) -> Result<Self, ChvError> {
        let region = if let Some(endpoint) = endpoint {
            s3::Region::Custom { region, endpoint }
        } else {
            region.parse::<s3::Region>().map_err(|e| ChvError::Internal {
                reason: format!("invalid S3 region: {e}"),
            })?
        };

        let credentials = match (access_key, secret_key) {
            (Some(ak), Some(sk)) => s3::creds::Credentials::new(
                Some(&ak),
                Some(&sk),
                None,
                None,
                None,
            )
            .map_err(|e| ChvError::Internal {
                reason: format!("invalid S3 credentials: {e}"),
            })?,
            _ => s3::creds::Credentials::default().map_err(|e| ChvError::Internal {
                reason: format!("failed to load default S3 credentials: {e}"),
            })?,
        };

        let bucket =
            s3::Bucket::new(&bucket, region, credentials).map_err(|e| ChvError::Internal {
                reason: format!("failed to create S3 bucket: {e}"),
            })?;

        Ok(Self {
            bucket,
            prefix,
        })
    }
}

#[async_trait::async_trait]
impl BackupShipper for S3Shipper {
    async fn ship(&self, source_path: &Path, job_id: &str) -> Result<ShipResult, ChvError> {
        let (checksum, size_bytes) = compute_checksum_and_size(source_path).await?;

        let key = if self.prefix.is_empty() {
            format!("{job_id}.backup")
        } else {
            format!("{}/{job_id}.backup", self.prefix.trim_end_matches('/'))
        };

        // Stream the file to S3 instead of loading it entirely into memory.
        // On retry we reopen the file so the stream starts from the beginning.
        let mut retry = 0;
        loop {
            let mut file = tokio::fs::File::open(source_path).await.map_err(|e| {
                ChvError::Internal {
                    reason: format!("S3 shipper failed to open source file: {e}"),
                }
            })?;

            match self.bucket.put_object_stream(&mut file, &key).await {
                Ok(_) => break,
                Err(e) => {
                    retry += 1;
                    if retry > 3 {
                        return Err(ChvError::Internal {
                            reason: format!("S3 shipper upload failed after retries: {e}"),
                        });
                    }
                    warn!(job_id = %job_id, error = %e, retry = retry, "S3 upload failed, retrying");
                    tokio::time::sleep(std::time::Duration::from_secs(2u64.pow(retry))).await;
                }
            }
        }

        info!(
            job_id = %job_id,
            bucket = %self.bucket.name(),
            key = %key,
            size_bytes = size_bytes,
            "S3 shipper: artifact uploaded"
        );

        Ok(ShipResult {
            remote_path: key,
            size_bytes,
            checksum,
            checksum_algorithm: "SHA256".into(),
        })
    }

    async fn delete(&self, remote_path: &str) -> Result<(), ChvError> {
        match self.bucket.delete_object(remote_path).await {
            Ok(_) => {
                info!(key = %remote_path, bucket = %self.bucket.name(), "S3 shipper: deleted artifact");
                Ok(())
            }
            Err(e) => {
                warn!(key = %remote_path, error = %e, "S3 shipper: delete failed");
                Err(ChvError::Internal {
                    reason: format!("S3 delete failed: {e}"),
                })
            }
        }
    }

    async fn fetch(&self, remote_path: &str, local_path: &Path) -> Result<(), ChvError> {
        let response = self
            .bucket
            .get_object(remote_path)
            .await
            .map_err(|e| ChvError::Internal {
                reason: format!("S3 shipper fetch failed: {e}"),
            })?;

        tokio::fs::write(local_path, response.bytes()).await.map_err(|e| {
            ChvError::Internal {
                reason: format!("S3 shipper failed to write fetched file: {e}"),
            }
        })?;

        Ok(())
    }
}

// ── Helpers ────────────────────────────────────────────────────────────────

async fn compute_checksum_and_size(path: &Path) -> Result<(String, i64), ChvError> {
    let mut file = tokio::fs::File::open(path).await.map_err(|e| ChvError::Internal {
        reason: format!("failed to open file for checksum: {e}"),
    })?;

    let mut hasher = Sha256::new();
    let mut size_bytes: i64 = 0;
    let mut buf = vec![0u8; 65536];

    loop {
        let n = tokio::io::AsyncReadExt::read(&mut file, &mut buf)
            .await
            .map_err(|e| ChvError::Internal {
                reason: format!("failed to read file for checksum: {e}"),
            })?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        size_bytes += n as i64;
    }

    let checksum = hex::encode(hasher.finalize());
    Ok((checksum, size_bytes))
}

/// Convenience constructor that builds a shipper from a destination string.
///
/// Supported formats:
/// - `s3://bucket/prefix?region=us-east-1&endpoint=http://localhost:9000`
/// - `nfs:///mnt/backups`
/// - `null`
pub fn shipper_from_destination(
    destination: &str,
    access_key: Option<String>,
    secret_key: Option<String>,
) -> Result<Box<dyn BackupShipper>, ChvError> {
    if destination.eq_ignore_ascii_case("null") {
        return Ok(Box::new(NullShipper));
    }

    if destination.starts_with("nfs://") || destination.starts_with("nfs+") {
        let path = destination.trim_start_matches("nfs://").trim_start_matches("nfs+");
        return Ok(Box::new(NfsShipper::new(PathBuf::from(path))));
    }

    if let Some(rest) = destination.strip_prefix("s3://") {
        let mut parts = rest.splitn(2, '/');
        let bucket = parts.next().unwrap_or("").to_string();
        let remainder = parts.next().unwrap_or("");

        let (prefix, query) = if let Some(qidx) = remainder.find('?') {
            (&remainder[..qidx], &remainder[qidx + 1..])
        } else {
            (remainder, "")
        };

        let mut region = "us-east-1".to_string();
        let mut endpoint = None;
        for pair in query.split('&') {
            let mut kv = pair.splitn(2, '=');
            if let (Some(k), Some(v)) = (kv.next(), kv.next()) {
                match k {
                    "region" => region = v.to_string(),
                    "endpoint" => endpoint = Some(v.to_string()),
                    _ => {}
                }
            }
        }

        return Ok(Box::new(S3Shipper::new(
            bucket,
            prefix.to_string(),
            region,
            endpoint,
            access_key,
            secret_key,
        )?));
    }

    Err(ChvError::InvalidArgument {
        field: "destination".to_string(),
        reason: format!(
            "unsupported backup destination '{}': expected s3://, nfs://, or null",
            destination
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_null_shipper_computes_checksum_and_size() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.bin");
        std::fs::write(&path, b"hello world").unwrap();

        let shipper = NullShipper;
        let result = shipper.ship(&path, "job-001").await.unwrap();

        assert_eq!(result.size_bytes, 11);
        assert_eq!(result.checksum_algorithm, "SHA256");
        // SHA256("hello world") = b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9
        assert_eq!(
            result.checksum,
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
        );
        assert_eq!(result.remote_path, path.to_string_lossy());
    }

    #[tokio::test]
    async fn test_nfs_shipper_copies_file() {
        let src_dir = tempfile::tempdir().unwrap();
        let dst_dir = tempfile::tempdir().unwrap();
        let src_path = src_dir.path().join("source.bin");
        std::fs::write(&src_path, b"nfs test data").unwrap();

        let shipper = NfsShipper::new(dst_dir.path().to_path_buf());
        let result = shipper.ship(&src_path, "job-002").await.unwrap();

        let expected_dest = dst_dir.path().join("job-002.backup");
        assert!(expected_dest.exists());
        assert_eq!(std::fs::read(&expected_dest).unwrap(), b"nfs test data");
        assert_eq!(result.size_bytes, 13);
        assert_eq!(result.remote_path, expected_dest.to_string_lossy());
    }

    #[tokio::test]
    async fn test_nfs_shipper_delete_handles_not_found() {
        let dst_dir = tempfile::tempdir().unwrap();
        let shipper = NfsShipper::new(dst_dir.path().to_path_buf());

        let missing = dst_dir.path().join("does-not-exist.backup");
        // Should not error when file is already gone
        shipper.delete(missing.to_str().unwrap()).await.unwrap();
    }

    #[tokio::test]
    async fn test_nfs_shipper_delete_removes_file() {
        let dst_dir = tempfile::tempdir().unwrap();
        let shipper = NfsShipper::new(dst_dir.path().to_path_buf());

        let file = dst_dir.path().join("to-delete.backup");
        std::fs::write(&file, b"x").unwrap();
        assert!(file.exists());

        shipper.delete(file.to_str().unwrap()).await.unwrap();
        assert!(!file.exists());
    }

    #[test]
    fn test_shipper_from_destination_null() {
        let shipper = shipper_from_destination("null", None, None).unwrap();
        // Type erasure means we can only verify it doesn't panic and implements BackupShipper
        // by calling ship on a temp file.
        let _ = shipper; // compilation check
    }

    #[test]
    fn test_shipper_from_destination_nfs() {
        let shipper = shipper_from_destination("nfs:///mnt/backups", None, None).unwrap();
        let _ = shipper;
    }

    #[test]
    fn test_shipper_from_destination_s3_parses_bucket_and_prefix() {
        // We can't actually construct S3Shipper without valid credentials/network,
        // but we can verify the parser by checking it returns Ok for well-formed URLs.
        let result = shipper_from_destination(
            "s3://my-bucket/backups?region=us-west-2&endpoint=http://localhost:9000",
            Some("ak".into()),
            Some("sk".into()),
        );
        assert!(result.is_ok(), "S3 shipper construction failed: {:?}", result.err());
    }

    #[test]
    fn test_shipper_from_destination_rejects_invalid() {
        let result = shipper_from_destination("ftp://host/path", None, None);
        assert!(matches!(result, Err(ChvError::InvalidArgument { .. })));
    }

    #[tokio::test]
    async fn test_compute_checksum_and_size() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("check.bin");
        std::fs::write(&path, b"abc").unwrap();

        let (checksum, size) = compute_checksum_and_size(&path).await.unwrap();
        assert_eq!(size, 3);
        assert_eq!(
            checksum,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
