use std::collections::{BTreeMap, BTreeSet};

use crate::config::{DxConfig, FamilyConfig, Profile};
use crate::curated_defaults_gen::{CROSS_FAMILY_TOOLS, FAMILIES, OPT_IN_FAMILIES, OVERLAPS, TOOLS};

/// One row of the generated curated table.
#[derive(Clone, Copy)]
pub struct CuratedFamily {
    pub id: &'static str,
    pub lint: &'static [&'static str],
    pub format: &'static [&'static str],
    pub typecheck: &'static [&'static str],
    pub audit: &'static [&'static str],
}

/// Two tools that own the same concern for one file class of a family.
pub struct Overlap {
    pub family: &'static str,
    pub class: &'static str,
    pub concern: &'static str,
    pub tools: [(&'static str, &'static str); 2],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FamilySelection {
    pub profile: Profile,
    pub lint: Vec<String>,
    pub format: Vec<String>,
    pub typecheck: Vec<String>,
    pub audit: Vec<String>,
}

impl FamilySelection {
    fn capability(&self, name: &str) -> &[String] {
        match name {
            "lint" => &self.lint,
            "format" => &self.format,
            "typecheck" => &self.typecheck,
            _ => &self.audit,
        }
    }

    fn tools(&self) -> impl Iterator<Item = &String> {
        self.lint
            .iter()
            .chain(&self.format)
            .chain(&self.typecheck)
            .chain(&self.audit)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Selection {
    pub profile: Profile,
    pub families: BTreeMap<String, FamilySelection>,
    /// Families dx.toml turns off.
    pub disabled: BTreeSet<String>,
    pub exclude: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum SelectionProblem {
    #[error("unknown family `{family}` in dx.toml{}", suggestion(.hint))]
    UnknownFamily {
        family: String,
        hint: Option<String>,
    },
    #[error("unknown tool `{tool}` for family `{family}` in dx.toml{}", suggestion(.hint))]
    UnknownTool {
        family: String,
        tool: String,
        hint: Option<String>,
    },
    #[error(
        "family `{family}` gives class `{class}` two owners for `{concern}`: `{first}` and `{second}`; \
         select one of them"
    )]
    OverlappingTools {
        family: String,
        class: String,
        concern: String,
        first: String,
        second: String,
    },
    #[error(
        "tool `{tool}` is selected by family `{first}` ({first_profile}) and family `{second}` \
         ({second_profile}); give both families the same profile or select `{tool}` in only one"
    )]
    ProfileConflict {
        tool: String,
        first: String,
        first_profile: &'static str,
        second: String,
        second_profile: &'static str,
    },
}

fn suggestion(closest: &Option<String>) -> String {
    match closest {
        Some(name) => format!("; did you mean `{name}`?"),
        None => String::new(),
    }
}

/// The candidate closest to `name`, when it is near enough to be a typo.
fn closest(name: &str, candidates: impl Iterator<Item = &'static str>) -> Option<String> {
    let limit = (name.len() / 3).max(2);
    candidates
        .map(|candidate| (distance(name, candidate), candidate))
        .filter(|(distance, _)| *distance <= limit)
        .min()
        .map(|(_, candidate)| candidate.to_owned())
}

fn distance(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut previous = row[0];
        row[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let held = row[j + 1];
            row[j + 1] = (previous + usize::from(ca != *cb))
                .min(row[j] + 1)
                .min(held + 1);
            previous = held;
        }
    }
    row[b.len()]
}

fn owned(tools: &[&str]) -> Vec<String> {
    tools.iter().map(|tool| (*tool).to_owned()).collect()
}

