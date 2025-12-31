# AntNS Development Guide

## Prerequisites

### 1. Rust Toolchain (1.70+)

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

### 2. Autonomi Network Access

You need a running Autonomi network (local testnet or live network):

```bash
cd /Users/markwylde/Documents/Projects/autonomi
# Follow Autonomi's setup instructions to run a local network
```

### 3. Wallet with Tokens (for network operations)

The wallet system is fully integrated. Network operations automatically load wallets from ant CLI:

**Setup:**
```bash
# Create a new wallet
ant wallet create

# Or import an existing wallet
ant wallet import <private-key>
```

**Usage:**
```bash
# AntNS will automatically find and prompt you to select from available wallets
./target/release/antns names register mydomain.ant

# Features:
# → Interactive wallet selection (if multiple wallets exist)
# → Support for encrypted and plain wallet formats
# → Password prompt for encrypted wallets
```

## Quick Start for Developers

### 1. Clone and Build

```bash
cd /Users/markwylde/Documents/Projects/antns

# Build debug version (fast compilation)
cargo build

# Build release version (optimized, slower compilation)
cargo build --release
```

The binary will be at:
- Debug: `./target/debug/antns`
- Release: `./target/release/antns`

### 2. Run Tests

```bash
# Fast: just check compilation
cargo check

# Run unit tests
cargo test

# Run with output
cargo test -- --nocapture
```

### 3. Try the CLI

```bash
# Help
./target/debug/antns --help

# List local domains
./target/debug/antns names list
```

## Current Limitations

**Network Requirements:**
- You need access to a running Autonomi network (local testnet or live network)
- A wallet with tokens is required for register/update operations
- `antns names lookup` will work if domains exist on the network
- `antns names list/export/import` work for local key management

**Timeout Behavior:**
- Network operations timeout after 30 seconds
- Non-existent domains show: "✗ Domain not found"
- No infinite retries!

## What's Implemented vs What's Not

### ✅ Working (No Network Required)

- **Cryptography**: Ed25519 key generation, signing, verification
- **Local Storage**: Save/load domain keys to ~/.local/share/autonomi/...
- **CLI Parsing**: All commands parse correctly
- **Data Structures**: Owner documents, signed records, register addressing

Test these:
```bash
# Generate and export a key (creates local file)
./target/debug/antns names import test.ant --key $(openssl rand -hex 32)
./target/debug/antns names export test.ant
./target/debug/antns names list
```

### ✅ Fully Working

- **DNS Server**: Hickory-DNS server on port 5354
- **HTTP Proxy**: Hyper proxy on port 18888
- **Domain Resolution Caching**: Configurable TTL (default 60 minutes)
- **Server Lifecycle**: Start, stop, status commands
- **Payment Integration**: Full ant CLI wallet support with:
  - Automatic wallet discovery from ant CLI directory
  - Encrypted/plain wallet support
  - Interactive wallet selection
  - Password prompts for encrypted wallets
- **Domain Registration**: Register domains with payment
- **Domain Lookup**: Query registered domains from the network
- **Domain Updates**: Update existing domain records with payment
- **Network Vault**: Backup/restore domain keys to Autonomi network

### ⚠️ Requires Network Connection

- **Domain Registration/Updates**: Needs:
  - Running Autonomi network
  - Wallet with tokens
  - Network connectivity

- **Domain Lookup**: Requires:
  - Network is running
  - Domain was previously registered

### ✅ All Core Features Complete

All phases (1-5) are now fully implemented.

## Testing Locally

### Option 1: Mock Tests (No Network)

Add to `src/register/create.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_address_derivation() {
        let addr1 = crate::register::get_register_address_for_domain("test.ant").unwrap();
        let addr2 = crate::register::get_register_address_for_domain("test.ant").unwrap();
        assert_eq!(addr1, addr2);
    }

    #[test]
    fn test_signing() {
        let keypair = DomainKeypair::generate();
        let records = vec![DnsRecord {
            record_type: "ant".to_string(),
            name: ".".to_string(),
            value: "abc123".to_string(),
        }];

        let sig = sign_records(&records, &keypair.signing_key).unwrap();
        assert!(verify_records(&records, &sig, &keypair.verifying_key));
    }
}
```

