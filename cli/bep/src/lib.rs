#![deny(clippy::indexing_slicing, clippy::string_slice)]

pub mod cache_errors;
pub mod failures;
pub mod ledger;
pub mod log_tail;
pub mod metrics;
pub mod outputs;
pub mod test_outputs;

pub use cache_errors::{CacheError, cache_error, stream_cache_error};
pub use failures::{
    BuildFailure, BuildFailures, FailedTargetKind, FailureStage, StderrRef, collect_build_failures,
};
pub use ledger::{EventGap, EventLedger};
pub use log_tail::log_tail;
pub use metrics::{
    ActionSummary, BuildTotals, MnemonicTime, collect_action_summary, collect_build_totals,
    collect_bytes_received,
};
pub use outputs::{OutputAccount, OutputEvents, collect_account, read_output_events};
pub use test_outputs::{
    TestEvents, TestResultFile, TestResultRecord, TestSummaryRecord, UnbuiltTest,
    collect_test_events,
};

use dx_path::label::{Label, Repo};
use std::collections::BTreeSet;
use std::io::BufRead;
use std::path::{Path, PathBuf};

pub trait ArtifactReader {
    fn read_artifact(&self, path: &Path) -> std::io::Result<Vec<u8>>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CollectorConfig {
    output_group: String,
    foreign: Foreign,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Foreign {
    pub aspects: BTreeSet<String>,
    pub labels: BTreeSet<String>,
}

impl Foreign {
    #[must_use]
    pub fn covers(&self, label: Option<&str>, aspect: Option<&str>) -> bool {
        aspect.is_some_and(|aspect| self.aspects.contains(aspect_key(aspect)))
            || label
                .and_then(target_key)
                .is_some_and(|key| self.labels.contains(&key))
    }
}

impl CollectorConfig {
    pub fn new(output_group: &str) -> Result<Self, BepError> {
        if output_group.is_empty() {
            return Err(BepError::EmptyOutputGroup);
        }
        Ok(CollectorConfig {
            output_group: output_group.to_owned(),
            foreign: Foreign::default(),
        })
    }

    #[must_use]
    pub fn output_group(&self) -> &str {
        &self.output_group
    }

    #[must_use]
    pub fn without(mut self, foreign: Foreign) -> Self {
        self.foreign = foreign;
        self
    }

    #[must_use]
    pub fn is_foreign(&self, label: Option<&str>, aspect: Option<&str>) -> bool {
        self.foreign.covers(label, aspect)
    }
}

#[must_use]
pub fn target_key(label: &str) -> Option<String> {
    let parsed = Label::parse(label).filter(|parsed| !parsed.relative)?;
    let main = matches!(
        parsed.repo,
        Repo::Local | Repo::Canonical("") | Repo::Apparent("")
    );
    let pattern = parsed.package.split('/').any(|part| part == "...")
        || matches!(parsed.name, "all" | "*" | "all-targets" | "...");
    (main && !pattern && label.contains("//"))
        .then(|| format!("//{}:{}", parsed.package, parsed.name))
}

#[must_use]
pub fn aspect_key(aspect: &str) -> &str {
    let named = aspect.split_once("//").map_or(aspect, |(_, rest)| rest);
    named.split_once('[').map_or(named, |(name, _)| name)
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
    #[error("unsupported artifact URI: {0}")]
    BadFileUri(dx_path::UriError),
    #[error("missing named set {id:?} referenced at line {line}")]
    MissingNamedSet { id: String, line: u64 },
    #[error("unreadable artifact {path:?}: {message}")]
    UnreadableArtifact { path: String, message: String },
    #[error(
        "missing test output {name:?} for {label} (run {run}, shard {shard}, attempt {attempt}): {reason}"
    )]
    MissingTestOutput {
        label: String,
        name: String,
        run: u32,
        shard: u32,
        attempt: u32,
        reason: String,
    },
    #[error("non-UTF-8 artifact path {path:?}: want a UTF-8 exec path")]
    NonUtf8ArtifactPath { path: String },
}

