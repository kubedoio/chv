use clap::Subcommand;
use serde_json::json;

use crate::client::{BffClient, CliError};
use crate::output::{self, OutputFormat};

#[derive(Subcommand)]
pub enum VolumeCommands {
    /// List all storage volumes
    List,
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
