use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use dx_output::{DiagnosticEvent, Origin, RelatedEvent, Severity, Snapshot};
use dx_plan::plan::OUTPUT_GROUP;
use quality_result::{build_deps, decode_validated, proto};

use super::common::{apply_to_bytes, collect_output_account, FileChange};
use super::quality_reconcile::{reconcile, Conflict, Producer};

#[derive(Default)]
struct Staged {
    tools: Vec<String>,
    analyzed: Vec<(String, String)>,
    initial: Vec<DiagnosticEvent>,
    terminal: Vec<DiagnosticEvent>,
    changes: Vec<FileChange>,
    digests: Vec<(String, [u8; 32])>,
    originals: Vec<(String, [u8; 32])>,
    unstable: Vec<Unstable>,
    build_deps: BTreeSet<BuildDepKey>,
}

/// Path, start, end, tool and rule of an unused dependency placed in a BUILD file.
type BuildDepKey = (String, u64, u64, String, Option<String>);

fn build_dep_key(event: &DiagnosticEvent) -> Option<BuildDepKey> {
    let (start, end) = event.range?;
    Some((
        event.path.clone()?,
        start,
        end,
        event.tool.clone(),
        event.rule.clone(),
    ))
}

impl Staged {
    fn absorb(&mut self, other: Staged) {
        self.tools.extend(other.tools);
        self.analyzed.extend(other.analyzed);
        self.initial.extend(other.initial);
        self.terminal.extend(other.terminal);
        self.changes.extend(other.changes);
        self.digests.extend(other.digests);
        self.originals.extend(other.originals);
        self.unstable.extend(other.unstable);
        self.build_deps.extend(other.build_deps);
    }
}

/// One valid result whose pipeline stopped before it converged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Unstable {
    pub(crate) label: String,
    pub(crate) configuration: Option<String>,
    pub(crate) producer: String,
    pub(crate) convergence: &'static str,
    pub(crate) rounds: u32,
    pub(crate) tools: Vec<String>,
}

impl Unstable {
    /// Names the producer, its cause and the tools it ran.
    pub(crate) fn describe(&self) -> String {
        let mut text = self.label.clone();
        if self.producer.trim_start_matches("@@") != self.label {
            text.push_str(&format!(" ({})", self.producer));
        }
        if let Some(configuration) = &self.configuration {
            text.push_str(&format!(" in configuration {configuration}"));
        }
        text.push_str(&format!(
            " did not converge: {} after {} round(s) of {}",
            self.convergence,
            self.rounds,
            self.tools.join(", ")
        ));
        text
    }
}

/// Decodes one result artifact into the collected shape, or None when it is not one.
fn stage_artifact(bytes: &[u8], workspace: &Path) -> Option<Staged> {
    stage_result(&decode_validated(bytes).ok()?, workspace)
}

/// Maps one decoded result, or None when a field is outside what the report can carry.
fn stage_result(result: &proto::QualityResult, workspace: &Path) -> Option<Staged> {
    let mut staged = Staged::default();
    for stage in &result.stages {
        staged.tools.push(stage.tool_id.clone());
        for path in &stage.source_paths {
            staged.analyzed.push((stage.tool_id.clone(), path.clone()));
        }
    }
    if let Some(convergence) = quality_result::unstable(result.convergence) {
        staged.unstable.push(Unstable {
            label: String::new(),
            configuration: None,
            producer: result.producer.clone(),
            convergence,
            rounds: result.completed_rounds,
            tools: staged.tools.clone(),
        });
    }
    for snapshot in &result.terminal_snapshot {
        let digest: [u8; 32] = snapshot.digest.as_slice().try_into().ok()?;
        staged.digests.push((snapshot.path.clone(), digest));
    }
    for snapshot in &result.original_snapshot {
        let digest: [u8; 32] = snapshot.digest.as_slice().try_into().ok()?;
        staged.originals.push((snapshot.path.clone(), digest));
    }
    let origin = Origin {
        producer: result.producer.clone(),
        target: result.producer.clone(),
        configuration: None,
    };
    for diagnostic in &result.initial_diagnostics {
        let mut mapped = map_diagnostic(diagnostic, Snapshot::Initial, workspace)?;
        mapped.origins.push(origin.clone());
        if diagnostic.path.is_empty() && diagnostic.build_dep.is_some() {
            staged.build_deps.extend(build_dep_key(&mapped));
        }
        staged.initial.push(mapped);
    }
    for diagnostic in &result.terminal_diagnostics {
        let mut mapped = map_diagnostic(diagnostic, Snapshot::Terminal, workspace)?;
        mapped.origins.push(origin.clone());
        if diagnostic.path.is_empty() && diagnostic.build_dep.is_some() {
            staged.build_deps.extend(build_dep_key(&mapped));
        }
        staged.terminal.push(mapped);
    }
    for file in &result.replacements {
        staged.changes.push(map_change(file)?);
    }
    Some(staged)
}

