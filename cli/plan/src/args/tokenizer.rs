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
            if let Some(text) = removed_version_flag(args, error) {
                return ArgsError::Usage { text };
            }
            if let Some(text) = removed_clean_flag(args, error) {
                return ArgsError::Usage { text };
            }
            if let Some(text) = removed_receipt_flag(error) {
                return ArgsError::Usage { text };
            }
            unsupported_option(args, error).unwrap_or_else(|| ArgsError::Usage {
                text: without_usage(&error.render().to_string()),
            })
        }
        clap::error::ErrorKind::InvalidValue => {
            output_value_error(args, error).unwrap_or_else(|| ArgsError::Usage {
                text: without_usage(&error.render().to_string()),
            })
        }
        clap::error::ErrorKind::InvalidSubcommand => match removed_command(error) {
            Some(text) => ArgsError::Usage { text },
            None => ArgsError::Usage {
                text: without_usage(&error.render().to_string()),
            },
        },
        _ => ArgsError::Usage {
            text: without_usage(&error.render().to_string()),
        },
    }
}

/// Maps a refused `--output` value to the mode error the command's grammar implies.
fn output_value_error<S: AsRef<OsStr>>(args: &[S], error: &clap::Error) -> Option<ArgsError> {
    use clap::error::{ContextKind, ContextValue};
    let flag = match error.get(ContextKind::InvalidArg) {
        Some(ContextValue::String(token)) => token.clone(),
        _ => return None,
    };
    if !flag.starts_with("--output") && !flag.starts_with("-o") {
        return None;
    }
    let value = match error.get(ContextKind::InvalidValue) {
        Some(ContextValue::String(value)) => value.clone(),
        _ => return None,
    };
    if value.is_empty() {
        return None;
    }
    if matches!(value.as_str(), "text" | "diff" | "json") {
        let command = args
            .iter()
            .filter_map(|word| word.as_ref().to_str())
            .find_map(Command::parse)?;
        return Some(ArgsError::UnsupportedOption {
            command: command.name(),
            option: format!("--output={value}"),
        });
    }
    Some(ArgsError::BadOutput { value })
}

/// Removed command names and the tip each error carries.
pub(crate) const REMOVED_COMMANDS: &[(&str, &str)] = &[
    ("capabilities", "run `dx status --verbose` for capability and readiness facts"),
    ("setup", "run `dx env` and `dx codegen` instead (each takes --check or --apply)"),
    ("verify", "run `dx check`, `dx build`, `dx test` and `dx coverage` directly, in the order you need"),
    ("owners", "run `bazel query 'rdeps(//..., <label>, 1)'`, or `bazel cquery` for the configured graph; wrap the expression in single quotes"),
    ("deps", "run `bazel query 'deps(<label>)'`, or `bazel cquery 'deps(<label>)' --platforms=<platform>` for the configured graph; wrap the expression in single quotes"),
    ("why", "run `bazel query 'somepath(<from>, <to>)'`, or `bazel cquery` for the configured graph; wrap the expression in single quotes"),
    ("tests", "run `dx list-tests` to list test targets, or `dx test` to run them"),
    ("rerun", "run `dx test <target>` for the targets you want to run again"),
    ("doctor", "run `dx status` for diagnostics"),
];

/// Command names dx never takes, and the tip each error carries.
pub(crate) const RESERVED_COMMANDS: &[(&str, &str)] = &[(
    "shim",
    "`dx shim ...` is handled by the global dx shim on PATH, not by the dx binary",
)];

fn removed_command(error: &clap::Error) -> Option<String> {
    let word = match error.get(clap::error::ContextKind::InvalidSubcommand) {
        Some(clap::error::ContextValue::String(word)) => word.as_str(),
        _ => return None,
    };
    if let Some((_, tip)) = RESERVED_COMMANDS.iter().find(|(name, _)| *name == word) {
        return Some(format!("error: reserved command '{word}'\n\n  tip: {tip}"));
    }
    let (_, tip) = REMOVED_COMMANDS.iter().find(|(name, _)| *name == word)?;
    Some(format!(
        "error: unsupported command '{word}'\n\n  tip: {tip}"
    ))
}

fn removed_receipt_flag(error: &clap::Error) -> Option<String> {
    let flag = match error.get(clap::error::ContextKind::InvalidArg) {
        Some(clap::error::ContextValue::String(token)) => reported_flag(token)?,
        _ => return None,
    };
    let tip = match flag.as_str() {
        "--run-output" => {
            "use `--report junit=<path>` or `--report lcov=<path>` for test and coverage reports"
        }
        "--cwd" => "use `--here` to select the current directory tree",
        "--strict-evidence" => {
            "incomplete test and coverage evidence now always fails; drop the flag"
        }
        _ => return None,
    };
    Some(format!(
        "error: unsupported option '{flag}'\n\n  tip: {tip}"
    ))
}

