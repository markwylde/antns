// Copyright 2025 AntNS Contributors
// Licensed under GPL-3.0

//! DNS resolver and HTTP proxy server

pub mod dns;
pub mod http;
pub mod resolver_setup;

pub use dns::run as run_dns;
pub use http::run as run_http;
pub use resolver_setup::{check_resolver_config, setup_resolver_config};