Run:
```bash
cargo test
```

### Option 2: Integration Tests (Needs Network)

1. **Start Autonomi local network:**
```bash
cd /Users/markwylde/Documents/Projects/autonomi
# Follow their README for local network setup
```

2. **Create integration test:**

`tests/integration_test.rs`:
```rust
use antns;
use autonomi::{Client, Wallet};

#[tokio::test]
#[ignore]  // Run with: cargo test -- --ignored
async fn test_full_flow() {
    let client = Client::init_local().await.unwrap();
    let wallet = Wallet::new_from_private_key(
        autonomi::Network::Testnet,
        "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"
    ).unwrap();

    // Register domain
    let payment = autonomi::client::payment::PaymentOption::from(&wallet);
    let reg = antns::register_domain(&client, "test.ant", "abc123", payment.clone()).await.unwrap();

    // Look it up
    let resolution = antns::lookup_domain(&client, "test.ant").await.unwrap();
    assert_eq!(resolution.target, "abc123");

    // Update it
    antns::update_domain(&client, "test.ant", "xyz789", &reg.owner_key, payment).await.unwrap();

    // Verify update
    let resolution2 = antns::lookup_domain(&client, "test.ant").await.unwrap();
    assert_eq!(resolution2.target, "xyz789");
}
```

3. **Run:**
```bash
cargo test -- --ignored --nocapture
```

## Code Style

### Use `?` for Error Handling

**Bad:**
```rust
let result = match some_operation() {
    Ok(val) => val,
    Err(e) => return Err(e),
};
```

**Good:**
```rust
let result = some_operation()?;
```

### Add Context to Errors

**Bad:**
```rust
let data = std::fs::read(path)?;
```

**Good:**
```rust
let data = std::fs::read(path)
    .context("Failed to read domain key file")?;
```

### Use Tracing for Logging

```rust
tracing::debug!("Register address: {}", addr);
tracing::info!("Domain '{}' registered successfully", domain);
tracing::warn!("Invalid signature on chunk {}", chunk_addr);
tracing::error!("Failed to connect to network: {}", err);
```

## Common Issues

### "Cannot find DNS_REGISTER_KEY"

The constant is in `src/constants.rs`. Make sure:
```rust
use crate::constants::DNS_REGISTER_KEY_HEX;
```

### "ChunkAddress not found"

Import from the right place:
```rust
use autonomi::chunk::ChunkAddress;  // Not client::address::str::ChunkAddress
```

### "RegisterAddress not found"

Import from:
```rust
use autonomi::register::RegisterAddress;  // Not client::registers::RegisterAddress
```

### Compilation is Slow

```bash
# Use debug builds for development
cargo build  # ~30s

# Only use release for final testing
cargo build --release  # ~2min
```

### Autonomi API Changed

If autonomi crate is updated:
```bash
cd /Users/markwylde/Documents/Projects/autonomi
git pull
cargo build

cd /Users/markwylde/Documents/Projects/antns
cargo clean
cargo build
```

## Performance Tips

### Parallel Chunk Downloads

In `src/register/lookup.rs`, you could optimize:

```rust
// Current: sequential
while let Some(chunk_addr) = history.next().await? {
    let chunk_data = client.chunk_get(&chunk).await?;
    // ...
}

// Better: parallel (future optimization)
let chunk_addrs: Vec<_> = history.collect().await?;
let futures: Vec<_> = chunk_addrs.iter()
    .map(|addr| client.chunk_get(&ChunkAddress::new(XorName(*addr))))
    .collect();
let results = futures::future::join_all(futures).await;
```

### Caching

Domain resolution caching is implemented with configurable TTL. See "DNS & HTTP Server" section below.

## DNS & HTTP Server

