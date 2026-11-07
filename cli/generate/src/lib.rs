#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

//! Renders the `.dx` tree and the root export block from `dx.toml`, without IO.

mod references;
mod root_block;
pub mod tools_gen;

use std::collections::{BTreeMap, BTreeSet};

use dx_config::{CROSS_FAMILY_TOOLS, DxConfig, Profile, SelectionProblem};
use dx_preset::Layer;

use crate::tools_gen::{TOOL_CONFIGS, ToolConfig};

/// The root BUILD file: its name (`BUILD` or `BUILD.bazel`) and text.
pub struct RootBuild {
    pub name: String,
    pub text: String,
}

/// Everything `render` reads.
pub struct Inputs {
    pub config: Option<DxConfig>,
    /// Apparent name the workspace gives rules_dx.
    pub repo: String,
    /// Names of the files in the workspace root.
    pub root_files: BTreeSet<String>,
    pub root_build: Option<RootBuild>,
    /// Text of the root config files that name other files.
    pub root_texts: BTreeMap<String, String>,
    /// Every workspace-relative file path, including BUILD files.
    pub files: BTreeSet<String>,
    /// Families whose files occur in the workspace.
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

/// The policy the correctness rc selects.
pub const POLICY_LABEL: &str = "//.dx:policy";

/// One selected tool and the tier profile it runs on.
struct Tool {
    config: &'static ToolConfig,
    profile: Profile,
}

impl Tool {
    fn dir(&self) -> &'static str {
        if self.config.tier_dir.is_empty() {
            self.config.id
        } else {
            self.config.tier_dir
        }
    }

    fn tier(&self) -> &'static str {
        match self.profile {
            Profile::Standard => self.config.standard,
            Profile::Strict => self.config.strict,
        }
    }

    /// `config/<dir>/base.<ext>` inside `.dx`, for a file tool with a tier.
    fn base(&self) -> Option<String> {
        let config = self.config;
        (config.kind == "file" && config.tier_active && !config.ext.is_empty())
            .then(|| format!("config/{}/base.{}", self.dir(), config.ext))
    }

    /// The root file a `<tool>_config` target binds, when the workspace has one.
    fn root_file<'a>(&self, root_files: &'a BTreeSet<String>) -> Option<&'a str> {
        let config = self.config;
        config
            .files
            .iter()
            .filter(|name| config.suffixes.iter().any(|suffix| name.ends_with(suffix)))
            .find_map(|name| root_files.get(*name))
            .map(String::as_str)
    }
}

/// Every file dx owns, keyed by workspace-relative path.
pub fn render(inputs: &Inputs) -> Result<BTreeMap<String, Vec<u8>>, GenerateError> {
    let selection = dx_config::effective(inputs.config.as_ref())?;
    let root = selection.profile;
    let mut families = BTreeMap::new();
    let mut profiles: BTreeMap<&str, Profile> = BTreeMap::new();
    for (id, family) in &selection.families {
        if !inputs.families.contains(id) {
            continue;
        }
        families.insert(id.as_str(), family);
        for tool in family
            .lint
            .iter()
            .chain(&family.format)
            .chain(&family.typecheck)
            .chain(&family.audit)
        {
            let profile = if CROSS_FAMILY_TOOLS.contains(&tool.as_str()) {
                root
            } else {
                family.profile
            };
            profiles.insert(tool, profile);
        }
    }
    let mut tools: Vec<Tool> = TOOL_CONFIGS
        .iter()
        .filter_map(|config| {
            profiles.get(config.id).map(|profile| Tool {
                config,
                profile: *profile,
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
            bazelrc(layer, &inputs.repo, root, &tools).into_bytes(),
        );
    }
    if let Some((path, text)) = root_block::render(inputs.root_build.as_ref(), &exports)? {
        out.insert(path, text.into_bytes());
    }
    Ok(out)
}

/// A Starlark list the way buildifier prints it at `indent` spaces.
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
    root: Profile,
    families: &BTreeMap<&str, &dx_config::FamilySelection>,
    disabled: &BTreeSet<String>,
    tools: &[Tool],
    bound: &[(&Tool, &str)],
    referenced: &BTreeMap<&str, Vec<String>>,
) -> String {
    let mut text = String::from(
        "# gazelle:ignore\n# Generated by dx from dx.toml; dx generate rewrites this file.\n\n",
    );
    if !bound.is_empty() {
        let symbols: Vec<String> = bound
            .iter()
            .map(|(tool, _)| format!("\"{}_config\"", tool.config.id))
            .collect();
        text.push_str(&format!(
            "load(\"@{repo}//quality:native_config.bzl\", {})\n",
            symbols.join(", ")
        ));
    }
    text.push_str(&format!(
        "load(\"@{repo}//quality/private:generated_policy.bzl\", \"dx_generated_policy\")\n\n\
         package(default_visibility = [\"//visibility:public\"])\n"
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
        .filter(|tool| tool.config.tier_base && tool.profile != root)
        .map(|tool| {
            format!(
                "        \"{}\": \"@{repo}//quality/tiers/{}:{}\",\n",
                tool.config.id,
                tool.dir(),
                tool.profile.as_str()
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
            if family.profile != root {
                fields.push(format!(
                    "            \"profile\": \"{}\",\n",
                    family.profile.as_str()
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
    text.push_str(&format!("    profile = \"{}\",\n)\n", root.as_str()));
    text
}

fn bazelrc(layer: Layer, repo: &str, root: Profile, tools: &[Tool]) -> String {
    let mut text =
        dx_preset::render_layer_pinned(layer, repo, Some(root == Profile::Strict), false);
    if layer != Layer::Correctness {
        return text;
    }
    text.push_str(&format!(
        "\n# The workspace policy dx generates from dx.toml.\ncommon --@{repo}//config:workspace={POLICY_LABEL}\n"
    ));
    for tool in tools.iter().filter(|tool| tool.config.kind == "flags") {
        text.push_str(&format!("\n# The {} tier.\n", tool.config.id));
        let lines = tool
            .tier()
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty() && *line != "-Dwarnings");
        // Setting any clippy_flag drops rules_rust's default `-Dwarnings`, so it comes back last.
        for line in lines.chain(["-Dwarnings"]) {
            text.push_str(&format!("common --@{repo}//config:clippy_flag={line}\n"));
        }
    }
    text
}

#[cfg(test)]
#[path = "render_tests.rs"]
mod render_tests;
