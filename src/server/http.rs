// Copyright 2025 AntNS Contributors
// Licensed under GPL-3.0

//! HTTP proxy server for .ant and .autonomi domains

use anyhow::{Context, Result};
use autonomi::Client;
use bytes::Bytes;
use http_body_util::Full;
use hyper::body::Incoming;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::net::TcpListener;
use tokio::sync::Mutex;

/// Cached domain lookup result
#[derive(Clone)]
struct CachedLookup {
    target: String,
    timestamp: SystemTime,
}

/// HTTP proxy service state
struct ProxyState {
    client: Client,
    upstream_template: String,
    cache: Mutex<HashMap<String, CachedLookup>>,
    cache_ttl: Duration,
}

/// Handle an HTTP request
async fn handle_request(
    state: Arc<ProxyState>,
    req: Request<Incoming>,
) -> Result<Response<Full<Bytes>>, hyper::Error> {
    let host = req
        .headers()
        .get("host")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("");

    println!(
        "\nHTTP request: {} {} {}",
        req.method(),
        host,
        req.uri().path()
    );

    // Extract domain from Host header
    let domain = host.split(':').next().unwrap_or(host);

    // Check if this is a .ant or .autonomi domain
    if !domain.ends_with(".ant") && !domain.ends_with(".autonomi") {
        println!("  ✗ Not a .ant or .autonomi domain");
        return Ok(Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body(Full::new(Bytes::from(
                "Only .ant and .autonomi domains are supported",
            )))
            .unwrap());
    }

    // Check cache first
    let target = if state.cache_ttl.as_secs() > 0 {
        let cache = state.cache.lock().await;
        if let Some(cached) = cache.get(domain) {
            let age = SystemTime::now()
                .duration_since(cached.timestamp)
                .unwrap_or(Duration::MAX);
            if age < state.cache_ttl {
                println!("  ✓ Cache hit (age: {}s)", age.as_secs());
                cached.target.clone()
            } else {
                println!("  Cache expired (age: {}s)", age.as_secs());
                drop(cache);
                match lookup_and_cache(&state, domain).await {
                    Ok(target) => target,
                    Err(resp) => return Ok(resp),
                }
            }
        } else {
            drop(cache);
            match lookup_and_cache(&state, domain).await {
                Ok(target) => target,
                Err(resp) => return Ok(resp),
            }
        }
    } else {
        // Caching disabled
        match lookup_domain_no_cache(&state, domain).await {
            Ok(target) => target,
            Err(resp) => return Ok(resp),
        }
    };

    // Build upstream URL by replacing $ADDRESS with the target
    let upstream_url = state.upstream_template.replace("$ADDRESS", &target);
    let path = req.uri().path();
    let query = req
        .uri()
        .query()
        .map(|q| format!("?{}", q))
        .unwrap_or_default();
    let full_upstream_url = format!("{}{}{}", upstream_url, path, query);

    println!("  Proxying to: {}", full_upstream_url);

    // Parse upstream URL
    let upstream_uri = match full_upstream_url.parse::<hyper::Uri>() {
        Ok(uri) => uri,
        Err(e) => {
            tracing::error!("Invalid upstream URL: {}", e);
            return Ok(Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Full::new(Bytes::from(format!(
                    "Invalid upstream URL: {}",
                    e
                ))))
                .unwrap());
        }
    };

    // Create HTTP client
    use hyper_util::client::legacy::Client;
    use hyper_util::rt::TokioExecutor;
    let client = Client::builder(TokioExecutor::new()).build_http();

    // Make request to upstream
    let mut upstream_req = Request::builder()
        .method(req.method())
        .uri(upstream_uri)
        .body(http_body_util::Empty::<Bytes>::new())
        .unwrap();

    // Copy headers from original request
    *upstream_req.headers_mut() = req.headers().clone();

    match client.request(upstream_req).await {
        Ok(upstream_resp) => {
            let status = upstream_resp.status();
            let headers = upstream_resp.headers().clone();
            println!("  ✓ Upstream responded: {}", status);

            // Collect body
            use http_body_util::BodyExt;
            let body_bytes = match upstream_resp.collect().await {
                Ok(collected) => {
                    let bytes = collected.to_bytes();
                    println!("  ✓ Received {} bytes", bytes.len());
                    bytes
                }
                Err(e) => {
                    println!("  ✗ Failed to read upstream response body: {}", e);
                    return Ok(Response::builder()
                        .status(StatusCode::BAD_GATEWAY)
                        .body(Full::new(Bytes::from("Failed to read upstream response")))
                        .unwrap());
                }
            };

            // Build response
            let mut response = Response::builder().status(status);

            // Copy headers from upstream response
            for (name, value) in headers.iter() {
                response = response.header(name, value);
            }

            // Add custom headers
            response = response
                .header("X-AntNS-Domain", domain)
                .header("X-AntNS-Target", &target)
                .header("X-AntNS-Upstream", &full_upstream_url);

            let resp = response.body(Full::new(body_bytes)).unwrap();
            println!("  ✓ Response sent to client");
            Ok(resp)
        }
        Err(e) => {
            println!("  ✗ Failed to proxy to upstream: {}", e);
            Ok(Response::builder()
                .status(StatusCode::BAD_GATEWAY)
                .body(Full::new(Bytes::from(format!(
                    "Failed to proxy to upstream: {}",
                    e
                ))))
                .unwrap())
        }
    }
}

