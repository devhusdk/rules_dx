use dx_apply::{FileSystem, RealFileSystem};
use dx_bep::{ArtifactReader, TestEvents};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

pub(crate) const RUN_OUTPUT_FORMAT: &str = "run-output";
pub(crate) const MANIFEST_NAME: &str = "manifest.json";
pub(crate) const CODE_RUN_OUTPUT_FAILED: &str = "run_output_failed";

static EXPORT_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Debug)]
pub(crate) struct ExportedRun {
    pub(crate) dir: PathBuf,
    pub(crate) manifest: PathBuf,
    pub(crate) retained: usize,
    pub(crate) missing: usize,
}

/// Copies every BEP-declared test output into one owned run directory.
///
/// Sources are only read. File names are sanitized so a hostile logical name
/// cannot escape the run directory, and collisions gain a numeric suffix.
pub(crate) fn export_run_outputs(
    parent: &Path,
    command: &str,
    pid: u32,
    nonce: u64,
    events: &TestEvents,
    reader: &dyn ArtifactReader,
) -> Result<ExportedRun, String> {
    std::fs::create_dir_all(parent).map_err(|error| {
        format!(
            "cannot use run output directory {}: {error}",
            parent.display()
        )
    })?;
    if !parent.is_dir() {
        return Err(format!(
            "cannot use run output directory {}: not a directory",
            parent.display()
        ));
    }
    let dir = claim_run_dir(parent, pid, nonce)?;
    let mut used: HashSet<String> = HashSet::new();
    let mut entries = Vec::new();
    let mut retained = 0usize;
    let mut missing = 0usize;
    for result in &events.results {
        for file in &result.files {
            let stored = unique_name(
                &mut used,
                &format!(
                    "{}-run{}-shard{}-attempt{}-{}",
                    sanitize(&result.label),
                    result.run,
                    result.shard,
                    result.attempt,
                    sanitize(&file.name)
                ),
            );
            match reader.read_artifact(&file.exec_path) {
                Ok(bytes) => {
                    let destination = dir.join(&stored);
                    match std::fs::write(&destination, &bytes) {
                        Ok(()) => {
                            retained += 1;
                            entries.push(serde_json::json!({
                                "target": result.label,
                                "configuration": result.configuration,
                                "run": result.run,
                                "shard": result.shard,
                                "attempt": result.attempt,
                                "status": result.status,
                                "name": file.name,
                                "state": "retained",
                                "path": stored,
                                "bytes": bytes.len(),
                                "detail": serde_json::Value::Null,
                            }));
                        }
                        Err(error) => {
                            missing += 1;
                            entries.push(serde_json::json!({
                                "target": result.label,
                                "configuration": result.configuration,
                                "run": result.run,
                                "shard": result.shard,
                                "attempt": result.attempt,
                                "status": result.status,
                                "name": file.name,
                                "state": "missing",
                                "path": serde_json::Value::Null,
                                "bytes": serde_json::Value::Null,
                                "detail": format!("cannot retain {stored}: {error}"),
                            }));
                        }
                    }
                }
                Err(error) => {
                    missing += 1;
                    entries.push(serde_json::json!({
                        "target": result.label,
                        "configuration": result.configuration,
                        "run": result.run,
                        "shard": result.shard,
                        "attempt": result.attempt,
                        "status": result.status,
                        "name": file.name,
                        "state": "missing",
                        "path": serde_json::Value::Null,
                        "bytes": serde_json::Value::Null,
                        "detail": format!("unreadable test output {}: {error}", file.exec_path.display()),
                    }));
                }
            }
        }
    }
    let manifest = dir.join(MANIFEST_NAME);
    let document = serde_json::json!({
        "version": 1,
        "command": command,
        "process": pid,
        "artifacts": entries,
    });
    let text = serde_json::to_string_pretty(&document)
        .map_err(|error| format!("cannot render run output manifest: {error}"))?;
    RealFileSystem
        .write_atomic(&manifest, text.as_bytes())
        .map_err(|error| {
            format!(
                "cannot write run output manifest {}: {error}",
                manifest.display()
            )
        })?;
    Ok(ExportedRun {
        dir,
        manifest,
        retained,
        missing,
    })
}

/// Claims one owned child directory, retrying past races on the same name.
fn claim_run_dir(parent: &Path, pid: u32, nonce: u64) -> Result<PathBuf, String> {
    for _ in 0..64 {
        let counter = EXPORT_COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = parent.join(format!("dx-run-outputs-{pid}-{nonce}-{counter}"));
        match std::fs::create_dir(&dir) {
            Ok(()) => return Ok(dir),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(format!(
                    "cannot create run output directory {}: {error}",
                    dir.display()
                ));
            }
        }
    }
    Err(format!(
        "cannot create run output directory under {}: export conflict",
        parent.display()
    ))
}

