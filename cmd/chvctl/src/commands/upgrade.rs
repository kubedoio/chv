use clap::Subcommand;
use serde_json::json;

use crate::client::{BffClient, CliError};
use crate::output::{self, OutputFormat};

#[derive(Subcommand)]
pub enum UpgradeCommands {
    /// Start an upgrade on a node
    Start {
        /// Node identifier to upgrade
        node_id: String,
        /// Target version to upgrade to
        #[arg(long)]
        version: String,
    },
    /// Check upgrade status for a node
    Status {
        /// Node identifier
        node_id: String,
    },
    /// Rollback an upgrade on a node
    Rollback {
        /// Node identifier
        node_id: String,
    },
    /// List all upgrades
    List,
}

pub async fn execute(
    client: &BffClient,
    command: UpgradeCommands,
    format: &OutputFormat,
) -> Result<(), CliError> {
    match command {
        UpgradeCommands::Start { node_id, version } => {
            let body = json!({ "node_id": node_id, "version": version });
            let resp = client.post("/v1/upgrades", &body).await?;
            println!("Upgrade initiated for node {node_id} to version {version}.");
            output::print_value(&resp, format);
        }
        UpgradeCommands::Status { node_id } => {
            let resp = client.get(&format!("/v1/upgrades/{}", node_id)).await?;
            output::print_value(&resp, format);
        }
        UpgradeCommands::Rollback { node_id } => {
            let body = json!({ "action": "rollback" });
            client
                .post(&format!("/v1/upgrades/{}/rollback", node_id), &body)
                .await?;
            println!("Rollback initiated for node {node_id}.");
        }
        UpgradeCommands::List => {
            let resp = client.get("/v1/upgrades").await?;
            let items = resp
                .get("upgrades")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            output::print_list(
                &items,
                &[
                    "node_id",
                    "from_version",
                    "to_version",
                    "status",
                    "started_at",
                ],
                format,
            );
        }
    }
    Ok(())
}
