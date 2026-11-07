//! Where dx keeps its state inside a workspace.
//!
//! `.dx/` holds committed files. Machine-local state lives under
//! `.dx/local/`, which stays out of git.

use std::path::{Path, PathBuf};

pub const DX_DIR: &str = ".dx";
pub const LOCAL_DIR: &str = ".dx/local";

pub const SETUPS: &str = "setups";
pub const CURRENT: &str = "current";
pub const CURRENT_NEXT: &str = "current.next";
pub const ENVIRONMENTS: &str = "environments";
pub const GENERATED: &str = "generated";
pub const BIN: &str = "bin";
pub const BIN_NEXT: &str = "bin.next";
pub const BIN_PREV: &str = "bin.prev";
pub const LEASES: &str = "leases";
pub const COMMIT_LOCK: &str = ".commit.lock";
pub const COMMIT_LOCK_HOLDER: &str = ".commit.lock.holder";
pub const RESULTS: &str = "results.json";
pub const DOCS: &str = "docs";
pub const TIMINGS: &str = "timings";
pub const FUZZ: &str = "fuzz";
pub const BENCH: &str = "bench";
pub const ADVISORY: &str = "advisory";
pub const IDE: &str = "ide";
pub const LAUNCHER: &str = "launcher";
pub const FINGERPRINT: &str = "fingerprint";
pub const SYNC_LOCK: &str = "sync.lock";

/// Entries that live under [`local_dir`].
pub const LOCAL_ENTRIES: [&str; 12] = [
    SETUPS,
    CURRENT,
    CURRENT_NEXT,
    ENVIRONMENTS,
    GENERATED,
    BIN,
    BIN_NEXT,
    BIN_PREV,
    LEASES,
    COMMIT_LOCK,
    COMMIT_LOCK_HOLDER,
    SYNC_LOCK,
];

pub fn dx_dir(workspace: &Path) -> PathBuf {
    workspace.join(DX_DIR)
}

pub fn local_dir(workspace: &Path) -> PathBuf {
    dx_dir(workspace).join("local")
}

/// The inputs and outputs hash from the last `.dx` sync.
pub fn fingerprint_path(workspace: &Path) -> PathBuf {
    local_dir(workspace).join(FINGERPRINT)
}

/// Held while a `.dx` sync renders and cleans up.
pub fn sync_lock_path(workspace: &Path) -> PathBuf {
    local_dir(workspace).join(SYNC_LOCK)
}

pub fn ide_dir(workspace: &Path) -> PathBuf {
    local_dir(workspace).join(IDE)
}

pub fn launcher_dir(workspace: &Path) -> PathBuf {
    local_dir(workspace).join(LAUNCHER)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dx_dir_is_one_component_under_the_workspace() {
        let ws = Path::new("ws");
        assert_eq!(dx_dir(ws), ws.join(".dx"));
    }

    #[test]
    fn local_dir_joins_components_one_at_a_time() {
        let ws = Path::new("ws");
        let dir = local_dir(ws);
        let tail: Vec<_> = dir.components().rev().take(2).collect();
        assert_eq!(tail[0].as_os_str(), "local");
        assert_eq!(tail[1].as_os_str(), ".dx");
        assert!(dir.starts_with(ws));
        assert_eq!(LOCAL_DIR, format!("{DX_DIR}/local"));
    }

    #[test]
    fn ide_and_launcher_dirs_live_under_local() {
        let ws = Path::new("ws");
        assert_eq!(ide_dir(ws), local_dir(ws).join("ide"));
        assert_eq!(launcher_dir(ws), local_dir(ws).join("launcher"));
        assert!(!LOCAL_ENTRIES.contains(&IDE));
        assert!(!LOCAL_ENTRIES.contains(&LAUNCHER));
    }
}
