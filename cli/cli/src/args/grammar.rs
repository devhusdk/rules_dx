use std::ffi::OsString;

use clap::Parser;

use super::command::Command;
use super::invocation::ReportRequest;

/// A report request.
fn report_request(raw: &str) -> Result<ReportRequest, String> {
    match raw.split_once('=') {
        Some((format, destination)) if !format.is_empty() && !destination.is_empty() => {
            Ok(ReportRequest {
                format: format.to_owned(),
                destination: destination.to_owned(),
            })
        }
        _ => Err(format!("want <format>=<destination>, got {raw:?}")),
    }
}

/// A value that must not be empty.
fn non_empty(raw: &str) -> Result<String, String> {
    if raw.is_empty() {
        Err("want a value".to_owned())
    } else {
        Ok(raw.to_owned())
    }
}

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
    #[arg(
        long,
        short = 'v',
        overrides_with = "verbose",
        conflicts_with = "log_level"
    )]
    pub(crate) verbose: bool,
    /// Set color output.
    #[arg(long, allow_negative_numbers = true, overrides_with = "color")]
    pub(crate) color: Option<String>,
    /// Set log level.
    #[arg(
        long = "log-level",
        value_name = "LEVEL",
        allow_negative_numbers = true,
        overrides_with = "log_level",
        conflicts_with = "verbose"
    )]
    pub(crate) log_level: Option<String>,
    /// Set output format.
    #[arg(long, allow_negative_numbers = true, overrides_with = "output")]
    pub(crate) output: Option<String>,
    /// Write a report file.
    #[arg(long, allow_negative_numbers = true, value_parser = report_request)]
    pub(crate) report: Vec<ReportRequest>,
    /// Fail on this severity.
    #[arg(long, allow_negative_numbers = true, overrides_with = "fail_on")]
    pub(crate) fail_on: Option<String>,
    /// Require this coverage percent.
    #[arg(
        long,
        allow_negative_numbers = true,
        overrides_with = "min_coverage",
        value_parser = clap::value_parser!(u32).range(0..=100)
    )]
    pub(crate) min_coverage: Option<u32>,
    /// Check without changing files.
    #[arg(long, overrides_with = "check")]
    pub(crate) check: bool,
    /// Use debug build.
    #[arg(long, overrides_with = "debug", conflicts_with = "release")]
    pub(crate) debug: bool,
    /// Use release build.
    #[arg(long, overrides_with = "release", conflicts_with = "debug")]
    pub(crate) release: bool,
    /// Also run bazel clean.
    #[arg(long = "bazel", overrides_with = "bazel_clean")]
    pub(crate) bazel_clean: bool,
    /// Re-pin to this version.
    #[arg(long, allow_negative_numbers = true, overrides_with = "pin", value_parser = non_empty)]
    pub(crate) pin: Option<String>,
    /// Re-pin the last release.
    #[arg(long, overrides_with = "rollback")]
    pub(crate) rollback: bool,
    /// Use cquery instead of query.
    #[arg(long, overrides_with = "configured")]
    pub(crate) configured: bool,
    /// Migrate from this version.
    #[arg(long, allow_negative_numbers = true, overrides_with = "from", value_parser = non_empty)]
    pub(crate) from: Option<String>,
    /// Migrate to this version.
    #[arg(long, allow_negative_numbers = true, overrides_with = "to", value_parser = non_empty)]
    pub(crate) to: Option<String>,
    /// Use the current dir tree.
    #[arg(long, visible_alias = "cwd", overrides_with = "here")]
    pub(crate) here: bool,
    /// Serve docs locally.
    #[arg(long, overrides_with = "serve")]
    pub(crate) serve: bool,
    /// Docs serve port.
    #[arg(
        long,
        allow_negative_numbers = true,
        overrides_with = "port",
        value_parser = clap::value_parser!(u16).range(1..)
    )]
    pub(crate) port: Option<u16>,
    /// Docs serve host.
    #[arg(long, allow_negative_numbers = true, overrides_with = "host", value_parser = non_empty)]
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

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    fn kind_of(words: &[&str]) -> clap::error::ErrorKind {
        Cli::try_parse_from(
            std::iter::once(OsString::from("dx")).chain(words.iter().map(OsString::from)),
        )
        .err()
        .map_or(clap::error::ErrorKind::UnknownArgument, |error| {
            error.kind()
        })
    }

    #[test]
    fn the_grammar_rejects_conflicting_value_types() {
        for words in [
            vec!["build", "--debug", "--release"],
            vec!["build", "--release", "--debug"],
            vec!["lint", "--verbose", "--log-level=debug"],
            vec!["lint", "-v", "--log-level=info"],
        ] {
            assert_eq!(
                kind_of(&words),
                clap::error::ErrorKind::ArgumentConflict,
                "words: {words:?}"
            );
        }
        for words in [
            vec!["coverage", "--min-coverage=eighty"],
            vec!["coverage", "--min-coverage=101"],
            vec!["lint", "--report=sarif"],
            vec!["docs", "--serve", "--port=0"],
            vec!["docs", "--serve", "--port=notanumber"],
            vec!["docs", "--serve", "--host="],
        ] {
            assert_eq!(
                kind_of(&words),
                clap::error::ErrorKind::ValueValidation,
                "words: {words:?}"
            );
        }
        for words in [
            vec!["build", "--debug"],
            vec!["build", "--release"],
            vec!["lint", "--verbose"],
            vec!["coverage", "--min-coverage=80"],
            vec!["coverage", "--min-coverage=0"],
            vec!["coverage", "--min-coverage=100"],
            vec!["lint", "--report=sarif=out.sarif"],
            vec!["docs", "--serve", "--port=1"],
            vec!["docs", "--serve", "--port=65535"],
            vec!["docs", "--serve", "--host=example.test"],
        ] {
            assert_eq!(
                kind_of(&words),
                clap::error::ErrorKind::UnknownArgument,
                "words: {words:?}"
            );
            Cli::try_parse_from(
                std::iter::once(OsString::from("dx")).chain(words.iter().map(OsString::from)),
            )
            .unwrap_or_else(|error| panic!("words: {words:?} must parse: {error}"));
        }
    }
}
