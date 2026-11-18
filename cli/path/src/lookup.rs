//! Finding an executable on a PATH-style search list.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

/// Returns the first regular file named one of `names` in the directories
/// of `path`, in PATH order.
///
/// Directories win over names: an earlier directory holding any of the names
/// beats a later directory holding an earlier name, which is how a shell
/// resolves a command. Directories named like a tool never match, and an empty
/// entry never searches the working directory.
#[must_use]
pub fn find_in<S: AsRef<Path>>(path: &OsStr, names: &[S]) -> Option<PathBuf> {
    std::env::split_paths(path).find_map(|dir| {
        if dir.as_os_str().is_empty() {
            return None;
        }
        names
            .iter()
            .map(|name| dir.join(name))
            .find(|candidate| candidate.is_file())
    })
}

/// The first file on `path` that the host runs as `command`: the bare name,
/// and on Windows also `command.exe` and the other executable suffixes.
#[must_use]
pub fn find_command_in(path: &OsStr, command: &str) -> Option<PathBuf> {
    let suffixes: &[&str] = if cfg!(windows) {
        crate::host::EXECUTABLE_SUFFIXES
    } else {
        &[]
    };
    find_in(path, &command_names(command, suffixes))
}

fn command_names(command: &str, suffixes: &[&str]) -> Vec<String> {
    std::iter::once(command.to_owned())
        .chain(suffixes.iter().map(|suffix| format!("{command}{suffix}")))
        .collect()
}

#[cfg(test)]
mod tests {
    use std::ffi::OsStr;

    use super::{command_names, find_command_in, find_in};

    #[test]
    fn a_command_is_its_bare_name_then_each_suffix() {
        assert_eq!(command_names("dx", &[]), ["dx"]);
        assert_eq!(
            command_names("dx", &[".exe", ".cmd"]),
            ["dx", "dx.exe", "dx.cmd"]
        );
    }

    #[test]
    fn find_command_in_takes_the_host_executable_suffixes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().as_os_str();
        assert_eq!(find_command_in(path, "dx"), None);
        std::fs::write(dir.path().join("dx.exe"), b"x").unwrap();
        // Windows runs `dx.exe` for `dx`; other hosts run only the bare name.
        let want = cfg!(windows).then(|| dir.path().join("dx.exe"));
        assert_eq!(find_command_in(path, "dx"), want);
        std::fs::write(dir.path().join("dx"), b"x").unwrap();
        assert_eq!(find_command_in(path, "dx"), Some(dir.path().join("dx")));
        assert_eq!(find_command_in(OsStr::new(""), "dx"), None);
    }

    #[test]
    fn finds_files_in_path_order_and_ignores_directories() {
        let first = tempfile::tempdir().unwrap();
        let second = tempfile::tempdir().unwrap();
        let path = std::env::join_paths([first.path(), second.path()]).unwrap();

        assert_eq!(find_in(&path, &["tool"]), None);
        std::fs::create_dir_all(first.path().join("tool")).unwrap();
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

    #[test]
    fn an_empty_path_entry_does_not_search_the_working_directory() {
        let here = tempfile::NamedTempFile::new_in(".").unwrap();
        let name = here.path().file_name().unwrap().to_owned();
        for path in ["", ":", "::"] {
            assert_eq!(find_in(OsStr::new(path), &[&name]), None, "{path:?}");
        }
    }
}
