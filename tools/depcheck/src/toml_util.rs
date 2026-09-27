pub(crate) fn toml_string(value: &toml::Value) -> Option<String> {
    match value {
        toml::Value::String(s) => Some(s.clone()),
        toml::Value::Integer(i) => Some(i.to_string()),
        toml::Value::Float(f) => Some(f.to_string()),
        toml::Value::Boolean(b) => Some(b.to_string()),
        _ => None,
    }
}

pub(crate) fn toml_bool(value: &toml::Value) -> bool {
    value.as_bool().unwrap_or(false)
}

pub(crate) fn parse_toml_file(path: &std::path::Path) -> Result<toml::Value, crate::DepcheckError> {
    let text = std::fs::read_to_string(path).map_err(crate::DepcheckError::ManifestIo)?;
    toml::from_str(&text).map_err(crate::DepcheckError::ManifestToml)
}
