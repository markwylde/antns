// Copyright 2025 AntNS Contributors
// Licensed under GPL-3.0

//! Chunk storage operations for uploading and downloading JSON documents

use autonomi::{Client, AttoTokens};
use autonomi::client::payment::PaymentOption;
use autonomi::data::DataAddress;
use bytes::Bytes;
use serde::{Deserialize, Serialize};
use xor_name::XorName;
use anyhow::{Context, Result};

/// Upload a document as a public chunk and return its address
pub async fn upload_document_as_chunk<T: Serialize>(
    client: &Client,
    document: &T,
    payment: PaymentOption,
) -> Result<(AttoTokens, [u8; 32])> {
    // Serialize to JSON
    let json = serde_json::to_vec(document)
        .context("Failed to serialize document to JSON")?;

    // Upload as public data
    let (cost, addr) = client
        .data_put_public(Bytes::from(json), payment)
        .await
        .context("Failed to upload chunk to network")?;

    // Extract 32-byte XorName from DataAddress
    let xorname = addr.xorname().0;

    Ok((cost, xorname))
}

/// Download a document from a data address
pub async fn download_document_from_chunk<T: for<'de> Deserialize<'de>>(
    client: &Client,
    chunk_addr: [u8; 32],
) -> Result<T> {
    // Create DataAddress from XorName
    let data_addr = DataAddress::new(XorName(chunk_addr));

    // Download public data
    let data_bytes = client
        .data_get_public(&data_addr)
        .await
        .context("Failed to download data from network")?;

    // Deserialize from JSON
    let document = serde_json::from_slice(&data_bytes)
        .context("Failed to deserialize data as JSON")?;

    Ok(document)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    struct TestDocument {
        name: String,
        value: u32,
    }

    // Note: These tests require a running Autonomi network
    // They are marked as ignored and should be run with --ignored flag

    #[tokio::test]
    #[ignore]
    async fn test_upload_and_download() {
        let client = Client::init().await.unwrap();
        // let wallet = get_test_wallet(); // TODO: Add wallet initialization

        let doc = TestDocument {
            name: "test".to_string(),
            value: 42,
        };

        // Upload
        // let (_, addr) = upload_document_as_chunk(&client, &doc, wallet).await.unwrap();

        // Download
        // let downloaded: TestDocument = download_document_from_chunk(&client, addr).await.unwrap();

        // assert_eq!(doc, downloaded);
    }
}
