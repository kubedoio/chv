use clap::Subcommand;
use serde_json::json;

use crate::client::{BffClient, CliError};
use crate::output::{self, OutputFormat};

#[derive(Subcommand)]
pub enum TaskCommands {
    /// List all tasks/operations
    List,
    /// Watch a task until completion
    Watch {
        /// Task identifier
        task_id: String,
    },
}

pub async fn execute(
    client: &BffClient,
    command: TaskCommands,
    format: &OutputFormat,
) -> Result<(), CliError> {
    match command {
        TaskCommands::List => {
            let resp = client.post("/v1/tasks", &json!({})).await?;
            let items = resp
                .get("tasks")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            output::print_list(
                &items,
                &["task_id", "type", "status", "resource_id", "created_at"],
                format,
            );
        }
        TaskCommands::Watch { task_id } => {
            println!("Watching task {task_id}...");
            loop {
                let resp = client
                    .post("/v1/tasks", &json!({ "task_id": task_id }))
                    .await?;
                let status = resp
                    .get("status")
                    .and_then(|s| s.as_str())
                    .unwrap_or("unknown");

                println!("  Status: {status}");

                match status {
                    "completed" | "failed" | "cancelled" => {
                        output::print_value(&resp, format);
                        break;
                    }
                    _ => {
                        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                    }
                }
            }
        }
    }
    Ok(())
}
