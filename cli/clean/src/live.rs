use std::fs;
use std::path::{Path, PathBuf};

use dx_env::DX_DIR_NAME;
use dx_setup::{GenerationId, ENVIRONMENTS_DIR_NAME, GENERATED_DIR_NAME, SETUPS_DIR_NAME};

use super::inventory::{collect_inventory, CollectedInventory};
use super::planning::GenerationView;
use super::records::GenerationKind;
use super::CleanError;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LiveHexes {
    pub setup: Vec<String>,
    pub generations: Vec<GenerationView>,
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

pub fn scan_live_hexes(proc_root: &Path, dx_dir: &Path) -> LiveHexes {
    let mut live = LiveHexes::default();
    let entries = match fs::read_dir(proc_root) {
        Ok(entries) => entries,
        Err(_) => return live,
    };
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
    live
}

pub fn collect_inventory_with_scan(
    workspace_root: &Path,
) -> Result<CollectedInventory, CleanError> {
    let dx_dir = workspace_root.join(DX_DIR_NAME);
    let live = scan_live_hexes(Path::new("/proc"), &dx_dir);
    let setup: Vec<String> = live.setup;
    let generations: Vec<String> = live
        .generations
        .iter()
        .map(|view| view.hex.clone())
        .collect();
    collect_inventory(workspace_root, &setup, &generations)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest(tag: char) -> String {
        tag.to_string().repeat(64)
    }

    fn pair_env_gen(env: char, gen: char) -> (String, String, String) {
        let environment = dx_setup::GenerationId::new(&digest(env)).expect("env digest");
        let generated = dx_setup::GenerationId::new(&digest(gen)).expect("gen digest");
        let pair = dx_setup::SetupPair {
            environment,
            generated,
        };
        (dx_setup::setup_hex(&pair), digest(env), digest(gen))
    }

    fn setup_pair(env: char, gen: char) -> dx_setup::SetupPair {
        dx_setup::SetupPair {
            environment: dx_setup::GenerationId::new(&digest(env)).expect("env digest"),
            generated: dx_setup::GenerationId::new(&digest(gen)).expect("gen digest"),
        }
    }

    fn workspace_of(root: &Path) -> PathBuf {
        root.join("ws")
    }

    fn two_record_workspace(root: &Path) -> (PathBuf, String, String) {
        use crate::records::GenerationKind as Kind;
        let workspace = workspace_of(root);
        let stale = setup_pair('3', '4');
        let current = setup_pair('1', '2');
        dx_setup::commit_pair(&workspace, &stale).expect("commit stale");
        dx_setup::commit_pair(&workspace, &current).expect("commit current");
        let dx_dir = workspace.join(".dx");
        for (kind, tag) in [
            (Kind::Environment, '1'),
            (Kind::Generated, '2'),
            (Kind::Environment, '3'),
            (Kind::Generated, '4'),
        ] {
            fs::create_dir_all(dx_dir.join(kind.dir_name()).join(digest(tag)))
                .expect("create generation dir");
        }
        let stale_hex = dx_setup::setup_hex(&stale);
        let current_hex = dx_setup::setup_hex(&current);
        (workspace, stale_hex, current_hex)
    }

    #[cfg(windows)]
    fn stage_symlink(target: &Path, link: &Path) {
        std::os::windows::fs::symlink_file(target, link).expect("stage test link");
    }

    #[cfg(not(windows))]
    fn stage_symlink(target: &Path, link: &Path) {
        std::os::unix::fs::symlink(target, link).expect("stage test link");
    }

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
    fn scan_missing_proc_root_scans_empty() {
        let scratch = {
            let __scratch = dx_test_scratch::scratch("dx-clean-test-scan-missing-");
            std::fs::create_dir_all(__scratch.path().join("ws")).expect("create workspace");
            __scratch
        };
        let root = scratch.path().to_path_buf();
        let dx_dir = workspace_of(&root).join(DX_DIR_NAME);
        assert_eq!(
            scan_live_hexes(&root.join("no-such-proc"), &dx_dir),
            LiveHexes::default()
        );
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
        assert_eq!(scan_live_hexes(&proc_root, &dx_dir), LiveHexes::default());
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
        let live = scan_live_hexes(&proc_root, &dx_dir);
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
        let live = scan_live_hexes(&proc_root, &dx_dir);
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
        let live = scan_live_hexes(&proc_root, &dx_dir);
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
        let scanned = collect_inventory_with_scan(&workspace).expect("scan collect");
        let plain = collect_inventory(&workspace, &[], &[]).expect("plain collect");
        assert_eq!(scanned.plan(), plain.plan());
        assert_eq!(scanned.plan().prune_setup_records, vec![stale_hex]);
        let _ = fs::remove_dir_all(&root);
    }
}
