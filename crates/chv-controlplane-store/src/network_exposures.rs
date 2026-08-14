use chv_controlplane_types::domain::ResourceId;

pub(crate) const UPSERT_SQL: &str = r#"
INSERT INTO network_exposures (
    network_id, service_name, protocol, listen_address, listen_port,
    target_address, target_port, exposure_policy, updated_at
)
VALUES ($1, $2, $3, $4, $5, $6, $7, $8, strftime('%Y-%m-%dT%H:%M:%SZ', $9 / 1000.0, 'unixepoch'))
ON CONFLICT (network_id, service_name) DO UPDATE SET
    protocol = EXCLUDED.protocol,
    listen_address = EXCLUDED.listen_address,
    listen_port = EXCLUDED.listen_port,
    target_address = EXCLUDED.target_address,
    target_port = EXCLUDED.target_port,
    exposure_policy = EXCLUDED.exposure_policy,
    updated_at = EXCLUDED.updated_at
"#;

#[derive(Clone)]
pub struct NetworkExposureInput {
    pub network_id: ResourceId,
    pub service_name: String,
    pub protocol: String,
    pub listen_address: Option<String>,
    pub listen_port: Option<i32>,
    pub target_address: Option<String>,
    pub target_port: Option<i32>,
    pub exposure_policy: Option<String>,
    pub updated_unix_ms: i64,
}
