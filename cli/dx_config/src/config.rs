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
    #[serde(default)]
    pub profile: Option<Profile>,
    /// Per-family overrides, keyed by family id.
    #[serde(default)]
    pub families: BTreeMap<String, FamilyConfig>,
    #[serde(default)]
    pub scope: Scope,
    #[serde(default)]
    pub large_files: LargeFiles,
}

/// Oversized files a change may keep. Lock files are always allowed.
#[derive(Clone, Debug, Default, Eq, PartialEq, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LargeFiles {
    /// Paths, or `dir/` prefixes, that the large-file check skips.
    #[serde(default)]
    pub allow: Vec<String>,
}

/// Replaces the curated tools of one family. A capability left out keeps the curated tools.
#[derive(Clone, Debug, Default, Eq, PartialEq, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FamilyConfig {
    #[serde(default)]
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
            })
        }
    };
    parse(&text).map(Some)
}

/// Parses and validates the text of a `dx.toml`.
pub fn parse(text: &str) -> Result<DxConfig, LoadProblem> {
    let table: toml::Table =
        text.parse()
            .map_err(|error: toml::de::Error| LoadProblem::InvalidToml {
                message: error.message().to_owned(),
            })?;
    if table.contains_key("paths") {
        return Err(LoadProblem::PathsNotSupported);
    }
    conventions::check_schema_version(table.get("schema_version"))?;
    let config: DxConfig =
        table
            .try_into()
            .map_err(|error: toml::de::Error| LoadProblem::Invalid {
                message: conventions::explain_unknown_key(error.message()),
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
