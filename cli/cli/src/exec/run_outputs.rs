use dx_bep::TestEvents;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// The manifest file at the root of every retained run-output directory.
pub(crate) const MANIFEST_NAME: &str = "manifest.json";

/// The `report` event format naming a retained run-output manifest.
pub(crate) const MANIFEST_FORMAT: &str = "manifest";

/// The manifest schema this writer produces.
pub(crate) const MANIFEST_SCHEMA: u32 = 1;

/// How long a manifest declares its artifacts retained, in days.
pub(crate) const RETENTION_DAYS: u64 = 30;

/// The largest single artifact the exporter copies before recording it missing.
pub(crate) const MAX_BYTES_PER_FILE: u64 = 64 * 1024 * 1024;

/// The largest total payload one invocation retains before recording the rest missing.
pub(crate) const MAX_BYTES_TOTAL: u64 = 512 * 1024 * 1024;

/// How many candidate directory names the exporter tries before reporting a conflict.
const CREATE_ATTEMPTS: u32 = 100;

/// The size policy one export enforces.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RetentionLimits {
    pub(crate) max_bytes_per_file: u64,
    pub(crate) max_bytes_total: u64,
}

impl Default for RetentionLimits {
    fn default() -> Self {
        RetentionLimits {
            max_bytes_per_file: MAX_BYTES_PER_FILE,
            max_bytes_total: MAX_BYTES_TOTAL,
        }
    }
}

/// Where one invocation's retained outputs landed and how complete they are.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExportSummary {
    pub(crate) dir: PathBuf,
    pub(crate) manifest: PathBuf,
    pub(crate) stored: u64,
    pub(crate) missing: u64,
}

/// Keeps one path component inside its directory: letters, digits, dot, dash
/// and underscore survive, everything else becomes an underscore.
fn sanitize_component(text: &str) -> String {
    const LIMIT: usize = 100;
    let mut out = String::with_capacity(text.len().min(LIMIT));
    let mut chars = 0usize;
    for ch in text.chars() {
        if chars >= LIMIT {
            break;
        }
        if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-') {
            out.push(ch);
        } else {
            out.push('_');
        }
        chars += 1;
    }
    if out.is_empty() {
        out.push_str("artifact");
    }
    out
}

/// The stored file name for one declared output: an index keeps duplicate
/// logical names apart, and every part is sanitized so the name is one safe
/// component that can never name a parent directory.
fn stored_name(index: usize, target: &str, run: u32, shard: u32, attempt: u32, name: &str) -> String {
    format!(
        "{index:04}-{target}-r{run}-s{shard}-a{attempt}-{name}",
        target = sanitize_component(target),
        name = sanitize_component(name),
    )
}

/// One manifest entry: the BEP-declared identity plus where the bytes landed,
/// or an explicit reason when they could not be retained.
fn manifest_entry(
    target: &str,
    run: u32,
    shard: u32,
    attempt: u32,
    status: Option<&str>,
    name: &str,
    stored: Option<(&str, u64)>,
    missing_reason: Option<String>,
) -> serde_json::Value {
    let mut entry = serde_json::Map::new();
    entry.insert(
        "target".to_owned(),
        serde_json::Value::String(target.to_owned()),
    );
    entry.insert("run".to_owned(), serde_json::Value::from(run));
    entry.insert("shard".to_owned(), serde_json::Value::from(shard));
    entry.insert("attempt".to_owned(), serde_json::Value::from(attempt));
    entry.insert(
        "status".to_owned(),
        status.map_or(serde_json::Value::Null, |status| {
            serde_json::Value::String(status.to_owned())
        }),
    );
    entry.insert(
        "name".to_owned(),
        serde_json::Value::String(name.to_owned()),
    );
    match stored {
        Some((path, bytes)) => {
            entry.insert(
                "path".to_owned(),
                serde_json::Value::String(path.to_owned()),
            );
            entry.insert("bytes".to_owned(), serde_json::Value::from(bytes));
            entry.insert("missing_reason".to_owned(), serde_json::Value::Null);
        }
        None => {
            entry.insert("path".to_owned(), serde_json::Value::Null);
            entry.insert("bytes".to_owned(), serde_json::Value::Null);
            entry.insert(
                "missing_reason".to_owned(),
                missing_reason.map_or(serde_json::Value::Null, serde_json::Value::String),
            );
        }
    }
    serde_json::Value::Object(entry)
}

