use crate::validation::{check_correlation, check_path, parse_digest, OutputError};
use serde_json::{json, Value};

pub use dx_schema::SCHEMA_MAJOR;
pub use dx_schema::SCHEMA_MINOR;

pub const EVENTS: &[&str] = &[
    "baseline",
    "capability",
    "change",
    "command_finished",
    "command_started",
    "diagnostic",
    "error",
    "mutation",
    "notice",
    "operation",
    "report",
    "selection",
    "status",
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

pub fn command_started(command: &str, dry_run: bool, mode: &str) -> Result<Value, OutputError> {
    nonempty("command", command)?;
    if mode != "default" && mode != "check" {
        return Err(OutputError::BadCommandMode {
            value: mode.to_owned(),
        });
    }
    let mut map = base("command_started");
    map.insert("command".to_owned(), Value::String(command.to_owned()));
    map.insert("dry_run".to_owned(), Value::Bool(dry_run));
    map.insert("mode".to_owned(), Value::String(mode.to_owned()));
    Ok(Value::Object(map))
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
    pub duration_millis: Option<u64>,
    pub cases_total: Option<u64>,
    pub cases_failed: Option<u64>,
    pub cases_error: Option<u64>,
    pub cases_skipped: Option<u64>,
    pub artifacts_collected: u64,
    pub artifacts_missing: u64,
    pub artifacts_invalid: u64,
    pub evidence_complete: bool,
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

pub fn test_outcome_event(outcome: &TestOutcome) -> Result<Value, OutputError> {
    nonempty("target", &outcome.target)?;
    nonempty("outcome", &outcome.outcome)?;
    let mut map = base("test_outcome");
    map.insert("target".to_owned(), Value::String(outcome.target.clone()));
    opt_str(&mut map, "configuration", &outcome.configuration);
    map.insert("outcome".to_owned(), Value::String(outcome.outcome.clone()));
    opt_str(&mut map, "status", &outcome.status);
    if let Some(cached) = outcome.cached {
        map.insert("cached".to_owned(), Value::Bool(cached));
    }
    opt_u64(&mut map, "run", outcome.run.map(u64::from));
    opt_u64(&mut map, "shard", outcome.shard.map(u64::from));
    opt_u64(&mut map, "attempt", outcome.attempt.map(u64::from));
    opt_u64(&mut map, "duration_millis", outcome.duration_millis);
    opt_u64(&mut map, "cases_total", outcome.cases_total);
    opt_u64(&mut map, "cases_failed", outcome.cases_failed);
    opt_u64(&mut map, "cases_error", outcome.cases_error);
    opt_u64(&mut map, "cases_skipped", outcome.cases_skipped);
    map.insert(
        "artifacts".to_owned(),
        serde_json::json!({
            "collected": outcome.artifacts_collected,
            "missing": outcome.artifacts_missing,
            "invalid": outcome.artifacts_invalid,
        }),
    );
    map.insert(
        "evidence_complete".to_owned(),
        Value::Bool(outcome.evidence_complete),
    );
    Ok(Value::Object(map))
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FinishedCounts {
    pub results_complete: Option<bool>,
    pub diagnostics: Option<[u64; 3]>,
    pub changes: Option<[u64; 2]>,
    pub mutations: Option<[u64; 2]>,
}

pub fn command_finished(exit_code: i32, counts: &FinishedCounts) -> Value {
    let mut map = base("command_finished");
    map.insert("exit_code".to_owned(), Value::from(exit_code));
    if let Some(results_complete) = counts.results_complete {
        map.insert("results_complete".to_owned(), Value::Bool(results_complete));
    }
    if let Some([info, warning, error]) = counts.diagnostics {
        map.insert(
            "diagnostics".to_owned(),
            json!({"info": info, "warning": warning, "error": error}),
        );
    }
    if let Some([create, modify]) = counts.changes {
        map.insert(
            "changes".to_owned(),
            json!({"create": create, "modify": modify}),
        );
    }
    if let Some([applied, not_applied]) = counts.mutations {
        map.insert(
            "mutations".to_owned(),
            json!({"applied": applied, "not_applied": not_applied}),
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
    let mut buf = serde_json::to_vec(event).map_err(|e| OutputError::Io(e.to_string()))?;
    buf.push(b'\n');
    writer
        .write_all(&buf)
        .map_err(|e| OutputError::Io(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    const DIGEST: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    #[test]
    fn command_started_shape() {
        let event = command_started("lint", false, "default").expect("started");
        assert_eq!(event["event"], Value::String("command_started".to_owned()));
        assert_eq!(event["schema"], schema());
        assert_eq!(event["mode"], Value::String("default".to_owned()));
        assert!(command_started("lint", false, "fancy").is_err());
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
            diagnostics: Some([0, 3, 1]),
            changes: None,
            mutations: Some([0, 2]),
        };
        let event = command_finished(1, &counts);
        assert_eq!(event["exit_code"], Value::from(1));
        assert_eq!(event["diagnostics"]["warning"], Value::from(3));
        assert!(event.get("changes").is_none());
    }

    #[test]
    fn finished_changes_render() {
        let counts = FinishedCounts {
            results_complete: None,
            diagnostics: None,
            changes: Some([1, 2]),
            mutations: None,
        };
        let event = command_finished(0, &counts);
        assert_eq!(event["changes"]["create"], Value::from(1));
        assert_eq!(event["changes"]["modify"], Value::from(2));
        assert!(event.get("diagnostics").is_none());
    }

    #[test]
    fn writer_rejects_prose() {
        let mut buf = Vec::new();
        let event = command_started("lint", false, "default").expect("started");
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
    fn status_event_shape() {
        let event = status_event(&StatusEvent {
            name: "pin".to_owned(),
            status: "ok".to_owned(),
            detail: "dx 0.0.0 vs module 0.0.0".to_owned(),
            hint: "dx version --pin 0.0.0".to_owned(),
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
    fn schema_is_minor_one_with_forward_compat() {
        assert_eq!(SCHEMA_MAJOR, 1);
        assert_eq!(SCHEMA_MINOR, 1);
        assert_eq!(schema(), serde_json::json!({"major": 1, "minor": 1}));
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
    fn test_outcome_shape_omits_unknowns() {
        let event = test_outcome_event(&TestOutcome {
            target: "//a:t".to_owned(),
            outcome: "passed".to_owned(),
            status: Some("PASSED".to_owned()),
            cached: Some(true),
            run: Some(1),
            shard: Some(1),
            attempt: Some(2),
            duration_millis: Some(29),
            cases_total: Some(3),
            cases_failed: Some(0),
            cases_error: Some(0),
            cases_skipped: Some(1),
            artifacts_collected: 1,
            artifacts_missing: 0,
            artifacts_invalid: 0,
            evidence_complete: true,
            ..TestOutcome::default()
        })
        .expect("outcome");
        assert_eq!(event["event"], Value::String("test_outcome".to_owned()));
        assert_eq!(event["target"], Value::String("//a:t".to_owned()));
        assert_eq!(event["outcome"], Value::String("passed".to_owned()));
        assert_eq!(event["cached"], Value::Bool(true));
        assert_eq!(event["attempt"], Value::from(2));
        assert_eq!(event["duration_millis"], Value::from(29));
        assert_eq!(event["cases_skipped"], Value::from(1));
        assert_eq!(event["artifacts"]["collected"], Value::from(1));
        assert_eq!(event["evidence_complete"], Value::Bool(true));
        assert!(event.get("configuration").is_none());
        let bare = test_outcome_event(&TestOutcome {
            target: "//a:t".to_owned(),
            outcome: "unknown".to_owned(),
            ..TestOutcome::default()
        })
        .expect("bare");
        for field in [
            "configuration",
            "status",
            "cached",
            "run",
            "cases_total",
            "duration_millis",
        ] {
            assert!(bare.get(field).is_none(), "{field} stays unknown");
        }
        assert!(bare.get("artifacts").is_some());
        assert!(test_outcome_event(&TestOutcome::default()).is_err());
        assert!(test_outcome_event(&TestOutcome {
            target: "//a:t".to_owned(),
            ..TestOutcome::default()
        })
        .is_err());
        let mut buf = Vec::new();
        write_event(&mut buf, &event).expect("write");
        let parsed: Value = serde_json::from_slice(&buf).expect("parse");
        assert_eq!(parsed["outcome"], Value::String("passed".to_owned()));
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
        let event = command_started("status", false, "default").expect("started");
        let error = write_event(&mut BrokenPipe, &event).expect_err("broken pipe");
        assert!(error.is_broken_pipe());
    }
}
