// Copyright 2025 AntNS Contributors
// Licensed under GPL-3.0

//! Register operations for domain management

pub mod create;
pub mod history;
pub mod lookup;
pub mod update;

use crate::constants::DNS_REGISTER_KEY_HEX;
use autonomi::register::RegisterAddress;
use autonomi::{Client, SecretKey};

/// Get the register address for a given domain name
/// Uses the shared DNS_REGISTER_KEY to ensure everyone derives the same address
pub fn get_register_address_for_domain(domain: &str) -> Result<RegisterAddress, hex::FromHexError> {
    let shared_key_bytes = hex::decode(DNS_REGISTER_KEY_HEX)?;
    let shared_key_array: [u8; 32] = shared_key_bytes
        .try_into()
        .map_err(|_| hex::FromHexError::InvalidStringLength)?;
    let shared_key = SecretKey::from_bytes(shared_key_array)
        .map_err(|_| hex::FromHexError::InvalidHexCharacter { c: '?', index: 0 })?;

    let register_key = Client::register_key_from_name(&shared_key, domain);
    Ok(RegisterAddress::new(register_key.public_key()))
}

/// Data structures for DNS records
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainOwnerDocument {
    #[serde(rename = "publicKey")]
    pub public_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainRecordsDocument {
    pub records: Vec<DnsRecord>,
    pub signature: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DnsRecord {
    #[serde(rename = "type")]
    pub record_type: String,
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone)]
pub struct DomainRegistration {
    pub domain: String,
    pub register_address: RegisterAddress,
    pub owner_key: ed25519_dalek::SigningKey,
    pub total_cost: autonomi::AttoTokens,
}

#[derive(Debug, Clone)]
pub struct DomainResolution {
    pub domain: String,
    pub target: String,
    pub owner_public_key: String,
}

#[derive(Debug, Clone)]
pub enum HistoryEntry {
    Owner {
        public_key: String,
        chunk_address: String,
    },
    Records {
        chunk_address: String,
        records: Option<Vec<DnsRecord>>,
        signature: Option<String>,
        is_valid: bool,
    },
}
