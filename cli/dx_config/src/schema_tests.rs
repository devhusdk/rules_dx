use crate::schema::dx_schema;

const SCHEMA: &str = include_str!("../schema/dx.schema.json");

fn validator() -> jsonschema::Validator {
    jsonschema::validator_for(&serde_json::from_str(SCHEMA).unwrap()).unwrap()
}

fn toml_value(text: &str) -> serde_json::Value {
    toml::from_str(text).unwrap()
}

#[test]
fn checked_in_schema_is_current() {
    assert_eq!(
        dx_schema(),
        SCHEMA,
        "run `bazel run //cli/dx_config:update_schema`"
    );
}

#[test]
fn schema_accepts_a_config_and_rejects_typos() {
    let schema = validator();
    assert!(schema.is_valid(&toml_value(
        "schema_version = 1\ntier = \"strict\"\n[families.go]\nlint = [\"typos\"]\n"
    )));
    assert!(!schema.is_valid(&toml_value("schema_version = 1\nprofle = \"strict\"")));
    assert!(!schema.is_valid(&toml_value("schema_version = 1\nprofile = \"loose\"")));
}
