//! Generates `docs/support-matrix.md` from CI workflows, per-platform row test queries and the quality registry.

#![deny(clippy::indexing_slicing, clippy::string_slice)]

pub mod render;
pub mod runfiles_source;
pub mod workflows;

use std::collections::{BTreeMap, BTreeSet};

use serde::Deserialize;

/// The page this crate generates, relative to the workspace root.
pub const MATRIX_PATH: &str = "docs/support-matrix.md";

/// The skip reasons dx reads at run time, relative to the workspace root.
pub const UNSUPPORTED_PATH: &str = "cli/cli/src/exec/unsupported.json";

/// The command that refreshes the page.
pub const UPDATE_COMMAND: &str = "bazel run //tools/docs/support_matrix:update";

const CI_WORKFLOW: &str = ".github/workflows/ci.yml";

/// The command that refreshes one platform's row test file.
/// Row to the labels tagged with it.
pub const ROW_LABELS_PATH: &str = "tools/docs/support_matrix/row_labels.json";
/// Row to the labels tagged `consumer-row:<row>`.
pub const CONSUMER_LABELS_PATH: &str = "tools/docs/support_matrix/consumer_labels.json";
pub const ROW_TESTS_COMMAND: &str = "tools/docs/support_matrix/row_tests.sh <platform>";

/// One CI platform from the language list.
#[derive(Clone, Debug, Deserialize)]
pub struct PlatformFact {
    pub id: String,
    pub os: String,
    pub cpu: String,
    /// Constraint values the CI host platform adds beyond os and cpu.
    #[serde(default)]
    pub constraints: Vec<String>,
}

/// One row of the language list.
#[derive(Clone, Debug, Deserialize)]
pub struct Row {
    pub name: String,
    pub title: String,
    pub section: String,
    pub families: Vec<String>,
    /// The API doc tool; empty when the language has none, absent when docs do not apply.
    #[serde(default)]
    pub doc: Option<String>,
    /// The unused-code tools and rules; empty when the language has none yet, absent when it does not apply.
    #[serde(default)]
    pub dead_code: Option<Vec<String>>,
    #[serde(default)]
    pub identity: Identity,
    pub ci_tests: Vec<String>,
    pub consumer_tests: Vec<String>,
    pub workflows: Vec<String>,
}

/// The compiler identity of a compiler row; empty for other rows.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct Identity {
    #[serde(default)]
    pub driver: String,
    #[serde(default)]
    pub coverage: String,
    /// The identity test per platform where it differs from the row's probe.
    #[serde(default)]
    pub platform_probes: BTreeMap<String, String>,
}

/// One row of the conformance manifest: a suite test of a language on a platform.
#[derive(Clone, Debug, Deserialize)]
pub struct ConformanceEntry {
    pub lang: String,
    pub capability: String,
    pub platform: String,
    /// `run`, `n/a` or `pending`.
    pub status: String,
    pub reason: String,
    #[serde(default)]
    pub label: String,
}

/// The conformance manifest: every suite test per language and platform, and the `lang.json` platforms.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct Conformance {
    pub entries: Vec<ConformanceEntry>,
    /// Language to its `lang.json` platforms; empty means every platform.
    pub platforms: BTreeMap<String, Vec<String>>,
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
    pub platforms: Vec<PlatformFact>,
    pub sections: Vec<String>,
    pub rows: Vec<Row>,
    #[serde(default)]
    pub conformance: Conformance,
    pub quality: BTreeMap<String, BTreeMap<String, Vec<String>>>,
    pub support: Vec<Support>,
    pub deferred_families: Vec<String>,
    /// Capability, then family, then the reason the capability never applies.
    #[serde(default)]
    pub not_applicable: BTreeMap<String, BTreeMap<String, String>>,
    /// Row name, then platform, then the reason the row cannot support that platform.
    #[serde(default)]
    pub platform_not_applicable: BTreeMap<String, BTreeMap<String, String>>,
    /// Compiler row name, then platform, then what the row's evidence on that platform still waits on.
    #[serde(default)]
    pub compiler_blockers: BTreeMap<String, BTreeMap<String, String>>,
    /// Family, capability, tool, then platform, then the reason the tool cannot run there.
    #[serde(default)]
    pub tool_not_applicable: ToolNotApplicable,
    /// Row name to the label of its unsupported constraint_value.
    #[serde(default)]
    pub unsupported_constraints: BTreeMap<String, String>,
    /// Language row name to the cell of its dependency sets, or none with the reason.
    #[serde(default)]
    pub dependencies: BTreeMap<String, String>,
    /// Language row name to its `*_bench` macro.
    #[serde(default)]
    pub bench: BTreeMap<String, String>,
    /// Language row name to its `*_fuzz_test` macro.
    #[serde(default)]
    pub fuzz: BTreeMap<String, String>,
    /// Publishing rule to the tool its api_compat check runs, or "not available".
    #[serde(default)]
    pub api_compat_tools: BTreeMap<String, String>,
    pub not_languages: Vec<String>,
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

