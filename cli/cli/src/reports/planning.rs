use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

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

/// Lexical key for comparing report destinations.
/// Symlinks are never resolved; Windows folds case.
pub fn destination_key(text: &str) -> String {
    let key = normalize_lexical(Path::new(text))
        .to_string_lossy()
        .into_owned();
    #[cfg(windows)]
    {
        key.to_lowercase()
    }
    #[cfg(not(windows))]
    {
        key
    }
}

fn normalize_lexical(path: &Path) -> std::path::PathBuf {
    use std::path::{Component, PathBuf};
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(part) => out.push(part.as_os_str()),
            Component::RootDir => out.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                if out.file_name().is_some() {
                    out.pop();
                } else if out.as_os_str().is_empty() || out.ends_with("..") {
                    out.push("..");
                }
            }
            Component::Normal(part) => out.push(part),
        }
    }
    if out.as_os_str().is_empty() {
        out.push(".");
    }
    out
}

/// Anchors a report destination at the workspace root.
/// Absolute destinations are used as written.
pub fn resolve_destination(workspace: &Path, destination: &str) -> std::path::PathBuf {
    let path = Path::new(destination);
    let joined = if path.is_absolute() {
        path.to_path_buf()
    } else {
        workspace.join(path)
    };
    normalize_lexical(&joined)
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
    mode: &OutputMode,
    dry_run: bool,
) -> Result<Vec<PlannedReport>, ReportError> {
    if dry_run && !requests.is_empty() {
        return Err(ReportError::DryRunConflict);
    }
    let entry = spec(command);
    let mut planned = Vec::with_capacity(requests.len());
    let mut seen = BTreeSet::new();
    let mut owners: BTreeMap<String, StandardFormat> = BTreeMap::new();
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
        let key = destination_key(&request.destination);
        if !seen.insert((format, key.clone())) {
            return Err(ReportError::DuplicateReport {
                format: format.name().to_owned(),
                destination: destination.display().to_owned(),
            });
        }
        if destination != Destination::Stdout {
            if let Some(owner) = owners.get(&key) {
                return Err(ReportError::ConflictingDestinations {
                    first: owner.name().to_owned(),
                    second: format.name().to_owned(),
                    destination: destination.display().to_owned(),
                });
            }
            owners.insert(key, format);
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

    #[test]
    fn file_reports_plan_in_destination_order() {
        let got = plan_reports(
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
    }

    #[test]
    fn stdout_report_plans_with_text_mode() {
        let got = plan_reports(
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
            }]
        );
    }

    #[test]
    fn stdout_report_conflicts_with_diff_and_json_modes() {
        for mode in [OutputMode::Diff, OutputMode::Json] {
            assert_eq!(
                plan_reports(Command::Lint, &requests(&[("sarif", "-")]), &mode, false),
                Err(ReportError::StdoutReportConflictsMode { mode: mode.name() })
            );
        }
        assert_eq!(
            plan_reports(
                Command::Lint,
                &requests(&[("sarif", "-"), ("sarif", "second.sarif".into())]),
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
                    destination: Destination::File("second.sarif".to_owned()),
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
    fn planning_rejects_cross_format_collisions_on_one_file() {
        assert_eq!(
            plan_reports(
                Command::License,
                &requests(&[("sarif", "out.dat"), ("spdx", "out.dat")]),
                &text_mode(),
                false,
            ),
            Err(ReportError::ConflictingDestinations {
                first: "sarif".to_owned(),
                second: "spdx".to_owned(),
                destination: "out.dat".to_owned(),
            })
        );
        assert_eq!(
            plan_reports(
                Command::License,
                &requests(&[("sarif", "out.dat"), ("spdx", "./out.dat")]),
                &text_mode(),
                false,
            ),
            Err(ReportError::ConflictingDestinations {
                first: "sarif".to_owned(),
                second: "spdx".to_owned(),
                destination: "./out.dat".to_owned(),
            })
        );
        assert_eq!(
            plan_reports(
                Command::License,
                &requests(&[("sarif", "sub/../out.dat"), ("spdx", "out.dat")]),
                &text_mode(),
                false,
            ),
            Err(ReportError::ConflictingDestinations {
                first: "sarif".to_owned(),
                second: "spdx".to_owned(),
                destination: "out.dat".to_owned(),
            })
        );
        plan_reports(
            Command::License,
            &requests(&[("sarif", "one.dat"), ("spdx", "other.dat")]),
            &text_mode(),
            false,
        )
        .expect("distinct files plan");
    }

    #[test]
    fn destination_keys_collapse_aliases_without_touching_the_filesystem() {
        assert_eq!(destination_key("out.sarif"), "out.sarif");
        assert_eq!(destination_key("./out.sarif"), "out.sarif");
        assert_eq!(destination_key("sub/../out.sarif"), "out.sarif");
        assert_eq!(destination_key("reports//lint.sarif"), "reports/lint.sarif");
        assert_eq!(
            destination_key("../shared/out.sarif"),
            "../shared/out.sarif"
        );
        assert_eq!(destination_key("../../out.sarif"), "../../out.sarif");
        assert_eq!(destination_key("/ws/./out.sarif"), "/ws/out.sarif");
        assert_eq!(destination_key("-"), "-");
        assert_ne!(destination_key("out.sarif"), destination_key("other.sarif"));
    }

    #[test]
    fn resolved_destinations_anchor_relative_paths_at_the_workspace() {
        let workspace = std::path::Path::new("/ws");
        assert_eq!(
            resolve_destination(workspace, "out.sarif"),
            std::path::PathBuf::from("/ws/out.sarif")
        );
        assert_eq!(
            resolve_destination(workspace, "sub/../out.sarif"),
            std::path::PathBuf::from("/ws/out.sarif")
        );
        assert_eq!(
            resolve_destination(workspace, "/tmp/extra.sarif"),
            std::path::PathBuf::from("/tmp/extra.sarif")
        );
        assert_eq!(
            resolve_destination(workspace, "../shared/out.sarif"),
            std::path::PathBuf::from("/shared/out.sarif")
        );
    }

    #[test]
    fn conflicting_destinations_render_an_actionable_message() {
        let message = ReportError::ConflictingDestinations {
            first: "sarif".to_owned(),
            second: "spdx".to_owned(),
            destination: "out.dat".to_owned(),
        }
        .to_string();
        assert!(message.contains("sarif"), "{message}");
        assert!(message.contains("spdx"), "{message}");
        assert!(message.contains("out.dat"), "{message}");
        assert!(message.contains("distinct"), "{message}");
    }

    #[test]
    fn planning_rejects_dry_run_and_unsupported_formats() {
        assert_eq!(
            plan_reports(
                Command::Lint,
                &requests(&[("sarif", "a.sarif")]),
                &text_mode(),
                true,
            ),
            Err(ReportError::DryRunConflict)
        );
        assert_eq!(
            plan_reports(
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
        assert!(plan_reports(Command::Format, &[], &text_mode(), false)
            .expect("plan")
            .is_empty());
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
