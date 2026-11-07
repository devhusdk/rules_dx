use crate::prune::records::SetupRecordView;
use dx_atomic_fs::lease::GenerationUse;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenerationView {
    pub kind: GenerationUse,
    pub hex: String,
}

pub struct PruneInputs<'a> {
    pub records: &'a [SetupRecordView],
    pub generations: &'a [GenerationView],
    pub current_hex: Option<&'a str>,
    pub active_setup_hexes: &'a [String],
    pub active_generation_hexes: &'a [String],
    pub unmanaged_names: &'a [String],
    pub observation_unknown: bool,
    /// The setup a sync just replaced. When processes cannot be listed at
    /// all, it is the only one kept besides the current setup. Callers leave
    /// it `None` when only some processes were unreadable, so all is kept.
    pub previous_hex: Option<&'a str>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrunePlan {
    pub prune_setup_records: Vec<String>,
    pub prune_generations: Vec<GenerationView>,
    pub refused_unmanaged: Vec<String>,
    pub preserved_current: Option<String>,
    pub observation_unknown: bool,
    pub preserved_unobserved_setup_records: Vec<String>,
    pub preserved_unobserved_generations: Vec<GenerationView>,
}

impl PrunePlan {
    /// Whether the plan would delete any setup record or generation.
    pub fn has_prune(&self) -> bool {
        !self.prune_setup_records.is_empty() || !self.prune_generations.is_empty()
    }

    /// Whether the plan keeps any state because process observation is unavailable.
    pub fn has_preserved_unobserved(&self) -> bool {
        !self.preserved_unobserved_setup_records.is_empty()
            || !self.preserved_unobserved_generations.is_empty()
    }
}

