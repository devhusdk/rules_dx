//! The write, refresh and report tail shared by the dependency verbs (`dx bump`, `dx add`).

use super::common::*;
use dx_output::{
    command_finished, error_event, notice_event, FinishedCounts, NoticeEvent, OutputMode,
};
use dx_plan::args::Invocation;

/// The words and codes one dependency verb reports with.
pub(crate) struct EditVerb {
    /// The command name, e.g. in `related_command`.
    pub(crate) command: &'static str,
    /// The noun for one edit, as in "(no widen performed)".
    pub(crate) noun: &'static str,
    /// What the verb does to a set, as in "cannot widen read-only set".
    pub(crate) set_action: &'static str,
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
    pub(crate) target: &'a dx_bump::DepTarget,
    pub(crate) selector: &'a str,
    /// The edit itself, e.g. "widened cargo:anyhow to 1.2.3 in Cargo.toml".
    pub(crate) edited: String,
    /// Where the edit stays when the refresh fails, e.g. "widen kept in Cargo.toml".
    pub(crate) kept: String,
    /// A suffix every success message carries.
    pub(crate) hint: Option<&'static str>,
}

/// The single manifest of a configured dx.toml bazel set the selector names, if any.
pub(crate) fn configured_bazel_set(
    verb: &EditVerb,
    invocation: &Invocation,
    out: &mut dyn std::io::Write,
    err: &mut dyn std::io::Write,
    workspace: &std::path::Path,
) -> Result<Option<(String, String)>, i32> {
    Ok(
        configured_set(verb, invocation, out, err, workspace, false)?
            .map(|(name, manifest, _)| (name, manifest)),
    )
}

