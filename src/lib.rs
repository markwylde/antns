// Copyright 2025 AntNS Contributors
// Licensed under GPL-3.0

//! AntNS - Autonomi Name System
//!
//! A decentralized domain name system for the Autonomi network that provides
//! human-readable .ant domain names with cryptographic ownership verification.

pub mod constants;
pub mod crypto;
pub mod register;
pub mod server;
pub mod storage;
pub mod vault;
pub mod wallet;

pub use constants::*;

// Re-export commonly used types
pub use crypto::ed25519::{sign_records, verify_records};
pub use register::{
    create::register_domain,
    lookup::{lookup_domain, lookup_domain_records},
    update::{update_domain, update_domain_records, add_domain_record, delete_domain_record, update_domain_record},
    history::get_domain_history,
};
pub use storage::list_local_domains;

/// Common error type for AntNS operations
pub type Result<T> = std::result::Result<T, anyhow::Error>;
