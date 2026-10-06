use clap::Subcommand;
use serde_json::json;

use crate::client::{BffClient, CliError};
use crate::output::{self, OutputFormat};

#[derive(Subcommand)]
pub enum UserCommands {
    /// List all users
    List,
    /// Create a new user
    Create {
        /// Username
        username: String,
        /// Password
        #[arg(long)]
        password: String,
        /// Role (admin, operator, viewer)
        #[arg(long, default_value = "viewer")]
        role: String,
    },
    /// Delete a user
    Delete {
        /// User identifier (from `user list`) — the BFF's delete contract
        /// key (#372 DP2; the command previously sent `username`, which
        /// the handler rejected with 400 `missing user_id` on every
        /// invocation — it had never worked, so there is no compat
        /// surface)
        user_id: String,
    },
}

pub async fn execute(
    client: &BffClient,
    command: UserCommands,
    format: &OutputFormat,
) -> Result<(), CliError> {
    match command {
        UserCommands::List => {
            let resp = client.post("/v1/users", &json!({})).await?;
            let items = resp
                .get("items")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            // `user_id` is served by the list and makes the delete
            // contract's key discoverable (#372 DP2).
            output::print_list(
                &items,
                &["user_id", "username", "role", "created_at"],
                format,
            );
        }
        UserCommands::Create {
            username,
            password,
            role,
        } => {
            let body = json!({
                "username": username,
                "password": password,
                "role": role,
            });
            let resp = client.post("/v1/users/create", &body).await?;
            println!("User '{username}' created.");
            output::print_value(&resp, format);
        }
        UserCommands::Delete { user_id } => {
            // The BFF's delete_user requires `user_id` (self-delete guard
            // on the JWT `sub`); `username` was read by nothing (#372 DP2).
            let body = json!({ "user_id": user_id });
            client.post("/v1/users/delete", &body).await?;
            println!("User '{user_id}' deleted.");
        }
    }
    Ok(())
}
