//! Finding an executable on a PATH-style search list.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

/// Returns the first regular file named one of `names` in the directories
/// of `path`, in PATH order.
///
/// Directories win over names: an earlier directory holding any of the names
/// beats a later directory holding an earlier name, which is how a shell
/// resolves a command. Directories named like a tool never match.
#[must_use]
pub fn find_in<S: AsRef<Path>>(path: &OsStr, names: &[S]) -> Option<PathBuf> {
    std::env::split_paths(path).find_map(|dir| {
        names
            .iter()
            .map(|name| dir.join(name))
            .find(|candidate| candidate.is_file())
    })
}

/// [`find_in`] over this process's `PATH`; `None` when `PATH` is unset.
#[must_use]
pub fn find<S: AsRef<Path>>(names: &[S]) -> Option<PathBuf> {
    find_in(&std::env::var_os("PATH")?, names)
}

#[cfg(test)]
mod tests {
    use std::ffi::OsStr;

    use super::find_in;

    #[test]
    fn finds_files_in_path_order_and_ignores_directories() {
        let first = tempfile::tempdir().unwrap();
        let second = tempfile::tempdir().unwrap();
        let path = std::env::join_paths([first.path(), second.path()]).unwrap();

        assert_eq!(find_in(&path, &["tool"]), None);
        std::fs::create_dir(first.path().join("tool")).unwrap();
        assert_eq!(find_in(&path, &["tool"]), None);

        std::fs::write(second.path().join("tool"), b"x").unwrap();
        assert_eq!(find_in(&path, &["tool"]), Some(second.path().join("tool")));

        // An earlier directory wins even when it only holds a later name.
        std::fs::write(first.path().join("tool.cmd"), b"x").unwrap();
        assert_eq!(
            find_in(&path, &["tool", "tool.cmd"]),
            Some(first.path().join("tool.cmd"))
        );
    }

    #[test]
    fn empty_path_or_names_find_nothing() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("tool"), b"x").unwrap();
        assert_eq!(find_in(OsStr::new(""), &["tool"]), None);
        assert_eq!(find_in(dir.path().as_os_str(), &[] as &[&str]), None);
    }
}
