#![deny(clippy::indexing_slicing, clippy::string_slice)]
#![cfg_attr(
    test,
    expect(
        clippy::disallowed_methods,
        reason = "tests run local programs under the Bazel test timeout"
    )
)]

mod references;
mod root_block;
mod sync;
pub mod tools_gen;

use std::collections::{BTreeMap, BTreeSet};

use dx_config::{CROSS_FAMILY_TOOLS, DxConfig, SelectionProblem, Tier};
use dx_preset::Layer;

pub use crate::references::dx_paths;
pub use crate::sync::{Mode, SyncError, SyncReport, plan, sync};
use crate::tools_gen::{TOOL_CONFIGS, ToolConfig};

pub struct RootBuild {
    pub name: String,
    pub text: String,
}

pub struct Inputs {
    pub config: Option<DxConfig>,
    pub repo: String,
    pub root_files: BTreeSet<String>,
    pub root_build: Option<RootBuild>,
    pub root_texts: BTreeMap<String, String>,
    pub files: BTreeSet<String>,
    pub families: BTreeSet<String>,
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum GenerateError {
    #[error(transparent)]
    Selection(#[from] SelectionProblem),
    #[error(
        "{build} exports `{file}` with visibility [{visibility}], which //.dx cannot see; \
         add \"//.dx:__pkg__\" to that visibility"
    )]
    NarrowExport {
        build: String,
        file: String,
        visibility: String,
    },
    #[error(
        "{build} has an exports_files call with a non-literal argument ({argument}); \
         list its files and visibility as plain strings so dx can read them"
    )]
    NonLiteralExport { build: String, argument: String },
    #[error("{file}: the {tool} reference `{path}` cannot be bound: {problem}")]
    Reference {
        tool: String,
        file: String,
        path: String,
        problem: String,
    },
    #[error("{build}: {source}")]
    Merge {
        build: String,
        source: dx_merge::Error,
    },
}

pub const GITLEAKS_BASE_PATH: &str = ".dx/config/gitleaks/base.toml";

const GITLEAKS_STANDARD: &str = include_str!("../../../core/quality/tiers/gitleaks/standard.toml");
const GITLEAKS_STRICT: &str = include_str!("../../../core/quality/tiers/gitleaks/strict.toml");

pub const POLICY_LABEL: &str = "//.dx:policy";

/// Tools whose aspect attr reads `//config:tool_<id>`; the rc points unselected ones at `//config:off_tool`.
const FLAGGED_TOOLS: &[&str] = &["pydoclint", "ruff", "ty"];

#[must_use]
pub fn base_path(config: &ToolConfig) -> Option<String> {
    (config.kind == "file" && config.tier_active && !config.ext.is_empty())
        .then(|| format!(".dx/config/{}/base.{}", tier_dir(config), config.ext))
}

#[must_use]
pub fn tier_dir(config: &ToolConfig) -> &str {
    if config.tier_dir.is_empty() {
        config.id
    } else {
        config.tier_dir
    }
}

struct Tool {
    config: &'static ToolConfig,
    tier: Tier,
}

impl Tool {
    fn dir(&self) -> &'static str {
        tier_dir(self.config)
    }

    fn tier(&self) -> &'static str {
        match self.tier {
            Tier::Standard => self.config.standard,
            Tier::Strict => self.config.strict,
        }
    }

    fn base(&self) -> Option<String> {
        base_path(self.config).map(|path| path.replacen(".dx/", "", 1))
    }

    fn root_file<'a>(&self, root_files: &'a BTreeSet<String>) -> Option<&'a str> {
        root_file(self.config, root_files)
    }
}

pub fn root_file<'a>(config: &ToolConfig, root_files: &'a BTreeSet<String>) -> Option<&'a str> {
    config
        .files
        .iter()
        .filter(|name| config.suffixes.iter().any(|suffix| name.ends_with(suffix)))
        .find_map(|name| root_files.get(*name))
        .map(String::as_str)
}

pub fn selected_configs(
    config: Option<&DxConfig>,
    families: &BTreeSet<String>,
) -> Result<Vec<&'static ToolConfig>, GenerateError> {
    let selection = dx_config::effective(config)?;
    let families = &with_named(config, families);
    let tiers = selected_tiers(&selection, families);
    Ok(TOOL_CONFIGS
        .iter()
        .filter(|config| tiers.contains_key(config.id))
        .collect())
}

