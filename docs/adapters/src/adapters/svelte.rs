use documentation_ir::proto::DocIr;

use crate::common::{make_symbol, shard, text, SymbolSpec};
use crate::types::AdapterError;
use crate::SVELTE_SVELD_PIN;

pub fn normalize_svelte(input: &str, package: &str) -> Result<DocIr, AdapterError> {
    if input.trim().is_empty() {
        return Err(AdapterError::EmptyInput);
    }
    let value: serde_json::Value =
        serde_json::from_str(input).map_err(|err| AdapterError::InvalidJson(err.to_string()))?;
    let found = text(value.get("sveld").unwrap_or(&serde_json::Value::Null));
    if found != SVELTE_SVELD_PIN {
        return Err(AdapterError::VersionMismatch {
            expected: SVELTE_SVELD_PIN.to_owned(),
            found,
        });
    }
    let components = value
        .get("components")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| AdapterError::InvalidJson("missing components".to_owned()))?;
    let mut symbols = Vec::new();
    for component in components {
        let name = text(component.get("name").unwrap_or(&serde_json::Value::Null));
        let doc = text(
            component
                .get("description")
                .unwrap_or(&serde_json::Value::Null),
        );
        let file = text(component.get("file").unwrap_or(&serde_json::Value::Null));
        symbols.push(make_symbol(SymbolSpec {
            language: "svelte",
            package,
            qualified: &name,
            kind_name: "class",
            signature: format!("component {name}"),
            doc,
            file,
            line: 1,
        })?);
        for prop in component
            .get("props")
            .and_then(serde_json::Value::as_array)
            .cloned()
            .unwrap_or_default()
        {
            let prop_name = text(prop.get("name").unwrap_or(&serde_json::Value::Null));
            let prop_doc = text(prop.get("description").unwrap_or(&serde_json::Value::Null));
            symbols.push(make_symbol(SymbolSpec {
                language: "svelte",
                package,
                qualified: &format!("{name}.{prop_name}"),
                kind_name: "property",
                signature: format!("prop {prop_name}"),
                doc: prop_doc,
                file: String::new(),
                line: 1,
            })?);
        }
    }
    shard("svelte", package, symbols)
}
