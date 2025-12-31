// Copyright 2025 AntNS Contributors
// Licensed under GPL-3.0

//! Domain registration operations

use crate::constants::DNS_REGISTER_KEY_HEX;
use crate::crypto::DomainKeypair;
use crate::register::{DomainOwnerDocument, DomainRegistration};
use crate::storage::chunks::upload_document_as_chunk;
use anyhow::{Context, Result};
use autonomi::client::payment::PaymentOption;
use autonomi::{Client, SecretKey};

/// Register a new domain on the Autonomi network
///
/// This only creates the domain ownership, no records are added.
/// Use add_domain_record to add records after registration.
///
/// # Arguments
/// * `client` - Autonomi client instance
/// * `domain` - Domain name (e.g., "mydomain.ant")
/// * `payment` - Payment option for network storage
///
/// # Returns
/// Domain registration details including the generated keypair
pub async fn register_domain(
    client: &Client,
    domain: &str,
    payment: PaymentOption,
) -> Result<DomainRegistration> {
    // Step 1: Generate Ed25519 keypair for domain ownership
    let keypair = DomainKeypair::generate();

    // Step 2: Create owner document
    let owner_doc = DomainOwnerDocument {
        public_key: keypair.public_key_hex(),
    };

    // Step 3: Upload owner document as public chunk
    let (owner_cost, owner_chunk_addr) =
        upload_document_as_chunk(client, &owner_doc, payment.clone())
            .await
            .context("Failed to upload owner document")?;

    tracing::debug!(
        "Owner document uploaded to chunk: {}",
        hex::encode(owner_chunk_addr)
    );

    // Step 4: Convert chunk address to RegisterValue (32 bytes)
    let owner_value = Client::register_value_from_bytes(&owner_chunk_addr)
        .context("Failed to create register value from chunk address")?;

    // Step 5: Create register with shared DNS key
    let shared_key_bytes = hex::decode(DNS_REGISTER_KEY_HEX)?;
    let shared_key_array: [u8; 32] = shared_key_bytes
        .try_into()
        .map_err(|_| anyhow::anyhow!("Invalid DNS register key length"))?;
    let shared_key = SecretKey::from_bytes(shared_key_array)
        .map_err(|e| anyhow::anyhow!("Invalid DNS register key: {:?}", e))?;

    let register_key = Client::register_key_from_name(&shared_key, domain);

    let (register_cost, register_addr) = client
        .register_create(&register_key, owner_value, payment.clone())
        .await
        .context("Failed to create register")?;

    tracing::info!("Domain '{}' registered at: {}", domain, register_addr);

    // Calculate total cost
    let total_cost = owner_cost
        .checked_add(register_cost)
        .context("Cost overflow")?;

    Ok(DomainRegistration {
        domain: domain.to_string(),
        register_address: register_addr,
        owner_key: keypair.signing_key,
        total_cost,
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_domain_validation() {
        // Add domain name validation tests
        assert!(validate_domain_name("mydomain.ant"));
        assert!(validate_domain_name("test.autonomi"));
        assert!(!validate_domain_name("invalid"));
    }

    fn validate_domain_name(domain: &str) -> bool {
        domain.ends_with(".ant") || domain.ends_with(".autonomi")
    }
}
