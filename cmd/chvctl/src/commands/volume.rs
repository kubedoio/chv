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
        /// Snapshot name
        #[arg(long)]
        name: Option<String>,
    },
    /// Clone a volume
    Clone {
        /// Source volume identifier
        volume_id: String,
        /// Name for the cloned volume
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
                .get("volumes")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            output::print_list(
                &items,
                &["volume_id", "name", "size", "status", "attached_to"],
                format,
            );
        }
        VolumeCommands::Snapshot { volume_id, name } => {
            let mut body = json!({ "volume_id": volume_id });
            if let Some(n) = name {
                body["name"] = json!(n);
            }
            let resp = client.post("/v1/volumes/snapshot", &body).await?;
            println!("Snapshot created.");
            output::print_value(&resp, format);
        }
        VolumeCommands::Clone { volume_id, name } => {
            let body = json!({ "volume_id": volume_id, "name": name });
            let resp = client.post("/v1/volumes/clone", &body).await?;
            println!("Volume cloned.");
            output::print_value(&resp, format);
        }
    }
    Ok(())
}
