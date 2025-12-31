// Copyright 2025 AntNS Contributors
// Licensed under GPL-3.0

//! Ed25519 signature operations for domain ownership verification

use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use crate::register::DnsRecord;
use anyhow::{Context, Result};

/// Sign a list of DNS records with an Ed25519 private key
/// Returns the signature as a hex string
pub fn sign_records(records: &[DnsRecord], signing_key: &SigningKey) -> Result<String> {
    // Serialize records to canonical JSON (deterministic ordering)
    let json = serde_json::to_string(records)
        .context("Failed to serialize records")?;

    // Sign the JSON bytes
    let signature = signing_key.sign(json.as_bytes());

    // Return hex-encoded signature
    Ok(hex::encode(signature.to_bytes()))
}

/// Verify a signature on DNS records using an Ed25519 public key
/// Returns true if the signature is valid
pub fn verify_records(
    records: &[DnsRecord],
    signature_hex: &str,
    verifying_key: &VerifyingKey,
) -> bool {
    // Decode signature from hex
    let sig_bytes = match hex::decode(signature_hex) {
        Ok(bytes) => bytes,
        Err(_) => return false,
    };

    // Parse signature
    let signature = match Signature::from_slice(&sig_bytes) {
        Ok(sig) => sig,
        Err(_) => return false,
    };

    // Serialize records to same canonical JSON format
    let json = match serde_json::to_string(records) {
        Ok(j) => j,
        Err(_) => return false,
    };

    // Verify signature
    verifying_key.verify(json.as_bytes(), &signature).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::SigningKey;
    use rand::rngs::OsRng;

    #[test]
    fn test_sign_and_verify() {
        let mut rng = OsRng;
        let signing_key = SigningKey::generate(&mut rng);
        let verifying_key = signing_key.verifying_key();

        let records = vec![DnsRecord {
            record_type: "ant".to_string(),
            name: ".".to_string(),
            value: "abc123".to_string(),
        }];

        let signature = sign_records(&records, &signing_key).unwrap();
        assert!(verify_records(&records, &signature, &verifying_key));
    }

    #[test]
    fn test_tamper_detection() {
        let mut rng = OsRng;
        let signing_key = SigningKey::generate(&mut rng);
        let verifying_key = signing_key.verifying_key();

        let records = vec![DnsRecord {
            record_type: "ant".to_string(),
            name: ".".to_string(),
            value: "abc123".to_string(),
        }];

        let signature = sign_records(&records, &signing_key).unwrap();

        // Tamper with records
        let tampered = vec![DnsRecord {
            record_type: "ant".to_string(),
            name: ".".to_string(),
            value: "xyz789".to_string(), // Changed!
        }];

        assert!(!verify_records(&tampered, &signature, &verifying_key));
    }
}
