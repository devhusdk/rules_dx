#![cfg_attr(not(test), deny(clippy::expect_used, clippy::unwrap_used))]

use proto::{
    Capability, Convergence, Diagnostic, Edit, FileEdits, FileSnapshot, QualityResult, Severity,
    Stage,
};
pub use result_proto::dx::quality::v1 as proto;

pub use dx_schema::SCHEMA_MAJOR;
pub use dx_schema::SCHEMA_MINOR;
pub const MAX_COMPLETED_ROUNDS: u32 = 10;
pub use dx_digest::DIGEST_LEN;

pub fn digest(bytes: &[u8]) -> [u8; 32] {
    dx_digest::blake3(bytes)
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error("cannot decode quality result: {0}")]
    Decode(String),
    #[error("unsupported schema major {found}: want {major}", major = SCHEMA_MAJOR)]
    UnsupportedMajor { found: u32 },
    #[error("empty producer: want a non-empty tool identity")]
    EmptyProducer,
    #[error("invalid capability: want a known capability")]
    InvalidCapability,
    #[error("invalid convergence: want a known convergence state")]
    InvalidConvergence,
    #[error("too many completed rounds ({found}): want at most {max}", max = MAX_COMPLETED_ROUNDS)]
    TooManyRounds { found: u32 },
    #[error("empty stages: want at least one stage")]
    EmptyStages,
    #[error("empty tool id at stage {stage}: want a non-empty tool identity")]
    EmptyStageToolId { stage: usize },
    #[error("empty classes at stage {stage}: want at least one file class")]
    EmptyStageClasses { stage: usize },
    #[error("empty sources at stage {stage}: want at least one source")]
    EmptyStageSources { stage: usize },
    #[error("invalid path at {at} {path:?}: {reason}")]
    BadPath {
        at: String,
        path: String,
        reason: &'static str,
    },
    #[error("duplicate snapshot path {path:?}")]
    DuplicateSnapshotPath { path: String },
    #[error("invalid digest length at {at} {path:?}: found {found} bytes, want {len}", len = DIGEST_LEN)]
    BadDigestLen {
        at: String,
        path: String,
        found: usize,
    },
    #[error("invalid severity at diagnostic[{index}]: want info, warning, or error")]
    BadSeverity { index: usize },
    #[error("empty message at diagnostic[{index}]: want a non-empty message")]
    EmptyDiagnosticMessage { index: usize },
    #[error("empty tool id at diagnostic[{index}]: want a non-empty tool identity")]
    EmptyDiagnosticToolId { index: usize },
    #[error("range without path at diagnostic[{index}]: ranges require a path")]
    RangeWithoutPath { index: usize },
    #[error("missing range at diagnostic[{index}]: path diagnostics require a byte range")]
    MissingRange { index: usize },
    #[error("inverted range at diagnostic[{index}]: start must not exceed end")]
    InvertedRange { index: usize },
    #[error("empty edits for {path:?}: want at least one edit")]
    EmptyEdits { path: String },
    #[error("no-op edit at {path:?}[{index}]: replacement is identical")]
    NoopEdit { path: String, index: usize },
    #[error("inverted edit at {path:?}[{index}]: start must not exceed end")]
    InvertedEdit { path: String, index: usize },
    #[error("invalid UTF-8 replacement at {path:?}[{index}]")]
    InvalidUtf8Replacement { path: String, index: usize },
    #[error("unordered edit at {path:?}[{index}]: edits must be ordered and non-overlapping")]
    EditOrder { path: String, index: usize },
    #[error("duplicate replacements path {path:?}")]
    DuplicateReplacementsPath { path: String },
    #[error("replacements without a stable terminal snapshot")]
    ReplacementsWithoutStability,
}

fn check_path(at: &str, path: &str) -> Result<(), Error> {
    match dx_path::reject_reason(path) {
        None => Ok(()),
        Some(reason) => Err(Error::BadPath {
            at: at.to_owned(),
            path: path.to_owned(),
            reason,
        }),
    }
}

