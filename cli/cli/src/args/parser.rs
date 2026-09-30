use std::ffi::OsStr;

use dx_output::{OutputMode, Threshold};

use super::command::Command;
use super::tokenizer::tokenize;
use super::values::{parse_min_coverage, parse_report, scope_error};
use super::{ArgsError, Invocation};

pub use super::grammar::cli_command;
pub(crate) use super::grammar::Cli;

fn decode_scope(value: &OsStr) -> Result<String, ArgsError> {
    match value.to_str() {
        Some(text) => Ok(text.to_owned()),
        None => Err(ArgsError::InvalidScope {
            scope: value.to_string_lossy().into_owned(),
        }),
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
    let (cli, bazel_options) = tokenize(args)?;
    let Cli {
        workspace: workspace_os,
        dry_run,
        quiet,
        verbose,
        log_level: log_level_name,
        color: color_name,
        output,
        report,
        fail_on,
        min_coverage: min_coverage_name,
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
        port: port_name,
        host: host_name,
        open,
        offline,
        command: command_name,
        targets: targets_os,
        ..
    } = cli;
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
        invocation_defaults::env_bool(env_get, invocation_defaults::DX_DRY_RUN_ENV),
        file.dry_run,
    );
    let quiet = invocation_defaults::resolve_bool(
        quiet,
        invocation_defaults::env_bool(env_get, invocation_defaults::DX_QUIET_ENV),
        file.quiet,
    );
    let verbose = invocation_defaults::resolve_bool(
        verbose,
        invocation_defaults::env_bool(env_get, invocation_defaults::DX_VERBOSE_ENV),
        file.verbose,
    );
    if pin.as_deref().is_some_and(str::is_empty) {
        return Err(ArgsError::MissingValue {
            option: "--pin".to_owned(),
        });
    }
    if from.as_deref().is_some_and(str::is_empty) {
        return Err(ArgsError::MissingValue {
            option: "--from".to_owned(),
        });
    }
    if to.as_deref().is_some_and(str::is_empty) {
        return Err(ArgsError::MissingValue {
            option: "--to".to_owned(),
        });
    }
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
    let mut reports = Vec::new();
    for value in &report {
        reports.push(parse_report(value)?);
    }
    let mut min_coverage: Option<u32> = None;
    if let Some(value) = &min_coverage_name {
        min_coverage = Some(parse_min_coverage(value)?);
    }
    let mut port: Option<u16> = None;
    if let Some(value) = &port_name {
        if value.is_empty() {
            return Err(ArgsError::MissingValue {
                option: "--port".to_owned(),
            });
        }
        match value.parse::<u16>() {
            Ok(port_value) if port_value != 0 => port = Some(port_value),
            _ => {
                return Err(ArgsError::MissingValue {
                    option: "--port".to_owned(),
                });
            }
        }
    }
    let mut host: Option<String> = None;
    if let Some(value) = &host_name {
        if value.is_empty() {
            return Err(ArgsError::MissingValue {
                option: "--host".to_owned(),
            });
        }
        host = Some(value.clone());
    }
    let command = command_name.ok_or(ArgsError::MissingCommand)?;
    if here && !command.supports_here() {
        return Err(ArgsError::UnsupportedOption {
            command: command.name(),
            option: "--here".to_owned(),
        });
    }
    if here && !targets.is_empty() {
        return Err(ArgsError::ConflictingHere);
    }
    if offline && !command.supports_offline() {
        return Err(ArgsError::UnsupportedOption {
            command: command.name(),
            option: "--offline".to_owned(),
        });
    }
    if command != Command::Bazel {
        for scope in &targets {
            if scope == "-" {
                return Err(ArgsError::UnknownOption {
                    option: scope.clone(),
                    suggestion: None,
                });
            }
            if scope.is_empty() || scope.starts_with(':') {
                return Err(scope_error(scope));
            }
        }
    }
    if check && !command.supports_check() {
        return Err(ArgsError::UnsupportedOption {
            command: command.name(),
            option: "--check".to_owned(),
        });
    }
    if (fail_on_given || fail_on_name != "warning") && !command.supports_fail_on() {
        return Err(ArgsError::UnsupportedOption {
            command: command.name(),
            option: "--fail-on".to_owned(),
        });
    }
    if command == Command::Clean {
        if output_name == "diff" {
            return Err(ArgsError::UnsupportedOption {
                command: command.name(),
                option: "--output=diff".to_owned(),
            });
        }
        if let Some(request) = reports.first() {
            return Err(ArgsError::UnsupportedOption {
                command: command.name(),
                option: format!("--report={}={}", request.format, request.destination),
            });
        }
        if let Some(scope) = targets.first() {
            return Err(ArgsError::UnsupportedOption {
                command: command.name(),
                option: scope.clone(),
            });
        }
        if !bazel_options.is_empty() {
            return Err(ArgsError::UnsupportedOption {
                command: command.name(),
                option: "--".to_owned(),
            });
        }
    } else if bazel_clean {
        return Err(ArgsError::UnsupportedOption {
            command: command.name(),
            option: "--bazel".to_owned(),
        });
    }
    if command.is_managed() {
        if output_name == "diff" {
            return Err(ArgsError::UnsupportedOption {
                command: command.name(),
                option: "--output=diff".to_owned(),
            });
        }
        if let Some(request) = reports.first() {
            return Err(ArgsError::UnsupportedOption {
                command: command.name(),
                option: format!("--report={}={}", request.format, request.destination),
            });
        }
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
        return Err(ArgsError::UnsupportedOption {
            command: command.name(),
            option: "--".to_owned(),
        });
    }
    if command == Command::Update {
        if output_name == "diff" {
            return Err(ArgsError::UnsupportedOption {
                command: command.name(),
                option: "--output=diff".to_owned(),
            });
        }
        if let Some(request) = reports.first() {
            return Err(ArgsError::UnsupportedOption {
                command: command.name(),
                option: format!("--report={}={}", request.format, request.destination),
            });
        }
        if !bazel_options.is_empty() {
            return Err(ArgsError::UnsupportedOption {
                command: command.name(),
                option: "--".to_owned(),
            });
        }
    }
    if command == Command::Bump {
        if output_name == "diff" {
            return Err(ArgsError::UnsupportedOption {
                command: command.name(),
                option: "--output=diff".to_owned(),
            });
        }
        if let Some(request) = reports.first() {
            return Err(ArgsError::UnsupportedOption {
                command: command.name(),
                option: format!("--report={}={}", request.format, request.destination),
            });
        }
        if !bazel_options.is_empty() {
            return Err(ArgsError::UnsupportedOption {
                command: command.name(),
                option: "--".to_owned(),
            });
        }
        if targets.len() != 2 {
            return Err(ArgsError::MissingValue {
                option: "<selector> <version>".to_owned(),
            });
        }
    }
    if command == Command::Migrate {
        if output_name == "diff" {
            return Err(ArgsError::UnsupportedOption {
                command: command.name(),
                option: "--output=diff".to_owned(),
            });
        }
        if let Some(request) = reports.first() {
            return Err(ArgsError::UnsupportedOption {
                command: command.name(),
                option: format!("--report={}={}", request.format, request.destination),
            });
        }
        if !bazel_options.is_empty() {
            return Err(ArgsError::UnsupportedOption {
                command: command.name(),
                option: "--".to_owned(),
            });
        }
        if from.is_none() || to.is_none() {
            return Err(ArgsError::MissingValue {
                option: "--from <version> --to <version>".to_owned(),
            });
        }
    }
    if command != Command::Migrate
        && command != Command::Upgrade
        && (from.is_some() || to.is_some())
    {
        return Err(ArgsError::UnsupportedOption {
            command: command.name(),
            option: if from.is_some() {
                "--from".to_owned()
            } else {
                "--to".to_owned()
            },
        });
    }
    if command == Command::Upgrade {
        if output_name == "diff" {
            return Err(ArgsError::UnsupportedOption {
                command: command.name(),
                option: "--output=diff".to_owned(),
            });
        }
        if let Some(request) = reports.first() {
            return Err(ArgsError::UnsupportedOption {
                command: command.name(),
                option: format!("--report={}={}", request.format, request.destination),
            });
        }
        if !bazel_options.is_empty() {
            return Err(ArgsError::UnsupportedOption {
                command: command.name(),
                option: "--".to_owned(),
            });
        }
        if from.is_none() || to.is_none() {
            return Err(ArgsError::MissingValue {
                option: "--from <version> --to <version>".to_owned(),
            });
        }
        if let Some(scope) = targets.first() {
            return Err(ArgsError::UnsupportedOption {
                command: command.name(),
                option: scope.clone(),
            });
        }
    }
    if command == Command::Docs {
        if output_name == "diff" {
            return Err(ArgsError::UnsupportedOption {
                command: command.name(),
                option: "--output=diff".to_owned(),
            });
        }
        if let Some(request) = reports.first() {
            return Err(ArgsError::UnsupportedOption {
                command: command.name(),
                option: format!("--report={}={}", request.format, request.destination),
            });
        }
        if port.is_some() && !serve {
            return Err(ArgsError::UnsupportedOption {
                command: command.name(),
                option: "--port".to_owned(),
            });
        }
        if host.is_some() && !serve {
            return Err(ArgsError::UnsupportedOption {
                command: command.name(),
                option: "--host".to_owned(),
            });
        }
        if open && !serve {
            return Err(ArgsError::UnsupportedOption {
                command: command.name(),
                option: "--open".to_owned(),
            });
        }
    }
    if command != Command::Docs && serve {
        return Err(ArgsError::UnsupportedOption {
            command: command.name(),
            option: "--serve".to_owned(),
        });
    }
    if command != Command::Docs && port.is_some() {
        return Err(ArgsError::UnsupportedOption {
            command: command.name(),
            option: "--port".to_owned(),
        });
    }
    if command != Command::Docs && host.is_some() {
        return Err(ArgsError::UnsupportedOption {
            command: command.name(),
            option: "--host".to_owned(),
        });
    }
    if command != Command::Docs && open {
        return Err(ArgsError::UnsupportedOption {
            command: command.name(),
            option: "--open".to_owned(),
        });
    }
    if min_coverage.is_some() && command != Command::Coverage {
        return Err(ArgsError::UnsupportedOption {
            command: command.name(),
            option: "--min-coverage".to_owned(),
        });
    }
    let output = OutputMode::parse(&output_name, quiet).map_err(|_| ArgsError::BadOutput {
        value: output_name.clone(),
    })?;
    let fail_on = Threshold::parse(&fail_on_name).map_err(|_| ArgsError::BadFailOn {
        value: fail_on_name.clone(),
    })?;
    if command.is_adoption() {
        if let Some(request) = reports.first() {
            return Err(ArgsError::UnsupportedOption {
                command: command.name(),
                option: format!("--report={}={}", request.format, request.destination),
            });
        }
        if bazel_clean {
            return Err(ArgsError::UnsupportedOption {
                command: command.name(),
                option: "--bazel".to_owned(),
            });
        }
        if !bazel_options.is_empty() {
            return Err(ArgsError::UnsupportedOption {
                command: command.name(),
                option: "--".to_owned(),
            });
        }
        match command {
            Command::Status | Command::Version => {
                if !targets.is_empty() && command == Command::Status {
                    return Err(ArgsError::UnsupportedOption {
                        command: command.name(),
                        option: targets[0].clone(),
                    });
                }
                if !targets.is_empty() && command == Command::Version && pin.is_none() {
                    return Err(ArgsError::UnsupportedOption {
                        command: command.name(),
                        option: targets[0].clone(),
                    });
                }
            }
            Command::Completion => {
                if check {
                    if targets.len() > 1 {
                        return Err(ArgsError::UnsupportedOption {
                            command: command.name(),
                            option: targets[1].clone(),
                        });
                    }
                } else if targets.len() != 1 {
                    return Err(ArgsError::MissingValue {
                        option: "<shell>".to_owned(),
                    });
                }
            }
            Command::Hooks | Command::Watch | Command::Owners | Command::Deps
                if targets.is_empty() =>
            {
                return Err(ArgsError::MissingValue {
                    option: "<scope>".to_owned(),
                });
            }
            Command::Why if targets.len() != 2 => {
                return Err(ArgsError::MissingValue {
                    option: "<file> <label>".to_owned(),
                });
            }
            Command::Init if targets.len() > 1 => {
                return Err(ArgsError::UnsupportedOption {
                    command: command.name(),
                    option: targets[1].clone(),
                });
            }
            Command::New if targets.is_empty() || targets.len() > 2 => {
                return Err(ArgsError::MissingValue {
                    option: "<language> [name]".to_owned(),
                });
            }
            Command::Upgrade if !targets.is_empty() => {
                return Err(ArgsError::UnsupportedOption {
                    command: command.name(),
                    option: targets[0].clone(),
                });
            }
            _ => {}
        }
    }
    if command == Command::Bazel {
        if output_name != "text" {
            return Err(ArgsError::UnsupportedOption {
                command: command.name(),
                option: format!("--output={output_name}"),
            });
        }
        if let Some(request) = reports.first() {
            return Err(ArgsError::UnsupportedOption {
                command: command.name(),
                option: format!("--report={}={}", request.format, request.destination),
            });
        }
        if bazel_clean {
            return Err(ArgsError::UnsupportedOption {
                command: command.name(),
                option: "--bazel".to_owned(),
            });
        }
    }
    if command == Command::Run || command == Command::Deploy {
        if command == Command::Deploy {
            if !matches!(output, OutputMode::Text { .. }) {
                return Err(ArgsError::UnsupportedOption {
                    command: command.name(),
                    option: format!("--output={output_name}"),
                });
            }
        } else if output_name == "diff" {
            return Err(ArgsError::UnsupportedOption {
                command: command.name(),
                option: "--output=diff".to_owned(),
            });
        }
        if let Some(request) = reports.first() {
            return Err(ArgsError::UnsupportedOption {
                command: command.name(),
                option: format!("--report={}={}", request.format, request.destination),
            });
        }
    }
    if output_name == "json" && !command.supports_json() {
        return Err(ArgsError::UnsupportedOption {
            command: command.name(),
            option: "--output=json".to_owned(),
        });
    }
    if output_name == "diff" && !command.supports_diff() {
        return Err(ArgsError::UnsupportedOption {
            command: command.name(),
            option: "--output=diff".to_owned(),
        });
    }
    if debug && release {
        return Err(ArgsError::ConflictingProfiles);
    }
    if (debug || release) && !command.supports_profile() {
        return Err(ArgsError::UnsupportedOption {
            command: command.name(),
            option: if debug {
                "--debug".to_owned()
            } else {
                "--release".to_owned()
            },
        });
    }
    if rollback && command != Command::Version {
        return Err(ArgsError::UnsupportedOption {
            command: command.name(),
            option: "--rollback".to_owned(),
        });
    }
    if pin.is_some() && command != Command::Version {
        return Err(ArgsError::UnsupportedOption {
            command: command.name(),
            option: "--pin".to_owned(),
        });
    }
    if configured
        && command != Command::Owners
        && command != Command::Deps
        && command != Command::Why
    {
        return Err(ArgsError::UnsupportedOption {
            command: command.name(),
            option: "--configured".to_owned(),
        });
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
