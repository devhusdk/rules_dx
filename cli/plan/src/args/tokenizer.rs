use std::ffi::{OsStr, OsString};

use clap::Parser;

use super::command::Command;
use super::grammar::{Cli, Flags, Verb};
use super::{ArgsError, help};

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
        clap::error::ErrorKind::InvalidValue => {
            output_value_error(args, error).unwrap_or_else(|| ArgsError::Usage {
                text: without_usage(&error.render().to_string()),
            })
        }
        _ => ArgsError::Usage {
            text: without_usage(&error.render().to_string()),
        },
    }
}

/// Maps a refused `--output`, `--color` or `--fail-on` value to its value error.
fn output_value_error<S: AsRef<OsStr>>(args: &[S], error: &clap::Error) -> Option<ArgsError> {
    use clap::error::{ContextKind, ContextValue};
    let flag = match error.get(ContextKind::InvalidArg) {
        Some(ContextValue::String(token)) => token.clone(),
        _ => return None,
    };
    let is_output = flag.starts_with("--output") || flag.starts_with("-o");
    let is_color = flag.starts_with("--color");
    let is_fail_on = flag.starts_with("--fail-on");
    if !is_output && !is_color && !is_fail_on {
        return None;
    }
    let value = match error.get(ContextKind::InvalidValue) {
        Some(ContextValue::String(value)) => value.clone(),
        _ => return None,
    };
    if value.is_empty() {
        return None;
    }
    if is_color {
        return Some(ArgsError::BadColor { value });
    }
    if is_fail_on {
        return Some(ArgsError::BadFailOn { value });
    }
    if matches!(value.as_str(), "text" | "json") {
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
    if !super::grammar::option_at(word, next).is_some_and(|option| option.used_next) {
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
    pub(crate) passthrough: Vec<String>,
}

pub(crate) fn tokenize<S: AsRef<OsStr>>(args: &[S]) -> Result<Tokenized, ArgsError> {
    let command = parse_tokens(args)?.ok_or(ArgsError::MissingCommand)?;
    let (command, flags, targets, passthrough) = command.into_parts();
    Ok(Tokenized {
        command,
        flags,
        targets,
        passthrough,
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
