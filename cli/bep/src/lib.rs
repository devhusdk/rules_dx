#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

pub mod failures;
pub mod ledger;
pub mod metrics;
pub mod outputs;
pub mod test_outputs;

pub use failures::{
    collect_build_failures, BuildFailure, BuildFailures, FailedTargetKind, FailureStage, StderrRef,
};
pub use ledger::{EventGap, EventLedger};
pub use metrics::{collect_action_summary, ActionSummary};
pub use outputs::{collect_account, OutputAccount};
pub use test_outputs::{
    collect_test_events, TestEvents, TestResultFile, TestResultRecord, TestSummaryRecord,
    UnbuiltTest,
};

use std::io::BufRead;
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
    pub aspect: Option<String>,
    pub configuration: Option<String>,
    pub success: bool,
    pub declared: bool,
    pub incomplete: bool,
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

/// Reads stream lines as JSON events, skipping garbled lines and keeping the first as the tail.
pub(crate) struct EventLines<R: BufRead> {
    lines: std::iter::Enumerate<std::io::Lines<R>>,
    pub(crate) tail: Option<(u64, String)>,
}

impl<R: BufRead> EventLines<R> {
    pub(crate) fn new(reader: R) -> Self {
        EventLines {
            lines: reader.lines().enumerate(),
            tail: None,
        }
    }
}

impl<R: BufRead> Iterator for EventLines<R> {
    type Item = Result<(u64, serde_json::Value), BepError>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let (index, line) = self.lines.next()?;
            let line_no = (index + 1) as u64;
            let parsed = match line {
                Ok(text) => serde_json::from_str::<serde_json::Value>(&text)
                    .map_err(|e| (text.trim().is_empty(), format!("invalid JSON: {e}"))),
                // Invalid UTF-8 spans one line; any other read error would repeat forever.
                Err(e) if e.kind() == std::io::ErrorKind::InvalidData => {
                    Err((false, format!("unreadable stream line: {e}")))
                }
                Err(e) => {
                    return Some(Err(BepError::MalformedEvent {
                        line: line_no,
                        reason: format!("unreadable stream line: {e}"),
                    }));
                }
            };
            match parsed {
                Ok(event) => return Some(Ok((line_no, event))),
                // A garbled line is skipped, not fatal: the first one is kept as the tail so
                // the caller reports the stream as damaged while keeping every other event.
                Err((blank, reason)) => {
                    if !blank && self.tail.is_none() {
                        self.tail = Some((line_no, reason));
                    }
                }
            }
        }
    }
}

/// Reports whether the stream at `path` holds a real `started` event; a synthetic aborted one does not count.
///
/// A missing file is an `io::ErrorKind::NotFound` error. Unparseable lines are skipped.
pub fn stream_started(path: &Path) -> std::io::Result<bool> {
    let reader = std::io::BufReader::new(std::fs::File::open(path)?);
    for line in reader.lines() {
        let Ok(line) = line else { continue };
        let Ok(event) = serde_json::from_str::<serde_json::Value>(&line) else {
            continue;
        };
        if event
            .pointer("/id/started")
            .is_some_and(|id| id.is_object())
            && event.get("started").is_some_and(|body| body.is_object())
        {
            return Ok(true);
        }
    }
    Ok(false)
}

pub(crate) fn file_uri_to_path(uri: &str) -> Result<PathBuf, BepError> {
    dx_path::uri_to_path(uri).map_err(|_| BepError::UnsupportedUri {
        uri: uri.to_owned(),
    })
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
    output_path: Option<PathBuf>,
}

