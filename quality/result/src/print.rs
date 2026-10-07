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
use quality_result::{decode_validated, print_text};

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
    print!("{}", print_text(&result));
}
