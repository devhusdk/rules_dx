#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

pub mod outputs;
pub mod remote;
pub mod test_outputs;

pub use outputs::{collect, collect_with_workspace};
pub use remote::{Downloader, LocalDownloader, RemoteConfig, UNWIRED_REMOTE_FLAGS};
pub use test_outputs::{collect_test_outputs, collect_test_outputs_with_workspace, TestOutputFile};

use std::path::{Path, PathBuf};

pub trait ArtifactReader {
    fn read_artifact(&self, path: &Path) -> std::io::Result<Vec<u8>>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CollectorConfig {
    output_group: String,
}

impl CollectorConfig {
    pub fn new(output_group: &str) -> Result<Self, BepError> {
        if output_group.is_empty() {
            return Err(BepError::EmptyOutputGroup);
        }
        Ok(CollectorConfig {
            output_group: output_group.to_owned(),
        })
    }

    pub fn output_group(&self) -> &str {
        &self.output_group
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CollectedArtifact {
    pub exec_path: PathBuf,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetOutput {
    pub label: String,
    pub success: bool,
    pub artifacts: Vec<CollectedArtifact>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BepError {
    #[error("empty output group: want a non-empty output group name")]
    EmptyOutputGroup,
    #[error("malformed event at line {line}: {reason}")]
    MalformedEvent { line: u64, reason: String },
    #[error("unsupported artifact URI {uri:?}: want a local file:// URI")]
    UnsupportedUri { uri: String },
    #[error("missing named set {id:?} referenced at line {line}")]
    MissingNamedSet { id: String, line: u64 },
    #[error("unreadable artifact {path:?}: {message}")]
    UnreadableArtifact { path: String, message: String },
}

pub(crate) fn malformed(line: u64, path: &str, detail: &str) -> BepError {
    BepError::MalformedEvent {
        line,
        reason: format!("{path}: {detail}"),
    }
}

pub(crate) fn file_uri_to_path(uri: &str) -> Result<PathBuf, BepError> {
    let unsupported = || BepError::UnsupportedUri {
        uri: uri.to_owned(),
    };
    let parsed = url::Url::parse(uri).map_err(|_| unsupported())?;
    if parsed.scheme() != "file" {
        return Err(unsupported());
    }
    parsed.to_file_path().map_err(|_| unsupported())
}

pub fn is_bytestream_uri(uri: &str) -> bool {
    uri.starts_with("bytestream://")
}

const VERBATIM_PREFIX: &str = r"\\?\";

fn strip_verbatim_prefix(path: &Path) -> PathBuf {
    let text = path.to_string_lossy();
    match text.strip_prefix(VERBATIM_PREFIX) {
        Some(rest) => PathBuf::from(rest),
        None => path.to_path_buf(),
    }
}

pub fn local_path_for_bep_file(workspace: &Path, path_prefix: &[String], name: &str) -> PathBuf {
    let mut path = strip_verbatim_prefix(workspace);
    for part in path_prefix {
        path.push(part);
    }
    path.push(name);
    path
}

fn test_output_filename(name: &str) -> &str {
    if name == "test.lcov" {
        "coverage.dat"
    } else {
        name
    }
}

pub fn testlog_path_for_label(workspace: &Path, label: &str, name: &str) -> Option<PathBuf> {
    if label.starts_with('@') {
        return None;
    }
    let rest = label.strip_prefix("//")?;
    let (package, target) = match rest.split_once(':') {
        Some((pkg, tgt)) => (pkg, tgt),
        None => {
            let base = rest.rsplit('/').next().unwrap_or(rest);
            (rest, base)
        }
    };
    if target.is_empty() {
        return None;
    }
    let mut path = strip_verbatim_prefix(workspace);
    path.push("bazel-testlogs");
    if !package.is_empty() {
        path.push(package);
    }
    path.push(target);
    path.push(test_output_filename(name));
    Some(path)
}

pub fn is_shard_artifact(path: &Path, suffix: &str) -> bool {
    path.as_os_str()
        .as_encoded_bytes()
        .ends_with(suffix.as_bytes())
}

pub fn exec_matches(artifact_path: &Path, exec_path: &str) -> bool {
    if exec_path.is_empty() {
        return false;
    }
    let rendered = artifact_path.as_os_str().to_string_lossy();
    rendered.as_ref() == exec_path || rendered.ends_with(&format!("/{exec_path}"))
}

pub fn suffix_matches(artifact: &str, exec_path: &str) -> bool {
    artifact == exec_path || artifact.ends_with(&format!("/{exec_path}"))
}

pub fn non_shard_artifact_paths(outputs: &[TargetOutput], suffix: &str) -> Vec<String> {
    let mut paths = Vec::new();
    for output in outputs {
        for artifact in &output.artifacts {
            if !is_shard_artifact(&artifact.exec_path, suffix) {
                paths.push(artifact.exec_path.display().to_string());
            }
        }
    }
    paths
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shard_helpers_split_on_suffix_and_component_boundaries() {
        assert!(is_shard_artifact(Path::new("/out/a.dxenv.pb"), ".dxenv.pb"));
        assert!(!is_shard_artifact(Path::new("/out/a.pb"), ".dxenv.pb"));
        assert!(!is_shard_artifact(
            Path::new("/out/a.dxenv.pb.txt"),
            ".dxenv.pb"
        ));
        assert!(exec_matches(Path::new("/out/rustc"), "rustc"));
        assert!(!exec_matches(Path::new("/out/xrustc"), "rustc"));
        assert!(!exec_matches(Path::new("/out/rustc"), ""));
        assert!(suffix_matches("bazel-out/bin/rustc", "bin/rustc"));
        assert!(!suffix_matches("bazel-out/xbin/rustc", "bin/rustc"));
    }

    #[test]
    fn file_uri_forms() {
        assert_eq!(
            file_uri_to_path("file:///out/a.pb").expect("abs"),
            PathBuf::from("/out/a.pb")
        );
        assert_eq!(
            file_uri_to_path("file://localhost/out/a.pb").expect("localhost"),
            PathBuf::from("/out/a.pb")
        );
        assert_eq!(
            file_uri_to_path("file:///out/a%20b.pb").expect("space"),
            PathBuf::from("/out/a b.pb")
        );
        assert_eq!(
            file_uri_to_path("file://localhost/out/a%20b.pb").expect("localhost space"),
            PathBuf::from("/out/a b.pb")
        );
        assert_eq!(
            file_uri_to_path("file:///C:/out/a.pb").expect("drive"),
            PathBuf::from(if cfg!(windows) {
                "C:\\out\\a.pb"
            } else {
                "/C:/out/a.pb"
            })
        );
        for uri in [
            "bytestream://x",
            "bytestream://remote/cache/a.pb",
            "/plain/path",
        ] {
            assert!(
                matches!(file_uri_to_path(uri), Err(BepError::UnsupportedUri { .. })),
                "{uri} must fail as UnsupportedUri"
            );
        }
        if cfg!(windows) {
            assert!(file_uri_to_path("file://otherhost/out/a.pb").is_ok());
        } else {
            for uri in ["file://relative/path", "file://otherhost/out/a.pb"] {
                assert!(
                    matches!(file_uri_to_path(uri), Err(BepError::UnsupportedUri { .. })),
                    "{uri} must fail as UnsupportedUri"
                );
            }
        }
    }

    #[test]
    fn verbatim_windows_workspaces_extend_normally() {
        let ws = Path::new(r"\\?\D:\a\rules_dx\rules_dx");
        let base = PathBuf::from(r"D:\a\rules_dx\rules_dx");
        assert_eq!(
            testlog_path_for_label(ws, "//csharp/x:hello", "test.lcov").expect("path"),
            base.join("bazel-testlogs")
                .join("csharp")
                .join("x")
                .join("hello")
                .join("coverage.dat")
        );
        assert_eq!(
            local_path_for_bep_file(ws, &["bazel-out".to_owned()], "a.pb"),
            base.join("bazel-out").join("a.pb")
        );
    }

    #[test]
    fn bytestream_fallback_paths_resolve_through_workspace_symlinks() {
        assert!(is_bytestream_uri(
            "bytestream://remote.buildbuddy.io/blobs/abc/269"
        ));
        assert!(!is_bytestream_uri("file:///out/a.pb"));
        let ws = Path::new("/ws");
        assert_eq!(
            local_path_for_bep_file(
                ws,
                &[
                    "bazel-out".to_owned(),
                    "k8-fastbuild".to_owned(),
                    "bin".to_owned()
                ],
                "docs/site/a.md"
            ),
            PathBuf::from("/ws/bazel-out/k8-fastbuild/bin/docs/site/a.md")
        );
        assert_eq!(
            testlog_path_for_label(ws, "//cli/bep:dx_bep_test", "test.xml").expect("path"),
            PathBuf::from("/ws/bazel-testlogs/cli/bep/dx_bep_test/test.xml")
        );
        assert_eq!(
            testlog_path_for_label(ws, "//cli/bep:dx_bep_test", "test.lcov").expect("lcov"),
            PathBuf::from("/ws/bazel-testlogs/cli/bep/dx_bep_test/coverage.dat")
        );
        assert_eq!(
            testlog_path_for_label(ws, "//:preset_parity_test", "test.xml").expect("root"),
            PathBuf::from("/ws/bazel-testlogs/preset_parity_test/test.xml")
        );
        assert!(testlog_path_for_label(ws, "@ext//pkg:t", "test.xml").is_none());
        assert_eq!(
            testlog_path_for_label(ws, "//cli/bep", "test.xml").expect("implicit"),
            PathBuf::from("/ws/bazel-testlogs/cli/bep/bep/test.xml")
        );
        assert!(testlog_path_for_label(ws, "//pkg:", "test.xml").is_none());
        assert!(testlog_path_for_label(ws, "pkg/t", "test.xml").is_none());
    }
}
