use crate::validation::{check_correlation, check_path, parse_digest, OutputError};
use serde_json::{json, Value};

pub const SCHEMA_MAJOR: u64 = 3;
pub const SCHEMA_MINOR: u64 = 4;

pub const EVENTS: &[&str] = &[
    "build_failure",
    "cache_miss",
    "capability",
    "change",
    "changed_line_coverage",
    "command_finished",
    "command_started",
    "coverage",
    "diagnostic",
    "error",
    "notice",
    "operation",
    "phase_finished",
    "phase_started",
    "report",
    "selection",
    "slot",
    "status",
    "summary",
    "test_outcome",
];

pub fn schema() -> Value {
    json!({"major": SCHEMA_MAJOR, "minor": SCHEMA_MINOR})
}

pub(crate) fn nonempty(field: &'static str, value: &str) -> Result<(), OutputError> {
    if value.is_empty() {
        return Err(OutputError::EmptyField { field });
    }
    Ok(())
}

pub(crate) fn base(event: &str) -> serde_json::Map<String, Value> {
    debug_assert!(
        EVENTS.contains(&event),
        "{event} is not a registered NDJSON event"
    );
    let mut map = serde_json::Map::new();
    map.insert("schema".to_owned(), schema());
    map.insert("event".to_owned(), Value::String(event.to_owned()));
    map
}

pub fn with_correlation(event: Value, correlation: &str) -> Result<Value, OutputError> {
    check_correlation(correlation)?;
    match event {
        Value::Object(mut map) => {
            if !map.contains_key("schema") || !matches!(map.get("event"), Some(Value::String(_))) {
                return Err(OutputError::NotAnEvent);
            }
            map.insert(
                "correlation".to_owned(),
                Value::String(correlation.to_owned()),
            );
            Ok(Value::Object(map))
        }
        _ => Err(OutputError::NotAnEvent),
    }
}

pub fn command_started(command: &str, mode: &str) -> Result<Value, OutputError> {
    nonempty("command", command)?;
    if mode != "default" && mode != "check" {
        return Err(OutputError::BadCommandMode {
            value: mode.to_owned(),
        });
    }
    let mut map = base("command_started");
    map.insert("command".to_owned(), Value::String(command.to_owned()));
    map.insert("mode".to_owned(), Value::String(mode.to_owned()));
    Ok(Value::Object(map))
}

/// Rewrites one child command event into its parent's stream under `phase`.
pub fn nest_phase_event(
    event: Value,
    command: &str,
    phase: &str,
    correlation: &str,
) -> Result<Value, OutputError> {
    nonempty("command", command)?;
    nonempty("phase", phase)?;
    let Value::Object(mut map) = event else {
        return Err(OutputError::NotAnEvent);
    };
    let name = match map.get("event") {
        Some(Value::String(name)) if map.contains_key("schema") => name.clone(),
        _ => return Err(OutputError::NotAnEvent),
    };
    let renamed = match name.as_str() {
        "command_started" => Some("phase_started"),
        "command_finished" => Some("phase_finished"),
        "phase_started" | "phase_finished" => return Err(OutputError::NotAnEvent),
        _ => None,
    };
    if let Some(renamed) = renamed {
        map.insert("event".to_owned(), Value::String(renamed.to_owned()));
        map.insert("command".to_owned(), Value::String(command.to_owned()));
        map.insert("phase".to_owned(), Value::String(phase.to_owned()));
    }
    with_correlation(Value::Object(map), correlation)
}

/// Parses one NDJSON line and rejects any schema major this dx does not write.
pub fn read_event(line: &str) -> Result<Value, OutputError> {
    let value: Value = serde_json::from_str(line).map_err(|_| OutputError::NotAnEvent)?;
    let Value::Object(map) = &value else {
        return Err(OutputError::NotAnEvent);
    };
    if !matches!(map.get("event"), Some(Value::String(_))) {
        return Err(OutputError::NotAnEvent);
    }
    let major = map.get("schema").and_then(|schema| schema.get("major"));
    let minor = map.get("schema").and_then(|schema| schema.get("minor"));
    match (major.and_then(Value::as_u64), minor.and_then(Value::as_u64)) {
        (Some(SCHEMA_MAJOR), Some(_)) => Ok(value),
        (Some(major), Some(minor)) => Err(OutputError::UnsupportedSchema { major, minor }),
        _ => Err(OutputError::NotAnEvent),
    }
}

pub fn operation_event(
    command: &str,
    phase: &str,
    scope: Option<&[String]>,
) -> Result<Value, OutputError> {
    nonempty("command", command)?;
    nonempty("phase", phase)?;
    let mut map = base("operation");
    map.insert("command".to_owned(), Value::String(command.to_owned()));
    map.insert("phase".to_owned(), Value::String(phase.to_owned()));
    if let Some(scope) = scope {
        map.insert(
            "scope".to_owned(),
            Value::Array(scope.iter().map(|s| Value::String(s.clone())).collect()),
        );
    }
    Ok(Value::Object(map))
}

pub fn report_event(
    format: &str,
    path: &str,
    results_complete: bool,
) -> Result<Value, OutputError> {
    nonempty("format", format)?;
    nonempty("path", path)?;
    let mut map = base("report");
    map.insert("format".to_owned(), Value::String(format.to_owned()));
    map.insert("path".to_owned(), Value::String(path.to_owned()));
    map.insert("results_complete".to_owned(), Value::Bool(results_complete));
    Ok(Value::Object(map))
}

pub fn selection_event(
    setup_id: &str,
    environment_id: &str,
    codegen_id: &str,
) -> Result<Value, OutputError> {
    parse_digest("setup_id", setup_id)?;
    parse_digest("environment_id", environment_id)?;
    parse_digest("codegen_id", codegen_id)?;
    let mut map = base("selection");
    map.insert("setup_id".to_owned(), Value::String(setup_id.to_owned()));
    map.insert(
        "environment_id".to_owned(),
        Value::String(environment_id.to_owned()),
    );
    map.insert(
        "codegen_id".to_owned(),
        Value::String(codegen_id.to_owned()),
    );
    Ok(Value::Object(map))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusEvent {
    pub name: String,
    pub status: String,
    pub detail: String,
    pub hint: String,
}

pub fn status_event(check: &StatusEvent) -> Result<Value, OutputError> {
    nonempty("name", &check.name)?;
    nonempty("status", &check.status)?;
    if check.status != "ok" && check.status != "warn" && check.status != "error" {
        return Err(OutputError::BadSeverity {
            value: check.status.clone(),
        });
    }
    nonempty("detail", &check.detail)?;
    nonempty("hint", &check.hint)?;
    let mut map = base("status");
    map.insert("name".to_owned(), Value::String(check.name.clone()));
    map.insert("status".to_owned(), Value::String(check.status.clone()));
    map.insert("detail".to_owned(), Value::String(check.detail.clone()));
    map.insert("hint".to_owned(), Value::String(check.hint.clone()));
    Ok(Value::Object(map))
}

/// One worktree slot and its state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlotEvent {
    pub action: String,
    pub path: String,
    pub slot: Option<u32>,
    pub branch: Option<String>,
    pub holder: Option<String>,
    pub state: String,
    pub reason: Option<String>,
    pub base: Option<String>,
    pub server: bool,
    pub disk_bytes: Option<u64>,
}

pub fn slot_event(slot: &SlotEvent) -> Result<Value, OutputError> {
    nonempty("action", &slot.action)?;
    nonempty("path", &slot.path)?;
    nonempty("state", &slot.state)?;
    let mut map = base("slot");
    map.insert("action".to_owned(), Value::String(slot.action.clone()));
    map.insert("path".to_owned(), Value::String(slot.path.clone()));
    if let Some(number) = slot.slot {
        map.insert("slot".to_owned(), Value::from(number));
    }
    if let Some(branch) = &slot.branch {
        map.insert("branch".to_owned(), Value::String(branch.clone()));
    }
    if let Some(holder) = &slot.holder {
        map.insert("holder".to_owned(), Value::String(holder.clone()));
    }
    map.insert("state".to_owned(), Value::String(slot.state.clone()));
    if let Some(reason) = &slot.reason {
        map.insert("reason".to_owned(), Value::String(reason.clone()));
    }
    if let Some(base) = &slot.base {
        map.insert("base".to_owned(), Value::String(base.clone()));
    }
    map.insert("server".to_owned(), Value::Bool(slot.server));
    if let Some(bytes) = slot.disk_bytes {
        map.insert("disk_bytes".to_owned(), Value::from(bytes));
    }
    Ok(Value::Object(map))
}

/// One cell's coverage account from `dx coverage`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoverageEvent {
    pub cell: String,
    pub covered_lines: u64,
    pub eligible_lines: u64,
    pub raw_lines: u64,
    pub excluded_lines: u64,
    pub eligible_files: u64,
    pub support_files: u64,
    pub excluded_files: u64,
    pub unsupported_files: u64,
    pub minimum: Option<u32>,
    pub passed: bool,
    pub errors: u64,
    pub dx_version: String,
}

