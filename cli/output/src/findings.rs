use crate::lifecycle::{base, nonempty};
use crate::severity::Severity;
use crate::validation::{check_path, OutputError};
use serde_json::{json, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Snapshot {
    Initial,
    Terminal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Resolution {
    Fixed,
    Remaining,
    NotApplied,
}

impl Resolution {
    fn name(self) -> &'static str {
        match self {
            Resolution::Fixed => "fixed",
            Resolution::Remaining => "remaining",
            Resolution::NotApplied => "not_applied",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelatedEvent {
    pub role: String,
    pub message: String,
    pub path: Option<String>,
    pub range: Option<(u64, u64)>,
    pub replacement: Option<String>,
    pub applicability: Option<String>,
}

/// One configured producer that reported a finding.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Origin {
    pub producer: String,
    pub target: String,
    pub configuration: Option<String>,
}

/// A 1-based line and a 1-based column counted in Unicode scalar values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Position {
    pub line: u64,
    pub column: u64,
}

pub type Span = (Position, Position);

/// One emitted finding: the diagnostic, its verified lines, its folded count and its baseline mark.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FindingRecord {
    pub diagnostic: DiagnosticEvent,
    pub span: Option<Span>,
    pub related_spans: Vec<Option<Span>>,
    pub count: u64,
    pub suppressed: bool,
}

impl FindingRecord {
    pub fn single(diagnostic: DiagnosticEvent) -> Self {
        FindingRecord {
            related_spans: vec![None; diagnostic.related.len()],
            diagnostic,
            span: None,
            count: 1,
            suppressed: false,
        }
    }
}

/// The dependency a vulnerability or license finding is about.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageRef {
    pub name: String,
    pub version: String,
    pub fixed: Option<Vec<String>>,
    pub license: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticEvent {
    pub severity: Severity,
    pub tool: String,
    pub message: String,
    pub rule: Option<String>,
    pub path: Option<String>,
    pub range: Option<(u64, u64)>,
    pub snapshot: Snapshot,
    pub fixable: bool,
    pub resolution: Option<Resolution>,
    pub related: Vec<RelatedEvent>,
    pub origins: Vec<Origin>,
    pub package: Option<PackageRef>,
}

impl DiagnosticEvent {
    /// Builds a terminal diagnostic with no rule, location, fix, resolution, related items, origins or package.
    pub fn new(severity: Severity, tool: impl Into<String>, message: impl Into<String>) -> Self {
        DiagnosticEvent {
            severity,
            tool: tool.into(),
            message: message.into(),
            rule: None,
            path: None,
            range: None,
            snapshot: Snapshot::Terminal,
            fixable: false,
            resolution: None,
            related: Vec::new(),
            origins: Vec::new(),
            package: None,
        }
    }
}

/// Validates and builds the event for a producer with no validator of its own.
pub fn diagnostic_event(diagnostic: &DiagnosticEvent) -> Result<Value, OutputError> {
    check_diagnostic(diagnostic)?;
    Ok(finding_value(&FindingRecord::single(diagnostic.clone())))
}

fn check_diagnostic(diagnostic: &DiagnosticEvent) -> Result<(), OutputError> {
    nonempty("tool", &diagnostic.tool)?;
    nonempty("message", &diagnostic.message)?;
    if let Some(path) = &diagnostic.path {
        check_path(path)?;
    } else if diagnostic.range.is_some() {
        return Err(OutputError::RangeWithoutPath);
    }
    if let Some((start, end)) = diagnostic.range {
        if start > end {
            return Err(OutputError::InvertedRange);
        }
    }
    for item in &diagnostic.related {
        nonempty("related role", &item.role)?;
        if let Some(path) = &item.path {
            check_path(path)?;
        } else if item.range.is_some() {
            return Err(OutputError::RangeWithoutPath);
        }
        if item.range.is_some_and(|(start, end)| start > end) {
            return Err(OutputError::InvertedRange);
        }
    }
    for origin in &diagnostic.origins {
        nonempty("origin producer", &origin.producer)?;
        nonempty("origin target", &origin.target)?;
    }
    if let Some(package) = &diagnostic.package {
        nonempty("package name", &package.name)?;
        nonempty("package version", &package.version)?;
        for fixed in package.fixed.iter().flatten() {
            nonempty("package fixed", fixed)?;
        }
        if let Some(license) = &package.license {
            nonempty("package license", license)?;
        }
    }
    if diagnostic.resolution.is_some() {
        return Err(OutputError::UnexpectedResolution);
    }
    Ok(())
}

fn location(path: Option<&String>, range: Option<(u64, u64)>, span: Option<Span>) -> &'static str {
    match (path, range, span) {
        (None, _, _) => "unknown",
        (Some(_), None, _) => "file",
        (Some(_), Some(_), None) => "bytes",
        (Some(_), Some(_), Some(_)) => "lines",
    }
}

