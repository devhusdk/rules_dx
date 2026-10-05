#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

use std::path::Path;
pub use tempfile::TempDir;

/// Longest prefix kept verbatim; the rest is folded into a checksum.
const PREFIX_KEEP: usize = 24;

fn fnv1a(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100_0000_01b3);
    }
    hash
}

/// Keeps the temp dir name short enough for the Windows 260-char path limit.
fn bounded_prefix(prefix: &str) -> String {
    let sanitized: String = prefix
        .chars()
        .map(|char| {
            if char.is_ascii_alphanumeric() || char == '-' || char == '_' {
                char
            } else {
                '-'
            }
        })
        .collect();
    if sanitized.len() <= PREFIX_KEEP {
        return sanitized;
    }
    format!(
        "{}-{:x}",
        &sanitized[..PREFIX_KEEP],
        fnv1a(prefix.as_bytes())
    )
}

pub fn scratch(prefix: &str) -> TempDir {
    let bounded = bounded_prefix(prefix);
    tempfile::Builder::new()
        .prefix(&bounded)
        .tempdir()
        .unwrap_or_else(|err| panic!("test scratch creates {prefix:?}: {err}"))
}

/// Removes a directory link without following it into its target.
pub fn remove_directory_link(path: &Path) -> std::io::Result<()> {
    #[cfg(windows)]
    let removed = std::fs::remove_dir(path);
    #[cfg(not(windows))]
    let removed = std::fs::remove_file(path);
    removed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(windows)]
    fn directory_link(target: &Path, link: &Path) -> std::io::Result<()> {
        std::os::windows::fs::symlink_dir(target, link)
    }

    #[cfg(not(windows))]
    fn directory_link(target: &Path, link: &Path) -> std::io::Result<()> {
        std::os::unix::fs::symlink(target, link)
    }

    #[test]
    fn long_prefixes_stay_inside_the_windows_path_limit() {
        let long = "dx-exec-test-audit-live-exec__audit__audit_live__audit_live_license_names_uncovered_dependency_sets-license-__go_tests_fixtures_hello:hello-5-ws-";
        let bounded = bounded_prefix(long);
        assert!(bounded.len() <= PREFIX_KEEP + 1 + 16, "{}", bounded.len());
        assert!(bounded.starts_with("dx-exec-test-audit-l"), "{bounded}");
        assert_eq!(bounded, bounded_prefix(long), "stable");
        assert_ne!(bounded, bounded_prefix(&format!("{long}x")), "distinct");
        assert_eq!(
            bounded_prefix("dx-clean-walk-"),
            "dx-clean-walk-",
            "short prefixes pass through"
        );
        assert!(
            !bounded.contains(':'),
            "separators are sanitized: {bounded}"
        );
    }

    #[test]
    fn scratch_uses_prefix_and_exists() {
        let dir = scratch("dx-test-scratch-");
        assert!(dir.path().is_dir(), "scratch exists: {:?}", dir.path());
        let name = dir.path().file_name().expect("tempdir has a name");
        assert!(
            name.to_string_lossy().starts_with("dx-test-scratch-"),
            "prefix honored: {name:?}"
        );
    }

    #[test]
    fn scratches_are_unique() {
        let first = scratch("dx-test-scratch-");
        let second = scratch("dx-test-scratch-");
        assert_ne!(first.path(), second.path(), "unique dirs");
    }

    #[test]
    fn scratch_cleans_on_drop() {
        let path = {
            let dir = scratch("dx-test-scratch-");
            let path = dir.path().to_path_buf();
            assert!(path.is_dir(), "exists before drop");
            path
        };
        assert!(!path.exists(), "removed on drop: {path:?}");
    }

    #[test]
    fn remove_directory_link_keeps_the_target() {
        let dir = scratch("dx-test-scratch-link-");
        let target = dir.path().join("target");
        std::fs::create_dir(&target).expect("target");
        std::fs::write(target.join("marker"), "marker").expect("marker");
        let link = dir.path().join("link");
        directory_link(&target, &link).expect("directory link");
        remove_directory_link(&link).expect("remove directory link");
        assert!(std::fs::symlink_metadata(&link).is_err(), "link removed");
        assert_eq!(
            std::fs::read_to_string(target.join("marker")).expect("marker survives"),
            "marker",
            "the target is not followed"
        );
    }
}
