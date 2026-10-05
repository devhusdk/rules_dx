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
pub mod test_outputs;

pub use outputs::{collect, collect_with_workspace};
pub use test_outputs::{collect_test_outputs, TestOutputFile};

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
    #[error("missing test output {name:?} for {label} (run {run}, shard {shard}, attempt {attempt}): {reason}")]
    MissingTestOutput {
        label: String,
        name: String,
        run: u32,
        shard: u32,
        attempt: u32,
        reason: String,
    },
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

/// Output roots for one workspace, from `bazel info` when it answered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputLocations {
    workspace: PathBuf,
    execution_root: Option<PathBuf>,
    testlogs: Option<PathBuf>,
}

impl OutputLocations {
    /// Locations for a workspace before any `bazel info` output is applied.
    pub fn new(workspace: &Path) -> Self {
        OutputLocations {
            workspace: workspace.to_path_buf(),
            execution_root: None,
            testlogs: None,
        }
    }

    /// Applies `bazel info` key and value lines to these locations.
    pub fn apply_bazel_info(&mut self, output: &[u8]) {
        let text = String::from_utf8_lossy(output);
        for line in text.lines() {
            let Some((key, value)) = line.split_once(": ") else {
                continue;
            };
            if value.is_empty() {
                continue;
            }
            match key {
                "bazel-testlogs" => self.testlogs = Some(PathBuf::from(value)),
                "execution_root" => self.execution_root = Some(PathBuf::from(value)),
                _ => {}
            }
        }
    }

    /// The testlogs root: the reported one when available, else the workspace symlink.
    fn testlogs_root(&self) -> PathBuf {
        match &self.testlogs {
            Some(root) => root.clone(),
            None => strip_verbatim_prefix(&self.workspace).join("bazel-testlogs"),
        }
    }

    fn execution_root(&self) -> Option<&Path> {
        self.execution_root.as_deref()
    }
}

/// One test result's indexes beside the totals reported with them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TestOutputIdentity {
    pub run: u32,
    pub shard: u32,
    pub attempt: u32,
    pub run_total: u32,
    pub shard_total: u32,
    pub attempt_total: u32,
}

/// Splits a test label into its repository, package, and target parts.
fn testlogs_label_parts(label: &str) -> Option<(Option<&str>, &str, &str)> {
    let (repo, rest) = if let Some(rest) = label.strip_prefix("//") {
        (None, rest)
    } else if let Some(rest) = label.strip_prefix("@@") {
        let (repo, rest) = rest.split_once("//")?;
        (Some(repo).filter(|repo| !repo.is_empty()), rest)
    } else {
        let rest = label.strip_prefix('@')?;
        let (repo, rest) = rest.split_once("//")?;
        (Some(repo), rest)
    };
    let (package, target) = match rest.split_once(':') {
        Some((package, target)) => (package, target),
        None => {
            let base = rest.rsplit('/').next().unwrap_or(rest);
            (rest, base)
        }
    };
    if target.is_empty() {
        return None;
    }
    Some((repo, package, target))
}

fn missing_test_output(
    label: &str,
    name: &str,
    identity: TestOutputIdentity,
    reason: &str,
) -> BepError {
    BepError::MissingTestOutput {
        label: label.to_owned(),
        name: name.to_owned(),
        run: identity.run,
        shard: identity.shard,
        attempt: identity.attempt,
        reason: reason.to_owned(),
    }
}

/// Resolves one named test output under the testlogs root for its label and identity.
pub fn testlog_path(
    locations: &OutputLocations,
    label: &str,
    name: &str,
    identity: TestOutputIdentity,
) -> Result<PathBuf, BepError> {
    let (repo, package, target) = testlogs_label_parts(label).ok_or_else(|| {
        missing_test_output(
            label,
            name,
            identity,
            "label does not address a testlogs directory",
        )
    })?;
    let mut path = locations.testlogs_root();
    if let Some(repo) = repo {
        path.push("external");
        path.push(repo);
    }
    if !package.is_empty() {
        path.push(package);
    }
    path.push(target);
    let mut dirs = Vec::new();
    if identity.shard_total > 1 {
        dirs.push(format!(
            "shard_{}_of_{}",
            identity.shard, identity.shard_total
        ));
    }
    if identity.run_total > 1 {
        dirs.push(format!("run_{}_of_{}", identity.run, identity.run_total));
    }
    if !dirs.is_empty() {
        path.push(dirs.join("_"));
    }
    let file_name = test_output_filename(name);
    if identity.attempt < identity.attempt_total {
        path.push("test_attempts");
        path.push(file_name.replacen("test", &format!("attempt_{}", identity.attempt), 1));
    } else {
        path.push(file_name);
    }
    Ok(path)
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
    suffix_matches(&rendered, exec_path)
}

