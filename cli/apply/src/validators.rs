use dx_digest::{is_hex, sha256_hex};

use super::envelope::FileOperation;

pub const MAX_OPERATION_BYTES: usize = 1024 * 1024;

pub const BLOCKED_EXTENSIONS: &[&str] = &[
    "a", "bin", "com", "dll", "dylib", "exe", "lib", "o", "obj", "so",
];

const INJECTION_PATTERNS: &[&str] = &[
    "ignore previous instructions",
    "ignore all prior instructions",
    "disregard all prior instructions",
    "override your system prompt",
    "reveal your system prompt",
];

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ValidationError {
    #[error("empty path")]
    EmptyPath,
    #[error("absolute path")]
    AbsolutePath,
    #[error("path escapes workspace")]
    EscapesWorkspace,
    #[error("malformed path")]
    MalformedPath,
    #[error("blocked extension: {extension}")]
    BlockedExtension { extension: String },
    #[error("operation too large: {bytes} bytes")]
    TooLarge { bytes: usize },
    #[error("prompt injection pattern: {pattern}")]
    PromptInjection { pattern: String },
    #[error("digest mismatch: {path}")]
    DigestMismatch { path: String },
    #[error("file exists: {path}")]
    FileExists { path: String },
    #[error("missing file: {path}")]
    MissingFile { path: String },
}

pub fn validate(op: &FileOperation, existing: Option<&[u8]>) -> Result<(), ValidationError> {
    match dx_path::classify(&op.path) {
        None => {}
        Some(dx_path::PathProblem::Empty) => return Err(ValidationError::EmptyPath),
        Some(dx_path::PathProblem::Absolute) => return Err(ValidationError::AbsolutePath),
        Some(dx_path::PathProblem::Backslash)
        | Some(dx_path::PathProblem::EmptyComponent)
        | Some(dx_path::PathProblem::Dot) => return Err(ValidationError::MalformedPath),
        Some(dx_path::PathProblem::DotDot) => return Err(ValidationError::EscapesWorkspace),
    }
    if let Some(extension) = extension_of(&op.path) {
        if BLOCKED_EXTENSIONS.contains(&extension.as_str()) {
            return Err(ValidationError::BlockedExtension { extension });
        }
    }
    if op.content.len() > MAX_OPERATION_BYTES {
        return Err(ValidationError::TooLarge {
            bytes: op.content.len(),
        });
    }
    let lowered = op.content.to_lowercase();
    for pattern in INJECTION_PATTERNS {
        if lowered.contains(pattern) {
            return Err(ValidationError::PromptInjection {
                pattern: (*pattern).to_owned(),
            });
        }
    }
    match (&op.original_sha256, existing) {
        (None, None) => Ok(()),
        (None, Some(_)) => Err(ValidationError::FileExists {
            path: op.path.clone(),
        }),
        (Some(_), None) => Err(ValidationError::MissingFile {
            path: op.path.clone(),
        }),
        (Some(expected), Some(bytes)) => {
            debug_assert!(is_hex(expected));
            if sha256_hex(bytes) == *expected {
                Ok(())
            } else {
                Err(ValidationError::DigestMismatch {
                    path: op.path.clone(),
                })
            }
        }
    }
}

