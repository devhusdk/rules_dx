//! Native harness that runs one quality matrix case and checks its snapshot.
#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

use std::collections::BTreeSet;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::Duration;

use dx_diff::{render_patch, FilePatch, PatchKind};
use dx_path::runfiles::Resolver;
use dx_process::lifecycle::{self, CapturePolicy, ChildOutcome, EnvPolicy, SpawnSpec};
use quality_result::{decode_validated, proto};
use serde::Deserialize;
use serde_json::json;

pub const MANIFEST_ENV: &str = "DX_MATRIX_MANIFEST";
pub const MANIFEST_SCHEMA_VERSION: u32 = 1;
const REQUEST_SCHEMA_VERSION: u32 = 1;
const CHILD_TIMEOUT: Duration = Duration::from_secs(120);
const CAPTURE_BYTES: usize = 8 * 1024 * 1024;
const WORK_DIR: &str = "matrix_work";
const RUNFILES_DIR_ENV: &str = "RUNFILES_DIR";
const UPDATE_EXPECT_ENV: &str = "UPDATE_EXPECT";

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum HarnessError {
    #[error("{MANIFEST_ENV} is not set: this binary runs one declared matrix case")]
    NoManifest,
    #[error("cannot read {MANIFEST_ENV}={key:?}: {detail}")]
    UnreadableManifest { key: String, detail: String },
    #[error("malformed manifest: {detail}")]
    MalformedManifest { detail: String },
    #[error("unsupported manifest schema {found}: want {MANIFEST_SCHEMA_VERSION}")]
    UnsupportedSchema { found: u32 },
    #[error("cannot resolve {what} runfile {key:?} in {source}: {detail}")]
    UnresolvedRunfile {
        what: &'static str,
        key: String,
        source: String,
        detail: String,
    },
    #[error("{what} runfile {key:?} is not a file: {path}")]
    MissingRunfile {
        what: &'static str,
        key: String,
        path: PathBuf,
    },
    #[error("unknown {what} value {value:?}: {reason}")]
    UnknownValue {
        what: &'static str,
        value: String,
        reason: &'static str,
    },
    #[error("malformed stage {spec:?}: want TOOL;classes;paths")]
    BadStage { spec: String },
    #[error("stage {stage} of {label} names undeclared source {path:?}")]
    UndeclaredSource {
        label: String,
        stage: usize,
        path: String,
    },
    #[error("TEST_TMPDIR is not set")]
    NoTestTmpdir,
    #[error("cannot write {path}: {detail}")]
    Unwritable { path: String, detail: String },
    #[error("cannot read {path}: {detail}")]
    Unreadable { path: String, detail: String },
    #[error("{program} failed with {exit}: {stderr}")]
    ChildFailed {
        program: String,
        exit: String,
        stderr: String,
    },
    #[error("{program} exceeded the {seconds}s deadline")]
    ChildTimedOut { program: String, seconds: u64 },
    #[error("{program} wrote more than {limit} bytes")]
    ChildTooLarge { program: String, limit: usize },
    #[error("invalid result protobuf {path}: {detail}")]
    InvalidResult { path: String, detail: String },
    #[error("result does not match the declared case: {detail}")]
    ResultMismatch { detail: String },
    #[error("printed evidence is invalid: {detail}")]
    PrintedInvalid { detail: String },
    #[error("golden snapshot mismatch for {label}\n{patch}")]
    SnapshotMismatch { label: String, patch: String },
    #[error("cannot stage the refreshed golden at {path}: {detail}")]
    Unstageable { path: String, detail: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
struct InputDoc {
    workspace: String,
    key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
struct ToolFileDoc {
    rel: String,
    key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
struct EnvDoc {
    key: String,
    #[serde(default)]
    value: Option<String>,
    #[serde(default)]
    value_from: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
struct ToolDoc {
    tool: String,
    #[serde(default)]
    binary: Option<String>,
    #[serde(default)]
    config: Option<String>,
    #[serde(default)]
    edition: Option<String>,
    #[serde(default)]
    files: Vec<ToolFileDoc>,
    #[serde(default)]
    env: Vec<EnvDoc>,
    #[serde(default)]
    upstream: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
struct ManifestDoc {
    schema_version: u32,
    label: String,
    producer: String,
    capability: String,
    runner: String,
    printer: String,
    expected: String,
    #[serde(default)]
    expected_rel: String,
    stages: Vec<String>,
    #[serde(default)]
    sources: Vec<InputDoc>,
    #[serde(default)]
    siblings: Vec<InputDoc>,
    #[serde(default)]
    tools: Vec<ToolDoc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stage {
    pub tool: String,
    pub classes: Vec<String>,
    pub sources: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manifest {
    pub label: String,
    pub producer: String,
    pub capability: String,
    pub runner: String,
    pub printer: String,
    pub expected: String,
    pub expected_rel: String,
    pub stages: Vec<Stage>,
    pub sources: Vec<(String, String)>,
    pub siblings: Vec<(String, String)>,
    pub tools: Vec<ToolDoc>,
}

impl Manifest {
    /// Reads one declared case manifest.
    pub fn parse(bytes: &[u8]) -> Result<Manifest, HarnessError> {
        let doc: ManifestDoc =
            serde_json::from_slice(bytes).map_err(|error| HarnessError::MalformedManifest {
                detail: error.to_string(),
            })?;
        if doc.schema_version != MANIFEST_SCHEMA_VERSION {
            return Err(HarnessError::UnsupportedSchema {
                found: doc.schema_version,
            });
        }
        for (what, value) in [
            ("label", &doc.label),
            ("producer", &doc.producer),
            ("runner", &doc.runner),
            ("printer", &doc.printer),
            ("expected", &doc.expected),
        ] {
            if value.is_empty() {
                return Err(HarnessError::UnknownValue {
                    what,
                    value: (*value).clone(),
                    reason: "must not be empty",
                });
            }
        }
        match doc.capability.as_str() {
            "lint" | "format" | "typecheck" => {}
            _ => {
                return Err(HarnessError::UnknownValue {
                    what: "capability",
                    value: doc.capability.clone(),
                    reason: "want lint, format, or typecheck",
                })
            }
        }
        if doc.stages.is_empty() {
            return Err(HarnessError::UnknownValue {
                what: "stages",
                value: String::new(),
                reason: "want at least one stage",
            });
        }
        let stages = doc
            .stages
            .iter()
            .map(|spec| parse_stage(spec))
            .collect::<Result<Vec<Stage>, HarnessError>>()?;
        let declared: BTreeSet<&str> = doc
            .sources
            .iter()
            .map(|input| input.workspace.as_str())
            .collect();
        for (index, stage) in stages.iter().enumerate() {
            for path in &stage.sources {
                if !declared.contains(path.as_str()) {
                    return Err(HarnessError::UndeclaredSource {
                        label: doc.label.clone(),
                        stage: index,
                        path: path.clone(),
                    });
                }
            }
        }
        let mut tools = BTreeSet::new();
        for tool in &doc.tools {
            if tool.tool.is_empty() {
                return Err(HarnessError::UnknownValue {
                    what: "tool",
                    value: tool.tool.clone(),
                    reason: "must not be empty",
                });
            }
            if !tools.insert(tool.tool.as_str()) {
                return Err(HarnessError::UnknownValue {
                    what: "tool",
                    value: tool.tool.clone(),
                    reason: "is declared twice",
                });
            }
        }
        for stage in &stages {
            if !tools.contains(stage.tool.as_str()) {
                return Err(HarnessError::UnknownValue {
                    what: "stage tool",
                    value: stage.tool.clone(),
                    reason: "has no declared binary or upstream diagnostics",
                });
            }
        }
        for entry in doc.tools.iter().flat_map(|tool| tool.env.iter()) {
            match (entry.value.as_deref(), entry.value_from.as_deref()) {
                (Some(_), None) | (None, Some(RUNFILES_DIR_ENV_LITERAL)) => {}
                _ => {
                    return Err(HarnessError::UnknownValue {
                        what: "tool env",
                        value: entry.key.clone(),
                        reason: "want exactly one of value or value_from=runfiles_dir",
                    })
                }
            }
        }
        Ok(Manifest {
            label: doc.label,
            producer: doc.producer,
            capability: doc.capability,
            runner: doc.runner,
            printer: doc.printer,
            expected: doc.expected,
            expected_rel: doc.expected_rel,
            stages,
            sources: pairs(&doc.sources),
            siblings: pairs(&doc.siblings),
            tools: doc.tools,
        })
    }

    /// Returns the structured request one case hands to the quality runner.
    pub fn request(&self, exec: &Exec, scratch: &Path) -> Result<Vec<u8>, HarnessError> {
        let stages: Vec<serde_json::Value> = self
            .stages
            .iter()
            .map(|stage| {
                json!({
                    "tool": stage.tool,
                    "classes": stage.classes,
                    "sources": stage.sources,
                })
            })
            .collect();
        let mut tools = serde_json::Map::new();
        for tool in &self.tools {
            let mut entry = serde_json::Map::new();
            if let Some(binary) = &tool.binary {
                entry.insert(
                    "binary".to_owned(),
                    json!(exec.path("tool binary", binary)?.display().to_string()),
                );
            }
            if let Some(config) = &tool.config {
                entry.insert("config".to_owned(), json!(config));
            }
            if let Some(edition) = &tool.edition {
                entry.insert("edition".to_owned(), json!(edition));
            }
            let mut files = Vec::with_capacity(tool.files.len());
            for file in &tool.files {
                files.push(json!({
                    "mirror_rel": file.rel,
                    "exec": exec.path("tool file", &file.key)?.display().to_string(),
                }));
            }
            let mut env = Vec::with_capacity(tool.env.len());
            for declared in &tool.env {
                let value = match declared.value_from.as_deref() {
                    Some(RUNFILES_DIR_ENV_LITERAL) => exec.runfiles_dir().display().to_string(),
                    _ => declared.value.clone().unwrap_or_default(),
                };
                env.push(json!({"key": declared.key, "value": value}));
            }
            let mut upstream = Vec::with_capacity(tool.upstream.len());
            for key in &tool.upstream {
                upstream.push(exec.path("upstream diagnostics", key)?.display().to_string());
            }
            entry.insert("files".to_owned(), json!(files));
            entry.insert("env".to_owned(), json!(env));
            entry.insert("upstream".to_owned(), json!(upstream));
            tools.insert(tool.tool.clone(), json!(entry));
        }
        let request = json!({
            "schema_version": REQUEST_SCHEMA_VERSION,
            "producer": self.producer,
            "capability": self.capability,
            "stages": stages,
            "sources": mappings(&self.sources, exec)?,
            "siblings": mappings(&self.siblings, exec)?,
            "resolves": [],
            "tools": tools,
            "scratch_parent": scratch.display().to_string(),
        });
        serde_json::to_vec(&request).map_err(|error| HarnessError::MalformedManifest {
            detail: error.to_string(),
        })
    }
}

const RUNFILES_DIR_ENV_LITERAL: &str = "runfiles_dir";

fn pairs(inputs: &[InputDoc]) -> Vec<(String, String)> {
    inputs
        .iter()
        .map(|input| (input.workspace.clone(), input.key.clone()))
        .collect()
}

fn mappings(entries: &[(String, String)], exec: &Exec) -> Result<Vec<serde_json::Value>, HarnessError> {
    let mut out = Vec::with_capacity(entries.len());
    for (workspace, key) in entries {
        out.push(json!({
            "workspace": workspace,
            "exec": exec.path("input", key)?.display().to_string(),
        }));
    }
    Ok(out)
}

fn parse_stage(spec: &str) -> Result<Stage, HarnessError> {
    let bad = || HarnessError::BadStage {
        spec: spec.to_owned(),
    };
    let (tool, rest) = spec.split_once(';').ok_or_else(bad)?;
    let (classes, sources) = rest.split_once(';').ok_or_else(bad)?;
    if tool.is_empty() || classes.is_empty() || sources.is_empty() {
        return Err(bad());
    }
    Ok(Stage {
        tool: tool.to_owned(),
        classes: classes.split(',').map(str::to_owned).collect(),
        sources: sources.split(',').map(str::to_owned).collect(),
    })
}

/// The runfiles one case resolves its declared inputs through.
#[derive(Debug)]
pub struct Exec {
    resolver: Resolver,
    runfiles_dir: PathBuf,
}

impl Exec {
    /// Reads the runfiles beside one harness binary.
    pub fn for_binary(binary: &Path) -> Result<Exec, HarnessError> {
        let resolver = Resolver::for_binary(binary).map_err(|error| HarnessError::Unreadable {
            path: binary.display().to_string(),
            detail: error.to_string(),
        })?;
        let source = resolver.source().to_path_buf();
        let runfiles_dir = source.parent().map(Path::to_path_buf).ok_or_else(|| {
            HarnessError::Unreadable {
                path: source.display().to_string(),
                detail: "has no parent directory".to_owned(),
            }
        })?;
        Ok(Exec {
            resolver,
            runfiles_dir,
        })
    }

    /// Returns the runfiles directory the resolver read.
    pub fn runfiles_dir(&self) -> &Path {
        &self.runfiles_dir
    }

    /// Returns the file one declared key names, requiring a regular file.
    pub fn path(&self, what: &'static str, key: &str) -> Result<PathBuf, HarnessError> {
        let path = self.resolver.lookup(key).map_err(|error| {
            HarnessError::MissingRunfile {
                what,
                key: key.to_owned(),
                path: PathBuf::from(key),
            }
            .detail(error.to_string())
        })?;
        if path.is_file() {
            return Ok(path);
        }
        Err(HarnessError::MissingRunfile {
            what,
            key: key.to_owned(),
            path,
        })
    }
}

fn capability_name(value: i32) -> Option<String> {
    proto::Capability::try_from(value)
        .ok()
        .map(|capability| capability.as_str_name().to_owned())
}

fn convergence_name(value: i32) -> Option<String> {
    proto::Convergence::try_from(value)
        .ok()
        .map(|convergence| convergence.as_str_name().to_owned())
}

fn find_line<'a>(text: &'a str, prefix: &str) -> Option<&'a str> {
    text.lines().find(|line| line.starts_with(prefix))
}

fn row_count(text: &str, name: &str) -> usize {
    let prefix = format!("{name} ");
    text.lines()
        .filter(|line| line.starts_with(&prefix))
        .filter(|line| line.split_whitespace().count() > 2)
        .count()
}

fn header_count(text: &str, name: &str) -> Result<u64, HarnessError> {
    let invalid = |detail: String| HarnessError::PrintedInvalid { detail };
    for line in text.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() == 2 && parts[0] == name {
            return parts[1].parse::<u64>().map_err(|error| {
                invalid(format!("{name} count {:?}: {error}", parts[1]))
            });
        }
    }
    Err(invalid(format!("missing {name} count")))
}

fn mismatch(detail: String) -> HarnessError {
    HarnessError::ResultMismatch { detail }
}

fn printed_invalid(detail: String) -> HarnessError {
    HarnessError::PrintedInvalid { detail }
}

/// Returns whether one result carries the case the manifest declared.
pub fn check_result(manifest: &Manifest, result: &proto::QualityResult) -> Result<(), HarnessError> {
    if result.producer != manifest.producer {
        return Err(mismatch(format!(
            "producer is {:?}, want {:?}",
            result.producer, manifest.producer
        )));
    }
    let capability = capability_name(result.capability).ok_or_else(|| {
        mismatch(format!("capability {} is not a known value", result.capability))
    })?;
    if capability != manifest.capability {
        return Err(mismatch(format!(
            "capability is {capability}, want {}",
            manifest.capability
        )));
    }
    if result.stages.len() != manifest.stages.len() {
        return Err(mismatch(format!(
            "result carries {} stages, want {}",
            result.stages.len(),
            manifest.stages.len()
        )));
    }
    for (index, (stage, want)) in manifest.stages.iter().enumerate() {
        let got = &result.stages[index];
        if got.tool_id != stage.tool
            || got.class_ids != stage.classes
            || got.source_paths != stage.sources
        {
            return Err(mismatch(format!(
                "stage {index} is {} {:?} {:?}, want {} {:?} {:?}",
                got.tool_id,
                got.class_ids,
                got.source_paths,
                stage.tool,
                stage.classes,
                stage.sources
            )));
        }
    }
    if result.completed_rounds == 0 {
        return Err(mismatch("completed_rounds is 0".to_owned()));
    }
    let convergence = convergence_name(result.convergence).ok_or_else(|| {
        mismatch(format!("convergence {} is not a known value", result.convergence))
    })?;
    if convergence != "STABLE" {
        return Err(mismatch(format!("convergence is {convergence}, want STABLE")));
    }
    Ok(())
}

/// Returns whether the printed evidence agrees with the decoded result.
pub fn check_printed(result: &proto::QualityResult, text: &str) -> Result<(), HarnessError> {
    if text.is_empty() {
        return Err(printed_invalid("empty print_result".to_owned()));
    }
    if !text.ends_with('\n') {
        return Err(printed_invalid(
            "print_result must end with a newline".to_owned(),
        ));
    }
    let capability = capability_name(result.capability)
        .ok_or_else(|| printed_invalid(format!("capability {} is not known", result.capability)))?;
    let convergence = convergence_name(result.convergence)
        .ok_or_else(|| printed_invalid(format!("convergence {} is not known", result.convergence)))?;
    let mut lines = text.lines();
    let want = format!("producer {}", result.producer);
    if lines.next() != Some(want.as_str()) {
        return Err(printed_invalid(format!("first line must be {want:?}")));
    }
    let want = format!("capability {capability}");
    if lines.next() != Some(want.as_str()) {
        return Err(printed_invalid(format!("second line must be {want:?}")));
    }
    let want = format!("stages {}", result.stages.len());
    if find_line(text, "stages ") != Some(want.as_str()) {
        return Err(printed_invalid(format!("want {want:?}")));
    }
    let stage_rows = row_count(text, "stage");
    if stage_rows != result.stages.len() {
        return Err(printed_invalid(format!(
            "stage count mismatch: {stage_rows} rows for {} stages",
            result.stages.len()
        )));
    }
    let want = format!("completed_rounds {}", result.completed_rounds);
    if find_line(text, "completed_rounds ") != Some(want.as_str()) {
        return Err(printed_invalid(format!("want {want:?}")));
    }
    if find_line(text, "convergence ") != Some(format!("convergence {convergence}").as_str()) {
        return Err(printed_invalid(format!(
            "want convergence line {:?}",
            format!("convergence {convergence}")
        )));
    }
    let edits: usize = result.replacements.iter().map(|file| file.edits.len()).sum();
    for (name, want_rows) in [
        ("initial", result.initial_diagnostics.len()),
        ("terminal", result.terminal_diagnostics.len()),
        ("replacement", edits),
    ] {
        let count = header_count(text, name)?;
        if count != want_rows as u64 {
            return Err(printed_invalid(format!(
                "{name} header says {count}, want {want_rows}"
            )));
        }
        let rows = row_count(text, name);
        if rows != want_rows {
            return Err(printed_invalid(format!(
                "{name} rows vs header mismatch: {rows} rows for {count}"
            )));
        }
    }
    Ok(())
}

/// Returns the directory one case stages a refreshed golden in.
pub fn update_dir() -> PathBuf {
    std::env::var("TEST_UNDECLARED_OUTPUTS_DIR")
        .ok()
        .filter(|dir| !dir.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var("TMPDIR").ok().map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("/tmp"))
}

/// Returns whether one case stages a refreshed golden instead of comparing it.
pub fn update_expect() -> bool {
    std::env::var(UPDATE_EXPECT_ENV).as_deref() == Ok("1")
}

/// Stages one refreshed golden for review without touching a source snapshot.
pub fn stage_update(label: &str, actual: &[u8]) -> Result<PathBuf, HarnessError> {
    let dir = update_dir();
    std::fs::create_dir_all(&dir).map_err(|error| HarnessError::Unstageable {
        path: dir.display().to_string(),
        detail: error.to_string(),
    })?;
    let staged = dir.join(format!("{label}.expected.update"));
    std::fs::write(&staged, actual).map_err(|error| HarnessError::Unstageable {
        path: staged.display().to_string(),
        detail: error.to_string(),
    })?;
    Ok(staged)
}

/// Returns the human difference between a pinned golden and fresh evidence.
pub fn snapshot_patch(label: &str, expected: &[u8], actual: &[u8]) -> String {
    let expected_text = String::from_utf8_lossy(expected).into_owned();
    let actual_text = String::from_utf8_lossy(actual).into_owned();
    match render_patch(&[FilePatch {
        path: label,
        kind: PatchKind::Modify,
        original: &expected_text,
        candidate: &actual_text,
    }]) {
        Ok(patch) => patch,
        Err(error) => format!(
            "cannot render a unified diff: {error}\n--- expected\n{expected_text}\n+++ actual\n{actual_text}"
        ),
    }
}

/// Resolves the declared inputs of one case.
pub fn load_case() -> Result<(Manifest, Exec), HarnessError> {
    let key = std::env::var(MANIFEST_ENV).map_err(|_| HarnessError::NoManifest)?;
    let exe = std::env::current_exe().map_err(|error| HarnessError::Unreadable {
        path: "the harness binary".to_owned(),
        detail: error.to_string(),
    })?;
    let exec = Exec::for_binary(&exe)?;
    let manifest_path = exec
        .resolver
        .lookup(&key)
        .map_err(|error| HarnessError::UnreadableManifest {
            key: key.clone(),
            detail: error.to_string(),
        })?;
    let bytes = std::fs::read(&manifest_path).map_err(|error| HarnessError::Unreadable {
        path: manifest_path.display().to_string(),
        detail: error.to_string(),
    })?;
    Ok((Manifest::parse(&bytes)?, exec))
}

/// Runs one case: resolve, run the pipeline, validate, and compare the snapshot.
pub fn run_case() -> Result<(), HarnessError> {
    let (manifest, exec) = load_case()?;
    let runner = exec.path("runner", &manifest.runner)?;
    let printer = exec.path("printer", &manifest.printer)?;
    let expected = exec.path("expected", &manifest.expected)?;
    let expected_bytes = std::fs::read(&expected).map_err(|error| HarnessError::Unreadable {
        path: expected.display().to_string(),
        detail: error.to_string(),
    })?;
    let work = work_dir()?;
    let scratch = work.join("scratch");
    std::fs::create_dir_all(&scratch).map_err(|error| HarnessError::Unwritable {
        path: scratch.display().to_string(),
        detail: error.to_string(),
    })?;
    let request = manifest.request(&exec, &scratch)?;
    let request_path = work.join("request.json");
    std::fs::write(&request_path, &request).map_err(|error| HarnessError::Unwritable {
        path: request_path.display().to_string(),
        detail: error.to_string(),
    })?;
    let runfiles_dir = exec.runfiles_dir().to_path_buf();
    let output = work.join("out.pb");
    run_child(
        &runner,
        &[
            OsString::from("--request"),
            request_path.into_os_string(),
            OsString::from("--output"),
            output.clone().into_os_string(),
        ],
        &work,
        &runfiles_dir,
    )?;
    let result_bytes = std::fs::read(&output).map_err(|error| HarnessError::Unreadable {
        path: output.display().to_string(),
        detail: error.to_string(),
    })?;
    let result = decode_validated(&result_bytes).map_err(|error| HarnessError::InvalidResult {
        path: output.display().to_string(),
        detail: error.to_string(),
    })?;
    check_result(&manifest, &result)?;
    let printed = run_child(
        &printer,
        &[output.into_os_string()],
        &work,
        &runfiles_dir,
    )?;
    check_printed(&result, &String::from_utf8_lossy(&printed))?;
    if update_expect() {
        let staged = stage_update(&manifest.label, &printed)?;
        println!(
            "snapshot UPDATE_EXPECT: staged fresh actual at {}",
            staged.display()
        );
        if manifest.expected_rel.is_empty() {
            println!("review the staged golden before pinning it.");
        } else {
            println!("copy it to {}, then review before pinning.", manifest.expected_rel);
        }
        println!("matrix PASS (updated): {}", manifest.label);
        return Ok(());
    }
    if printed != expected_bytes {
        return Err(HarnessError::SnapshotMismatch {
            label: manifest.label.clone(),
            patch: snapshot_patch(&manifest.label, &expected_bytes, &printed),
        });
    }
    println!("matrix PASS: {}", manifest.label);
    Ok(())
}

fn work_dir() -> Result<PathBuf, HarnessError> {
    let tmp = std::env::var("TEST_TMPDIR")
        .ok()
        .filter(|dir| !dir.is_empty())
        .ok_or(HarnessError::NoTestTmpdir)?;
    Ok(PathBuf::from(tmp).join(WORK_DIR))
}

fn run_child(
    program: &Path,
    args: &[OsString],
    cwd: &Path,
    runfiles_dir: &Path,
) -> Result<Vec<u8>, HarnessError> {
    let name = program.display().to_string();
    let mut argv = vec![program.as_os_str().to_owned()];
    argv.extend(args.iter().cloned());
    let spec = SpawnSpec {
        argv,
        cwd: cwd.to_path_buf(),
        env: EnvPolicy::Inherited {
            extra: vec![(
                OsString::from(RUNFILES_DIR_ENV),
                runfiles_dir.as_os_str().to_owned(),
            )],
        },
        capture: CapturePolicy {
            max_bytes: CAPTURE_BYTES,
        },
        timeout: CHILD_TIMEOUT,
    };
    let outcome =
        lifecycle::run(&spec).map_err(|error| HarnessError::Unreadable {
            path: name.clone(),
            detail: error.to_string(),
        })?;
    match outcome {
        ChildOutcome::Finished {
            exit,
            stdout,
            stderr,
        } => {
            if exit.code() != Some(0) {
                return Err(HarnessError::ChildFailed {
                    program: name,
                    exit: format!("{exit:?}"),
                    stderr: String::from_utf8_lossy(&stderr).into_owned(),
                });
            }
            Ok(stdout)
        }
        ChildOutcome::TimedOut => Err(HarnessError::ChildTimedOut {
            program: name,
            seconds: CHILD_TIMEOUT.as_secs(),
        }),
        ChildOutcome::OutputTooLarge { limit } => Err(HarnessError::ChildTooLarge {
            program: name,
            limit,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn manifest_doc() -> serde_json::Value {
        json!({
            "schema_version": 1,
            "label": "//quality/testdata:matrix_case",
            "producer": "//quality/testdata:matrix_case",
            "capability": "lint",
            "runner": "_main/run/runner",
            "printer": "_main/run/printer",
            "expected": "_main/run/expected.txt",
            "expected_rel": "quality/testdata/matrix/matrix_case.expected.txt",
            "stages": ["ruff;python;a.py"],
            "sources": [{"workspace": "a.py", "key": "_main/run/a.py"}],
            "siblings": [{"workspace": "sibling.txt", "key": "_main/run/sibling.txt"}],
            "tools": [{
                "tool": "ruff",
                "binary": "_main/run/ruff",
                "config": "ruff.toml",
                "edition": "2021",
                "files": [{"rel": "extra.toml", "key": "_main/run/extra.toml"}],
                "env": [{"key": "RUNFILES_DIR", "value_from": "runfiles_dir"}],
                "upstream": ["_main/run/upstream.txt"],
            }],
        })
    }

    fn parse(doc: &serde_json::Value) -> Result<Manifest, HarnessError> {
        Manifest::parse(&serde_json::to_vec(doc).expect("manifest encodes"))
    }

    fn parsed() -> Manifest {
        parse(&manifest_doc()).expect("manifest parses")
    }

    fn result() -> proto::QualityResult {
        proto::QualityResult {
            schema_major: 1,
            schema_minor: 0,
            producer: "//quality/testdata:matrix_case".to_owned(),
            capability: proto::Capability::Lint as i32,
            stages: vec![proto::Stage {
                tool_id: "ruff".to_owned(),
                class_ids: vec!["python".to_owned()],
                source_paths: vec!["a.py".to_owned()],
            }],
            completed_rounds: 1,
            convergence: proto::Convergence::Stable as i32,
            ..proto::QualityResult::default()
        }
    }

    fn printed(result: &proto::QualityResult) -> String {
        format!(
            "producer {}\ncapability LINT\nstages 1\nstage ruff classes=python sources=a.py\ncompleted_rounds {}\nconvergence STABLE\ninitial 0\nterminal 0\nreplacements 0\n",
            result.producer, result.completed_rounds
        )
    }

    fn scratch(name: &str) -> PathBuf {
        let base = std::env::var("TEST_TMPDIR")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        let dir = base.join(format!("dx-matrix-harness-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("run")).expect("scratch");
        dir
    }

    fn write(dir: &Path, name: &str, body: &str) -> PathBuf {
        let path = dir.join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("parent");
        }
        std::fs::write(&path, body).expect("write");
        path
    }

    fn exec_for(dir: &Path, keys: &[&str]) -> Exec {
        let binary = write(dir, "harness", "binary\n");
        let mut manifest = String::new();
        for key in keys {
            let path = write(dir, &format!("run/{key}"), "payload\n");
            manifest.push_str(&format!("{key} {}\n", path.display()));
        }
        write(
            dir,
            "harness.runfiles_manifest",
            &format!("{manifest}"),
        );
        Exec::for_binary(&binary).expect("runfiles resolve")
    }

    #[test]
    fn a_manifest_keeps_every_declared_input() {
        let manifest = parsed();
        assert_eq!(manifest.label, "//quality/testdata:matrix_case");
        assert_eq!(manifest.capability, "lint");
        assert_eq!(manifest.expected_rel, "quality/testdata/matrix/matrix_case.expected.txt");
        assert_eq!(
            manifest.sources,
            [("a.py".to_owned(), "_main/run/a.py".to_owned())]
        );
        assert_eq!(
            manifest.siblings,
            [("sibling.txt".to_owned(), "_main/run/sibling.txt".to_owned())]
        );
        assert_eq!(manifest.stages[0].tool, "ruff");
        assert_eq!(manifest.stages[0].classes, ["python"]);
        assert_eq!(manifest.stages[0].sources, ["a.py"]);
        assert_eq!(manifest.tools[0].config.as_deref(), Some("ruff.toml"));
        assert_eq!(manifest.tools[0].edition.as_deref(), Some("2021"));
        assert_eq!(manifest.tools[0].files[0].rel, "extra.toml");
        assert_eq!(manifest.tools[0].upstream, ["_main/run/upstream.txt"]);
    }

    #[test]
    fn an_unknown_manifest_field_is_rejected() {
        let mut doc = manifest_doc();
        doc["bogus"] = json!(true);
        assert!(
            matches!(parse(&doc), Err(HarnessError::MalformedManifest { .. })),
            "an unknown field must fail"
        );
    }

    #[test]
    fn the_manifest_schema_version_is_pinned() {
        let mut doc = manifest_doc();
        doc["schema_version"] = json!(2);
        assert_eq!(
            parse(&doc).expect_err("schema 2"),
            HarnessError::UnsupportedSchema { found: 2 }
        );
    }

    #[test]
    fn envelope_fields_must_be_present_and_known() {
        for field in ["label", "producer", "runner", "printer", "expected"] {
            let mut doc = manifest_doc();
            doc[field] = json!("");
            assert!(
                matches!(parse(&doc), Err(HarnessError::UnknownValue { what, .. }) if what == field),
                "{field} must be rejected when empty"
            );
        }
        let mut doc = manifest_doc();
        doc["capability"] = json!("audit");
        assert!(
            matches!(parse(&doc), Err(HarnessError::UnknownValue { what, .. }) if what == "capability"),
            "an unknown capability must fail"
        );
        let mut doc = manifest_doc();
        doc["stages"] = json!([]);
        assert!(
            matches!(parse(&doc), Err(HarnessError::UnknownValue { what, .. }) if what == "stages"),
            "a case without stages must fail"
        );
    }

    #[test]
    fn malformed_stages_and_undeclared_sources_fail() {
        for spec in ["ruff", "ruff;python", ";python;a.py", "ruff;;a.py", "ruff;python;"] {
            let mut doc = manifest_doc();
            doc["stages"] = json!([spec]);
            assert!(
                matches!(parse(&doc), Err(HarnessError::BadStage { .. })),
                "stage {spec:?} must fail"
            );
        }
        let mut doc = manifest_doc();
        doc["stages"] = json!(["ruff;python;absent.py"]);
        assert_eq!(
            parse(&doc).expect_err("undeclared source"),
            HarnessError::UndeclaredSource {
                label: "//quality/testdata:matrix_case".to_owned(),
                stage: 0,
                path: "absent.py".to_owned(),
            }
        );
    }

    #[test]
    fn tools_must_be_declared_once_and_used_by_a_stage() {
        let mut doc = manifest_doc();
        doc["tools"] = json!([doc["tools"][0].clone(), doc["tools"][0].clone()]);
        assert!(
            matches!(parse(&doc), Err(HarnessError::UnknownValue { what, .. }) if what == "tool"),
            "a duplicated tool must fail"
        );
        let mut doc = manifest_doc();
        doc["stages"] = json!(["ty;python;a.py"]);
        assert!(
            matches!(parse(&doc), Err(HarnessError::UnknownValue { what, .. }) if what == "stage tool"),
            "a staged tool without a declared entry must fail"
        );
    }

    #[test]
    fn a_tool_env_entry_names_exactly_one_value_source() {
        for env in [
            json!([{"key": "A"}]),
            json!([{"key": "A", "value": "1", "value_from": "runfiles_dir"}]),
            json!([{"key": "A", "value_from": "ambient"}]),
        ] {
            let mut doc = manifest_doc();
            doc["tools"][0]["env"] = env;
            assert!(
                matches!(parse(&doc), Err(HarnessError::UnknownValue { what, .. }) if what == "tool env"),
                "{env} must fail"
            );
        }
    }

    #[test]
    fn the_request_carries_resolved_paths_and_the_scratch_parent() {
        let dir = scratch("request");
        let exec = exec_for(
            &dir,
            &[
                "a.py",
                "sibling.txt",
                "ruff",
                "extra.toml",
                "upstream.txt",
            ],
        );
        let manifest = parsed();
        let request = manifest.request(&exec, &dir).expect("request builds");
        let value: serde_json::Value = serde_json::from_slice(&request).expect("request decodes");
        assert_eq!(value["schema_version"], json!(1));
        assert_eq!(value["producer"], json!("//quality/testdata:matrix_case"));
        assert_eq!(value["capability"], json!("lint"));
        assert_eq!(
            value["stages"],
            json!([{"tool": "ruff", "classes": ["python"], "sources": ["a.py"]}])
        );
        assert_eq!(
            value["sources"],
            json!([{"workspace": "a.py", "exec": dir.join("run/a.py").display().to_string()}])
        );
        assert_eq!(
            value["siblings"],
            json!([{"workspace": "sibling.txt", "exec": dir.join("run/sibling.txt").display().to_string()}])
        );
        assert_eq!(value["resolves"], json!([]));
        assert_eq!(value["scratch_parent"], json!(dir.display().to_string()));
        let tool = &value["tools"]["ruff"];
        assert_eq!(
            tool["binary"],
            json!(dir.join("run/ruff").display().to_string())
        );
        assert_eq!(tool["config"], json!("ruff.toml"));
        assert_eq!(tool["edition"], json!("2021"));
        assert_eq!(
            tool["files"],
            json!([{
                "mirror_rel": "extra.toml",
                "exec": dir.join("run/extra.toml").display().to_string(),
            }])
        );
        assert_eq!(
            tool["upstream"],
            json!([dir.join("run/upstream.txt").display().to_string()])
        );
        assert_eq!(
            tool["env"],
            json!([{"key": "RUNFILES_DIR", "value": dir.display().to_string()}])
        );
    }

    #[test]
    fn a_declared_key_that_is_not_a_file_fails() {
        let dir = scratch("missing");
        let exec = exec_for(&dir, &["a.py"]);
        assert_eq!(
            exec.path("expected", "a.py").expect("declared file"),
            dir.join("run/a.py")
        );
        assert_eq!(
            exec.path("expected", "absent.py")
                .expect_err("undeclared key"),
            HarnessError::MissingRunfile {
                what: "expected",
                key: "absent.py".to_owned(),
                path: PathBuf::from("absent.py"),
            }
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_result_must_match_the_declared_case() {
        let manifest = parsed();
        check_result(&manifest, &result()).expect("the declared result matches");
        let mut foreign = result();
        foreign.producer = "//pkg:other".to_owned();
        assert!(
            check_result(&manifest, &foreign).is_err(),
            "a foreign producer must fail"
        );
        let mut foreign = result();
        foreign.capability = proto::Capability::Format as i32;
        assert!(
            check_result(&manifest, &foreign).is_err(),
            "a foreign capability must fail"
        );
        let mut foreign = result();
        foreign.stages.clear();
        assert!(
            check_result(&manifest, &foreign).is_err(),
            "a missing stage must fail"
        );
        let mut foreign = result();
        foreign.stages[0].source_paths = vec!["other.py".to_owned()];
        assert!(
            check_result(&manifest, &foreign).is_err(),
            "a renamed stage source must fail"
        );
        let mut foreign = result();
        foreign.completed_rounds = 0;
        assert!(
            check_result(&manifest, &foreign).is_err(),
            "zero completed rounds must fail"
        );
        let mut foreign = result();
        foreign.convergence = proto::Convergence::Oscillation as i32;
        assert!(
            check_result(&manifest, &foreign).is_err(),
            "an unstable pipeline must fail"
        );
    }

    #[test]
    fn printed_evidence_must_agree_with_the_decoded_result() {
        let result = result();
        check_printed(&result, &printed(&result)).expect("printed evidence agrees");
    }

    #[test]
    fn empty_or_unterminated_printed_evidence_fails() {
        let result = result();
        assert!(
            check_printed(&result, "").is_err(),
            "empty evidence must fail"
        );
        assert!(
            check_printed(&result, "producer x\n").is_err(),
            "evidence without a trailing newline must fail"
        );
    }

    #[test]
    fn a_count_or_header_mismatch_fails() {
        let result = result();
        for (from, to, because) in [
            ("initial 0", "initial 1", "an initial header must match"),
            ("terminal 0", "terminal 1", "a terminal header must match"),
            ("replacements 0", "replacements 1", "a replacement header must match"),
            ("stages 1", "stages 2", "a stage count must match"),
            ("stage ruff", "stage ty", "a stage row must count"),
            (
                "convergence STABLE",
                "convergence OSCILLATION",
                "a convergence line must match",
            ),
            (
                "completed_rounds 1",
                "completed_rounds 4",
                "a rounds line must match",
            ),
            ("producer //", "producer //x", "a producer line must match"),
            (
                "capability LINT",
                "capability FORMAT",
                "a capability line must match",
            ),
        ] {
            let text = printed(&result).replace(from, to);
            assert!(
                check_printed(&result, &text).is_err(),
                "{because}: {text}"
            );
        }
        let text = printed(&result).replace("initial 0", "initial");
        assert!(
            check_printed(&result, &text).is_err(),
            "an initial header without a count must fail"
        );
    }

    #[test]
    fn count_headers_and_rows_are_read_apart() {
        let text = "initial 2\ninitial ERROR ruff F401 a.py 1 2 fixable=false \"x\"\ninitial ERROR ruff F401 a.py 3 4 fixable=false \"y\"\n";
        assert_eq!(header_count(text, "initial").expect("header"), 2);
        assert_eq!(row_count(text, "initial"), 2);
        assert_eq!(row_count(text, "terminal"), 0);
        assert!(
            header_count(text, "terminal").is_err(),
            "an absent header must fail"
        );
        assert!(
            header_count("initial many\n", "initial").is_err(),
            "a non-numeric header must fail"
        );
    }

    #[test]
    fn a_snapshot_mismatch_renders_both_sides() {
        let patch = snapshot_patch("case", b"a\nb\n", b"a\nc\n");
        assert!(patch.contains("--- a/case"), "header: {patch}");
        assert!(patch.contains("+++ b/case"), "header: {patch}");
        assert!(patch.contains("-b"), "old line: {patch}");
        assert!(patch.contains("+c"), "new line: {patch}");
    }

    #[test]
    fn a_staged_update_is_a_reviewable_file() {
        let dir = scratch("update");
        let previous = std::env::var("TEST_UNDECLARED_OUTPUTS_DIR").ok();
        std::env::set_var("TEST_UNDECLARED_OUTPUTS_DIR", &dir);
        let staged = stage_update("matrix_case", b"producer //pkg:case\n");
        std::env::remove_var("TEST_UNDECLARED_OUTPUTS_DIR");
        if let Some(previous) = previous {
            std::env::set_var("TEST_UNDECLARED_OUTPUTS_DIR", previous);
        }
        let staged = staged.expect("staged");
        assert_eq!(staged, dir.join("matrix_case.expected.update"));
        assert_eq!(
            std::fs::read(&staged).expect("staged bytes"),
            b"producer //pkg:case\n"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}