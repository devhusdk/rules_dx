use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use dx_output::{DiagnosticEvent, Origin, RelatedEvent, Severity, Snapshot};
use dx_plan::plan::OUTPUT_GROUP;
use dx_plan::resolve::path_in_filter;
use quality_result::{build_deps, decode_validated, proto};

use crate::quality_reconcile::{Conflict, Producer, reconcile};
use dx_exec_core::common::{FileChange, apply_to_bytes, collect_own_account};

#[derive(Default)]
struct Staged {
    tools: Vec<String>,
    initial: Vec<DiagnosticEvent>,
    terminal: Vec<DiagnosticEvent>,
    changes: Vec<FileChange>,
    digests: Vec<(String, [u8; 32])>,
    originals: Vec<(String, [u8; 32])>,
    unstable: Vec<Unstable>,
    build_deps: BTreeMap<BuildDepKey, Option<Insert>>,
}

type BuildDepKey = (String, u64, u64, String, Option<String>, String);

#[derive(Debug, Clone, PartialEq, Eq)]
struct Insert {
    target: String,
    label: String,
}

#[expect(
    clippy::option_option,
    reason = "None skips the finding, Some(None) removes the entry"
)]
fn build_dep_fix(dep: &proto::BuildDep) -> Option<Option<Insert>> {
    if !dep.missing {
        return Some(None);
    }
    let label = if let Some(label) = build_deps::short_label(&dep.package, &dep.label, None) {
        label
    } else {
        build_deps::canonical_repo(&dep.label)?;
        dep.label.clone()
    };
    Some(Some(Insert {
        target: dep.target.clone(),
        label,
    }))
}

fn stage_build_dep(staged: &mut Staged, diagnostic: &proto::Diagnostic, mapped: &DiagnosticEvent) {
    let Some(dep) = &diagnostic.build_dep else {
        return;
    };
    if !diagnostic.path.is_empty() {
        return;
    }
    if let (Some(key), Some(fix)) = (build_dep_key(mapped), build_dep_fix(dep)) {
        staged.build_deps.insert(key, fix);
    }
}

fn build_dep_key(event: &DiagnosticEvent) -> Option<BuildDepKey> {
    let (start, end) = event.range?;
    Some((
        event.path.clone()?,
        start,
        end,
        event.tool.clone(),
        event.rule.clone(),
        event.message.clone(),
    ))
}

impl Staged {
    fn absorb(&mut self, other: Staged) {
        self.tools.extend(other.tools);
        self.initial.extend(other.initial);
        self.terminal.extend(other.terminal);
        self.changes.extend(other.changes);
        self.digests.extend(other.digests);
        self.originals.extend(other.originals);
        self.unstable.extend(other.unstable);
        self.build_deps.extend(other.build_deps);
    }
}

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

fn stage_artifact(bytes: &[u8], workspace: &Path) -> Option<Staged> {
    stage_result(&decode_validated(bytes).ok()?, workspace)
}

fn stage_result(result: &proto::QualityResult, workspace: &Path) -> Option<Staged> {
    let mut staged = Staged::default();
    for stage in &result.stages {
        staged.tools.push(stage.tool_id.clone());
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
        if kept_by_tag(diagnostic, workspace) {
            continue;
        }
        let mut mapped = map_diagnostic(diagnostic, Snapshot::Initial, workspace)?;
        mapped.origins.push(origin.clone());
        stage_build_dep(&mut staged, diagnostic, &mapped);
        staged.initial.push(mapped);
    }
    for diagnostic in &result.terminal_diagnostics {
        if kept_by_tag(diagnostic, workspace) {
            continue;
        }
        let mut mapped = map_diagnostic(diagnostic, Snapshot::Terminal, workspace)?;
        mapped.origins.push(origin.clone());
        stage_build_dep(&mut staged, diagnostic, &mapped);
        staged.terminal.push(mapped);
    }
    for file in &result.replacements {
        staged.changes.push(map_change(file)?);
    }
    Some(staged)
}

