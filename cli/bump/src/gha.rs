use super::sets::BumpSet;
use super::version::{self, WidenVersion};

pub const WORKFLOW_DIR: &str = ".github/workflows";

pub fn upstream_client() -> &'static str {
    "GitHub releases"
}

pub fn workflow_file(name: &str) -> bool {
    name.ends_with(".yml") || name.ends_with(".yaml")
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TagSnapshot {
    pub package: String,
    pub tag: String,
    pub sha: String,
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum GhaError {
    #[error("empty GHA tag entry; expected `owner/repo` plus tag plus SHA")]
    Empty,
    #[error("invalid package {package:?} for github-actions: expected owner/repo")]
    InvalidPackage { package: String },
    #[error("invalid tag {tag:?} for {package:?}: expected a Git tag (e.g. v4)")]
    InvalidTag { package: String, tag: String },
    #[error("invalid SHA {sha:?} for {package:?} tag {tag:?}: expected a 40/64-char commit SHA")]
    InvalidSha {
        package: String,
        tag: String,
        sha: String,
    },
    #[error("unknown tag {tag:?} for {package:?}: no upstream GitHub releases snapshot (nothing widened; never invent a SHA)")]
    UnknownTag { package: String, tag: String },
}

pub fn parse_snapshot(package: &str, tag: &str, sha: &str) -> Result<TagSnapshot, GhaError> {
    if package.is_empty() || tag.is_empty() || sha.is_empty() {
        return Err(GhaError::Empty);
    }
    if !is_owner_repo(package) {
        return Err(GhaError::InvalidPackage {
            package: package.to_owned(),
        });
    }
    match version::parse(BumpSet::GithubActions, tag) {
        Ok(WidenVersion::GitTag(_)) => {}
        _ => {
            return Err(GhaError::InvalidTag {
                package: package.to_owned(),
                tag: tag.to_owned(),
            });
        }
    }
    match version::parse(BumpSet::GithubActions, sha) {
        Ok(WidenVersion::GitCommit(_)) => {}
        _ => {
            return Err(GhaError::InvalidSha {
                package: package.to_owned(),
                tag: tag.to_owned(),
                sha: sha.to_owned(),
            });
        }
    }
    Ok(TagSnapshot {
        package: package.to_owned(),
        tag: tag.to_owned(),
        sha: sha.to_owned(),
    })
}

pub fn resolve_tag(
    package: &str,
    tag: &str,
    snapshots: &[TagSnapshot],
) -> Result<String, GhaError> {
    if package.is_empty() || tag.is_empty() {
        return Err(GhaError::Empty);
    }
    if !is_owner_repo(package) {
        return Err(GhaError::InvalidPackage {
            package: package.to_owned(),
        });
    }
    match version::parse(BumpSet::GithubActions, tag) {
        Ok(WidenVersion::GitTag(_)) => {}
        _ => {
            return Err(GhaError::InvalidTag {
                package: package.to_owned(),
                tag: tag.to_owned(),
            });
        }
    }
    for snapshot in snapshots {
        if snapshot.package == package && snapshot.tag == tag {
            match version::parse(BumpSet::GithubActions, &snapshot.sha) {
                Ok(WidenVersion::GitCommit(_)) => return Ok(snapshot.sha.clone()),
                _ => {
                    return Err(GhaError::InvalidSha {
                        package: package.to_owned(),
                        tag: tag.to_owned(),
                        sha: snapshot.sha.clone(),
                    });
                }
            }
        }
    }
    Err(GhaError::UnknownTag {
        package: package.to_owned(),
        tag: tag.to_owned(),
    })
}

fn is_owner_repo(package: &str) -> bool {
    let (owner, repo) = match package.split_once('/') {
        Some((owner, repo)) => (owner, repo),
        None => return false,
    };
    if owner.is_empty()
        || repo.is_empty()
        || repo.contains('/')
        || repo.contains(' ')
        || owner.contains(' ')
    {
        return false;
    }
    is_dotted(owner) && is_dotted(repo)
}

fn is_dotted(text: &str) -> bool {
    !text.is_empty()
        && text
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot() -> Vec<TagSnapshot> {
        vec![
            parse_snapshot(
                "actions/checkout",
                "v5",
                "3d3c42e5aac5ba805825da76410c181273ba90b1",
            )
            .expect("snapshot"),
            parse_snapshot(
                "actions/cache",
                "v4",
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            )
            .expect("snapshot"),
        ]
    }

    #[test]
    fn upstream_client_is_github_releases() {
        assert_eq!(upstream_client(), "GitHub releases");
        assert_eq!(
            super::super::discovery::registry_client(BumpSet::GithubActions),
            "GitHub releases"
        );
    }

    #[test]
    fn tag_auto_resolves_to_sha_from_upstream_snapshot() {
        let snapshots = snapshot();
        assert_eq!(
            resolve_tag("actions/checkout", "v5", &snapshots).expect("resolve"),
            "3d3c42e5aac5ba805825da76410c181273ba90b1"
        );
        assert_eq!(
            resolve_tag("actions/cache", "v4", &snapshots).expect("resolve"),
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
        );
    }

    #[test]
    fn unknown_tag_fails_closed_without_inventing_sha() {
        let snapshots = snapshot();
        assert!(matches!(
            resolve_tag("actions/checkout", "v9", &snapshots),
            Err(GhaError::UnknownTag { .. })
        ));
        assert!(matches!(
            resolve_tag("actions/unknown", "v5", &snapshots),
            Err(GhaError::UnknownTag { .. })
        ));
        let empty: Vec<TagSnapshot> = Vec::new();
        assert!(matches!(
            resolve_tag("actions/checkout", "v5", &empty),
            Err(GhaError::UnknownTag { .. })
        ));
    }

    #[test]
    fn malformed_entries_fail_closed() {
        let snapshots = snapshot();
        assert!(matches!(
            resolve_tag("", "v5", &snapshots),
            Err(GhaError::Empty)
        ));
        assert!(matches!(
            resolve_tag("actions/checkout", "", &snapshots),
            Err(GhaError::Empty)
        ));
        assert!(matches!(
            parse_snapshot("", "v5", "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
            Err(GhaError::Empty)
        ));
        assert!(matches!(
            resolve_tag("not-an-action", "v5", &snapshots),
            Err(GhaError::InvalidPackage { .. })
        ));
        assert!(matches!(
            parse_snapshot(
                "not-an-action",
                "v5",
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
            ),
            Err(GhaError::InvalidPackage { .. })
        ));
        assert!(matches!(
            resolve_tag(
                "actions/checkout",
                "3d3c42e5aac5ba805825da76410c181273ba90b1",
                &snapshots
            ),
            Err(GhaError::InvalidTag { .. })
        ));
        assert!(matches!(
            resolve_tag("actions/checkout", "bad tag", &snapshots),
            Err(GhaError::InvalidTag { .. })
        ));
    }

    #[test]
    fn invalid_snapshot_sha_fails_closed() {
        assert!(matches!(
            parse_snapshot("actions/checkout", "v5", "not-a-sha!!!"),
            Err(GhaError::InvalidSha { .. })
        ));
        let corrupt = vec![TagSnapshot {
            package: "actions/checkout".to_owned(),
            tag: "v5".to_owned(),
            sha: "not-a-sha".to_owned(),
        }];
        assert!(matches!(
            resolve_tag("actions/checkout", "v5", &corrupt),
            Err(GhaError::InvalidSha { .. })
        ));
    }

    #[test]
    fn snapshot_shapes_validate_through_upstream_parse() {
        assert!(matches!(
            version::parse(BumpSet::GithubActions, "v5"),
            Ok(WidenVersion::GitTag(_))
        ));
        assert!(matches!(
            version::parse(
                BumpSet::GithubActions,
                "3d3c42e5aac5ba805825da76410c181273ba90b1"
            ),
            Ok(WidenVersion::GitCommit(_))
        ));
    }
}
