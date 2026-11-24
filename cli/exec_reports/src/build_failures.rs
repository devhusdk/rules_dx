use crate::compiler_stderr;
use crate::rustc_stderr::{self, ActionDiagnostic};
use crate::skipped_targets::language_in_message;
use dx_bep::{BuildFailure, FailureStage, StderrRef, collect_build_failures};
use dx_exec_core::common::{CODE_BUILD_EVENTS_INCOMPLETE, emit_built};
use dx_output::{
    BuildFailureEvent, FailureDiagnostic, FailureRelated, NoticeEvent, OutputMode,
    build_failure_event, notice_event,
};
use std::fmt;
use std::io::{BufReader, Read, Write};
use std::path::Path;

const DIAGNOSTIC_LIMIT: usize = 50;
const STDERR_LIMIT: u64 = 8 * 1024 * 1024;
const LINK_PREFIX: &str = "linking with";
const NOT_BUILT: &str = "target_not_built";

struct Diagnostics {
    status: &'static str,
    total: u64,
    items: Vec<ActionDiagnostic>,
}

fn read_stderr(path: &Path) -> Option<Vec<u8>> {
    let file = std::fs::File::open(path).ok()?;
    let mut bytes = Vec::new();
    file.take(STDERR_LIMIT + 1).read_to_end(&mut bytes).ok()?;
    if bytes.len() as u64 > STDERR_LIMIT {
        return None;
    }
    Some(bytes)
}

fn stderr_diagnostics(
    failure: &BuildFailure,
    parse: fn(&[u8]) -> Vec<ActionDiagnostic>,
) -> Diagnostics {
    let StderrRef::Local(path) = &failure.stderr else {
        return Diagnostics {
            status: "unavailable",
            total: 0,
            items: Vec::new(),
        };
    };
    let parsed = read_stderr(path).map(|bytes| parse(&bytes));
    match parsed {
        None => Diagnostics {
            status: "unavailable",
            total: 0,
            items: Vec::new(),
        },
        Some(items) if items.is_empty() => Diagnostics {
            status: "none",
            total: 0,
            items,
        },
        Some(items) => Diagnostics {
            status: "parsed",
            total: items.len() as u64,
            items,
        },
    }
}

fn compiler_tool(mnemonic: &str) -> Option<&'static str> {
    let tools = [
        ("CppCompile", "cc"),
        ("Javac", "javac"),
        ("GoCompile", "go"),
        ("Kotlin", "kotlinc"),
        ("Scala", "scalac"),
        ("TypeScript", "tsc"),
        ("CSharp", "csc"),
        ("Swift", "swiftc"),
    ];
    tools
        .iter()
        .find(|(prefix, _)| mnemonic.starts_with(prefix))
        .map(|(_, tool)| *tool)
}

pub(crate) fn to_event(failure: &BuildFailure) -> BuildFailureEvent {
    let mnemonic = failure.mnemonic.as_deref();
    let is_rustc = match mnemonic {
        Some(name) => name == "Clippy" || name.starts_with("Rustc"),
        None => false,
    };
    let tool = if is_rustc {
        Some(if mnemonic == Some("Clippy") {
            "clippy"
        } else {
            "rustc"
        })
    } else {
        mnemonic.and_then(compiler_tool)
    };
    let diagnostics = if is_rustc {
        stderr_diagnostics(failure, rustc_stderr::parse)
    } else if failure.stage == FailureStage::Compilation {
        stderr_diagnostics(failure, compiler_stderr::parse)
    } else {
        Diagnostics {
            status: "unknown",
            total: 0,
            items: Vec::new(),
        }
    };
    let (mut stage, mut category) = (failure.stage, failure.category);
    if is_rustc
        && stage == FailureStage::Compilation
        && diagnostics
            .items
            .iter()
            .any(|item| item.message.starts_with(LINK_PREFIX))
    {
        stage = FailureStage::Linking;
        category = "link_failed";
    }
    BuildFailureEvent {
        target: failure.target.clone(),
        configuration: failure.configuration.clone(),
        target_kind: failure.target_kind.name().to_owned(),
        stage: stage.name().to_owned(),
        category: category.to_owned(),
        mnemonic: failure.mnemonic.clone(),
        primary_output: failure.primary_output.clone(),
        exit_code: failure.exit_code,
        tool: tool.map(str::to_owned),
        message: failure.message.clone(),
        diagnostics_status: diagnostics.status.to_owned(),
        diagnostics_total: diagnostics.total,
        diagnostics: diagnostics
            .items
            .into_iter()
            .take(DIAGNOSTIC_LIMIT)
            .map(|item| FailureDiagnostic {
                severity: item.severity.to_owned(),
                code: item.code,
                message: item.message,
                path: item.file,
                line: item.line,
                column: item.column,
                related: item
                    .related
                    .into_iter()
                    .map(|related| FailureRelated {
                        role: related.role.to_owned(),
                        message: related.message,
                        path: related.file,
                        line: related.line,
                        column: related.column,
                        replacement: related.replacement,
                        applicability: related.applicability,
                    })
                    .collect(),
            })
            .collect(),
        count: 1,
        targets: Vec::new(),
    }
}

