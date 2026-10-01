use generation_result::proto::{
    file_result, Edit, FileResult, GenerationManifest, IgnoredImport, Mode, Modification, Scope,
    WriteOutcome,
};
use serde::Deserialize;
use std::path::Path;

pub const FAILURE_WRITE_MISMATCH: &str = "write_mismatch";
pub const FAILURE_MISSING_FILE: &str = "missing_file";
pub const FAILURE_UNREADABLE_FILE: &str = "unreadable_file";

pub struct FinalizeInput<'a> {
    pub intended_json: &'a [u8],
    pub workspace: &'a Path,
    pub check: bool,
    pub gazelle_ok: bool,
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum FinalizeError {
    #[error("malformed intended manifest: {0}")]
    Malformed(String),
    #[error("check run cannot be finalized: gazelle did not complete successfully")]
    IncompleteCheck,
    #[error("invalid generation manifest: {0}")]
    Invalid(#[from] generation_result::Error),
}

fn decode_b64(value: &str) -> Result<Vec<u8>, ()> {
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    if value.is_empty() {
        return Ok(Vec::new());
    }
    STANDARD.decode(value).map_err(|_| ())
}

fn de_b64<'de, D>(d: D) -> Result<Vec<u8>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s = String::deserialize(d)?;
    decode_b64(&s).map_err(|()| serde::de::Error::custom("invalid base64"))
}

