//! The dx.powershell.api/1 contract written by extract.ps1.

use serde::Deserialize;
use std::collections::BTreeMap;

/// The only contract version this mapper reads.
pub const SCHEMA: &str = "dx.powershell.api/1";

/// Every fact extract.ps1 reports about one module.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Api {
    pub schema: String,
    pub pwsh: String,
    #[serde(default)]
    pub manifest: Option<Manifest>,
    pub files: Vec<File>,
}

/// The constant keys of one module manifest.
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

/// A manifest export list: either "*" or a list of names and wildcards.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum Names {
    All(String),
    List(Vec<String>),
}

/// The facts of one source file.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct File {
    pub rel: String,
    pub functions: Vec<Function>,
    pub exports: Exports,
    #[serde(default)]
    pub unresolved_uses: Vec<String>,
}

/// The Export-ModuleMember calls of one file.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Exports {
    pub kind: ExportKind,
    pub functions: Vec<String>,
}

/// Whether a file exports functions, and whether the names are literal.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ExportKind {
    None,
    Literal,
    Dynamic,
}

/// One top-level function definition.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Function {
    pub name: String,
    pub line: u64,
    #[serde(default)]
    pub help: Option<Help>,
    #[serde(default)]
    pub cmdlet_binding: Option<CmdletBinding>,
    pub output_types: Vec<String>,
    pub aliases: Vec<String>,
    pub params: Vec<Parameter>,
}

/// The CmdletBinding attribute of one function.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CmdletBinding {
    #[serde(default)]
    pub default_set: Option<String>,
    pub positional_binding: bool,
}

/// One parameter of a function.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Parameter {
    pub name: String,
    #[serde(default, rename = "type")]
    pub type_name: Option<String>,
    pub sets: Vec<ParameterSet>,
    pub aliases: Vec<String>,
}

/// One Parameter attribute: the set it belongs to and how it binds there.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ParameterSet {
    pub name: String,
    pub mandatory: bool,
    #[serde(default)]
    pub position: Option<u32>,
    pub pipeline: bool,
}

/// Comment-based help as the parser reports it.
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

/// Parses the contract, rejecting other versions before any field check.
pub fn parse_api(json: &str) -> Result<Api, String> {
    let value: serde_json::Value =
        serde_json::from_str(json).map_err(|error| format!("API JSON is malformed: {error}"))?;
    match value.get("schema").and_then(serde_json::Value::as_str) {
        Some(SCHEMA) => {}
        Some(other) => {
            return Err(format!(
                "API JSON schema '{other}' is not supported, expected '{SCHEMA}'"
            ))
        }
        None => return Err("API JSON has no schema".to_owned()),
    }
    serde_json::from_value(value)
        .map_err(|error| format!("API JSON does not match {SCHEMA}: {error}"))
}
