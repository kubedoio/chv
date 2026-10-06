use clap::Subcommand;
use serde_json::json;

use super::vm::{parse_size_bytes, validate_storage_class};
use crate::client::{BffClient, CliError};
use crate::output::{self, OutputFormat};

#[derive(Subcommand)]
pub enum VolumeCommands {
    /// List all storage volumes
    List,
    /// Create a standalone data volume on a named node
    Create {
        /// Name for the new volume
        name: String,
        /// Placement node id (required — standalone volumes have no VM
        /// to place them and there is no default node)
        #[arg(long)]
        node: String,
        /// Capacity in BYTES (e.g. "1073741824", "512M", "10G"; a bare
        /// number is bytes, K/M/G/T suffixes are binary KiB/MiB/GiB/TiB),
        /// bounded 1 byte ..= 64 TiB. NOTE: unlike `vm create`'s
        /// GiB-valued --disk-size-gb, this flag is bytes-denominated —
        /// the volume-create contract field is `capacity_bytes` (#513
        /// DP9).
        #[arg(long)]
        size: String,
        /// Storage class (local, iscsi, ceph, lvm) — validated
        /// client-side against the same shared vocabulary the BFF
        /// checks (#372 DP9 / #513 DP5)
        #[arg(long)]
        storage_class: Option<String>,
    },
    /// Create a snapshot of a volume
    Snapshot {
        /// Volume identifier
        volume_id: String,
        /// Snapshot name (required — the BFF rejects the request without it)
        #[arg(long)]
        name: String,
    },
    /// Clone a volume
    Clone {
        /// Source volume identifier
        volume_id: String,
        /// Target volume id for the clone (the new volume's id)
        #[arg(long)]
        name: String,
    },
}

pub async fn execute(
    client: &BffClient,
    command: VolumeCommands,
    format: &OutputFormat,
) -> Result<(), CliError> {
    match command {
        VolumeCommands::List => {
            let resp = client.post("/v1/volumes", &json!({})).await?;
            let items = resp
                .get("items")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            // `attached_to` was a phantom column — the BFF serves
            // `attached_vm_id`/`attached_vm_name` (#372 DP10/§2.7(a)).
            output::print_list(
                &items,
                &[
                    "volume_id",
                    "name",
                    "size",
                    "status",
                    "attached_vm_id",
                    "attached_vm_name",
                ],
                format,
            );
        }
        VolumeCommands::Create {
            name,
            node,
            size,
            storage_class,
        } => {
            // #513 DP9: the wire contract is POST /v1/volumes/create —
            // `name`, a REQUIRED `node_id` (the flag is clap-required,
            // so the client never sends a node-less create; the BFF has
            // no first-enrolled-node default for storage placement),
            // `capacity_bytes` in BYTES, and an optional
            // `storage_class`. The capacity is deliberately NOT
            // GiB-denominated like `vm create`'s --disk-size-gb: the
            // route's field is `capacity_bytes`, so --size goes through
            // parse_size_bytes and the help text spells the unit out.
            let capacity_bytes = parse_size_bytes(&size)?;
            // Client-side mirror of the BFF's bound — the single shared
            // constant (chv_hypervisor_api::resources::MAX_VOLUME_BYTES,
            // the same shared home as the class vocabulary) — so a
            // typo'd or oversized --size fails locally with the same
            // rule the server enforces; the server check stays
            // authoritative. parse_size_bytes accepts "0", so the
            // lower bound is checked here too.
            if capacity_bytes <= 0
                || capacity_bytes > chv_hypervisor_api::resources::MAX_VOLUME_BYTES
            {
                return Err(CliError::Parse(format!(
                    "invalid --size {size:?}: must be between 1 and {} bytes (64 TiB)",
                    chv_hypervisor_api::resources::MAX_VOLUME_BYTES
                )));
            }
            let mut body = json!({
                "name": name,
                "node_id": node,
                "capacity_bytes": capacity_bytes,
            });
            if let Some(class) = storage_class {
                // Trim before validating AND before sending (the #519
                // discipline, parity with `vm create --storage-class`
                // and the BFF's trim-before-check): an untrimmed
                // client-side compare would over-reject where the
                // server trims first.
                let class = class.trim();
                validate_storage_class(class)?;
                body["storage_class"] = json!(class);
            }
            // The reserved keys (`attached_vm_id`, `seed_image_ref`)
            // are deliberately NOT expressible from the CLI — the BFF
            // rejects both with a loud 400 (DP4), and chvctl has no
            // flag for them; attach is the mutate path's job.
            let resp = client.post("/v1/volumes/create", &body).await?;
            println!("Volume created successfully.");
            output::print_value(&resp, format);
        }
        VolumeCommands::Snapshot { volume_id, name } => {
            // Field names are the BFF/proto contract (#372): the request
            // is rejected without a snapshot_name.
            let body = json!({ "volume_id": volume_id, "snapshot_name": name });
            let resp = client.post("/v1/volumes/snapshot", &body).await?;
            println!("Snapshot created.");
            output::print_value(&resp, format);
        }
        VolumeCommands::Clone { volume_id, name } => {
            // Field names are the BFF/proto contract (#372): the clone
            // target is a full volume id, not a display name.
            let body = json!({ "source_volume_id": volume_id, "target_volume_id": name });
            let resp = client.post("/v1/volumes/clone", &body).await?;
            println!("Volume cloned.");
            output::print_value(&resp, format);
        }
    }
    Ok(())
}
