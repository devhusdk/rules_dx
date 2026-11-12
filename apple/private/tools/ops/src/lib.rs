#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]
//! The Apple operations: simulators, test simulators, running a macOS app, export, signing and notarization.

#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

pub mod app_check;
pub mod codesign;
pub mod export;
pub mod library_check;
pub mod notarize;
pub mod probe;
pub mod run_macos;
pub mod signing;
pub mod simctl;
pub mod swift;
pub mod test_sim;
pub mod util;
pub mod xcode;

use std::path::PathBuf;

use util::{Failure, Outcome, System, process_env};
use xcode::Tools;

fn report(prefix: &str, outcome: Outcome) -> u8 {
    match outcome {
        Ok(code) => code,
        Err(Failure { code, message }) => {
            eprintln!("{prefix}: {message}");
            code
        }
    }
}

/// Runs the operation `args` names and returns its exit code.
pub fn run(args: &[String]) -> u8 {
    let (name, rest) = match args.split_first() {
        Some((name, rest)) => (name.as_str(), rest),
        None => ("", args),
    };
    match name {
        "simctl" => report(
            simctl::PREFIX,
            xcode::for_launcher(&process_env).and_then(|developer_dir| {
                let tools = Tools {
                    developer_dir: Some(developer_dir),
                    runner: &System,
                };
                simctl::run(&tools, rest, &process_env)
            }),
        ),
        "swift" | "llvm-cov" | "llvm-profdata" => report(
            swift::PREFIX,
            xcode::for_launcher(&process_env).and_then(|developer_dir| {
                let tools = Tools {
                    developer_dir: Some(developer_dir),
                    runner: &System,
                };
                swift::run(&tools, name, rest)
            }),
        ),
        "export" | "codesign" | "notarize" => report(
            match name {
                "export" => export::PREFIX,
                "codesign" => codesign::PREFIX,
                _ => notarize::PREFIX,
            },
            xcode::for_launcher(&process_env).and_then(|developer_dir| {
                let tools = Tools {
                    developer_dir: Some(developer_dir),
                    runner: &System,
                };
                match name {
                    "export" => export::run(&tools, rest, &process_env),
                    "codesign" => codesign::run(&tools, rest, &process_env),
                    _ => notarize::run(&tools, rest, &process_env),
                }
            }),
        ),
        "run-macos-app" => {
            let tools = Tools {
                developer_dir: None,
                runner: &System,
            };
            report(
                run_macos::PREFIX,
                run_macos::run(&tools, rest, &process_env),
            )
        }
        "probe" => {
            let tools = Tools {
                developer_dir: process_env("DEVELOPER_DIR").map(PathBuf::from),
                runner: &System,
            };
            report(
                probe::PREFIX,
                probe::run(&tools, &probe::executor(), rest, &process_env),
            )
        }
        "library-check" => {
            let tools = Tools {
                developer_dir: process_env("DEVELOPER_DIR").map(PathBuf::from),
                runner: &System,
            };
            report(
                rest.first().map_or("library-check", String::as_str),
                library_check::run(&tools, rest),
            )
        }
        "macos-app-check" => {
            let tools = Tools {
                developer_dir: process_env("DEVELOPER_DIR").map(PathBuf::from),
                runner: &System,
            };
            report(app_check::PREFIX, app_check::run(&tools, rest))
        }
        _ => report(
            "dx-apple-ops",
            Err(util::usage(
                "usage: apple_ops <simctl|swift|run-macos-app|export|codesign|notarize|probe|library-check|macos-app-check> ...",
            )),
        ),
    }
}

/// The process group of this process.
fn own_group() -> i32 {
    // SAFETY: getpgrp has no arguments and cannot fail.
    unsafe { libc::getpgrp() }
}

/// Whether the process group `group` still has a process.
fn group_alive(group: i32) -> bool {
    // SAFETY: kill with signal 0 only checks that the group exists.
    let rc = unsafe { libc::kill(-group, 0) };
    rc == 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}

/// Creates one test simulator and prints its UDID.
pub fn create_test_simulator() -> u8 {
    report(
        test_sim::PREFIX,
        xcode::for_test_runner(&process_env).and_then(|developer_dir| {
            let tools = Tools {
                developer_dir: Some(developer_dir),
                runner: &System,
            };
            let udid = test_sim::create(&tools, &process_env, own_group(), &group_alive)?;
            println!("{udid}");
            Ok(0)
        }),
    )
}

/// Deletes the test simulator `SIMULATOR_UDID` names.
pub fn delete_test_simulator() -> u8 {
    report(
        test_sim::PREFIX,
        xcode::for_test_runner(&process_env).and_then(|developer_dir| {
            let tools = Tools {
                developer_dir: Some(developer_dir),
                runner: &System,
            };
            test_sim::delete(&tools, &process_env)
        }),
    )
}