/// Whether `exec_path` names `artifact` outright or as a whole trailing path suffix.
pub fn suffix_matches(artifact: &str, exec_path: &str) -> bool {
    if artifact == exec_path {
        return true;
    }
    artifact
        .strip_suffix(exec_path)
        .is_some_and(|head| head.ends_with('/'))
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

    /// A file URI naming one path under a root this host can resolve.
    fn out_uri(rel: &str) -> String {
        let root = if cfg!(windows) { "C:/out" } else { "/out" };
        dx_path::uri(&std::path::Path::new(root).join(rel))
    }

    /// One run, one shard, one attempt: the layout Bazel uses without sharding or retries.
    const IDENTITY: TestOutputIdentity = TestOutputIdentity {
        run: 1,
        shard: 1,
        attempt: 1,
        run_total: 1,
        shard_total: 1,
        attempt_total: 1,
    };

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
            file_uri_to_path(&out_uri("a.pb")).expect("abs"),
            PathBuf::from("/out/a.pb")
        );
        assert_eq!(
            file_uri_to_path("file://localhost/out/a.pb").expect("localhost"),
            PathBuf::from("/out/a.pb")
        );
        assert_eq!(
            file_uri_to_path(&out_uri("a%20b.pb")).expect("space"),
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
        let locations = OutputLocations::new(ws);
        assert_eq!(
            testlog_path(&locations, "//csharp/x:hello", "test.lcov", IDENTITY).expect("path"),
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
    fn testlogs_paths_cover_the_identity_layout() {
        let locations = OutputLocations::new(Path::new("/ws"));
        assert_eq!(
            testlog_path(&locations, "//a:t", "test.xml", IDENTITY).expect("plain"),
            PathBuf::from("/ws/bazel-testlogs/a/t/test.xml")
        );
        let sharded = TestOutputIdentity {
            shard: 2,
            shard_total: 3,
            ..IDENTITY
        };
        assert_eq!(
            testlog_path(&locations, "//a:t", "test.xml", sharded).expect("shard"),
            PathBuf::from("/ws/bazel-testlogs/a/t/shard_2_of_3/test.xml")
        );
        let repeated = TestOutputIdentity {
            run: 2,
            run_total: 4,
            ..IDENTITY
        };
        assert_eq!(
            testlog_path(&locations, "//a:t", "test.xml", repeated).expect("run"),
            PathBuf::from("/ws/bazel-testlogs/a/t/run_2_of_4/test.xml")
        );
        let both = TestOutputIdentity {
            run: 2,
            shard: 3,
            run_total: 4,
            shard_total: 3,
            ..IDENTITY
        };
        assert_eq!(
            testlog_path(&locations, "//a:t", "test.xml", both).expect("both"),
            PathBuf::from("/ws/bazel-testlogs/a/t/shard_3_of_3_run_2_of_4/test.xml")
        );
        let retried = TestOutputIdentity {
            attempt: 1,
            attempt_total: 2,
            ..IDENTITY
        };
        assert_eq!(
            testlog_path(&locations, "//a:t", "test.xml", retried).expect("attempt"),
            PathBuf::from("/ws/bazel-testlogs/a/t/test_attempts/attempt_1.xml")
        );
        assert_eq!(
            testlog_path(&locations, "//a:t", "test.lcov", retried).expect("attempt lcov"),
            PathBuf::from("/ws/bazel-testlogs/a/t/test_attempts/coverage.dat")
        );
        assert_eq!(
            testlog_path(&locations, "//a:t", "test.xml", both).expect("final attempt"),
            PathBuf::from("/ws/bazel-testlogs/a/t/shard_3_of_3_run_2_of_4/test.xml")
        );
    }

    #[test]
    fn testlogs_paths_cover_labels_and_roots() {
        let locations = OutputLocations::new(Path::new("/ws"));
        assert_eq!(
            testlog_path(&locations, "//:preset_parity_test", "test.xml", IDENTITY).expect("root"),
            PathBuf::from("/ws/bazel-testlogs/preset_parity_test/test.xml")
        );
        assert_eq!(
            testlog_path(&locations, "//cli/bep", "test.xml", IDENTITY).expect("implicit"),
            PathBuf::from("/ws/bazel-testlogs/cli/bep/bep/test.xml")
        );
        assert_eq!(
            testlog_path(&locations, "@@dep+//pkg:t", "test.xml", IDENTITY).expect("external"),
            PathBuf::from("/ws/bazel-testlogs/external/dep+/pkg/t/test.xml")
        );
        assert_eq!(
            testlog_path(&locations, "@dep//pkg:t", "test.xml", IDENTITY).expect("apparent"),
            PathBuf::from("/ws/bazel-testlogs/external/dep/pkg/t/test.xml")
        );
        assert_eq!(
            testlog_path(&locations, "@@//pkg:t", "test.xml", IDENTITY).expect("canonical main"),
            PathBuf::from("/ws/bazel-testlogs/pkg/t/test.xml")
        );
        for label in ["//pkg:", "pkg/t", "not-a-label"] {
            let err = testlog_path(&locations, label, "test.xml", IDENTITY).expect_err("must fail");
            let BepError::MissingTestOutput {
                label: got,
                run,
                shard,
                attempt,
                ..
            } = err
            else {
                panic!("want MissingTestOutput, got {err:?}");
            };
            assert_eq!(got, label);
            assert_eq!((run, shard, attempt), (1, 1, 1));
        }
        let mut info = OutputLocations::new(Path::new("/ws"));
        info.apply_bazel_info(
            b"bazel-testlogs: /out/k8-fastbuild/testlogs\nexecution_root: /out/execroot\n",
        );
        assert_eq!(
            testlog_path(&info, "//a:t", "test.xml", IDENTITY).expect("reported root"),
            PathBuf::from("/out/k8-fastbuild/testlogs/a/t/test.xml")
        );
    }

    #[test]
    fn bazel_info_lines_fill_only_the_known_roots() {
        let mut locations = OutputLocations::new(Path::new("/ws"));
        locations.apply_bazel_info(
            b"bazel-testlogs: /out/testlogs\nexecution_root: /out/execroot\nunrelated: /x\nno_key\nexecution_root: \n",
        );
        assert_eq!(
            locations.testlogs_root(),
            PathBuf::from("/out/testlogs"),
            "bazel-testlogs wins over the workspace symlink"
        );
        assert_eq!(
            locations.execution_root(),
            Some(Path::new("/out/execroot")),
            "execution_root parses and an empty value is ignored"
        );
        locations.apply_bazel_info(b"");
        assert_eq!(
            locations.testlogs_root(),
            PathBuf::from("/out/testlogs"),
            "an empty info output changes nothing"
        );
        let plain = OutputLocations::new(Path::new("/ws"));
        assert_eq!(
            plain.testlogs_root(),
            PathBuf::from("/ws/bazel-testlogs"),
            "without info the workspace symlink is the fallback"
        );
        assert_eq!(plain.execution_root(), None);
    }

    #[test]
    fn bytestream_fallback_paths_resolve_through_workspace_symlinks() {
        assert!(is_bytestream_uri(
            "bytestream://remote.buildbuddy.io/blobs/abc/269"
        ));
        assert!(!is_bytestream_uri(&out_uri("a.pb")));
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
        let locations = OutputLocations::new(ws);
        assert_eq!(
            testlog_path(&locations, "//cli/bep:dx_bep_test", "test.xml", IDENTITY).expect("path"),
            PathBuf::from("/ws/bazel-testlogs/cli/bep/dx_bep_test/test.xml")
        );
        assert_eq!(
            testlog_path(&locations, "//cli/bep:dx_bep_test", "test.lcov", IDENTITY).expect("lcov"),
            PathBuf::from("/ws/bazel-testlogs/cli/bep/dx_bep_test/coverage.dat")
        );
    }
}
