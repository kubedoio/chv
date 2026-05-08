use clap::Subcommand;
use serde_json::json;

use crate::client::{BffClient, CliError};
use crate::output::{self, OutputFormat};

#[derive(Subcommand)]
pub enum BackupCommands {
    /// List all backups
    List,
    /// Run a backup for a VM
    Run {
        /// VM identifier to back up
        vm_id: String,
        /// Backup label
        #[arg(long)]
        label: Option<String>,
    },
}

pub async fn execute(
    client: &BffClient,
    command: BackupCommands,
    format: &OutputFormat,
) -> Result<(), CliError> {
    match command {
        BackupCommands::List => {
            let resp = client.get("/v1/backups").await?;
            let items = resp
                .get("backups")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            output::print_list(
                &items,
                &[
                    "backup_id",
                    "vm_id",
                    "label",
                    "size",
                    "status",
                    "created_at",
                ],
                format,
            );
        }
        BackupCommands::Run { vm_id, label } => {
            let mut body = json!({ "vm_id": vm_id });
            if let Some(l) = label {
                body["label"] = json!(l);
            }
            let resp = client.post("/v1/backups/run", &body).await?;
            println!("Backup initiated.");
            output::print_value(&resp, format);
        }
    }
    Ok(())
}
