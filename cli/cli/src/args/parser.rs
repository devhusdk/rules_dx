use std::ffi::OsStr;

use dx_output::{OutputMode, Threshold};

use super::command::Command;
use super::grammar::Flags;
use super::scope_error;
use super::tokenizer::tokenize;
use super::{ArgsError, Invocation, ReportRequest};

pub use super::grammar::cli_command;

fn decode_scope(value: &OsStr) -> Result<String, ArgsError> {
    match value.to_str() {
        Some(text) => Ok(text.to_owned()),
        None => Err(ArgsError::InvalidScope {
            scope: value.to_string_lossy().into_owned(),
        }),
    }
}

/// Names the positionals the command's own usage line requires.
fn missing_positional(command: Command) -> ArgsError {
    ArgsError::MissingValue {
        option: command.required_slot().unwrap_or(command.name()).to_owned(),
    }
}

/// Names the flag the command does not take.
fn unsupported(command: Command, option: impl Into<String>) -> ArgsError {
    ArgsError::UnsupportedOption {
        command: command.name(),
        option: option.into(),
    }
}

/// Names the one positional the command does not take.
fn extra_positional(command: Command, token: &str) -> ArgsError {
    unsupported(command, token)
}

/// Rejects `--output=diff`, which these commands never take.
fn reject_diff_output(command: Command, output_name: &str) -> Result<(), ArgsError> {
    if output_name == "diff" {
        return Err(unsupported(command, "--output=diff"));
    }
    Ok(())
}

/// Rejects the first `--report` request, naming its format and destination.
fn reject_report(command: Command, reports: &[ReportRequest]) -> Result<(), ArgsError> {
    match reports.first() {
        Some(request) => Err(unsupported(
            command,
            format!("--report={}={}", request.format, request.destination),
        )),
        None => Ok(()),
    }
}

/// Rejects Bazel options passed after `--`.
fn reject_passthrough(command: Command, bazel_options: &[String]) -> Result<(), ArgsError> {
    if !bazel_options.is_empty() {
        return Err(unsupported(command, "--"));
    }
    Ok(())
}

/// Names an environment default dx will not read.
fn bad_default(error: dx_adopt::AdoptError) -> ArgsError {
    ArgsError::BadDefault {
        detail: error.to_string(),
    }
}

pub fn parse<S: AsRef<OsStr>>(args: &[S]) -> Result<Invocation, ArgsError> {
    parse_with(args, &|_| None, &super::FileDefaults::default())
}

pub fn load_file_defaults(start: &std::path::Path) -> Result<super::FileDefaults, String> {
    match dx_adopt::defaults::load_defaults(start) {
        Ok((defaults, _)) => Ok(defaults),
        Err(error) => Err(error.to_string()),
    }
}

