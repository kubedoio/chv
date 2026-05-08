use clap::Subcommand;
use serde_json::json;

use crate::client::{BffClient, CliError};
use crate::output::{self, OutputFormat};

#[derive(Subcommand)]
pub enum NodeCommands {
    /// List all compute nodes
    List,
    /// Get details of a specific node
    Get {
        /// Node identifier
        node_id: String,
    },
    /// Drain a node (evacuate all VMs)
    Drain {
        /// Node identifier
        node_id: String,
    },
    /// Put a node into maintenance mode
    Maintenance {
        /// Node identifier
        node_id: String,
        /// Enable or disable maintenance mode
        #[arg(long)]
        enable: bool,
    },
}

pub async fn execute(
    client: &BffClient,
    command: NodeCommands,
    format: &OutputFormat,
) -> Result<(), CliError> {
    match command {
        NodeCommands::List => {
            let resp = client.post("/v1/nodes", &json!({})).await?;
            let items = resp
                .get("nodes")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            output::print_list(
                &items,
                &[
                    "node_id",
                    "hostname",
                    "status",
                    "cpu_total",
                    "memory_total",
                    "vm_count",
                ],
                format,
            );
        }
        NodeCommands::Get { node_id } => {
            let resp = client
                .post("/v1/nodes", &json!({ "node_id": node_id }))
                .await?;
            output::print_value(&resp, format);
        }
        NodeCommands::Drain { node_id } => {
            let body = json!({ "node_id": node_id, "action": "drain" });
            client.post("/v1/nodes/mutate", &body).await?;
            println!("Node {node_id} draining.");
        }
        NodeCommands::Maintenance { node_id, enable } => {
            let action = if enable {
                "enter_maintenance"
            } else {
                "exit_maintenance"
            };
            let body = json!({ "node_id": node_id, "action": action });
            client.post("/v1/nodes/mutate", &body).await?;
            if enable {
                println!("Node {node_id} entering maintenance mode.");
            } else {
                println!("Node {node_id} exiting maintenance mode.");
            }
        }
    }
    Ok(())
}
