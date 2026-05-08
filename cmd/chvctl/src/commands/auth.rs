use clap::Args;
use serde_json::json;

use crate::client::{BffClient, CliError};
use crate::config;

#[derive(Args)]
pub struct LoginArgs {
    /// Username for authentication
    #[arg(long)]
    pub username: String,

    /// Password for authentication
    #[arg(long)]
    pub password: String,
}

pub async fn execute(client: &BffClient, args: LoginArgs) -> Result<(), CliError> {
    let body = json!({
        "username": args.username,
        "password": args.password,
    });

    let resp = client.post("/v1/auth/login", &body).await?;

    let token = resp
        .get("token")
        .and_then(|t| t.as_str())
        .ok_or_else(|| CliError::Parse("no token in response".to_string()))?;

    config::save_credentials(token).map_err(|e| CliError::Parse(e))?;

    println!("Login successful. Token saved to ~/.config/chvctl/credentials");
    Ok(())
}