pub fn plan_prune(inputs: PruneInputs<'_>) -> PrunePlan {
    let mut retained: Vec<&str> = Vec::new();
    if let Some(current) = inputs.current_hex {
        retained.push(current);
    }
    retained.extend(
        inputs
            .active_setup_hexes
            .iter()
            .map(std::string::String::as_str),
    );
    retained.sort_unstable();
    retained.dedup();

    let mut prune_setup_records: Vec<String> = inputs
        .records
        .iter()
        .filter(|record| !retained.contains(&record.hex.as_str()))
        .map(|record| record.hex.clone())
        .collect();
    prune_setup_records.sort();
    prune_setup_records.dedup();

    let mut referenced: Vec<(GenerationUse, &str)> = Vec::new();
    for record in inputs.records {
        if retained.contains(&record.hex.as_str()) {
            referenced.push((GenerationUse::Environment, record.environment_hex.as_str()));
            referenced.push((GenerationUse::Generated, record.generated_hex.as_str()));
        }
    }
    let mut prune_generations: Vec<GenerationView> = inputs
        .generations
        .iter()
        .filter(|generation| {
            !inputs
                .active_generation_hexes
                .iter()
                .any(|active| active == &generation.hex)
                && !referenced
                    .iter()
                    .any(|(kind, hex)| *kind == generation.kind && *hex == generation.hex.as_str())
        })
        .cloned()
        .collect();
    prune_generations.sort_by(|left, right| {
        left.kind
            .dir_name()
            .cmp(right.kind.dir_name())
            .then_with(|| left.hex.cmp(&right.hex))
    });

    let mut refused_unmanaged: Vec<String> =
        inputs.unmanaged_names.iter().map(Clone::clone).collect();
    refused_unmanaged.sort();
    refused_unmanaged.dedup();

    let mut preserved_unobserved_setup_records = Vec::new();
    let mut preserved_unobserved_generations = Vec::new();
    if inputs.observation_unknown {
        match inputs.previous_hex {
            None => {
                preserved_unobserved_setup_records = std::mem::take(&mut prune_setup_records);
                preserved_unobserved_generations = std::mem::take(&mut prune_generations);
            }
            Some(previous) => {
                let sides = inputs
                    .records
                    .iter()
                    .find(|record| record.hex == previous)
                    .map(|record| {
                        (
                            record.environment_hex.as_str(),
                            record.generated_hex.as_str(),
                        )
                    });
                let of_previous = |generation: &GenerationView| {
                    sides.is_some_and(|(environment, generated)| match generation.kind {
                        GenerationUse::Environment => generation.hex == environment,
                        GenerationUse::Generated => generation.hex == generated,
                    })
                };
                (preserved_unobserved_setup_records, prune_setup_records) = prune_setup_records
                    .into_iter()
                    .partition(|hex| hex == previous);
                (preserved_unobserved_generations, prune_generations) =
                    prune_generations.into_iter().partition(of_previous);
            }
        }
    }

    PrunePlan {
        prune_setup_records,
        prune_generations,
        refused_unmanaged,
        preserved_current: inputs.current_hex.map(str::to_owned),
        observation_unknown: inputs.observation_unknown,
        preserved_unobserved_setup_records,
        preserved_unobserved_generations,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prune::records::validate_record;

    fn digest(tag: char) -> String {
        tag.to_string().repeat(64)
    }

    fn record(env: char, gen_id: char) -> SetupRecordView {
        let environment_hex = digest(env);
        let generated_hex = digest(gen_id);
        let pair = crate::SetupPair {
            environment: crate::GenerationId::new(&environment_hex).expect("env digest"),
            generated: crate::GenerationId::new(&generated_hex).expect("gen digest"),
        };
        let hex = crate::setup_hex(&pair);
        validate_record(&hex, &environment_hex, &generated_hex).expect("valid record")
    }

    fn generation(kind: GenerationUse, tag: char) -> GenerationView {
        GenerationView {
            kind,
            hex: digest(tag),
        }
    }

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
            previous_hex: None,
        }
    }

    #[test]
    fn current_record_and_its_generations_are_preserved() {
        let current = record('1', '2');
        let stale = record('3', '4');
        let records = vec![current.clone(), stale.clone()];
        let generations = vec![
            generation(GenerationUse::Environment, '1'),
            generation(GenerationUse::Generated, '2'),
            generation(GenerationUse::Environment, '3'),
            generation(GenerationUse::Generated, '4'),
        ];
        let plan = plan_prune(inputs(&records, &generations, Some(&current.hex)));
        assert_eq!(plan.prune_setup_records, vec![stale.hex.clone()]);
        assert_eq!(
            plan.prune_generations,
            vec![
                generation(GenerationUse::Environment, '3'),
                generation(GenerationUse::Generated, '4'),
            ]
        );
        assert_eq!(plan.preserved_current, Some(current.hex.clone()));
        assert!(plan.refused_unmanaged.is_empty());
    }

    #[test]
    fn generations_shared_with_retained_records_are_preserved() {
        let current = record('1', '2');
        let records = vec![current.clone()];
        let generations = vec![
            generation(GenerationUse::Environment, '1'),
            generation(GenerationUse::Generated, '2'),
        ];
        let plan = plan_prune(inputs(&records, &generations, Some(&current.hex)));
        assert!(plan.prune_setup_records.is_empty());
        assert!(plan.prune_generations.is_empty());
    }

    #[test]
    fn active_records_and_generations_are_preserved() {
        let current = record('1', '2');
        let active_record = record('3', '4');
        let records = vec![current.clone(), active_record.clone()];
        let generations = vec![
            generation(GenerationUse::Environment, '3'),
            generation(GenerationUse::Generated, '4'),
        ];
        let active_setup = vec![active_record.hex.clone()];
        let active_generations = vec![digest('3'), digest('4')];
        let plan = plan_prune(PruneInputs {
            records: &records,
            generations: &generations,
            current_hex: Some(&current.hex),
            active_setup_hexes: &active_setup,
            active_generation_hexes: &active_generations,
            unmanaged_names: &[],
            observation_unknown: false,
            previous_hex: None,
        });
        assert!(plan.prune_setup_records.is_empty());
        assert!(plan.prune_generations.is_empty());
    }

    #[test]
    fn unmanaged_names_are_refused_never_pruned() {
        let records = vec![record('1', '2')];
        let unmanaged = vec!["latest".to_owned(), "not-hex".to_owned()];
        let plan = plan_prune(PruneInputs {
            records: &records,
            generations: &[],
            current_hex: None,
            active_setup_hexes: &[],
            active_generation_hexes: &[],
            unmanaged_names: &unmanaged,
            observation_unknown: false,
            previous_hex: None,
        });
        assert_eq!(plan.refused_unmanaged, unmanaged);
        assert!(
            !plan
                .prune_setup_records
                .iter()
                .any(|hex| unmanaged.contains(hex))
        );
    }

    #[test]
    fn unknown_observation_preserves_instead_of_pruning_by_default() {
        let current = record('1', '2');
        let stale = record('3', '4');
        let records = vec![current.clone(), stale.clone()];
        let generations = vec![
            generation(GenerationUse::Environment, '3'),
            generation(GenerationUse::Generated, '4'),
        ];
        let plan = plan_prune(PruneInputs {
            records: &records,
            generations: &generations,
            current_hex: Some(&current.hex),
            active_setup_hexes: &[],
            active_generation_hexes: &[],
            unmanaged_names: &[],
            observation_unknown: true,
            previous_hex: None,
        });
        assert!(plan.observation_unknown);
        assert!(plan.prune_setup_records.is_empty());
        assert!(plan.prune_generations.is_empty());
        assert_eq!(plan.preserved_unobserved_setup_records, vec![stale.hex]);
        assert_eq!(plan.preserved_unobserved_generations, generations);
        assert_eq!(plan.preserved_current, Some(current.hex));
        assert!(!plan.has_prune());
        assert!(plan.has_preserved_unobserved());
    }

    #[test]
    fn unknown_observation_keeps_current_and_previous_only() {
        let current = record('1', '2');
        let previous = record('3', '2');
        let older = record('5', '6');
        let records = vec![current.clone(), previous.clone(), older.clone()];
        let generations = vec![
            generation(GenerationUse::Environment, '1'),
            generation(GenerationUse::Generated, '2'),
            generation(GenerationUse::Environment, '3'),
            generation(GenerationUse::Environment, '5'),
            generation(GenerationUse::Generated, '6'),
        ];
        let plan = plan_prune(PruneInputs {
            observation_unknown: true,
            previous_hex: Some(&previous.hex),
            ..inputs(&records, &generations, Some(&current.hex))
        });
        assert_eq!(plan.prune_setup_records, vec![older.hex]);
        assert_eq!(
            plan.prune_generations,
            vec![
                generation(GenerationUse::Environment, '5'),
                generation(GenerationUse::Generated, '6'),
            ]
        );
        assert_eq!(plan.preserved_unobserved_setup_records, vec![previous.hex]);
        assert_eq!(
            plan.preserved_unobserved_generations,
            vec![generation(GenerationUse::Environment, '3')]
        );
    }

    #[test]
    fn known_observation_ignores_previous() {
        let current = record('1', '2');
        let previous = record('3', '4');
        let records = vec![current.clone(), previous.clone()];
        let plan = plan_prune(PruneInputs {
            previous_hex: Some(&previous.hex),
            ..inputs(&records, &[], Some(&current.hex))
        });
        assert_eq!(plan.prune_setup_records, vec![previous.hex]);
        assert!(!plan.has_preserved_unobserved());
    }
}
