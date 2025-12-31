// Copyright 2025 AntNS Contributors
// Licensed under GPL-3.0

//! OS-specific DNS resolver configuration

use anyhow::{Context, Result};
use std::fs;
use std::process::Command;

/// Check if resolver configuration is set up correctly
pub fn check_resolver_config(port: u16) -> Result<bool> {
    let os = std::env::consts::OS;

    match os {
        "macos" => check_macos_resolver(port),
        "linux" => check_linux_resolver(port),
        "windows" => check_windows_resolver(port),
        _ => {
            tracing::warn!("Unsupported OS for automatic resolver setup: {}", os);
            Ok(false)
        }
    }
}

/// Set up resolver configuration for the current OS
pub fn setup_resolver_config(port: u16) -> Result<()> {
    let os = std::env::consts::OS;

    match os {
        "macos" => setup_macos_resolver(port),
        "linux" => setup_linux_resolver(port),
        "windows" => setup_windows_resolver(port),
        _ => {
            anyhow::bail!("Unsupported OS for automatic resolver setup: {}", os)
        }
    }
}

/// Check macOS resolver configuration
fn check_macos_resolver(port: u16) -> Result<bool> {
    let ant_config = "/etc/resolver/ant";
    let autonomi_config = "/etc/resolver/autonomi";

    let ant_ok = check_resolver_file(ant_config, port)?;
    let autonomi_ok = check_resolver_file(autonomi_config, port)?;

    Ok(ant_ok && autonomi_ok)
}

/// Check if a resolver file exists and has correct content
fn check_resolver_file(path: &str, port: u16) -> Result<bool> {
    if !std::path::Path::new(path).exists() {
        return Ok(false);
    }

    let content = fs::read_to_string(path).context(format!("Failed to read {}", path))?;

    let expected = format!("nameserver 127.0.0.1\nport {}\n", port);

    Ok(content == expected)
}

/// Set up macOS resolver configuration
fn setup_macos_resolver(port: u16) -> Result<()> {
    println!("\nSetting up macOS DNS resolver...");
    println!("This requires sudo access.\n");

    // Create /etc/resolver directory if it doesn't exist
    let resolver_dir = "/etc/resolver";
    if !std::path::Path::new(resolver_dir).exists() {
        println!("Creating {}...", resolver_dir);
        let status = Command::new("sudo")
            .args(["mkdir", "-p", resolver_dir])
            .status()
            .context("Failed to create resolver directory")?;

        if !status.success() {
            anyhow::bail!("Failed to create resolver directory");
        }
    }

    // Create /etc/resolver/ant
    create_resolver_file_sudo("ant", port)?;

    // Create /etc/resolver/autonomi
    create_resolver_file_sudo("autonomi", port)?;

    println!("\n✓ Resolver configuration complete!");
    println!(
        "All .ant and .autonomi domains will now resolve via localhost:{}",
        port
    );

    Ok(())
}

/// Create a resolver file using sudo
fn create_resolver_file_sudo(domain: &str, port: u16) -> Result<()> {
    use std::io::Write;

    let path = format!("/etc/resolver/{}", domain);
    let content = format!("nameserver 127.0.0.1\nport {}\n", port);

    println!("Creating {}...", path);

    let mut child = Command::new("sudo")
        .args(["tee", &path])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .spawn()
        .context("Failed to spawn sudo tee")?;

    child
        .stdin
        .as_mut()
        .context("Failed to get stdin")?
        .write_all(content.as_bytes())
        .context("Failed to write to stdin")?;

    let status = child.wait().context("Failed to wait for sudo tee")?;

    if !status.success() {
        anyhow::bail!("Failed to create resolver file");
    }

    Ok(())
}

/// Check Linux systemd-resolved configuration
fn check_linux_resolver(_port: u16) -> Result<bool> {
    let config_dir = "/etc/systemd/resolved.conf.d";
    let ant_config = format!("{}/ant.conf", config_dir);
    let autonomi_config = format!("{}/autonomi.conf", config_dir);

    if !std::path::Path::new(&ant_config).exists()
        || !std::path::Path::new(&autonomi_config).exists()
    {
        return Ok(false);
    }

    // Check if systemd-resolved is active
    let output = Command::new("systemctl")
        .args(["is-active", "systemd-resolved"])
        .output();

    match output {
        Ok(out) => Ok(out.status.success()),
        Err(_) => Ok(false),
    }
}

