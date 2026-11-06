//! The write, refresh and report tail shared by the dependency verbs (`dx update`).

use super::common::*;
use dx_output::{
    FinishedCounts, NoticeEvent, OutputMode, command_finished, error_event, notice_event,
};
use dx_plan::args::Invocation;

/// The words and codes one dependency verb reports with.
pub(crate) struct EditVerb {
    /// The command name, e.g. in `related_command`.
    pub(crate) command: &'static str,
    /// The noun for one edit, as in "(no widen performed)".
    pub(crate) noun: &'static str,
    /// The notice code for a written manifest edit.
    pub(crate) notice_code: &'static str,
    /// The error code for a failed edit.
    pub(crate) failure_code: &'static str,
    /// What a no-op refresh of a pinned module lock reports.
    pub(crate) noop_note: &'static str,
}

/// One planned manifest edit and how to report it.
pub(crate) struct DepEdit<'a> {
    pub(crate) verb: &'a EditVerb,
    pub(crate) target: &'a dx_update::edit::DepTarget,
    pub(crate) selector: &'a str,
    /// The edit itself, e.g. "widened cargo:anyhow to 1.2.3 in Cargo.toml".
    pub(crate) edited: String,
    /// Where the edit stays when the refresh fails, e.g. "widen kept in Cargo.toml".
    pub(crate) kept: String,
}

/// A dependency set a dependency verb edits.
pub(crate) struct ConfiguredTarget {
    pub(crate) name: String,
    /// Every manifest of the set; a bazel, uv, npm, maven or nuget set has exactly one.
    pub(crate) manifests: Vec<String>,
    pub(crate) kind: dx_update::edit::ConfiguredKind,
    /// The hub repository a maven set names.
    pub(crate) repository: Option<String>,
}

/// The error for a selector whose set is neither declared nor built in.
pub(crate) fn unknown_set_error(workspace: &std::path::Path, head: &str) -> String {
    let mut names: Vec<String> = match dx_adopt::dependency_sets::load(workspace) {
        Ok(Some(registry)) => registry.names().into_iter().map(String::from).collect(),
        _ => Vec::new(),
    };
    names.sort();
    names.extend(
        dx_update::edit::UpdateSet::builtin_names()
            .into_iter()
            .map(String::from),
    );
    format!(
        "unknown dependency set \"{head}\"; expected {}",
        names.join(", ")
    )
}