pub(crate) struct Collected {
    pub(crate) tools: Vec<String>,
    pub(crate) ran: BTreeSet<(String, String)>,
    pub(crate) analyzed: BTreeSet<(String, String)>,
    pub(crate) initial: Vec<DiagnosticEvent>,
    pub(crate) terminal: Vec<DiagnosticEvent>,
    pub(crate) changes: Vec<FileChange>,
    pub(crate) terminal_digests: BTreeMap<String, [u8; 32]>,
    pub(crate) original_digests: BTreeMap<String, [u8; 32]>,
    pub(crate) conflicts: Vec<Conflict>,
    pub(crate) unstable: Vec<Unstable>,
    pub(crate) complete: bool,
    pub(crate) problems: Vec<String>,
}

impl Collected {
    /// Names the first missing or invalid result, with how many there were.
    pub(crate) fn problem_detail(&self) -> Option<String> {
        let first = self.problems.first()?;
        Some(match self.problems.len() {
            1 => first.clone(),
            count => format!("{count} missing or invalid results: {first}"),
        })
    }
}

fn map_severity(value: i32) -> Option<Severity> {
    match proto::Severity::try_from(value).ok()? {
        proto::Severity::Unspecified => None,
        proto::Severity::Info => Some(Severity::Info),
        proto::Severity::Warning => Some(Severity::Warning),
        proto::Severity::Error => Some(Severity::Error),
    }
}

/// Finds the `deps` entry of an unused dependency in the package's BUILD file.
fn place_build_dep(dep: &proto::BuildDep, workspace: &Path) -> Option<(String, (u64, u64))> {
    for name in ["BUILD.bazel", "BUILD"] {
        let relative = if dep.package.is_empty() {
            name.to_owned()
        } else {
            format!("{}/{name}", dep.package)
        };
        let Ok(text) = std::fs::read_to_string(workspace.join(&relative)) else {
            continue;
        };
        let (start, end) = build_deps::locate(&text, &dep.target, &dep.dependency)?;
        return Some((relative, (start as u64, end as u64)));
    }
    None
}

fn map_diagnostic(
    diagnostic: &proto::Diagnostic,
    snapshot: Snapshot,
    workspace: &Path,
) -> Option<DiagnosticEvent> {
    let mut range = match (diagnostic.start_byte, diagnostic.end_byte) {
        (Some(start), Some(end)) => Some((start, end)),
        (None, None) => None,
        _ => return None,
    };
    let mut path = (!diagnostic.path.is_empty()).then(|| diagnostic.path.clone());
    if let (None, Some(dep)) = (&path, &diagnostic.build_dep) {
        if let Some((found, at)) = place_build_dep(dep, workspace) {
            path = Some(found);
            range = Some(at);
        }
    }
    if path.is_none() && range.is_some() {
        return None;
    }
    Some(DiagnosticEvent {
        rule: (!diagnostic.rule_id.is_empty()).then(|| diagnostic.rule_id.clone()),
        path,
        range,
        snapshot,
        fixable: diagnostic.fixable,
        unfixable_reason: (!diagnostic.unfixable_reason.is_empty())
            .then(|| diagnostic.unfixable_reason.clone()),
        related: diagnostic.related.iter().map(map_related).collect(),
        ..DiagnosticEvent::new(
            map_severity(diagnostic.severity)?,
            diagnostic.tool_id.clone(),
            diagnostic.message.clone(),
        )
    })
}

fn map_related(item: &proto::Related) -> RelatedEvent {
    let located = !item.path.is_empty();
    RelatedEvent {
        role: item.role.clone(),
        message: item.message.clone(),
        path: located.then(|| item.path.clone()),
        range: match (located, item.start_byte, item.end_byte) {
            (true, Some(start), Some(end)) => Some((start, end)),
            _ => None,
        },
        replacement: item.replacement.clone(),
        applicability: (!item.applicability.is_empty()).then(|| item.applicability.clone()),
    }
}

fn map_change(change: &proto::FileEdits) -> Option<FileChange> {
    let original_digest: [u8; 32] = change.original_digest.as_slice().try_into().ok()?;
    let mut edits = Vec::with_capacity(change.edits.len());
    for edit in &change.edits {
        edits.push((
            edit.start_byte,
            edit.end_byte,
            String::from_utf8(edit.replacement.clone()).ok()?,
        ));
    }
    Some(FileChange {
        path: change.path.clone(),
        original_digest,
        edits,
    })
}

