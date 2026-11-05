use std::ffi::OsStr;

use dx_output::{OutputMode, Threshold};

use super::command::Command;
use super::grammar::{self, Flags};
use super::scope_error;
use super::tokenizer::tokenize;
use super::{ArgsError, Invocation};
use dx_worktree::HookKind;

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

/// Names an environment default dx will not read.
fn bad_default(error: dx_adopt::AdoptError) -> ArgsError {
    ArgsError::BadDefault {
        detail: error.to_string(),
    }
}

pub fn parse<S: AsRef<OsStr>>(args: &[S]) -> Result<Invocation, ArgsError> {
    parse_with(args, &|_| None)
}

/// Reads the last output value before `--`, short clusters included; an empty value counts.
pub fn early_output_flag<S: AsRef<OsStr>>(args: &[S]) -> Option<String> {
    let words: Vec<&str> = args
        .iter()
        .map(|word| word.as_ref().to_str())
        .take_while(|word| word.is_some_and(|word| word != "--"))
        .map(|word| word.unwrap_or(""))
        .collect();
    let mut seen: Option<String> = None;
    let mut index = 0;
    while index < words.len() {
        let next = words.get(index + 1).copied();
        if let Some(option) = super::grammar::option_at(words[index], next) {
            if matches!(option.name.as_str(), "--output" | "-o") && option.value.is_some() {
                seen = option.value;
            }
            if option.used_next {
                index += 1;
            }
        }
        index += 1;
    }
    seen
}

/// Whether a default output value applies to the command.
fn output_supported(command: Command, name: &str) -> bool {
    match name {
        "json" => command.supports_json(),
        _ => true,
    }
}

/// The one output resolution shared by startup guessing and full parsing.
///
/// Precedence is flag, then `DX_OUTPUT`, then the agent default. A non-flag value that does not apply to the command (or
/// the worktree verb) falls back to the agent default. `command` is `None`
/// while it is still unknown, in which case every value applies.
fn resolve_output_name(
    command: Option<Command>,
    flag: Option<&str>,
    env: Option<String>,
    agent: bool,
) -> String {
    if let Some(flag) = flag {
        return flag.to_owned();
    }
    if let Some(value) = env.as_deref() {
        if command.is_none_or(|command| output_supported(command, value)) {
            return value.to_owned();
        }
    }
    if agent && command.is_none_or(Command::supports_json) {
        "json".to_owned()
    } else {
        "text".to_owned()
    }
}

/// Whether a startup failure may report structured JSON events on stdout.
pub fn json_output_intent<S: AsRef<OsStr>>(
    args: &[S],
    env_get: &dyn Fn(&str) -> Option<String>,
    is_ci: bool,
) -> bool {
    if let Some(flag) = early_output_flag(args) {
        return flag == "json";
    }
    let command = super::help::help_command_in(args);
    let env = if is_ci {
        None
    } else {
        dx_adopt::defaults::env_string(env_get, dx_adopt::defaults::DX_OUTPUT_ENV)
    };
    let agent = !is_ci && dx_adopt::defaults::is_agent(env_get);
    resolve_output_name(command, None, env, agent) == "json"
}

/// Whether the command runs without MODULE.bazel discovery.
pub fn is_discovery_exempt(command: Command) -> bool {
    matches!(
        command,
        Command::Init | Command::Completion | Command::Worktree
    )
}

/// The workspace a run selects: the flag, then the environment.
pub fn startup_workspace(
    flag_workspace: Option<String>,
    env_workspace: Option<String>,
    is_ci: bool,
) -> Option<String> {
    let env_workspace = env_workspace
        .filter(|value| !value.is_empty())
        .filter(|_| !ci_ignores_env(dx_adopt::defaults::DX_WORKSPACE_ENV, is_ci));
    flag_workspace.or(env_workspace)
}