impl OutputLocations {
    /// Locations for a workspace before any `bazel info` output is applied.
    pub fn new(workspace: &Path) -> Self {
        OutputLocations {
            workspace: workspace.to_path_buf(),
            execution_root: None,
            testlogs: None,
            output_path: None,
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
                "output_path" => self.output_path = Some(PathBuf::from(value)),
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

    /// The testlogs root of one configuration's output directory.
    fn config_testlogs_root(&self, mnemonic: &str) -> PathBuf {
        let mut root = match &self.output_path {
            Some(root) => root.clone(),
            None => strip_verbatim_prefix(&self.workspace).join("bazel-out"),
        };
        root.push(mnemonic);
        root.push("testlogs");
        root
    }

    /// The testlogs root a BEP path prefix names under the execution root.
    pub(crate) fn prefix_testlogs_root(&self, prefix: &[String]) -> Option<PathBuf> {
        if prefix.is_empty() || !prefix.iter().all(|part| is_path_segment(part)) {
            return None;
        }
        let mut root = self.execution_root.clone()?;
        root.extend(prefix);
        Some(root)
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

/// Whether `value` is one plain path segment.
fn is_path_segment(value: &str) -> bool {
    !value.is_empty() && value != "." && value != ".." && !value.contains(['/', '\\', '\0'])
}

/// Resolves one named test output under the testlogs root for its label and identity.
pub fn testlog_path(
    locations: &OutputLocations,
    mnemonic: Option<&str>,
    label: &str,
    name: &str,
    identity: TestOutputIdentity,
) -> Result<PathBuf, BepError> {
    let root = match mnemonic.filter(|m| is_path_segment(m)) {
        Some(mnemonic) => locations.config_testlogs_root(mnemonic),
        None => locations.testlogs_root(),
    };
    testlog_path_under(root, label, name, identity)
}

/// Resolves one named test output under `root` for its label and identity.
pub(crate) fn testlog_path_under(
    root: PathBuf,
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
    let mut path = root;
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

    fn started_in(text: &str) -> std::io::Result<bool> {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("bep.json");
        std::fs::write(&path, text).expect("write");
        stream_started(&path)
    }

    #[test]
    fn stream_started_reads_only_real_started_events() {
        let missing = stream_started(Path::new("/nonexistent/dx/bep.json")).unwrap_err();
        assert_eq!(missing.kind(), std::io::ErrorKind::NotFound);
        assert!(!started_in("").unwrap());
        assert!(
            !started_in("{\"id\":{\"started\":{}},\"aborted\":{\"reason\":\"INTERNAL\"}}\n")
                .unwrap()
        );
        assert!(started_in("{\"id\":{\"started\":{}},\"started\":{\"uuid\":\"x\"}}\n").unwrap());
        assert!(!started_in("not json\n{\"id\":{\"started\":{}},\"star").unwrap());
        assert!(started_in("garbage\n{\"id\":{\"started\":{}},\"started\":{}}\n").unwrap());
    }

    /// A hand-written file URI naming one path under a root this host can resolve.
    fn out_uri(rel: &str) -> String {
        if cfg!(windows) {
            format!("file:///C:/out/{rel}")
        } else {
            format!("file:///out/{rel}")
        }
    }

    /// A hand-written file URI naming the fixture root with a localhost authority.
    fn localhost_uri(rel: &str) -> String {
        if cfg!(windows) {
            format!("file://localhost/C:/out/{rel}")
        } else {
            format!("file://localhost/out/{rel}")
        }
    }

    /// The host path one fixture root plus its path parts name.
    fn want_path(root: &str, parts: &[&str]) -> PathBuf {
        let mut path = PathBuf::from(root);
        for part in parts {
            path.push(part);
        }
        path
    }

    /// The host path one slash-separated relative name under the fixture root names.
    fn out_path(rel: &str) -> PathBuf {
        let root = if cfg!(windows) { r"C:\out" } else { "/out" };
        want_path(root, &rel.split('/').collect::<Vec<_>>())
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
            out_path("a.pb")
        );
        assert_eq!(
            file_uri_to_path(&localhost_uri("a.pb")).expect("localhost"),
            out_path("a.pb")
        );
        assert_eq!(
            file_uri_to_path(&out_uri("a%20b.pb")).expect("space"),
            out_path("a b.pb")
        );
        assert_eq!(
            file_uri_to_path(&localhost_uri("a%20b.pb")).expect("localhost space"),
            out_path("a b.pb")
        );
        assert_eq!(
            file_uri_to_path(&out_uri("a%2520b.pb")).expect("literal percent"),
            out_path("a%20b.pb")
        );
        assert_eq!(
            file_uri_to_path("file:///C:/").expect("drive root"),
            PathBuf::from(if cfg!(windows) { r"C:\" } else { "/C:/" })
        );
        assert_eq!(
            file_uri_to_path("file:///C:/out/a.pb").expect("drive"),
            PathBuf::from(if cfg!(windows) {
                r"C:\out\a.pb"
            } else {
                "/C:/out/a.pb"
            })
        );
        for uri in [
            "bytestream://x",
            "bytestream://remote/cache/a.pb",
            "https://example.com/a.pb",
            "/plain/path",
            "file:a.pb",
            "file:///out/a.pb?x=1",
            "file:///out/a.pb#frag",
        ] {
            assert!(
                matches!(file_uri_to_path(uri), Err(BepError::UnsupportedUri { .. })),
                "{uri} must fail as UnsupportedUri"
            );
        }
        if cfg!(windows) {
            assert_eq!(
                file_uri_to_path("file://otherhost/out/a.pb").expect("unc host"),
                PathBuf::from(r"\\otherhost\out\a.pb")
            );
            assert_eq!(
                file_uri_to_path("file://srv/share/a%23b.pb").expect("unc share"),
                PathBuf::from(r"\\srv\share\a#b.pb")
            );
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
            testlog_path(&locations, None, "//csharp/x:hello", "test.lcov", IDENTITY)
                .expect("path"),
            base.join("bazel-testlogs")
                .join("csharp/x")
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
            testlog_path(&locations, None, "//a:t", "test.xml", IDENTITY).expect("plain"),
            want_path("/ws", &["bazel-testlogs", "a", "t", "test.xml"])
        );
        let sharded = TestOutputIdentity {
            shard: 2,
            shard_total: 3,
            ..IDENTITY
        };
        assert_eq!(
            testlog_path(&locations, None, "//a:t", "test.xml", sharded).expect("shard"),
            want_path(
                "/ws",
                &["bazel-testlogs", "a", "t", "shard_2_of_3", "test.xml"]
            )
        );
        let repeated = TestOutputIdentity {
            run: 2,
            run_total: 4,
            ..IDENTITY
        };
        assert_eq!(
            testlog_path(&locations, None, "//a:t", "test.xml", repeated).expect("run"),
            want_path(
                "/ws",
                &["bazel-testlogs", "a", "t", "run_2_of_4", "test.xml"]
            )
        );
        let both = TestOutputIdentity {
            run: 2,
            shard: 3,
            run_total: 4,
            shard_total: 3,
            ..IDENTITY
        };
        assert_eq!(
            testlog_path(&locations, None, "//a:t", "test.xml", both).expect("both"),
            want_path(
                "/ws",
                &[
                    "bazel-testlogs",
                    "a",
                    "t",
                    "shard_3_of_3_run_2_of_4",
                    "test.xml"
                ]
            )
        );
        let retried = TestOutputIdentity {
            attempt: 1,
            attempt_total: 2,
            ..IDENTITY
        };
        assert_eq!(
            testlog_path(&locations, None, "//a:t", "test.xml", retried).expect("attempt"),
            want_path(
                "/ws",
                &["bazel-testlogs", "a", "t", "test_attempts", "attempt_1.xml"]
            )
        );
        assert_eq!(
            testlog_path(&locations, None, "//a:t", "test.lcov", retried).expect("attempt lcov"),
            want_path(
                "/ws",
                &["bazel-testlogs", "a", "t", "test_attempts", "coverage.dat"]
            )
        );
        assert_eq!(
            testlog_path(&locations, None, "//a:t", "test.xml", both).expect("final attempt"),
            want_path(
                "/ws",
                &[
                    "bazel-testlogs",
                    "a",
                    "t",
                    "shard_3_of_3_run_2_of_4",
                    "test.xml"
                ]
            )
        );
    }

    #[test]
    fn testlogs_paths_cover_labels_and_roots() {
        let locations = OutputLocations::new(Path::new("/ws"));
        assert_eq!(
            testlog_path(
                &locations,
                None,
                "//quality:preset_parity_test",
                "test.xml",
                IDENTITY
            )
            .expect("root"),
            want_path(
                "/ws",
                &[
                    "bazel-testlogs",
                    "quality",
                    "preset_parity_test",
                    "test.xml"
                ]
            )
        );
        assert_eq!(
            testlog_path(&locations, None, "//cli/bep", "test.xml", IDENTITY).expect("implicit"),
            want_path("/ws", &["bazel-testlogs", "cli/bep", "bep", "test.xml"])
        );
        assert_eq!(
            testlog_path(&locations, None, "@@dep+//pkg:t", "test.xml", IDENTITY)
                .expect("external"),
            want_path(
                "/ws",
                &["bazel-testlogs", "external", "dep+", "pkg", "t", "test.xml"]
            )
        );
        assert_eq!(
            testlog_path(&locations, None, "@dep//pkg:t", "test.xml", IDENTITY).expect("apparent"),
            want_path(
                "/ws",
                &["bazel-testlogs", "external", "dep", "pkg", "t", "test.xml"]
            )
        );
        assert_eq!(
            testlog_path(&locations, None, "@@//pkg:t", "test.xml", IDENTITY)
                .expect("canonical main"),
            want_path("/ws", &["bazel-testlogs", "pkg", "t", "test.xml"])
        );
        for label in ["//pkg:", "pkg/t", "not-a-label"] {
            let err =
                testlog_path(&locations, None, label, "test.xml", IDENTITY).expect_err("must fail");
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
            testlog_path(&info, None, "//a:t", "test.xml", IDENTITY).expect("reported root"),
            want_path("/out/k8-fastbuild/testlogs", &["a", "t", "test.xml"])
        );
    }

    #[test]
    fn testlogs_paths_use_the_configuration_mnemonic() {
        let mut info = OutputLocations::new(Path::new("/ws"));
        info.apply_bazel_info(b"bazel-testlogs: /out/k8-fastbuild/testlogs\noutput_path: /out\n");
        let mnemonic = Some("k8-fastbuild-ST-2c5af747e1f8");
        assert_eq!(
            testlog_path(&info, mnemonic, "//a:t", "test.xml", IDENTITY).expect("info"),
            want_path(
                "/out",
                &[
                    "k8-fastbuild-ST-2c5af747e1f8",
                    "testlogs",
                    "a",
                    "t",
                    "test.xml"
                ]
            )
        );
        let plain = OutputLocations::new(Path::new("/ws"));
        assert_eq!(
            testlog_path(&plain, mnemonic, "//a:t", "test.xml", IDENTITY).expect("no info"),
            want_path(
                "/ws",
                &[
                    "bazel-out",
                    "k8-fastbuild-ST-2c5af747e1f8",
                    "testlogs",
                    "a",
                    "t",
                    "test.xml"
                ]
            )
        );
        for unsafe_mnemonic in ["", ".", "..", "a/b", "a\\b", "a\0b"] {
            assert_eq!(
                testlog_path(&info, Some(unsafe_mnemonic), "//a:t", "test.xml", IDENTITY)
                    .expect("unsafe"),
                want_path("/out/k8-fastbuild/testlogs", &["a", "t", "test.xml"]),
                "{unsafe_mnemonic:?} falls back to bazel-testlogs"
            );
        }
        let layout = TestOutputIdentity {
            run: 2,
            shard: 3,
            attempt: 1,
            run_total: 4,
            shard_total: 3,
            attempt_total: 2,
        };
        assert_eq!(
            testlog_path(&info, mnemonic, "@@dep+//pkg:t", "test.xml", layout).expect("layout"),
            want_path(
                "/out",
                &[
                    "k8-fastbuild-ST-2c5af747e1f8",
                    "testlogs",
                    "external",
                    "dep+",
                    "pkg",
                    "t",
                    "shard_3_of_3_run_2_of_4",
                    "test_attempts",
                    "attempt_1.xml"
                ]
            )
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
            locations.prefix_testlogs_root(&["bazel-out".to_owned()]),
            Some(PathBuf::from("/out/execroot/bazel-out")),
            "execution_root parses and an empty value is ignored"
        );
        assert_eq!(locations.prefix_testlogs_root(&[]), None);
        locations.apply_bazel_info(b"");
        assert_eq!(
            locations.testlogs_root(),
            PathBuf::from("/out/testlogs"),
            "an empty info output changes nothing"
        );
        let plain = OutputLocations::new(Path::new("/ws"));
        assert_eq!(
            plain.testlogs_root(),
            want_path("/ws", &["bazel-testlogs"]),
            "without info the workspace symlink is the fallback"
        );
        assert_eq!(plain.prefix_testlogs_root(&["bazel-out".to_owned()]), None);
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
                "tools/docs/site/a.md"
            ),
            want_path(
                "/ws",
                &["bazel-out", "k8-fastbuild", "bin", "tools/docs/site/a.md"]
            )
        );
        let locations = OutputLocations::new(ws);
        assert_eq!(
            testlog_path(
                &locations,
                None,
                "//cli/bep:dx_bep_test",
                "test.xml",
                IDENTITY
            )
            .expect("path"),
            want_path(
                "/ws",
                &["bazel-testlogs", "cli/bep", "dx_bep_test", "test.xml"]
            )
        );
        assert_eq!(
            testlog_path(
                &locations,
                None,
                "//cli/bep:dx_bep_test",
                "test.lcov",
                IDENTITY
            )
            .expect("lcov"),
            want_path(
                "/ws",
                &["bazel-testlogs", "cli/bep", "dx_bep_test", "coverage.dat"]
            )
        );
    }
}
