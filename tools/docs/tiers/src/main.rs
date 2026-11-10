//! Usage: tiers MANIFEST STANDARD_OUT STRICT_OUT. Tier paths in the manifest resolve against the working directory.

#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

use std::collections::BTreeMap;
use std::process::ExitCode;

use tiers::{parse_manifest, standard_page, strict_page};

fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [manifest, standard_out, strict_out] = &args[..] else {
        return Err("usage: tiers MANIFEST STANDARD_OUT STRICT_OUT".to_owned());
    };
    let text = std::fs::read_to_string(manifest).map_err(|error| format!("{manifest}: {error}"))?;
    let tiers = parse_manifest(&text)?;
    let mut files = BTreeMap::new();
    for tier in &tiers {
        for path in [&tier.standard_path, &tier.strict_path] {
            let body = std::fs::read_to_string(path).map_err(|error| format!("{path}: {error}"))?;
            files.insert(path.clone(), body);
        }
    }
    std::fs::write(standard_out, standard_page(&tiers, &files)?)
        .map_err(|error| format!("{standard_out}: {error}"))?;
    std::fs::write(strict_out, strict_page(&tiers, &files)?)
        .map_err(|error| format!("{strict_out}: {error}"))
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("tiers: {error}");
            ExitCode::FAILURE
        }
    }
}
