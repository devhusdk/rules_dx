use std::fs;
use std::io;
use std::path::Path;
use std::time::Duration;

use dx_env::{acquire_lock, LockError, DX_DIR_NAME};
use dx_setup::{read_current_pair, GenerationId, SETUPS_DIR_NAME};

use super::inventory::collect_inventory;
use super::leases::{remove_lease_file, try_exclusive_lease, GenerationLease};
use super::live::scan_live_hexes;
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
    pub skipped_leased: Vec<GenerationView>,
    pub skipped_active: Vec<GenerationView>,
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
    let mut held_leases: Vec<GenerationLease> = Vec::new();
    let apply_observation = scan_live_hexes(Path::new("/proc"), &dx_dir);
    let apply_active: Vec<String> = apply_observation
        .live
        .generations
        .iter()
        .map(|view| view.hex.clone())
        .collect();
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
        if apply_active.iter().any(|hex| hex == &generation.hex) {
            outcome.skipped_active.push(generation.clone());
            continue;
        }
        let lease = match try_exclusive_lease(&dx_dir, generation.kind, &generation.hex) {
            Ok(Some(lease)) => lease,
            Ok(None) => {
                outcome.skipped_leased.push(generation.clone());
                continue;
            }
            Err(e) => return Err(e),
        };
        let dir = dx_dir
            .join(generation.kind.dir_name())
            .join(&generation.hex);
        match fs::symlink_metadata(&dir) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                remove_lease_file(&dx_dir, generation.kind, &generation.hex);
                continue;
            }
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
        remove_lease_file(&dx_dir, generation.kind, &generation.hex);
        held_leases.push(lease);
        outcome.removed_generations.push(generation.clone());
    }
    outcome.removed_setup_records.sort();
    outcome.removed_generations.sort_by(|left, right| {
        left.kind
            .dir_name()
            .cmp(right.kind.dir_name())
            .then_with(|| left.hex.cmp(&right.hex))
    });
    for skipped in [&mut outcome.skipped_leased, &mut outcome.skipped_active] {
        skipped.sort_by(|left, right| {
            left.kind
                .dir_name()
                .cmp(right.kind.dir_name())
                .then_with(|| left.hex.cmp(&right.hex))
        });
    }
    Ok(outcome)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures::*;
    use crate::inventory::collect_inventory;
    use crate::leases::acquire_shared_lease;
    use crate::live::ObservationStatus;
    use crate::planning::{plan_prune, PruneInputs};
    use std::path::PathBuf;
    use std::process::Command;
    use std::time::Instant;

    const HOLDER_FILTER: &str = "DX_CLEAN_HOLDER_FILTER";
    const HOLDER_DXDIR: &str = "DX_CLEAN_HOLDER_DXDIR";
    const HOLDER_KIND: &str = "DX_CLEAN_HOLDER_KIND";
    const HOLDER_HEX: &str = "DX_CLEAN_HOLDER_HEX";
    const HOLDER_READY: &str = "DX_CLEAN_HOLDER_READY";
    const HOLDER_RELEASE: &str = "DX_CLEAN_HOLDER_RELEASE";
    const HOLDER_MODE: &str = "DX_CLEAN_HOLDER_MODE";
    const HOLDER_SITDIR: &str = "DX_CLEAN_HOLDER_SITDIR";
    const HOLDER_CHILD_READY: &str = "DX_CLEAN_HOLDER_CHILD_READY";

    const HOLDER_READY_WAIT: Duration = Duration::from_secs(30);
    const HOLDER_RELEASE_CAP: Duration = Duration::from_secs(60);
    const HOLDER_POLL: Duration = Duration::from_millis(50);

    fn test_filter(test_name: &str) -> String {
        let path = module_path!();
        let relative = path.split_once("::").map(|(_, rest)| rest).unwrap_or(path);
        format!("{relative}::{test_name}")
    }

    fn wait_for(path: &Path, present: bool, timeout: Duration) {
        let start = Instant::now();
        while path.exists() != present {
            assert!(
                start.elapsed() < timeout,
                "timed out waiting for {} {}",
                path.display(),
                if present { "to appear" } else { "to disappear" }
            );
            std::thread::sleep(HOLDER_POLL);
        }
    }

    fn spawn_holder(
        test_name: &str,
        dx_dir: &Path,
        kind: GenerationKind,
        hex: &str,
        ready: &Path,
        release: &Path,
        mode: &str,
        sit_dir: Option<&Path>,
        child_ready: Option<&Path>,
    ) -> std::process::Child {
        let mut command = Command::new(std::env::current_exe().expect("test binary"));
        command
            .arg(test_filter(test_name))
            .arg("--exact")
            .env(HOLDER_FILTER, test_filter(test_name))
            .env(HOLDER_DXDIR, dx_dir)
            .env(
                HOLDER_KIND,
                match kind {
                    GenerationKind::Environment => "environments",
                    GenerationKind::Generated => "generated",
                },
            )
            .env(HOLDER_HEX, hex)
            .env(HOLDER_READY, ready)
            .env(HOLDER_RELEASE, release)
            .env(HOLDER_MODE, mode);
        if let Some(dir) = sit_dir {
            command.env(HOLDER_SITDIR, dir);
        }
        if let Some(ready) = child_ready {
            command.env(HOLDER_CHILD_READY, ready);
        }
        command
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .expect("spawn holder child")
    }

    fn holder_kind() -> GenerationKind {
        match std::env::var(HOLDER_KIND).expect("holder kind").as_str() {
            "environments" => GenerationKind::Environment,
            "generated" => GenerationKind::Generated,
            other => panic!("unknown holder kind {other}"),
        }
    }

    fn run_holder_child() -> bool {
        if std::env::var_os(HOLDER_FILTER).is_none() {
            return false;
        }
        let dx_dir = PathBuf::from(std::env::var(HOLDER_DXDIR).expect("holder dx dir"));
        let kind = holder_kind();
        let hex = std::env::var(HOLDER_HEX).expect("holder hex");
        let ready = PathBuf::from(std::env::var(HOLDER_READY).expect("holder ready"));
        let release = PathBuf::from(std::env::var(HOLDER_RELEASE).expect("holder release"));
        let mode = std::env::var(HOLDER_MODE).expect("holder mode");
        let _lease = if mode == "sit" {
            let sit = PathBuf::from(std::env::var(HOLDER_SITDIR).expect("holder sit dir"));
            std::env::set_current_dir(&sit).expect("holder sits in generation");
            None
        } else {
            let lease =
                acquire_shared_lease(&dx_dir, kind, &hex, HOLDER_READY_WAIT).expect("holder lease");
            if mode == "lease-spawn" {
                let child_ready =
                    PathBuf::from(std::env::var(HOLDER_CHILD_READY).expect("child ready"));
                let filter = std::env::var(HOLDER_FILTER).expect("holder filter");
                let child = Command::new(std::env::current_exe().expect("test binary"))
                    .arg(&filter)
                    .arg("--exact")
                    .env(HOLDER_FILTER, &filter)
                    .env(HOLDER_DXDIR, &dx_dir)
                    .env(HOLDER_KIND, std::env::var(HOLDER_KIND).expect("kind"))
                    .env(HOLDER_HEX, &hex)
                    .env(HOLDER_READY, &child_ready)
                    .env(HOLDER_RELEASE, &release)
                    .env(HOLDER_MODE, "lease")
                    .stdin(std::process::Stdio::null())
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .spawn()
                    .expect("spawn holder grandchild");
                drop(child);
                wait_for(&child_ready, true, HOLDER_READY_WAIT);
            }
            Some(lease)
        };
        fs::write(&ready, b"held\n").expect("holder ready");
        wait_for(&release, true, HOLDER_RELEASE_CAP);
        let _ = fs::remove_file(&ready);
        drop(_lease);
        true
    }

    fn assert_single_test_ran(output: &std::process::Output, name: &str) {
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            stdout.contains("1 passed"),
            "{name} child must run exactly one test: {stdout}"
        );
    }

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
            observation: crate::live::ObservationStatus::Known,
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
            observation: crate::live::ObservationStatus::Known,
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
            observation: crate::live::ObservationStatus::Known,
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
            observation: crate::live::ObservationStatus::Known,
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

    fn race_workspace(root: &Path) -> (PathBuf, PathBuf) {
        fs::create_dir_all(root.join("ws")).expect("create workspace");
        let (workspace, _, _) = two_record_workspace(root);
        let plan = collect_inventory(&workspace, &[], &[])
            .expect("collect")
            .plan();
        assert_eq!(plan.prune_generations.len(), 2);
        (workspace.clone(), workspace.join(".dx"))
    }

    #[test]
    fn leased_generation_survives_prune_and_frees_on_exit() {
        if run_holder_child() {
            return;
        }
        let scratch = dx_test_scratch::scratch("dx-clean-test-leased-");
        let root = scratch.path().to_path_buf();
        let (workspace, dx_dir) = race_workspace(&root);
        let hex = digest('3');
        let ready = root.join("holder.ready");
        let release = root.join("holder.release");
        let holder = spawn_holder(
            "leased_generation_survives_prune_and_frees_on_exit",
            &dx_dir,
            GenerationKind::Environment,
            &hex,
            &ready,
            &release,
            "lease",
            None,
            None,
        );
        wait_for(&ready, true, HOLDER_READY_WAIT);
        let plan = collect_inventory(&workspace, &[], &[])
            .expect("collect")
            .plan();
        let outcome = apply_plan(&workspace, &plan).expect("apply under lease");
        assert_eq!(
            outcome.skipped_leased,
            vec![GenerationView {
                kind: GenerationKind::Environment,
                hex: hex.clone(),
            }]
        );
        assert!(
            dx_dir.join("environments").join(&hex).is_dir(),
            "leased generation survives prune"
        );
        assert!(
            !dx_dir.join("generated").join(digest('4')).exists(),
            "unleased stale generation is still pruned"
        );
        fs::write(&release, b"go\n").expect("release holder");
        let output = holder.wait_with_output().expect("reap holder");
        assert!(output.status.success());
        assert_single_test_ran(
            &output,
            "leased_generation_survives_prune_and_frees_on_exit",
        );
        let again = apply_plan(&workspace, &plan).expect("apply after release");
        assert!(again.skipped_leased.is_empty());
        assert!(
            !dx_dir.join("environments").join(&hex).exists(),
            "released generation prunes on the next apply"
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn crashed_holder_releases_lease() {
        if run_holder_child() {
            return;
        }
        let scratch = dx_test_scratch::scratch("dx-clean-test-crash-");
        let root = scratch.path().to_path_buf();
        let (workspace, dx_dir) = race_workspace(&root);
        let hex = digest('3');
        let ready = root.join("holder.ready");
        let release = root.join("holder.release");
        let mut holder = spawn_holder(
            "crashed_holder_releases_lease",
            &dx_dir,
            GenerationKind::Environment,
            &hex,
            &ready,
            &release,
            "lease",
            None,
            None,
        );
        wait_for(&ready, true, HOLDER_READY_WAIT);
        holder.kill().expect("crash the holder");
        let status = holder.wait().expect("reap crashed holder");
        assert!(!status.success(), "killed holder must not exit zero");
        assert!(
            try_exclusive_lease(&dx_dir, GenerationKind::Environment, &hex)
                .expect("exclusive attempt")
                .is_some(),
            "crashed holder must release its lease"
        );
        let plan = collect_inventory(&workspace, &[], &[])
            .expect("collect")
            .plan();
        let outcome = apply_plan(&workspace, &plan).expect("apply after crash");
        assert!(outcome.skipped_leased.is_empty());
        assert!(
            !dx_dir.join("environments").join(&hex).exists(),
            "generation prunes after the crash release"
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn surviving_child_blocks_prune_after_parent_crash() {
        if run_holder_child() {
            return;
        }
        let scratch = dx_test_scratch::scratch("dx-clean-test-child-");
        let root = scratch.path().to_path_buf();
        let (workspace, dx_dir) = race_workspace(&root);
        let hex = digest('4');
        let ready = root.join("parent.ready");
        let release = root.join("holder.release");
        let child_ready = root.join("child.ready");
        let mut parent = spawn_holder(
            "surviving_child_blocks_prune_after_parent_crash",
            &dx_dir,
            GenerationKind::Generated,
            &hex,
            &ready,
            &release,
            "lease-spawn",
            None,
            Some(&child_ready),
        );
        wait_for(&child_ready, true, HOLDER_READY_WAIT);
        parent.kill().expect("crash the parent holder");
        let status = parent.wait().expect("reap crashed parent");
        assert!(!status.success(), "killed parent must not exit zero");
        let plan = collect_inventory(&workspace, &[], &[])
            .expect("collect")
            .plan();
        let outcome = apply_plan(&workspace, &plan).expect("apply after parent crash");
        assert_eq!(
            outcome.skipped_leased,
            vec![GenerationView {
                kind: GenerationKind::Generated,
                hex: hex.clone(),
            }]
        );
        assert!(
            dx_dir.join("generated").join(&hex).is_dir(),
            "surviving child keeps the generation leased"
        );
        fs::write(&release, b"go\n").expect("release surviving child");
        wait_for(&child_ready, false, HOLDER_READY_WAIT);
        let again = apply_plan(&workspace, &plan).expect("apply after child exit");
        assert!(again.skipped_leased.is_empty());
        assert!(
            !dx_dir.join("generated").join(&hex).exists(),
            "generation prunes once the child exits"
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn shared_acquire_succeeds_while_commit_lock_held() {
        if run_holder_child() {
            return;
        }
        let scratch = dx_test_scratch::scratch("dx-clean-test-acquire-");
        let root = scratch.path().to_path_buf();
        let (workspace, dx_dir) = race_workspace(&root);
        let coordination =
            acquire_lock(&dx_dir, Duration::from_secs(10)).expect("hold coordination lock");
        let hex = digest('3');
        let ready = root.join("holder.ready");
        let release = root.join("holder.release");
        let holder = spawn_holder(
            "shared_acquire_succeeds_while_commit_lock_held",
            &dx_dir,
            GenerationKind::Environment,
            &hex,
            &ready,
            &release,
            "lease",
            None,
            None,
        );
        wait_for(&ready, true, HOLDER_READY_WAIT);
        drop(coordination);
        let plan = collect_inventory(&workspace, &[], &[])
            .expect("collect")
            .plan();
        let outcome = apply_plan(&workspace, &plan).expect("apply");
        assert_eq!(outcome.skipped_leased.len(), 1);
        assert!(
            dx_dir.join("environments").join(&hex).is_dir(),
            "lease acquired during cleanup still protects"
        );
        fs::write(&release, b"go\n").expect("release holder");
        let output = holder.wait_with_output().expect("reap holder");
        assert!(output.status.success());
        assert_single_test_ran(&output, "shared_acquire_succeeds_while_commit_lock_held");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn process_sitting_in_generation_is_skipped_when_observed() {
        if run_holder_child() {
            return;
        }
        let scratch = dx_test_scratch::scratch("dx-clean-test-sit-");
        let root = scratch.path().to_path_buf();
        let (workspace, dx_dir) = race_workspace(&root);
        let hex = digest('4');
        let sit_dir = dx_dir.join("generated").join(&hex);
        let ready = root.join("sitter.ready");
        let release = root.join("sitter.release");
        let sitter = spawn_holder(
            "process_sitting_in_generation_is_skipped_when_observed",
            &dx_dir,
            GenerationKind::Generated,
            &hex,
            &ready,
            &release,
            "sit",
            Some(&sit_dir),
            None,
        );
        wait_for(&ready, true, HOLDER_READY_WAIT);
        let observed =
            scan_live_hexes(Path::new("/proc"), &dx_dir).status == ObservationStatus::Known;
        let plan = collect_inventory(&workspace, &[], &[])
            .expect("collect")
            .plan();
        let outcome = apply_plan(&workspace, &plan).expect("apply with sitter");
        if observed {
            assert_eq!(
                outcome.skipped_active,
                vec![GenerationView {
                    kind: GenerationKind::Generated,
                    hex: hex.clone(),
                }]
            );
            assert!(sit_dir.is_dir(), "observed live generation survives prune");
        } else {
            assert!(
                !sit_dir.exists(),
                "without process observation the sitter is invisible and the prune proceeds"
            );
        }
        fs::write(&release, b"go\n").expect("release sitter");
        let output = sitter.wait_with_output().expect("reap sitter");
        assert!(output.status.success());
        assert_single_test_ran(
            &output,
            "process_sitting_in_generation_is_skipped_when_observed",
        );
        let _ = fs::remove_dir_all(&root);
    }
}
