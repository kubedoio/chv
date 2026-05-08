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
        /// Username
        username: String,
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
                .get("users")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            output::print_list(&items, &["username", "role", "created_at"], format);
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
        UserCommands::Delete { username } => {
            let body = json!({ "username": username });
            client.post("/v1/users/delete", &body).await?;
            println!("User '{username}' deleted.");
        }
    }
    Ok(())
}
