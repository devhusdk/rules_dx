//! Where nested-Bazel tests keep their servers: one slot directory with `lock`, `u` and `w` per slot.

use std::ffi::OsStr;
use std::path::PathBuf;

/// The environment variable that moves the root.
pub const ROOT_ENV: &str = "DX_NESTED_BAZEL_ROOT";
/// The file in a slot the harness locks while a test uses the slot.
pub const LOCK: &str = "lock";
/// The slot's Bazel output user root.
pub const USER_ROOT: &str = "u";
/// The slot's directory of test case workspaces.
pub const WORKSPACES: &str = "w";

/// The nested-Bazel root: `env` when set and non-empty, else `/tmp/dxnb-<uid>`, else `dxnb` in the temp dir.
pub fn root(env: Option<&OsStr>, uid: Option<u32>) -> PathBuf {
    if let Some(env) = env.filter(|value| !value.is_empty()) {
        return PathBuf::from(env);
    }
    match uid {
        Some(uid) => PathBuf::from(format!("/tmp/dxnb-{uid}")),
        None => std::env::temp_dir().join("dxnb"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_environment_wins_then_the_uid_then_the_temp_dir() {
        assert_eq!(
            root(Some(OsStr::new("/custom/root")), Some(1000)),
            PathBuf::from("/custom/root")
        );
        assert_eq!(
            root(Some(OsStr::new("")), Some(1000)),
            PathBuf::from("/tmp/dxnb-1000")
        );
        assert_eq!(root(None, Some(0)), PathBuf::from("/tmp/dxnb-0"));
        assert_eq!(root(None, None), std::env::temp_dir().join("dxnb"));
    }
}
