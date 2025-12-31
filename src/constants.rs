// Copyright 2025 AntNS Contributors
// Licensed under GPL-3.0

/// The shared DNS register key used for deriving deterministic register addresses
/// This allows anyone to discover domains at the same network address
pub const DNS_REGISTER_KEY_HEX: &str =
    "3c2ad130b7863b34b17cf11b474fff302522f427e8818a3543d105d91cdb384c";

/// Default DNS resolver port
pub const DNS_PORT: u16 = 5354;

/// Default HTTP proxy port
pub const HTTP_PROXY_PORT: u16 = 18888;

/// Domain suffix for AntNS domains
pub const DOMAIN_SUFFIX: &str = ".ant";

/// Alternative domain suffix
pub const DOMAIN_SUFFIX_ALT: &str = ".autonomi";
