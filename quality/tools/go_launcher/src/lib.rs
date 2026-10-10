//! Runs a staged Go-toolchain tool with a hermetic SDK from the scratch tree.
//!
//! The quality runner stages the tool binary at a fixed scratch-relative path
//! and the Go SDK tree beside it. The launcher derives every path from TMPDIR,
//! which the runner always points at the action scratch root, so no host PATH,
//! HOME, or toolchain install ever leaks into the tool action.

#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// Scratch-relative directory holding the staged Go SDK tree.
pub const GOROOT_REL: &str = "dx-goroot";

/// Scratch-relative path of the staged staticcheck binary without suffix.
pub const TOOL_REL: &str = "dx-staticcheck/staticcheck";

/// Scratch-relative Go build cache directory created per action.
pub const GOCACHE_REL: &str = "dx-gocache";

/// Scratch-relative HOME directory created per action.
pub const GOHOME_REL: &str = "dx-home";

/// Scratch-relative GOPATH directory created per action.
pub const GOPATH_REL: &str = "dx-gopath";

/// The fully resolved child command plus the environment it runs with.
#[derive(Debug)]
pub struct Plan {
    pub tool: PathBuf,
    pub args: Vec<OsString>,
    pub env_set: Vec<(OsString, OsString)>,
}

fn missing(what: impl std::fmt::Display) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::NotFound, format!("go_launcher: {what}"))
}

/// Returns the tool file name for this host.
pub fn tool_file_name() -> &'static str {
    if cfg!(windows) {
        "staticcheck.exe"
    } else {
        "staticcheck"
    }
}

/// Returns the go binary file name for this host.
pub fn go_file_name() -> &'static str {
    if cfg!(windows) {
        "go.exe"
    } else {
        "go"
    }
}

/// Resolves the launch from the current environment.
pub fn plan(args: &[OsString]) -> std::io::Result<Plan> {
    let scratch = std::env::var_os("TMPDIR")
        .map(PathBuf::from)
        .filter(|path| !path.as_os_str().is_empty())
        .ok_or_else(|| missing("TMPDIR must name the action scratch root"))?;
    plan_in(&scratch, args)
}

/// Resolves one launch as if TMPDIR named `scratch`.
pub fn plan_in(scratch: &Path, args: &[OsString]) -> std::io::Result<Plan> {
    let goroot = scratch.join(GOROOT_REL);
    if !goroot.is_dir() {
        return Err(missing(format!(
            "Go SDK tree is missing: {}",
            goroot.display()
        )));
    }
    let go_bin = goroot.join("bin").join(go_file_name());
    if !go_bin.is_file() {
        return Err(missing(format!(
            "Go binary is missing: {}",
            go_bin.display()
        )));
    }
    let tool = scratch.join("dx-staticcheck").join(tool_file_name());
    if !tool.is_file() {
        return Err(missing(format!(
            "staticcheck binary is missing: {}",
            tool.display()
        )));
    }
    let gocache = scratch.join(GOCACHE_REL);
    let home = scratch.join(GOHOME_REL);
    let gopath = scratch.join(GOPATH_REL);
    for dir in [&gocache, &home, &gopath] {
        std::fs::create_dir_all(dir)?;
    }
    let path = goroot.join("bin").as_os_str().to_owned();
    let env_set = vec![
        (OsString::from("GOROOT"), goroot.as_os_str().to_owned()),
        (OsString::from("PATH"), path),
        (OsString::from("GOCACHE"), gocache.as_os_str().to_owned()),
        (OsString::from("HOME"), home.as_os_str().to_owned()),
        (OsString::from("GOPATH"), gopath.as_os_str().to_owned()),
        (OsString::from("GOPROXY"), OsString::from("off")),
        (OsString::from("GOTOOLCHAIN"), OsString::from("local")),
    ];
    Ok(Plan {
        tool,
        args: args.to_vec(),
        env_set,
    })
}

#[cfg(test)]
mod lib_tests;
