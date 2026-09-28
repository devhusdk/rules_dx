use documentation_ir::proto::DocIr;

use crate::common::{make_symbol, shard, text, SymbolSpec};
use crate::types::AdapterError;
use crate::{FSHARP_DOTNET_PIN, FSHARP_SERVICE_PIN};

pub fn normalize_fsharp(input: &str, package: &str) -> Result<DocIr, AdapterError> {
    if input.trim().is_empty() {
        return Err(AdapterError::EmptyInput);
    }
    let value: serde_json::Value =
        serde_json::from_str(input).map_err(|err| AdapterError::InvalidJson(err.to_string()))?;
    let sdk = text(value.get("sdk").unwrap_or(&serde_json::Value::Null));
    if sdk != FSHARP_DOTNET_PIN {
        return Err(AdapterError::VersionMismatch {
            expected: FSHARP_DOTNET_PIN.to_owned(),
            found: sdk,
        });
    }
    let service = text(value.get("fcs").unwrap_or(&serde_json::Value::Null));
    if service != FSHARP_SERVICE_PIN {
        return Err(AdapterError::VersionMismatch {
            expected: FSHARP_SERVICE_PIN.to_owned(),
            found: service,
        });
    }
    let entities = value
        .get("entities")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| AdapterError::InvalidJson("missing entities".to_owned()))?;
    let docs = value
        .get("docs")
        .and_then(serde_json::Value::as_object)
        .cloned()
        .unwrap_or_default();
    let mut symbols = Vec::new();
    for entity in entities {
        let signature = text(entity.get("signature").unwrap_or(&serde_json::Value::Null));
        let kind = text(entity.get("kind").unwrap_or(&serde_json::Value::Null));
        let name = text(entity.get("name").unwrap_or(&serde_json::Value::Null));
        let file = text(entity.get("file").unwrap_or(&serde_json::Value::Null));
        let line = entity
            .get("line")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(1);
        let doc = docs.get(&signature).map(text).unwrap_or_default();
        symbols.push(make_symbol(SymbolSpec {
            language: "fsharp",
            package,
            qualified: &name,
            kind_name: &kind.to_lowercase(),
            signature: format!("{kind} {name}"),
            doc,
            file,
            line,
        })?);
    }
    shard("fsharp", package, symbols)
}
