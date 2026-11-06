//! Test-only SBOM checks: the vendored official SPDX 2.3 and CycloneDX 1.6 JSON schemas plus the
//! dx_sbom graph rules.

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

/// The CycloneDX 1.6 JSON schema (draft-07) and the two files it references, vendored from
/// CycloneDX/specification at tag 1.6.
pub const CYCLONEDX_1_6_SCHEMA: &str = include_str!("../bom-1.6.schema.json");

const CYCLONEDX_SPDX_SCHEMA: &str = include_str!("../spdx.schema.json");

const CYCLONEDX_JSF_SCHEMA: &str = include_str!("../jsf-0.82.schema.json");

/// Returns every CycloneDX 1.6 JSON schema violation in `instance`, as `<path>: <message>` lines.
pub fn cyclonedx_schema_errors(instance: &Value) -> Vec<String> {
    let parse = |text: &str| serde_json::from_str::<Value>(text).map_err(|error| error.to_string());
    let (bom, spdx, jsf) = match (
        parse(CYCLONEDX_1_6_SCHEMA),
        parse(CYCLONEDX_SPDX_SCHEMA),
        parse(CYCLONEDX_JSF_SCHEMA),
    ) {
        (Ok(bom), Ok(spdx), Ok(jsf)) => (bom, spdx, jsf),
        (Err(error), _, _) | (_, Err(error), _) | (_, _, Err(error)) => {
            return vec![format!("vendored CycloneDX schema is not JSON: {error}")];
        }
    };
    let registry = jsonschema::Registry::new()
        .add("http://cyclonedx.org/schema/spdx.schema.json", spdx)
        .and_then(|builder| builder.add("http://cyclonedx.org/schema/jsf-0.82.schema.json", jsf))
        .and_then(|builder| builder.prepare());
    let registry = match registry {
        Ok(registry) => registry,
        Err(error) => return vec![format!("vendored CycloneDX schema registry: {error}")],
    };
    let validator = match jsonschema::draft7::options()
        .with_registry(&registry)
        .build(&bom)
    {
        Ok(validator) => validator,
        Err(error) => {
            return vec![format!(
                "vendored CycloneDX schema does not compile: {error}"
            )];
        }
    };
    validator
        .iter_errors(instance)
        .map(|error| format!("{}: {error}", error.instance_path()))
        .collect()
}

/// Checks CycloneDX JSON text against the 1.6 schema, then against
/// [`dx_sbom::cyclonedx::reference_errors`].
pub fn cyclonedx_errors(text: &str) -> Vec<String> {
    let value: Value = match serde_json::from_str(text) {
        Ok(value) => value,
        Err(error) => return vec![format!("CycloneDX text is not JSON: {error}")],
    };
    let mut errors = cyclonedx_schema_errors(&value);
    match serde_json::from_value::<dx_sbom::cyclonedx::CycloneDxDocument>(value) {
        Ok(document) => errors.extend(dx_sbom::cyclonedx::reference_errors(&document)),
        Err(error) => errors.push(format!("CycloneDX text is not a dx_sbom document: {error}")),
    }
    errors
}

#[cfg(test)]
mod tests;
