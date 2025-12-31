// Copyright 2025 AntNS Contributors
// Licensed under GPL-3.0

//! Wallet loading utilities
//!
//! Reuses the wallet infrastructure from the `ant` CLI.
//! Users manage wallets with `ant wallet` commands, and AntNS loads them automatically.

use anyhow::{Context, Result};
use autonomi::Wallet;
use ring::aead::{BoundKey, Nonce, NonceSequence};
use ring::error::Unspecified;
use std::env;
use std::io::Read;
use std::num::NonZeroU32;
use std::path::PathBuf;
use std::sync::LazyLock;

const SECRET_KEY_ENV: &str = "SECRET_KEY";
const ENCRYPTED_PRIVATE_KEY_EXT: &str = ".encrypted";
const SALT_LENGTH: usize = 8;
const NONCE_LENGTH: usize = 12;

/// Number of iterations for pbkdf2.
static ITERATIONS: LazyLock<NonZeroU32> = LazyLock::new(|| {
    #[allow(clippy::expect_used)]
    NonZeroU32::new(100_000).expect("Infallible")
});

struct NonceSeq([u8; 12]);

impl NonceSequence for NonceSeq {
    fn advance(&mut self) -> Result<Nonce, Unspecified> {
        Nonce::try_assume_unique_for_key(&self.0)
    }
}

/// Load wallet using the network from an initialized client
///
/// This ensures the wallet and client use the same EVM network.
///
/// Priority:
/// 1. SECRET_KEY environment variable
/// 2. Wallet files in ~/.local/share/autonomi/client/wallets/
///
/// # Examples
///
/// ```bash
/// # Using environment variable
/// export SECRET_KEY=0x...
/// antns names register test.ant
///
/// # Using ant CLI wallet (prompts for selection if multiple exist)
/// ant wallet create
/// antns names register test.ant
/// ```
pub fn load_wallet_from_client(client: &autonomi::Client) -> Result<Wallet> {
    let (wallet, _) = load_wallet_with_private_key(client)?;
    Ok(wallet)
}

/// Load wallet and its private key using the network from an initialized client
///
/// Returns both the Wallet and the private key hex string.
/// The private key is needed for vault operations.
pub fn load_wallet_with_private_key(client: &autonomi::Client) -> Result<(Wallet, String)> {
    // Get the network from the client to ensure they match
    let network = client.evm_network().clone();

    // Try environment variable first
    if let Ok(secret_key) = env::var(SECRET_KEY_ENV) {
        tracing::info!("Loading wallet from SECRET_KEY environment variable");
        let wallet = Wallet::new_from_private_key(network, &secret_key)
            .context("Failed to create wallet from SECRET_KEY environment variable")?;
        return Ok((wallet, secret_key));
    }

    // Try loading from ant CLI's wallet directory
    tracing::info!("Attempting to load wallet from disk");
    match load_wallet_with_key_from_disk(&network) {
        Ok((wallet, private_key)) => {
            tracing::info!("Loaded wallet from disk: {}", wallet.address());
            Ok((wallet, private_key))
        }
        Err(e) => {
            anyhow::bail!(
                "No wallet found: {}\n\n\
                Please either:\n\
                1. Set SECRET_KEY environment variable:\n\
                   export SECRET_KEY=0x...\n\n\
                2. Create a wallet with ant CLI:\n\
                   ant wallet create\n\n\
                3. Import a wallet with ant CLI:\n\
                   ant wallet import 0x...\n\n\
                Wallet directory: {:?}",
                e,
                get_wallet_dir_path()
                    .unwrap_or_else(|_| PathBuf::from("~/.local/share/autonomi/client/wallets"))
            )
        }
    }
}

/// Get the ant CLI wallet directory path
fn get_wallet_dir_path() -> Result<PathBuf> {
    // Use the same path as ant CLI
    use directories::ProjectDirs;

    let proj_dirs = ProjectDirs::from("", "", "autonomi")
        .ok_or_else(|| anyhow::anyhow!("Could not determine project directories"))?;

    let mut path = proj_dirs.data_local_dir().to_path_buf();
    path.push("client");
    path.push("wallets");

    Ok(path)
}

/// Load wallet and private key from ant CLI's wallet directory
fn load_wallet_with_key_from_disk(network: &autonomi::Network) -> Result<(Wallet, String)> {
    let wallet_dir = get_wallet_dir_path().context("Failed to get wallet directory path")?;

    if !wallet_dir.exists() {
        anyhow::bail!("Wallet directory does not exist: {:?}", wallet_dir);
    }

    // Get all wallet files
    let wallet_files: Vec<_> = std::fs::read_dir(&wallet_dir)
        .context("Failed to read wallet directory")?
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let path = entry.path();
            if path.is_file() {
                Some(path)
            } else {
                None
            }
        })
        .collect();

    if wallet_files.is_empty() {
        anyhow::bail!("No wallet files found in directory: {:?}", wallet_dir);
    }

    // Select wallet
    let wallet_path = if wallet_files.len() == 1 {
        // Only one wallet - use it automatically
        wallet_files[0].clone()
    } else {
        // Multiple wallets - let user select
        select_wallet_from_list(&wallet_files)?
    };

    tracing::debug!("Loading wallet from: {:?}", wallet_path);

    // Load private key from file
    let private_key = load_private_key_from_file(&wallet_path)?;

    // Create wallet
    let wallet = Wallet::new_from_private_key(network.clone(), &private_key)
        .context("Failed to create wallet from private key file")?;

    Ok((wallet, private_key))
}