fn range_value(range: (u64, u64), span: Option<Span>) -> Value {
    let (start, end) = range;
    match span {
        Some((first, last)) => json!({
            "start_byte": start,
            "end_byte": end,
            "start_line": first.line,
            "start_column": first.column,
            "end_line": last.line,
            "end_column": last.column,
        }),
        None => json!({"start_byte": start, "end_byte": end}),
    }
}

/// Builds the event for one finding whose producer already validated it.
pub fn finding_value(record: &FindingRecord) -> Value {
    let diagnostic = &record.diagnostic;
    let mut map = base("diagnostic");
    map.insert(
        "severity".to_owned(),
        Value::String(diagnostic.severity.name().to_owned()),
    );
    map.insert("tool".to_owned(), Value::String(diagnostic.tool.clone()));
    map.insert(
        "message".to_owned(),
        Value::String(diagnostic.message.clone()),
    );
    if let Some(rule) = &diagnostic.rule {
        map.insert("rule".to_owned(), Value::String(rule.clone()));
    }
    let span = diagnostic.range.and(record.span);
    map.insert(
        "location".to_owned(),
        Value::String(location(diagnostic.path.as_ref(), diagnostic.range, span).to_owned()),
    );
    if let Some(path) = &diagnostic.path {
        map.insert("path".to_owned(), Value::String(path.clone()));
    }
    if let Some(range) = diagnostic.range {
        map.insert("range".to_owned(), range_value(range, span));
    }
    if let Some(package) = &diagnostic.package {
        map.insert("package".to_owned(), package_value(package));
    }
    map.insert(
        "snapshot".to_owned(),
        Value::String(
            match diagnostic.snapshot {
                Snapshot::Initial => "initial",
                Snapshot::Terminal => "terminal",
            }
            .to_owned(),
        ),
    );
    map.insert("fixable".to_owned(), Value::Bool(diagnostic.fixable));
    if let Some(resolution) = diagnostic.resolution {
        map.insert(
            "resolution".to_owned(),
            Value::String(resolution.name().to_owned()),
        );
    }
    map.insert("count".to_owned(), Value::from(record.count));
    if record.suppressed {
        map.insert(
            "baseline".to_owned(),
            Value::String("suppressed".to_owned()),
        );
    }
    if !diagnostic.origins.is_empty() {
        map.insert(
            "origins".to_owned(),
            Value::Array(
                diagnostic
                    .origins
                    .iter()
                    .map(|origin| {
                        json!({
                            "producer": origin.producer,
                            "target": origin.target,
                            "configuration": origin.configuration.as_deref().unwrap_or("unknown"),
                        })
                    })
                    .collect(),
            ),
        );
    }
    if !diagnostic.related.is_empty() {
        map.insert(
            "related".to_owned(),
            Value::Array(
                diagnostic
                    .related
                    .iter()
                    .enumerate()
                    .map(|(index, item)| {
                        related_value(item, record.related_spans.get(index).copied().flatten())
                    })
                    .collect(),
            ),
        );
    }
    Value::Object(map)
}

fn package_value(package: &PackageRef) -> Value {
    let mut map = serde_json::Map::new();
    map.insert("name".to_owned(), Value::String(package.name.clone()));
    map.insert("version".to_owned(), Value::String(package.version.clone()));
    if let Some(fixed) = &package.fixed {
        map.insert(
            "fixed".to_owned(),
            Value::Array(fixed.iter().map(|v| Value::String(v.clone())).collect()),
        );
    }
    if let Some(license) = &package.license {
        map.insert("license".to_owned(), Value::String(license.clone()));
    }
    Value::Object(map)
}

