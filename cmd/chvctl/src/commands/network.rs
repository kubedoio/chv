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
                .get("items")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            // Columns are the keys the BFF actually serves (#372 DP10):
            // the old `cidr`/`vlan`/`status` were phantom columns — the
            // handler serves scope/health/exposure/ipam_mode/is_default.
            // `--vlan` was removed outright (#372 DP7): the capability
            // does not exist at any layer and the flag was silently
            // dropped by the server; #517 tracks real VLAN support.
            output::print_list(
                &items,
                &[
                    "network_id",
                    "name",
                    "scope",
                    "health",
                    "exposure",
                    "ipam_mode",
                    "is_default",
                ],
                format,
            );
        }
        NetworkCommands::Create { name, cidr } => {
            let body = json!({ "name": name, "cidr": cidr });
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