/// The single manifest of a dependency set the selector names, if any.
///
/// Bazel sets always qualify; uv, npm, cargo, maven and nuget sets only when `foreign`
/// is set, and then their kind comes back.
pub(crate) fn configured_set(
    verb: &EditVerb,
    invocation: &Invocation,
    out: &mut dyn std::io::Write,
    err: &mut dyn std::io::Write,
    workspace: &std::path::Path,
    foreign: bool,
) -> Result<Option<ConfiguredTarget>, i32> {
    let registry = match dx_adopt::dependency_sets::load(workspace) {
        Ok(Some(registry)) => registry,
        Ok(None) => return Ok(None),
        Err(error) => return Err(pre_exec(invocation.output, out, err, &error.to_string())),
    };
    let selector = &invocation.targets[0];
    let head = selector
        .split_once(':')
        .map(|(head, _)| head)
        .unwrap_or(selector);
    let Some(set) = registry.find(head) else {
        return Ok(None);
    };
    let kind = match set.ecosystem {
        dx_adopt::dependency_sets::Ecosystem::Bazel => Some(dx_update::edit::ConfiguredKind::Bazel),
        dx_adopt::dependency_sets::Ecosystem::Uv if foreign => {
            Some(dx_update::edit::ConfiguredKind::Uv)
        }
        dx_adopt::dependency_sets::Ecosystem::Npm if foreign => {
            Some(dx_update::edit::ConfiguredKind::Npm)
        }
        dx_adopt::dependency_sets::Ecosystem::Cargo if foreign => {
            Some(dx_update::edit::ConfiguredKind::Cargo)
        }
        dx_adopt::dependency_sets::Ecosystem::Maven if foreign => {
            Some(dx_update::edit::ConfiguredKind::Maven)
        }
        dx_adopt::dependency_sets::Ecosystem::Nuget if foreign => {
            Some(dx_update::edit::ConfiguredKind::Nuget)
        }
        dx_adopt::dependency_sets::Ecosystem::Go if foreign => {
            Some(dx_update::edit::ConfiguredKind::Go)
        }
        dx_adopt::dependency_sets::Ecosystem::Ruby if foreign => {
            Some(dx_update::edit::ConfiguredKind::Ruby)
        }
        dx_adopt::dependency_sets::Ecosystem::Cpan if foreign => {
            Some(dx_update::edit::ConfiguredKind::Cpan)
        }
        dx_adopt::dependency_sets::Ecosystem::Pub if foreign => {
            Some(dx_update::edit::ConfiguredKind::Pub)
        }
        dx_adopt::dependency_sets::Ecosystem::Julia if foreign => {
            Some(dx_update::edit::ConfiguredKind::Julia)
        }
        dx_adopt::dependency_sets::Ecosystem::Composer if foreign => {
            Some(dx_update::edit::ConfiguredKind::Composer)
        }
        dx_adopt::dependency_sets::Ecosystem::Cabal if foreign => {
            Some(dx_update::edit::ConfiguredKind::Cabal)
        }
        dx_adopt::dependency_sets::Ecosystem::SwiftPm if foreign => {
            Some(dx_update::edit::ConfiguredKind::SwiftPm)
        }
        dx_adopt::dependency_sets::Ecosystem::Opam if foreign => {
            Some(dx_update::edit::ConfiguredKind::Opam)
        }
        dx_adopt::dependency_sets::Ecosystem::Hex if foreign => {
            Some(dx_update::edit::ConfiguredKind::Hex)
        }
        _ => None,
    };
    match (set.ecosystem, kind) {
        (_, Some(kind)) => {
            let cabal_manifests;
            let manifests = if kind == dx_update::edit::ConfiguredKind::Cabal {
                cabal_manifests = super::update_outdated::cabal_files(workspace, &set.manifests);
                &cabal_manifests
            } else {
                &set.manifests
            };
            let arity_ok = if kind == dx_update::edit::ConfiguredKind::Cargo {
                !manifests.is_empty()
            } else {
                manifests.len() == 1
            };
            if !arity_ok {
                return Err(operational(
                    invocation,
                    out,
                    err,
                    verb.failure_code,
                    &format!(
                        "{} needs {} in set {}",
                        verb.command,
                        match kind {
                            dx_update::edit::ConfiguredKind::Bazel => "exactly one MODULE.bazel",
                            dx_update::edit::ConfiguredKind::Npm => "exactly one package.json",
                            dx_update::edit::ConfiguredKind::Uv => "exactly one pyproject.toml",
                            dx_update::edit::ConfiguredKind::Cargo => "at least one Cargo.toml",
                            dx_update::edit::ConfiguredKind::Maven => "exactly one MODULE.bazel",
                            dx_update::edit::ConfiguredKind::Nuget =>
                                "exactly one paket.dependencies",
                            dx_update::edit::ConfiguredKind::Go => "exactly one go.mod",
                            dx_update::edit::ConfiguredKind::Ruby => "exactly one Gemfile",
                            dx_update::edit::ConfiguredKind::Cpan => "exactly one cpanfile",
                            dx_update::edit::ConfiguredKind::Pub => "exactly one pubspec.yaml",
                            dx_update::edit::ConfiguredKind::Hex => "exactly one mix.exs",
                            dx_update::edit::ConfiguredKind::Julia => "exactly one Project.toml",
                            dx_update::edit::ConfiguredKind::Composer =>
                                "exactly one composer.json",
                            dx_update::edit::ConfiguredKind::Cabal => "exactly one .cabal file",
                            dx_update::edit::ConfiguredKind::SwiftPm => "exactly one Package.swift",
                            dx_update::edit::ConfiguredKind::Opam => "exactly one .opam file",
                        },
                        set.name
                    ),
                ));
            }
            Ok(Some(ConfiguredTarget {
                name: set.name.clone(),
                manifests: manifests.clone(),
                kind,
                repository: set.repository.clone(),
            }))
        }
        (
            dx_adopt::dependency_sets::Ecosystem::Bazel
            | dx_adopt::dependency_sets::Ecosystem::Uv
            | dx_adopt::dependency_sets::Ecosystem::Npm
            | dx_adopt::dependency_sets::Ecosystem::Ruby
            | dx_adopt::dependency_sets::Ecosystem::PowerShell
            | dx_adopt::dependency_sets::Ecosystem::Cargo
            | dx_adopt::dependency_sets::Ecosystem::Maven
            | dx_adopt::dependency_sets::Ecosystem::Nuget
            | dx_adopt::dependency_sets::Ecosystem::Go
            | dx_adopt::dependency_sets::Ecosystem::Cpan
            | dx_adopt::dependency_sets::Ecosystem::Pub
            | dx_adopt::dependency_sets::Ecosystem::Conan
            | dx_adopt::dependency_sets::Ecosystem::Julia
            | dx_adopt::dependency_sets::Ecosystem::Hex
            | dx_adopt::dependency_sets::Ecosystem::Composer
            | dx_adopt::dependency_sets::Ecosystem::Opam
            | dx_adopt::dependency_sets::Ecosystem::SwiftPm
            | dx_adopt::dependency_sets::Ecosystem::Cabal,
            _,
        ) => Err(operational(
            invocation,
            out,
            err,
            verb.failure_code,
            &format!(
                "{} is not supported for ecosystem {} set {} (supported operations: {})",
                verb.command,
                set.ecosystem.name(),
                set.name,
                set.ecosystem.supported_operations(),
            ),
        )),
    }
}

