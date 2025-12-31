// Copyright 2025 AntNS Contributors
// Licensed under GPL-3.0

//! DNS resolver and HTTP proxy server commands

use anyhow::Result;
use clap::Subcommand;

#[derive(Subcommand)]
pub enum ServerCommands {
    /// Start DNS resolver and HTTP proxy
    Start {
        /// DNS port
        #[arg(long, default_value = "5354")]
        dns_port: u16,
        /// HTTP proxy port
        #[arg(long, default_value = "80")]
        proxy_port: u16,
        /// Upstream URL template for HTTP proxy (use $ADDRESS for target)
        #[arg(long, default_value = "http://localhost:18888/$ADDRESS")]
        upstream: String,
        /// Cache TTL in minutes (0 to disable caching)
        #[arg(long, default_value = "60")]
        ttl: u64,
    },
    /// Set up DNS resolver configuration
    Setup {
        /// DNS port
        #[arg(long, default_value = "5354")]
        dns_port: u16,
    },
    /// Stop running servers
    Stop,
    /// Show server status
    Status,
}

pub async fn execute(command: ServerCommands) -> Result<()> {
    match command {
        ServerCommands::Start {
            dns_port,
            proxy_port,
            upstream,
            ttl,
        } => start_command(dns_port, proxy_port, upstream, ttl).await,
        ServerCommands::Setup { dns_port } => setup_command(dns_port).await,
        ServerCommands::Stop => stop_command().await,
        ServerCommands::Status => status_command().await,
    }
}

async fn start_command(
    dns_port: u16,
    proxy_port: u16,
    upstream: String,
    ttl_minutes: u64,
) -> Result<()> {
    use anyhow::Context;

    println!("Starting AntNS servers...");
    println!("DNS Resolver: port {}", dns_port);
    println!("HTTP Proxy: port {}", proxy_port);
    println!("Upstream: {}", upstream);
    if ttl_minutes > 0 {
        println!("Cache TTL: {} minutes", ttl_minutes);
    } else {
        println!("Cache: disabled");
    }

    // Check resolver configuration
    println!("\nChecking DNS resolver configuration...");
    let resolver_ok = antns::server::check_resolver_config(dns_port)
        .context("Failed to check resolver configuration")?;

    if !resolver_ok {
        println!("⚠️  DNS resolver not configured.");
        println!("\nWould you like to set up the DNS resolver now? (y/n)");

        let mut input = String::new();
        std::io::stdin()
            .read_line(&mut input)
            .context("Failed to read input")?;

        if input.trim().to_lowercase() == "y" {
            antns::server::setup_resolver_config(dns_port)
                .context("Failed to setup resolver configuration")?;
        } else {
            println!("\nSkipping resolver setup. You can set it up later with:");
            println!("  antns server setup");
        }
    } else {
        println!("✓ DNS resolver configuration OK");
    }

    println!("\nStarting servers...\n");

    // Start both servers concurrently
    tokio::select! {
        result = antns::server::run_dns(dns_port) => {
            eprintln!("DNS server exited: {:?}", result);
        }
        result = antns::server::run_http(proxy_port, upstream, ttl_minutes) => {
            eprintln!("HTTP proxy exited: {:?}", result);
        }
        _ = tokio::signal::ctrl_c() => {
            println!("\n\nShutting down servers...");
        }
    }

    println!("Servers stopped.");

    Ok(())
}

async fn setup_command(dns_port: u16) -> Result<()> {
    use anyhow::Context;

    println!("Setting up DNS resolver configuration...\n");

    antns::server::setup_resolver_config(dns_port)
        .context("Failed to setup resolver configuration")?;

    println!("\nSetup complete! You can now start the server with:");
    println!("  antns server start");

    Ok(())
}

async fn stop_command() -> Result<()> {
    println!("Stopping AntNS servers...");
    println!("\n⚠️  Server management not yet implemented.");
    println!("To stop the server, press Ctrl+C in the terminal where it's running.");
    Ok(())
}

async fn status_command() -> Result<()> {
    use anyhow::Context;

    println!("AntNS Server Status\n");

    // Check resolver configuration
    let dns_port = 5354u16;
    let resolver_ok = antns::server::check_resolver_config(dns_port)
        .context("Failed to check resolver configuration")?;

    if resolver_ok {
        println!("DNS Resolver Configuration: ✓ Configured");
    } else {
        println!("DNS Resolver Configuration: ✗ Not configured");
        println!("  Run 'antns server setup' to configure");
    }

    // Check if servers are running
    println!("\nServers:");
    let dns_running = check_port_in_use(dns_port);
    let proxy_port = 80u16;
    let proxy_running = check_port_in_use(proxy_port);

    if dns_running {
        println!("  DNS Resolver (port {}): ✓ Running", dns_port);
    } else {
        println!("  DNS Resolver (port {}): ✗ Not running", dns_port);
    }

    if proxy_running {
        println!("  HTTP Proxy (port {}): ✓ Running", proxy_port);
    } else {
        println!("  HTTP Proxy (port {}): ✗ Not running", proxy_port);
    }

    if !dns_running && !proxy_running {
        println!("\nStart servers with: antns server start");
    }

    Ok(())
}

/// Check if a port is in use
fn check_port_in_use(port: u16) -> bool {
    use std::net::TcpListener;

    TcpListener::bind(("127.0.0.1", port)).is_err()
}
