use clap::Subcommand;
use serde_json::json;

use crate::client::{BffClient, CliError};
use crate::output::{self, OutputFormat};

#[derive(Subcommand)]
pub enum VmCommands {
    /// List all virtual machines
    List,
    /// Get details of a specific VM
    Get {
        /// VM identifier
        vm_id: String,
    },
    /// Create a new virtual machine
    Create {
        /// Name for the new VM
        name: String,
        /// Number of vCPUs
        #[arg(long)]
        cpu: Option<u32>,
        /// Memory size (e.g. "2G", "512M")
        #[arg(long)]
        memory: Option<String>,
        /// Base image to use
        #[arg(long)]
        image: Option<String>,
        /// Network to attach
        #[arg(long)]
        network: Option<String>,
    },
    /// Start a virtual machine
    Start {
        /// VM identifier
        vm_id: String,
    },
    /// Stop a virtual machine
    Stop {
        /// VM identifier
        vm_id: String,
    },
    /// Reboot a virtual machine
    Reboot {
        /// VM identifier
        vm_id: String,
    },
    /// Delete a virtual machine
    Delete {
        /// VM identifier
        vm_id: String,
    },
    /// Migrate a VM to another node
    Migrate {
        /// VM identifier
        vm_id: String,
        /// Target node ID
        #[arg(long)]
        to: String,
    },
    /// Resize a VM's resources
    Resize {
        /// VM identifier
        vm_id: String,
        /// New vCPU count
        #[arg(long)]
        cpu: Option<u32>,
        /// New memory size (e.g. "4G")
        #[arg(long)]
        memory: Option<String>,
    },
}

pub async fn execute(
    client: &BffClient,
    command: VmCommands,
    format: &OutputFormat,
) -> Result<(), CliError> {
    match command {
        VmCommands::List => {
            let resp = client.post("/v1/vms", &json!({})).await?;
            let items = resp
                .get("vms")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            output::print_list(
                &items,
                &["vm_id", "name", "status", "node_id", "cpu", "memory"],
                format,
            );
        }
        VmCommands::Get { vm_id } => {
            let resp = client.post("/v1/vms", &json!({ "vm_id": vm_id })).await?;
            output::print_value(&resp, format);
        }
        VmCommands::Create {
            name,
            cpu,
            memory,
            image,
            network,
        } => {
            let mut body = json!({ "name": name });
            if let Some(c) = cpu {
                body["cpu"] = json!(c);
            }
            if let Some(m) = memory {
                body["memory"] = json!(m);
            }
            if let Some(i) = image {
                body["image"] = json!(i);
            }
            if let Some(n) = network {
                body["network"] = json!(n);
            }
            let resp = client.post("/v1/vms/create", &body).await?;
            println!("VM created successfully.");
            output::print_value(&resp, format);
        }
        VmCommands::Start { vm_id } => {
            let body = json!({ "vm_id": vm_id, "action": "start", "force": false });
            client.post("/v1/vms/mutate", &body).await?;
            println!("VM {vm_id} starting.");
        }
        VmCommands::Stop { vm_id } => {
            let body = json!({ "vm_id": vm_id, "action": "stop", "force": false });
            client.post("/v1/vms/mutate", &body).await?;
            println!("VM {vm_id} stopping.");
        }
        VmCommands::Reboot { vm_id } => {
            let body = json!({ "vm_id": vm_id, "action": "reboot", "force": false });
            client.post("/v1/vms/mutate", &body).await?;
            println!("VM {vm_id} rebooting.");
        }
        VmCommands::Delete { vm_id } => {
            let body = json!({ "vm_id": vm_id });
            client.post("/v1/vms/delete", &body).await?;
            println!("VM {vm_id} deleted.");
        }
        VmCommands::Migrate { vm_id, to } => {
            let body = json!({
                "vm_id": vm_id,
                "action": "migrate",
                "target_node_id": to,
            });
            client.post("/v1/vms/mutate", &body).await?;
            println!("VM {vm_id} migrating to node {to}.");
        }
        VmCommands::Resize { vm_id, cpu, memory } => {
            let mut body = json!({ "vm_id": vm_id });
            if let Some(c) = cpu {
                body["cpu"] = json!(c);
            }
            if let Some(m) = memory {
                body["memory"] = json!(m);
            }
            client.post("/v1/vms/resize", &body).await?;
            println!("VM {vm_id} resized.");
        }
    }
    Ok(())
}