fn with_named(config: Option<&DxConfig>, families: &BTreeSet<String>) -> BTreeSet<String> {
    let mut all = families.clone();
    all.extend(
        config
            .into_iter()
            .flat_map(|config| config.families.keys().cloned()),
    );
    all
}

fn selected_tiers<'a>(
    selection: &'a dx_config::Selection,
    families: &BTreeSet<String>,
) -> BTreeMap<&'a str, Tier> {
    let mut tiers = BTreeMap::new();
    for (id, family) in &selection.families {
        if !families.contains(id) {
            continue;
        }
        for tool in family
            .lint
            .iter()
            .chain(&family.format)
            .chain(&family.typecheck)
            .chain(&family.audit)
        {
            let tier = if CROSS_FAMILY_TOOLS.contains(&tool.as_str()) {
                selection.tier
            } else {
                family.tier
            };
            tiers.insert(tool.as_str(), tier);
        }
    }
    tiers
}

pub fn bound_configs(
    config: Option<&DxConfig>,
    families: &BTreeSet<String>,
    root_files: &BTreeSet<String>,
) -> Result<Vec<(&'static str, String)>, GenerateError> {
    let selection = dx_config::effective(config)?;
    let families = &with_named(config, families);
    let tiers = selected_tiers(&selection, families);
    let mut bound: Vec<(&'static str, String)> = TOOL_CONFIGS
        .iter()
        .filter(|config| !config.suffixes.is_empty())
        .filter_map(|config| {
            let tool = Tool {
                config,
                tier: *tiers.get(config.id)?,
            };
            Some((config.id, tool.root_file(root_files)?.to_owned()))
        })
        .collect();
    bound.sort();
    Ok(bound)
}

pub fn render(inputs: &Inputs) -> Result<BTreeMap<String, Vec<u8>>, GenerateError> {
    let selection = dx_config::effective(inputs.config.as_ref())?;
    let root = selection.tier;
    let detected = with_named(inputs.config.as_ref(), &inputs.families);
    let tiers = selected_tiers(&selection, &detected);
    let families: BTreeMap<&str, &dx_config::FamilySelection> = selection
        .families
        .iter()
        .filter(|(id, _)| detected.contains(*id))
        .map(|(id, family)| (id.as_str(), family))
        .collect();
    let mut tools: Vec<Tool> = TOOL_CONFIGS
        .iter()
        .filter_map(|config| {
            tiers.get(config.id).map(|tier| Tool {
                config,
                tier: *tier,
            })
        })
        .collect();
    tools.sort_by_key(|tool| tool.config.id);

    let mut out = BTreeMap::new();
    for tool in &tools {
        if let Some(base) = tool.base() {
            out.insert(format!(".dx/{base}"), tool.tier().as_bytes().to_vec());
        }
    }
    out.insert(
        GITLEAKS_BASE_PATH.to_owned(),
        match root {
            Tier::Standard => GITLEAKS_STANDARD,
            Tier::Strict => GITLEAKS_STRICT,
        }
        .as_bytes()
        .to_vec(),
    );
    let bound: Vec<(&Tool, &str)> = tools
        .iter()
        .filter(|tool| !tool.config.suffixes.is_empty())
        .filter_map(|tool| Some((tool, tool.root_file(&inputs.root_files)?)))
        .collect();
    let mut data: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    let mut exports: BTreeSet<&str> = bound.iter().map(|(_, file)| *file).collect();
    let mut referenced: BTreeSet<String> = BTreeSet::new();
    for (tool, file) in &bound {
        let Some(text) = inputs.root_texts.get(*file) else {
            continue;
        };
        let mut labels = Vec::new();
        for reference in references::resolve(tool.config, file, text, &inputs.files)? {
            labels.push(reference.label);
            referenced.extend(reference.export);
        }
        data.insert(tool.config.id, labels);
    }
    exports.extend(referenced.iter().map(String::as_str));
    out.insert(
        ".dx/BUILD.bazel".to_owned(),
        dx_build(
            &inputs.repo,
            root,
            &families,
            &selection.disabled,
            &tools,
            &bound,
            &data,
        )
        .into_bytes(),
    );
    for layer in Layer::ALL {
        out.insert(
            format!(".dx/bazelrc/{}", layer.file_name()),
            bazelrc(layer, &inputs.repo, root, &tools, &tiers, &detected).into_bytes(),
        );
    }
    if let Some((path, text)) = root_block::render(inputs.root_build.as_ref(), &exports)? {
        out.insert(path, text.into_bytes());
    }
    Ok(out)
}

fn list(items: &[String], indent: usize) -> String {
    match items {
        [] => "[]".to_owned(),
        [one] => format!("[\"{one}\"]"),
        _ => {
            let pad = " ".repeat(indent + 4);
            let body: String = items
                .iter()
                .map(|item| format!("{pad}\"{item}\",\n"))
                .collect();
            format!("[\n{body}{}]", " ".repeat(indent))
        }
    }
}

fn dx_build(
    repo: &str,
    root: Tier,
    families: &BTreeMap<&str, &dx_config::FamilySelection>,
    disabled: &BTreeSet<String>,
    tools: &[Tool],
    bound: &[(&Tool, &str)],
    referenced: &BTreeMap<&str, Vec<String>>,
) -> String {
    let mut text = String::from(
        "# gazelle:ignore\n# Generated by dx from dx.toml; dx generate rewrites this file.\n\n",
    );
    let mut symbols: Vec<String> = bound
        .iter()
        .map(|(tool, _)| format!("\"{}_config\"", tool.config.id))
        .chain(std::iter::once("\"dx_generated_policy\"".to_string()))
        .collect();
    symbols.sort();
    text.push_str(&format!(
        "load(\"@{repo}//core:defs.bzl\", {})\n\n\
         package(default_visibility = [\"//visibility:public\"])\n",
        symbols.join(", ")
    ));
    for (tool, file) in bound {
        let mut data: Vec<String> = tool.base().into_iter().collect();
        data.extend(
            referenced
                .get(tool.config.id)
                .into_iter()
                .flatten()
                .cloned(),
        );
        text.push_str(&format!(
            "\n{id}_config(\n    name = \"{id}\",\n    src = \"//:{file}\",\n",
            id = tool.config.id
        ));
        if !data.is_empty() {
            text.push_str(&format!("    data = {},\n", list(&data, 4)));
        }
        text.push_str(")\n");
    }
    text.push_str("\ndx_generated_policy(\n    name = \"policy\",\n");
    let bases: Vec<String> = tools
        .iter()
        .filter(|tool| tool.config.tier_base && tool.tier != root)
        .map(|tool| {
            format!(
                "        \"{}\": \"@{repo}//core/quality/tiers/{}:{}\",\n",
                tool.config.id,
                tool.dir(),
                tool.tier.as_str()
            )
        })
        .collect();
    if !bases.is_empty() {
        text.push_str(&format!("    bases = {{\n{}    }},\n", bases.concat()));
    }
    let configs: Vec<String> = bound
        .iter()
        .map(|(tool, _)| format!(":{}", tool.config.id))
        .collect();
    if !configs.is_empty() {
        text.push_str(&format!("    configs = {},\n", list(&configs, 4)));
    }
    if !disabled.is_empty() {
        let disabled: Vec<String> = disabled.iter().cloned().collect();
        text.push_str(&format!("    disabled = {},\n", list(&disabled, 4)));
    }
    text.push_str("    families = {");
    if families.is_empty() {
        text.push_str("},\n");
    } else {
        text.push('\n');
        for (id, family) in families {
            text.push_str(&format!("        \"{id}\": {{"));
            let capabilities = [
                ("audit", &family.audit),
                ("format", &family.format),
                ("lint", &family.lint),
                ("typecheck", &family.typecheck),
            ];
            let mut fields: Vec<String> = capabilities
                .iter()
                .filter(|(_, tools)| !tools.is_empty())
                .map(|(key, tools)| format!("            \"{key}\": {},\n", list(tools, 12)))
                .collect();
            let disabled: Vec<String> = capabilities
                .iter()
                .filter(|(_, tools)| tools.is_empty())
                .map(|(key, _)| (*key).to_owned())
                .collect();
            if !disabled.is_empty() {
                fields.push(format!(
                    "            \"disabled\": {},\n",
                    list(&disabled, 12)
                ));
            }
            if family.tier != root {
                fields.push(format!(
                    "            \"tier\": \"{}\",\n",
                    family.tier.as_str()
                ));
            }
            fields.sort();
            if fields.is_empty() {
                text.push_str("},\n");
            } else {
                text.push_str(&format!("\n{}        }},\n", fields.concat()));
            }
        }
        text.push_str("    },\n");
    }
    text.push_str(&format!("    tier = \"{}\",\n)\n", root.as_str()));
    text
}

#[must_use]
pub fn policy_families(text: &str) -> BTreeMap<String, BTreeMap<String, Vec<String>>> {
    let Some(rest) = text
        .find("families = {")
        .and_then(|start| text.get(start + "families = ".len()..))
    else {
        return BTreeMap::new();
    };
    let mut literal = Literal { rest };
    let Some(Value::Dict(families)) = literal.value() else {
        return BTreeMap::new();
    };
    families
        .into_iter()
        .filter_map(|(family, fields)| {
            let Value::Dict(fields) = fields else {
                return None;
            };
            let capabilities = fields
                .into_iter()
                .filter_map(|(capability, tools)| match tools {
                    Value::List(tools) if capability != "disabled" => Some((
                        capability,
                        tools
                            .into_iter()
                            .filter_map(|tool| match tool {
                                Value::Str(tool) => Some(tool),
                                Value::List(_) | Value::Dict(_) => None,
                            })
                            .collect(),
                    )),
                    Value::Str(_) | Value::List(_) | Value::Dict(_) => None,
                })
                .collect();
            Some((family, capabilities))
        })
        .collect()
}

enum Value {
    Str(String),
    List(Vec<Value>),
    Dict(Vec<(String, Value)>),
}

struct Literal<'a> {
    rest: &'a str,
}

