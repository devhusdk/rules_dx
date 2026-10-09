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

/// The startup defaults and a file-directed workspace hop for one more load.
#[derive(Debug)]
pub struct StartupDefaults {
    /// The defaults read from the preliminary workspace.
    pub defaults: super::FileDefaults,
    /// The config-directed workspace when no flag or environment value set one.
    pub file_workspace: Option<String>,
}

/// Reads the `--workspace` value off the raw command line, if one is spelled.
pub fn early_workspace_flag<S: AsRef<OsStr>>(args: &[S]) -> Option<String> {
    let words: Vec<&str> = args
        .iter()
        .map(|word| word.as_ref().to_str())
        .take_while(|word| word.is_some_and(|word| word != "--"))
        .map(|word| word.unwrap_or(""))
        .collect();
    let mut index = 0;
    while index < words.len() {
        let word = words[index];
        if let Some(value) = word.strip_prefix("--workspace=") {
            return (!value.is_empty()).then(|| value.to_owned());
        }
        if word == "--workspace" {
            match words.get(index + 1) {
                Some(next) if !next.is_empty() && !next.starts_with('-') => {
                    return Some((*next).to_owned());
                }
                _ => return None,
            }
        }
        if !word.contains('=') && super::grammar::VALUE_OPTIONS.contains(&word) {
            index += 1;
        }
        index += 1;
    }
    None
}

/// Whether the raw command line asks for help or the delivered version.
pub fn is_help_request<S: AsRef<OsStr>>(args: &[S]) -> bool {
    if super::help::help_verb_error_in(args).is_some() {
        return true;
    }
    if args
        .first()
        .is_some_and(|first| first.as_ref() == OsStr::new("bazel"))
    {
        return false;
    }
    let words: Vec<&str> = args
        .iter()
        .map(|word| word.as_ref().to_str())
        .take_while(|word| word.is_some_and(|word| word != "--"))
        .map(|word| word.unwrap_or(""))
        .collect();
    let mut index = 0;
    while index < words.len() {
        let word = words[index];
        if word == "-h"
            || word == "--help"
            || word == "-V"
            || word == "--version"
            || word.starts_with("--help=")
            || word.starts_with("--version=")
        {
            return true;
        }
        if !word.contains('=') && super::grammar::VALUE_OPTIONS.contains(&word) {
            index += 1;
        }
        index += 1;
    }
    false
}

/// Whether the command runs without MODULE.bazel discovery.
pub fn is_discovery_exempt(command: Command) -> bool {
    matches!(
        command,
        Command::Init | Command::New | Command::Completion | Command::Capabilities
    )
}

/// Loads defaults from the selected workspace: the flag, the environment, else
/// the start directory. A config-directed workspace is reported for one more
/// load instead of being followed here, so a redirect cycle cannot loop.
pub fn select_startup_defaults(
    start: &std::path::Path,
    flag_workspace: Option<String>,
    env_workspace: Option<String>,
) -> Result<StartupDefaults, String> {
    let preliminary = flag_workspace.or(env_workspace);
    let dir = match &preliminary {
        Some(raw) => dx_process::resolve_override_display(std::path::Path::new(raw), start),
        None => start.to_path_buf(),
    };
    match dx_adopt::defaults::load_defaults(&dir) {
        Ok((defaults, _)) => {
            let file_workspace = match preliminary {
                Some(_) => None,
                None => defaults.workspace.clone(),
            };
            Ok(StartupDefaults {
                defaults,
                file_workspace,
            })
        }
        Err(error) => Err(error.to_string()),
    }
}