/// Parses a watch line as the command it wraps, then marks it as a watch.
fn wrapped_invocation<S: AsRef<OsStr>>(
    args: &[S],
    tokenized: &super::tokenizer::Tokenized,
    env_get: &dyn Fn(&str) -> Option<String>,
) -> Option<Result<Invocation, ArgsError>> {
    let wrapped = tokenized.targets.first()?.to_str()?.to_owned();
    if !dx_adopt::WATCHABLE_COMMANDS.contains(&wrapped.as_str()) {
        return None;
    }
    let watch_at = args
        .iter()
        .position(|word| word.as_ref().to_str().and_then(Command::parse) == Some(Command::Watch))?;
    let wrapped_at = args
        .iter()
        .enumerate()
        .skip(watch_at + 1)
        .find(|(_, word)| word.as_ref().to_str() == Some(wrapped.as_str()))?
        .0;
    // `--poll` is watch's own flag, so the wrapped command never sees it; words after `--` go to Bazel.
    let dashes_at = args
        .iter()
        .position(|word| word.as_ref().to_str() == Some("--"))
        .unwrap_or(args.len());
    let poll = args[..dashes_at]
        .iter()
        .any(|word| word.as_ref().to_str() == Some("--poll"));
    let mut rest: Vec<&OsStr> = vec![OsStr::new(&wrapped)];
    rest.extend(
        args.iter()
            .enumerate()
            .filter(|(index, word)| {
                *index != watch_at
                    && *index != wrapped_at
                    && !(*index < dashes_at && word.as_ref().to_str() == Some("--poll"))
            })
            .map(|(_, word)| word.as_ref()),
    );
    Some(parse_inner(&rest, env_get).map(|mut inner| {
        inner.command = Command::Watch;
        inner.poll = poll;
        inner.targets.insert(0, wrapped);
        inner
    }))
}

pub fn parse_with<S: AsRef<OsStr>>(
    args: &[S],
    env_get: &dyn Fn(&str) -> Option<String>,
) -> Result<Invocation, ArgsError> {
    parse_with_ci(args, env_get, false)
}

fn validate_worktree(targets: &[String]) -> Result<(), ArgsError> {
    let command = Command::Worktree;
    let Some(sub) = targets.first() else {
        return Ok(());
    };
    let (min, max) = match sub.as_str() {
        "add" | "path" => (2, 2),
        "release" => (1, 2),
        "list" | "prune" => (1, 1),
        "hook" => (2, 2),
        other => return Err(unsupported(command, other)),
    };
    if targets.len() < min {
        return Err(missing_positional(command));
    }
    if targets.len() > max {
        return Err(extra_positional(command, &targets[max]));
    }
    if sub == "hook" && HookKind::from_word(&targets[1]).is_none() {
        return Err(ArgsError::Usage {
            text: format!(
                "unknown dx worktree hook {:?}: want one of: {}",
                targets[1],
                HookKind::names()
            ),
        });
    }
    Ok(())
}

pub fn parse_with_ci<S: AsRef<OsStr>>(
    args: &[S],
    env_get: &dyn Fn(&str) -> Option<String>,
    is_ci: bool,
) -> Result<Invocation, ArgsError> {
    if is_ci {
        let filtered = |name: &str| {
            if ci_ignores_env(name, true) || dx_adopt::defaults::AGENT_ENVS.contains(&name) {
                None
            } else {
                env_get(name)
            }
        };
        return parse_inner(args, &filtered);
    }
    parse_inner(args, env_get)
}

/// Whether CI ignores this environment default.
pub fn ci_ignores_env(name: &str, is_ci: bool) -> bool {
    is_ci
        && matches!(
            name,
            dx_adopt::defaults::DX_WORKSPACE_ENV
                | dx_adopt::defaults::DX_OUTPUT_ENV
                | dx_adopt::defaults::DX_VERBOSE_ENV
                | dx_adopt::defaults::DX_COLOR_ENV
                | dx_adopt::defaults::DX_QUIET_ENV
                | dx_adopt::defaults::DX_FAIL_ON_ENV
                | dx_adopt::defaults::DX_BUILD_SLOTS_ENV
        )
}

/// Resolve `--build-slots` across flag, `DX_BUILD_SLOTS`, then unset.
fn resolve_build_slots(
    flag: Option<String>,
    env_get: &dyn Fn(&str) -> Option<String>,
) -> Result<Option<dx_adopt::defaults::BuildSlots>, ArgsError> {
    use dx_adopt::defaults::{env_string, BuildSlots, DX_BUILD_SLOTS_ENV};
    let (source, value) = match (flag, env_string(env_get, DX_BUILD_SLOTS_ENV)) {
        (Some(flag), _) => ("--build-slots", flag),
        (None, Some(env)) => (DX_BUILD_SLOTS_ENV, env),
        (None, None) => return Ok(None),
    };
    BuildSlots::parse(&value)
        .map(Some)
        .ok_or(ArgsError::BadBuildSlots {
            origin: source,
            value,
        })
}

