use std::fs;
use std::io;
use std::path::Path;
use std::time::Duration;

use dx_atomic_fs::lease::{self, GenerationUse};
use dx_env::{acquire_lock, LockError, DX_DIR_NAME};
use dx_setup::{read_current_pair, GenerationId, SETUPS_DIR_NAME};

use super::inventory::collect_inventory;
use super::planning::{CleanPlan, GenerationView};
use super::records::GenerationKind;
use super::CleanError;

pub const CLEAN_LOCK_TIMEOUT: Duration = Duration::from_secs(10);

fn map_lock_error(error: LockError) -> CleanError {
    match error {
        LockError::Busy { path } => CleanError::Busy { path },
        LockError::LockFailed { path, reason } => CleanError::LockFailed { path, reason },
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CleanOutcome {
    pub removed_setup_records: Vec<String>,
    pub removed_generations: Vec<GenerationView>,
    pub skipped_leased_generations: Vec<GenerationView>,
}

fn lease_use(kind: GenerationKind) -> GenerationUse {
    match kind {
        GenerationKind::Environment => GenerationUse::Environment,
        GenerationKind::Generated => GenerationUse::Generated,
    }
}

pub fn apply_plan(workspace_root: &Path, plan: &CleanPlan) -> Result<CleanOutcome, CleanError> {
    apply_plan_with_timeout(workspace_root, plan, CLEAN_LOCK_TIMEOUT)
}

pub fn apply_plan_with_timeout(
    workspace_root: &Path,
    plan: &CleanPlan,
    timeout: Duration,
) -> Result<CleanOutcome, CleanError> {
    if !workspace_root.is_dir() {
        return Err(CleanError::WorkspaceRoot {
            path: workspace_root.to_path_buf(),
        });
    }
    let dx_dir = workspace_root.join(DX_DIR_NAME);
    fs::create_dir_all(&dx_dir).map_err(|e| CleanError::Install {
        reason: format!("cannot create {}: {e}", dx_dir.display()),
    })?;
    let _lock = acquire_lock(&dx_dir, timeout).map_err(map_lock_error)?;
    let live = collect_inventory(workspace_root, &[], &[])?;
    let live_current = live.current_hex;
    let live_pair = match read_current_pair(workspace_root) {
        Ok(pair) => pair,
        Err(e) => {
            return Err(CleanError::CurrentInvalid {
                reason: e.to_string(),
            });
        }
    };
    let mut outcome = CleanOutcome::default();
    let setups_dir = dx_dir.join(SETUPS_DIR_NAME);
    for hex in &plan.prune_setup_records {
        if GenerationId::new(hex).is_err() {
            continue;
        }
        if live_current.as_deref() == Some(hex.as_str()) {
            continue;
        }
        let record = setups_dir.join(hex);
        match fs::symlink_metadata(&record) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => continue,
            Err(e) => {
                return Err(CleanError::Install {
                    reason: format!("cannot inspect {}: {e}", record.display()),
                });
            }
            Ok(_) => {}
        }
        fs::remove_dir_all(&record).map_err(|e| CleanError::Install {
            reason: format!("cannot prune {}: {e}", record.display()),
        })?;
        outcome.removed_setup_records.push(hex.clone());
    }
    let mut live_referenced: Vec<(GenerationKind, String)> = Vec::new();
    if let Some(pair) = live_pair {
        live_referenced.push((
            GenerationKind::Environment,
            pair.environment.as_str().to_owned(),
        ));
        live_referenced.push((
            GenerationKind::Generated,
            pair.generated.as_str().to_owned(),
        ));
    }
    for generation in &plan.prune_generations {
        if GenerationId::new(&generation.hex).is_err() {
            continue;
        }
        if live_referenced
            .iter()
            .any(|(kind, hex)| *kind == generation.kind && *hex == generation.hex)
        {
            continue;
        }
        let lease = match lease::try_acquire_exclusive(
            &dx_dir,
            lease_use(generation.kind),
            &generation.hex,
        ) {
            Ok(Some(lease)) => lease,
            Ok(None) => {
                outcome.skipped_leased_generations.push(generation.clone());
                continue;
            }
            Err(error) => {
                return Err(CleanError::Install {
                    reason: format!(
                        "cannot probe the lease for .dx/{}/{}: {error}",
                        generation.kind.dir_name(),
                        generation.hex
                    ),
                });
            }
        };
        let dir = dx_dir
            .join(generation.kind.dir_name())
            .join(&generation.hex);
        match fs::symlink_metadata(&dir) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => continue,
            Err(e) => {
                return Err(CleanError::Install {
                    reason: format!("cannot inspect {}: {e}", dir.display()),
                });
            }
            Ok(_) => {}
        }
        fs::remove_dir_all(&dir).map_err(|e| CleanError::Install {
            reason: format!("cannot prune {}: {e}", dir.display()),
        })?;
        drop(lease);
        outcome.removed_generations.push(generation.clone());
    }
    outcome.removed_setup_records.sort();
    outcome.removed_generations.sort_by(|left, right| {
        left.kind
            .dir_name()
            .cmp(right.kind.dir_name())
            .then_with(|| left.hex.cmp(&right.hex))
    });
    outcome.skipped_leased_generations.sort_by(|left, right| {
        left.kind
            .dir_name()
            .cmp(right.kind.dir_name())
            .then_with(|| left.hex.cmp(&right.hex))
    });
    Ok(outcome)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures::*;
    use crate::inventory::collect_inventory;
    use crate::planning::{plan_prune, PruneInputs, UnobservedPolicy};
    use std::path::PathBuf;

    #[test]
    fn clean_lock_deadline_matches_env_owner() {
        assert_eq!(CLEAN_LOCK_TIMEOUT, dx_env::LOCK_TIMEOUT);
    }

    #[test]
    fn empty_workspace_collects_nothing() {
        let scratch = {
            let __scratch = dx_test_scratch::scratch("dx-clean-test-empty-");
            std::fs::create_dir_all(__scratch.path().join("ws")).expect("create workspace");
            __scratch
        };
        let root = scratch.path().to_path_buf();
        let workspace = workspace_of(&root);
        let inventory = collect_inventory(&workspace, &[], &[]).expect("collect");
        assert!(inventory.records.is_empty());
        assert!(inventory.generations.is_empty());
        assert_eq!(inventory.current_hex, None);
        assert!(inventory.unmanaged_names.is_empty());
        let plan = inventory.plan();
        assert!(plan.prune_setup_records.is_empty());
        assert!(plan.prune_generations.is_empty());
        let outcome = apply_plan(&workspace, &plan).expect("apply empty");
        assert_eq!(outcome, CleanOutcome::default());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn malformed_current_fails_closed() {
        let scratch = {
            let __scratch = dx_test_scratch::scratch("dx-clean-test-bad-current-");
            std::fs::create_dir_all(__scratch.path().join("ws")).expect("create workspace");
            __scratch
        };
        let root = scratch.path().to_path_buf();
        let workspace = workspace_of(&root);
        dx_setup::commit_pair(&workspace, &setup_pair('1', '2')).expect("commit");
        let current = workspace.join(".dx").join("setups").join("current");
        dx_test_scratch::remove_directory_link(&current).expect("remove pointer");
        fs::write(&current, "not a symlink").expect("file pointer");
        assert!(matches!(
            collect_inventory(&workspace, &[], &[]),
            Err(CleanError::CurrentInvalid { .. })
        ));
        let plan = CleanPlan {
            prune_setup_records: Vec::new(),
            prune_generations: Vec::new(),
            refused_unmanaged: Vec::new(),
            preserved_current: None,
            observation_unknown: false,
            preserved_unobserved_setup_records: Vec::new(),
            preserved_unobserved_generations: Vec::new(),
        };
        assert!(matches!(
            apply_plan(&workspace, &plan),
            Err(CleanError::CurrentInvalid { .. })
        ));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn apply_removes_prune_set_and_preserves_current() {
        let scratch = {
            let __scratch = dx_test_scratch::scratch("dx-clean-test-apply-");
            std::fs::create_dir_all(__scratch.path().join("ws")).expect("create workspace");
            __scratch
        };
        let root = scratch.path().to_path_buf();
        let (workspace, stale_hex, current_hex) = two_record_workspace(&root);
        let inventory = collect_inventory(&workspace, &[], &[]).expect("collect");
        let plan = inventory.plan();
        assert_eq!(plan.prune_setup_records, vec![stale_hex.clone()]);
        let mut pruned_hexes: Vec<String> = plan
            .prune_generations
            .iter()
            .map(|generation| generation.hex.clone())
            .collect();
        pruned_hexes.sort();
        assert_eq!(pruned_hexes, vec![digest('3'), digest('4')]);
        let outcome = apply_plan(&workspace, &plan).expect("apply");
        assert_eq!(outcome.removed_setup_records, vec![stale_hex.clone()]);
        assert_eq!(outcome.removed_generations.len(), 2);
        let setups = workspace.join(".dx").join("setups");
        assert!(setups.join(&current_hex).join("environment").is_symlink());
        assert!(workspace
            .join(".dx")
            .join("environments")
            .join(digest('1'))
            .is_dir());
        assert!(workspace
            .join(".dx")
            .join("generated")
            .join(digest('2'))
            .is_dir());
        assert!(!setups.join(&stale_hex).exists());
        assert_eq!(
            dx_setup::read_current_pair(&workspace)
                .expect("read")
                .map(|pair| dx_setup::setup_hex(&pair)),
            Some(current_hex)
        );
        let again = apply_plan(&workspace, &plan).expect("re-apply");
        assert_eq!(again, CleanOutcome::default());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn apply_skips_entries_that_became_current() {
        let scratch = {
            let __scratch = dx_test_scratch::scratch("dx-clean-test-race-");
            std::fs::create_dir_all(__scratch.path().join("ws")).expect("create workspace");
            __scratch
        };
        let root = scratch.path().to_path_buf();
        let workspace = workspace_of(&root);
        let first = setup_pair('1', '2');
        let second = setup_pair('3', '4');
        dx_setup::commit_pair(&workspace, &first).expect("commit first");
        dx_setup::commit_pair(&workspace, &second).expect("commit second");
        let plan = collect_inventory(&workspace, &[], &[])
            .expect("collect")
            .plan();
        assert_eq!(plan.prune_setup_records, vec![dx_setup::setup_hex(&first)]);
        dx_setup::commit_pair(&workspace, &first).expect("reselect first");
        let outcome = apply_plan(&workspace, &plan).expect("apply");
        assert!(outcome.removed_setup_records.is_empty());
        assert_eq!(
            dx_setup::read_current_pair(&workspace).expect("read"),
            Some(first)
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn apply_skips_generations_referenced_by_live_current() {
        let scratch = {
            let __scratch = dx_test_scratch::scratch("dx-clean-test-race-gen-");
            std::fs::create_dir_all(__scratch.path().join("ws")).expect("create workspace");
            __scratch
        };
        let root = scratch.path().to_path_buf();
        let (workspace, stale_hex, _) = two_record_workspace(&root);
        let stale_record = record('3', '4');
        let plan = plan_prune(PruneInputs {
            records: &[stale_record],
            generations: &[
                generation(GenerationKind::Environment, '1'),
                generation(GenerationKind::Generated, '2'),
            ],
            current_hex: None,
            active_setup_hexes: &[],
            active_generation_hexes: &[],
            unmanaged_names: &[],
            observation_unknown: false,
            unobserved: UnobservedPolicy::Preserve,
        });
        assert_eq!(plan.prune_setup_records, vec![stale_hex.clone()]);
        assert_eq!(plan.prune_generations.len(), 2);
        let outcome = apply_plan(&workspace, &plan).expect("apply");
        assert_eq!(outcome.removed_setup_records, vec![stale_hex]);
        assert!(outcome.removed_generations.is_empty());
        assert!(workspace
            .join(".dx")
            .join("environments")
            .join(digest('1'))
            .is_dir());
        let _ = fs::remove_dir_all(&root);
    }

    fn wait_for(path: &Path, what: &str) {
        let start = std::time::Instant::now();
        while !path.exists() {
            if start.elapsed() > Duration::from_secs(30) {
                panic!("timed out waiting for {what}");
            }
            std::thread::sleep(Duration::from_millis(25));
        }
    }

    #[test]
    fn prune_cannot_remove_a_generation_leased_by_another_process() {
        if std::env::var_os("DX_CLEAN_LEASE_CHILD").is_some() {
            let workspace = std::env::var_os("DX_CLEAN_LEASE_ROOT")
                .map(PathBuf::from)
                .expect("child workspace");
            let hex = std::env::var_os("DX_CLEAN_LEASE_HEX")
                .map(|hex| hex.into_string().expect("child hex"))
                .expect("child hex");
            let dx_dir = workspace.join(".dx");
            let _lease = dx_atomic_fs::lease::acquire_shared(
                &dx_dir,
                dx_atomic_fs::lease::GenerationUse::Generated,
                &hex,
                Duration::from_secs(30),
            )
            .expect("child leases its generation");
            fs::write(workspace.join("child-ready"), b"ready").expect("signal ready");
            wait_for(&workspace.join("child-release"), "release");
            return;
        }
        let scratch = {
            let __scratch = dx_test_scratch::scratch("dx-clean-test-lease-race-");
            std::fs::create_dir_all(__scratch.path().join("ws")).expect("create workspace");
            __scratch
        };
        let root = scratch.path().to_path_buf();
        let (workspace, stale_hex, _) = two_record_workspace(&root);
        let stale_gen = digest('4');
        let stale_dir = workspace.join(".dx").join("generated").join(&stale_gen);
        assert!(stale_dir.is_dir(), "the stale generation exists before pruning");
        let mut child = std::process::Command::new(std::env::current_exe().expect("test binary"))
            .arg("apply::tests::prune_cannot_remove_a_generation_leased_by_another_process")
            .arg("--exact")
            .env("DX_CLEAN_LEASE_CHILD", "1")
            .env("DX_CLEAN_LEASE_ROOT", &workspace)
            .env("DX_CLEAN_LEASE_HEX", &stale_gen)
            .spawn()
            .expect("spawn the leasing reader");
        wait_for(&workspace.join("child-ready"), "child ready");
        let plan = collect_inventory(&workspace, &[], &[]).expect("collect");
        let plan = plan.plan();
        assert!(
            plan.prune_generations
                .iter()
                .any(|generation| generation.hex == stale_gen),
            "the plan names the stale generation before the lease is consulted"
        );
        let outcome = apply_plan(&workspace, &plan).expect("apply");
        assert_eq!(outcome.removed_setup_records, vec![stale_hex]);
        assert!(outcome.removed_generations.is_empty());
        assert_eq!(outcome.skipped_leased_generations.len(), 1);
        assert!(stale_dir.is_dir(), "the leased generation survives pruning");
        fs::write(workspace.join("child-release"), b"go").expect("release the reader");
        let status = child.wait().expect("reap the reader");
        assert!(status.success(), "the reader exits cleanly");
        let outcome = apply_plan(&workspace, &plan).expect("re-apply");
        assert!(outcome.skipped_leased_generations.is_empty());
        assert_eq!(outcome.removed_generations.len(), 2);
        assert!(!stale_dir.exists(), "the released generation prunes");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn busy_lock_fails_after_deadline() {
        let scratch = {
            let __scratch = dx_test_scratch::scratch("dx-clean-test-busy-");
            std::fs::create_dir_all(__scratch.path().join("ws")).expect("create workspace");
            __scratch
        };
        let root = scratch.path().to_path_buf();
        let workspace = workspace_of(&root);
        let dx_dir = workspace.join(".dx");
        fs::create_dir_all(&dx_dir).expect("dx dir");
        let _held = acquire_lock(&dx_dir, Duration::from_secs(10)).expect("hold commit lock");
        let plan = CleanPlan {
            prune_setup_records: Vec::new(),
            prune_generations: Vec::new(),
            refused_unmanaged: Vec::new(),
            preserved_current: None,
            observation_unknown: false,
            preserved_unobserved_setup_records: Vec::new(),
            preserved_unobserved_generations: Vec::new(),
        };
        let error =
            apply_plan_with_timeout(&workspace, &plan, Duration::from_millis(1)).unwrap_err();
        assert!(matches!(error, CleanError::Busy { .. }));
        drop(_held);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn workspace_missing_fails() {
        let scratch = {
            let __scratch = dx_test_scratch::scratch("dx-clean-test-ws-missing-");
            std::fs::create_dir_all(__scratch.path().join("ws")).expect("create workspace");
            __scratch
        };
        let root = scratch.path().to_path_buf();
        let missing = root.join("no-such-dir");
        assert!(matches!(
            collect_inventory(&missing, &[], &[]),
            Err(CleanError::WorkspaceRoot { .. })
        ));
        let plan = CleanPlan {
            prune_setup_records: Vec::new(),
            prune_generations: Vec::new(),
            refused_unmanaged: Vec::new(),
            preserved_current: None,
            observation_unknown: false,
            preserved_unobserved_setup_records: Vec::new(),
            preserved_unobserved_generations: Vec::new(),
        };
        assert!(matches!(
            apply_plan(&missing, &plan),
            Err(CleanError::WorkspaceRoot { .. })
        ));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn clean_errors_display() {
        let errors = [
            CleanError::WorkspaceRoot {
                path: PathBuf::from("/ws"),
            },
            CleanError::Busy {
                path: PathBuf::from("/ws/.dx/.commit.lock"),
            },
            CleanError::LockFailed {
                path: PathBuf::from("/ws/.dx/.commit.lock"),
                reason: "r".to_string(),
            },
            CleanError::CurrentInvalid {
                reason: "r".to_string(),
            },
            CleanError::Install {
                reason: "r".to_string(),
            },
        ];
        for error in &errors {
            assert!(!format!("{error}").is_empty());
        }
    }
}
