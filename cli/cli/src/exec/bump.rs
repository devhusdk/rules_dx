use super::common::*;
use super::dep_edit;
pub use dx_bump::gha::COMMIT_JSON_ENV;
use dx_output::{
    command_finished, command_started, notice_event, FinishedCounts, NoticeEvent, OutputMode,
};
use dx_plan::args::{Command, Invocation};
use dx_plan::reports::plan_reports;

pub(crate) fn execute_bump(invocation: &Invocation, env: Env<'_>) -> i32 {
    debug_assert!(
        invocation.command == Command::Bump,
        "bump dispatch guards commands"
    );
    let Env {
        workspace,
        runner,
        query_runner,
        out,
        err,
        bump_commit_json,
        ..
    } = env;
    match plan_reports(
        workspace,
        invocation.command,
        &invocation.reports,
        &invocation.output,
    ) {
        Ok(_) => {}
        Err(error) => return pre_exec(invocation.output, out, err, &error.to_string()),
    }
    if invocation.targets.len() != 2 {
        return pre_exec(
            invocation.output,
            out,
            err,
            "bump needs exactly <selector> <version>",
        );
    }
    let request = match configured_request(invocation, out, err, workspace) {
        Ok(Some(request)) => request,
        Ok(None) => {
            match dx_bump::BumpRequest::parse(&invocation.targets[0], &invocation.targets[1]) {
                Ok(request) => request,
                Err(error) => return pre_exec(invocation.output, out, err, &error.to_string()),
            }
        }
        Err(exit) => return exit,
    };
    let request =
        match dx_bump::DepVersion::from_flags(invocation.floor, invocation.reason.as_deref())
            .and_then(|target| request.with_dep_version(target))
        {
            Ok(request) => request,
            Err(error) => return pre_exec(invocation.output, out, err, &error.to_string()),
        };
    let request = match resolve_gha_tag_request(&request, bump_commit_json) {
        Ok(request) => request,
        Err(message) => {
            return operational(invocation, out, err, CODE_BUMP_FAILED, &message);
        }
    };
    let summary = frozen_summary(
        offline_summary(request.summary(), invocation.offline),
        invocation.frozen,
    );
    let verbose = invocation.chatty();
    if !invocation.applies() {
        return check_bump(invocation, out, err, workspace, &request, verbose);
    }
    if invocation.output == OutputMode::Json {
        if let Err(exit) = emit_built(out, command_started(invocation.command.name(), "default")) {
            return exit;
        }
    } else if verbose {
        if let Err(exit) = check_stdout_write(writeln!(out, "{summary}")) {
            return exit;
        }
    }
    let planned = match plan_widens(&request, workspace) {
        Ok(planned) => planned,
        Err(WidenFailure::Usage(message)) => {
            return pre_exec(invocation.output, out, err, &message);
        }
        Err(WidenFailure::Failed(message)) => {
            return operational(invocation, out, err, CODE_BUMP_FAILED, &message);
        }
    };
    if planned.iter().all(|(_, _, changed)| !changed) {
        return emit_bump_no_change(invocation, out, &request, &planned, verbose);
    }
    let target = request.target();
    if let Err(exit) = dep_edit::preflight(
        &BUMP_VERB,
        invocation,
        out,
        err,
        workspace,
        &target,
        &request.selector,
    ) {
        return exit;
    }
    let changed: Vec<(String, String)> = planned
        .into_iter()
        .filter(|(_, _, changed)| *changed)
        .map(|(manifest, text, _)| (manifest, text))
        .collect();
    let manifest = changed
        .iter()
        .map(|(manifest, _)| manifest.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    let edit = dep_edit::DepEdit {
        verb: &BUMP_VERB,
        target: &target,
        selector: &request.selector,
        edited: format!(
            "widened {} to {} in {manifest}",
            request.selector,
            request.version.display()
        ),
        kept: format!("widen kept in {manifest}"),
        hint: request
            .version
            .is_semver()
            .then(dx_bump::generic_major_bump_hint),
    };
    let io = dep_edit::EditIo {
        workspace,
        runner,
        query_runner,
        out,
        err,
    };
    dep_edit::write_and_refresh(&edit, invocation, io, &changed, verbose)
}

const BUMP_VERB: dep_edit::EditVerb = dep_edit::EditVerb {
    command: "bump",
    noun: "widen",
    set_action: "widen",
    notice_code: CODE_BUMP_WIDENED,
    failure_code: CODE_BUMP_FAILED,
    noop_note: "pinned module lock; no-op success",
};

fn configured_request(
    invocation: &Invocation,
    out: &mut dyn std::io::Write,
    err: &mut dyn std::io::Write,
    workspace: &std::path::Path,
) -> Result<Option<dx_bump::BumpRequest>, i32> {
    let Some(configured) =
        dep_edit::configured_set(&BUMP_VERB, invocation, out, err, workspace, true)?
    else {
        return Ok(None);
    };
    dx_bump::BumpRequest::parse_configured_set(
        configured.kind,
        &configured.name,
        &configured.manifests,
        configured.repository.as_deref(),
        &invocation.targets[0],
        &invocation.targets[1],
    )
    .map(Some)
    .map_err(|error| pre_exec(invocation.output, out, err, &error.to_string()))
}

fn check_bump(
    invocation: &Invocation,
    out: &mut dyn std::io::Write,
    err: &mut dyn std::io::Write,
    workspace: &std::path::Path,
    request: &dx_bump::BumpRequest,
    verbose: bool,
) -> i32 {
    if invocation.output == OutputMode::Json {
        if let Err(exit) = emit_built(out, command_started(invocation.command.name(), "check")) {
            return exit;
        }
    }
    let planned = match plan_widens(request, workspace) {
        Ok(planned) => planned,
        Err(WidenFailure::Usage(message)) => {
            return pre_exec(invocation.output, out, err, &message);
        }
        Err(WidenFailure::Failed(message)) => {
            return operational(invocation, out, err, CODE_BUMP_FAILED, &message);
        }
    };
    let mut drifted: Vec<String> = Vec::new();
    for (manifest, widened, _) in &planned {
        let current = dx_path::workspace::resolve_in_workspace(workspace, manifest)
            .and_then(std::fs::read)
            .ok()
            .and_then(|bytes| String::from_utf8(bytes).ok());
        if current.as_deref() != Some(widened.as_str()) {
            drifted.push(manifest.clone());
        }
    }
    let manifest = planned
        .iter()
        .map(|(manifest, _, _)| manifest.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    if drifted.is_empty() {
        let message = format!(
            "bump {} to {} current in {manifest}",
            request.selector,
            request.version.display()
        );
        if invocation.output == OutputMode::Json {
            if let Err(exit) = emit_built(
                out,
                notice_event(&NoticeEvent {
                    related_command: Some("bump".to_owned()),
                    scope: Some(vec![request.selector.clone()]),
                    path: Some(manifest.clone()),
                    ..NoticeEvent::info(CODE_BUMP_WIDENED, message.clone())
                }),
            ) {
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
    let message = format!(
        "bump drift: {} needs widening to {} in {} (run `dx bump {} {} --apply`)",
        request.selector,
        request.version.display(),
        drifted.join(", "),
        request.selector,
        request.version.display()
    );
    operational(invocation, out, err, CODE_BUMP_FAILED, &message)
}

fn widen_targets(
    request: &dx_bump::BumpRequest,
    workspace: &std::path::Path,
) -> Result<Vec<String>, String> {
    if let Some(manifests) = request.configured_manifests() {
        return Ok(manifests.to_vec());
    }
    match request.set {
        dx_bump::BumpSet::GithubActions => {}
        dx_bump::BumpSet::Cargo => {
            let module = std::fs::read_to_string(workspace.join("MODULE.bazel")).map_err(|_| {
                format!(
                    "failed to widen {}: cannot read MODULE.bazel",
                    request.selector
                )
            })?;
            return dx_bump::cargo_manifests(&module)
                .map_err(|error| format!("failed to widen {}: {error}", request.selector));
        }
        _ => return Ok(vec![request.target_manifest().to_owned()]),
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

/// Why a widen could not be planned: a refused argument (exit 2) or a failed edit.
enum WidenFailure {
    Usage(String),
    Failed(String),
}

impl From<String> for WidenFailure {
    fn from(message: String) -> Self {
        WidenFailure::Failed(message)
    }
}

fn plan_widens(
    request: &dx_bump::BumpRequest,
    workspace: &std::path::Path,
) -> Result<Vec<(String, String, bool)>, WidenFailure> {
    let mut planned = Vec::new();
    let targets = widen_targets(request, workspace)?;
    for manifest in &targets {
        let manifest = manifest.clone();
        let original = dx_path::workspace::resolve_in_workspace(workspace, &manifest)
            .and_then(std::fs::read)
            .map_err(|error| {
                if error.kind() == std::io::ErrorKind::InvalidInput {
                    format!(
                        "failed to widen {}: {manifest} leaves the workspace",
                        request.selector
                    )
                } else {
                    format!(
                        "failed to widen {}: cannot read {manifest}",
                        request.selector
                    )
                }
            })?;
        let text = String::from_utf8(original).map_err(|_| {
            format!(
                "failed to widen {}: {manifest} is not valid UTF-8",
                request.selector
            )
        })?;
        match request.plan_edit_outcome(&manifest, &text) {
            Ok(dx_bump::BumpPlanOutcome::Changed(widened)) => {
                planned.push((manifest, widened, true));
            }
            Ok(dx_bump::BumpPlanOutcome::Unchanged) => {
                planned.push((manifest, text, false));
            }
            Err(dx_bump::BumpError::NotFound { .. }) if request.set.spans_manifests() => {}
            Err(error) if error.is_usage() => {
                return Err(WidenFailure::Usage(format!(
                    "failed to widen {}: {error}",
                    request.selector
                )));
            }
            Err(error) => {
                return Err(format!("failed to widen {}: {error}", request.selector).into());
            }
        }
    }
    if planned.is_empty() {
        let manifest = if let Some(name) = request.configured_name() {
            if request.set == dx_bump::BumpSet::Cargo {
                format!("any manifest of set {name}")
            } else {
                request.target_manifest().to_owned()
            }
        } else if request.set == dx_bump::BumpSet::Cargo {
            format!(
                "any of the {} manifests of the crate.from_cargo hub in MODULE.bazel",
                targets.len()
            )
        } else {
            request.target_manifest().to_owned()
        };
        return Err(format!(
            "failed to widen {}: {}",
            request.selector,
            dx_bump::BumpError::NotFound {
                manifest,
                package: request.package.clone(),
            }
        )
        .into());
    }
    Ok(planned)
}

fn resolve_gha_tag_request(
    request: &dx_bump::BumpRequest,
    commit_json: Option<&str>,
) -> Result<dx_bump::BumpRequest, String> {
    let is_gha_tag = matches!(&request.version, dx_bump::WidenVersion::GitTag(_))
        && request.set == dx_bump::BumpSet::GithubActions;
    if !is_gha_tag {
        return Ok(request.clone());
    }
    let Some(path) = commit_json else {
        return Ok(request.clone());
    };
    let text = std::fs::read_to_string(&path).map_err(|_| {
        format!(
            "failed to widen {}: cannot read {path} (nothing widened; unknown tags fail closed, never invent a SHA)",
            request.selector
        )
    })?;
    let sha = dx_bump::gha::resolve_sha_from_commit_response(&text)
        .map_err(|error| format!("failed to widen {}: {error}", request.selector))?;
    dx_bump::BumpRequest::parse(&request.selector, &sha)
        .map_err(|error| format!("failed to widen {}: {error}", request.selector))
}

fn emit_bump_no_change(
    invocation: &Invocation,
    out: &mut dyn std::io::Write,
    request: &dx_bump::BumpRequest,
    planned: &[(String, String, bool)],
    verbose: bool,
) -> i32 {
    let manifest = planned
        .iter()
        .map(|(manifest, _, _)| manifest.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    let message = format!(
        "{} already at {} in {manifest} (no-change; nothing widened)",
        request.selector,
        request.version.display()
    );
    if invocation.output == OutputMode::Json {
        if let Err(exit) = emit_built(
            out,
            notice_event(&NoticeEvent {
                related_command: Some("bump".to_owned()),
                scope: Some(vec![request.selector.clone()]),
                path: Some(manifest),
                ..NoticeEvent::info(CODE_BUMP_NO_CHANGE, message.clone())
            }),
        ) {
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
    0
}

#[cfg(test)]
mod tests {
    use super::super::test_support::*;

    const HELLO_CARGO: &str = "rust/tests/fixtures/hello/Cargo.toml";

    /// A root MODULE.bazel whose dx_crates hub reads the given Cargo.toml labels.
    fn cargo_module(labels: &[&str]) -> String {
        let manifests = labels
            .iter()
            .map(|label| format!("        \"{label}\",\n"))
            .collect::<String>();
        format!(
            "bazel_dep(name = \"rules_rust\", version = \"0.74.0\")\n\ncrate = use_extension(\"@rules_rust//crate_universe:extensions.bzl\", \"crate\")\ncrate.from_cargo(\n    name = \"dx_crates\",\n    cargo_lockfile = \"//rust/tests/fixtures/hello:Cargo.lock\",\n    manifests = [\n{manifests}    ],\n)\n"
        )
    }

    /// Writes the cargo manifests and a MODULE.bazel hub that lists all of them.
    fn seed_cargo(harness: &Harness, manifests: &[(&str, &str)]) {
        let labels: Vec<String> = manifests
            .iter()
            .map(|(path, _)| {
                let (package, file) = path.rsplit_once('/').expect("package path");
                format!("//{package}:{file}")
            })
            .collect();
        let labels: Vec<&str> = labels.iter().map(String::as_str).collect();
        harness.write_source("MODULE.bazel", &cargo_module(&labels));
        for (path, text) in manifests {
            harness.write_source(path, text);
        }
    }

    /// Writes one manifest; a Cargo.toml also gets the MODULE.bazel hub that lists it.
    fn seed(harness: &Harness, path: &str, text: &str) {
        if path.ends_with("Cargo.toml") {
            seed_cargo(harness, &[(path, text)]);
        } else {
            harness.write_source(path, text);
        }
    }

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
    fn consumer_configured_uv_set_needs_a_package_and_widens_pyproject() {
        let harness = Harness::new("bump-consumer");
        harness.write_source("dx.toml", CONSUMER_DX_TOML);
        harness.write_source("apps/frontend/pyproject.toml", CONSUMER_PYPROJECT);
        let (code, _, err) = harness.run(&["bump", "frontend", "4.1.0", "--apply"]);
        assert_ne!(code, 0, "{err}");
        assert!(
            err.contains("bump needs one package, not a whole set"),
            "{err}"
        );
        assert_eq!(
            std::fs::read(harness.workspace.join("apps/frontend/pyproject.toml"))
                .expect("manifest"),
            CONSUMER_PYPROJECT.as_bytes()
        );
        assert!(harness.seen_env.borrow().is_empty());
        // The refresh cannot run offline, but the widen is refused before any write.
        let (code, _, err) =
            harness.run(&["bump", "frontend:anyio", "4.1.0", "--apply", "--offline"]);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("without network"), "{err}");
        assert_eq!(
            std::fs::read(harness.workspace.join("apps/frontend/pyproject.toml"))
                .expect("manifest"),
            CONSUMER_PYPROJECT.as_bytes()
        );
    }

    const PINNED_DX_TOML: &str = r#"
schema_version = 1

[[dependency_set]]
name = "gems"
ecosystem = "ruby"
manifests = ["apps/rb/Gemfile"]
locks = ["apps/rb/Gemfile.lock"]
scopes = ["apps/rb"]

[[dependency_set]]
name = "modules"
ecosystem = "powershell"
manifests = ["tools/ps/PSGallery.requirements.psd1"]
locks = ["tools/ps/PSGallery.lock.json"]
scopes = ["tools/ps"]
"#;

    #[test]
    fn consumer_pinned_sets_refuse_bump_without_mutation() {
        let harness = Harness::new("bump-consumer-pinned");
        harness.write_source("dx.toml", PINNED_DX_TOML);
        let files = [
            (
                "apps/rb/Gemfile",
                "source \"https://rubygems.org\"\ngem \"rack\"\n",
            ),
            ("apps/rb/Gemfile.lock", "GEM\n  specs:\n    rack (3.0.0)\n"),
            (
                "tools/ps/PSGallery.requirements.psd1",
                "@{ Pester = '5.5.0' }\n",
            ),
            ("tools/ps/PSGallery.lock.json", "{}\n"),
        ];
        for (rel, text) in files {
            harness.write_source(rel, text);
        }
        for (selector, want) in [
            (
                "gems:rack",
                "bump is not supported for ecosystem ruby set gems \
(supported operations: audit; check and update report pinned)",
            ),
            (
                "modules:Pester",
                "bump is not supported for ecosystem powershell set modules \
(supported operations: audit; check and update report pinned)",
            ),
        ] {
            let (code, _, err) = harness.run(&["bump", selector, "1.0", "--apply"]);
            assert_eq!(code, 1, "{err}");
            assert!(err.contains(want), "{err}");
        }
        for (rel, text) in files {
            assert_eq!(
                std::fs::read(harness.workspace.join(rel)).expect("unchanged"),
                text.as_bytes(),
                "{rel}"
            );
        }
        assert!(harness.seen_env.borrow().is_empty());
    }

    const CARGO_SET_DX_TOML: &str = "schema_version = 1\n[[dependency_set]]\nname = \"crates\"\necosystem = \"cargo\"\nmanifests = [\"rs/a/Cargo.toml\"]\nlocks = [\"rs/a/Cargo.lock\"]\nscopes = [\"rs/a\"]\nrepository = \"crates\"\n[[dependency_set]]\nname = \"tools\"\necosystem = \"cargo\"\nmanifests = [\"rs/b/Cargo.toml\"]\nlocks = [\"rs/b/Cargo.lock\"]\nscopes = [\"rs/b\"]\nrepository = \"tools\"\n";
    const CARGO_A: &str =
        "[package]\nname = \"a\"\nversion = \"0.1.0\"\n\n[dependencies]\nserde = \"1\"\n";
    const CARGO_B: &str =
        "[package]\nname = \"b\"\nversion = \"0.1.0\"\n\n[dependencies]\nanyhow = \"1\"\n";

    fn seed_cargo_set(harness: &Harness) {
        harness.write_source("dx.toml", CARGO_SET_DX_TOML);
        harness.write_source("rs/a/Cargo.toml", CARGO_A);
        harness.write_source("rs/b/Cargo.toml", CARGO_B);
        harness.write_source("rs/a/Cargo.lock", "version = 4\n");
        harness.write_source("rs/b/Cargo.lock", "version = 4\n");
    }

    fn assert_unchanged(harness: &Harness, files: &[(&str, &str)]) {
        for (rel, text) in files {
            assert_eq!(
                std::fs::read(harness.workspace.join(rel)).expect("unchanged"),
                text.as_bytes(),
                "{rel}"
            );
        }
    }

    #[test]
    fn consumer_cargo_set_widens_its_manifest_only_and_repins_the_workspace() {
        let harness = Harness::new("bump-consumer-cargo");
        seed_cargo_set(&harness);
        let (code, out, err) = harness.run(&["bump", "crates:serde", "2.0.0", "--apply"]);
        assert_eq!(code, 0, "{out}{err}");
        assert!(
            out.contains("widened crates:serde to 2.0.0 in rs/a/Cargo.toml"),
            "{out}"
        );
        assert!(
            out.contains("refreshed crates via `dx update crates`"),
            "{out}"
        );
        assert_eq!(
            read(&harness, "rs/a/Cargo.toml"),
            CARGO_A.replace("\"1\"", "\"2.0.0\"")
        );
        assert_eq!(read(&harness, "rs/b/Cargo.toml"), CARGO_B);
        let runs = harness.build_argv.borrow();
        assert_eq!(runs.len(), 1, "{runs:?}");
        assert!(runs[0].iter().any(|arg| arg == "fetch"), "{:?}", runs[0]);
        assert_eq!(runs[0].last().map(String::as_str), Some("--repo=@crates"));
        assert_eq!(
            *harness.seen_env.borrow(),
            vec![dx_update::backend::cargo_env("workspace", "crates")]
        );
        assert_eq!(*harness.seen_cwd.borrow(), vec![harness.workspace.clone()]);
    }

    #[test]
    fn consumer_cargo_set_refusals_write_nothing_and_launch_nothing() {
        let harness = Harness::new("bump-consumer-cargo-refuse");
        seed_cargo_set(&harness);
        let files = [
            ("rs/a/Cargo.toml", CARGO_A),
            ("rs/b/Cargo.toml", CARGO_B),
            ("rs/a/Cargo.lock", "version = 4\n"),
            ("rs/b/Cargo.lock", "version = 4\n"),
        ];
        let (code, _, err) =
            harness.run(&["bump", "crates:serde", "2.0.0", "--offline", "--apply"]);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("offline_required"), "{err}");
        let (code, _, err) = harness.run(&["bump", "crates:serde", "2.0.0", "--frozen", "--apply"]);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("frozen_locked"), "{err}");
        let (code, _, err) = harness.run(&["bump", "crates:missing", "2.0.0", "--apply"]);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("any manifest of set crates"), "{err}");
        let (code, _, err) = harness.run(&["bump", "crates", "2.0.0", "--apply"]);
        assert_eq!(code, 2, "{err}");
        let (code, _, err) = harness.run(&[
            "bump",
            "crates:serde",
            "2.0.0",
            "--floor",
            "--reason",
            "x",
            "--apply",
        ]);
        assert_ne!(code, 0, "{err}");
        assert_unchanged(&harness, &files);
        harness.write_source(
            "dx.toml",
            &CARGO_SET_DX_TOML.replace(
                "scopes = [\"rs/a\"]",
                "scopes = [\"rs/a\"]\nwritable = false",
            ),
        );
        let (code, _, err) = harness.run(&["bump", "crates:serde", "2.0.0", "--apply"]);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("read-only set crates"), "{err}");
        harness.write_source(
            "dx.toml",
            &CARGO_SET_DX_TOML.replace(
                "locks = [\"rs/a/Cargo.lock\"]",
                "locks = [\"rs/a/other.lock\"]",
            ),
        );
        let (code, _, err) = harness.run(&["bump", "crates:serde", "2.0.0", "--apply"]);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("no widen performed"), "{err}");
        assert_unchanged(&harness, &files);
        assert!(harness.build_argv.borrow().is_empty());
        assert!(harness.seen_env.borrow().is_empty());
    }

    #[test]
    fn consumer_cargo_set_refresh_failure_keeps_the_widen_and_check_reports_drift() {
        let mut harness = Harness::new("bump-consumer-cargo-fail");
        harness.bazel_code = 3;
        seed_cargo_set(&harness);
        let (code, _, err) = harness.run(&["bump", "crates:serde", "2.0.0"]);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("bump drift"), "{err}");
        assert_unchanged(&harness, &[("rs/a/Cargo.toml", CARGO_A)]);
        assert!(harness.build_argv.borrow().is_empty());
        let (code, _, err) = harness.run(&["bump", "crates:serde", "2.0.0", "--apply"]);
        assert_eq!(code, 1, "{err}");
        assert!(read(&harness, "rs/a/Cargo.toml").contains("2.0.0"));
        assert_eq!(read(&harness, "rs/b/Cargo.toml"), CARGO_B);
    }

    const MAVEN_SET_DX_TOML: &str = "schema_version = 1\n[[dependency_set]]\nname = \"jvm\"\necosystem = \"maven\"\nmanifests = [\"MODULE.bazel\"]\nlocks = [\"third_party/maven_install.json\"]\nscopes = [\"third_party\"]\nrepository = \"maven\"\n";
    const MAVEN_MODULE: &str = "maven = use_extension(\"@rules_jvm_external//:extensions.bzl\", \"maven\")\nmaven.install(\n    name = \"maven\",\n    artifacts = [\"junit:junit:4.13.2\"],\n    lock_file = \"//third_party:maven_install.json\",\n)\nmaven.install(\n    name = \"other\",\n    artifacts = [\"junit:junit:4.12\"],\n)\nuse_repo(maven, \"maven\", \"other\")\n";

    fn seed_maven_set(harness: &Harness) {
        harness.write_source("dx.toml", MAVEN_SET_DX_TOML);
        harness.write_source("MODULE.bazel", MAVEN_MODULE);
        harness.write_source("third_party/maven_install.json", "{}\n");
    }

    #[test]
    fn consumer_maven_set_edits_only_its_own_install_and_repins() {
        let harness = Harness::new("bump-consumer-maven");
        seed_maven_set(&harness);
        let (code, out, err) = harness.run(&["bump", "jvm:junit:junit", "4.13.3", "--apply"]);
        assert_eq!(code, 0, "{out}{err}");
        assert!(out.contains("refreshed jvm via `dx update jvm`"), "{out}");
        assert_eq!(
            read(&harness, "MODULE.bazel"),
            MAVEN_MODULE.replace("junit:junit:4.13.2", "junit:junit:4.13.3")
        );
        let runs = harness.build_argv.borrow();
        assert_eq!(runs.len(), 1, "{runs:?}");
        assert!(runs[0].iter().any(|arg| arg == "run"), "{:?}", runs[0]);
        assert_eq!(runs[0].last().map(String::as_str), Some("@maven//:pin"));
        assert_eq!(
            *harness.seen_env.borrow(),
            vec![dx_update::backend::maven_env(true)]
        );
    }

    #[test]
    fn consumer_maven_set_refusals_write_nothing_and_launch_nothing() {
        let harness = Harness::new("bump-consumer-maven-refuse");
        seed_maven_set(&harness);
        let files = [
            ("MODULE.bazel", MAVEN_MODULE),
            ("third_party/maven_install.json", "{}\n"),
        ];
        for (flag, want) in [
            ("--offline", "offline_required"),
            ("--frozen", "frozen_locked"),
        ] {
            let (code, _, err) =
                harness.run(&["bump", "jvm:junit:junit", "4.13.3", flag, "--apply"]);
            assert_eq!(code, 1, "{err}");
            assert!(err.contains(want), "{err}");
        }
        let (code, _, err) =
            harness.run(&["bump", "jvm:com.google.guava:guava", "33.0.0", "--apply"]);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("no declared requirement"), "{err}");
        harness.write_source("third_party/maven_install.json", "{}\n");
        harness.write_source(
            "dx.toml",
            &MAVEN_SET_DX_TOML.replace("\"third_party/maven_install.json\"", "\"other.json\""),
        );
        let (code, _, err) = harness.run(&["bump", "jvm:junit:junit", "4.13.3", "--apply"]);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("no widen performed"), "{err}");
        assert_unchanged(&harness, &files);
        assert!(harness.build_argv.borrow().is_empty());
        assert!(harness.seen_env.borrow().is_empty());
    }

    const NUGET_SET_DX_TOML: &str = "schema_version = 1\n[[dependency_set]]\nname = \"dotnet\"\necosystem = \"nuget\"\nmanifests = [\"dotnet/paket.dependencies\"]\nlocks = [\"dotnet/paket.lock\", \"dotnet/paket.main.bzl\", \"dotnet/paket.main_extension.bzl\"]\nscopes = [\"dotnet\"]\n";
    const PAKET_DEPS: &str =
        "source https://api.nuget.org/v3/index.json\nnuget Newtonsoft.Json 12.0.1\n";

    #[test]
    fn consumer_nuget_set_edits_paket_dependencies_only_and_names_the_follow_up() {
        let harness = Harness::new("bump-consumer-nuget");
        harness.write_source("dx.toml", NUGET_SET_DX_TOML);
        let files = [
            ("dotnet/paket.lock", "NUGET\n"),
            ("dotnet/paket.main.bzl", "main\n"),
            ("dotnet/paket.main_extension.bzl", "ext\n"),
        ];
        harness.write_source("dotnet/paket.dependencies", PAKET_DEPS);
        for (rel, text) in files {
            harness.write_source(rel, text);
        }
        let (code, _, err) = harness.run(&[
            "bump",
            "dotnet:Newtonsoft.Json",
            "13.0.3",
            "--frozen",
            "--apply",
        ]);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("frozen_locked"), "{err}");
        assert_unchanged(&harness, &[("dotnet/paket.dependencies", PAKET_DEPS)]);
        let (code, out, err) = harness.run(&[
            "bump",
            "dotnet:Newtonsoft.Json",
            "13.0.3",
            "--offline",
            "--apply",
        ]);
        assert_eq!(code, 0, "{out}{err}");
        assert!(
            out.contains("run `paket update` in dotnet, then `dx update dotnet`"),
            "{out}"
        );
        assert!(!out.contains("refreshed"), "{out}");
        assert_eq!(
            read(&harness, "dotnet/paket.dependencies"),
            PAKET_DEPS.replace("12.0.1", "13.0.3")
        );
        assert_unchanged(&harness, &files);
        assert!(harness.build_argv.borrow().is_empty());
        assert!(harness.seen_env.borrow().is_empty());
    }

    #[test]
    fn consumer_go_set_refuses_bump_without_mutation() {
        let harness = Harness::new("bump-consumer-go");
        harness.write_source(
            "dx.toml",
            "schema_version = 1\n[[dependency_set]]\nname = \"gomods\"\necosystem = \"go\"\nmanifests = [\"go.mod\"]\nlocks = [\"go.sum\"]\nscopes = [\".\"]\n",
        );
        let files = [
            (
                "go.mod",
                "module example.com/m\n\ngo 1.22\n\nrequire github.com/google/go-cmp v0.6.0\n",
            ),
            ("go.sum", "github.com/google/go-cmp v0.6.0 h1:abc=\n"),
        ];
        for (rel, text) in files {
            harness.write_source(rel, text);
        }
        let (code, _, err) = harness.run(&[
            "bump",
            "gomods:github.com/google/go-cmp",
            "v0.7.0",
            "--apply",
        ]);
        assert_eq!(code, 1, "{err}");
        assert!(
            err.contains(
                "bump is not supported for ecosystem go set gomods \
(supported operations: check, update, audit)"
            ),
            "{err}"
        );
        for (rel, text) in files {
            assert_eq!(
                std::fs::read(harness.workspace.join(rel)).expect("unchanged"),
                text.as_bytes(),
                "{rel}"
            );
        }
        assert!(harness.seen_env.borrow().is_empty());
    }

    const BAZEL_SET_DX_TOML: &str = r#"
schema_version = 1

[[dependency_set]]
name = "bazel-modules"
ecosystem = "bazel"
manifests = ["tools/MODULE.bazel"]
locks = ["tools/MODULE.bazel.lock"]
scopes = ["tools"]
"#;

    const BAZEL_SET_MODULE: &str =
        "module(name = \"tools\")\n\nbazel_dep(name = \"rules_cc\", version = \"0.2.0\")\n";

    const ROOT_MODULE: &str =
        "module(name = \"root\")\n\nbazel_dep(name = \"rules_cc\", version = \"0.1.0\")\n";

    #[test]
    fn consumer_bazel_set_widens_its_own_module_bazel() {
        let harness = Harness::new("bump-consumer-bazel");
        harness.write_source("dx.toml", BAZEL_SET_DX_TOML);
        harness.write_source("tools/MODULE.bazel", BAZEL_SET_MODULE);
        harness.write_source("MODULE.bazel", ROOT_MODULE);
        let read =
            |path: &str| std::fs::read_to_string(harness.workspace.join(path)).expect("manifest");
        let after = BAZEL_SET_MODULE.replace("0.2.0", "0.3.0");
        let (code, _, err) = harness.run(&["bump", "bazel-modules:rules_cc", "0.3.0"]);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("bump drift"), "{err}");
        assert!(err.contains("tools/MODULE.bazel"), "{err}");
        assert_eq!(read("tools/MODULE.bazel"), BAZEL_SET_MODULE);
        let (code, _, err) = harness.run(&[
            "bump",
            "bazel-modules:rules_cc",
            "0.3.0",
            "--frozen",
            "--apply",
        ]);
        assert_eq!(code, 1, "{err}");
        assert!(
            err.contains(&format!("dx: {}: ", super::CODE_FROZEN_LOCKED)),
            "{err}"
        );
        assert_eq!(read("tools/MODULE.bazel"), BAZEL_SET_MODULE);
        let (code, out, err) = harness.run(&["bump", "bazel-modules:rules_cc", "0.3.0", "--apply"]);
        assert_eq!(code, 0, "{out}{err}");
        assert!(
            out.contains("widened bazel-modules:rules_cc to 0.3.0 in tools/MODULE.bazel"),
            "{out}"
        );
        assert!(
            out.contains("refreshed bazel-modules via `bazel mod deps`")
                || out.contains("refreshed tools/MODULE.bazel.lock via `bazel mod deps`"),
            "{out}"
        );
        assert_eq!(read("tools/MODULE.bazel"), after);
        assert_eq!(read("MODULE.bazel"), ROOT_MODULE);
        {
            let runs = harness.build_argv.borrow();
            assert_eq!(runs.len(), 1, "{runs:?}");
            assert_eq!(
                runs[0][runs[0].len() - 3..],
                ["mod", "deps", "--lockfile_mode=update"]
            );
            assert!(runs[0].iter().any(|arg| arg == "--max_idle_secs=15"));
            assert!(!runs[0].iter().any(|arg| arg.contains("--output_base")));
            assert_eq!(
                *harness.seen_cwd.borrow(),
                vec![harness.workspace.join("tools")]
            );
        }
        let (code, out, err) = harness.run(&[
            "bump",
            "bazel-modules:rules_cc",
            "0.3.0",
            "--output=json",
            "--apply",
        ]);
        assert_eq!(code, 0, "{out}{err}");
        assert_eq!(read("tools/MODULE.bazel"), after, "second run is a no-op");
        assert_eq!(read("MODULE.bazel"), ROOT_MODULE);
        let events = json_events(&out);
        let notice = event(&events, "notice");
        assert_eq!(notice["code"], "bump_no_change");
        assert_eq!(notice["path"], "tools/MODULE.bazel");
        assert_eq!(events.last().expect("finished")["exit_code"], 0);
        assert_eq!(
            harness.build_argv.borrow().len(),
            1,
            "no-op run launches nothing"
        );
    }

    #[test]
    fn consumer_bazel_set_refresh_failure_keeps_the_edit() {
        let mut harness = Harness::new("bump-consumer-bazel-fail");
        harness.bazel_code = 3;
        harness.write_source("dx.toml", BAZEL_SET_DX_TOML);
        harness.write_source("tools/MODULE.bazel", BAZEL_SET_MODULE);
        let (code, _, err) = harness.run(&["bump", "bazel-modules:rules_cc", "0.3.0", "--apply"]);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("`bazel mod deps` in tools exited 3"), "{err}");
        assert!(!err.contains("dx update"), "{err}");
        let text = std::fs::read_to_string(harness.workspace.join("tools/MODULE.bazel"))
            .expect("manifest");
        assert!(text.contains("0.3.0"), "{text}");
    }

    #[test]
    fn consumer_bazel_set_offline_and_foreign_lock_fail_before_writing() {
        let harness = Harness::new("bump-consumer-bazel-offline");
        harness.write_source("dx.toml", BAZEL_SET_DX_TOML);
        harness.write_source("tools/MODULE.bazel", BAZEL_SET_MODULE);
        let manifest = || {
            std::fs::read_to_string(harness.workspace.join("tools/MODULE.bazel")).expect("manifest")
        };
        let (code, _, err) = harness.run(&[
            "bump",
            "bazel-modules:rules_cc",
            "0.3.0",
            "--offline",
            "--apply",
        ]);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("offline_required"), "{err}");
        assert_eq!(manifest(), BAZEL_SET_MODULE);
        harness.write_source(
            "dx.toml",
            &BAZEL_SET_DX_TOML.replace("tools/MODULE.bazel.lock", "other/MODULE.bazel.lock"),
        );
        let (code, _, err) = harness.run(&["bump", "bazel-modules:rules_cc", "0.3.0", "--apply"]);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("no widen performed"), "{err}");
        assert_eq!(manifest(), BAZEL_SET_MODULE);
        assert!(harness.build_argv.borrow().is_empty());
    }

    #[test]
    fn consumer_bazel_set_refuses_read_only_bare_and_bazelversion() {
        let harness = Harness::new("bump-consumer-bazel-refuse");
        harness.write_source("dx.toml", BAZEL_SET_DX_TOML);
        harness.write_source("tools/MODULE.bazel", BAZEL_SET_MODULE);
        harness.write_source("tools/.bazelversion", "9.0.0\n");
        let read = || {
            std::fs::read_to_string(harness.workspace.join("tools/MODULE.bazel")).expect("manifest")
        };
        for (selector, exit, reason) in [
            (
                "bazel-modules",
                2,
                "\"bazel-modules\" selects the set; use `bazel-modules:<package> <version>`",
            ),
            (
                "bazel-modules:.bazelversion",
                2,
                "configured bazel sets widen MODULE.bazel bazel_dep pins only",
            ),
        ] {
            let (code, _, err) = harness.run(&["bump", selector, "9.1.0", "--apply"]);
            assert_eq!(code, exit, "{selector}: {err}");
            assert!(err.contains(reason), "{selector}: {err}");
        }
        harness.write_source(
            "dx.toml",
            &BAZEL_SET_DX_TOML.replace(
                "scopes = [\"tools\"]",
                "scopes = [\"tools\"]\nwritable = false",
            ),
        );
        let (code, _, err) = harness.run(&["bump", "bazel-modules:rules_cc", "0.3.0", "--apply"]);
        assert_eq!(code, 1, "{err}");
        assert!(
            err.contains("bump cannot widen read-only set bazel-modules (writable = false)"),
            "{err}"
        );
        assert_eq!(read(), BAZEL_SET_MODULE);
        assert_eq!(
            std::fs::read_to_string(harness.workspace.join("tools/.bazelversion")).expect("pin"),
            "9.0.0\n"
        );
        assert!(harness.seen_env.borrow().is_empty());
    }

    #[test]
    fn consumer_registry_leaves_builtin_bump_selectors_alone() {
        let harness = Harness::new("bump-consumer-builtin");
        harness.write_source("dx.toml", CONSUMER_DX_TOML);
        let (code, _, err) = harness.run(&["bump", "cargo:anyhow", "1.2.3", "--apply"]);
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
                    harness.run(&["bump", "bazel:.bazelversion", "9.3.0", output, "--apply"]);
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
                "github-actions:actions/checkout",
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                ".github/workflows/ci.yml",
                "jobs:\n  b:\n    steps:\n      - uses: actions/checkout@v1\n",
                "jobs:\n  b:\n    steps:\n      - uses: actions/checkout@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\n",
            ),
            (
                "go:example.com/demo",
                "1.3.0",
                "third_party/go/go.mod",
                "module example.com/root\nrequire example.com/demo v1.2.0 // was v1.0.0\n",
                "module example.com/root\nrequire example.com/demo v1.3.0 // was v1.0.0\n",
            ),
        ] {
            let harness = Harness::new("bump-file-offline");
            seed(&harness, path, before);
            let (code, out, err) = harness.run(&[
                "bump",
                selector,
                version,
                "--offline",
                "--output=json",
                "--apply",
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
                HELLO_CARGO,
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
            seed(&harness, path, before);
            let (code, out, err) = harness.run(&[
                "bump",
                selector,
                version,
                "--offline",
                "--output=json",
                "--apply",
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
                HELLO_CARGO,
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
            seed(&harness, path, before);
            let (code, out, err) = harness.run(&[
                "bump",
                selector,
                version,
                "--frozen",
                "--output=json",
                "--apply",
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
        seed(&harness, HELLO_CARGO, "[dependencies]\ndemo = \"1\"\n");
        let (code, out, err) =
            harness.run(&["bump", "cargo:demo", "2.0.0", "--output=json", "--apply"]);
        assert_eq!(code, 1, "{out}{err}");
        assert!(err.contains("signal"));
        assert!(out.contains("bump_widened"));
        assert!(std::fs::read_to_string(harness.workspace.join(HELLO_CARGO))
            .expect("manifest")
            .contains("2.0.0"));
    }

    #[test]
    fn dry_run_rejects_bad_selector_and_version() {
        let harness = Harness::new("bump-dryrun-bad-selector");
        let (code, _, err) = harness.run(&["bump", "crates", "1.2.3"]);
        assert_eq!(code, 2, "{err}");
        let harness = Harness::new("bump-dryrun-bad-version");
        let (code, _, err) = harness.run(&["bump", "cargo:anyhow", "nope!!!"]);
        assert_eq!(code, 2, "{err}");
    }

    #[test]
    fn live_widens_one_cargo_requirement_atomically() {
        let harness = Harness::new("bump-live-cargo");
        seed(
            &harness,
            HELLO_CARGO,
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
        let widened = std::fs::read_to_string(harness.workspace.join(HELLO_CARGO)).expect("read");
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
    fn minified_npm_manifest_apply_changes_only_the_dependency() {
        let harness = Harness::new("bump-npm-minified");
        harness.write_source(
            "package.json",
            "{\"name\":\"demo\",\"dependencies\":{\"lodash\":\"4.17.20\"}}",
        );
        let (code, out, err) = harness.run(&["bump", "npm:lodash", "4.17.21", "--apply"]);
        assert_eq!(code, 0, "{out}{err}");
        assert_eq!(
            std::fs::read_to_string(harness.workspace.join("package.json")).expect("read"),
            "{\"name\":\"demo\",\"dependencies\":{\"lodash\":\"4.17.21\"}}"
        );
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
    fn go_bump_leaves_a_malformed_go_mod_unchanged() {
        let harness = Harness::new("bump-go-malformed");
        let before = "module example.com/root\nrequire (\n\texample.com/demo v1.2.0 // v1.0.0\n";
        harness.write_source("third_party/go/go.mod", before);
        let (code, out, err) = harness.run(&[
            "bump",
            "go:example.com/demo",
            "1.3.0",
            "--offline",
            "--apply",
        ]);
        assert_ne!(code, 0, "{out}{err}");
        assert_eq!(
            std::fs::read_to_string(harness.workspace.join("third_party/go/go.mod")).expect("read"),
            before
        );
    }

    #[test]
    fn live_maven_chains_full_refresh_automatically() {
        let harness = Harness::new("bump-live-maven-chain");
        harness.write_source(
            "MODULE.bazel",
            "maven = use_extension(\"@rules_jvm_external//:extensions.bzl\", \"maven\")\nmaven.install(\n    artifacts = [\n        \"junit:junit:4.13.2\",\n    ],\n)\n",
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
        seed(&harness, HELLO_CARGO, "[dependencies]\nanyhow = \"1\"\n");
        let (code, out, err) = harness.run(&["bump", "cargo:anyhow", "1.2.3", "--apply"]);
        assert_eq!(code, 1, "{out}{err}");
        assert!(err.contains("update_failed"), "{err}");
        assert!(err.contains("updater exited 1"), "{err}");
        assert!(err.contains("widen kept"), "{err}");
        assert_eq!(harness.seen_env.borrow().len(), 1, "refresh attempted once");
        let widened = std::fs::read_to_string(harness.workspace.join(HELLO_CARGO)).expect("read");
        assert!(widened.contains("anyhow = \"1.2.3\""), "{widened}");
    }

    #[test]
    fn live_refresh_launch_failure_keeps_widen() {
        let mut harness = Harness::new("bump-live-launch-fail");
        harness.io_error = true;
        seed(&harness, HELLO_CARGO, "[dependencies]\nanyhow = \"1\"\n");
        let (code, _, err) = harness.run(&["bump", "cargo:anyhow", "1.2.3", "--apply"]);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("update_failed"), "{err}");
        assert!(err.contains("failed to launch updater"), "{err}");
        assert!(err.contains("widen kept"), "{err}");
        let widened = std::fs::read_to_string(harness.workspace.join(HELLO_CARGO)).expect("read");
        assert!(widened.contains("anyhow = \"1.2.3\""), "{widened}");
    }

    #[test]
    fn live_json_chained_emits_widened_plus_update_success() {
        let harness = Harness::new("bump-live-json-chain");
        seed(&harness, HELLO_CARGO, "[dependencies]\nanyhow = \"1\"\n");
        let (code, out, err) =
            harness.run(&["bump", "cargo:anyhow", "1.2.3", "--output=json", "--apply"]);
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
        seed(&harness, HELLO_CARGO, "[dependencies]\nanyhow = \"1\"\n");
        let (code, out, err) =
            harness.run(&["bump", "cargo:anyhow", "1.2.3", "--output=json", "--apply"]);
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
    fn live_bazel_module_edits_one_literal_version_atomically() {
        let harness = Harness::new("bump-live-bazel-module");
        let before = "module(name = 'demo')\n\n# bazel_dep(name = \"rules_cc\", version = \"0.0.1\")\nNOTE = \"bazel_dep(name = 'rules_cc', version = '0.0.2')\"\nbazel_dep(\n    version = '0.2.21',  # ) \" (\n    name = 'rules_cc',\n    repo_name = \"cc\",\n)\nbazel_dep(name = \"rules_rust\", version = V)\n";
        let after = before.replace("'0.2.21'", "'0.2.22'");
        harness.write_source("MODULE.bazel", before);
        let read =
            || std::fs::read_to_string(harness.workspace.join("MODULE.bazel")).expect("read");
        let (code, _, err) = harness.run(&["bump", "bazel:rules_cc", "0.2.22"]);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("bump drift"), "{err}");
        assert_eq!(read(), before, "check must not write");
        let (code, out, err) = harness.run(&["bump", "bazel:rules_cc", "0.2.22", "--apply"]);
        assert_eq!(code, 0, "{out}{err}");
        assert!(out.contains("widened bazel:rules_cc to 0.2.22"), "{out}");
        assert_eq!(read(), after);
        let (code, out, err) = harness.run(&["bump", "bazel:rules_cc", "0.2.22", "--apply"]);
        assert_eq!(code, 0, "{out}{err}");
        assert_eq!(read(), after, "second run is a no-op");
        for (selector, reason) in [
            (
                "bazel:rules_rust",
                "bazel_dep \"rules_rust\" at line 10 has a computed version",
            ),
            ("bazel:rules_go", "no declared requirement for \"rules_go\""),
        ] {
            let (code, _, err) = harness.run(&["bump", selector, "1.0.0", "--apply"]);
            assert_eq!(code, 1, "{err}");
            assert!(err.contains(reason), "{err}");
            assert_eq!(read(), after, "{selector} must not write");
        }
        harness.write_source(
            "MODULE.bazel",
            "bazel_dep(name = 'rules_cc', version = '0.2.21'\n",
        );
        let (code, _, err) = harness.run(&["bump", "bazel:rules_cc", "0.2.22", "--apply"]);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("MODULE.bazel does not parse at line"), "{err}");
        assert_eq!(read(), "bazel_dep(name = 'rules_cc', version = '0.2.21'\n");
    }

    const SPLIT_MODULE: &str = "# floor: equals tested\nbazel_dep(name = \"rules_cc\", version = \"0.2.21\")\nsingle_version_override(\n    module_name = \"rules_cc\",\n    version = \"0.2.21\",\n)\n";

    #[test]
    fn live_bazel_bump_targets_the_tested_override_or_the_floor() {
        let harness = Harness::new("bump-live-bazel-floor");
        harness.write_source("MODULE.bazel", SPLIT_MODULE);
        let read =
            || std::fs::read_to_string(harness.workspace.join("MODULE.bazel")).expect("read");
        let (code, out, err) = harness.run(&["bump", "bazel:rules_cc", "0.2.22", "--apply"]);
        assert_eq!(code, 0, "{out}{err}");
        let tested = SPLIT_MODULE.replace("    version = \"0.2.21\",", "    version = \"0.2.22\",");
        assert_eq!(read(), tested);
        let (code, out, err) = harness.run(&[
            "bump",
            "bazel:rules_cc",
            "0.2.22",
            "--floor",
            "--reason",
            "oldest passing the floor job",
            "--apply",
        ]);
        assert_eq!(code, 0, "{out}{err}");
        assert_eq!(
            read(),
            tested
                .replace(
                    "# floor: equals tested",
                    "# floor: oldest passing the floor job"
                )
                .replace("version = \"0.2.21\")", "version = \"0.2.22\")")
        );
    }

    #[test]
    fn floor_flags_and_order_refusals_are_usage_errors_that_write_nothing() {
        let harness = Harness::new("bump-floor-refusals");
        harness.write_source("MODULE.bazel", SPLIT_MODULE);
        let read =
            || std::fs::read_to_string(harness.workspace.join("MODULE.bazel")).expect("read");
        for (args, needle) in [
            (
                vec!["bump", "bazel:rules_cc", "0.2.20", "--floor", "--apply"],
                "--floor needs --reason",
            ),
            (
                vec![
                    "bump",
                    "bazel:rules_cc",
                    "0.2.20",
                    "--reason",
                    "x",
                    "--apply",
                ],
                "--reason only applies with --floor",
            ),
            (
                vec!["bump", "cargo:anyhow", "1.2.3", "--floor", "--reason", "x"],
                "--floor applies only to a MODULE.bazel bazel_dep",
            ),
            (
                vec![
                    "bump",
                    "bazel:rules_cc",
                    "0.3.0",
                    "--floor",
                    "--reason",
                    "x",
                    "--apply",
                ],
                "above its tested version",
            ),
            (
                vec!["bump", "bazel:rules_cc", "0.2.0", "--apply"],
                "below its floor",
            ),
        ] {
            let (code, _, err) = harness.run(&args);
            assert_eq!(code, 2, "{args:?}: {err}");
            assert!(err.contains(needle), "{args:?}: {err}");
            assert_eq!(read(), SPLIT_MODULE, "{args:?} must not write");
        }
    }

    #[test]
    fn live_missing_requirement_fails_without_writing() {
        let harness = Harness::new("bump-live-missing");
        seed(&harness, HELLO_CARGO, "[dependencies]\nserde = \"1\"\n");
        let (code, _, err) = harness.run(&["bump", "cargo:anyhow", "1.2.3", "--apply"]);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("bump_failed"), "{err}");
        assert!(err.contains("no declared requirement"), "{err}");
        assert_eq!(
            std::fs::read_to_string(harness.workspace.join(HELLO_CARGO)).expect("read"),
            "[dependencies]\nserde = \"1\"\n"
        );
    }

    #[test]
    fn live_github_tag_resolution_matrix() {
        let harness = Harness::new("bump-live-gha-tag");
        harness.write_source(
            ".github/workflows/ci.yml",
            "jobs:\n  b:\n    steps:\n      - uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1 # v7\n",
        );
        let (code, _, err) =
            harness.run(&["bump", "github-actions:actions/checkout", "v5", "--apply"]);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("bump_failed"), "{err}");
        assert!(err.contains("needs SHA resolution"), "{err}");

        let sha = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        let commit_json = format!("{{\"sha\": \"{sha}\", \"commit\": {{\"message\": \"v5\"}}}}");
        let mut harness = Harness::new("bump-live-gha-resolved");
        harness.write_source(
            ".github/workflows/ci.yml",
            "jobs:\n  b:\n    steps:\n      - uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1 # v7\n      - uses: actions/cache@bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb # v6\n",
        );
        harness.write_source(
            ".github/workflows/reusable.yaml",
            "jobs:\n  b:\n    steps:\n      - uses: actions/checkout@bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb # v6\n",
        );
        harness.write_source(
            ".github/workflows/ghcr.yml",
            "jobs:\n  b:\n    steps:\n      - uses: actions/cache@bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb # v6\n",
        );
        harness.write_source("commit.json", &commit_json);
        harness.bump_commit_json =
            Some(harness.workspace.join("commit.json").display().to_string());
        let (code, out, err) = harness.run(&[
            "bump",
            "github-actions:actions/checkout",
            "v5",
            "--output=json",
            "--apply",
        ]);
        assert_eq!(code, 0, "{out}{err}");
        assert_eq!(err, "", "{err}");
        let widened = std::fs::read_to_string(harness.workspace.join(".github/workflows/ci.yml"))
            .expect("workflow");
        assert!(
            widened.contains(&format!("actions/checkout@{sha}")),
            "{widened}"
        );
        assert!(widened.contains("actions/cache@bbbb"), "{widened}");
        let widened_yaml =
            std::fs::read_to_string(harness.workspace.join(".github/workflows/reusable.yaml"))
                .expect("workflow");
        assert!(
            widened_yaml.contains(&format!("actions/checkout@{sha}")),
            "{widened_yaml}"
        );
        let untouched =
            std::fs::read_to_string(harness.workspace.join(".github/workflows/ghcr.yml"))
                .expect("workflow");
        assert!(untouched.contains("actions/cache@bbbb"), "{untouched}");
        let events = json_events(&out);
        assert!(
            events.iter().any(|event| event["code"] == "bump_widened"),
            "{out}"
        );
        assert_eq!(events.last().expect("finished")["exit_code"], 0);

        let mut harness = Harness::new("bump-live-gha-nochange");
        harness.write_source(
            ".github/workflows/ci.yml",
            &format!("jobs:\n  b:\n    steps:\n      - uses: actions/checkout@{sha} # v5\n"),
        );
        harness.write_source("commit.json", &commit_json);
        harness.bump_commit_json =
            Some(harness.workspace.join("commit.json").display().to_string());
        let (code, out, err) = harness.run(&[
            "bump",
            "github-actions:actions/checkout",
            "v5",
            "--output=json",
            "--apply",
        ]);
        assert_eq!(code, 0, "{out}{err}");
        assert_eq!(err, "", "{err}");
        assert_eq!(
            std::fs::read_to_string(harness.workspace.join(".github/workflows/ci.yml"))
                .expect("workflow"),
            format!("jobs:\n  b:\n    steps:\n      - uses: actions/checkout@{sha} # v5\n")
        );
        let events = json_events(&out);
        assert!(
            events.iter().any(|event| event["code"] == "bump_no_change"),
            "{out}"
        );
        assert_eq!(events.last().expect("finished")["exit_code"], 0);

        let (code, out, err) =
            harness.run(&["bump", "github-actions:actions/checkout", "v5", "--apply"]);
        assert_eq!(code, 0, "{out}{err}");
        assert!(out.contains("no-change"), "{out}");
        assert!(err.is_empty(), "{err}");

        for (name, body) in [
            ("missing-file", None),
            ("invalid-json", Some("not json")),
            ("upstream-error", Some("{\"message\": \"Not Found\"}")),
            ("missing-sha", Some("{\"commit\": {}}")),
            ("invalid-sha", Some("{\"sha\": \"v5\"}")),
        ] {
            let mut harness = Harness::new(&format!("bump-live-gha-fail-{name}"));
            harness.write_source(
                ".github/workflows/ci.yml",
                "jobs:\n  b:\n    steps:\n      - uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1 # v7\n",
            );
            match body {
                Some(body) => {
                    harness.write_source("commit.json", body);
                    harness.bump_commit_json =
                        Some(harness.workspace.join("commit.json").display().to_string());
                }
                None => {
                    harness.bump_commit_json =
                        Some(harness.workspace.join("absent.json").display().to_string());
                }
            }
            let (code, out, err) =
                harness.run(&["bump", "github-actions:actions/checkout", "v5", "--apply"]);
            assert_eq!(code, 1, "{name}: {out}{err}");
            assert!(err.contains("bump_failed"), "{name}: {err}");
            assert_eq!(
                std::fs::read_to_string(harness.workspace.join(".github/workflows/ci.yml"))
                    .expect("workflow"),
                "jobs:\n  b:\n    steps:\n      - uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1 # v7\n",
                "{name}: failed lookup must not widen"
            );
        }
    }

    #[test]
    fn live_github_actions_widens_every_workflow_that_pins_the_action() {
        let harness = Harness::new("bump-live-gha-every");
        let sha = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        for name in ["ci.yml", "ghcr.yml", "reusable.yaml"] {
            harness.write_source(
                &format!(".github/workflows/{name}"),
                "jobs:\n  b:\n    steps:\n      - uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1 # v7\n      - uses: actions/cache@bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb # v6\n",
            );
        }
        harness.write_source(
            ".github/workflows/bump.yml",
            "jobs:\n  b:\n    steps:\n      - uses: bazel-contrib/setup-bazel@c5acdfb288317d0b5c0bbd7a396a3dc868bb0f86 # v0.19.0\n",
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
            ("ambiguous", vec![("ci.yml", vec!["jobs:\n  b:\n    steps:\n      - uses: actions/checkout@1111111111111111111111111111111111111111\n", "      - uses: actions/checkout@2222222222222222222222222222222222222222\n"])], "ambiguous requirement"),
            ("unpinned", vec![("ci.yml", vec!["jobs:\n  b:\n    steps:\n      - uses: bazel-contrib/setup-bazel@cccccccccccccccccccccccccccccccccccccccc # v0.19.0\n"])], "no declared requirement"),
            ("malformed", vec![("a.yml", vec!["jobs:\n  b:\n    steps:\n      - uses: actions/checkout@1111111111111111111111111111111111111111\n"]), ("b.yml", vec!["jobs: [\n"])], "not valid YAML"),
            ("aliased", vec![("a.yml", vec!["jobs:\n  b:\n    steps:\n      - uses: actions/checkout@1111111111111111111111111111111111111111\n"]), ("b.yml", vec!["x: &s\n  - uses: actions/checkout@v1\njobs:\n  b:\n    steps: *s\n"])], "aliases or merge keys"),
            ("no-workflows", vec![("README.md", vec!["not a workflow\n"])], "no declared requirement"),
        ];
        for (name, workflows, expected) in cases {
            let harness = Harness::new(&format!("bump-live-gha-{name}"));
            for (file, lines) in &workflows {
                harness.write_source(&format!(".github/workflows/{file}"), &lines.concat());
            }
            let (code, _, err) =
                harness.run(&["bump", "github-actions:actions/checkout", sha, "--apply"]);
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
        let (code, _, err) =
            missing.run(&["bump", "github-actions:actions/checkout", sha, "--apply"]);
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
            "--output=json",
            "--apply",
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
    fn major_bump_carries_migrate_hint() {
        let harness = Harness::new("bump-major-hint-live");
        seed(&harness, HELLO_CARGO, "[dependencies]\nanyhow = \"1\"\n");
        let (code, out, err) = harness.run(&["bump", "cargo:anyhow", "2.0.0", "--apply"]);
        assert_eq!(code, 0, "{out}{err}");
        assert!(out.contains("widened cargo:anyhow to 2.0.0"), "{out}");
        assert!(out.contains("major bump"), "{out}");
        assert!(out.contains("migrate_failed"), "{out}");
        assert!(out.contains("missing-versions"), "{out}");
    }
    #[test]
    fn offline_live_resolver_fails_before_widen_without_mutation() {
        let harness = Harness::new("bump-offline-resolver");
        seed(&harness, HELLO_CARGO, "[dependencies]\nanyhow = \"1\"\n");
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
            std::fs::read_to_string(harness.workspace.join(HELLO_CARGO)).expect("read"),
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
    fn default_checks_without_writing_and_apply_widens() {
        let harness = Harness::new("bump-default-check");
        seed(&harness, HELLO_CARGO, "[dependencies]\nanyhow = \"1\"\n");
        let (code, out, err) = harness.run(&["bump", "cargo:anyhow", "1.2.3"]);
        assert_eq!(code, 1, "{out}{err}");
        assert!(err.contains("bump_failed"), "{err}");
        assert!(err.contains("bump drift"), "{err}");
        assert!(err.contains("--apply"), "{err}");
        assert_eq!(
            std::fs::read_to_string(harness.workspace.join(HELLO_CARGO)).expect("read"),
            "[dependencies]\nanyhow = \"1\"\n",
            "check must not widen"
        );
        assert!(
            harness.seen_env.borrow().is_empty(),
            "check launches nothing"
        );
        let (code, out, err) = harness.run(&["bump", "cargo:anyhow", "1.2.3", "--apply"]);
        assert_eq!(code, 0, "{out}{err}");
        assert!(out.contains("widened cargo:anyhow to 1.2.3"), "{out}");
        assert_eq!(
            std::fs::read_to_string(harness.workspace.join(HELLO_CARGO)).expect("read"),
            std::fs::read_to_string(harness.workspace.join(HELLO_CARGO)).expect("read"),
        );
        let (code, out, err) = harness.run(&["bump", "cargo:anyhow", "1.2.3"]);
        assert_eq!(code, 0, "{out}{err}");
        assert!(out.contains("current"), "{out}");
    }

    const GHA_SHA: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    /// One already-current declaration per bump set: selector, version, manifest, text.
    fn current_pins() -> Vec<(&'static str, &'static str, &'static str, String)> {
        vec![
            (
                "bazel:rules_cc",
                "0.2.22",
                "MODULE.bazel",
                "bazel_dep(name = \"rules_cc\", version = \"0.2.22\")\n".to_owned(),
            ),
            (
                "cargo:anyhow",
                "1.2.3",
                HELLO_CARGO,
                "[dependencies]\nanyhow = \"1.2.3\"\n".to_owned(),
            ),
            (
                "github-actions:actions/checkout",
                GHA_SHA,
                ".github/workflows/ci.yml",
                format!("jobs:\n  b:\n    steps:\n      - uses: actions/checkout@{GHA_SHA} # v5\n"),
            ),
            (
                "go:example.com/demo",
                "1.3.0",
                "third_party/go/go.mod",
                "module example.com/root\nrequire example.com/demo v1.3.0\n".to_owned(),
            ),
            (
                "maven:junit:junit",
                "4.13.2",
                "MODULE.bazel",
                "maven = use_extension(\"@rules_jvm_external//:extensions.bzl\", \"maven\")\nmaven.install(\n    artifacts = [\n        \"junit:junit:4.13.2\",\n    ],\n)\n".to_owned(),
            ),
            (
                "npm:demo",
                "1.2.3",
                "package.json",
                "{\n  \"dependencies\": {\n    \"demo\": \"1.2.3\"\n  }\n}\n".to_owned(),
            ),
            (
                "nuget:FSharp.Core",
                "10.1.201",
                "third_party/dotnet/paket.dependencies",
                "source https://api.nuget.org/v3/index.json\nnuget FSharp.Core 10.1.201\n".to_owned(),
            ),
        ]
    }

    #[test]
    fn unchanged_version_reports_no_change_for_every_set() {
        for (selector, version, path, text) in current_pins() {
            let harness = Harness::new("bump-no-change-every-set");
            seed(&harness, path, &text);
            let (code, out, err) = harness.run(&["bump", selector, version, "--apply"]);
            assert_eq!(code, 0, "{selector}: {out}{err}");
            assert!(out.contains("no-change"), "{selector}: {out}");
            assert!(err.is_empty(), "{selector}: {err}");
            let (code, out, err) =
                harness.run(&["bump", selector, version, "--output=json", "--apply"]);
            assert_eq!(code, 0, "{selector}: {out}{err}");
            let events = json_events(&out);
            assert_eq!(event(&events, "notice")["code"], "bump_no_change", "{out}");
            assert_eq!(events.last().expect("finished")["exit_code"], 0);
            assert_eq!(
                std::fs::read_to_string(harness.workspace.join(path)).expect("manifest"),
                text,
                "{selector}: no-change writes nothing"
            );
            assert!(
                harness.seen_env.borrow().is_empty(),
                "{selector}: no-change refreshes nothing"
            );
        }
    }

    #[test]
    fn unchanged_version_under_frozen_and_offline_exits_zero() {
        for flag in ["--frozen", "--offline"] {
            for (selector, version, path, text) in current_pins() {
                let harness = Harness::new("bump-no-change-gated");
                seed(&harness, path, &text);
                let (code, out, err) =
                    harness.run(&["bump", selector, version, flag, "--output=json", "--apply"]);
                assert_eq!(code, 0, "{selector} {flag}: {out}{err}");
                let events = json_events(&out);
                assert_eq!(event(&events, "notice")["code"], "bump_no_change", "{out}");
                assert_eq!(
                    std::fs::read_to_string(harness.workspace.join(path)).expect("manifest"),
                    text
                );
                assert!(harness.seen_env.borrow().is_empty(), "{selector} {flag}");
            }
        }
    }

    #[test]
    fn github_actions_writes_only_changed_workflows() {
        let harness = Harness::new("bump-gha-only-changed");
        let pinned =
            format!("jobs:\n  b:\n    steps:\n      - uses: actions/checkout@{GHA_SHA} # v5\n");
        harness.write_source(
            ".github/workflows/a.yml",
            "jobs:\n  b:\n    steps:\n      - uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1 # v4\n",
        );
        harness.write_source(".github/workflows/b.yml", &pinned);
        let (code, out, err) = harness.run(&[
            "bump",
            "github-actions:actions/checkout",
            GHA_SHA,
            "--apply",
        ]);
        assert_eq!(code, 0, "{out}{err}");
        assert!(
            out.contains(&format!(
                "widened github-actions:actions/checkout to {GHA_SHA} in .github/workflows/a.yml"
            )),
            "{out}"
        );
        assert!(!out.contains("b.yml"), "{out}");
        assert!(
            std::fs::read_to_string(harness.workspace.join(".github/workflows/a.yml"))
                .expect("workflow")
                .contains(GHA_SHA)
        );
        assert_eq!(
            std::fs::read_to_string(harness.workspace.join(".github/workflows/b.yml"))
                .expect("workflow"),
            pinned
        );
    }

    const CARGO_TREE: [(&str, &str); 3] = [
        ("cli/a/Cargo.toml", "[dependencies]\nanyhow = \"1\"\n"),
        ("cli/b/Cargo.toml", "[dependencies]\nserde = \"1\"\n"),
        (
            "rust/tests/fixtures/hello/Cargo.toml",
            "[dev-dependencies]\nanyhow = { version = \"1.0\" }\n",
        ),
    ];

    fn read(harness: &Harness, path: &str) -> String {
        std::fs::read_to_string(harness.workspace.join(path)).expect("manifest")
    }

    #[test]
    fn live_cargo_widens_every_manifest_that_declares_the_crate() {
        let harness = Harness::new("bump-cargo-every-manifest");
        seed_cargo(&harness, &CARGO_TREE);
        let (code, out, err) = harness.run(&["bump", "cargo:anyhow", "1.2.3", "--apply"]);
        assert_eq!(code, 0, "{out}{err}");
        assert!(
            out.contains(
                "widened cargo:anyhow to 1.2.3 in cli/a/Cargo.toml, rust/tests/fixtures/hello/Cargo.toml"
            ),
            "{out}"
        );
        assert_eq!(
            read(&harness, "cli/a/Cargo.toml"),
            "[dependencies]\nanyhow = \"1.2.3\"\n"
        );
        assert_eq!(
            read(&harness, HELLO_CARGO),
            "[dev-dependencies]\nanyhow = { version = \"1.2.3\" }\n"
        );
        assert_eq!(read(&harness, "cli/b/Cargo.toml"), CARGO_TREE[1].1);
        assert_eq!(harness.seen_env.borrow().len(), 1, "one refresh launch");
    }

    #[test]
    fn cargo_check_mode_reports_drift_across_manifests() {
        let harness = Harness::new("bump-cargo-check-manifests");
        seed_cargo(&harness, &CARGO_TREE);
        let (code, _, err) = harness.run(&["bump", "cargo:anyhow", "1.2.3"]);
        assert_eq!(code, 1, "{err}");
        assert!(
            err.contains(
                "needs widening to 1.2.3 in cli/a/Cargo.toml, rust/tests/fixtures/hello/Cargo.toml"
            ),
            "{err}"
        );
        for (path, text) in CARGO_TREE {
            assert_eq!(read(&harness, path), text, "check must not write {path}");
        }
        assert!(harness.seen_env.borrow().is_empty());
    }

    #[test]
    fn cargo_crate_missing_from_every_manifest_fails_without_writing() {
        let harness = Harness::new("bump-cargo-missing-everywhere");
        seed_cargo(&harness, &CARGO_TREE);
        let (code, _, err) = harness.run(&["bump", "cargo:tokio", "1.2.3", "--apply"]);
        assert_eq!(code, 1, "{err}");
        assert!(
            err.contains(
                "no declared requirement for \"tokio\" in any of the 3 manifests of the crate.from_cargo hub in MODULE.bazel"
            ),
            "{err}"
        );
        for (path, text) in CARGO_TREE {
            assert_eq!(read(&harness, path), text, "{path}");
        }
        assert!(harness.seen_env.borrow().is_empty());
    }

    #[test]
    fn cargo_without_module_bazel_fails_without_writing() {
        let harness = Harness::new("bump-cargo-no-module");
        harness.write_source(HELLO_CARGO, "[dependencies]\nanyhow = \"1\"\n");
        let (code, _, err) = harness.run(&["bump", "cargo:anyhow", "1.2.3", "--apply"]);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("cannot read MODULE.bazel"), "{err}");
        harness.write_source(
            "MODULE.bazel",
            "bazel_dep(name = \"rules_rust\", version = \"0.74.0\")\n",
        );
        let (code, _, err) = harness.run(&["bump", "cargo:anyhow", "1.2.3", "--apply"]);
        assert_eq!(code, 1, "{err}");
        assert!(
            err.contains("no crate.from_cargo hub has cargo_lockfile"),
            "{err}"
        );
        assert_eq!(
            read(&harness, HELLO_CARGO),
            "[dependencies]\nanyhow = \"1\"\n"
        );
        assert!(harness.seen_env.borrow().is_empty());
    }

    #[test]
    fn cargo_path_dependency_in_one_manifest_fails_closed_writing_nothing() {
        let harness = Harness::new("bump-cargo-path-dep");
        let tree = [
            CARGO_TREE[0],
            (
                "cli/b/Cargo.toml",
                "[dependencies]\nanyhow = { path = \"../anyhow\" }\n",
            ),
        ];
        seed_cargo(&harness, &tree);
        let (code, _, err) = harness.run(&["bump", "cargo:anyhow", "1.2.3", "--apply"]);
        assert_eq!(code, 1, "{err}");
        assert!(
            err.contains("unsupported manifest shape in cli/b/Cargo.toml"),
            "{err}"
        );
        for (path, text) in tree {
            assert_eq!(read(&harness, path), text, "{path}");
        }
        assert!(harness.seen_env.borrow().is_empty());
    }

    #[test]
    fn default_check_json_reports_drift_without_writing() {
        let harness = Harness::new("bump-default-json");
        harness.write_source(".bazelversion", "9.2.0\n");
        let (code, out, err) =
            harness.run(&["bump", "bazel:.bazelversion", "9.3.0", "--output=json"]);
        assert_eq!(code, 1, "{out}{err}");
        assert!(err.contains("bump_failed"), "{err}");
        assert_eq!(
            std::fs::read_to_string(harness.workspace.join(".bazelversion")).expect("read"),
            "9.2.0\n",
            "check must not write"
        );
        let events = json_events(&out);
        assert_eq!(event(&events, "error")["code"], super::CODE_BUMP_FAILED);
        assert_eq!(
            events.last().expect("finished")["exit_code"],
            serde_json::json!(1)
        );
        let (code, _, _) = harness.run(&[
            "bump",
            "bazel:.bazelversion",
            "9.3.0",
            "--output=json",
            "--apply",
        ]);
        assert_eq!(code, 0);
        assert_eq!(
            std::fs::read_to_string(harness.workspace.join(".bazelversion")).expect("read"),
            "9.3.0\n"
        );
    }
}
