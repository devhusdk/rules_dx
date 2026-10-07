#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

use clap::Parser;
use quality_result::{decode_validated, proto};

fn opt_number(value: Option<u64>) -> String {
    value
        .map(|v| v.to_string())
        .unwrap_or_else(|| "-".to_owned())
}

#[derive(Parser, Debug)]
#[command(disable_help_flag = true, disable_version_flag = true)]
struct Cli {
    #[arg(value_name = "OUT.pb")]
    input: Option<String>,
    #[arg(long = "help", short = 'h', action = clap::ArgAction::SetTrue)]
    help: bool,
}

fn main() {
    dx_output::init_diagnostics(false);
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(error) => {
            let first = error
                .to_string()
                .lines()
                .next()
                .unwrap_or("invalid arguments")
                .to_owned();
            tracing::error!("{first}");
            tracing::error!("usage: print_result OUT.pb");
            std::process::exit(2);
        }
    };
    if cli.help {
        tracing::error!("usage: print_result OUT.pb");
        std::process::exit(2);
    }
    let path = match cli.input {
        Some(path) => path,
        None => {
            tracing::error!("usage: print_result OUT.pb");
            std::process::exit(2);
        }
    };
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(err) => {
            tracing::error!("print_result: cannot read {path}: {err}");
            std::process::exit(1);
        }
    };
    let result = match decode_validated(&bytes) {
        Ok(result) => result,
        Err(err) => {
            tracing::error!("print_result: invalid result protobuf {path}: {err}");
            std::process::exit(1);
        }
    };
    println!("producer {}", result.producer);
    println!("capability {}", capability_name(result.capability));
    println!("stages {}", result.stages.len());
    for stage in &result.stages {
        println!(
            "stage {} classes={} sources={}",
            stage.tool_id,
            stage.class_ids.join(","),
            stage.source_paths.join(","),
        );
    }
    println!("completed_rounds {}", result.completed_rounds);
    println!("convergence {}", convergence_name(result.convergence));
    print_diagnostics("initial", &result.initial_diagnostics);
    print_diagnostics("terminal", &result.terminal_diagnostics);
    println!("replacements {}", result.replacements.len());
    for file in &result.replacements {
        for edit in &file.edits {
            println!(
                "replacement {} {} {} {:?}",
                file.path,
                edit.start_byte,
                edit.end_byte,
                String::from_utf8_lossy(&edit.replacement),
            );
        }
    }
}
