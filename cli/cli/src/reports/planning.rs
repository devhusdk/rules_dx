use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};

use super::{ReportError, ReportWriteCause, ReportWriteError};
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

pub fn normalize_report_destination(raw: &str) -> String {
    let mut prefix = String::new();
    let mut stack: Vec<String> = Vec::new();
    for component in Path::new(raw).components() {
        match component {
            Component::Prefix(part) => {
                prefix.push_str(&part.as_os_str().to_string_lossy());
            }
            Component::RootDir => prefix.push('/'),
            Component::CurDir => {}
            Component::ParentDir => {
                if stack.pop().is_none() && prefix.is_empty() {
                    stack.push("..".to_owned());
                }
            }
            Component::Normal(part) => {
                stack.push(part.to_string_lossy().into_owned());
            }
        }
    }
    let mut out = prefix;
    out.push_str(&stack.join("/"));
    if out.is_empty() {
        out.push('.');
    }
    out
}

fn collision_key_for(normalized: &str, case_insensitive: bool) -> String {
    if case_insensitive {
        normalized.to_lowercase()
    } else {
        normalized.to_owned()
    }
}

fn collision_key(normalized: &str) -> String {
    collision_key_for(normalized, cfg!(windows))
}

pub fn resolve_file_report_path(workspace: &Path, raw: &str) -> PathBuf {
    let normalized = normalize_report_destination(raw);
    let path = Path::new(&normalized);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        workspace.join(path)
    }
}

pub fn resolve_report_path(workspace: &Path, destination: &Destination) -> Option<PathBuf> {
    match destination {
        Destination::Stdout => None,
        Destination::File(raw) => Some(resolve_file_report_path(workspace, raw)),
    }
}