/// One producer that deletes every unused `deps` entry, with one change per BUILD file.
fn build_dep_producer(workspace: &Path, keys: &BTreeSet<BuildDepKey>) -> Option<Producer> {
    let mut by_path: BTreeMap<&str, Vec<(usize, usize)>> = BTreeMap::new();
    for (path, start, end, _, _) in keys {
        by_path
            .entry(path)
            .or_default()
            .push((*start as usize, *end as usize));
    }
    let mut producer = Producer {
        label: "dx unused deps".to_owned(),
        tools: keys.iter().map(|key| key.3.clone()).collect(),
        changes: Vec::new(),
        digests: Vec::new(),
        originals: Vec::new(),
        terminal: Vec::new(),
    };
    producer.tools.dedup();
    for (path, ranges) in by_path {
        let Ok(bytes) = std::fs::read(workspace.join(path)) else {
            continue;
        };
        let Ok(text) = std::str::from_utf8(&bytes) else {
            continue;
        };
        let mut cuts: Vec<(usize, usize)> = ranges
            .into_iter()
            .filter(|(start, end)| start <= end && *end <= text.len())
            .filter(|(start, end)| text.is_char_boundary(*start) && text.is_char_boundary(*end))
            .map(|(start, end)| build_deps::removal(text, start, end))
            .collect();
        cuts.sort_unstable();
        cuts.dedup();
        let mut edits: Vec<(u64, u64, String)> = Vec::new();
        let mut kept_end = 0;
        for (from, to) in cuts {
            if from < kept_end {
                continue;
            }
            kept_end = to;
            edits.push((from as u64, to as u64, String::new()));
        }
        let Some(after) = apply_to_bytes(&bytes, &edits) else {
            continue;
        };
        let original = dx_digest::blake3(&bytes);
        producer.originals.push((path.to_owned(), original));
        producer
            .digests
            .push((path.to_owned(), dx_digest::blake3(&after)));
        producer.changes.push(FileChange {
            path: path.to_owned(),
            original_digest: original,
            edits,
        });
    }
    (!producer.changes.is_empty()).then_some(producer)
}

pub(crate) fn collect_results(bep: &Path, workspace: &Path) -> Result<Collected, (String, String)> {
    collect_results_in(bep, OUTPUT_GROUP, workspace)
}

pub(crate) fn collect_results_in(
    bep: &Path,
    group: &str,
    workspace: &Path,
) -> Result<Collected, (String, String)> {
    let account = collect_output_account(bep, group, workspace)?;
    let mut tools = BTreeSet::new();
    let mut ran = BTreeSet::new();
    let mut analyzed = BTreeSet::new();
    let mut initial = Vec::new();
    let mut producers = Vec::new();
    let mut originals: BTreeMap<String, Option<[u8; 32]>> = BTreeMap::new();
    let mut unstable = Vec::new();
    let mut build_dep_keys = BTreeSet::new();
    let mut complete = true;
    let mut seen = BTreeSet::new();
    let mut problems = Vec::new();
    if let Some(problem) = &account.problem {
        problems.push(format!("build events cut off at {problem}"));
    }
    for gap in account.gaps.iter().filter(|gap| !gap.is_skipped()) {
        problems.push(gap.describe());
    }
    if !account.ended {
        problems.push("build events ended before Bazel's last event".to_owned());
    }
    for target in &account.targets {
        if target.incomplete {
            problems.push(format!(
                "{} reported an incomplete {group} output group",
                target.label
            ));
        }
        if !target.success {
            complete = false;
            continue;
        }
        let mut staged = Staged::default();
        let mut staged_ok = true;
        for artifact in &target.artifacts {
            if !seen.insert(artifact.exec_path.clone()) {
                continue;
            }
            let Some(part) = stage_artifact(&artifact.bytes, workspace) else {
                staged_ok = false;
                break;
            };
            staged.absorb(part);
        }
        if !staged_ok {
            problems.push(format!(
                "{} reported an invalid quality result",
                target.label
            ));
            continue;
        }
        for origin in staged
            .initial
            .iter_mut()
            .chain(staged.terminal.iter_mut())
            .flat_map(|diagnostic| diagnostic.origins.iter_mut())
        {
            origin.target.clone_from(&target.label);
            origin.configuration.clone_from(&target.configuration);
        }
        tools.extend(staged.tools.iter().cloned());
        ran.extend(
            staged
                .tools
                .iter()
                .map(|tool| (tool.clone(), target.label.clone())),
        );
        analyzed.extend(staged.analyzed.iter().cloned());
        for mut item in staged.unstable {
            item.label = target.label.clone();
            item.configuration = target.configuration.clone();
            unstable.push(item);
        }
        for (path, digest) in &staged.originals {
            originals
                .entry(path.clone())
                .and_modify(|held| {
                    if *held != Some(*digest) {
                        *held = None;
                    }
                })
                .or_insert(Some(*digest));
        }
        build_dep_keys.extend(staged.build_deps);
        initial.extend(staged.initial);
        producers.push(Producer {
            label: target.label.clone(),
            tools: staged.tools,
            changes: staged.changes,
            digests: staged.digests,
            originals: staged.originals.clone(),
            terminal: staged.terminal,
        });
    }
    unstable.sort_by(|a: &Unstable, b: &Unstable| a.label.cmp(&b.label));
    let removal = build_dep_producer(workspace, &build_dep_keys);
    let removed: BTreeSet<&str> = removal
        .iter()
        .flat_map(|producer| producer.changes.iter().map(|change| change.path.as_str()))
        .collect();
    let removes = |event: &DiagnosticEvent| {
        build_dep_key(event)
            .is_some_and(|key| removed.contains(key.0.as_str()) && build_dep_keys.contains(&key))
    };
    for producer in &mut producers {
        producer.terminal.retain(|event| !removes(event));
    }
    let removed_paths: BTreeSet<String> = removed.iter().map(|path| (*path).to_owned()).collect();
    if let Some(producer) = removal {
        for (path, digest) in &producer.originals {
            originals
                .entry(path.clone())
                .and_modify(|held| {
                    if *held != Some(*digest) {
                        *held = None;
                    }
                })
                .or_insert(Some(*digest));
        }
        producers.push(producer);
    }
    let reconciled = reconcile(workspace, producers);
    for event in &mut initial {
        let applied = event
            .path
            .as_ref()
            .is_some_and(|path| reconciled.changes.iter().any(|change| &change.path == path));
        if applied
            && removed_paths.contains(event.path.as_deref().unwrap_or(""))
            && build_dep_key(event).is_some_and(|key| build_dep_keys.contains(&key))
        {
            event.fixable = true;
        }
    }
    analyzed.retain(|(_, path)| reconciled.digests.contains_key(path));
    Ok(Collected {
        tools: tools.into_iter().collect(),
        ran,
        analyzed,
        initial,
        terminal: reconciled.terminal,
        changes: reconciled.changes,
        terminal_digests: reconciled.digests,
        original_digests: originals
            .into_iter()
            .filter_map(|(path, digest)| Some((path, digest?)))
            .collect(),
        conflicts: reconciled.conflicts,
        unstable,
        complete: complete && problems.is_empty(),
        problems,
    })
}

