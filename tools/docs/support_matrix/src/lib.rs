//! Generates `docs/support-matrix.md` from CI workflows, test BUILD files and the quality registry.

pub mod render;
pub mod runfiles_source;
pub mod starlark;
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
    /// The coverage collector; absent when coverage does not apply to the row.
    #[serde(default)]
    pub coverage: Option<Coverage>,
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

/// The coverage collectors of a row with a note on them, or the reason it has none.
#[derive(Clone, Debug, Deserialize)]
pub struct Coverage {
    pub collectors: Vec<String>,
    pub reason: String,
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
    pub quality: BTreeMap<String, BTreeMap<String, Vec<String>>>,
    pub support: Vec<Support>,
    pub deferred_families: Vec<String>,
    #[serde(default)]
    pub pending_compilers: BTreeMap<String, String>,
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
    pub not_languages: Vec<String>,
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

/// Family, then capability, then tool, then platform, to the reason.
pub type ToolNotApplicable =
    BTreeMap<String, BTreeMap<String, BTreeMap<String, BTreeMap<String, String>>>>;

/// Parses the facts JSON.
pub fn parse_facts(text: &str) -> Result<Facts, String> {
    serde_json::from_str(text).map_err(|error| format!("facts JSON is invalid: {error}"))
}

struct BuildLoader<'a> {
    source: &'a dyn Source,
    package: String,
    computed: &'a BTreeMap<String, Vec<Vec<String>>>,
}

fn bzl_path(package: &str, label: &str) -> Result<String, String> {
    let (pkg, file) = if let Some(rest) = label
        .strip_prefix("@rules_dx//")
        .or_else(|| label.strip_prefix("//"))
    {
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
        let label = label
            .strip_prefix("@rules_dx")
            .filter(|rest| rest.starts_with("//"))
            .unwrap_or(label);
        self.computed.get(&format!("{label}%{symbol}")).cloned()
    }
}

fn split_label(label: &str) -> Result<(String, &str), String> {
    let external = [
        "@rules_dx_android",
        "@rules_dx_apple",
        "@rules_dx_beam",
        "@rules_dx_dotnet",
        "@rules_dx_jvm",
    ]
    .into_iter()
    .find_map(|repo| Some((repo, label.strip_prefix(repo)?.strip_prefix("//")?)));
    let (prefix, rest) = match external {
        Some((repo, rest)) => (format!("{repo}/"), rest),
        None => (String::new(), label.strip_prefix("//").unwrap_or("")),
    };
    rest.split_once(':')
        .map(|(package, name)| (format!("{prefix}{package}"), name))
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
    fn tested_on(
        &self,
        row: &Row,
        labels: &[String],
        consumer: bool,
    ) -> Result<Vec<String>, String> {
        let mut on = BTreeSet::new();
        for label in labels {
            let (package, name) = split_label(label)?;
            let path = format!("{package}/BUILD.bazel");
            let file = starlark::parse(&path, &self.source.read(&path)?)?;
            let loader = BuildLoader {
                source: self.source,
                package: package.clone(),
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
            if consumer
                && row.section == "compilers"
                && !facts.tags.iter().any(|tag| tag == "hermetic-consumer")
            {
                return Err(format!(
                    "{label} is a consumer test of compiler row {} but lacks the hermetic-consumer tag, so it is not a nested qualification",
                    row.name
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
        source,
        computed: &facts.computed_constraints,
        platforms: facts
            .platforms
            .iter()
            .map(|platform| starlark::Platform {
                id: platform.id.clone(),
                os: platform.os.clone(),
                cpu: platform.cpu.clone(),
                constraints: platform.constraints.clone(),
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
                build: context.tested_on(row, &row.ci_tests, false)?,
                consumer: context.tested_on(row, &row.consumer_tests, true)?,
            },
        );
    }
    Ok(out)
}

/// Generates the page from the facts and the workspace sources.
pub fn generate(facts_json: &str, source: &dyn Source) -> Result<String, String> {
    let facts = parse_facts(facts_json)?;
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
    fn resolves_load_paths() {
        assert_eq!(
            bzl_path("a/b", "//tools/testing:nested_bazel.bzl").expect("path"),
            "tools/testing/nested_bazel.bzl"
        );
        assert_eq!(bzl_path("a/b", ":x.bzl").expect("path"), "a/b/x.bzl");
        assert_eq!(
            bzl_path("a", "@rules_dx//tools/testing:x.bzl").expect("path"),
            "tools/testing/x.bzl"
        );
        assert!(bzl_path("a", "@platforms//os:BUILD").is_err());
    }

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
              "unsupported_constraints": {"hs": "//haskell/toolchain:unsupported"},
              "not_languages": [], "computed_constraints": {}
            }"#,
        )
        .expect("facts");
        assert_eq!(
            unsupported_json(&facts),
            "{\n  \"//haskell/toolchain:unsupported\": {\n    \"platforms\": {\n      \"b\": \"no bindist\"\n    },\n    \"title\": \"Haskell\"\n  }\n}\n"
        );
    }

    #[test]
    fn diff_names_changed_lines() {
        assert_eq!(diff("a\nb\n", "a\nb\n"), None);
        assert_eq!(diff("a\nb\n", "a\nc\n").expect("diff"), "-2: b\n+2: c\n");
    }
}