fn check_snapshot(snapshot: &FileSnapshot, at: &str) -> Result<(), Error> {
    check_path(at, &snapshot.path)?;
    if snapshot.digest.len() != DIGEST_LEN {
        return Err(Error::BadDigestLen {
            at: at.to_owned(),
            path: snapshot.path.clone(),
            found: snapshot.digest.len(),
        });
    }
    Ok(())
}

fn check_unique_paths(
    paths: impl Iterator<Item = String>,
    duplicate: impl Fn(String) -> Error,
) -> Result<(), Error> {
    let mut seen = std::collections::BTreeSet::new();
    for path in paths {
        dx_proto_validate::check_unique_insert(&mut seen, path, |existing| {
            duplicate(existing.clone())
        })?;
    }
    Ok(())
}

fn check_diagnostic(diagnostic: &Diagnostic, index: usize) -> Result<(), Error> {
    if diagnostic.severity == Severity::Unspecified as i32 {
        return Err(Error::BadSeverity { index });
    }
    if diagnostic.message.is_empty() {
        return Err(Error::EmptyDiagnosticMessage { index });
    }
    if diagnostic.tool_id.is_empty() {
        return Err(Error::EmptyDiagnosticToolId { index });
    }
    if diagnostic.path.is_empty() {
        if diagnostic.start_byte.is_some() || diagnostic.end_byte.is_some() {
            return Err(Error::RangeWithoutPath { index });
        }
        return Ok(());
    }
    check_path(&format!("diagnostic[{index}]"), &diagnostic.path)?;
    match (diagnostic.start_byte, diagnostic.end_byte) {
        (Some(start), Some(end)) => {
            if start > end {
                return Err(Error::InvertedRange { index });
            }
            Ok(())
        }
        _ => Err(Error::MissingRange { index }),
    }
}

fn check_edits(file: &FileEdits) -> Result<(), Error> {
    check_path("replacements", &file.path)?;
    if file.original_digest.len() != DIGEST_LEN {
        return Err(Error::BadDigestLen {
            at: "replacements".to_owned(),
            path: file.path.clone(),
            found: file.original_digest.len(),
        });
    }
    if file.edits.is_empty() {
        return Err(Error::EmptyEdits {
            path: file.path.clone(),
        });
    }
    let mut previous: Option<&Edit> = None;
    for (index, edit) in file.edits.iter().enumerate() {
        if edit.start_byte > edit.end_byte {
            return Err(Error::InvertedEdit {
                path: file.path.clone(),
                index,
            });
        }
        if edit.start_byte == edit.end_byte && edit.replacement.is_empty() {
            return Err(Error::NoopEdit {
                path: file.path.clone(),
                index,
            });
        }
        if str::from_utf8(&edit.replacement).is_err() {
            return Err(Error::InvalidUtf8Replacement {
                path: file.path.clone(),
                index,
            });
        }
        if let Some(previous) = previous {
            if previous.start_byte >= edit.start_byte || previous.end_byte > edit.start_byte {
                return Err(Error::EditOrder {
                    path: file.path.clone(),
                    index,
                });
            }
        }
        previous = Some(edit);
    }
    Ok(())
}