fn apply(
    curated: &CuratedFamily,
    root: Profile,
    over: Option<&FamilyConfig>,
) -> Result<FamilySelection, SelectionProblem> {
    let pick =
        |own: &Option<Vec<String>>, base: &[&str]| -> Result<Vec<String>, SelectionProblem> {
            let Some(tools) = own else {
                return Ok(owned(base));
            };
            for tool in tools {
                if !TOOLS.contains(&tool.as_str()) {
                    return Err(SelectionProblem::UnknownTool {
                        family: curated.id.to_owned(),
                        tool: tool.clone(),
                        hint: closest(tool, TOOLS.iter().copied()),
                    });
                }
            }
            Ok(tools.clone())
        };
    let none = FamilyConfig::default();
    let over = over.unwrap_or(&none);
    Ok(FamilySelection {
        profile: over.profile.unwrap_or(root),
        lint: pick(&over.lint, curated.lint)?,
        format: pick(&over.format, curated.format)?,
        typecheck: pick(&over.typecheck, curated.typecheck)?,
        audit: pick(&over.audit, curated.audit)?,
    })
}

/// The curated selection with the `dx.toml` overrides applied. No file means the
/// curated tools at the standard tier.
pub fn effective(config: Option<&DxConfig>) -> Result<Selection, SelectionProblem> {
    let root = config.and_then(|config| config.profile).unwrap_or_default();
    if let Some(config) = config {
        for id in config.families.keys() {
            let known = FAMILIES.iter().map(|family| family.id);
            if !known
                .clone()
                .chain(OPT_IN_FAMILIES.iter().copied())
                .any(|known| known == id)
            {
                return Err(SelectionProblem::UnknownFamily {
                    family: id.clone(),
                    hint: closest(id, known.chain(OPT_IN_FAMILIES.iter().copied())),
                });
            }
        }
    }
    let mut families = BTreeMap::new();
    let mut disabled = BTreeSet::new();
    let opt_in = OPT_IN_FAMILIES
        .iter()
        .filter(|id| config.is_some_and(|config| config.families.contains_key(**id)))
        .map(|id| CuratedFamily {
            id,
            lint: &[],
            format: &[],
            typecheck: &[],
            audit: &[],
        });
    for curated in FAMILIES.iter().copied().chain(opt_in) {
        let over = config.and_then(|config| config.families.get(curated.id));
        let selection = apply(&curated, root, over)?;
        if over.is_some_and(|over| over.disabled) {
            disabled.insert(curated.id.to_owned());
        } else {
            families.insert(curated.id.to_owned(), selection);
        }
    }
    check_profiles(&families)?;
    check_overlaps(&families)?;
    Ok(Selection {
        profile: root,
        families,
        disabled,
        exclude: config
            .map(|config| config.scope.exclude.clone())
            .unwrap_or_default(),
    })
}

fn check_profiles(families: &BTreeMap<String, FamilySelection>) -> Result<(), SelectionProblem> {
    let mut seen: BTreeMap<&str, (&str, Profile)> = BTreeMap::new();
    for (id, family) in families {
        for tool in family.tools() {
            if CROSS_FAMILY_TOOLS.contains(&tool.as_str()) {
                continue;
            }
            match seen.get(tool.as_str()) {
                Some((first, profile)) if *profile != family.profile => {
                    return Err(SelectionProblem::ProfileConflict {
                        tool: tool.clone(),
                        first: (*first).to_owned(),
                        first_profile: profile.as_str(),
                        second: id.clone(),
                        second_profile: family.profile.as_str(),
                    });
                }
                Some(_) => {}
                None => {
                    seen.insert(tool, (id, family.profile));
                }
            }
        }
    }
    Ok(())
}

fn check_overlaps(families: &BTreeMap<String, FamilySelection>) -> Result<(), SelectionProblem> {
    for overlap in OVERLAPS {
        let Some(family) = families.get(overlap.family) else {
            continue;
        };
        let [(first, first_capability), (second, second_capability)] = overlap.tools;
        let selected =
            |tool: &str, capability: &str| family.capability(capability).iter().any(|t| t == tool);
        if selected(first, first_capability) && selected(second, second_capability) {
            return Err(SelectionProblem::OverlappingTools {
                family: overlap.family.to_owned(),
                class: overlap.class.to_owned(),
                concern: overlap.concern.to_owned(),
                first: first.to_owned(),
                second: second.to_owned(),
            });
        }
    }
    Ok(())
}
