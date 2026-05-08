use clap::Subcommand;
use serde_json::json;

use crate::client::{BffClient, CliError};
use crate::output::{self, OutputFormat};

#[derive(Subcommand)]
pub enum NetworkCommands {
    /// List all networks
    List,
    /// Create a new network
    Create {
        /// Network name
        name: String,
        /// CIDR block (e.g. "10.0.0.0/24")
        #[arg(long)]
        cidr: String,
        /// VLAN ID
        #[arg(long)]
        vlan: Option<u32>,
    },
    /// Delete a network
    Delete {
        /// Network identifier
        network_id: String,
    },
}

pub async fn execute(
    client: &BffClient,
    command: NetworkCommands,
    format: &OutputFormat,
) -> Result<(), CliError> {
    match command {
        NetworkCommands::List => {
            let resp = client.post("/v1/networks", &json!({})).await?;
            let items = resp
                .get("networks")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            output::print_list(
                &items,
                &["network_id", "name", "cidr", "vlan", "status"],
                format,
            );
        }
        NetworkCommands::Create { name, cidr, vlan } => {
            let mut body = json!({ "name": name, "cidr": cidr });
            if let Some(v) = vlan {
                body["vlan"] = json!(v);
            }
            let resp = client.post("/v1/networks/create", &body).await?;
            println!("Network created.");
            output::print_value(&resp, format);
        }
        NetworkCommands::Delete { network_id } => {
            let body = json!({ "network_id": network_id });
            client.post("/v1/networks/delete", &body).await?;
            println!("Network {network_id} deleted.");
        }
    }
    Ok(())
}
