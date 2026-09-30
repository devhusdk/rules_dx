use std::ffi::{OsStr, OsString};

use clap::Parser;

use super::command::Command;
use super::grammar::{Cli, GlobalArgs, Verb};
use super::{help, ArgsError};

/// clap's error text with its own usage block dropped.
pub(crate) fn without_usage(text: &str) -> String {
    let mut out = String::new();
    let mut skipping = false;
    for line in text.lines() {
        if line.starts_with("Usage: ") {
            skipping = true;
            continue;
        }
        if skipping {
            if !line.trim().is_empty() {
                continue;
            }
            skipping = false;
        }
        if line.trim().is_empty() && out.ends_with("\n\n") {
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    out.trim_end().to_owned()
}

fn map_clap_error<S: AsRef<OsStr>>(args: &[S], error: &clap::Error) -> ArgsError {
    match error.kind() {
        clap::error::ErrorKind::DisplayHelp => {
            let text = match help::help_command_in(args) {
                Some(command) => help::render_command_help(command),
                None => help::render_top_help(),
            };
            ArgsError::Help { text }
        }
        clap::error::ErrorKind::DisplayVersion => {
            use clap::CommandFactory;
            ArgsError::Help {
                text: Cli::command().render_version().to_string(),
            }
        }
        _ => ArgsError::Usage {
            text: without_usage(&error.render().to_string()),
        },
    }
}

fn parse_tokens<S: AsRef<OsStr>>(args: &[S]) -> Result<Option<Verb>, ArgsError> {
    Cli::try_parse_from(
        std::iter::once(OsString::from("dx"))
            .chain(args.iter().map(|arg| arg.as_ref().to_os_string())),
    )
    .map(|cli| cli.verb)
    .map_err(|error| map_clap_error(args, &error))
}

/// Every word after the first `--` is forwarded to the Bazel launcher.
fn collect_bazel_options<S: AsRef<OsStr>>(args: &[S]) -> Result<Vec<String>, ArgsError> {
    let mut forwarded = Vec::with_capacity(args.len());
    let mut past_separator = false;
    for arg in args {
        let raw = arg.as_ref();
        if !past_separator && raw == OsStr::new("--") {
            past_separator = true;
            continue;
        }
        forwarded.push(match raw.to_str() {
            Some(text) => text.to_owned(),
            None => {
                return Err(ArgsError::InvalidScope {
                    scope: raw.to_string_lossy().into_owned(),
                });
            }
        });
    }
    Ok(forwarded)
}

/// The parsed invocation: the command, its shared options, its scopes, and its `--` payload.
pub(crate) struct Tokenized {
    pub(crate) command: Command,
    pub(crate) global: GlobalArgs,
    pub(crate) targets: Vec<OsString>,
    pub(crate) bazel_options: Vec<String>,
}

/// `dx bazel` forwards every later word to the Bazel launcher.
pub(crate) fn tokenize<S: AsRef<OsStr>>(args: &[S]) -> Result<Tokenized, ArgsError> {
    if args
        .first()
        .is_some_and(|first| first.as_ref() == OsStr::new("bazel"))
    {
        return Ok(Tokenized {
            command: Command::Bazel,
            global: GlobalArgs::default(),
            targets: Vec::new(),
            bazel_options: collect_bazel_options(&args[1..])?,
        });
    }
    let command = parse_tokens(args)?.ok_or(ArgsError::MissingCommand)?;
    let (command, global, targets, bazel_options) = command.into_parts();
    Ok(Tokenized {
        command,
        global,
        targets,
        bazel_options,
    })
}
