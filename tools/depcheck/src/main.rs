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

use dx_depcheck::Ecosystem;

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
        #[arg(long)]
        ecosystem: Ecosystem,
        #[arg(long)]
        manifest: PathBuf,
        #[arg(long)]
        lock: PathBuf,
    },
    Usage {
        #[arg(long)]
        ecosystem: Ecosystem,
        #[arg(long)]
        manifest: PathBuf,
        #[arg(long)]
        sources: PathBuf,
        #[arg(long)]
        exceptions: Option<PathBuf>,
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

fn run_from<I, T>(args: I) -> i32
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
            let code =
                dx_depcheck::cmd_consistency(ecosystem, &manifest, &lock, &mut out, &mut err);
            emit(&out, &err);
            code
        }
        Command::Usage {
            ecosystem,
            manifest,
            sources,
            exceptions,
        } => {
            let mut out = String::new();
            let mut err = String::new();
            let code = dx_depcheck::cmd_usage(
                ecosystem,
                &manifest,
                &sources,
                exceptions.as_deref(),
                &mut out,
                &mut err,
            );
            emit(&out, &err);
            code
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
            let code = dx_depcheck::cmd_locks(
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
            code
        }
    }
}

// LCOV_EXCL_START - reason: process shim, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
fn run() -> i32 {
    run_from(std::env::args_os())
}

fn main() {
    std::process::exit(run());
}
// LCOV_EXCL_STOP - reason: end process shim, issue: 1055, policy: docs/cli/commands/build-test-coverage.md

#[cfg(test)]
mod tests {
    use super::*;

    const ECOSYSTEMS: [&str; 12] = [
        "rust", "python", "js", "ts", "go", "java", "kotlin", "scala", "csharp", "fsharp", "cc",
        "ruby",
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
            let eco: Ecosystem = name.parse().unwrap_or_else(|_| panic!("{name} must parse"));
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
        let code = run_from([
            "depcheck",
            "consistency",
            "--ecosystem=rust",
            "--manifest=/nonexistent/Cargo.toml",
            "--lock=/nonexistent/Cargo.lock",
        ]);
        assert_eq!(code, 2);
    }

    #[test]
    fn usage_dispatch_accepts_the_optional_exceptions_flag() {
        let code = run_from([
            "depcheck",
            "usage",
            "--ecosystem=python",
            "--manifest=/nonexistent/pyproject.toml",
            "--sources=/nonexistent",
            "--exceptions=/nonexistent/exceptions.toml",
        ]);
        assert_eq!(code, 2);
    }

    #[test]
    fn locks_dispatch_reports_the_first_unreadable_lock() {
        let code = run_from(lock_args("missing"));
        assert_eq!(code, 2);
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
        let code = run_from([
            "depcheck".to_owned(),
            "consistency".to_owned(),
            format!("--manifest={}", manifest.display()),
            format!("--lock={}", lock.display()),
            "--ecosystem=rust".to_owned(),
        ]);
        assert_eq!(code, 0);
    }
}