fn parse_inner<S: AsRef<OsStr>>(
    args: &[S],
    env_get: &dyn Fn(&str) -> Option<String>,
) -> Result<Invocation, ArgsError> {
    if let Some(error) = super::help::help_verb_error_in(args) {
        return Err(error);
    }
    let tokenized = tokenize(args)?;
    if tokenized.command == Command::Watch {
        if let Some(wrapped) = wrapped_invocation(args, &tokenized, env_get) {
            return wrapped;
        }
    }
    let command = tokenized.command;
    let Flags {
        workspace: workspace_os,
        quiet,
        verbose,
        color: color_name,
        output,
        diff,
        poll,
        path_only,
        shell,
        report,
        fail_on,
        build_slots,
        include_edits,
        min_coverage,
        min_changed_line_coverage,
        apply,
        online,
        debug,
        release,
        prune_unobserved,
        agents,
        profile,
        to,
        here,
        affected,
        serve,
        port,
        host,
        open,
        offline,
        frozen,
        outdated,
        prune,
        pr,
        group,
        labels,
        automerge,
        min_age,
        ignore,
        allow_major,
        prereleases,
        max_prs,
        floor,
        reason,
        cases,
        create,
        force,
        disk,
        bases,
        capabilities,
        explain,
        build_profile,
        old_log,
        new_log,
        time,
        jobs,
        fail_survived,
        check,
        baseline,
        bazel_startup_options: startup_tokens,
        bazel_options: option_tokens,
    } = tokenized.flags;
    let targets_os = tokenized.targets;
    let passthrough = tokenized.passthrough;
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
    let workspace = flag_workspace.or(invocation_defaults::env_string(
        env_get,
        invocation_defaults::DX_WORKSPACE_ENV,
    ));
    let quiet = invocation_defaults::resolve_bool(
        quiet,
        invocation_defaults::env_bool(env_get, invocation_defaults::DX_QUIET_ENV)
            .map_err(bad_default)?,
    );
    let verbose = invocation_defaults::resolve_bool(
        verbose,
        invocation_defaults::env_bool(env_get, invocation_defaults::DX_VERBOSE_ENV)
            .map_err(bad_default)?,
    );
    let agent = invocation_defaults::is_agent(env_get);
    let output_from_flag = output.is_some();
    let fail_on_from_flag = fail_on.is_some();
    let apply_without_output_flag = command == Command::Init && apply && !output_from_flag;
    let output_name = resolve_output_name(
        Some(command),
        output.as_deref(),
        invocation_defaults::env_string(env_get, invocation_defaults::DX_OUTPUT_ENV),
        agent && !apply_without_output_flag,
    );
    let fail_on_name = invocation_defaults::resolve_string(
        fail_on,
        invocation_defaults::env_string(env_get, invocation_defaults::DX_FAIL_ON_ENV),
        if command == Command::Unused {
            "none"
        } else {
            "warning"
        },
    );
    let build_slots = resolve_build_slots(build_slots, env_get)?;
    let color_name = invocation_defaults::resolve_string(
        color_name,
        invocation_defaults::env_string(env_get, invocation_defaults::DX_COLOR_ENV),
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
    if affected && here {
        return Err(ArgsError::AffectedWithHere);
    }
    if affected && !targets.is_empty() {
        return Err(ArgsError::ConflictingAffected);
    }
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
    let fail_on_uncovered = fail_on_name == "uncovered" && command == Command::Security;
    if fail_on_name == "uncovered" && !fail_on_uncovered && fail_on_from_flag {
        return Err(ArgsError::BadFailOn {
            value: fail_on_name,
        });
    }
    let fail_on_no_collector = fail_on_name == "no-collector" && command == Command::Coverage;
    if fail_on_name == "no-collector" && !fail_on_no_collector && fail_on_from_flag {
        return Err(ArgsError::BadFailOn {
            value: fail_on_name,
        });
    }
    let fail_on_unused = fail_on_name == "unused" && command == Command::Update;
    if fail_on_name == "unused" && !fail_on_unused && fail_on_from_flag {
        return Err(ArgsError::BadFailOn {
            value: fail_on_name,
        });
    }
    let fail_on_none = fail_on_name == "none" && command == Command::Unused;
    if fail_on_name == "none" && !fail_on_none {
        return Err(ArgsError::BadFailOn {
            value: fail_on_name,
        });
    }
    let mut fail_on = if fail_on_none {
        // The unused tools only emit warnings, so only a tool error fails the run.
        Threshold::Error
    } else if matches!(fail_on_name.as_str(), "uncovered" | "no-collector" | "unused") {
        Threshold::Warning
    } else {
        Threshold::parse(&fail_on_name).map_err(|_| ArgsError::BadFailOn {
            value: fail_on_name.clone(),
        })?
    };
    if fail_on_name != "warning" && command != Command::Watch && !command.supports_fail_on() {
        if fail_on_from_flag {
            return Err(unsupported(command, "--fail-on"));
        }
        fail_on = Threshold::Warning;
    }
    if command == Command::Clean {
        if let Some(scope) = targets.first() {
            return Err(unsupported(command, scope.clone()));
        }
    }
    if !passthrough.is_empty() && !grammar::advertises_passthrough(command) {
        return Err(unsupported(command, "--"));
    }
    let (mut bazel_options, app_args) = if grammar::takes_app_args(command) {
        (option_tokens, passthrough)
    } else {
        (passthrough, Vec::new())
    };
    if online {
        bazel_options.insert(0, "--//config:lint_online=true".to_owned());
    }
    if command.is_managed() {
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
    if command == Command::Release && check && apply {
        return Err(unsupported(command, "--check with --apply"));
    }
    if command == Command::Update && outdated {
        let culprit = if apply {
            Some("--apply")
        } else if floor {
            Some("--floor")
        } else if reason.is_some() {
            Some("--reason")
        } else if targets.len() == 2 && super::invocation::is_version(&targets[1]) {
            Some("<version>")
        } else {
            None
        };
        if let Some(option) = culprit {
            return Err(unsupported(command, option));
        }
    }
    if command == Command::Update && prune && (!apply || frozen) {
        return Err(unsupported(
            command,
            if apply {
                "--prune with --frozen"
            } else {
                "--prune without --apply"
            },
        ));
    }
    if command == Command::Update && pr {
        let culprit = if outdated {
            Some("--outdated")
        } else if prune {
            Some("--prune")
        } else if apply {
            Some("--apply")
        } else if floor {
            Some("--floor")
        } else if reason.is_some() {
            Some("--reason")
        } else if offline {
            Some("--offline")
        } else if targets.len() == 2 && super::invocation::is_version(&targets[1]) {
            Some("<version>")
        } else {
            None
        };
        if let Some(option) = culprit {
            return Err(unsupported(command, option));
        }
    }
    if command == Command::Update && !pr {
        let stray = [
            (group.is_some(), "--group"),
            (labels.is_some(), "--labels"),
            (automerge.is_some(), "--automerge"),
            (min_age.is_some(), "--min-age"),
            (!ignore.is_empty(), "--ignore"),
            (allow_major, "--allow-major"),
            (prereleases, "--prereleases"),
            (max_prs.is_some(), "--max-prs"),
        ];
        if let Some((_, name)) = stray.iter().find(|(set, _)| *set) {
            return Err(unsupported(command, *name));
        }
    }
    if command == Command::Update && (floor || reason.is_some()) {
        let versioned = targets.len() == 2 && super::invocation::is_version(&targets[1]);
        if !versioned {
            return Err(unsupported(
                command,
                if floor { "--floor" } else { "--reason" },
            ));
        }
    }
    if matches!(command, Command::Upgrade) && to.is_none() {
        return Err(ArgsError::MissingValue {
            option: "--to <version>".to_owned(),
        });
    }
    if command == Command::Generate {
        if let Some(scope) = targets.iter().find(|scope| {
            scope.starts_with('@') && !scope.starts_with("@//") && !scope.starts_with("@@//")
        }) {
            return Err(ArgsError::InvalidScope {
                scope: scope.clone(),
            });
        }
    }
    if matches!(command, Command::Explain | Command::Release) {
        if let Some(scope) = targets.first() {
            return Err(extra_positional(command, scope));
        }
    }
    if command == Command::Upgrade {
        if let Some(scope) = targets.first() {
            return Err(unsupported(command, scope.clone()));
        }
    }
    if command == Command::Docs {
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

    if command == Command::Worktree {
        validate_worktree(&targets)?;
        if path_only && targets.first().is_none_or(|verb| verb != "add") {
            return Err(unsupported(command, "--path-only"));
        }
    }
    if diff && output_from_flag && output_name == "json" {
        return Err(unsupported(command, "--diff"));
    }
    let output = if diff {
        OutputMode::Diff
    } else {
        OutputMode::parse(&output_name, quiet).map_err(|_| ArgsError::BadOutput {
            value: output_name.clone(),
        })?
    };
    if command.is_adoption() {
        match command {
            Command::Status => {
                if !targets.is_empty() {
                    return Err(extra_positional(command, &targets[0]));
                }
            }
            Command::Completion => {
                if targets.len() > 1 {
                    return Err(extra_positional(command, &targets[1]));
                }
                if targets.is_empty() {
                    return Err(missing_positional(command));
                }
            }
            Command::Watch if targets.is_empty() => {
                return Err(missing_positional(command));
            }
            Command::Init if targets.len() > 1 => {
                return Err(extra_positional(command, &targets[1]));
            }
            _ => {}
        }
    }
    if output_from_flag && !output_supported(command, &output_name) {
        return Err(unsupported(command, format!("--output={output_name}")));
    }
    if include_edits && output_name != "json" {
        return Err(ArgsError::NeedsJson {
            option: "--include-edits",
        });
    }
    let mut once = Vec::new();
    for agent in agents {
        if !once.contains(&agent) {
            once.push(agent);
        }
    }
    let agents = once;
    Ok(Invocation {
        command,
        apply,
        debug,
        release,
        workspace,
        quiet,
        verbose,
        color,
        output,
        reports,
        fail_on,
        fail_on_uncovered,
        fail_on_no_collector,
        fail_on_unused,
        build_slots,
        poll,
        baseline,
        include_edits,
        min_coverage,
        min_changed_line_coverage,
        targets,
        bazel_options,
        app_args,
        prune_unobserved,
        agents,
        profile,
        to,
        here,
        affected,
        affected_handoff: None,
        serve,
        port,
        host,
        open,
        offline,
        frozen,
        outdated,
        prune,
        pr,
        group,
        labels,
        automerge,
        min_age,
        ignore,
        allow_major,
        prereleases,
        max_prs,
        floor,
        reason,
        cases,
        create,
        force,
        disk,
        bases,
        capabilities,
        explain,
        build_profile,
        exec_log: None,
        old_log,
        new_log,
        time,
        jobs,
        fail_survived,
        path_only,
        shell,
        agent,
        bazel_startup_options,
    })
}

#[cfg(test)]
#[path = "affected_tests.rs"]
mod affected_tests;
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
#[path = "tutorial_tests.rs"]
mod tutorial_tests;

#[cfg(test)]
mod startup_tests {
    use super::*;
    use dx_testing::cli::strings;

    #[test]
    fn early_output_flag_reads_the_last_spelling_before_passthrough() {
        assert_eq!(
            early_output_flag(&strings(&["lint", "--output=json"])),
            Some("json".to_owned())
        );
        assert_eq!(
            early_output_flag(&strings(&["lint", "--output", "json"])),
            Some("json".to_owned())
        );
        assert_eq!(
            early_output_flag(&strings(&["lint", "--output=text", "--output", "json"])),
            Some("json".to_owned())
        );
        assert_eq!(
            early_output_flag(&strings(&["lint", "--output=json", "--output=text"])),
            Some("text".to_owned())
        );
        assert_eq!(
            early_output_flag(&strings(&["lint", "--output="])),
            Some(String::new())
        );
        assert_eq!(early_output_flag(&strings(&["lint"])), None);
        assert_eq!(early_output_flag(&strings(&["lint", "--output"])), None);
        assert_eq!(
            early_output_flag(&strings(&["lint", "--output", "--quiet"])),
            None
        );
        assert_eq!(
            early_output_flag(&strings(&["lint", "--", "--output=json"])),
            None
        );
        assert_eq!(
            early_output_flag(&strings(&["lint", "--to", "--output"])),
            None,
            "a value slot swallows the next word"
        );
        for short in [
            &["lint", "-o", "json"][..],
            &["lint", "-o=json"],
            &["lint", "-ojson"],
        ] {
            assert_eq!(
                early_output_flag(&strings(short)),
                Some("json".to_owned()),
                "{short:?}"
            );
        }
        assert_eq!(
            early_output_flag(&strings(&["lint", "-o="])),
            Some(String::new())
        );
        assert_eq!(early_output_flag(&strings(&["lint", "-o"])), None);
        assert_eq!(
            early_output_flag(&strings(&["lint", "-o", "--quiet"])),
            None
        );
        assert_eq!(early_output_flag(&strings(&["lint", "--", "-ojson"])), None);
        assert_eq!(
            early_output_flag(&strings(&["lint", "--output=json", "-o", "text"])),
            Some("text".to_owned())
        );
        assert_eq!(
            early_output_flag(&strings(&["lint", "-ojson", "--output", "text"])),
            Some("text".to_owned())
        );
        assert_eq!(
            early_output_flag(&strings(&["lint", "-o", "text", "-o=json"])),
            Some("json".to_owned())
        );
        assert_eq!(
            early_output_flag(&strings(&["lint", "-ojson", "-o"])),
            Some("json".to_owned()),
            "a bare flag keeps the earlier value"
        );
        assert_eq!(
            early_output_flag(&strings(&["lint", "--to", "-1", "-o"])),
            None,
            "a value slot swallows a negative number"
        );
        for cluster in [
            &["lint", "-qojson"][..],
            &["lint", "-vo", "json"],
            &["lint", "-qvojson"],
            &["lint", "-qo=json"],
        ] {
            assert_eq!(
                early_output_flag(&strings(cluster)),
                Some("json".to_owned()),
                "{cluster:?}"
            );
        }
        for words in [
            &["lint", "-qo"][..],
            &["lint", "-qo", "--quiet"],
            &["lint", "-q=false"],
            &["lint", "-json"],
            &["lint", "-qv"],
            &["lint", "--", "-qojson"],
        ] {
            assert_eq!(early_output_flag(&strings(words)), None, "{words:?}");
        }
        assert_eq!(
            early_output_flag(&strings(&["lint", "-qojson", "-o", "text"])),
            Some("text".to_owned())
        );
    }

    #[test]
    fn json_intent_follows_flag_and_environment_layers() {
        let empty_env = |_: &str| None;
        assert!(!json_output_intent(&strings(&["lint"]), &empty_env, false));
        assert!(json_output_intent(
            &strings(&["lint", "--output=json"]),
            &empty_env,
            false
        ));
        assert!(!json_output_intent(
            &strings(&["lint", "--output=text"]),
            &empty_env,
            false
        ));
        assert!(!json_output_intent(
            &strings(&["lint", "--output=xml"]),
            &empty_env,
            false
        ));
        let json_env = |name: &str| (name == "DX_OUTPUT").then(|| "json".to_owned());
        assert!(json_output_intent(&strings(&["lint"]), &json_env, false));
        assert!(!json_output_intent(&strings(&["lint"]), &json_env, true));
        assert!(json_output_intent(
            &strings(&["lint", "--output=json"]),
            &json_env,
            true
        ));
        assert!(!json_output_intent(
            &strings(&["lint", "--output=text"]),
            &json_env,
            false
        ));
        assert!(!json_output_intent(
            &strings(&["lint", "--output="]),
            &json_env,
            false
        ));
    }

    #[test]
    fn json_intent_applies_per_command_fallback() {
        let empty_env = |_: &str| None;
        let json_env = |name: &str| (name == "DX_OUTPUT").then(|| "json".to_owned());
        for words in [&["completion"][..], &["deploy"][..]] {
            assert!(
                !json_output_intent(&strings(words), &json_env, false),
                "{words:?}"
            );
        }
        assert!(json_output_intent(&strings(&["lint"]), &json_env, false));
        assert!(json_output_intent(
            &strings(&["completion", "--output=json"]),
            &empty_env,
            false
        ));
        let none: [&str; 0] = [];
        assert!(json_output_intent(&none, &json_env, false));
        assert!(json_output_intent(&strings(&["--quiet"]), &json_env, false));
    }

    #[test]
    fn json_intent_matches_parsed_output_across_layers() {
        let agent_env = dx_adopt::defaults::AGENT_ENVS[0];
        let mut cases = 0;
        for words in [
            &["lint"][..],
            &["lint", "--output=text"][..],
            &["lint", "--output=json"][..],
            &["worktree", "add", "branch"][..],
            &["worktree", "list"][..],
            &["status"][..],
        ] {
            for env_output in [None, Some("json"), Some("text")] {
                for agent in [false, true] {
                    let env = |name: &str| {
                        if name == dx_adopt::defaults::DX_OUTPUT_ENV {
                            env_output.map(str::to_owned)
                        } else if name == agent_env && agent {
                            Some("1".to_owned())
                        } else {
                            None
                        }
                    };
                    let args = strings(words);
                    let Ok(parsed) = parse_with(&args, &env) else {
                        continue;
                    };
                    cases += 1;
                    assert_eq!(
                        json_output_intent(&args, &env, false),
                        parsed.output == OutputMode::Json,
                        "{words:?} env={env_output:?} agent={agent}"
                    );
                }
            }
        }
        assert!(cases > 25, "{cases}");
    }

    #[test]
    fn json_intent_under_agents_follows_supports_json() {
        use clap::ValueEnum;
        for marker in dx_adopt::defaults::AGENT_ENVS {
            let agent = |name: &str| (name == marker).then(|| "1".to_owned());
            for command in Command::value_variants() {
                let name = command.name();
                assert_eq!(
                    crate::args::help::help_command_in(&strings(&[name])),
                    Some(*command),
                    "{name}"
                );
                for words in [
                    &[name, "--bogus"][..],
                    &["--workspace", "/repo", name, "--bogus"][..],
                ] {
                    assert_eq!(
                        json_output_intent(&strings(words), &agent, false),
                        command.supports_json(),
                        "{marker} {words:?}"
                    );
                    assert!(
                        !json_output_intent(&strings(words), &agent, true),
                        "{marker} {words:?} ci"
                    );
                }
            }
            assert!(
                !json_output_intent(&strings(&["deploy", "--bogus"]), &agent, false),
                "{marker} deploy --bogus"
            );
            assert!(
                json_output_intent(&strings(&["lint", "--bogus"]), &agent, false),
                "{marker} lint"
            );
        }
    }

    #[test]
    fn json_intent_reads_short_output_spellings() {
        let intent = |words: &[&str], env: &dyn Fn(&str) -> Option<String>, is_ci: bool| {
            json_output_intent(&strings(words), env, is_ci)
        };
        let empty_env = |_: &str| None;
        assert!(intent(&["lint", "-o", "json"], &empty_env, false));
        assert!(intent(&["lint", "-ojson"], &empty_env, false));
        assert!(intent(&["lint", "-o=json"], &empty_env, false));
        assert!(!intent(&["lint", "-o", "text"], &empty_env, false));
        assert!(!intent(&["lint", "-oxml"], &empty_env, false));
        let json_env = |name: &str| (name == "DX_OUTPUT").then(|| "json".to_owned());
        assert!(!intent(&["lint", "-o", "text"], &json_env, false));
        assert!(!intent(&["lint", "-otext"], &json_env, false));
        assert!(intent(&["lint", "-o", "json"], &json_env, true));
        assert!(intent(&["lint", "-ojson"], &empty_env, true));
        let agent_env = |name: &str| (name == "CLAUDECODE").then(|| "1".to_owned());
        assert!(intent(&["lint"], &agent_env, false));
        assert!(!intent(&["lint", "-o", "text"], &agent_env, false));
        assert!(intent(&["lint", "-qojson", "--bogus"], &empty_env, true));
        assert!(intent(&["lint", "-vo", "json"], &empty_env, false));
        assert!(!intent(&["lint", "-qotext"], &json_env, false));
        assert!(intent(&["lint", "-json"], &json_env, false));
    }

    #[test]
    fn discovery_runs_without_a_module_only_for_init_and_completion() {
        for command in [Command::Init, Command::Completion] {
            assert!(is_discovery_exempt(command), "{command:?} is exempt");
        }
        for command in [
            Command::Lint,
            Command::Build,
            Command::Status,
            Command::Docs,
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
    fn startup_workspace_layers_flag_then_env() {
        assert_eq!(startup_workspace(None, None, false), None);
        assert_eq!(
            startup_workspace(None, Some("/env".to_owned()), false),
            Some("/env".to_owned())
        );
        assert_eq!(
            startup_workspace(None, Some("/env".to_owned()), true),
            None,
            "CI ignores the environment workspace"
        );
        assert_eq!(
            startup_workspace(Some("/flag".to_owned()), Some("/env".to_owned()), false),
            Some("/flag".to_owned())
        );
        assert_eq!(startup_workspace(None, Some(String::new()), false), None);
    }
}
