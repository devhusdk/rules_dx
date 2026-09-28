use documentation_ir::proto::DocIr;

use crate::common::{make_symbol, shard, text, SymbolSpec};
use crate::types::AdapterError;
use crate::CSHARP_DOTNET_PIN;

pub fn normalize_csharp(input: &str, package: &str) -> Result<DocIr, AdapterError> {
    if input.trim().is_empty() {
        return Err(AdapterError::EmptyInput);
    }
    let value: serde_json::Value =
        serde_json::from_str(input).map_err(|err| AdapterError::InvalidJson(err.to_string()))?;
    let found = text(value.get("sdk").unwrap_or(&serde_json::Value::Null));
    if found != CSHARP_DOTNET_PIN {
        return Err(AdapterError::VersionMismatch {
            expected: CSHARP_DOTNET_PIN.to_owned(),
            found,
        });
    }
    let members = value
        .get("members")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| AdapterError::InvalidJson("missing members".to_owned()))?;
    let docs = value
        .get("docs")
        .and_then(serde_json::Value::as_object)
        .cloned()
        .unwrap_or_default();
    let mut symbols = Vec::new();
    for member in members {
        let id = text(member.get("id").unwrap_or(&serde_json::Value::Null));
        let kind = text(member.get("kind").unwrap_or(&serde_json::Value::Null));
        let name = text(member.get("name").unwrap_or(&serde_json::Value::Null));
        let file = text(member.get("file").unwrap_or(&serde_json::Value::Null));
        let line = member
            .get("line")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(1);
        let doc = docs.get(&id).map(text).unwrap_or_default();
        symbols.push(make_symbol(SymbolSpec {
            language: "csharp",
            package,
            qualified: &name,
            kind_name: &kind.to_lowercase(),
            signature: format!("{kind} {name}"),
            doc,
            file,
            line,
        })?);
        let _ = id;
    }
    shard("csharp", package, symbols)
}
