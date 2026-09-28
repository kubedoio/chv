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
                .get("items")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            output::print_list(
                &items,
                &["vm_id", "name", "power_state", "node_id", "cpu", "memory"],
                format,
            );
        }
        VmCommands::Get { vm_id } => {
            let resp = client
                .post("/v1/vms/get", &json!({ "vm_id": vm_id }))
                .await?;
            output::print_value(&resp, format);
        }
        VmCommands::Create {
            name,
            cpu,
            memory,
            image,
            network,
        } => {
            // Field names follow the BFF create_vm contract: cpu_count,
            // memory_bytes, image_ref, network_id (the legacy "cpu"/"memory"/
            // "image"/"network" spellings are not read by the handler and
            // were silently ignored, always creating a default-spec VM).
            let mut body = json!({ "name": name });
            if let Some(c) = cpu {
                body["cpu_count"] = json!(c);
            }
            if let Some(m) = memory {
                body["memory_bytes"] = json!(parse_size_bytes(&m)?);
            }
            if let Some(i) = image {
                body["image_ref"] = json!(i);
            }
            if let Some(n) = network {
                body["network_id"] = json!(n);
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
            // The BFF mutation contract accepts "restart" (mapped to the
            // reboot_vm RPC); "reboot" is rejected as an invalid action.
            let body = json!({ "vm_id": vm_id, "action": "restart", "force": false });
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
            // Same BFF contract as create: cpu_count / memory_bytes (the
            // handler rejects the legacy "cpu"/"memory" spellings with a
            // 400 before this fix).
            let mut body = json!({ "vm_id": vm_id });
            if let Some(c) = cpu {
                body["cpu_count"] = json!(c);
            }
            if let Some(m) = memory {
                body["memory_bytes"] = json!(parse_size_bytes(&m)?);
            }
            client.post("/v1/vms/resize", &body).await?;
            println!("VM {vm_id} resized.");
        }
    }
    Ok(())
}

/// Parse a human size ("512M", "2G", "1.5GiB", "4096") into bytes.
///
/// Suffixes are binary (K/M/G/T = KiB/MiB/GiB/TiB, base 1024), matching the
/// BFF's MiB-based `memory_mb` handling and display formatting. A bare
/// number is bytes.
pub(crate) fn parse_size_bytes(input: &str) -> Result<i64, CliError> {
    let s = input.trim();
    if s.is_empty() {
        return Err(CliError::Parse("empty size string".into()));
    }
    let (num_part, suffix) =
        s.split_at(s.find(|c: char| c.is_ascii_alphabetic()).unwrap_or(s.len()));
    let num: f64 = num_part
        .trim()
        .parse()
        .map_err(|_| CliError::Parse(format!("invalid size: {input:?}")))?;
    if num < 0.0 {
        return Err(CliError::Parse(format!("negative size: {input:?}")));
    }
    let mult: i64 = match suffix
        .trim_end_matches("B")
        .trim_end_matches("i")
        .to_uppercase()
        .as_str()
    {
        "" => 1,
        "K" => 1024,
        "M" => 1024 * 1024,
        "G" => 1024 * 1024 * 1024,
        "T" => 1024_i64 * 1024 * 1024 * 1024,
        _ => return Err(CliError::Parse(format!("unknown size suffix: {input:?}"))),
    };
    let bytes = num * mult as f64;
    if !bytes.is_finite() || bytes > i64::MAX as f64 {
        return Err(CliError::Parse(format!("size out of range: {input:?}")));
    }
    Ok(bytes as i64)
}

#[cfg(test)]
mod tests {
    use super::parse_size_bytes;

    #[test]
    fn parses_plain_bytes() {
        assert_eq!(parse_size_bytes("4096").unwrap(), 4096);
        assert_eq!(parse_size_bytes("0").unwrap(), 0);
    }

    #[test]
    fn parses_binary_suffixes() {
        assert_eq!(parse_size_bytes("512M").unwrap(), 512 * 1024 * 1024);
        assert_eq!(parse_size_bytes("2G").unwrap(), 2 * 1024 * 1024 * 1024);
        assert_eq!(parse_size_bytes("1T").unwrap(), 1024_i64.pow(4));
        assert_eq!(parse_size_bytes("128K").unwrap(), 128 * 1024);
    }

    #[test]
    fn parses_iec_and_uppercase_spellings() {
        assert_eq!(parse_size_bytes("512MiB").unwrap(), 512 * 1024 * 1024);
        assert_eq!(parse_size_bytes("512MB").unwrap(), 512 * 1024 * 1024);
        assert_eq!(parse_size_bytes("1g").unwrap(), 1024 * 1024 * 1024);
        assert_eq!(
            parse_size_bytes("1.5G").unwrap(),
            (1.5 * 1024.0 * 1024.0 * 1024.0) as i64
        );
    }

    #[test]
    fn rejects_garbage() {
        assert!(parse_size_bytes("").is_err());
        assert!(parse_size_bytes("M").is_err());
        assert!(parse_size_bytes("12X").is_err());
        assert!(parse_size_bytes("-2G").is_err());
        assert!(parse_size_bytes("abc").is_err());
    }
}
