use documentation_ir::proto::DocIr;

use crate::common::{make_symbol, shard, text, SymbolSpec};
use crate::types::AdapterError;
use crate::{GO_TOOLCHAIN_PIN, GO_XTOOLS_PIN};

pub fn normalize_go(input: &str, package: &str) -> Result<DocIr, AdapterError> {
    if input.trim().is_empty() {
        return Err(AdapterError::EmptyInput);
    }
    let value: serde_json::Value =
        serde_json::from_str(input).map_err(|err| AdapterError::InvalidJson(err.to_string()))?;
    let toolchain = text(value.get("go").unwrap_or(&serde_json::Value::Null));
    if toolchain != GO_TOOLCHAIN_PIN {
        return Err(AdapterError::VersionMismatch {
            expected: GO_TOOLCHAIN_PIN.to_owned(),
            found: toolchain,
        });
    }
    let xtools = text(value.get("xtools").unwrap_or(&serde_json::Value::Null));
    if xtools != GO_XTOOLS_PIN {
        return Err(AdapterError::VersionMismatch {
            expected: GO_XTOOLS_PIN.to_owned(),
            found: xtools,
        });
    }
    let funcs = value
        .get("funcs")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| AdapterError::InvalidJson("missing funcs".to_owned()))?;
    let mut symbols = Vec::new();
    for func in funcs {
        let name = text(func.get("name").unwrap_or(&serde_json::Value::Null));
        let doc = text(func.get("doc").unwrap_or(&serde_json::Value::Null));
        let pos = text(func.get("pos").unwrap_or(&serde_json::Value::Null));
        let (file, line) = split_pos(&pos);
        symbols.push(make_symbol(SymbolSpec {
            language: "go",
            package,
            qualified: &name,
            kind_name: "function",
            signature: format!("func {name}"),
            doc,
            file,
            line,
        })?);
    }
    shard("go", package, symbols)
}

pub(crate) fn split_pos(pos: &str) -> (String, u64) {
    match pos.rsplit_once(':') {
        Some((file, line)) => (file.to_owned(), line.parse().unwrap_or(1)),
        None => (pos.to_owned(), 1),
    }
}
