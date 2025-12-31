```
Internet-Draft                                                M. Wylde
Intended status: Informational                          December 2025
Expires: June 2026


                  ANT-DNS: Autonomi Name System Protocol
                         draft-ant-dns-protocol-01


Abstract

   This document specifies the ANT-DNS protocol, a decentralized domain
   name system for the Autonomi network. ANT-DNS provides human-readable
   .ant domain names with cryptographic ownership verification using
   Ed25519 signatures and a shared register key mechanism.

Status of This Memo

   This Internet-Draft is submitted in full conformance with the
   provisions of BCP 78 and BCP 79.

   Internet-Drafts are working documents of the Internet Engineering
   Task Force (IETF). This document is a protocol specification for
   the Autonomi network ecosystem.

Table of Contents

   1. Introduction
      1.1. Requirements Language
      1.2. Terminology
   2. Protocol Overview
      2.1. Design Goals
      2.2. The Discoverability-Ownership Problem
      2.3. Solution: Shared Key + Signatures
   3. Data Structures
      3.1. Owner Document
      3.2. Signed Records Document
      3.3. DNS Record Format
   4. Cryptographic Operations
      4.1. Ed25519 Signature Generation
      4.2. Signature Verification
      4.3. Register Address Derivation
   5. Protocol Operations
      5.1. Domain Registration
      5.2. Domain Lookup
      5.3. Domain Update
      5.4. History Retrieval
   6. Constants and Parameters
   7. Security Considerations
      7.1. Ownership Security
      7.2. Spam Mitigation
      7.3. Replay Attack Resistance
      7.4. Key Management
   8. IANA Considerations
   9. References
      9.1. Normative References
      9.2. Informative References

1. Introduction

   ANT-DNS is a decentralized naming protocol that maps human-readable
   domain names (e.g., "mydomain.ant") to Autonomi network addresses.
   Unlike traditional DNS, ANT-DNS operates without central authorities,
   registrars, or hierarchical name servers.

   The protocol achieves both universal discoverability (anyone can find
   a domain) and cryptographic ownership (only the key holder can update
   it) through a combination of shared register keys and Ed25519
   digital signatures.

1.1. Requirements Language

   The key words "MUST", "MUST NOT", "REQUIRED", "SHALL", "SHALL NOT",
   "SHOULD", "SHOULD NOT", "RECOMMENDED", "MAY", and "OPTIONAL" in this
   document are to be interpreted as described in RFC 2119.

1.2. Terminology

   Register: An Autonomi network data structure that stores an ordered
   history of chunk addresses.

   Chunk: An immutable data object stored on the Autonomi network,
   addressed by content hash.

   Domain Owner: The entity controlling the Ed25519 private key for a
   registered domain.

   Spam Entry: A register entry with an invalid signature, ignored
   during domain resolution.

2. Protocol Overview

2.1. Design Goals

   The ANT-DNS protocol is designed to achieve:

   1. Universal Discoverability: Any client can locate any domain
      without prior coordination
   2. Cryptographic Ownership: Only the domain owner can create valid
      updates
   3. Spam Resistance: Invalid entries do not affect resolution
   4. Decentralization: No central authority or registrar
   5. Auditability: Complete history preserved on-chain
   6. Recovery: Key backup mechanisms for domain owner

2.2. The Discoverability-Ownership Problem

   Autonomi registers are addressed by:
      register_address = hash(signing_key + register_name)

   This creates a dilemma:
   - Unique keys: Secure ownership, but addresses are unpredictable
   - Shared keys: Predictable addresses, but no ownership control

2.3. Solution: Shared Key + Signatures

   ANT-DNS uses a well-known shared register key to ensure all clients
   derive identical register addresses for a given domain name. Within
   the register, entries are stored as chunk addresses pointing to
   signed documents. Only entries with valid Ed25519 signatures from
   the domain owner's key are considered valid.

   This approach provides:
   - Deterministic addressing via shared key
   - Ownership enforcement via signature verification
   - Spam filtering by rejecting invalid signatures

3. Data Structures

3.1. Owner Document

   The first entry in a domain's register MUST be an Owner Document,
   which establishes the domain's public key.

   Format (JSON):

   {
     "publicKey": "<64-character-hex-ed25519-public-key>"
   }

   Fields:
   - publicKey: REQUIRED. The Ed25519 public key in hexadecimal encoding.

3.2. Signed Records Document

   Subsequent register entries MUST be Signed Records Documents.

   Format (JSON):

   {
     "records": [
       {
         "type": "<record-type>",
         "name": "<record-name>",
         "value": "<record-value>"
       }
     ],
     "signature": "<128-character-hex-ed25519-signature>"
   }

   Fields:
   - records: REQUIRED. Array of DNS record objects.
   - signature: REQUIRED. Ed25519 signature of the canonical JSON
     representation of the records array.

3.3. DNS Record Format

   Each record in the records array has the following fields:

   - type: REQUIRED. Record type (e.g., "ant", "a", "cname").
   - name: REQUIRED. Record name. Root domain is represented as ".".
   - value: REQUIRED. Record value (e.g., Autonomi chunk address for
     "ant" type).

   Example:

   {
     "type": "ant",
     "name": ".",
     "value": "a33082163be512fb471a1cca385332b32c19917deec3989a97e100d827f97baf"
   }

4. Cryptographic Operations

4.1. Ed25519 Signature Generation

   To create a signature for a records array:

   1. Serialize the records array to canonical JSON:
      - Sort object keys alphabetically
      - No whitespace between elements
      - UTF-8 encoding

   2. Compute the Ed25519 signature:
      signature = Ed25519.sign(private_key, canonical_json_bytes)

   3. Encode signature as hexadecimal (128 characters)

   Example canonical JSON:
   [{"name":".","type":"ant","value":"a33082..."}]

4.2. Signature Verification

   To verify a Signed Records Document:

   1. Extract the records array and signature from the document
   2. Serialize records to canonical JSON (as in 4.1)
   3. Verify the signature:
      is_valid = Ed25519.verify(public_key, canonical_json_bytes, signature)
   4. If verification fails, the entry MUST be ignored

4.3. Register Address Derivation

   The register address for a domain is derived as:

   1. Construct a BLS SecretKey from DNS_REGISTER_KEY_HEX
   2. Derive a register-specific key:
      register_key = BLS.derive_key(DNS_REGISTER_KEY, domain_name)
   3. Compute register address:
      register_address = RegisterAddress(register_key.public_key())

   All clients MUST use the same DNS_REGISTER_KEY_HEX constant to ensure
   address determinism.

5. Protocol Operations

5.1. Domain Registration

   To register a new domain:

   1. Generate a new Ed25519 keypair for the domain
   2. Create an Owner Document containing the public key
   3. Upload Owner Document to Autonomi as a public chunk
      -> owner_chunk_address
   4. Create a register with:
      - Name: domain_name
      - Signing key: derived from DNS_REGISTER_KEY
      - Initial value: owner_chunk_address
   5. Create a Signed Records Document with initial DNS records
   6. Sign the records array with the domain's private key
   7. Upload Signed Records Document as a public chunk
      -> records_chunk_address
   8. Append records_chunk_address to the register
   9. Store private key securely (local storage + optional network vault)

5.2. Domain Lookup

   To resolve a domain:

   1. Derive the register address from the domain name
   2. Fetch the complete register history (ordered list of chunk addresses)
   3. Download the first entry (Owner Document)
   4. Extract the public key from the Owner Document
   5. For each subsequent entry in order:
      a. Download the chunk
      b. Parse as Signed Records Document
      c. Verify the signature using the public key
      d. If valid: update current_records with this entry's records
      e. If invalid: skip this entry (spam)
   6. Return the records from the last valid entry

   The last valid entry represents the current state of the domain.

5.3. Domain Update

   To update a domain (owner only):

   1. Load the domain's private key from secure storage
   2. Create a new Signed Records Document with updated records
   3. Sign the records array with the private key
   4. Upload as a public chunk -> new_records_chunk_address
   5. Append new_records_chunk_address to the register
   6. Update backup storage (if using network vault)

   Note: Anyone can write to the register due to the shared key, but
   only entries signed by the owner's key will be considered valid
   during lookup.

5.4. History Retrieval

   To retrieve domain history:

   1. Perform steps 1-4 from Domain Lookup (5.2)
   2. For each entry, record:
      - Entry number
      - Chunk address
      - Validity status (valid signature or invalid)
      - Records (if valid)
      - Timestamp (if available from chunk metadata)
   3. Return the complete history with validation status

6. Constants and Parameters

   DNS_REGISTER_KEY_HEX:
      "055f218d56343b8ff7f4ebf5ba8f137c27a634add32c6174c63fab7df204271a"

   This constant MUST be used by all ANT-DNS implementations to ensure
   register address determinism.

   TLD: ".ant"

   All ANT-DNS domains MUST use the .ant top-level domain suffix.

7. Security Considerations

7.1. Ownership Security

   Domain ownership is secured by Ed25519 digital signatures, providing
   approximately 128 bits of security. Private keys MUST be generated
   using a cryptographically secure random number generator.

   Key compromise would allow an attacker to update the domain, but
   the complete history remains auditable on-chain, allowing detection
   of unauthorized changes.

7.2. Spam Mitigation

   The shared register key allows anyone to write entries to any domain's
   register. However, signature verification ensures that only entries
   signed by the domain owner's key affect resolution.

   Implementations MUST filter invalid signatures during lookup. Spam
   entries increase register size but do not affect security or
   correctness of resolution.

7.3. Replay Attack Resistance

   The protocol does not use nonces or sequence numbers. Instead, the
   "last valid entry wins" principle applies. Replaying an old valid
   entry would add it to the history, but subsequent lookups would still
   return the most recent valid entry.

   Clients MAY implement additional protections such as timestamp
   validation or out-of-band verification if required for their use case.

7.4. Key Management

   Private keys SHOULD be stored encrypted at rest. Implementations
   SHOULD provide mechanisms for key backup, such as:
   - Export to secure external storage
   - Encrypted backup to Autonomi network vault
   - Hardware security module (HSM) integration

   Key loss results in permanent inability to update the domain. Keys
   MUST be backed up securely.

8. IANA Considerations

   This document has no IANA actions. The .ant TLD operates outside the
   traditional DNS hierarchy and is resolved through ANT-DNS
   implementations, not IANA-delegated name servers.

9. References

9.1. Normative References

   [RFC2119]  Bradner, S., "Key words for use in RFCs to Indicate
              Requirement Levels", BCP 14, RFC 2119, March 1997.

   [RFC8032]  Josefsson, S. and I. Liusvaara, "Edwards-Curve Digital
              Signature Algorithm (EdDSA)", RFC 8032, January 2017.

9.2. Informative References

   [AUTONOMI] Autonomi Network, "Autonomi Protocol Specification",
              https://autonomi.com

   [BLS]      Boneh, D., Lynn, B., and H. Shacham, "Short Signatures
              from the Weil Pairing", Journal of Cryptology, 2004.

Author's Address

   Mark Wylde
   Email: mark@wylde.net

Appendix A. Example Domain Registration

   This section provides a complete example of registering the domain
   "example.ant".

   A.1. Generate Ed25519 Keypair

      private_key = <randomly generated 32 bytes>
      public_key = Ed25519.public_key(private_key)

   A.2. Create Owner Document

      {
        "publicKey": "98daa2aba6513e5c72f8c7c8e6e8f8a8b8c8d8e8f8a8b8c8d8e8f8a8b8c8d8e8"
      }

   A.3. Upload Owner Document

      owner_chunk_address = upload_chunk(owner_document_json)

   A.4. Create Register

      register = create_register(
        name: "example.ant",
        signing_key: derive_from(DNS_REGISTER_KEY_HEX, "example.ant"),
        initial_value: owner_chunk_address
      )

   A.5. Create Signed Records

      records = [
        {
          "type": "ant",
          "name": ".",
          "value": "a33082163be512fb471a1cca385332b32c19917deec3989a97e100d827f97baf"
        }
      ]

      canonical_json = '[{"name":".","type":"ant","value":"a33082..."}]'
      signature = Ed25519.sign(private_key, canonical_json)

      signed_document = {
        "records": records,
        "signature": hex(signature)
      }

   A.6. Upload and Append

      records_chunk_address = upload_chunk(signed_document_json)
      register.append(records_chunk_address)

Appendix B. Canonical JSON Serialization

   For signature generation and verification, records MUST be serialized
   to canonical JSON format:

   1. Object keys sorted alphabetically
   2. No whitespace between elements
   3. UTF-8 encoding without BOM
   4. Compact representation (no pretty-printing)

   Example:

   Input:
   [
     {
       "type": "ant",
       "name": ".",
       "value": "abc123"
     }
   ]

   Canonical form:
   [{"name":".","type":"ant","value":"abc123"}]

```