/// Set up Linux systemd-resolved configuration
fn setup_linux_resolver(port: u16) -> Result<()> {
    println!("\nSetting up Linux DNS resolver (systemd-resolved)...");
    println!("This requires sudo access.\n");

    // Check if systemd-resolved is running
    let status = Command::new("systemctl")
        .args(["is-active", "systemd-resolved"])
        .status()
        .context("Failed to check systemd-resolved status")?;

    if !status.success() {
        anyhow::bail!("systemd-resolved is not running. Please enable it first:\n  sudo systemctl enable --now systemd-resolved");
    }

    let config_dir = "/etc/systemd/resolved.conf.d";

    // Create directory if it doesn't exist
    if !std::path::Path::new(config_dir).exists() {
        println!("Creating {}...", config_dir);
        let status = Command::new("sudo")
            .args(["mkdir", "-p", config_dir])
            .status()
            .context("Failed to create resolved.conf.d directory")?;

        if !status.success() {
            anyhow::bail!("Failed to create directory");
        }
    }

    // Create ant.conf
    create_systemd_resolved_config("ant", port)?;

    // Create autonomi.conf
    create_systemd_resolved_config("autonomi", port)?;

    // Restart systemd-resolved
    println!("Restarting systemd-resolved...");
    let status = Command::new("sudo")
        .args(["systemctl", "restart", "systemd-resolved"])
        .status()
        .context("Failed to restart systemd-resolved")?;

    if !status.success() {
        anyhow::bail!("Failed to restart systemd-resolved");
    }

    println!("\n✓ Resolver configuration complete!");
    println!(
        "All .ant and .autonomi domains will now resolve via localhost:{}",
        port
    );

    Ok(())
}

/// Create systemd-resolved configuration file
fn create_systemd_resolved_config(domain: &str, port: u16) -> Result<()> {
    let path = format!("/etc/systemd/resolved.conf.d/{}.conf", domain);
    let content = format!("[Resolve]\nDNS=127.0.0.1:{}\nDomains=~{}\n", port, domain);

    println!("Creating {}...", path);

    use std::io::Write;
    let mut child = Command::new("sudo")
        .args(["tee", &path])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .spawn()
        .context("Failed to spawn sudo tee")?;

    child
        .stdin
        .as_mut()
        .context("Failed to get stdin")?
        .write_all(content.as_bytes())
        .context("Failed to write content")?;

    let status = child.wait().context("Failed to wait for sudo tee")?;

    if !status.success() {
        anyhow::bail!("Failed to create config file");
    }

    Ok(())
}

/// Check Windows NRPT configuration
fn check_windows_resolver(_port: u16) -> Result<bool> {
    // Check if NRPT rules exist for .ant and .autonomi
    let output = Command::new("powershell")
        .args(["-Command", "Get-DnsClientNrptRule | Where-Object { $_.Namespace -eq '.ant' -or $_.Namespace -eq '.autonomi' }"])
        .output();

    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            Ok(!stdout.is_empty())
        }
        Err(_) => Ok(false),
    }
}

/// Set up Windows NRPT configuration
fn setup_windows_resolver(port: u16) -> Result<()> {
    println!("\nSetting up Windows DNS resolver (NRPT)...");
    println!("This requires Administrator privileges.\n");

    // Add NRPT rule for .ant
    add_nrpt_rule("ant", port)?;

    // Add NRPT rule for .autonomi
    add_nrpt_rule("autonomi", port)?;

    println!("\n✓ Resolver configuration complete!");
    println!(
        "All .ant and .autonomi domains will now resolve via localhost:{}",
        port
    );

    Ok(())
}

/// Add Windows NRPT rule
fn add_nrpt_rule(domain: &str, port: u16) -> Result<()> {
    let namespace = format!(".{}", domain);
    let nameserver = format!("127.0.0.1:{}", port);

    println!("Adding NRPT rule for {}...", namespace);

    let status = Command::new("powershell")
        .args([
            "-Command",
            &format!(
                "Add-DnsClientNrptRule -Namespace '{}' -NameServers '{}'",
                namespace, nameserver
            ),
        ])
        .status()
        .context("Failed to add NRPT rule")?;

    if !status.success() {
        anyhow::bail!("Failed to add NRPT rule for {}", namespace);
    }

    Ok(())
}