#[cfg(test)]
mod tests {
    use super::super::test_support::*;
    use super::*;
    use crate::exec::common::CODE_INVALID_BEP;
    use quality_result::{encode_validated, proto};

    #[test]
    fn severity_mapping_covers_all_arms() {
        assert!(map_severity(proto::Severity::Unspecified as i32).is_none());
        assert!(matches!(
            map_severity(proto::Severity::Info as i32),
            Some(Severity::Info)
        ));
        assert!(matches!(
            map_severity(proto::Severity::Warning as i32),
            Some(Severity::Warning)
        ));
        assert!(matches!(
            map_severity(proto::Severity::Error as i32),
            Some(Severity::Error)
        ));
        assert!(map_severity(99).is_none());
    }

    #[test]
    fn diagnostic_mapping_rejects_bad_shapes() {
        let base = Harness::diagnostic("m", false);
        let mut partial = base.clone();
        partial.end_byte = None;
        assert!(map_diagnostic(&partial, Snapshot::Initial, Path::new(".")).is_none());
        let mut pathless = base.clone();
        pathless.path = String::new();
        assert!(map_diagnostic(&pathless, Snapshot::Initial, Path::new(".")).is_none());
        assert!(map_diagnostic(&pathless, Snapshot::Terminal, Path::new(".")).is_none());
        let mut bare = pathless.clone();
        bare.start_byte = None;
        bare.end_byte = None;
        assert!(map_diagnostic(&bare, Snapshot::Terminal, Path::new(".")).is_some());
    }

    #[test]
    fn unused_dependency_is_placed_on_its_build_line() {
        let root = tempfile::tempdir().expect("workspace");
        std::fs::create_dir(root.path().join("p")).expect("package");
        let build = "rust_library(\n    name = \"lib\",\n    deps = [\"//a:a\"],\n)\n";
        std::fs::write(root.path().join("p/BUILD.bazel"), build).expect("build");
        let mut diagnostic = Harness::diagnostic("unused", false);
        diagnostic.path = String::new();
        diagnostic.start_byte = None;
        diagnostic.end_byte = None;
        diagnostic.build_dep = Some(proto::BuildDep {
            package: "p".to_owned(),
            target: "lib".to_owned(),
            dependency: "a".to_owned(),
        });
        let placed = map_diagnostic(&diagnostic, Snapshot::Terminal, root.path()).expect("mapped");
        let (start, end) = placed.range.expect("range");
        assert_eq!(placed.path.as_deref(), Some("p/BUILD.bazel"));
        assert_eq!(&build[start as usize..end as usize], "\"//a:a\"");

        std::fs::remove_file(root.path().join("p/BUILD.bazel")).expect("remove");
        let kept = map_diagnostic(&diagnostic, Snapshot::Terminal, root.path()).expect("mapped");
        assert!(kept.path.is_none() && kept.range.is_none());
    }

