use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::path::{Component, Path, PathBuf};

use super::ReportError;
use crate::args::{Command, ReportRequest};
use crate::plan::spec;
use dx_output::{check_output_conflict, OutputError, OutputMode};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum StandardFormat {
    Sarif,
    Junit,
    Lcov,
    Spdx,
}

impl StandardFormat {
    pub fn name(self) -> &'static str {
        match self {
            StandardFormat::Sarif => "sarif",
            StandardFormat::Junit => "junit",
            StandardFormat::Lcov => "lcov",
            StandardFormat::Spdx => "spdx",
        }
    }

    pub(crate) fn parse(text: &str) -> Option<Self> {
        match text {
            "sarif" => Some(StandardFormat::Sarif),
            "junit" => Some(StandardFormat::Junit),
            "lcov" => Some(StandardFormat::Lcov),
            "spdx" => Some(StandardFormat::Spdx),
            _ => None,
        }
    }
}

pub fn format_names(formats: &[StandardFormat]) -> Vec<&'static str> {
    formats.iter().map(|format| format.name()).collect()
}

/// One report destination as requested and as resolved on disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportPath {
    requested: String,
    resolved: PathBuf,
}

impl ReportPath {
    pub fn requested(&self) -> &str {
        &self.requested
    }

    pub fn resolved(&self) -> &Path {
        &self.resolved
    }
}

impl Ord for ReportPath {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.requested.cmp(&other.requested)
    }
}