pub fn validate(result: &QualityResult) -> Result<(), Error> {
    dx_schema::check_major(result.schema_major)
        .map_err(|found| Error::UnsupportedMajor { found })?;
    if result.producer.is_empty() {
        return Err(Error::EmptyProducer);
    }
    if result.capability == Capability::Unspecified as i32 {
        return Err(Error::InvalidCapability);
    }
    if result.convergence == Convergence::Unspecified as i32 {
        return Err(Error::InvalidConvergence);
    }
    if result.completed_rounds > MAX_COMPLETED_ROUNDS {
        return Err(Error::TooManyRounds {
            found: result.completed_rounds,
        });
    }
    if result.stages.is_empty() {
        return Err(Error::EmptyStages);
    }
    for (index, stage) in result.stages.iter().enumerate() {
        validate_stage(stage, index)?;
    }
    for snapshot in &result.original_snapshot {
        check_snapshot(snapshot, "original_snapshot")?;
    }
    check_unique_paths(
        result.original_snapshot.iter().map(|s| s.path.clone()),
        |path| Error::DuplicateSnapshotPath { path },
    )?;
    for snapshot in &result.terminal_snapshot {
        check_snapshot(snapshot, "terminal_snapshot")?;
    }
    check_unique_paths(
        result.terminal_snapshot.iter().map(|s| s.path.clone()),
        |path| Error::DuplicateSnapshotPath { path },
    )?;
    for (index, diagnostic) in result.initial_diagnostics.iter().enumerate() {
        check_diagnostic(diagnostic, index)?;
    }
    for (index, diagnostic) in result.terminal_diagnostics.iter().enumerate() {
        check_diagnostic(diagnostic, index)?;
    }
    for file in &result.replacements {
        check_edits(file)?;
    }
    check_unique_paths(result.replacements.iter().map(|f| f.path.clone()), |path| {
        Error::DuplicateReplacementsPath { path }
    })?;
    if result.convergence != Convergence::Stable as i32 && !result.replacements.is_empty() {
        return Err(Error::ReplacementsWithoutStability);
    }
    Ok(())
}

fn validate_stage(stage: &Stage, index: usize) -> Result<(), Error> {
    if stage.tool_id.is_empty() {
        return Err(Error::EmptyStageToolId { stage: index });
    }
    if stage.class_ids.is_empty() {
        return Err(Error::EmptyStageClasses { stage: index });
    }
    if stage.source_paths.is_empty() {
        return Err(Error::EmptyStageSources { stage: index });
    }
    for path in &stage.source_paths {
        check_path(&format!("stages[{index}]"), path)?;
    }
    Ok(())
}

pub fn encode_validated(result: &QualityResult) -> Result<Vec<u8>, Error> {
    dx_proto_validate::encode_with_validation(result, validate)
}

pub fn decode_validated(bytes: &[u8]) -> Result<QualityResult, Error> {
    dx_proto_validate::decode_with_validation(bytes, validate, Error::Decode)
}

