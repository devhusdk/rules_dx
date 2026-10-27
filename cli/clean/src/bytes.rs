use std::fs;
use std::io;
use std::path::Path;

use dx_env::DX_DIR_NAME;
use dx_setup::SETUPS_DIR_NAME;

use super::apply::CleanOutcome;
use super::planning::{CleanPlan, GenerationView};
use super::CleanError;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PruneBytes {
    pub setup_record_bytes: Vec<(String, u64)>,
    pub generation_bytes: Vec<(GenerationView, u64)>,
}

impl PruneBytes {
    pub fn total(&self) -> u64 {
        let mut total = 0u64;
        for (_, bytes) in &self.setup_record_bytes {
            total = total.saturating_add(*bytes);
        }
        for (_, bytes) in &self.generation_bytes {
            total = total.saturating_add(*bytes);
        }
        total
    }

    pub fn reclaimed(&self, outcome: &CleanOutcome) -> u64 {
        let mut total = 0u64;
        for removed in &outcome.removed_setup_records {
            for (hex, bytes) in &self.setup_record_bytes {
                if hex == removed {
                    total = total.saturating_add(*bytes);
                }
            }
        }
        for removed in &outcome.removed_generations {
            for (generation, bytes) in &self.generation_bytes {
                if generation == removed {
                    total = total.saturating_add(*bytes);
                }
            }
        }
        total
    }
}

fn dir_bytes(dir: &Path) -> Result<u64, CleanError> {
    let fail = |reason: String| CleanError::Install { reason };
    if fs::read_dir(dir).is_err_and(|e| e.kind() == io::ErrorKind::NotFound) {
        return Ok(0);
    }
    let mut total = 0u64;
    for entry in walkdir::WalkDir::new(dir).follow_links(false) {
        let entry = entry.map_err(|e| fail(format!("cannot list {}: {e}", dir.display())))?;
        let path = entry.path();
        if path == dir {
            continue;
        }
        let meta = fs::symlink_metadata(path)
            .map_err(|e| fail(format!("cannot inspect {}: {e}", path.display())))?;
        if meta.is_dir() {
            continue;
        }
        let len = if meta.file_type().is_symlink() {
            fs::read_link(path)
                .map_err(|e| fail(format!("cannot read link {}: {e}", path.display())))?
                .as_os_str()
                .len() as u64
        } else {
            meta.len()
        };
        total = total.saturating_add(len);
    }
    Ok(total)
}

pub fn measure_prune_bytes(
    workspace_root: &Path,
    plan: &CleanPlan,
) -> Result<PruneBytes, CleanError> {
    measure(
        workspace_root,
        &plan.prune_setup_records,
        &plan.prune_generations,
    )
}

pub fn measure_preserved_bytes(
    workspace_root: &Path,
    plan: &CleanPlan,
) -> Result<PruneBytes, CleanError> {
    measure(
        workspace_root,
        &plan.preserved_unobserved_setup_records,
        &plan.preserved_unobserved_generations,
    )
}

fn measure(
    workspace_root: &Path,
    setup_records: &[String],
    generations: &[GenerationView],
) -> Result<PruneBytes, CleanError> {
    if !workspace_root.is_dir() {
        return Err(CleanError::WorkspaceRoot {
            path: workspace_root.to_path_buf(),
        });
    }
    let dx_dir = workspace_root.join(DX_DIR_NAME);
    let setups_dir = dx_dir.join(SETUPS_DIR_NAME);
    let mut measured = PruneBytes::default();
    for hex in setup_records {
        let bytes = dir_bytes(&setups_dir.join(hex))?;
        measured.setup_record_bytes.push((hex.clone(), bytes));
    }
    for generation in generations {
        let bytes = dir_bytes(
            &dx_dir
                .join(generation.kind.dir_name())
                .join(&generation.hex),
        )?;
        measured.generation_bytes.push((generation.clone(), bytes));
    }
    Ok(measured)
}

