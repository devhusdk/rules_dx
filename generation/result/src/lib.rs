#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

use proto::{
    file_result, FileResult, GenerationManifest, IgnoredImport, Mode, Modification, Scope,
};
pub use result_proto::dx::generation::v1 as proto;

pub use dx_digest::DIGEST_LEN;
pub use dx_schema::SCHEMA_MAJOR;
pub use dx_schema::SCHEMA_MINOR;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error("cannot decode generation result: {0}")]
    Decode(String),
    #[error("unsupported schema major {found}: want {major}", major = SCHEMA_MAJOR)]
    UnsupportedMajor { found: u32 },
    #[error("invalid mode {found}: want default or check")]
    InvalidMode { found: i32 },
    #[error("empty scopes: want at least one scope")]
    EmptyScopes,
    #[error("empty scope at scope[{index}]: want a non-empty scope")]
    EmptyScope { index: usize },
    #[error("duplicate scope {value:?}")]
    DuplicateScope { value: String },
    #[error("incomplete scope at scope[{index}]: check runs require completed scopes")]
    ScopeCompletion { index: usize },
    #[error("invalid path at {at} {path:?}: {reason}")]
    BadPath {
        at: String,
        path: String,
        reason: &'static str,
    },
    #[error("invalid build basename for {path:?}: want a BUILD.bazel path")]
    BadBuildBasename { path: String },
    #[error("invalid scope index at {at}: found {found}")]
    ScopeIndex { at: String, found: u32 },
    #[error("duplicate file {path:?}")]
    DuplicateFile { path: String },
    #[error("missing change for {path:?}")]
    MissingChange { path: String },
    #[error("invalid UTF-8 at {at}")]
    InvalidUtf8 { at: String },
    #[error("invalid digest length for {path:?}: found {found} bytes, want {len}", len = DIGEST_LEN)]
    BadDigestLen { path: String, found: usize },
    #[error("digest mismatch for {path:?}: original digest does not match content")]
    DigestMismatch { path: String },
    #[error("empty edits for {path:?}: want at least one edit")]
    EmptyEdits { path: String },
    #[error("inverted edit at {path:?}[{index}]: start must not exceed end")]
    InvertedEdit { path: String, index: usize },
    #[error("edit out of bounds at {path:?}[{index}]")]
    EditOutOfBounds { path: String, index: usize },
    #[error("edit not on UTF-8 boundary at {path:?}[{index}]")]
    EditNotUtf8Boundary { path: String, index: usize },
    #[error("invalid UTF-8 replacement at {path:?}[{index}]")]
    InvalidUtf8Replacement { path: String, index: usize },
    #[error("no-op edit at {path:?}[{index}]: replacement is identical")]
    NoopEdit { path: String, index: usize },
    #[error("unordered edit at {path:?}[{index}]: edits must be ordered and non-overlapping")]
    EditOrder { path: String, index: usize },
    #[error("unchanged candidate for {path:?}: candidate is identical to original")]
    UnchangedCandidate { path: String },
    #[error("unexpected check outcome for {path:?}: check runs carry no write outcome")]
    CheckOutcome { path: String },
    #[error("missing write outcome for {path:?}")]
    MissingOutcome { path: String },
    #[error("invalid write outcome {found} for {path:?}")]
    InvalidOutcome { path: String, found: i32 },
    #[error("unexpected failure code for applied {path:?}: applied files carry no failure code")]
    AppliedFailureCode { path: String },
    #[error("missing failure code for not-applied {path:?}")]
    MissingFailureCode { path: String },
    #[error("empty language at ignored_import[{index}]: want a non-empty language")]
    EmptyIgnoredLanguage { index: usize },
    #[error("empty import at ignored_import[{index}]: want a non-empty import")]
    EmptyIgnoredImport { index: usize },
    #[error("unordered ignored import at index {index}")]
    IgnoredOrder { index: usize },
    #[error("duplicate ignored import at index {index}")]
    DuplicateIgnoredImport { index: usize },
}

pub fn digest(bytes: &[u8]) -> [u8; DIGEST_LEN] {
    dx_digest::blake3(bytes)
}

fn check_path(at: &str, path: &str) -> Result<(), Error> {
    match dx_path::reject_reason(path) {
        Some(reason) => Err(Error::BadPath {
            at: at.to_owned(),
            path: path.to_owned(),
            reason,
        }),
        None => Ok(()),
    }
}

