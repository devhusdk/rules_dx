#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

use std::path::PathBuf;

pub mod apply;
pub mod bytes;
pub mod flags;
pub mod inventory;
pub mod leases;
pub mod live;
pub mod planning;
pub mod records;

pub use apply::{apply_plan, apply_plan_with_timeout, CleanOutcome, CLEAN_LOCK_TIMEOUT};

pub use bytes::{measure_prune_bytes, render_dry_run, PruneBytes};
pub use flags::{bazel_forward_argv, BAZEL_FLAG, DRY_RUN_FLAG, RECOVERY_GUIDANCE};
pub use inventory::{collect_inventory, walk_filtered, CollectedInventory};
pub use leases::{
    acquire_shared_lease, lease_path, remove_lease_file, try_exclusive_lease, GenerationLease,
    LEASES_DIR_NAME, LEASE_TIMEOUT,
};
pub use live::{
    collect_inventory_with_scan, scan_live_hexes, LiveHexes, LiveObservation, ObservationStatus,
};
pub use planning::{plan_prune, CleanPlan, GenerationView, PruneInputs};
pub use records::{validate_record, GenerationKind, RecordProblem, SetupRecordView};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CleanError {
    #[error("invalid workspace root {path:?}: missing or not a directory", path = path.display())]
    WorkspaceRoot { path: PathBuf },
    #[error("workspace busy at {path:?}: another command holds the commit lock", path = path.display())]
    Busy { path: PathBuf },
    #[error("cannot lock {path:?}: {reason}", path = path.display())]
    LockFailed { path: PathBuf, reason: String },
    #[error("invalid current selection: {reason}")]
    CurrentInvalid { reason: String },
    #[error("install failed: {reason}")]
    Install { reason: String },
}

#[cfg(test)]
mod fixtures;