pub fn render_prune_listing(plan: &CleanPlan, bytes: &PruneBytes) -> String {
    let mut lines = vec!["dx clean: reclaimable managed state".to_owned()];
    if plan.prune_setup_records.is_empty()
        && plan.prune_generations.is_empty()
        && plan.preserved_unobserved_setup_records.is_empty()
        && plan.preserved_unobserved_generations.is_empty()
    {
        lines.push("nothing to prune".to_owned());
    }
    let preserved_unobserved = !plan.preserved_unobserved_setup_records.is_empty()
        || !plan.preserved_unobserved_generations.is_empty();
    let pruned_unobserved =
        !plan.prune_setup_records.is_empty() || !plan.prune_generations.is_empty();
    if plan.observation_unknown && preserved_unobserved {
        lines.push(
            "process observation unavailable: unobserved state is preserved, never assumed idle"
                .to_owned(),
        );
    } else if plan.observation_unknown && pruned_unobserved {
        lines.push(
            "process observation unavailable: unobserved state pruned under --prune-unobserved"
                .to_owned(),
        );
    }
    for hex in &plan.prune_setup_records {
        let size = bytes
            .setup_record_bytes
            .iter()
            .find(|(entry, _)| entry == hex)
            .map(|(_, size)| *size)
            .unwrap_or(0);
        lines.push(format!(
            "prune setup record: .dx/setups/{hex} ({size} bytes)"
        ));
    }
    for generation in &plan.prune_generations {
        let size = bytes
            .generation_bytes
            .iter()
            .find(|(entry, _)| entry == generation)
            .map(|(_, size)| *size)
            .unwrap_or(0);
        lines.push(format!(
            "prune generation: .dx/{}/{} ({size} bytes)",
            generation.kind.dir_name(),
            generation.hex
        ));
    }
    match &plan.preserved_current {
        Some(current) => lines.push(format!("preserve current: .dx/setups/{current}")),
        None => lines.push("no current selection".to_owned()),
    }
    for hex in &plan.preserved_unobserved_setup_records {
        lines.push(format!(
            "preserve unobserved setup record: .dx/setups/{hex}"
        ));
    }
    for generation in &plan.preserved_unobserved_generations {
        lines.push(format!(
            "preserve unobserved generation: .dx/{}/{}",
            generation.kind.dir_name(),
            generation.hex
        ));
    }
    for unmanaged in &plan.refused_unmanaged {
        lines.push(format!("refuse unmanaged path: {unmanaged}"));
    }
    lines.push(format!("reclaimable total: {} bytes", bytes.total()));
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures::*;
    use crate::inventory::collect_inventory;
    use crate::planning::{plan_prune, PruneInputs, UnobservedPolicy};
    use crate::records::SetupRecordView;
    use dx_atomic_fs::lease::GenerationUse;

    fn inputs<'a>(
        records: &'a [SetupRecordView],
        generations: &'a [GenerationView],
        current_hex: Option<&'a str>,
    ) -> PruneInputs<'a> {
        PruneInputs {
            records,
            generations,
            current_hex,
            active_setup_hexes: &[],
            active_generation_hexes: &[],
            unmanaged_names: &[],
            observation_unknown: false,
            unobserved: UnobservedPolicy::Preserve,
        }
    }

    #[test]
    fn listing_lists_prune_preserve_and_refuse() {
        let current = record('1', '2');
        let stale = record('3', '4');
        let records = vec![current.clone(), stale.clone()];
        let generations = vec![generation(GenerationUse::Environment, '3')];
        let unmanaged = vec!["latest".to_owned()];
        let plan = plan_prune(PruneInputs {
            records: &records,
            generations: &generations,
            current_hex: Some(&current.hex),
            active_setup_hexes: &[],
            active_generation_hexes: &[],
            unmanaged_names: &unmanaged,
            observation_unknown: false,
            unobserved: UnobservedPolicy::Preserve,
        });
        let listing = render_prune_listing(&plan, &PruneBytes::default());
        assert!(listing.contains(&format!(".dx/setups/{}", stale.hex)));
        assert!(listing.contains(&format!(".dx/environments/{}", digest('3'))));
        assert!(listing.contains(&format!(".dx/setups/{}", current.hex)));
        assert!(listing.contains("latest"));
        assert!(listing.contains("(0 bytes)"));
        assert!(listing.contains("reclaimable total: 0 bytes"));
    }

    #[test]
    fn listing_reports_empty_prune_set() {
        let current = record('1', '2');
        let records = vec![current.clone()];
        let plan = plan_prune(inputs(&records, &[], Some(&current.hex)));
        let listing = render_prune_listing(&plan, &PruneBytes::default());
        assert!(listing.contains("nothing to prune"));
        assert!(listing.contains("reclaimable total: 0 bytes"));
    }

    #[test]
    fn listing_omits_observation_line_without_unobserved_state() {
        let current = record('1', '2');
        let records = vec![current.clone()];
        let plan = plan_prune(PruneInputs {
            observation_unknown: true,
            ..inputs(&records, &[], Some(&current.hex))
        });
        let listing = render_prune_listing(&plan, &PruneBytes::default());
        assert!(
            !listing.contains("process observation unavailable"),
            "{listing}"
        );
        assert!(listing.contains("nothing to prune"), "{listing}");
    }

    #[test]
    fn listing_reports_preserved_unobserved_state() {
        let current = record('1', '2');
        let stale = record('3', '4');
        let records = vec![current.clone(), stale.clone()];
        let plan = plan_prune(PruneInputs {
            observation_unknown: true,
            ..inputs(&records, &[], Some(&current.hex))
        });
        let listing = render_prune_listing(&plan, &PruneBytes::default());
        assert!(
            listing.contains(
                "process observation unavailable: unobserved state is preserved, never assumed idle"
            ),
            "{listing}"
        );
        assert!(
            listing.contains(&format!(
                "preserve unobserved setup record: .dx/setups/{}",
                stale.hex
            )),
            "{listing}"
        );
        assert!(
            !listing.contains("pruned under --prune-unobserved"),
            "{listing}"
        );
        assert!(!listing.contains("prune setup record:"), "{listing}");
    }

    #[test]
    fn listing_reports_unobserved_state_pruned_when_acknowledged() {
        let current = record('1', '2');
        let stale = record('3', '4');
        let records = vec![current.clone(), stale.clone()];
        let plan = plan_prune(PruneInputs {
            observation_unknown: true,
            unobserved: UnobservedPolicy::PruneAcknowledged,
            ..inputs(&records, &[], Some(&current.hex))
        });
        let listing = render_prune_listing(&plan, &PruneBytes::default());
        assert!(
            listing.contains(
                "process observation unavailable: unobserved state pruned under --prune-unobserved"
            ),
            "{listing}"
        );
        assert!(
            listing.contains(&format!("prune setup record: .dx/setups/{}", stale.hex)),
            "{listing}"
        );
        assert!(!listing.contains("is preserved"), "{listing}");
        assert!(!listing.contains("preserve unobserved"), "{listing}");
    }

    #[test]
    fn measure_sums_prune_entries_and_skips_missing() {
        let scratch = {
            let __scratch = dx_test_scratch::scratch("dx-clean-test-measure-");
            std::fs::create_dir_all(__scratch.path().join("ws")).expect("create workspace");
            __scratch
        };
        let root = scratch.path().to_path_buf();
        let (workspace, stale_hex, _) = two_record_workspace(&root);
        let plan = collect_inventory(&workspace, &[], &[])
            .expect("collect")
            .plan();
        let bytes = measure_prune_bytes(&workspace, &plan).expect("measure");
        let (measured_hex, record_bytes) = bytes
            .setup_record_bytes
            .iter()
            .find(|(hex, _)| hex == &stale_hex)
            .expect("stale record measured");
        assert_eq!(measured_hex, &stale_hex);
        assert!(
            *record_bytes > 0,
            "pair links measure nonzero: {record_bytes}"
        );
        assert_eq!(bytes.generation_bytes.len(), 2);
        assert!(bytes.generation_bytes.iter().all(|(_, size)| *size == 0));
        assert_eq!(
            bytes.total(),
            *record_bytes,
            "empty generation dirs add nothing"
        );
        let ghost = CleanPlan {
            prune_setup_records: vec![digest('a')],
            prune_generations: vec![GenerationView {
                kind: GenerationUse::Environment,
                hex: digest('b'),
            }],
            refused_unmanaged: Vec::new(),
            preserved_current: None,
            observation_unknown: false,
            preserved_unobserved_setup_records: Vec::new(),
            preserved_unobserved_generations: Vec::new(),
        };
        let ghost_bytes = measure_prune_bytes(&workspace, &ghost).expect("ghost measure");
        assert_eq!(ghost_bytes.total(), 0);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn measure_never_follows_symlinks() {
        let scratch = {
            let __scratch = dx_test_scratch::scratch("dx-clean-test-measure-nofollow-");
            std::fs::create_dir_all(__scratch.path().join("ws")).expect("create workspace");
            __scratch
        };
        let root = scratch.path().to_path_buf();
        let workspace = workspace_of(&root);
        dx_setup::commit_pair(&workspace, &setup_pair('1', '2')).expect("commit");
        let dx_dir = workspace.join(".dx");
        let outside = root.join("fat.bin");
        fs::write(&outside, vec![7u8; 1 << 20]).expect("fat file");
        let stale_gen = dx_dir.join("generated").join(digest('9'));
        fs::create_dir_all(&stale_gen).expect("stale generation");
        stage_symlink(&outside, &stale_gen.join("artifact"));
        let plan = CleanPlan {
            prune_setup_records: Vec::new(),
            prune_generations: vec![GenerationView {
                kind: GenerationUse::Generated,
                hex: digest('9'),
            }],
            refused_unmanaged: Vec::new(),
            preserved_current: None,
            observation_unknown: false,
            preserved_unobserved_setup_records: Vec::new(),
            preserved_unobserved_generations: Vec::new(),
        };
        let bytes = measure_prune_bytes(&workspace, &plan).expect("measure");
        assert!(
            bytes.total() < 1 << 20,
            "link target must not count: {}",
            bytes.total()
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn measure_preserved_counts_only_preserved_entries() {
        let scratch = {
            let __scratch = dx_test_scratch::scratch("dx-clean-test-measure-preserved-");
            std::fs::create_dir_all(__scratch.path().join("ws")).expect("create workspace");
            __scratch
        };
        let root = scratch.path().to_path_buf();
        let workspace = workspace_of(&root);
        let dx_dir = workspace.join(".dx");
        let kept_gen = dx_dir.join("generated").join(digest('8'));
        fs::create_dir_all(&kept_gen).expect("kept generation");
        fs::write(kept_gen.join("artifact"), b"12345").expect("artifact");
        let kept = GenerationView {
            kind: GenerationUse::Generated,
            hex: digest('8'),
        };
        let plan = CleanPlan {
            prune_setup_records: Vec::new(),
            prune_generations: Vec::new(),
            refused_unmanaged: Vec::new(),
            preserved_current: None,
            observation_unknown: true,
            preserved_unobserved_setup_records: Vec::new(),
            preserved_unobserved_generations: vec![kept.clone()],
        };
        let preserved = measure_preserved_bytes(&workspace, &plan).expect("measure preserved");
        assert_eq!(preserved.generation_bytes, vec![(kept, 5)]);
        let prune = measure_prune_bytes(&workspace, &plan).expect("measure prune");
        assert_eq!(prune.total(), 0);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn measure_missing_workspace_fails() {
        let scratch = {
            let __scratch = dx_test_scratch::scratch("dx-clean-test-measure-missing-");
            std::fs::create_dir_all(__scratch.path().join("ws")).expect("create workspace");
            __scratch
        };
        let root = scratch.path().to_path_buf();
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
            measure_prune_bytes(&root.join("no-such-dir"), &plan),
            Err(CleanError::WorkspaceRoot { .. })
        ));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn reclaimed_counts_only_removed_entries() {
        let removed_hex = digest('3');
        let removed_generation = GenerationView {
            kind: GenerationUse::Environment,
            hex: digest('5'),
        };
        let bytes = PruneBytes {
            setup_record_bytes: vec![(removed_hex.clone(), 100), (digest('4'), 200)],
            generation_bytes: vec![
                (removed_generation.clone(), 300),
                (
                    GenerationView {
                        kind: GenerationUse::Generated,
                        hex: digest('6'),
                    },
                    400,
                ),
            ],
        };
        assert_eq!(bytes.total(), 1000);
        let outcome = CleanOutcome {
            removed_setup_records: vec![removed_hex],
            removed_generations: vec![removed_generation],
            skipped_leased_generations: Vec::new(),
        };
        assert_eq!(bytes.reclaimed(&outcome), 400);
        assert_eq!(
            bytes.reclaimed(&CleanOutcome::default()),
            0,
            "empty outcome reclaims nothing"
        );
    }

    #[test]
    fn render_lists_per_entry_bytes_and_total() {
        let stale = record('3', '4');
        let current = record('1', '2');
        let records = vec![current.clone(), stale.clone()];
        let generations = vec![generation(GenerationUse::Environment, '3')];
        let plan = plan_prune(inputs(&records, &generations, Some(&current.hex)));
        let bytes = PruneBytes {
            setup_record_bytes: vec![(stale.hex.clone(), 120)],
            generation_bytes: vec![(generation(GenerationUse::Environment, '3'), 34)],
        };
        let listing = render_prune_listing(&plan, &bytes);
        assert!(
            listing.contains(&format!(
                "prune setup record: .dx/setups/{} (120 bytes)",
                stale.hex
            )),
            "{listing}"
        );
        assert!(
            listing.contains(&format!(
                "prune generation: .dx/environments/{} (34 bytes)",
                digest('3')
            )),
            "{listing}"
        );
        assert!(
            listing.contains("reclaimable total: 154 bytes"),
            "{listing}"
        );
    }
}
