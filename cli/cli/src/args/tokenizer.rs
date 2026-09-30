use std::ffi::{OsStr, OsString};

use clap::Parser;

use super::command::Command;
use super::grammar::{Cli, VALUE_OPTIONS};
use super::{help, ArgsError};

fn arg_text(arg: &OsStr) -> Option<&str> {
    arg.to_str()
}

fn split_bazel_verbatim<S: AsRef<OsStr>>(args: &[S]) -> Option<usize> {
    let mut index = 0;
    while index < args.len() {
        let raw = args[index].as_ref();
        let arg = arg_text(raw)?;
        if arg == "--" {
            return None;
        }
        if arg.starts_with('-') {
            let name = arg.split_once('=').map_or(arg, |(name, _)| name);
            if !arg.contains('=') && VALUE_OPTIONS.contains(&name) {
                let next = args.get(index + 1)?;
                let next_raw = next.as_ref();
                let is_flag = next_raw == OsStr::new("--")
                    || next_raw.to_str().is_some_and(|text| text.starts_with("--"));
                if !is_flag {
                    index += 2;
                } else {
                    return None;
                }
                continue;
            }
            index += 1;
            continue;
        }
        return (arg == "bazel").then_some(index);
    }
    None
}

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

fn parse_tokens<S: AsRef<OsStr>>(args: &[S]) -> Result<Cli, ArgsError> {
    Cli::try_parse_from(
        std::iter::once(OsString::from("dx"))
            .chain(args.iter().map(|arg| arg.as_ref().to_os_string())),
    )
    .map_err(|error| map_clap_error(args, &error))
}

pub(crate) fn tokenize<S: AsRef<OsStr>>(args: &[S]) -> Result<(Cli, Vec<String>), ArgsError> {
    if let Some(at) = split_bazel_verbatim(args) {
        let prefix: Vec<OsString> = args[..at]
            .iter()
            .map(|arg| arg.as_ref().to_os_string())
            .collect();
        let mut cli = parse_tokens(&prefix)?;
        cli.command = Some(Command::Bazel);
        let mut bazel_options = Vec::new();
        let mut tail = args[at + 1..].iter();
        for arg in tail.by_ref() {
            let raw = arg.as_ref();
            if raw == OsStr::new("--") {
                break;
            }
            match raw.to_str() {
                Some(text) => bazel_options.push(text.to_owned()),
                None => {
                    return Err(ArgsError::InvalidScope {
                        scope: raw.to_string_lossy().into_owned(),
                    });
                }
            }
        }
        for arg in tail {
            let raw = arg.as_ref();
            match raw.to_str() {
                Some(text) => bazel_options.push(text.to_owned()),
                None => {
                    return Err(ArgsError::InvalidScope {
                        scope: raw.to_string_lossy().into_owned(),
                    });
                }
            }
        }
        return Ok((cli, bazel_options));
    }
    let mut cli = parse_tokens(args)?;
    let bazel_options = std::mem::take(&mut cli.bazel_options);
    Ok((cli, bazel_options))
}