    #[test]
    fn staging_rejects_results_the_report_cannot_carry() {
        let harness = Harness::new("stage-reject");
        harness.write_source("src/a.py", "x = 1\n");
        let bytes = harness.valid_result(
            vec![Harness::diagnostic("unused", false)],
            vec![harness.replacement(b"y")],
        );
        let mut result = decode_validated(&bytes).expect("valid result");
        result.terminal_diagnostics = result.initial_diagnostics.clone();
        assert!(
            stage_artifact(&bytes, Path::new(".")).is_some(),
            "a valid artifact stages"
        );
        assert!(
            stage_result(&result, Path::new(".")).is_some(),
            "a valid result stages"
        );
        assert!(stage_artifact(b"not-a-validated-result", Path::new(".")).is_none());

        let mut short_snapshot = result.clone();
        short_snapshot.terminal_snapshot[0].digest.pop();
        let mut no_severity = result.clone();
        no_severity.initial_diagnostics[0].severity = proto::Severity::Unspecified as i32;
        let mut no_end = result.clone();
        no_end.terminal_diagnostics[0].end_byte = None;
        let mut short_original = result.clone();
        short_original.replacements[0].original_digest.pop();
        for broken in [short_snapshot, no_severity, no_end, short_original] {
            assert!(stage_result(&broken, Path::new(".")).is_none());
            assert!(
                encode_validated(&broken).is_err(),
                "a producer cannot send what staging rejects"
            );
        }
    }

    #[test]
    fn staging_rejects_non_utf8_replacements() {
        let harness = Harness::new("stage-bytes");
        harness.write_source("src/a.py", "x = 1\n");
        let bytes = harness.valid_result(vec![], vec![harness.replacement(b"y")]);
        let mut result = decode_validated(&bytes).expect("valid result");
        result.replacements[0].edits[0].replacement = vec![0xFF];
        assert!(
            stage_result(&result, Path::new(".")).is_none(),
            "a change whose replacement is not text never reaches the report"
        );
        assert!(
            encode_validated(&result).is_err(),
            "a producer cannot send what staging rejects"
        );
    }

    #[test]
    fn unstable_description_names_target_configuration_and_cause() {
        let mut item = Unstable {
            label: "//a:one".to_owned(),
            configuration: Some("cfg1".to_owned()),
            producer: "@@//a:one".to_owned(),
            convergence: "ITERATION_LIMIT",
            rounds: 10,
            tools: vec!["biome".to_owned(), "prettier".to_owned()],
        };
        assert_eq!(
            item.describe(),
            "//a:one in configuration cfg1 did not converge: \
             ITERATION_LIMIT after 10 round(s) of biome, prettier"
        );
        item.producer = "//b:other".to_owned();
        item.configuration = None;
        assert_eq!(
            item.describe(),
            "//a:one (//b:other) did not converge: \
             ITERATION_LIMIT after 10 round(s) of biome, prettier"
        );
    }

    #[test]
    fn invalid_output_group_rejected() {
        let dir = temp_dir("group-tmp");
        let bep = dir.path().join("empty.json");
        std::fs::write(&bep, "").expect("bep");
        let Err((code, message)) = collect_results_in(&bep, "", dir.path()) else {
            panic!("empty output group must fail"); // LCOV_EXCL_LINE - reason: defensive branch, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
        };
        assert_eq!(code, CODE_INVALID_BEP);
        assert!(message.contains("invalid BEP config"));
    }

    const ASPECT: &str = "//quality:real_aspects.bzl%real_lint_aspect";

    fn completion_id(label: &str) -> serde_json::Value {
        serde_json::json!({"targetCompleted": {
            "label": label,
            "aspect": ASPECT,
            "configuration": {"id": "cfg1"},
        }})
    }

    fn configured(labels: &[&str]) -> String {
        let children: Vec<serde_json::Value> =
            labels.iter().map(|label| completion_id(label)).collect();
        serde_json::json!({
            "id": {"targetConfigured": {"label": labels[0], "aspect": ASPECT}},
            "children": children,
        })
        .to_string()
    }