pub fn write_report_target(
    workspace: &Path,
    raw: &str,
    document: &[u8],
    write: impl FnOnce(&Path, &[u8]) -> std::io::Result<()>,
) -> Result<(), ReportWriteError> {
    let target = resolve_file_report_path(workspace, raw);
    let resolved = target.to_string_lossy().into_owned();
    if let Some(parent) = target.parent().filter(|path| !path.as_os_str().is_empty()) {
        if !parent.exists() {
            return Err(ReportWriteError {
                destination: resolved,
                cause: ReportWriteCause::MissingParent {
                    parent: parent.to_string_lossy().into_owned(),
                },
            });
        }
        if !parent.is_dir() {
            return Err(ReportWriteError {
                destination: resolved,
                cause: ReportWriteCause::ParentNotDirectory {
                    parent: parent.to_string_lossy().into_owned(),
                },
            });
        }
    }
    write(&target, document).map_err(|error| ReportWriteError {
        destination: resolved,
        cause: ReportWriteCause::Io {
            detail: error.to_string(),
        },
    })
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
    let mut seen_files: BTreeMap<String, (StandardFormat, String)> = BTreeMap::new();
    let mut seen_stdout: BTreeSet<StandardFormat> = BTreeSet::new();
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
        match &destination {
            Destination::Stdout => {
                if !seen_stdout.insert(format) {
                    return Err(ReportError::DuplicateReport {
                        format: format.name().to_owned(),
                        destination: destination.display().to_owned(),
                    });
                }
            }
            Destination::File(raw) => {
                let key = collision_key(&normalize_report_destination(raw));
                match seen_files.insert(key, (format, destination.display().to_owned())) {
                    None => {}
                    Some((first, _)) if first == format => {
                        return Err(ReportError::DuplicateReport {
                            format: format.name().to_owned(),
                            destination: destination.display().to_owned(),
                        });
                    }
                    Some((first, _)) => {
                        return Err(ReportError::ConflictingDestinations {
                            first: first.name().to_owned(),
                            second: format.name().to_owned(),
                            destination: normalize_report_destination(request.destination.as_str()),
                        });
                    }
                }
            }
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
    fn cross_format_same_destination_conflicts_before_any_write() {
        assert_eq!(
            plan_reports(
                Command::License,
                &requests(&[("sarif", "out.sarif"), ("spdx", "out.sarif")]),
                &text_mode(),
                false,
            ),
            Err(ReportError::ConflictingDestinations {
                first: "sarif".to_owned(),
                second: "spdx".to_owned(),
                destination: "out.sarif".to_owned(),
            })
        );
    }

    #[test]
    fn normalized_aliases_share_one_destination() {
        assert_eq!(
            plan_reports(
                Command::Lint,
                &requests(&[("sarif", "out.sarif"), ("sarif", "./sub/../out.sarif")]),
                &text_mode(),
                false,
            ),
            Err(ReportError::DuplicateReport {
                format: "sarif".to_owned(),
                destination: "./sub/../out.sarif".to_owned(),
            })
        );
        assert_eq!(
            plan_reports(
                Command::License,
                &requests(&[("sarif", "out.sarif"), ("spdx", "sub/../out.sarif")]),
                &text_mode(),
                false,
            ),
            Err(ReportError::ConflictingDestinations {
                first: "sarif".to_owned(),
                second: "spdx".to_owned(),
                destination: "out.sarif".to_owned(),
            })
        );
        plan_reports(
            Command::License,
            &requests(&[("sarif", "sarif-out.sarif"), ("spdx", "spdx-out.spdx")]),
            &text_mode(),
            false,
        )
        .expect("distinct destinations plan");
    }

    #[test]
    fn normalize_report_destination_collapses_aliases() {
        for (raw, want) in [
            ("out.sarif", "out.sarif"),
            ("./out.sarif", "out.sarif"),
            ("sub/../out.sarif", "out.sarif"),
            ("a/./b.sarif", "a/b.sarif"),
            ("a//b.sarif", "a/b.sarif"),
            ("../out.sarif", "../out.sarif"),
            ("a/../../out.sarif", "../out.sarif"),
            ("/abs/out.sarif", "/abs/out.sarif"),
            ("/a/../out.sarif", "/out.sarif"),
            ("/../out.sarif", "/out.sarif"),
        ] {
            assert_eq!(normalize_report_destination(raw), want, "raw {raw:?}");
        }
    }

    #[test]
    fn resolve_report_path_anchors_relative_and_keeps_absolute() {
        let workspace = Path::new("/ws");
        assert_eq!(
            resolve_report_path(
                workspace,
                &Destination::File("./sub/../out.sarif".to_owned())
            ),
            Some(PathBuf::from("/ws/out.sarif"))
        );
        assert_eq!(
            resolve_report_path(workspace, &Destination::File("/tmp/out.sarif".to_owned())),
            Some(PathBuf::from("/tmp/out.sarif"))
        );
        assert_eq!(resolve_report_path(workspace, &Destination::Stdout), None);
    }

    #[test]
    fn write_report_target_names_parent_and_cause() {
        let dir = tempfile::tempdir().expect("tempdir");
        let missing = write_report_target(dir.path(), "nodir/out.sarif", b"{}\n", |_, _| {
            panic!("must not write without a parent")
        })
        .expect_err("missing parent");
        assert_eq!(
            missing.destination,
            dir.path().join("nodir/out.sarif").to_string_lossy()
        );
        assert_eq!(
            missing.message("sarif"),
            format!(
                "failed to write sarif report to {}: parent directory {:?} does not exist",
                dir.path().join("nodir/out.sarif").to_string_lossy(),
                dir.path().join("nodir").to_string_lossy(),
            )
        );
        assert!(!dir.path().join("nodir").exists());

        std::fs::write(dir.path().join("blocker"), b"stale").expect("blocker");
        let blocked = write_report_target(dir.path(), "blocker/out.sarif", b"{}\n", |_, _| {
            panic!("must not write through a file")
        })
        .expect_err("parent is a file");
        assert!(blocked.message("sarif").contains("is not a directory"));
        assert_eq!(
            std::fs::read(dir.path().join("blocker")).expect("blocker intact"),
            b"stale"
        );

        let failed = write_report_target(dir.path(), "out.sarif", b"{}\n", |_, _| {
            Err(std::io::Error::new(std::io::ErrorKind::Other, "boom"))
        })
        .expect_err("io failure");
        assert!(failed.message("sarif").contains("write failed: boom"));
        assert!(!dir.path().join("out.sarif").exists());

        write_report_target(dir.path(), "out.sarif", b"{}\n", |target, bytes| {
            assert_eq!(target, dir.path().join("out.sarif"));
            std::fs::write(target, bytes)
        })
        .expect("write");
        assert_eq!(
            std::fs::read(dir.path().join("out.sarif")).expect("report"),
            b"{}\n"
        );
    }

    #[test]
    fn collision_key_folds_case_only_where_the_platform_needs_it() {
        assert_eq!(collision_key_for("OUT.SARIF", true), "out.sarif");
        assert_eq!(collision_key_for("OUT.SARIF", false), "OUT.SARIF");
        if cfg!(windows) {
            assert_eq!(collision_key("OUT.SARIF"), "out.sarif");
        } else {
            assert_eq!(collision_key("OUT.SARIF"), "OUT.SARIF");
        }
    }

    #[test]
    fn conflicting_destinations_and_write_errors_display() {
        assert!(format!(
            "{}",
            ReportError::ConflictingDestinations {
                first: "sarif".to_owned(),
                second: "spdx".to_owned(),
                destination: "out.sarif".to_owned(),
            }
        )
        .contains("use distinct destinations"));
        let error = crate::reports::ReportWriteError {
            destination: "out.sarif".to_owned(),
            cause: crate::reports::ReportWriteCause::MissingParent {
                parent: "nodir".to_owned(),
            },
        };
        assert!(format!("{error}").contains("nodir"));
        assert!(error
            .message("sarif")
            .starts_with("failed to write sarif report to out.sarif: parent directory"));
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
