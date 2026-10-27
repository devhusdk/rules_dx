//! Free-disk thresholds shared by the worktree reclaim and the Bazel host-failure diagnosis.

use std::path::{Component, Path, PathBuf};

/// Bazel exit code for a failure of the local environment, such as a full disk.
pub(crate) const BAZEL_LOCAL_ENVIRONMENT_EXIT: i32 = 36;

/// Free bytes under which a disk counts as low, together with `LOW_DISK_PERCENT`.
pub(crate) const LOW_DISK_BYTES: u64 = 20 * 1024 * 1024 * 1024;
/// Free percentage under which a disk counts as low, together with `LOW_DISK_BYTES`.
pub(crate) const LOW_DISK_PERCENT: u64 = 10;
/// Free bytes under which a disk counts as full, whatever its size.
pub(crate) const DISK_FULL_BYTES: u64 = 1 << 30;

/// Whether `free` of `total` bytes is under 20 GiB and under 10%, or full.
///
/// A full disk always counts as low, so reclaim runs on a small disk before `add` refuses.
pub(crate) fn is_low(free: u64, total: u64) -> bool {
    is_full(free)
        || (free < LOW_DISK_BYTES
            && u128::from(free) * 100 < u128::from(total) * u128::from(LOW_DISK_PERCENT))
}

/// Whether `free` bytes is under 1 GiB; the percentage rule does not apply.
pub(crate) fn is_full(free: u64) -> bool {
    free < DISK_FULL_BYTES
}

/// The directories a Bazel run writes to: workspace, output roots, temp dir and home.
pub(crate) fn disk_paths(workspace: &Path, argv: &[String]) -> Vec<PathBuf> {
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
pub(crate) fn shortage(
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
pub(crate) fn full_message(code: i32, path: &Path, free: u64) -> String {
    format!(
        "Bazel exited {code} with {} free at {}; the disk is full",
        crate::worktree::ops::size(free),
        path.display()
    )
}

/// The `disk_low` warning for `free` bytes left at `path`, shown before a Bazel run.
pub(crate) fn low_message(path: &Path, free: u64) -> String {
    format!(
        "{} free at {}; free disk space with `dx worktree prune --apply`",
        crate::worktree::ops::size(free),
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
    fn low_needs_both_limits_and_full_needs_only_one() {
        let gib = 1u64 << 30;
        assert!(is_low(5 * gib, 100 * gib));
        assert!(!is_low(5 * gib, 20 * gib));
        assert!(!is_low(30 * gib, 1000 * gib));
        assert!(is_low(gib - 1, 8 * gib), "a full small disk is low");
        assert!(is_full(gib - 1));
        assert!(!is_full(gib));
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