impl PartialOrd for ReportPath {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Destination {
    Stdout,
    File(ReportPath),
}

impl Destination {
    pub fn display(&self) -> &str {
        match self {
            Destination::Stdout => "-",
            Destination::File(path) => path.requested(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedReport {
    pub format: StandardFormat,
    pub destination: Destination,
}

impl PlannedReport {
    /// The absolute file one report writes to, or `None` when it goes to stdout.
    pub fn resolved_path(&self) -> Option<&Path> {
        match &self.destination {
            Destination::Stdout => None,
            Destination::File(path) => Some(path.resolved()),
        }
    }
}

/// Drops `.` and folds `..` without asking the filesystem.
fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                let folds = out
                    .components()
                    .next_back()
                    .is_some_and(|last| matches!(last, Component::Normal(_)))
                    && out.pop();
                if !folds {
                    out.push(Component::ParentDir);
                }
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// Anchors one destination at the workspace root unless it is already absolute.
pub fn resolve_report_path(workspace: &Path, requested: &str) -> ReportPath {
    let raw = Path::new(requested);
    let anchored = if raw.is_absolute() {
        raw.to_path_buf()
    } else {
        workspace.join(raw)
    };
    ReportPath {
        requested: requested.to_owned(),
        resolved: normalize(&anchored),
    }
}

/// Canonicalizes the longest existing prefix so aliases of one file share an identity.
fn existing_prefix(path: &Path) -> PathBuf {
    let mut head = path.to_path_buf();
    let mut tail: Vec<OsString> = Vec::new();
    loop {
        if let Ok(real) = head.canonicalize() {
            let mut joined = real;
            for name in tail.iter().rev() {
                joined.push(name);
            }
            return joined;
        }
        let Some(name) = head.file_name().map(OsString::from) else {
            return path.to_path_buf();
        };
        tail.push(name);
        if !head.pop() {
            return path.to_path_buf();
        }
    }
}

/// The identity two report destinations may not share.
fn collision_key(path: &Path) -> String {
    let identity = existing_prefix(path).to_string_lossy().into_owned();
    if cfg!(windows) {
        identity.to_lowercase()
    } else {
        identity
    }
}

fn stdout_conflict(error: OutputError) -> ReportError {
    match error {
        OutputError::SecondStdoutReport => ReportError::MultipleStdoutReports,
        OutputError::ConflictingStdoutReport { mode } => {
            ReportError::StdoutReportConflictsMode { mode }
        }
        unexpected => ReportError::UnexpectedOutputConflict {
            detail: unexpected.to_string(),
        },
    }
}

pub fn plan_reports(
    command: Command,
    requests: &[ReportRequest],
    workspace: &Path,
    mode: &OutputMode,
    dry_run: bool,
) -> Result<Vec<PlannedReport>, ReportError> {
    if dry_run && !requests.is_empty() {
        return Err(ReportError::DryRunConflict);
    }
    let entry = spec(command);
    let mut planned = Vec::with_capacity(requests.len());
    let mut seen = BTreeSet::new();
    let mut claimed: BTreeMap<String, String> = BTreeMap::new();
    for request in requests {
        let format = StandardFormat::parse(&request.format).ok_or_else(|| {
            ReportError::UnsupportedFormat {
                command: command.name(),
                format: request.format.clone(),
                supported: format_names(entry.reports),
            }
        })?;
        if !entry.reports.contains(&format) {
            return Err(ReportError::UnsupportedFormat {
                command: command.name(),
                format: request.format.clone(),
                supported: format_names(entry.reports),
            });
        }
        let destination = if request.destination == "-" {
            Destination::Stdout
        } else {
            Destination::File(resolve_report_path(workspace, &request.destination))
        };
        if !seen.insert((format, destination.clone())) {
            return Err(ReportError::DuplicateReport {
                format: format.name().to_owned(),
                destination: destination.display().to_owned(),
            });
        }
        if let Destination::File(path) = &destination {
            let key = collision_key(path.resolved());
            if let Some(first) = claimed.get(&key) {
                return Err(ReportError::DestinationCollision {
                    first: first.clone(),
                    second: path.requested().to_owned(),
                    resolved: path.resolved().display().to_string(),
                });
            }
            claimed.insert(key, path.requested().to_owned());
        }
        planned.push(PlannedReport {
            format,
            destination,
        });
    }
    let stdout_reports = planned
        .iter()
        .filter(|report| report.destination == Destination::Stdout)
        .count();
    check_output_conflict(mode, stdout_reports).map_err(stdout_conflict)?;
    planned.sort_by(|a, b| {
        (a.format, a.destination.display()).cmp(&(b.format, b.destination.display()))
    });
    Ok(planned)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn requests(pairs: &[(&str, &str)]) -> Vec<ReportRequest> {
        pairs
            .iter()
            .map(|(format, destination)| ReportRequest {
                format: (*format).to_owned(),
                destination: (*destination).to_owned(),
            })
            .collect()
    }

    fn text_mode() -> OutputMode {
        OutputMode::Text { quiet: false }
    }

    fn ws() -> &'static Path {
        Path::new("/ws")
    }

    #[test]
    fn destinations_resolve_against_the_workspace_root() {
        for (requested, resolved) in [
            ("out.sarif", "/ws/out.sarif"),
            ("./out.sarif", "/ws/out.sarif"),
            ("a//out.sarif", "/ws/a/out.sarif"),
            ("a/../out.sarif", "/ws/out.sarif"),
            ("../out.sarif", "/out.sarif"),
            ("/elsewhere/out.sarif", "/elsewhere/out.sarif"),
            ("/elsewhere/../out.sarif", "/out.sarif"),
        ] {
            let got = resolve_report_path(ws(), requested);
            assert_eq!(got.requested(), requested);
            assert_eq!(got.resolved(), Path::new(resolved), "{requested}");
        }
    }

    #[test]
    fn one_file_may_only_hold_one_report() {
        for (pair, resolved) in [
            ([("sarif", "out.dat"), ("spdx", "out.dat")], "/ws/out.dat"),
            (
                [("sarif", "out.sarif"), ("spdx", "./out.sarif")],
                "/ws/out.sarif",
            ),
            (
                [("sarif", "out.sarif"), ("spdx", "a/../out.sarif")],
                "/ws/out.sarif",
            ),
            (
                [("sarif", "/ws/out.sarif"), ("spdx", "out.sarif")],
                "/ws/out.sarif",
            ),
        ] {
            assert_eq!(
                plan_reports(
                    Command::License,
                    &requests(&pair),
                    ws(),
                    &text_mode(),
                    false
                ),
                Err(ReportError::DestinationCollision {
                    first: pair[0].1.to_owned(),
                    second: pair[1].1.to_owned(),
                    resolved: resolved.to_owned(),
                }),
                "{pair:?}"
            );
        }
    }

    #[test]
    fn file_reports_plan_in_destination_order() {
        let got = plan_reports(
            Command::Lint,
            &requests(&[("sarif", "b.sarif"), ("sarif", "a.sarif")]),
            ws(),
            &text_mode(),
            false,
        )
        .expect("plan");
        assert_eq!(
            got.iter()
                .map(|report| report.destination.display().to_owned())
                .collect::<Vec<_>>(),
            vec!["a.sarif".to_owned(), "b.sarif".to_owned()]
        );
    }

    #[test]
    fn stdout_report_plans_with_text_mode() {
        let got = plan_reports(
            Command::Typecheck,
            &requests(&[("sarif", "-")]),
            ws(),
            &text_mode(),
            false,
        )
        .expect("plan");
        assert_eq!(
            got,
            vec![PlannedReport {
                format: StandardFormat::Sarif,
                destination: Destination::Stdout,
            }]
        );
    }

    #[test]
    fn stdout_report_conflicts_with_diff_and_json_modes() {
        for mode in [OutputMode::Diff, OutputMode::Json] {
            assert_eq!(
                plan_reports(
                    Command::Lint,
                    &requests(&[("sarif", "-")]),
                    ws(),
                    &mode,
                    false
                ),
                Err(ReportError::StdoutReportConflictsMode { mode: mode.name() })
            );
        }
        assert_eq!(
            plan_reports(
                Command::Lint,
                &requests(&[("sarif", "-"), ("sarif", "second.sarif".into())]),
                ws(),
                &text_mode(),
                false,
            ),
            Ok(vec![
                PlannedReport {
                    format: StandardFormat::Sarif,
                    destination: Destination::Stdout,
                },
                PlannedReport {
                    format: StandardFormat::Sarif,
                    destination: Destination::File(resolve_report_path(ws(), "second.sarif")),
                },
            ])
        );
    }

    #[test]
    fn planning_rejects_duplicates_and_second_stdout() {
        assert_eq!(
            plan_reports(
                Command::Lint,
                &requests(&[("sarif", "a.sarif"), ("sarif", "a.sarif")]),
                ws(),
                &text_mode(),
                false,
            ),
            Err(ReportError::DuplicateReport {
                format: "sarif".to_owned(),
                destination: "a.sarif".to_owned(),
            })
        );
        assert_eq!(
            plan_reports(
                Command::Lint,
                &requests(&[("sarif", "-"), ("sarif", "-")]),
                ws(),
                &text_mode(),
                false,
            ),
            Err(ReportError::DuplicateReport {
                format: "sarif".to_owned(),
                destination: "-".to_owned(),
            })
        );
    }

    #[test]
    fn planning_rejects_dry_run_and_unsupported_formats() {
        assert_eq!(
            plan_reports(
                Command::Lint,
                &requests(&[("sarif", "a.sarif")]),
                ws(),
                &text_mode(),
                true,
            ),
            Err(ReportError::DryRunConflict)
        );
        assert_eq!(
            plan_reports(
                Command::Lint,
                &requests(&[("junit", "a.xml")]),
                ws(),
                &text_mode(),
                false,
            ),
            Err(ReportError::UnsupportedFormat {
                command: "lint",
                format: "junit".to_owned(),
                supported: vec!["sarif"],
            })
        );
        assert_eq!(
            plan_reports(
                Command::Format,
                &requests(&[("sarif", "a.sarif")]),
                ws(),
                &text_mode(),
                false,
            ),
            Err(ReportError::UnsupportedFormat {
                command: "format",
                format: "sarif".to_owned(),
                supported: vec![],
            })
        );
        assert!(
            plan_reports(Command::Format, &[], ws(), &text_mode(), false)
                .expect("plan")
                .is_empty()
        );
    }

    #[test]
    fn error_display_reports_variant() {
        assert_eq!(
            stdout_conflict(OutputError::SecondStdoutReport),
            ReportError::MultipleStdoutReports
        );
        assert!(format!("{}", ReportError::DryRunConflict).contains("dry-run"));
        assert!(format!(
            "{}",
            ReportError::UnsupportedFormat {
                command: "lint",
                format: "junit".to_owned(),
                supported: vec![],
            }
        )
        .contains("no standard report exists"));
        assert!(format!(
            "{}",
            ReportError::UnsupportedFormat {
                command: "lint",
                format: "junit".to_owned(),
                supported: vec!["sarif"],
            }
        )
        .contains("sarif"));
        assert!(format!(
            "{}",
            ReportError::DuplicateReport {
                format: "sarif".to_owned(),
                destination: "out.sarif".to_owned(),
            }
        )
        .contains("duplicate"));
        assert!(format!(
            "{}",
            ReportError::DestinationCollision {
                first: "out.sarif".to_owned(),
                second: "./out.sarif".to_owned(),
                resolved: "/ws/out.sarif".to_owned(),
            }
        )
        .contains("/ws/out.sarif"));
        assert!(format!("{}", ReportError::MultipleStdoutReports).contains("stdout"));
        assert!(format!(
            "{}",
            ReportError::StdoutReportConflictsMode { mode: "diff" }
        )
        .contains("diff"));
        assert!(format!(
            "{}",
            ReportError::InvalidFinding {
                detail: "empty tool",
            }
        )
        .contains("empty tool"));
        assert!(format!("{}", ReportError::RangeWithoutPath).contains("byte range"));
        assert!(format!("{}", ReportError::InvertedRange).contains("starts after"));
        assert!(format!(
            "{}",
            ReportError::MissingSnapshot {
                path: "src/a.py".to_owned(),
            }
        )
        .contains("src/a.py"));
        assert!(format!(
            "{}",
            ReportError::UnknownTool {
                tool: "other".to_owned(),
            }
        )
        .contains("other"));
        assert!(format!(
            "{}",
            ReportError::BadOffset {
                path: "src/a.py".to_owned(),
                offset: 1,
            }
        )
        .contains("character boundary"));
        assert!(format!(
            "{}",
            ReportError::InvalidJunit {
                detail: "boom".to_owned(),
            }
        )
        .contains("boom"));
        assert!(format!(
            "{}",
            ReportError::InvalidLcov {
                detail: "boom".to_owned(),
            }
        )
        .contains("boom"));
        assert!(format!(
            "{}",
            ReportError::UnexpectedOutputConflict {
                detail: "unknown output mode \"xml\"".to_owned(),
            }
        )
        .contains("unexpected output conflict"));
        assert!(format!(
            "{}",
            ReportError::JunitRender {
                detail: "boom".to_owned(),
            }
        )
        .contains("junit report serialization failed"));
        assert!(format!(
            "{}",
            ReportError::Fingerprint(dx_fingerprint::FingerprintError::Json {
                detail: "boom".to_owned(),
            })
        )
        .contains("fingerprint JSON serializes"));
    }

    #[test]
    fn unexpected_output_conflicts_fail_closed_typed() {
        assert_eq!(
            stdout_conflict(OutputError::UnknownOutputMode {
                value: "xml".to_owned(),
            }),
            ReportError::UnexpectedOutputConflict {
                detail: "unknown output mode \"xml\"".to_owned(),
            }
        );
        assert_eq!(
            stdout_conflict(OutputError::BadDigest {
                field: "d",
                value: "zz".to_owned(),
            }),
            ReportError::UnexpectedOutputConflict {
                detail: "invalid digest for d \"zz\"".to_owned(),
            }
        );
    }

    #[test]
    fn unknown_format_is_unsupported_before_registry() {
        let err = plan_reports(
            Command::Lint,
            &requests(&[("bogus", "a.xml")]),
            ws(),
            &text_mode(),
            false,
        )
        .expect_err("bogus");
        assert_eq!(
            err,
            ReportError::UnsupportedFormat {
                command: "lint",
                format: "bogus".to_owned(),
                supported: vec!["sarif"],
            }
        );
    }

    #[test]
    fn execution_gaps_report_matrix_is_wont_fix() {
        for (command, format) in [
            (Command::Lint, "sarif"),
            (Command::Typecheck, "sarif"),
            (Command::Check, "sarif"),
            (Command::Fix, "sarif"),
            (Command::Test, "junit"),
            (Command::Coverage, "lcov"),
            (Command::Security, "sarif"),
            (Command::License, "sarif"),
            (Command::License, "spdx"),
        ] {
            plan_reports(
                command,
                &requests(&[(format, "out.dat")]),
                ws(),
                &text_mode(),
                false,
            )
            .expect("supported combo must plan");
        }
        for (command, format) in [
            (Command::Lint, "junit"),
            (Command::Test, "sarif"),
            (Command::Coverage, "sarif"),
            (Command::Build, "sarif"),
            (Command::Format, "sarif"),
            (Command::Generate, "sarif"),
            (Command::Update, "sarif"),
            (Command::Run, "junit"),
            (Command::Security, "spdx"),
        ] {
            let err = plan_reports(
                command,
                &requests(&[(format, "out.dat")]),
                ws(),
                &text_mode(),
                false,
            )
            .expect_err("unsupported combo must fail");
            let want = spec(command);
            assert_eq!(
                err,
                ReportError::UnsupportedFormat {
                    command: command.name(),
                    format: format.to_owned(),
                    supported: format_names(want.reports),
                },
                "{command:?} {format} must stay unsupported"
            );
        }
        assert!(spec(Command::Format).reports.is_empty());
    }
}