    fn produced(harness: &Harness, label: &str, set: &str) -> Vec<String> {
        let artifact = harness.temp.join(format!("{set}.pb"));
        std::fs::write(&artifact, harness.valid_result(vec![], vec![])).expect("artifact");
        vec![
            serde_json::json!({
                "id": {"namedSet": {"id": set}},
                "namedSetOfFiles": {"files": [{"uri": format!("file://{}", artifact.display())}]},
            })
            .to_string(),
            serde_json::json!({
                "id": completion_id(label),
                "completed": {
                    "success": true,
                    "outputGroup": [{"name": "dx_results", "fileSets": [{"id": set}]}],
                },
            })
            .to_string(),
        ]
    }

    fn collect(harness: &Harness, lines: &[String]) -> Collected {
        let bep = harness.temp.join("events.json");
        std::fs::write(&bep, lines.join("\n")).expect("bep");
        collect_results_in(&bep, "dx_results", &harness.workspace).expect("collected")
    }

    #[test]
    fn two_producers_both_reported_are_complete() {
        let harness = Harness::new("producers-both");
        harness.write_source("src/a.py", "x = 1\n");
        let mut lines = vec![configured(&["//a:one", "//a:two"])];
        lines.extend(produced(&harness, "//a:one", "0"));
        lines.extend(produced(&harness, "//a:two", "1"));
        lines.push(LAST_EVENT.to_owned());
        let collected = collect(&harness, &lines);
        assert!(collected.complete, "{:?}", collected.problems);
        assert!(collected.problem_detail().is_none());
    }

    #[test]
    fn an_announced_producer_that_never_reports_is_incomplete() {
        let harness = Harness::new("producers-one-missing");
        harness.write_source("src/a.py", "x = 1\n");
        let mut lines = vec![configured(&["//a:one", "//a:two"])];
        lines.extend(produced(&harness, "//a:one", "0"));
        lines.push(LAST_EVENT.to_owned());
        let collected = collect(&harness, &lines);
        assert!(!collected.complete);
        assert_eq!(
            collected.problem_detail().as_deref(),
            Some(
                "targetCompleted for //a:two (//quality:real_aspects.bzl%real_lint_aspect) \
                 was announced but never reported"
            )
        );
    }

    #[test]
    fn a_stream_without_bazels_last_event_is_incomplete() {
        let harness = Harness::new("producers-unterminated");
        harness.write_source("src/a.py", "x = 1\n");
        let lines = produced(&harness, "//a:one", "0");
        let collected = collect(&harness, &lines);
        assert!(!collected.complete);
        assert_eq!(
            collected.problems,
            vec!["build events ended before Bazel's last event".to_owned()]
        );
        let empty = collect(&harness, &[]);
        assert!(!empty.complete, "an empty stream proves nothing");
    }

    #[test]
    fn no_applicable_work_is_complete_only_once_every_target_reported() {
        let harness = Harness::new("producers-no-work");
        let done = serde_json::json!({
            "id": {"targetCompleted": {"label": "//a:docs", "configuration": {"id": "cfg1"}}},
            "completed": {"success": true},
        })
        .to_string();
        let announce = serde_json::json!({
            "id": {"targetConfigured": {"label": "//a:docs"}},
            "children": [{"targetCompleted": {"label": "//a:docs", "configuration": {"id": "cfg1"}}}],
        })
        .to_string();
        let clean = collect(
            &harness,
            &[announce.clone(), done.clone(), LAST_EVENT.to_owned()],
        );
        assert!(clean.complete, "{:?}", clean.problems);
        assert!(clean.tools.is_empty());
        let missing = collect(&harness, &[announce, LAST_EVENT.to_owned()]);
        assert!(!missing.complete);
    }

    #[test]
    fn skipped_targets_are_not_missing_results() {
        let harness = Harness::new("producers-skipped");
        harness.write_source("src/a.py", "x = 1\n");
        let mut lines = vec![configured(&["//a:one", "//a:windows_only"])];
        lines.extend(produced(&harness, "//a:one", "0"));
        lines.push(
            serde_json::json!({"id": completion_id("//a:windows_only"), "aborted": {"reason": "SKIPPED"}})
                .to_string(),
        );
        lines.push(LAST_EVENT.to_owned());
        let collected = collect(&harness, &lines);
        assert!(collected.complete, "{:?}", collected.problems);
    }

