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
}

pub fn diagnostic_event(
    diagnostic: &DiagnosticEvent,
    mutating: bool,
) -> Result<Value, OutputError> {
    check_diagnostic(diagnostic, mutating)?;
    Ok(diagnostic_value(diagnostic))
}

fn check_diagnostic(diagnostic: &DiagnosticEvent, mutating: bool) -> Result<(), OutputError> {
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
    match (diagnostic.snapshot, diagnostic.resolution, mutating) {
        (Snapshot::Terminal, Some(_), _) | (Snapshot::Initial, Some(_), false) => {
            return Err(OutputError::UnexpectedResolution);
        }
        (Snapshot::Initial, None, true) => return Err(OutputError::MissingResolution),
        (Snapshot::Initial, _, _) | (Snapshot::Terminal, None, _) => {}
    }
    Ok(())
}

/// Builds the event for a diagnostic whose producer already validated it.
pub fn diagnostic_value(diagnostic: &DiagnosticEvent) -> Value {
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
    if let Some(path) = &diagnostic.path {
        map.insert("path".to_owned(), Value::String(path.clone()));
    }
    if let Some((start, end)) = diagnostic.range {
        map.insert(
            "range".to_owned(),
            json!({"start_byte": start, "end_byte": end}),
        );
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
    Value::Object(map)
}

pub fn compare_diagnostics(a: &DiagnosticEvent, b: &DiagnosticEvent) -> std::cmp::Ordering {
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
}

pub fn sort_diagnostics(diagnostics: &mut [DiagnosticEvent]) {
    diagnostics.sort_by(compare_diagnostics);
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
            severity: Severity::Warning,
            tool: "ruff".to_owned(),
            message: "Imported but unused".to_owned(),
            rule: Some("F401".to_owned()),
            path: Some("src/app.py".to_owned()),
            range: Some((18, 24)),
            snapshot: Snapshot::Initial,
            fixable: true,
            resolution: None,
        }
    }

    #[test]
    fn diagnostic_resolution_rules() {
        diagnostic_event(&finding(), false).expect("check initial");
        assert_eq!(
            diagnostic_event(&finding(), true).expect_err("missing resolution"),
            OutputError::MissingResolution
        );
        let mut terminal = finding();
        terminal.snapshot = Snapshot::Terminal;
        terminal.fixable = false;
        terminal.resolution = None;
        diagnostic_event(&terminal, true).expect("terminal");
        terminal.resolution = Some(Resolution::Fixed);
        assert_eq!(
            diagnostic_event(&terminal, true).expect_err("terminal resolution"),
            OutputError::UnexpectedResolution
        );
        let mut ranged = finding();
        ranged.path = None;
        assert_eq!(
            diagnostic_event(&ranged, false).expect_err("range without path"),
            OutputError::RangeWithoutPath
        );
    }

    #[test]
    fn diagnostics_sort_deterministically() {
        let mut events = vec![
            DiagnosticEvent {
                severity: Severity::Error,
                tool: "b".to_owned(),
                message: "m".to_owned(),
                rule: None,
                path: None,
                range: None,
                snapshot: Snapshot::Terminal,
                fixable: false,
                resolution: None,
            },
            DiagnosticEvent {
                severity: Severity::Info,
                tool: "a".to_owned(),
                message: "m".to_owned(),
                rule: None,
                path: Some("z.py".to_owned()),
                range: None,
                snapshot: Snapshot::Initial,
                fixable: false,
                resolution: None,
            },
            DiagnosticEvent {
                severity: Severity::Warning,
                tool: "a".to_owned(),
                message: "m".to_owned(),
                rule: None,
                path: Some("a.py".to_owned()),
                range: Some((5, 9)),
                snapshot: Snapshot::Initial,
                fixable: false,
                resolution: None,
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
        let event = diagnostic_event(&finding(), false).expect("check initial");
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
    fn diagnostic_value_matches_the_checked_event() {
        let mut resolved = finding();
        resolved.resolution = Some(Resolution::Remaining);
        let event = diagnostic_value(&resolved);
        assert_eq!(
            event["resolution"],
            Value::from("remaining"),
            "an unset resolution must still be rendered when the producer set one"
        );
        assert_eq!(event["tool"], Value::from("ruff"));
        assert_eq!(event["range"]["end_byte"], Value::from(24));
        assert_eq!(
            event,
            diagnostic_event(&resolved, true).expect("resolved"),
            "the checked entry point must agree with the value builder"
        );
        let initial = diagnostic_value(&finding());
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
    fn inverted_range_fails() {
        let mut bad = finding();
        bad.range = Some((24, 18));
        assert_eq!(
            diagnostic_event(&bad, false).expect_err("inverted range"),
            OutputError::InvertedRange
        );
    }

    #[test]
    fn pathless_rangeless_diagnostic_renders() {
        let mut tool_level = finding();
        tool_level.path = None;
        tool_level.range = None;
        tool_level.snapshot = Snapshot::Terminal;
        let event = diagnostic_event(&tool_level, false).expect("tool-level");
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
            let event = diagnostic_event(&resolved, true).expect("resolved");
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
}
