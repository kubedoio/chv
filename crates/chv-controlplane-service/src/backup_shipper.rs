use chv_errors::ChvError;
use tracing::{info, warn};

/// Abstraction over backup artifact shipping targets.
#[async_trait::async_trait]
pub trait BackupShipper: Send + Sync {
    /// Delete a remote artifact.
    async fn delete(&self, remote_path: &str) -> Result<(), ChvError>;
}

// ── Null Shipper (dev/testing) ─────────────────────────────────────────────

pub struct NullShipper;

#[async_trait::async_trait]
impl BackupShipper for NullShipper {
    async fn delete(&self, _remote_path: &str) -> Result<(), ChvError> {
        Ok(())
    }
}

// ── NFS Shipper ────────────────────────────────────────────────────────────

pub struct NfsShipper;

impl NfsShipper {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl BackupShipper for NfsShipper {
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
}

// ── S3 Shipper ─────────────────────────────────────────────────────────────

pub struct S3Shipper {
    bucket: Box<s3::Bucket>,
}

impl S3Shipper {
    pub fn new(
        bucket: String,
        region: String,
        endpoint: Option<String>,
        access_key: Option<String>,
        secret_key: Option<String>,
    ) -> Result<Self, ChvError> {
        let region = if let Some(endpoint) = endpoint {
            s3::Region::Custom { region, endpoint }
        } else {
            region
                .parse::<s3::Region>()
                .map_err(|e| ChvError::Internal {
                    reason: format!("invalid S3 region: {e}"),
                })?
        };

        let credentials = match (access_key, secret_key) {
            (Some(ak), Some(sk)) => {
                s3::creds::Credentials::new(Some(&ak), Some(&sk), None, None, None).map_err(
                    |e| ChvError::Internal {
                        reason: format!("invalid S3 credentials: {e}"),
                    },
                )?
            }
            _ => s3::creds::Credentials::default().map_err(|e| ChvError::Internal {
                reason: format!("failed to load default S3 credentials: {e}"),
            })?,
        };

        let bucket =
            s3::Bucket::new(&bucket, region, credentials).map_err(|e| ChvError::Internal {
                reason: format!("failed to create S3 bucket: {e}"),
            })?;

        Ok(Self { bucket })
    }
}

#[async_trait::async_trait]
impl BackupShipper for S3Shipper {
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
        return Ok(Box::new(NfsShipper::new()));
    }

    if let Some(rest) = destination.strip_prefix("s3://") {
        let mut parts = rest.splitn(2, '/');
        let bucket = parts.next().unwrap_or("").to_string();
        let remainder = parts.next().unwrap_or("");

        let (_prefix, query) = if let Some(qidx) = remainder.find('?') {
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
            bucket, region, endpoint, access_key, secret_key,
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
    async fn test_nfs_shipper_delete_handles_not_found() {
        let dst_dir = tempfile::tempdir().unwrap();
        let shipper = NfsShipper::new();

        let missing = dst_dir.path().join("does-not-exist.backup");
        // Should not error when file is already gone
        shipper.delete(missing.to_str().unwrap()).await.unwrap();
    }

    #[tokio::test]
    async fn test_nfs_shipper_delete_removes_file() {
        let dst_dir = tempfile::tempdir().unwrap();
        let shipper = NfsShipper::new();

        let file = dst_dir.path().join("to-delete.backup");
        std::fs::write(&file, b"x").unwrap();
        assert!(file.exists());

        shipper.delete(file.to_str().unwrap()).await.unwrap();
        assert!(!file.exists());
    }

    #[test]
    fn test_shipper_from_destination_null() {
        let shipper = shipper_from_destination("null", None, None).unwrap();
        // Type erasure means we can only verify it doesn't panic and implements BackupShipper.
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
        assert!(
            result.is_ok(),
            "S3 shipper construction failed: {:?}",
            result.err()
        );
    }

    #[test]
    fn test_shipper_from_destination_rejects_invalid() {
        let result = shipper_from_destination("ftp://host/path", None, None);
        assert!(matches!(result, Err(ChvError::InvalidArgument { .. })));
    }
}
