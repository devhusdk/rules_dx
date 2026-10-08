use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use dx_env::DX_DIR_NAME;
use dx_setup::{
    GenerationId, CURRENT_LINK_NAME, CURRENT_STAGE_NAME, ENVIRONMENT_LINK_NAME,
    GENERATED_LINK_NAME, SETUPS_DIR_NAME,
};

use super::planning::{plan_prune, CleanPlan, GenerationView, PruneInputs, UnobservedPolicy};
use super::records::{validate_record, GenerationKind, SetupRecordView};
use super::CleanError;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CollectedInventory {
    pub records: Vec<SetupRecordView>,
    pub generations: Vec<GenerationView>,
    pub current_hex: Option<String>,
    pub active_setup_hexes: Vec<String>,
    pub active_generation_hexes: Vec<String>,
    pub unmanaged_names: Vec<String>,
    pub observation_unknown: bool,
    pub unobserved: UnobservedPolicy,
}

impl CollectedInventory {
    pub fn prune_inputs(&self) -> PruneInputs<'_> {
        PruneInputs {
            records: &self.records,
            generations: &self.generations,
            current_hex: self.current_hex.as_deref(),
            active_setup_hexes: &self.active_setup_hexes,
            active_generation_hexes: &self.active_generation_hexes,
            unmanaged_names: &self.unmanaged_names,
            observation_unknown: self.observation_unknown,
            unobserved: self.unobserved,
        }
    }

    pub fn plan(&self) -> CleanPlan {
        plan_prune(self.prune_inputs())
    }
}

fn entry_names(dir: &Path) -> Result<Vec<String>, CleanError> {
    if !dir.exists() {
        match fs::read_dir(dir) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => {
                return Err(CleanError::Install {
                    reason: format!("cannot list {}: {e}", dir.display()),
                });
            }
            Ok(_) => {}
        }
    }
    let mut names = Vec::new();
    for entry in walkdir::WalkDir::new(dir).max_depth(1).min_depth(1) {
        let entry = entry.map_err(|e| CleanError::Install {
            reason: format!("cannot list {}: {e}", dir.display()),
        })?;
        if let Some(name) = entry.file_name().to_str() {
            names.push(name.to_owned());
        } else {
            names.push("<non-utf8-name>".to_owned());
        }
    }
    names.sort();
    Ok(names)
}

pub fn walk_filtered(root: &Path, exclude_globs: &[String]) -> Result<Vec<PathBuf>, CleanError> {
    let mut builder = globset::GlobSetBuilder::new();
    for pattern in exclude_globs {
        let glob = globset::Glob::new(pattern).map_err(|e| CleanError::Install {
            reason: format!("invalid exclude glob {pattern:?}: {e}"),
        })?;
        builder.add(glob);
    }
    let excludes = builder.build().map_err(|e| CleanError::Install {
        reason: format!("invalid exclude globs: {e}"),
    })?;
    if fs::read_dir(root).is_err_and(|e| e.kind() == io::ErrorKind::NotFound) {
        return Ok(Vec::new());
    }
    let mut paths = Vec::new();
    let walker = ignore::WalkBuilder::new(root)
        .hidden(true)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .require_git(false)
        .parents(true)
        .build();
    for entry in walker {
        let entry = entry.map_err(|e| CleanError::Install {
            reason: format!("cannot walk {}: {e}", root.display()),
        })?;
        let path = entry.path().to_path_buf();
        if path == root {
            continue;
        }
        let relative = path.strip_prefix(root).unwrap_or(&path);
        if excludes.is_match(relative) {
            continue;
        }
        paths.push(path);
    }
    paths.sort();
    Ok(paths)
}

fn generation_hex_from_link_target(target: &Path) -> Option<String> {
    target
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|text| GenerationId::new(text).is_ok())
        .map(str::to_owned)
}

