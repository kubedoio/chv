use clap::Subcommand;
use serde_json::json;

use crate::client::{BffClient, CliError};
use crate::output::{self, OutputFormat};

#[derive(Subcommand)]
pub enum ImageCommands {
    /// List all disk images
    List,
    /// Import a new image
    Import {
        /// Image name
        name: String,
        /// Source URL for the image
        #[arg(long)]
        url: String,
        /// Image format (qcow2, raw)
        #[arg(long, default_value = "qcow2")]
        format: String,
    },
    /// Delete an image
    Delete {
        /// Image identifier
        image_id: String,
    },
}

pub async fn execute(
    client: &BffClient,
    command: ImageCommands,
    format: &OutputFormat,
) -> Result<(), CliError> {
    match command {
        ImageCommands::List => {
            let resp = client.post("/v1/images", &json!({})).await?;
            let items = resp
                .get("images")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            output::print_list(
                &items,
                &["image_id", "name", "format", "size", "status"],
                format,
            );
        }
        ImageCommands::Import {
            name,
            url,
            format: img_format,
        } => {
            let body = json!({
                "name": name,
                "url": url,
                "format": img_format,
            });
            let resp = client.post("/v1/images/import", &body).await?;
            println!("Image import initiated.");
            output::print_value(&resp, format);
        }
        ImageCommands::Delete { image_id } => {
            let body = json!({ "image_id": image_id });
            client.post("/v1/images/delete", &body).await?;
            println!("Image {image_id} deleted.");
        }
    }
    Ok(())
}
