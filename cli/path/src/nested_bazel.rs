//! Where nested-Bazel tests keep their servers: one slot directory with `lock`, `u` and `w` per slot.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

/// The environment variable that moves the root.
pub const ROOT_ENV: &str = "DX_NESTED_BAZEL_ROOT";
/// The file in a slot the harness locks while a test uses the slot.
pub const LOCK: &str = "lock";
/// The slot's Bazel output user root.
pub const USER_ROOT: &str = "u";
/// The slot's directory of test case workspaces.
pub const WORKSPACES: &str = "w";
/// The environment variable that sets how many nested Bazel servers tests run at once.
pub const SLOTS_ENV: &str = "DX_NESTED_BAZEL_SLOTS";
/// Nested Bazel servers tests run at once by default.
pub const DEFAULT_SLOTS: u16 = 3;

/// The nested-Bazel root: `env` when set and non-empty, else `/tmp/dxnb-<uid>`, else `.dxnb` in the
/// user profile (Windows), else `dxnb` in the temp dir.
pub fn root(env: Option<&OsStr>, uid: Option<u32>, profile: Option<&Path>) -> PathBuf {
    if let Some(env) = env.filter(|value| !value.is_empty()) {
        return PathBuf::from(env);
    }
    match (uid, profile.filter(|value| !value.as_os_str().is_empty())) {
        (Some(uid), _) => PathBuf::from(format!("/tmp/dxnb-{uid}")),
        (None, Some(profile)) => profile.join(".dxnb"),
        (None, None) => std::env::temp_dir().join("dxnb"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_environment_wins_then_the_uid_then_the_profile_then_the_temp_dir() {
        let profile = Path::new("/users/me");
        assert_eq!(
            root(Some(OsStr::new("/custom/root")), Some(1000), Some(profile)),
            PathBuf::from("/custom/root")
        );
        assert_eq!(
            root(Some(OsStr::new("")), Some(1000), None),
            PathBuf::from("/tmp/dxnb-1000")
        );
        assert_eq!(
            root(None, Some(0), Some(profile)),
            PathBuf::from("/tmp/dxnb-0")
        );
        assert_eq!(root(None, None, Some(profile)), profile.join(".dxnb"));
        assert_eq!(
            root(None, None, Some(Path::new(""))),
            std::env::temp_dir().join("dxnb")
        );
        assert_eq!(root(None, None, None), std::env::temp_dir().join("dxnb"));
    }
}