pub fn coverage_event(coverage: &CoverageEvent) -> Result<Value, OutputError> {
    nonempty("cell", &coverage.cell)?;
    nonempty("dx_version", &coverage.dx_version)?;
    let mut map = base("coverage");
    map.insert("cell".to_owned(), Value::String(coverage.cell.clone()));
    for (key, value) in [
        ("covered_lines", coverage.covered_lines),
        ("eligible_lines", coverage.eligible_lines),
        ("raw_lines", coverage.raw_lines),
        ("excluded_lines", coverage.excluded_lines),
        ("eligible_files", coverage.eligible_files),
        ("support_files", coverage.support_files),
        ("excluded_files", coverage.excluded_files),
        ("unsupported_files", coverage.unsupported_files),
    ] {
        map.insert(key.to_owned(), Value::from(value));
    }
    if let Some(minimum) = coverage.minimum {
        map.insert("minimum".to_owned(), Value::from(minimum));
    }
    map.insert("passed".to_owned(), Value::Bool(coverage.passed));
    map.insert("errors".to_owned(), Value::from(coverage.errors));
    map.insert(
        "dx_version".to_owned(),
        Value::String(coverage.dx_version.clone()),
    );
    Ok(Value::Object(map))
}

/// The changed-line coverage gate from `dx coverage --min-changed-line-coverage`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangedLineCoverageEvent {
    pub base: String,
    pub merge_base: String,
    pub minimum: u32,
    pub covered_lines: u64,
    pub eligible_lines: u64,
    pub uncovered: Vec<(String, u32)>,
    pub uninstrumented: Vec<String>,
    pub result: String,
    pub dx_version: String,
}

const CHANGED_LINE_RESULTS: &[&str] = &[
    "pass",
    "fail",
    "not-applicable",
    "incomplete",
    "unavailable",
];

pub fn changed_line_coverage_event(gate: &ChangedLineCoverageEvent) -> Result<Value, OutputError> {
    nonempty("dx_version", &gate.dx_version)?;
    if !CHANGED_LINE_RESULTS.contains(&gate.result.as_str()) {
        return Err(OutputError::BadValue {
            field: "changed_line_coverage result",
            value: gate.result.clone(),
        });
    }
    if gate.minimum > 100 {
        return Err(OutputError::BadValue {
            field: "changed_line_coverage minimum",
            value: gate.minimum.to_string(),
        });
    }
    if gate.covered_lines > gate.eligible_lines {
        return Err(OutputError::BadValue {
            field: "changed_line_coverage covered_lines",
            value: gate.covered_lines.to_string(),
        });
    }
    let mut map = base("changed_line_coverage");
    map.insert("base".to_owned(), Value::String(gate.base.clone()));
    map.insert(
        "merge_base".to_owned(),
        Value::String(gate.merge_base.clone()),
    );
    map.insert("minimum".to_owned(), Value::from(gate.minimum));
    map.insert("covered_lines".to_owned(), Value::from(gate.covered_lines));
    map.insert(
        "eligible_lines".to_owned(),
        Value::from(gate.eligible_lines),
    );
    let mut uncovered = Vec::with_capacity(gate.uncovered.len());
    for (path, line) in &gate.uncovered {
        nonempty("uncovered path", path)?;
        uncovered.push(json!({"path": path, "line": line}));
    }
    map.insert("uncovered".to_owned(), Value::Array(uncovered));
    for path in &gate.uninstrumented {
        nonempty("uninstrumented path", path)?;
    }
    map.insert("uninstrumented".to_owned(), json!(gate.uninstrumented));
    map.insert("result".to_owned(), Value::String(gate.result.clone()));
    map.insert(
        "dx_version".to_owned(),
        Value::String(gate.dx_version.clone()),
    );
    Ok(Value::Object(map))
}

/// One reason an action re-ran, from `dx explain`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CacheMissChange {
    /// `input`, `env`, `arg`, `platform`, `not_in_old_log`, or `not_cacheable`.
    pub kind: String,
    /// `modified`, `added`, `removed`, or `order_changed`; empty for kinds without one.
    pub change: String,
    /// The path, name, or value the change is about, under the field its kind names.
    pub subject: Option<String>,
}

/// One action that ran in the newer execution log, with why it missed the cache.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CacheMissEvent {
    pub mnemonic: String,
    pub target: String,
    pub primary_output: String,
    pub changes: Vec<CacheMissChange>,
    pub changes_omitted: u64,
}

const CACHE_MISS_KINDS: &[(&str, Option<&str>)] = &[
    ("input", Some("path")),
    ("env", Some("name")),
    ("arg", Some("value")),
    ("platform", Some("name")),
    ("not_in_old_log", None),
    ("not_cacheable", None),
];

const CACHE_MISS_CHANGES: &[&str] = &["modified", "added", "removed", "order_changed"];

pub fn cache_miss_event(miss: &CacheMissEvent) -> Result<Value, OutputError> {
    nonempty("cache_miss mnemonic", &miss.mnemonic)?;
    nonempty("cache_miss primary_output", &miss.primary_output)?;
    let mut changes = Vec::with_capacity(miss.changes.len());
    for change in &miss.changes {
        let Some((_, field)) = CACHE_MISS_KINDS
            .iter()
            .find(|(kind, _)| *kind == change.kind)
        else {
            return Err(OutputError::BadValue {
                field: "cache_miss kind",
                value: change.kind.clone(),
            });
        };
        let mut item = serde_json::Map::new();
        item.insert("kind".to_owned(), Value::String(change.kind.clone()));
        match field {
            Some(field) => {
                if !CACHE_MISS_CHANGES.contains(&change.change.as_str()) {
                    return Err(OutputError::BadValue {
                        field: "cache_miss change",
                        value: change.change.clone(),
                    });
                }
                item.insert("change".to_owned(), Value::String(change.change.clone()));
                if let Some(subject) = &change.subject {
                    item.insert((*field).to_owned(), Value::String(subject.clone()));
                } else if change.change != "order_changed" {
                    return Err(OutputError::EmptyField {
                        field: "cache_miss subject",
                    });
                }
            }
            None => {
                if !change.change.is_empty() || change.subject.is_some() {
                    return Err(OutputError::BadValue {
                        field: "cache_miss change",
                        value: change.change.clone(),
                    });
                }
            }
        }
        changes.push(Value::Object(item));
    }
    let mut map = base("cache_miss");
    map.insert("mnemonic".to_owned(), Value::String(miss.mnemonic.clone()));
    map.insert("target".to_owned(), Value::String(miss.target.clone()));
    map.insert(
        "primary_output".to_owned(),
        Value::String(miss.primary_output.clone()),
    );
    map.insert("changes".to_owned(), Value::Array(changes));
    if miss.changes_omitted > 0 {
        map.insert(
            "changes_omitted".to_owned(),
            Value::from(miss.changes_omitted),
        );
    }
    Ok(Value::Object(map))
}

/// The machine totals a worktree listing ends with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SummaryEvent {
    pub slots: u64,
    pub servers: u64,
    pub servers_rss_bytes: u64,
    pub disk_bytes: Option<u64>,
    pub disk_cache_bytes: Option<u64>,
}

pub fn summary_event(summary: &SummaryEvent) -> Value {
    let mut map = base("summary");
    map.insert("slots".to_owned(), Value::from(summary.slots));
    map.insert("servers".to_owned(), Value::from(summary.servers));
    map.insert(
        "servers_rss_bytes".to_owned(),
        Value::from(summary.servers_rss_bytes),
    );
    if let Some(bytes) = summary.disk_bytes {
        map.insert("disk_bytes".to_owned(), Value::from(bytes));
    }
    if let Some(bytes) = summary.disk_cache_bytes {
        map.insert("disk_cache_bytes".to_owned(), Value::from(bytes));
    }
    Value::Object(map)
}

/// One declared capability and where its availability was observed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityEvent {
    pub name: String,
    pub source: String,
    pub availability: String,
    pub detail: String,
    pub flags: Vec<String>,
    pub outputs: Vec<String>,
    pub reports: Vec<String>,
    pub scope_policy: String,
    pub effect: String,
    pub passthrough: bool,
}

/// Builds the event for a capability whose producer already validated it.
pub fn capability_value(capability: &CapabilityEvent) -> Value {
    let mut map = base("capability");
    map.insert("name".to_owned(), Value::String(capability.name.clone()));
    map.insert(
        "source".to_owned(),
        Value::String(capability.source.clone()),
    );
    map.insert(
        "availability".to_owned(),
        Value::String(capability.availability.clone()),
    );
    map.insert(
        "detail".to_owned(),
        Value::String(capability.detail.clone()),
    );
    map.insert(
        "flags".to_owned(),
        Value::Array(
            capability
                .flags
                .iter()
                .map(|flag| Value::String(flag.clone()))
                .collect(),
        ),
    );
    map.insert(
        "outputs".to_owned(),
        Value::Array(
            capability
                .outputs
                .iter()
                .map(|mode| Value::String(mode.clone()))
                .collect(),
        ),
    );
    map.insert(
        "reports".to_owned(),
        Value::Array(
            capability
                .reports
                .iter()
                .map(|format| Value::String(format.clone()))
                .collect(),
        ),
    );
    map.insert(
        "scope_policy".to_owned(),
        Value::String(capability.scope_policy.clone()),
    );
    map.insert(
        "effect".to_owned(),
        Value::String(capability.effect.clone()),
    );
    map.insert(
        "passthrough".to_owned(),
        Value::Bool(capability.passthrough),
    );
    Value::Object(map)
}