The server provides DNS resolution and HTTP proxy for `.ant` domains with built-in caching.

### Usage

```bash
# Start server with default 60 minute cache
sudo ./target/release/antns server start

# Custom cache TTL (10 minutes)
sudo ./target/release/antns server start --ttl=10

# Disable caching (always query network)
sudo ./target/release/antns server start --ttl=0

# Stop server
sudo ./target/release/antns server stop

# Check status
sudo ./target/release/antns server status
```

### macOS Configuration

For `.ant` domains to resolve automatically:

```bash
# Create resolver config
sudo mkdir -p /etc/resolver
echo "nameserver 127.0.0.1" | sudo tee /etc/resolver/ant
echo "port 5354" | sudo tee -a /etc/resolver/ant
```

### Cache Behavior

Console output shows cache status:
- `✓ Cache hit (age: 25s)` - Used cached result
- `Cache expired (age: 3601s)` - Re-querying because cache expired
- `Looking up domain:` - Cache miss, querying network

## Logging Control

```bash
# Quiet mode (default): only warnings/errors
./target/release/antns names list

# Verbose mode: show INFO messages
./target/release/antns -v names list

# Debug mode: see everything
RUST_LOG=debug ./target/release/antns names list

# Target specific modules
RUST_LOG=antns=debug,autonomi=warn ./target/release/antns names lookup example.ant
```

## Dependencies

Key dependencies:
- `autonomi` - Autonomi network client (local path)
- `ant-protocol` - Autonomi protocol types (local path)
- `ed25519-dalek` - Ed25519 signatures for domain ownership
- `blsttc` (as `bls`) - BLS cryptography for register operations
- `clap` - CLI argument parsing
- `hickory-dns` - DNS server
- `hyper` - HTTP proxy
- `tokio` - Async runtime
- `serde` / `serde_json` - Serialization
- `anyhow` / `thiserror` - Error handling
- `tracing` - Logging

## Future Enhancements

All core features are complete. Future improvements could include:

### Performance Optimizations
- [ ] Parallel chunk downloads during lookup
- [ ] Connection pooling
- [ ] Batch operations
- [ ] Domain resolution caching improvements

### Enhanced Features
- [ ] Cost estimation before operations
- [ ] Payment receipt handling and history
- [ ] Subdomain support (e.g., `www.mydomain.ant`)
- [ ] Multiple record types (CNAME, TXT, MX)
- [ ] Web UI for domain management

## Useful Commands

```bash
# Fast iteration during development
cargo watch -x check

# Format before committing
cargo fmt

# Catch common mistakes
cargo clippy

# View binary size
du -h target/release/antns

# Clean build artifacts
cargo clean

# Update dependencies
cargo update

# Check for outdated deps
cargo install cargo-outdated
cargo outdated

# Security audit
cargo install cargo-audit
cargo audit
```

## Documentation

Generate API docs:
```bash
cargo doc --open
```

Add doc comments:
```rust
/// Register a new domain on the Autonomi network
///
/// # Arguments
/// * `client` - Autonomi client instance
/// * `domain` - Domain name (e.g., "mydomain.ant")
///
/// # Example
/// ```no_run
/// let reg = register_domain(&client, "test.ant", "target", payment).await?;
/// ```
pub async fn register_domain(...) -> Result<DomainRegistration> {
```

## Contributing

1. Create feature branch: `git checkout -b feature/network-vault`
2. Make changes
3. Run tests: `cargo test`
4. Format: `cargo fmt`
5. Commit: `git commit -m "Add network vault support"`
6. Push: `git push origin feature/network-vault`

## Getting Help

- **Autonomi API docs**: Check `/Users/markwylde/Documents/Projects/autonomi/autonomi/src`
- **Rust book**: https://doc.rust-lang.org/book/
- **Tokio guide**: https://tokio.rs/tokio/tutorial
- **AntNS Protocol**: See `RFC-ANT-DNS.md` for protocol specification
- **AntNS Architecture**: See `ARCHITECTURE.md` for system design