pub(crate) struct Collected {
    pub(crate) tools: Vec<String>,
    pub(crate) targets: usize,
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
    pub(crate) fn keep_paths(&mut self, filter: &[String]) {
        let wanted = |path: &str| path_in_filter(path, filter);
        let wanted_event = |event: &DiagnosticEvent| event.path.as_deref().is_none_or(wanted);
        self.initial.retain(wanted_event);
        self.terminal.retain(wanted_event);
        self.changes.retain(|change| wanted(&change.path));
        self.conflicts.retain(|conflict| wanted(&conflict.path));
        self.original_digests.retain(|path, _| wanted(path));
        self.terminal_digests.retain(|path, _| wanted(path));
    }

    pub(crate) fn absorb_host_result(&mut self, bytes: &[u8], label: &str, workspace: &Path) {
        let Some(mut staged) = stage_artifact(bytes, workspace) else {
            self.problems
                .push(format!("{label} reported an invalid quality result"));
            return;
        };
        for origin in staged
            .initial
            .iter_mut()
            .chain(staged.terminal.iter_mut())
            .flat_map(|diagnostic| diagnostic.origins.iter_mut())
        {
            label.clone_into(&mut origin.target);
        }
        for tool in staged.tools {
            if !self.tools.contains(&tool) {
                self.tools.push(tool);
            }
        }
        self.initial.extend(staged.initial);
        self.terminal.extend(staged.terminal);
        for (path, digest) in staged.digests {
            self.terminal_digests.entry(path).or_insert(digest);
        }
        for (path, digest) in staged.originals {
            self.original_digests.entry(path).or_insert(digest);
        }
    }

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
        let (start, end) = if dep.missing {
            build_deps::locate_deps_list(&text, &dep.target)?
        } else {
            build_deps::locate(&text, &dep.target, &dep.dependency)?
        };
        return Some((relative, (start as u64, end as u64)));
    }
    None
}

fn record_placed_build_files<'a>(
    workspace: &Path,
    events: impl Iterator<Item = &'a DiagnosticEvent>,
    originals: &mut BTreeMap<String, Option<[u8; 32]>>,
    terminal_digests: &mut BTreeMap<String, [u8; 32]>,
) {
    for event in events {
        let Some(path) = event.path.as_deref().filter(|_| event.range.is_some()) else {
            continue;
        };
        if !matches!(path.rsplit('/').next(), Some("BUILD" | "BUILD.bazel")) {
            continue;
        }
        let Ok(bytes) = std::fs::read(workspace.join(path)) else {
            continue;
        };
        let digest = dx_digest::blake3(&bytes);
        if !originals.contains_key(path) && !terminal_digests.contains_key(path) {
            originals.insert(path.to_owned(), Some(digest));
            terminal_digests.insert(path.to_owned(), digest);
        }
    }
}

