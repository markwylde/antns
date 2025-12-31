// Copyright 2025 AntNS Contributors
// Licensed under GPL-3.0

//! Domain records management commands

use anyhow::{Context, Result};
use autonomi::Client;
use clap::Subcommand;

#[derive(Subcommand)]
pub enum RecordsCommands {
    /// List all records for a domain
    List {
        /// Domain name
        #[arg(long)]
        name: String,
    },
    /// Add a new record to a domain
    Add {
        /// Domain name
        #[arg(long)]
        name: String,
        /// Record type (TEXT or ANT)
        record_type: String,
        /// Record name (use . for root)
        record_name: String,
        /// Record value
        value: String,
    },
    /// Delete a record by index
    Delete {
        /// Domain name
        #[arg(long)]
        name: String,
        /// Record index to delete
        index: usize,
    },
    /// Update a record by index
    Update {
        /// Domain name
        #[arg(long)]
        name: String,
        /// Record index to update
        index: usize,
        /// New record type (TEXT or ANT)
        record_type: String,
        /// New record name (use . for root)
        record_name: String,
        /// New record value
        value: String,
    },
}

pub async fn execute(command: RecordsCommands) -> Result<()> {
    match command {
        RecordsCommands::List { name } => list_command(name).await,
        RecordsCommands::Add {
            name,
            record_type,
            record_name,
            value,
        } => add_command(name, record_type, record_name, value).await,
        RecordsCommands::Delete { name, index } => delete_command(name, index).await,
        RecordsCommands::Update {
            name,
            index,
            record_type,
            record_name,
            value,
        } => update_command(name, index, record_type, record_name, value).await,
    }
}

async fn list_command(domain: String) -> Result<()> {
    println!("Listing records for domain: {}\n", domain);

    let client = Client::init()
        .await
        .context("Failed to initialize Autonomi client")?;

    // Fetch current records
    match antns::lookup_domain_records(&client, &domain).await {
        Ok(records) => {
            if records.is_empty() {
                println!("No records found for domain: {}", domain);
                println!(
                    "\nUse 'antns records add --name {} [type] [name] [value]' to add records.",
                    domain
                );
            } else {
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
                println!("\n✗ Domain not found: {}", domain);
                println!("The domain may not be registered, or the network may be unreachable.");
                Ok(())
            } else {
                Err(e).context("Failed to list domain records")
            }
        }
    }
}

async fn add_command(
    domain: String,
    record_type: String,
    record_name: String,
    value: String,
) -> Result<()> {
    println!("Adding record to domain: {}", domain);
    println!(
        "Type: {}, Name: {}, Value: {}",
        record_type, record_name, value
    );

    // Validate record type
    if record_type != "TEXT" && record_type != "ANT" {
        anyhow::bail!("Invalid record type. Must be TEXT or ANT");
    }

    // Load keypair
    let keypair = antns::crypto::load_keypair(&domain)
        .context("Failed to load domain keypair. Do you own this domain?")?;

    let client = Client::init()
        .await
        .context("Failed to initialize Autonomi client")?;

    // Load wallet using the client's network
    let wallet =
        antns::wallet::load_wallet_from_client(&client).context("Failed to load wallet")?;

    println!("Using wallet: {}", wallet.address());

    // Create payment option
    let payment = autonomi::client::payment::PaymentOption::from(&wallet);

    // Create record
    let record = antns::register::DnsRecord {
        record_type,
        name: record_name,
        value,
    };

    // Add record
    let cost = antns::add_domain_record(&client, &domain, record, &keypair.signing_key, payment)
        .await
        .context("Failed to add record")?;

    println!("\n✓ Record added successfully!");
    println!("Cost: {} AttoTokens", cost);

    Ok(())
}

async fn delete_command(domain: String, index: usize) -> Result<()> {
    println!("Deleting record {} from domain: {}", index, domain);

    // Load keypair
    let keypair = antns::crypto::load_keypair(&domain)
        .context("Failed to load domain keypair. Do you own this domain?")?;

    let client = Client::init()
        .await
        .context("Failed to initialize Autonomi client")?;

    // Load wallet using the client's network
    let wallet =
        antns::wallet::load_wallet_from_client(&client).context("Failed to load wallet")?;

    println!("Using wallet: {}", wallet.address());

    // Create payment option
    let payment = autonomi::client::payment::PaymentOption::from(&wallet);

    // Delete record
    let cost = antns::delete_domain_record(&client, &domain, index, &keypair.signing_key, payment)
        .await
        .context("Failed to delete record")?;

    println!("\n✓ Record deleted successfully!");
    println!("Cost: {} AttoTokens", cost);

    Ok(())
}

async fn update_command(
    domain: String,
    index: usize,
    record_type: String,
    record_name: String,
    value: String,
) -> Result<()> {
    println!("Updating record {} for domain: {}", index, domain);
    println!(
        "New Type: {}, Name: {}, Value: {}",
        record_type, record_name, value
    );

    // Validate record type
    if record_type != "TEXT" && record_type != "ANT" {
        anyhow::bail!("Invalid record type. Must be TEXT or ANT");
    }

    // Load keypair
    let keypair = antns::crypto::load_keypair(&domain)
        .context("Failed to load domain keypair. Do you own this domain?")?;

    let client = Client::init()
        .await
        .context("Failed to initialize Autonomi client")?;

    // Load wallet using the client's network
    let wallet =
        antns::wallet::load_wallet_from_client(&client).context("Failed to load wallet")?;

    println!("Using wallet: {}", wallet.address());

    // Create payment option
    let payment = autonomi::client::payment::PaymentOption::from(&wallet);

    // Create record
    let record = antns::register::DnsRecord {
        record_type,
        name: record_name,
        value,
    };

    // Update record
    let cost = antns::update_domain_record(
        &client,
        &domain,
        index,
        record,
        &keypair.signing_key,
        payment,
    )
    .await
    .context("Failed to update record")?;

    println!("\n✓ Record updated successfully!");
    println!("Cost: {} AttoTokens", cost);

    Ok(())
}
