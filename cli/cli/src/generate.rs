use dx_diff::{render_patch, DiffError, FilePatch, PatchKind};
use dx_output::{ChangeEvent, ChangeKind, Edit, FinishedCounts, MutationOutcome, NoticeEvent};
use generation_result::{
    candidate,
    proto::{file_result, GenerationManifest, Mode, WriteOutcome},
    validate, Error,
};

pub const IGNORED_IMPORT_CODE: &str = "ignored_import";
pub const IGNORED_IMPORT_LEVEL: &str = "warning";
pub const IGNORED_IMPORT_MESSAGE: &str =
    "Static import intentionally contributes no Bazel dependency";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectedFile {
    pub change: ChangeEvent,
    pub original: String,
    pub candidate: String,
    pub outcome: Option<MutationOutcome>,
    pub failure_code: Option<String>,
}

impl ProjectedFile {
    pub fn kind(&self) -> ChangeKind {
        self.change.kind
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectedManifest {
    pub files: Vec<ProjectedFile>,
    pub notices: Vec<NoticeEvent>,
    pub results_complete: bool,
    pub is_check: bool,
}

impl ProjectedManifest {
    pub fn creates(&self) -> u64 {
        self.files
            .iter()
            .filter(|file| file.change.kind == ChangeKind::Create)
            .count() as u64
    }

    pub fn modifies(&self) -> u64 {
        self.files
            .iter()
            .filter(|file| file.change.kind == ChangeKind::Modify)
            .count() as u64
    }

    pub fn applied(&self) -> u64 {
        self.files
            .iter()
            .filter(|file| file.outcome == Some(MutationOutcome::Applied))
            .count() as u64
    }

    pub fn not_applied(&self) -> u64 {
        self.files
            .iter()
            .filter(|file| file.outcome == Some(MutationOutcome::NotApplied))
            .count() as u64
    }

    pub fn sorted_files(&self) -> Vec<&ProjectedFile> {
        let mut ordered: Vec<&ProjectedFile> = self.files.iter().collect();
        ordered.sort_by(|a, b| a.change.path.as_bytes().cmp(b.change.path.as_bytes()));
        ordered
    }

    pub fn finished_counts(&self) -> FinishedCounts {
        FinishedCounts {
            results_complete: Some(self.results_complete),
            diagnostics: None,
            changes: Some([self.creates(), self.modifies()]),
            mutations: if self.is_check {
                None
            } else {
                Some([self.applied(), self.not_applied()])
            },
        }
    }

    pub fn exit_code(&self, bazel_code: i32) -> i32 {
        if bazel_code != 0 {
            return bazel_code;
        }
        if !self.results_complete {
            return 1;
        }
        if self.is_check {
            if self.files.is_empty() {
                0
            } else {
                1
            }
        } else if self.not_applied() > 0 {
            1
        } else {
            0
        }
    }
}

fn hex_digest(bytes: &[u8]) -> String {
    dx_digest::to_hex_bytes(bytes)
}

pub fn manifest_is_check(manifest: &GenerationManifest) -> bool {
    manifest.mode == Mode::Check as i32
}

pub fn ensure_mode(manifest: &GenerationManifest, check: bool) -> Result<(), Error> {
    let want = if check {
        Mode::Check as i32
    } else {
        Mode::Default as i32
    };
    if manifest.mode != want {
        return Err(Error::InvalidMode {
            found: manifest.mode,
        });
    }
    Ok(())
}

fn map_outcome(
    path: &str,
    outcome: i32,
    failure_code: &str,
) -> Result<(Option<MutationOutcome>, Option<String>), Error> {
    match WriteOutcome::try_from(outcome) {
        Ok(WriteOutcome::Applied) => Ok((Some(MutationOutcome::Applied), None)),
        Ok(WriteOutcome::NotApplied) => Ok((
            Some(MutationOutcome::NotApplied),
            Some(failure_code.to_owned()),
        )),
        _ => Err(Error::InvalidOutcome {
            path: path.to_owned(),
            found: outcome,
        }),
    }
}

pub fn project(manifest: &GenerationManifest) -> Result<ProjectedManifest, Error> {
    validate(manifest)?;
    let is_check = manifest_is_check(manifest);
    let results_complete = manifest
        .scopes
        .iter()
        .all(|scope| scope.results_complete == Some(true));
    let mut files = Vec::with_capacity(manifest.files.len());
    for file in &manifest.files {
        let candidate_bytes = candidate(file)?;
        // LCOV_EXCL_START - reason: defensive unreachable, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
        let change_ref = file.change.as_ref().ok_or(Error::MissingChange {
            path: file.path.clone(),
        })?;
        // LCOV_EXCL_STOP - reason: end defensive unreachable, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
        let (change, original, candidate_text) = match change_ref {
            file_result::Change::CreateContent(content) => {
                // LCOV_EXCL_START - reason: defensive unreachable, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
                let text = String::from_utf8(content.clone()).map_err(|_| Error::InvalidUtf8 {
                    at: file.path.clone(),
                })?;
                // LCOV_EXCL_STOP - reason: end defensive unreachable, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
                let change = ChangeEvent {
                    path: file.path.clone(),
                    kind: ChangeKind::Create,
                    source_digest: None,
                    edits: vec![Edit {
                        start: 0,
                        end: 0,
                        replacement: text.clone(),
                    }],
                };
                (change, String::new(), text)
            }
            file_result::Change::Modification(modification) => {
                // LCOV_EXCL_START - reason: defensive unreachable, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
                let original_text = String::from_utf8(modification.original_content.clone())
                    .map_err(|_| Error::InvalidUtf8 {
                        at: file.path.clone(),
                    })?;
                let candidate_text =
                    String::from_utf8(candidate_bytes.clone()).map_err(|_| Error::InvalidUtf8 {
                        at: file.path.clone(),
                    })?;
                // LCOV_EXCL_STOP - reason: end defensive unreachable, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
                let mut edits = Vec::with_capacity(modification.edits.len());
                for edit in &modification.edits {
                    // LCOV_EXCL_START - reason: defensive unreachable, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
                    let replacement =
                        String::from_utf8(edit.replacement.clone()).map_err(|_| {
                            Error::InvalidUtf8 {
                                at: file.path.clone(),
                            }
                        })?;
                    // LCOV_EXCL_STOP - reason: end defensive unreachable, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
                    edits.push(Edit {
                        start: edit.start_byte,
                        end: edit.end_byte,
                        replacement,
                    });
                }
                let change = ChangeEvent {
                    path: file.path.clone(),
                    kind: ChangeKind::Modify,
                    source_digest: Some(hex_digest(&modification.original_digest)),
                    edits,
                };
                (change, original_text, candidate_text)
            }
        };
        let (outcome, failure_code) = if is_check {
            (None, None)
        } else {
            map_outcome(&file.path, file.outcome, &file.failure_code)?
        };
        files.push(ProjectedFile {
            change,
            original,
            candidate: candidate_text,
            outcome,
            failure_code,
        });
    }
    let mut notices = Vec::with_capacity(manifest.ignored_imports.len());
    for ignored in &manifest.ignored_imports {
        notices.push(NoticeEvent {
            level: IGNORED_IMPORT_LEVEL.to_owned(),
            code: IGNORED_IMPORT_CODE.to_owned(),
            message: IGNORED_IMPORT_MESSAGE.to_owned(),
            related_command: None,
            scope: None,
            path: Some(ignored.path.clone()),
            language: Some(ignored.language.clone()),
            import: Some(ignored.import.clone()),
        });
    }
    Ok(ProjectedManifest {
        files,
        notices,
        results_complete,
        is_check,
    })
}

pub fn render_diff(projected: &ProjectedManifest) -> Result<String, DiffError> {
    let sorted = projected.sorted_files();
    let patches: Vec<FilePatch<'_>> = sorted
        .iter()
        .map(|file| FilePatch {
            path: file.change.path.as_str(),
            kind: match file.change.kind {
                ChangeKind::Modify => PatchKind::Modify,
                ChangeKind::Create => PatchKind::Create,
            },
            original: file.original.as_str(),
            candidate: file.candidate.as_str(),
        })
        .collect();
    render_patch(&patches)
}

pub fn text_lines(projected: &ProjectedManifest) -> Vec<String> {
    let mut lines = Vec::with_capacity(projected.files.len() + projected.notices.len());
    for file in projected.sorted_files() {
        match file.change.kind {
            ChangeKind::Create => lines.push(format!("Created {}", file.change.path)),
            ChangeKind::Modify => lines.push(format!("Modified {}", file.change.path)),
        }
    }
    for notice in &projected.notices {
        let path = notice.path.as_deref().unwrap_or("");
        let language = notice.language.as_deref().unwrap_or("");
        let import = notice.import.as_deref().unwrap_or("");
        lines.push(format!(
            "Ignored import '{import}' in {path} [{language}]: {IGNORED_IMPORT_MESSAGE}"
        ));
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use dx_digest::blake3 as digest;
    use dx_output::{change_event, mutation_event, notice_event};
    use generation_result::proto::{
        Edit as ProtoEdit, FileResult, IgnoredImport, Modification, Scope,
    };

    fn scope(value: &str, complete: bool) -> Scope {
        Scope {
            value: value.to_owned(),
            results_complete: Some(complete),
        }
    }

    fn modify_file(
        path: &str,
        original: &[u8],
        replacement: &[u8],
        outcome: WriteOutcome,
        failure_code: &str,
    ) -> FileResult {
        let end = u64::try_from(original.len()).expect("fixture fits u64");
        FileResult {
            path: path.to_owned(),
            scope_index: 0,
            change: Some(file_result::Change::Modification(Modification {
                original_digest: digest(original).to_vec(),
                original_content: original.to_vec(),
                edits: vec![ProtoEdit {
                    start_byte: 0,
                    end_byte: end,
                    replacement: replacement.to_vec(),
                }],
            })),
            outcome: outcome as i32,
            failure_code: failure_code.to_owned(),
        }
    }

    fn create_file(
        path: &str,
        content: &[u8],
        outcome: WriteOutcome,
        failure_code: &str,
    ) -> FileResult {
        FileResult {
            path: path.to_owned(),
            scope_index: 0,
            change: Some(file_result::Change::CreateContent(content.to_vec())),
            outcome: outcome as i32,
            failure_code: failure_code.to_owned(),
        }
    }

    fn ignored(path: &str, language: &str, import: &str) -> IgnoredImport {
        IgnoredImport {
            path: path.to_owned(),
            language: language.to_owned(),
            import: import.to_owned(),
            scope_index: 0,
        }
    }

    fn check_manifest() -> GenerationManifest {
        GenerationManifest {
            schema_major: generation_result::SCHEMA_MAJOR,
            schema_minor: generation_result::SCHEMA_MINOR,
            mode: Mode::Check as i32,
            scopes: vec![scope("//...", true)],
            files: vec![modify_file(
                "pkg/BUILD.bazel",
                b"a\n",
                b"b\n",
                WriteOutcome::Unspecified,
                "",
            )],
            ignored_imports: vec![ignored("pkg/a.rs", "rust", "optional")],
        }
    }

    fn default_manifest() -> GenerationManifest {
        GenerationManifest {
            schema_major: generation_result::SCHEMA_MAJOR,
            schema_minor: generation_result::SCHEMA_MINOR,
            mode: Mode::Default as i32,
            scopes: vec![scope("//...", true)],
            files: vec![
                create_file("z/BUILD.bazel", b"c\n", WriteOutcome::Applied, ""),
                modify_file(
                    "a/BUILD.bazel",
                    b"a\n",
                    b"b\n",
                    WriteOutcome::NotApplied,
                    "io_error",
                ),
            ],
            ignored_imports: vec![ignored("pkg/a.rs", "rust", "optional")],
        }
    }

    #[test]
    fn mode_helpers_match_invocation() {
        let check = check_manifest();
        let default = default_manifest();
        assert!(manifest_is_check(&check));
        assert!(!manifest_is_check(&default));
        assert!(ensure_mode(&check, true).is_ok());
        assert!(ensure_mode(&default, false).is_ok());
        assert!(matches!(
            ensure_mode(&check, false),
            Err(Error::InvalidMode { .. })
        ));
        assert!(matches!(
            ensure_mode(&default, true),
            Err(Error::InvalidMode { .. })
        ));
    }

    #[test]
    fn outcome_mapping_covers_all_shapes() {
        assert_eq!(
            map_outcome("p/BUILD.bazel", WriteOutcome::Applied as i32, "").unwrap(),
            (Some(MutationOutcome::Applied), None)
        );
        assert_eq!(
            map_outcome("p/BUILD.bazel", WriteOutcome::NotApplied as i32, "io_error").unwrap(),
            (
                Some(MutationOutcome::NotApplied),
                Some("io_error".to_owned())
            )
        );
        assert!(matches!(
            map_outcome("p/BUILD.bazel", 0, ""),
            Err(Error::InvalidOutcome { .. })
        ));
        assert!(matches!(
            map_outcome("p/BUILD.bazel", 99, ""),
            Err(Error::InvalidOutcome { .. })
        ));
    }

    #[test]
    fn check_projection_has_no_outcomes_and_valid_events() {
        let projected = project(&check_manifest()).unwrap();
        assert!(projected.is_check);
        assert!(projected.results_complete);
        assert_eq!(projected.files.len(), 1);
        assert_eq!(projected.creates(), 0);
        assert_eq!(projected.modifies(), 1);
        assert_eq!(projected.applied(), 0);
        assert_eq!(projected.not_applied(), 0);
        assert_eq!(projected.files[0].outcome, None);
        assert_eq!(projected.files[0].failure_code, None);
        assert_eq!(projected.files[0].kind(), ChangeKind::Modify);
        assert_eq!(projected.files[0].original, "a\n");
        assert_eq!(projected.files[0].candidate, "b\n");
        assert_eq!(projected.files[0].change.edits.len(), 1);
        assert_eq!(
            projected.files[0]
                .change
                .source_digest
                .as_ref()
                .unwrap()
                .len(),
            64
        );
        assert!(change_event(&projected.files[0].change).is_ok());
        assert!(notice_event(&projected.notices[0]).is_ok());
        assert_eq!(projected.notices[0].code, IGNORED_IMPORT_CODE);
        assert_eq!(projected.notices[0].level, IGNORED_IMPORT_LEVEL);
        assert_eq!(projected.notices[0].message, IGNORED_IMPORT_MESSAGE);
        assert_eq!(projected.notices[0].scope, None);
        assert_eq!(projected.notices[0].related_command, None);
        let counts = projected.finished_counts();
        assert_eq!(counts.results_complete, Some(true));
        assert_eq!(counts.diagnostics, None);
        assert_eq!(counts.changes, Some([0, 1]));
        assert_eq!(counts.mutations, None);
        assert_eq!(projected.exit_code(0), 1);
        assert_eq!(projected.exit_code(3), 3);
        assert_eq!(text_lines(&projected), vec![
            "Modified pkg/BUILD.bazel".to_owned(),
            "Ignored import 'optional' in pkg/a.rs [rust]: Static import intentionally contributes no Bazel dependency".to_owned(),
        ]);
        let diff = render_diff(&projected).unwrap();
        assert!(diff.contains("--- a/pkg/BUILD.bazel"));
        assert!(diff.contains("+++ b/pkg/BUILD.bazel"));
    }

    #[test]
    fn default_projection_keeps_attempt_order_and_sorts_presentation() {
        let projected = project(&default_manifest()).unwrap();
        assert!(!projected.is_check);
        assert!(projected.results_complete);
        assert_eq!(projected.files.len(), 2);
        assert_eq!(projected.creates(), 1);
        assert_eq!(projected.modifies(), 1);
        assert_eq!(projected.applied(), 1);
        assert_eq!(projected.not_applied(), 1);
        assert_eq!(projected.files[0].change.path, "z/BUILD.bazel");
        assert_eq!(projected.files[1].change.path, "a/BUILD.bazel");
        assert_eq!(projected.files[0].outcome, Some(MutationOutcome::Applied));
        assert_eq!(projected.files[0].failure_code, None);
        assert_eq!(
            projected.files[1].outcome,
            Some(MutationOutcome::NotApplied)
        );
        assert_eq!(projected.files[1].failure_code.as_deref(), Some("io_error"));
        assert_eq!(projected.files[0].kind(), ChangeKind::Create);
        assert_eq!(projected.files[0].original, "");
        let sorted: Vec<&str> = projected
            .sorted_files()
            .iter()
            .map(|file| file.change.path.as_str())
            .collect();
        assert_eq!(sorted, vec!["a/BUILD.bazel", "z/BUILD.bazel"]);
        for file in &projected.files {
            assert!(change_event(&file.change).is_ok());
            assert!(mutation_event(
                &file.change.path,
                file.change.kind,
                file.outcome.unwrap(),
                file.failure_code.as_deref(),
            )
            .is_ok());
        }
        let counts = projected.finished_counts();
        assert_eq!(counts.changes, Some([1, 1]));
        assert_eq!(counts.mutations, Some([1, 1]));
        assert_eq!(projected.exit_code(0), 1);
        assert_eq!(
            text_lines(&projected),
            vec![
                "Modified a/BUILD.bazel".to_owned(),
                "Created z/BUILD.bazel".to_owned(),
                "Ignored import 'optional' in pkg/a.rs [rust]: Static import intentionally contributes no Bazel dependency".to_owned(),
            ]
        );
        let diff = render_diff(&projected).unwrap();
        assert!(diff.find("a/BUILD.bazel").unwrap() < diff.find("z/BUILD.bazel").unwrap());
        assert!(diff.contains("--- /dev/null"));
    }

    #[test]
    fn empty_manifest_projects_to_zero_counts() {
        let manifest = GenerationManifest {
            schema_major: generation_result::SCHEMA_MAJOR,
            schema_minor: generation_result::SCHEMA_MINOR,
            mode: Mode::Check as i32,
            scopes: vec![scope("//...", true)],
            files: vec![],
            ignored_imports: vec![],
        };
        let projected = project(&manifest).unwrap();
        assert!(projected.files.is_empty());
        assert!(projected.notices.is_empty());
        assert_eq!(projected.creates(), 0);
        assert_eq!(projected.modifies(), 0);
        assert_eq!(projected.applied(), 0);
        assert_eq!(projected.not_applied(), 0);
        assert!(projected.sorted_files().is_empty());
        assert_eq!(text_lines(&projected), Vec::<String>::new());
        assert_eq!(render_diff(&projected).unwrap(), "");
        assert_eq!(
            projected.finished_counts(),
            FinishedCounts {
                results_complete: Some(true),
                diagnostics: None,
                changes: Some([0, 0]),
                mutations: None,
            }
        );
        assert_eq!(projected.exit_code(0), 0);
    }

    #[test]
    fn empty_create_fails_to_render_a_diff() {
        let manifest = GenerationManifest {
            schema_major: generation_result::SCHEMA_MAJOR,
            schema_minor: generation_result::SCHEMA_MINOR,
            mode: Mode::Default as i32,
            scopes: vec![scope("//...", true)],
            files: vec![create_file("e/BUILD.bazel", b"", WriteOutcome::Applied, "")],
            ignored_imports: vec![],
        };
        let projected = project(&manifest).unwrap();
        assert_eq!(projected.files[0].candidate, "");
        assert_eq!(
            render_diff(&projected).expect_err("no hunk can express an empty file"),
            DiffError::EmptyCreate {
                path: "e/BUILD.bazel".to_owned()
            }
        );
        assert_eq!(projected.exit_code(0), 0);
    }

    #[test]
    fn exit_code_covers_bazel_incomplete_and_success_shapes() {
        let mut incomplete = default_manifest();
        incomplete.scopes[0].results_complete = Some(false);
        let incomplete = project(&incomplete).unwrap();
        assert!(!incomplete.results_complete);
        assert_eq!(incomplete.exit_code(0), 1);
        assert_eq!(incomplete.exit_code(2), 2);

        let success_manifest = GenerationManifest {
            schema_major: generation_result::SCHEMA_MAJOR,
            schema_minor: generation_result::SCHEMA_MINOR,
            mode: Mode::Default as i32,
            scopes: vec![scope("//...", true)],
            files: vec![create_file(
                "s/BUILD.bazel",
                b"x\n",
                WriteOutcome::Applied,
                "",
            )],
            ignored_imports: vec![],
        };
        let success = project(&success_manifest).unwrap();
        assert_eq!(success.exit_code(0), 0);
        assert_eq!(success.finished_counts().mutations, Some([1, 0]));

        let check_empty_manifest = GenerationManifest {
            schema_major: generation_result::SCHEMA_MAJOR,
            schema_minor: generation_result::SCHEMA_MINOR,
            mode: Mode::Check as i32,
            scopes: vec![scope("//...", true)],
            files: vec![],
            ignored_imports: vec![],
        };
        let check_empty = project(&check_empty_manifest).unwrap();
        assert_eq!(check_empty.exit_code(0), 0);
        assert_eq!(check_empty.finished_counts().mutations, None);
    }

    #[test]
    fn invalid_manifests_fail_closed_without_partial_output() {
        let mut bad_major = check_manifest();
        bad_major.schema_major = 99;
        assert!(project(&bad_major).is_err());

        let mut empty_scopes = check_manifest();
        empty_scopes.scopes.clear();
        assert!(project(&empty_scopes).is_err());

        let mut duplicate = default_manifest();
        duplicate.files.push(duplicate.files[0].clone());
        assert!(project(&duplicate).is_err());

        let mut bad_digest = check_manifest();
        if let Some(file_result::Change::Modification(modification)) =
            bad_digest.files[0].change.as_mut()
        {
            modification.original_digest = vec![0u8; 3];
        }
        assert!(project(&bad_digest).is_err());

        let mut check_outcome = check_manifest();
        check_outcome.files[0].outcome = WriteOutcome::Applied as i32;
        assert!(project(&check_outcome).is_err());
    }

    #[test]
    fn diff_errors_surface_unrepresentable_paths() {
        let projected = ProjectedManifest {
            files: vec![
                ProjectedFile {
                    change: ChangeEvent {
                        path: "bad\tpath/BUILD.bazel".to_owned(),
                        kind: ChangeKind::Modify,
                        source_digest: None,
                        edits: vec![],
                    },
                    original: "a\n".to_owned(),
                    candidate: "b\n".to_owned(),
                    outcome: None,
                    failure_code: None,
                },
                ProjectedFile {
                    change: ChangeEvent {
                        path: "bad\tpath/BUILD.bazel".to_owned(),
                        kind: ChangeKind::Modify,
                        source_digest: None,
                        edits: vec![],
                    },
                    original: "a\n".to_owned(),
                    candidate: "b\n".to_owned(),
                    outcome: None,
                    failure_code: None,
                },
            ],
            notices: vec![],
            results_complete: true,
            is_check: true,
        };
        assert!(render_diff(&projected).is_err());
    }
}
