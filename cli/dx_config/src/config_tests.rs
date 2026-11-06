use crate::config::{LoadProblem, Profile, load, parse};
use crate::conventions::ConventionProblem;

fn message(text: &str) -> String {
    parse(text).unwrap_err().to_string()
}

#[test]
fn a_full_file_parses() {
    let config = parse(
        r#"
schema_version = 1
tier = "strict"

[families.rust]
tier = "standard"
lint = ["clippy"]
disabled = false

[scope]
exclude = ["//third_party/..."]
"#,
    )
    .unwrap();
    assert_eq!(config.profile, Some(Profile::Strict));
    assert_eq!(config.families["rust"].profile, Some(Profile::Standard));
    assert_eq!(config.scope.exclude, ["//third_party/..."]);
}

#[test]
fn schema_version_is_required() {
    assert_eq!(
        parse("tier = \"strict\"").unwrap_err(),
        LoadProblem::Convention(ConventionProblem::MissingSchemaVersion)
    );
    assert!(message("schema_version = 9").contains("newer"));
    assert!(message("schema_version = \"one\"").contains("whole number"));
}

#[test]
fn unknown_keys_are_errors() {
    assert!(message("schema_version = 1\ncolour = 1").contains("unknown field `colour`"));
    assert!(
        message("schema_version = 1\n[families.go]\nlnit = []").contains("unknown field `lnit`")
    );
    assert!(message("schema_version = 1\n[scope]\nskip = []").contains("unknown field `skip`"));
}

#[test]
fn paths_are_not_supported_yet() {
    assert_eq!(
        parse("schema_version = 1\n[[paths]]\nglob = \"a\"").unwrap_err(),
        LoadProblem::PathsNotSupported
    );
    assert!(message("schema_version = 1\n[[paths]]").contains("not supported yet"));
}

#[test]
fn bad_profile_and_pattern_are_errors() {
    assert!(message("schema_version = 1\ntier = \"loose\"").contains("loose"));
    assert!(
        message("schema_version = 1\n[scope]\nexclude = [\"third_party\"]")
            .contains("invalid label pattern")
    );
}

#[test]
fn a_missing_file_is_the_standard_tier() {
    let dir = tempfile_dir();
    assert_eq!(load(&dir).unwrap(), None);
    std::fs::write(dir.join("dx.toml"), "schema_version = 1\n").unwrap();
    assert_eq!(load(&dir).unwrap().unwrap().schema_version, 1);
    std::fs::remove_dir_all(&dir).unwrap();
}

fn tempfile_dir() -> std::path::PathBuf {
    let base = std::env::var_os("TEST_TMPDIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let dir = base.join(format!("dx_config_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn large_files_allow_parses_and_rejects_unknown_keys() {
    let config = parse("schema_version = 1\n[large_files]\nallow = [\"data/\"]\n").unwrap();
    assert_eq!(config.large_files.allow, ["data/"]);
    assert!(parse("schema_version = 1\n[large_files]\nallw = []\n").is_err());
}