/// The single manifest of a configured dx.toml set the selector names, if any.
///
/// Bazel sets always qualify; uv and npm sets only when `foreign` is set, and then
/// their kind comes back (`None` is a bazel set).
pub(crate) fn configured_set(
    verb: &EditVerb,
    invocation: &Invocation,
    out: &mut dyn std::io::Write,
    err: &mut dyn std::io::Write,
    workspace: &std::path::Path,
    foreign: bool,
) -> Result<Option<(String, String, Option<dx_bump::ConfiguredKind>)>, i32> {
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
        dx_adopt::dependency_sets::Ecosystem::Uv if foreign => Some(dx_bump::ConfiguredKind::Uv),
        dx_adopt::dependency_sets::Ecosystem::Npm if foreign => Some(dx_bump::ConfiguredKind::Npm),
        _ => None,
    };
    match (set.ecosystem, kind) {
        (dx_adopt::dependency_sets::Ecosystem::Bazel, _)
        | (
            dx_adopt::dependency_sets::Ecosystem::Uv | dx_adopt::dependency_sets::Ecosystem::Npm,
            Some(_),
        ) => {
            if !set.writable {
                return Err(operational(
                    invocation,
                    out,
                    err,
                    verb.failure_code,
                    &format!(
                        "{} cannot {} read-only set {} (writable = false)",
                        verb.command, verb.set_action, set.name
                    ),
                ));
            }
            let [manifest] = set.manifests.as_slice() else {
                return Err(operational(
                    invocation,
                    out,
                    err,
                    verb.failure_code,
                    &format!(
                        "{} needs exactly one {} in set {}",
                        verb.command,
                        match kind {
                            None => "MODULE.bazel",
                            Some(dx_bump::ConfiguredKind::Npm) => "package.json",
                            Some(dx_bump::ConfiguredKind::Uv) => "pyproject.toml",
                        },
                        set.name
                    ),
                ));
            };
            Ok(Some((set.name.clone(), manifest.clone(), kind)))
        }
        (
            dx_adopt::dependency_sets::Ecosystem::Uv
            | dx_adopt::dependency_sets::Ecosystem::Npm
            | dx_adopt::dependency_sets::Ecosystem::Ruby
            | dx_adopt::dependency_sets::Ecosystem::PowerShell
            | dx_adopt::dependency_sets::Ecosystem::Cargo,
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
    target: &dx_bump::DepTarget,
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
    if invocation.offline && target.needs_update_refresh() {
        if let Some((_, Err(dx_update::backend::BackendError::OfflineRequired { .. }))) =
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
    }
    Ok(())
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
        let mut message = format!(
            "{} (then run preset flag-diff review plus `bazel build //...`)",
            edit.edited
        );
        if let Some(hint) = edit.hint {
            message.push_str(&format!("; {hint}"));
        }
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
        if verbose {
            if let Err(exit) = check_stdout_write(writeln!(out, "{message}")) {
                return exit;
            }
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
            unavailable @ dx_update::backend::BackendError::BackendUnavailable { .. } => {
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
        let mut message = format!(
            "{} and refreshed {update_selector} via `dx update {update_selector}` automatically ({note})",
            edit.edited
        );
        if let Some(hint) = edit.hint {
            message.push_str(&format!("; {hint}"));
        }
        message
    };
    let kept = &edit.kept;
    match plan {
        dx_update::backend::BackendPlan::Noop => {
            let message = refreshed(verb.noop_note);
            match emit_refreshed(
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
            }
        }
        dx_update::backend::BackendPlan::Run { argv, env: extra } => {
            let env_refs: Vec<(&str, &str)> = extra
                .iter()
                .map(|(key, value)| (key.as_str(), value.as_str()))
                .collect();
            match runner.run(&argv, workspace, &env_refs) {
                Err(error) => refresh_failed(
                    edit,
                    invocation,
                    out,
                    err,
                    manifest,
                    &format!(
                        "failed to refresh {update_selector}: failed to launch updater: {error} ({kept})"
                    ),
                ),
                Ok(status) => match status.code {
                    Some(0) => {
                        let message = refreshed("resolver-owned");
                        match emit_refreshed(
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
                        }
                    }
                    Some(code) => refresh_failed(
                        edit,
                        invocation,
                        out,
                        err,
                        manifest,
                        &format!(
                            "failed to refresh {update_selector}: updater exited {code} ({kept})"
                        ),
                    ),
                    None => refresh_failed(
                        edit,
                        invocation,
                        out,
                        err,
                        manifest,
                        &format!(
                            "failed to refresh {update_selector}: updater terminated by signal ({kept})"
                        ),
                    ),
                },
            }
        }
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
    target: &dx_bump::DepTarget,
    tools: Option<(&dyn dx_process::Runner, &dyn dx_plan::resolve::QueryRunner)>,
) -> Option<(
    String,
    Result<dx_update::backend::BackendPlan, dx_update::backend::BackendError>,
)> {
    let Some(configured) = &target.configured else {
        let (set, request) = refresh_target(target)?;
        let plan = dx_update::backend::plan(
            workspace,
            set,
            &request,
            invocation.offline,
            invocation.frozen,
            &invocation.bazel_head(),
        );
        return Some((set.name().to_owned(), plan));
    };
    let name = configured.name.clone();
    let gone = || dx_update::backend::BackendError::Unsupported {
        set: name.clone(),
        reason: "the dx.toml dependency set is no longer configured",
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
        dx_bump::ConfiguredKind::Npm => (
            dx_adopt::dependency_sets::Selection::Packages(vec![target.package.clone()]),
            dx_update::backend::UvClosure::default(),
        ),
        dx_bump::ConfiguredKind::Uv => (
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
    Some((name, plan))
}

/// The `dx update` set and request that refresh the locks of `target`'s set.
pub(crate) fn refresh_target(
    target: &dx_bump::DepTarget,
) -> Option<(dx_update::sets::SetId, dx_update::selector::SetRequest)> {
    match target.set {
        dx_bump::BumpSet::Cargo => Some((
            dx_update::sets::SetId::Cargo,
            dx_update::selector::SetRequest::Full,
        )),
        dx_bump::BumpSet::Npm => Some((
            dx_update::sets::SetId::Npm,
            dx_update::selector::SetRequest::Packages(vec![target.package.clone()]),
        )),
        dx_bump::BumpSet::Go => Some((
            dx_update::sets::SetId::Go,
            dx_update::selector::SetRequest::Full,
        )),
        dx_bump::BumpSet::Maven => Some((
            dx_update::sets::SetId::Maven,
            dx_update::selector::SetRequest::Full,
        )),
        dx_bump::BumpSet::NuGet => Some((
            dx_update::sets::SetId::NuGet,
            dx_update::selector::SetRequest::Full,
        )),
        dx_bump::BumpSet::Bazel | dx_bump::BumpSet::GithubActions => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io;

    const VERB: EditVerb = EditVerb {
        command: "bump",
        noun: "widen",
        set_action: "widen",
        notice_code: "bump_widened",
        failure_code: CODE_BUMP_FAILED,
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
            "bump",
            "cargo:anyhow",
            "1.2.3",
            "--output=json",
        ]))
        .expect("parse")
    }

    fn target() -> dx_bump::DepTarget {
        dx_bump::DepTarget {
            set: dx_bump::BumpSet::Cargo,
            package: "anyhow".to_owned(),
            manifest: None,
            configured: None,
        }
    }

    fn edit(target: &dx_bump::DepTarget) -> DepEdit<'_> {
        DepEdit {
            verb: &VERB,
            target,
            selector: "cargo:anyhow",
            edited: "widened cargo:anyhow to 1.2.3 in Cargo.toml".to_owned(),
            kept: "widen kept in Cargo.toml".to_owned(),
            hint: None,
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