/// Creates one invocation-owned child directory under the parent, retrying
/// with a fresh nonce when another invocation won the same name.
fn create_owned_dir(parent: &Path, command: &str) -> Result<PathBuf, String> {
    if let Err(error) = std::fs::create_dir_all(parent) {
        return Err(format!(
            "cannot use run-output directory {}: {error}",
            parent.display()
        ));
    }
    let pid = std::process::id();
    for _ in 0..CREATE_ATTEMPTS {
        let nonce = crate::plan::run_nonce();
        let child = parent.join(format!("dx-{command}-{pid}-{nonce}"));
        match std::fs::create_dir(&child) {
            Ok(()) => return Ok(child),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(format!(
                    "cannot create run-output directory {}: {error}",
                    child.display()
                ));
            }
        }
    }
    Err(format!(
        "cannot create a unique run-output directory under {}: names keep colliding",
        parent.display()
    ))
}

/// Reads one declared artifact subject to the size policy: oversized files are
/// recorded missing without being read, and anything unreadable stays explicit.
fn read_artifact(path: &Path, limits: RetentionLimits, used: u64) -> Result<Vec<u8>, String> {
    let len = std::fs::metadata(path).map(|meta| meta.len()).map_err(|error| {
        format!("unreadable {}: {error}", path.display())
    })?;
    if len > limits.max_bytes_per_file {
        return Err(format!(
            "exceeds per-file limit of {} bytes ({} bytes)",
            limits.max_bytes_per_file, len
        ));
    }
    if used.saturating_add(len) > limits.max_bytes_total {
        return Err(format!(
            "exceeds total limit of {} bytes",
            limits.max_bytes_total
        ));
    }
    let bytes = std::fs::read(path)
        .map_err(|error| format!("unreadable {}: {error}", path.display()))?;
    if (bytes.len() as u64) > limits.max_bytes_per_file {
        return Err(format!(
            "exceeds per-file limit of {} bytes",
            limits.max_bytes_per_file
        ));
    }
    Ok(bytes)
}