fn related_value(item: &RelatedEvent, span: Option<Span>) -> Value {
    let span = item.range.and(span);
    let mut map = serde_json::Map::new();
    map.insert("role".to_owned(), Value::String(item.role.clone()));
    map.insert("message".to_owned(), Value::String(item.message.clone()));
    map.insert(
        "location".to_owned(),
        Value::String(location(item.path.as_ref(), item.range, span).to_owned()),
    );
    if let Some(path) = &item.path {
        map.insert("path".to_owned(), Value::String(path.clone()));
    }
    if let Some(range) = item.range {
        map.insert("range".to_owned(), range_value(range, span));
    }
    if let Some(replacement) = &item.replacement {
        map.insert("replacement".to_owned(), Value::String(replacement.clone()));
    }
    if let Some(applicability) = &item.applicability {
        map.insert(
            "applicability".to_owned(),
            Value::String(applicability.clone()),
        );
    }
    Value::Object(map)
}

pub fn sort_diagnostics(diagnostics: &mut [DiagnosticEvent]) {
    diagnostics.sort_by(|a, b| {
        a.snapshot
            .cmp(&b.snapshot)
            .then_with(|| a.path.is_none().cmp(&b.path.is_none()))
            .then_with(|| a.path.cmp(&b.path))
            .then_with(|| a.range.is_none().cmp(&b.range.is_none()))
            .then_with(|| a.range.map(|r| r.0).cmp(&b.range.map(|r| r.0)))
            .then_with(|| a.range.map(|r| r.1).cmp(&b.range.map(|r| r.1)))
            .then_with(|| a.severity.rank().cmp(&b.severity.rank()))
            .then_with(|| a.tool.cmp(&b.tool))
            .then_with(|| a.rule.is_none().cmp(&b.rule.is_none()))
            .then_with(|| a.rule.cmp(&b.rule))
            .then_with(|| a.message.cmp(&b.message))
            .then_with(|| a.origins.cmp(&b.origins))
    });
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoticeEvent {
    pub level: String,
    pub code: String,
    pub message: String,
    pub related_command: Option<String>,
    pub scope: Option<Vec<String>>,
    pub path: Option<String>,
    pub language: Option<String>,
    pub import: Option<String>,
}

impl NoticeEvent {
    /// An `info` notice with no related command, scope, or path.
    pub fn info(code: &str, message: impl Into<String>) -> Self {
        Self::at_level("info", code, message.into())
    }

    /// A `warning` notice with no related command, scope, or path.
    pub fn warning(code: &str, message: impl Into<String>) -> Self {
        Self::at_level("warning", code, message.into())
    }

    fn at_level(level: &str, code: &str, message: String) -> Self {
        NoticeEvent {
            level: level.to_owned(),
            code: code.to_owned(),
            message,
            related_command: None,
            scope: None,
            path: None,
            language: None,
            import: None,
        }
    }
}

pub fn notice_event(notice: &NoticeEvent) -> Result<Value, OutputError> {
    check_notice(notice)?;
    Ok(notice_value(notice))
}

fn check_notice(notice: &NoticeEvent) -> Result<(), OutputError> {
    if notice.level != "info" && notice.level != "warning" {
        return Err(OutputError::EmptyField { field: "level" });
    }
    nonempty("code", &notice.code)?;
    nonempty("message", &notice.message)?;
    if notice.code == "ignored_import" {
        let path = notice
            .path
            .as_ref()
            .ok_or(OutputError::EmptyField { field: "path" })?;
        check_path(path)?;
        nonempty("language", notice.language.as_deref().unwrap_or(""))?;
        nonempty("import", notice.import.as_deref().unwrap_or(""))?;
        if notice.scope.is_some() || notice.related_command.is_some() {
            return Err(OutputError::UnexpectedResolution);
        }
    }
    Ok(())
}

/// Builds the event for a notice whose producer already validated it.
pub fn notice_value(notice: &NoticeEvent) -> Value {
    let mut map = base("notice");
    map.insert("level".to_owned(), Value::String(notice.level.clone()));
    map.insert("code".to_owned(), Value::String(notice.code.clone()));
    map.insert("message".to_owned(), Value::String(notice.message.clone()));
    if let Some(related) = &notice.related_command {
        map.insert("related_command".to_owned(), Value::String(related.clone()));
    }
    if let Some(scope) = &notice.scope {
        map.insert(
            "scope".to_owned(),
            Value::Array(scope.iter().map(|s| Value::String(s.clone())).collect()),
        );
    }
    if let Some(path) = &notice.path {
        map.insert("path".to_owned(), Value::String(path.clone()));
    }
    if let Some(language) = &notice.language {
        map.insert("language".to_owned(), Value::String(language.clone()));
    }
    if let Some(import) = &notice.import {
        map.insert("import".to_owned(), Value::String(import.clone()));
    }
    Value::Object(map)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn finding() -> DiagnosticEvent {
        DiagnosticEvent {
            rule: Some("F401".to_owned()),
            path: Some("src/app.py".to_owned()),
            range: Some((18, 24)),
            snapshot: Snapshot::Initial,
            fixable: true,
            ..DiagnosticEvent::new(Severity::Warning, "ruff", "Imported but unused")
        }
    }

    #[test]
    fn diagnostic_event_rejects_any_resolution() {
        diagnostic_event(&finding()).expect("check initial");
        let mut initial = finding();
        initial.resolution = Some(Resolution::Fixed);
        assert_eq!(
            diagnostic_event(&initial).expect_err("initial resolution"),
            OutputError::UnexpectedResolution
        );
        let mut terminal = finding();
        terminal.snapshot = Snapshot::Terminal;
        terminal.fixable = false;
        diagnostic_event(&terminal).expect("terminal");
        terminal.resolution = Some(Resolution::Fixed);
        assert_eq!(
            diagnostic_event(&terminal).expect_err("terminal resolution"),
            OutputError::UnexpectedResolution
        );
        let mut ranged = finding();
        ranged.path = None;
        assert_eq!(
            diagnostic_event(&ranged).expect_err("range without path"),
            OutputError::RangeWithoutPath
        );
    }

    #[test]
    fn new_builds_a_valid_terminal_diagnostic() {
        let event = DiagnosticEvent::new(Severity::Error, "vuln", "m");
        assert_eq!(event.snapshot, Snapshot::Terminal);
        diagnostic_event(&event).expect("terminal");
    }

    fn package(fixed: Option<Vec<String>>, license: Option<&str>) -> PackageRef {
        PackageRef {
            name: "left-pad".to_owned(),
            version: "1.0.0".to_owned(),
            fixed,
            license: license.map(str::to_owned),
        }
    }

    #[test]
    fn package_is_absent_unless_set() {
        let event = diagnostic_event(&finding()).expect("no package");
        assert!(event.get("package").is_none());
    }

    #[test]
    fn package_keeps_an_empty_fixed_list() {
        let event = diagnostic_event(&DiagnosticEvent {
            package: Some(package(Some(Vec::new()), None)),
            ..DiagnosticEvent::new(Severity::Error, "vuln", "m")
        })
        .expect("vuln package");
        assert_eq!(
            event["package"],
            serde_json::json!({"name": "left-pad", "version": "1.0.0", "fixed": []})
        );
        let fixed = finding_value(&FindingRecord::single(DiagnosticEvent {
            package: Some(package(Some(vec!["1.0.1".to_owned()]), None)),
            ..DiagnosticEvent::new(Severity::Error, "vuln", "m")
        }));
        assert_eq!(fixed["package"]["fixed"], serde_json::json!(["1.0.1"]));
    }

    #[test]
    fn package_carries_a_license() {
        let event = diagnostic_event(&DiagnosticEvent {
            package: Some(package(None, Some("MIT"))),
            ..DiagnosticEvent::new(Severity::Error, "license", "m")
        })
        .expect("license package");
        assert_eq!(
            event["package"],
            serde_json::json!({"name": "left-pad", "version": "1.0.0", "license": "MIT"})
        );
    }

    #[test]
    fn package_rejects_empty_identity() {
        for (name, version, field) in [
            ("", "1.0.0", "package name"),
            ("left-pad", "", "package version"),
        ] {
            let event = DiagnosticEvent {
                package: Some(PackageRef {
                    name: name.to_owned(),
                    version: version.to_owned(),
                    ..package(None, None)
                }),
                ..DiagnosticEvent::new(Severity::Error, "vuln", "m")
            };
            assert_eq!(
                diagnostic_event(&event).expect_err("empty identity"),
                OutputError::EmptyField { field }
            );
        }
    }

    #[test]
    fn diagnostics_sort_deterministically() {
        let mut events = vec![
            DiagnosticEvent::new(Severity::Error, "b", "m"),
            DiagnosticEvent {
                path: Some("z.py".to_owned()),
                snapshot: Snapshot::Initial,
                ..DiagnosticEvent::new(Severity::Info, "a", "m")
            },
            DiagnosticEvent {
                path: Some("a.py".to_owned()),
                range: Some((5, 9)),
                snapshot: Snapshot::Initial,
                ..DiagnosticEvent::new(Severity::Warning, "a", "m")
            },
        ];
        sort_diagnostics(&mut events);
        let paths: Vec<Option<&str>> = events.iter().map(|e| e.path.as_deref()).collect();
        assert_eq!(paths, vec![Some("a.py"), Some("z.py"), None]);
        assert_eq!(events[2].snapshot, Snapshot::Terminal);
    }

    #[test]
    fn ignored_import_notice_shape() {
        let notice = NoticeEvent {
            level: "warning".to_owned(),
            code: "ignored_import".to_owned(),
            message: "ignored".to_owned(),
            related_command: None,
            scope: None,
            path: Some("src/plugin.py".to_owned()),
            language: Some("python".to_owned()),
            import: Some("optional_runtime_module".to_owned()),
        };
        let event = notice_event(&notice).expect("notice");
        assert!(event.get("scope").is_none());
        assert_eq!(
            event["import"],
            Value::String("optional_runtime_module".to_owned())
        );
        let mut bad = notice.clone();
        bad.language = None;
        assert!(notice_event(&bad).is_err());
    }

    #[test]
    fn diagnostic_events_carry_every_field() {
        let event = diagnostic_event(&finding()).expect("check initial");
        assert_eq!(event["severity"], Value::from("warning"));
        assert_eq!(event["tool"], Value::from("ruff"));
        assert_eq!(event["message"], Value::from("Imported but unused"));
        assert_eq!(event["rule"], Value::from("F401"));
        assert_eq!(event["path"], Value::from("src/app.py"));
        assert_eq!(event["range"]["start_byte"], Value::from(18));
        assert_eq!(event["range"]["end_byte"], Value::from(24));
        assert_eq!(event["snapshot"], Value::from("initial"));
        assert_eq!(event["fixable"], Value::from(true));
        assert!(event.get("resolution").is_none());
    }

    #[test]
    fn diagnostic_events_render_resolution() {
        let mut resolved = finding();
        resolved.resolution = Some(Resolution::Remaining);
        let event = finding_value(&FindingRecord::single(resolved));
        assert_eq!(
            event["resolution"],
            Value::from("remaining"),
            "an unset resolution must still be rendered when the producer set one"
        );
        assert_eq!(event["tool"], Value::from("ruff"));
        assert_eq!(event["range"]["end_byte"], Value::from(24));
        let initial = finding_value(&FindingRecord::single(finding()));
        assert!(initial.get("resolution").is_none());
        assert_eq!(initial["snapshot"], Value::from("initial"));
        assert_eq!(initial["rule"], Value::from("F401"));
    }

    #[test]
    fn notice_events_carry_every_field() {
        let notice = NoticeEvent {
            level: "warning".to_owned(),
            code: "ignored_import".to_owned(),
            message: "ignored".to_owned(),
            related_command: None,
            scope: None,
            path: Some("src/plugin.py".to_owned()),
            language: Some("python".to_owned()),
            import: Some("optional_runtime_module".to_owned()),
        };
        let event = notice_value(&notice);
        assert_eq!(event, notice_event(&notice).expect("ignored import"));
        assert_eq!(event["level"], Value::from("warning"));
        assert_eq!(event["code"], Value::from("ignored_import"));
        assert_eq!(event["message"], Value::from("ignored"));
        assert_eq!(event["path"], Value::from("src/plugin.py"));
        assert_eq!(event["language"], Value::from("python"));
        assert!(event.get("scope").is_none());

        let plain = NoticeEvent {
            level: "info".to_owned(),
            code: "other".to_owned(),
            message: "m".to_owned(),
            related_command: Some("lint".to_owned()),
            scope: Some(vec!["//src/...".to_owned()]),
            path: None,
            language: None,
            import: None,
        };
        let event = notice_value(&plain);
        assert_eq!(event, notice_event(&plain).expect("plain"));
        assert_eq!(event["related_command"], Value::from("lint"));
        assert_eq!(event["scope"][0], Value::from("//src/..."));
        assert!(event.get("path").is_none());
    }

    #[test]
    fn notice_constructors_match_the_literal() {
        for (level, notice) in [
            ("info", NoticeEvent::info("other", "m")),
            ("warning", NoticeEvent::warning("other", "m")),
        ] {
            let literal = NoticeEvent {
                level: level.to_owned(),
                code: "other".to_owned(),
                message: "m".to_owned(),
                related_command: None,
                scope: None,
                path: None,
                language: None,
                import: None,
            };
            assert_eq!(notice, literal);
            assert_eq!(
                notice_event(&notice).expect("constructed notice"),
                notice_value(&literal)
            );
        }
    }

    #[test]
    fn inverted_range_fails() {
        let mut bad = finding();
        bad.range = Some((24, 18));
        assert_eq!(
            diagnostic_event(&bad).expect_err("inverted range"),
            OutputError::InvertedRange
        );
    }

    #[test]
    fn pathless_rangeless_diagnostic_renders() {
        let mut tool_level = finding();
        tool_level.path = None;
        tool_level.range = None;
        tool_level.snapshot = Snapshot::Terminal;
        let event = diagnostic_event(&tool_level).expect("tool-level");
        assert_eq!(event["snapshot"], Value::String("terminal".to_owned()));
        assert!(event.get("path").is_none());
        assert!(event.get("range").is_none());
    }

    #[test]
    fn resolutions_render_by_name() {
        for (resolution, name) in [
            (Resolution::Fixed, "fixed"),
            (Resolution::Remaining, "remaining"),
            (Resolution::NotApplied, "not_applied"),
        ] {
            let mut resolved = finding();
            resolved.resolution = Some(resolution);
            let event = finding_value(&FindingRecord::single(resolved));
            assert_eq!(event["resolution"], Value::String(name.to_owned()));
        }
    }

    #[test]
    fn notice_level_and_scope_rules() {
        let mut bad_level = NoticeEvent {
            level: "debug".to_owned(),
            code: "other".to_owned(),
            message: "m".to_owned(),
            related_command: None,
            scope: None,
            path: None,
            language: None,
            import: None,
        };
        assert!(notice_event(&bad_level).is_err());
        bad_level.level = "info".to_owned();
        let event = notice_event(&bad_level).expect("plain notice");
        assert_eq!(event["level"], Value::String("info".to_owned()));
        let mut scoped = bad_level.clone();
        scoped.related_command = Some("lint".to_owned());
        scoped.scope = Some(vec!["//src/...".to_owned()]);
        let event = notice_event(&scoped).expect("scoped notice");
        assert_eq!(event["related_command"], Value::String("lint".to_owned()));
        assert_eq!(event["scope"][0], Value::String("//src/...".to_owned()));
        let mut import = NoticeEvent {
            level: "warning".to_owned(),
            code: "ignored_import".to_owned(),
            message: "ignored".to_owned(),
            related_command: Some("lint".to_owned()),
            scope: None,
            path: Some("src/plugin.py".to_owned()),
            language: Some("python".to_owned()),
            import: Some("mod".to_owned()),
        };
        assert_eq!(
            notice_event(&import).expect_err("scoped import"),
            OutputError::UnexpectedResolution
        );
        import.related_command = None;
        import.scope = Some(vec!["//src/...".to_owned()]);
        assert_eq!(
            notice_event(&import).expect_err("scoped import"),
            OutputError::UnexpectedResolution
        );
    }

    #[test]
    fn related_context_serializes_in_order() {
        let mut event = finding();
        event.related = vec![
            RelatedEvent {
                role: "label".to_owned(),
                message: "expected `i32`".to_owned(),
                path: Some("src/b.rs".to_owned()),
                range: Some((1, 4)),
                replacement: None,
                applicability: None,
            },
            RelatedEvent {
                role: "help".to_owned(),
                message: "try".to_owned(),
                path: None,
                range: None,
                replacement: Some("1".to_owned()),
                applicability: Some("MaybeIncorrect".to_owned()),
            },
        ];
        let value = diagnostic_event(&event).expect("event");
        let related = value["related"].as_array().expect("related");
        assert_eq!(related.len(), 2);
        assert_eq!(related[0]["role"], Value::from("label"));
        assert_eq!(related[0]["range"]["end_byte"], Value::from(4));
        assert_eq!(related[1]["applicability"], Value::from("MaybeIncorrect"));
        assert!(related[1].get("path").is_none());
        event.related[0].range = Some((4, 1));
        assert_eq!(
            diagnostic_event(&event).expect_err("inverted"),
            OutputError::InvertedRange
        );
        let plain = diagnostic_event(&finding()).expect("plain");
        assert!(plain.get("related").is_none());
    }

    fn at(line: u64, column: u64) -> Position {
        Position { line, column }
    }

    #[test]
    fn locations_say_what_was_verified() {
        let bytes = finding_value(&FindingRecord::single(finding()));
        assert_eq!(bytes["location"], Value::from("bytes"));
        assert_eq!(bytes["count"], Value::from(1));
        assert!(bytes["range"].get("start_line").is_none());
        assert!(bytes.get("origins").is_none());
        assert!(bytes.get("baseline").is_none());
        let mut file = finding();
        file.range = None;
        assert_eq!(
            finding_value(&FindingRecord::single(file.clone()))["location"],
            Value::from("file")
        );
        let mut unknown = finding();
        unknown.path = None;
        unknown.range = None;
        assert_eq!(
            finding_value(&FindingRecord::single(unknown))["location"],
            Value::from("unknown")
        );
        let mut record = FindingRecord::single(finding());
        record.span = Some((at(2, 1), at(2, 7)));
        let lines = finding_value(&record);
        assert_eq!(lines["location"], Value::from("lines"));
        assert_eq!(
            lines["range"],
            serde_json::json!({
                "start_byte": 18,
                "end_byte": 24,
                "start_line": 2,
                "start_column": 1,
                "end_line": 2,
                "end_column": 7,
            })
        );
        let mut stray = FindingRecord::single(file);
        stray.span = Some((at(1, 1), at(1, 1)));
        assert_eq!(finding_value(&stray)["location"], Value::from("file"));
    }

    #[test]
    fn folded_findings_keep_every_origin_and_mark() {
        let mut diagnostic = finding();
        diagnostic.origins = vec![
            Origin {
                producer: "//src:a".to_owned(),
                target: "//src:a".to_owned(),
                configuration: Some("k8-fastbuild".to_owned()),
            },
            Origin {
                producer: "//src:b".to_owned(),
                target: "//src:b".to_owned(),
                configuration: None,
            },
        ];
        diagnostic_event(&diagnostic).expect("origins validate");
        let record = FindingRecord {
            count: 2,
            suppressed: true,
            ..FindingRecord::single(diagnostic.clone())
        };
        let event = finding_value(&record);
        assert_eq!(event["count"], Value::from(2));
        assert_eq!(event["baseline"], Value::from("suppressed"));
        assert_eq!(event["origins"][0]["producer"], Value::from("//src:a"));
        assert_eq!(
            event["origins"][0]["configuration"],
            Value::from("k8-fastbuild")
        );
        assert_eq!(event["origins"][1]["target"], Value::from("//src:b"));
        assert_eq!(event["origins"][1]["configuration"], Value::from("unknown"));
        diagnostic.origins[0].producer = String::new();
        assert!(diagnostic_event(&diagnostic).is_err());
    }

    #[test]
    fn related_locations_carry_lines_when_verified() {
        let mut diagnostic = finding();
        diagnostic.related = vec![
            RelatedEvent {
                role: "label".to_owned(),
                message: "here".to_owned(),
                path: Some("src/b.rs".to_owned()),
                range: Some((1, 4)),
                replacement: None,
                applicability: None,
            },
            RelatedEvent {
                role: "note".to_owned(),
                message: "plain".to_owned(),
                path: None,
                range: None,
                replacement: None,
                applicability: None,
            },
        ];
        let mut record = FindingRecord::single(diagnostic);
        record.related_spans = vec![Some((at(1, 2), at(1, 5))), None];
        let event = finding_value(&record);
        assert_eq!(event["related"][0]["location"], Value::from("lines"));
        assert_eq!(event["related"][0]["range"]["end_column"], Value::from(5));
        assert_eq!(event["related"][1]["location"], Value::from("unknown"));
        record.related_spans.clear();
        let event = finding_value(&record);
        assert_eq!(event["related"][0]["location"], Value::from("bytes"));
    }
}
