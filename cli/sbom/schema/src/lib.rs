//! Test-only SPDX 2.3 checks: the vendored official JSON schema plus the dx_sbom graph rules.

#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

use serde_json::Value;

/// The SPDX 2.3 JSON schema (draft-07), vendored from spdx/spdx-spec at tag v2.3.
pub const SPDX_2_3_SCHEMA: &str = include_str!("../spdx-schema.json");

/// Returns every SPDX 2.3 JSON schema violation in `instance`, as `<path>: <message>` lines.
pub fn schema_errors(instance: &Value) -> Vec<String> {
    let schema: Value = match serde_json::from_str(SPDX_2_3_SCHEMA) {
        Ok(schema) => schema,
        Err(error) => return vec![format!("vendored SPDX schema is not JSON: {error}")],
    };
    let validator = match jsonschema::draft7::new(&schema) {
        Ok(validator) => validator,
        Err(error) => return vec![format!("vendored SPDX schema does not compile: {error}")],
    };
    validator
        .iter_errors(instance)
        .map(|error| format!("{}: {error}", error.instance_path()))
        .collect()
}

/// Checks SPDX JSON text against the 2.3 schema, then against [`dx_sbom::reference_errors`].
pub fn spdx_errors(text: &str) -> Vec<String> {
    let value: Value = match serde_json::from_str(text) {
        Ok(value) => value,
        Err(error) => return vec![format!("SPDX text is not JSON: {error}")],
    };
    let mut errors = schema_errors(&value);
    match serde_json::from_value::<dx_sbom::SpdxDocument>(value) {
        Ok(document) => errors.extend(dx_sbom::reference_errors(&document)),
        Err(error) => errors.push(format!("SPDX text is not a dx_sbom document: {error}")),
    }
    errors
}

#[cfg(test)]
mod tests;
