// Copyright 2025 AntNS Contributors
// Licensed under GPL-3.0

//! Key management and backup commands

use anyhow::{Context, Result};
use clap::Subcommand;

#[derive(Subcommand)]
pub enum KeysCommands {
    /// Backup all domain keys to Autonomi network
    Backup,
    /// Restore keys from network backup
    Restore,
    /// Show backup status
    Status,
}

pub async fn execute(command: KeysCommands) -> Result<()> {
    match command {
        KeysCommands::Backup => backup_command().await,
        KeysCommands::Restore => restore_command().await,
        KeysCommands::Status => status_command().await,
    }
}

async fn backup_command() -> Result<()> {
    use autonomi::Client;

    println!("Backing up domain keys to Autonomi network...\n");

    // Initialize client
    let client = Client::init()
        .await
        .context("Failed to initialize Autonomi client")?;

    // Load wallet and private key using the client's network
    let (wallet, wallet_private_key) =
        antns::wallet::load_wallet_with_private_key(&client).context("Failed to load wallet")?;

    println!("Using wallet: {}\n", wallet.address());

    // Create payment option
    let payment = autonomi::client::payment::PaymentOption::from(&wallet);

    // Perform backup
    antns::vault::backup_keys(&client, &wallet_private_key, payment).await?;

    Ok(())
}

async fn restore_command() -> Result<()> {
    use autonomi::Client;

    println!("Restoring domain keys from Autonomi network...\n");

    // Initialize client
    let client = Client::init()
        .await
        .context("Failed to initialize Autonomi client")?;

    // Load wallet and private key using the client's network
    let (wallet, wallet_private_key) =
        antns::wallet::load_wallet_with_private_key(&client).context("Failed to load wallet")?;

    println!("Using wallet: {}\n", wallet.address());

    // Perform restore
    antns::vault::restore_keys(&client, &wallet_private_key).await?;

    Ok(())
}

async fn status_command() -> Result<()> {
    use autonomi::Client;

    println!("Key Backup Status\n");

    let domains = antns::storage::list_local_domains()?;
    println!("Local domains: {}", domains.len());

    if !domains.is_empty() {
        for domain in &domains {
            println!("  • {}", domain);
        }
    }

    // Check vault status
    println!("\nChecking vault backup...");

    let client = Client::init()
        .await
        .context("Failed to initialize Autonomi client")?;

    let (wallet, wallet_private_key) =
        antns::wallet::load_wallet_with_private_key(&client).context("Failed to load wallet")?;

    println!("Using wallet: {}", wallet.address());

    match antns::vault::restore_keys(&client, &wallet_private_key).await {
        Ok(_) => {
            println!("✓ Vault backup exists (not restored, just checked)");
        }
        Err(_) => {
            println!("✗ No vault backup found");
            println!("  Run 'antns keys backup' to create one");
        }
    }

    Ok(())
}