/// Fails before any write when `--frozen` is set or `--offline` cannot refresh the set.
pub(crate) fn preflight(
    verb: &EditVerb,
    invocation: &Invocation,
    out: &mut dyn std::io::Write,
    err: &mut dyn std::io::Write,
    workspace: &std::path::Path,
    target: &dx_update::edit::DepTarget,
    selector: &str,
) -> Result<(), i32> {
    if invocation.frozen {
        return Err(operational(
            invocation,
            out,
            err,
            CODE_FROZEN_LOCKED,
            &format!(
                "frozen_locked: cannot {} {selector} while frozen: frozen forbids manifest and lock resolution changes (no manifest changes performed)",
                verb.noun,
            ),
        ));
    }
    if invocation.offline
        && target.needs_update_refresh()
        && let Some((_, Err(dx_update::backend::BackendError::OfflineRequired { .. }))) =
            refresh_plan(workspace, invocation, target, None)
    {
        return Err(operational(
            invocation,
            out,
            err,
            CODE_OFFLINE_REQUIRED,
            &format!(
                "cannot refresh {selector} without network: re-run without --offline once connected (no {} performed)",
                verb.noun,
            ),
        ));
    }
    if target.needs_update_refresh() && target.configured.is_none() {
        return Err(operational(
            invocation,
            out,
            err,
            verb.failure_code,
            &format!(
                "cannot refresh {selector}: no dependency set in MODULE.bazel owns it (no {} performed)",
                verb.noun
            ),
        ));
    }
    if let Some(refusal) = hub_gate_refusal(workspace, target) {
        return Err(operational(
            invocation,
            out,
            err,
            verb.failure_code,
            &format!(
                "cannot refresh {selector}: {refusal} (no {} performed)",
                verb.noun
            ),
        ));
    }
    if target.is_configured_bazel()
        || matches!(&target.configured, Some(set) if matches!(
            set.kind,
            dx_update::edit::ConfiguredKind::Cargo
                | dx_update::edit::ConfiguredKind::Maven
                | dx_update::edit::ConfiguredKind::Go
        ))
    {
        // A foreign lock layout or a vanished set would only fail after the write.
        if let Some((_, Err(dx_update::backend::BackendError::Unsupported { reason, .. }))) =
            refresh_plan(workspace, invocation, target, None)
        {
            return Err(operational(
                invocation,
                out,
                err,
                verb.failure_code,
                &format!(
                    "cannot refresh {selector}: {reason} (no {} performed)",
                    verb.noun,
                ),
            ));
        }
    }
    Ok(())
}

