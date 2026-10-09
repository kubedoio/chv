//! `netbox-sim` — standalone launcher for the CHV NetBox simulator
//! (ADR-024, issue kubedoio/chv#586).
//!
//! A dev tool for the `make netbox-demo` harness and interactive use;
//! never built by `cargo build --workspace` (the target is gated
//! behind the `bin` cargo feature) and never release-packaged.
//!
//! ```text
//! netbox-sim [--listen 127.0.0.1:8080] --token <TOKEN>... [--seed-file <PATH>]
//! ```
//!
//! The seed file is the same JSON format as `POST /__seed` (one key
//! per collection: `devices`, `virtual_machines`, `interfaces`,
//! `prefixes`, `vlans`, `ip_addresses`; entries are the write-body
//! form plus optional `id`/`created`/`last_updated`). After startup
//! the base URL is printed to stdout — everything else goes to
//! stderr, so the URL is trivially consumable by scripts:
//!
//! ```sh
//! NETBOX_URL=$(netbox-sim --listen 127.0.0.1:8080 --token demo)
//! ```

use std::net::SocketAddr;
use std::path::PathBuf;
use std::process::ExitCode;

use chv_netbox_sim::{NetboxSim, NetboxSimConfig, SeedPayload};

const USAGE: &str = "\
netbox-sim — stateful NetBox 4.x simulator (dev tool, ADR-024)

USAGE:
    netbox-sim [OPTIONS]

OPTIONS:
    -l, --listen <ADDR>     Bind address (default: 127.0.0.1:8080).
                            Loopback only (127.0.0.0/8 or [::1]): the
                            `__` control plane is unauthenticated, so the
                            simulator must never be exposed to a network.
    -t, --token <TOKEN>     Accepted API token (repeatable, at least one required)
    -f, --seed-file <PATH>  JSON seed loaded at startup (same format as POST /__seed)
    -h, --help              Print this help
";

struct Args {
    listen: SocketAddr,
    tokens: Vec<String>,
    seed_file: Option<PathBuf>,
}

/// The `__`-prefixed control plane (`/__seed`, `/__state`,
/// `/__reset`, `/__faults`) is unauthenticated, so the simulator
/// must never be reachable from a network: refuse any non-loopback
/// bind address — IPv4 `127.0.0.0/8` or IPv6 `::1` — with a clear
/// error instead of silently exposing it.
fn validate_listen(addr: SocketAddr) -> Result<(), String> {
    if addr.ip().is_loopback() {
        Ok(())
    } else {
        Err(format!(
            "refusing to bind non-loopback address {addr}: netbox-sim is a dev tool and its \
             `__` control plane (/__seed, /__state, /__reset, /__faults) is unauthenticated; \
             bind a loopback address instead, e.g. 127.0.0.1:8080 or [::1]:8080"
        ))
    }
}

fn parse_args() -> Result<Args, String> {
    let mut listen: SocketAddr = "127.0.0.1:8080"
        .parse()
        .expect("static bind address parses");
    let mut tokens = Vec::new();
    let mut seed_file = None;
    let mut arguments = std::env::args().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "-l" | "--listen" => {
                let value = arguments.next().ok_or("--listen requires a value")?;
                listen = value
                    .parse()
                    .map_err(|_| format!("invalid --listen address: {value}"))?;
            }
            "-t" | "--token" => {
                let value = arguments.next().ok_or("--token requires a value")?;
                tokens.push(value);
            }
            "-f" | "--seed-file" => {
                let value = arguments.next().ok_or("--seed-file requires a value")?;
                seed_file = Some(PathBuf::from(value));
            }
            "-h" | "--help" => {
                print!("{USAGE}");
                std::process::exit(0);
            }
            other => return Err(format!("unknown argument: {other}")),
        }
    }
    if tokens.is_empty() {
        return Err("at least one --token is required".to_string());
    }
    validate_listen(listen)?;
    Ok(Args {
        listen,
        tokens,
        seed_file,
    })
}

#[tokio::main]
async fn main() -> ExitCode {
    let args = match parse_args() {
        Ok(args) => args,
        Err(error) => {
            eprintln!("netbox-sim: {error}\n\n{USAGE}");
            return ExitCode::from(2);
        }
    };

    let config = NetboxSimConfig::with_tokens(args.tokens);
    let sim = match NetboxSim::start_on(args.listen, config).await {
        Ok(sim) => sim,
        Err(error) => {
            eprintln!("netbox-sim: failed to bind {}: {error}", args.listen);
            return ExitCode::FAILURE;
        }
    };

    if let Some(path) = &args.seed_file {
        let raw = match std::fs::read_to_string(path) {
            Ok(raw) => raw,
            Err(error) => {
                eprintln!(
                    "netbox-sim: cannot read seed file {}: {error}",
                    path.display()
                );
                return ExitCode::FAILURE;
            }
        };
        let payload: SeedPayload = match serde_json::from_str(&raw) {
            Ok(payload) => payload,
            Err(error) => {
                eprintln!("netbox-sim: invalid seed file {}: {error}", path.display());
                return ExitCode::FAILURE;
            }
        };
        let counts = match sim.seed(&payload) {
            Ok(counts) => counts,
            Err(error) => {
                eprintln!("netbox-sim: seed failed: {error}");
                return ExitCode::FAILURE;
            }
        };
        let total: usize = counts.values().sum();
        eprintln!("netbox-sim: seeded {total} objects from {}", path.display());
    }

    // The one stdout line: the base URL, for scripts to consume.
    println!("{}", sim.base_url());

    if let Err(error) = tokio::signal::ctrl_c().await {
        eprintln!("netbox-sim: failed to wait for ctrl-c: {error}");
    }
    sim.shutdown().await;
    ExitCode::SUCCESS
}

#[cfg(test)]
mod tests {
    use super::validate_listen;
    use std::net::SocketAddr;

    #[test]
    fn loopback_binds_are_accepted() {
        // Both IPv4 (all of 127.0.0.0/8) and IPv6 (::1) loopback.
        for addr in ["127.0.0.1:8080", "127.9.9.9:1", "[::1]:8080"] {
            let addr: SocketAddr = addr.parse().expect("parses");
            validate_listen(addr).unwrap_or_else(|error| panic!("loopback refused: {error}"));
        }
    }

    #[test]
    fn non_loopback_binds_are_refused() {
        for addr in [
            "0.0.0.0:8080",
            "192.168.1.10:8080",
            "10.0.0.1:8080",
            "[::]:8080",
            "[2001:db8::1]:8080",
        ] {
            let addr: SocketAddr = addr.parse().expect("parses");
            let error = validate_listen(addr).expect_err("non-loopback must be refused");
            // The error must explain why and suggest an alternative.
            assert!(error.contains("unauthenticated"), "{error}");
            assert!(error.contains("127.0.0.1"), "{error}");
        }
    }
}