impl Literal<'_> {
    fn eat(&mut self, token: char) -> bool {
        loop {
            self.rest = self.rest.trim_start();
            let Some(comment) = self.rest.strip_prefix('#') else {
                break;
            };
            self.rest = comment.split_once('\n').map_or("", |(_, rest)| rest);
        }
        match self.rest.strip_prefix(token) {
            Some(rest) => {
                self.rest = rest;
                true
            }
            None => false,
        }
    }

    fn value(&mut self) -> Option<Value> {
        if self.eat('"') {
            let (text, rest) = self.rest.split_once('"')?;
            self.rest = rest;
            return Some(Value::Str(text.to_owned()));
        }
        if self.eat('[') {
            let mut items = Vec::new();
            while !self.eat(']') {
                items.push(self.value()?);
                if !self.eat(',') {
                    return self.eat(']').then_some(Value::List(items));
                }
            }
            return Some(Value::List(items));
        }
        if self.eat('{') {
            let mut fields = Vec::new();
            while !self.eat('}') {
                let Value::Str(key) = self.value()? else {
                    return None;
                };
                if !self.eat(':') {
                    return None;
                }
                fields.push((key, self.value()?));
                if !self.eat(',') {
                    return self.eat('}').then_some(Value::Dict(fields));
                }
            }
            return Some(Value::Dict(fields));
        }
        None
    }
}