/// Builds the event for one declared capability.
pub fn capability_event(capability: &CapabilityEvent) -> Result<Value, OutputError> {
    nonempty("name", &capability.name)?;
    match capability.source.as_str() {
        "cli-grammar" | "workspace-record" | "unobserved" => {}
        _ => {
            return Err(OutputError::BadSource {
                value: capability.source.clone(),
            });
        }
    }
    match capability.availability.as_str() {
        "available" | "unavailable" | "unknown" => {}
        _ => {
            return Err(OutputError::BadAvailability {
                value: capability.availability.clone(),
            });
        }
    }
    nonempty("detail", &capability.detail)?;
    nonempty("scope_policy", &capability.scope_policy)?;
    nonempty("effect", &capability.effect)?;
    Ok(capability_value(capability))
}

pub fn error_event(
    code: &str,
    message: &str,
    path: Option<&str>,
    flag: Option<&str>,
    phase: Option<&str>,
) -> Result<Value, OutputError> {
    nonempty("code", code)?;
    nonempty("message", message)?;
    if let Some(path) = path {
        check_path(path)?;
    }
    if let Some(flag) = flag {
        nonempty("flag", flag)?;
    }
    if let Some(phase) = phase {
        nonempty("phase", phase)?;
    }
    let mut map = base("error");
    map.insert("code".to_owned(), Value::String(code.to_owned()));
    map.insert("message".to_owned(), Value::String(message.to_owned()));
    if let Some(path) = path {
        map.insert("path".to_owned(), Value::String(path.to_owned()));
    }
    if let Some(flag) = flag {
        map.insert("flag".to_owned(), Value::String(flag.to_owned()));
    }
    if let Some(phase) = phase {
        map.insert("phase".to_owned(), Value::String(phase.to_owned()));
    }
    Ok(Value::Object(map))
}

/// The class, child status and diagnostic tail of one failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FailureFacts<'a> {
    pub cause: &'a str,
    pub status: Option<i32>,
    pub excerpt: Option<&'a crate::diagnostics::Excerpt>,
}

/// An `error` event that also names the failure's cause, child status and bounded excerpt.
pub fn failure_event(
    code: &str,
    message: &str,
    phase: Option<&str>,
    facts: &FailureFacts<'_>,
) -> Result<Value, OutputError> {
    nonempty("cause", facts.cause)?;
    let mut event = error_event(code, message, None, None, phase)?;
    if let Value::Object(map) = &mut event {
        map.insert("cause".to_owned(), Value::String(facts.cause.to_owned()));
        if let Some(status) = facts.status {
            map.insert("status".to_owned(), Value::from(status));
        }
        if let Some(excerpt) = facts.excerpt {
            map.insert("excerpt".to_owned(), Value::String(excerpt.text.clone()));
            map.insert("truncated".to_owned(), Value::Bool(excerpt.truncated));
        }
    }
    Ok(event)
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TestOutcome {
    pub target: String,
    pub configuration: Option<String>,
    pub outcome: String,
    pub status: Option<String>,
    pub cached: Option<bool>,
    pub run: Option<u32>,
    pub shard: Option<u32>,
    pub attempt: Option<u32>,
    pub superseded: bool,
    pub duration_millis: Option<u64>,
    pub cases_total: Option<u64>,
    pub cases_failed: Option<u64>,
    pub cases_error: Option<u64>,
    pub cases_skipped: Option<u64>,
    pub artifacts_collected: u64,
    pub artifacts_missing: u64,
    pub artifacts_invalid: u64,
    pub cases_omitted: u64,
    pub evidence_complete: bool,
    pub cases: Vec<TestCase>,
    pub log_path: Option<String>,
    pub log_tail: Option<String>,
}

/// One test case read from a native test report.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TestCase {
    pub classname: Option<String>,
    pub name: String,
    pub outcome: String,
    pub message: Option<String>,
}

/// Failing cases one `test_outcome` event carries before it counts the rest.
pub const FAILING_CASE_LIMIT: usize = 10;

/// Longest case message a `test_outcome` event carries, in bytes.
pub const EVENT_MESSAGE_LIMIT: usize = 1024;

impl TestCase {
    fn is_failing(&self) -> bool {
        self.outcome == "failed" || self.outcome == "error"
    }
}

fn opt_str(map: &mut serde_json::Map<String, Value>, field: &str, value: &Option<String>) {
    if let Some(value) = value {
        map.insert(field.to_owned(), Value::String(value.clone()));
    }
}

fn opt_u64(map: &mut serde_json::Map<String, Value>, field: &str, value: Option<u64>) {
    if let Some(value) = value {
        map.insert(field.to_owned(), Value::from(value));
    }
}

fn cut(text: &str, limit: usize) -> &str {
    if text.len() <= limit {
        return text;
    }
    let mut end = limit;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

fn case_value(case: &TestCase, message_limit: usize) -> Value {
    let mut map = serde_json::Map::new();
    opt_str(&mut map, "classname", &case.classname);
    map.insert("name".to_owned(), Value::String(case.name.clone()));
    map.insert("outcome".to_owned(), Value::String(case.outcome.clone()));
    if let Some(message) = &case.message {
        map.insert(
            "message".to_owned(),
            Value::String(cut(message, message_limit).to_owned()),
        );
    }
    Value::Object(map)
}

fn outcome_fields(
    map: &mut serde_json::Map<String, Value>,
    outcome: &TestOutcome,
) -> Result<(), OutputError> {
    nonempty("target", &outcome.target)?;
    nonempty("outcome", &outcome.outcome)?;
    map.insert("target".to_owned(), Value::String(outcome.target.clone()));
    opt_str(map, "configuration", &outcome.configuration);
    map.insert("outcome".to_owned(), Value::String(outcome.outcome.clone()));
    opt_str(map, "status", &outcome.status);
    if let Some(cached) = outcome.cached {
        map.insert("cached".to_owned(), Value::Bool(cached));
    }
    opt_u64(map, "run", outcome.run.map(u64::from));
    opt_u64(map, "shard", outcome.shard.map(u64::from));
    opt_u64(map, "attempt", outcome.attempt.map(u64::from));
    if outcome.superseded {
        map.insert("final".to_owned(), Value::Bool(false));
    }
    opt_u64(map, "duration_millis", outcome.duration_millis);
    opt_u64(map, "cases_total", outcome.cases_total);
    opt_u64(map, "cases_failed", outcome.cases_failed.filter(|n| *n > 0));
    opt_u64(map, "cases_error", outcome.cases_error.filter(|n| *n > 0));
    opt_u64(
        map,
        "cases_skipped",
        outcome.cases_skipped.filter(|n| *n > 0),
    );
    let mut artifacts = serde_json::Map::new();
    if outcome.artifacts_missing > 0 {
        artifacts.insert("missing".to_owned(), Value::from(outcome.artifacts_missing));
    }
    if outcome.artifacts_invalid > 0 {
        artifacts.insert("invalid".to_owned(), Value::from(outcome.artifacts_invalid));
    }
    if !artifacts.is_empty() {
        map.insert("artifacts".to_owned(), Value::Object(artifacts));
    }
    if outcome.cases_omitted > 0 {
        map.insert(
            "cases_omitted".to_owned(),
            Value::from(outcome.cases_omitted),
        );
    }
    if !outcome.evidence_complete {
        map.insert("evidence_complete".to_owned(), Value::Bool(false));
    }
    Ok(())
}

/// One `test_outcome` event for a test result that did not plainly pass, with its failing cases and log tail.
pub fn test_outcome_event(outcome: &TestOutcome) -> Result<Value, OutputError> {
    let mut map = base("test_outcome");
    outcome_fields(&mut map, outcome)?;
    let failing: Vec<&TestCase> = outcome.cases.iter().filter(|c| c.is_failing()).collect();
    if !failing.is_empty() {
        map.insert(
            "failing_cases".to_owned(),
            Value::Array(
                failing
                    .iter()
                    .take(FAILING_CASE_LIMIT)
                    .map(|case| case_value(case, EVENT_MESSAGE_LIMIT))
                    .collect(),
            ),
        );
        if failing.len() > FAILING_CASE_LIMIT {
            map.insert(
                "failing_cases_omitted".to_owned(),
                Value::from(failing.len() - FAILING_CASE_LIMIT),
            );
        }
    }
    opt_str(&mut map, "log_path", &outcome.log_path);
    opt_str(&mut map, "log_tail", &outcome.log_tail);
    Ok(Value::Object(map))
}

/// One entry of the `results` report: the outcome and every case read for it.
pub fn test_result_record(outcome: &TestOutcome) -> Result<Value, OutputError> {
    let mut map = serde_json::Map::new();
    outcome_fields(&mut map, outcome)?;
    if !outcome.cases.is_empty() {
        map.insert(
            "cases".to_owned(),
            Value::Array(
                outcome
                    .cases
                    .iter()
                    .map(|case| case_value(case, usize::MAX))
                    .collect(),
            ),
        );
    }
    opt_str(&mut map, "log_path", &outcome.log_path);
    Ok(Value::Object(map))
}

/// A label, note or help attached to a failure diagnostic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FailureRelated {
    pub role: String,
    pub message: String,
    pub path: Option<String>,
    pub line: Option<u64>,
    pub column: Option<u64>,
    pub replacement: Option<String>,
    pub applicability: Option<String>,
}

