use super::common::*;
use crate::args::{Command, Invocation};
use crate::reports::plan_reports;
use dx_output::{
    command_finished, command_started, error_event, notice_event, write_event, FinishedCounts,
    NoticeEvent, OutputMode,
};

pub(crate) fn execute_bump(invocation: &Invocation, env: Env<'_>) -> i32 {
    debug_assert!(
        invocation.command == Command::Bump,
        "bump dispatch guards commands"
    );
    let Env {
        workspace,
        runner,
        out,
        err,
        ..
    } = env;
    match plan_reports(
        workspace,
        invocation.command,
        &invocation.reports,
        &invocation.output,
        invocation.dry_run,
    ) {
        Ok(_) => {}
        Err(error) => return pre_exec(err, &error.to_string()),
    }
    if invocation.targets.len() != 2 {
        return pre_exec(err, "bump needs exactly <selector> <version>");
    }
    if let Some(registry) = match dx_adopt::dependency_sets::load(workspace) {
        Ok(registry) => registry,
        Err(error) => return pre_exec(err, &error.to_string()),
    } {
        let head = invocation.targets[0]
            .split_once(':')
            .map(|(head, _)| head)
            .unwrap_or(&invocation.targets[0]);
        if let Some(set) = registry.find(head) {
            return operational(
                invocation,
                out,
                err,
                CODE_BUMP_FAILED,
                &format!(
                    "bump is not supported for ecosystem {} set {} (supported operations: {})",
                    set.ecosystem.name(),
                    set.name,
                    [
                        set.ecosystem.supported_check(),
                        set.ecosystem.supported_update(),
                        set.ecosystem.supported_audit(),
                    ]
                    .join(", "),
                ),
            );
        }
    }
    let request = match dx_bump::BumpRequest::parse(&invocation.targets[0], &invocation.targets[1])
    {
        Ok(request) => request,
        Err(error) => return pre_exec(err, &error.to_string()),
    };
    let summary = frozen_summary(
        offline_summary(request.summary(), invocation.offline),
        invocation.frozen,
    );
    let verbose = invocation.chatty();
    if invocation.dry_run {
        if invocation.output == OutputMode::Json {
            if let Ok(event) = command_started(invocation.command.name(), true, "default") {
                let _ = write_event(out, &event);
            }
            if let Ok(event) = notice_event(&NoticeEvent {
                level: "info".to_owned(),
                code: "bump_planned".to_owned(),
                message: summary,
                related_command: Some("bump".to_owned()),
                scope: Some(vec![request.selector.clone()]),
                path: Some(request.target_manifest().to_owned()),
                language: None,
                import: None,
            }) {
                let _ = write_event(out, &event);
            }
            let finished = command_finished(0, &FinishedCounts::default());
            let _ = write_event(out, &finished);
        } else if verbose {
            let _ = writeln!(out, "{summary}");
        }
        return 0;
    }
    if !invocation.applies() {
        return execute_bump_check(invocation, out, err, workspace, &request, verbose);
    }
    if invocation.output == OutputMode::Json {
        if let Ok(event) = command_started(invocation.command.name(), false, "default") {
            let _ = write_event(out, &event);
        }
    } else if verbose {
        let _ = writeln!(out, "{summary}");
    }
    if invocation.frozen {
        return operational(
            invocation,
            out,
            err,
            CODE_FROZEN_LOCKED,
            &format!(
                "frozen_locked: cannot widen {} while frozen: frozen forbids manifest and lock resolution changes (no manifest changes performed)",
                request.selector,
            ),
        );
    }
    if invocation.offline && request.needs_update_refresh() {
        if let Some((set, req)) = refresh_target(&request) {
            if let Err(dx_update::backend::BackendError::OfflineRequired { .. }) =
                dx_update::backend::plan(workspace, set, &req, true, invocation.frozen)
            {
                return operational(
                    invocation,
                    out,
                    err,
                    CODE_OFFLINE_REQUIRED,
                    &format!(
                        "cannot refresh {} without network: re-run without --offline once connected (no widen performed)",
                        request.selector,
                    ),
                );
            }
        }
    }
    let planned = match plan_widens(&request, workspace) {
        Ok(planned) => planned,
        Err(message) => {
            return operational(invocation, out, err, CODE_BUMP_FAILED, &message);
        }
    };
    for (manifest, widened) in &planned {
        if dx_atomic_fs::write_atomic(&workspace.join(manifest), widened.as_bytes()).is_err() {
            return operational(
                invocation,
                out,
                err,
                CODE_BUMP_FAILED,
                &format!(
                    "failed to widen {}: cannot write {manifest}",
                    request.selector
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
            "widened {} to {} in {manifest} (then run preset flag-diff review plus `bazel build //...`)",
            request.selector,
            request.version.display()
        );
        if request.version.is_semver() {
            message.push_str(&format!("; {}", dx_bump::generic_major_bump_hint()));
        }
        if invocation.output == OutputMode::Json {
            if let Ok(event) = notice_event(&NoticeEvent {
                level: "info".to_owned(),
                code: "bump_widened".to_owned(),
                message: message.clone(),
                related_command: Some("bump".to_owned()),
                scope: Some(vec![request.selector.clone()]),
                path: Some(manifest.to_owned()),
                language: None,
                import: None,
            }) {
                let _ = write_event(out, &event);
            }
            let finished = command_finished(
                0,
                &FinishedCounts {
                    results_complete: Some(true),
                    ..FinishedCounts::default()
                },
            );
            let _ = write_event(out, &finished);
            return 0;
        }
        if verbose {
            let _ = writeln!(out, "{message}");
        }
        return 0;
    }
    let (update_set, update_request) = match refresh_target(&request) {
        Some(target) => target,
        None => {
            return operational(
                invocation,
                out,
                err,
                CODE_BUMP_FAILED,
                &format!(
                    "failed to refresh {}: no refresh for file-only set",
                    request.selector
                ),
            );
        }
    };
    let update_selector = request.refresh_selector();
    let plan = match dx_update::backend::plan(
        workspace,
        update_set,
        &update_request,
        invocation.offline,
        invocation.frozen,
    ) {
        Ok(plan) => plan,
        Err(error) => match error {
            dx_update::backend::BackendError::Unsupported { reason, .. }
            | dx_update::backend::BackendError::UnsupportedOwned { reason, .. } => {
                return bump_refresh_failed(
                    invocation,
                    out,
                    err,
                    &request,
                    manifest,
                    &format!("unsupported refresh: {reason}"),
                );
            }
            dx_update::backend::BackendError::OfflineRequired { .. }
            | dx_update::backend::BackendError::OfflineRequiredOwned { .. } => {
                return bump_offline_failed(invocation, out, err, &request, manifest);
            }
            dx_update::backend::BackendError::FrozenLocked { .. }
            | dx_update::backend::BackendError::FrozenLockedOwned { .. } => {
                return bump_frozen_failed(invocation, out, err, &request, manifest);
            }
        },
    };
    match plan {
        dx_update::backend::BackendPlan::Noop => {
            let widened_message = format!(
                "widened {} to {} in {manifest}",
                request.selector,
                request.version.display()
            );
            let mut refreshed_message = format!(
                "{widened_message} and refreshed {update_selector} via `dx update {update_selector}` automatically (pinned module lock; no-op success)"
            );
            if request.version.is_semver() {
                refreshed_message.push_str(&format!("; {}", dx_bump::generic_major_bump_hint()));
            }
            emit_bump_refreshed(BumpRefreshedNotice {
                invocation,
                out,
                err,
                request: &request,
                manifest,
                widened_message: &widened_message,
                refreshed_message: &refreshed_message,
                set_name: update_set.name(),
                verbose,
            });
            0
        }
        dx_update::backend::BackendPlan::Run { argv, env: extra } => {
            let env_refs: Vec<(&str, &str)> = extra
                .iter()
                .map(|(key, value)| (key.as_str(), value.as_str()))
                .collect();
            match runner.run(&argv, workspace, &env_refs) {
                Err(error) => bump_refresh_failed(
                    invocation,
                    out,
                    err,
                    &request,
                    manifest,
                    &format!(
                        "failed to refresh {update_selector}: failed to launch updater: {error} (widen kept in {manifest})"
                    ),
                ),
                Ok(status) => match status.code {
                    Some(0) => {
                        let widened_message = format!(
                            "widened {} to {} in {manifest}",
                            request.selector,
                            request.version.display()
                        );
                        let mut refreshed_message = format!(
                            "{widened_message} and refreshed {update_selector} via `dx update {update_selector}` automatically (resolver-owned)"
                        );
                        if request.version.is_semver() {
                            refreshed_message.push_str(&format!(
                                "; {}",
                                dx_bump::generic_major_bump_hint()
                            ));
                        }
                        emit_bump_refreshed(BumpRefreshedNotice {
                            invocation,
                            out,
                            err,
                            request: &request,
                            manifest,
                            widened_message: &widened_message,
                            refreshed_message: &refreshed_message,
                            set_name: update_set.name(),
                            verbose,
                        });
                        0
                    }
                    Some(code) => bump_refresh_failed(
                        invocation,
                        out,
                        err,
                        &request,
                        manifest,
                        &format!(
                            "failed to refresh {update_selector}: updater exited {code} (widen kept in {manifest})"
                        ),
                    ),
                    None => bump_refresh_failed(
                        invocation,
                        out,
                        err,
                        &request,
                        manifest,
                        &format!(
                            "failed to refresh {update_selector}: updater terminated by signal (widen kept in {manifest})"
                        ),
                    ),
                },
            }
        }
    }
}

fn widen_targets(
    request: &dx_bump::BumpRequest,
    workspace: &std::path::Path,
) -> Result<Vec<String>, String> {
    if !request.set.spans_manifest_directory() {
        return Ok(vec![request.target_manifest().to_owned()]);
    }
    let dir = workspace.join(dx_bump::gha::WORKFLOW_DIR);
    let entries = std::fs::read_dir(&dir).map_err(|_| {
        format!(
            "failed to widen {}: cannot list {}",
            request.selector,
            dx_bump::gha::WORKFLOW_DIR
        )
    })?;
    let mut manifests: Vec<String> = entries
        .filter_map(|entry| {
            let name = entry.ok()?.file_name().to_string_lossy().into_owned();
            dx_bump::gha::workflow_file(&name)
                .then(|| format!("{}/{name}", dx_bump::gha::WORKFLOW_DIR))
        })
        .collect();
    manifests.sort();
    Ok(manifests)
}

fn execute_bump_check(
    invocation: &Invocation,
    out: &mut dyn std::io::Write,
    err: &mut dyn std::io::Write,
    workspace: &std::path::Path,
    request: &dx_bump::BumpRequest,
    verbose: bool,
) -> i32 {
    if invocation.output == OutputMode::Json {
        if let Ok(event) = command_started(invocation.command.name(), false, "check") {
            let _ = write_event(out, &event);
        }
    }
    let planned = match plan_widens(request, workspace) {
        Ok(planned) => planned,
        Err(message) => {
            return operational(invocation, out, err, CODE_BUMP_FAILED, &message);
        }
    };
    let mut drifted = Vec::new();
    for (manifest, widened) in &planned {
        match std::fs::read(workspace.join(manifest)) {
            Ok(current) if current == widened.as_bytes() => {}
            _ => drifted.push(manifest.clone()),
        }
    }
    let manifest = planned
        .iter()
        .map(|(manifest, _)| manifest.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    if drifted.is_empty() {
        let message = format!(
            "{} already at {} in {manifest}",
            request.selector,
            request.version.display()
        );
        if invocation.output == OutputMode::Json {
            if let Ok(event) = notice_event(&NoticeEvent {
                level: "info".to_owned(),
                code: "bump_current".to_owned(),
                message,
                related_command: Some("bump".to_owned()),
                scope: Some(vec![request.selector.clone()]),
                path: Some(manifest),
                language: None,
                import: None,
            }) {
                let _ = write_event(out, &event);
            }
            let finished = command_finished(0, &FinishedCounts::default());
            let _ = write_event(out, &finished);
            return 0;
        }
        if verbose {
            let _ = writeln!(
                out,
                "{} already at {} in {manifest}",
                request.selector,
                request.version.display()
            );
        }
        return 0;
    }
    let mut message = format!(
        "bump check failed: {} would widen {} (re-run with --apply to write)",
        request.selector,
        drifted.join(", "),
    );
    if request.needs_update_refresh() {
        let selector = request.refresh_selector();
        message.push_str(&format!(
            "; --apply also refreshes {selector} via `dx update {selector}`"
        ));
    }
    operational(invocation, out, err, CODE_BUMP_FAILED, &message)
}

fn plan_widens(
    request: &dx_bump::BumpRequest,
    workspace: &std::path::Path,
) -> Result<Vec<(String, String)>, String> {
    let mut planned = Vec::new();
    for manifest in widen_targets(request, workspace)? {
        let original = std::fs::read(workspace.join(&manifest)).map_err(|_| {
            format!(
                "failed to widen {}: cannot read {manifest}",
                request.selector
            )
        })?;
        let text = String::from_utf8(original).map_err(|_| {
            format!(
                "failed to widen {}: {manifest} is not valid UTF-8",
                request.selector
            )
        })?;
        match request.plan_edit(&text) {
            Ok(widened) => planned.push((manifest, widened)),
            Err(dx_bump::BumpError::NotFound { .. }) if request.set.spans_manifest_directory() => {}
            Err(error) => {
                return Err(format!("failed to widen {}: {error}", request.selector));
            }
        }
    }
    if planned.is_empty() {
        return Err(format!(
            "failed to widen {}: {}",
            request.selector,
            dx_bump::BumpError::NotFound {
                manifest: request.target_manifest().to_owned(),
                package: request.package.clone(),
            }
        ));
    }
    Ok(planned)
}

struct BumpRefreshedNotice<'a> {
    invocation: &'a Invocation,
    out: &'a mut dyn std::io::Write,
    err: &'a mut dyn std::io::Write,
    request: &'a dx_bump::BumpRequest,
    manifest: &'a str,
    widened_message: &'a str,
    refreshed_message: &'a str,
    set_name: &'a str,
    verbose: bool,
}

fn emit_bump_refreshed(notice: BumpRefreshedNotice<'_>) {
    let BumpRefreshedNotice {
        invocation,
        out,
        err,
        request,
        manifest,
        widened_message,
        refreshed_message,
        set_name,
        verbose,
    } = notice;
    let _ = err;
    if invocation.output == OutputMode::Json {
        if let Ok(event) = notice_event(&NoticeEvent {
            level: "info".to_owned(),
            code: "bump_widened".to_owned(),
            message: widened_message.to_owned(),
            related_command: Some("bump".to_owned()),
            scope: Some(vec![request.selector.clone()]),
            path: Some(manifest.to_owned()),
            language: None,
            import: None,
        }) {
            let _ = write_event(out, &event);
        }
        if let Ok(event) = notice_event(&NoticeEvent {
            level: "info".to_owned(),
            code: "update_set_success".to_owned(),
            message: refreshed_message.to_owned(),
            related_command: Some("update".to_owned()),
            scope: Some(vec![set_name.to_owned()]),
            path: None,
            language: None,
            import: None,
        }) {
            let _ = write_event(out, &event);
        }
        let finished = command_finished(
            0,
            &FinishedCounts {
                results_complete: Some(true),
                ..FinishedCounts::default()
            },
        );
        let _ = write_event(out, &finished);
    } else if verbose {
        let _ = writeln!(out, "{refreshed_message}");
    }
}

fn bump_refresh_failed(
    invocation: &Invocation,
    out: &mut dyn std::io::Write,
    err: &mut dyn std::io::Write,
    request: &dx_bump::BumpRequest,
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
        let widened_message = format!(
            "widened {} to {} in {manifest}",
            request.selector,
            request.version.display()
        );
        if let Ok(event) = notice_event(&NoticeEvent {
            level: "info".to_owned(),
            code: "bump_widened".to_owned(),
            message: widened_message,
            related_command: Some("bump".to_owned()),
            scope: Some(vec![request.selector.clone()]),
            path: Some(manifest.to_owned()),
            language: None,
            import: None,
        }) {
            let _ = write_event(out, &event);
        }
        if let Ok(event) = error_event(code, message, None, None, Some("execute")) {
            let _ = write_event(out, &event);
        }
        let finished = command_finished(
            1,
            &FinishedCounts {
                results_complete: Some(false),
                ..FinishedCounts::default()
            },
        );
        let _ = write_event(out, &finished);
    }
    dx_process::operational_code()
}

fn refresh_target(
    request: &dx_bump::BumpRequest,
) -> Option<(dx_update::sets::SetId, dx_update::selector::SetRequest)> {
    match request.set {
        dx_bump::BumpSet::Cargo => Some((
            dx_update::sets::SetId::Cargo,
            dx_update::selector::SetRequest::Full,
        )),
        dx_bump::BumpSet::Npm => Some((
            dx_update::sets::SetId::Npm,
            dx_update::selector::SetRequest::Packages(vec![request.package.clone()]),
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

fn bump_offline_failed(
    invocation: &Invocation,
    out: &mut dyn std::io::Write,
    err: &mut dyn std::io::Write,
    request: &dx_bump::BumpRequest,
    manifest: &str,
) -> i32 {
    bump_refresh_failed(
        invocation,
        out,
        err,
        request,
        manifest,
        &format!(
            "failed to refresh {}: {} (widen kept in {manifest})",
            request.selector,
            dx_update::backend::BackendError::OfflineRequired {
                set: request.set.name(),
            }
        ),
    )
}

fn bump_frozen_failed(
    invocation: &Invocation,
    out: &mut dyn std::io::Write,
    err: &mut dyn std::io::Write,
    request: &dx_bump::BumpRequest,
    manifest: &str,
) -> i32 {
    bump_refresh_failed(
        invocation,
        out,
        err,
        request,
        manifest,
        &format!(
            "failed to refresh {}: {} (widen kept in {manifest})",
            request.selector,
            dx_update::backend::BackendError::FrozenLocked {
                set: request.set.name(),
            }
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::super::test_support::*;

    const CARGO_MANIFEST: &str = "rust/tests/fixtures/hello/Cargo.toml";
    const ANYHOW_REQUIREMENT: &str = "[dependencies]\nanyhow = \"1\"\n";
    const BUMP_SUMMARY: &str =
        "Widen cargo:anyhow to 1.2.3 in rust/tests/fixtures/hello/Cargo.toml \
(then refresh via `dx update cargo` automatically); if major bump, run `dx migrate --from <old> \
--to <new>` (no manifest yet => migrate_failed exit 1; missing --from/--to => exit 2 \
missing-versions)";

    const CONSUMER_DX_TOML: &str = r#"
schema_version = 1

[[dependency_set]]
name = "frontend"
ecosystem = "uv"
manifests = ["apps/frontend/pyproject.toml"]
locks = ["apps/frontend/uv.lock"]
scopes = ["apps/frontend"]
"#;

    const CONSUMER_PYPROJECT: &str = r#"
[project]
name = "frontend"
version = "0.1.0"
requires-python = ">=3.9"
dependencies = ["anyio>=4"]
"#;

    #[test]
    fn consumer_configured_set_reports_unsupported_bump_without_mutation() {
        let harness = Harness::new("bump-consumer");
        harness.write_source("dx.toml", CONSUMER_DX_TOML);
        harness.write_source("apps/frontend/pyproject.toml", CONSUMER_PYPROJECT);
        for selector in ["frontend", "frontend:anyio"] {
            let (code, _, err) = harness.run(&["bump", selector, "4.1.0"]);
            assert_eq!(code, 1, "{err}");
            assert!(
                err.contains(
                    "bump is not supported for ecosystem uv set frontend \
(supported operations: check, update, audit)"
                ),
                "{err}"
            );
        }
        assert_eq!(
            std::fs::read(harness.workspace.join("apps/frontend/pyproject.toml"))
                .expect("manifest"),
            CONSUMER_PYPROJECT.as_bytes()
        );
        assert!(harness.seen_env.borrow().is_empty());
    }

    #[test]
    fn consumer_registry_leaves_builtin_bump_selectors_alone() {
        let harness = Harness::new("bump-consumer-builtin");
        harness.write_source("dx.toml", CONSUMER_DX_TOML);
        let (code, _, err) = harness.run(&["bump", "cargo:anyhow", "1.2.3"]);
        assert_ne!(code, 0, "{err}");
        assert!(
            !err.contains("not supported for ecosystem"),
            "builtin selectors must not hit the configured branch: {err}"
        );
    }

    #[test]
    fn unreadable_and_non_utf8_manifests_fail_without_refresh() {
        for json in [false, true] {
            for invalid_utf8 in [false, true] {
                let harness = Harness::new("bump-unreadable");
                if invalid_utf8 {
                    std::fs::write(harness.workspace.join(".bazelversion"), [0xff])
                        .expect("invalid bytes");
                }
                let output = if json {
                    "--output=json"
                } else {
                    "--output=text"
                };
                let (code, out, err) =
                    harness.run(&["bump", "bazel:.bazelversion", "9.3.0", output]);
                assert_eq!(code, 1, "{out}{err}");
                assert!(err.contains(if invalid_utf8 {
                    "not valid UTF-8"
                } else {
                    "cannot read"
                }));
                assert!(harness.seen_env.borrow().is_empty());
                if json {
                    let events: Vec<serde_json::Value> = out
                        .lines()
                        .map(|line| serde_json::from_str(line).expect("event"))
                        .collect();
                    assert_eq!(events[1]["code"], super::CODE_BUMP_FAILED);
                    assert_eq!(events.last().expect("finished")["exit_code"], 1);
                }
            }
        }
    }

    #[test]
    fn file_only_and_noop_bumps_work_offline_with_json_reports() {
        for (selector, version, path, before, after) in [
            (
                "bazel:.bazelversion",
                "9.3.0",
                ".bazelversion",
                "9.2.0\n",
                "9.3.0\n",
            ),
            (
                "gha:actions/checkout",
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                ".github/workflows/ci.yml",
                "- uses: actions/checkout@v1\n",
                "- uses: actions/checkout@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\n",
            ),
            (
                "go:example.com/demo",
                "1.3.0",
                "third_party/go/go.mod",
                "require example.com/demo v1.2.0\n",
                "require example.com/demo v1.3.0\n",
            ),
        ] {
            let harness = Harness::new("bump-file-offline");
            harness.write_source(path, before);
            let (code, out, err) = harness.run(&[
                "bump",
                selector,
                version,
                "--offline",
                "--apply",
                "--output=json",
            ]);
            assert_eq!(code, 0, "{out}{err}");
            assert_eq!(
                std::fs::read_to_string(harness.workspace.join(path)).expect("manifest"),
                after
            );
            assert!(harness.seen_env.borrow().is_empty());
            let events: Vec<serde_json::Value> = out
                .lines()
                .map(|line| serde_json::from_str(line).expect("event"))
                .collect();
            assert_eq!(events[0]["event"], "command_started");
            assert!(events.iter().any(|event| event["code"] == "bump_widened"));
            assert_eq!(events.last().expect("finished")["exit_code"], 0);
        }
    }

    #[test]
    fn offline_refresh_needed_fails_before_widening() {
        for (selector, version, path, before) in [
            (
                "cargo:demo",
                "1.2.3",
                "rust/tests/fixtures/hello/Cargo.toml",
                "[dependencies]\ndemo = \"1\"\n",
            ),
            (
                "npm:demo",
                "1.2.3",
                "package.json",
                "{\n  \"dependencies\": {\n    \"demo\": \"1.0.0\"\n  }\n}\n",
            ),
        ] {
            let harness = Harness::new("bump-offline-refresh");
            harness.write_source(path, before);
            let (code, out, err) = harness.run(&[
                "bump",
                selector,
                version,
                "--offline",
                "--apply",
                "--output=json",
            ]);
            assert_eq!(code, 1, "{selector}: {out}{err}");
            assert!(
                err.contains(&format!("dx: {}: ", super::CODE_OFFLINE_REQUIRED)),
                "{selector}: {err}"
            );
            assert!(err.contains("no widen performed"), "{selector}: {err}");
            assert_eq!(
                std::fs::read_to_string(harness.workspace.join(path)).expect("manifest"),
                before,
                "{selector}: offline must not widen"
            );
            assert!(
                harness.seen_env.borrow().is_empty(),
                "{selector}: offline launches nothing"
            );
            let events = json_events(&out);
            assert_eq!(
                event(&events, "error")["code"],
                super::CODE_OFFLINE_REQUIRED
            );
            let finished = events.last().expect("finished");
            assert_eq!(finished["event"], "command_finished");
            assert_eq!(finished["exit_code"], 1);
            assert_eq!(finished["results_complete"], false);
        }
    }

    #[test]
    fn frozen_widen_needed_fails_before_any_manifest_write() {
        for (selector, version, path, before) in [
            (
                "cargo:demo",
                "1.2.3",
                "rust/tests/fixtures/hello/Cargo.toml",
                "[dependencies]\ndemo = \"1\"\n",
            ),
            (
                "npm:demo",
                "1.2.3",
                "package.json",
                "{\n  \"dependencies\": {\n    \"demo\": \"1.0.0\"\n  }\n}\n",
            ),
        ] {
            let harness = Harness::new("bump-frozen-widen");
            harness.write_source(path, before);
            let (code, out, err) = harness.run(&[
                "bump",
                selector,
                version,
                "--frozen",
                "--apply",
                "--output=json",
            ]);
            assert_eq!(code, 1, "{selector}: {out}{err}");
            assert!(
                err.contains(&format!("dx: {}: ", super::CODE_FROZEN_LOCKED)),
                "{selector}: {err}"
            );
            assert!(
                err.contains("no manifest changes performed"),
                "{selector}: {err}"
            );
            assert!(
                !err.contains("offline_required"),
                "{selector}: frozen stays independent: {err}"
            );
            assert_eq!(
                std::fs::read_to_string(harness.workspace.join(path)).expect("manifest"),
                before,
                "{selector}: frozen must not widen"
            );
            assert!(
                harness.seen_env.borrow().is_empty(),
                "{selector}: frozen launches nothing"
            );
            let events = json_events(&out);
            assert_eq!(event(&events, "error")["code"], super::CODE_FROZEN_LOCKED);
            let finished = events.last().expect("finished");
            assert_eq!(finished["event"], "command_finished");
            assert_eq!(finished["exit_code"], 1);
            assert_eq!(finished["results_complete"], false);
        }
    }

    #[test]
    fn signalled_refresh_keeps_widen_and_reports_failure() {
        let mut harness = Harness::new("bump-signalled");
        harness.signalled = true;
        harness.write_source(
            "rust/tests/fixtures/hello/Cargo.toml",
            "[dependencies]\ndemo = \"1\"\n",
        );
        let (code, out, err) =
            harness.run(&["bump", "cargo:demo", "2.0.0", "--apply", "--output=json"]);
        assert_eq!(code, 1, "{out}{err}");
        assert!(err.contains("signal"));
        assert!(out.contains("bump_widened"));
        assert!(std::fs::read_to_string(
            harness
                .workspace
                .join("rust/tests/fixtures/hello/Cargo.toml")
        )
        .expect("manifest")
        .contains("2.0.0"));
    }

    #[test]
    fn dry_run_plans_without_writing() {
        let harness = Harness::new("bump-dryrun");
        harness.write_source(CARGO_MANIFEST, ANYHOW_REQUIREMENT);
        let (code, out, err) = harness.run(&["bump", "cargo:anyhow", "1.2.3", "--dry-run"]);
        assert_eq!(code, 0, "{out}{err}");
        assert_eq!(out, format!("{BUMP_SUMMARY}\n"), "{out}");
        assert_eq!(err, "", "{err}");
        assert!(
            harness.seen_env.borrow().is_empty(),
            "dry-run launches nothing"
        );
        assert_eq!(
            std::fs::read_to_string(harness.workspace.join(CARGO_MANIFEST)).expect("read"),
            ANYHOW_REQUIREMENT
        );
    }

    #[test]
    fn dry_run_json_streams_started_planned_notice_and_finished() {
        let harness = Harness::new("bump-dryrun-json");
        harness.write_source(
            "rust/tests/fixtures/hello/Cargo.toml",
            "[dependencies]\ndemo = \"1\"\n",
        );
        let (code, out, err) =
            harness.run(&["bump", "cargo:demo", "1.2.3", "--dry-run", "--output=json"]);
        assert_eq!(code, 0, "{out}{err}");
        assert_eq!(err, "", "{err}");
        let events = json_events(&out);
        assert_eq!(events.len(), 3, "{events:?}");
        let started = event(&events, "command_started");
        assert_eq!(started["command"], "bump");
        assert_eq!(started["dry_run"], true);
        let planned = event(&events, "notice");
        assert_eq!(planned["code"], "bump_planned");
        assert_eq!(planned["level"], "info");
        assert_eq!(planned["related_command"], "bump");
        assert_eq!(planned["scope"][0], "cargo:demo");
        assert_eq!(planned["path"], "rust/tests/fixtures/hello/Cargo.toml");
        assert!(
            planned["message"]
                .as_str()
                .is_some_and(|message| message.contains("Widen cargo:demo to 1.2.3")),
            "{planned}"
        );
        let finished = events.last().expect("finished");
        assert_eq!(finished["event"], "command_finished");
        assert_eq!(finished["exit_code"], 0);
        assert_eq!(
            std::fs::read_to_string(
                harness
                    .workspace
                    .join("rust/tests/fixtures/hello/Cargo.toml")
            )
            .expect("read"),
            "[dependencies]\ndemo = \"1\"\n"
        );
    }

    #[test]
    fn dry_run_rejects_bad_selector_and_version() {
        let harness = Harness::new("bump-dryrun-bad-selector");
        let (code, _, err) = harness.run(&["bump", "crates", "1.2.3", "--dry-run"]);
        assert_eq!(code, 2, "{err}");
        let harness = Harness::new("bump-dryrun-bad-version");
        let (code, _, err) = harness.run(&["bump", "cargo:anyhow", "nope!!!", "--dry-run"]);
        assert_eq!(code, 2, "{err}");
    }

    #[test]
    fn live_widens_one_cargo_requirement_atomically() {
        let harness = Harness::new("bump-live-cargo");
        harness.write_source(
            "rust/tests/fixtures/hello/Cargo.toml",
            "[dependencies]\nanyhow = \"1\"\nserde = \"1\"\n",
        );
        let (code, out, err) = harness.run(&["bump", "cargo:anyhow", "1.2.3", "--apply"]);
        assert_eq!(code, 0, "{out}{err}");
        assert!(out.contains("widened cargo:anyhow to 1.2.3"), "{out}");
        assert!(
            out.contains("and refreshed cargo via `dx update cargo` automatically"),
            "{out}"
        );
        assert!(err.is_empty(), "{err}");
        assert_eq!(
            harness.seen_env.borrow().len(),
            1,
            "cargo chains one refresh launch"
        );
        let widened = std::fs::read_to_string(
            harness
                .workspace
                .join("rust/tests/fixtures/hello/Cargo.toml"),
        )
        .expect("read");
        assert!(widened.contains("anyhow = \"1.2.3\""), "{widened}");
        assert!(widened.contains("serde = \"1\""), "{widened}");
    }

    #[test]
    fn live_npm_chains_selective_refresh_automatically() {
        let harness = Harness::new("bump-live-npm-chain");
        harness.write_source(
            "package.json",
            "{\n  \"dependencies\": {\n    \"jest\": \"30.2.0\",\n    \"vue\": \"3.5.42\"\n  }\n}\n",
        );
        let (code, out, err) = harness.run(&["bump", "npm:jest", "30.3.0", "--apply"]);
        assert_eq!(code, 0, "{out}{err}");
        assert!(out.contains("widened npm:jest to 30.3.0"), "{out}");
        assert!(
            out.contains("and refreshed npm:jest via `dx update npm:jest` automatically"),
            "{out}"
        );
        assert!(err.is_empty(), "{err}");
        assert_eq!(
            harness.seen_env.borrow().len(),
            1,
            "npm chains one selective launch"
        );
        let widened =
            std::fs::read_to_string(harness.workspace.join("package.json")).expect("read");
        assert!(widened.contains("\"jest\": \"30.3.0\""), "{widened}");
        assert!(widened.contains("\"vue\": \"3.5.42\""), "{widened}");
    }

    #[test]
    fn live_go_chains_noop_without_launch() {
        let harness = Harness::new("bump-live-go-chain");
        harness.write_source(
            "third_party/go/go.mod",
            "module example.com/mod\n\nrequire example.com/mod v1.2.3\n",
        );
        let (code, out, err) = harness.run(&["bump", "go:example.com/mod", "1.3.0", "--apply"]);
        assert_eq!(code, 0, "{out}{err}");
        assert!(out.contains("widened go:example.com/mod"), "{out}");
        assert!(
            out.contains("and refreshed go via `dx update go` automatically"),
            "{out}"
        );
        assert!(out.contains("no-op success"), "{out}");
        assert!(err.is_empty(), "{err}");
        assert!(
            harness.seen_env.borrow().is_empty(),
            "go noop chains without launch"
        );
        let widened =
            std::fs::read_to_string(harness.workspace.join("third_party/go/go.mod")).expect("read");
        assert!(widened.contains("v1.3.0"), "{widened}");
    }

    #[test]
    fn live_maven_chains_full_refresh_automatically() {
        let harness = Harness::new("bump-live-maven-chain");
        harness.write_source(
            "MODULE.bazel",
            "maven.install(\n    artifacts = [\n        \"junit:junit:4.13.2\",\n    ],\n)\n",
        );
        let (code, out, err) = harness.run(&["bump", "maven:junit:junit", "4.13.3", "--apply"]);
        assert_eq!(code, 0, "{out}{err}");
        assert!(out.contains("widened maven:junit:junit"), "{out}");
        assert!(
            out.contains("and refreshed maven via `dx update maven` automatically"),
            "{out}"
        );
        assert!(err.is_empty(), "{err}");
        assert_eq!(
            harness.seen_env.borrow().len(),
            1,
            "maven chains one full launch"
        );
        let widened =
            std::fs::read_to_string(harness.workspace.join("MODULE.bazel")).expect("read");
        assert!(widened.contains("\"junit:junit:4.13.3\""), "{widened}");
    }

    #[test]
    fn live_nuget_chains_full_refresh_automatically() {
        let harness = Harness::new("bump-live-nuget-chain");
        harness.write_source(
            "third_party/dotnet/paket.dependencies",
            "source https://api.nuget.org/v3/index.json\nnuget FSharp.Core 10.1.201\n",
        );
        let (code, out, err) = harness.run(&["bump", "nuget:FSharp.Core", "10.1.202", "--apply"]);
        assert_eq!(code, 0, "{out}{err}");
        assert!(out.contains("widened nuget:FSharp.Core"), "{out}");
        assert!(
            out.contains("and refreshed nuget via `dx update nuget` automatically"),
            "{out}"
        );
        assert!(err.is_empty(), "{err}");
        assert_eq!(
            harness.seen_env.borrow().len(),
            1,
            "nuget chains one full launch"
        );
        let widened = std::fs::read_to_string(
            harness
                .workspace
                .join("third_party/dotnet/paket.dependencies"),
        )
        .expect("read");
        assert!(widened.contains("nuget FSharp.Core 10.1.202"), "{widened}");
    }

    #[test]
    fn live_bazel_file_only_has_no_refresh_launch() {
        let harness = Harness::new("bump-live-bazel-fileonly");
        harness.write_source(".bazelversion", "9.2.0\n");
        let (code, out, err) = harness.run(&["bump", "bazel:.bazelversion", "9.3.0", "--apply"]);
        assert_eq!(code, 0, "{out}{err}");
        assert!(
            out.contains("widened bazel:.bazelversion to 9.3.0"),
            "{out}"
        );
        assert!(out.contains("flag-diff"), "{out}");
        assert!(!out.contains("automatically"), "{out}");
        assert!(err.is_empty(), "{err}");
        assert!(
            harness.seen_env.borrow().is_empty(),
            "file-only chains nothing"
        );
    }

    #[test]
    fn live_refresh_failure_keeps_widen_and_reports_update_failed() {
        let mut harness = Harness::new("bump-live-refresh-fail");
        harness.bazel_code = 1;
        harness.write_source(
            "rust/tests/fixtures/hello/Cargo.toml",
            "[dependencies]\nanyhow = \"1\"\n",
        );
        let (code, out, err) = harness.run(&["bump", "cargo:anyhow", "1.2.3", "--apply"]);
        assert_eq!(code, 1, "{out}{err}");
        assert!(err.contains("update_failed"), "{err}");
        assert!(err.contains("updater exited 1"), "{err}");
        assert!(err.contains("widen kept"), "{err}");
        assert_eq!(harness.seen_env.borrow().len(), 1, "refresh attempted once");
        let widened = std::fs::read_to_string(
            harness
                .workspace
                .join("rust/tests/fixtures/hello/Cargo.toml"),
        )
        .expect("read");
        assert!(widened.contains("anyhow = \"1.2.3\""), "{widened}");
    }

    #[test]
    fn live_refresh_launch_failure_keeps_widen() {
        let mut harness = Harness::new("bump-live-launch-fail");
        harness.io_error = true;
        harness.write_source(
            "rust/tests/fixtures/hello/Cargo.toml",
            "[dependencies]\nanyhow = \"1\"\n",
        );
        let (code, _, err) = harness.run(&["bump", "cargo:anyhow", "1.2.3", "--apply"]);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("update_failed"), "{err}");
        assert!(err.contains("failed to launch updater"), "{err}");
        assert!(err.contains("widen kept"), "{err}");
        let widened = std::fs::read_to_string(
            harness
                .workspace
                .join("rust/tests/fixtures/hello/Cargo.toml"),
        )
        .expect("read");
        assert!(widened.contains("anyhow = \"1.2.3\""), "{widened}");
    }

    #[test]
    fn live_json_chained_emits_widened_plus_update_success() {
        let harness = Harness::new("bump-live-json-chain");
        harness.write_source(
            "rust/tests/fixtures/hello/Cargo.toml",
            "[dependencies]\nanyhow = \"1\"\n",
        );
        let (code, out, err) =
            harness.run(&["bump", "cargo:anyhow", "1.2.3", "--apply", "--output=json"]);
        assert_eq!(code, 0, "{out}{err}");
        let events = json_events(&out);
        let kinds = event_kinds(&events);
        assert_eq!(kinds[0], "command_started");
        assert_eq!(kinds[kinds.len() - 1], "command_finished");
        let notices = events_of_kind(&events, "notice");
        assert_eq!(notices.len(), 2, "{out}");
        assert_eq!(notices[0]["code"], serde_json::json!("bump_widened"));
        assert_eq!(notices[1]["code"], serde_json::json!("update_set_success"));
        assert_eq!(
            events.last().expect("finished")["exit_code"],
            serde_json::json!(0)
        );
        assert_eq!(err, "", "{err}");
    }

    #[test]
    fn live_json_refresh_failure_emits_widened_plus_error() {
        let mut harness = Harness::new("bump-live-json-fail");
        harness.bazel_code = 2;
        harness.write_source(
            "rust/tests/fixtures/hello/Cargo.toml",
            "[dependencies]\nanyhow = \"1\"\n",
        );
        let (code, out, err) =
            harness.run(&["bump", "cargo:anyhow", "1.2.3", "--apply", "--output=json"]);
        assert_eq!(code, 1, "{out}{err}");
        assert!(err.contains("update_failed"), "{err}");
        let events = json_events(&out);
        let kinds = event_kinds(&events);
        assert_eq!(kinds[0], "command_started");
        assert_eq!(kinds[kinds.len() - 1], "command_finished");
        assert!(kinds.contains(&"notice"), "{out}");
        assert!(kinds.contains(&"error"), "{out}");
        assert_eq!(
            events.last().expect("finished")["exit_code"],
            serde_json::json!(1)
        );
    }

    #[test]
    fn live_missing_requirement_fails_without_writing() {
        let harness = Harness::new("bump-live-missing");
        harness.write_source(
            "rust/tests/fixtures/hello/Cargo.toml",
            "[dependencies]\nserde = \"1\"\n",
        );
        let (code, _, err) = harness.run(&["bump", "cargo:anyhow", "1.2.3"]);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("bump_failed"), "{err}");
        assert!(err.contains("no declared requirement"), "{err}");
        assert_eq!(
            std::fs::read_to_string(
                harness
                    .workspace
                    .join("rust/tests/fixtures/hello/Cargo.toml")
            )
            .expect("read"),
            "[dependencies]\nserde = \"1\"\n"
        );
    }

    #[test]
    fn live_github_tag_needs_sha_resolution() {
        let harness = Harness::new("bump-live-gha-tag");
        harness.write_source(
            ".github/workflows/ci.yml",
            "      - uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1 # v7\n",
        );
        let (code, _, err) = harness.run(&["bump", "github-actions:actions/checkout", "v5"]);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("bump_failed"), "{err}");
        assert!(err.contains("needs SHA resolution"), "{err}");
    }

    #[test]
    fn live_github_actions_widens_every_workflow_that_pins_the_action() {
        let harness = Harness::new("bump-live-gha-every");
        let sha = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        for name in ["ci.yml", "ghcr.yml", "reusable.yaml"] {
            harness.write_source(
                &format!(".github/workflows/{name}"),
                "      - uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1 # v7\n      - uses: actions/cache@bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb # v6\n",
            );
        }
        harness.write_source(
            ".github/workflows/bump.yml",
            "      - uses: bazel-contrib/setup-bazel@c5acdfb288317d0b5c0bbd7a396a3dc868bb0f86 # v0.19.0\n",
        );
        harness.write_source(".github/workflows/README.md", "not a workflow\n");
        let (code, out, err) =
            harness.run(&["bump", "github-actions:actions/checkout", sha, "--apply"]);
        assert_eq!(code, 0, "{out}{err}");
        assert!(err.is_empty(), "{err}");
        assert!(
            out.contains(
                "widened github-actions:actions/checkout to aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa in \
                 .github/workflows/ci.yml, .github/workflows/ghcr.yml, .github/workflows/reusable.yaml"
            ),
            "{out}"
        );
        for name in ["ci.yml", "ghcr.yml", "reusable.yaml"] {
            let widened = std::fs::read_to_string(
                harness.workspace.join(&format!(".github/workflows/{name}")),
            )
            .expect("workflow");
            assert!(
                widened.contains(&format!("actions/checkout@{sha} # v7")),
                "{widened}"
            );
            assert!(widened.contains("actions/cache@bbbb"), "{widened}");
        }
        let untouched =
            std::fs::read_to_string(harness.workspace.join(".github/workflows/bump.yml"))
                .expect("workflow");
        assert!(untouched.contains("setup-bazel"), "{untouched}");
    }

    #[test]
    fn live_github_actions_fails_closed_and_writes_no_workflow() {
        let sha = "cccccccccccccccccccccccccccccccccccccccc";
        let cases = [
            ("ambiguous", vec![("ci.yml", vec!["- uses: actions/checkout@1111111111111111111111111111111111111111\n", "- uses: actions/checkout@2222222222222222222222222222222222222222\n"])], "ambiguous requirement"),
            ("unpinned", vec![("ci.yml", vec!["- uses: bazel-contrib/setup-bazel@cccccccccccccccccccccccccccccccccccccccc # v0.19.0\n"])], "no declared requirement"),
            ("no-workflows", vec![("README.md", vec!["not a workflow\n"])], "no declared requirement"),
        ];
        for (name, workflows, expected) in cases {
            let harness = Harness::new(&format!("bump-live-gha-{name}"));
            for (file, lines) in &workflows {
                harness.write_source(&format!(".github/workflows/{file}"), &lines.concat());
            }
            let (code, _, err) = harness.run(&["bump", "github-actions:actions/checkout", sha]);
            assert_eq!(code, 1, "{name}: {err}");
            assert!(err.contains("bump_failed"), "{name}: {err}");
            assert!(err.contains(expected), "{name}: {err}");
            assert!(err.contains(".github/workflows"), "{name}: {err}");
            for (file, lines) in &workflows {
                let text = std::fs::read_to_string(
                    harness.workspace.join(&format!(".github/workflows/{file}")),
                )
                .expect("workflow");
                assert_eq!(&text, &lines.concat(), "{name} must widen nothing");
            }
        }
        let missing = Harness::new("bump-live-gha-missing-dir");
        missing.write_source(".bazelversion", "9.2.0\n");
        let (code, _, err) = missing.run(&["bump", "github-actions:actions/checkout", sha]);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("cannot list .github/workflows"), "{err}");
    }

    #[test]
    fn live_json_emits_planned_or_widened_and_finished() {
        let harness = Harness::new("bump-live-json");
        harness.write_source(".bazelversion", "9.2.0\n");
        let (code, out, err) = harness.run(&[
            "bump",
            "bazel:.bazelversion",
            "9.3.0",
            "--apply",
            "--output=json",
        ]);
        assert_eq!(code, 0, "{out}{err}");
        let events = json_events(&out);
        let kinds = event_kinds(&events);
        assert_eq!(kinds[0], "command_started");
        assert_eq!(kinds[kinds.len() - 1], "command_finished");
        assert!(kinds.contains(&"notice"));
        assert_eq!(
            events.last().expect("finished")["exit_code"],
            serde_json::json!(0)
        );
        assert_eq!(err, "", "{err}");
        assert_eq!(
            std::fs::read_to_string(harness.workspace.join(".bazelversion")).expect("read"),
            "9.3.0\n"
        );
    }

    #[test]
    fn major_bump_plans_carry_migrate_hint_with_exit_mapping() {
        let harness = Harness::new("bump-major-hint-dryrun");
        harness.write_source(
            "rust/tests/fixtures/hello/Cargo.toml",
            "[dependencies]\nanyhow = \"1\"\n",
        );
        let (code, out, err) = harness.run(&["bump", "cargo:anyhow", "2.0.0", "--dry-run"]);
        assert_eq!(code, 0, "{out}{err}");
        assert!(out.contains("major bump"), "{out}");
        assert!(out.contains("dx migrate --from"), "{out}");
        assert!(out.contains("migrate_failed"), "{out}");
        assert!(out.contains("missing-versions"), "{out}");
        let harness = Harness::new("bump-major-hint-live");
        harness.write_source(
            "rust/tests/fixtures/hello/Cargo.toml",
            "[dependencies]\nanyhow = \"1\"\n",
        );
        let (code, out, err) = harness.run(&["bump", "cargo:anyhow", "2.0.0", "--apply"]);
        assert_eq!(code, 0, "{out}{err}");
        assert!(out.contains("widened cargo:anyhow to 2.0.0"), "{out}");
        assert!(out.contains("major bump"), "{out}");
        assert!(out.contains("migrate_failed"), "{out}");
        assert!(out.contains("missing-versions"), "{out}");
    }
    #[test]
    fn offline_dry_run_plans_cache_only_without_writing() {
        let harness = Harness::new("bump-offline-dryrun");
        harness.write_source(CARGO_MANIFEST, ANYHOW_REQUIREMENT);
        let (code, out, err) =
            harness.run(&["bump", "cargo:anyhow", "1.2.3", "--offline", "--dry-run"]);
        assert_eq!(code, 0, "{out}{err}");
        assert_eq!(
            out,
            format!("{BUMP_SUMMARY} (offline, cache-only)\n"),
            "{out}"
        );
        assert_eq!(err, "", "{err}");
        assert!(
            harness.seen_env.borrow().is_empty(),
            "offline dry-run launches nothing"
        );
        assert_eq!(
            std::fs::read_to_string(harness.workspace.join(CARGO_MANIFEST)).expect("read"),
            ANYHOW_REQUIREMENT
        );
    }

    #[test]
    fn offline_live_resolver_fails_before_widen_without_mutation() {
        let harness = Harness::new("bump-offline-resolver");
        harness.write_source(
            "rust/tests/fixtures/hello/Cargo.toml",
            "[dependencies]\nanyhow = \"1\"\n",
        );
        let (code, _, err) =
            harness.run(&["bump", "cargo:anyhow", "1.2.3", "--offline", "--apply"]);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("offline_required"), "{err}");
        assert!(err.contains("without network"), "{err}");
        assert!(
            harness.seen_env.borrow().is_empty(),
            "offline launches nothing"
        );
        assert_eq!(
            std::fs::read_to_string(
                harness
                    .workspace
                    .join("rust/tests/fixtures/hello/Cargo.toml")
            )
            .expect("read"),
            "[dependencies]\nanyhow = \"1\"\n",
            "offline must not widen"
        );
        let fileonly = Harness::new("bump-offline-fileonly");
        fileonly.write_source(".bazelversion", "9.2.0\n");
        let (code, out, err) = fileonly.run(&[
            "bump",
            "bazel:.bazelversion",
            "9.3.0",
            "--offline",
            "--apply",
        ]);
        assert_eq!(code, 0, "{out}{err}");
        assert!(out.contains("widened bazel:.bazelversion"), "{out}");
        assert!(fileonly.seen_env.borrow().is_empty());
        let go = Harness::new("bump-offline-go");
        go.write_source(
            "third_party/go/go.mod",
            "module example.com/mod\n\nrequire example.com/mod v1.2.3\n",
        );
        let (code, out, err) = go.run(&[
            "bump",
            "go:example.com/mod",
            "1.3.0",
            "--offline",
            "--apply",
        ]);
        assert_eq!(code, 0, "{out}{err}");
        assert!(out.contains("no-op success"), "{out}");
        assert!(go.seen_env.borrow().is_empty());
    }

    #[test]
    fn check_reports_drift_without_writing() {
        let harness = Harness::new("bump-check-drift");
        harness.write_source(
            "rust/tests/fixtures/hello/Cargo.toml",
            "[dependencies]\nanyhow = \"1\"\n",
        );
        let (code, out, err) = harness.run(&["bump", "cargo:anyhow", "1.2.3"]);
        assert_eq!(code, 1, "{out}{err}");
        assert!(err.contains("bump_failed"), "{err}");
        assert!(
            err.contains("would widen rust/tests/fixtures/hello/Cargo.toml"),
            "{err}"
        );
        assert!(err.contains("--apply"), "{err}");
        assert!(
            err.contains("also refreshes cargo via `dx update cargo`"),
            "{err}"
        );
        assert_eq!(
            std::fs::read_to_string(
                harness
                    .workspace
                    .join("rust/tests/fixtures/hello/Cargo.toml")
            )
            .expect("read"),
            "[dependencies]\nanyhow = \"1\"\n",
            "check must not widen"
        );
        assert!(
            harness.seen_env.borrow().is_empty(),
            "check launches no refresh"
        );
    }

    #[test]
    fn check_passes_when_already_at_version() {
        let harness = Harness::new("bump-check-current");
        harness.write_source(".bazelversion", "9.3.0\n");
        let (code, out, err) = harness.run(&["bump", "bazel:.bazelversion", "9.3.0"]);
        assert_eq!(code, 0, "{out}{err}");
        assert!(out.contains("already at 9.3.0 in .bazelversion"), "{out}");
        assert_eq!(err, "", "{err}");
        assert_eq!(
            std::fs::read_to_string(harness.workspace.join(".bazelversion")).expect("read"),
            "9.3.0\n"
        );
        assert!(harness.seen_env.borrow().is_empty());
    }

    #[test]
    fn check_json_reports_drift_with_error_and_finished() {
        let harness = Harness::new("bump-check-json");
        harness.write_source(".bazelversion", "9.2.0\n");
        let (code, out, err) =
            harness.run(&["bump", "bazel:.bazelversion", "9.3.0", "--output=json"]);
        assert_eq!(code, 1, "{out}{err}");
        let events = json_events(&out);
        let kinds = event_kinds(&events);
        assert_eq!(
            kinds,
            vec!["command_started", "error", "command_finished"],
            "{out}"
        );
        assert_eq!(
            events[1]["code"],
            serde_json::json!(super::CODE_BUMP_FAILED)
        );
        assert_eq!(
            events.last().expect("finished")["exit_code"],
            serde_json::json!(1)
        );
        assert_eq!(
            std::fs::read_to_string(harness.workspace.join(".bazelversion")).expect("read"),
            "9.2.0\n",
            "check must not write"
        );
    }

    #[test]
    fn check_explicit_flag_matches_the_bare_default() {
        let harness = Harness::new("bump-check-explicit");
        harness.write_source(".bazelversion", "9.3.0\n");
        let (code, out, err) = harness.run(&["bump", "bazel:.bazelversion", "9.3.0", "--check"]);
        assert_eq!(code, 0, "{out}{err}");
        assert!(out.contains("already at 9.3.0 in .bazelversion"), "{out}");
    }
}
