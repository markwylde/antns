// Copyright 2025 AntNS Contributors
// Licensed under GPL-3.0

//! Vault backup and restore operations for domain keypairs

use autonomi::Client;
use autonomi::client::payment::PaymentOption;
use autonomi::client::vault::{vault_derive_key, vault_content_type_from_app_name};
use anyhow::{Context, Result};
use bytes::Bytes;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

const ANTNS_VAULT_CONTENT_TYPE: &str = "antns_keys";

/// Backup structure for domain keypairs
#[derive(Debug, Serialize, Deserialize)]
struct KeysBackup {
    /// Map of domain name to private key hex
    keys: HashMap<String, String>,
    /// Backup timestamp
    created_at: String,
    /// Version for future compatibility
    version: u32,
}

/// Backup all domain keypairs to the vault
pub async fn backup_keys(
    client: &Client,
    wallet_private_key: &str,
    payment: PaymentOption,
) -> Result<()> {
    println!("Collecting domain keypairs...");

    // Get all domain keys
    let keys_dir = crate::storage::local::get_domain_keys_dir()?;

    if !keys_dir.exists() {
        anyhow::bail!("No domain keys directory found. Have you registered any domains?");
    }

    let mut keys_map = HashMap::new();

    // Read all domain-key-*.txt files
    for entry in std::fs::read_dir(&keys_dir)
        .context("Failed to read keys directory")?
    {
        let entry = entry.context("Failed to read directory entry")?;
        let path = entry.path();

        if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
            // Match domain-key-{domain}.txt pattern
            if file_name.starts_with("domain-key-") && file_name.ends_with(".txt") {
                // Extract domain name
                let domain = file_name
                    .strip_prefix("domain-key-")
                    .and_then(|s| s.strip_suffix(".txt"))
                    .context("Invalid key file name format")?;

                // Read key hex
                let key_hex = std::fs::read_to_string(&path)
                    .with_context(|| format!("Failed to read key file for {}", domain))?;

                keys_map.insert(domain.to_string(), key_hex.trim().to_string());
                println!("  Found key for: {}", domain);
            }
        }
    }

    if keys_map.is_empty() {
        anyhow::bail!("No domain keys found to backup");
    }

    println!("\nBacking up {} domain key(s)...", keys_map.len());

    // Create backup structure
    let backup = KeysBackup {
        keys: keys_map,
        created_at: chrono::Utc::now().to_rfc3339(),
        version: 1,
    };

    // Serialize to JSON
    let backup_json = serde_json::to_string_pretty(&backup)
        .context("Failed to serialize backup")?;
    let backup_bytes = Bytes::from(backup_json);

    // Derive vault key from wallet private key
    let vault_key = vault_derive_key(wallet_private_key)
        .context("Failed to derive vault key from wallet")?;

    // Get content type for antns
    let content_type = vault_content_type_from_app_name(ANTNS_VAULT_CONTENT_TYPE);

    // Store in vault
    let cost = client
        .vault_put(backup_bytes, payment, &vault_key, content_type)
        .await
        .context("Failed to store backup in vault")?;

    println!("\n✓ Backup stored in vault successfully!");
    println!("Cost: {} AttoTokens", cost);
    println!("\nYour domain keys are now backed up to the Autonomi network.");
    println!("Run 'ant vault sync' to ensure your vault is synced.");

    Ok(())
}

/// Restore domain keypairs from the vault
pub async fn restore_keys(
    client: &Client,
    wallet_private_key: &str,
) -> Result<()> {
    println!("Fetching backup from vault...");

    // Derive vault key from wallet private key
    let vault_key = vault_derive_key(wallet_private_key)
        .context("Failed to derive vault key from wallet")?;

    // Get from vault
    let (backup_bytes, _content_type) = client
        .vault_get(&vault_key)
        .await
        .context("Failed to retrieve backup from vault. Have you created a backup yet?")?;

    // Deserialize backup
    let backup_json = String::from_utf8(backup_bytes.to_vec())
        .context("Backup data is not valid UTF-8")?;

    let backup: KeysBackup = serde_json::from_str(&backup_json)
        .context("Failed to parse backup data")?;

    println!("Found backup from: {}", backup.created_at);
    println!("Restoring {} domain key(s)...", backup.keys.len());

    // Ensure keys directory exists
    let keys_dir = crate::storage::local::get_domain_keys_dir()?;
    std::fs::create_dir_all(&keys_dir)
        .context("Failed to create keys directory")?;

    // Restore each keypair
    for (domain, key_hex) in backup.keys.iter() {
        // Decode key
        let key_bytes = hex::decode(key_hex.trim())
            .with_context(|| format!("Invalid hex in backup for domain: {}", domain))?;

        // Create keypair
        let keypair = crate::crypto::DomainKeypair::from_bytes(&key_bytes)
            .with_context(|| format!("Failed to create keypair for domain: {}", domain))?;

        // Save to local storage
        crate::crypto::save_keypair(domain, &keypair)
            .with_context(|| format!("Failed to save keypair for domain: {}", domain))?;

        println!("  Restored: {}", domain);
    }

    println!("\n✓ Successfully restored {} domain key(s)!", backup.keys.len());
    println!("\nYou can now manage these domains:");
    for domain in backup.keys.keys() {
        println!("  • {}", domain);
    }

    Ok(())
}
