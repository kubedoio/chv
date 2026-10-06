use clap::Subcommand;
use serde_json::json;
use std::time::{Duration, Instant};

use crate::client::{BffClient, CliError};
use crate::output::{self, OutputFormat};

/// Terminal operation statuses (`chv-controlplane-types` `OperationStatus`
/// — capitalized `Succeeded`, never the old lowercase `completed`).
/// `watch` stops on these; `Succeeded` is the only success, every other
/// terminal status exits non-zero (#372 DP6).
const TERMINAL_STATUSES: [&str; 6] = [
    "Succeeded",
    "Failed",
    "Cancelled",
    "Rejected",
    "Stale",
    "Conflict",
];

/// The recorded terminal-failure cause of an operation row (#502): the
/// BFF's task-carrying responses surface the journaled
/// `error_code`/`error_message` pair (the #498/#500 fast-fail columns
/// — `UNSUPPORTED_BY_AGENT`, `DISPATCH_FAILED`, `AGENT_REJECTED`,
/// `MIGRATION_FAILED`, … plus the agents' verbatim refusal text).
/// NULL until a failure records them; never fabricated here — a
/// terminal op with no recorded cause renders no cause line at all.
pub fn failure_cause(detail: &serde_json::Value) -> Option<String> {
    let code = detail
        .get("error_code")
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty());
    let message = detail
        .get("error_message")
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty());
    match (code, message) {
        (Some(code), Some(message)) => Some(format!("{code} — {message}")),
        (Some(code), None) => Some(code.to_string()),
        (None, Some(message)) => Some(message.to_string()),
        (None, None) => None,
    }
}

#[derive(Subcommand)]
pub enum TaskCommands {
    /// List all tasks/operations
    List,
    /// Watch a task until it reaches a terminal status
    Watch {
        /// Task identifier
        task_id: String,
        /// Give up after this many seconds (default 900 = 15 minutes)
        /// so a stuck task cannot hang the CLI (#372 DP6)
        #[arg(long, default_value_t = 900)]
        timeout: u64,
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
                .get("items")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            // Columns are the keys the BFF actually serves (#372 DP10):
            // the old `type`/`created_at` were phantom columns — the
            // handler serves `operation` and `started_unix_ms`.
            output::print_list(
                &items,
                &[
                    "task_id",
                    "status",
                    "operation",
                    "resource_id",
                    "started_unix_ms",
                ],
                format,
            );
        }
        TaskCommands::Watch { task_id, timeout } => {
            println!("Watching task {task_id}...");
            let deadline = Instant::now() + Duration::from_secs(timeout);
            loop {
                // Single-task get route (#372 DP6): the old code polled
                // POST /v1/tasks with a `task_id` key the list handler
                // silently ignored — the response had no top-level
                // `status`, so the loop printed `Status: unknown`
                // forever. A task id that does not exist now surfaces
                // the route's 404 immediately instead of hanging.
                let resp = client
                    .post("/v1/tasks/get", &json!({ "task_id": task_id }))
                    .await?;
                let detail = resp
                    .get("detail")
                    .cloned()
                    .ok_or_else(|| CliError::Parse("unexpected /v1/tasks/get shape".into()))?;
                let status = detail
                    .get("status")
                    .and_then(|s| s.as_str())
                    .unwrap_or("unknown");

                println!("  Status: {status}");

                if status == "Succeeded" {
                    output::print_value(&detail, format);
                    break;
                }
                if TERMINAL_STATUSES.contains(&status) {
                    // #502: surface WHY the task failed terminally —
                    // the journaled code plus the agents' verbatim
                    // refusal text, printed before the full detail and
                    // the non-zero exit (which is unchanged).
                    if let Some(cause) = failure_cause(&detail) {
                        println!("  Cause: {cause}");
                    }
                    output::print_value(&detail, format);
                    return Err(CliError::Task(format!(
                        "task {task_id} reached terminal status {status}"
                    )));
                }
                if Instant::now() >= deadline {
                    return Err(CliError::Task(format!(
                        "watch timed out after {timeout}s (task {task_id} still {status})"
                    )));
                }
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        }
    }
    Ok(())
}
