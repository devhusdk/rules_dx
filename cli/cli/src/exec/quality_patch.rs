use super::common::{apply_to_bytes, FileChange, SourceRead};
use dx_diff::{render_patch, FilePatch, PatchKind};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub(crate) enum PatchError {
    #[error("cannot render patch without verified source for {path}")]
    MissingSource { path: String },
    #[error("source for {path} is not UTF-8 text")]
    NonUtf8Source { path: String },
    #[error("cannot apply recorded edits for {path}")]
    Unappliable { path: String },
    #[error("candidate for {path} is not UTF-8 text")]
    NonUtf8Candidate { path: String },
    #[error("failed to render patch: {detail}")]
    Render { detail: String },
}

pub(crate) fn render_diff_patch(
    sources: &BTreeMap<String, SourceRead>,
    changes: &[FileChange],
) -> Result<String, PatchError> {
    let mut owned: Vec<(String, String, String)> = Vec::with_capacity(changes.len());
    for change in changes {
        let original = match sources.get(&change.path) {
            Some(SourceRead::Bytes(bytes)) => bytes,
            _ => {
                return Err(PatchError::MissingSource {
                    path: change.path.clone(),
                });
            }
        };
        let original_text = match std::str::from_utf8(original) {
            Ok(text) => text,
            Err(_) => {
                return Err(PatchError::NonUtf8Source {
                    path: change.path.clone(),
                });
            }
        };
        let Some(candidate) = apply_to_bytes(original, &change.edits) else {
            return Err(PatchError::Unappliable {
                path: change.path.clone(),
            });
        };
        let candidate_text = match String::from_utf8(candidate) {
            Ok(text) => text,
            // LCOV_EXCL_START - reason: utf8 candidate, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
            Err(_) => {
                return Err(PatchError::NonUtf8Candidate {
                    path: change.path.clone(),
                });
            } // LCOV_EXCL_STOP - reason: end utf8 candidate, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
        };
        owned.push((
            change.path.clone(),
            original_text.to_owned(),
            candidate_text,
        ));
    }
    let patches: Vec<FilePatch<'_>> = owned
        .iter()
        .map(|(path, original, candidate)| FilePatch {
            path,
            kind: PatchKind::Modify,
            original,
            candidate,
        })
        .collect();
    render_patch(&patches).map_err(|error| PatchError::Render {
        detail: error.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::super::common::{FileChange, SourceRead};
    use super::{render_diff_patch, PatchError};
    use std::collections::BTreeMap;

    #[test]
    fn diff_patch_is_deterministic_and_untruncated() {
        let original = b"line one\nline two\n";
        let long_body = format!("{}\n", "x".repeat(10_000));
        let mut sources = BTreeMap::new();
        sources.insert("src/a.py".to_owned(), SourceRead::Bytes(original.to_vec()));
        sources.insert(
            "src/long.py".to_owned(),
            SourceRead::Bytes(b"old\n".to_vec()),
        );
        let changes = vec![
            FileChange {
                path: "src/a.py".to_owned(),
                original_digest: dx_digest::blake3(original),
                edits: vec![(0, original.len() as u64, "line one\nline TWO\n".to_owned())],
            },
            FileChange {
                path: "src/long.py".to_owned(),
                original_digest: dx_digest::blake3(b"old\n"),
                edits: vec![(0, 4, long_body.clone())],
            },
        ];
        let first = render_diff_patch(&sources, &changes).expect("render patch");
        let second = render_diff_patch(&sources, &changes).expect("render patch");
        assert_eq!(first, second);
        assert!(first.contains("--- a/src/a.py"));
        assert!(first.contains("+++ b/src/a.py"));
        assert!(first.contains("line TWO"));
        assert!(first.contains(&"x".repeat(10_000)));
    }

    #[test]
    fn patch_errors_stay_typed_with_stable_display() {
        assert_eq!(
            PatchError::MissingSource {
                path: "src/a.py".to_owned(),
            }
            .to_string(),
            "cannot render patch without verified source for src/a.py"
        );
        assert_eq!(
            PatchError::NonUtf8Source {
                path: "src/a.py".to_owned(),
            }
            .to_string(),
            "source for src/a.py is not UTF-8 text"
        );
        assert_eq!(
            PatchError::Unappliable {
                path: "src/a.py".to_owned(),
            }
            .to_string(),
            "cannot apply recorded edits for src/a.py"
        );
        assert_eq!(
            PatchError::NonUtf8Candidate {
                path: "src/a.py".to_owned(),
            }
            .to_string(),
            "candidate for src/a.py is not UTF-8 text"
        );
        assert_eq!(
            PatchError::Render {
                detail: "boom".to_owned(),
            }
            .to_string(),
            "failed to render patch: boom"
        );
        let sources = BTreeMap::new();
        let changes = vec![FileChange {
            path: "src/missing.py".to_owned(),
            original_digest: dx_digest::blake3(b"x"),
            edits: vec![],
        }];
        assert_eq!(
            render_diff_patch(&sources, &changes).expect_err("missing source"),
            PatchError::MissingSource {
                path: "src/missing.py".to_owned(),
            }
        );
    }
}
