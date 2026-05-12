use clap::Subcommand;

use crate::client::{BffClient, CliError};
use crate::output::{self, OutputFormat};

#[derive(Subcommand)]
pub enum HealthCommands {
    /// Run a quick health check against the control plane
    Check,
    /// Get detailed health report for a specific node
    Report {
        /// Node identifier
        node_id: String,
    },
    /// Get cluster-wide health summary
    Cluster,
}

pub async fn execute(
    client: &BffClient,
    command: HealthCommands,
    format: &OutputFormat,
) -> Result<(), CliError> {
    match command {
        HealthCommands::Check => {
            let resp = client.get("/v1/health").await?;
            output::print_value(&resp, format);
        }
        HealthCommands::Report { node_id } => {
            let resp = client.get(&format!("/v1/nodes/{}/health", node_id)).await?;
            output::print_value(&resp, format);
        }
        HealthCommands::Cluster => {
            let resp = client.get("/v1/cluster/health").await?;
            output::print_value(&resp, format);
        }
    }
    Ok(())
}
