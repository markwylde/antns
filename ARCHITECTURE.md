# AntNS Architecture

Detailed technical architecture and design decisions for the Autonomi Name System.

## Table of Contents

- [The Problem](#the-problem)
- [Our Solution](#our-solution)
- [Architecture](#architecture)
- [DNS Resolver & Proxy](#dns-resolver--proxy)
- [CLI API](#cli-api)
- [Security Model](#security-model)
- [Technical Details](#technical-details)
- [Why This Design?](#why-this-design)
- [Limitations](#limitations)
- [Future Enhancements](#future-enhancements)

## The Problem

Autonomi's register system has a fundamental conflict between discoverability and ownership:

### Option 1: Unique Register Keys (Secure but Not Discoverable)

```bash
# Each user generates their own unique register signing key
ant register create mydomain "target-address"
```

**Problem:** Register addresses = hash(user's_unique_key + name)
- ✅ Only the key holder can edit
- ❌ Nobody else can look it up (they don't have your unique key)
- ❌ `ant register get mydomain` fails for everyone else

### Option 2: Shared Global Key (Discoverable but Not Secure)

```bash
# Everyone uses the same well-known key
ant register create mydomain "target-address"
```

**Problem:** Everyone derives the same address
- ✅ Anyone can find it
- ❌ Anyone can edit it (no ownership)

## Our Solution: Global Key + Cryptographic Signatures

We use a hybrid approach:
1. **Shared DNS register key** → Universal discoverability
2. **Ed25519 signatures** → Cryptographic ownership
3. **Chunk-based storage** → Spam resistance

### Register Structure

```
Register Name: mydomain.ant (using shared DNS_REGISTER_KEY)
├─ Entry 1: chunk_abc123 → { "publicKey": "owner_ed25519_pubkey" }
├─ Entry 2: chunk_def456 → { "records": [...], "signature": "valid" } ✅
├─ Entry 3: chunk_spam88 → { "records": [...], "signature": "invalid" } ❌ ignored
└─ Entry 4: chunk_ghi789 → { "records": [...], "signature": "valid" } ✅
```

**Key Properties:**
- Anyone can write to the register (global key)
- Only entries signed by the owner's Ed25519 key are valid
- Last valid entry wins
- Spam entries are ignored during lookup

### Why Registers Can't Be Deleted

Autonomi registers have "mutable but immutable history" by design:
- ✅ Unlimited updates allowed
- ❌ No deletion or history trimming
- ❌ No `register_delete` operation exists
- **Reason:** Built on immutable primitives (GraphEntries), permanent audit trail

This validates our approach: Invalid entries can't be deleted, but signature verification makes them irrelevant.

---

## Architecture

### 1. Registration Flow

```bash
antns names register mydomain.ant a33082163be512fb471a1cca385332b32c19917deec3989a97e100d827f97baf
```

**Steps:**

1. **Generate Ed25519 keypair for the domain**
   - Private key → stored in vault (backed up to network)
   - Public key → embedded in owner document

2. **Create owner document (Entry 1)**
   ```json
   { "publicKey": "98daa2aba6513e5c..." }
   ```
   - Upload as public chunk → get `owner_chunk_address`

3. **Create register with shared DNS_REGISTER_KEY**
   ```bash
   ant register create --name mydomain owner_chunk_address --hex
   ```
   - Register address = hash(DNS_REGISTER_KEY + "mydomain")
   - Everyone derives the same address ✅

4. **Create signed records (Entry 2)**
   ```json
   {
     "records": [
       { "type": "ant", "name": ".", "value": "a33082163be512fb..." }
     ],
     "signature": "ed25519_signature_of_records_array"
   }
   ```
   - Sign `JSON.stringify(records)` with Ed25519 private key
   - Upload as public chunk → get `records_chunk_address`

5. **Add records to register**
   ```bash
   ant register edit --name mydomain records_chunk_address --hex
   ```

6. **Backup to network vault**
   - Domain keys auto-sync to Autonomi network
   - Survives app uninstall/device loss

### 2. Lookup Flow

```bash
antns names lookup mydomain.ant
```

**Steps:**

1. **Set shared DNS key**
   ```bash
   printf "055f218d56343b8ff7f4ebf5ba8f137c27a634add32c6174c63fab7df204271a" \
     > ~/.local/share/autonomi/client/register_signing_key
   ```

2. **Fetch register history**
   ```bash
   ant register history --name mydomain --hex
   ```
   Returns: `[chunk_abc, chunk_def, chunk_spam, chunk_ghi, ...]`

3. **Download entry 1 (owner document)**
   ```bash
   ant file download chunk_abc owner.json
   ```
   Extract `publicKey` from JSON

4. **Process entries 2+ in order**
   - For each chunk address:
     - Download chunk
     - Parse JSON → `{ records, signature }`
     - Verify: `ed25519_verify(signature, JSON.stringify(records), publicKey)`
     - If valid: update `lastValidTarget`
     - If invalid: skip (spam)

5. **Return last valid target**

### 3. Update Flow

```bash
antns names update mydomain.ant new_target_address
```

**Steps:**

1. **Load private key from vault**

2. **Create new records**
   ```json
   {
     "records": [
       { "type": "ant", "name": ".", "value": "new_target_address" }
     ],
     "signature": "new_signature"
   }
   ```

3. **Upload as public chunk**

4. **Add to register**
   ```bash
   ant register edit --name mydomain new_chunk_address --hex
   ```

5. **Sync vault (auto-backup)**

### 4. History View

```bash
antns names history mydomain.ant
```

**Output:**

```
Entry 1 (Owner):
  Public Key: 98daa2aba6513e5c...

Entry 2 (2025-12-30 10:15:32) ✅ Valid
  Target: a33082163be512fb471a1cca385332b32c19917deec3989a97e100d827f97baf
  Signature: Valid

Entry 3 (2025-12-30 10:18:45) ❌ Invalid
  Reason: Signature verification failed (spam)

Entry 4 (2025-12-30 11:02:11) ✅ Valid
  Target: b44193274cf623ac582b2ddb496443c43d2aa28eff4ca9ba8ae211e938g08cca
  Signature: Valid

Current Target: b44193274cf623ac... (from Entry 4)
```

---

## DNS Resolver & Proxy

### DNS Resolver (Port 5354)

**Purpose:** Intercept `.ant` domain queries and resolve to localhost

**Flow:**
```
Browser → DNS query for "mysite.ant"
    ↓
AntNS DNS Server (port 5354)
    ↓
Returns: 127.0.0.1
```

**Configuration:**

```bash
# /etc/resolver/ant
nameserver 127.0.0.1
port 5354

# /etc/resolver/autonomi
nameserver 127.0.0.1
port 5354
```

### HTTP Proxy (Port 18888)

**Purpose:** Fetch content from Autonomi and serve to browser

**Flow:**
```
Browser → http://mysite.ant/page.html
    ↓
DNS resolves to 127.0.0.1
    ↓
Proxy on localhost:18888
    ↓
1. Extract domain: "mysite"
2. Lookup target address via AntNS
3. Download content from Autonomi
4. Return to browser
```

**Example:**

```bash
antns start --upstream=http://127.0.0.1:18888/$1

# Browser requests: http://mysite.ant/index.html
# Proxy does:
#   1. antns names lookup mysite.ant → a33082163be512fb...
#   2. ant file download a33082163be512fb... → index.html
#   3. Serve index.html to browser
```

### Domain Resolution Caching

**Purpose:** Reduce network queries and improve response times by caching domain lookups

**Features:**
- Configurable TTL (Time To Live) for cached entries
- Default cache duration: 60 minutes
- Cache can be disabled for testing or real-time requirements
- Cache entries expire automatically after TTL

**How it works:**
1. **First lookup:** Queries Autonomi network, stores result in cache with timestamp
2. **Subsequent lookups:** Returns cached result if less than TTL old
3. **Expired cache:** Re-queries network and updates cache
4. **Cache disabled (ttl=0):** Always queries network

**Usage:**

```bash
# Default 60 minute cache
sudo antns server start

# Custom TTL (10 minutes)
sudo antns server start --ttl=10

# Disable caching
sudo antns server start --ttl=0
```

**Console output:**
- `✓ Cache hit (age: 25s)` - Used cached result
- `Cache expired (age: 3601s)` - Re-querying because cache expired
- `Looking up domain:` - Cache miss, querying network

**Benefits:**
- Faster page loads for frequently accessed domains
- Reduced network bandwidth and cost
- Better user experience with instant DNS resolution

---

## CLI API

### Register Commands

```bash
# Register a domain
antns names register <domain> <target-address>

# Lookup current target
antns names lookup <domain>

# Update target (must own domain)
antns names update <domain> <new-target>

# View full history
antns names history <domain>

# List owned domains
antns names list

# Export private key (backup)
antns names export <domain>

# Import private key (restore)
antns names import <domain> <private-key-hex>
```

### Server Commands

```bash
# Start DNS resolver + HTTP proxy
antns server start [--upstream=URL] [--dns-port=5354] [--proxy-port=18888] [--ttl=60]

# Stop servers
antns server stop

# View status
antns server status
```

**Server start options:**
- `--upstream=URL` - Upstream server for content fetching (default: `http://127.0.0.1:18888/$1`)
- `--dns-port=5354` - Port for DNS resolver (default: 5354)
- `--proxy-port=18888` - Port for HTTP proxy (default: 18888)
- `--ttl=60` - Cache TTL in minutes (default: 60, set to 0 to disable)

### Key Management

```bash
# Backup all domain keys to network
antns keys backup

# Restore keys from network
antns keys restore

# Show backup status
antns keys status
```

---

## Security Model

| Attack Vector     | Protection                                               |
|-------------------|----------------------------------------------------------|
| Domain hijacking  | Ed25519 signatures (256-bit security)                    |
| Spam entries      | Signature verification filters invalid entries           |
| Man-in-the-middle | HTTPS proxy mode (optional)                              |
| Key loss          | Network vault backup (encrypted)                         |
| Replay attacks    | Not applicable (no nonces needed, last valid entry wins) |

**Trust Model:**
- ✅ **Ownership:** Cryptographic (Ed25519)
- ✅ **Discovery:** Trustless (deterministic register address)
- ⚠️ **Availability:** Depends on Autonomi network uptime
- ⚠️ **Content:** No verification (up to domain owner)

---

## Technical Details

### Constants

```rust
DNS_REGISTER_KEY = "055f218d56343b8ff7f4ebf5ba8f137c27a634add32c6174c63fab7df204271a"
```

### Data Structures

**Owner Document:**
```json
{
  "publicKey": "64-char-hex-ed25519-public-key"
}
```

**Signed Records Document:**
```json
{
  "records": [
    {
      "type": "ant",
      "name": ".",
      "value": "64-char-hex-autonomi-address"
    }
  ],
  "signature": "128-char-hex-ed25519-signature"
}
```

### Signature Generation

```javascript
// Consistent JSON encoding (sorted keys)
recordsJSON = JSON.stringify(records, Object.keys(records).sort())
signature = ed25519_sign(privateKey, recordsJSON)
```

### Signature Verification

```javascript
recordsJSON = JSON.stringify(records, Object.keys(records).sort())
isValid = ed25519_verify(publicKey, recordsJSON, signature)
```

### Storage Locations

**macOS:**
```
~/.local/share/autonomi/client/user_data/domain-keys/
  ├─ domain-key-mydomain.txt          # Ed25519 private key (hex)
  └─ domain-meta-mydomain.json        # Metadata
```

**Network Vault:**
```
Register: domain-backup-0x1234567890abcdef-v2
  └─ Points to encrypted backup file containing all domain keys
```

---

## Why This Design?

1. **Discoverability:** Shared register key → everyone finds `mydomain.ant` at the same address
2. **Ownership:** Ed25519 signatures → only key holder can create valid entries
3. **Spam Resistance:** Invalid entries accumulate but are ignored (network immutability)
4. **Decentralization:** No central authority, no DNS servers, no registrars
5. **Permanence:** Autonomi's immutable history = perfect audit trail
6. **Recovery:** Network vault backup survives device loss

---

## Limitations

1. **History Growth:** Register history grows unbounded (Autonomi design choice)
2. **Lookup Cost:** Must download + verify all entries (O(n) where n = # updates)
3. **Spam Possible:** Anyone can add invalid entries (filtered client-side)
4. **No Subdomains:** Current design only supports `domain.ant`, not `sub.domain.ant`
5. **Public Records:** All DNS data is public (design choice for discoverability)

---

## Future Enhancements

- **Subdomain support:** `{ "type": "ant", "name": "www", "value": "..." }`
- **Multiple record types:** A, CNAME, TXT, MX
- **DNSSEC equivalent:** Chain-of-trust signatures
- **Performance:** Cache valid entries, skip spam detection
- **Privacy:** Optional encrypted records (trade-off: not universally resolvable)
