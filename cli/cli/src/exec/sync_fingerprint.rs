//! What the last `dx sync` of `.dx/local` saw, so a build knows without Bazel whether to sync again.
//!
//! The record is one file: `v1 <digest>`, then `ok` or `error <message>`, then `targets` and the
//! sync's targets, then one generated artifact path per line. The digest covers the selected setup,
//! the stats of the workspace files that steer the sync and the stats of every artifact the
//! selected generation links to, so a build that regenerates one of them changes it.

use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

/// Bump when the record layout or the digest inputs change.
const VERSION: &str = "v1";

/// Workspace files whose stats the digest covers.
const INPUTS: [&str; 6] = [
    "MODULE.bazel",
    "MODULE.bazel.lock",
    ".bazelversion",
    ".bazelrc",
    "dx.toml",
    ".dx/pins",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Record {
    /// The targets the sync ran with; empty for the whole repository.
    pub(crate) targets: Vec<String>,
    pub(crate) artifacts: Vec<PathBuf>,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Check {
    Missing,
    Changed(Record),
    Same,
    SameFailed(String),
}

/// Records the inputs as they are now; `error` is the failure of a sync that did not commit.
pub(crate) fn record(
    workspace: &Path,
    record: &Record,
    error: Option<&str>,
) -> std::io::Result<()> {
    let mut text = format!("{VERSION} {}\n", digest(workspace, record));
    match error {
        None => text.push_str("ok\n"),
        Some(message) => {
            let message = message.replace(['\r', '\n'], " ");
            text.push_str(&format!("error {message}\n"));
        }
    }
    text.push_str("targets");
    for target in &record.targets {
        text.push(' ');
        text.push_str(target);
    }
    text.push('\n');
    for artifact in &record.artifacts {
        text.push_str(&artifact.to_string_lossy());
        text.push('\n');
    }
    dx_atomic_fs::write_atomic(
        &dx_path::state::sync_fingerprint_path(workspace),
        text.as_bytes(),
    )
}

pub(crate) fn check(workspace: &Path) -> Check {
    let Ok(text) = std::fs::read_to_string(dx_path::state::sync_fingerprint_path(workspace)) else {
        return Check::Missing;
    };
    let mut lines = text.lines();
    let (Some(head), Some(status), Some(targets)) = (lines.next(), lines.next(), lines.next())
    else {
        return Check::Missing;
    };
    let Some(stored) = head
        .strip_prefix(VERSION)
        .and_then(|rest| rest.strip_prefix(' '))
    else {
        return Check::Missing;
    };
    let Some(targets) = targets.strip_prefix("targets") else {
        return Check::Missing;
    };
    let record = Record {
        targets: targets.split_whitespace().map(str::to_owned).collect(),
        artifacts: lines.map(PathBuf::from).collect(),
    };
    if stored != digest(workspace, &record) {
        return Check::Changed(record);
    }
    match status.strip_prefix("error ") {
        Some(message) => Check::SameFailed(message.to_owned()),
        None => Check::Same,
    }
}

fn digest(workspace: &Path, record: &Record) -> String {
    let mut buf = Vec::new();
    let mut field = |bytes: &[u8]| {
        buf.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
        buf.extend_from_slice(bytes);
    };
    field(VERSION.as_bytes());
    field(dx_adopt::DX_VERSION.as_bytes());
    match dx_setup::read_current_pair(workspace).ok().flatten() {
        Some(pair) => field(dx_setup::setup_hex(&pair).as_bytes()),
        None => field(b"no setup"),
    }
    field(record.targets.join(" ").as_bytes());
    let inputs = INPUTS.iter().map(|rel| workspace.join(rel));
    for path in inputs.chain(record.artifacts.iter().cloned()) {
        field(path.to_string_lossy().as_bytes());
        match stat(&path) {
            Some((len, mtime)) => {
                field(&len.to_le_bytes());
                field(&mtime.to_le_bytes());
            }
            None => field(b"absent"),
        }
    }
    dx_digest::to_hex(&dx_digest::blake3(&buf))
}

fn stat(path: &Path) -> Option<(u64, u128)> {
    let metadata = std::fs::metadata(path).ok()?;
    let mtime = metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |since| since.as_nanos());
    Some((metadata.len(), mtime))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn workspace() -> tempfile::TempDir {
        let temp = tempfile::tempdir().expect("temp");
        std::fs::create_dir_all(dx_path::state::local_dir(temp.path())).expect("local");
        std::fs::write(temp.path().join("MODULE.bazel"), "module(name = \"w\")\n").expect("module");
        temp
    }

    fn artifact(temp: &tempfile::TempDir) -> Record {
        let path = temp.path().join("out.rs");
        std::fs::write(&path, "pub fn a() {}\n").expect("artifact");
        Record {
            targets: Vec::new(),
            artifacts: vec![path],
        }
    }

    #[test]
    fn missing_without_a_record() {
        let temp = workspace();
        assert_eq!(check(temp.path()), Check::Missing);
    }

    #[test]
    fn missing_for_an_unknown_version() {
        let temp = workspace();
        std::fs::write(
            dx_path::state::sync_fingerprint_path(temp.path()),
            "v0 abc\nok\ntargets\n",
        )
        .expect("write");
        assert_eq!(check(temp.path()), Check::Missing);
    }

    #[test]
    fn same_until_an_artifact_changes() {
        let temp = workspace();
        let record = artifact(&temp);
        super::record(temp.path(), &record, None).expect("record");
        assert_eq!(check(temp.path()), Check::Same);
        std::fs::write(&record.artifacts[0], "pub fn a() {}\npub fn b() {}\n").expect("rewrite");
        assert_eq!(check(temp.path()), Check::Changed(record));
    }

    #[test]
    fn changed_when_an_input_changes() {
        let temp = workspace();
        let record = artifact(&temp);
        super::record(temp.path(), &record, None).expect("record");
        std::fs::write(temp.path().join("dx.toml"), "[scope]\n").expect("dx.toml");
        assert_eq!(check(temp.path()), Check::Changed(record));
    }

    #[test]
    fn same_failed_keeps_the_message_and_targets() {
        let temp = workspace();
        let record = Record {
            targets: vec!["//app:x".to_owned()],
            ..artifact(&temp)
        };
        super::record(temp.path(), &record, Some("codegen\nbroke")).expect("record");
        assert_eq!(
            check(temp.path()),
            Check::SameFailed("codegen broke".to_owned())
        );
        std::fs::remove_file(&record.artifacts[0]).expect("remove");
        assert_eq!(check(temp.path()), Check::Changed(record));
    }
}
