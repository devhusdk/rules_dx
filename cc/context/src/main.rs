//! Emits one clang-tidy consumption bundle from a Bazel aquery record.
#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::process::ExitCode;

use cc_context::{self, Error};

const USAGE: &str = "Usage: compile_db --aquery <path|-> --execroot <dir> --label <label> \
--config <policy path> --output <dir> [--require-source <path>]...";

fn main() -> ExitCode {
    match run(std::env::args().skip(1).collect()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("{err}");
            ExitCode::FAILURE
        }
    }
}

fn run(args: Vec<String>) -> Result<(), Error> {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!("{USAGE}");
        return Ok(());
    }
    let mut named: BTreeSet<&str> = BTreeSet::new();
    let mut single: Vec<(&str, String)> = Vec::new();
    let mut sources: Vec<String> = Vec::new();
    let mut index = 0;
    while index < args.len() {
        let flag = args[index].as_str();
        if !flag.starts_with("--") {
            return Err(Error::Usage(format!("unexpected argument '{flag}'")));
        }
        let value = args
            .get(index + 1)
            .ok_or_else(|| Error::Usage(format!("'{flag}' needs a value")))?;
        if flag == "--require-source" {
            sources.push(value.clone());
        } else if named.insert(flag) {
            single.push((flag, value.clone()));
        } else {
            return Err(Error::Usage(format!("'{flag}' is repeated")));
        }
        index += 2;
    }
    let wanted: BTreeSet<&str> =
        BTreeSet::from(["--aquery", "--execroot", "--label", "--config", "--output"]);
    for (flag, _) in single.iter() {
        if !wanted.contains(flag) {
            return Err(Error::Usage(format!("unknown flag '{flag}'")));
        }
    }
    let value = |name: &str| -> Result<String, Error> {
        single
            .iter()
            .find(|(flag, _)| *flag == name)
            .map(|(_, value)| value.clone())
            .ok_or_else(|| Error::Usage(format!("'{name}' is required")))
    };
    let aquery = value("--aquery")?;
    let label = value("--label")?;
    let policy_path = PathBuf::from(value("--config")?);
    let output = PathBuf::from(value("--output")?);
    let execroot = PathBuf::from(value("--execroot")?);
    let policy = cc_context::read_policy(&policy_path)?;
    let graph = cc_context::AqueryGraph::parse(&cc_context::read_input(&aquery)?)?;
    let commands = graph.compile_commands(&execroot)?;
    let entries = cc_context::database(&commands, &execroot, &label, &sources)?;
    cc_context::write_bundle(&output, &entries, &policy)?;
    println!(
        "compilation_db: {} compile command(s) for {label} in {}",
        entries.len(),
        output.display()
    );
    Ok(())
}
