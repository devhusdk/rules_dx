//! Generates `docs/support-matrix.md` from CI workflows, test BUILD files and the quality registry.

pub mod gaps;
pub mod render;
pub mod runfiles_source;
pub mod starlark;
pub mod workflows;

use std::collections::{BTreeMap, BTreeSet};

use serde::Deserialize;

/// The page this crate generates, relative to the workspace root.
pub const MATRIX_PATH: &str = "docs/support-matrix.md";

/// The command that refreshes the page.
pub const UPDATE_COMMAND: &str = "bazel run //tools/docs/support_matrix:update";

const CI_WORKFLOW: &str = ".github/workflows/ci.yml";

/// One CI platform from the language list.
#[derive(Clone, Debug, Deserialize)]
pub struct PlatformFact {
    pub id: String,
    pub os: String,
    pub cpu: String,
}

/// One row of the language list.
#[derive(Clone, Debug, Deserialize)]
pub struct Row {
    pub name: String,
    pub title: String,
    pub section: String,
    pub families: Vec<String>,
    pub ci_tests: Vec<String>,
    pub consumer_tests: Vec<String>,
    pub workflows: Vec<String>,
}

/// One explicit tool support record.
#[derive(Clone, Debug, Deserialize)]
pub struct Support {
    pub family: String,
    pub capability: String,
    pub tool: String,
    pub platforms: Vec<String>,
}

/// The facts the Starlark rule writes.
#[derive(Clone, Debug, Deserialize)]
pub struct Facts {
    pub schema_version: u32,
    pub platforms: Vec<PlatformFact>,
    pub sections: Vec<String>,
    pub rows: Vec<Row>,
    pub quality: BTreeMap<String, BTreeMap<String, Vec<String>>>,
    pub support: Vec<Support>,
    pub deferred_families: Vec<String>,
    /// Capability, then family, then the reason the capability never applies.
    #[serde(default)]
    pub not_applicable: BTreeMap<String, BTreeMap<String, String>>,
    pub not_languages: Vec<String>,
    pub gap_issues: BTreeMap<String, u64>,
    pub gap_enforcement: bool,
    pub computed_constraints: BTreeMap<String, Vec<Vec<String>>>,
}

/// Reads workspace files by their repository-relative path.
pub trait Source {
    fn read(&self, path: &str) -> Result<String, String>;
}

/// The platforms each row is tested on.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RowPlatforms {
    pub build: Vec<String>,
    pub consumer: Vec<String>,
}

/// The rendered page and the gaps it shows.
#[derive(Clone, Debug)]
pub struct Matrix {
    pub markdown: String,
    pub gaps: Vec<String>,
}

/// Parses the facts JSON.
pub fn parse_facts(text: &str) -> Result<Facts, String> {
    let facts: Facts =
        serde_json::from_str(text).map_err(|error| format!("facts JSON is invalid: {error}"))?;
    if facts.schema_version != 1 {
        return Err(format!(
            "facts schema v{} is unsupported (want v1)",
            facts.schema_version
        ));
    }
    Ok(facts)
}

struct BuildLoader<'a> {
    source: &'a dyn Source,
    package: String,
    computed: &'a BTreeMap<String, Vec<Vec<String>>>,
}

fn bzl_path(package: &str, label: &str) -> Result<String, String> {
    let (pkg, file) = if let Some(rest) = label.strip_prefix("//") {
        rest.split_once(':')
            .ok_or_else(|| format!("load label {label} has no `:`"))?
    } else if let Some(file) = label.strip_prefix(':') {
        (package, file)
    } else {
        return Err(format!("load label {label} is outside the main repository"));
    };
    Ok(if pkg.is_empty() {
        file.to_owned()
    } else {
        format!("{pkg}/{file}")
    })
}

impl starlark::Loader for BuildLoader<'_> {
    fn load(&self, label: &str) -> Result<starlark::File, String> {
        let path = bzl_path(&self.package, label)?;
        starlark::parse(&path, &self.source.read(&path)?)
    }

    fn computed(&self, label: &str, symbol: &str) -> Option<Vec<Vec<String>>> {
        self.computed.get(&format!("{label}%{symbol}")).cloned()
    }
}

fn split_label(label: &str) -> Result<(&str, &str), String> {
    label
        .strip_prefix("//")
        .and_then(|rest| rest.split_once(':'))
        .ok_or_else(|| format!("test label {label} is not //package:name"))
}

struct Context<'a> {
    source: &'a dyn Source,
    computed: &'a BTreeMap<String, Vec<Vec<String>>>,
    platforms: Vec<starlark::Platform>,
    dogfood: Vec<String>,
    tag_jobs: BTreeMap<String, Vec<String>>,
    extra: BTreeMap<String, (Vec<String>, String)>,
}

