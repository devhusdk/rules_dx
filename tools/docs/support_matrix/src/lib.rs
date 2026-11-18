//! Generates `docs/support-matrix.md` from CI workflows, per-platform row test queries and the quality registry.

#![deny(clippy::indexing_slicing, clippy::string_slice)]

pub mod render;
pub mod runfiles_source;
pub mod workflows;

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::{self, Write as _};

use serde::Deserialize;

pub use workflows::WorkflowError;

/// Why the support matrix cannot be generated.
#[derive(Debug)]
pub enum SupportMatrixError {
    Runfiles(runfiles::RunfilesError),
    NotInRunfiles {
        path: String,
    },
    Read {
        path: String,
        source: dx_atomic_fs::path_io::PathIoError,
    },
    Facts(serde_json::Error),
    Json {
        path: String,
        source: serde_json::Error,
    },
    Workflow(WorkflowError),
    UnknownPlatform {
        platform: String,
    },
    UnknownRow {
        labels_path: &'static str,
        tag: &'static str,
        row: String,
    },
    UnknownWorkflow {
        row: String,
        workflow: String,
    },
    UnqueriedLabel {
        row: String,
        label: String,
    },
    FilteredConsumer {
        label: String,
    },
    UndeclaredSuiteTest {
        row: String,
        test: String,
        platform: String,
    },
    ManifestEntries {
        row: String,
        test: String,
        platform: String,
        count: usize,
    },
}

impl fmt::Display for SupportMatrixError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Runfiles(source) => write!(f, "cannot open runfiles: {source}"),
            Self::NotInRunfiles { path } => write!(f, "{path} is not in the runfiles"),
            Self::Read { path, source } => write!(
                f,
                "{source}; add {path} to the support_matrix data and export it"
            ),
            Self::Facts(source) => write!(f, "facts JSON is invalid: {source}"),
            Self::Json { path, source } => write!(f, "{path} is invalid: {source}"),
            Self::Workflow(source) => source.fmt(f),
            Self::UnknownPlatform { platform } => write!(
                f,
                "{CI_WORKFLOW} names platform {platform}, which languages.bzl does not list"
            ),
            Self::UnknownRow {
                labels_path,
                tag,
                row,
            } => write!(
                f,
                "{labels_path} lists {tag}{row}, which names no row in languages.bzl; run `{ROW_TESTS_COMMAND}`"
            ),
            Self::UnknownWorkflow { row, workflow } => {
                write!(f, "row {row} names unknown workflow {workflow}")
            }
            Self::UnqueriedLabel { row, label } => write!(
                f,
                "{label} of row {row} is in no row_tests/<platform>.json; run `{ROW_TESTS_COMMAND}` on each platform"
            ),
            Self::FilteredConsumer { label } => write!(
                f,
                "{label} is filtered out of tag-selected CI jobs, so it cannot be a consumer test"
            ),
            Self::UndeclaredSuiteTest {
                row,
                test,
                platform,
            } => write!(
                f,
                "{row} {test} on {platform} runs but its suite declares no test; add a pending.bzl line or a not_applicable entry"
            ),
            Self::ManifestEntries {
                row,
                test,
                platform,
                count,
            } => write!(
                f,
                "{row} {test} on {platform} has {count} conformance manifest entries, want 1; add a pending.bzl line or a not_applicable entry"
            ),
        }
    }
}

impl std::error::Error for SupportMatrixError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Runfiles(source) => Some(source),
            Self::Read { source, .. } => Some(source),
            Self::Facts(source) | Self::Json { source, .. } => Some(source),
            Self::Workflow(source) => Some(source),
            Self::NotInRunfiles { .. }
            | Self::UnknownPlatform { .. }
            | Self::UnknownRow { .. }
            | Self::UnknownWorkflow { .. }
            | Self::UnqueriedLabel { .. }
            | Self::FilteredConsumer { .. }
            | Self::UndeclaredSuiteTest { .. }
            | Self::ManifestEntries { .. } => None,
        }
    }
}

impl From<WorkflowError> for SupportMatrixError {
    fn from(source: WorkflowError) -> Self {
        Self::Workflow(source)
    }
}

