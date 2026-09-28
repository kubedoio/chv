//! Startup load + validation of the storage-migration mTLS identity.
//!
//! This is the daemon-wiring half of issue #232: production configuration must
//! be able to supply the existing [`MigrationTlsConfig`] capability (previously
//! hard-coded as `None` in `cmd/chv-stord`).
//!
//! # Fail-closed semantics
//!
//! - `enabled = false` → `Ok(None)`: the daemon starts without migration
//!   credentials, and migration actions fail as *unavailable* — the sender
//!   already refuses to run without `MigrationTlsConfig` (no plaintext
//!   fallback, see `migration/sender.rs`).
//! - `enabled = true` → all four identity inputs are required, files must be
//!   readable, the certificate/key pair must match, and the CA bundle must
//!   parse. Any problem is a **startup error** (fail-closed).
//!
//! # Security rules honored here
//!
//! - No plaintext fallback and no "skip verify" option.
//! - No PEM/private-key material in logs or errors — only file paths and
//!   generic failure reasons.

use crate::migration::sender::MigrationTlsConfig;
use std::fmt;
use std::path::Path;

/// Errors produced while loading/validating the migration TLS identity.
///
/// Deliberately does not carry PEM/private-key material (see module docs).
#[derive(Debug)]
pub enum MigrationTlsLoadError {
    /// A required identity field is missing while migration is enabled.
    Missing(String),
    /// A configured identity file could not be read.
    Unreadable {
        path: std::path::PathBuf,
        source: std::io::Error,
    },
    /// The identity material failed to parse or the key does not match the
    /// certificate.
    Invalid(String),
}

impl fmt::Display for MigrationTlsLoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MigrationTlsLoadError::Missing(msg) => write!(f, "migration TLS config missing: {msg}"),
            MigrationTlsLoadError::Unreadable { path, source } => {
                write!(
                    f,
                    "cannot read migration TLS file {}: {source}",
                    path.display()
                )
            }
            MigrationTlsLoadError::Invalid(msg) => {
                write!(f, "invalid migration TLS material: {msg}")
            }
        }
    }
}

impl std::error::Error for MigrationTlsLoadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            MigrationTlsLoadError::Unreadable { source, .. } => Some(source),
            _ => None,
        }
    }
}

/// Load and validate the migration mTLS identity from configuration fields.
///
/// Returns `Ok(None)` when `enabled` is `false`. When `enabled` is `true`,
/// requires and validates `client_cert_path`, `client_key_path`,
/// `ca_cert_path`, and `dest_server_name`, returning a fully validated
/// [`MigrationTlsConfig`] on success.
pub fn load_migration_tls(
    enabled: bool,
    client_cert_path: Option<&Path>,
    client_key_path: Option<&Path>,
    ca_cert_path: Option<&Path>,
    dest_server_name: Option<&str>,
) -> Result<Option<MigrationTlsConfig>, MigrationTlsLoadError> {
    if !enabled {
        tracing::info!("storage migration is disabled: migration actions will be unavailable");
        return Ok(None);
    }

    let cert_path = client_cert_path.ok_or_else(|| {
        MigrationTlsLoadError::Missing(
            "migration.client_cert_path is required when migration.enabled = true".into(),
        )
    })?;
    let key_path = client_key_path.ok_or_else(|| {
        MigrationTlsLoadError::Missing(
            "migration.client_key_path is required when migration.enabled = true".into(),
        )
    })?;
    let ca_path = ca_cert_path.ok_or_else(|| {
        MigrationTlsLoadError::Missing(
            "migration.ca_cert_path is required when migration.enabled = true".into(),
        )
    })?;
    let dest = dest_server_name
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| {
            MigrationTlsLoadError::Missing(
                "migration.dest_server_name is required when migration.enabled = true".into(),
            )
        })?;

    let cert_pem = read_identity_file(cert_path, "client certificate")?;
    let key_pem = read_identity_file(key_path, "client key")?;
    let ca_pem = read_identity_file(ca_path, "CA bundle")?;

    validate_keypair(&cert_pem, &key_pem)?;
    validate_ca_bundle(&ca_pem)?;

    Ok(Some(MigrationTlsConfig {
        cert_pem,
        key_pem,
        ca_pem,
        dest_domain: dest.to_string(),
    }))
}

