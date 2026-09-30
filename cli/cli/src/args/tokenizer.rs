use std::ffi::{OsStr, OsString};

use clap::Parser;

use super::command::Command;
use super::grammar::{Cli, VALUE_OPTIONS};
use super::{help, suggest, ArgsError};

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

fn invalid_token(error: &clap::Error) -> Option<String> {
    let token = dx_output::invalid_token(error);
    if token.is_empty() {
        None
    } else {
        Some(token)
    }
}

fn invalid_value(error: &clap::Error) -> Option<String> {
    dx_output::rejected_value(error)
}

fn rejected_token(error: &clap::Error) -> String {
    match error.get(clap::error::ContextKind::InvalidValue) {
        Some(clap::error::ContextValue::String(value)) => value.clone(),
        Some(clap::error::ContextValue::Strings(values)) => {
            values.first().cloned().unwrap_or_default()
        }
        _ => String::new(),
    }
}

fn argument_conflict(error: &clap::Error) -> ArgsError {
    match invalid_token(error).unwrap_or_default().as_str() {
        "--debug" | "--release" => ArgsError::ConflictingProfiles,
        _ => ArgsError::ConflictingVerboseLogLevel,
    }
}

fn value_validation(error: &clap::Error) -> ArgsError {
    let token = invalid_token(error).unwrap_or_default();
    let option = leading_flag(&token);
    let value = rejected_token(error);
    match option.as_str() {
        "--min-coverage" => ArgsError::BadMinCoverage { value },
        "--report" => ArgsError::BadReport { value },
        _ => ArgsError::MissingValue {
            option: option.to_owned(),
        },
    }
}

fn recover_token<S: AsRef<OsStr>>(args: &[S], token: Option<String>) -> String {
    let token = token.unwrap_or_default();
    for arg in args {
        let text = arg.as_ref().to_string_lossy();
        if text == token {
            return text.into_owned();
        }
    }
    for arg in args {
        let text = arg.as_ref().to_string_lossy();
        if text.starts_with(&format!("{token}=")) {
            return text.into_owned();
        }
    }
    token
}

fn leading_flag(token: &str) -> String {
    dx_output::leading_flag(token).to_owned()
}

fn is_command_positional(token: &str) -> bool {
    token
        .trim_matches(|cut| cut == '<' || cut == '>' || cut == '[' || cut == ']')
        .eq_ignore_ascii_case("command")
}

fn map_clap_error<S: AsRef<OsStr>>(args: &[S], error: &clap::Error) -> ArgsError {
    use clap::error::ErrorKind;
    match error.kind() {
        ErrorKind::DisplayHelp => {
            let text = match help::help_command_in(args) {
                Some(command) => help::render_command_help(command),
                None => help::render_top_help(),
            };
            ArgsError::Help { text }
        }
        ErrorKind::DisplayVersion => {
            use clap::CommandFactory;
            ArgsError::Help {
                text: Cli::command().render_version().to_string(),
            }
        }
        ErrorKind::UnknownArgument | ErrorKind::TooManyValues => {
            let option = recover_token(args, invalid_token(error));
            let suggestion =
                suggest::clap_suggestion(error).or_else(|| suggest::suggest_option(&option));
            ArgsError::UnknownOption { option, suggestion }
        }
        ErrorKind::ValueValidation => value_validation(error),
        ErrorKind::ArgumentConflict => argument_conflict(error),
        ErrorKind::InvalidValue => {
            let token = invalid_token(error).unwrap_or_default();
            if invalid_value(error).is_none_or(|value| value.is_empty()) {
                ArgsError::MissingValue {
                    option: leading_flag(&token),
                }
            } else if is_command_positional(&token) {
                let value = invalid_value(error).unwrap_or(token);
                if value.starts_with('-') {
                    let option = recover_token(args, Some(value.clone()));
                    let suggestion = suggest::clap_suggestion(error)
                        .or_else(|| suggest::suggest_option(&option));
                    ArgsError::UnknownOption { option, suggestion }
                } else {
                    let command = recover_token(args, Some(value.clone()));
                    let suggestion = if command.eq_ignore_ascii_case("doctor")
                        || command.eq_ignore_ascii_case("configure")
                    {
                        Some("status".to_owned())
                    } else {
                        suggest::clap_command_suggestion(error)
                            .or_else(|| suggest::suggest_command(&command))
                    };
                    ArgsError::UnknownCommand {
                        command,
                        suggestion,
                    }
                }
            } else {
                let option = recover_token(args, Some(token));
                let suggestion =
                    suggest::clap_suggestion(error).or_else(|| suggest::suggest_option(&option));
                ArgsError::UnknownOption { option, suggestion }
            }
        }
        _ => ArgsError::InvalidArguments {
            message: error
                .render()
                .to_string()
                .lines()
                .next()
                .unwrap_or("invalid arguments")
                .trim()
                .trim_start_matches("error:")
                .trim()
                .to_owned(),
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
