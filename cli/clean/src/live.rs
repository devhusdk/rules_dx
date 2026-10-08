use std::fs;
use std::path::{Path, PathBuf};

use dx_atomic_fs::lease::{self, GenerationUse};
use dx_env::DX_DIR_NAME;
use dx_setup::{GenerationId, ENVIRONMENTS_DIR_NAME, GENERATED_DIR_NAME, SETUPS_DIR_NAME};

use super::inventory::{collect_inventory, CollectedInventory};
use super::planning::{GenerationView, UnobservedPolicy};
use super::records::GenerationKind;
use super::CleanError;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LiveHexes {
    pub setup: Vec<String>,
    pub generations: Vec<GenerationView>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LiveObservation {
    Known(LiveHexes),
    Unknown { reason: String },
}

fn classify_managed_path(
    dx_dir: &Path,
    observed: &Path,
) -> (Option<String>, Option<GenerationView>) {
    let Ok(relative) = observed.strip_prefix(dx_dir) else {
        return (None, None);
    };
    let mut components = relative.components();
    let (Some(root), Some(name)) = (components.next(), components.next()) else {
        return (None, None);
    };
    let (Some(root), Some(name)) = (root.as_os_str().to_str(), name.as_os_str().to_str()) else {
        return (None, None);
    };
    if GenerationId::new(name).is_err() {
        return (None, None);
    }
    if root == SETUPS_DIR_NAME {
        (Some(name.to_owned()), None)
    } else if root == ENVIRONMENTS_DIR_NAME {
        (
            None,
            Some(GenerationView {
                kind: GenerationKind::Environment,
                hex: name.to_owned(),
            }),
        )
    } else if root == GENERATED_DIR_NAME {
        (
            None,
            Some(GenerationView {
                kind: GenerationKind::Generated,
                hex: name.to_owned(),
            }),
        )
    } else {
        (None, None)
    }
}

fn observe_process(dir: &Path, dx_dir: &Path, live: &mut LiveHexes) {
    let mut targets: Vec<PathBuf> = Vec::new();
    if let Ok(cwd) = fs::read_link(dir.join("cwd")) {
        targets.push(cwd);
    }
    if let Ok(fds) = fs::read_dir(dir.join("fd")) {
        for fd in fds.flatten() {
            if let Ok(target) = fs::read_link(fd.path()) {
                targets.push(target);
            }
        }
    }
    for target in &targets {
        let text = target.to_string_lossy();
        let trimmed: &str = text.trim_end_matches(" (deleted)");
        let (setup, generation) = classify_managed_path(dx_dir, Path::new(trimmed));
        if let Some(hex) = setup {
            live.setup.push(hex);
        }
        if let Some(generation) = generation {
            live.generations.push(generation);
        }
    }
}

pub fn scan_live_observation(proc_root: &Path, dx_dir: &Path) -> LiveObservation {
    let entries = match fs::read_dir(proc_root) {
        Ok(entries) => entries,
        Err(error) => {
            return LiveObservation::Unknown {
                reason: format!("cannot list {}: {error}", proc_root.display()),
            };
        }
    };
    let mut live = LiveHexes::default();
    for entry in entries.flatten() {
        let name = entry.file_name();
        let pid = name.to_string_lossy();
        if pid.bytes().all(|byte| byte.is_ascii_digit()) {
            observe_process(&entry.path(), dx_dir, &mut live);
        }
    }
    live.setup.sort();
    live.setup.dedup();
    live.generations.sort_by(|left, right| {
        left.kind
            .dir_name()
            .cmp(right.kind.dir_name())
            .then_with(|| left.hex.cmp(&right.hex))
    });
    live.generations.dedup();
    LiveObservation::Known(live)
}

fn lease_use(kind: GenerationKind) -> GenerationUse {
    match kind {
        GenerationKind::Environment => GenerationUse::Environment,
        GenerationKind::Generated => GenerationUse::Generated,
    }
}

fn leased_generation_hexes(
    workspace_root: &Path,
    generations: &[GenerationView],
) -> Result<Vec<String>, CleanError> {
    let dx_dir = workspace_root.join(DX_DIR_NAME);
    let mut leased = Vec::new();
    for generation in generations {
        let held = lease::shared_held(&dx_dir, lease_use(generation.kind), &generation.hex)
            .map_err(|error| CleanError::Install {
                reason: format!(
                    "cannot probe the lease for .dx/{}/{}: {error}",
                    generation.kind.dir_name(),
                    generation.hex
                ),
            })?;
        if held {
            leased.push(generation.hex.clone());
        }
    }
    leased.sort();
    leased.dedup();
    Ok(leased)
}

pub fn collect_inventory_with_scan(
    workspace_root: &Path,
    unobserved: UnobservedPolicy,
) -> Result<CollectedInventory, CleanError> {
    collect_inventory_with_scan_from(workspace_root, Path::new("/proc"), unobserved)
}

pub fn collect_inventory_with_scan_from(
    workspace_root: &Path,
    proc_root: &Path,
    unobserved: UnobservedPolicy,
) -> Result<CollectedInventory, CleanError> {
    let dx_dir = workspace_root.join(DX_DIR_NAME);
    let (live, observation_unknown) = match scan_live_observation(proc_root, &dx_dir) {
        LiveObservation::Known(live) => (live, false),
        LiveObservation::Unknown { .. } => (LiveHexes::default(), true),
    };
    let mut inventory = collect_inventory(workspace_root, &[], &[])?;
    let leased = leased_generation_hexes(workspace_root, &inventory.generations)?;
    let mut generations: Vec<String> = live
        .generations
        .iter()
        .map(|view| view.hex.clone())
        .collect();
    generations.extend(leased);
    generations.sort();
    generations.dedup();
    inventory.active_setup_hexes = live.setup;
    inventory.active_generation_hexes = generations;
    inventory.observation_unknown = observation_unknown;
    inventory.unobserved = unobserved;
    Ok(inventory)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures::*;

    fn stage_process(proc_root: &Path, pid: &str, cwd: Option<&Path>, fds: &[&Path]) {
        let dir = proc_root.join(pid);
        fs::create_dir_all(dir.join("fd")).expect("stage fd dir");
        if let Some(target) = cwd {
            stage_symlink(target, &dir.join("cwd"));
        }
        for (index, target) in fds.iter().enumerate() {
            stage_symlink(target, &dir.join("fd").join(index.to_string()));
        }
    }

    #[test]
    fn scan_missing_proc_root_reports_unknown_not_idle() {
        let scratch = {
            let __scratch = dx_test_scratch::scratch("dx-clean-test-scan-missing-");
            std::fs::create_dir_all(__scratch.path().join("ws")).expect("create workspace");
            __scratch
        };
        let root = scratch.path().to_path_buf();
        let dx_dir = workspace_of(&root).join(DX_DIR_NAME);
        match scan_live_observation(&root.join("no-such-proc"), &dx_dir) {
            LiveObservation::Unknown { reason } => assert!(!reason.is_empty()),
            LiveObservation::Known(_) => panic!("a missing proc root is unknown, never idle"),
        }
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn scan_file_proc_root_reports_unknown() {
        let scratch = {
            let __scratch = dx_test_scratch::scratch("dx-clean-test-scan-file-");
            std::fs::create_dir_all(__scratch.path().join("ws")).expect("create workspace");
            __scratch
        };
        let root = scratch.path().to_path_buf();
        let dx_dir = workspace_of(&root).join(DX_DIR_NAME);
        let blocker = root.join("proc");
        fs::write(&blocker, "not a directory").expect("blocker file");
        match scan_live_observation(&blocker, &dx_dir) {
            LiveObservation::Unknown { .. } => {}
            LiveObservation::Known(_) => panic!("a file proc root is unknown, never idle"),
        }
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn scan_ignores_non_numeric_entries() {
        let scratch = {
            let __scratch = dx_test_scratch::scratch("dx-clean-test-scan-nonnumeric-");
            std::fs::create_dir_all(__scratch.path().join("ws")).expect("create workspace");
            __scratch
        };
        let root = scratch.path().to_path_buf();
        let workspace = workspace_of(&root);
        let dx_dir = workspace.join(".dx");
        let proc_root = root.join("proc");
        fs::create_dir_all(&proc_root).expect("proc root");
        let stale = digest('3');
        stage_process(
            &proc_root,
            "self",
            Some(&dx_dir.join("setups").join(&stale)),
            &[],
        );
        match scan_live_observation(&proc_root, &dx_dir) {
            LiveObservation::Known(live) => assert_eq!(live, LiveHexes::default()),
            LiveObservation::Unknown { .. } => panic!("a listed proc root is known"),
        }
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn scan_reports_cwd_and_fd_targets_under_managed_roots() {
        let scratch = {
            let __scratch = dx_test_scratch::scratch("dx-clean-test-scan-live-");
            std::fs::create_dir_all(__scratch.path().join("ws")).expect("create workspace");
            __scratch
        };
        let root = scratch.path().to_path_buf();
        let workspace = workspace_of(&root);
        let dx_dir = workspace.join(".dx");
        let proc_root = root.join("proc");
        fs::create_dir_all(&proc_root).expect("proc root");
        let (setup_hex_value, _, _) = pair_env_gen('3', '4');
        let env_hex = digest('1');
        let gen_hex = digest('2');
        let foreign = root.join("elsewhere");
        stage_process(
            &proc_root,
            "4242",
            Some(&dx_dir.join("setups").join(&setup_hex_value)),
            &[
                &dx_dir.join("environments").join(&env_hex),
                &dx_dir.join("generated").join(&gen_hex),
                &foreign,
            ],
        );
        let live = match scan_live_observation(&proc_root, &dx_dir) {
            LiveObservation::Known(live) => live,
            LiveObservation::Unknown { .. } => panic!("a listed proc root is known"),
        };
        assert_eq!(live.setup, vec![setup_hex_value]);
        assert_eq!(
            live.generations,
            vec![
                GenerationView {
                    kind: GenerationKind::Environment,
                    hex: env_hex,
                },
                GenerationView {
                    kind: GenerationKind::Generated,
                    hex: gen_hex,
                },
            ]
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn scan_trims_deleted_suffix_and_ignores_unmanaged_paths() {
        let scratch = {
            let __scratch = dx_test_scratch::scratch("dx-clean-test-scan-edge-");
            std::fs::create_dir_all(__scratch.path().join("ws")).expect("create workspace");
            __scratch
        };
        let root = scratch.path().to_path_buf();
        let workspace = workspace_of(&root);
        let dx_dir = workspace.join(".dx");
        let proc_root = root.join("proc");
        fs::create_dir_all(&proc_root).expect("proc root");
        let env_hex = digest('5');
        let deleted = dx_dir.join("environments").join(&env_hex);
        let deleted_text = format!("{} (deleted)", deleted.display());
        let dir = proc_root.join("7");
        fs::create_dir_all(dir.join("fd")).expect("fd dir");
        stage_symlink(Path::new(&deleted_text), &dir.join("cwd"));
        stage_symlink(
            &dx_dir.join("setups").join("latest"),
            &dir.join("fd").join("0"),
        );
        stage_symlink(
            &dx_dir.join("notes").join(digest('9')),
            &dir.join("fd").join("1"),
        );
        stage_symlink(
            &root.join("elsewhere").join(digest('8')),
            &dir.join("fd").join("2"),
        );
        let live = match scan_live_observation(&proc_root, &dx_dir) {
            LiveObservation::Known(live) => live,
            LiveObservation::Unknown { .. } => panic!("a listed proc root is known"),
        };
        assert!(live.setup.is_empty());
        assert_eq!(
            live.generations,
            vec![GenerationView {
                kind: GenerationKind::Environment,
                hex: env_hex,
            }]
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn scan_dedupes_and_sorts_across_processes() {
        let scratch = {
            let __scratch = dx_test_scratch::scratch("dx-clean-test-scan-dedupe-");
            std::fs::create_dir_all(__scratch.path().join("ws")).expect("create workspace");
            __scratch
        };
        let root = scratch.path().to_path_buf();
        let workspace = workspace_of(&root);
        let dx_dir = workspace.join(".dx");
        let proc_root = root.join("proc");
        fs::create_dir_all(&proc_root).expect("proc root");
        let low = digest('1');
        let high = digest('9');
        stage_process(
            &proc_root,
            "100",
            Some(&dx_dir.join("generated").join(&high)),
            &[&dx_dir.join("generated").join(&low)],
        );
        stage_process(
            &proc_root,
            "200",
            Some(&dx_dir.join("generated").join(&low)),
            &[&dx_dir.join("generated").join(&high)],
        );
        let live = match scan_live_observation(&proc_root, &dx_dir) {
            LiveObservation::Known(live) => live,
            LiveObservation::Unknown { .. } => panic!("a listed proc root is known"),
        };
        assert!(live.setup.is_empty());
        assert_eq!(
            live.generations,
            vec![
                GenerationView {
                    kind: GenerationKind::Generated,
                    hex: low,
                },
                GenerationView {
                    kind: GenerationKind::Generated,
                    hex: high,
                },
            ]
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn with_scan_matches_plain_inventory_without_live_processes() {
        let scratch = {
            let __scratch = dx_test_scratch::scratch("dx-clean-test-with-scan-");
            std::fs::create_dir_all(__scratch.path().join("ws")).expect("create workspace");
            __scratch
        };
        let root = scratch.path().to_path_buf();
        let (workspace, stale_hex, _) = two_record_workspace(&root);
        let proc_root = root.join("proc");
        fs::create_dir_all(&proc_root).expect("empty proc root");
        let scanned = collect_inventory_with_scan_from(
            &workspace,
            &proc_root,
            UnobservedPolicy::Preserve,
        )
        .expect("scan");
        assert!(!scanned.observation_unknown);
        let plain = collect_inventory(&workspace, &[], &[]).expect("plain collect");
        assert_eq!(scanned.plan(), plain.plan());
        assert_eq!(scanned.plan().prune_setup_records, vec![stale_hex]);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn unknown_observation_preserves_unobserved_state_by_default() {
        let scratch = {
            let __scratch = dx_test_scratch::scratch("dx-clean-test-unknown-");
            std::fs::create_dir_all(__scratch.path().join("ws")).expect("create workspace");
            __scratch
        };
        let root = scratch.path().to_path_buf();
        let (workspace, stale_hex, _) = two_record_workspace(&root);
        let blocker = root.join("proc");
        fs::write(&blocker, "not a directory").expect("blocker file");
        let scanned = collect_inventory_with_scan_from(
            &workspace,
            &blocker,
            UnobservedPolicy::Preserve,
        )
        .expect("scan");
        assert!(scanned.observation_unknown);
        let plan = scanned.plan();
        assert!(plan.observation_unknown);
        assert!(plan.prune_setup_records.is_empty());
        assert!(plan.prune_generations.is_empty());
        assert_eq!(plan.preserved_unobserved_setup_records, vec![stale_hex]);
        assert_eq!(plan.preserved_unobserved_generations.len(), 2);
        let listing = super::super::bytes::render_dry_run(&plan, &super::super::bytes::PruneBytes::default());
        assert!(listing.contains("observation unavailable"));
        assert!(!listing.contains("nothing to prune"));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn unknown_observation_prunes_only_with_an_explicit_acknowledgement() {
        let scratch = {
            let __scratch = dx_test_scratch::scratch("dx-clean-test-unknown-ack-");
            std::fs::create_dir_all(__scratch.path().join("ws")).expect("create workspace");
            __scratch
        };
        let root = scratch.path().to_path_buf();
        let (workspace, stale_hex, _) = two_record_workspace(&root);
        let blocker = root.join("proc");
        fs::write(&blocker, "not a directory").expect("blocker file");
        let scanned = collect_inventory_with_scan_from(
            &workspace,
            &blocker,
            UnobservedPolicy::PruneAcknowledged,
        )
        .expect("scan");
        assert!(scanned.observation_unknown);
        let plan = scanned.plan();
        assert!(plan.observation_unknown);
        assert_eq!(plan.prune_setup_records, vec![stale_hex]);
        assert_eq!(plan.prune_generations.len(), 2);
        assert!(plan.preserved_unobserved_setup_records.is_empty());
        assert!(plan.preserved_unobserved_generations.is_empty());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn leased_generations_are_active_without_a_proc_sighting() {
        let scratch = {
            let __scratch = dx_test_scratch::scratch("dx-clean-test-leased-");
            std::fs::create_dir_all(__scratch.path().join("ws")).expect("create workspace");
            __scratch
        };
        let root = scratch.path().to_path_buf();
        let (workspace, _, _) = two_record_workspace(&root);
        let proc_root = root.join("proc");
        fs::create_dir_all(&proc_root).expect("empty proc root");
        let stale_env = digest('3');
        let dx_dir = workspace.join(DX_DIR_NAME);
        let lease = dx_atomic_fs::lease::acquire_shared(
            &dx_dir,
            dx_atomic_fs::lease::GenerationUse::Environment,
            &stale_env,
            std::time::Duration::from_secs(10),
        )
        .expect("lease the stale environment");
        let scanned = collect_inventory_with_scan_from(
            &workspace,
            &proc_root,
            UnobservedPolicy::Preserve,
        )
        .expect("scan");
        assert!(scanned.active_generation_hexes.contains(&stale_env));
        let plan = scanned.plan();
        assert!(
            !plan
                .prune_generations
                .iter()
                .any(|generation| generation.hex == stale_env),
            "a leased generation is never planned for pruning"
        );
        drop(lease);
        let rescanned = collect_inventory_with_scan_from(
            &workspace,
            &proc_root,
            UnobservedPolicy::Preserve,
        )
        .expect("rescan");
        assert!(!rescanned.active_generation_hexes.contains(&stale_env));
        assert!(
            rescanned
                .plan()
                .prune_generations
                .iter()
                .any(|generation| generation.hex == stale_env),
            "releasing the lease makes the generation prunable again"
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn lease_families_match_setup_generation_directories() {
        use dx_atomic_fs::lease::GenerationUse;
        assert_eq!(
            GenerationUse::Environment.dir_name(),
            dx_setup::ENVIRONMENTS_DIR_NAME
        );
        assert_eq!(
            GenerationUse::Generated.dir_name(),
            dx_setup::GENERATED_DIR_NAME
        );
        assert_eq!(
            GenerationKind::Environment.dir_name(),
            GenerationUse::Environment.dir_name()
        );
        assert_eq!(
            GenerationKind::Generated.dir_name(),
            GenerationUse::Generated.dir_name()
        );
    }
}