/// Why a configured cargo, maven or go set cannot be refreshed, checked before any write.
///
/// These are the same refusals `dx update` makes for the set: missing files, an
/// unsupported lock layout and host environment that redirects resolution.
fn hub_gate_refusal(
    workspace: &std::path::Path,
    target: &dx_update::edit::DepTarget,
) -> Option<String> {
    let configured = target.configured.as_ref()?;
    if !matches!(
        configured.kind,
        dx_update::edit::ConfiguredKind::Cargo
            | dx_update::edit::ConfiguredKind::Maven
            | dx_update::edit::ConfiguredKind::Go
    ) {
        return None;
    }
    let registry = dx_adopt::dependency_sets::load(workspace).ok()??;
    let set = registry.find(&configured.name)?;
    let set = dx_adopt::dependency_sets::ResolvedSet::of(set);
    if let Err(error) = dx_adopt::dependency_sets::check_files(workspace, &set, false) {
        return Some(error.to_string());
    }
    if let Err(error) = dx_adopt::dependency_sets::check_backend_settings(workspace, &set) {
        return Some(error.to_string());
    }
    dx_update::backend::host_env_refusal(&set, false, &|key| std::env::var_os(key).is_some())
        .map(|error| error.to_string())
}

/// The workspace, updater runner and output streams one edit uses.
pub(crate) struct EditIo<'a> {
    pub(crate) workspace: &'a std::path::Path,
    pub(crate) runner: &'a dyn dx_process::Runner,
    pub(crate) query_runner: &'a dyn dx_plan::resolve::QueryRunner,
    pub(crate) out: &'a mut dyn std::io::Write,
    pub(crate) err: &'a mut dyn std::io::Write,
}

