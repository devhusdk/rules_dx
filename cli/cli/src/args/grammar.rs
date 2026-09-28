use std::ffi::OsString;

use clap::Parser;

use super::command::Command;

#[derive(Parser)]
#[command(
    name = "dx",
    about = "Run Bazel workflows",
    long_about = "dx [global-options] <command> [scope ...] [-- bazel-options ...]\n\nScopes: labels, patterns, files, or dirs. No scope means //... for most commands.\n\nExit codes: 0 success, 2 usage error, 1 failed check.",
    version,
    disable_help_subcommand = true
)]
pub(crate) struct Cli {
    /// Use this workspace dir.
    #[arg(long, allow_negative_numbers = true, overrides_with = "workspace")]
    pub(crate) workspace: Option<OsString>,
    /// Show the plan without running it.
    #[arg(long, overrides_with = "dry_run")]
    pub(crate) dry_run: bool,
    /// Hide summaries.
    #[arg(long, overrides_with = "quiet")]
    pub(crate) quiet: bool,
    /// Show more logs.
    #[arg(long, short = 'v', overrides_with = "verbose")]
    pub(crate) verbose: bool,
    /// Set color output.
    #[arg(long, allow_negative_numbers = true, overrides_with = "color")]
    pub(crate) color: Option<String>,
    /// Set log level.
    #[arg(
        long = "log-level",
        value_name = "LEVEL",
        allow_negative_numbers = true,
        overrides_with = "log_level"
    )]
    pub(crate) log_level: Option<String>,
    /// Set output format.
    #[arg(long, allow_negative_numbers = true, overrides_with = "output")]
    pub(crate) output: Option<String>,
    /// Write a report file.
    #[arg(long, allow_negative_numbers = true)]
    pub(crate) report: Vec<String>,
    /// Fail on this severity.
    #[arg(long, allow_negative_numbers = true, overrides_with = "fail_on")]
    pub(crate) fail_on: Option<String>,
    /// Require this coverage percent.
    #[arg(long, allow_negative_numbers = true, overrides_with = "min_coverage")]
    pub(crate) min_coverage: Option<String>,
    /// Check without changing files.
    #[arg(long, overrides_with = "check")]
    pub(crate) check: bool,
    /// Use debug build.
    #[arg(long, overrides_with = "debug")]
    pub(crate) debug: bool,
    /// Use release build.
    #[arg(long, overrides_with = "release")]
    pub(crate) release: bool,
    /// Also run bazel clean.
    #[arg(long = "bazel", overrides_with = "bazel_clean")]
    pub(crate) bazel_clean: bool,
    /// Re-pin to this version.
    #[arg(long, allow_negative_numbers = true, overrides_with = "pin")]
    pub(crate) pin: Option<String>,
    /// Re-pin the last release.
    #[arg(long, overrides_with = "rollback")]
    pub(crate) rollback: bool,
    /// Use cquery instead of query.
    #[arg(long, overrides_with = "configured")]
    pub(crate) configured: bool,
    /// Migrate from this version.
    #[arg(long, allow_negative_numbers = true, overrides_with = "from")]
    pub(crate) from: Option<String>,
    /// Migrate to this version.
    #[arg(long, allow_negative_numbers = true, overrides_with = "to")]
    pub(crate) to: Option<String>,
    /// Use the current dir tree.
    #[arg(long, visible_alias = "cwd", overrides_with = "here")]
    pub(crate) here: bool,
    /// Serve docs locally.
    #[arg(long, overrides_with = "serve")]
    pub(crate) serve: bool,
    /// Docs serve port.
    #[arg(long, allow_negative_numbers = true, overrides_with = "port")]
    pub(crate) port: Option<String>,
    /// Docs serve host.
    #[arg(long, allow_negative_numbers = true, overrides_with = "host")]
    pub(crate) host: Option<String>,
    /// Open docs in a browser.
    #[arg(long, overrides_with = "open")]
    pub(crate) open: bool,
    /// Run without network.
    #[arg(long, visible_alias = "frozen", overrides_with = "offline")]
    pub(crate) offline: bool,
    /// Command to run.
    #[arg(value_enum)]
    pub(crate) command: Option<Command>,
    /// Scopes to run on.
    pub(crate) targets: Vec<OsString>,
    /// Args after --.
    #[arg(last = true)]
    pub(crate) bazel_options: Vec<String>,
}

pub fn cli_command() -> clap::Command {
    use clap::CommandFactory;
    Cli::command()
}

pub(crate) const VALUE_OPTIONS: &[&str] = &[
    "--workspace",
    "--output",
    "--report",
    "--fail-on",
    "--min-coverage",
    "--pin",
    "--from",
    "--to",
    "--port",
    "--host",
    "--color",
    "--log-level",
];