/// Family, then capability, then tool, then platform, to the reason.
pub type ToolNotApplicable =
    BTreeMap<String, BTreeMap<String, BTreeMap<String, BTreeMap<String, String>>>>;

/// Parses the facts JSON.
pub fn parse_facts(text: &str) -> Result<Facts, String> {
    serde_json::from_str(text).map_err(|error| format!("facts JSON is invalid: {error}"))
}

/// Row test label to its tags, for the row tests compatible with one platform.
pub type RowTests = BTreeMap<String, Vec<String>>;

/// Reads `tools/docs/support_matrix/row_tests/<platform>.json` for every platform.
pub fn read_row_tests(
    facts: &Facts,
    source: &dyn Source,
) -> Result<BTreeMap<String, RowTests>, String> {
    let mut out = BTreeMap::new();
    for platform in &facts.platforms {
        let path = format!("tools/docs/support_matrix/row_tests/{}.json", platform.id);
        let tests: RowTests = serde_json::from_str(&source.read(&path)?)
            .map_err(|error| format!("{path} is invalid: {error}"))?;
        out.insert(platform.id.clone(), tests);
    }
    Ok(out)
}

/// Reads `tools/docs/support_matrix/row_labels.json`: row name to the sorted labels tagged `row:<name>`.
pub fn read_row_labels(source: &dyn Source) -> Result<BTreeMap<String, Vec<String>>, String> {
    read_labels(source, ROW_LABELS_PATH)
}

/// Reads `tools/docs/support_matrix/consumer_labels.json`: row name to the sorted labels tagged `consumer-row:<name>`.
pub fn read_consumer_labels(source: &dyn Source) -> Result<BTreeMap<String, Vec<String>>, String> {
    read_labels(source, CONSUMER_LABELS_PATH)
}

fn read_labels(source: &dyn Source, path: &str) -> Result<BTreeMap<String, Vec<String>>, String> {
    let text = source.read(path)?;
    serde_json::from_str(&text).map_err(|error| format!("{path} is invalid: {error}"))
}

struct Context {
    platforms: Vec<String>,
    row_tests: BTreeMap<String, RowTests>,
    dogfood: Vec<String>,
    tag_jobs: BTreeMap<String, Vec<String>>,
    extra: BTreeMap<String, (Vec<String>, String)>,
}

