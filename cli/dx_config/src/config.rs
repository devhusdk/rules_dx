use std::collections::BTreeMap;
use std::path::Path;

use schemars::JsonSchema;
use serde::Deserialize;

use crate::conventions::{self, ConventionProblem};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Profile {
    #[default]
    Standard,
    Strict,
}

impl Profile {
    pub fn as_str(self) -> &'static str {
        match self {
            Profile::Standard => "standard",
            Profile::Strict => "strict",
        }
    }
}

/// The optional root file for tier, tool selection and scope.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DxConfig {
    pub schema_version: u32,
    /// The tier for every family that does not set its own.
    #[serde(default, rename = "tier")]
    pub profile: Option<Profile>,
    /// Tools dropped from every family, for example `["typos"]`.
    #[serde(default)]
    pub skip_tools: Vec<String>,
    /// Per-family overrides, keyed by family id.
    #[serde(default)]
    pub families: BTreeMap<String, FamilyConfig>,
    #[serde(default)]
    pub scope: Scope,
    #[serde(default)]
    pub large_files: LargeFiles,
    #[serde(default)]
    pub check: CheckConfig,
}

/// Oversized files a change may keep. Lock files are always allowed.
#[derive(Clone, Debug, Default, Eq, PartialEq, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LargeFiles {
    /// Paths, or `dir/` prefixes, that the large-file check skips.
    #[serde(default)]
    pub allow: Vec<String>,
}

/// What `dx check --affected` runs besides the quality phases.
#[derive(Clone, Debug, Default, Eq, PartialEq, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CheckConfig {
    /// Lint the commit subjects since the merge base. Off when left out.
    #[serde(default)]
    pub commits: Option<CommitsPolicy>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum CommitsPolicy {
    /// Every subject must be a conventional commit.
    Conventional,
}

/// Changes the curated tools of one family. A capability left out keeps the curated tools.
/// A list replaces them, unless every entry starts with `+` or `-`: then it adds or removes
/// those tools, as in `lint = ["+yamllint", "-typos"]`.
#[derive(Clone, Debug, Default, Eq, PartialEq, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FamilyConfig {
    #[serde(default, rename = "tier")]
    pub profile: Option<Profile>,
    #[serde(default)]
    pub lint: Option<Vec<String>>,
    #[serde(default)]
    pub format: Option<Vec<String>>,
    #[serde(default)]
    pub typecheck: Option<Vec<String>>,
    #[serde(default)]
    pub audit: Option<Vec<String>>,
    /// Turns the whole family off.
    #[serde(default)]
    pub disabled: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Scope {
    /// Bazel label patterns that dx leaves out.
    #[serde(default)]
    pub exclude: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum LoadProblem {
    #[error("cannot read {path}: {message}")]
    Read { path: String, message: String },
    #[error("invalid TOML: {message}")]
    InvalidToml { message: String },
    #[error("`paths` is not supported yet")]
    PathsNotSupported,
    #[error("{0}")]
    Convention(#[from] ConventionProblem),
    #[error("{message}")]
    Invalid { message: String },
}

/// Reads `dx.toml` in `workspace`. A missing file is `None`, which means the standard tier.
pub fn load(workspace: &Path) -> Result<Option<DxConfig>, LoadProblem> {
    let path = workspace.join("dx.toml");
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(LoadProblem::Read {
                path: path.display().to_string(),
                message: error.to_string(),
            });
        }
    };
    parse(&text).map(Some)
}

/// Prefixes `message` with `dx.toml:<line>:<column>:` when the error has a position.
fn located(text: &str, error: &toml::de::Error, message: String) -> String {
    let Some(span) = error.span() else {
        return message;
    };
    let before = &text[..span.start.min(text.len())];
    let line = before.matches('\n').count() + 1;
    let column = before.rsplit('\n').next().map_or(0, |last| last.chars().count()) + 1;
    format!("dx.toml:{line}:{column}: {message}")
}

/// Parses and validates the text of a `dx.toml`.
pub fn parse(text: &str) -> Result<DxConfig, LoadProblem> {
    let table: toml::Table =
        text.parse()
            .map_err(|error: toml::de::Error| LoadProblem::InvalidToml {
                message: located(text, &error, error.message().to_owned()),
            })?;
    if table.contains_key("paths") {
        return Err(LoadProblem::PathsNotSupported);
    }
    conventions::check_schema_version(table.get("schema_version"))?;
    let config: DxConfig =
        toml::from_str(text).map_err(|error: toml::de::Error| LoadProblem::Invalid {
            message: located(text, &error, conventions::explain_unknown_key(error.message())),
        })?;
    for pattern in &config.scope.exclude {
        if !conventions::is_valid_pattern(pattern) {
            return Err(ConventionProblem::BadPattern {
                value: pattern.clone(),
            }
            .into());
        }
    }
    Ok(config)
}
