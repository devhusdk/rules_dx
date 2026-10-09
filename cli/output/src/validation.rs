#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum OutputError {
    #[error("unknown output mode {value:?}")]
    UnknownOutputMode { value: String },
    #[error("conflicting stdout report with {mode} mode: reports must target files")]
    ConflictingStdoutReport { mode: &'static str },
    #[error("second stdout report: at most one report may target stdout")]
    SecondStdoutReport,
    #[error("empty {field}: want a non-empty value")]
    EmptyField { field: &'static str },
    #[error("invalid command mode {value:?}: want default or check")]
    BadCommandMode { value: String },
    #[error("invalid severity {value:?}")]
    BadSeverity { value: String },
    #[error("invalid threshold {value:?}")]
    BadThreshold { value: String },
    #[error("invalid skew {value:?}: want proceed|warn|refuse")]
    BadSkew { value: String },
    #[error("invalid log level {value:?}: want error|warn|info|debug|trace")]
    BadLogLevel { value: String },
    #[error("invalid color {value:?}: want auto|always|never")]
    BadColor { value: String },
    #[error("invalid path {path:?}: {reason}")]
    BadPath { path: String, reason: &'static str },
    #[error("invalid digest for {field} {value:?}")]
    BadDigest { field: &'static str, value: String },
    #[error("invalid edit at index {index}: {reason}")]
    BadEdit { index: usize, reason: &'static str },
    #[error("range without path: ranges require a file path")]
    RangeWithoutPath,
    #[error("inverted range: start must not exceed end")]
    InvertedRange,
    #[error("missing resolution: default-mode diagnostics require a resolution")]
    MissingResolution,
    #[error("unexpected resolution: check-mode diagnostics carry no resolution")]
    UnexpectedResolution,
    #[error("missing failure reason for {path:?}")]
    MissingReason { path: String },
    #[error("unexpected failure reason for {path:?}: applied mutations carry no reason")]
    UnexpectedReason { path: String },
    #[error("not an event: value is not an NDJSON event object")]
    NotAnEvent,
    #[error("I/O error: {0}")]
    Io(String),
    #[error("invalid correlation {value:?}: want 1-128 chars of [A-Za-z0-9/_:.-]")]
    BadCorrelation { value: String },
}

impl OutputError {
    pub fn is_broken_pipe(&self) -> bool {
        match self {
            OutputError::Io(message) => message.to_lowercase().contains("broken pipe"),
            _ => false,
        }
    }
}

pub fn check_path(path: &str) -> Result<(), OutputError> {
    match dx_path::reject_reason(path) {
        None => Ok(()),
        Some(reason) => Err(OutputError::BadPath {
            path: path.to_owned(),
            reason,
        }),
    }
}

pub fn parse_digest(field: &'static str, text: &str) -> Result<[u8; 32], OutputError> {
    dx_digest::parse_hex(text).map_err(|_| OutputError::BadDigest {
        field,
        value: text.to_owned(),
    })
}

pub fn check_correlation(value: &str) -> Result<(), OutputError> {
    if value.is_empty() || value.len() > 128 {
        return Err(OutputError::BadCorrelation {
            value: value.to_owned(),
        });
    }
    let ok = value.bytes().all(|b| {
        b.is_ascii_alphanumeric() || b == b'/' || b == b'_' || b == b':' || b == b'.' || b == b'-'
    });
    if !ok {
        return Err(OutputError::BadCorrelation {
            value: value.to_owned(),
        });
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edit {
    pub start: u64,
    pub end: u64,
    pub replacement: String,
}

pub fn check_edits(edits: &[Edit]) -> Result<(), OutputError> {
    if edits.is_empty() {
        return Err(OutputError::BadEdit {
            index: 0,
            reason: "edit set must be non-empty",
        });
    }
    for (index, edit) in edits.iter().enumerate() {
        if edit.start > edit.end {
            return Err(OutputError::BadEdit {
                index,
                reason: "start must not exceed end",
            });
        }
        if index > 0 {
            let prev = &edits[index - 1];
            if edit.start <= prev.start {
                return Err(OutputError::BadEdit {
                    index,
                    reason: "edits must be in strictly increasing start order",
                });
            }
            if prev.end > edit.start {
                return Err(OutputError::BadEdit {
                    index,
                    reason: "an insertion cannot sit inside a replaced range",
                });
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIGEST: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    #[test]
    fn paths_mirror_result_protocol_rules() {
        check_path("src/app.py").expect("valid");
        assert!(check_path("").is_err());
        assert!(check_path("/abs").is_err());
        assert!(check_path("C:/Windows/System32/x").is_err());
        assert!(check_path("a\\b").is_err());
        assert!(check_path("a//b").is_err());
        assert!(check_path("./a").is_err());
        assert!(check_path("a/../b").is_err());
    }

    #[test]
    fn path_messages_are_pinned_to_dx_path_ladder() {
        for (path, reason) in [
            ("", "path must be non-empty"),
            ("/abs", "path must be workspace-relative, not absolute"),
            (
                "C:/Windows/System32/x",
                "path must be workspace-relative, not absolute",
            ),
            ("a\\b", "path must use forward slashes"),
            ("a//b", "path must have no empty component"),
            ("trailing/", "path must have no empty component"),
            ("./a", "path must have no '.' component"),
            (".", "path must have no '.' component"),
            ("a/../b", "path must have no '..' component"),
            ("..", "path must have no '..' component"),
        ] {
            assert_eq!(
                check_path(path),
                Err(OutputError::BadPath {
                    path: path.to_owned(),
                    reason,
                }),
                "path: {path:?}"
            );
        }
        assert_eq!(
            check_path("/a//b"),
            Err(OutputError::BadPath {
                path: "/a//b".to_owned(),
                reason: "path must be workspace-relative, not absolute",
            })
        );
        assert_eq!(
            check_path("a/./../b"),
            Err(OutputError::BadPath {
                path: "a/./../b".to_owned(),
                reason: "path must have no '.' component",
            })
        );
    }

    #[test]
    fn digests_require_lowercase_hex() {
        assert_eq!(parse_digest("d", DIGEST).expect("valid").len(), 32);
        assert!(parse_digest("d", &DIGEST.to_uppercase()).is_err());
        assert!(parse_digest("d", "0123").is_err());
        assert!(parse_digest("d", &format!("{DIGEST}00")).is_err());
        assert!(parse_digest("d", &"zz".repeat(32)).is_err());
    }

    #[test]
    fn edit_shapes_validated() {
        let edits = vec![
            Edit {
                start: 0,
                end: 5,
                replacement: String::new(),
            },
            Edit {
                start: 5,
                end: 5,
                replacement: "x".to_owned(),
            },
        ];
        check_edits(&edits).expect("adjacent ok");
        assert!(check_edits(&[]).is_err());
        assert!(check_edits(&[Edit {
            start: 9,
            end: 3,
            replacement: String::new(),
        }])
        .is_err());
        assert!(check_edits(&[
            Edit {
                start: 4,
                end: 4,
                replacement: "a".to_owned(),
            },
            Edit {
                start: 4,
                end: 4,
                replacement: "b".to_owned(),
            },
        ])
        .is_err());
        assert!(check_edits(&[
            Edit {
                start: 2,
                end: 8,
                replacement: "a".to_owned(),
            },
            Edit {
                start: 5,
                end: 5,
                replacement: "b".to_owned(),
            },
        ])
        .is_err());
    }

    #[test]
    fn display_is_human_readable() {
        assert_eq!(
            OutputError::SecondStdoutReport.to_string(),
            "second stdout report: at most one report may target stdout"
        );
        assert_eq!(
            OutputError::MissingResolution.to_string(),
            "missing resolution: default-mode diagnostics require a resolution"
        );
    }

    #[test]
    fn correlation_shape_validated() {
        check_correlation("update:cargo").expect("namespaced");
        check_correlation("run//app:bin").expect("target scope");
        check_correlation("check/format").expect("umbrella phase");
        assert!(check_correlation("").is_err());
        assert!(check_correlation("has space").is_err());
        assert!(check_correlation(&"x".repeat(129)).is_err());
        assert_eq!(
            check_correlation("bad!").expect_err("punctuation"),
            OutputError::BadCorrelation {
                value: "bad!".to_owned()
            }
        );
    }

    #[test]
    fn broken_pipe_detected_case_insensitive() {
        assert!(OutputError::Io("Broken pipe (os error 32)".to_owned()).is_broken_pipe());
        assert!(OutputError::Io("broken pipe".to_owned()).is_broken_pipe());
        assert!(!OutputError::Io("boom".to_owned()).is_broken_pipe());
        assert!(!OutputError::NotAnEvent.is_broken_pipe());
    }
}