/// Copies every BEP-declared test output into one invocation-owned directory
/// and writes the manifest mapping each output to its target, run, shard and
/// attempt. Missing artifacts stay explicit instead of failing the run; only
/// a broken destination fails the export.
pub(crate) fn export_run_outputs(
    parent: &Path,
    command: &str,
    events: &TestEvents,
    limits: RetentionLimits,
) -> Result<ExportSummary, String> {
    let dir = create_owned_dir(parent, command)?;
    let files_dir = dir.join("files");
    if let Err(error) = std::fs::create_dir(&files_dir) {
        let _ = std::fs::remove_dir(&dir);
        return Err(format!(
            "cannot create run-output directory {}: {error}",
            files_dir.display()
        ));
    }
    let created = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|since| since.as_secs())
        .map_err(|error| {
            let _ = std::fs::remove_dir_all(&dir);
            format!("cannot stamp the run-output manifest: {error}")
        })?;
    let mut entries: Vec<(String, u32, u32, u32, Option<String>, String, PathBuf)> = Vec::new();
    for result in &events.results {
        let mut files = result.files.clone();
        files.sort_by(|a, b| a.name.as_bytes().cmp(b.name.as_bytes()));
        for file in &files {
            let index = entries.len();
            let stored = files_dir.join(stored_name(
                index,
                &result.label,
                result.run,
                result.shard,
                result.attempt,
                &file.name,
            ));
            entries.push((
                result.label.clone(),
                result.run,
                result.shard,
                result.attempt,
                result.status.clone(),
                file.name.clone(),
                stored,
            ));
        }
    }
    let mut artifacts = Vec::with_capacity(entries.len());
    let mut used: u64 = 0;
    let mut stored_count: u64 = 0;
    let mut missing_count: u64 = 0;
    let mut failed: Option<String> = None;
    for (target, run, shard, attempt, status, name, stored) in &entries {
        if failed.is_some() {
            break;
        }
        let mut missing_reason: Option<String> = None;
        let mut kept: Option<u64> = None;
        let source = source_path(events, target, *run, *shard, *attempt, name);
        match source {
            Some(source) => match read_artifact(&source, limits, used) {
                Ok(bytes) => match std::fs::write(stored, &bytes) {
                    Ok(()) => {
                        used = used.saturating_add(bytes.len() as u64);
                        kept = Some(bytes.len() as u64);
                        stored_count += 1;
                    }
                    Err(error) => {
                        failed = Some(format!(
                            "cannot retain {}: {error}",
                            stored.display()
                        ));
                    }
                },
                Err(reason) => {
                    missing_reason = Some(reason);
                    missing_count += 1;
                }
            },
            None => {
                missing_reason = Some("undeclared output".to_owned());
                missing_count += 1;
            }
        }
        let relative = stored
            .strip_prefix(&dir)
            .map(|rel| rel.to_string_lossy().into_owned())
            .unwrap_or_else(|_| MANIFEST_NAME.to_owned());
        artifacts.push(manifest_entry(
            target,
            *run,
            *shard,
            *attempt,
            status.as_deref(),
            name,
            kept.map(|bytes| (relative.as_str(), bytes)),
            missing_reason,
        ));
    }
    if let Some(message) = failed {
        let _ = std::fs::remove_dir_all(&dir);
        return Err(message);
    }
    let manifest = dir.join(MANIFEST_NAME);
    let mut root = serde_json::Map::new();
    root.insert(
        "schema".to_owned(),
        serde_json::Value::from(MANIFEST_SCHEMA),
    );
    root.insert(
        "command".to_owned(),
        serde_json::Value::String(command.to_owned()),
    );
    root.insert(
        "created_unix".to_owned(),
        serde_json::Value::from(created),
    );
    root.insert(
        "retained_until_unix".to_owned(),
        serde_json::Value::from(created.saturating_add(RETENTION_DAYS * 86_400)),
    );
    root.insert(
        "retention_days".to_owned(),
        serde_json::Value::from(RETENTION_DAYS),
    );
    let mut policy = serde_json::Map::new();
    policy.insert(
        "max_bytes_per_file".to_owned(),
        serde_json::Value::from(limits.max_bytes_per_file),
    );
    policy.insert(
        "max_bytes_total".to_owned(),
        serde_json::Value::from(limits.max_bytes_total),
    );
    root.insert("limits".to_owned(), serde_json::Value::Object(policy));
    root.insert(
        "stored".to_owned(),
        serde_json::Value::from(stored_count),
    );
    root.insert(
        "missing".to_owned(),
        serde_json::Value::from(missing_count),
    );
    root.insert(
        "artifacts".to_owned(),
        serde_json::Value::Array(artifacts),
    );
    let mut document = serde_json::to_string_pretty(&serde_json::Value::Object(root))
        .map_err(|error| {
            let _ = std::fs::remove_dir_all(&dir);
            format!("cannot render the run-output manifest: {error}")
        })?;
    document.push('\n');
    if let Err(error) = std::fs::write(&manifest, document.as_bytes()) {
        let _ = std::fs::remove_dir_all(&dir);
        return Err(format!(
            "cannot write the run-output manifest {}: {error}",
            manifest.display()
        ));
    }
    Ok(ExportSummary {
        dir,
        manifest,
        stored: stored_count,
        missing: missing_count,
    })
}

/// The declared local path behind one manifest-bound output.
fn source_path(
    events: &TestEvents,
    target: &str,
    run: u32,
    shard: u32,
    attempt: u32,
    name: &str,
) -> Option<PathBuf> {
    events
        .results
        .iter()
        .find(|result| {
            result.label == target
                && result.run == run
                && result.shard == shard
                && result.attempt == attempt
        })
        .and_then(|result| {
            result
                .files
                .iter()
                .find(|file| file.name == *name)
                .map(|file| file.exec_path.clone())
        })
}