/// Read a PEM identity file. Errors are surfaced without file contents.
fn read_identity_file(path: &Path, what: &str) -> Result<Vec<u8>, MigrationTlsLoadError> {
    match std::fs::read(path) {
        Ok(bytes) => Ok(bytes),
        Err(source) => {
            let err = MigrationTlsLoadError::Unreadable {
                path: path.to_path_buf(),
                source,
            };
            tracing::error!(path = %path.display(), %what, "cannot read migration TLS file");
            Err(err)
        }
    }
}

/// Validate that `key_pem` is a parseable private key and that it matches the
/// public key in `cert_pem` (SPKI comparison).
fn validate_keypair(cert_pem: &[u8], key_pem: &[u8]) -> Result<(), MigrationTlsLoadError> {
    let cert_str = String::from_utf8_lossy(cert_pem);
    let key_str = String::from_utf8_lossy(key_pem);

    // Parsing the key validates it is a well-formed private key.
    let key = rcgen::KeyPair::from_pem(&key_str)
        .map_err(|e| MigrationTlsLoadError::Invalid(format!("invalid private key: {e}")))?;

    // Parsing the certificate validates it is a well-formed X.509 cert.
    // Bind the `Pem` so the borrowed `X509Certificate` does not dangle.
    let (_, pem) = x509_parser::pem::parse_x509_pem(cert_str.as_bytes())
        .map_err(|e| MigrationTlsLoadError::Invalid(format!("invalid certificate: {e}")))?;
    let cert = pem
        .parse_x509()
        .map_err(|e| MigrationTlsLoadError::Invalid(format!("invalid certificate: {e}")))?;

    // Canonical SPKI comparison: both sides go through rcgen's
    // `SubjectPublicKeyInfo` (normalized key encoding), avoiding any
    // platform-specific raw-key representation differences.
    let cert_spki = rcgen::SubjectPublicKeyInfo::from_der(cert.public_key().raw)
        .map_err(|e| MigrationTlsLoadError::Invalid(format!("invalid certificate: {e}")))?;
    let key_spki = rcgen::SubjectPublicKeyInfo::from_pem(&key.public_key_pem())
        .map_err(|e| MigrationTlsLoadError::Invalid(format!("invalid private key: {e}")))?;

    if cert_spki != key_spki {
        return Err(MigrationTlsLoadError::Invalid(
            "certificate does not match private key".into(),
        ));
    }

    Ok(())
}

/// Validate that `ca_pem` contains at least one parseable X.509 certificate.
fn validate_ca_bundle(ca_pem: &[u8]) -> Result<(), MigrationTlsLoadError> {
    let mut rest: &[u8] = ca_pem;
    let mut count = 0u32;
    loop {
        let trimmed = trim_ascii_whitespace(rest);
        if trimmed.is_empty() {
            break;
        }
        let (remaining, pem) = x509_parser::pem::parse_x509_pem(trimmed)
            .map_err(|e| MigrationTlsLoadError::Invalid(format!("invalid CA bundle: {e}")))?;
        pem.parse_x509()
            .map_err(|e| MigrationTlsLoadError::Invalid(format!("invalid CA bundle: {e}")))?;
        count += 1;
        rest = remaining;
    }
    if count == 0 {
        return Err(MigrationTlsLoadError::Invalid(
            "CA bundle contains no certificates".into(),
        ));
    }
    Ok(())
}

