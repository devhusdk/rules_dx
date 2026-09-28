use documentation_ir::proto::DocIr;

use crate::common::{make_symbol, shard, text, SymbolSpec};
use crate::types::AdapterError;
use crate::{KOTLIN_DOKKA_PIN, KOTLIN_PIN};

pub fn normalize_kotlin(input: &str, package: &str) -> Result<DocIr, AdapterError> {
    if input.trim().is_empty() {
        return Err(AdapterError::EmptyInput);
    }
    let value: serde_json::Value =
        serde_json::from_str(input).map_err(|err| AdapterError::InvalidJson(err.to_string()))?;
    let dokka = text(value.get("dokka").unwrap_or(&serde_json::Value::Null));
    if dokka != KOTLIN_DOKKA_PIN {
        return Err(AdapterError::VersionMismatch {
            expected: KOTLIN_DOKKA_PIN.to_owned(),
            found: dokka,
        });
    }
    let kotlin = text(value.get("kotlin").unwrap_or(&serde_json::Value::Null));
    if kotlin != KOTLIN_PIN {
        return Err(AdapterError::VersionMismatch {
            expected: KOTLIN_PIN.to_owned(),
            found: kotlin,
        });
    }
    let declarations = value
        .get("declarations")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| AdapterError::InvalidJson("missing declarations".to_owned()))?;
    let mut symbols = Vec::new();
    for declaration in declarations {
        let qualified = text(
            declaration
                .get("fqname")
                .unwrap_or(&serde_json::Value::Null),
        );
        let kind = text(declaration.get("kind").unwrap_or(&serde_json::Value::Null));
        let doc = text(declaration.get("doc").unwrap_or(&serde_json::Value::Null));
        let file = text(declaration.get("file").unwrap_or(&serde_json::Value::Null));
        let line = declaration
            .get("line")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(1);
        symbols.push(make_symbol(SymbolSpec {
            language: "kotlin",
            package,
            qualified: &qualified,
            kind_name: &kind.to_lowercase(),
            signature: format!("{kind} {qualified}"),
            doc,
            file,
            line,
        })?);
    }
    shard("kotlin", package, symbols)
}