pub(crate) fn malformed(line: u64, path: &str, detail: &str) -> BepError {
    BepError::MalformedEvent {
        line,
        reason: format!("{path}: {detail}"),
    }
}

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
                Err((blank, reason)) => {
                    if !blank && self.tail.is_none() {
                        self.tail = Some((line_no, reason));
                    }
                }
            }
        }
    }
}

pub(crate) fn readable_lines(reader: impl BufRead) -> impl Iterator<Item = String> {
    reader
        .lines()
        .map_while(|line| match line {
            Err(error) if error.kind() != std::io::ErrorKind::InvalidData => None,
            other => Some(other.ok()),
        })
        .flatten()
}

fn visit_events(
    path: &Path,
    mut visit: impl FnMut(&serde_json::Value) -> bool,
) -> std::io::Result<()> {
    let reader = std::io::BufReader::new(std::fs::File::open(path)?);
    for line in reader.lines() {
        let line = match line {
            Ok(line) => line,
            Err(error) if error.kind() == std::io::ErrorKind::InvalidData => continue,
            Err(error) => return Err(error),
        };
        let Ok(event) = serde_json::from_str::<serde_json::Value>(&line) else {
            continue;
        };
        if visit(&event) {
            break;
        }
    }
    Ok(())
}

pub fn stream_started(path: &Path) -> std::io::Result<bool> {
    let mut started = false;
    visit_events(path, |event| {
        started = event
            .pointer("/id/started")
            .is_some_and(serde_json::Value::is_object)
            && event
                .get("started")
                .is_some_and(serde_json::Value::is_object);
        started
    })?;
    Ok(started)
}

pub fn stream_build_tool_version(path: &Path) -> std::io::Result<Option<String>> {
    let mut found = None;
    visit_events(path, |event| {
        if event
            .pointer("/id/started")
            .is_some_and(serde_json::Value::is_object)
        {
            found = event
                .pointer("/started/buildToolVersion")
                .and_then(serde_json::Value::as_str)
                .filter(|version| !version.is_empty())
                .map(str::to_owned);
        }
        found.is_some()
    })?;
    Ok(found)
}

pub(crate) fn is_plain_relative(value: &str) -> bool {
    !value.is_empty()
        && Path::new(value).components().all(|part| {
            matches!(
                part,
                std::path::Component::Normal(_) | std::path::Component::CurDir
            )
        })
}

pub(crate) fn file_uri_to_path(uri: &str) -> Result<PathBuf, BepError> {
    dx_path::uri_to_path(uri).map_err(BepError::BadFileUri)
}

#[must_use]
pub fn is_bytestream_uri(uri: &str) -> bool {
    uri.starts_with("bytestream://")
}

#[must_use]
pub fn local_path_for_bep_file(workspace: &Path, path_prefix: &[String], name: &str) -> PathBuf {
    let mut path = dx_path::without_verbatim(workspace);
    for part in path_prefix {
        path.push(part);
    }
    path.push(name);
    path
}

