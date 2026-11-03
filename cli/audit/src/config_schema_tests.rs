use super::{licenses_schema, security_schema};

const SECURITY_SCHEMA: &str = include_str!("../schema/security.schema.json");
const LICENSES_SCHEMA: &str = include_str!("../schema/licenses.schema.json");
const SECURITY_TOML: &str = include_str!("../../../security.toml");
const LICENSES_TOML: &str = include_str!("../../../licenses.toml");

fn validator(schema: &str) -> jsonschema::Validator {
    jsonschema::validator_for(&serde_json::from_str(schema).unwrap()).unwrap()
}

fn toml_value(text: &str) -> serde_json::Value {
    toml::from_str(text).unwrap()
}

#[test]
fn checked_in_schemas_are_current() {
    assert_eq!(
        security_schema(),
        SECURITY_SCHEMA,
        "run `bazel run //cli/audit:config_schemas`"
    );
    assert_eq!(
        licenses_schema(),
        LICENSES_SCHEMA,
        "run `bazel run //cli/audit:config_schemas`"
    );
}

#[test]
fn repo_files_match_their_schemas() {
    let security = validator(SECURITY_SCHEMA);
    assert!(security.is_valid(&toml_value(SECURITY_TOML)));
    let licenses = validator(LICENSES_SCHEMA);
    assert!(licenses.is_valid(&toml_value(LICENSES_TOML)));
}

#[test]
fn typo_key_is_rejected() {
    let security = validator(SECURITY_SCHEMA);
    assert!(!security.is_valid(&toml_value("schema_versoin = 1")));
    let licenses = validator(LICENSES_SCHEMA);
    assert!(!licenses.is_valid(&toml_value("[polcy]\nblocked = []")));
}
