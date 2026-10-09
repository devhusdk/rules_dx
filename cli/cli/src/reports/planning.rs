use std::collections::BTreeSet;
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

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Destination {
    Stdout,
    File(String),
}

impl Destination {
    pub fn display(&self) -> &str {
        match self {
            Destination::Stdout => "-",
            Destination::File(path) => path,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedReport {
    pub format: StandardFormat,
    pub destination: Destination,
    pub path: Option<PathBuf>,
}

pub fn resolve_destination(workspace: &Path, raw: &str) -> PathBuf {
    let candidate = Path::new(raw);
    let joined = if candidate.is_absolute() {
        candidate.to_path_buf()
    } else {
        workspace.join(candidate)
    };
    normalize_lexical(&joined)
}

fn normalize_lexical(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if out
                    .components()
                    .next_back()
                    .is_some_and(|last| matches!(last, Component::Normal(_)))
                {
                    out.pop();
                } else if !out.has_root() {
                    out.push("..");
                }
            }
            other => out.push(other.as_os_str()),
        }
    }
    if out.as_os_str().is_empty() {
        PathBuf::from(".")
    } else {
        out
    }
}

fn identity_key(resolved: &Path) -> PathBuf {
    let anchored = if let Ok(canonical) = std::fs::canonicalize(resolved) {
        canonical
    } else if let Some(parent) = resolved.parent() {
        match std::fs::canonicalize(parent) {
            Ok(canonical_parent) => match resolved.file_name() {
                Some(name) => canonical_parent.join(name),
                None => resolved.to_path_buf(),
            },
            Err(_) => resolved.to_path_buf(),
        }
    } else {
        resolved.to_path_buf()
    };
    #[cfg(windows)]
    {
        PathBuf::from(anchored.to_string_lossy().to_lowercase())
    }
    #[cfg(not(windows))]
    {
        anchored
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
    workspace: &Path,
    command: Command,
    requests: &[ReportRequest],
    mode: &OutputMode,
    dry_run: bool,
) -> Result<Vec<PlannedReport>, ReportError> {
    if dry_run && !requests.is_empty() {
        return Err(ReportError::DryRunConflict);
    }
    let entry = spec(command);
    let mut planned = Vec::with_capacity(requests.len());
    let mut seen = BTreeSet::new();
    let mut claimed: Vec<(PathBuf, String)> = Vec::new();
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
            Destination::File(request.destination.clone())
        };
        if !seen.insert((format, destination.clone())) {
            return Err(ReportError::DuplicateReport {
                format: format.name().to_owned(),
                destination: destination.display().to_owned(),
            });
        }
        let path = match &destination {
            Destination::Stdout => None,
            Destination::File(raw) => {
                let resolved = resolve_destination(workspace, raw);
                let key = identity_key(&resolved);
                if let Some((_, first)) = claimed.iter().find(|(claimed, _)| *claimed == key) {
                    return Err(ReportError::ConflictingReports {
                        first: first.clone(),
                        second: format!("{}={}", request.format, request.destination),
                        destination: resolved.display().to_string(),
                    });
                }
                claimed.push((key, format!("{}={}", request.format, request.destination)));
                Some(resolved)
            }
        };
        planned.push(PlannedReport {
            format,
            destination,
            path,
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

    fn workspace() -> tempfile::TempDir {
        tempfile::tempdir().expect("tempdir")
    }

    #[test]
    fn file_reports_plan_in_destination_order() {
        let workspace = workspace();
        let got = plan_reports(
            workspace.path(),
            Command::Lint,
            &requests(&[("sarif", "b.sarif"), ("sarif", "a.sarif")]),
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
        assert_eq!(
            got.iter()
                .map(|report| report.path.clone().expect("resolved"))
                .collect::<Vec<_>>(),
            vec![
                workspace.path().join("a.sarif"),
                workspace.path().join("b.sarif"),
            ]
        );
    }

    #[test]
    fn stdout_report_plans_with_text_mode() {
        let workspace = workspace();
        let got = plan_reports(
            workspace.path(),
            Command::Typecheck,
            &requests(&[("sarif", "-")]),
            &text_mode(),
            false,
        )
        .expect("plan");
        assert_eq!(
            got,
            vec![PlannedReport {
                format: StandardFormat::Sarif,
                destination: Destination::Stdout,
                path: None,
            }]
        );
    }

    #[test]
    fn stdout_report_conflicts_with_diff_and_json_modes() {
        let workspace = workspace();
        for mode in [OutputMode::Diff, OutputMode::Json] {
            assert_eq!(
                plan_reports(
                    workspace.path(),
                    Command::Lint,
                    &requests(&[("sarif", "-")]),
                    &mode,
                    false
                ),
                Err(ReportError::StdoutReportConflictsMode { mode: mode.name() })
            );
        }
        assert_eq!(
            plan_reports(
                workspace.path(),
                Command::Lint,
                &requests(&[("sarif", "-"), ("sarif", "second.sarif".into())]),
                &text_mode(),
                false,
            ),
            Ok(vec![
                PlannedReport {
                    format: StandardFormat::Sarif,
                    destination: Destination::Stdout,
                    path: None,
                },
                PlannedReport {
                    format: StandardFormat::Sarif,
                    destination: Destination::File("second.sarif".to_owned()),
                    path: Some(workspace.path().join("second.sarif")),
                },
            ])
        );
    }

    #[test]
    fn planning_rejects_duplicates_and_second_stdout() {
        let workspace = workspace();
        assert_eq!(
            plan_reports(
                workspace.path(),
                Command::Lint,
                &requests(&[("sarif", "a.sarif"), ("sarif", "a.sarif")]),
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
                workspace.path(),
                Command::Lint,
                &requests(&[("sarif", "-"), ("sarif", "-")]),
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
        let workspace = workspace();
        assert_eq!(
            plan_reports(
                workspace.path(),
                Command::Lint,
                &requests(&[("sarif", "a.sarif")]),
                &text_mode(),
                true,
            ),
            Err(ReportError::DryRunConflict)
        );
        assert_eq!(
            plan_reports(
                workspace.path(),
                Command::Lint,
                &requests(&[("junit", "a.xml")]),
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
                workspace.path(),
                Command::Format,
                &requests(&[("sarif", "a.sarif")]),
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
            plan_reports(workspace.path(), Command::Format, &[], &text_mode(), false)
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
            ReportError::ConflictingReports {
                first: "sarif=out.json".to_owned(),
                second: "spdx=out.json".to_owned(),
                destination: "out.json".to_owned(),
            }
        )
        .contains("same file"));
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
        let workspace = workspace();
        let err = plan_reports(
            workspace.path(),
            Command::Lint,
            &requests(&[("bogus", "a.xml")]),
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
        let workspace = workspace();
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
                workspace.path(),
                command,
                &requests(&[(format, "out.dat")]),
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
                workspace.path(),
                command,
                &requests(&[(format, "out.dat")]),
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

    #[test]
    fn planning_rejects_cross_format_collisions() {
        let workspace = workspace();
        let err = plan_reports(
            workspace.path(),
            Command::License,
            &requests(&[("sarif", "out.json"), ("spdx", "out.json")]),
            &text_mode(),
            false,
        )
        .expect_err("cross-format collision");
        assert_eq!(
            err,
            ReportError::ConflictingReports {
                first: "sarif=out.json".to_owned(),
                second: "spdx=out.json".to_owned(),
                destination: workspace.path().join("out.json").display().to_string(),
            }
        );
        assert!(format!("{err}").contains("same file"), "{err}");
    }

    #[test]
    fn planning_rejects_relative_alias_collisions() {
        let workspace = workspace();
        for alias in ["./a.sarif", "sub/../a.sarif", "sub//../a.sarif"] {
            let err = plan_reports(
                workspace.path(),
                Command::Lint,
                &requests(&[("sarif", "a.sarif"), ("sarif", alias)]),
                &text_mode(),
                false,
            )
            .expect_err("alias collision");
            assert_eq!(
                err,
                ReportError::ConflictingReports {
                    first: "sarif=a.sarif".to_owned(),
                    second: format!("sarif={alias}"),
                    destination: workspace.path().join("a.sarif").display().to_string(),
                },
                "alias {alias} must collide"
            );
        }
    }

    #[test]
    fn planning_rejects_absolute_and_relative_aliases() {
        let workspace = workspace();
        let absolute = workspace.path().join("out.sarif").display().to_string();
        let err = plan_reports(
            workspace.path(),
            Command::Lint,
            &requests(&[("sarif", "out.sarif"), ("sarif", &absolute)]),
            &text_mode(),
            false,
        )
        .expect_err("absolute alias collision");
        assert_eq!(
            err,
            ReportError::ConflictingReports {
                first: "sarif=out.sarif".to_owned(),
                second: format!("sarif={absolute}"),
                destination: absolute,
            }
        );
    }

    #[test]
    fn planning_accepts_distinct_files_with_resolved_paths() {
        let workspace = workspace();
        let got = plan_reports(
            workspace.path(),
            Command::License,
            &requests(&[("sarif", "a.sarif"), ("spdx", "b.json")]),
            &text_mode(),
            false,
        )
        .expect("distinct files plan");
        assert_eq!(
            got.iter()
                .map(|report| report.path.clone().expect("resolved"))
                .collect::<Vec<_>>(),
            vec![
                workspace.path().join("a.sarif"),
                workspace.path().join("b.json"),
            ]
        );
    }

    #[test]
    fn resolve_destination_anchors_relative_paths_to_the_workspace() {
        let workspace = workspace();
        assert_eq!(
            resolve_destination(workspace.path(), "out.sarif"),
            workspace.path().join("out.sarif")
        );
        assert_eq!(
            resolve_destination(workspace.path(), "./sub/../out.sarif"),
            workspace.path().join("out.sarif")
        );
        let absolute = workspace.path().join("out.sarif").display().to_string();
        assert_eq!(
            resolve_destination(workspace.path(), &absolute),
            workspace.path().join("out.sarif")
        );
        let outside = workspace
            .path()
            .parent()
            .expect("workspace parent")
            .join("shared-out.sarif")
            .display()
            .to_string();
        assert_eq!(
            resolve_destination(workspace.path(), "../shared-out.sarif")
                .display()
                .to_string(),
            outside
        );
    }

    #[cfg(unix)]
    #[test]
    fn planning_rejects_symlink_alias_collisions() {
        use std::os::unix::fs::symlink;
        let workspace = workspace();
        std::fs::write(workspace.path().join("real.sarif"), "{}\n").expect("real");
        symlink(
            workspace.path().join("real.sarif"),
            workspace.path().join("link.sarif"),
        )
        .expect("symlink");
        let err = plan_reports(
            workspace.path(),
            Command::Lint,
            &requests(&[("sarif", "real.sarif"), ("sarif", "link.sarif")]),
            &text_mode(),
            false,
        )
        .expect_err("symlink collision");
        assert!(
            matches!(err, ReportError::ConflictingReports { .. }),
            "symlink alias must collide: {err}"
        );
    }

    #[test]
    fn planning_treats_case_variants_per_platform() {
        let workspace = workspace();
        let result = plan_reports(
            workspace.path(),
            Command::Lint,
            &requests(&[("sarif", "out.sarif"), ("sarif", "OUT.sarif")]),
            &text_mode(),
            false,
        );
        if cfg!(windows) {
            assert!(
                matches!(result, Err(ReportError::ConflictingReports { .. })),
                "case variants collide on Windows"
            );
        } else {
            assert!(result.is_ok(), "case variants stay distinct here");
        }
    }
}
