// Copyright 2025 AntNS Contributors
// Licensed under GPL-3.0

//! Storage operations for chunks and local data

pub mod chunks;
pub mod local;

pub use chunks::{upload_document_as_chunk, download_document_from_chunk};
pub use local::{get_domain_keys_dir, list_local_domains};
