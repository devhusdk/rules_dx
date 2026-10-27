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
    parse_with(args, &|_| None, &super::FileDefaults::default())
}

/// The startup defaults and a file-directed workspace hop for one more load.
#[derive(Debug)]
pub struct StartupDefaults {
    /// The defaults read from the preliminary workspace.
    pub defaults: super::FileDefaults,
    /// The config-directed workspace when no flag or environment value set one.
    pub file_workspace: Option<String>,
}

/// Reads the workspace the canonical grammar selects, last flag winning.
pub fn early_workspace_flag<S: AsRef<OsStr>>(args: &[S]) -> Option<String> {
    let value = super::tokenizer::selected_workspace(args)?;
    value.into_string().ok().filter(|value| !value.is_empty())
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
        let word = words[index];
        let next = words.get(index + 1).copied();
        if let Some((value, consumed_next)) = output_spelling(word, next) {
            if value.is_some() {
                seen = value;
            }
            if consumed_next {
                index += 1;
            }
        } else if let Some((letter, value, consumed_next)) =
            super::grammar::short_cluster(word, next)
        {
            if letter == 'o' && value.is_some() {
                seen = value;
            }
            if consumed_next {
                index += 1;
            }
        } else if !word.contains('=') && super::grammar::VALUE_OPTIONS.contains(&word) {
            index += 1;
        }
        index += 1;
    }
    seen
}

/// Matches one `--output` spelling and whether it took the next word.
fn output_spelling(word: &str, next: Option<&str>) -> Option<(Option<String>, bool)> {
    if word == "--output" {
        return Some(match next {
            Some(next) if !next.is_empty() && !next.starts_with('-') => {
                (Some(next.to_owned()), true)
            }
            _ => (None, false),
        });
    }
    let value = word.strip_prefix("--output=")?;
    Some((Some(value.to_owned()), false))
}

/// Whether a default output value applies to the command.
fn output_supported(command: Command, name: &str) -> bool {
    match name {
        "json" => command.supports_json(),
        "diff" => command.supports_diff(),
        "path" => command == Command::Worktree,
        _ => true,
    }
}

/// Whether a default output value applies to the command and worktree verb.
///
/// The path value is a default only for `worktree add`; the flag path keeps
/// accepting it on every worktree verb.
fn default_output_applies(command: Command, name: &str, worktree_verb: Option<&str>) -> bool {
    match name {
        "path" => command == Command::Worktree && worktree_verb == Some("add"),
        _ => output_supported(command, name),
    }
}

/// The first positional after the command word, skipping value options.
fn verb_after_command<S: AsRef<OsStr>>(args: &[S]) -> Option<String> {
    let mut positionals = 0;
    let mut index = 0;
    while index < args.len() {
        let word = args[index].as_ref().to_str()?;
        if word == "--" {
            return None;
        }
        if word.starts_with('-') {
            let next = args.get(index + 1).and_then(|next| next.as_ref().to_str());
            if super::grammar::consumes_next(word, next) {
                index += 1;
            }
        } else {
            positionals += 1;
            if positionals == 2 {
                return Some(word.to_owned());
            }
        }
        index += 1;
    }
    None
}

/// Whether a startup failure may report structured JSON events on stdout.
pub fn json_output_intent<S: AsRef<OsStr>>(
    args: &[S],
    env_get: &dyn Fn(&str) -> Option<String>,
    file: &super::FileDefaults,
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
    if let Some(value) = env.as_deref().or(file.output.as_deref()) {
        let verb = verb_after_command(args);
        if command.is_none_or(|command| default_output_applies(command, value, verb.as_deref())) {
            return value == "json";
        }
    }
    !is_ci && dx_adopt::defaults::is_agent(env_get) && command.is_none_or(Command::supports_json)
}

