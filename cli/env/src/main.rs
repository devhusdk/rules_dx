#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

use std::path::{Path, PathBuf};
use std::time::Duration;

use clap::Parser;
use dx_env::{identity_hex, parse_staged, probe_symlink, refresh, RefreshOptions, RefreshOutcome};

const METADATA_CANDIDATES: &[(&str, &str)] = &[
    ("rules_dx/env/default_tree.metadata.json", "_main"),
    ("_main/env/default_tree.metadata.json", "_main"),
];

fn usage_error(message: &str) -> i32 {
    tracing::error!("dx env: {message}");
    tracing::error!(
        "usage: env [--workspace DIR] [--staged-bin DIR --metadata FILE] [--lock-timeout-ms N]"
    );
    2
}

#[derive(Parser)]
#[command(disable_help_flag = true)]
struct Cli {
    #[arg(long, allow_hyphen_values = true, overrides_with = "workspace")]
    workspace: Option<String>,
    #[arg(long, allow_hyphen_values = true, overrides_with = "staged_bin")]
    staged_bin: Option<String>,
    #[arg(long, allow_hyphen_values = true, overrides_with = "metadata")]
    metadata: Option<String>,
    #[arg(long, allow_hyphen_values = true, overrides_with = "lock_timeout_ms")]
    lock_timeout_ms: Option<String>,
    #[arg(long = "help", short = 'h', action = clap::ArgAction::SetTrue)]
    help: bool,
}

fn parse_args(args: &[String]) -> Result<Cli, String> {
    Cli::try_parse_from(std::iter::once("env").chain(args.iter().map(|arg| arg as &str)))
        .map_err(|error| dx_output::parse_error(&error, args))
}

#[derive(Debug)]
enum Plan {
    Help,
    Install {
        workspace: Option<String>,
        staged_bin: Option<String>,
        metadata: Option<String>,
        lock_timeout: Duration,
    },
}

fn plan(args: &[String]) -> Result<Plan, String> {
    let cli = parse_args(args)?;
    if cli.help {
        return Ok(Plan::Help);
    }
    if cli.staged_bin.is_none() != cli.metadata.is_none() {
        return Err("--staged-bin and --metadata must be passed together".to_owned());
    }
    let lock_timeout = match cli.lock_timeout_ms {
        None => dx_env::LOCK_TIMEOUT,
        Some(millis) => match millis.parse::<u64>() {
            Ok(millis) => Duration::from_millis(millis),
            Err(_) => return Err("--lock-timeout-ms must be a non-negative integer".to_owned()),
        },
    };
    Ok(Plan::Install {
        workspace: cli.workspace,
        staged_bin: cli.staged_bin,
        metadata: cli.metadata,
        lock_timeout,
    })
}

// LCOV_EXCL_START - reason: thin shim, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
fn run() -> i32 {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (workspace, staged_bin, metadata, lock_timeout) = match plan(&args) {
        Ok(Plan::Help) => return usage_error("install the staged environment tree into .dx/bin"),
        Ok(Plan::Install {
            workspace,
            staged_bin,
            metadata,
            lock_timeout,
        }) => (workspace, staged_bin, metadata, lock_timeout),
        Err(message) => return usage_error(&message),
    };
    let start = match workspace {
        Some(dir) => PathBuf::from(dir),
        None => {
            let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
            dx_process::workspace_start(&cwd)
        }
    };
    let (staged_bin, metadata) = match (staged_bin, metadata) {
        (Some(bin), Some(meta)) => (PathBuf::from(bin), PathBuf::from(meta)),
        _ => match locate_default_tree() {
            Some(paths) => paths,
            None => {
                tracing::error!("dx env: default tree not found in runfiles; pass --staged-bin and --metadata explicitly");
                return 1;
            }
        },
    };
    let options = RefreshOptions {
        workspace_root: start,
        staged_bin,
        staged_metadata: metadata.clone(),
        os: std::env::consts::OS,
        lock_timeout,
    };
    let tools = match std::fs::read_to_string(&metadata) {
        Ok(text) => match parse_staged(&text) {
            Ok(tools) => tools,
            Err(error) => {
                tracing::error!("dx env: {error}");
                return 1;
            }
        },
        Err(error) => {
            tracing::error!(
                "dx env: cannot read staged metadata {}: {error}",
                metadata.display()
            );
            return 1;
        }
    };
    match refresh(&options, &probe_symlink) {
        Ok(RefreshOutcome::AlreadyCurrent) => {
            println!("dx env: already current (.dx/bin {})", identity_hex(&tools));
            0
        }
        Ok(RefreshOutcome::InstalledFresh) => {
            println!(
                "dx env: installed {} tool(s) (.dx/bin {})",
                tools.len(),
                identity_hex(&tools)
            );
            0
        }
        Ok(RefreshOutcome::InstalledReplacement) => {
            println!(
                "dx env: replaced managed tree with {} tool(s) (.dx/bin {})",
                tools.len(),
                identity_hex(&tools)
            );
            0
        }
        Err(error) => {
            tracing::error!("dx env: {error}");
            1
        }
    }
}