fn test_output_segments(name: &str) -> Vec<String> {
    match name {
        "test.lcov" => return vec!["coverage.dat".to_owned()],
        "test.stderr" => return vec!["test.err".to_owned()],
        _ => {}
    }
    if let Some(file) = name.strip_prefix("test.outputs__") {
        return vec!["test.outputs".to_owned(), file.to_owned()];
    }
    if let Some(file) = name.strip_prefix("test.outputs_manifest__") {
        return vec!["test.outputs_manifest".to_owned(), file.to_owned()];
    }
    vec![name.to_owned()]
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputLocations {
    workspace: PathBuf,
    execution_root: Option<PathBuf>,
    testlogs: Option<PathBuf>,
    output_path: Option<PathBuf>,
}

impl OutputLocations {
    #[must_use]
    pub fn new(workspace: &Path) -> Self {
        OutputLocations {
            workspace: workspace.to_path_buf(),
            execution_root: None,
            testlogs: None,
            output_path: None,
        }
    }

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

    fn testlogs_root(&self) -> PathBuf {
        match &self.testlogs {
            Some(root) => root.clone(),
            None => dx_path::without_verbatim(&self.workspace).join("bazel-testlogs"),
        }
    }

    fn config_testlogs_root(&self, mnemonic: &str) -> PathBuf {
        let mut root = match &self.output_path {
            Some(root) => root.clone(),
            None => dx_path::without_verbatim(&self.workspace).join("bazel-out"),
        };
        root.push(mnemonic);
        root.push("testlogs");
        root
    }

    pub(crate) fn prefix_testlogs_root(&self, prefix: &[String]) -> Option<PathBuf> {
        if prefix.is_empty() || !prefix.iter().all(|part| is_path_segment(part)) {
            return None;
        }
        let mut root = self.execution_root.clone()?;
        root.extend(prefix);
        Some(root)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TestOutputIdentity {
    pub run: u32,
    pub shard: u32,
    pub attempt: u32,
    pub run_total: u32,
    pub shard_total: u32,
    pub attempt_total: u32,
}

fn testlogs_label_parts(label: &str) -> Option<(Option<&str>, &str, &str)> {
    let parsed = Label::parse(label).filter(|parsed| !parsed.relative)?;
    let repo = match parsed.repo {
        Repo::Local => None,
        Repo::Canonical(repo) => Some(repo).filter(|repo| !repo.is_empty()),
        Repo::Apparent(repo) => Some(repo),
    };
    Some((repo, parsed.package, parsed.name))
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

fn is_path_segment(value: &str) -> bool {
    !value.is_empty() && value != "." && value != ".." && !value.contains(['/', '\\', '\0'])
}

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
    testlog_path_under(root, label, name, identity, false)
}

pub(crate) fn prefix_names_repo(prefix: &[String], label: &str) -> bool {
    match (testlogs_label_parts(label), prefix.last()) {
        (Some((Some(repo), _, _)), Some(last)) => last == repo,
        _ => false,
    }
}

pub(crate) fn testlog_path_under(
    root: PathBuf,
    label: &str,
    name: &str,
    identity: TestOutputIdentity,
    root_names_repo: bool,
) -> Result<PathBuf, BepError> {
    let (repo, package, target) = testlogs_label_parts(label).ok_or_else(|| {
        missing_test_output(
            label,
            name,
            identity,
            "label does not address a testlogs directory",
        )
    })?;
    if !package.split('/').all(is_path_segment) && !package.is_empty()
        || !is_plain_relative(target)
        || repo.is_some_and(|repo| !is_path_segment(repo))
    {
        return Err(missing_test_output(
            label,
            name,
            identity,
            "label does not name a path under the testlogs directory",
        ));
    }
    let mut path = root;
    if let (Some(repo), false) = (repo, root_names_repo) {
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
    let mut segments = test_output_segments(name);
    if let Some(bad) = segments.iter().find(|segment| !is_path_segment(segment)) {
        return Err(missing_test_output(
            label,
            name,
            identity,
            &format!("test output path segment `{bad}` is not a plain name"),
        ));
    }
    if identity.attempt < identity.attempt_total {
        path.push("test_attempts");
        if let Some(first) = segments.first_mut() {
            *first = first.replacen("test", &format!("attempt_{}", identity.attempt), 1);
        }
    }
    path.extend(segments);
    Ok(path)
}

#[must_use]
pub fn is_shard_artifact(path: &Path, suffix: &str) -> bool {
    path.as_os_str()
        .as_encoded_bytes()
        .ends_with(suffix.as_bytes())
}

#[must_use]
pub fn exec_matches(artifact_path: &Path, exec_path: &str) -> bool {
    if exec_path.is_empty() {
        return false;
    }
    bytes_suffix_matches(
        artifact_path.as_os_str().as_encoded_bytes(),
        exec_path.as_bytes(),
    )
}

#[must_use]
pub fn suffix_matches(artifact: &str, exec_path: &str) -> bool {
    bytes_suffix_matches(artifact.as_bytes(), exec_path.as_bytes())
}

fn bytes_suffix_matches(artifact: &[u8], exec_path: &[u8]) -> bool {
    if artifact == exec_path {
        return true;
    }
    artifact
        .strip_suffix(exec_path)
        .is_some_and(|head| head.ends_with(b"/"))
}

pub fn non_shard_artifact_paths(
    outputs: &[TargetOutput],
    suffix: &str,
) -> Result<Vec<String>, BepError> {
    let mut paths = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for output in outputs {
        for artifact in &output.artifacts {
            if !is_shard_artifact(&artifact.exec_path, suffix) {
                let path =
                    artifact
                        .exec_path
                        .to_str()
                        .ok_or_else(|| BepError::NonUtf8ArtifactPath {
                            path: artifact.exec_path.display().to_string(),
                        })?;
                if seen.insert(path) {
                    paths.push(path.to_owned());
                }
            }
        }
    }
    Ok(paths)
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

    #[test]
    fn non_shard_artifact_paths_skip_shard_files() {
        let artifact = |name: &str| CollectedArtifact {
            exec_path: PathBuf::from(name),
            bytes: Vec::new(),
        };
        let output = TargetOutput {
            label: "//a:t".to_owned(),
            aspect: None,
            configuration: None,
            success: true,
            declared: true,
            incomplete: false,
            artifacts: vec![artifact("out/a.shard.pb"), artifact("out/b.pb")],
        };
        assert_eq!(
            non_shard_artifact_paths(&[output], ".shard.pb").unwrap(),
            ["out/b.pb"]
        );
    }

    #[test]
    fn non_shard_artifact_paths_report_each_path_once() {
        let artifact = |name: &str| CollectedArtifact {
            exec_path: PathBuf::from(name),
            bytes: Vec::new(),
        };
        let output = |aspect: Option<&str>, names: &[&str]| TargetOutput {
            label: "//a:t".to_owned(),
            aspect: aspect.map(str::to_owned),
            configuration: None,
            success: true,
            declared: true,
            incomplete: false,
            artifacts: names.iter().map(|name| artifact(name)).collect(),
        };
        let outputs = [
            output(None, &["out/lib.rs", "out/b.pb"]),
            output(
                Some("core/env/private:plan.bzl%dx_env_plan_aspect"),
                &["out/lib.rs"],
            ),
        ];
        assert_eq!(
            non_shard_artifact_paths(&outputs, ".shard.pb").unwrap(),
            ["out/lib.rs", "out/b.pb"]
        );
    }

    #[cfg(unix)]
    fn non_utf8(prefix: &str, byte: u8, suffix: &str) -> PathBuf {
        use std::os::unix::ffi::OsStringExt;
        let mut bytes = prefix.as_bytes().to_vec();
        bytes.push(byte);
        bytes.extend_from_slice(suffix.as_bytes());
        PathBuf::from(std::ffi::OsString::from_vec(bytes))
    }

    #[cfg(unix)]
    #[test]
    fn non_shard_artifact_paths_reject_non_utf8_paths_by_name() {
        let artifact = |exec_path: PathBuf| CollectedArtifact {
            exec_path,
            bytes: Vec::new(),
        };
        let output = TargetOutput {
            label: "//a:t".to_owned(),
            aspect: None,
            configuration: None,
            success: true,
            declared: true,
            incomplete: false,
            artifacts: vec![
                artifact(non_utf8("out/", 0xff, "/lib.rs")),
                artifact(non_utf8("out/", 0xfe, "/lib.rs")),
            ],
        };
        let error = non_shard_artifact_paths(&[output], ".shard.pb").unwrap_err();
        assert_eq!(
            error,
            BepError::NonUtf8ArtifactPath {
                path: "out/\u{fffd}/lib.rs".to_owned(),
            }
        );
        assert!(error.to_string().contains("out/\u{fffd}/lib.rs"));
    }

    #[cfg(unix)]
    #[test]
    fn exec_matches_keeps_non_utf8_paths_distinct() {
        let artifact = non_utf8("out/", 0xff, "/lib.rs");
        assert!(exec_matches(&artifact, "lib.rs"));
        assert!(!exec_matches(&artifact, "\u{fffd}/lib.rs"));
        assert!(!exec_matches(&artifact, "out/\u{fffd}/lib.rs"));
        assert!(!exec_matches(&non_utf8("out/", 0xff, ""), "out/\u{fffd}"));
    }

    #[test]
    fn build_tool_version_comes_from_the_started_event() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("bep.json");
        std::fs::write(
            &path,
            "garbage\n{\"id\":{\"started\":{}},\"started\":{\"buildToolVersion\":\"9.0.0\"}}\n",
        )
        .expect("write");
        assert_eq!(
            stream_build_tool_version(&path).unwrap().as_deref(),
            Some("9.0.0")
        );
        std::fs::write(
            &path,
            "{\"id\":{\"started\":{}},\"started\":{}}\n{\"id\":{\"progress\":{}}}\n",
        )
        .expect("write");
        assert_eq!(stream_build_tool_version(&path).unwrap(), None);
        assert!(stream_build_tool_version(Path::new("/nonexistent/dx/bep.json")).is_err());
    }

    fn on_windows<T>(windows: T, other: T) -> T {
        if cfg!(windows) { windows } else { other }
    }

    fn out_uri(rel: &str) -> String {
        let root = if cfg!(windows) { "C:/out" } else { "out" };
        format!("file:///{root}/{rel}")
    }

    fn localhost_uri(rel: &str) -> String {
        let root = if cfg!(windows) { "C:/out" } else { "out" };
        format!("file://localhost/{root}/{rel}")
    }

    fn want_path(root: &str, parts: &[&str]) -> PathBuf {
        let mut path = PathBuf::from(root);
        for part in parts {
            path.push(part);
        }
        path
    }

    fn out_path(rel: &str) -> PathBuf {
        let root = if cfg!(windows) { r"C:\out" } else { "/out" };
        want_path(root, &rel.split('/').collect::<Vec<_>>())
    }

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
        assert!(suffix_matches("bin/rustc", "bin/rustc"));
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
        let drive_path = on_windows(r"C:\out\a.pb", "/C:/out/a.pb");
        assert_eq!(
            file_uri_to_path("file:///C:/out/a.pb").expect("drive"),
            PathBuf::from(drive_path)
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
                matches!(file_uri_to_path(uri), Err(BepError::BadFileUri(_))),
                "{uri} must fail as BadFileUri"
            );
        }
        let unc_host = on_windows(Some(r"\\otherhost\out\a.pb"), None);
        let unc_share = on_windows(Some(r"\\srv\share\a#b.pb"), None);
        assert_eq!(
            file_uri_to_path("file://otherhost/out/a.pb").ok(),
            unc_host.map(PathBuf::from)
        );
        assert_eq!(
            file_uri_to_path("file://srv/share/a%23b.pb").ok(),
            unc_share.map(PathBuf::from)
        );
        let unc_relative = on_windows(Some(r"\\relative\path"), None);
        assert_eq!(
            file_uri_to_path("file://relative/path").ok(),
            unc_relative.map(PathBuf::from)
        );
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
    fn verbatim_share_workspaces_keep_their_share() {
        let ws = Path::new(r"\\?\UNC\h\s\ws");
        assert_eq!(
            testlog_path(
                &OutputLocations::new(ws),
                None,
                "//x:t",
                "test.lcov",
                IDENTITY
            )
            .expect("path"),
            PathBuf::from(r"\\h\s\ws")
                .join("bazel-testlogs")
                .join("x")
                .join("t")
                .join("coverage.dat")
        );
        assert_eq!(
            local_path_for_bep_file(ws, &["bazel-out".to_owned()], "a.pb"),
            PathBuf::from(r"\\h\s\ws").join("bazel-out").join("a.pb")
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
        let zip = "test.outputs__outputs.zip";
        assert_eq!(
            testlog_path(&locations, None, "//a:t", zip, IDENTITY).expect("zip"),
            want_path(
                "/ws",
                &["bazel-testlogs", "a", "t", "test.outputs", "outputs.zip"]
            )
        );
        assert_eq!(
            testlog_path(&locations, None, "//a:t", zip, retried).expect("attempt zip"),
            want_path(
                "/ws",
                &[
                    "bazel-testlogs",
                    "a",
                    "t",
                    "test_attempts",
                    "attempt_1.outputs",
                    "outputs.zip"
                ]
            )
        );
        assert_eq!(
            testlog_path(
                &locations,
                None,
                "//a:t",
                "test.outputs_manifest__MANIFEST",
                IDENTITY
            )
            .expect("manifest"),
            want_path(
                "/ws",
                &[
                    "bazel-testlogs",
                    "a",
                    "t",
                    "test.outputs_manifest",
                    "MANIFEST"
                ]
            )
        );
        assert_eq!(
            testlog_path(&locations, None, "//a:t", "test.stderr", IDENTITY).expect("stderr"),
            want_path("/ws", &["bazel-testlogs", "a", "t", "test.err"])
        );
        assert!(testlog_path(&locations, None, "//a:t", "../x", IDENTITY).is_err());
        assert!(testlog_path(&locations, None, "//a:t", "test.outputs__../x", IDENTITY).is_err());
    }

    #[test]
    fn testlogs_paths_cover_labels_and_roots() {
        let locations = OutputLocations::new(Path::new("/ws"));
        assert_eq!(
            testlog_path(
                &locations,
                None,
                "//core/quality:preset_parity_test",
                "test.xml",
                IDENTITY
            )
            .expect("root"),
            want_path(
                "/ws",
                &[
                    "bazel-testlogs",
                    "core/quality",
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
            // LCOV_EXCL_START - reason: failure arm of a test assertion, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
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
            // LCOV_EXCL_STOP - reason: end failure arm of a test assertion, issue: 8104, policy: docs/cli/commands/build-test-coverage.md
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

    #[test]
    fn stream_readers_report_a_read_error_instead_of_spinning() {
        let dir = tempfile::tempdir().expect("tempdir");
        if let Ok(started) = stream_started(dir.path()) {
            assert!(!started, "a directory holds no started event");
        }
        if let Ok(version) = stream_build_tool_version(dir.path()) {
            assert_eq!(version, None);
        }
    }

    #[test]
    fn stream_readers_skip_a_non_utf8_line() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("bep.json");
        let mut bytes = b"\xff\xfe\n".to_vec();
        bytes.extend_from_slice(
            b"{\"id\":{\"started\":{}},\"started\":{\"buildToolVersion\":\"9.0.0\"}}\n",
        );
        std::fs::write(&path, bytes).expect("write");
        assert!(stream_started(&path).expect("read"));
        assert_eq!(
            stream_build_tool_version(&path).expect("read").as_deref(),
            Some("9.0.0")
        );
    }

    #[test]
    fn testlog_path_rejects_a_label_package_that_leaves_the_root() {
        let locations = OutputLocations::new(Path::new("/ws"));
        for label in ["//a/../..:t", "//../x:t", "//a/./b:t", "//a:../t"] {
            assert!(
                testlog_path(&locations, None, label, "test.xml", IDENTITY).is_err(),
                "{label}"
            );
        }
    }
}
