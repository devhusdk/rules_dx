use documentation_ir::proto::DocIr;

use crate::common::{make_symbol, shard, text, SymbolSpec};
use crate::types::AdapterError;
use crate::SCALA_PIN;

pub fn normalize_scala(input: &str, package: &str) -> Result<DocIr, AdapterError> {
    if input.trim().is_empty() {
        return Err(AdapterError::EmptyInput);
    }
    let value: serde_json::Value =
        serde_json::from_str(input).map_err(|err| AdapterError::InvalidJson(err.to_string()))?;
    let scala = text(value.get("scala").unwrap_or(&serde_json::Value::Null));
    if scala != SCALA_PIN {
        return Err(AdapterError::VersionMismatch {
            expected: SCALA_PIN.to_owned(),
            found: scala,
        });
    }
    let tasty = value.get("tasty").ok_or(AdapterError::MissingTasty)?;
    if tasty
        .get("missing")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
    {
        return Err(AdapterError::MissingTasty);
    }
    let tasty_version = text(tasty.get("version").unwrap_or(&serde_json::Value::Null));
    if tasty_version != SCALA_PIN {
        return Err(AdapterError::VersionMismatch {
            expected: SCALA_PIN.to_owned(),
            found: tasty_version,
        });
    }
    let symbols_json = tasty
        .get("symbols")
        .and_then(serde_json::Value::as_array)
        .ok_or(AdapterError::MissingTasty)?;
    if symbols_json.is_empty() {
        return Err(AdapterError::MissingTasty);
    }
    let mut symbols = Vec::new();
    for node in symbols_json {
        let qualified = text(node.get("fqname").unwrap_or(&serde_json::Value::Null));
        let kind = text(node.get("kind").unwrap_or(&serde_json::Value::Null));
        let doc = text(node.get("doc").unwrap_or(&serde_json::Value::Null));
        let file = text(node.get("file").unwrap_or(&serde_json::Value::Null));
        let line = node
            .get("line")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(1);
        symbols.push(make_symbol(SymbolSpec {
            language: "scala",
            package,
            qualified: &qualified,
            kind_name: &kind.to_lowercase(),
            signature: format!("{kind} {qualified}"),
            doc,
            file,
            line,
        })?);
    }
    shard("scala", package, symbols)
}
