use documentation_ir::proto::{DocIr, Param, Relations, SourceRef, Symbol, Visibility};

use crate::common::{kind_of, overload_id, shard, symbol_id, text};
use crate::types::AdapterError;
use crate::TYPESCRIPT_TYPEDOC_PIN;

pub fn normalize_typescript(input: &str, package: &str) -> Result<DocIr, AdapterError> {
    if input.trim().is_empty() {
        return Err(AdapterError::EmptyInput);
    }
    let value: serde_json::Value =
        serde_json::from_str(input).map_err(|err| AdapterError::InvalidJson(err.to_string()))?;
    let found = text(value.get("typedoc").unwrap_or(&serde_json::Value::Null));
    if found != TYPESCRIPT_TYPEDOC_PIN {
        return Err(AdapterError::VersionMismatch {
            expected: TYPESCRIPT_TYPEDOC_PIN.to_owned(),
            found,
        });
    }
    let mut symbols = Vec::new();
    collect_typedoc("typescript", package, &value, &mut symbols)?;
    shard("typescript", package, symbols)
}

fn collect_typedoc(
    language: &str,
    package: &str,
    node: &serde_json::Value,
    out: &mut Vec<Symbol>,
) -> Result<(), AdapterError> {
    if let Some(children) = node.get("children").and_then(serde_json::Value::as_array) {
        for child in children {
            let name = text(child.get("name").unwrap_or(&serde_json::Value::Null));
            let kind_num = child
                .get("kind")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0);
            let kind = match kind_num {
                128 => "class",
                256 => "interface",
                512 => "method",
                1024 => "function",
                32 => "enum",
                _ => "function",
            };
            let doc = child
                .get("comment")
                .and_then(|comment| comment.get("summary"))
                .and_then(serde_json::Value::as_array)
                .map(|blocks| blocks.iter().map(text).collect::<Vec<_>>().join(""))
                .unwrap_or_default();
            let file = child
                .get("sources")
                .and_then(serde_json::Value::as_array)
                .and_then(|sources| sources.first())
                .and_then(|source| source.get("fileName"))
                .map(text)
                .unwrap_or_default();
            let line = child
                .get("sources")
                .and_then(serde_json::Value::as_array)
                .and_then(|sources| sources.first())
                .and_then(|source| source.get("line"))
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(1);
            if !name.is_empty() && child.get("flags").is_some() {
                let params: Vec<String> = child
                    .get("signatures")
                    .and_then(serde_json::Value::as_array)
                    .and_then(|sigs| sigs.first())
                    .and_then(|sig| sig.get("parameters"))
                    .and_then(serde_json::Value::as_array)
                    .map(|items| {
                        items
                            .iter()
                            .map(|param| {
                                param
                                    .get("type")
                                    .and_then(|ty| ty.get("name"))
                                    .map(text)
                                    .unwrap_or_default()
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                let base = symbol_id(language, package, &name)?;
                let id = if params.is_empty() {
                    base
                } else {
                    overload_id(&base, &params)
                };
                out.push(Symbol {
                    id,
                    kind: kind_of(kind),
                    signature_text: format!("{kind} {name}"),
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
            }
            collect_typedoc(language, package, child, out)?;
        }
    }
    Ok(())
}
