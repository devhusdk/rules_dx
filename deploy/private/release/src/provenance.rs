//! Typed view of the in-toto Statement v1 / SLSA provenance v1 that `render_provenance` writes.
//! Verification reads these fields; it never matches on the raw text.

use std::fmt;

use serde::Deserialize;

pub const STATEMENT_TYPE: &str = "https://in-toto.io/Statement/v1";
pub const PREDICATE_TYPE: &str = "https://slsa.dev/provenance/v1";

#[derive(Clone, Debug, Default, Deserialize)]
pub struct Digest {
    #[serde(default)]
    pub sha256: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct Subject {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub digest: Digest,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct Builder {
    #[serde(default)]
    pub id: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct RunDetails {
    #[serde(default)]
    pub builder: Builder,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct Predicate {
    #[serde(rename = "runDetails", default)]
    pub run_details: RunDetails,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct Statement {
    #[serde(rename = "_type", default)]
    pub kind: String,
    #[serde(rename = "predicateType", default)]
    pub predicate_type: String,
    #[serde(default)]
    pub subject: Vec<Subject>,
    #[serde(default)]
    pub predicate: Predicate,
}

/// A provenance statement that does not bind the artifact to an allowlisted builder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProvenanceError {
    StatementType,
    PredicateType,
    Subject { digest: String, base: String },
    Builder { id: String, detail: String },
}

impl fmt::Display for ProvenanceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StatementType => f.write_str("provenance missing in-toto Statement v1"),
            Self::PredicateType => f.write_str("provenance missing SLSA v1 predicate"),
            Self::Subject { digest, base } => {
                write!(f, "provenance missing subject digest {digest} for {base}")
            }
            Self::Builder { id, detail } => write!(
                f,
                "provenance builder.id '{id}' is not an allowlisted workflow identity: {detail}"
            ),
        }
    }
}

impl std::error::Error for ProvenanceError {}

/// Checks every binding with exact field comparisons. `builder` is the identity the caller expects;
/// it must also pass the allowlist, so an arbitrary string fails closed.
pub fn statement_error(
    statement: &Statement,
    base: &str,
    digest: &str,
    builder: &str,
) -> Result<(), ProvenanceError> {
    if statement.kind != STATEMENT_TYPE {
        return Err(ProvenanceError::StatementType);
    }
    if statement.predicate_type != PREDICATE_TYPE {
        return Err(ProvenanceError::PredicateType);
    }
    if !statement
        .subject
        .iter()
        .any(|subject| subject.name == base && subject.digest.sha256 == digest)
    {
        return Err(ProvenanceError::Subject {
            digest: digest.to_owned(),
            base: base.to_owned(),
        });
    }
    let id = &statement.predicate.run_details.builder.id;
    let detail = match crate::provenance_builder_error(id) {
        Err(detail) => detail,
        Ok(()) if id == builder => return Ok(()),
        Ok(()) => format!("want '{builder}'"),
    };
    Err(ProvenanceError::Builder {
        id: id.clone(),
        detail,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PROVENANCE_BUILDER_RELEASE;

    const DIGEST: &str = "abc123";

    fn good(builder: &str) -> Statement {
        Statement {
            kind: STATEMENT_TYPE.to_owned(),
            predicate_type: PREDICATE_TYPE.to_owned(),
            subject: vec![Subject {
                name: "a.bin".to_owned(),
                digest: Digest {
                    sha256: DIGEST.to_owned(),
                },
            }],
            predicate: Predicate {
                run_details: RunDetails {
                    builder: Builder {
                        id: builder.to_owned(),
                    },
                },
            },
        }
    }

    fn check(statement: &Statement, builder: &str) -> Result<(), ProvenanceError> {
        statement_error(statement, "a.bin", DIGEST, builder)
    }

    #[test]
    fn accepts_matching_statement() {
        assert!(
            check(
                &good(PROVENANCE_BUILDER_RELEASE),
                PROVENANCE_BUILDER_RELEASE
            )
            .is_ok()
        );
    }

    #[test]
    fn accepts_extra_subjects() {
        let mut statement = good(PROVENANCE_BUILDER_RELEASE);
        statement.subject.insert(0, Subject::default());
        assert!(check(&statement, PROVENANCE_BUILDER_RELEASE).is_ok());
    }

    #[test]
    fn rejects_wrong_statement_type() {
        let mut statement = good(PROVENANCE_BUILDER_RELEASE);
        statement.kind.push('x');
        let err = check(&statement, PROVENANCE_BUILDER_RELEASE).expect_err("type");
        assert_eq!(err, ProvenanceError::StatementType);
        assert_eq!(err.to_string(), "provenance missing in-toto Statement v1");
    }

    #[test]
    fn rejects_wrong_predicate_type() {
        let mut statement = good(PROVENANCE_BUILDER_RELEASE);
        statement.predicate_type.clear();
        let err = check(&statement, PROVENANCE_BUILDER_RELEASE).expect_err("predicate");
        assert_eq!(err, ProvenanceError::PredicateType);
        assert_eq!(err.to_string(), "provenance missing SLSA v1 predicate");
    }

    #[test]
    fn rejects_wrong_subject() {
        for (name, digest) in [("other.bin", DIGEST), ("a.bin", "000")] {
            let mut statement = good(PROVENANCE_BUILDER_RELEASE);
            statement.subject[0].name = name.to_owned();
            statement.subject[0].digest.sha256 = digest.to_owned();
            let err = check(&statement, PROVENANCE_BUILDER_RELEASE).expect_err("subject");
            assert!(matches!(err, ProvenanceError::Subject { .. }), "{err:?}");
            assert_eq!(
                err.to_string(),
                format!("provenance missing subject digest {DIGEST} for a.bin")
            );
        }
    }

    #[test]
    fn rejects_prefix_forged_and_mismatched_builders() {
        let prefixed = format!("{PROVENANCE_BUILDER_RELEASE}.evil");
        for (id, want) in [
            (prefixed.as_str(), prefixed.as_str()),
            ("", PROVENANCE_BUILDER_RELEASE),
        ] {
            let err = check(&good(id), want).expect_err("builder");
            assert!(matches!(err, ProvenanceError::Builder { .. }), "{err:?}");
            let err = err.to_string();
            assert!(err.contains("builder.id"), "{err}");
            assert!(err.contains("is not an allowlisted"), "{err}");
        }
    }
}
