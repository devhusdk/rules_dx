//! The Bazel host-failure diagnosis for a full or low disk.

use std::path::{Component, Path, PathBuf};

use dx_worktree::disk::is_full;

/// Bazel exit code for a failure of the local environment, such as a full disk.
pub const BAZEL_LOCAL_ENVIRONMENT_EXIT: i32 = 36;

/// The directories a Bazel run writes to: workspace, output roots, temp dir and home.
pub fn disk_paths(workspace: &Path, argv: &[String]) -> Vec<PathBuf> {
    let mut paths = vec![workspace.to_path_buf()];
    for arg in argv {
        for flag in ["--output_base=", "--output_user_root="] {
            if let Some(value) = arg.strip_prefix(flag) {
                paths.push(PathBuf::from(value));
            }
        }
    }
    paths.push(std::env::temp_dir());
    if let Some(home) = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .filter(|value| !value.is_empty())
    {
        paths.push(PathBuf::from(home));
    }
    let mut seen: Vec<std::ffi::OsString> = Vec::new();
    paths.retain(|path| {
        let key = match path.components().next() {
            Some(Component::Prefix(prefix)) => prefix.as_os_str().to_ascii_uppercase(),
            _ => path.as_os_str().to_owned(),
        };
        if seen.contains(&key) {
            return false;
        }
        seen.push(key);
        true
    });
    paths
}

/// The probed path with the least free space, when that is under `DISK_FULL_BYTES`.
pub fn shortage(
    probe: &dyn Fn(&Path) -> Option<u64>,
    workspace: &Path,
    argv: &[String],
) -> Option<(PathBuf, u64)> {
    disk_paths(workspace, argv)
        .into_iter()
        .filter_map(|path| probe(&path).map(|free| (path, free)))
        .filter(|(_, free)| is_full(*free))
        .min_by_key(|(_, free)| *free)
}

/// The diagnosis of a Bazel exit `code` with `free` bytes left at `path`.
pub fn full_message(code: i32, path: &Path, free: u64) -> String {
    format!(
        "Bazel exited {code} with {} free at {}; the disk is full",
        dx_worktree::ops::size(free),
        path.display()
    )
}

/// The `disk_low` warning for `free` bytes left at `path`, shown before a Bazel run.
pub fn low_message(path: &Path, free: u64) -> String {
    format!(
        "{} free at {}; free disk space with `dx worktree prune --apply`",
        dx_worktree::ops::size(free),
        path.display()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn low_message_names_the_free_space_and_the_path() {
        let message = low_message(Path::new("/out"), 512 << 20);
        assert!(message.contains("free at /out;"), "{message}");
        assert!(
            message.ends_with("`dx worktree prune --apply`"),
            "{message}"
        );
    }

    #[test]
    fn disk_paths_cover_the_workspace_output_roots_and_temp_dir() {
        let paths = disk_paths(
            Path::new("/ws"),
            &[
                "--output_user_root=/root".to_owned(),
                "--output_base=/ob".to_owned(),
                "build".to_owned(),
            ],
        );
        assert_eq!(paths[0], PathBuf::from("/ws"));
        assert!(paths.contains(&PathBuf::from("/root")), "{paths:?}");
        assert!(paths.contains(&PathBuf::from("/ob")), "{paths:?}");
        #[cfg(unix)]
        assert!(paths.contains(&std::env::temp_dir()), "{paths:?}");
    }

    #[cfg(windows)]
    #[test]
    fn disk_paths_probe_one_windows_drive_once() {
        let paths = disk_paths(Path::new("C:\\ws"), &["--output_base=c:\\ob".to_owned()]);
        let on_c = paths
            .iter()
            .filter(|path| {
                path.to_string_lossy()
                    .to_ascii_uppercase()
                    .starts_with("C:")
            })
            .count();
        assert_eq!(on_c, 1, "{paths:?}");
    }
}