/// Returns the defaults with the workspace pinned to the redirect target, so a
/// second load never follows another redirect.
pub fn freeze_workspace(defaults: &super::FileDefaults, workspace: &str) -> super::FileDefaults {
    let mut frozen = defaults.clone();
    frozen.workspace = Some(workspace.to_owned());
    frozen
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
        strict_evidence,
        check,
        apply,
        debug,
        release,
        bazel_clean,
        prune_unobserved,
        pin,
        rollback,
        configured,
        from,
        to,
        here,
        workspace_capabilities,
        serve,
        port,
        host,
        open,
        offline,
        bazel_startup_options: startup_tokens,
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
    if dry_run && apply {
        return Err(ArgsError::ConflictingModes {
            first: "--dry-run",
            second: "--apply",
        });
    }
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
    if check && apply {
        return Err(ArgsError::ConflictingModes {
            first: "--check",
            second: "--apply",
        });
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
    let mut bazel_startup_options = Vec::with_capacity(startup_tokens.len());
    for token in &startup_tokens {
        match dx_process::validate_startup_option(token) {
            Ok(kept) => bazel_startup_options.push(kept),
            Err(dx_process::StartupOptionError::MissingValue { flag }) => {
                return Err(ArgsError::BadStartupOption {
                    value: format!("--{flag}"),
                });
            }
            Err(dx_process::StartupOptionError::UnsupportedStartup { flag }) => {
                return Err(ArgsError::BadStartupOption { value: flag });
            }
        }
    }
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
            Command::Capabilities => {
                if let Some(scope) = targets.first() {
                    return Err(extra_positional(command, scope));
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
        strict_evidence,
        apply,
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
        prune_unobserved,
        pin,
        rollback,
        configured,
        from,
        to,
        here,
        workspace_capabilities,
        serve,
        port,
        host,
        open,
        offline,
        bazel_startup_options,
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

#[cfg(test)]
mod startup_tests {
    use super::*;
    use crate::test_support::strings;

    fn scratch_with_config(name: &str, config: Option<&str>) -> tempfile::TempDir {
        let scratch = dx_test_scratch::scratch(name);
        std::fs::create_dir_all(scratch.path().join(".dx")).expect("dx dir");
        if let Some(text) = config {
            std::fs::write(scratch.path().join(".dx/config.toml"), text).expect("config");
        }
        scratch
    }

    #[test]
    fn early_workspace_flag_reads_both_spellings_after_the_command() {
        assert_eq!(
            early_workspace_flag(&strings(&["lint", "--workspace", "/repo"])),
            Some("/repo".to_owned())
        );
        assert_eq!(
            early_workspace_flag(&strings(&["lint", "--workspace=/repo"])),
            Some("/repo".to_owned())
        );
        assert_eq!(
            early_workspace_flag(&strings(&["--workspace", "/repo", "lint"])),
            Some("/repo".to_owned())
        );
        assert_eq!(early_workspace_flag(&strings(&["lint"])), None);
        assert_eq!(
            early_workspace_flag(&strings(&["lint", "--workspace="])),
            None
        );
        assert_eq!(
            early_workspace_flag(&strings(&["lint", "--workspace"])),
            None
        );
        assert_eq!(
            early_workspace_flag(&strings(&["lint", "--workspace", "--quiet"])),
            None
        );
        assert_eq!(
            early_workspace_flag(&strings(&["lint", "--", "--workspace=/repo"])),
            None
        );
        assert_eq!(
            early_workspace_flag(&strings(&[
                "lint",
                "--output",
                "json",
                "--workspace",
                "/repo"
            ])),
            Some("/repo".to_owned())
        );
    }

    #[test]
    fn help_requests_cover_verbs_flags_and_the_delivered_version() {
        for words in [
            vec!["--help"],
            vec!["-h"],
            vec!["--version"],
            vec!["-V"],
            vec!["help"],
            vec!["help", "lint"],
            vec!["lint", "--help"],
            vec!["lint", "--output", "json", "--help"],
            vec!["version", "--workspace=/repo", "--help"],
        ] {
            assert!(is_help_request(&strings(&words)), "{words:?} asks for help");
        }
        for words in [
            vec!["lint"],
            vec!["lint", "--check"],
            vec!["new", "rust", "demo"],
            vec!["version"],
            vec!["bazel", "--help"],
            vec!["lint", "--", "--help"],
            vec!["lint", "--output", "--help"],
        ] {
            assert!(
                !is_help_request(&strings(&words)),
                "{words:?} is operational"
            );
        }
    }

    #[test]
    fn discovery_runs_without_a_module_only_for_init_new_and_completion() {
        for command in [
            Command::Init,
            Command::New,
            Command::Completion,
            Command::Capabilities,
        ] {
            assert!(is_discovery_exempt(command), "{command:?} is exempt");
        }
        for command in [
            Command::Lint,
            Command::Build,
            Command::Status,
            Command::Version,
            Command::Docs,
            Command::Bazel,
            Command::Check,
            Command::Update,
        ] {
            assert!(
                !is_discovery_exempt(command),
                "{command:?} needs a workspace"
            );
        }
    }

    #[test]
    fn startup_defaults_come_from_the_selected_workspace() {
        let a = scratch_with_config("startup-select-a-", Some("[dx]\noutput = \"json\"\n"));
        let b = scratch_with_config("startup-select-b-", Some("[dx]\noutput = \"text\"\n"));
        let a_root = a.path().to_path_buf();
        let selected =
            select_startup_defaults(&a_root, None, None).expect("loads the start directory");
        assert_eq!(selected.defaults.output, Some("json".to_owned()));
        assert_eq!(selected.file_workspace, None);
        let selected =
            select_startup_defaults(&a_root, Some(b.path().to_string_lossy().into_owned()), None)
                .expect("loads the flag workspace");
        assert_eq!(
            selected.defaults.output,
            Some("text".to_owned()),
            "the flag workspace supplies the defaults"
        );
        assert_eq!(selected.file_workspace, None);
        let selected =
            select_startup_defaults(&a_root, None, Some(b.path().to_string_lossy().into_owned()))
                .expect("loads the environment workspace");
        assert_eq!(selected.defaults.output, Some("text".to_owned()));
        let missing = a_root.join("no-such-dir");
        let selected = select_startup_defaults(&missing, None, None).expect("walks up");
        assert_eq!(selected.defaults.output, Some("json".to_owned()));
    }

    #[test]
    fn startup_reports_a_file_directed_workspace_for_one_more_load() {
        let b = scratch_with_config(
            "startup-redirect-b-",
            Some("[dx]\noutput = \"text\"\nworkspace = \"/elsewhere\"\n"),
        );
        let config = format!(
            "[dx]\noutput = \"json\"\nworkspace = \"{}\"\n",
            b.path().display()
        );
        let a = scratch_with_config("startup-redirect-a-", Some(&config));
        let selected =
            select_startup_defaults(a.path(), None, None).expect("loads the start directory");
        assert_eq!(selected.defaults.output, Some("json".to_owned()));
        let target = selected
            .file_workspace
            .expect("the file directs a workspace");
        assert_eq!(target, b.path().to_string_lossy());
        let reloaded =
            select_startup_defaults(b.path(), Some(target.clone()), None).expect("follows once");
        assert_eq!(
            reloaded.file_workspace, None,
            "the second load follows nothing"
        );
        let frozen = freeze_workspace(&reloaded.defaults, &target);
        assert_eq!(frozen.workspace, Some(target));
        assert_eq!(frozen.output, Some("text".to_owned()));
    }

    #[test]
    fn startup_malformed_config_fails_with_the_file_and_key() {
        let scratch = scratch_with_config("startup-malformed-", Some("not toml = ["));
        let error =
            select_startup_defaults(scratch.path(), None, None).expect_err("malformed fails");
        assert!(error.contains("config.toml"), "{error}");
    }
}