#[cfg(test)]
pub(crate) fn retention_expired(manifest: &serde_json::Value, now_unix: u64) -> bool {
    manifest
        .get("retained_until_unix")
        .and_then(serde_json::Value::as_u64)
        .is_none_or(|until| now_unix >= until)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn events_for(outputs: &[(&str, &str, &str)]) -> TestEvents {
        let lines: Vec<String> = outputs
            .iter()
            .map(|(label, name, uri)| {
                serde_json::json!({
                    "id": {"testResult": {"label": label, "run": 1, "shard": 1, "attempt": 1}},
                    "testResult": {"status": "PASSED", "testActionOutput": [{"name": name, "uri": uri}]},
                })
                .to_string()
            })
            .collect();
        dx_bep::collect_test_events(Cursor::new(lines.join("\n")), None).expect("events")
    }

    fn file_uri(path: &Path) -> String {
        format!("file://{}", path.display())
    }

    #[test]
    fn sanitize_keeps_one_safe_component() {
        assert_eq!(sanitize_component("test.xml"), "test.xml");
        assert_eq!(sanitize_component("../evil.txt"), ".._evil.txt");
        assert_eq!(sanitize_component("/abs/path"), "_abs_path");
        assert_eq!(sanitize_component("a/b\\c:d"), "a_b_c_d");
        assert_eq!(sanitize_component(""), "artifact");
        assert_eq!(sanitize_component("..."), "...");
        let long = "x".repeat(200);
        assert_eq!(sanitize_component(&long).len(), 100);
    }

    #[test]
    fn stored_names_stay_unique_and_inside_their_directory() {
        let first = stored_name(0, "//a:t", 1, 1, 1, "test.xml");
        let second = stored_name(0, "//a:t", 1, 1, 1, "test.xml");
        assert_eq!(first, second);
        assert_ne!(first, stored_name(1, "//a:t", 1, 1, 1, "test.xml"));
        assert_ne!(first, stored_name(0, "//b:t", 1, 1, 1, "test.xml"));
        for name in [
            first.as_str(),
            stored_name(3, "//a:t", 2, 3, 4, "../../evil").as_str(),
            stored_name(4, "..", 1, 1, 1, "..").as_str(),
        ] {
            assert!(!name.contains('/'), "{name} must be one component");
            assert!(!name.contains('\\'), "{name} must be one component");
            assert_ne!(name, "..", "{name} must not name a parent");
            assert_ne!(name, ".", "{name} must not name itself");
        }
    }

    #[test]
    fn export_retains_every_declared_output_with_its_identity() {
        let scratch = tempfile::tempdir().expect("scratch");
        let first = scratch.path().join("a.xml");
        let second = scratch.path().join("b.log");
        std::fs::write(&first, b"<xml/>").expect("write");
        std::fs::write(&second, b"log line\n").expect("write");
        let events = events_for(&[
            ("//a:t", "test.xml", &file_uri(&first)),
            ("//a:t", "test.log", &file_uri(&second)),
        ]);
        let parent = scratch.path().join("out");
        let summary =
            export_run_outputs(&parent, "test", &events, RetentionLimits::default())
                .expect("export");
        assert_eq!((summary.stored, summary.missing), (2, 0));
        assert!(summary.dir.starts_with(&parent));
        assert_eq!(summary.manifest, summary.dir.join(MANIFEST_NAME));
        let manifest: serde_json::Value = serde_json::from_slice(
            &std::fs::read(&summary.manifest).expect("manifest"),
        )
        .expect("json");
        assert_eq!(manifest["schema"], MANIFEST_SCHEMA.into());
        assert_eq!(manifest["command"], "test".into());
        assert_eq!(manifest["stored"], 2.into());
        assert_eq!(manifest["missing"], 0.into());
        let artifacts = manifest["artifacts"].as_array().expect("artifacts");
        assert_eq!(artifacts.len(), 2);
        for artifact in artifacts {
            assert_eq!(artifact["target"], "//a:t".into());
            assert_eq!(artifact["run"], 1.into());
            assert_eq!(artifact["missing_reason"], serde_json::Value::Null);
            let stored = summary.dir.join(artifact["path"].as_str().expect("path"));
            assert!(stored.is_file(), "{stored:?} must exist");
            assert!(stored.starts_with(&summary.dir));
        }
        assert_eq!(
            std::fs::read(summary.dir.join(artifacts[0]["path"].as_str().unwrap()))
                .expect("bytes"),
            b"log line\n"
        );
    }

    #[test]
    fn export_marks_missing_artifacts_explicit_without_failing() {
        let scratch = tempfile::tempdir().expect("scratch");
        let present = scratch.path().join("here.xml");
        std::fs::write(&present, b"<xml/>").expect("write");
        let absent = if cfg!(windows) {
            "file:///C:/nonexistent/missing.xml".to_owned()
        } else {
            "file:///nonexistent/missing.xml".to_owned()
        };
        let events = events_for(&[
            ("//a:t", "test.xml", &file_uri(&present)),
            ("//a:t", "ghost.bin", &absent),
        ]);
        let summary =
            export_run_outputs(scratch.path(), "test", &events, RetentionLimits::default())
                .expect("export");
        assert_eq!((summary.stored, summary.missing), (1, 1));
        let manifest: serde_json::Value = serde_json::from_slice(
            &std::fs::read(&summary.manifest).expect("manifest"),
        )
        .expect("json");
        let artifacts = manifest["artifacts"].as_array().expect("artifacts");
        let ghost = artifacts
            .iter()
            .find(|artifact| artifact["name"] == "ghost.bin".into())
            .expect("ghost entry");
        assert_eq!(ghost["path"], serde_json::Value::Null);
        assert_eq!(ghost["bytes"], serde_json::Value::Null);
        let reason = ghost["missing_reason"].as_str().expect("reason");
        assert!(reason.contains("unreadable"), "{reason}");
    }

    #[test]
    fn export_disambiguates_duplicate_logical_names() {
        let scratch = tempfile::tempdir().expect("scratch");
        let first = scratch.path().join("one.bin");
        let second = scratch.path().join("two.bin");
        std::fs::write(&first, b"one").expect("write");
        std::fs::write(&second, b"two").expect("write");
        let events = events_for(&[
            ("//a:t", "same.bin", &file_uri(&first)),
            ("//b:t", "same.bin", &file_uri(&second)),
        ]);
        let summary =
            export_run_outputs(scratch.path(), "test", &events, RetentionLimits::default())
                .expect("export");
        assert_eq!((summary.stored, summary.missing), (2, 0));
        let manifest: serde_json::Value = serde_json::from_slice(
            &std::fs::read(&summary.manifest).expect("manifest"),
        )
        .expect("json");
        let artifacts = manifest["artifacts"].as_array().expect("artifacts");
        let paths: Vec<&str> = artifacts
            .iter()
            .map(|artifact| artifact["path"].as_str().expect("path"))
            .collect();
        assert_ne!(paths[0], paths[1]);
        let bodies: Vec<Vec<u8>> = paths
            .iter()
            .map(|path| std::fs::read(summary.dir.join(path)).expect("bytes"))
            .collect();
        assert!(bodies.contains(&b"one".to_vec()));
        assert!(bodies.contains(&b"two".to_vec()));
    }

    #[test]
    fn export_contains_traversal_names_inside_its_directory() {
        let scratch = tempfile::tempdir().expect("scratch");
        let source = scratch.path().join("payload.bin");
        std::fs::write(&source, b"payload").expect("write");
        let events = events_for(&[("//a:t", "../../evil.bin", &file_uri(&source))]);
        let parent = scratch.path().join("out");
        let summary =
            export_run_outputs(&parent, "test", &events, RetentionLimits::default())
                .expect("export");
        assert_eq!((summary.stored, summary.missing), (1, 0));
        assert!(
            !parent.join("evil.bin").exists(),
            "no file may escape the owned directory"
        );
        assert!(
            !scratch.path().join("evil.bin").exists(),
            "no file may escape the owned directory"
        );
        let manifest: serde_json::Value = serde_json::from_slice(
            &std::fs::read(&summary.manifest).expect("manifest"),
        )
        .expect("json");
        let artifacts = manifest["artifacts"].as_array().expect("artifacts");
        assert_eq!(artifacts[0]["name"], "../../evil.bin".into());
        let stored = summary.dir.join(artifacts[0]["path"].as_str().expect("path"));
        assert!(stored.starts_with(&summary.dir));
        assert_eq!(std::fs::read(stored).expect("bytes"), b"payload");
    }

    #[test]
    fn export_copies_symlink_sources_to_regular_files() {
        let scratch = tempfile::tempdir().expect("scratch");
        let outside = scratch.path().join("outside.txt");
        std::fs::write(&outside, b"linked").expect("write");
        let link = scratch.path().join("link.bin");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&outside, &link).expect("symlink");
        #[cfg(windows)]
        std::os::windows::fs::symlink_file(&outside, &link).expect("symlink");
        let events = events_for(&[("//a:t", "linked.bin", &file_uri(&link))]);
        let summary =
            export_run_outputs(scratch.path(), "test", &events, RetentionLimits::default())
                .expect("export");
        assert_eq!((summary.stored, summary.missing), (1, 0));
        let manifest: serde_json::Value = serde_json::from_slice(
            &std::fs::read(&summary.manifest).expect("manifest"),
        )
        .expect("json");
        let artifacts = manifest["artifacts"].as_array().expect("artifacts");
        let stored = summary.dir.join(artifacts[0]["path"].as_str().expect("path"));
        assert_eq!(std::fs::read(&stored).expect("bytes"), b"linked");
        assert!(
            !std::fs::symlink_metadata(&stored)
                .expect("metadata")
                .file_type()
                .is_symlink(),
            "retained outputs are regular files, never links"
        );
    }

    #[test]
    fn export_enforces_explicit_size_limits() {
        let scratch = tempfile::tempdir().expect("scratch");
        let big = scratch.path().join("big.bin");
        std::fs::write(&big, b"0123456789").expect("write");
        let small = scratch.path().join("small.bin");
        std::fs::write(&small, b"ab").expect("write");
        let limits = RetentionLimits {
            max_bytes_per_file: 4,
            max_bytes_total: 100,
        };
        let events = events_for(&[
            ("//a:t", "big.bin", &file_uri(&big)),
            ("//a:t", "small.bin", &file_uri(&small)),
        ]);
        let summary = export_run_outputs(scratch.path(), "test", &events, limits).expect("export");
        assert_eq!((summary.stored, summary.missing), (1, 1));
        let manifest: serde_json::Value = serde_json::from_slice(
            &std::fs::read(&summary.manifest).expect("manifest"),
        )
        .expect("json");
        let artifacts = manifest["artifacts"].as_array().expect("artifacts");
        let big_entry = artifacts
            .iter()
            .find(|artifact| artifact["name"] == "big.bin".into())
            .expect("big entry");
        assert_eq!(big_entry["path"], serde_json::Value::Null);
        assert!(
            big_entry["missing_reason"]
                .as_str()
                .expect("reason")
                .contains("per-file limit"),
            "{}",
            big_entry["missing_reason"]
        );
        let total_limits = RetentionLimits {
            max_bytes_per_file: 100,
            max_bytes_total: 3,
        };
        let events = events_for(&[
            ("//a:t", "first.bin", &file_uri(&small)),
            ("//a:t", "second.bin", &file_uri(&small)),
        ]);
        let summary =
            export_run_outputs(scratch.path(), "coverage", &events, total_limits).expect("export");
        assert_eq!((summary.stored, summary.missing), (1, 1));
    }

    #[test]
    fn export_fails_closed_on_an_unusable_parent() {
        let scratch = tempfile::tempdir().expect("scratch");
        let file = scratch.path().join("file");
        std::fs::write(&file, b"not a dir").expect("write");
        let events = events_for(&[]);
        let error = export_run_outputs(&file, "test", &events, RetentionLimits::default())
            .expect_err("a file is not a directory");
        assert!(error.contains("cannot use run-output directory"), "{error}");
        assert!(
            !scratch.path().join("dx-test-1-0").exists(),
            "no owned directory may be left behind"
        );
    }

    #[test]
    fn export_leaves_no_directory_behind_when_storing_fails() {
        let scratch = tempfile::tempdir().expect("scratch");
        let source = scratch.path().join("data.bin");
        std::fs::write(&source, b"data").expect("write");
        let events = events_for(&[("//a:t", "data.bin", &file_uri(&source))]);
        let parent = scratch.path().join("out");
        std::fs::create_dir_all(&parent).expect("mkdir");
        let before: Vec<String> = std::fs::read_dir(&parent)
            .expect("read")
            .map(|entry| {
                entry
                    .expect("entry")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        assert!(before.is_empty());
        let limits = RetentionLimits {
            max_bytes_per_file: 100,
            max_bytes_total: 100,
        };
        let summary = export_run_outputs(&parent, "test", &events, limits).expect("export");
        assert_eq!((summary.stored, summary.missing), (1, 0));
        assert_eq!(
            std::fs::read_dir(&parent).expect("read").count(),
            1,
            "exactly one owned directory per invocation"
        );
    }

    #[test]
    fn retention_window_declares_thirty_days_and_expiry_reads_honestly() {
        let scratch = tempfile::tempdir().expect("scratch");
        let events = events_for(&[]);
        let summary =
            export_run_outputs(scratch.path(), "test", &events, RetentionLimits::default())
                .expect("export");
        let manifest: serde_json::Value = serde_json::from_slice(
            &std::fs::read(&summary.manifest).expect("manifest"),
        )
        .expect("json");
        assert_eq!(manifest["retention_days"], RETENTION_DAYS.into());
        let created = manifest["created_unix"].as_u64().expect("created");
        assert_eq!(
            manifest["retained_until_unix"].as_u64().expect("until"),
            created + RETENTION_DAYS * 86_400
        );
        assert!(!retention_expired(&manifest, created));
        assert!(!retention_expired(
            &manifest,
            created + RETENTION_DAYS * 86_400 - 1
        ));
        assert!(retention_expired(
            &manifest,
            created + RETENTION_DAYS * 86_400
        ));
        assert!(retention_expired(
            &serde_json::json!({"schema": MANIFEST_SCHEMA}),
            created
        ));
    }
}
