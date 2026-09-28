use documentation_ir::proto::{DocIr, Param, Relations, SourceRef, Symbol, Visibility};

use crate::common::{kind_of, overload_id, shard, symbol_id, text};
use crate::types::AdapterError;
use crate::PYTHON_GRIFFE_PIN;

pub fn normalize_python(input: &str, package: &str) -> Result<DocIr, AdapterError> {
    if input.trim().is_empty() {
        return Err(AdapterError::EmptyInput);
    }
    let value: serde_json::Value =
        serde_json::from_str(input).map_err(|err| AdapterError::InvalidJson(err.to_string()))?;
    let found = text(
        value
            .get("griffe_version")
            .unwrap_or(&serde_json::Value::Null),
    );
    if found != PYTHON_GRIFFE_PIN {
        return Err(AdapterError::VersionMismatch {
            expected: PYTHON_GRIFFE_PIN.to_owned(),
            found,
        });
    }
    let mut symbols = Vec::new();
    let members = value
        .get("members")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| AdapterError::InvalidJson("missing members".to_owned()))?;
    for member in members {
        collect_griffe("python", package, member, &mut symbols)?;
    }
    shard("python", package, symbols)
}

fn collect_griffe(
    language: &str,
    package: &str,
    node: &serde_json::Value,
    out: &mut Vec<Symbol>,
) -> Result<(), AdapterError> {
    let kind = text(node.get("kind").unwrap_or(&serde_json::Value::Null));
    let name = text(node.get("name").unwrap_or(&serde_json::Value::Null));
    let path = text(node.get("path").unwrap_or(&serde_json::Value::Null));
    let qualified = if path.is_empty() { name.clone() } else { path };
    if qualified.is_empty() {
        return Err(AdapterError::InvalidJson("member without path".to_owned()));
    }
    let doc = text(node.get("docstring").unwrap_or(&serde_json::Value::Null));
    let file = text(node.get("file").unwrap_or(&serde_json::Value::Null));
    let line = node
        .get("lineno")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(1);
    let params: Vec<String> = node
        .get("parameters")
        .and_then(serde_json::Value::as_array)
        .map(|items| {
            items
                .iter()
                .map(|param| text(param.get("type").unwrap_or(&serde_json::Value::Null)))
                .collect()
        })
        .unwrap_or_default();
    let base = symbol_id(language, package, &qualified)?;
    let id = if params.is_empty() {
        base.clone()
    } else {
        overload_id(&base, &params)
    };
    out.push(Symbol {
        id,
        kind: kind_of(&kind),
        signature_text: format!("{kind} {qualified}"),
        doc_markdown: doc,
        params: params
            .iter()
            .map(|ty| Param {
                name: String::new(),
                r#type: ty.clone(),
                doc: String::new(),
            })
            .collect(),
        returns: None,
        examples: vec![],
        source: Some(SourceRef { file, line }),
        visibility: Visibility::Public as i32,
        relations: Some(Relations {
            member_of: vec![],
            implements: vec![],
        }),
        extensions: vec![],
    });
    if let Some(children) = node.get("members").and_then(serde_json::Value::as_array) {
        for child in children {
            collect_griffe(language, package, child, out)?;
        }
    }
    Ok(())
}
