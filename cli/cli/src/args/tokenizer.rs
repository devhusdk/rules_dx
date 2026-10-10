use std::ffi::{OsStr, OsString};

use clap::Parser;

use super::command::Command;
use super::grammar::{Cli, Flags, Verb};
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
        clap::error::ErrorKind::UnknownArgument => {
            unsupported_option(args, error).unwrap_or_else(|| ArgsError::Usage {
                text: without_usage(&error.render().to_string()),
            })
        }
        _ => ArgsError::Usage {
            text: without_usage(&error.render().to_string()),
        },
    }
}

/// The flag name the parser reports, with visible aliases spelled canonically.
fn reported_flag(token: &str) -> Option<String> {
    if !token.starts_with("--") {
        return None;
    }
    let name = token.split('=').next().unwrap_or(token);
    Some(match name {
        "--cwd" => "--here".to_owned(),
        _ => name.to_owned(),
    })
}

/// Every long flag name any command's grammar accepts.
fn known_flags() -> Vec<String> {
    use clap::CommandFactory;
    let mut names = Vec::new();
    for sub in Cli::command().get_subcommands() {
        for arg in sub.get_arguments() {
            for name in arg.get_long_and_visible_aliases().into_iter().flatten() {
                names.push(format!("--{name}"));
            }
        }
    }
    names.sort();
    names.dedup();
    names
}

/// The words after the command word, up to the `--` separator, with their
/// flags spelled canonically and inline values kept.
fn words_after_command<S: AsRef<OsStr>>(args: &[S], command_at: usize) -> Vec<String> {
    args[command_at + 1..]
        .iter()
        .filter_map(|word| word.as_ref().to_str())
        .take_while(|word| *word != "--")
        .map(canonical_word)
        .collect()
}

/// One word with its flag name spelled canonically, keeping any `=value`.
fn canonical_word(word: &str) -> String {
    let Some((name, value)) = word.split_once('=') else {
        return reported_flag(word).unwrap_or_else(|| word.to_owned());
    };
    let flag = reported_flag(name).unwrap_or_else(|| name.to_owned());
    format!("{flag}={value}")
}

/// The flag name without its `=value`.
fn name_of(word: &str) -> &str {
    word.split('=').next().unwrap_or(word)
}

/// The flag's value, inline or the next word.
fn inline_value(words: &[String], index: usize) -> Option<&str> {
    let word = words.get(index)?;
    match word.split_once('=') {
        Some((_, value)) => Some(value),
        None => words.get(index + 1).map(String::as_str),
    }
}

/// The first `--report` request, named with its payload.
fn report_request(words: &[String]) -> String {
    let index = words
        .iter()
        .position(|word| name_of(word) == "--report")
        .unwrap_or(0);
    let word = words.get(index).map_or("--report", String::as_str);
    if word.contains('=') {
        return word.to_owned();
    }
    match words.get(index + 1) {
        Some(value) => format!("--report={value}"),
        None => "--report".to_owned(),
    }
}

/// Translate clap's unknown-flag failure into the command's own refusal,
/// naming the flag dx would have named first when several are refused.
fn unsupported_option<S: AsRef<OsStr>>(args: &[S], error: &clap::Error) -> Option<ArgsError> {
    use clap::error::{ContextKind, ContextValue};
    let token = match error.get(ContextKind::InvalidArg) {
        Some(ContextValue::String(token)) => token.clone(),
        _ => return None,
    };
    let token = reported_flag(&token)?;
    if !known_flags().contains(&token) {
        return None;
    }
    let utf8: Vec<(usize, &str)> = args
        .iter()
        .enumerate()
        .filter_map(|(index, word)| word.as_ref().to_str().map(|word| (index, word)))
        .collect();
    let command_at = utf8
        .iter()
        .position(|(_, word)| Command::parse(word).is_some())?;
    let command = Command::parse(utf8[command_at].1)?;
    let after_command = utf8.iter().any(|(index, word)| {
        *index > command_at && reported_flag(word).as_deref() == Some(token.as_str())
    });
    if !after_command {
        return None;
    }
    let words = words_after_command(args, command_at);
    let present = |flag: &str| words.iter().any(|word| name_of(word) == flag);
    let named = if present("--here") && !command.supports_here() {
        "--here".to_owned()
    } else if present("--offline") && !command.supports_offline() {
        "--offline".to_owned()
    } else if present("--frozen") && !command.supports_frozen() {
        "--frozen".to_owned()
    } else if present("--check") && !command.supports_check() {
        "--check".to_owned()
    } else if present("--fail-on") && !command.supports_fail_on() {
        "--fail-on".to_owned()
    } else if !command.supports_diff()
        && words
            .iter()
            .position(|word| name_of(word) == "--output")
            .is_some_and(|index| inline_value(&words, index) == Some("diff"))
    {
        "--output=diff".to_owned()
    } else if crate::plan::spec(command).reports.is_empty() && present("--report") {
        report_request(&words)
    } else {
        token.clone()
    };
    Some(ArgsError::UnsupportedOption {
        command: command.name(),
        option: named,
    })
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

/// The parsed invocation: the command, its flags, its scopes, and its `--` payload.
pub(crate) struct Tokenized {
    pub(crate) command: Command,
    pub(crate) flags: Flags,
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
            flags: Flags::default(),
            targets: Vec::new(),
            bazel_options: collect_bazel_options(&args[1..])?,
        });
    }
    let command = parse_tokens(args)?.ok_or(ArgsError::MissingCommand)?;
    let (command, flags, targets, bazel_options) = command.into_parts();
    Ok(Tokenized {
        command,
        flags,
        targets,
        bazel_options,
    })
}
