use clap::Subcommand;
use serde_json::json;

use crate::client::{BffClient, CliError};
use crate::output::{self, OutputFormat};

#[derive(Subcommand)]
pub enum MigrateCommands {
    /// Start a live migration for a VM
    Start {
        /// VM identifier to migrate
        vm_id: String,
        /// Target node to migrate to
        target_node: String,
    },
    /// Check status of a migration
    Status {
        /// Migration identifier
        migration_id: String,
    },
    /// Cancel an in-progress migration
    Cancel {
        /// Migration identifier
        migration_id: String,
    },
    /// List all migrations
    List,
}

pub async fn execute(
    client: &BffClient,
    command: MigrateCommands,
    format: &OutputFormat,
) -> Result<(), CliError> {
    match command {
        MigrateCommands::Start { vm_id, target_node } => {
            let body = json!({ "vm_id": vm_id, "target_node": target_node });
            let resp = client.post("/v1/migrations", &body).await?;
            println!("Migration initiated.");
            output::print_value(&resp, format);
        }
        MigrateCommands::Status { migration_id } => {
            let resp = client
                .get(&format!("/v1/migrations/{}", migration_id))
                .await?;
            output::print_value(&resp, format);
        }
        MigrateCommands::Cancel { migration_id } => {
            let body = json!({ "action": "cancel" });
            client
                .post(&format!("/v1/migrations/{}/cancel", migration_id), &body)
                .await?;
            println!("Migration {migration_id} cancelled.");
        }
        MigrateCommands::List => {
            let resp = client.get("/v1/migrations").await?;
            let items = resp
                .get("migrations")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            output::print_list(
                &items,
                &[
                    "migration_id",
                    "vm_id",
                    "source_node",
                    "target_node",
                    "status",
                    "progress",
                ],
                format,
            );
        }
    }
    Ok(())
}