/// One rustc diagnostic that Bazel's failed action produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FailureDiagnostic {
    pub severity: String,
    pub code: Option<String>,
    pub message: String,
    pub path: Option<String>,
    pub line: Option<u64>,
    pub column: Option<u64>,
    pub related: Vec<FailureRelated>,
}

/// One failure Bazel reported while building. `None` identity fields are unknown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildFailureEvent {
    pub target: Option<String>,
    pub configuration: Option<String>,
    pub target_kind: String,
    pub stage: String,
    pub category: String,
    pub mnemonic: Option<String>,
    pub primary_output: Option<String>,
    pub exit_code: Option<i64>,
    pub tool: Option<String>,
    pub message: Option<String>,
    pub diagnostics_status: String,
    pub diagnostics_total: u64,
    pub diagnostics: Vec<FailureDiagnostic>,
    pub count: u64,
    pub targets: Vec<String>,
}

pub const BUILD_FAILURE_RECORD_VERSION: u64 = 1;

fn known(value: &Option<String>) -> Value {
    Value::String(value.clone().unwrap_or_else(|| "unknown".to_owned()))
}

pub fn build_failure_event(failure: &BuildFailureEvent) -> Result<Value, OutputError> {
    nonempty("stage", &failure.stage)?;
    nonempty("category", &failure.category)?;
    nonempty("target_kind", &failure.target_kind)?;
    nonempty("diagnostics_status", &failure.diagnostics_status)?;
    let mut map = base("build_failure");
    map.insert(
        "record_version".to_owned(),
        Value::from(BUILD_FAILURE_RECORD_VERSION),
    );
    map.insert("target".to_owned(), known(&failure.target));
    map.insert("configuration".to_owned(), known(&failure.configuration));
    map.insert(
        "target_kind".to_owned(),
        Value::String(failure.target_kind.clone()),
    );
    map.insert("stage".to_owned(), Value::String(failure.stage.clone()));
    map.insert(
        "category".to_owned(),
        Value::String(failure.category.clone()),
    );
    let mut action = serde_json::Map::new();
    action.insert("mnemonic".to_owned(), known(&failure.mnemonic));
    action.insert("primary_output".to_owned(), known(&failure.primary_output));
    opt_i64(&mut action, "exit_code", failure.exit_code);
    map.insert("action".to_owned(), Value::Object(action));
    map.insert("tool".to_owned(), known(&failure.tool));
    opt_str(&mut map, "message", &failure.message);
    if failure.count > 1 {
        map.insert("count".to_owned(), Value::from(failure.count));
        if !failure.targets.is_empty() {
            map.insert(
                "targets".to_owned(),
                Value::Array(failure.targets.iter().cloned().map(Value::String).collect()),
            );
        }
    }
    let items = failure
        .diagnostics
        .iter()
        .map(|diagnostic| {
            let mut item = serde_json::Map::new();
            item.insert(
                "severity".to_owned(),
                Value::String(diagnostic.severity.clone()),
            );
            opt_str(&mut item, "code", &diagnostic.code);
            item.insert(
                "message".to_owned(),
                Value::String(diagnostic.message.clone()),
            );
            opt_str(&mut item, "path", &diagnostic.path);
            opt_u64(&mut item, "line", diagnostic.line);
            opt_u64(&mut item, "column", diagnostic.column);
            if !diagnostic.related.is_empty() {
                let related = diagnostic
                    .related
                    .iter()
                    .map(related_value)
                    .collect::<Result<Vec<_>, _>>()?;
                item.insert("related".to_owned(), Value::Array(related));
            }
            Ok(Value::Object(item))
        })
        .collect::<Result<Vec<Value>, OutputError>>()?;
    map.insert(
        "diagnostics".to_owned(),
        json!({
            "status": failure.diagnostics_status,
            "total": failure.diagnostics_total,
            "items": items,
        }),
    );
    Ok(Value::Object(map))
}

fn related_value(related: &FailureRelated) -> Result<Value, OutputError> {
    nonempty("related.role", &related.role)?;
    nonempty("related.message", &related.message)?;
    let mut item = serde_json::Map::new();
    item.insert("role".to_owned(), Value::String(related.role.clone()));
    item.insert("message".to_owned(), Value::String(related.message.clone()));
    opt_str(&mut item, "path", &related.path);
    opt_u64(&mut item, "line", related.line);
    opt_u64(&mut item, "column", related.column);
    opt_str(&mut item, "replacement", &related.replacement);
    opt_str(&mut item, "applicability", &related.applicability);
    Ok(Value::Object(item))
}

fn opt_i64(map: &mut serde_json::Map<String, Value>, field: &str, value: Option<i64>) {
    if let Some(value) = value {
        map.insert(field.to_owned(), Value::from(value));
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FinishedCounts {
    pub results_complete: Option<bool>,
    /// Ties `.dx/results.json` to the run that wrote it.
    pub run_id: Option<String>,
    pub diagnostics: Option<[u64; 3]>,
    pub changes: Option<[u64; 2]>,
    pub effects: Option<[u64; 2]>,
    pub baseline: Option<[u64; 3]>,
    /// Test result counts; see `TestCounts` for the unit.
    pub tests: Option<TestCounts>,
    /// Bazel action counts summed over the command's builds.
    pub actions: Option<ActionCounts>,
}

/// Bazel action counts; `executed` excludes disk and remote cache hits.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ActionCounts {
    pub created: u64,
    pub executed: u64,
    pub disk: u64,
    pub remote: u64,
    pub local: u64,
}

impl std::ops::Add for ActionCounts {
    type Output = ActionCounts;

    fn add(self, other: ActionCounts) -> ActionCounts {
        ActionCounts {
            created: self.created.saturating_add(other.created),
            executed: self.executed.saturating_add(other.executed),
            disk: self.disk.saturating_add(other.disk),
            remote: self.remote.saturating_add(other.remote),
            local: self.local.saturating_add(other.local),
        }
    }
}

impl ActionCounts {
    pub fn cached(&self) -> u64 {
        self.disk
            .saturating_add(self.remote)
            .saturating_add(self.local)
    }
}

/// Test result counts; `no_cases` is the part of `passed` whose report had no cases.
///
/// The unit is one result: one per target, configuration, run and shard, counted at its final
/// attempt. A target with 4 shards counts 4. `skipped` counts once per skipped or filtered target
/// and configuration.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TestCounts {
    pub passed: u64,
    pub failed: u64,
    pub cached: u64,
    pub no_cases: u64,
    pub skipped: u64,
}

pub fn command_finished(exit_code: i32, counts: &FinishedCounts) -> Value {
    let mut map = base("command_finished");
    map.insert("exit_code".to_owned(), Value::from(exit_code));
    if let Some(results_complete) = counts.results_complete {
        map.insert("results_complete".to_owned(), Value::Bool(results_complete));
    }
    if let Some(run_id) = &counts.run_id {
        map.insert("run_id".to_owned(), Value::from(run_id.as_str()));
    }
    if let Some([info, warning, error]) = counts.diagnostics {
        map.insert(
            "diagnostics".to_owned(),
            json!({"info": info, "warning": warning, "error": error}),
        );
    }
    if counts.changes.is_some() || counts.effects.is_some() {
        let [create, modify] = counts.changes.unwrap_or([0, 0]);
        let mut changes = serde_json::Map::new();
        changes.insert("create".to_owned(), Value::from(create));
        changes.insert("modify".to_owned(), Value::from(modify));
        if let Some([applied, refused]) = counts.effects {
            changes.insert("applied".to_owned(), Value::from(applied));
            changes.insert("refused".to_owned(), Value::from(refused));
        }
        map.insert("changes".to_owned(), Value::Object(changes));
    }
    if let Some([total, new, suppressed]) = counts.baseline {
        map.insert(
            "baseline".to_owned(),
            json!({"total": total, "new": new, "suppressed": suppressed}),
        );
    }
    if let Some(t) = &counts.tests {
        map.insert(
            "tests".to_owned(),
            json!({"passed": t.passed, "failed": t.failed, "cached": t.cached, "no_cases": t.no_cases, "skipped": t.skipped}),
        );
    }
    if let Some(a) = &counts.actions {
        map.insert(
            "actions".to_owned(),
            json!({"created": a.created, "executed": a.executed, "cache_hits": {"disk": a.disk, "remote": a.remote, "local": a.local}}),
        );
    }
    Value::Object(map)
}