fn trim_ascii_whitespace(mut b: &[u8]) -> &[u8] {
    while let [first, rest @ ..] = b {
        if first.is_ascii_whitespace() {
            b = rest;
        } else {
            break;
        }
    }
    b
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Generate a self-signed leaf cert for `cn` plus its key, as a matching pair.
    fn matching_pair(cn: &str) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
        use rcgen::string::Ia5String;
        use rcgen::{CertificateParams, DistinguishedName, DnType, IsCa, KeyPair, SanType};

        let key = KeyPair::generate().unwrap();
        let mut params = CertificateParams::default();
        params.distinguished_name = DistinguishedName::new();
        params.distinguished_name.push(DnType::CommonName, cn);
        let dns = Ia5String::try_from(cn.to_string()).unwrap();
        params.subject_alt_names.push(SanType::DnsName(dns));
        params.is_ca = IsCa::NoCa;
        let cert = params.self_signed(&key).unwrap();
        let ca = crate::migration::tests::test_ca_pem();

        (
            cert.pem().into_bytes(),
            key.serialize_pem().into_bytes(),
            ca,
        )
    }

    fn mismatched_pair(cn: &str) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
        use rcgen::{CertificateParams, DistinguishedName, DnType, IsCa, KeyPair};

        let (_, _, ca) = matching_pair("unused");
        // Cert signed by one key, but we attach a *different* key.
        let cert_key = KeyPair::generate().unwrap();
        let mut params = CertificateParams::default();
        params.distinguished_name = DistinguishedName::new();
        params.distinguished_name.push(DnType::CommonName, cn);
        params.is_ca = IsCa::NoCa;
        let cert = params.self_signed(&cert_key).unwrap();
        let wrong_key = KeyPair::generate().unwrap();
        (
            cert.pem().into_bytes(),
            wrong_key.serialize_pem().into_bytes(),
            ca,
        )
    }

    fn tmp_file(bytes: &[u8]) -> tempfile::NamedTempFile {
        let mut f = tempfile::NamedTempFile::new().unwrap();
        std::io::Write::write_all(&mut f, bytes).unwrap();
        f
    }

    /// Write cert/key/CA to temp files. Returns the temp files (kept alive so
    /// their paths remain readable for the test's duration) plus their paths.
    #[allow(clippy::type_complexity)]
    fn write_all(
        cert: &[u8],
        key: &[u8],
        ca: &[u8],
    ) -> (
        (
            tempfile::NamedTempFile,
            tempfile::NamedTempFile,
            tempfile::NamedTempFile,
        ),
        (std::path::PathBuf, std::path::PathBuf, std::path::PathBuf),
    ) {
        let c = tmp_file(cert);
        let k = tmp_file(key);
        let a = tmp_file(ca);
        let paths = (
            c.path().to_path_buf(),
            k.path().to_path_buf(),
            a.path().to_path_buf(),
        );
        ((c, k, a), paths)
    }

    #[test]
    fn disabled_returns_none_without_any_inputs() {
        match load_migration_tls(false, None, None, None, None).expect("no error when disabled") {
            None => {}
            Some(_) => panic!("migration disabled must not produce a TLS config"),
        }
    }

    #[test]
    fn enabled_requires_all_fields() {
        match load_migration_tls(true, None, None, None, None) {
            Err(MigrationTlsLoadError::Missing(_)) => {}
            Ok(_) => panic!("expected a Missing error, but a TLS config was produced"),
            Err(other) => panic!("unexpected error variant: {other:?}"),
        }
    }

    #[test]
    fn enabled_missing_dest_server_name_fails() {
        let ((_c, _k, _a), (cp, kp, ap)) = {
            let (c, k, a) = matching_pair("node-a");
            write_all(&c, &k, &a)
        };
        match load_migration_tls(true, Some(&cp), Some(&kp), Some(&ap), None) {
            Err(MigrationTlsLoadError::Missing(_)) => {}
            Ok(_) => panic!("expected a Missing error, but a TLS config was produced"),
            Err(other) => panic!("unexpected error variant: {other:?}"),
        }
    }

    #[test]
    fn enabled_valid_pair_loads_config_without_plaintext() {
        let (c, k, a) = matching_pair("node-a");
        let ((_c, _k, _a), (cp, kp, ap)) = write_all(&c, &k, &a);
        let opt = load_migration_tls(true, Some(&cp), Some(&kp), Some(&ap), Some("stord-peer"))
            .expect("valid pair should load");
        let cfg = match opt {
            Some(cfg) => cfg,
            None => panic!("enabled migration must produce a TLS config"),
        };
        assert_eq!(cfg.dest_domain, "stord-peer");
        assert!(cfg.cert_pem == c, "cert PEM must be preserved");
        assert!(cfg.key_pem == k, "key PEM must be preserved");
        assert!(cfg.ca_pem == a, "CA PEM must be preserved");
    }

    /// x509-parser 0.18 behavior lock: the PEM reader ignores lines that are
    /// not valid UTF-8 in the comment section before BEGIN (some provisioning
    /// tools emit them). A certificate PEM carrying such a comment must still
    /// load and validate.
    #[test]
    fn enabled_cert_pem_with_non_utf8_comment_loads() {
        let (cert, key, ca) = matching_pair("node-a");
        let mut commented = b"# provisioning note: \xFF\xFE\xf0\x28\x8c\x28\n".to_vec();
        commented.extend_from_slice(&cert);
        let ((_c, _k, _a), (cp, kp, ap)) = write_all(&commented, &key, &ca);
        load_migration_tls(true, Some(&cp), Some(&kp), Some(&ap), Some("stord-peer"))
            .expect("PEM with a non-UTF-8 comment line must load");
    }

    #[test]
    fn enabled_mismatched_keypair_fails() {
        let (c, k, a) = mismatched_pair("node-a");
        let ((_c, _k, _a), (cp, kp, ap)) = write_all(&c, &k, &a);
        match load_migration_tls(true, Some(&cp), Some(&kp), Some(&ap), Some("stord-peer")) {
            Err(MigrationTlsLoadError::Invalid(msg)) => {
                assert!(
                    msg.contains("does not match"),
                    "expected mismatch message, got: {msg}"
                );
            }
            Ok(_) => panic!("expected an Invalid error, but a TLS config was produced"),
            Err(other) => panic!("unexpected error variant: {other:?}"),
        }
    }

    #[test]
    fn enabled_unreadable_file_fails() {
        let (c, k, a) = matching_pair("node-a");
        let ((_c, _k, _a), (_, kp, ap)) = write_all(&c, &k, &a);
        let missing = std::path::PathBuf::from("/nonexistent/chv-migration-cert.pem");
        match load_migration_tls(
            true,
            Some(&missing),
            Some(&kp),
            Some(&ap),
            Some("stord-peer"),
        ) {
            Err(MigrationTlsLoadError::Unreadable { path, .. }) => {
                assert_eq!(path, missing);
            }
            Ok(_) => panic!("expected an Unreadable error, but a TLS config was produced"),
            Err(other) => panic!("unexpected error variant: {other:?}"),
        }
    }

    #[test]
    fn enabled_garbage_pem_rejected() {
        let (c, k, a) = matching_pair("node-a");
        let ((_c, _k, _a), (_, kp, ap)) = write_all(&c, &k, &a);
        let bad = tmp_file(b"not a pem at all\n");
        match load_migration_tls(
            true,
            Some(bad.path()),
            Some(&kp),
            Some(&ap),
            Some("stord-peer"),
        ) {
            Err(MigrationTlsLoadError::Invalid(_)) => {}
            Ok(_) => panic!("expected an Invalid error, but a TLS config was produced"),
            Err(other) => panic!("unexpected error variant: {other:?}"),
        }
    }
}
