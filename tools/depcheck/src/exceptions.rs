use std::collections::BTreeMap;
use std::path::Path;

use crate::ecosystem::{cc, dotnet, go, js, jvm, python, ruby};
use crate::{DepcheckError, Ecosystem, Exception};

pub fn parse_exceptions(path: Option<&Path>) -> Result<BTreeMap<String, Exception>, DepcheckError> {
    let Some(path) = path else {
        return Ok(BTreeMap::new());
    };
    if !path.exists() {
        return Err(DepcheckError::ExceptionsMissing(path.display().to_string()));
    }
    let text = std::fs::read_to_string(path).map_err(DepcheckError::ExceptionsIo)?;
    let data: toml::Value = toml::from_str(&text).map_err(DepcheckError::ExceptionsToml)?;
    let mut out = BTreeMap::new();
    let items = data
        .get("exception")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    for item in items {
        let name = item
            .get("dependency")
            .and_then(crate::toml_util::toml_string)
            .unwrap_or_default()
            .trim()
            .to_owned();
        let reason = item
            .get("reason")
            .and_then(crate::toml_util::toml_string)
            .unwrap_or_default()
            .trim()
            .to_owned();
        if name.is_empty() {
            return Err(DepcheckError::ExceptionEntry);
        }
        out.insert(
            name.to_lowercase().replace('-', "_"),
            Exception { raw: name, reason },
        );
    }
    Ok(out)
}

pub(crate) fn normalize_exception_key(eco: Ecosystem, raw: &str) -> String {
    match eco {
        Ecosystem::Python => python::normalize_py(raw),
        Ecosystem::Rust => raw.to_lowercase(),
        Ecosystem::Go => go::normalize_go(raw),
        Ecosystem::Java | Ecosystem::Kotlin | Ecosystem::Scala => jvm::normalize_jvm(raw),
        Ecosystem::Csharp | Ecosystem::Fsharp => dotnet::normalize_dotnet(raw),
        Ecosystem::Cc => cc::normalize_cc(raw),
        Ecosystem::Js | Ecosystem::Ts => js::normalize_js(raw),
        Ecosystem::Ruby => ruby::normalize_ruby(raw),
    }
}