/// Lookup domain and cache the result
async fn lookup_and_cache(
    state: &ProxyState,
    domain: &str,
) -> Result<String, Response<Full<Bytes>>> {
    println!("  Looking up domain: {}", domain);
    match crate::lookup_domain(&state.client, domain).await {
        Ok(resolution) => {
            println!("  ✓ Resolved to: {}", resolution.target);
            let target = resolution.target.clone();

            // Store in cache
            let mut cache = state.cache.lock().await;
            cache.insert(
                domain.to_string(),
                CachedLookup {
                    target: target.clone(),
                    timestamp: SystemTime::now(),
                },
            );

            Ok(target)
        }
        Err(e) => {
            println!("  ✗ Lookup failed: {}", e);
            Err(Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(Full::new(Bytes::from(format!(
                    "Domain not found: {}",
                    domain
                ))))
                .unwrap())
        }
    }
}

/// Lookup domain without caching
async fn lookup_domain_no_cache(
    state: &ProxyState,
    domain: &str,
) -> Result<String, Response<Full<Bytes>>> {
    println!("  Looking up domain: {}", domain);
    match crate::lookup_domain(&state.client, domain).await {
        Ok(resolution) => {
            println!("  ✓ Resolved to: {}", resolution.target);
            Ok(resolution.target)
        }
        Err(e) => {
            println!("  ✗ Lookup failed: {}", e);
            Err(Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(Full::new(Bytes::from(format!(
                    "Domain not found: {}",
                    domain
                ))))
                .unwrap())
        }
    }
}

/// Start the HTTP proxy server on the specified port
pub async fn run(port: u16, upstream_template: String, cache_ttl_minutes: u64) -> Result<()> {
    let addr = format!("127.0.0.1:{}", port);

    println!("HTTP proxy starting on {}", addr);
    println!("Upstream template: {}", upstream_template);

    let cache_ttl = Duration::from_secs(cache_ttl_minutes * 60);
    if cache_ttl_minutes > 0 {
        println!("Cache TTL: {} minutes", cache_ttl_minutes);
    } else {
        println!("Cache: disabled");
    }

    println!("Initializing Autonomi client...");

    // Initialize Autonomi client
    let client = Client::init()
        .await
        .context("Failed to initialize Autonomi client")?;

    println!("✓ Autonomi client initialized");

    let state = Arc::new(ProxyState {
        client,
        upstream_template,
        cache: Mutex::new(HashMap::new()),
        cache_ttl,
    });

    let listener = TcpListener::bind(&addr)
        .await
        .context("Failed to bind HTTP proxy socket")?;

    println!("✓ HTTP proxy listening on http://{}\n", addr);

    loop {
        let (stream, remote_addr) = match listener.accept().await {
            Ok(conn) => conn,
            Err(e) => {
                tracing::error!("Failed to accept connection: {}", e);
                continue;
            }
        };

        let state = state.clone();

        tokio::spawn(async move {
            let service = service_fn(move |req| {
                let state = state.clone();
                handle_request(state, req)
            });

            let io = TokioIo::new(stream);

            if let Err(e) = http1::Builder::new().serve_connection(io, service).await {
                tracing::error!("Connection error from {}: {}", remote_addr, e);
            }
        });
    }
}