/// Let user select a wallet from multiple options
fn select_wallet_from_list(wallet_files: &[PathBuf]) -> Result<PathBuf> {
    println!("\nMultiple wallets found:");
    println!();

    // Display wallet list
    for (i, path) in wallet_files.iter().enumerate() {
        let file_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");

        // Extract wallet address (remove .encrypted extension if present)
        let address = file_name.replace(ENCRYPTED_PRIVATE_KEY_EXT, "");

        // Check if encrypted
        let encrypted = file_name.contains(ENCRYPTED_PRIVATE_KEY_EXT);
        let status = if encrypted { "(encrypted)" } else { "" };

        println!("  [{}] {} {}", i + 1, address, status);
    }

    println!();
    print!("Select wallet by index: ");
    std::io::Write::flush(&mut std::io::stdout()).context("Failed to flush stdout")?;

    // Read user input
    let mut input = String::new();
    std::io::stdin()
        .read_line(&mut input)
        .context("Failed to read wallet selection")?;

    let input = input.trim();
    let selection = input
        .parse::<usize>()
        .with_context(|| format!("Invalid input '{}'. Please enter a number.", input))?;

    if selection < 1 || selection > wallet_files.len() {
        anyhow::bail!(
            "Invalid wallet selection '{}'. Please choose a number between 1 and {}",
            selection,
            wallet_files.len()
        );
    }

    Ok(wallet_files[selection - 1].clone())
}

/// Load private key from file (handles both plain and encrypted files)
fn load_private_key_from_file(path: &PathBuf) -> Result<String> {
    let mut file = std::fs::File::open(path)
        .with_context(|| format!("Failed to open wallet file: {:?}", path))?;

    let mut buffer = String::new();
    file.read_to_string(&mut buffer)
        .context("Failed to read wallet file")?;

    let buffer = buffer.trim();

    // Check if file is encrypted
    let is_encrypted = path
        .file_name()
        .and_then(|n| n.to_str())
        .map(|n| n.contains(ENCRYPTED_PRIVATE_KEY_EXT))
        .unwrap_or(false);

    if is_encrypted {
        // Prompt for password
        let password = rpassword::prompt_password("Enter wallet password: ")
            .context("Failed to read password")?;

        // Decrypt the private key
        decrypt_private_key(buffer, &password)
            .context("Failed to decrypt wallet. Check your password and try again.")
    } else {
        Ok(buffer.to_string())
    }
}

/// Decrypt an encrypted private key using CHACHA20_POLY1305
fn decrypt_private_key(encrypted_data: &str, password: &str) -> Result<String> {
    let encrypted_data = hex::decode(encrypted_data).context("Encrypted data is invalid")?;

    let salt: [u8; SALT_LENGTH] = encrypted_data[..SALT_LENGTH]
        .try_into()
        .context("Could not extract salt from encrypted data")?;

    let nonce: [u8; NONCE_LENGTH] = encrypted_data[SALT_LENGTH..SALT_LENGTH + NONCE_LENGTH]
        .try_into()
        .context("Could not extract nonce from encrypted data")?;

    let encrypted_private_key = &encrypted_data[SALT_LENGTH + NONCE_LENGTH..];

    let mut key = [0; 32];

    // Reconstruct the key from salt and password using PBKDF2
    ring::pbkdf2::derive(
        ring::pbkdf2::PBKDF2_HMAC_SHA512,
        *ITERATIONS,
        &salt,
        password.as_bytes(),
        &mut key,
    );

    // Create an unbound key from the reconstructed key
    let unbound_key = ring::aead::UnboundKey::new(&ring::aead::CHACHA20_POLY1305, &key)
        .context("Failed to create decryption key")?;

    // Create an opening key using the unbound key and original nonce
    let mut opening_key = ring::aead::OpeningKey::new(unbound_key, NonceSeq(nonce));
    let aad = ring::aead::Aad::from(&[]);

    let mut encrypted_private_key = encrypted_private_key.to_vec();

    // Decrypt the encrypted private key bytes
    let decrypted_data = opening_key
        .open_in_place(aad, &mut encrypted_private_key)
        .context("Could not decrypt wallet. Please check your password.")?;

    // Convert decrypted bytes to string
    String::from_utf8(decrypted_data.to_vec())
        .context("Failed to convert decrypted private key to string")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wallet_dir_path() {
        let path = get_wallet_dir_path();
        assert!(path.is_ok());

        if let Ok(p) = path {
            assert!(p.to_string_lossy().contains("autonomi"));
            assert!(p.to_string_lossy().contains("wallets"));
        }
    }
}
