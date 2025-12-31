// Copyright 2025 AntNS Contributors
// Licensed under GPL-3.0

//! Cryptographic operations for domain ownership

pub mod ed25519;
pub mod keypair;

pub use ed25519::{sign_records, verify_records};
pub use keypair::{DomainKeypair, save_keypair, load_keypair};
