use crate::PowershellExtractError;
use serde::Deserialize;
use std::collections::BTreeMap;

pub const SCHEMA: &str = "dx.powershell.api/1";

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Api {
    pub schema: String,
    pub pwsh: String,
    #[serde(default)]
    pub manifest: Option<Manifest>,
    pub files: Vec<File>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub rel: String,
    #[serde(default)]
    pub root_module: Option<String>,
    #[serde(default)]
    pub module_version: Option<String>,
    #[serde(default)]
    pub functions_to_export: Option<Names>,
    #[serde(default)]
    pub aliases_to_export: Option<Names>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum Names {
    All(String),
    List(Vec<String>),
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct File {
    pub rel: String,
    pub functions: Vec<Function>,
    pub exports: Exports,
    #[serde(default)]
    pub unresolved_uses: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Exports {
    pub kind: ExportKind,
    pub functions: Vec<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ExportKind {
    None,
    Literal,
    Dynamic,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Function {
    pub name: String,
    #[serde(default)]
    pub scope: Option<String>,
    pub line: u64,
    #[serde(default)]
    pub help: Option<Help>,
    #[serde(default)]
    pub cmdlet_binding: Option<CmdletBinding>,
    pub output_types: Vec<String>,
    pub aliases: Vec<String>,
    pub params: Vec<Parameter>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CmdletBinding {
    #[serde(default)]
    pub default_set: Option<String>,
    pub positional_binding: bool,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Parameter {
    pub name: String,
    #[serde(default, rename = "type")]
    pub type_name: Option<String>,
    pub sets: Vec<ParameterSet>,
    pub aliases: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ParameterSet {
    pub name: String,
    pub mandatory: bool,
    #[serde(default)]
    pub position: Option<u32>,
    pub pipeline: bool,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Help {
    pub synopsis: String,
    pub description: String,
    pub parameters: BTreeMap<String, String>,
    pub examples: Vec<String>,
    pub outputs: Vec<String>,
    pub notes: String,
}

pub fn parse_api(json: &str) -> Result<Api, PowershellExtractError> {
    let value: serde_json::Value =
        serde_json::from_str(json).map_err(PowershellExtractError::MalformedApi)?;
    match value.get("schema").and_then(serde_json::Value::as_str) {
        Some(SCHEMA) => {}
        Some(other) => {
            return Err(PowershellExtractError::UnsupportedSchema {
                schema: other.to_owned(),
            });
        }
        None => return Err(PowershellExtractError::NoSchema),
    }
    serde_json::from_value(value).map_err(PowershellExtractError::ApiShape)
}
