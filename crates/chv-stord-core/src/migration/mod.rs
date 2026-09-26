pub mod flow_control;
pub mod receiver;
pub mod sender;
pub mod service;
pub mod task;
pub mod tls_config;

/// Test-support helpers shared across the migration module's unit tests.
#[cfg(test)]
pub mod tests {
    use rcgen::{BasicConstraints, CertificateParams, DistinguishedName, DnType, IsCa, KeyPair};

    /// A CA certificate PEM used by migration mTLS unit/negative tests.
    pub fn test_ca_pem() -> Vec<u8> {
        let key = KeyPair::generate().unwrap();
        let mut params = CertificateParams::default();
        params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
        params.distinguished_name = DistinguishedName::new();
        params
            .distinguished_name
            .push(DnType::CommonName, "chv-test-ca");
        let cert = params.self_signed(&key).unwrap();
        cert.pem().into_bytes()
    }
}