pub fn collect_inventory(
    workspace_root: &Path,
    active_setup_hexes: &[String],
    active_generation_hexes: &[String],
) -> Result<CollectedInventory, CleanError> {
    if !workspace_root.is_dir() {
        return Err(CleanError::WorkspaceRoot {
            path: workspace_root.to_path_buf(),
        });
    }
    let dx_dir = workspace_root.join(DX_DIR_NAME);
    let setups_dir = dx_dir.join(SETUPS_DIR_NAME);
    let mut inventory = CollectedInventory {
        active_setup_hexes: active_setup_hexes.to_vec(),
        active_generation_hexes: active_generation_hexes.to_vec(),
        ..CollectedInventory::default()
    };

    let current_link = setups_dir.join(CURRENT_LINK_NAME);
    match fs::symlink_metadata(&current_link) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => {
            return Err(CleanError::CurrentInvalid {
                reason: format!("cannot inspect {}: {e}", current_link.display()),
            });
        }
        Ok(meta) => {
            if !meta.file_type().is_symlink() {
                return Err(CleanError::CurrentInvalid {
                    reason: format!(
                        "{} is not a symlink; refusing to adopt foreign state",
                        current_link.display()
                    ),
                });
            }
            let target = fs::read_link(&current_link).map_err(|e| CleanError::CurrentInvalid {
                reason: format!("cannot read {}: {e}", current_link.display()),
            })?;
            let hex = target
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default()
                .to_owned();
            if GenerationId::new(&hex).is_err() {
                return Err(CleanError::CurrentInvalid {
                    reason: format!(
                        "{} points at {hex:?}, want a setup digest; refusing digest-spoofed state",
                        current_link.display()
                    ),
                });
            }
            inventory.current_hex = Some(hex);
        }
    }

    for name in entry_names(&setups_dir)? {
        if name == CURRENT_LINK_NAME || name == CURRENT_STAGE_NAME {
            continue;
        }
        if GenerationId::new(&name).is_err() {
            inventory.unmanaged_names.push(name);
            continue;
        }
        let record = setups_dir.join(&name);
        let environment = fs::read_link(record.join(ENVIRONMENT_LINK_NAME))
            .ok()
            .and_then(|target| generation_hex_from_link_target(&target));
        let generated = fs::read_link(record.join(GENERATED_LINK_NAME))
            .ok()
            .and_then(|target| generation_hex_from_link_target(&target));
        match (environment, generated) {
            (Some(environment_hex), Some(generated_hex)) => {
                match validate_record(&name, &environment_hex, &generated_hex) {
                    Ok(view) => inventory.records.push(view),
                    Err(_) => inventory.unmanaged_names.push(name),
                }
            }
            _ => inventory.unmanaged_names.push(name),
        }
    }

    for kind in [GenerationKind::Environment, GenerationKind::Generated] {
        let dir = dx_dir.join(kind.dir_name());
        for name in entry_names(&dir)? {
            if GenerationId::new(&name).is_ok() {
                inventory
                    .generations
                    .push(GenerationView { kind, hex: name });
            } else {
                inventory.unmanaged_names.push(name);
            }
        }
    }

    inventory
        .records
        .sort_by(|left, right| left.hex.cmp(&right.hex));
    inventory.generations.sort_by(|left, right| {
        left.kind
            .dir_name()
            .cmp(right.kind.dir_name())
            .then_with(|| left.hex.cmp(&right.hex))
    });
    inventory.unmanaged_names.sort();
    inventory.unmanaged_names.dedup();
    Ok(inventory)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures::*;

    #[test]
    fn walk_filtered_skips_gitignored_and_glob_excluded_files() {
        let scratch = dx_test_scratch::scratch("dx-clean-walk-");
        let root = scratch.path();
        fs::write(root.join(".gitignore"), "ignored.txt\n").expect("gitignore");
        fs::write(root.join("ignored.txt"), "skip me").expect("ignored file");
        fs::write(root.join("kept.txt"), "keep me").expect("kept file");
        fs::write(root.join("skip.log"), "glob me").expect("glob file");
        fs::create_dir_all(root.join("sub")).expect("subdir");
        fs::write(root.join("sub").join("nested.txt"), "nested").expect("nested");

        let walked = walk_filtered(root, &[]).expect("walk");
        let names: Vec<String> = walked
            .iter()
            .filter_map(|path| {
                path.strip_prefix(root)
                    .ok()
                    .and_then(|relative| relative.to_str().map(str::to_owned))
            })
            .collect();
        assert!(
            names.iter().any(|name| name == "kept.txt"),
            "kept file must walk: {names:?}"
        );
        assert!(
            names
                .iter()
                .any(|name| name == "sub/nested.txt" || name == "sub\\nested.txt"),
            "nested file must walk: {names:?}"
        );
        assert!(
            !names.iter().any(|name| name == "ignored.txt"),
            "gitignored file must be skipped: {names:?}"
        );

        let filtered = walk_filtered(root, &["*.log".to_owned()]).expect("filtered walk");
        let filtered_names: Vec<String> = filtered
            .iter()
            .filter_map(|path| {
                path.strip_prefix(root)
                    .ok()
                    .and_then(|relative| relative.to_str().map(str::to_owned))
            })
            .collect();
        assert!(
            !filtered_names.iter().any(|name| name == "skip.log"),
            "glob-excluded file must be skipped: {filtered_names:?}"
        );
        assert!(
            filtered_names.iter().any(|name| name == "kept.txt"),
            "kept file must survive glob filtering: {filtered_names:?}"
        );
        let mut sorted = filtered.clone();
        sorted.sort();
        assert_eq!(filtered, sorted, "walks must be sorted");
    }

    #[test]
    fn walk_filtered_rejects_invalid_globs_and_walks_missing_empty() {
        let missing = std::env::temp_dir().join("dx-clean-missing-walk-root");
        let _ = fs::remove_dir_all(&missing);
        let empty = walk_filtered(&missing, &[]).expect("missing walks empty");
        assert!(empty.is_empty());
        let scratch = dx_test_scratch::scratch("dx-clean-glob-");
        assert!(matches!(
            walk_filtered(scratch.path(), &["[invalid".to_owned()]),
            Err(CleanError::Install { .. })
        ));
    }

    #[cfg(windows)]
    fn symlink_dir(target: &Path, link: &Path) {
        std::os::windows::fs::symlink_dir(target, link).expect("stage test link");
    }

    #[cfg(not(windows))]
    fn symlink_dir(target: &Path, link: &Path) {
        std::os::unix::fs::symlink(target, link).expect("stage test link");
    }

    #[test]
    fn inventory_validates_records_and_flags_unmanaged() {
        let scratch = {
            let __scratch = dx_test_scratch::scratch("dx-clean-test-inventory-");
            std::fs::create_dir_all(__scratch.path().join("ws")).expect("create workspace");
            __scratch
        };
        let root = scratch.path().to_path_buf();
        let (workspace, stale_hex, current_hex) = two_record_workspace(&root);
        let dx_dir = workspace.join(".dx");
        fs::create_dir_all(dx_dir.join("environments").join("latest")).expect("unmanaged gen");
        fs::create_dir_all(dx_dir.join("generated").join("not-hex")).expect("unmanaged gen");
        fs::create_dir_all(dx_dir.join("setups").join("scratch")).expect("unmanaged record");
        let (spoofed_hex, _, _) = pair_env_gen('5', '6');
        let spoofed = dx_dir.join("setups").join(&spoofed_hex);
        fs::create_dir_all(&spoofed).expect("spoof record");
        symlink_dir(
            Path::new(&format!("../../environments/{}/", digest('1'))),
            &spoofed.join("environment"),
        );
        symlink_dir(
            Path::new(&format!("../../generated/{}/", digest('2'))),
            &spoofed.join("generated"),
        );

        let inventory = collect_inventory(&workspace, &[], &[]).expect("collect");
        assert_eq!(inventory.current_hex, Some(current_hex.clone()));
        let hexes: Vec<&str> = inventory
            .records
            .iter()
            .map(|record| record.hex.as_str())
            .collect();
        assert!(hexes.contains(&stale_hex.as_str()));
        assert!(hexes.contains(&current_hex.as_str()));
        assert!(!hexes.contains(&spoofed_hex.as_str()));
        assert_eq!(inventory.generations.len(), 4);
        for refused in ["latest", "not-hex", "scratch", spoofed_hex.as_str()] {
            assert!(
                inventory.unmanaged_names.contains(&refused.to_owned()),
                "refused names must contain {refused:?}: {:?}",
                inventory.unmanaged_names
            );
        }
        let plan = inventory.plan();
        assert!(!plan.prune_setup_records.contains(&spoofed_hex));
        assert!(!plan
            .prune_generations
            .iter()
            .any(|generation| generation.hex == "latest" || generation.hex == "not-hex"));
        let _ = fs::remove_dir_all(&root);
    }
}
