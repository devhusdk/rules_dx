use documentation_ir::proto::DocIr;

use crate::common::{make_symbol, shard, text, SymbolSpec};
use crate::types::AdapterError;
use crate::JAVA_JDK_PIN;

pub fn normalize_java(input: &str, package: &str) -> Result<DocIr, AdapterError> {
    if input.trim().is_empty() {
        return Err(AdapterError::EmptyInput);
    }
    let value: serde_json::Value =
        serde_json::from_str(input).map_err(|err| AdapterError::InvalidJson(err.to_string()))?;
    let found = text(value.get("jdk").unwrap_or(&serde_json::Value::Null));
    if found != JAVA_JDK_PIN {
        return Err(AdapterError::VersionMismatch {
            expected: JAVA_JDK_PIN.to_owned(),
            found,
        });
    }
    let elements = value
        .get("elements")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| AdapterError::InvalidJson("missing elements".to_owned()))?;
    let mut symbols = Vec::new();
    for element in elements {
        let qualified = text(
            element
                .get("qualifiedName")
                .unwrap_or(&serde_json::Value::Null),
        );
        let kind = text(element.get("kind").unwrap_or(&serde_json::Value::Null));
        let doc = text(element.get("doc").unwrap_or(&serde_json::Value::Null));
        let file = text(element.get("file").unwrap_or(&serde_json::Value::Null));
        let line = element
            .get("line")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(1);
        symbols.push(make_symbol(SymbolSpec {
            language: "java",
            package,
            qualified: &qualified,
            kind_name: &kind.to_lowercase(),
            signature: format!("{kind} {qualified}"),
            doc,
            file,
            line,
        })?);
    }
    shard("java", package, symbols)
}
