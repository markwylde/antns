// Copyright 2025 AntNS Contributors
// Licensed under GPL-3.0

//! DNS resolver server for .ant and .autonomi domains

use anyhow::{Context, Result};
use hickory_proto::op::{Header, ResponseCode};
use hickory_proto::rr::rdata::A;
use hickory_proto::rr::{Name, RData, Record};
use hickory_server::authority::MessageResponseBuilder;
use hickory_server::server::{Request, RequestHandler, ResponseHandler, ResponseInfo};
use hickory_server::ServerFuture;
use std::net::Ipv4Addr;

/// DNS request handler for .ant and .autonomi domains
#[derive(Clone)]
struct AntDnsHandler;

#[async_trait::async_trait]
impl RequestHandler for AntDnsHandler {
    async fn handle_request<R: ResponseHandler>(
        &self,
        request: &Request,
        mut response_handler: R,
    ) -> ResponseInfo {
        let query = request.query();
        let name = query.name();
        let query_type = query.query_type();

        println!("DNS query: {} {:?}", name, query_type);

        // Check if this is a .ant or .autonomi domain
        // Note: DNS names in the protocol have a trailing dot (e.g., "mark2.ant.")
        let name_str = name.to_string();
        let is_ant_domain = name_str.ends_with(".ant.") || name_str.ends_with(".autonomi.");

        let mut header = Header::response_from_request(request.header());
        header.set_authoritative(true);

        if is_ant_domain {
            // Respond with 127.0.0.1 for .ant/.autonomi domains
            println!("  → Resolving to 127.0.0.1");
            let mut records = Vec::new();

            if query_type == hickory_proto::rr::RecordType::A {
                let rdata = RData::A(A(Ipv4Addr::new(127, 0, 0, 1)));
                let record = Record::from_rdata(Name::from(name.clone()), 300, rdata);
                records.push(record);
            }

            header.set_response_code(ResponseCode::NoError);
            let response = MessageResponseBuilder::from_message_request(request).build(
                header,
                records.iter(),
                &[],
                &[],
                &[],
            );

            match response_handler.send_response(response).await {
                Ok(info) => return info,
                Err(e) => {
                    println!("  ✗ Failed to send DNS response: {}", e);
                    return ResponseInfo::from(header);
                }
            }
        } else {
            // Return NXDOMAIN for non-.ant domains
            println!("  → NXDOMAIN (not a .ant/.autonomi domain)");
            header.set_response_code(ResponseCode::NXDomain);
            let response =
                MessageResponseBuilder::from_message_request(request).build_no_records(header);

            match response_handler.send_response(response).await {
                Ok(info) => return info,
                Err(e) => {
                    println!("  ✗ Failed to send DNS response: {}", e);
                    return ResponseInfo::from(header);
                }
            }
        }
    }
}

/// Start the DNS server on the specified port
pub async fn run(port: u16) -> Result<()> {
    let addr = format!("127.0.0.1:{}", port);

    println!("DNS server starting on {}", addr);

    let handler = AntDnsHandler;
    let mut server = ServerFuture::new(handler);

    server.register_socket(
        tokio::net::UdpSocket::bind(&addr)
            .await
            .context("Failed to bind DNS UDP socket")?,
    );

    server.register_listener(
        tokio::net::TcpListener::bind(&addr)
            .await
            .context("Failed to bind DNS TCP socket")?,
        std::time::Duration::from_secs(5),
    );

    println!("✓ DNS server listening on {}\n", addr);

    server
        .block_until_done()
        .await
        .context("DNS server error")?;

    Ok(())
}