/// Writes every planned manifest, then refreshes the set's locks through `dx update`.
pub(crate) fn write_and_refresh(
    edit: &DepEdit<'_>,
    invocation: &Invocation,
    io: EditIo<'_>,
    planned: &[(String, String)],
    verbose: bool,
) -> i32 {
    let EditIo {
        workspace,
        runner,
        query_runner,
        out,
        err,
    } = io;
    let verb = edit.verb;
    let request = edit.target;
    for (manifest, text) in planned {
        let written = dx_path::workspace::resolve_target_in_workspace(workspace, manifest)
            .and_then(|target| dx_atomic_fs::write_atomic(&target, text.as_bytes()));
        if written.is_err() {
            return operational(
                invocation,
                out,
                err,
                verb.failure_code,
                &format!(
                    "failed to {} {}: cannot write {manifest}",
                    verb.noun, edit.selector
                ),
            );
        }
    }
    let manifest = planned
        .iter()
        .map(|(manifest, _)| manifest.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    let manifest = manifest.as_str();
    if !request.needs_update_refresh() {
        let message = match request.follow_up() {
            Some(follow_up) => format!("{} ({follow_up})", edit.edited),
            None => format!(
                "{} (then run preset flag-diff review plus `bazel build //...`)",
                edit.edited
            ),
        };
        if invocation.output == OutputMode::Json {
            if let Err(exit) =
                emit_built(out, notice_event(&edited_notice(edit, manifest, &message)))
            {
                return exit;
            }
            let finished = command_finished(
                0,
                &FinishedCounts {
                    results_complete: Some(true),
                    ..FinishedCounts::default()
                },
            );
            if let Err(exit) = emit_event(out, &finished) {
                return exit;
            }
            return 0;
        }
        if verbose && let Err(exit) = check_stdout_write(writeln!(out, "{message}")) {
            return exit;
        }
        return 0;
    }
    let Some((update_name, planned_refresh)) =
        refresh_plan(workspace, invocation, request, Some((runner, query_runner)))
    else {
        return operational(
            invocation,
            out,
            err,
            verb.failure_code,
            &format!(
                "failed to refresh {}: no refresh for file-only set",
                edit.selector
            ),
        );
    };
    let update_selector = request.refresh_selector();
    let refresh_command = request.refresh_command();
    let plan = match planned_refresh {
        Ok(plan) => plan,
        Err(error) => match error {
            dx_update::backend::BackendError::Unsupported { reason, .. } => {
                return refresh_failed(
                    edit,
                    invocation,
                    out,
                    err,
                    manifest,
                    &format!("unsupported refresh: {reason}"),
                );
            }
            dx_update::backend::BackendError::OfflineRequired { .. } => {
                return backend_failed(
                    edit,
                    invocation,
                    out,
                    err,
                    manifest,
                    dx_update::backend::BackendError::OfflineRequired {
                        set: update_name.clone(),
                    },
                );
            }
            dx_update::backend::BackendError::FrozenLocked { .. } => {
                return backend_failed(
                    edit,
                    invocation,
                    out,
                    err,
                    manifest,
                    dx_update::backend::BackendError::FrozenLocked {
                        set: update_name.clone(),
                    },
                );
            }
            unavailable @ (dx_update::backend::BackendError::BackendUnavailable { .. }
            | dx_update::backend::BackendError::NonUtf8Path { .. }) => {
                return refresh_failed(
                    edit,
                    invocation,
                    out,
                    err,
                    manifest,
                    &unavailable.to_string(),
                );
            }
        },
    };
    let refreshed = |note: &str| {
        format!(
            "{} and refreshed {update_selector} via `{refresh_command}` automatically ({note})",
            edit.edited
        )
    };
    let kept = &edit.kept;
    let mut finish = |result: Result<(), String>, message: String| match result {
        Ok(()) => match emit_refreshed(
            edit,
            invocation,
            out,
            manifest,
            &message,
            &update_name,
            verbose,
        ) {
            Ok(()) => 0,
            Err(exit) => exit,
        },
        Err(failure) => refresh_failed(edit, invocation, out, err, manifest, &failure),
    };
    match plan {
        RefreshPlan::Backend(dx_update::backend::BackendPlan::Noop) => {
            finish(Ok(()), refreshed(verb.noop_note))
        }
        // Only a go set plans a staged run here; nuget updates never plan a refresh.
        RefreshPlan::Backend(dx_update::backend::BackendPlan::Staged(run)) => {
            let result = refresh_staged(
                &run,
                runner,
                workspace,
                &update_name,
                &update_selector,
                request.target_manifest(),
                kept,
            );
            finish(result, refreshed("go.sum regenerated"))
        }
        RefreshPlan::Backend(dx_update::backend::BackendPlan::Run { argv, env }) => {
            let result = launch_refresh(
                runner,
                &argv,
                &env,
                workspace,
                &update_selector,
                "updater",
                kept,
            );
            finish(result, refreshed("resolver-owned"))
        }
        RefreshPlan::Backend(dx_update::backend::BackendPlan::Sequence(runs)) => {
            let result = runs.iter().try_for_each(|run| {
                launch_refresh(
                    runner,
                    &run.argv,
                    &run.env,
                    workspace,
                    &update_selector,
                    "updater",
                    kept,
                )
            });
            finish(result, refreshed("resolver-owned"))
        }
        RefreshPlan::InDir(run) => {
            let lock_name = if request.is_configured_ruby() {
                "Gemfile.lock"
            } else {
                "MODULE.bazel.lock"
            };
            let lock = match run.dir.as_str() {
                "." => lock_name.to_owned(),
                dir => format!("{dir}/{lock_name}"),
            };
            let result = launch_refresh(
                runner,
                &run.argv,
                &run.env,
                &workspace.join(&run.dir),
                &lock,
                &format!("`{refresh_command}` in {}", run.dir),
                kept,
            );
            let message = format!(
                "{} and refreshed {lock} via `{refresh_command}`",
                edit.edited
            );
            finish(result, message)
        }
    }
}

/// Refreshes a go set's lock through the staged runner `dx update` uses.
///
/// Nothing but the declared lock is written; when go fails or edits go.mod the widen stays
/// and the message names the manual next step.
fn refresh_staged(
    run: &dx_update::backend::StagedRun,
    runner: &dyn dx_process::Runner,
    workspace: &std::path::Path,
    set: &str,
    subject: &str,
    manifest: &str,
    kept: &str,
) -> Result<(), String> {
    let mut launch = |argv: &[String], env: &[(String, String)]| {
        let env_refs: Vec<(&str, &str)> = env
            .iter()
            .map(|(key, value)| (key.as_str(), value.as_str()))
            .collect();
        Ok(super::staged::ToolRun {
            launched: runner.run(argv, workspace, &env_refs),
            wrote: None,
        })
    };
    let dir = manifest.rsplit_once('/').map_or(".", |(dir, _)| dir);
    let next = format!("run `go mod tidy` in {dir}, then `dx update {set}`");
    match super::staged::run_staged_core(run, workspace, subject, "refresh", &mut launch) {
        Ok(differing) => super::staged::write_outputs(workspace, differing)
            .map_err(|error| format!("failed to refresh {subject}: {error} ({kept}); {next}")),
        Err(super::staged::StagedError::Failed { message, .. }) => {
            Err(format!("{message} ({kept}); {next}"))
        }
        Err(super::staged::StagedError::Exit(_)) => {
            Err(format!("failed to refresh {subject} ({kept}); {next}"))
        }
    }
}

/// Runs one refresh process in `cwd`; the error is the full failure message.
fn launch_refresh(
    runner: &dyn dx_process::Runner,
    argv: &[String],
    env: &[(String, String)],
    cwd: &std::path::Path,
    subject: &str,
    tool: &str,
    kept: &str,
) -> Result<(), String> {
    let env_refs: Vec<(&str, &str)> = env
        .iter()
        .map(|(key, value)| (key.as_str(), value.as_str()))
        .collect();
    match runner.run(argv, cwd, &env_refs) {
        Err(error) => Err(format!(
            "failed to refresh {subject}: failed to launch {tool}: {error} ({kept})"
        )),
        Ok(status) => match status.code {
            Some(0) => Ok(()),
            Some(code) => Err(format!(
                "failed to refresh {subject}: {tool} exited {code} ({kept})"
            )),
            None => Err(format!(
                "failed to refresh {subject}: {tool} terminated by signal ({kept})"
            )),
        },
    }
}

fn edited_notice(edit: &DepEdit<'_>, manifest: &str, message: &str) -> NoticeEvent {
    NoticeEvent {
        related_command: Some(edit.verb.command.to_owned()),
        scope: Some(vec![edit.selector.to_owned()]),
        path: Some(manifest.to_owned()),
        ..NoticeEvent::info(edit.verb.notice_code, message)
    }
}

fn emit_refreshed(
    edit: &DepEdit<'_>,
    invocation: &Invocation,
    out: &mut dyn std::io::Write,
    manifest: &str,
    refreshed_message: &str,
    set_name: &str,
    verbose: bool,
) -> Result<(), i32> {
    if invocation.output == OutputMode::Json {
        emit_built(
            out,
            notice_event(&edited_notice(edit, manifest, &edit.edited)),
        )?;
        emit_built(
            out,
            notice_event(&NoticeEvent {
                related_command: Some("update".to_owned()),
                scope: Some(vec![set_name.to_owned()]),
                ..NoticeEvent::info(CODE_UPDATE_SET_SUCCESS, refreshed_message)
            }),
        )?;
        let finished = command_finished(
            0,
            &FinishedCounts {
                results_complete: Some(true),
                ..FinishedCounts::default()
            },
        );
        emit_event(out, &finished)?;
    } else if verbose {
        check_stdout_write(writeln!(out, "{refreshed_message}"))?;
    }
    Ok(())
}

fn refresh_failed(
    edit: &DepEdit<'_>,
    invocation: &Invocation,
    out: &mut dyn std::io::Write,
    err: &mut dyn std::io::Write,
    manifest: &str,
    message: &str,
) -> i32 {
    let code = if message.contains(CODE_OFFLINE_REQUIRED) {
        CODE_OFFLINE_REQUIRED
    } else if message.contains(CODE_FROZEN_LOCKED) {
        CODE_FROZEN_LOCKED
    } else {
        CODE_UPDATE_FAILED
    };
    let _ = writeln!(err, "dx: {code}: {message}");
    if invocation.output == OutputMode::Json {
        if let Err(exit) = emit_built(
            out,
            notice_event(&edited_notice(edit, manifest, &edit.edited)),
        ) {
            return exit;
        }
        if let Err(exit) = emit_built(out, error_event(code, message, None, None, Some("execute")))
        {
            return exit;
        }
        let finished = command_finished(
            1,
            &FinishedCounts {
                results_complete: Some(false),
                ..FinishedCounts::default()
            },
        );
        if let Err(exit) = emit_event(out, &finished) {
            return exit;
        }
    }
    dx_process::operational_code()
}

fn backend_failed(
    edit: &DepEdit<'_>,
    invocation: &Invocation,
    out: &mut dyn std::io::Write,
    err: &mut dyn std::io::Write,
    manifest: &str,
    error: dx_update::backend::BackendError,
) -> i32 {
    refresh_failed(
        edit,
        invocation,
        out,
        err,
        manifest,
        &format!(
            "failed to refresh {}: {error} ({})",
            edit.selector, edit.kept
        ),
    )
}

/// The updater plan that refreshes the locks of `target`'s set, with the set name.
///
/// `tools` resolves the pinned uv for a configured uv set; without it the plan is only
/// a feasibility probe (offline and frozen are refused before any tool is needed).
fn refresh_plan(
    workspace: &std::path::Path,
    invocation: &Invocation,
    target: &dx_update::edit::DepTarget,
    tools: Option<(&dyn dx_process::Runner, &dyn dx_plan::resolve::QueryRunner)>,
) -> Option<(
    String,
    Result<RefreshPlan, dx_update::backend::BackendError>,
)> {
    let configured = target.configured.as_ref()?;
    let name = configured.name.clone();
    let gone = || dx_update::backend::BackendError::Unsupported {
        set: name.clone(),
        reason: "the dependency set is no longer in MODULE.bazel",
    };
    let resolved = match dx_adopt::dependency_sets::load(workspace) {
        Ok(Some(registry)) => {
            dx_adopt::dependency_sets::resolve(&registry, std::slice::from_ref(&name))
        }
        _ => return Some((name.clone(), Err(gone()))),
    };
    let Some(set) = resolved.ok().and_then(|mut all| all.pop()).map(|t| t.set) else {
        return Some((name.clone(), Err(gone())));
    };
    let (request, closure) = match configured.kind {
        dx_update::edit::ConfiguredKind::Bazel => {
            let plan = dx_update::backend::plan_configured_bazel_lock(
                &set,
                invocation.offline,
                invocation.frozen,
                &invocation.bazel_head(),
            );
            return Some((name, plan.map(RefreshPlan::InDir)));
        }
        dx_update::edit::ConfiguredKind::Npm
        | dx_update::edit::ConfiguredKind::Cpan
        | dx_update::edit::ConfiguredKind::Pub
        | dx_update::edit::ConfiguredKind::Composer
        | dx_update::edit::ConfiguredKind::SwiftPm
        | dx_update::edit::ConfiguredKind::Opam
        | dx_update::edit::ConfiguredKind::Hex
        | dx_update::edit::ConfiguredKind::Julia => (
            dx_adopt::dependency_sets::Selection::Packages(vec![target.package.clone()]),
            dx_update::backend::UvClosure::default(),
        ),
        // A hub repin re-resolves the whole lock; it only touches changed requirements.
        // go sums cover the whole module graph, so a go refresh is always a full one.
        dx_update::edit::ConfiguredKind::Cargo
        | dx_update::edit::ConfiguredKind::Maven
        | dx_update::edit::ConfiguredKind::Cabal
        | dx_update::edit::ConfiguredKind::Go => (
            dx_adopt::dependency_sets::Selection::Full,
            dx_update::backend::UvClosure::default(),
        ),
        dx_update::edit::ConfiguredKind::Ruby => {
            let plan = dx_update::backend::plan_ruby_lock(
                &set,
                &target.package,
                invocation.offline,
                invocation.frozen,
                &invocation.bazel_head(),
            );
            return Some((name, plan.map(RefreshPlan::InDir)));
        }
        // Nothing re-resolves paket.lock, so a nuget update never plans a refresh.
        dx_update::edit::ConfiguredKind::Nuget => return None,
        dx_update::edit::ConfiguredKind::Uv => (
            dx_adopt::dependency_sets::Selection::Full,
            match tools {
                Some((runner, query_runner)) if !invocation.offline && !invocation.frozen => {
                    super::update::configured_uv_closure(
                        runner,
                        query_runner,
                        workspace,
                        invocation,
                    )
                }
                _ => dx_update::backend::UvClosure::default(),
            },
        ),
    };
    let plan = dx_update::backend::plan_configured(
        workspace,
        &set,
        &request,
        invocation.offline,
        invocation.frozen,
        &invocation.bazel_head(),
        &closure,
    );
    Some((name, plan.map(RefreshPlan::Backend)))
}

/// What refreshes a dependency's lock: an updater plan, or `bazel mod deps` in a directory.
enum RefreshPlan {
    Backend(dx_update::backend::BackendPlan),
    InDir(dx_update::backend::DirRun),
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io;

    const VERB: EditVerb = EditVerb {
        command: "update",
        noun: "widen",
        notice_code: "update_widened",
        failure_code: CODE_UPDATE_WIDEN_FAILED,
        noop_note: "pinned module lock; no-op success",
    };

    struct BrokenPipe;

    impl io::Write for BrokenPipe {
        fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
            Err(io::Error::new(io::ErrorKind::BrokenPipe, "broken pipe"))
        }

        fn flush(&mut self) -> io::Result<()> {
            Err(io::Error::new(io::ErrorKind::BrokenPipe, "broken pipe"))
        }
    }

    fn json_invocation() -> Invocation {
        dx_plan::args::parse(&dx_testing::cli::strings(&[
            "update",
            "cargo:anyhow",
            "1.2.3",
            "--output=json",
        ]))
        .expect("parse")
    }

    fn target() -> dx_update::edit::DepTarget {
        dx_update::edit::DepTarget {
            set: dx_update::edit::UpdateSet::Cargo,
            package: "anyhow".to_owned(),
            manifest: None,
            configured: None,
        }
    }

    fn edit(target: &dx_update::edit::DepTarget) -> DepEdit<'_> {
        DepEdit {
            verb: &VERB,
            target,
            selector: "cargo:anyhow",
            edited: "widened cargo:anyhow to 1.2.3 in Cargo.toml".to_owned(),
            kept: "widen kept in Cargo.toml".to_owned(),
        }
    }

    #[test]
    fn refresh_failed_with_an_empty_message_ends_the_stream_once() {
        let target = target();
        let mut out = Vec::new();
        let mut err = Vec::new();
        let code = refresh_failed(
            &edit(&target),
            &json_invocation(),
            &mut out,
            &mut err,
            "Cargo.toml",
            "",
        );
        assert_eq!(code, dx_process::operational_code());
        let events: Vec<serde_json::Value> = String::from_utf8(out)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        let kinds: Vec<&str> = events
            .iter()
            .map(|event| event["event"].as_str().unwrap())
            .collect();
        assert_eq!(kinds, ["notice", "error", "command_finished"], "{events:?}");
        assert_eq!(events[1]["code"], CODE_INVALID_RESULT);
    }

    #[test]
    fn refresh_failed_returns_the_stdout_code_when_stdout_breaks() {
        let target = target();
        let mut err = Vec::new();
        let code = refresh_failed(
            &edit(&target),
            &json_invocation(),
            &mut BrokenPipe,
            &mut err,
            "Cargo.toml",
            "failed to refresh cargo:anyhow",
        );
        assert_eq!(code, dx_process::broken_pipe_code());
    }
}
