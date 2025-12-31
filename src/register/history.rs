// Copyright 2025 AntNS Contributors
// Licensed under GPL-3.0

//! Domain history operations

use autonomi::Client;
use autonomi::chunk::ChunkAddress;
use ed25519_dalek::VerifyingKey;
use xor_name::XorName;
use anyhow::{Context, Result};
use crate::crypto::verify_records;
use crate::register::{DomainOwnerDocument, DomainRecordsDocument, HistoryEntry};
use crate::register::get_register_address_for_domain;

/// Get the full history of a domain including all entries and their validation status
///
/// # Arguments
/// * `client` - Autonomi client instance
/// * `domain` - Domain name to query
///
/// # Returns
/// Vector of history entries with validation status
pub async fn get_domain_history(
    client: &Client,
    domain: &str,
) -> Result<Vec<HistoryEntry>> {
    // Get register address
    let register_addr = get_register_address_for_domain(domain)
        .context("Failed to derive register address")?;

    tracing::debug!("Fetching history for domain '{}' at register: {}", domain, register_addr);

    // Fetch register history
    let mut history = client
        .register_history(&register_addr);

    let mut entries = Vec::new();

    // First entry: owner document
    let owner_chunk_addr = history
        .next()
        .await?
        .ok_or_else(|| anyhow::anyhow!("Register not found for domain: {}", domain))?;

    let owner_chunk = ChunkAddress::new(XorName(owner_chunk_addr));
    let owner_chunk_data = client
        .chunk_get(&owner_chunk)
        .await
        .context("Failed to download owner document chunk")?;

    let owner_doc: DomainOwnerDocument = serde_json::from_slice(owner_chunk_data.value.as_ref())
        .context("Failed to parse owner document")?;

    // Parse owner public key for verification
    let owner_pubkey_bytes = hex::decode(&owner_doc.public_key)?;
    let owner_pubkey = VerifyingKey::from_bytes(
        owner_pubkey_bytes.as_slice().try_into()?
    )?;

    entries.push(HistoryEntry::Owner {
        public_key: owner_doc.public_key.clone(),
        chunk_address: hex::encode(owner_chunk_addr),
    });

    // Subsequent entries: records
    while let Some(chunk_addr) = history.next().await? {
        let chunk = ChunkAddress::new(XorName(chunk_addr));

        // Try to download and parse
        let (records, signature, is_valid) = match client.chunk_get(&chunk).await {
            Ok(chunk_data) => {
                match serde_json::from_slice::<DomainRecordsDocument>(chunk_data.value.as_ref()) {
                    Ok(doc) => {
                        // Verify signature
                        let is_valid = verify_records(
                            &doc.records,
                            &doc.signature,
                            &owner_pubkey
                        );
                        (Some(doc.records), Some(doc.signature), is_valid)
                    }
                    Err(e) => {
                        tracing::warn!("Failed to parse records document: {}", e);
                        (None, None, false)
                    }
                }
            }
            Err(e) => {
                tracing::warn!("Failed to download chunk: {}", e);
                (None, None, false)
            }
        };

        entries.push(HistoryEntry::Records {
            chunk_address: hex::encode(chunk_addr),
            records,
            signature,
            is_valid,
        });
    }

    Ok(entries)
}

/// Get statistics about a domain's history
pub struct HistoryStats {
    pub total_entries: usize,
    pub valid_entries: usize,
    pub invalid_entries: usize,
    pub spam_entries: usize,
}

/// Calculate statistics from history entries
pub fn calculate_history_stats(entries: &[HistoryEntry]) -> HistoryStats {
    let mut stats = HistoryStats {
        total_entries: entries.len(),
        valid_entries: 0,
        invalid_entries: 0,
        spam_entries: 0,
    };

    for entry in entries {
        match entry {
            HistoryEntry::Owner { .. } => {
                stats.valid_entries += 1;
            }
            HistoryEntry::Records { is_valid, records, .. } => {
                if *is_valid {
                    stats.valid_entries += 1;
                } else if records.is_some() {
                    // Parsed but invalid signature = spam
                    stats.spam_entries += 1;
                } else {
                    // Couldn't parse = corrupted
                    stats.invalid_entries += 1;
                }
            }
        }
    }

    stats
}
