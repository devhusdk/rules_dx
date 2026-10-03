use serde::{Deserialize, Serialize};

pub const ENVELOPE_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileOperation {
    pub path: String,
    pub original_sha256: Option<String>,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Envelope {
    pub version: u32,
    pub operations: Vec<FileOperation>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EnvelopeError {
    #[error("invalid envelope JSON: {0}")]
    InvalidJson(String),
    #[error("unsupported envelope version: {got}")]
    UnsupportedVersion { got: u32 },
    #[error("envelope has no operations")]
    EmptyOperations,
    #[error("envelope operation has empty path")]
    EmptyPath,
    #[error("envelope is not serializable: {0}")]
    Unserializable(String),
    #[error("invalid digest: {path}")]
    InvalidDigest { path: String },
}

pub fn parse_envelope(json: &str) -> Result<Envelope, EnvelopeError> {
    let envelope: Envelope =
        serde_json::from_str(json).map_err(|err| EnvelopeError::InvalidJson(err.to_string()))?;
    if envelope.version != ENVELOPE_VERSION {
        return Err(EnvelopeError::UnsupportedVersion {
            got: envelope.version,
        });
    }
    if envelope.operations.is_empty() {
        return Err(EnvelopeError::EmptyOperations);
    }
    for op in &envelope.operations {
        if op.path.is_empty() {
            return Err(EnvelopeError::EmptyPath);
        }
        if let Some(digest) = &op.original_sha256 {
            if !dx_digest::is_hex(digest) {
                return Err(EnvelopeError::InvalidDigest {
                    path: op.path.clone(),
                });
            }
        }
    }
    Ok(envelope)
}

pub fn emit_envelope(envelope: &Envelope) -> Result<String, EnvelopeError> {
    // LCOV_EXCL_START - reason: unreachable serialize, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
    serde_json::to_string(envelope).map_err(|err| EnvelopeError::Unserializable(err.to_string()))
    // LCOV_EXCL_STOP - reason: end unreachable serialize, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Envelope {
        Envelope {
            version: ENVELOPE_VERSION,
            operations: vec![FileOperation {
                path: "a.txt".to_owned(),
                original_sha256: Some(dx_digest::sha256_hex(b"old\n")),
                content: "new\n".to_owned(),
            }],
        }
    }

    #[test]
    fn round_trip_parse_emit() -> Result<(), EnvelopeError> {
        let envelope = sample();
        let json = emit_envelope(&envelope)?;
        assert_eq!(parse_envelope(&json), Ok(envelope));
        Ok(())
    }

    #[test]
    fn create_without_digest_parses() -> Result<(), EnvelopeError> {
        let envelope = Envelope {
            version: ENVELOPE_VERSION,
            operations: vec![FileOperation {
                path: "new.txt".to_owned(),
                original_sha256: None,
                content: "hi\n".to_owned(),
            }],
        };
        let json = emit_envelope(&envelope)?;
        assert_eq!(parse_envelope(&json), Ok(envelope));
        Ok(())
    }

    #[test]
    fn missing_required_key_fails() {
        let err = parse_envelope(r#"{"version":1}"#).expect_err("operations is required");
        assert!(matches!(err, EnvelopeError::InvalidJson(_)));
    }

    #[test]
    fn unknown_key_fails() {
        let err = parse_envelope(r#"{"version":1,"operations":[],"extra":true}"#)
            .expect_err("unknown keys are rejected");
        assert!(matches!(err, EnvelopeError::InvalidJson(_)));
    }

    #[test]
    fn wrong_version_fails() {
        assert_eq!(
            parse_envelope(r#"{"version":2,"operations":[]}"#),
            Err(EnvelopeError::UnsupportedVersion { got: 2 })
        );
    }

    #[test]
    fn empty_operations_fail() {
        assert_eq!(
            parse_envelope(r#"{"version":1,"operations":[]}"#),
            Err(EnvelopeError::EmptyOperations)
        );
    }

    #[test]
    fn malformed_digest_fails() {
        let json = r#"{"version":1,"operations":[{"path":"a.txt","original_sha256":"xyz","content":"x"}]}"#;
        assert_eq!(
            parse_envelope(json),
            Err(EnvelopeError::InvalidDigest {
                path: "a.txt".to_owned()
            })
        );
    }

    #[test]
    fn uppercase_digest_fails() {
        let digest = dx_digest::sha256_hex(b"old\n").to_uppercase();
        let json = format!(
            r#"{{"version":1,"operations":[{{"path":"a.txt","original_sha256":"{digest}","content":"x"}}]}}"#
        );
        assert_eq!(
            parse_envelope(&json),
            Err(EnvelopeError::InvalidDigest {
                path: "a.txt".to_owned()
            })
        );
    }

    #[test]
    fn empty_path_fails() {
        let json = r#"{"version":1,"operations":[{"path":"","content":"x"}]}"#;
        assert_eq!(parse_envelope(json), Err(EnvelopeError::EmptyPath));
    }
}