fn extension_of(path: &str) -> Option<String> {
    let name = path.rsplit('/').next().unwrap_or_default();
    let (_, extension) = name.rsplit_once('.')?;
    if extension.is_empty() {
        return None;
    }
    Some(extension.to_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn update(path: &str, content: &str) -> FileOperation {
        FileOperation {
            path: path.to_owned(),
            original_sha256: Some(sha256_hex(b"old\n")),
            content: content.to_owned(),
        }
    }

    fn create(path: &str, content: &str) -> FileOperation {
        FileOperation {
            path: path.to_owned(),
            original_sha256: None,
            content: content.to_owned(),
        }
    }

    #[test]
    fn valid_update_passes() {
        assert_eq!(validate(&update("a.txt", "new\n"), Some(b"old\n")), Ok(()));
    }

    #[test]
    fn valid_create_passes() {
        assert_eq!(validate(&create("new.txt", "hi\n"), None), Ok(()));
    }

    #[test]
    fn absolute_path_rejected() {
        assert_eq!(
            validate(&create("/etc/x", "hi\n"), None),
            Err(ValidationError::AbsolutePath)
        );
    }

    #[test]
    fn drive_prefixed_path_rejected() {
        for path in ["C:/Windows/System32/drivers/etc/hosts", "C:hosts", "c:/x"] {
            assert_eq!(
                validate(&create(path, "hi\n"), None),
                Err(ValidationError::AbsolutePath),
                "path: {path:?}"
            );
        }
    }

    #[test]
    fn empty_path_rejected() {
        let mut empty = update("a.txt", "new\n");
        empty.path = String::new();
        assert_eq!(
            validate(&empty, Some(b"old\n")),
            Err(ValidationError::EmptyPath)
        );
    }

    #[test]
    fn parent_traversal_rejected() {
        assert_eq!(
            validate(&create("a/../../x", "hi\n"), None),
            Err(ValidationError::EscapesWorkspace)
        );
    }

    #[test]
    fn empty_segment_rejected() {
        assert_eq!(
            validate(&create("a//b", "hi\n"), None),
            Err(ValidationError::MalformedPath)
        );
    }

    #[test]
    fn backslash_and_dot_segment_rejected() {
        assert_eq!(
            validate(&create("a\\b", "hi\n"), None),
            Err(ValidationError::MalformedPath)
        );
        assert_eq!(
            validate(&create("a/./b", "hi\n"), None),
            Err(ValidationError::MalformedPath)
        );
    }

    #[test]
    fn path_rungs_are_pinned_to_dx_path_ladder() {
        for (path, expected) in [
            ("", ValidationError::EmptyPath),
            ("/etc/x", ValidationError::AbsolutePath),
            ("C:/Windows/System32/x", ValidationError::AbsolutePath),
            ("a\\b", ValidationError::MalformedPath),
            ("a//b", ValidationError::MalformedPath),
            ("trailing/", ValidationError::MalformedPath),
            ("a/./b", ValidationError::MalformedPath),
            (".", ValidationError::MalformedPath),
            ("a/../b", ValidationError::EscapesWorkspace),
            ("..", ValidationError::EscapesWorkspace),
        ] {
            assert_eq!(
                validate(&create(path, "hi\n"), None),
                Err(expected),
                "path: {path:?}"
            );
        }
        assert_eq!(
            validate(&create("/a//b", "hi\n"), None),
            Err(ValidationError::AbsolutePath)
        );
        assert_eq!(
            validate(&create("a\\//b", "hi\n"), None),
            Err(ValidationError::MalformedPath)
        );
        assert_eq!(
            validate(&create("a//./b", "hi\n"), None),
            Err(ValidationError::MalformedPath)
        );
        assert_eq!(
            validate(&create("a/./../b", "hi\n"), None),
            Err(ValidationError::MalformedPath)
        );
        assert_eq!(
            validate(&create("a/../../x", "hi\n"), None),
            Err(ValidationError::EscapesWorkspace)
        );
    }

    #[test]
    fn blocked_extension_rejected_case_insensitive() {
        assert_eq!(
            validate(&create("tool.EXE", "hi\n"), None),
            Err(ValidationError::BlockedExtension {
                extension: "exe".to_owned()
            })
        );
    }

    #[test]
    fn dotless_names_allowed() {
        assert_eq!(validate(&create("Makefile", "hi\n"), None), Ok(()));
    }

    #[test]
    fn trailing_dot_has_no_extension() {
        assert_eq!(validate(&create("dir/name.", "hi\n"), None), Ok(()));
    }

    #[test]
    fn oversize_content_rejected() {
        let big = "x".repeat(MAX_OPERATION_BYTES + 1);
        assert_eq!(
            validate(&create("a.txt", &big), None),
            Err(ValidationError::TooLarge { bytes: big.len() })
        );
    }

    #[test]
    fn injection_marker_rejected() {
        let err = validate(
            &create("a.txt", "Please IGNORE PREVIOUS INSTRUCTIONS now"),
            None,
        )
        .expect_err("injection must fail");
        assert_eq!(
            err,
            ValidationError::PromptInjection {
                pattern: "ignore previous instructions".to_owned()
            }
        );
    }

    #[test]
    fn digest_mismatch_rejected() {
        assert_eq!(
            validate(&update("a.txt", "new\n"), Some(b"other\n")),
            Err(ValidationError::DigestMismatch {
                path: "a.txt".to_owned()
            })
        );
    }

    #[test]
    fn create_over_existing_rejected() {
        assert_eq!(
            validate(&create("a.txt", "new\n"), Some(b"old\n")),
            Err(ValidationError::FileExists {
                path: "a.txt".to_owned()
            })
        );
    }

    #[test]
    fn update_of_missing_rejected() {
        assert_eq!(
            validate(&update("a.txt", "new\n"), None),
            Err(ValidationError::MissingFile {
                path: "a.txt".to_owned()
            })
        );
    }
}