/// Whether the raw command line asks for help or the delivered version.
pub fn is_help_request<S: AsRef<OsStr>>(args: &[S]) -> bool {
    if super::help::help_verb_error_in(args).is_some() {
        return true;
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
        if super::grammar::consumes_next(word, words.get(index + 1).copied()) {
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
        Command::Init | Command::New | Command::Completion | Command::Worktree
    )
}

/// Loads defaults from the selected workspace: the flag, the environment, else
/// the start directory. A config-directed workspace is reported for one more
/// load instead of being followed here, so a redirect cycle cannot loop.
pub fn select_startup_defaults_with_ci(
    start: &std::path::Path,
    base: &std::path::Path,
    flag_workspace: Option<String>,
    env_workspace: Option<String>,
    is_ci: bool,
) -> Result<StartupDefaults, String> {
    let env_workspace =
        env_workspace.filter(|_| !ci_ignores_env(dx_adopt::defaults::DX_WORKSPACE_ENV, is_ci));
    let preliminary = flag_workspace.or(env_workspace);
    let dir = match &preliminary {
        Some(raw) => dx_process::resolve_override_display(std::path::Path::new(raw), base),
        None => start.to_path_buf(),
    };
    match dx_adopt::defaults::load_defaults_with_mode(&dir, is_ci) {
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

/// Why the startup defaults did not load, with the defaults read before the failure.
#[derive(Debug)]
pub struct DefaultsFailure {
    pub partial: Box<super::FileDefaults>,
    pub detail: String,
}

fn raw_output_defaults(
    start: &std::path::Path,
    base: &std::path::Path,
    flag_workspace: Option<&str>,
    env_workspace: Option<&str>,
    is_ci: bool,
) -> super::FileDefaults {
    let env_workspace =
        env_workspace.filter(|_| !ci_ignores_env(dx_adopt::defaults::DX_WORKSPACE_ENV, is_ci));
    let dir = match flag_workspace.or(env_workspace) {
        Some(raw) => dx_process::resolve_override_display(std::path::Path::new(raw), base),
        None => start.to_path_buf(),
    };
    super::FileDefaults {
        output: dx_adopt::defaults::raw_output(&dir, is_ci),
        ..super::FileDefaults::default()
    }
}

/// Loads the defaults a run uses, following one config-directed workspace hop.
pub fn load_startup_defaults(
    start: &std::path::Path,
    base: &std::path::Path,
    flag_workspace: Option<String>,
    env_workspace: Option<String>,
    is_ci: bool,
) -> Result<super::FileDefaults, DefaultsFailure> {
    let lenient = raw_output_defaults(
        start,
        base,
        flag_workspace.as_deref(),
        env_workspace.as_deref(),
        is_ci,
    );
    let selected =
        select_startup_defaults_with_ci(start, base, flag_workspace, env_workspace, is_ci)
            .map_err(|detail| DefaultsFailure {
                partial: Box::new(lenient),
                detail,
            })?;
    let Some(raw_target) = selected.file_workspace else {
        return Ok(selected.defaults);
    };
    let resolved = dx_process::resolve_override_display(std::path::Path::new(&raw_target), start);
    let target = resolved.to_str().map_or(raw_target.clone(), str::to_owned);
    match select_startup_defaults_with_ci(start, start, Some(target.clone()), None, is_ci) {
        Ok(redirected) => Ok(freeze_workspace(&redirected.defaults, &target)),
        Err(detail) => Err(DefaultsFailure {
            partial: Box::new(selected.defaults),
            detail,
        }),
    }
}

/// The workspace a run selects: the flag, the environment, then the defaults.
pub fn startup_workspace(
    start: &std::path::Path,
    flag_workspace: Option<String>,
    env_workspace: Option<String>,
    is_ci: bool,
) -> Option<String> {
    let env_workspace = env_workspace
        .filter(|value| !value.is_empty())
        .filter(|_| !ci_ignores_env(dx_adopt::defaults::DX_WORKSPACE_ENV, is_ci));
    let file_workspace = load_startup_defaults(
        start,
        start,
        flag_workspace.clone(),
        env_workspace.clone(),
        is_ci,
    )
    .ok()
    .and_then(|defaults| defaults.workspace);
    dx_adopt::defaults::resolve_workspace(flag_workspace, env_workspace, file_workspace)
}

/// Parses a watch line as the command it wraps, then marks it as a watch.
fn wrapped_invocation<S: AsRef<OsStr>>(
    args: &[S],
    tokenized: &super::tokenizer::Tokenized,
    env_get: &dyn Fn(&str) -> Option<String>,
    file: &super::FileDefaults,
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
    let mut rest: Vec<&OsStr> = vec![OsStr::new(&wrapped)];
    rest.extend(
        args.iter()
            .enumerate()
            .filter(|(index, _)| *index != watch_at && *index != wrapped_at)
            .map(|(_, word)| word.as_ref()),
    );
    Some(parse_inner(&rest, env_get, file).map(|mut inner| {
        inner.command = Command::Watch;
        inner.targets.insert(0, wrapped);
        inner
    }))
}

pub fn parse_with<S: AsRef<OsStr>>(
    args: &[S],
    env_get: &dyn Fn(&str) -> Option<String>,
    file: &super::FileDefaults,
) -> Result<Invocation, ArgsError> {
    parse_with_ci(args, env_get, file, false)
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
    file: &super::FileDefaults,
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
        return parse_inner(args, &filtered, file);
    }
    parse_inner(args, env_get, file)
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

/// Resolve `--build-slots` across flag, `DX_BUILD_SLOTS`, the config files, then auto.
fn resolve_build_slots(
    flag: Option<String>,
    env_get: &dyn Fn(&str) -> Option<String>,
    file: Option<dx_adopt::defaults::BuildSlots>,
) -> Result<dx_adopt::defaults::BuildSlots, ArgsError> {
    use dx_adopt::defaults::{env_string, BuildSlots, DX_BUILD_SLOTS_ENV};
    let (source, value) = match (flag, env_string(env_get, DX_BUILD_SLOTS_ENV)) {
        (Some(flag), _) => ("--build-slots", flag),
        (None, Some(env)) => (DX_BUILD_SLOTS_ENV, env),
        (None, None) => return Ok(file.unwrap_or_default()),
    };
    BuildSlots::parse(&value).ok_or(ArgsError::BadBuildSlots {
        origin: source,
        value,
    })
}

fn parse_inner<S: AsRef<OsStr>>(
    args: &[S],
    env_get: &dyn Fn(&str) -> Option<String>,
    file: &super::FileDefaults,
) -> Result<Invocation, ArgsError> {
    if let Some(error) = super::help::help_verb_error_in(args) {
        return Err(error);
    }
    dx_adopt::defaults::reject_removed_env(env_get).map_err(bad_default)?;
    let tokenized = tokenize(args)?;
    if tokenized.command == Command::Watch {
        if let Some(wrapped) = wrapped_invocation(args, &tokenized, env_get, file) {
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
        report,
        fail_on,
        build_slots,
        include_edits,
        min_coverage,
        check,
        apply,
        debug,
        release,
        prune_unobserved,
        configured,
        from,
        to,
        here,
        affected,
        serve,
        port,
        host,
        open,
        offline,
        frozen,
        cases,
        create,
        force,
        disk,
        bases,
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
    let agent = invocation_defaults::is_agent(env_get);
    let output_from_flag = output.is_some();
    let fail_on_from_flag = fail_on.is_some();
    let default_output = if agent && command.supports_json() {
        "json"
    } else {
        "text"
    };
    let output_name = invocation_defaults::resolve_string(
        output,
        invocation_defaults::env_string(env_get, invocation_defaults::DX_OUTPUT_ENV),
        file.output.clone(),
        default_output,
    );
    let fail_on_name = invocation_defaults::resolve_string(
        fail_on,
        invocation_defaults::env_string(env_get, invocation_defaults::DX_FAIL_ON_ENV),
        file.fail_on.clone(),
        "warning",
    );
    let build_slots = resolve_build_slots(build_slots, env_get, file.build_slots)?;
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
    let mut fail_on = Threshold::parse(&fail_on_name).map_err(|_| ArgsError::BadFailOn {
        value: fail_on_name.clone(),
    })?;
    if fail_on_name != "warning" && !command.supports_fail_on() {
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
    if !bazel_options.is_empty() && !grammar::advertises_passthrough(command) {
        return Err(unsupported(command, "--"));
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
    if matches!(command, Command::Bump | Command::Add) {
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
    if command == Command::Generate {
        if let Some(scope) = targets.iter().find(|scope| {
            scope.starts_with('@') && !scope.starts_with("@//") && !scope.starts_with("@@//")
        }) {
            return Err(ArgsError::InvalidScope {
                scope: scope.clone(),
            });
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
    let mut output_name = if output_from_flag
        || default_output_applies(command, &output_name, targets.first().map(String::as_str))
    {
        output_name
    } else {
        default_output.to_owned()
    };
    let mut path_only = false;
    if command == Command::Worktree {
        validate_worktree(&targets)?;
        if output_name == "path" {
            if output_from_flag || targets.first().is_some_and(|verb| verb == "add") {
                path_only = true;
                output_name = "text".to_owned();
            } else {
                output_name = default_output.to_owned();
            }
        }
    }
    let output = OutputMode::parse(&output_name, quiet).map_err(|_| ArgsError::BadOutput {
        value: output_name.clone(),
    })?;
    if command.is_adoption() {
        match command {
            Command::Status => {
                if !targets.is_empty() {
                    return Err(extra_positional(command, &targets[0]));
                }
            }
            Command::Version | Command::Rollback => {
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
            Command::New => {
                if targets.len() > 2 {
                    return Err(extra_positional(command, &targets[2]));
                }
                if targets.is_empty() {
                    return Err(missing_positional(command));
                }
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
    Ok(Invocation {
        command,
        check,
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
        build_slots,
        baseline: file.baseline.clone(),
        include_edits,
        min_coverage,
        targets,
        bazel_options,
        prune_unobserved,
        configured,
        from,
        to,
        here,
        affected,
        test_targets: None,
        serve,
        port,
        host,
        open,
        offline,
        frozen,
        cases,
        create,
        force,
        disk,
        bases,
        path_only,
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
mod startup_tests {
    use super::*;
    use dx_testing::cli::strings;

    fn scratch_with_config(name: &str, config: Option<&str>) -> tempfile::TempDir {
        let scratch = dx_test_scratch::scratch(name);
        std::fs::create_dir_all(scratch.path().join(".dx")).expect("dx dir");
        if let Some(text) = config {
            std::fs::write(scratch.path().join("dx.toml"), text).expect("config");
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
            None
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
    fn early_workspace_flag_follows_the_last_spelling() {
        assert_eq!(
            early_workspace_flag(&strings(&["lint", "--workspace", "/a", "--workspace=/b"])),
            Some("/b".to_owned())
        );
        assert_eq!(
            early_workspace_flag(&strings(&["lint", "--workspace=/a", "--workspace", "/b"])),
            Some("/b".to_owned())
        );
        assert_eq!(
            early_workspace_flag(&strings(&[
                "lint",
                "--workspace",
                "/a",
                "--output",
                "--workspace"
            ])),
            None
        );
        assert_eq!(
            early_workspace_flag(&strings(&["lint", "--output", "--workspace", "//..."])),
            None
        );
    }

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
            early_output_flag(&strings(&["lint", "--from", "--output"])),
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
            early_output_flag(&strings(&["lint", "--from", "-ojson"])),
            None,
            "a value slot swallows a short spelling"
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
            &["lint", "--from", "-qojson"],
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
    fn json_intent_ignores_path_default_for_non_add_worktree_verbs() {
        let file = crate::args::FileDefaults::default();
        let env = |name: &str| match name {
            "DX_OUTPUT" => Some("path".to_owned()),
            "CLAUDECODE" => Some("1".to_owned()),
            _ => None,
        };
        assert!(json_output_intent(
            &strings(&["worktree", "list"]),
            &env,
            &file,
            false
        ));
        assert!(!json_output_intent(
            &strings(&["worktree", "add", "x"]),
            &env,
            &file,
            false
        ));
    }

    #[test]
    fn json_intent_follows_flag_environment_and_file_layers() {
        let file = crate::args::FileDefaults::default();
        let empty_env = |_: &str| None;
        assert!(!json_output_intent(
            &strings(&["lint"]),
            &empty_env,
            &file,
            false
        ));
        assert!(json_output_intent(
            &strings(&["lint", "--output=json"]),
            &empty_env,
            &file,
            false
        ));
        assert!(!json_output_intent(
            &strings(&["lint", "--output=text"]),
            &empty_env,
            &file,
            false
        ));
        assert!(!json_output_intent(
            &strings(&["lint", "--output=xml"]),
            &empty_env,
            &file,
            false
        ));
        let json_env = |name: &str| (name == "DX_OUTPUT").then(|| "json".to_owned());
        assert!(json_output_intent(
            &strings(&["lint"]),
            &json_env,
            &file,
            false
        ));
        assert!(!json_output_intent(
            &strings(&["lint"]),
            &json_env,
            &file,
            true
        ));
        assert!(json_output_intent(
            &strings(&["lint", "--output=json"]),
            &json_env,
            &file,
            true
        ));
        assert!(!json_output_intent(
            &strings(&["lint", "--output=text"]),
            &json_env,
            &file,
            false
        ));
        assert!(!json_output_intent(
            &strings(&["lint", "--output="]),
            &json_env,
            &file,
            false
        ));
        let mut json_file = crate::args::FileDefaults::default();
        json_file.output = Some("json".to_owned());
        assert!(json_output_intent(
            &strings(&["lint"]),
            &empty_env,
            &json_file,
            false
        ));
        assert!(json_output_intent(
            &strings(&["lint"]),
            &json_env,
            &json_file,
            true
        ));
    }

    #[test]
    fn json_intent_applies_per_command_fallback() {
        let empty_env = |_: &str| None;
        let json_env = |name: &str| (name == "DX_OUTPUT").then(|| "json".to_owned());
        let empty_file = crate::args::FileDefaults::default();
        let mut json_file = crate::args::FileDefaults::default();
        json_file.output = Some("json".to_owned());
        for (label, env, file) in [
            (
                "file",
                &empty_env as &dyn Fn(&str) -> Option<String>,
                &json_file,
            ),
            ("env", &json_env, &empty_file),
        ] {
            for words in [
                &["completion"][..],
                &["init", "a", "b"][..],
                &["new"][..],
                &["deploy"][..],
            ] {
                assert!(
                    !json_output_intent(&strings(words), env, file, false),
                    "{words:?} {label}"
                );
            }
            assert!(
                json_output_intent(&strings(&["lint"]), env, file, false),
                "lint {label}"
            );
        }
        let agent = |name: &str| (name == "CLAUDECODE").then(|| "1".to_owned());
        let mut path_file = crate::args::FileDefaults::default();
        path_file.output = Some("path".to_owned());
        assert!(json_output_intent(
            &strings(&["lint"]),
            &agent,
            &path_file,
            false
        ));
        assert!(!json_output_intent(
            &strings(&["completion"]),
            &agent,
            &path_file,
            false
        ));
        assert!(json_output_intent(
            &strings(&["completion", "--output=json"]),
            &empty_env,
            &empty_file,
            false
        ));
        let none: [&str; 0] = [];
        assert!(json_output_intent(&none, &empty_env, &json_file, false));
        assert!(json_output_intent(
            &strings(&["--quiet"]),
            &empty_env,
            &json_file,
            false
        ));
    }

    #[test]
    fn json_intent_under_agents_follows_supports_json() {
        use clap::ValueEnum;
        let file = crate::args::FileDefaults::default();
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
                        json_output_intent(&strings(words), &agent, &file, false),
                        command.supports_json(),
                        "{marker} {words:?}"
                    );
                    assert!(
                        !json_output_intent(&strings(words), &agent, &file, true),
                        "{marker} {words:?} ci"
                    );
                }
            }
            for words in [&["deploy", "--bogus"][..], &["init"][..]] {
                assert!(
                    !json_output_intent(&strings(words), &agent, &file, false),
                    "{marker} {words:?}"
                );
            }
            assert!(
                json_output_intent(&strings(&["lint", "--bogus"]), &agent, &file, false),
                "{marker} lint"
            );
        }
    }

    #[test]
    fn json_intent_reads_short_output_spellings() {
        let file = crate::args::FileDefaults::default();
        let intent =
            |words: &[&str],
             env: &dyn Fn(&str) -> Option<String>,
             file: &crate::args::FileDefaults,
             is_ci: bool| { json_output_intent(&strings(words), env, file, is_ci) };
        let empty_env = |_: &str| None;
        assert!(intent(&["lint", "-o", "json"], &empty_env, &file, false));
        assert!(intent(&["lint", "-ojson"], &empty_env, &file, false));
        assert!(intent(&["lint", "-o=json"], &empty_env, &file, false));
        assert!(!intent(&["lint", "-o", "text"], &empty_env, &file, false));
        assert!(!intent(&["lint", "-oxml"], &empty_env, &file, false));
        let json_env = |name: &str| (name == "DX_OUTPUT").then(|| "json".to_owned());
        assert!(!intent(&["lint", "-o", "text"], &json_env, &file, false));
        assert!(!intent(&["lint", "-otext"], &json_env, &file, false));
        assert!(intent(&["lint", "-o", "json"], &json_env, &file, true));
        let mut json_file = crate::args::FileDefaults::default();
        json_file.output = Some("json".to_owned());
        assert!(!intent(
            &["lint", "-o", "text"],
            &empty_env,
            &json_file,
            false
        ));
        assert!(intent(&["lint", "-ojson"], &empty_env, &json_file, true));
        let agent_env = |name: &str| (name == "CLAUDECODE").then(|| "1".to_owned());
        assert!(intent(&["lint"], &agent_env, &file, false));
        assert!(!intent(&["lint", "-o", "text"], &agent_env, &file, false));
        assert!(intent(
            &["lint", "-qojson", "--bogus"],
            &empty_env,
            &file,
            true
        ));
        assert!(intent(&["lint", "-vo", "json"], &empty_env, &file, false));
        assert!(!intent(&["lint", "-qotext"], &json_env, &file, false));
        assert!(intent(&["lint", "-json"], &json_env, &file, false));
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
            vec!["lint", "-qo", "json", "-h"],
            vec!["version", "--workspace=/repo", "--help"],
        ] {
            assert!(is_help_request(&strings(&words)), "{words:?} asks for help");
        }
        for words in [
            vec!["lint"],
            vec!["lint", "--check"],
            vec!["new", "rust", "demo"],
            vec!["version"],
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
        for command in [Command::Init, Command::New, Command::Completion] {
            assert!(is_discovery_exempt(command), "{command:?} is exempt");
        }
        for command in [
            Command::Lint,
            Command::Build,
            Command::Status,
            Command::Version,
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
    fn startup_defaults_come_from_the_selected_workspace() {
        let a = scratch_with_config("startup-select-a-", Some("[dx]\noutput = \"json\"\n"));
        let b = scratch_with_config("startup-select-b-", Some("[dx]\noutput = \"text\"\n"));
        let a_root = a.path().to_path_buf();
        let selected = select_startup_defaults_with_ci(&a_root, &a_root, None, None, false)
            .expect("loads the start directory");
        assert_eq!(selected.defaults.output, Some("json".to_owned()));
        assert_eq!(selected.file_workspace, None);
        let selected = select_startup_defaults_with_ci(
            &a_root,
            &a_root,
            Some(b.path().to_string_lossy().into_owned()),
            None,
            false,
        )
        .expect("loads the flag workspace");
        assert_eq!(
            selected.defaults.output,
            Some("text".to_owned()),
            "the flag workspace supplies the defaults"
        );
        assert_eq!(selected.file_workspace, None);
        let selected = select_startup_defaults_with_ci(
            &a_root,
            &a_root,
            None,
            Some(b.path().to_string_lossy().into_owned()),
            false,
        )
        .expect("loads the environment workspace");
        assert_eq!(selected.defaults.output, Some("text".to_owned()));
        let selected = select_startup_defaults_with_ci(
            &a_root,
            &a_root,
            None,
            Some(b.path().to_string_lossy().into_owned()),
            true,
        )
        .expect("ci ignores the environment workspace");
        assert_eq!(selected.defaults.output, Some("json".to_owned()));
        let missing = a_root.join("no-such-dir");
        let selected = select_startup_defaults_with_ci(&missing, &missing, None, None, false)
            .expect("walks up");
        assert_eq!(selected.defaults.output, Some("json".to_owned()));
    }

    #[test]
    fn startup_reports_a_file_directed_workspace_for_one_more_load() {
        let b = scratch_with_config(
            "startup-redirect-b-",
            Some("[dx]\noutput = \"text\"\nworkspace = \"/elsewhere\"\n"),
        );
        let config = format!(
            "[dx]\noutput = \"json\"\nworkspace = '{}'\n",
            b.path().display()
        );
        let a = scratch_with_config("startup-redirect-a-", Some(&config));
        let selected = select_startup_defaults_with_ci(a.path(), a.path(), None, None, false)
            .expect("loads the start directory");
        assert_eq!(selected.defaults.output, Some("json".to_owned()));
        let target = selected
            .file_workspace
            .expect("the file directs a workspace");
        assert_eq!(target, b.path().to_string_lossy());
        let reloaded =
            select_startup_defaults_with_ci(b.path(), b.path(), Some(target.clone()), None, false)
                .expect("follows once");
        assert_eq!(
            reloaded.file_workspace, None,
            "the second load follows nothing"
        );
        let frozen = freeze_workspace(&reloaded.defaults, &target);
        assert_eq!(frozen.workspace, Some(target));
        assert_eq!(frozen.output, Some("text".to_owned()));
    }

    #[test]
    fn startup_loading_follows_one_redirect_and_keeps_partial_defaults() {
        let b = scratch_with_config(
            "startup-load-b-",
            Some("[dx]\noutput = \"text\"\nworkspace = \"/elsewhere\"\n"),
        );
        let config = format!(
            "[dx]\noutput = \"json\"\nworkspace = '{}'\n",
            b.path().display()
        );
        let a = scratch_with_config("startup-load-a-", Some(&config));
        let target = b.path().to_string_lossy().into_owned();
        let loaded =
            load_startup_defaults(a.path(), a.path(), None, None, false).expect("follows once");
        assert_eq!(loaded.workspace, Some(target.clone()));
        assert_eq!(loaded.output, Some("text".to_owned()));
        let broken = scratch_with_config("startup-load-broken-", Some("not toml = ["));
        let config = format!(
            "[dx]\noutput = \"json\"\nworkspace = '{}'\n",
            broken.path().display()
        );
        let c = scratch_with_config("startup-load-c-", Some(&config));
        let failure = load_startup_defaults(c.path(), c.path(), None, None, false)
            .expect_err("broken target fails");
        assert_eq!(failure.partial.output, Some("json".to_owned()));
        assert!(failure.detail.contains("dx.toml"), "{}", failure.detail);
        let failure = load_startup_defaults(broken.path(), broken.path(), None, None, false)
            .expect_err("broken start fails");
        assert_eq!(*failure.partial, crate::args::FileDefaults::default());
        let invalid = scratch_with_config(
            "startup-load-invalid-key-",
            Some("[dx]\noutput = \"json\"\nlog-level = \"debug\"\n"),
        );
        let failure = load_startup_defaults(invalid.path(), invalid.path(), None, None, false)
            .expect_err("invalid key fails");
        assert_eq!(failure.partial.output, Some("json".to_owned()));
        assert!(failure.detail.contains("log-level"), "{}", failure.detail);
    }

    #[test]
    fn startup_loading_resolves_relative_flag_from_base_and_file_key_from_start() {
        let root = dx_test_scratch::scratch("startup-base-");
        let write = |dir: &std::path::Path, text: &str| {
            std::fs::create_dir_all(dir.join(".dx")).expect("dx dir");
            std::fs::write(dir.join("dx.toml"), text).expect("config");
        };
        let start = root.path().join("start");
        let base = root.path().join("base");
        write(&start, "[dx]\noutput = \"json\"\nworkspace = \"sub\"\n");
        write(&start.join("sub"), "[dx]\noutput = \"text\"\n");
        write(&start.join("rel"), "[dx]\noutput = \"json\"\n");
        write(&base, "[dx]\noutput = \"json\"\n");
        write(&base.join("rel"), "[dx]\noutput = \"text\"\n");
        write(&base.join("sub"), "[dx]\noutput = \"json\"\n");
        let loaded = load_startup_defaults(&start, &base, Some("rel".to_owned()), None, false)
            .expect("loads the flag workspace");
        assert_eq!(loaded.output, Some("text".to_owned()));
        let loaded = load_startup_defaults(&start, &base, None, Some("rel".to_owned()), false)
            .expect("loads the environment workspace");
        assert_eq!(loaded.output, Some("text".to_owned()));
        let loaded =
            load_startup_defaults(&start, &base, None, None, false).expect("follows the key");
        assert_eq!(loaded.output, Some("text".to_owned()));
        assert_eq!(
            loaded.workspace,
            Some(start.join("sub").to_string_lossy().into_owned())
        );
    }

    #[test]
    fn startup_workspace_layers_flag_env_and_defaults() {
        let b = scratch_with_config("startup-ws-b-", None);
        let config = format!("[dx]\nworkspace = '{}'\n", b.path().display());
        let a = scratch_with_config("startup-ws-a-", Some(&config));
        let target = b.path().to_string_lossy().into_owned();
        assert_eq!(
            startup_workspace(a.path(), None, None, false),
            Some(target.clone())
        );
        assert_eq!(
            startup_workspace(a.path(), None, Some("/env".to_owned()), false),
            Some("/env".to_owned())
        );
        assert_eq!(
            startup_workspace(a.path(), None, Some("/env".to_owned()), true),
            Some(target.clone()),
            "CI ignores the environment workspace"
        );
        assert_eq!(
            startup_workspace(
                a.path(),
                Some("/flag".to_owned()),
                Some("/env".to_owned()),
                false
            ),
            Some("/flag".to_owned())
        );
        assert_eq!(
            startup_workspace(a.path(), None, Some(String::new()), false),
            Some(target)
        );
        let broken = scratch_with_config("startup-ws-broken-", Some("not toml = ["));
        assert_eq!(startup_workspace(broken.path(), None, None, false), None);
        assert_eq!(
            startup_workspace(broken.path(), None, Some("/env".to_owned()), false),
            Some("/env".to_owned())
        );
    }

    #[test]
    fn startup_malformed_config_fails_with_the_file_and_key() {
        let scratch = scratch_with_config("startup-malformed-", Some("not toml = ["));
        let error =
            select_startup_defaults_with_ci(scratch.path(), scratch.path(), None, None, false)
                .expect_err("malformed fails");
        assert!(error.contains("dx.toml"), "{error}");
    }

    #[test]
    fn startup_ci_skips_the_local_file_but_keeps_committed() {
        let scratch = dx_test_scratch::scratch("startup-ci-local-");
        std::fs::create_dir_all(scratch.path().join(".dx")).expect("dx dir");
        std::fs::write(scratch.path().join("dx.toml"), "[dx]\noutput = \"json\"\n")
            .expect("committed");
        std::fs::write(
            scratch.path().join("dx.local.toml"),
            "[dx]\noutput = \"text\"\n",
        )
        .expect("local");
        let selected =
            select_startup_defaults_with_ci(scratch.path(), scratch.path(), None, None, false)
                .expect("local wins");
        assert_eq!(selected.defaults.output, Some("text".to_owned()));
        let selected =
            select_startup_defaults_with_ci(scratch.path(), scratch.path(), None, None, true)
                .expect("ci skips local");
        assert_eq!(selected.defaults.output, Some("json".to_owned()));
    }
}
