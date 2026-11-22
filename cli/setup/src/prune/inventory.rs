use std::fs;
use std::io;
use std::path::Path;

use crate::{
    CURRENT_LINK_NAME, CURRENT_STAGE_NAME, ENVIRONMENT_LINK_NAME, GENERATED_LINK_NAME,
    GenerationId, SETUPS_DIR_NAME,
};

use crate::prune::PruneError;
use crate::prune::planning::{GenerationView, PruneInputs, PrunePlan, plan_prune};
use crate::prune::records::{SetupRecordView, validate_record};
use dx_atomic_fs::lease::GenerationUse;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CollectedInventory {
    pub records: Vec<SetupRecordView>,
    pub generations: Vec<GenerationView>,
    pub current_hex: Option<String>,
    pub active_setup_hexes: Vec<String>,
    pub active_generation_hexes: Vec<String>,
    pub unmanaged_names: Vec<String>,
    pub observation_unknown: bool,
    pub previous_hex: Option<String>,
}

impl CollectedInventory {
    #[must_use]
    pub fn prune_inputs(&self) -> PruneInputs<'_> {
        PruneInputs {
            records: &self.records,
            generations: &self.generations,
            current_hex: self.current_hex.as_deref(),
            active_setup_hexes: &self.active_setup_hexes,
            active_generation_hexes: &self.active_generation_hexes,
            unmanaged_names: &self.unmanaged_names,
            observation_unknown: self.observation_unknown,
            previous_hex: self.previous_hex.as_deref(),
        }
    }

    #[must_use]
    pub fn plan(&self) -> PrunePlan {
        plan_prune(self.prune_inputs())
    }
}

fn entry_names(dir: &Path) -> Result<Vec<String>, PruneError> {
    match fs::metadata(dir) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => {
            return Err(PruneError::Install {
                reason: format!("cannot list {}: {e}", dir.display()),
            });
        }
        Ok(meta) if !meta.is_dir() => {
            return Err(PruneError::Install {
                reason: format!("{} is not a directory", dir.display()),
            });
        }
        Ok(_) => {}
    }
    let mut names = Vec::new();
    for entry in walkdir::WalkDir::new(dir).max_depth(1).min_depth(1) {
        let entry = entry.map_err(|e| PruneError::Install {
            reason: format!("cannot list {}: {e}", dir.display()),
        })?;
        names.push(entry.file_name().to_string_lossy().into_owned());
    }
    names.sort();
    Ok(names)
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
) -> Result<CollectedInventory, PruneError> {
    if !workspace_root.is_dir() {
        return Err(PruneError::WorkspaceRoot {
            path: workspace_root.to_path_buf(),
        });
    }
    let dx_dir = dx_path::state::local_dir(workspace_root);
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
            return Err(PruneError::CurrentInvalid {
                reason: format!("cannot inspect {}: {e}", current_link.display()),
            });
        }
        Ok(meta) => {
            if !meta.file_type().is_symlink() {
                return Err(PruneError::CurrentInvalid {
                    reason: format!(
                        "{} is not a symlink; refusing to adopt foreign state",
                        current_link.display()
                    ),
                });
            }
            let target = fs::read_link(&current_link).map_err(|e| PruneError::CurrentInvalid {
                reason: format!("cannot read {}: {e}", current_link.display()),
            })?;
            let hex = target.to_str().unwrap_or_default().to_owned();
            if GenerationId::new(&hex).is_err() {
                return Err(PruneError::CurrentInvalid {
                    reason: format!(
                        "{} points at {target:?}, want a setup digest beside it; refusing foreign or digest-spoofed state",
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

    for kind in [GenerationUse::Environment, GenerationUse::Generated] {
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
    use crate::prune::fixtures::*;

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
            let __scratch = dx_test_scratch::scratch("dx-prune-test-inventory-").unwrap();
            std::fs::create_dir_all(__scratch.path().join("ws")).expect("create workspace");
            __scratch
        };
        let root = scratch.path().to_path_buf();
        let (workspace, stale_hex, current_hex) = two_record_workspace(&root);
        let dx_dir = dx_path::state::local_dir(&workspace);
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
        assert!(
            !plan
                .prune_generations
                .iter()
                .any(|generation| generation.hex == "latest" || generation.hex == "not-hex")
        );
        dx_atomic_fs::discard_dir(&root);
    }

    #[test]
    fn generation_root_that_is_a_file_is_an_error() {
        let scratch = dx_test_scratch::scratch("dx-prune-test-file-root-").unwrap();
        let workspace = workspace_of(scratch.path());
        let dx_dir = dx_path::state::local_dir(&workspace);
        fs::create_dir_all(&dx_dir).expect("dx dir");
        fs::write(dx_dir.join("environments"), "not a directory").expect("file root");
        assert!(matches!(
            collect_inventory(&workspace, &[], &[]),
            Err(PruneError::Install { .. })
        ));
    }

    #[test]
    fn current_pointer_outside_setups_is_refused() {
        let scratch = dx_test_scratch::scratch("dx-prune-test-pointer-outside-").unwrap();
        let workspace = workspace_of(scratch.path());
        fs::create_dir_all(&workspace).expect("workspace");
        crate::commit_pair(&workspace, &setup_pair('1', '2')).expect("commit");
        let (hex, _, _) = pair_env_gen('1', '2');
        let foreign = scratch.path().join("foreign").join(&hex);
        fs::create_dir_all(&foreign).expect("foreign record");
        let current = dx_path::state::local_dir(&workspace)
            .join("setups")
            .join("current");
        dx_test_scratch::remove_directory_link(&current).expect("remove pointer");
        symlink_dir(&foreign, &current);
        assert!(matches!(
            collect_inventory(&workspace, &[], &[]),
            Err(PruneError::CurrentInvalid { .. })
        ));
    }

    #[cfg(unix)]
    #[test]
    fn distinct_non_utf8_names_stay_distinct() {
        use std::ffi::OsStr;
        use std::os::unix::ffi::OsStrExt;
        let scratch = dx_test_scratch::scratch("dx-prune-test-non-utf8-").unwrap();
        let workspace = workspace_of(scratch.path());
        let environments = dx_path::state::local_dir(&workspace).join("environments");
        fs::create_dir_all(&environments).expect("environments");
        for raw in [&b"a\xff"[..], &b"b\xfe"[..]] {
            fs::create_dir_all(environments.join(OsStr::from_bytes(raw))).expect("odd name");
        }
        let inventory = collect_inventory(&workspace, &[], &[]).expect("collect");
        assert_eq!(inventory.unmanaged_names.len(), 2);
    }
}
