use clap::{Parser, Subcommand};

#[allow(dead_code)]
mod client;
mod commands;
mod config;
mod output;

use commands::{auth, backup, image, network, node, task, user, vm, volume};
use output::OutputFormat;

#[derive(Parser)]
#[command(
    name = "chvctl",
    version,
    about = "CLI for managing the CHV hypervisor platform"
)]
struct Cli {
    /// BFF server URL (default: http://localhost:8080 or from config)
    #[arg(long, global = true)]
    server: Option<String>,

    /// Auth token (overrides stored credential)
    #[arg(long, global = true)]
    token: Option<String>,

    /// Output format: table (default), json, yaml
    #[arg(long, global = true, default_value = "table")]
    output: OutputFormat,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Authenticate and store token
    Login(auth::LoginArgs),
    /// Manage virtual machines
    Vm {
        #[command(subcommand)]
        command: vm::VmCommands,
    },
    /// Manage compute nodes
    Node {
        #[command(subcommand)]
        command: node::NodeCommands,
    },
    /// Manage disk images
    Image {
        #[command(subcommand)]
        command: image::ImageCommands,
    },
    /// Manage storage volumes
    Volume {
        #[command(subcommand)]
        command: volume::VolumeCommands,
    },
    /// Manage networks
    Network {
        #[command(subcommand)]
        command: network::NetworkCommands,
    },
    /// Manage tasks/operations
    Task {
        #[command(subcommand)]
        command: task::TaskCommands,
    },
    /// Manage backups
    Backup {
        #[command(subcommand)]
        command: backup::BackupCommands,
    },
    /// Manage users (admin only)
    User {
        #[command(subcommand)]
        command: user::UserCommands,
    },
    /// Show version info
    Version,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    let cfg = config::load();

    let server_url = cli
        .server
        .or(cfg.server_url.clone())
        .unwrap_or_else(|| "http://localhost:8080".to_string());

    let token = cli.token.or_else(config::load_credentials);

    let client = client::BffClient::new(server_url, token);

    let result = match cli.command {
        Commands::Login(args) => auth::execute(&client, args).await,
        Commands::Vm { command } => vm::execute(&client, command, &cli.output).await,
        Commands::Node { command } => node::execute(&client, command, &cli.output).await,
        Commands::Image { command } => image::execute(&client, command, &cli.output).await,
        Commands::Volume { command } => volume::execute(&client, command, &cli.output).await,
        Commands::Network { command } => network::execute(&client, command, &cli.output).await,
        Commands::Task { command } => task::execute(&client, command, &cli.output).await,
        Commands::Backup { command } => backup::execute(&client, command, &cli.output).await,
        Commands::User { command } => user::execute(&client, command, &cli.output).await,
        Commands::Version => {
            println!("chvctl {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
    };

    if let Err(e) = result {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}