fn de_opt_b64<'de, D>(d: D) -> Result<Option<Vec<u8>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let opt = Option::<String>::deserialize(d)?;
    opt.map(|s| decode_b64(&s).map_err(|()| serde::de::Error::custom("invalid base64")))
        .transpose()
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct IntendedPayload {
    schema_major: u32,
    schema_minor: u32,
    mode: String,
    #[serde(default)]
    scopes: Vec<IntendedScope>,
    #[serde(default)]
    files: Vec<IntendedFile>,
    #[serde(default)]
    ignored_imports: Vec<IntendedIgnored>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct IntendedScope {
    value: String,
    #[serde(default)]
    results_complete: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct IntendedFile {
    path: String,
    #[serde(default)]
    scope_index: u32,
    #[serde(default, deserialize_with = "de_opt_b64")]
    original_content: Option<Vec<u8>>,
    #[serde(default, deserialize_with = "de_opt_b64")]
    create_content: Option<Vec<u8>>,
    #[serde(default)]
    edits: Vec<IntendedEdit>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct IntendedEdit {
    start_byte: u64,
    end_byte: u64,
    #[serde(deserialize_with = "de_b64")]
    replacement: Vec<u8>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct IntendedIgnored {
    path: String,
    language: String,
    #[serde(rename = "import")]
    import: String,
    #[serde(default)]
    scope_index: u32,
}

fn check_joinable(path: &str) -> Result<(), FinalizeError> {
    match dx_path::reject_reason(path) {
        Some(reason) => Err(FinalizeError::Malformed(format!(
            "unsafe path {path:?}: {reason}"
        ))),
        None => Ok(()),
    }
}

fn default_outcome(workspace: &Path, path: &str, intended: &[u8]) -> (i32, String) {
    match std::fs::read(workspace.join(path)) {
        Ok(actual) if actual == intended => (WriteOutcome::Applied as i32, String::new()),
        Ok(_) => (
            WriteOutcome::NotApplied as i32,
            FAILURE_WRITE_MISMATCH.to_owned(),
        ),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => (
            WriteOutcome::NotApplied as i32,
            FAILURE_MISSING_FILE.to_owned(),
        ),
        Err(_) => (
            WriteOutcome::NotApplied as i32,
            FAILURE_UNREADABLE_FILE.to_owned(),
        ),
    }
}

pub fn finalize(input: &FinalizeInput<'_>) -> Result<GenerationManifest, FinalizeError> {
    let payload: IntendedPayload = serde_json::from_slice(input.intended_json)
        .map_err(|err| FinalizeError::Malformed(format!("invalid intended JSON: {err}")))?;
    if payload.schema_major != generation_result::SCHEMA_MAJOR {
        return Err(FinalizeError::Malformed(format!(
            "unsupported schema {}.{}; want major {}",
            payload.schema_major,
            payload.schema_minor,
            generation_result::SCHEMA_MAJOR,
        )));
    }
    let want_mode = if input.check { "check" } else { "default" };
    if payload.mode != want_mode {
        return Err(FinalizeError::Malformed(format!(
            "mode mismatch: payload says {:?}, wrapper ran {want_mode:?}",
            payload.mode,
        )));
    }
    let witnessed_complete = payload.scopes.iter().all(|s| s.results_complete);
    if input.check && !witnessed_complete && !input.gazelle_ok {
        return Err(FinalizeError::IncompleteCheck);
    }
    for file in &payload.files {
        check_joinable(&file.path)?;
    }
    for ignored in &payload.ignored_imports {
        check_joinable(&ignored.path)?;
    }

    let scopes = payload
        .scopes
        .into_iter()
        .map(|scope| Scope {
            value: scope.value,
            results_complete: Some(if input.check {
                scope.results_complete
            } else {
                input.gazelle_ok && scope.results_complete
            }),
        })
        .collect();

    let mut files = Vec::with_capacity(payload.files.len());
    for file in &payload.files {
        let change = match (&file.create_content, &file.original_content) {
            (Some(_), Some(_)) => {
                return Err(FinalizeError::Malformed(format!(
                    "contradictory file {:?}: both create and modify content",
                    file.path,
                )));
            }
            (Some(content), None) => file_result::Change::CreateContent(content.clone()),
            (None, original) => {
                let original = original.clone().unwrap_or_default();
                let edits = file
                    .edits
                    .iter()
                    .map(|edit| Edit {
                        start_byte: edit.start_byte,
                        end_byte: edit.end_byte,
                        replacement: edit.replacement.clone(),
                    })
                    .collect();
                file_result::Change::Modification(Modification {
                    original_digest: dx_digest::blake3(&original).to_vec(),
                    original_content: original,
                    edits,
                })
            }
        };
        let probe = FileResult {
            path: file.path.clone(),
            scope_index: file.scope_index,
            change: Some(change),
            outcome: WriteOutcome::Unspecified as i32,
            failure_code: String::new(),
        };
        let intended = generation_result::candidate(&probe).map_err(|err| {
            FinalizeError::Malformed(format!("file {:?} change does not apply: {err}", file.path))
        })?;
        let (outcome, failure_code) = if input.check {
            (WriteOutcome::Unspecified as i32, String::new())
        } else {
            default_outcome(input.workspace, &file.path, &intended)
        };
        files.push(FileResult {
            outcome,
            failure_code,
            ..probe
        });
    }

    let ignored_imports = payload
        .ignored_imports
        .into_iter()
        .map(|ignored| IgnoredImport {
            path: ignored.path,
            language: ignored.language,
            import: ignored.import,
            scope_index: ignored.scope_index,
        })
        .collect();

    let manifest = GenerationManifest {
        schema_major: generation_result::SCHEMA_MAJOR,
        schema_minor: generation_result::SCHEMA_MINOR,
        mode: if input.check {
            Mode::Check as i32
        } else {
            Mode::Default as i32
        },
        scopes,
        files,
        ignored_imports,
    };
    generation_result::validate(&manifest).map_err(FinalizeError::Invalid)?;
    Ok(manifest)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fmt::Write as _;

    fn payload(mode: &str, results_complete: bool) -> Vec<u8> {
        format!(
            concat!(
                r#"{{"schema_major":1,"schema_minor":0,"mode":"{mode}","#,
                r#""scopes":[{{"value":"//...","results_complete":{complete}}}],"#,
                r#""files":[{{"path":"rust/tests/fixtures/hello/BUILD.bazel","scope_index":0,"#,
                r#""original_content":"YWJjCg==","#,
                r#""edits":[{{"start_byte":0,"end_byte":4,"replacement":"eHl6Cg=="}}]}}],"#,
                r#""ignored_imports":[{{"path":"rust/tests/fixtures/hello/BUILD.bazel","language":"rust","#,
                r#""import":"serde","scope_index":0}}]}}"#
            ),
            mode = mode,
            complete = results_complete,
        )
        .into_bytes()
    }

    fn workspace_with(contents: &[u8]) -> dx_test_scratch::TempDir {
        let dir = dx_test_scratch::scratch("dx-finalize-test-");
        let target = dir.path().join("rust/tests/fixtures/hello/BUILD.bazel");
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::fs::write(&target, contents).unwrap();
        dir
    }

    #[test]
    fn default_mode_applied_when_workspace_matches() {
        let dir = workspace_with(b"xyz\n");
        let manifest = finalize(&FinalizeInput {
            intended_json: &payload("default", true),
            workspace: dir.path(),
            check: false,
            gazelle_ok: true,
        })
        .unwrap();
        assert_eq!(manifest.mode, Mode::Default as i32);
        let file = manifest.files.first().unwrap();
        assert_eq!(file.outcome, WriteOutcome::Applied as i32);
        assert!(file.failure_code.is_empty());
        assert_eq!(generation_result::candidate(file).unwrap(), b"xyz\n");
        assert!(matches!(
            file.change,
            Some(file_result::Change::Modification(_))
        ));
        let Some(file_result::Change::Modification(modification)) = &file.change else {
            unreachable!("expected modification change"); // LCOV_EXCL_LINE - reason: unreachable arm, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
        };
        assert_eq!(modification.original_content, b"abc\n");
        assert_eq!(
            modification.original_digest,
            dx_digest::blake3(b"abc\n").to_vec()
        );
        assert_eq!(manifest.scopes[0].results_complete, Some(true));
        assert_eq!(manifest.ignored_imports.len(), 1);
    }

    #[test]
    fn default_mode_mismatch_missing_and_unreadable_degrade() {
        let dir = workspace_with(b"stale\n");
        let run = |workspace: &Path| {
            finalize(&FinalizeInput {
                intended_json: &payload("default", true),
                workspace,
                check: false,
                gazelle_ok: true,
            })
            .unwrap()
            .files
            .pop()
            .unwrap()
        };
        let mismatched = run(dir.path());
        assert_eq!(mismatched.outcome, WriteOutcome::NotApplied as i32);
        assert_eq!(mismatched.failure_code, FAILURE_WRITE_MISMATCH);

        let missing_dir = dx_test_scratch::scratch("dx-finalize-test-");
        let missing = run(missing_dir.path());
        assert_eq!(missing.outcome, WriteOutcome::NotApplied as i32);
        assert_eq!(missing.failure_code, FAILURE_MISSING_FILE);

        std::fs::create_dir_all(dir.path().join("rust/tests/fixtures/hello/BUILD.bazel.dir"))
            .unwrap();
        std::fs::rename(
            dir.path().join("rust/tests/fixtures/hello/BUILD.bazel"),
            dir.path()
                .join("rust/tests/fixtures/hello/BUILD.bazel.dir/BUILD.bazel"),
        )
        .unwrap();
        std::fs::create_dir_all(dir.path().join("rust/tests/fixtures/hello/BUILD.bazel")).unwrap();
        let unreadable = run(dir.path());
        assert_eq!(unreadable.outcome, WriteOutcome::NotApplied as i32);
        assert_eq!(unreadable.failure_code, FAILURE_UNREADABLE_FILE);
    }

    #[test]
    fn check_mode_reads_no_files_and_marks_unspecified() {
        let dir = dx_test_scratch::scratch("dx-finalize-test-");
        let manifest = finalize(&FinalizeInput {
            intended_json: &payload("check", true),
            workspace: dir.path(),
            check: true,
            gazelle_ok: true,
        })
        .unwrap();
        assert_eq!(manifest.mode, Mode::Check as i32);
        let file = manifest.files.first().unwrap();
        assert_eq!(file.outcome, WriteOutcome::Unspecified as i32);
        assert!(file.failure_code.is_empty());
        assert_eq!(manifest.scopes[0].results_complete, Some(true));
    }

    #[test]
    fn check_mode_complete_witness_finalizes_after_failed_run() {
        let dir = dx_test_scratch::scratch("dx-finalize-test-");
        let manifest = finalize(&FinalizeInput {
            intended_json: &payload("check", true),
            workspace: dir.path(),
            check: true,
            gazelle_ok: false,
        })
        .expect("complete check witness finalizes");
        assert_eq!(manifest.mode, Mode::Check as i32);
        assert_eq!(manifest.scopes[0].results_complete, Some(true));
        let file = manifest.files.first().expect("witness file");
        assert_eq!(file.outcome, WriteOutcome::Unspecified as i32);
        assert!(file.failure_code.is_empty());
        assert_eq!(manifest.ignored_imports.len(), 1);
    }

    #[test]
    fn check_mode_rejects_incomplete_scope() {
        let dir = dx_test_scratch::scratch("dx-finalize-test-");
        let err = finalize(&FinalizeInput {
            intended_json: &payload("check", false),
            workspace: dir.path(),
            check: true,
            gazelle_ok: true,
        })
        .unwrap_err();
        assert!(matches!(err, FinalizeError::Invalid(_)), "{err}");
    }

    #[test]
    fn create_file_finalizes() {
        let json = concat!(
            r#"{"schema_major":1,"schema_minor":0,"mode":"default","#,
            r#""scopes":[{"value":"//...","results_complete":true}],"#,
            r#""files":[{"path":"new/BUILD.bazel","scope_index":0,"#,
            r#""create_content":"eAo="}],"ignored_imports":[]}"#,
        );
        let dir = workspace_with(b"");
        std::fs::create_dir_all(dir.path().join("new")).unwrap();
        std::fs::write(dir.path().join("new/BUILD.bazel"), b"x\n").unwrap();
        let manifest = finalize(&FinalizeInput {
            intended_json: json.as_bytes(),
            workspace: dir.path(),
            check: false,
            gazelle_ok: true,
        })
        .unwrap();
        let file = manifest.files.first().unwrap();
        assert!(matches!(
            file.change,
            Some(file_result::Change::CreateContent(_))
        ));
        assert_eq!(file.outcome, WriteOutcome::Applied as i32);
    }

    #[test]
    fn rejects_garbage_schema_mode_and_shape() {
        let dir = dx_test_scratch::scratch("dx-finalize-test-");
        let run = |json: &[u8], check: bool, gazelle_ok: bool| {
            finalize(&FinalizeInput {
                intended_json: json,
                workspace: dir.path(),
                check,
                gazelle_ok,
            })
            .unwrap_err()
        };
        assert!(matches!(
            run(b"not json", false, true),
            FinalizeError::Malformed(_)
        ));
        assert!(matches!(
            run(br#"{"schema_major":1}"#, false, true),
            FinalizeError::Malformed(_)
        ));
        let bad_schema = br#"{"schema_major":2,"schema_minor":0,"mode":"default"}"#;
        assert!(matches!(
            run(bad_schema, false, true),
            FinalizeError::Malformed(_)
        ));
        assert!(matches!(
            run(&payload("check", true), false, true),
            FinalizeError::Malformed(_)
        ));
        assert!(matches!(
            run(&payload("default", true), true, true),
            FinalizeError::Malformed(_)
        ));
        assert!(matches!(
            run(b"not json", true, false),
            FinalizeError::Malformed(_)
        ));
        assert!(matches!(
            run(&payload("default", true), true, false),
            FinalizeError::Malformed(_)
        ));
        assert!(matches!(
            run(&payload("check", false), true, false),
            FinalizeError::IncompleteCheck
        ));
        assert!(matches!(
            run(
                br#"{"schema_major":1,"schema_minor":0,"mode":"default","bogus":[]}"#,
                false,
                true
            ),
            FinalizeError::Malformed(_)
        ));
        let both = concat!(
            r#"{"schema_major":1,"schema_minor":0,"mode":"default","scopes":[],"#,
            r#""files":[{"path":"a","create_content":"eA==","original_content":"eQ=="}],"#,
            r#""ignored_imports":[]}"#,
        );
        let both_err = run(both.as_bytes(), false, true);
        assert!(matches!(both_err, FinalizeError::Malformed(_)));
        assert!(
            format!("{both_err}").contains("contradictory file"),
            "unexpected message: {both_err}"
        );
        let bad_b64 = concat!(
            r#"{"schema_major":1,"schema_minor":0,"mode":"default","scopes":[],"#,
            r#""files":[{"path":"a","create_content":"!!!"}],"ignored_imports":[]}"#,
        );
        assert!(matches!(
            run(bad_b64.as_bytes(), false, true),
            FinalizeError::Malformed(_)
        ));
        let bad_edits = concat!(
            r#"{"schema_major":1,"schema_minor":0,"mode":"default","scopes":[],"#,
            r#""files":[{"path":"a","original_content":"YWJjCg==","#,
            r#""edits":[{"start_byte":9,"end_byte":2,"replacement":"eA=="}]}],"#,
            r#""ignored_imports":[]}"#,
        );
        let edits_err = run(bad_edits.as_bytes(), false, true);
        assert!(matches!(edits_err, FinalizeError::Malformed(_)));
        assert!(
            format!("{edits_err}").contains("does not apply"),
            "unexpected message: {edits_err}"
        );
    }

    #[test]
    fn rejects_unsafe_paths_before_filesystem_access() {
        let dir = dx_test_scratch::scratch("dx-finalize-test-");
        std::fs::create_dir_all(dir.path().join("etc")).unwrap();
        std::fs::write(dir.path().join("etc/passwd"), b"xyz\n").unwrap();
        for path in [
            "../etc/passwd",
            "/etc/passwd",
            "",
            "a//b",
            "a/./b",
            "..\\etc\\passwd",
            "C:\\Windows\\passwd",
            "C:/Windows/passwd",
            "C:passwd",
            "c:/windows/passwd",
            "\\Windows\\passwd",
            "\\\\server\\share\\passwd",
            "back\\slash",
        ] {
            let json = format!(
                concat!(
                    r#"{{"schema_major":1,"schema_minor":0,"mode":"default","#,
                    r#""scopes":[{{"value":"//...","results_complete":true}}],"#,
                    r#""files":[{{"path":{path:?},"scope_index":0,"#,
                    r#""create_content":"eHl6Cg=="}}],"#,
                    r#""ignored_imports":[]}}"#
                ),
                path = path,
            );
            let err = finalize(&FinalizeInput {
                intended_json: json.as_bytes(),
                workspace: dir.path(),
                check: false,
                gazelle_ok: true,
            })
            .unwrap_err();
            assert!(matches!(err, FinalizeError::Malformed(_)), "{path}: {err}");
            assert_eq!(
                format!("{err}"),
                format!(
                    "malformed intended manifest: unsafe path {path:?}: {}",
                    dx_path::reject_reason(path).expect("rung")
                ),
                "reason does not come from the dx_path ladder: {path}"
            );
        }
    }

    #[test]
    fn crate_validation_still_guards_structural_rules() {
        let json = concat!(
            r#"{"schema_major":1,"schema_minor":0,"mode":"default","#,
            r#""scopes":[{"value":"//...","results_complete":true}],"#,
            r#""files":[{"path":"a","scope_index":7,"create_content":"eA=="}],"#,
            r#""ignored_imports":[]}"#,
        );
        let dir = dx_test_scratch::scratch("dx-finalize-test-");
        let err = finalize(&FinalizeInput {
            intended_json: json.as_bytes(),
            workspace: dir.path(),
            check: false,
            gazelle_ok: true,
        })
        .unwrap_err();
        assert!(matches!(err, FinalizeError::Invalid(_)), "{err}");
        assert!(format!("{err}").contains("invalid generation manifest"));
        assert!(std::error::Error::source(&err).is_some());
    }

    #[test]
    fn error_display_and_incomplete_results_complete() {
        let dir = dx_test_scratch::scratch("dx-finalize-test-");
        let err = finalize(&FinalizeInput {
            intended_json: &payload("check", false),
            workspace: dir.path(),
            check: true,
            gazelle_ok: false,
        })
        .unwrap_err();
        assert_eq!(
            format!("{err}"),
            "check run cannot be finalized: gazelle did not complete successfully"
        );
        assert!(std::error::Error::source(&err).is_none());

        let dir = workspace_with(b"xyz\n");
        let manifest = finalize(&FinalizeInput {
            intended_json: &payload("default", false),
            workspace: dir.path(),
            check: false,
            gazelle_ok: true,
        })
        .unwrap();
        assert_eq!(manifest.scopes[0].results_complete, Some(false));
    }

    #[test]
    fn base64_vectors() {
        assert_eq!(decode_b64("").unwrap(), b"");
        assert_eq!(decode_b64("Zg==").unwrap(), b"f");
        assert_eq!(decode_b64("Zm8=").unwrap(), b"fo");
        assert_eq!(decode_b64("Zm9v").unwrap(), b"foo");
        assert_eq!(decode_b64("Zm9vYmFy").unwrap(), b"foobar");
        assert_eq!(decode_b64("+/8=").unwrap(), vec![0xFB, 0xFF]);
        assert_eq!(decode_b64("YWJjCg==").unwrap(), b"abc\n");
        for bad in [
            "Zg=", "Z", "Zm9!", "Zm9v\n", " Zm9v", "====", "Zm9v====", "Zg==Ym8=", "Zm==Ym8=",
            "Zm=8", "=m9v", "Z=9v",
        ] {
            assert!(decode_b64(bad).is_err(), "{bad:?} must be rejected");
        }
    }

    #[test]
    fn scope_shape_round_trip() {
        let json = concat!(
            r#"{"schema_major":1,"schema_minor":0,"mode":"default","#,
            r#""scopes":[{"value":"//rust/...","results_complete":true}],"#,
            r#""files":[],"ignored_imports":[]}"#,
        );
        let dir = dx_test_scratch::scratch("dx-finalize-test-");
        let manifest = finalize(&FinalizeInput {
            intended_json: json.as_bytes(),
            workspace: dir.path(),
            check: false,
            gazelle_ok: true,
        })
        .unwrap();
        assert_eq!(
            manifest.scopes,
            vec![Scope {
                value: "//rust/...".to_owned(),
                results_complete: Some(true),
            }]
        );
    }

    #[test]
    fn display_and_debug_formatting() {
        let err = FinalizeError::Malformed("x".to_owned());
        assert!(format!("{err:?}").contains("Malformed"));
        let mut buf = String::new();
        write!(buf, "{err}").unwrap();
        assert!(buf.contains("malformed intended manifest"));
    }
}
