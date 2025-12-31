// Copyright 2025 AntNS Contributors
// Licensed under GPL-3.0

//! AntNS CLI application

use anyhow::Result;
use clap::{Parser, Subcommand};

mod cli;

#[derive(Parser)]
#[command(name = "antns")]
#[command(about = "Autonomi Name System - Decentralized DNS for the Autonomi network", long_about = None)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Enable verbose logging
    #[arg(short, long, global = true)]
    verbose: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Domain name management
    Names {
        #[command(subcommand)]
        command: cli::names::NamesCommands,
    },
    /// Domain records management
    Records {
        #[command(subcommand)]
        command: cli::records::RecordsCommands,
    },
    /// DNS resolver and HTTP proxy server
    Server {
        #[command(subcommand)]
        command: cli::server::ServerCommands,
    },
    /// Key management and backup
    Keys {
        #[command(subcommand)]
        command: cli::keys::KeysCommands,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize logging
    // Without -v: only show WARN and ERROR from antns (quiet mode, suppress autonomi internal errors)
    // With -v: show INFO, WARN, ERROR from antns, WARN from autonomi (verbose)
    // With RUST_LOG=debug: show everything (debug)
    if std::env::var("RUST_LOG").is_err() {
        use tracing_subscriber::EnvFilter;

        let filter = if cli.verbose {
            // Verbose: INFO from antns, WARN from autonomi
            EnvFilter::new("antns=info,autonomi=warn")
        } else {
            // Quiet: only WARN from antns, suppress all autonomi internal logs
            EnvFilter::new("antns=warn")
        };

        tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_target(false)
            .compact()
            .init();
    } else {
        // RUST_LOG is set, use default env filter
        tracing_subscriber::fmt()
            .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
            .with_target(true)
            .init();
    }

    // Execute command
    match cli.command {
        Commands::Names { command } => {
            cli::names::execute(command).await?;
        }
        Commands::Records { command } => {
            cli::records::execute(command).await?;
        }
        Commands::Server { command } => {
            cli::server::execute(command).await?;
        }
        Commands::Keys { command } => {
            cli::keys::execute(command).await?;
        }
    }

    Ok(())
}
