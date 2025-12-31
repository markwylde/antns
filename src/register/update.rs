// Copyright 2025 AntNS Contributors
// Licensed under GPL-3.0

//! Domain update operations

use autonomi::{Client, SecretKey, AttoTokens};
use autonomi::client::payment::PaymentOption;
use ed25519_dalek::SigningKey;
use anyhow::{Context, Result};
use crate::crypto::sign_records;
use crate::register::{DomainRecordsDocument, DnsRecord};
use crate::storage::chunks::upload_document_as_chunk;
use crate::constants::DNS_REGISTER_KEY_HEX;

/// Update a domain's target address
///
/// # Arguments
/// * `client` - Autonomi client instance
/// * `domain` - Domain name to update
/// * `new_target` - New target address (hex)
/// * `owner_key` - Domain owner's Ed25519 signing key
/// * `payment` - Payment option for chunk upload
///
/// # Returns
/// Total cost of the update operation
pub async fn update_domain(
    client: &Client,
    domain: &str,
    new_target: &str,
    owner_key: &SigningKey,
    payment: PaymentOption,
) -> Result<AttoTokens> {
    tracing::info!("Updating domain '{}' to target: {}", domain, new_target);

    // Step 1: Create new records
    let records = vec![DnsRecord {
        record_type: "ant".to_string(),
        name: ".".to_string(),
        value: new_target.to_string(),
    }];

    // Step 2: Sign records with owner key
    let signature = sign_records(&records, owner_key)
        .context("Failed to sign records")?;

    let records_doc = DomainRecordsDocument {
        records,
        signature,
    };

    // Step 3: Upload new records as chunk
    let (chunk_cost, records_chunk_addr) = upload_document_as_chunk(
        client,
        &records_doc,
        payment.clone(),
    ).await.context("Failed to upload new records document")?;

    tracing::debug!("New records uploaded to chunk: {}", hex::encode(records_chunk_addr));

    // Step 4: Append to register
    let records_value = Client::register_value_from_bytes(&records_chunk_addr)
        .context("Failed to create register value from chunk address")?;

    // Derive register key from shared DNS key and domain name
    let shared_key_bytes = hex::decode(DNS_REGISTER_KEY_HEX)?;
    let shared_key_array: [u8; 32] = shared_key_bytes
        .try_into()
        .map_err(|_| anyhow::anyhow!("Invalid DNS register key length"))?;
    let shared_key = SecretKey::from_bytes(shared_key_array)
        .map_err(|e| anyhow::anyhow!("Invalid DNS register key: {:?}", e))?;

    let register_key = Client::register_key_from_name(&shared_key, domain);

    let update_cost = client
        .register_update(&register_key, records_value, payment)
        .await
        .context("Failed to update register")?;

    tracing::info!("Domain '{}' updated successfully", domain);

    // Total cost
    let total_cost = chunk_cost
        .checked_add(update_cost)
        .context("Cost overflow")?;

    Ok(total_cost)
}

/// Update domain with multiple record types
pub async fn update_domain_records(
    client: &Client,
    domain: &str,
    records: Vec<DnsRecord>,
    owner_key: &SigningKey,
    payment: PaymentOption,
) -> Result<AttoTokens> {
    tracing::info!("Updating domain '{}' with {} records", domain, records.len());

    // Sign all records
    let signature = sign_records(&records, owner_key)
        .context("Failed to sign records")?;

    let records_doc = DomainRecordsDocument {
        records,
        signature,
    };

    // Upload records document
    let (chunk_cost, records_chunk_addr) = upload_document_as_chunk(
        client,
        &records_doc,
        payment.clone(),
    ).await.context("Failed to upload records document")?;

    // Update register
    let records_value = Client::register_value_from_bytes(&records_chunk_addr)?;

    let shared_key_bytes = hex::decode(DNS_REGISTER_KEY_HEX)?;
    let shared_key_array: [u8; 32] = shared_key_bytes
        .try_into()
        .map_err(|_| anyhow::anyhow!("Invalid DNS register key length"))?;
    let shared_key = SecretKey::from_bytes(shared_key_array)
        .map_err(|e| anyhow::anyhow!("Invalid DNS register key: {:?}", e))?;

    let register_key = Client::register_key_from_name(&shared_key, domain);

    let update_cost = client
        .register_update(&register_key, records_value, payment)
        .await
        .context("Failed to update register")?;

    let total_cost = chunk_cost.checked_add(update_cost)
        .context("Cost overflow")?;

    Ok(total_cost)
}

/// Add a new record to a domain
///
/// Fetches current records, adds the new one, and updates the register
pub async fn add_domain_record(
    client: &Client,
    domain: &str,
    new_record: DnsRecord,
    owner_key: &SigningKey,
    payment: PaymentOption,
) -> Result<AttoTokens> {
    tracing::info!("Adding record to domain '{}'", domain);

    // Fetch current records
    let mut current_records = crate::lookup_domain_records(client, domain)
        .await
        .unwrap_or_else(|_| Vec::new()); // If no records exist, start with empty

    // Add new record
    current_records.push(new_record);

    // Update with all records
    update_domain_records(client, domain, current_records, owner_key, payment).await
}

/// Delete a record by index
///
/// Fetches current records, removes the specified one, and updates the register
pub async fn delete_domain_record(
    client: &Client,
    domain: &str,
    index: usize,
    owner_key: &SigningKey,
    payment: PaymentOption,
) -> Result<AttoTokens> {
    tracing::info!("Deleting record {} from domain '{}'", index, domain);

    // Fetch current records
    let mut current_records = crate::lookup_domain_records(client, domain)
        .await
        .context("Failed to fetch current records")?;

    // Validate index
    if index >= current_records.len() {
        anyhow::bail!("Record index {} out of bounds (total records: {})", index, current_records.len());
    }

    // Remove record
    current_records.remove(index);

    // Update with remaining records
    update_domain_records(client, domain, current_records, owner_key, payment).await
}

/// Update a record by index
///
/// Fetches current records, replaces the specified one, and updates the register
pub async fn update_domain_record(
    client: &Client,
    domain: &str,
    index: usize,
    new_record: DnsRecord,
    owner_key: &SigningKey,
    payment: PaymentOption,
) -> Result<AttoTokens> {
    tracing::info!("Updating record {} for domain '{}'", index, domain);

    // Fetch current records
    let mut current_records = crate::lookup_domain_records(client, domain)
        .await
        .context("Failed to fetch current records")?;

    // Validate index
    if index >= current_records.len() {
        anyhow::bail!("Record index {} out of bounds (total records: {})", index, current_records.len());
    }

    // Replace record
    current_records[index] = new_record;

    // Update with modified records
    update_domain_records(client, domain, current_records, owner_key, payment).await
}