/// Spells one path component with only filesystem-safe characters.
fn sanitize(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for got in text.chars() {
        if got.is_ascii_alphanumeric() || matches!(got, '.' | '-' | '_') {
            out.push(got);
        } else {
            out.push('_');
        }
    }
    if out.is_empty() {
        out.push('_');
    }
    out
}

/// Makes one file name unique inside the run directory without moving it out.
fn unique_name(used: &mut HashSet<String>, base: &str) -> String {
    let base = truncate_name(base);
    if used.insert(base.clone()) {
        return base;
    }
    let mut index = 2u64;
    loop {
        let candidate = truncate_name(&format!("{base}-{index}"));
        if used.insert(candidate.clone()) {
            return candidate;
        }
        index = index.wrapping_add(1);
    }
}

/// Caps one file name below the filesystem component limit.
fn truncate_name(base: &str) -> String {
    const LIMIT: usize = 180;
    if base.len() <= LIMIT {
        return base.to_owned();
    }
    let mut end = LIMIT;
    while end > 0 && !base.is_char_boundary(end) {
        end -= 1;
    }
    base[..end].to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    struct MapReader {
        entries: Vec<(PathBuf, Vec<u8>)>,
    }

    impl ArtifactReader for MapReader {
        fn read_artifact(&self, path: &Path) -> std::io::Result<Vec<u8>> {
            self.entries
                .iter()
                .find(|(name, _)| name == path)
                .map(|(_, bytes)| bytes.clone())
                .ok_or_else(|| {
                    std::io::Error::new(std::io::ErrorKind::NotFound, "no such artifact")
                })
        }
    }

    fn events_for(outputs: &[(&str, &str)]) -> TestEvents {
        let lines: Vec<String> = outputs
            .iter()
            .map(|(name, uri)| {
                serde_json::json!({
                    "id": {"testResult": {"label": "//a:t", "run": 1, "shard": 1, "attempt": 1}},
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
    fn export_retains_bytes_and_indexes_them() {
        let root = tempfile::tempdir().expect("root");
        let source = root.path().join("test.xml");
        std::fs::write(&source, b"<xml/>").expect("source");
        let reader = MapReader {
            entries: vec![(source, b"<xml/>".to_vec())],
        };
        let events = events_for(&[("test.xml", &file_uri(&reader.entries[0].0))]);
        let parent = root.path().join("runs");
        let got = export_run_outputs(&parent, "test", 7, 3, &events, &reader).expect("export");
        assert!(got.dir.is_dir());
        assert!(got.dir.starts_with(&parent));
        assert_eq!((got.retained, got.missing), (1, 0));
        let manifest: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&got.manifest).expect("manifest"))
                .expect("manifest parses");
        assert_eq!(manifest["version"], serde_json::json!(1));
        assert_eq!(manifest["command"], serde_json::json!("test"));
        let artifacts = manifest["artifacts"].as_array().expect("artifacts");
        assert_eq!(artifacts.len(), 1);
        let entry = &artifacts[0];
        assert_eq!(entry["target"], serde_json::json!("//a:t"));
        assert_eq!(entry["run"], serde_json::json!(1));
        assert_eq!(entry["shard"], serde_json::json!(1));
        assert_eq!(entry["attempt"], serde_json::json!(1));
        assert_eq!(entry["status"], serde_json::json!("PASSED"));
        assert_eq!(entry["name"], serde_json::json!("test.xml"));
        assert_eq!(entry["state"], serde_json::json!("retained"));
        let stored = entry["path"].as_str().expect("retained path");
        assert_eq!(entry["bytes"], serde_json::json!(6));
        assert_eq!(
            std::fs::read(got.dir.join(stored)).expect("retained bytes"),
            b"<xml/>"
        );
        assert_eq!(
            std::fs::read(&reader.entries[0].0).expect("source intact"),
            b"<xml/>"
        );
    }

    #[test]
    fn export_marks_missing_outputs_explicit() {
        let root = tempfile::tempdir().expect("root");
        let reader = MapReader { entries: vec![] };
        let events = events_for(&[("test.xml", "file:///nonexistent/a.xml")]);
        let got = export_run_outputs(root.path(), "test", 7, 3, &events, &reader).expect("export");
        assert_eq!((got.retained, got.missing), (0, 1));
        let manifest: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&got.manifest).expect("manifest"))
                .expect("manifest parses");
        let entry = &manifest["artifacts"][0];
        assert_eq!(entry["state"], serde_json::json!("missing"));
        assert!(entry["path"].is_null());
        assert!(entry["detail"]
            .as_str()
            .expect("detail")
            .contains("unreadable"));
    }

    #[test]
    fn export_never_escapes_its_directory() {
        let root = tempfile::tempdir().expect("root");
        let outside = root.path().join("outside.txt");
        let reader = MapReader { entries: vec![] };
        let events = events_for(&[
            ("../../outside.txt", "file:///nonexistent/a.xml"),
            ("..\\outside.txt", "file:///nonexistent/b.xml"),
            ("/absolute.txt", "file:///nonexistent/c.xml"),
        ]);
        let parent = root.path().join("runs");
        let got = export_run_outputs(&parent, "test", 7, 3, &events, &reader).expect("export");
        assert!(!outside.exists());
        for entry in std::fs::read_dir(&got.dir).expect("entries") {
            let entry = entry.expect("entry");
            assert_eq!(entry.path().parent(), Some(got.dir.as_path()));
            assert!(entry.file_type().expect("type").is_file());
        }
        let manifest: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&got.manifest).expect("manifest"))
                .expect("manifest parses");
        assert_eq!(manifest["artifacts"].as_array().expect("array").len(), 3);
    }

    #[test]
    fn export_disambiguates_duplicate_names() {
        let root = tempfile::tempdir().expect("root");
        let first = root.path().join("a.xml");
        let second = root.path().join("b.xml");
        std::fs::write(&first, b"one").expect("first");
        std::fs::write(&second, b"two").expect("second");
        let reader = MapReader {
            entries: vec![(first, b"one".to_vec()), (second, b"two".to_vec())],
        };
        let events = events_for(&[
            ("a/b", &file_uri(&reader.entries[0].0)),
            ("a_b", &file_uri(&reader.entries[1].0)),
        ]);
        let got = export_run_outputs(root.path(), "test", 7, 3, &events, &reader).expect("export");
        assert_eq!((got.retained, got.missing), (2, 0));
        let manifest: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&got.manifest).expect("manifest"))
                .expect("manifest parses");
        let paths: Vec<&str> = manifest["artifacts"]
            .as_array()
            .expect("array")
            .iter()
            .map(|entry| entry["path"].as_str().expect("path"))
            .collect();
        assert_eq!(paths.len(), 2);
        assert_ne!(paths[0], paths[1]);
        let mut bodies: Vec<Vec<u8>> = paths
            .iter()
            .map(|name| std::fs::read(got.dir.join(name)).expect("body"))
            .collect();
        bodies.sort();
        assert_eq!(bodies, vec![b"one".to_vec(), b"two".to_vec()]);
    }

    #[test]
    fn export_refuses_a_parent_that_is_a_file() {
        let root = tempfile::tempdir().expect("root");
        let parent = root.path().join("file");
        std::fs::write(&parent, b"owned").expect("file");
        let reader = MapReader { entries: vec![] };
        let events = events_for(&[("test.xml", "file:///nonexistent/a.xml")]);
        let error = export_run_outputs(&parent, "test", 7, 3, &events, &reader).expect_err("file");
        assert!(error.contains("run output directory"), "{error}");
        assert_eq!(std::fs::read(&parent).expect("untouched"), b"owned");
    }

    #[test]
    fn export_reads_through_source_symlinks_without_creating_any() {
        let root = tempfile::tempdir().expect("root");
        let source = root.path().join("real.xml");
        std::fs::write(&source, b"linked").expect("source");
        let link = root.path().join("link.xml");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&source, &link).expect("symlink");
        #[cfg(windows)]
        std::os::windows::fs::symlink_file(&source, &link).expect("symlink");
        let reader = MapReader {
            entries: vec![(link.clone(), std::fs::read(&link).expect("through link"))],
        };
        let link_uri = file_uri(&reader.entries[0].0);
        let events = events_for(&[("test.xml", &link_uri)]);
        let got = export_run_outputs(root.path(), "test", 7, 3, &events, &reader).expect("export");
        assert_eq!((got.retained, got.missing), (1, 0));
        for entry in std::fs::read_dir(&got.dir).expect("entries") {
            let entry = entry.expect("entry");
            assert!(
                entry.file_type().expect("type").is_file(),
                "the run directory holds copies, not links"
            );
        }
    }

    #[test]
    fn sanitize_keeps_safe_characters_only() {
        assert_eq!(sanitize("//a:b"), "__a_b");
        assert_eq!(sanitize("test.xml"), "test.xml");
        assert_eq!(sanitize(""), "_");
        assert!(truncate_name(&"x".repeat(400)).len() <= 180);
    }
}