    #[test]
    fn aborted_and_incomplete_producers_name_their_cause() {
        let harness = Harness::new("producers-aborted");
        harness.write_source("src/a.py", "x = 1\n");
        let mut lines = vec![configured(&["//a:one", "//a:two"])];
        lines.extend(produced(&harness, "//a:one", "0"));
        lines.push(
            serde_json::json!({"id": completion_id("//a:two"), "aborted": {"reason": "USER_INTERRUPTED"}})
                .to_string(),
        );
        lines.push(LAST_EVENT.to_owned());
        let collected = collect(&harness, &lines);
        assert!(!collected.complete);
        assert!(
            collected.problems[0].ends_with("was aborted: USER_INTERRUPTED"),
            "{:?}",
            collected.problems
        );

        let partial = serde_json::json!({
            "id": completion_id("//a:one"),
            "completed": {
                "success": true,
                "outputGroup": [{"name": "dx_results", "incomplete": true}],
            },
        })
        .to_string();
        let collected = collect(&harness, &[partial, LAST_EVENT.to_owned()]);
        assert!(!collected.complete);
        assert_eq!(
            collected.problems,
            vec!["//a:one reported an incomplete dx_results output group".to_owned()]
        );
    }

    #[test]
    fn an_invalid_result_names_its_producer() {
        let harness = Harness::new("producers-invalid");
        let artifact = harness.temp.join("bad.pb");
        std::fs::write(&artifact, b"not a result").expect("artifact");
        let lines = vec![
            serde_json::json!({
                "id": {"namedSet": {"id": "0"}},
                "namedSetOfFiles": {"files": [{"uri": format!("file://{}", artifact.display())}]},
            })
            .to_string(),
            serde_json::json!({
                "id": completion_id("//a:one"),
                "completed": {
                    "success": true,
                    "outputGroup": [{"name": "dx_results", "fileSets": [{"id": "0"}]}],
                },
            })
            .to_string(),
            LAST_EVENT.to_owned(),
        ];
        let collected = collect(&harness, &lines);
        assert!(!collected.complete);
        assert_eq!(
            collected.problems,
            vec!["//a:one reported an invalid quality result".to_owned()]
        );
    }