pub fn write_event(writer: &mut dyn std::io::Write, event: &Value) -> Result<(), OutputError> {
    match event {
        Value::Object(map) => {
            let is_event =
                matches!(map.get("event"), Some(Value::String(_))) && map.contains_key("schema");
            if !is_event {
                return Err(OutputError::NotAnEvent);
            }
        }
        _ => return Err(OutputError::NotAnEvent),
    }
    let mut buf =
        serde_json::to_vec(event).map_err(|e| OutputError::from(std::io::Error::from(e)))?;
    buf.push(b'\n');
    writer.write_all(&buf).map_err(OutputError::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    #[test]
    fn failure_events_carry_cause_status_and_excerpt() {
        let excerpt = crate::diagnostics::Excerpt {
            text: "ERROR: boom".to_owned(),
            truncated: true,
        };
        let event = failure_event(
            "query_failed",
            "query failed",
            None,
            &FailureFacts {
                cause: "query",
                status: Some(7),
                excerpt: Some(&excerpt),
            },
        )
        .expect("event");
        assert_eq!(event["event"], "error");
        assert_eq!(event["cause"], "query");
        assert_eq!(event["status"], 7);
        assert_eq!(event["excerpt"], "ERROR: boom");
        assert_eq!(event["truncated"], true);
        let bare = failure_event(
            "invalid_scope",
            "bad scope",
            Some("resolve"),
            &FailureFacts {
                cause: "input",
                status: None,
                excerpt: None,
            },
        )
        .expect("event");
        assert_eq!(bare["phase"], "resolve");
        assert!(bare.get("status").is_none());
        assert!(bare.get("excerpt").is_none());
        assert!(bare.get("truncated").is_none());
        assert!(failure_event(
            "x",
            "y",
            None,
            &FailureFacts {
                cause: "",
                status: None,
                excerpt: None
            }
        )
        .is_err());
    }

    const DIGEST: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    #[test]
    fn command_started_shape() {
        let event = command_started("lint", "default").expect("started");
        assert_eq!(event["event"], Value::String("command_started".to_owned()));
        assert_eq!(event["schema"], schema());
        assert_eq!(event["mode"], Value::String("default".to_owned()));
        assert!(event.get("dry_run").is_none());
        assert!(command_started("lint", "fancy").is_err());
    }

    #[test]
    fn operation_scope_omitted_before_resolution() {
        let event = operation_event("lint", "resolve", None).expect("op");
        assert!(event.get("scope").is_none());
        let scope = vec!["//src/auth/...".to_owned()];
        let event = operation_event("lint", "execute", Some(&scope)).expect("op");
        assert_eq!(
            event["scope"][0],
            Value::String("//src/auth/...".to_owned())
        );
    }

    #[test]
    fn report_event_shape() {
        let event = report_event("sarif", "out.sarif", true).expect("report");
        assert_eq!(event["event"], Value::String("report".to_owned()));
        assert_eq!(event["format"], Value::String("sarif".to_owned()));
        assert_eq!(event["results_complete"], Value::Bool(true));
        assert!(report_event("", "out.sarif", true).is_err());
        assert!(report_event("sarif", "", true).is_err());
    }

    #[test]
    fn selection_event_shape() {
        let event = selection_event(DIGEST, DIGEST, DIGEST).expect("selection");
        assert_eq!(event["event"], Value::String("selection".to_owned()));
        assert_eq!(event["setup_id"], Value::String(DIGEST.to_owned()));
        assert!(selection_event("nope", DIGEST, DIGEST).is_err());
        assert!(selection_event(DIGEST, DIGEST, "nope").is_err());
    }

    #[test]
    fn error_event_never_carries_values() {
        let event = error_event(
            "conflicting_option",
            "keeps going",
            None,
            Some("--keep_going"),
            None,
        )
        .expect("error");
        assert_eq!(event["flag"], Value::String("--keep_going".to_owned()));
        assert!(event.get("path").is_none());
    }

    #[test]
    fn error_event_carries_optional_fields() {
        let event = error_event(
            "bazel_failed",
            "build broke",
            Some("src/app.py"),
            None,
            Some("execute"),
        )
        .expect("error");
        assert_eq!(event["path"], Value::String("src/app.py".to_owned()));
        assert_eq!(event["phase"], Value::String("execute".to_owned()));
        assert!(event.get("flag").is_none());
        assert!(error_event("", "m", None, None, None).is_err());
        assert!(error_event("c", "", None, None, None).is_err());
        assert!(error_event("c", "m", Some("/abs"), None, None).is_err());
        assert!(error_event("c", "m", None, Some(""), None).is_err());
        assert!(error_event("c", "m", None, None, Some("")).is_err());
    }

    #[test]
    fn finished_counts_render_exactly() {
        let counts = FinishedCounts {
            results_complete: Some(true),
            run_id: Some("7-1-abc".to_owned()),
            diagnostics: Some([0, 3, 1]),
            changes: Some([0, 2]),
            effects: Some([1, 1]),
            baseline: None,
            tests: None,
            actions: None,
        };
        let event = command_finished(1, &counts);
        assert_eq!(event["exit_code"], Value::from(1));
        assert_eq!(event["run_id"], Value::from("7-1-abc"));
        assert_eq!(event["diagnostics"]["warning"], Value::from(3));
        assert_eq!(
            event["changes"],
            serde_json::json!({"create": 0, "modify": 2, "applied": 1, "refused": 1})
        );
        assert!(event.get("mutations").is_none());
        assert!(event.get("baseline").is_none());
        let bare = command_finished(0, &FinishedCounts::default());
        assert!(bare.get("changes").is_none());
        assert!(bare.get("run_id").is_none());
    }

    #[test]
    fn command_finished_reports_action_counts() {
        let counts = FinishedCounts {
            actions: Some(ActionCounts {
                created: 9,
                executed: 4,
                disk: 1,
                remote: 2,
                local: 3,
            }),
            ..FinishedCounts::default()
        };
        assert_eq!(
            command_finished(0, &counts)["actions"],
            serde_json::json!({"created": 9, "executed": 4, "cache_hits": {"disk": 1, "remote": 2, "local": 3}})
        );
        assert!(command_finished(0, &FinishedCounts::default())
            .get("actions")
            .is_none());
        let sum = ActionCounts {
            created: 1,
            executed: 2,
            disk: 3,
            remote: 4,
            local: 5,
        } + ActionCounts {
            created: 1,
            executed: 1,
            disk: 1,
            remote: 1,
            local: 1,
        };
        assert_eq!(sum.executed, 3);
        assert_eq!(sum.cached(), 15);
    }

    #[test]
    fn finished_changes_render() {
        let counts = FinishedCounts {
            results_complete: None,
            run_id: None,
            diagnostics: None,
            changes: Some([1, 2]),
            effects: None,
            baseline: Some([5, 1, 4]),
            tests: None,
            actions: None,
        };
        let event = command_finished(0, &counts);
        assert_eq!(event["changes"]["create"], Value::from(1));
        assert_eq!(event["changes"]["modify"], Value::from(2));
        assert!(event["changes"].get("applied").is_none());
        assert!(event.get("diagnostics").is_none());
        assert_eq!(event["baseline"]["total"], Value::from(5));
        assert_eq!(event["baseline"]["new"], Value::from(1));
        assert_eq!(event["baseline"]["suppressed"], Value::from(4));
    }

    #[test]
    fn writer_rejects_prose() {
        let mut buf = Vec::new();
        let event = command_started("lint", "default").expect("started");
        write_event(&mut buf, &event).expect("write");
        assert!(buf.ends_with(b"\n"));
        assert_eq!(buf.iter().filter(|b| **b == b'\n').count(), 1);
        let prose = Value::String("Running lint".to_owned());
        assert_eq!(
            write_event(&mut Vec::new(), &prose).expect_err("prose rejected"),
            OutputError::NotAnEvent
        );
    }

    #[test]
    fn writer_rejects_schemaless_object() {
        let fake = serde_json::json!({"event": "command_started"});
        assert_eq!(
            write_event(&mut Vec::new(), &fake).expect_err("no schema"),
            OutputError::NotAnEvent
        );
    }

    #[test]
    fn capability_event_shape() {
        let capability = CapabilityEvent {
            name: "lint".to_owned(),
            source: "cli-grammar".to_owned(),
            availability: "available".to_owned(),
            detail: "run lint analysis".to_owned(),
            flags: vec!["--check".to_owned()],
            outputs: vec!["text".to_owned(), "json".to_owned()],
            reports: vec!["sarif".to_owned()],
            scope_policy: "default-//...".to_owned(),
            effect: "apply".to_owned(),
            passthrough: true,
        };
        let event = capability_event(&capability).expect("capability");
        assert_eq!(event["event"], Value::String("capability".to_owned()));
        assert_eq!(event["schema"], schema());
        assert_eq!(event["name"], Value::String("lint".to_owned()));
        assert_eq!(event["flags"][0], Value::String("--check".to_owned()));
        assert_eq!(
            event,
            capability_value(&capability),
            "the checked entry point must agree with the value builder"
        );
        let mut buf = Vec::new();
        write_event(&mut buf, &event).expect("write");
        let parsed: Value = serde_json::from_slice(&buf).expect("parse");
        assert_eq!(
            parsed["availability"],
            Value::String("available".to_owned())
        );
        let mut with_unknown = event.clone();
        with_unknown["reader_future"] = Value::String("ignored".to_owned());
        assert_eq!(
            with_unknown["name"],
            Value::String("lint".to_owned()),
            "a new field must not move the fields a reader owns"
        );
        let mut bad = capability.clone();
        bad.source = "probed".to_owned();
        assert!(capability_event(&bad).is_err());
        bad.source = "cli-grammar".to_owned();
        bad.availability = "maybe".to_owned();
        assert!(capability_event(&bad).is_err());
        bad.availability = "available".to_owned();
        bad.name = String::new();
        assert!(capability_event(&bad).is_err());
    }

    #[test]
    fn cache_miss_event_shape() {
        let miss = CacheMissEvent {
            mnemonic: "Rustc".to_owned(),
            target: "//cli/digest:dx_digest".to_owned(),
            primary_output: "bazel-out/k8/bin/cli/digest/libdx_digest.rlib".to_owned(),
            changes: vec![
                CacheMissChange {
                    kind: "input".to_owned(),
                    change: "modified".to_owned(),
                    subject: Some("cli/digest/src/lib.rs".to_owned()),
                },
                CacheMissChange {
                    kind: "env".to_owned(),
                    change: "added".to_owned(),
                    subject: Some("RUSTFLAGS".to_owned()),
                },
                CacheMissChange {
                    kind: "arg".to_owned(),
                    change: "order_changed".to_owned(),
                    subject: None,
                },
                CacheMissChange {
                    kind: "not_cacheable".to_owned(),
                    change: String::new(),
                    subject: None,
                },
            ],
            changes_omitted: 3,
        };
        let event = cache_miss_event(&miss).expect("event");
        assert_eq!(
            event,
            serde_json::json!({
                "schema": {"major": 3, "minor": 4},
                "event": "cache_miss",
                "mnemonic": "Rustc",
                "target": "//cli/digest:dx_digest",
                "primary_output": "bazel-out/k8/bin/cli/digest/libdx_digest.rlib",
                "changes": [
                    {"kind": "input", "change": "modified", "path": "cli/digest/src/lib.rs"},
                    {"kind": "env", "change": "added", "name": "RUSTFLAGS"},
                    {"kind": "arg", "change": "order_changed"},
                    {"kind": "not_cacheable"},
                ],
                "changes_omitted": 3,
            })
        );
        assert!(EVENTS.contains(&"cache_miss"));
        let mut bad = miss.clone();
        bad.mnemonic.clear();
        assert!(cache_miss_event(&bad).is_err());
        let mut bad = miss.clone();
        bad.primary_output.clear();
        assert!(cache_miss_event(&bad).is_err());
        let mut bad = miss.clone();
        bad.changes[0].kind = "mystery".to_owned();
        assert!(cache_miss_event(&bad).is_err());
        let mut bad = miss.clone();
        bad.changes[0].change = "renamed".to_owned();
        assert!(cache_miss_event(&bad).is_err());
        let mut bad = miss.clone();
        bad.changes[0].subject = None;
        assert!(cache_miss_event(&bad).is_err());
        let mut bad = miss;
        bad.changes[3].change = "added".to_owned();
        assert!(cache_miss_event(&bad).is_err());
    }

    #[test]
    fn slot_event_shape() {
        let event = slot_event(&SlotEvent {
            action: "list".to_owned(),
            path: "/pool/repo/1".to_owned(),
            slot: Some(1),
            branch: Some("topic".to_owned()),
            holder: None,
            state: "free".to_owned(),
            reason: None,
            base: Some("origin/main".to_owned()),
            server: false,
            disk_bytes: Some(12),
        })
        .expect("event");
        assert_eq!(event["event"], "slot");
        assert_eq!(event["slot"], 1);
        assert_eq!(event["branch"], "topic");
        assert_eq!(event["disk_bytes"], 12);
        assert_eq!(event["base"], "origin/main");
        assert!(event.get("holder").is_none());
        let unmeasured = slot_event(&SlotEvent {
            action: "list".to_owned(),
            path: "/pool/repo/1".to_owned(),
            slot: Some(1),
            branch: None,
            holder: None,
            state: "free".to_owned(),
            reason: None,
            base: None,
            server: false,
            disk_bytes: None,
        })
        .expect("event");
        assert!(unmeasured.get("disk_bytes").is_none());
        assert!(unmeasured.get("base").is_none());
        let bad = slot_event(&SlotEvent {
            action: "list".to_owned(),
            path: String::new(),
            slot: None,
            branch: None,
            holder: None,
            state: "free".to_owned(),
            reason: None,
            base: None,
            server: false,
            disk_bytes: None,
        });
        assert!(bad.is_err());
    }

    #[test]
    fn summary_event_shape() {
        let event = summary_event(&SummaryEvent {
            slots: 2,
            servers: 3,
            servers_rss_bytes: 4096,
            disk_bytes: Some(8192),
            disk_cache_bytes: Some(16384),
        });
        assert_eq!(event["event"], "summary");
        assert_eq!(event["servers"], 3);
        assert_eq!(event["servers_rss_bytes"], 4096);
        assert_eq!(event["disk_bytes"], 8192);
        assert_eq!(event["disk_cache_bytes"], 16384);
        let unmeasured = summary_event(&SummaryEvent {
            slots: 2,
            servers: 0,
            servers_rss_bytes: 0,
            disk_bytes: None,
            disk_cache_bytes: None,
        });
        assert!(unmeasured.get("disk_bytes").is_none());
        assert!(unmeasured.get("disk_cache_bytes").is_none());
        assert_eq!(unmeasured["slots"], 2);
    }

    #[test]
    fn status_event_shape() {
        let event = status_event(&StatusEvent {
            name: "pin".to_owned(),
            status: "ok".to_owned(),
            detail: "dx 0.0.0 vs module 0.0.0".to_owned(),
            hint: "dx version --check".to_owned(),
        })
        .expect("status");
        assert_eq!(event["event"], Value::String("status".to_owned()));
        assert_eq!(event["schema"], schema());
        assert_eq!(event["name"], Value::String("pin".to_owned()));
        assert_eq!(event["status"], Value::String("ok".to_owned()));
        let mut bad = StatusEvent {
            name: "pin".to_owned(),
            status: "bogus".to_owned(),
            detail: "d".to_owned(),
            hint: "h".to_owned(),
        };
        assert!(status_event(&bad).is_err());
        bad.status = String::new();
        assert!(status_event(&bad).is_err());
        assert!(status_event(&StatusEvent {
            name: String::new(),
            status: "ok".to_owned(),
            detail: "d".to_owned(),
            hint: "h".to_owned(),
        })
        .is_err());
        let mut buf = Vec::new();
        write_event(&mut buf, &event).expect("write");
        assert!(buf.ends_with(b"\n"));
    }

    #[test]
    fn correlation_attaches_and_validates() {
        let operation =
            operation_event("run", "execute", Some(&["//app:bin".to_owned()])).expect("operation");
        assert!(operation.get("correlation").is_none());
        let correlated = with_correlation(operation, "run://app:bin").expect("correlation");
        assert_eq!(
            correlated["correlation"],
            Value::String("run://app:bin".to_owned())
        );
        assert_eq!(correlated["schema"], schema());
        let mut buf = Vec::new();
        write_event(&mut buf, &correlated).expect("write correlated");
        let parsed: Value = serde_json::from_slice(&buf).expect("parse");
        assert_eq!(
            parsed["correlation"],
            Value::String("run://app:bin".to_owned())
        );
        let operation = operation_event("update", "execute", None).expect("op");
        assert!(with_correlation(operation.clone(), "").is_err());
        assert!(with_correlation(operation, "has space").is_err());
        let prose = Value::String("Running lint".to_owned());
        assert!(with_correlation(prose, "update:cargo").is_err());
    }

    #[test]
    fn schema_is_three_three_with_forward_compat() {
        assert_eq!(SCHEMA_MAJOR, 3);
        assert_eq!(SCHEMA_MINOR, 4);
        assert_eq!(schema(), serde_json::json!({"major": 3, "minor": 4}));
        assert!(!EVENTS.contains(&"test_case"));
        assert!(!EVENTS.contains(&"mutation"));
        let correlated = with_correlation(
            operation_event("run", "execute", None).expect("op"),
            "run://a:bin",
        )
        .expect("correlation");
        let reparsed: serde_json::Map<String, Value> =
            serde_json::from_value(correlated).expect("map");
        assert_eq!(
            reparsed.get("command"),
            Some(&Value::String("run".to_owned()))
        );
        assert_eq!(
            reparsed.get("correlation"),
            Some(&Value::String("run://a:bin".to_owned()))
        );
        let bare = operation_event("run", "execute", None).expect("bare");
        assert!(bare.get("correlation").is_none());
    }

    #[test]
    fn readers_reject_other_schema_majors() {
        let line = serde_json::to_string(&command_started("lint", "check").expect("started"))
            .expect("line");
        assert_eq!(
            read_event(&line).expect("current")["event"],
            "command_started"
        );
        let newer = r#"{"schema":{"major":3,"minor":7},"event":"future","added":1}"#;
        assert_eq!(read_event(newer).expect("newer minor")["added"], 1);
        assert_eq!(
            read_event(r#"{"schema":{"major":1,"minor":1},"event":"command_started"}"#)
                .expect_err("old major"),
            OutputError::UnsupportedSchema { major: 1, minor: 1 }
        );
        assert_eq!(
            read_event(r#"{"schema":{"major":2,"minor":0},"event":"x"}"#).expect_err("old major"),
            OutputError::UnsupportedSchema { major: 2, minor: 0 }
        );
        assert_eq!(
            read_event(r#"{"schema":{"major":4,"minor":0},"event":"x"}"#).expect_err("new major"),
            OutputError::UnsupportedSchema { major: 4, minor: 0 }
        );
        for bad in [
            "",
            "Running lint",
            "[]",
            r#"{"event":"x"}"#,
            r#"{"schema":{"major":3},"event":"x"}"#,
            r#"{"schema":{"major":3,"minor":0}}"#,
        ] {
            assert_eq!(read_event(bad).expect_err(bad), OutputError::NotAnEvent);
        }
    }

    #[test]
    fn nested_phases_never_finish_the_parent() {
        let started = command_started("format", "check").expect("started");
        let nested = nest_phase_event(started, "check", "format", "check/format").expect("nest");
        assert_eq!(nested["event"], "phase_started");
        assert_eq!(nested["command"], "check");
        assert_eq!(nested["phase"], "format");
        assert_eq!(nested["mode"], "check");
        assert_eq!(nested["correlation"], "check/format");
        let finished = command_finished(
            1,
            &FinishedCounts {
                results_complete: Some(true),
                ..FinishedCounts::default()
            },
        );
        let nested = nest_phase_event(finished, "check", "format", "check/format").expect("nest");
        assert_eq!(nested["event"], "phase_finished");
        assert_eq!(nested["exit_code"], 1);
        assert_eq!(nested["results_complete"], true);
        let report = report_event("sarif", "out.sarif", true).expect("report");
        let nested = nest_phase_event(report, "check", "lint", "check/lint").expect("nest");
        assert_eq!(nested["event"], "report");
        assert!(nested.get("phase").is_none());
        assert_eq!(nested["correlation"], "check/lint");
        let twice = nest_phase_event(
            nest_phase_event(
                command_started("lint", "check").expect("started"),
                "check",
                "lint",
                "check/lint",
            )
            .expect("nest"),
            "check",
            "lint",
            "check/lint",
        );
        assert_eq!(twice.expect_err("nested twice"), OutputError::NotAnEvent);
        assert!(nest_phase_event(Value::Null, "check", "lint", "check/lint").is_err());
        assert!(nest_phase_event(schema(), "check", "lint", "check/lint").is_err());
        assert!(nest_phase_event(
            command_finished(0, &FinishedCounts::default()),
            "",
            "lint",
            "c"
        )
        .is_err());
    }

    #[test]
    fn test_outcome_shape_omits_defaults() {
        let event = test_outcome_event(&TestOutcome {
            target: "//a:t".to_owned(),
            outcome: "failed".to_owned(),
            cached: Some(false),
            attempt: Some(2),
            duration_millis: Some(29),
            cases_total: Some(3),
            cases_failed: Some(1),
            cases_error: Some(0),
            cases_skipped: Some(0),
            artifacts_collected: 1,
            evidence_complete: true,
            cases: vec![
                TestCase {
                    name: "ok".to_owned(),
                    outcome: "passed".to_owned(),
                    ..TestCase::default()
                },
                TestCase {
                    classname: Some("suite".to_owned()),
                    name: "fails".to_owned(),
                    outcome: "failed".to_owned(),
                    message: Some("boom".to_owned()),
                },
            ],
            log_path: Some("/out/test.log".to_owned()),
            log_tail: Some("last\n".to_owned()),
            ..TestOutcome::default()
        })
        .expect("outcome");
        assert_eq!(event["event"], "test_outcome");
        assert_eq!(event["outcome"], "failed");
        assert_eq!(event["attempt"], 2);
        assert_eq!(event["cases_failed"], 1);
        assert_eq!(event["failing_cases"].as_array().expect("cases").len(), 1);
        assert_eq!(event["failing_cases"][0]["name"], "fails");
        assert_eq!(event["failing_cases"][0]["classname"], "suite");
        assert_eq!(event["failing_cases"][0]["message"], "boom");
        assert_eq!(event["log_path"], "/out/test.log");
        assert_eq!(event["log_tail"], "last\n");
        for field in [
            "configuration",
            "status",
            "cases_error",
            "cases_skipped",
            "artifacts",
            "evidence_complete",
            "cases_omitted",
            "cases",
            "final",
        ] {
            assert!(event.get(field).is_none(), "{field} is left out");
        }
        let replaced = TestOutcome {
            target: "//a:t".to_owned(),
            outcome: "failed".to_owned(),
            attempt: Some(1),
            superseded: true,
            evidence_complete: true,
            ..TestOutcome::default()
        };
        assert_eq!(
            test_outcome_event(&replaced).expect("event")["final"],
            false
        );
        assert_eq!(
            test_result_record(&replaced).expect("record")["final"],
            false
        );
        let incomplete = test_outcome_event(&TestOutcome {
            target: "//a:t".to_owned(),
            outcome: "unknown".to_owned(),
            artifacts_missing: 1,
            cases_omitted: 3,
            ..TestOutcome::default()
        })
        .expect("incomplete");
        assert_eq!(incomplete["artifacts"], serde_json::json!({"missing": 1}));
        assert_eq!(incomplete["evidence_complete"], false);
        assert_eq!(incomplete["cases_omitted"], 3);
        assert!(test_outcome_event(&TestOutcome::default()).is_err());
        assert!(test_outcome_event(&TestOutcome {
            target: "//a:t".to_owned(),
            ..TestOutcome::default()
        })
        .is_err());
    }

    #[test]
    fn failing_cases_are_bounded_in_the_event_but_complete_in_the_record() {
        let cases: Vec<TestCase> = (0..12)
            .map(|n| TestCase {
                name: format!("case{n}"),
                outcome: "failed".to_owned(),
                message: Some("x".repeat(EVENT_MESSAGE_LIMIT + 10)),
                ..TestCase::default()
            })
            .collect();
        let outcome = TestOutcome {
            target: "//a:t".to_owned(),
            outcome: "failed".to_owned(),
            evidence_complete: true,
            cases,
            ..TestOutcome::default()
        };
        let event = test_outcome_event(&outcome).expect("event");
        assert_eq!(
            event["failing_cases"].as_array().expect("cases").len(),
            FAILING_CASE_LIMIT
        );
        assert_eq!(event["failing_cases_omitted"], 2);
        assert_eq!(
            event["failing_cases"][0]["message"]
                .as_str()
                .expect("message")
                .len(),
            EVENT_MESSAGE_LIMIT
        );
        let record = test_result_record(&outcome).expect("record");
        assert!(record.get("event").is_none());
        assert!(record.get("schema").is_none());
        assert_eq!(record["cases"].as_array().expect("cases").len(), 12);
        assert_eq!(
            record["cases"][0]["message"].as_str().expect("m").len(),
            EVENT_MESSAGE_LIMIT + 10
        );
    }

    #[test]
    fn cut_stops_on_a_char_boundary() {
        for wide in ["\u{e9}", "\u{20ac}", "\u{1f30d}"] {
            let s = format!("{}{wide}tail", "x".repeat(EVENT_MESSAGE_LIMIT - 1));
            let out = cut(&s, EVENT_MESSAGE_LIMIT);
            assert_eq!(out, "x".repeat(EVENT_MESSAGE_LIMIT - 1), "{wide}");
            assert!(out.len() <= EVENT_MESSAGE_LIMIT);
        }
        let exact = format!("{}\u{e9}tail", "x".repeat(EVENT_MESSAGE_LIMIT - 2));
        let out = cut(&exact, EVENT_MESSAGE_LIMIT);
        assert_eq!(out.len(), EVENT_MESSAGE_LIMIT);
        assert!(out.ends_with('\u{e9}'));
        assert_eq!(cut("", 5), "");
        assert_eq!(cut("abc", 0), "");
        assert_eq!(cut("\u{e9}", 1), "");
    }

    #[test]
    fn a_multibyte_case_message_straddling_the_limit_is_cut_whole() {
        let message = format!("{}\u{1f30d}tail", "x".repeat(EVENT_MESSAGE_LIMIT - 2));
        let outcome = TestOutcome {
            target: "//a:t".to_owned(),
            outcome: "failed".to_owned(),
            evidence_complete: true,
            cases: vec![TestCase {
                name: "wide".to_owned(),
                outcome: "failed".to_owned(),
                message: Some(message.clone()),
                ..TestCase::default()
            }],
            ..TestOutcome::default()
        };
        let event = test_outcome_event(&outcome).expect("event");
        let json = serde_json::to_string(&event).expect("json");
        let parsed: Value = serde_json::from_str(&json).expect("valid json");
        let cut_message = parsed["failing_cases"][0]["message"]
            .as_str()
            .expect("message");
        assert!(cut_message.len() <= EVENT_MESSAGE_LIMIT);
        assert!(message.starts_with(cut_message));
        assert!(!cut_message.contains('\u{1f30d}'));
        assert!(!cut_message.contains('\u{fffd}'));
        assert_eq!(cut_message, "x".repeat(EVENT_MESSAGE_LIMIT - 2));
        let record = test_result_record(&outcome).expect("record");
        assert_eq!(
            record["cases"][0]["message"].as_str(),
            Some(message.as_str())
        );
    }

    #[test]
    fn finished_counts_report_passed_and_cached_tests() {
        let counts = |tests| {
            command_finished(
                0,
                &FinishedCounts {
                    tests,
                    ..FinishedCounts::default()
                },
            )
        };
        assert_eq!(
            counts(Some(TestCounts {
                passed: 5,
                failed: 1,
                cached: 2,
                no_cases: 3,
                skipped: 4
            }))["tests"],
            serde_json::json!({"passed": 5, "failed": 1, "cached": 2, "no_cases": 3, "skipped": 4})
        );
        assert_eq!(
            counts(Some(TestCounts::default()))["tests"],
            serde_json::json!({"passed": 0, "failed": 0, "cached": 0, "no_cases": 0, "skipped": 0})
        );
        assert!(counts(None).get("tests").is_none());
    }

    #[test]
    fn write_event_reports_broken_pipe() {
        struct BrokenPipe;
        impl std::io::Write for BrokenPipe {
            fn write(&mut self, _buf: &[u8]) -> std::io::Result<usize> {
                Err(std::io::Error::new(
                    std::io::ErrorKind::BrokenPipe,
                    "broken pipe",
                ))
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Err(std::io::Error::new(
                    std::io::ErrorKind::BrokenPipe,
                    "broken pipe",
                ))
            }
        }
        let event = command_started("status", "default").expect("started");
        let error = write_event(&mut BrokenPipe, &event).expect_err("broken pipe");
        assert!(error.is_broken_pipe());
    }
}

#[cfg(test)]
mod build_failure_tests {
    use super::*;

    fn sample() -> BuildFailureEvent {
        BuildFailureEvent {
            target: Some("//a:lib".to_owned()),
            configuration: None,
            target_kind: "other".to_owned(),
            stage: "compilation".to_owned(),
            category: "compile_failed".to_owned(),
            mnemonic: Some("Rustc".to_owned()),
            primary_output: None,
            exit_code: Some(1),
            tool: Some("rustc".to_owned()),
            message: None,
            diagnostics_status: "parsed".to_owned(),
            diagnostics_total: 1,
            diagnostics: vec![FailureDiagnostic {
                severity: "error".to_owned(),
                code: Some("E0425".to_owned()),
                message: "cannot find value".to_owned(),
                path: Some("a/lib.rs".to_owned()),
                line: Some(3),
                column: None,
                related: Vec::new(),
            }],
            count: 1,
            targets: Vec::new(),
        }
    }

    #[test]
    fn missing_facts_are_left_out() {
        let event = build_failure_event(&sample()).expect("event");
        assert_eq!(event["event"], "build_failure");
        assert_eq!(event["record_version"], 1);
        assert_eq!(event["configuration"], "unknown");
        assert_eq!(event["action"]["primary_output"], "unknown");
        assert!(event.get("count").is_none());
        assert_eq!(event["action"]["exit_code"], 1);
        assert_eq!(event["diagnostics"]["items"][0]["code"], "E0425");
        assert!(event["diagnostics"]["items"][0].get("column").is_none());
    }

    fn related(role: &str, message: &str) -> FailureRelated {
        FailureRelated {
            role: role.to_owned(),
            message: message.to_owned(),
            path: None,
            line: None,
            column: None,
            replacement: None,
            applicability: None,
        }
    }

    #[test]
    fn related_items_keep_their_order() {
        let mut failure = sample();
        let mut help = related("help", "use `x`");
        help.path = Some("a/lib.rs".to_owned());
        help.line = Some(3);
        help.replacement = Some("x".to_owned());
        help.applicability = Some("MaybeIncorrect".to_owned());
        failure.diagnostics[0].related = vec![related("label", "first"), help];
        let event = build_failure_event(&failure).expect("event");
        let items = &event["diagnostics"]["items"][0]["related"];
        assert_eq!(items[0]["role"], "label");
        assert_eq!(items[0]["message"], "first");
        assert!(items[0].get("path").is_none());
        assert_eq!(items[1]["role"], "help");
        assert_eq!(items[1]["path"], "a/lib.rs");
        assert_eq!(items[1]["line"], 3);
        assert!(items[1].get("column").is_none());
        assert_eq!(items[1]["replacement"], "x");
        assert_eq!(items[1]["applicability"], "MaybeIncorrect");
    }

    #[test]
    fn empty_related_is_left_out_and_empty_role_is_refused() {
        let mut failure = sample();
        let event = build_failure_event(&failure).expect("event");
        assert!(event["diagnostics"]["items"][0].get("related").is_none());
        failure.diagnostics[0].related = vec![related("", "text")];
        assert!(build_failure_event(&failure).is_err());
    }

    #[test]
    fn repeated_failures_carry_a_count_and_their_targets() {
        let mut failure = sample();
        failure.count = 3;
        failure.targets = vec!["//b:lib".to_owned(), "//c:lib".to_owned()];
        let event = build_failure_event(&failure).expect("event");
        assert_eq!(event["count"], 3);
        assert_eq!(event["targets"], serde_json::json!(["//b:lib", "//c:lib"]));
        failure.mnemonic = None;
        failure.exit_code = None;
        failure.diagnostics_status = "unknown".to_owned();
        let bare = build_failure_event(&failure).expect("bare");
        assert_eq!(bare["action"]["mnemonic"], "unknown");
        assert_eq!(bare["diagnostics"]["status"], "unknown");
    }

    #[test]
    fn an_empty_stage_is_rejected() {
        let mut failure = sample();
        failure.stage.clear();
        assert!(build_failure_event(&failure).is_err());
    }

    fn coverage_fixture() -> CoverageEvent {
        CoverageEvent {
            cell: "linux_x86_64".to_owned(),
            covered_lines: 7,
            eligible_lines: 8,
            raw_lines: 10,
            excluded_lines: 2,
            eligible_files: 3,
            support_files: 4,
            excluded_files: 1,
            unsupported_files: 5,
            minimum: Some(100),
            passed: false,
            errors: 2,
            dx_version: "1.2.3".to_owned(),
        }
    }

    #[test]
    fn coverage_event_carries_every_count() {
        let event = coverage_event(&coverage_fixture()).expect("coverage");
        assert_eq!(
            event,
            serde_json::json!({
                "schema": {"major": 3, "minor": 4},
                "event": "coverage",
                "cell": "linux_x86_64",
                "covered_lines": 7,
                "eligible_lines": 8,
                "raw_lines": 10,
                "excluded_lines": 2,
                "eligible_files": 3,
                "support_files": 4,
                "excluded_files": 1,
                "unsupported_files": 5,
                "minimum": 100,
                "passed": false,
                "errors": 2,
                "dx_version": "1.2.3",
            })
        );
    }

    fn changed_fixture() -> ChangedLineCoverageEvent {
        ChangedLineCoverageEvent {
            base: "origin/main".to_owned(),
            merge_base: "abc123".to_owned(),
            minimum: 80,
            covered_lines: 3,
            eligible_lines: 4,
            uncovered: vec![("src/a.rs".to_owned(), 7)],
            uninstrumented: vec!["src/b.rs".to_owned()],
            result: "incomplete".to_owned(),
            dx_version: "1.2.3".to_owned(),
        }
    }

    #[test]
    fn changed_line_coverage_event_carries_the_gate() {
        let event = changed_line_coverage_event(&changed_fixture()).expect("event");
        assert_eq!(
            event,
            serde_json::json!({
                "schema": {"major": 3, "minor": 4},
                "event": "changed_line_coverage",
                "base": "origin/main",
                "merge_base": "abc123",
                "minimum": 80,
                "covered_lines": 3,
                "eligible_lines": 4,
                "uncovered": [{"path": "src/a.rs", "line": 7}],
                "uninstrumented": ["src/b.rs"],
                "result": "incomplete",
                "dx_version": "1.2.3",
            })
        );
        assert!(EVENTS.contains(&"changed_line_coverage"));
    }

    #[test]
    fn changed_line_coverage_event_rejects_bad_fields() {
        let mut gate = changed_fixture();
        gate.dx_version.clear();
        assert!(changed_line_coverage_event(&gate).is_err());
        let mut gate = changed_fixture();
        gate.result = "maybe".to_owned();
        assert!(changed_line_coverage_event(&gate).is_err());
        let mut gate = changed_fixture();
        gate.minimum = 101;
        assert!(changed_line_coverage_event(&gate).is_err());
        let mut gate = changed_fixture();
        gate.covered_lines = 5;
        assert!(changed_line_coverage_event(&gate).is_err());
        let mut gate = changed_fixture();
        gate.uncovered = vec![(String::new(), 1)];
        assert!(changed_line_coverage_event(&gate).is_err());
        let mut gate = changed_fixture();
        gate.uninstrumented = vec![String::new()];
        assert!(changed_line_coverage_event(&gate).is_err());
        let mut gate = changed_fixture();
        gate.result = "unavailable".to_owned();
        gate.base.clear();
        gate.merge_base.clear();
        assert!(changed_line_coverage_event(&gate).is_ok());
    }

    #[test]
    fn coverage_event_omits_an_absent_minimum_and_rejects_empty_names() {
        let mut coverage = coverage_fixture();
        coverage.minimum = None;
        let event = coverage_event(&coverage).expect("coverage");
        assert!(event.get("minimum").is_none(), "{event}");
        coverage.cell.clear();
        assert!(coverage_event(&coverage).is_err());
        let mut coverage = coverage_fixture();
        coverage.dx_version.clear();
        assert!(coverage_event(&coverage).is_err());
    }
}
