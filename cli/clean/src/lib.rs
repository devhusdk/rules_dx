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
pub mod hold;
pub mod inventory;
pub mod live;
pub mod planning;
pub mod records;

pub use apply::{CLEAN_LOCK_TIMEOUT, CleanOutcome, apply_plan, apply_plan_with_timeout};

pub use bytes::{PruneBytes, measure_preserved_bytes, measure_prune_bytes, render_prune_listing};
pub use flags::PRUNE_UNOBSERVED_FLAG;
pub use hold::{HeldSelection, HoldError, hold_current};
pub use inventory::{CollectedInventory, collect_inventory, walk_filtered};
pub use live::{
    LiveHexes, LiveObservation, collect_inventory_with_scan_from, scan_live_observation,
};
pub use planning::{CleanPlan, GenerationView, PruneInputs, UnobservedPolicy, plan_prune};
pub use records::{RecordProblem, SetupRecordView, validate_record};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CleanError {
    #[error("invalid workspace root {path:?}: missing or not a directory", path = path.display())]
    WorkspaceRoot { path: PathBuf },
    #[error("{}", dx_atomic_fs::commit::busy_message(path, holder.as_deref()))]
    Busy {
        path: PathBuf,
        holder: Option<String>,
    },
    #[error("cannot lock {path:?}: {reason}", path = path.display())]
    LockFailed { path: PathBuf, reason: String },
    #[error("invalid current selection: {reason}")]
    CurrentInvalid { reason: String },
    #[error("install failed: {reason}")]
    Install { reason: String },
}

#[cfg(test)]
mod fixtures;