fn locate_default_tree() -> Option<(PathBuf, PathBuf)> {
    let runfiles = runfiles::Runfiles::create().ok()?;
    for (path, source_repo) in METADATA_CANDIDATES {
        if let Some(metadata) = runfiles.rlocation_from(Path::new(path), source_repo) {
            if metadata.is_file() {
                if let Some(parent) = metadata.parent() {
                    let staged_bin = parent.join(dx_env::BIN_DIR_NAME);
                    if staged_bin.is_dir() {
                        return Some((staged_bin, metadata));
                    }
                }
            }
        }
    }
    None
}

fn main() {
    dx_output::init_diagnostics(false);
    std::process::exit(run());
}
// LCOV_EXCL_STOP - reason: end thin shim, issue: 1055, policy: docs/cli/commands/build-test-coverage.md

#[cfg(test)]
mod tests {
    use super::*;

    struct Installed {
        workspace: Option<String>,
        staged_bin: Option<String>,
        metadata: Option<String>,
        lock_timeout: Duration,
    }

    fn plan_of(args: &[&str]) -> Result<Plan, String> {
        let args: Vec<String> = args.iter().map(|arg| (*arg).to_owned()).collect();
        plan(&args)
    }

    fn installed(args: &[&str]) -> Installed {
        match plan_of(args).expect("plan must accept these arguments") {
            Plan::Help => panic!("expected an install plan"),
            Plan::Install {
                workspace,
                staged_bin,
                metadata,
                lock_timeout,
            } => Installed {
                workspace,
                staged_bin,
                metadata,
                lock_timeout,
            },
        }
    }

    fn rejected(args: &[&str]) -> String {
        plan_of(args).expect_err("plan must reject these arguments")
    }

    #[test]
    fn no_arguments_default_every_option() {
        let got = installed(&[]);
        assert_eq!(got.workspace, None);
        assert_eq!(got.staged_bin, None);
        assert_eq!(got.metadata, None);
        assert_eq!(got.lock_timeout, dx_env::LOCK_TIMEOUT);
    }

    #[test]
    fn workspace_is_carried_through() {
        assert_eq!(
            installed(&["--workspace", "/tmp/work space"])
                .workspace
                .as_deref(),
            Some("/tmp/work space")
        );
    }

    #[test]
    fn help_beats_every_other_option() {
        for args in [
            &["--help"][..],
            &["-h"][..],
            &["--staged-bin", "bin", "--help"][..],
            &["--metadata", "meta", "--help"][..],
            &["--lock-timeout-ms", "nope", "--help"][..],
        ] {
            assert!(
                matches!(plan_of(args), Ok(Plan::Help)),
                "help did not win for {args:?}"
            );
        }
    }

    #[test]
    fn staged_bin_and_metadata_must_come_together() {
        for args in [
            &["--staged-bin", "bin"][..],
            &["--metadata", "meta"][..],
            &["--workspace", "w", "--staged-bin", "bin"][..],
        ] {
            assert_eq!(
                rejected(args),
                "--staged-bin and --metadata must be passed together",
                "accepted a lone flag in {args:?}"
            );
        }
    }

    #[test]
    fn staged_bin_and_metadata_are_carried_through() {
        let got = installed(&["--staged-bin", "bin", "--metadata", "meta"]);
        assert_eq!(got.staged_bin.as_deref(), Some("bin"));
        assert_eq!(got.metadata.as_deref(), Some("meta"));
    }

    #[test]
    fn lock_timeout_takes_a_whole_number_of_milliseconds() {
        assert_eq!(
            installed(&["--lock-timeout-ms", "0"]).lock_timeout,
            Duration::ZERO
        );
        assert_eq!(
            installed(&["--lock-timeout-ms", "250"]).lock_timeout,
            Duration::from_millis(250)
        );
    }

    #[test]
    fn lock_timeout_rejects_anything_else() {
        for bad in ["nope", "-1", "1.5", "", "12ms"] {
            assert_eq!(
                rejected(&["--lock-timeout-ms", bad]),
                "--lock-timeout-ms must be a non-negative integer",
                "accepted a bad timeout: {bad:?}"
            );
        }
    }

    #[test]
    fn unknown_flags_are_named() {
        assert_eq!(rejected(&["--nope"]), "unknown flag \"--nope\"");
        assert_eq!(rejected(&["--nope", "value"]), "unknown flag \"--nope\"");
        assert_eq!(rejected(&["-z"]), "unknown flag \"-z\"");
        assert_eq!(rejected(&["--nope=value"]), "unknown flag \"--nope=value\"");
    }

    #[test]
    fn unknown_flags_echo_every_equals_sign() {
        assert_eq!(
            rejected(&["--workspace", "w", "--nope=other=value"]),
            "unknown flag \"--nope=other=value\""
        );
    }

    #[test]
    fn missing_values_name_the_flag() {
        assert_eq!(rejected(&["--workspace"]), "missing value for --workspace");
        assert_eq!(
            rejected(&["--metadata", "meta", "--staged-bin"]),
            "missing value for --staged-bin"
        );
    }

    #[test]
    fn hyphen_values_are_kept_verbatim() {
        assert_eq!(
            installed(&["--workspace", "-weird"]).workspace.as_deref(),
            Some("-weird")
        );
    }

    #[test]
    fn repeated_flags_keep_the_last_value() {
        assert_eq!(
            installed(&["--workspace", "first", "--workspace", "second"])
                .workspace
                .as_deref(),
            Some("second")
        );
    }

    #[test]
    fn usage_errors_report_two() {
        assert_eq!(usage_error("bad flag"), 2);
    }
}