    #[test]
    fn a_cut_off_last_line_is_incomplete_not_invalid() {
        let harness = Harness::new("producers-cut-off");
        harness.write_source("src/a.py", "x = 1\n");
        let mut lines = produced(&harness, "//a:one", "0");
        lines.push(r#"{"id":{"targetCo"#.to_owned());
        let collected = collect(&harness, &lines);
        assert!(!collected.complete);
        assert!(
            collected.problems[0].starts_with("build events cut off at line 3: invalid JSON"),
            "{:?}",
            collected.problems
        );
        assert!(
            collected
                .problems
                .contains(&"build events ended before Bazel's last event".to_owned()),
            "{:?}",
            collected.problems
        );
        assert!(!collected.tools.is_empty(), "{:?}", collected.tools);
    }

    #[test]
    fn an_artifact_reported_by_two_targets_is_read_once() {
        let harness = Harness::new("producers-shared-artifact");
        harness.write_source("src/a.py", "x = 1\n");
        let artifact = harness.temp.join("shared.pb");
        let bytes = harness.valid_result(vec![Harness::diagnostic("unused", false)], vec![]);
        std::fs::write(&artifact, bytes).expect("artifact");
        let report = |label: &str| {
            serde_json::json!({
                "id": completion_id(label),
                "completed": {
                    "success": true,
                    "outputGroup": [{"name": "dx_results", "fileSets": [{"id": "0"}]}],
                },
            })
            .to_string()
        };
        let lines = vec![
            serde_json::json!({
                "id": {"namedSet": {"id": "0"}},
                "namedSetOfFiles": {"files": [{"uri": format!("file://{}", artifact.display())}]},
            })
            .to_string(),
            report("//a:x"),
            report("//a:x_test"),
            LAST_EVENT.to_owned(),
        ];
        let once = collect(&harness, &lines[..3]);
        let twice = collect(&harness, &lines);
        assert_eq!(once.initial.len(), twice.initial.len());
        assert!(!twice.initial.is_empty());
        assert!(twice.ran.iter().all(|(_, label)| label == "//a:x"));
    }

    #[test]
    fn a_bad_line_mid_stream_is_reported_not_fatal() {
        let harness = Harness::new("producers-mid-stream");
        let mut lines = vec!["{\"id\":{\"targetCo".to_owned()];
        lines.extend(produced(&harness, "//a:one", "0"));
        lines.push(LAST_EVENT.to_owned());
        let bep = harness.temp.join("events.json");
        std::fs::write(&bep, lines.join("\n")).expect("bep");
        let collected = collect_results_in(&bep, "dx_results", &harness.workspace)
            .expect("a garbled line must not fail the run");
        assert!(
            collected
                .problems
                .iter()
                .any(|p| p.contains("line 1: invalid JSON")),
            "{:?}",
            collected.problems
        );
        assert!(!collected.tools.is_empty(), "{:?}", collected.tools);
    }

    const TWO_DEPS: &str = "rust_library(\n    name = \"one\",\n    deps = [\"//a:a\"],\n)\n\nrust_library(\n    name = \"two\",\n    deps = [\"//b:b\"],\n)\n";

    fn unused(target: &str, dependency: &str) -> proto::Diagnostic {
        let mut diagnostic = Harness::diagnostic("unused", false);
        diagnostic.path = String::new();
        diagnostic.start_byte = None;
        diagnostic.end_byte = None;
        diagnostic.build_dep = Some(proto::BuildDep {
            package: "p".to_owned(),
            target: target.to_owned(),
            dependency: dependency.to_owned(),
        });
        diagnostic
    }

    fn collect_artifacts(harness: &Harness, artifacts: &[(&str, Vec<u8>)]) -> Collected {
        let mut lines = vec![configured(
            &artifacts
                .iter()
                .map(|(label, _)| *label)
                .collect::<Vec<_>>(),
        )];
        for (index, (label, bytes)) in artifacts.iter().enumerate() {
            let set = index.to_string();
            let artifact = harness.temp.join(format!("{set}.pb"));
            std::fs::write(&artifact, bytes).expect("artifact");
            lines.push(
                serde_json::json!({
                    "id": {"namedSet": {"id": set}},
                    "namedSetOfFiles": {"files": [{"uri": format!("file://{}", artifact.display())}]},
                })
                .to_string(),
            );
            lines.push(
                serde_json::json!({
                    "id": completion_id(label),
                    "completed": {
                        "success": true,
                        "outputGroup": [{"name": "dx_results", "fileSets": [{"id": set}]}],
                    },
                })
                .to_string(),
            );
        }
        lines.push(LAST_EVENT.to_owned());
        collect(harness, &lines)
    }

    fn unused_deps_artifact(harness: &Harness, deps: &[(&str, &str)]) -> Vec<u8> {
        let found: Vec<proto::Diagnostic> = deps.iter().map(|(t, d)| unused(t, d)).collect();
        harness.result_full(found.clone(), found, vec![], vec![])
    }

    #[test]
    fn unused_deps_in_one_build_file_become_one_change() {
        let harness = Harness::new("unused-deps-one");
        harness.write_source("p/BUILD.bazel", TWO_DEPS);
        let bytes = unused_deps_artifact(&harness, &[("one", "a"), ("two", "b"), ("one", "a")]);
        let collected = collect_artifacts(&harness, &[("//p:one", bytes)]);
        assert!(collected.conflicts.is_empty());
        assert_eq!(collected.changes.len(), 1);
        let change = &collected.changes[0];
        assert_eq!(change.path, "p/BUILD.bazel");
        assert_eq!(change.edits.len(), 2);
        let after = apply_to_bytes(TWO_DEPS.as_bytes(), &change.edits).expect("applied");
        let after = String::from_utf8(after).expect("utf8");
        assert!(
            !after.contains("//a:a") && !after.contains("//b:b"),
            "{after}"
        );
        assert!(collected
            .terminal
            .iter()
            .all(|e| e.path.as_deref() != Some("p/BUILD.bazel")));
        assert!(collected.initial.iter().all(|e| e.fixable));
    }

    #[test]
    fn unused_dep_edit_conflicts_with_another_editor() {
        let harness = Harness::new("unused-deps-conflict");
        harness.write_source("p/BUILD.bazel", TWO_DEPS);
        let digest = dx_digest::blake3(TWO_DEPS.as_bytes()).to_vec();
        let snapshot = proto::FileSnapshot {
            path: "p/BUILD.bazel".to_owned(),
            digest,
        };
        let editor = harness.result_full(
            vec![],
            vec![],
            vec![harness.replacement_in("p/BUILD.bazel", b"X")],
            vec![snapshot],
        );
        let unused = unused_deps_artifact(&harness, &[("one", "a")]);
        let collected = collect_artifacts(&harness, &[("//p:fmt", editor), ("//p:one", unused)]);
        assert!(collected.changes.is_empty());
        assert_eq!(collected.conflicts.len(), 1);
        assert_eq!(collected.conflicts[0].path, "p/BUILD.bazel");
    }

    #[test]
    fn rewritten_build_file_makes_the_unused_dep_change_stale() {
        let harness = Harness::new("unused-deps-stale");
        harness.write_source("p/BUILD.bazel", TWO_DEPS);
        let bytes = unused_deps_artifact(&harness, &[("one", "a")]);
        let collected = collect_artifacts(&harness, &[("//p:one", bytes)]);
        harness.write_source("p/BUILD.bazel", "# changed\n");
        let outcome = crate::exec::quality_apply::apply_collected_changes(
            &dx_apply::RealFileSystem,
            &harness.workspace,
            true,
            true,
            &collected.changes,
        );
        assert_eq!(outcome.applied.get("p/BUILD.bazel"), Some(&false));
        assert_eq!(
            outcome.not_applied,
            vec![("p/BUILD.bazel".to_owned(), "stale_source")]
        );
    }
}