fn kept_by_tag(diagnostic: &proto::Diagnostic, workspace: &Path) -> bool {
    let Some(dep) = &diagnostic.build_dep else {
        return false;
    };
    ["BUILD.bazel", "BUILD"].iter().any(|name| {
        let relative = if dep.package.is_empty() {
            (*name).to_owned()
        } else {
            format!("{}/{name}", dep.package)
        };
        std::fs::read_to_string(workspace.join(relative))
            .is_ok_and(|text| build_deps::has_keep_tag(&text, &dep.target))
    })
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
    if let (None, Some(dep)) = (&path, &diagnostic.build_dep)
        && let Some((found, at)) = place_build_dep(dep, workspace)
    {
        path = Some(found);
        range = Some(at);
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

fn build_dep_producer(
    workspace: &Path,
    keys: &BTreeMap<BuildDepKey, Option<Insert>>,
) -> (Option<Producer>, BTreeSet<BuildDepKey>) {
    let mut by_path: BTreeMap<&str, Vec<(&BuildDepKey, &Option<Insert>)>> = BTreeMap::new();
    for item @ (key, _) in keys {
        by_path.entry(&key.0).or_default().push(item);
    }
    let mut producer = Producer {
        label: "dx deps".to_owned(),
        tools: keys.keys().map(|key| key.3.clone()).collect(),
        changes: Vec::new(),
        digests: Vec::new(),
        originals: Vec::new(),
        terminal: Vec::new(),
    };
    producer.tools.dedup();
    let mut fixed = BTreeSet::new();
    for (path, items) in by_path {
        let Ok(bytes) = std::fs::read(workspace.join(path)) else {
            continue;
        };
        let Ok(text) = std::str::from_utf8(&bytes) else {
            continue;
        };
        let mut cuts: Vec<(usize, usize)> = items
            .iter()
            .filter(|(_, fix)| fix.is_none())
            .map(|(key, _)| {
                (
                    usize::try_from(key.1).unwrap_or(usize::MAX),
                    usize::try_from(key.2).unwrap_or(usize::MAX),
                )
            })
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
        let Some(removed) = apply_to_bytes(&bytes, &edits) else {
            continue;
        };
        let Ok(mut after) = String::from_utf8(removed) else {
            continue;
        };
        let has_edits = !edits.is_empty();
        for (key, fix) in &items {
            match fix {
                None if has_edits => {
                    fixed.insert((*key).clone());
                }
                None => {}
                Some(insert) => {
                    let Some((at, add)) =
                        build_deps::insertion(&after, &insert.target, &insert.label)
                    else {
                        continue;
                    };
                    after.insert_str(at, &add);
                    fixed.insert((*key).clone());
                }
            }
        }
        let Some(edit) = whole_edit(&bytes, after.as_bytes()) else {
            continue;
        };
        let edits = vec![edit];
        let original = dx_digest::blake3(&bytes);
        producer.originals.push((path.to_owned(), original));
        producer
            .digests
            .push((path.to_owned(), dx_digest::blake3(after.as_bytes())));
        producer.changes.push(FileChange {
            path: path.to_owned(),
            original_digest: original,
            edits,
        });
    }
    ((!producer.changes.is_empty()).then_some(producer), fixed)
}

fn whole_edit(before: &[u8], after: &[u8]) -> Option<(u64, u64, String)> {
    if before == after {
        return None;
    }
    let prefix = before.iter().zip(after).take_while(|(a, b)| a == b).count();
    let suffix = before
        .iter()
        .skip(prefix)
        .rev()
        .zip(after.iter().skip(prefix).rev())
        .take_while(|(a, b)| a == b)
        .count();
    let middle = String::from_utf8(after.get(prefix..after.len() - suffix)?.to_vec()).ok()?;
    Some((prefix as u64, (before.len() - suffix) as u64, middle))
}

pub(crate) type RepoMapping<'a> = &'a dyn Fn() -> Option<BTreeMap<String, String>>;

pub(crate) fn invert_repo_mapping(json: &[u8]) -> Option<BTreeMap<String, String>> {
    let map: BTreeMap<String, String> = serde_json::from_slice(json).ok()?;
    let mut inverted = BTreeMap::new();
    for (apparent, canonical) in map {
        if !apparent.is_empty() && !canonical.is_empty() {
            inverted.entry(canonical).or_insert(apparent);
        }
    }
    Some(inverted)
}

fn name_external_repos(
    keys: &mut BTreeMap<BuildDepKey, Option<Insert>>,
    repo_mapping: RepoMapping<'_>,
) {
    let external = |insert: &Insert| build_deps::canonical_repo(&insert.label).is_some();
    if !keys.values().flatten().any(external) {
        return;
    }
    let mapping = repo_mapping();
    keys.retain(|_, fix| {
        let Some(insert) = fix.as_mut().filter(|insert| external(insert)) else {
            return true;
        };
        let named = build_deps::short_label("", &insert.label, mapping.as_ref());
        named.map(|label| insert.label = label).is_some()
    });
}

pub(crate) fn collect_results(
    bep: &Path,
    foreign: &dx_bep::Foreign,
    workspace: &Path,
    repo_mapping: RepoMapping<'_>,
) -> Result<Collected, dx_error::DxError> {
    collect_results_in(bep, foreign, OUTPUT_GROUP, workspace, repo_mapping)
}

fn account_problems(account: &dx_bep::OutputAccount) -> Vec<String> {
    let mut problems = Vec::new();
    if let Some(problem) = &account.problem {
        problems.push(format!("build events cut off at {problem}"));
    }
    let missing =
        |gap: &&dx_bep::EventGap| !gap.is_skipped() && gap.aborted.as_deref() != Some("NO_BUILD");
    for gap in account.gaps.iter().filter(missing) {
        problems.push(gap.describe());
    }
    if !account.ended {
        problems.push("build events ended before Bazel's last event".to_owned());
    }
    problems
}

fn label_origins(staged: &mut Staged, target: &dx_bep::TargetOutput) {
    for origin in staged
        .initial
        .iter_mut()
        .chain(staged.terminal.iter_mut())
        .flat_map(|diagnostic| diagnostic.origins.iter_mut())
    {
        origin.target.clone_from(&target.label);
        origin.configuration.clone_from(&target.configuration);
    }
}

fn merge_originals(
    originals: &mut BTreeMap<String, Option<[u8; 32]>>,
    incoming: &[(String, [u8; 32])],
) {
    for (path, digest) in incoming {
        originals
            .entry(path.clone())
            .and_modify(|held| {
                if *held != Some(*digest) {
                    *held = None;
                }
            })
            .or_insert(Some(*digest));
    }
}

fn stage_target(
    target: &dx_bep::TargetOutput,
    workspace: &Path,
    seen: &mut BTreeSet<PathBuf>,
) -> Option<Staged> {
    let mut staged = Staged::default();
    for artifact in &target.artifacts {
        if !seen.insert(artifact.exec_path.clone()) {
            continue;
        }
        staged.absorb(stage_artifact(&artifact.bytes, workspace)?);
    }
    Some(staged)
}

fn mark_removals_fixable(
    initial: &mut [DiagnosticEvent],
    changes: &[FileChange],
    removed_paths: &BTreeSet<String>,
    fixed: &BTreeSet<BuildDepKey>,
) {
    for event in initial {
        let applied = event
            .path
            .as_ref()
            .is_some_and(|path| changes.iter().any(|change| &change.path == path));
        if applied
            && removed_paths.contains(event.path.as_deref().unwrap_or(""))
            && build_dep_key(event).is_some_and(|key| fixed.contains(&key))
        {
            event.fixable = true;
        }
    }
}

pub(crate) fn collect_results_in(
    bep: &Path,
    foreign: &dx_bep::Foreign,
    group: &str,
    workspace: &Path,
    repo_mapping: RepoMapping<'_>,
) -> Result<Collected, dx_error::DxError> {
    let account = collect_own_account(bep, group, workspace, foreign)?;
    let mut tools = BTreeSet::new();
    let mut initial = Vec::new();
    let mut producers = Vec::new();
    let mut originals: BTreeMap<String, Option<[u8; 32]>> = BTreeMap::new();
    let mut unstable = Vec::new();
    let mut build_dep_keys = BTreeMap::new();
    let mut complete = true;
    let mut seen = BTreeSet::new();
    let mut reported = BTreeSet::new();
    let mut problems = account_problems(&account);
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
        let Some(mut staged) = stage_target(target, workspace, &mut seen) else {
            problems.push(format!(
                "{} reported an invalid quality result",
                target.label
            ));
            continue;
        };
        if !target.artifacts.is_empty() {
            reported.insert(target.label.clone());
        }
        label_origins(&mut staged, target);
        tools.extend(staged.tools.iter().cloned());
        for mut item in staged.unstable {
            item.label.clone_from(&target.label);
            item.configuration.clone_from(&target.configuration);
            unstable.push(item);
        }
        merge_originals(&mut originals, &staged.originals);
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
    name_external_repos(&mut build_dep_keys, repo_mapping);
    let (removal, fixed) = build_dep_producer(workspace, &build_dep_keys);
    let removed: BTreeSet<&str> = removal
        .iter()
        .flat_map(|producer| producer.changes.iter().map(|change| change.path.as_str()))
        .collect();
    let is_fixed =
        |event: &DiagnosticEvent| build_dep_key(event).is_some_and(|key| fixed.contains(&key));
    for producer in &mut producers {
        producer.terminal.retain(|event| !is_fixed(event));
    }
    let removed_paths: BTreeSet<String> = removed.iter().map(|path| (*path).to_owned()).collect();
    if let Some(producer) = removal {
        merge_originals(&mut originals, &producer.originals);
        producers.push(producer);
    }
    let reconciled = reconcile(workspace, producers);
    mark_removals_fixable(&mut initial, &reconciled.changes, &removed_paths, &fixed);
    let mut terminal_digests = reconciled.digests;
    record_placed_build_files(
        workspace,
        initial.iter().chain(&reconciled.terminal),
        &mut originals,
        &mut terminal_digests,
    );
    Ok(Collected {
        tools: tools.into_iter().collect(),
        targets: reported.len(),
        initial,
        terminal: reconciled.terminal,
        changes: reconciled.changes,
        terminal_digests,
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
#[path = "results_tests.rs"]
mod tests;
