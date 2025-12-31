// Copyright 2025 AntNS Contributors
// Licensed under GPL-3.0

//! Domain name management commands

use anyhow::{Context, Result};
use autonomi::Client;
use clap::Subcommand;

#[derive(Subcommand)]
pub enum NamesCommands {
    /// Register a new domain
    Register {
        /// Domain name (e.g., mydomain.ant)
        domain: String,
    },
    /// Look up a domain's records
    Lookup {
        /// Domain name to look up
        domain: String,
    },
    /// View domain history
    History {
        /// Domain name
        domain: String,
    },
    /// List locally owned domains
    List,
    /// Export domain private key
    Export {
        /// Domain name to export
        domain: String,
    },
    /// Import domain private key
    Import {
        /// Domain name
        domain: String,
        /// Private key (hex)
        #[arg(long)]
        key: String,
    },
}

pub async fn execute(command: NamesCommands) -> Result<()> {
    match command {
        NamesCommands::Register { domain } => register_command(domain).await,
        NamesCommands::Lookup { domain } => lookup_command(domain).await,
        NamesCommands::History { domain } => history_command(domain).await,
        NamesCommands::List => list_command().await,
        NamesCommands::Export { domain } => export_command(domain).await,
        NamesCommands::Import { domain, key } => import_command(domain, key).await,
    }
}

async fn register_command(domain: String) -> Result<()> {
    println!("Registering domain: {}", domain);

    // Initialize client first (to determine network)
    let client = Client::init()
        .await
        .context("Failed to initialize Autonomi client")?;

    // Load wallet using the client's network
    let wallet =
        antns::wallet::load_wallet_from_client(&client).context("Failed to load wallet")?;

    println!("Using wallet: {}", wallet.address());

    // Create payment option
    let payment = autonomi::client::payment::PaymentOption::from(&wallet);

    // Register domain
    let registration = antns::register_domain(&client, &domain, payment)
        .await
        .context("Failed to register domain")?;

    // Extract keypair components before moving
    let verifying_key = registration.owner_key.verifying_key();
    let signing_key = registration.owner_key;

    // Save keypair locally
    antns::crypto::save_keypair(
        &domain,
        &antns::crypto::DomainKeypair {
            signing_key,
            verifying_key,
        },
    )
    .context("Failed to save keypair")?;

    println!("\n✓ Domain registered successfully!");
    println!("Register address: {}", registration.register_address);
    println!("Total cost: {} AttoTokens", registration.total_cost);
    println!("\nPrivate key saved to local storage.");
    println!(
        "\nUse 'antns records add --name {} [type] [name] [value]' to add records.",
        domain
    );

    Ok(())
}

async fn lookup_command(domain: String) -> Result<()> {
    println!("Looking up domain: {}\n", domain);

    let client = Client::init()
        .await
        .context("Failed to initialize Autonomi client")?;

    match antns::lookup_domain_records(&client, &domain).await {
        Ok(records) => {
            if records.is_empty() {
                println!("Domain '{}' is registered but has no records.", domain);
                println!(
                    "\nUse 'antns records add --name {} [type] [name] [value]' to add records.",
                    domain
                );
            } else {
                println!("Records for domain '{}':\n", domain);
                for (i, record) in records.iter().enumerate() {
                    println!(
                        "[{}] {} {} {}",
                        i, record.record_type, record.name, record.value
                    );
                }
            }
            Ok(())
        }
        Err(e) => {
            let err_msg = format!("{:#}", e);
            if err_msg.contains("Timeout")
                || err_msg.contains("not found")
                || err_msg.contains("Register not found")
            {
                println!("✗ Domain not found: {}", domain);
                println!("The domain may not be registered, or the network may be unreachable.");
                Ok(()) // Don't error out, just inform the user
            } else {
                Err(e).context("Domain lookup failed")
            }
        }
    }
}

async fn history_command(domain: String) -> Result<()> {
    println!("Fetching history for domain: {}\n", domain);

    let client = Client::init()
        .await
        .context("Failed to initialize Autonomi client")?;

    let history = antns::get_domain_history(&client, &domain)
        .await
        .context("Failed to fetch domain history")?;

    for (i, entry) in history.iter().enumerate() {
        match entry {
            antns::register::HistoryEntry::Owner {
                public_key,
                chunk_address,
            } => {
                println!("Entry {} (Owner):", i + 1);
                println!("  Public Key: {}", public_key);
                println!("  Chunk: {}", chunk_address);
            }
            antns::register::HistoryEntry::Records {
                chunk_address,
                records,
                signature: _,
                is_valid,
            } => {
                let status = if *is_valid {
                    "✓ Valid"
                } else {
                    "✗ Invalid"
                };
                println!("Entry {} ({}):", i + 1, status);
                println!("  Chunk: {}", chunk_address);

                if let Some(recs) = records {
                    for rec in recs {
                        println!("  {} {}: {}", rec.record_type, rec.name, rec.value);
                    }
                }

                if !is_valid {
                    println!("  Reason: Invalid signature (spam)");
                }
            }
        }
        println!();
    }

    // Calculate stats
    let stats = antns::register::history::calculate_history_stats(&history);
    println!("Statistics:");
    println!("  Total entries: {}", stats.total_entries);
    println!("  Valid entries: {}", stats.valid_entries);
    println!("  Spam entries: {}", stats.spam_entries);
    println!("  Corrupted entries: {}", stats.invalid_entries);

    Ok(())
}

async fn list_command() -> Result<()> {
    println!("Locally owned domains:\n");

    let domains = antns::storage::list_local_domains().context("Failed to list local domains")?;

    if domains.is_empty() {
        println!("No domains found.");
        println!("Register a domain with: antns names register <domain>");
    } else {
        for domain in domains {
            println!("  • {}", domain);
        }
    }

    Ok(())
}

async fn export_command(domain: String) -> Result<()> {
    println!("Exporting private key for domain: {}\n", domain);

    let keypair = antns::crypto::load_keypair(&domain).context("Failed to load domain keypair")?;

    println!("PRIVATE KEY (keep this secret!):");
    println!("{}", hex::encode(keypair.to_bytes()));
    println!("\nPublic Key:");
    println!("{}", keypair.public_key_hex());

    println!("\n⚠️  WARNING: Anyone with this private key can update your domain!");
    println!("Store it securely and never share it.");

    Ok(())
}

async fn import_command(domain: String, key: String) -> Result<()> {
    println!("Importing private key for domain: {}", domain);

    let key_bytes = hex::decode(&key).context("Invalid hex in private key")?;

    let keypair =
        antns::crypto::DomainKeypair::from_bytes(&key_bytes).context("Invalid private key")?;

    antns::crypto::save_keypair(&domain, &keypair).context("Failed to save keypair")?;

    println!("\n✓ Private key imported successfully!");
    println!("Public Key: {}", keypair.public_key_hex());

    Ok(())
}
