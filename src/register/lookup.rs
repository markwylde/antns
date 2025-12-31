// Copyright 2025 AntNS Contributors
// Licensed under GPL-3.0

//! Domain lookup and resolution operations

use autonomi::Client;
use autonomi::data::DataAddress;
use ed25519_dalek::VerifyingKey;
use xor_name::XorName;
use anyhow::{Context, Result};
use crate::crypto::verify_records;
use crate::register::{DomainOwnerDocument, DomainRecordsDocument, DomainResolution};
use crate::register::get_register_address_for_domain;

/// Look up a domain and return its current target address
///
/// # Arguments
/// * `client` - Autonomi client instance
/// * `domain` - Domain name to look up
///
/// # Returns
/// Domain resolution with target address and owner public key
pub async fn lookup_domain(
    client: &Client,
    domain: &str,
) -> Result<DomainResolution> {
    // Step 1: Get register address (deterministic from domain name)
    let register_addr = get_register_address_for_domain(domain)
        .context("Failed to derive register address")?;

    tracing::debug!("Looking up domain '{}' at register: {}", domain, register_addr);

    // Step 2: Fetch register history (all chunk addresses)
    let mut history = client
        .register_history(&register_addr);

    // Step 3: Download first entry (owner document)
    let owner_chunk_addr = history
        .next()
        .await
        .context("Failed to get first history entry")?
        .ok_or_else(|| anyhow::anyhow!("Register not found for domain: {}", domain))?;

    let owner_data_addr = DataAddress::new(XorName(owner_chunk_addr));
    let owner_data = client.data_get_public(&owner_data_addr)
        .await
        .context("Failed to download owner document")?;

    let owner_doc: DomainOwnerDocument = serde_json::from_slice(&owner_data)
        .context("Failed to parse owner document")?;

    tracing::debug!("Owner public key: {}", owner_doc.public_key);

    // Parse owner's Ed25519 public key
    let owner_pubkey_bytes = hex::decode(&owner_doc.public_key)
        .context("Invalid hex in owner public key")?;
    let owner_pubkey = VerifyingKey::from_bytes(
        owner_pubkey_bytes.as_slice().try_into()
            .context("Invalid owner public key length")?
    ).context("Invalid Ed25519 public key")?;

    // Step 4: Process remaining entries (records), verify signatures
    let mut last_valid_target: Option<String> = None;
    let mut valid_count = 0;
    let mut invalid_count = 0;

    while let Some(chunk_addr) = history.next().await? {
        let data_addr = DataAddress::new(XorName(chunk_addr));

        // Download data
        let data_bytes = match client.data_get_public(&data_addr).await {
            Ok(d) => d,
            Err(e) => {
                tracing::warn!("Failed to download data {}: {}", hex::encode(chunk_addr), e);
                invalid_count += 1;
                continue; // Skip corrupted entries
            }
        };

        // Parse records document
        let records_doc: DomainRecordsDocument = match serde_json::from_slice(&data_bytes) {
            Ok(d) => d,
            Err(e) => {
                tracing::warn!("Failed to parse data as records document: {}", e);
                invalid_count += 1;
                continue; // Skip invalid JSON
            }
        };

        // Verify signature
        if verify_records(&records_doc.records, &records_doc.signature, &owner_pubkey) {
            // Valid signature - extract target
            if let Some(record) = records_doc.records.iter()
                .find(|r| r.record_type.eq_ignore_ascii_case("ant") && r.name == ".")
            {
                last_valid_target = Some(record.value.clone());
                valid_count += 1;
                tracing::debug!("Valid record found: {}", record.value);
            }
        } else {
            // Invalid signature - spam entry, ignore
            tracing::debug!("Invalid signature on chunk {}, ignoring", hex::encode(chunk_addr));
            invalid_count += 1;
        }
    }

    tracing::info!(
        "Domain lookup complete: {} valid entries, {} invalid/spam entries",
        valid_count,
        invalid_count
    );

    // Step 5: Return last valid target
    let target = last_valid_target
        .ok_or_else(|| anyhow::anyhow!("No valid DNS records found for domain: {}", domain))?;

    Ok(DomainResolution {
        domain: domain.to_string(),
        target,
        owner_public_key: owner_doc.public_key,
    })
}