fn bazelrc(
    layer: Layer,
    repo: &str,
    root: Tier,
    tools: &[Tool],
    selected: &BTreeMap<&str, Tier>,
    detected: &BTreeSet<String>,
) -> String {
    let mut text = dx_preset::render_layer_with(layer, repo, Some(root == Tier::Strict));
    if layer != Layer::Correctness {
        return text;
    }
    text.push_str(&format!("\n# The workspace policy dx generates from dx.toml.\ncommon --@{repo}//config:workspace={POLICY_LABEL}\n"));
    let off: Vec<&str> = FLAGGED_TOOLS
        .iter()
        .copied()
        .filter(|tool| !selected.contains_key(tool))
        .collect();
    if !off.is_empty() {
        text.push_str("\n# The tools no family runs.\n");
        for tool in off {
            text.push_str(&format!(
                "common --@{repo}//config:tool_{tool}=@{repo}//config:off_tool\n"
            ));
        }
    }
    for tool in tools.iter().filter(|tool| tool.config.kind == "flags") {
        text.push_str(&format!("\n# The {} tier.\n", tool.config.id));
        let lines = tool
            .tier()
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty() && *line != "-Dwarnings");
        for line in lines.chain(["-Dwarnings"]) {
            text.push_str(&format!("common --@{repo}//config:clippy_flag={line}\n"));
        }
    }
    for (language, flags) in dx_families::setup_flags(detected.iter().map(String::as_str)) {
        text.push_str(&format!("\n# The {language} setup flags.\n"));
        for flag in flags {
            text.push_str(&flag.replace("@rules_dx//", &format!("@{repo}//")));
            text.push('\n');
        }
    }
    text
}

#[cfg(test)]
#[path = "render_tests.rs"]
mod render_tests;