const TARGETS_LISTED: usize = 20;

fn identity(event: &BuildFailureEvent) -> String {
    let items: Vec<String> = event
        .diagnostics
        .iter()
        .map(|d| {
            format!(
                "{}|{:?}|{}|{:?}|{:?}|{:?}",
                d.severity, d.code, d.message, d.path, d.line, d.column
            )
        })
        .collect();
    let analysis_with_message =
        event.stage == FailureStage::Analysis.name() && event.message.is_some();
    let owner = if event.diagnostics.is_empty() && !analysis_with_message {
        event.target.clone()
    } else {
        None
    };
    format!(
        "{}|{}|{}|{:?}|{:?}|{:?}|{:?}|{}|{owner:?}|{items:?}",
        event.target_kind,
        event.stage,
        event.category,
        event.mnemonic,
        event.tool,
        event.message,
        event.exit_code,
        event.diagnostics_status,
    )
}

fn dedupe(events: Vec<BuildFailureEvent>) -> Vec<BuildFailureEvent> {
    let mut merged: Vec<BuildFailureEvent> = Vec::new();
    let mut seen: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for event in events {
        let key = identity(&event);
        let Some(first) = seen.get(&key).and_then(|&at| merged.get_mut(at)) else {
            seen.insert(key, merged.len());
            merged.push(event);
            continue;
        };
        first.count += 1;
        if let Some(target) = event.target {
            let known =
                first.target.as_deref() == Some(target.as_str()) || first.targets.contains(&target);
            if !known && first.targets.len() < TARGETS_LISTED {
                first.targets.push(target);
            }
        }
    }
    merged
}

fn add_skip_reason(event: &mut BuildFailureEvent) {
    let Some(language) = event.message.as_deref().and_then(language_in_message) else {
        return;
    };
    if let Some(message) = &mut event.message {
        message.push_str(&format!(
            "\n{} is not supported on this platform: {} (docs/support-matrix.md)",
            language.title, language.reason
        ));
    }
}

struct Described<'a>(&'a BuildFailureEvent);

impl fmt::Display for Described<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let event = self.0;
        let unknown = "unknown";
        let mut listed = vec![event.target.as_deref().unwrap_or(unknown)];
        listed.extend(event.targets.iter().map(String::as_str));
        write!(
            f,
            "dx: build failure: {} ({}/{}",
            listed.join(", "),
            event.stage,
            event.category
        )?;
        if let Some(mnemonic) = &event.mnemonic {
            write!(f, ", {mnemonic}")?;
        }
        f.write_str(")")?;
        if event.count > 1 {
            write!(f, " x{}", event.count)?;
        }
        if let Some(message) = &event.message {
            write!(f, ": {message}")?;
        }
        for diagnostic in &event.diagnostics {
            write!(f, "\n  {}", diagnostic.severity)?;
            if let Some(code) = &diagnostic.code {
                write!(f, "[{code}]")?;
            }
            write!(f, ": {}", diagnostic.message)?;
            if let Some(path) = &diagnostic.path {
                write!(f, " ({path}")?;
                if let Some(line) = diagnostic.line {
                    write!(f, ":{line}")?;
                    if let Some(column) = diagnostic.column {
                        write!(f, ":{column}")?;
                    }
                }
                f.write_str(")")?;
            }
            for related in &diagnostic.related {
                write!(f, "\n    {}: {}", related.role, related.message)?;
                if let Some(path) = &related.path {
                    write!(f, " ({path}")?;
                    if let Some(line) = related.line {
                        write!(f, ":{line}")?;
                        if let Some(column) = related.column {
                            write!(f, ":{column}")?;
                        }
                    }
                    f.write_str(")")?;
                }
                if let Some(replacement) = &related.replacement {
                    write!(f, " suggests `{replacement}`")?;
                    if let Some(applicability) = &related.applicability {
                        write!(f, " [{applicability}]")?;
                    }
                }
            }
        }
        Ok(())
    }
}

