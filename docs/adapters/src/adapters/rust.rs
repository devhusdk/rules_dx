use documentation_ir::proto::{DocIr, Extension, Param, Relations, SourceRef, Visibility};

use crate::common::{kind_of, overload_id, shard, symbol_id, text};
use crate::types::AdapterError;
use crate::{RUST_FORMAT_VERSION, RUST_RUSTDOC_PIN};

pub fn normalize_rust(input: &str, package: &str) -> Result<DocIr, AdapterError> {
    if input.trim().is_empty() {
        return Err(AdapterError::EmptyInput);
    }
    let value: serde_json::Value =
        serde_json::from_str(input).map_err(|err| AdapterError::InvalidJson(err.to_string()))?;
    let found = value
        .get("format_version")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0)
        .to_string();
    if found != RUST_FORMAT_VERSION.to_string() {
        return Err(AdapterError::VersionMismatch {
            expected: RUST_FORMAT_VERSION.to_string(),
            found,
        });
    }
    let nightly = value
        .get("nightly")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    if nightly != RUST_RUSTDOC_PIN {
        return Err(AdapterError::VersionMismatch {
            expected: RUST_RUSTDOC_PIN.to_owned(),
            found: nightly.to_owned(),
        });
    }
    let index = value
        .get("index")
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| AdapterError::InvalidJson("missing index".to_owned()))?;
    let mut symbols = Vec::new();
    for item in index.values() {
        let name = text(item.get("name").unwrap_or(&serde_json::Value::Null));
        let kind = text(item.get("kind").unwrap_or(&serde_json::Value::Null));
        let docs = text(item.get("docs").unwrap_or(&serde_json::Value::Null));
        let file = text(
            item.get("span")
                .and_then(|span| span.get("filename"))
                .unwrap_or(&serde_json::Value::Null),
        );
        let line = item
            .get("span")
            .and_then(|span| span.get("line"))
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(1);
        if name.is_empty() {
            continue;
        }
        let params: Vec<String> = item
            .get("params")
            .and_then(serde_json::Value::as_array)
            .map(|items| items.iter().map(text).collect())
            .unwrap_or_default();
        let base = symbol_id("rust", package, &name)?;
        let id = if params.is_empty() {
            base.clone()
        } else {
            overload_id(&base, &params)
        };
        symbols.push(documentation_ir::proto::Symbol {
            id,
            kind: kind_of(&kind),
            signature_text: format!("{kind} {name}"),
            doc_markdown: docs,
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
            extensions: vec![Extension {
                key: "rustdoc.format_version".to_owned(),
                value: RUST_FORMAT_VERSION.to_string().into_bytes(),
            }],
        });
    }
    shard("rust", package, symbols)
}
