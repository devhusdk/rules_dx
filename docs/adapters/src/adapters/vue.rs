use documentation_ir::proto::DocIr;

use crate::common::{make_symbol, shard, text, SymbolSpec};
use crate::types::AdapterError;
use crate::VUE_DOCGEN_PIN;

pub fn normalize_vue(input: &str, package: &str) -> Result<DocIr, AdapterError> {
    if input.trim().is_empty() {
        return Err(AdapterError::EmptyInput);
    }
    let value: serde_json::Value =
        serde_json::from_str(input).map_err(|err| AdapterError::InvalidJson(err.to_string()))?;
    let found = text(value.get("docgen").unwrap_or(&serde_json::Value::Null));
    if found != VUE_DOCGEN_PIN {
        return Err(AdapterError::VersionMismatch {
            expected: VUE_DOCGEN_PIN.to_owned(),
            found,
        });
    }
    let components = value
        .get("components")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| AdapterError::InvalidJson("missing components".to_owned()))?;
    let mut symbols = Vec::new();
    for component in components {
        for key in ["props", "events", "slots", "methods"] {
            if let Some(node) = component.get(key) {
                if !node.is_array() {
                    return Err(AdapterError::InvalidJson(format!("{key} must be an array")));
                }
            }
        }
        let export = text(
            component
                .get("exportName")
                .unwrap_or(&serde_json::Value::Null),
        );
        let description = text(
            component
                .get("description")
                .unwrap_or(&serde_json::Value::Null),
        );
        let file = text(component.get("file").unwrap_or(&serde_json::Value::Null));
        symbols.push(make_symbol(SymbolSpec {
            language: "vue",
            package,
            qualified: &export,
            kind_name: "class",
            signature: format!("component {export}"),
            doc: description,
            file,
            line: 1,
        })?);
        for prop in component
            .get("props")
            .and_then(serde_json::Value::as_array)
            .cloned()
            .unwrap_or_default()
        {
            let name = text(prop.get("name").unwrap_or(&serde_json::Value::Null));
            let doc = text(prop.get("description").unwrap_or(&serde_json::Value::Null));
            symbols.push(make_symbol(SymbolSpec {
                language: "vue",
                package,
                qualified: &format!("{export}.{name}"),
                kind_name: "property",
                signature: format!("prop {name}"),
                doc,
                file: String::new(),
                line: 1,
            })?);
        }
    }
    shard("vue", package, symbols)
}