impl Context<'_> {
    fn tested_on(&self, row: &Row, labels: &[String]) -> Result<Vec<String>, String> {
        let mut on = BTreeSet::new();
        for label in labels {
            let (package, name) = split_label(label)?;
            let path = format!("{package}/BUILD.bazel");
            let file = starlark::parse(&path, &self.source.read(&path)?)?;
            let loader = BuildLoader {
                source: self.source,
                package: package.to_owned(),
                computed: self.computed,
            };
            let facts = starlark::test_facts(&file, name, &loader)?;
            if label.ends_with("_upstream")
                && facts.tags.iter().any(|t| self.tag_jobs.contains_key(t))
            {
                return Err(format!(
                    "{label} is filtered out of tag-selected CI jobs, so it cannot be a consumer test"
                ));
            }
            let manual = facts.tags.iter().any(|tag| tag == "manual");
            for platform in &self.platforms {
                if !starlark::compatible(&facts.compatible_with, platform)
                    .map_err(|error| format!("{path} :{name}: {error}"))?
                {
                    continue;
                }
                if !manual && self.dogfood.contains(&platform.id) {
                    on.insert(platform.id.clone());
                }
                for tag in &facts.tags {
                    if self
                        .tag_jobs
                        .get(tag)
                        .is_some_and(|platforms| platforms.contains(&platform.id))
                    {
                        on.insert(platform.id.clone());
                    }
                }
                for workflow in &row.workflows {
                    let Some((platforms, text)) = self.extra.get(workflow) else {
                        return Err(format!(
                            "row {} names unknown workflow {workflow}",
                            row.name
                        ));
                    };
                    if platforms.contains(&platform.id) && text.contains(label.as_str()) {
                        on.insert(platform.id.clone());
                    }
                }
            }
        }
        Ok(self
            .platforms
            .iter()
            .filter(|platform| on.contains(&platform.id))
            .map(|platform| platform.id.clone())
            .collect())
    }
}

/// Reads every source and returns the platforms of each row.
pub fn row_platforms(
    facts: &Facts,
    source: &dyn Source,
) -> Result<BTreeMap<String, RowPlatforms>, String> {
    let ci = source.read(CI_WORKFLOW)?;
    let dogfood = workflows::dogfood_platforms(CI_WORKFLOW, &ci)?;
    let runners = workflows::runner_platforms(CI_WORKFLOW, &ci)?;
    let ids: Vec<&str> = facts.platforms.iter().map(|p| p.id.as_str()).collect();
    for platform in dogfood.iter().chain(runners.values()) {
        if !ids.contains(&platform.as_str()) {
            return Err(format!(
                "{CI_WORKFLOW} names platform {platform}, which languages.bzl does not list"
            ));
        }
    }
    let mut tag_jobs = workflows::tag_job_platforms(CI_WORKFLOW, &ci, &runners)?;
    let mut extra = BTreeMap::new();
    for workflow in facts.rows.iter().flat_map(|row| row.workflows.iter()) {
        if extra.contains_key(workflow) {
            continue;
        }
        let path = format!(".github/workflows/{workflow}");
        let text = source.read(&path)?;
        let platforms = workflows::workflow_platforms(&path, &text, &runners)?;
        for (tag, on) in workflows::tag_job_platforms(&path, &text, &runners)? {
            let entry: &mut Vec<String> = tag_jobs.entry(tag).or_default();
            for platform in on {
                if !entry.contains(&platform) {
                    entry.push(platform);
                }
            }
        }
        extra.insert(workflow.clone(), (platforms, text));
    }
    let context = Context {
        source,
        computed: &facts.computed_constraints,
        platforms: facts
            .platforms
            .iter()
            .map(|platform| starlark::Platform {
                id: platform.id.clone(),
                os: platform.os.clone(),
                cpu: platform.cpu.clone(),
            })
            .collect(),
        dogfood,
        tag_jobs,
        extra,
    };
    let mut out = BTreeMap::new();
    for row in &facts.rows {
        out.insert(
            row.name.clone(),
            RowPlatforms {
                build: context.tested_on(row, &row.ci_tests)?,
                consumer: context.tested_on(row, &row.consumer_tests)?,
            },
        );
    }
    Ok(out)
}

/// Generates the page from the facts and the workspace sources.
pub fn generate(facts_json: &str, source: &dyn Source) -> Result<Matrix, String> {
    let facts = parse_facts(facts_json)?;
    let platforms = row_platforms(&facts, source)?;
    let gaps = gaps::gaps(&facts, &platforms);
    if facts.gap_enforcement {
        gaps::check_ledger(&gaps, &facts.gap_issues)?;
    }
    Ok(Matrix {
        markdown: render::render(&facts, &platforms),
        gaps,
    })
}

/// Returns a short line diff of `want` against `have`, or None when equal.
pub fn diff(have: &str, want: &str) -> Option<String> {
    if have == want {
        return None;
    }
    let have_lines: Vec<&str> = have.lines().collect();
    let want_lines: Vec<&str> = want.lines().collect();
    let mut out = String::new();
    for index in 0..have_lines.len().max(want_lines.len()) {
        let (old, new) = (have_lines.get(index), want_lines.get(index));
        if old == new {
            continue;
        }
        if let Some(old) = old {
            out.push_str(&format!("-{}: {old}\n", index + 1));
        }
        if let Some(new) = new {
            out.push_str(&format!("+{}: {new}\n", index + 1));
        }
    }
    if out.is_empty() {
        out.push_str("line endings differ\n");
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_load_paths() {
        assert_eq!(
            bzl_path("a/b", "//tools/testing:nested_bazel.bzl").expect("path"),
            "tools/testing/nested_bazel.bzl"
        );
        assert_eq!(bzl_path("a/b", ":x.bzl").expect("path"), "a/b/x.bzl");
        assert!(bzl_path("a", "@platforms//os:BUILD").is_err());
    }

    #[test]
    fn diff_names_changed_lines() {
        assert_eq!(diff("a\nb\n", "a\nb\n"), None);
        assert_eq!(diff("a\nb\n", "a\nc\n").expect("diff"), "-2: b\n+2: c\n");
    }
}