fn unavailable(
    output: OutputMode,
    out: &mut dyn Write,
    err: &mut dyn Write,
    detail: &str,
) -> Result<(), i32> {
    match output {
        OutputMode::Json => {
            let notice = NoticeEvent::warning(CODE_BUILD_EVENTS_INCOMPLETE, detail.to_owned());
            emit_built(out, notice_event(&notice))
        }
        OutputMode::Text { .. } => {
            dx_output::write_stderr(err, format_args!("dx: build_events_incomplete: {detail}\n"));
            Ok(())
        }
        OutputMode::Diff => Ok(()),
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ReportedFailures {
    pub count: usize,
    pub compile_only: bool,
    pub build_stage: bool,
}

pub fn report_build_failures(
    bep: &Path,
    output: OutputMode,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<ReportedFailures, i32> {
    let file = match std::fs::File::open(bep) {
        Ok(file) => file,
        Err(error) => {
            unavailable(
                output,
                out,
                err,
                &format!("build failures are unknown: {error}"),
            )?;
            return Ok(ReportedFailures::default());
        }
    };
    let collected = collect_build_failures(BufReader::new(file));
    let mut events: Vec<BuildFailureEvent> = collected.failures.iter().map(to_event).collect();
    for event in &mut events {
        add_skip_reason(event);
    }
    let events = dedupe(events);
    for event in &events {
        match output {
            OutputMode::Json => {
                emit_built(out, build_failure_event(event))?;
            }
            OutputMode::Text { .. } => {
                dx_output::write_stderr(err, format_args!("{}\n", Described(event)));
            }
            OutputMode::Diff => {}
        }
    }
    if let Some(problem) = &collected.problem {
        unavailable(
            output,
            out,
            err,
            &format!("build events end early, so later failures may be missing: {problem}"),
        )?;
    }
    let compiling = |event: &BuildFailureEvent| event.stage == FailureStage::Compilation.name();
    Ok(ReportedFailures {
        count: events.len(),
        build_stage: events.iter().any(|event| {
            event.stage != FailureStage::Analysis.name() && event.category != NOT_BUILT
        }) && !events
            .iter()
            .any(|event| event.stage == FailureStage::Analysis.name()),
        compile_only: collected.problem.is_none()
            && events.iter().any(compiling)
            && events
                .iter()
                .all(|event| compiling(event) || event.category == NOT_BUILT),
    })
}

#[cfg(test)]
mod tests {
    use super::{BuildFailureEvent, FailureDiagnostic, FailureRelated, dedupe};
    use serde_json::Value;

    #[test]
    fn events_that_differ_only_in_related_still_dedupe() {
        let mut a = to_failure_event_for_test();
        let mut b = a.clone();
        a.diagnostics[0].related = Vec::new();
        b.diagnostics[0].related = vec![FailureRelated {
            role: "note".to_owned(),
            message: "n".to_owned(),
            path: None,
            line: None,
            column: None,
            replacement: None,
            applicability: None,
        }];
        let merged = dedupe(vec![a, b]);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].count, 2);
        assert!(
            merged[0].diagnostics[0].related.is_empty(),
            "{:?}",
            merged[0].diagnostics[0].related
        );
    }

    #[test]
    fn an_explicit_incompatible_target_gets_the_language_reason() {
        let mut event = to_failure_event_for_test();
        event.message = Some(
            "Target //a:x is incompatible and cannot be built, but was explicitly requested.\n  @@rules_dx+//core/platforms:dart_unsupported"
                .to_owned(),
        );
        super::add_skip_reason(&mut event);
        let message = event.message.unwrap();
        assert!(
            message.contains("Dart is not supported on this platform"),
            "{message}"
        );
        let mut plain = to_failure_event_for_test();
        plain.message = Some("boom".to_owned());
        super::add_skip_reason(&mut plain);
        assert_eq!(plain.message.as_deref(), Some("boom"));
    }

    #[test]
    fn analysis_failures_with_the_same_message_merge_across_targets() {
        let mut a = to_failure_event_for_test();
        a.stage = "analysis".to_owned();
        a.category = "analysis_failed".to_owned();
        a.diagnostics = Vec::new();
        a.message = Some("Ruff needs a config.".to_owned());
        let mut b = a.clone();
        b.target = Some("//b:lib".to_owned());
        let merged = dedupe(vec![a, b]);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].count, 2);
        assert_eq!(merged[0].targets, vec!["//b:lib".to_owned()]);
    }

    fn to_failure_event_for_test() -> BuildFailureEvent {
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
                code: None,
                message: "m".to_owned(),
                path: None,
                line: None,
                column: None,
                related: Vec::new(),
            }],
            count: 1,
            targets: Vec::new(),
        }
    }

    struct BrokenPipeWriter;

    impl std::io::Write for BrokenPipeWriter {
        fn write(&mut self, _buf: &[u8]) -> std::io::Result<usize> {
            Err(std::io::ErrorKind::BrokenPipe.into())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Err(std::io::ErrorKind::BrokenPipe.into())
        }
    }

    #[test]
    fn unavailable_returns_the_stdout_code_when_its_notice_cannot_land() {
        let mut err = Vec::new();
        assert_eq!(
            super::unavailable(
                dx_output::OutputMode::Json,
                &mut BrokenPipeWriter,
                &mut err,
                "gone",
            ),
            Err(128 + 13)
        );
        assert!(err.is_empty(), "{err:?}");
    }

    #[test]
    fn a_notice_that_fails_to_build_ends_the_stream_with_invalid_result() {
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let exit = super::unavailable(dx_output::OutputMode::Json, &mut out, &mut err, "");
        assert_eq!(exit, Err(dx_process::operational_code()));
        let lines: Vec<Value> = String::from_utf8(out)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        assert_eq!(lines.len(), 2, "{lines:?}");
        assert_eq!(lines[0]["event"], "error");
        assert_eq!(lines[0]["code"], "invalid_result");
        assert_eq!(lines[1]["event"], "command_finished");
        assert_eq!(lines[1]["exit_code"], dx_process::operational_code());
    }
}
