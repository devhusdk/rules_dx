use std::collections::{BTreeMap, BTreeSet};

use super::envelope::Envelope;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ConsensusError {
    #[error("envelope path sets differ")]
    PathSetMismatch {
        only_a: Vec<String>,
        only_b: Vec<String>,
    },
    #[error("envelope content differs: {path}")]
    ContentMismatch { path: String },
}

pub fn merge(a: &Envelope, b: &Envelope) -> Result<Envelope, ConsensusError> {
    let paths_a: BTreeSet<&str> = a.operations.iter().map(|op| op.path.as_str()).collect();
    let paths_b: BTreeSet<&str> = b.operations.iter().map(|op| op.path.as_str()).collect();
    if paths_a != paths_b {
        let only_a = paths_a
            .difference(&paths_b)
            .map(ToString::to_string)
            .collect();
        let only_b = paths_b
            .difference(&paths_a)
            .map(ToString::to_string)
            .collect();
        return Err(ConsensusError::PathSetMismatch { only_a, only_b });
    }
    let mut by_path_b: BTreeMap<&str, &str> = BTreeMap::new();
    for op in &b.operations {
        by_path_b.insert(op.path.as_str(), op.content.as_str());
    }
    let digest_b: BTreeMap<&str, Option<&str>> = b
        .operations
        .iter()
        .map(|op| (op.path.as_str(), op.original_sha256.as_deref()))
        .collect();
    for op in &a.operations {
        let same_content = by_path_b.get(op.path.as_str()) == Some(&op.content.as_str());
        let same_digest = digest_b.get(op.path.as_str()) == Some(&op.original_sha256.as_deref());
        if !same_content || !same_digest {
            return Err(ConsensusError::ContentMismatch {
                path: op.path.clone(),
            });
        }
    }
    Ok(Envelope {
        version: a.version,
        operations: a.operations.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::super::envelope::{ENVELOPE_VERSION, FileOperation};
    use super::*;

    fn op(path: &str, content: &str) -> FileOperation {
        FileOperation {
            path: path.to_owned(),
            original_sha256: None,
            content: content.to_owned(),
        }
    }

    fn envelope(ops: Vec<FileOperation>) -> Envelope {
        Envelope {
            version: ENVELOPE_VERSION,
            operations: ops,
        }
    }

    #[test]
    fn identical_envelopes_merge() {
        let a = envelope(vec![op("a.txt", "x\n"), op("b.txt", "y\n")]);
        let b = envelope(vec![op("b.txt", "y\n"), op("a.txt", "x\n")]);
        let merged = merge(&a, &b).expect("agree");
        assert_eq!(merged.operations, a.operations);
    }

    #[test]
    fn content_disagreement_conflicts() {
        let a = envelope(vec![op("a.txt", "x\n")]);
        let b = envelope(vec![op("a.txt", "y\n")]);
        assert_eq!(
            merge(&a, &b),
            Err(ConsensusError::ContentMismatch {
                path: "a.txt".to_owned()
            })
        );
    }

    #[test]
    fn digest_disagreement_conflicts() {
        let mut a = envelope(vec![op("a.txt", "x\n")]);
        let mut b = a.clone();
        a.operations[0].original_sha256 = Some("0".repeat(64));
        b.operations[0].original_sha256 = Some("1".repeat(64));
        assert_eq!(
            merge(&a, &b),
            Err(ConsensusError::ContentMismatch {
                path: "a.txt".to_owned()
            })
        );
    }

    #[test]
    fn path_set_mismatch_reports_both_sides() {
        let a = envelope(vec![op("a.txt", "x\n"), op("c.txt", "z\n")]);
        let b = envelope(vec![op("a.txt", "x\n"), op("b.txt", "y\n")]);
        assert_eq!(
            merge(&a, &b),
            Err(ConsensusError::PathSetMismatch {
                only_a: vec!["c.txt".to_owned()],
                only_b: vec!["b.txt".to_owned()],
            })
        );
    }
}