pub fn assert_all_equal<T: PartialEq + std::fmt::Debug>(items: &[T]) {
    assert!(
        !items.is_empty(),
        "assert_all_equal: want at least one item"
    );
    for other in items.iter().skip(1) {
        assert_eq!(&items[0], other);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use prost::Message;

    fn digest_of(byte: u8) -> Vec<u8> {
        vec![byte; DIGEST_LEN]
    }

    fn sample() -> QualityResult {
        QualityResult {
            schema_major: SCHEMA_MAJOR,
            schema_minor: SCHEMA_MINOR,
            producer: "//quality:test".to_owned(),
            capability: Capability::Lint as i32,
            stages: vec![Stage {
                tool_id: "lint-a".to_owned(),
                class_ids: vec!["rust".to_owned()],
                source_paths: vec!["src/lib.rs".to_owned()],
            }],
            completed_rounds: 1,
            convergence: Convergence::Stable as i32,
            original_snapshot: vec![FileSnapshot {
                path: "src/lib.rs".to_owned(),
                digest: digest_of(0xAB),
            }],
            terminal_snapshot: vec![FileSnapshot {
                path: "src/lib.rs".to_owned(),
                digest: digest_of(0xCD),
            }],
            initial_diagnostics: vec![Diagnostic {
                severity: Severity::Warning as i32,
                message: "trailing whitespace".to_owned(),
                tool_id: "lint-a".to_owned(),
                path: "src/lib.rs".to_owned(),
                start_byte: Some(0),
                end_byte: Some(1),
                ..Default::default()
            }],
            terminal_diagnostics: vec![],
            replacements: vec![FileEdits {
                path: "src/lib.rs".to_owned(),
                original_digest: digest_of(0xAB),
                edits: vec![Edit {
                    start_byte: 0,
                    end_byte: 1,
                    replacement: b"x".to_vec(),
                }],
            }],
        }
    }

    #[test]
    fn digest_empty_matches_official_vector() {
        let expected = [
            0xaf, 0x13, 0x49, 0xb9, 0xf5, 0xf9, 0xa1, 0xa6, 0xa0, 0x40, 0x4d, 0xea, 0x36, 0xdc,
            0xc9, 0x49, 0x9b, 0xcb, 0x25, 0xc9, 0xad, 0xc1, 0x12, 0xb7, 0xcc, 0x9a, 0x93, 0xca,
            0xe4, 0x1f, 0x32, 0x62,
        ];
        assert_eq!(digest(b""), expected);
    }

    #[test]
    fn roundtrip_is_valid_and_deterministic() {
        let result = sample();
        let first = encode_validated(&result).unwrap();
        let second = encode_validated(&result).unwrap();
        assert_eq!(first, second);
        assert_eq!(decode_validated(&first).unwrap(), result);
    }

    #[test]
    fn newer_minor_accepted_when_bytes_satisfy_rules() {
        let mut result = sample();
        result.schema_minor = SCHEMA_MINOR + 1;
        let bytes = result.encode_to_vec();
        assert_eq!(
            decode_validated(&bytes).unwrap().schema_minor,
            SCHEMA_MINOR + 1
        );
    }

    #[test]
    fn unknown_major_rejected() {
        let mut result = sample();
        result.schema_major = SCHEMA_MAJOR + 1;
        let bytes = result.encode_to_vec();
        assert_eq!(
            decode_validated(&bytes),
            Err(Error::UnsupportedMajor {
                found: SCHEMA_MAJOR + 1
            })
        );
    }

    #[test]
    fn unknown_field_ignored() {
        let mut bytes = encode_validated(&sample()).unwrap();
        bytes.extend_from_slice(&[0xA0, 0x06, 0x07]);
        assert_eq!(decode_validated(&bytes).unwrap(), sample());
    }

    #[test]
    fn bad_paths_rejected() {
        for path in [
            "",
            "/absolute",
            "a//b",
            "a/./b",
            "a/../b",
            "..",
            ".",
            "trailing/",
            "back\\slash",
        ] {
            let mut result = sample();
            result.stages[0].source_paths = vec![path.to_owned()];
            assert!(validate(&result).is_err(), "path accepted: {path:?}");
        }
    }

    #[test]
    fn path_messages_are_pinned_to_dx_path_ladder() {
        for (path, reason) in [
            ("", "path must be non-empty"),
            ("/absolute", "path must be workspace-relative, not absolute"),
            (
                "C:/absolute",
                "path must be workspace-relative, not absolute",
            ),
            ("back\\slash", "path must use forward slashes"),
            ("a//b", "path must have no empty component"),
            ("trailing/", "path must have no empty component"),
            ("a/./b", "path must have no '.' component"),
            (".", "path must have no '.' component"),
            ("a/../b", "path must have no '..' component"),
            ("..", "path must have no '..' component"),
        ] {
            let mut result = sample();
            result.stages[0].source_paths = vec![path.to_owned()];
            assert_eq!(
                validate(&result),
                Err(Error::BadPath {
                    at: "stages[0]".to_owned(),
                    path: path.to_owned(),
                    reason,
                }),
                "path: {path:?}"
            );
        }
        let mut result = sample();
        result.stages[0].source_paths = vec!["/a//b".to_owned()];
        assert_eq!(
            validate(&result),
            Err(Error::BadPath {
                at: "stages[0]".to_owned(),
                path: "/a//b".to_owned(),
                reason: "path must be workspace-relative, not absolute",
            })
        );
        let mut result = sample();
        result.stages[0].source_paths = vec!["a/./../b".to_owned()];
        assert_eq!(
            validate(&result),
            Err(Error::BadPath {
                at: "stages[0]".to_owned(),
                path: "a/./../b".to_owned(),
                reason: "path must have no '.' component",
            })
        );
    }

    #[test]
    fn digest_length_enforced() {
        let mut result = sample();
        result.original_snapshot[0].digest = vec![0xAB; DIGEST_LEN - 1];
        assert_eq!(
            validate(&result),
            Err(Error::BadDigestLen {
                at: "original_snapshot".to_owned(),
                path: "src/lib.rs".to_owned(),
                found: DIGEST_LEN - 1,
            })
        );
    }

    #[test]
    fn diagnostic_shape_enforced() {
        let mut result = sample();
        result.terminal_diagnostics = vec![Diagnostic {
            severity: Severity::Unspecified as i32,
            ..Default::default()
        }];
        assert_eq!(validate(&result), Err(Error::BadSeverity { index: 0 }));

        let mut result = sample();
        result.terminal_diagnostics = vec![Diagnostic {
            severity: Severity::Error as i32,
            start_byte: Some(0),
            ..Default::default()
        }];
        assert!(validate(&result).is_err());

        let mut result = sample();
        result.terminal_diagnostics = vec![Diagnostic {
            severity: Severity::Error as i32,
            message: "m".to_owned(),
            tool_id: "t".to_owned(),
            start_byte: Some(2),
            end_byte: Some(1),
            path: "src/lib.rs".to_owned(),
            ..Default::default()
        }];
        assert_eq!(validate(&result), Err(Error::InvertedRange { index: 0 }));
    }

    #[test]
    fn edit_ordering_enforced() {
        let mut result = sample();
        result.replacements[0].edits = vec![
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
        assert_eq!(
            validate(&result),
            Err(Error::EditOrder {
                path: "src/lib.rs".to_owned(),
                index: 1,
            })
        );

        let mut result = sample();
        result.replacements[0].edits = vec![
            Edit {
                start_byte: 0,
                end_byte: 4,
                replacement: b"x".to_vec(),
            },
            Edit {
                start_byte: 2,
                end_byte: 2,
                replacement: b"y".to_vec(),
            },
        ];
        assert!(validate(&result).is_err());

        let mut result = sample();
        result.replacements[0].edits = vec![Edit {
            start_byte: 1,
            end_byte: 1,
            replacement: vec![],
        }];
        assert_eq!(
            validate(&result),
            Err(Error::NoopEdit {
                path: "src/lib.rs".to_owned(),
                index: 0,
            })
        );

        let mut result = sample();
        result.replacements[0].edits = vec![Edit {
            start_byte: 0,
            end_byte: 1,
            replacement: vec![0xFF],
        }];
        assert_eq!(
            validate(&result),
            Err(Error::InvalidUtf8Replacement {
                path: "src/lib.rs".to_owned(),
                index: 0,
            })
        );

        let mut result = sample();
        result.replacements[0].edits = vec![
            Edit {
                start_byte: 0,
                end_byte: 1,
                replacement: b"x".to_vec(),
            },
            Edit {
                start_byte: 1,
                end_byte: 2,
                replacement: b"y".to_vec(),
            },
        ];
        assert!(validate(&result).is_ok());
    }

    #[test]
    fn stability_gate_enforced() {
        let mut result = sample();
        result.convergence = Convergence::IterationLimit as i32;
        assert_eq!(validate(&result), Err(Error::ReplacementsWithoutStability));

        let mut result = sample();
        result.convergence = Convergence::IterationLimit as i32;
        result.replacements = vec![];
        assert!(validate(&result).is_ok());
    }

    #[test]
    fn header_shape_enforced() {
        let mut result = sample();
        result.producer.clear();
        assert_eq!(validate(&result), Err(Error::EmptyProducer));

        let mut result = sample();
        result.capability = Capability::Unspecified as i32;
        assert_eq!(validate(&result), Err(Error::InvalidCapability));

        let mut result = sample();
        result.convergence = Convergence::Unspecified as i32;
        assert_eq!(validate(&result), Err(Error::InvalidConvergence));

        let mut result = sample();
        result.completed_rounds = MAX_COMPLETED_ROUNDS + 1;
        assert_eq!(
            validate(&result),
            Err(Error::TooManyRounds {
                found: MAX_COMPLETED_ROUNDS + 1
            })
        );

        let mut result = sample();
        result.stages = vec![];
        assert_eq!(validate(&result), Err(Error::EmptyStages));
    }

    #[test]
    fn stage_shape_enforced() {
        let mut result = sample();
        result.stages[0].tool_id.clear();
        assert_eq!(validate(&result), Err(Error::EmptyStageToolId { stage: 0 }));

        let mut result = sample();
        result.stages[0].class_ids = vec![];
        assert_eq!(
            validate(&result),
            Err(Error::EmptyStageClasses { stage: 0 })
        );

        let mut result = sample();
        result.stages[0].source_paths = vec![];
        assert_eq!(
            validate(&result),
            Err(Error::EmptyStageSources { stage: 0 })
        );
    }

    #[test]
    fn diagnostic_identity_enforced() {
        let valid = || Diagnostic {
            severity: Severity::Error as i32,
            message: "m".to_owned(),
            tool_id: "t".to_owned(),
            ..Default::default()
        };

        let mut diagnostic = valid();
        diagnostic.start_byte = Some(0);
        let mut result = sample();
        result.terminal_diagnostics = vec![diagnostic];
        assert_eq!(validate(&result), Err(Error::RangeWithoutPath { index: 0 }));

        let mut result = sample();
        result.terminal_diagnostics = vec![valid()];
        assert!(validate(&result).is_ok());

        let mut diagnostic = valid();
        diagnostic.tool_id.clear();
        let mut result = sample();
        result.terminal_diagnostics = vec![diagnostic];
        assert_eq!(
            validate(&result),
            Err(Error::EmptyDiagnosticToolId { index: 0 })
        );

        let mut diagnostic = valid();
        diagnostic.path = "src/lib.rs".to_owned();
        diagnostic.start_byte = Some(0);
        let mut result = sample();
        result.terminal_diagnostics = vec![diagnostic];
        assert_eq!(validate(&result), Err(Error::MissingRange { index: 0 }));
    }

    #[test]
    fn replacements_shape_enforced() {
        let mut result = sample();
        result.replacements[0].original_digest = vec![0xAB; DIGEST_LEN + 1];
        assert_eq!(
            validate(&result),
            Err(Error::BadDigestLen {
                at: "replacements".to_owned(),
                path: "src/lib.rs".to_owned(),
                found: DIGEST_LEN + 1,
            })
        );

        let mut result = sample();
        result.replacements[0].edits = vec![];
        assert_eq!(
            validate(&result),
            Err(Error::EmptyEdits {
                path: "src/lib.rs".to_owned(),
            })
        );

        let mut result = sample();
        result.replacements[0].edits = vec![Edit {
            start_byte: 2,
            end_byte: 1,
            replacement: b"x".to_vec(),
        }];
        assert_eq!(
            validate(&result),
            Err(Error::InvertedEdit {
                path: "src/lib.rs".to_owned(),
                index: 0,
            })
        );

        let mut result = sample();
        result.replacements.push(result.replacements[0].clone());
        assert_eq!(
            validate(&result),
            Err(Error::DuplicateReplacementsPath {
                path: "src/lib.rs".to_owned(),
            })
        );
    }

    #[test]
    fn duplicate_snapshot_paths_rejected() {
        let mut result = sample();
        result
            .original_snapshot
            .push(result.original_snapshot[0].clone());
        assert_eq!(
            validate(&result),
            Err(Error::DuplicateSnapshotPath {
                path: "src/lib.rs".to_owned(),
            })
        );

        let mut result = sample();
        result
            .terminal_snapshot
            .push(result.terminal_snapshot[0].clone());
        assert_eq!(
            validate(&result),
            Err(Error::DuplicateSnapshotPath {
                path: "src/lib.rs".to_owned(),
            })
        );
    }

    #[test]
    fn malformed_bytes_rejected() {
        assert!(matches!(
            decode_validated(&[0xFF, 0xFF, 0xFF, 0xFF, 0xFF]),
            Err(Error::Decode(_))
        ));
    }

    #[test]
    fn error_display_reports_variant() {
        let rendered = format!("{}", Error::ReplacementsWithoutStability);
        assert!(rendered.contains("without a stable terminal snapshot"));
    }
}