fn check_scope_index(at: &str, scope_index: u32, count: usize) -> Result<(), Error> {
    if usize::try_from(scope_index).map_or(true, |index| index >= count) {
        return Err(Error::ScopeIndex {
            at: at.to_owned(),
            found: scope_index,
        });
    }
    Ok(())
}

fn validate_scopes(scopes: &[Scope]) -> Result<(), Error> {
    if scopes.is_empty() {
        return Err(Error::EmptyScopes);
    }
    let mut seen = std::collections::BTreeSet::new();
    for (index, scope) in scopes.iter().enumerate() {
        if scope.value.is_empty() {
            return Err(Error::EmptyScope { index });
        }
        dx_proto_validate::check_unique_insert(&mut seen, &scope.value, |existing| {
            Error::DuplicateScope {
                value: (*existing).clone(),
            }
        })?;
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Replacement<'a> {
    pub start_byte: u64,
    pub end_byte: u64,
    pub text: &'a str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModificationText<'a> {
    pub original: &'a str,
    pub original_digest: &'a [u8],
    pub edits: Vec<Replacement<'a>>,
    pub candidate: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileText<'a> {
    Created { content: &'a str },
    Modified(ModificationText<'a>),
}

fn modification_text<'a>(
    path: &str,
    modification: &'a Modification,
) -> Result<ModificationText<'a>, Error> {
    let original =
        str::from_utf8(&modification.original_content).map_err(|_| Error::InvalidUtf8 {
            at: path.to_owned(),
        })?;
    if modification.original_digest.len() != DIGEST_LEN {
        return Err(Error::BadDigestLen {
            path: path.to_owned(),
            found: modification.original_digest.len(),
        });
    }
    if modification.original_digest.as_slice() != digest(original.as_bytes()) {
        return Err(Error::DigestMismatch {
            path: path.to_owned(),
        });
    }
    if modification.edits.is_empty() {
        return Err(Error::EmptyEdits {
            path: path.to_owned(),
        });
    }

    let mut previous_end = 0_u64;
    let mut previous_start = None;
    let mut edits = Vec::with_capacity(modification.edits.len());
    for (index, edit) in modification.edits.iter().enumerate() {
        if edit.start_byte > edit.end_byte {
            return Err(Error::InvertedEdit {
                path: path.to_owned(),
                index,
            });
        }
        // LCOV_EXCL_START - reason: defensive unreachable, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
        let Ok(start) = usize::try_from(edit.start_byte) else {
            return Err(Error::EditOutOfBounds {
                path: path.to_owned(),
                index,
            });
        };
        let Ok(end) = usize::try_from(edit.end_byte) else {
            return Err(Error::EditOutOfBounds {
                path: path.to_owned(),
                index,
            });
        };
        // LCOV_EXCL_STOP - reason: end defensive unreachable, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
        if end > original.len() {
            return Err(Error::EditOutOfBounds {
                path: path.to_owned(),
                index,
            });
        }
        if !original.is_char_boundary(start) || !original.is_char_boundary(end) {
            return Err(Error::EditNotUtf8Boundary {
                path: path.to_owned(),
                index,
            });
        }
        let text =
            str::from_utf8(&edit.replacement).map_err(|_| Error::InvalidUtf8Replacement {
                path: path.to_owned(),
                index,
            })?;
        if &original.as_bytes()[start..end] == edit.replacement.as_slice() {
            return Err(Error::NoopEdit {
                path: path.to_owned(),
                index,
            });
        }
        if previous_start.is_some_and(|start| start >= edit.start_byte)
            || (index > 0 && previous_end > edit.start_byte)
        {
            return Err(Error::EditOrder {
                path: path.to_owned(),
                index,
            });
        }
        previous_start = Some(edit.start_byte);
        previous_end = edit.end_byte;
        edits.push(Replacement {
            start_byte: edit.start_byte,
            end_byte: edit.end_byte,
            text,
        });
    }

    let capacity = original.len()
        + modification
            .edits
            .iter()
            .map(|edit| edit.replacement.len())
            .sum::<usize>();
    let mut candidate = String::with_capacity(capacity);
    let mut cursor = 0;
    for (edit, replacement) in modification.edits.iter().zip(&edits) {
        candidate.push_str(&original[cursor..edit.start_byte as usize]);
        candidate.push_str(replacement.text);
        cursor = edit.end_byte as usize;
    }
    candidate.push_str(&original[cursor..]);
    if candidate == original {
        return Err(Error::UnchangedCandidate {
            path: path.to_owned(),
        });
    }
    Ok(ModificationText {
        original,
        original_digest: &modification.original_digest,
        edits,
        candidate,
    })
}

pub fn file_text(file: &FileResult) -> Result<FileText<'_>, Error> {
    let change = file.change.as_ref().ok_or_else(|| Error::MissingChange {
        path: file.path.clone(),
    })?;
    match change {
        file_result::Change::CreateContent(content) => Ok(FileText::Created {
            content: str::from_utf8(content).map_err(|_| Error::InvalidUtf8 {
                at: file.path.clone(),
            })?,
        }),
        file_result::Change::Modification(modification) => Ok(FileText::Modified(
            modification_text(&file.path, modification)?,
        )),
    }
}

pub fn candidate(file: &FileResult) -> Result<Vec<u8>, Error> {
    Ok(match file_text(file)? {
        FileText::Created { content } => content.as_bytes().to_vec(),
        FileText::Modified(text) => text.candidate.into_bytes(),
    })
}

fn validate_file(file: &FileResult, mode: Mode, scope_count: usize) -> Result<(), Error> {
    check_path("file", &file.path)?;
    if !matches!(file.path.rsplit('/').next(), Some("BUILD" | "BUILD.bazel")) {
        return Err(Error::BadBuildBasename {
            path: file.path.clone(),
        });
    }
    check_scope_index("file", file.scope_index, scope_count)?;
    candidate(file)?;

    if mode == Mode::Check {
        if file.outcome != 0 || !file.failure_code.is_empty() {
            return Err(Error::CheckOutcome {
                path: file.path.clone(),
            });
        }
        return Ok(());
    }
    match proto::WriteOutcome::try_from(file.outcome) {
        Ok(proto::WriteOutcome::Applied) if file.failure_code.is_empty() => Ok(()),
        Ok(proto::WriteOutcome::Applied) => Err(Error::AppliedFailureCode {
            path: file.path.clone(),
        }),
        Ok(proto::WriteOutcome::NotApplied) if !file.failure_code.is_empty() => Ok(()),
        Ok(proto::WriteOutcome::NotApplied) => Err(Error::MissingFailureCode {
            path: file.path.clone(),
        }),
        Ok(proto::WriteOutcome::Unspecified) => Err(Error::MissingOutcome {
            path: file.path.clone(),
        }),
        Err(_) => Err(Error::InvalidOutcome {
            path: file.path.clone(),
            found: file.outcome,
        }),
    }
}

fn validate_ignored(ignored: &[IgnoredImport], scope_count: usize) -> Result<(), Error> {
    let mut previous: Option<(&str, &str, &str)> = None;
    for (index, item) in ignored.iter().enumerate() {
        check_path(&format!("ignored_imports[{index}]"), &item.path)?;
        check_scope_index("ignored_import", item.scope_index, scope_count)?;
        if item.language.is_empty() {
            return Err(Error::EmptyIgnoredLanguage { index });
        }
        if item.import.is_empty() {
            return Err(Error::EmptyIgnoredImport { index });
        }
        let key = (
            item.path.as_str(),
            item.language.as_str(),
            item.import.as_str(),
        );
        match dx_proto_validate::check_sorted_next(previous.as_ref(), &key) {
            Ok(()) => {}
            Err(dx_proto_validate::OrderViolation::Duplicate) => {
                return Err(Error::DuplicateIgnoredImport { index });
            }
            Err(dx_proto_validate::OrderViolation::Unsorted) => {
                return Err(Error::IgnoredOrder { index });
            }
        }
        previous = Some(key);
    }
    Ok(())
}

pub fn validate(manifest: &GenerationManifest) -> Result<(), Error> {
    dx_schema::check_major(manifest.schema_major)
        .map_err(|found| Error::UnsupportedMajor { found })?;
    let mode = Mode::try_from(manifest.mode).map_err(|_| Error::InvalidMode {
        found: manifest.mode,
    })?;
    if mode == Mode::Unspecified {
        return Err(Error::InvalidMode {
            found: manifest.mode,
        });
    }
    validate_scopes(&manifest.scopes)?;
    if mode == Mode::Check {
        for (index, scope) in manifest.scopes.iter().enumerate() {
            if scope.results_complete != Some(true) {
                return Err(Error::ScopeCompletion { index });
            }
        }
    }
    let mut files = std::collections::BTreeSet::new();
    for file in &manifest.files {
        validate_file(file, mode, manifest.scopes.len())?;
        dx_proto_validate::check_unique_insert(&mut files, &file.path, |existing| {
            Error::DuplicateFile {
                path: (*existing).clone(),
            }
        })?;
    }
    validate_ignored(&manifest.ignored_imports, manifest.scopes.len())
}

pub fn encode_validated(manifest: &GenerationManifest) -> Result<Vec<u8>, Error> {
    dx_proto_validate::encode_with_validation(manifest, validate)
}

pub fn decode_validated(bytes: &[u8]) -> Result<GenerationManifest, Error> {
    dx_proto_validate::decode_with_validation(bytes, validate, Error::Decode)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proto::{Edit, WriteOutcome};

    fn modification(content: &[u8]) -> Modification {
        Modification {
            original_digest: digest(content).to_vec(),
            original_content: content.to_vec(),
            edits: vec![Edit {
                start_byte: 0,
                end_byte: 1,
                replacement: b"z".to_vec(),
            }],
        }
    }

    fn sample(mode: Mode) -> GenerationManifest {
        GenerationManifest {
            schema_major: SCHEMA_MAJOR,
            schema_minor: SCHEMA_MINOR,
            mode: mode as i32,
            scopes: vec![Scope {
                value: "//...".into(),
                results_complete: Some(true),
            }],
            files: vec![FileResult {
                path: "pkg/BUILD.bazel".into(),
                scope_index: 0,
                change: Some(file_result::Change::Modification(modification(b"abc\n"))),
                outcome: if mode == Mode::Default {
                    WriteOutcome::Applied as i32
                } else {
                    0
                },
                failure_code: String::new(),
            }],
            ignored_imports: vec![IgnoredImport {
                path: "pkg/a.rs".into(),
                language: "rust".into(),
                import: "optional".into(),
                scope_index: 0,
            }],
        }
    }

    fn sample_with(mode: Mode, modification: Modification) -> GenerationManifest {
        let mut manifest = sample(mode);
        manifest.files[0].change = Some(file_result::Change::Modification(modification));
        manifest
    }

    fn assert_error(
        mut manifest: GenerationManifest,
        mutate: impl FnOnce(&mut GenerationManifest),
        expected: Error,
    ) {
        mutate(&mut manifest);
        assert_eq!(validate(&manifest), Err(expected));
    }

    #[test]
    fn digest_roundtrip_and_compatibility() {
        let expected = [
            0xaf, 0x13, 0x49, 0xb9, 0xf5, 0xf9, 0xa1, 0xa6, 0xa0, 0x40, 0x4d, 0xea, 0x36, 0xdc,
            0xc9, 0x49, 0x9b, 0xcb, 0x25, 0xc9, 0xad, 0xc1, 0x12, 0xb7, 0xcc, 0x9a, 0x93, 0xca,
            0xe4, 0x1f, 0x32, 0x62,
        ];
        assert_eq!(digest(b""), expected);
        let mut manifest = sample(Mode::Default);
        manifest.schema_minor += 1;
        let bytes = encode_validated(&manifest).unwrap();
        assert_eq!(decode_validated(&bytes).unwrap(), manifest);
        let mut unknown = bytes;
        unknown.extend_from_slice(&[0xa0, 0x06, 0x07]);
        assert_eq!(decode_validated(&unknown).unwrap(), manifest);
        assert!(matches!(
            decode_validated(&[0xff; 5]),
            Err(Error::Decode(_))
        ));
    }

    #[test]
    fn header_and_scope_rules() {
        assert_error(
            sample(Mode::Check),
            |m| m.schema_major = 2,
            Error::UnsupportedMajor { found: 2 },
        );
        assert_error(
            sample(Mode::Check),
            |m| m.mode = 99,
            Error::InvalidMode { found: 99 },
        );
        assert_error(
            sample(Mode::Check),
            |m| m.mode = 0,
            Error::InvalidMode { found: 0 },
        );
        assert_error(
            sample(Mode::Check),
            |m| m.scopes.clear(),
            Error::EmptyScopes,
        );
        assert_error(
            sample(Mode::Check),
            |m| m.scopes[0].value.clear(),
            Error::EmptyScope { index: 0 },
        );
        assert_error(
            sample(Mode::Check),
            |m| m.scopes.push(m.scopes[0].clone()),
            Error::DuplicateScope {
                value: "//...".into(),
            },
        );
        assert_error(
            sample(Mode::Check),
            |m| m.scopes[0].results_complete = None,
            Error::ScopeCompletion { index: 0 },
        );
        let mut partial = sample(Mode::Default);
        partial.scopes[0].results_complete = Some(false);
        assert!(validate(&partial).is_ok());
    }

    #[test]
    fn paths_scope_and_unique_files() {
        for path in [
            "",
            "/BUILD",
            "a//BUILD",
            "a/./BUILD",
            "a/../BUILD",
            "a\\BUILD",
        ] {
            let mut manifest = sample(Mode::Check);
            manifest.files[0].path = path.into();
            assert!(matches!(validate(&manifest), Err(Error::BadPath { .. })));
        }
        assert_error(
            sample(Mode::Check),
            |m| m.files[0].path = "pkg/file.rs".into(),
            Error::BadBuildBasename {
                path: "pkg/file.rs".into(),
            },
        );
        assert_error(
            sample(Mode::Check),
            |m| m.files[0].scope_index = 1,
            Error::ScopeIndex {
                at: "file".into(),
                found: 1,
            },
        );
        assert_error(
            sample(Mode::Check),
            |m| m.files.push(m.files[0].clone()),
            Error::DuplicateFile {
                path: "pkg/BUILD.bazel".into(),
            },
        );
    }

    #[test]
    fn path_messages_are_pinned_to_dx_path_ladder() {
        for (path, reason) in [
            ("", "path must be non-empty"),
            ("/BUILD", "path must be workspace-relative, not absolute"),
            ("C:/BUILD", "path must be workspace-relative, not absolute"),
            ("a\\BUILD", "path must use forward slashes"),
            ("a//BUILD", "path must have no empty component"),
            ("trailing/", "path must have no empty component"),
            ("a/./BUILD", "path must have no '.' component"),
            (".", "path must have no '.' component"),
            ("a/../BUILD", "path must have no '..' component"),
            ("..", "path must have no '..' component"),
        ] {
            let mut manifest = sample(Mode::Check);
            manifest.files[0].path = path.into();
            assert_eq!(
                validate(&manifest),
                Err(Error::BadPath {
                    at: "file".into(),
                    path: path.to_owned(),
                    reason,
                }),
                "path: {path:?}"
            );
        }
        let mut manifest = sample(Mode::Check);
        manifest.files[0].path = "/a//BUILD".into();
        assert_eq!(
            validate(&manifest),
            Err(Error::BadPath {
                at: "file".into(),
                path: "/a//BUILD".into(),
                reason: "path must be workspace-relative, not absolute",
            })
        );
        let mut manifest = sample(Mode::Check);
        manifest.files[0].path = "a\\//BUILD".into();
        assert_eq!(
            validate(&manifest),
            Err(Error::BadPath {
                at: "file".into(),
                path: "a\\//BUILD".into(),
                reason: "path must use forward slashes",
            })
        );
    }

    #[test]
    fn create_and_change_presence_rules() {
        let mut manifest = sample(Mode::Check);
        manifest.files[0].change = Some(file_result::Change::CreateContent(b"x\n".to_vec()));
        assert_eq!(candidate(&manifest.files[0]).unwrap(), b"x\n");
        manifest.files[0].change = Some(file_result::Change::CreateContent(vec![]));
        assert_eq!(candidate(&manifest.files[0]).unwrap(), b"");
        manifest.files[0].change = Some(file_result::Change::CreateContent(vec![0xff]));
        assert!(matches!(
            validate(&manifest),
            Err(Error::InvalidUtf8 { .. })
        ));
        manifest.files[0].change = None;
        assert!(matches!(
            validate(&manifest),
            Err(Error::MissingChange { .. })
        ));
    }

    #[test]
    fn file_text_hands_out_only_utf8() {
        let mut manifest = sample(Mode::Check);
        manifest.files[0].change = Some(file_result::Change::CreateContent(b"x\n".to_vec()));
        assert_eq!(
            file_text(&manifest.files[0]).unwrap(),
            FileText::Created { content: "x\n" }
        );
        manifest.files[0].change = Some(file_result::Change::CreateContent(vec![0xff]));
        assert!(matches!(
            file_text(&manifest.files[0]),
            Err(Error::InvalidUtf8 { .. })
        ));
        manifest.files[0].change = None;
        assert!(matches!(
            file_text(&manifest.files[0]),
            Err(Error::MissingChange { .. })
        ));

        let mut inner = modification(b"abc\n");
        inner.edits.push(Edit {
            start_byte: 4,
            end_byte: 4,
            replacement: b"q".to_vec(),
        });
        let manifest = sample_with(Mode::Check, inner);
        assert_eq!(
            file_text(&manifest.files[0]).unwrap(),
            FileText::Modified(ModificationText {
                original: "abc\n",
                original_digest: digest(b"abc\n").as_slice(),
                edits: vec![
                    Replacement {
                        start_byte: 0,
                        end_byte: 1,
                        text: "z",
                    },
                    Replacement {
                        start_byte: 4,
                        end_byte: 4,
                        text: "q",
                    },
                ],
                candidate: "zbc\nq".to_owned(),
            })
        );
        assert_eq!(candidate(&manifest.files[0]).unwrap(), b"zbc\nq");

        let mut invalid = modification(b"abc\n");
        invalid.original_content = vec![0xff];
        assert!(matches!(
            file_text(&sample_with(Mode::Check, invalid).files[0]),
            Err(Error::InvalidUtf8 { .. })
        ));
        let mut invalid = modification(b"abc\n");
        invalid.edits[0].replacement = vec![0xff];
        assert!(matches!(
            file_text(&sample_with(Mode::Check, invalid).files[0]),
            Err(Error::InvalidUtf8Replacement { .. })
        ));
    }

    #[test]
    fn modification_content_digest_and_edits() {
        let manifest = sample(Mode::Check);
        assert_eq!(candidate(&manifest.files[0]).unwrap(), b"zbc\n");
        let mut invalid = modification(b"abc\n");
        invalid.original_content = vec![0xff];
        assert!(matches!(
            validate(&sample_with(Mode::Check, invalid)),
            Err(Error::InvalidUtf8 { .. })
        ));

        let cases: Vec<(Box<dyn Fn(&mut Modification)>, Error)> = vec![
            (
                Box::new(|m| m.original_digest.pop().map(drop).unwrap_or(())),
                Error::BadDigestLen {
                    path: "pkg/BUILD.bazel".into(),
                    found: 31,
                },
            ),
            (
                Box::new(|m| m.original_digest[0] ^= 1),
                Error::DigestMismatch {
                    path: "pkg/BUILD.bazel".into(),
                },
            ),
            (
                Box::new(|m| m.edits.clear()),
                Error::EmptyEdits {
                    path: "pkg/BUILD.bazel".into(),
                },
            ),
            (
                Box::new(|m| m.edits[0].start_byte = 2),
                Error::InvertedEdit {
                    path: "pkg/BUILD.bazel".into(),
                    index: 0,
                },
            ),
            (
                Box::new(|m| m.edits[0].end_byte = 99),
                Error::EditOutOfBounds {
                    path: "pkg/BUILD.bazel".into(),
                    index: 0,
                },
            ),
            (
                Box::new(|m| m.edits[0].replacement = vec![0xff]),
                Error::InvalidUtf8Replacement {
                    path: "pkg/BUILD.bazel".into(),
                    index: 0,
                },
            ),
            (
                Box::new(|m| m.edits[0].replacement = b"a".to_vec()),
                Error::NoopEdit {
                    path: "pkg/BUILD.bazel".into(),
                    index: 0,
                },
            ),
        ];
        for (mutate, expected) in cases {
            let mut inner = modification(b"abc\n");
            mutate(&mut inner);
            assert_eq!(validate(&sample_with(Mode::Check, inner)), Err(expected));
        }
    }

    #[test]
    fn utf8_order_overlap_and_candidate_rules() {
        let mut split = modification("éx".as_bytes());
        split.edits[0].end_byte = 1;
        assert!(matches!(
            validate(&sample_with(Mode::Check, split)),
            Err(Error::EditNotUtf8Boundary { .. })
        ));

        let mut overlapping = modification(b"abc\n");
        overlapping.edits = vec![
            Edit {
                start_byte: 0,
                end_byte: 2,
                replacement: b"x".to_vec(),
            },
            Edit {
                start_byte: 1,
                end_byte: 1,
                replacement: b"y".to_vec(),
            },
        ];
        assert!(matches!(
            validate(&sample_with(Mode::Check, overlapping)),
            Err(Error::EditOrder { index: 1, .. })
        ));

        let mut same_start = modification(b"abc\n");
        same_start.edits = vec![
            Edit {
                start_byte: 0,
                end_byte: 1,
                replacement: b"x".to_vec(),
            },
            Edit {
                start_byte: 0,
                end_byte: 0,
                replacement: b"y".to_vec(),
            },
        ];
        assert!(matches!(
            validate(&sample_with(Mode::Check, same_start)),
            Err(Error::EditOrder { index: 1, .. })
        ));

        let mut compensated = modification(b"abc\n");
        compensated.edits = vec![
            Edit {
                start_byte: 0,
                end_byte: 1,
                replacement: Vec::new(),
            },
            Edit {
                start_byte: 1,
                end_byte: 1,
                replacement: b"a".to_vec(),
            },
        ];
        assert_eq!(
            validate(&sample_with(Mode::Check, compensated)),
            Err(Error::UnchangedCandidate {
                path: "pkg/BUILD.bazel".into()
            })
        );
    }

    #[test]
    fn mode_outcome_consistency() {
        assert_error(
            sample(Mode::Check),
            |m| m.files[0].outcome = WriteOutcome::Applied as i32,
            Error::CheckOutcome {
                path: "pkg/BUILD.bazel".into(),
            },
        );
        assert_error(
            sample(Mode::Default),
            |m| m.files[0].outcome = 0,
            Error::MissingOutcome {
                path: "pkg/BUILD.bazel".into(),
            },
        );
        assert_error(
            sample(Mode::Default),
            |m| m.files[0].outcome = 99,
            Error::InvalidOutcome {
                path: "pkg/BUILD.bazel".into(),
                found: 99,
            },
        );
        assert_error(
            sample(Mode::Default),
            |m| m.files[0].failure_code = "write".into(),
            Error::AppliedFailureCode {
                path: "pkg/BUILD.bazel".into(),
            },
        );
        let mut failed = sample(Mode::Default);
        failed.files[0].outcome = WriteOutcome::NotApplied as i32;
        assert_eq!(
            validate(&failed),
            Err(Error::MissingFailureCode {
                path: "pkg/BUILD.bazel".into()
            })
        );
        failed.files[0].failure_code = "write".into();
        assert!(validate(&failed).is_ok());
    }

    #[test]
    fn ignored_import_shape_order_and_uniqueness() {
        assert_error(
            sample(Mode::Check),
            |m| m.ignored_imports[0].path = "../x".into(),
            Error::BadPath {
                at: "ignored_imports[0]".into(),
                path: "../x".into(),
                reason: "path must have no '..' component",
            },
        );
        assert_error(
            sample(Mode::Check),
            |m| m.ignored_imports[0].scope_index = 1,
            Error::ScopeIndex {
                at: "ignored_import".into(),
                found: 1,
            },
        );
        assert_error(
            sample(Mode::Check),
            |m| m.ignored_imports[0].language.clear(),
            Error::EmptyIgnoredLanguage { index: 0 },
        );
        assert_error(
            sample(Mode::Check),
            |m| m.ignored_imports[0].import.clear(),
            Error::EmptyIgnoredImport { index: 0 },
        );
        assert_error(
            sample(Mode::Check),
            |m| m.ignored_imports.push(m.ignored_imports[0].clone()),
            Error::DuplicateIgnoredImport { index: 1 },
        );
        let mut unordered = sample(Mode::Check);
        let mut first = unordered.ignored_imports[0].clone();
        first.path = "a.rs".into();
        unordered.ignored_imports.push(first);
        assert_eq!(validate(&unordered), Err(Error::IgnoredOrder { index: 1 }));
        let mut ordered = sample(Mode::Check);
        ordered.ignored_imports.push(IgnoredImport {
            path: "pkg/b.rs".into(),
            language: "rust".into(),
            import: "optional".into(),
            scope_index: 0,
        });
        assert!(validate(&ordered).is_ok());
    }

    #[test]
    fn display_names_the_error() {
        assert!(Error::EmptyScopes.to_string().contains("empty scopes"));
    }
}