/// Look up all current records for a domain
///
/// Returns the latest valid records (verified signature)
pub async fn lookup_domain_records(
    client: &Client,
    domain: &str,
) -> Result<Vec<crate::register::DnsRecord>> {
    use crate::register::DnsRecord;

    // Step 1: Get register address
    let register_addr = get_register_address_for_domain(domain)
        .context("Failed to derive register address")?;

    tracing::debug!("Looking up records for domain '{}' at register: {}", domain, register_addr);

    // Step 2: Fetch register history
    let mut history = client
        .register_history(&register_addr);

    // Step 3: Download first entry (owner document)
    let owner_chunk_addr = history
        .next()
        .await
        .context("Failed to get first history entry")?
        .ok_or_else(|| anyhow::anyhow!("Register not found for domain: {}", domain))?;

    let owner_data_addr = DataAddress::new(XorName(owner_chunk_addr));
    let owner_data = client.data_get_public(&owner_data_addr)
        .await
        .context("Failed to download owner document")?;

    let owner_doc: DomainOwnerDocument = serde_json::from_slice(&owner_data)
        .context("Failed to parse owner document")?;

    // Parse owner's Ed25519 public key
    let owner_pubkey_bytes = hex::decode(&owner_doc.public_key)
        .context("Invalid hex in owner public key")?;
    let owner_pubkey = VerifyingKey::from_bytes(
        owner_pubkey_bytes.as_slice().try_into()
            .context("Invalid owner public key length")?
    ).context("Invalid Ed25519 public key")?;

    // Step 4: Process remaining entries, find latest valid records
    let mut last_valid_records: Option<Vec<DnsRecord>> = None;

    while let Some(chunk_addr) = history.next().await? {
        let data_addr = DataAddress::new(XorName(chunk_addr));

        // Download data
        let data_bytes = match client.data_get_public(&data_addr).await {
            Ok(d) => d,
            Err(e) => {
                tracing::warn!("Failed to download data {}: {}", hex::encode(chunk_addr), e);
                continue;
            }
        };

        // Parse records document
        let records_doc: DomainRecordsDocument = match serde_json::from_slice(&data_bytes) {
            Ok(d) => d,
            Err(e) => {
                tracing::warn!("Failed to parse data as records document: {}", e);
                continue;
            }
        };

        // Verify signature
        if verify_records(&records_doc.records, &records_doc.signature, &owner_pubkey) {
            last_valid_records = Some(records_doc.records);
        } else {
            tracing::debug!("Invalid signature on chunk {}, ignoring", hex::encode(chunk_addr));
        }
    }

    // Return last valid records or empty if none found
    Ok(last_valid_records.unwrap_or_default())
}

/// Quick lookup that only fetches the current register value
/// (less thorough but faster - doesn't verify full history)
pub async fn quick_lookup(
    client: &Client,
    domain: &str,
) -> Result<String> {
    let register_addr = get_register_address_for_domain(domain)?;

    // Get current value (latest chunk address)
    let current_value = client
        .register_get(&register_addr)
        .await
        .context("Failed to get current register value")?;

    // Download the data
    let data_addr = DataAddress::new(XorName(current_value));
    let data_bytes = client
        .data_get_public(&data_addr)
        .await
        .context("Failed to download current records data")?;

    // Parse records
    let records_doc: DomainRecordsDocument = serde_json::from_slice(&data_bytes)
        .context("Failed to parse current records")?;

    // Extract target (note: this doesn't verify signature!)
    let target = records_doc.records.iter()
        .find(|r| r.record_type.eq_ignore_ascii_case("ant") && r.name == ".")
        .map(|r| r.value.clone())
        .ok_or_else(|| anyhow::anyhow!("No target record found"))?;

    Ok(target)
}