impl Context {
    fn tested_on(
        &self,
        row: &Row,
        labels: &[String],
        consumer: bool,
    ) -> Result<Vec<String>, String> {
        let mut on = BTreeSet::new();
        for label in labels {
            let mut tags = None;
            for platform in &self.platforms {
                if let Some(found) = self
                    .row_tests
                    .get(platform)
                    .and_then(|tests| tests.get(label))
                {
                    tags = Some(found);
                }
            }
            let Some(tags) = tags else {
                return Err(format!(
                    "{label} of row {} is in no row_tests/<platform>.json; run `{ROW_TESTS_COMMAND}` on each platform",
                    row.name
                ));
            };
            if label.ends_with("_upstream") && tags.iter().any(|t| self.tag_jobs.contains_key(t)) {
                return Err(format!(
                    "{label} is filtered out of tag-selected CI jobs, so it cannot be a consumer test"
                ));
            }
            if consumer
                && row.section == "compilers"
                && !tags.iter().any(|tag| tag == "hermetic-consumer")
            {
                return Err(format!(
                    "{label} is a consumer test of compiler row {} but lacks the hermetic-consumer tag, so it is not a nested qualification",
                    row.name
                ));
            }
            let manual = tags.iter().any(|tag| tag == "manual");
            for platform in &self.platforms {
                if !self
                    .row_tests
                    .get(platform)
                    .is_some_and(|tests| tests.contains_key(label))
                {
                    continue;
                }
                if !manual && self.dogfood.contains(platform) {
                    on.insert(platform.clone());
                }
                for tag in tags {
                    if self
                        .tag_jobs
                        .get(tag)
                        .is_some_and(|platforms| platforms.contains(platform))
                    {
                        on.insert(platform.clone());
                    }
                }
                for workflow in &row.workflows {
                    let Some((platforms, text)) = self.extra.get(workflow) else {
                        return Err(format!(
                            "row {} names unknown workflow {workflow}",
                            row.name
                        ));
                    };
                    if platforms.contains(platform) && text.contains(label.as_str()) {
                        on.insert(platform.clone());
                    }
                }
            }
        }
        Ok(self
            .platforms
            .iter()
            .filter(|platform| on.contains(*platform))
            .cloned()
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
    let lanes = workflows::preflight_quality(CI_WORKFLOW, &ci)?;
    workflows::check_evidence_lanes(CI_WORKFLOW, &lanes, &ids)?;
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
        platforms: ids.iter().map(|id| (*id).to_owned()).collect(),
        row_tests: read_row_tests(facts, source)?,
        dogfood,
        tag_jobs,
        extra,
    };
    let row_labels = read_row_labels(source)?;
    for name in row_labels.keys() {
        if !facts.rows.iter().any(|row| &row.name == name) {
            return Err(format!(
                "{ROW_LABELS_PATH} lists row:{name}, which names no row in languages.bzl; run `{ROW_TESTS_COMMAND}`"
            ));
        }
    }
    let consumer_labels = read_consumer_labels(source)?;
    for name in consumer_labels.keys() {
        if !facts.rows.iter().any(|row| &row.name == name) {
            return Err(format!(
                "{CONSUMER_LABELS_PATH} lists consumer-row:{name}, which names no row in languages.bzl; run `{ROW_TESTS_COMMAND}`"
            ));
        }
    }
    let mut out = BTreeMap::new();
    for row in &facts.rows {
        let labels = row_labels.get(&row.name).map_or(&[][..], Vec::as_slice);
        let consumers = consumer_labels
            .get(&row.name)
            .map_or(&[][..], Vec::as_slice);
        out.insert(
            row.name.clone(),
            RowPlatforms {
                build: context.tested_on(row, labels, false)?,
                consumer: context.tested_on(row, consumers, true)?,
            },
        );
    }
    Ok(out)
}

/// Checks that every `lang.json` language has one manifest entry per suite column and `lang.json` platform, and that each run entry names its test.
pub fn check_conformance(facts: &Facts) -> Result<(), String> {
    let every = vec!["*".to_owned()];
    for row in facts.rows.iter().filter(|row| row.section == "languages") {
        let platforms = match facts.conformance.platforms.get(&row.name) {
            None => continue,
            Some(ids) if ids.is_empty() => &every,
            Some(ids) => ids,
        };
        for test in render::SUITE_TESTS {
            for platform in platforms {
                let entries: Vec<_> = facts
                    .conformance
                    .entries
                    .iter()
                    .filter(|e| {
                        e.lang == row.name && e.capability == test && &e.platform == platform
                    })
                    .collect();
                match entries.as_slice() {
                    [entry] if entry.status != "run" || !entry.label.is_empty() => {}
                    [_] => {
                        return Err(format!(
                            "{} {test} on {platform} runs but its suite declares no test; add a pending.bzl line or a not_applicable entry",
                            row.name
                        ));
                    }
                    _ => {
                        return Err(format!(
                            "{} {test} on {platform} has {} conformance manifest entries, want 1; add a pending.bzl line or a not_applicable entry",
                            row.name,
                            entries.len()
                        ));
                    }
                }
            }
        }
    }
    Ok(())
}

/// Generates the page from the facts and the workspace sources.
pub fn generate(facts_json: &str, source: &dyn Source) -> Result<String, String> {
    let facts = parse_facts(facts_json)?;
    check_conformance(&facts)?;
    let platforms = row_platforms(&facts, source)?;
    Ok(render::render(&facts, &platforms))
}

/// Renders the per-language skip reasons as JSON keyed by the unsupported constraint label.
pub fn unsupported_json(facts: &Facts) -> String {
    let mut out = serde_json::Map::new();
    for row in &facts.rows {
        let (Some(label), Some(reasons)) = (
            facts.unsupported_constraints.get(&row.name),
            facts.platform_not_applicable.get(&row.name),
        ) else {
            continue;
        };
        let platforms: serde_json::Map<String, serde_json::Value> = facts
            .platforms
            .iter()
            .filter_map(|platform| {
                let reason = reasons.get(&platform.id)?;
                Some((platform.id.clone(), reason.clone().into()))
            })
            .collect();
        out.insert(
            label.clone(),
            serde_json::json!({"title": row.title, "platforms": platforms}),
        );
    }
    let mut text = serde_json::to_string_pretty(&out).unwrap_or_default();
    text.push('\n');
    text
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
    fn unsupported_json_lists_declared_reasons() {
        let facts = parse_facts(
            r#"{
              "platforms": [{"id": "a", "os": "x", "cpu": "y"}, {"id": "b", "os": "x", "cpu": "z"}],
              "sections": [],
              "rows": [
                {"name": "hs", "title": "Haskell", "section": "languages", "families": [],
                 "ci_tests": [], "consumer_tests": [], "workflows": []},
                {"name": "go", "title": "Go", "section": "languages", "families": [],
                 "ci_tests": [], "consumer_tests": [], "workflows": []}
              ],
              "quality": {}, "support": [], "deferred_families": [],
              "platform_not_applicable": {"hs": {"b": "no bindist"}, "go": {"a": "x"}},
              "unsupported_constraints": {"hs": "//haskell/private/toolchain:unsupported"},
              "not_languages": []
            }"#,
        )
        .expect("facts");
        assert_eq!(
            unsupported_json(&facts),
            "{\n  \"//haskell/private/toolchain:unsupported\": {\n    \"platforms\": {\n      \"b\": \"no bindist\"\n    },\n    \"title\": \"Haskell\"\n  }\n}\n"
        );
    }

    #[test]
    fn diff_names_changed_lines() {
        assert_eq!(diff("a\nb\n", "a\nb\n"), None);
        assert_eq!(diff("a\nb\n", "a\nc\n").expect("diff"), "-2: b\n+2: c\n");
    }
}