pub fn parse_with<S: AsRef<OsStr>>(
    args: &[S],
    env_get: &dyn Fn(&str) -> Option<String>,
    file: &super::FileDefaults,
) -> Result<Invocation, ArgsError> {
    if let Some(error) = super::help::help_verb_error_in(args) {
        return Err(error);
    }
    let tokenized = tokenize(args)?;
    let command = tokenized.command;
    let Flags {
        workspace: workspace_os,
        dry_run,
        quiet,
        verbose,
        log_level: log_level_name,
        color: color_name,
        output,
        report,
        fail_on,
        min_coverage,
        check,
        debug,
        release,
        bazel_clean,
        pin,
        rollback,
        configured,
        from,
        to,
        here,
        serve,
        port,
        host,
        open,
        offline,
    } = tokenized.flags;
    let targets_os = tokenized.targets;
    let bazel_options = tokenized.bazel_options;
    let flag_workspace = match workspace_os {
        Some(value) => Some(decode_scope(value.as_os_str())?),
        None => None,
    };
    let mut targets = Vec::with_capacity(targets_os.len());
    for scope in &targets_os {
        targets.push(decode_scope(scope.as_os_str())?);
    }
    if flag_workspace.as_deref().is_some_and(str::is_empty) {
        return Err(ArgsError::MissingValue {
            option: "--workspace".to_owned(),
        });
    }
    use dx_adopt::defaults as invocation_defaults;
    let workspace = invocation_defaults::resolve_workspace(
        flag_workspace,
        invocation_defaults::env_string(env_get, invocation_defaults::DX_WORKSPACE_ENV),
        file.workspace.clone(),
    );
    let dry_run = invocation_defaults::resolve_bool(
        dry_run,
        invocation_defaults::env_bool(env_get, invocation_defaults::DX_DRY_RUN_ENV)
            .map_err(bad_default)?,
        file.dry_run,
    );
    let quiet = invocation_defaults::resolve_bool(
        quiet,
        invocation_defaults::env_bool(env_get, invocation_defaults::DX_QUIET_ENV)
            .map_err(bad_default)?,
        file.quiet,
    );
    let verbose = invocation_defaults::resolve_bool(
        verbose,
        invocation_defaults::env_bool(env_get, invocation_defaults::DX_VERBOSE_ENV)
            .map_err(bad_default)?,
        file.verbose,
    );
    let output_name = invocation_defaults::resolve_string(
        output,
        invocation_defaults::env_string(env_get, invocation_defaults::DX_OUTPUT_ENV),
        file.output.clone(),
        "text",
    );
    let fail_on_given = fail_on.is_some();
    let fail_on_name = invocation_defaults::resolve_string(
        fail_on,
        invocation_defaults::env_string(env_get, invocation_defaults::DX_FAIL_ON_ENV),
        file.fail_on.clone(),
        "warning",
    );
    let log_level = match log_level_name.as_deref() {
        None => None,
        Some(value) => {
            Some(
                dx_output::LogLevel::parse(value).map_err(|_| ArgsError::BadLogLevel {
                    value: value.to_owned(),
                })?,
            )
        }
    };
    if verbose && log_level.is_some() {
        return Err(ArgsError::ConflictingVerboseLogLevel);
    }
    let color_name = invocation_defaults::resolve_string(
        color_name,
        invocation_defaults::env_string(env_get, invocation_defaults::DX_COLOR_ENV),
        file.color.clone(),
        "auto",
    );
    let color = dx_output::ColorMode::parse(&color_name).map_err(|_| ArgsError::BadColor {
        value: color_name.clone(),
    })?;
    let reports = report;
    if here && !targets.is_empty() {
        return Err(ArgsError::ConflictingHere);
    }
    if command != Command::Bazel {
        for scope in &targets {
            if scope == "-" {
                return Err(ArgsError::UnknownOption {
                    option: scope.clone(),
                });
            }
            if scope.is_empty() || scope.starts_with(':') {
                return Err(scope_error(scope));
            }
        }
    }
    if (fail_on_given || fail_on_name != "warning") && !command.supports_fail_on() {
        return Err(unsupported(command, "--fail-on"));
    }
    if command == Command::Clean {
        reject_diff_output(command, &output_name)?;
        reject_report(command, &reports)?;
        if let Some(scope) = targets.first() {
            return Err(unsupported(command, scope.clone()));
        }
        reject_passthrough(command, &bazel_options)?;
    }
    if command.is_managed() {
        reject_diff_output(command, &output_name)?;
        reject_report(command, &reports)?;
        if let Err(error) = dx_setup::resolve_scope(&targets) {
            return Err(match error {
                dx_setup::ScopeError::MultipleTargets { count } => ArgsError::UnsupportedOption {
                    command: command.name(),
                    option: targets
                        .get(1)
                        .cloned()
                        .unwrap_or(format!("<{count} targets>")),
                },
                dx_setup::ScopeError::TargetPattern { value }
                | dx_setup::ScopeError::NotTargetLabel { value } => scope_error(&value),
            });
        }
    }
    if (command == Command::Security || command == Command::License) && !bazel_options.is_empty() {
        return Err(unsupported(command, "--"));
    }
    if matches!(
        command,
        Command::Update | Command::Bump | Command::Migrate | Command::Upgrade
    ) {
        reject_diff_output(command, &output_name)?;
        reject_report(command, &reports)?;
        reject_passthrough(command, &bazel_options)?;
    }
    if command == Command::Bump {
        if targets.len() > 2 {
            return Err(extra_positional(command, &targets[2]));
        }
        if targets.len() < 2 {
            return Err(missing_positional(command));
        }
    }
    if matches!(command, Command::Migrate | Command::Upgrade) && (from.is_none() || to.is_none()) {
        return Err(ArgsError::MissingValue {
            option: "--from <version> --to <version>".to_owned(),
        });
    }
    if command == Command::Upgrade {
        if let Some(scope) = targets.first() {
            return Err(unsupported(command, scope.clone()));
        }
    }
    if command == Command::Docs {
        reject_diff_output(command, &output_name)?;
        reject_report(command, &reports)?;
        if port.is_some() && !serve {
            return Err(unsupported(command, "--port"));
        }
        if host.is_some() && !serve {
            return Err(unsupported(command, "--host"));
        }
        if open && !serve {
            return Err(unsupported(command, "--open"));
        }
    }
    let output = OutputMode::parse(&output_name, quiet).map_err(|_| ArgsError::BadOutput {
        value: output_name.clone(),
    })?;
    let fail_on = Threshold::parse(&fail_on_name).map_err(|_| ArgsError::BadFailOn {
        value: fail_on_name.clone(),
    })?;
    if command.is_adoption() {
        reject_report(command, &reports)?;
        reject_passthrough(command, &bazel_options)?;
        match command {
            Command::Status | Command::Version => {
                if !targets.is_empty() && command == Command::Status {
                    return Err(extra_positional(command, &targets[0]));
                }
                if !targets.is_empty() && command == Command::Version && pin.is_none() {
                    return Err(extra_positional(command, &targets[0]));
                }
            }
            Command::Completion => {
                if targets.len() > 1 {
                    return Err(extra_positional(command, &targets[1]));
                }
                if !check && targets.is_empty() {
                    return Err(missing_positional(command));
                }
            }
            Command::Hooks if targets.is_empty() => {
                return Err(missing_positional(command));
            }
            Command::Hooks if targets.len() > 2 => {
                return Err(extra_positional(command, &targets[2]));
            }
            Command::Watch | Command::Owners | Command::Deps if targets.is_empty() => {
                return Err(missing_positional(command));
            }
            Command::Why => {
                if targets.len() > 2 {
                    return Err(extra_positional(command, &targets[2]));
                }
                if targets.len() < 2 {
                    return Err(missing_positional(command));
                }
            }
            Command::Init if targets.len() > 1 => {
                return Err(extra_positional(command, &targets[1]));
            }
            Command::New => {
                if targets.len() > 2 {
                    return Err(extra_positional(command, &targets[2]));
                }
                if targets.is_empty() {
                    return Err(missing_positional(command));
                }
            }
            Command::Upgrade if !targets.is_empty() => {
                return Err(extra_positional(command, &targets[0]));
            }
            _ => {}
        }
    }
    if command == Command::Bazel {
        if output_name != "text" {
            return Err(unsupported(command, format!("--output={output_name}")));
        }
        reject_report(command, &reports)?;
        if bazel_clean {
            return Err(unsupported(command, "--bazel"));
        }
    }
    if command == Command::Run || command == Command::Deploy {
        if command == Command::Deploy {
            if !matches!(output, OutputMode::Text { .. }) {
                return Err(unsupported(command, format!("--output={output_name}")));
            }
        } else if output_name == "diff" {
            return Err(unsupported(command, "--output=diff"));
        }
        reject_report(command, &reports)?;
    }
    if output_name == "json" && !command.supports_json() {
        return Err(unsupported(command, "--output=json"));
    }
    if output_name == "diff" && !command.supports_diff() {
        return Err(unsupported(command, "--output=diff"));
    }
    Ok(Invocation {
        command,
        check,
        debug,
        release,
        workspace,
        dry_run,
        quiet,
        verbose,
        log_level,
        color,
        output,
        reports,
        fail_on,
        min_coverage,
        targets,
        bazel_options,
        bazel_clean,
        pin,
        rollback,
        configured,
        from,
        to,
        here,
        serve,
        port,
        host,
        open,
        offline,
    })
}

#[cfg(test)]
#[path = "defaults_tests.rs"]
mod defaults_tests;
#[cfg(test)]
#[path = "parser_commands.rs"]
mod parser_commands;
#[cfg(test)]
#[path = "parser_core.rs"]
mod parser_core;
#[cfg(test)]
#[path = "strict_tests.rs"]
mod strict_tests;