fn removed_clean_flag<S: AsRef<OsStr>>(args: &[S], error: &clap::Error) -> Option<String> {
    use clap::error::{ContextKind, ContextValue};
    let flag = match error.get(ContextKind::InvalidArg) {
        Some(ContextValue::String(token)) => reported_flag(token)?,
        _ => return None,
    };
    if flag != "--bazel" {
        return None;
    }
    let is_clean = args
        .iter()
        .filter_map(|word| word.as_ref().to_str())
        .find(|word| Command::parse(word).is_some())
        == Some("clean");
    if !is_clean {
        return None;
    }
    Some(
        "error: unsupported option '--bazel' for dx clean\n\n  tip: dx clean only prunes managed state; run `bazel clean` to clean Bazel outputs"
            .to_owned(),
    )
}

fn removed_version_flag<S: AsRef<OsStr>>(args: &[S], error: &clap::Error) -> Option<String> {
    use clap::error::{ContextKind, ContextValue};
    let flag = match error.get(ContextKind::InvalidArg) {
        Some(ContextValue::String(token)) => reported_flag(token)?,
        _ => return None,
    };
    if !matches!(flag.as_str(), "--pin" | "--rollback") {
        return None;
    }
    let is_version = args
        .iter()
        .filter_map(|word| word.as_ref().to_str())
        .find(|word| Command::parse(word).is_some())
        == Some("version");
    if !is_version {
        return None;
    }
    let tip = if flag == "--pin" {
        "run `dx upgrade --from <version> --to <version>` to change the pin"
    } else {
        "run `dx rollback --apply` to restore the previous pin"
    };
    Some(format!(
        "error: unsupported option '{flag}' for dx version\n\n  tip: dx version only reports; {tip}"
    ))
}

/// The flag name the parser reports, with visible aliases spelled canonically.
fn reported_flag(token: &str) -> Option<String> {
    if !token.starts_with("--") {
        return None;
    }
    let name = token.split('=').next().unwrap_or(token);
    Some(name.to_owned())
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

/// The flag as typed, with its value attached when the value is a separate word.
fn spelled(words: &[String], index: usize) -> String {
    let word = &words[index];
    let next = words.get(index + 1).map(String::as_str);
    if !super::grammar::consumes_next(word, next) {
        return word.clone();
    }
    match next {
        Some(value) => format!("{word}={value}"),
        None => word.clone(),
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
    let named = words
        .iter()
        .position(|word| name_of(word) == token)
        .map_or_else(|| token.clone(), |index| spelled(&words, index));
    let named = if matches!(token.as_str(), "--output" | "--report") {
        named
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

/// The workspace value the grammar selects, or none when the words do not parse.
pub(crate) fn selected_workspace<S: AsRef<OsStr>>(args: &[S]) -> Option<OsString> {
    selected_identity(args)?.0
}

/// The workspace and Bazel startup tokens the grammar selects, if the words parse.
pub(crate) fn selected_identity<S: AsRef<OsStr>>(
    args: &[S],
) -> Option<(Option<OsString>, Vec<String>)> {
    let cli = Cli::try_parse_from(
        std::iter::once(OsString::from("dx"))
            .chain(args.iter().map(|arg| arg.as_ref().to_os_string())),
    )
    .ok()?;
    let (_, flags, _, _) = cli.verb?.into_parts();
    Some((flags.workspace, flags.bazel_startup_options))
}

/// The parsed invocation: the command, its flags, its scopes, and its `--` payload.
pub(crate) struct Tokenized {
    pub(crate) command: Command,
    pub(crate) flags: Flags,
    pub(crate) targets: Vec<OsString>,
    pub(crate) bazel_options: Vec<String>,
}

pub(crate) fn tokenize<S: AsRef<OsStr>>(args: &[S]) -> Result<Tokenized, ArgsError> {
    let command = parse_tokens(args)?.ok_or(ArgsError::MissingCommand)?;
    let (command, flags, targets, bazel_options) = command.into_parts();
    Ok(Tokenized {
        command,
        flags,
        targets,
        bazel_options,
    })
}

#[cfg(test)]
mod tests {
    use super::spelled;

    #[test]
    fn a_short_cluster_takes_its_value_word() {
        let words: Vec<String> = ["-qo", "json"].map(str::to_owned).to_vec();
        assert_eq!(spelled(&words, 0), "-qo=json");
        let words: Vec<String> = ["-q", "json"].map(str::to_owned).to_vec();
        assert_eq!(spelled(&words, 0), "-q");
    }
}
