#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

use std::collections::{BTreeMap, BTreeSet, HashSet};

use dx_digest::{blake3 as digest, DIGEST_LEN};
use quality_result::{
    proto::{
        Capability, Convergence, Diagnostic, Edit, FileEdits, FileSnapshot, QualityResult, Stage,
    },
    MAX_COMPLETED_ROUNDS, SCHEMA_MAJOR, SCHEMA_MINOR,
};

pub mod real;
pub mod request;

pub fn max_rounds_for_capability(capability: &str) -> u32 {
    if capability == "audit" || capability == "typecheck" {
        1
    } else {
        MAX_COMPLETED_ROUNDS
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StageSpec {
    pub tool_id: String,
    pub class_ids: Vec<String>,
    pub source_paths: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileInput {
    pub path: String,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RunnerError {
    #[error("empty producer: want a non-empty producer")]
    EmptyProducer,
    #[error("unknown capability {capability:?}: want lint, typecheck, format, or audit")]
    UnknownCapability { capability: String },
    #[error("empty stages: want at least one stage")]
    EmptyStages,
    #[error("empty tool id at stage {stage}")]
    EmptyToolId { stage: usize },
    #[error("unknown tool {tool_id:?}")]
    UnknownTool { tool_id: String },
    #[error("empty class ids at stage {stage}")]
    EmptyClassIds { stage: usize },
    #[error("empty stage sources at stage {stage}")]
    EmptyStageSources { stage: usize },
    #[error("duplicate file {path:?}")]
    DuplicateFile { path: String },
    #[error("invalid UTF-8 for {path:?}")]
    InvalidUtf8 { path: String },
    #[error("missing file {path:?}")]
    MissingFile { path: String },
    #[error("tool execution failed for {tool_id}: {detail}")]
    ToolExecution { tool_id: String, detail: String },
    #[error("invalid tool output for {tool_id}: {detail}")]
    ToolOutput { tool_id: String, detail: String },
    #[error("unplaceable finding for {tool_id}: {detail}")]
    UnplaceableFinding { tool_id: String, detail: String },
}

fn parse_capability(capability: &str) -> Result<i32, RunnerError> {
    match capability {
        "lint" => Ok(Capability::Lint as i32),
        "typecheck" => Ok(Capability::Typecheck as i32),
        "format" => Ok(Capability::Format as i32),
        "audit" => Ok(Capability::Audit as i32),
        _ => Err(RunnerError::UnknownCapability {
            capability: capability.to_owned(),
        }),
    }
}

fn state_digest(files: &BTreeMap<String, String>) -> [u8; 32] {
    let mut canonical = Vec::with_capacity(files.len() * 2 * DIGEST_LEN);
    for (path, body) in files {
        canonical.extend_from_slice(&digest(path.as_bytes()));
        canonical.extend_from_slice(&digest(body.as_bytes()));
    }
    digest(&canonical)
}

/// File texts keyed by workspace path.
pub(crate) type Texts = BTreeMap<String, String>;

/// Selects every stage source for the first convergence round.
pub(crate) fn full_selection(stages: &[StageSpec]) -> Vec<BTreeSet<String>> {
    stages
        .iter()
        .map(|stage| stage.source_paths.iter().cloned().collect())
        .collect()
}

fn round_targets(
    stage: &StageSpec,
    selected: Option<&BTreeSet<String>>,
    touched: &BTreeSet<String>,
) -> Vec<String> {
    let mut targets: Vec<String> = Vec::new();
    for path in &stage.source_paths {
        let wanted = selected.is_some_and(|set| set.contains(path)) || touched.contains(path);
        if wanted && !targets.contains(path) {
            targets.push(path.clone());
        }
    }
    targets
}

fn run_convergence(
    initial: &BTreeMap<String, String>,
    stages: &[StageSpec],
    max_rounds: u32,
    round1: &[BTreeSet<String>],
    mut apply: impl FnMut(usize, &StageSpec, &Texts, &[String]) -> Result<Texts, RunnerError>,
) -> Result<(BTreeMap<String, String>, u32, Convergence), RunnerError> {
    let mut current = initial.clone();
    let mut seen = HashSet::with_capacity(max_rounds.min(1024) as usize + 1);
    let mut prev_id = state_digest(initial);
    seen.insert(prev_id);
    let mut completed_rounds = 0;
    let mut dirty: BTreeSet<String> = BTreeSet::new();
    for round in 1..=max_rounds {
        completed_rounds = round;
        let mut touched: BTreeSet<String> = BTreeSet::new();
        for (index, stage) in stages.iter().enumerate() {
            let selected = if round == 1 {
                round1.get(index)
            } else {
                Some(&dirty)
            };
            let targets = round_targets(stage, selected, &touched);
            if targets.is_empty() {
                continue;
            }
            for path in &targets {
                if !current.contains_key(path) {
                    return Err(RunnerError::MissingFile { path: path.clone() });
                }
            }
            let mut fixed = apply(index, stage, &current, &targets)?;
            if let Some(extra) = fixed.keys().find(|path| !targets.contains(path)) {
                return Err(RunnerError::ToolOutput {
                    tool_id: stage.tool_id.clone(),
                    detail: format!("fixer returned unrequested file {extra:?}"),
                });
            }
            for path in &targets {
                let next = fixed
                    .remove(path)
                    .ok_or(RunnerError::MissingFile { path: path.clone() })?;
                if current.get(path) != Some(&next) {
                    touched.insert(path.clone());
                }
                current.insert(path.clone(), next);
            }
        }
        if touched.is_empty() {
            return Ok((current, completed_rounds, Convergence::Stable));
        }
        let id = state_digest(&current);
        if id == prev_id {
            return Ok((current, completed_rounds, Convergence::Oscillation));
        }
        if !seen.insert(id) {
            return Ok((current, completed_rounds, Convergence::Oscillation));
        }
        prev_id = id;
        dirty = touched;
    }
    Ok((current, completed_rounds, Convergence::IterationLimit))
}

fn snapshot(files: &BTreeMap<String, String>) -> Vec<FileSnapshot> {
    files
        .iter()
        .map(|(path, body)| FileSnapshot {
            path: path.clone(),
            digest: digest(body.as_bytes()).to_vec(),
        })
        .collect()
}

fn minimal_edit(original: &str, terminal: &str) -> Edit {
    let orig = original.as_bytes();
    let term = terminal.as_bytes();
    let mut prefix = 0;
    while prefix < orig.len() && prefix < term.len() && orig[prefix] == term[prefix] {
        prefix += 1;
    }
    while prefix > 0 && !original.is_char_boundary(prefix) {
        prefix -= 1;
    }
    let mut suffix = 0;
    while suffix < orig.len() - prefix
        && suffix < term.len() - prefix
        && orig[orig.len() - 1 - suffix] == term[term.len() - 1 - suffix]
    {
        suffix += 1;
    }
    while suffix > 0
        && (!original.is_char_boundary(orig.len() - suffix)
            || !terminal.is_char_boundary(term.len() - suffix))
    {
        suffix -= 1;
    }
    Edit {
        start_byte: prefix as u64,
        end_byte: (orig.len() - suffix) as u64,
        replacement: term[prefix..term.len() - suffix].to_vec(),
    }
}

fn sort_diagnostics(diagnostics: &mut [Diagnostic]) {
    diagnostics.sort_by(|a, b| {
        (
            &a.path,
            a.start_byte,
            a.end_byte,
            a.severity,
            &a.tool_id,
            &a.rule_id,
            &a.message,
        )
            .cmp(&(
                &b.path,
                b.start_byte,
                b.end_byte,
                b.severity,
                &b.tool_id,
                &b.rule_id,
                &b.message,
            ))
    });
}

fn validate_request(
    producer: &str,
    capability: &str,
    stages: &[StageSpec],
    files: &[FileInput],
    tool_known: impl Fn(&str) -> bool,
) -> Result<(i32, BTreeMap<String, String>), RunnerError> {
    if producer.is_empty() {
        return Err(RunnerError::EmptyProducer);
    }
    let capability_value = parse_capability(capability)?;
    if stages.is_empty() {
        return Err(RunnerError::EmptyStages);
    }
    for (index, stage) in stages.iter().enumerate() {
        if stage.tool_id.is_empty() {
            return Err(RunnerError::EmptyToolId { stage: index });
        }
        if !tool_known(&stage.tool_id) {
            return Err(RunnerError::UnknownTool {
                tool_id: stage.tool_id.clone(),
            });
        }
        if stage.class_ids.is_empty() {
            return Err(RunnerError::EmptyClassIds { stage: index });
        }
        if stage.source_paths.is_empty() {
            return Err(RunnerError::EmptyStageSources { stage: index });
        }
    }
    let mut initial: BTreeMap<String, String> = BTreeMap::new();
    for file in files {
        if initial.contains_key(&file.path) {
            return Err(RunnerError::DuplicateFile {
                path: file.path.clone(),
            });
        }
        let text = std::str::from_utf8(&file.bytes).map_err(|_| RunnerError::InvalidUtf8 {
            path: file.path.clone(),
        })?;
        initial.insert(file.path.clone(), text.to_owned());
    }
    for stage in stages {
        for path in &stage.source_paths {
            if !initial.contains_key(path) {
                return Err(RunnerError::MissingFile { path: path.clone() });
            }
        }
    }
    Ok((capability_value, initial))
}

fn assemble(
    producer: &str,
    capability_value: i32,
    stages: &[StageSpec],
    initial: &BTreeMap<String, String>,
    terminal: &BTreeMap<String, String>,
    diagnostics: (Vec<Diagnostic>, Vec<Diagnostic>),
    outcome: (u32, Convergence),
) -> Result<QualityResult, RunnerError> {
    let (mut initial_diagnostics, mut terminal_diagnostics) = diagnostics;
    let (completed_rounds, convergence) = outcome;
    sort_diagnostics(&mut initial_diagnostics);
    sort_diagnostics(&mut terminal_diagnostics);
    let stable = convergence == Convergence::Stable;
    let mut replacements = Vec::new();
    for (path, original) in initial {
        let terminal_body = terminal
            .get(path)
            .ok_or(RunnerError::MissingFile { path: path.clone() })?;
        if stable && terminal_body != original {
            replacements.push(FileEdits {
                path: path.clone(),
                original_digest: digest(original.as_bytes()).to_vec(),
                edits: vec![minimal_edit(original, terminal_body)],
            });
        }
    }
    for diagnostic in &mut initial_diagnostics {
        let resolved = replacements
            .iter()
            .any(|edits| edits.path == diagnostic.path);
        let persists = terminal_diagnostics.iter().any(|terminal_diagnostic| {
            terminal_diagnostic.tool_id == diagnostic.tool_id
                && terminal_diagnostic.message == diagnostic.message
                && terminal_diagnostic.path == diagnostic.path
        });
        diagnostic.fixable = stable && resolved && !persists;
    }
    let stages_proto = stages
        .iter()
        .map(|stage| Stage {
            tool_id: stage.tool_id.clone(),
            class_ids: stage.class_ids.clone(),
            source_paths: stage.source_paths.clone(),
        })
        .collect();
    Ok(QualityResult {
        schema_major: SCHEMA_MAJOR,
        schema_minor: SCHEMA_MINOR,
        producer: producer.to_owned(),
        capability: capability_value,
        stages: stages_proto,
        completed_rounds,
        convergence: convergence as i32,
        original_snapshot: snapshot(initial),
        terminal_snapshot: snapshot(terminal),
        initial_diagnostics,
        terminal_diagnostics,
        replacements,
    })
}

fn stage_subset(
    stage: &StageSpec,
    files: &BTreeMap<String, String>,
) -> Result<BTreeMap<String, String>, RunnerError> {
    let mut out = BTreeMap::new();
    for path in &stage.source_paths {
        let body = files
            .get(path)
            .ok_or(RunnerError::MissingFile { path: path.clone() })?;
        out.insert(path.clone(), body.clone());
    }
    Ok(out)
}

#[cfg(test)]
#[path = "lib_determinism.rs"]
mod lib_determinism;
#[cfg(test)]
#[path = "lib_gates.rs"]
mod lib_gates;
#[cfg(test)]
#[path = "lib_pipeline.rs"]
mod lib_pipeline;
#[cfg(test)]
#[path = "lib_synthetic.rs"]
pub(crate) mod synthetic;
