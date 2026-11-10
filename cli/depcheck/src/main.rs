#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

use clap::{Parser, Subcommand};
use std::ffi::OsString;
use std::path::PathBuf;

use dx_ecosystems::Language;
use dx_error::DxError;

// LCOV_EXCL_START - reason: clap schema, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
#[derive(Parser)]
#[command(name = "depcheck")]
struct Cli {
    #[command(subcommand)]
    cmd: Command,
}

#[derive(Subcommand)]
#[allow(clippy::large_enum_variant)]
enum Command {
    Consistency {
        #[arg(long, value_parser = dx_depcheck::language_named)]
        ecosystem: Language,
        #[arg(long)]
        manifest: PathBuf,
        #[arg(long)]
        lock: PathBuf,
    },
    Usage {
        #[arg(long, value_parser = dx_depcheck::language_named)]
        ecosystem: Language,
        #[arg(long)]
        manifest: PathBuf,
        #[arg(long)]
        sources: PathBuf,
    },
    Locks {
        #[arg(long)]
        cargo_manifest: PathBuf,
        #[arg(long)]
        cargo_lock: PathBuf,
        #[arg(long)]
        uv_manifest: PathBuf,
        #[arg(long)]
        uv_lock: PathBuf,
        #[arg(long)]
        pnpm_manifest: PathBuf,
        #[arg(long)]
        pnpm_lock: PathBuf,
        #[arg(long)]
        go_manifest: PathBuf,
        #[arg(long)]
        go_lock: PathBuf,
        #[arg(long)]
        maven_artifacts: PathBuf,
        #[arg(long)]
        maven_lock: PathBuf,
        #[arg(long)]
        paket_manifest: PathBuf,
        #[arg(long)]
        paket_lock: PathBuf,
        #[arg(long)]
        ruby_manifest: PathBuf,
        #[arg(long)]
        ruby_lock: PathBuf,
    },
}
// LCOV_EXCL_STOP - reason: end clap schema, issue: 1055, policy: docs/cli/commands/build-test-coverage.md

fn emit(out: &str, err: &str) {
    if !out.is_empty() {
        print!("{out}");
    }
    if !err.is_empty() {
        eprint!("{err}");
    }
}

fn run_from<I, T>(args: I) -> Result<(), DxError>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    match Cli::parse_from(args).cmd {
        Command::Consistency {
            ecosystem,
            manifest,
            lock,
        } => {
            let mut out = String::new();
            let mut err = String::new();
            let result =
                dx_depcheck::cmd_consistency(ecosystem, &manifest, &lock, &mut out, &mut err);
            emit(&out, &err);
            result
        }
        Command::Usage {
            ecosystem,
            manifest,
            sources,
        } => {
            let mut out = String::new();
            let mut err = String::new();
            let result = dx_depcheck::cmd_usage(ecosystem, &manifest, &sources, &mut out, &mut err);
            emit(&out, &err);
            result
        }
        Command::Locks {
            cargo_manifest,
            cargo_lock,
            uv_manifest,
            uv_lock,
            pnpm_manifest,
            pnpm_lock,
            go_manifest,
            go_lock,
            maven_artifacts,
            maven_lock,
            paket_manifest,
            paket_lock,
            ruby_manifest,
            ruby_lock,
        } => {
            let mut out = String::new();
            let mut err = String::new();
            let result = dx_depcheck::cmd_locks(
                &dx_depcheck::WorkspaceLocks {
                    cargo_manifest: &cargo_manifest,
                    cargo_lock: &cargo_lock,
                    uv_manifest: &uv_manifest,
                    uv_lock: &uv_lock,
                    pnpm_manifest: &pnpm_manifest,
                    pnpm_lock: &pnpm_lock,
                    go_manifest: &go_manifest,
                    go_lock: &go_lock,
                    maven_artifacts: &maven_artifacts,
                    maven_lock: &maven_lock,
                    paket_manifest: &paket_manifest,
                    paket_lock: &paket_lock,
                    ruby_manifest: &ruby_manifest,
                    ruby_lock: &ruby_lock,
                },
                &mut out,
                &mut err,
            );
            emit(&out, &err);
            result
        }
    }
}

