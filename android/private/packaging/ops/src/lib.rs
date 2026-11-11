#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]
//! The Android packaging operations: adb, APK sets, package checks and release signing.

#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

pub mod adb;
pub mod apk_set;
pub mod check;
pub mod sign;
pub mod util;

/// Runs the operation `args` names and returns its exit code.
pub fn run(args: &[String]) -> u8 {
    let (name, rest) = match args.split_first() {
        Some((name, rest)) => (name.as_str(), rest),
        None => ("", args),
    };
    let (prefix, outcome) = match name {
        "adb" => (adb::PREFIX, adb::run(rest)),
        "apk-set" => (apk_set::PREFIX, apk_set::run(rest)),
        "check" => (check::PREFIX, check::run(rest)),
        "sign-apk" => (sign::PREFIX, sign::run_apk(rest)),
        "sign-aab" => (sign::PREFIX, sign::run_aab(rest)),
        _ => (
            "dx-android-ops",
            Err(util::usage(
                "usage: android_ops <adb|apk-set|check|sign-apk|sign-aab> ...",
            )),
        ),
    };
    match outcome {
        Ok(code) => code,
        Err(failure) => {
            eprintln!("{prefix}: {}", failure.message);
            failure.code
        }
    }
}
