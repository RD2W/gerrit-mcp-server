// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Maxim Krutovercev (RD2W) <mkrutovercev@yandex.ru>

//! Integration tests for CLI argument parsing and version output.

use std::process::Command;

#[test]
fn parse_default_config() {
    let output = Command::new(env!("CARGO_BIN_EXE_gerrit-mcp"))
        .arg("--help")
        .output()
        .expect("failed to run gerrit-mcp --help");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("--config"));
    assert!(stdout.contains("-c"));
    assert!(stdout.contains("config/config.toml"));
}

#[test]
fn parse_custom_config_long() {
    let output = Command::new(env!("CARGO_BIN_EXE_gerrit-mcp"))
        .arg("--config")
        .arg("/etc/gerrit.toml")
        .arg("--help")
        .output()
        .expect("failed to run gerrit-mcp --config --help");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("--config"));
}

#[test]
fn parse_custom_config_short() {
    let output = Command::new(env!("CARGO_BIN_EXE_gerrit-mcp"))
        .arg("-c")
        .arg("/etc/gerrit.toml")
        .arg("--help")
        .output()
        .expect("failed to run gerrit-mcp -c --help");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("-c"));
}

#[test]
fn version_text_has_fields() {
    let output = Command::new(env!("CARGO_BIN_EXE_gerrit-mcp"))
        .arg("--version")
        .output()
        .expect("failed to run gerrit-mcp --version");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("author:"), "version missing author field");
    assert!(stdout.contains("commit:"), "version missing commit field");
    assert!(stdout.contains("built:"), "version missing built field");
    assert!(stdout.contains("target:"), "version missing target field");
}

#[test]
fn version_text_package_name() {
    let output = Command::new(env!("CARGO_BIN_EXE_gerrit-mcp"))
        .arg("--version")
        .output()
        .expect("failed to run gerrit-mcp --version");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("gerrit-mcp"), "version missing package name");
}
