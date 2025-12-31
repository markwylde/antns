// Copyright 2025 AntNS Contributors
// Licensed under GPL-3.0

//! Local filesystem storage operations

use anyhow::{Context, Result};
use std::path::PathBuf;

/// Get the directory where domain keys are stored
pub fn get_domain_keys_dir() -> Result<PathBuf> {
    let home = directories::BaseDirs::new().context("Failed to determine home directory")?;

    Ok(home
        .data_local_dir()
        .join("autonomi")
        .join("client")
        .join("user_data")
        .join("domain-keys"))
}

/// List all locally stored domains
pub fn list_local_domains() -> Result<Vec<String>> {
    let keys_dir = get_domain_keys_dir()?;

    if !keys_dir.exists() {
        return Ok(Vec::new());
    }

    let mut domains = Vec::new();

    for entry in std::fs::read_dir(&keys_dir)? {
        let entry = entry?;
        let filename = entry.file_name();
        let filename_str = filename.to_string_lossy();

        // Look for domain-key-*.txt files
        if filename_str.starts_with("domain-key-") && filename_str.ends_with(".txt") {
            // Extract domain name
            let domain = filename_str
                .strip_prefix("domain-key-")
                .and_then(|s| s.strip_suffix(".txt"))
                .map(|s| s.to_string());

            if let Some(domain) = domain {
                domains.push(domain);
            }
        }
    }

    domains.sort();
    Ok(domains)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_domain_keys_dir() {
        let dir = get_domain_keys_dir().unwrap();
        assert!(dir.to_string_lossy().contains("autonomi"));
        assert!(dir.to_string_lossy().contains("domain-keys"));
    }
}
