use std::ffi::OsString;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::{ENVIRONMENTS_DIR_NAME, GENERATED_DIR_NAME, GenerationId, SETUPS_DIR_NAME};
use dx_atomic_fs::lease::{self, GenerationUse};

use crate::prune::PruneError;
use crate::prune::inventory::{CollectedInventory, collect_inventory};
use crate::prune::planning::GenerationView;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LiveHexes {
    pub setup: Vec<String>,
    pub generations: Vec<GenerationView>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LiveObservation {
    Known(LiveHexes),
    Unknown { partial: LiveHexes, reason: String },
    Unavailable { reason: String },
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
                kind: GenerationUse::Environment,
                hex: name.to_owned(),
            }),
        )
    } else if root == GENERATED_DIR_NAME {
        (
            None,
            Some(GenerationView {
                kind: GenerationUse::Generated,
                hex: name.to_owned(),
            }),
        )
    } else {
        (None, None)
    }
}

const ESRCH: i32 = 3;

fn gone(error: &io::Error) -> bool {
    error.kind() == io::ErrorKind::NotFound || error.raw_os_error() == Some(ESRCH)
}

fn denied(error: &io::Error) -> bool {
    error.kind() == io::ErrorKind::PermissionDenied
}

trait ProcFs {
    fn list_names(&self, dir: &Path) -> io::Result<Vec<io::Result<OsString>>>;
    fn read_link(&self, path: &Path) -> io::Result<PathBuf>;
    fn read(&self, path: &Path) -> io::Result<Vec<u8>>;
}

struct RealProcFs;

impl ProcFs for RealProcFs {
    fn list_names(&self, dir: &Path) -> io::Result<Vec<io::Result<OsString>>> {
        Ok(fs::read_dir(dir)?
            .map(|entry| entry.map(|entry| entry.file_name()))
            .collect())
    }

    fn read_link(&self, path: &Path) -> io::Result<PathBuf> {
        fs::read_link(path)
    }

    fn read(&self, path: &Path) -> io::Result<Vec<u8>> {
        fs::read(path)
    }
}

enum Stop {
    Denied,
    Failed(String),
}

fn checked<T>(path: &Path, result: io::Result<T>) -> Result<Option<T>, Stop> {
    match result {
        Ok(value) => Ok(Some(value)),
        Err(error) if gone(&error) => Ok(None),
        Err(error) if denied(&error) => Err(Stop::Denied),
        Err(error) => Err(Stop::Failed(format!(
            "cannot read {}: {error}",
            path.display()
        ))),
    }
}

fn read_targets(
    proc: &dyn ProcFs,
    dir: &Path,
    dx_dir: &Path,
    targets: &mut Vec<PathBuf>,
) -> Result<(), Stop> {
    for name in ["cwd", "exe"] {
        let path = dir.join(name);
        targets.extend(checked(&path, proc.read_link(&path))?);
    }
    let fd_dir = dir.join("fd");
    for fd in checked(&fd_dir, proc.list_names(&fd_dir))?.unwrap_or_default() {
        let Some(fd) = checked(&fd_dir, fd)? else {
            continue;
        };
        let path = fd_dir.join(fd);
        targets.extend(checked(&path, proc.read_link(&path))?);
    }
    let maps_path = dir.join("maps");
    let maps = checked(&maps_path, proc.read(&maps_path))?.unwrap_or_default();
    for line in String::from_utf8_lossy(&maps).lines() {
        if let Some(start) = line.find('/') {
            let mapped = Path::new(&line[start..]);
            if mapped.starts_with(dx_dir) {
                targets.push(mapped.to_path_buf());
            }
        }
    }
    Ok(())
}

fn observe_process(
    proc: &dyn ProcFs,
    dir: &Path,
    dx_dir: &Path,
    live: &mut LiveHexes,
) -> Result<(), String> {
    let mut targets: Vec<PathBuf> = Vec::new();
    let outcome = read_targets(proc, dir, dx_dir, &mut targets);
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
    match outcome {
        Ok(()) | Err(Stop::Denied) => Ok(()),
        Err(Stop::Failed(reason)) => Err(reason),
    }
}

