//! The numbered execution logs `build`/`test --explain` save under `.dx/explain`.

use std::fs::OpenOptions;
use std::io;
use std::path::{Path, PathBuf};

const SUFFIX: &str = ".binpb.zst";

pub(crate) const CODE_EXPLAIN_LOG: &str = "explain_log";
pub(crate) const CODE_EXPLAIN_LOG_UNWRITABLE: &str = "explain_log_unwritable";

/// How many logs a `--explain` run keeps.
pub(crate) const KEEP: usize = 2;

pub(crate) fn dir(workspace: &Path) -> PathBuf {
    workspace.join(".dx").join("explain")
}

fn number(name: &str) -> Option<u64> {
    let digits = name.strip_suffix(SUFFIX)?;
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    digits.parse().ok()
}

fn path_for(workspace: &Path, number: u64) -> PathBuf {
    dir(workspace).join(format!("{number}{SUFFIX}"))
}

/// Saved log numbers, newest first.
fn numbers(workspace: &Path) -> io::Result<Vec<u64>> {
    let entries = match std::fs::read_dir(dir(workspace)) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };
    let mut found = Vec::new();
    for entry in entries {
        let entry = entry?;
        if let Some(number) = entry.file_name().to_str().and_then(number) {
            found.push(number);
        }
    }
    found.sort_unstable_by(|a, b| b.cmp(a));
    Ok(found)
}

/// A reserved log path, created empty so concurrent runs never share a number.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Reserved {
    pub(crate) number: u64,
    pub(crate) path: PathBuf,
}

/// Creates the next numbered log file.
pub(crate) fn reserve(workspace: &Path) -> io::Result<Reserved> {
    std::fs::create_dir_all(dir(workspace))?;
    let mut next = numbers(workspace)?.first().map_or(1, |newest| newest + 1);
    loop {
        let path = path_for(workspace, next);
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(_) => return Ok(Reserved { number: next, path }),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => next += 1,
            Err(error) => return Err(error),
        }
    }
}

/// Deletes logs numbered at most `own`, keeping the `keep` newest of them.
pub(crate) fn prune(workspace: &Path, keep: usize, own: u64) -> io::Result<()> {
    for number in numbers(workspace)?
        .into_iter()
        .filter(|number| *number <= own)
        .skip(keep)
    {
        match std::fs::remove_file(path_for(workspace, number)) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

/// The `n` newest saved logs, newest first.
pub(crate) fn latest(workspace: &Path, n: usize) -> io::Result<Vec<PathBuf>> {
    Ok(numbers(workspace)?
        .into_iter()
        .take(n)
        .map(|number| path_for(workspace, number))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(workspace: &Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(dir(workspace))
            .expect("dir")
            .map(|entry| {
                entry
                    .expect("entry")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        names.sort();
        names
    }

    #[test]
    fn reserve_numbers_upward_and_skips_taken_names() {
        let ws = tempfile::tempdir().expect("ws");
        assert_eq!(reserve(ws.path()).expect("first").number, 1);
        assert_eq!(reserve(ws.path()).expect("second").number, 2);
        std::fs::write(dir(ws.path()).join("9.binpb.zst"), "").expect("nine");
        let got = reserve(ws.path()).expect("third");
        assert_eq!(got.number, 10);
        assert_eq!(got.path, dir(ws.path()).join("10.binpb.zst"));
        assert!(got.path.is_file());
    }

    #[test]
    fn only_digit_names_count() {
        let ws = tempfile::tempdir().expect("ws");
        std::fs::create_dir_all(dir(ws.path())).expect("dir");
        for name in [
            "x.binpb.zst",
            ".binpb.zst",
            "3.binpb",
            "4.binpb.zst.tmp",
            "-1.binpb.zst",
        ] {
            std::fs::write(dir(ws.path()).join(name), "").expect("write");
        }
        assert!(latest(ws.path(), 5).expect("latest").is_empty());
        assert_eq!(reserve(ws.path()).expect("reserve").number, 1);
        prune(ws.path(), 0, 1).expect("prune");
        assert_eq!(names(ws.path()).len(), 5);
    }

    #[test]
    fn prune_keeps_the_newest_and_never_touches_higher_numbers() {
        let ws = tempfile::tempdir().expect("ws");
        for _ in 0..5 {
            reserve(ws.path()).expect("reserve");
        }
        prune(ws.path(), 2, 3).expect("prune");
        assert_eq!(
            names(ws.path()),
            ["2.binpb.zst", "3.binpb.zst", "4.binpb.zst", "5.binpb.zst"]
        );
        assert_eq!(
            latest(ws.path(), 2).expect("latest"),
            [path_for(ws.path(), 5), path_for(ws.path(), 4)]
        );
    }

    #[test]
    fn a_missing_store_is_empty() {
        let ws = tempfile::tempdir().expect("ws");
        assert!(latest(ws.path(), 2).expect("latest").is_empty());
        prune(ws.path(), 2, 7).expect("prune");
    }
}