// LCOV_EXCL_START - reason: process shim, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
fn main() -> std::process::ExitCode {
    match run_from(std::env::args_os()) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => error.into(),
    }
}
// LCOV_EXCL_STOP - reason: end process shim, issue: 1055, policy: docs/cli/commands/build-test-coverage.md

#[cfg(test)]
mod tests {
    use super::*;
    use dx_error::ErrorKind;

    const ECOSYSTEMS: [&str; 26] = [
        "rust",
        "python",
        "js",
        "ts",
        "go",
        "java",
        "kotlin",
        "scala",
        "csharp",
        "fsharp",
        "cc",
        "ruby",
        "perl",
        "zig",
        "julia",
        "dart",
        "php",
        "elixir",
        "erlang",
        "fortran",
        "d",
        "haskell",
        "swift",
        "powershell",
        "lua",
        "ocaml",
    ];

    fn lock_args(name: &str) -> Vec<String> {
        let mut args = vec!["depcheck".to_owned(), "locks".to_owned()];
        for flag in [
            "cargo-manifest",
            "cargo-lock",
            "uv-manifest",
            "uv-lock",
            "pnpm-manifest",
            "pnpm-lock",
            "go-manifest",
            "go-lock",
            "maven-artifacts",
            "maven-lock",
            "paket-manifest",
            "paket-lock",
            "ruby-manifest",
            "ruby-lock",
        ] {
            args.push(format!("--{flag}"));
            args.push(format!("/nonexistent/{name}"));
        }
        args
    }

    #[test]
    fn every_documented_ecosystem_name_round_trips() {
        for name in ECOSYSTEMS {
            let eco = Language::parse(name).unwrap_or_else(|| panic!("{name} must parse"));
            assert_eq!(eco.name(), name);
        }
    }

    #[test]
    fn clap_rejects_an_unknown_ecosystem_once_and_without_a_repeated_sentence() {
        let err = Cli::try_parse_from([
            "depcheck",
            "consistency",
            "--ecosystem=bogus",
            "--manifest=/m",
            "--lock=/l",
        ])
        .err()
        .expect("unknown ecosystem must be rejected");
        let text = err.to_string();
        assert_eq!(
            text.matches("invalid value").count(),
            1,
            "clap already prefixes the flag, so the parser must not repeat it: {text}"
        );
        assert!(
            text.contains("unknown ecosystem: bogus"),
            "the parser must name the offender: {text}"
        );
    }

    #[test]
    fn consistency_dispatch_reports_a_missing_manifest_instead_of_a_usage_error() {
        let err = run_from([
            "depcheck",
            "consistency",
            "--ecosystem=rust",
            "--manifest=/nonexistent/Cargo.toml",
            "--lock=/nonexistent/Cargo.lock",
        ])
        .expect_err("missing manifest");
        assert_eq!(err.kind(), ErrorKind::Usage);
    }

    #[test]
    fn locks_dispatch_reports_the_first_unreadable_lock() {
        let err = run_from(lock_args("missing")).expect_err("missing locks");
        assert_eq!(err.kind(), ErrorKind::Usage);
    }

    #[test]
    fn a_consistent_pair_prints_the_ok_line_and_exits_zero() {
        let dir = tempfile::tempdir().expect("scratch");
        let manifest = dir.path().join("Cargo.toml");
        std::fs::write(
            &manifest,
            "[package]\nname = \"hello\"\nversion = \"0.0.0\"\nedition = \"2021\"\n\n\
             [dependencies]\nanyhow = \"1\"\n",
        )
        .expect("manifest");
        let lock = dir.path().join("Cargo.lock");
        std::fs::write(
            &lock,
            "version = 4\n\n[[package]]\nname = \"hello\"\nversion = \"0.0.0\"\n\n\
             [[package]]\nname = \"anyhow\"\nversion = \"1.0.0\"\n\
             source = \"registry+https://github.com/rust-lang/crates.io-index\"\n\
             checksum = \"fixture\"\n",
        )
        .expect("lock");
        let result = run_from([
            "depcheck".to_owned(),
            "consistency".to_owned(),
            format!("--manifest={}", manifest.display()),
            format!("--lock={}", lock.display()),
            "--ecosystem=rust".to_owned(),
        ]);
        assert_eq!(result, Ok(()));
    }
}
