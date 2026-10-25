//! Bazel package detection by BUILD marker files.

use std::path::Path;

/// The file names that make a directory a Bazel package, in lookup order.
pub const PACKAGE_FILES: [&str; 2] = ["BUILD.bazel", "BUILD"];

/// Whether a directory entry is a BUILD marker that is a file after following symlinks.
pub fn is_package_marker(entry: &std::fs::DirEntry) -> bool {
    let name = entry.file_name();
    PACKAGE_FILES
        .iter()
        .any(|marker| name == std::ffi::OsStr::new(marker))
        && std::fs::metadata(entry.path()).is_ok_and(|meta| meta.is_file())
}

/// Whether a directory holds a BUILD marker that is a file under its exact name.
pub fn is_package_dir(dir: &Path) -> bool {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return false;
    };
    entries
        .filter_map(Result::ok)
        .any(|entry| is_package_marker(&entry))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_build_file_makes_a_package() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(dir.path().join("BUILD.bazel"), "").expect("write");
        assert!(is_package_dir(dir.path()));
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(dir.path().join("BUILD"), "").expect("write");
        assert!(is_package_dir(dir.path()));
    }

    #[test]
    fn a_build_directory_is_not_a_package() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir(dir.path().join("BUILD")).expect("mkdir");
        std::fs::write(dir.path().join("BUILD/out.js"), "").expect("write");
        assert!(!is_package_dir(dir.path()));
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir(dir.path().join("BUILD.bazel")).expect("mkdir");
        assert!(!is_package_dir(dir.path()));
    }

    #[test]
    fn a_case_variant_file_is_not_a_package() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(dir.path().join("build"), "").expect("write");
        std::fs::write(dir.path().join("Build.bazel"), "").expect("write");
        assert!(!is_package_dir(dir.path()));
    }

    #[test]
    fn a_missing_dir_is_not_a_package() {
        let dir = tempfile::tempdir().expect("tempdir");
        assert!(!is_package_dir(&dir.path().join("gone")));
        assert!(!is_package_dir(dir.path()));
    }

    #[cfg(unix)]
    #[test]
    fn symlinks_count_only_when_they_reach_a_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(dir.path().join("real.bzl"), "").expect("write");
        std::os::unix::fs::symlink("real.bzl", dir.path().join("BUILD")).expect("symlink");
        assert!(is_package_dir(dir.path()));
        let dir = tempfile::tempdir().expect("tempdir");
        std::os::unix::fs::symlink("missing", dir.path().join("BUILD")).expect("symlink");
        assert!(!is_package_dir(dir.path()));
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir(dir.path().join("out")).expect("mkdir");
        std::os::unix::fs::symlink("out", dir.path().join("BUILD.bazel")).expect("symlink");
        assert!(!is_package_dir(dir.path()));
    }
}
