// Copyright 2025 AntNS Contributors
// Licensed under GPL-3.0

//! Domain keypair management and storage

use ed25519_dalek::{SigningKey, VerifyingKey};
use anyhow::{Context, Result};
use std::path::PathBuf;

/// Domain keypair structure
#[derive(Debug)]
pub struct DomainKeypair {
    pub signing_key: SigningKey,
    pub verifying_key: VerifyingKey,
}

impl DomainKeypair {
    /// Generate a new random keypair
    pub fn generate() -> Self {
        let signing_key = SigningKey::generate(&mut rand::rngs::OsRng);
        let verifying_key = signing_key.verifying_key();

        Self {
            signing_key,
            verifying_key,
        }
    }

    /// Create from existing signing key bytes
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let signing_key = SigningKey::from_bytes(
            bytes.try_into()
                .context("Invalid key length, expected 32 bytes")?
        );
        let verifying_key = signing_key.verifying_key();

        Ok(Self {
            signing_key,
            verifying_key,
        })
    }

    /// Get signing key as bytes
    pub fn to_bytes(&self) -> [u8; 32] {
        self.signing_key.to_bytes()
    }

    /// Get public key as hex string
    pub fn public_key_hex(&self) -> String {
        hex::encode(self.verifying_key.to_bytes())
    }
}

/// Save a domain keypair to local storage
pub fn save_keypair(domain: &str, keypair: &DomainKeypair) -> Result<PathBuf> {
    let keys_dir = crate::storage::local::get_domain_keys_dir()?;
    std::fs::create_dir_all(&keys_dir)
        .context("Failed to create domain keys directory")?;

    // Save private key
    let key_file = keys_dir.join(format!("domain-key-{}.txt", domain));
    let key_hex = hex::encode(keypair.to_bytes());
    std::fs::write(&key_file, key_hex)
        .context("Failed to write private key file")?;

    // Save metadata
    let meta_file = keys_dir.join(format!("domain-meta-{}.json", domain));
    let metadata = serde_json::json!({
        "domain": domain,
        "publicKey": keypair.public_key_hex(),
        "created": chrono::Utc::now().to_rfc3339(),
    });
    std::fs::write(&meta_file, serde_json::to_string_pretty(&metadata)?)
        .context("Failed to write metadata file")?;

    Ok(key_file)
}

/// Load a domain keypair from local storage
pub fn load_keypair(domain: &str) -> Result<DomainKeypair> {
    let keys_dir = crate::storage::local::get_domain_keys_dir()?;
    let key_file = keys_dir.join(format!("domain-key-{}.txt", domain));

    let key_hex = std::fs::read_to_string(&key_file)
        .context("Failed to read private key file")?;

    let key_bytes = hex::decode(key_hex.trim())
        .context("Invalid hex in private key file")?;

    DomainKeypair::from_bytes(&key_bytes)
}
