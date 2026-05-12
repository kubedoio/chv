use clap::Subcommand;
use serde_json::json;

use crate::client::{BffClient, CliError};
use crate::output::{self, OutputFormat};

#[derive(Subcommand)]
pub enum StorageCommands {
    /// List storage pools
    List,
    /// Show storage pool details
    Show {
        /// Pool identifier
        pool_id: String,
    },
    /// Create a storage pool
    Create {
        /// Pool name
        name: String,
        /// Storage backend type (e.g. "local", "ceph", "iscsi")
        #[arg(long)]
        backend: String,
        /// Path for local backends
        #[arg(long)]
        path: Option<String>,
    },
    /// Delete a storage pool
    Delete {
        /// Pool identifier
        pool_id: String,
    },
}

pub async fn execute(
    client: &BffClient,
    command: StorageCommands,
    format: &OutputFormat,
) -> Result<(), CliError> {
    match command {
        StorageCommands::List => {
            let resp = client.get("/v1/storage/pools").await?;
            let items = resp
                .get("pools")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            output::print_list(
                &items,
                &["pool_id", "name", "backend", "path", "status", "capacity"],
                format,
            );
        }
        StorageCommands::Show { pool_id } => {
            let resp = client
                .get(&format!("/v1/storage/pools/{}", pool_id))
                .await?;
            output::print_value(&resp, format);
        }
        StorageCommands::Create {
            name,
            backend,
            path,
        } => {
            let body = json!({ "name": name, "backend": backend, "path": path });
            let resp = client.post("/v1/storage/pools", &body).await?;
            println!("Storage pool created.");
            output::print_value(&resp, format);
        }
        StorageCommands::Delete { pool_id } => {
            client
                .delete(&format!("/v1/storage/pools/{}", pool_id))
                .await?;
            println!("Storage pool {pool_id} deleted.");
        }
    }
    Ok(())
}
