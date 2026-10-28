//! The dx.dotnet-api/1 contract written by the C# and F# drivers.

use serde::Deserialize;

/// The only contract version this mapper reads.
pub const SCHEMA: &str = "dx.dotnet-api/1";

/// Every public fact one driver reports about one assembly.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Api {
    pub schema: String,
    pub language: String,
    pub assembly: String,
    pub items: Vec<Item>,
}

/// One public namespace, module, type or member.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Item {
    pub doc_id: String,
    pub kind: String,
    pub name: String,
    #[serde(default)]
    pub display_name: String,
    #[serde(default)]
    pub parent_doc_id: Option<String>,
    #[serde(default)]
    pub signature: String,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub remarks: String,
    #[serde(default)]
    pub params: Vec<Param>,
    #[serde(default)]
    pub returns: Option<Returns>,
    #[serde(default)]
    pub examples: Vec<String>,
    #[serde(default)]
    pub file: String,
    #[serde(default)]
    pub line: u64,
    #[serde(default)]
    pub implements: Vec<String>,
    #[serde(default)]
    pub incomplete: Vec<String>,
}

/// One documented parameter.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Param {
    pub name: String,
    #[serde(rename = "type", default)]
    pub type_name: String,
    #[serde(default)]
    pub doc: String,
}

/// The documented return value.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Returns {
    #[serde(rename = "type", default)]
    pub type_name: String,
    #[serde(default)]
    pub doc: String,
}

/// Parses one API document, rejecting a wrong schema version or unknown fields.
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