/// Scans `proc_root` for processes whose cwd, executable, open files or
/// mapped files sit in `dx_dir`.
///
/// Reading stops at the first file the kernel refuses to show (another uid,
/// or non-dumpable): such a process cannot be using this user's `.dx/local`.
/// A root-owned process working in someone's `.dx/local` is therefore not seen.
pub fn scan_live_observation(proc_root: &Path, dx_dir: &Path) -> LiveObservation {
    scan_with(&RealProcFs, proc_root, dx_dir)
}

fn scan_with(proc: &dyn ProcFs, proc_root: &Path, dx_dir: &Path) -> LiveObservation {
    let names = match proc.list_names(proc_root) {
        Ok(names) => names,
        Err(error) => {
            return LiveObservation::Unavailable {
                reason: format!("cannot list {}: {error}", proc_root.display()),
            };
        }
    };
    let mut live = LiveHexes::default();
    let mut unreadable: Vec<String> = Vec::new();
    for name in names {
        let name = match name {
            Ok(name) => name,
            Err(error) if gone(&error) => continue,
            Err(error) => {
                unreadable.push(format!("cannot list {}: {error}", proc_root.display()));
                continue;
            }
        };
        let pid = name.to_string_lossy();
        if !pid.is_empty()
            && pid.bytes().all(|byte| byte.is_ascii_digit())
            && let Err(reason) = observe_process(proc, &proc_root.join(&name), dx_dir, &mut live)
        {
            unreadable.push(format!("process {pid}: {reason}"));
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
    let Some(first) = unreadable.first() else {
        return LiveObservation::Known(live);
    };
    let mut reason = first.clone();
    if unreadable.len() > 1 {
        reason.push_str(&format!(
            " (+{} more unreadable processes)",
            unreadable.len() - 1
        ));
    }
    LiveObservation::Unknown {
        partial: live,
        reason,
    }
}

fn leased_generation_hexes(
    workspace_root: &Path,
    generations: &[GenerationView],
) -> Result<Vec<String>, PruneError> {
    let dx_dir = dx_path::state::local_dir(workspace_root);
    let mut leased = Vec::new();
    for generation in generations {
        let held =
            lease::shared_held(&dx_dir, generation.kind, &generation.hex).map_err(|error| {
                PruneError::Install {
                    reason: format!(
                        "cannot probe the lease for .dx/local/{}/{}: {error}",
                        generation.kind.dir_name(),
                        generation.hex
                    ),
                }
            })?;
        if held {
            leased.push(generation.hex.clone());
        }
    }
    leased.sort();
    leased.dedup();
    Ok(leased)
}

/// Collects the inventory with live processes from `proc_root`.
///
/// `previous` is the setup the caller just replaced, if any. When processes
/// cannot be listed at all, only it and the current setup are kept. When some
/// process could not be read, everything is kept.
pub fn collect_inventory_with_scan_from(
    workspace_root: &Path,
    proc_root: &Path,
    previous: Option<&str>,
) -> Result<CollectedInventory, PruneError> {
    collect_inventory_with(&RealProcFs, workspace_root, proc_root, previous)
}

fn collect_inventory_with(
    proc: &dyn ProcFs,
    workspace_root: &Path,
    proc_root: &Path,
    previous: Option<&str>,
) -> Result<CollectedInventory, PruneError> {
    let dx_dir = dx_path::state::local_dir(workspace_root);
    let (live, observation_unknown, previous) = match scan_with(proc, proc_root, &dx_dir) {
        LiveObservation::Known(live) => (live, false, previous),
        LiveObservation::Unavailable { .. } => (LiveHexes::default(), true, previous),
        LiveObservation::Unknown { partial, .. } => (partial, true, None),
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
    inventory.previous_hex = previous.map(str::to_owned);
    Ok(inventory)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prune::fixtures::*;
    use std::collections::HashMap;

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
    fn scan_missing_proc_root_reports_unavailable_not_idle() {
        let scratch = {
            let __scratch = dx_test_scratch::scratch("dx-prune-test-scan-missing-");
            std::fs::create_dir_all(__scratch.path().join("ws")).expect("create workspace");
            __scratch
        };
        let root = scratch.path().to_path_buf();
        let dx_dir = dx_path::state::local_dir(&workspace_of(&root));
        match scan_live_observation(&root.join("no-such-proc"), &dx_dir) {
            LiveObservation::Unavailable { reason } => assert!(!reason.is_empty()),
            other => panic!("a missing proc root is unavailable, never idle: {other:?}"),
        }
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn scan_file_proc_root_reports_unavailable() {
        let scratch = {
            let __scratch = dx_test_scratch::scratch("dx-prune-test-scan-file-");
            std::fs::create_dir_all(__scratch.path().join("ws")).expect("create workspace");
            __scratch
        };
        let root = scratch.path().to_path_buf();
        let dx_dir = dx_path::state::local_dir(&workspace_of(&root));
        let blocker = root.join("proc");
        fs::write(&blocker, "not a directory").expect("blocker file");
        match scan_live_observation(&blocker, &dx_dir) {
            LiveObservation::Unavailable { .. } => {}
            other => panic!("a file proc root is unavailable, never idle: {other:?}"),
        }
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn scan_ignores_non_numeric_entries() {
        let scratch = {
            let __scratch = dx_test_scratch::scratch("dx-prune-test-scan-nonnumeric-");
            std::fs::create_dir_all(__scratch.path().join("ws")).expect("create workspace");
            __scratch
        };
        let root = scratch.path().to_path_buf();
        let workspace = workspace_of(&root);
        let dx_dir = dx_path::state::local_dir(&workspace);
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
            other => panic!("a listed proc root is known: {other:?}"),
        }
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn scan_reports_cwd_and_fd_targets_under_managed_roots() {
        let scratch = {
            let __scratch = dx_test_scratch::scratch("dx-prune-test-scan-live-");
            std::fs::create_dir_all(__scratch.path().join("ws")).expect("create workspace");
            __scratch
        };
        let root = scratch.path().to_path_buf();
        let workspace = workspace_of(&root);
        let dx_dir = dx_path::state::local_dir(&workspace);
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
            other => panic!("a listed proc root is known: {other:?}"),
        };
        assert_eq!(live.setup, vec![setup_hex_value]);
        assert_eq!(
            live.generations,
            vec![
                GenerationView {
                    kind: GenerationUse::Environment,
                    hex: env_hex,
                },
                GenerationView {
                    kind: GenerationUse::Generated,
                    hex: gen_hex,
                },
            ]
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn scan_trims_deleted_suffix_and_ignores_unmanaged_paths() {
        let scratch = {
            let __scratch = dx_test_scratch::scratch("dx-prune-test-scan-edge-");
            std::fs::create_dir_all(__scratch.path().join("ws")).expect("create workspace");
            __scratch
        };
        let root = scratch.path().to_path_buf();
        let workspace = workspace_of(&root);
        let dx_dir = dx_path::state::local_dir(&workspace);
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
            other => panic!("a listed proc root is known: {other:?}"),
        };
        assert!(live.setup.is_empty());
        assert_eq!(
            live.generations,
            vec![GenerationView {
                kind: GenerationUse::Environment,
                hex: env_hex,
            }]
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn scan_dedupes_and_sorts_across_processes() {
        let scratch = {
            let __scratch = dx_test_scratch::scratch("dx-prune-test-scan-dedupe-");
            std::fs::create_dir_all(__scratch.path().join("ws")).expect("create workspace");
            __scratch
        };
        let root = scratch.path().to_path_buf();
        let workspace = workspace_of(&root);
        let dx_dir = dx_path::state::local_dir(&workspace);
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
            other => panic!("a listed proc root is known: {other:?}"),
        };
        assert!(live.setup.is_empty());
        assert_eq!(
            live.generations,
            vec![
                GenerationView {
                    kind: GenerationUse::Generated,
                    hex: low,
                },
                GenerationView {
                    kind: GenerationUse::Generated,
                    hex: high,
                },
            ]
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn with_scan_matches_plain_inventory_without_live_processes() {
        let scratch = {
            let __scratch = dx_test_scratch::scratch("dx-prune-test-with-scan-");
            std::fs::create_dir_all(__scratch.path().join("ws")).expect("create workspace");
            __scratch
        };
        let root = scratch.path().to_path_buf();
        let (workspace, stale_hex, _) = two_record_workspace(&root);
        let proc_root = root.join("proc");
        fs::create_dir_all(&proc_root).expect("empty proc root");
        let scanned = collect_inventory_with_scan_from(&workspace, &proc_root, None).expect("scan");
        assert!(!scanned.observation_unknown);
        let plain = collect_inventory(&workspace, &[], &[]).expect("plain collect");
        assert_eq!(scanned.plan(), plain.plan());
        assert_eq!(scanned.plan().prune_setup_records, vec![stale_hex]);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn unknown_observation_preserves_unobserved_state_by_default() {
        let scratch = {
            let __scratch = dx_test_scratch::scratch("dx-prune-test-unknown-");
            std::fs::create_dir_all(__scratch.path().join("ws")).expect("create workspace");
            __scratch
        };
        let root = scratch.path().to_path_buf();
        let (workspace, stale_hex, _) = two_record_workspace(&root);
        let blocker = root.join("proc");
        fs::write(&blocker, "not a directory").expect("blocker file");
        let scanned = collect_inventory_with_scan_from(&workspace, &blocker, None).expect("scan");
        assert!(scanned.observation_unknown);
        let plan = scanned.plan();
        assert!(plan.observation_unknown);
        assert!(plan.prune_setup_records.is_empty());
        assert!(plan.prune_generations.is_empty());
        assert_eq!(plan.preserved_unobserved_setup_records, vec![stale_hex]);
        assert_eq!(plan.preserved_unobserved_generations.len(), 2);
        let _ = fs::remove_dir_all(&root);
    }

    fn stage_unreadable_cwd(proc_root: &Path, pid: &str) {
        let dir = proc_root.join(pid);
        fs::create_dir_all(dir.join("fd")).expect("stage fd dir");
        fs::write(dir.join("cwd"), "not a link").expect("cwd file");
    }

    #[test]
    fn scan_unreadable_cwd_reports_unknown_with_partial_hexes() {
        let scratch = dx_test_scratch::scratch("dx-prune-test-scan-unreadable-cwd-");
        let root = scratch.path().to_path_buf();
        let dx_dir = dx_path::state::local_dir(&workspace_of(&root));
        let proc_root = root.join("proc");
        fs::create_dir_all(&proc_root).expect("proc root");
        let hex = digest('3');
        stage_process(&proc_root, "10", None, &[&dx_dir.join("setups").join(&hex)]);
        stage_unreadable_cwd(&proc_root, "11");
        match scan_live_observation(&proc_root, &dx_dir) {
            LiveObservation::Unknown { partial, reason } => {
                assert_eq!(partial.setup, vec![hex]);
                assert!(reason.contains("11"), "{reason}");
            }
            other => panic!("an unreadable process is unknown: {other:?}"),
        }
    }

    #[test]
    fn scan_unreadable_fd_dir_reports_unknown() {
        let scratch = dx_test_scratch::scratch("dx-prune-test-scan-unreadable-fd-");
        let root = scratch.path().to_path_buf();
        let dx_dir = dx_path::state::local_dir(&workspace_of(&root));
        let proc_root = root.join("proc");
        fs::create_dir_all(proc_root.join("12")).expect("pid dir");
        fs::write(proc_root.join("12").join("fd"), "not a dir").expect("fd file");
        match scan_live_observation(&proc_root, &dx_dir) {
            LiveObservation::Unknown { reason, .. } => assert!(reason.contains("12"), "{reason}"),
            other => panic!("an unreadable fd dir is unknown: {other:?}"),
        }
    }

    #[test]
    fn scan_skips_processes_that_are_gone() {
        let scratch = dx_test_scratch::scratch("dx-prune-test-scan-gone-");
        let root = scratch.path().to_path_buf();
        let dx_dir = dx_path::state::local_dir(&workspace_of(&root));
        let proc_root = root.join("proc");
        fs::create_dir_all(proc_root.join("13")).expect("pid dir");
        match scan_live_observation(&proc_root, &dx_dir) {
            LiveObservation::Known(live) => assert_eq!(live, LiveHexes::default()),
            other => panic!("gone process is skipped: {other:?}"),
        }
    }

    #[test]
    fn unreadable_process_keeps_observed_state() {
        let scratch = dx_test_scratch::scratch("dx-prune-test-unreadable-prune-");
        std::fs::create_dir_all(scratch.path().join("ws")).expect("create workspace");
        let root = scratch.path().to_path_buf();
        let (workspace, stale_hex, _) = two_record_workspace(&root);
        let dx_dir = dx_path::state::local_dir(&workspace);
        let proc_root = root.join("proc");
        fs::create_dir_all(&proc_root).expect("proc root");
        stage_process(
            &proc_root,
            "20",
            Some(&dx_dir.join("setups").join(&stale_hex)),
            &[],
        );
        stage_unreadable_cwd(&proc_root, "21");
        let preserved =
            collect_inventory_with_scan_from(&workspace, &proc_root, None).expect("scan");
        assert!(preserved.observation_unknown);
        let plan = preserved.plan();
        assert!(plan.prune_setup_records.is_empty());
        assert!(plan.prune_generations.is_empty());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn real_proc_is_observable() {
        let scratch = dx_test_scratch::scratch("dx-prune-test-real-proc-");
        let dx_dir = dx_path::state::local_dir(&workspace_of(scratch.path()));
        match scan_live_observation(Path::new("/proc"), &dx_dir) {
            LiveObservation::Known(_) => {}
            other => panic!("/proc is observable: {other:?}"),
        }
    }

    #[test]
    fn unavailable_observation_keeps_only_current_and_previous() {
        let scratch = dx_test_scratch::scratch("dx-prune-test-unknown-previous-");
        std::fs::create_dir_all(scratch.path().join("ws")).expect("create workspace");
        let root = scratch.path().to_path_buf();
        let (workspace, older_hex, _) = two_record_workspace(&root);
        let previous = setup_pair('5', '6');
        crate::commit_pair(&workspace, &previous).expect("commit previous");
        crate::commit_pair(&workspace, &setup_pair('1', '2')).expect("commit current");
        let previous_hex = crate::setup_hex(&previous);
        let blocker = root.join("proc");
        fs::write(&blocker, "not a directory").expect("blocker file");
        let scanned = collect_inventory_with_scan_from(&workspace, &blocker, Some(&previous_hex))
            .expect("scan");
        assert!(scanned.observation_unknown);
        let plan = scanned.plan();
        assert_eq!(plan.prune_setup_records, vec![older_hex]);
        assert_eq!(plan.preserved_unobserved_setup_records, vec![previous_hex]);
    }

    #[test]
    fn leased_generations_are_active_without_a_proc_sighting() {
        let scratch = {
            let __scratch = dx_test_scratch::scratch("dx-prune-test-leased-");
            std::fs::create_dir_all(__scratch.path().join("ws")).expect("create workspace");
            __scratch
        };
        let root = scratch.path().to_path_buf();
        let (workspace, _, _) = two_record_workspace(&root);
        let proc_root = root.join("proc");
        fs::create_dir_all(&proc_root).expect("empty proc root");
        let stale_env = digest('3');
        let dx_dir = dx_path::state::local_dir(&workspace);
        let lease = dx_atomic_fs::lease::acquire_shared(
            &dx_dir,
            dx_atomic_fs::lease::GenerationUse::Environment,
            &stale_env,
            std::time::Duration::from_secs(10),
        )
        .expect("lease the stale environment");
        let scanned = collect_inventory_with_scan_from(&workspace, &proc_root, None).expect("scan");
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
        let rescanned =
            collect_inventory_with_scan_from(&workspace, &proc_root, None).expect("rescan");
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

    const EPERM: i32 = 1;
    const EIO: i32 = 5;
    const EACCES: i32 = 13;
    const EINVAL: i32 = 22;

    fn staged_root(scratch: &dx_test_scratch::TempDir) -> PathBuf {
        fs::create_dir_all(workspace_of(scratch.path())).expect("create workspace");
        scratch.path().to_path_buf()
    }

    #[derive(Default)]
    struct FakeProc {
        dirs: HashMap<PathBuf, Vec<OsString>>,
        links: HashMap<PathBuf, PathBuf>,
        files: HashMap<PathBuf, Vec<u8>>,
        errors: HashMap<PathBuf, i32>,
    }

    impl FakeProc {
        fn root() -> PathBuf {
            PathBuf::from("/fake-proc")
        }

        fn process(&mut self, pid: &str) -> PathBuf {
            self.dirs.entry(Self::root()).or_default().push(pid.into());
            Self::root().join(pid)
        }

        fn fd(&mut self, pid_dir: &Path, fd: &str, target: &Path) {
            let fd_dir = pid_dir.join("fd");
            self.dirs.entry(fd_dir.clone()).or_default().push(fd.into());
            self.links.insert(fd_dir.join(fd), target.to_path_buf());
        }

        fn fail(&mut self, path: PathBuf, code: i32) {
            self.errors.insert(path, code);
        }

        fn error(&self, path: &Path) -> io::Result<()> {
            match self.errors.get(path) {
                Some(code) => Err(io::Error::from_raw_os_error(*code)),
                None => Ok(()),
            }
        }
    }

    impl ProcFs for FakeProc {
        fn list_names(&self, dir: &Path) -> io::Result<Vec<io::Result<OsString>>> {
            self.error(dir)?;
            let names = self.dirs.get(dir).ok_or(io::ErrorKind::NotFound)?;
            Ok(names.iter().cloned().map(Ok).collect())
        }

        fn read_link(&self, path: &Path) -> io::Result<PathBuf> {
            self.error(path)?;
            Ok(self.links.get(path).ok_or(io::ErrorKind::NotFound)?.clone())
        }

        fn read(&self, path: &Path) -> io::Result<Vec<u8>> {
            self.error(path)?;
            Ok(self.files.get(path).ok_or(io::ErrorKind::NotFound)?.clone())
        }
    }

    #[test]
    fn denied_foreign_process_does_not_block_pruning() {
        for code in [EACCES, EPERM] {
            let scratch = dx_test_scratch::scratch("dx-prune-test-scan-denied-");
            let (workspace, stale_hex, _) = two_record_workspace(&staged_root(&scratch));
            let dx_dir = dx_path::state::local_dir(&workspace);
            let mut proc = FakeProc::default();
            let pid = proc.process("1");
            proc.links
                .insert(pid.join("exe"), dx_dir.join("setups").join(&stale_hex));
            proc.fail(pid.join("cwd"), code);
            let scanned =
                collect_inventory_with(&proc, &workspace, &FakeProc::root(), None).expect("scan");
            assert!(!scanned.observation_unknown, "errno {code}");
            assert_eq!(scanned.plan().prune_setup_records, vec![stale_hex]);
        }
    }

    #[test]
    fn unreadable_same_user_process_keeps_everything_despite_previous() {
        let scratch = dx_test_scratch::scratch("dx-prune-test-scan-einval-");
        let (workspace, older_hex, _) = two_record_workspace(&staged_root(&scratch));
        let previous = setup_pair('5', '6');
        crate::commit_pair(&workspace, &previous).expect("commit previous");
        crate::commit_pair(&workspace, &setup_pair('1', '2')).expect("commit current");
        let previous_hex = crate::setup_hex(&previous);
        let mut proc = FakeProc::default();
        let pid = proc.process("30");
        proc.fail(pid.join("cwd"), EINVAL);
        let scanned =
            collect_inventory_with(&proc, &workspace, &FakeProc::root(), Some(&previous_hex))
                .expect("scan");
        assert!(scanned.observation_unknown);
        let plan = scanned.plan();
        assert!(plan.prune_setup_records.is_empty());
        assert!(plan.prune_generations.is_empty());
        let mut kept = vec![older_hex, previous_hex];
        kept.sort();
        assert_eq!(plan.preserved_unobserved_setup_records, kept);
        assert_eq!(plan.preserved_unobserved_generations.len(), 2);
    }

    #[test]
    fn denied_fd_keeps_the_cwd_already_read() {
        let scratch = dx_test_scratch::scratch("dx-prune-test-scan-denied-fd-");
        let (workspace, stale_hex, _) = two_record_workspace(&staged_root(&scratch));
        let dx_dir = dx_path::state::local_dir(&workspace);
        let mut proc = FakeProc::default();
        let pid = proc.process("31");
        proc.links
            .insert(pid.join("cwd"), dx_dir.join("setups").join(&stale_hex));
        proc.fail(pid.join("fd"), EACCES);
        let scanned =
            collect_inventory_with(&proc, &workspace, &FakeProc::root(), None).expect("scan");
        assert!(!scanned.observation_unknown);
        assert_eq!(scanned.active_setup_hexes, vec![stale_hex]);
        assert!(scanned.plan().prune_setup_records.is_empty());
    }

    #[test]
    fn exe_and_mapped_files_keep_their_generations() {
        let scratch = dx_test_scratch::scratch("dx-prune-test-scan-exe-maps-");
        let (workspace, _, _) = two_record_workspace(&staged_root(&scratch));
        let dx_dir = dx_path::state::local_dir(&workspace);
        let old_env = digest('3');
        let old_gen = digest('4');
        let mut proc = FakeProc::default();
        let pid = proc.process("32");
        proc.links.insert(pid.join("cwd"), PathBuf::from("/home"));
        proc.links.insert(
            pid.join("exe"),
            dx_dir.join("environments").join(&old_env).join("bin/tool"),
        );
        proc.fd(&pid, "0", Path::new("/dev/null"));
        let library = dx_dir.join("generated").join(&old_gen).join("lib.so");
        let maps = format!(
            "55d0-55d1 r--p 00000000 08:01 7 /usr/bin/tool\n\
             55d2-55d3 rw-p 00000000 00:00 0 [heap]\n\
             7f00-7f01 r-xp 00000000 08:01 9 {} (deleted)\n",
            library.display()
        );
        proc.files.insert(pid.join("maps"), maps.into_bytes());
        let live = match scan_with(&proc, &FakeProc::root(), &dx_dir) {
            LiveObservation::Known(live) => live,
            other => panic!("a readable process is known: {other:?}"),
        };
        assert_eq!(
            live.generations,
            vec![
                GenerationView {
                    kind: GenerationUse::Environment,
                    hex: old_env.clone(),
                },
                GenerationView {
                    kind: GenerationUse::Generated,
                    hex: old_gen.clone(),
                },
            ]
        );
        let plan = collect_inventory_with(&proc, &workspace, &FakeProc::root(), None)
            .expect("scan")
            .plan();
        assert!(
            !plan
                .prune_generations
                .iter()
                .any(|generation| generation.hex == old_env || generation.hex == old_gen),
            "{plan:?}"
        );
    }

    #[test]
    fn unreadable_maps_reports_unknown_with_partial_hexes() {
        let scratch = dx_test_scratch::scratch("dx-prune-test-scan-maps-eio-");
        let dx_dir = dx_path::state::local_dir(&workspace_of(scratch.path()));
        let hex = digest('7');
        let mut proc = FakeProc::default();
        let pid = proc.process("33");
        proc.links
            .insert(pid.join("cwd"), dx_dir.join("setups").join(&hex));
        proc.fail(pid.join("maps"), EIO);
        match scan_with(&proc, &FakeProc::root(), &dx_dir) {
            LiveObservation::Unknown { partial, reason } => {
                assert_eq!(partial.setup, vec![hex]);
                assert!(reason.contains("maps"), "{reason}");
            }
            other => panic!("an unreadable maps file is unknown: {other:?}"),
        }
    }
}
