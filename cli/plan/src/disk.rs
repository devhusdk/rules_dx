use std::path::{Component, Path, PathBuf};

use dx_worktree::disk::is_full;

pub const BAZEL_LOCAL_ENVIRONMENT_EXIT: i32 = 36;

#[must_use]
pub fn disk_paths(workspace: &Path, argv: &[String]) -> Vec<PathBuf> {
    let mut paths = vec![workspace.to_path_buf()];
    let mut args = argv.iter();
    while let Some(arg) = args.next() {
        for flag in ["--output_base", "--output_user_root"] {
            let value = match arg.strip_prefix(flag) {
                Some("") => args.next().map(String::as_str),
                Some(rest) => rest.strip_prefix('='),
                None => None,
            };
            if let Some(value) = value.filter(|value| !value.is_empty()) {
                paths.push(PathBuf::from(value));
            }
        }
    }
    paths.push(std::env::temp_dir());
    if let Some(home) = dx_process::home_dir() {
        paths.push(home);
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

#[must_use]
pub fn full_message(code: i32, path: &Path, free: u64) -> String {
    format!(
        "Bazel exited {code} with {} free at {}; the disk is full",
        dx_worktree::ops::size(free),
        path.display()
    )
}

#[must_use]
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

    #[test]
    fn disk_paths_read_a_separate_value_and_skip_an_empty_one() {
        let paths = disk_paths(
            Path::new("/ws"),
            &[
                "--output_base".to_owned(),
                "/ob".to_owned(),
                "--output_user_root=".to_owned(),
            ],
        );
        assert!(paths.contains(&PathBuf::from("/ob")), "{paths:?}");
        assert!(!paths.contains(&PathBuf::new()), "{paths:?}");
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
