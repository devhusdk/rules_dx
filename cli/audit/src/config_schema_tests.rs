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

#[test]
fn schema_version_is_required_and_extends_is_unknown() {
    for schema in [SECURITY_SCHEMA, LICENSES_SCHEMA] {
        let schema = validator(schema);
        assert!(schema.is_valid(&toml_value("schema_version = 1")));
        assert!(!schema.is_valid(&toml_value("")));
        assert!(!schema.is_valid(&toml_value("schema_version = 1\nextends = \"base.toml\"")));
    }
}

const CONFIG_DOC: &str = include_str!("../../../docs/cli/commands/config.md");

fn table_keys(file: &str, table: &str) -> Vec<String> {
    let heading = format!("### {file} {table}");
    let section = CONFIG_DOC
        .split("\n### ")
        .map(|part| format!("### {part}"))
        .find(|part| part.starts_with(&heading) && part[heading.len()..].starts_with('\n'))
        .unwrap_or_else(|| panic!("config.md has no section {heading:?}"));
    section
        .lines()
        .filter_map(|line| line.strip_prefix("| `"))
        .filter_map(|row| row.split('`').next())
        .map(str::to_owned)
        .collect()
}

fn assert_listed(file: &str, table: &str, keys: Vec<&'static str>) {
    let listed = table_keys(file, table);
    for key in keys {
        assert!(
            listed.iter().any(|name| name == key),
            "config.md: {file} {table} does not list `{key}`"
        );
    }
}

#[test]
fn config_page_lists_every_security_key() {
    use crate::exception::{RiskException, SecurityFile, UncoveredAcceptance};
    assert_listed("security.toml", "top level", SecurityFile::key_names());
    assert_listed(
        "security.toml",
        "`[[exception]]`",
        RiskException::key_names(),
    );
    assert_listed(
        "security.toml",
        "`[[uncovered]]`",
        UncoveredAcceptance::key_names(),
    );
}

#[test]
fn config_page_lists_every_license_key() {
    use crate::license_policy::{
        Distribution, LicenseException, LicenseInventory, LicensesFile, PolicyFile, PolicyLists,
        SetReview,
    };
    let file = "licenses.toml";
    assert_listed(file, "top level", LicensesFile::key_names());
    assert_listed(file, "`[policy]`", PolicyFile::key_names());
    assert_listed(file, "`[policy.distributed]`", PolicyLists::key_names());
    assert_listed(file, "`[policy.sets.<set>]`", SetReview::key_names());
    assert_listed(file, "`[distribution]`", Distribution::key_names());
    assert_listed(file, "`[[exception]]`", LicenseException::key_names());
    assert_listed(file, "`[[inventory]]`", LicenseInventory::key_names());
}

#[test]
fn config_page_check_catches_a_missing_key() {
    let listed = table_keys("security.toml", "`[[uncovered]]`");
    assert!(!listed.iter().any(|name| name == "not_a_key"));
}