/// The page this crate generates, relative to the workspace root.
pub const MATRIX_PATH: &str = "docs/support-matrix.md";

/// The skip reasons dx reads at run time, relative to the workspace root.
pub const UNSUPPORTED_PATH: &str = "cli/exec_reports/src/unsupported.json";

/// The command that refreshes the page.
pub const UPDATE_COMMAND: &str = "bazel run //tools/docs/support_matrix:update";

const CI_WORKFLOW: &str = ".github/workflows/ci.yml";

/// The command that refreshes one platform's row test file.
/// Row to the labels tagged with it.
pub const ROW_LABELS_PATH: &str = "tools/docs/support_matrix/row_labels.json";
/// Row to the labels tagged `consumer-row:<row>`.
pub const CONSUMER_LABELS_PATH: &str = "tools/docs/support_matrix/consumer_labels.json";
pub const ROW_TESTS_COMMAND: &str = "bazel run //tools/docs/support_matrix:row_tests -- <platform>";

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
    /// Row name to the label of its unsupported `constraint_value`.
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
    /// Publishing rule to the tool its `api_compat` check runs, or "not available".
    #[serde(default)]
    pub api_compat_tools: BTreeMap<String, String>,
    pub not_languages: Vec<String>,
}

/// Reads workspace files by their repository-relative path.
pub trait Source {
    /// # Errors
    ///
    /// Returns a [`SupportMatrixError`] when the file cannot be found or read.
    fn read(&self, path: &str) -> Result<String, SupportMatrixError>;
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
///
/// # Errors
///
/// Returns a [`SupportMatrixError`] naming the input that is missing, invalid or inconsistent.
pub fn parse_facts(text: &str) -> Result<Facts, SupportMatrixError> {
    serde_json::from_str(text).map_err(SupportMatrixError::Facts)
}

/// Row test label to its tags, for the row tests compatible with one platform.
pub type RowTests = BTreeMap<String, Vec<String>>;

/// Reads `tools/docs/support_matrix/row_tests/<platform>.json` for every platform.
///
/// # Errors
///
/// Returns a [`SupportMatrixError`] naming the input that is missing, invalid or inconsistent.
pub fn read_row_tests(
    facts: &Facts,
    source: &dyn Source,
) -> Result<BTreeMap<String, RowTests>, SupportMatrixError> {
    let mut out = BTreeMap::new();
    for platform in &facts.platforms {
        let path = format!("tools/docs/support_matrix/row_tests/{}.json", platform.id);
        let tests: RowTests = serde_json::from_str(&source.read(&path)?)
            .map_err(|source| SupportMatrixError::Json { path, source })?;
        out.insert(platform.id.clone(), tests);
    }
    Ok(out)
}

/// Reads `tools/docs/support_matrix/row_labels.json`: row name to the sorted labels tagged `row:<name>`.
///
/// # Errors
///
/// Returns a [`SupportMatrixError`] naming the input that is missing, invalid or inconsistent.
pub fn read_row_labels(
    source: &dyn Source,
) -> Result<BTreeMap<String, Vec<String>>, SupportMatrixError> {
    read_labels(source, ROW_LABELS_PATH)
}

/// Reads `tools/docs/support_matrix/consumer_labels.json`: row name to the sorted labels tagged `consumer-row:<name>`.
///
/// # Errors
///
/// Returns a [`SupportMatrixError`] naming the input that is missing, invalid or inconsistent.
pub fn read_consumer_labels(
    source: &dyn Source,
) -> Result<BTreeMap<String, Vec<String>>, SupportMatrixError> {
    read_labels(source, CONSUMER_LABELS_PATH)
}

fn read_labels(
    source: &dyn Source,
    path: &str,
) -> Result<BTreeMap<String, Vec<String>>, SupportMatrixError> {
    let text = source.read(path)?;
    serde_json::from_str(&text).map_err(|source| SupportMatrixError::Json {
        path: path.to_owned(),
        source,
    })
}

struct Context {
    platforms: Vec<String>,
    row_tests: BTreeMap<String, RowTests>,
    dogfood: Vec<String>,
    tag_jobs: BTreeMap<String, Vec<String>>,
    extra: BTreeMap<String, (Vec<String>, String)>,
}

impl Context {
    fn tested_on(&self, row: &Row, labels: &[String]) -> Result<Vec<String>, SupportMatrixError> {
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
                return Err(SupportMatrixError::UnqueriedLabel {
                    row: row.name.clone(),
                    label: label.clone(),
                });
            };
            if label.ends_with("_upstream") && tags.iter().any(|t| self.tag_jobs.contains_key(t)) {
                return Err(SupportMatrixError::FilteredConsumer {
                    label: label.clone(),
                });
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
                        return Err(SupportMatrixError::UnknownWorkflow {
                            row: row.name.clone(),
                            workflow: workflow.clone(),
                        });
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
///
/// # Errors
///
/// Returns a [`SupportMatrixError`] naming the input that is missing, invalid or inconsistent.
pub fn row_platforms(
    facts: &Facts,
    source: &dyn Source,
) -> Result<BTreeMap<String, RowPlatforms>, SupportMatrixError> {
    let ci = source.read(CI_WORKFLOW)?;
    let dogfood = workflows::dogfood_platforms(CI_WORKFLOW, &ci)?;
    let runners = workflows::runner_platforms(CI_WORKFLOW, &ci)?;
    let ids: Vec<&str> = facts.platforms.iter().map(|p| p.id.as_str()).collect();
    for platform in dogfood.iter().chain(runners.values()) {
        if !ids.contains(&platform.as_str()) {
            return Err(SupportMatrixError::UnknownPlatform {
                platform: platform.clone(),
            });
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
            return Err(SupportMatrixError::UnknownRow {
                labels_path: ROW_LABELS_PATH,
                tag: "row:",
                row: name.clone(),
            });
        }
    }
    let consumer_labels = read_consumer_labels(source)?;
    for name in consumer_labels.keys() {
        if !facts.rows.iter().any(|row| &row.name == name) {
            return Err(SupportMatrixError::UnknownRow {
                labels_path: CONSUMER_LABELS_PATH,
                tag: "consumer-row:",
                row: name.clone(),
            });
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
                build: context.tested_on(row, labels)?,
                consumer: context.tested_on(row, consumers)?,
            },
        );
    }
    Ok(out)
}

/// Checks that every `lang.json` language has one manifest entry per suite column and `lang.json` platform, and that each run entry names its test.
///
/// # Errors
///
/// Returns a [`SupportMatrixError`] naming the input that is missing, invalid or inconsistent.
pub fn check_conformance(facts: &Facts) -> Result<(), SupportMatrixError> {
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
                        return Err(SupportMatrixError::UndeclaredSuiteTest {
                            row: row.name.clone(),
                            test: (*test).to_owned(),
                            platform: platform.clone(),
                        });
                    }
                    _ => {
                        return Err(SupportMatrixError::ManifestEntries {
                            row: row.name.clone(),
                            test: (*test).to_owned(),
                            platform: platform.clone(),
                            count: entries.len(),
                        });
                    }
                }
            }
        }
    }
    Ok(())
}

/// Generates the page from the facts and the workspace sources.
///
/// # Errors
///
/// Returns a [`SupportMatrixError`] naming the input that is missing, invalid or inconsistent.
pub fn generate(facts_json: &str, source: &dyn Source) -> Result<String, SupportMatrixError> {
    let facts = parse_facts(facts_json)?;
    check_conformance(&facts)?;
    let platforms = row_platforms(&facts, source)?;
    Ok(render::render(&facts, &platforms))
}

/// Renders the per-language skip reasons as JSON keyed by the unsupported constraint label.
#[must_use]
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
#[must_use]
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
            writeln!(out, "-{}: {old}", index + 1).unwrap_or_default();
        }
        if let Some(new) = new {
            writeln!(out, "+{}: {new}", index + 1).unwrap_or_default();
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
                 "workflows": []},
                {"name": "go", "title": "Go", "section": "languages", "families": [],
                 "workflows": []}
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
