use std::collections::BTreeMap;
use std::path::Path;

pub const VERIFY_TOML_REL: &str = "dx.verify.toml";

pub const VERIFY_SCHEMA_VERSION: u32 = 1;

pub const VERIFY_STEP_COMMANDS: [&str; 7] = [
    "format",
    "lint",
    "typecheck",
    "generate",
    "build",
    "test",
    "coverage",
];

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum VerifySetError {
    #[error("no {file} in {workspace}: commit one with schema = 1 and [sets.<name>]")]
    MissingFile { file: String, workspace: String },
    #[error("read {file}: {detail}")]
    Unreadable { file: String, detail: String },
    #[error("parse {file}: {detail}")]
    Malformed { file: String, detail: String },
    #[error("{file} schema is {found}, want {wanted}")]
    UnsupportedSchema {
        file: String,
        found: u32,
        wanted: u32,
    },
    #[error("{file} has no set {name:?} (sets: {known})")]
    UnknownSet {
        file: String,
        name: String,
        known: String,
    },
    #[error("{file} set {name:?} lists no steps")]
    EmptySteps { file: String, name: String },
    #[error("{file} set {set:?} step {index} names unknown command {command:?} (want one of format, lint, typecheck, generate, build, test, coverage)")]
    UnknownStepCommand {
        file: String,
        set: String,
        index: usize,
        command: String,
    },
    #[error("{file} set {set:?} step {index} names rejected scope {scope:?}")]
    BadStepScope {
        file: String,
        set: String,
        index: usize,
        scope: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct VerifyFile {
    schema: u32,
    sets: BTreeMap<String, VerifySetFile>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct VerifySetFile {
    steps: Vec<VerifyStepFile>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct VerifyStepFile {
    command: String,
    #[serde(default)]
    scopes: Vec<String>,
    #[serde(default)]
    bazel_options: Vec<String>,
    #[serde(default)]
    required: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedStep {
    pub command: String,
    pub scopes: Vec<String>,
    pub bazel_options: Vec<String>,
    pub required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedSet {
    pub name: String,
    pub steps: Vec<ResolvedStep>,
}

fn known_sets(sets: &BTreeMap<String, VerifySetFile>) -> String {
    let mut names: Vec<&str> = sets.keys().map(String::as_str).collect();
    names.sort_unstable();
    if names.is_empty() {
        return "<none>".to_owned();
    }
    names.join(", ")
}

pub fn load_verify_set(workspace: &Path, name: &str) -> Result<ResolvedSet, VerifySetError> {
    let path = workspace.join(VERIFY_TOML_REL);
    let text = std::fs::read_to_string(&path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            VerifySetError::MissingFile {
                file: VERIFY_TOML_REL.to_owned(),
                workspace: workspace.display().to_string(),
            }
        } else {
            VerifySetError::Unreadable {
                file: VERIFY_TOML_REL.to_owned(),
                detail: error.to_string(),
            }
        }
    })?;
    let file: VerifyFile = toml::from_str(&text).map_err(|error| VerifySetError::Malformed {
        file: VERIFY_TOML_REL.to_owned(),
        detail: error.to_string(),
    })?;
    if file.schema != VERIFY_SCHEMA_VERSION {
        return Err(VerifySetError::UnsupportedSchema {
            file: VERIFY_TOML_REL.to_owned(),
            found: file.schema,
            wanted: VERIFY_SCHEMA_VERSION,
        });
    }
    let set = file
        .sets
        .get(name)
        .ok_or_else(|| VerifySetError::UnknownSet {
            file: VERIFY_TOML_REL.to_owned(),
            name: name.to_owned(),
            known: known_sets(&file.sets),
        })?;
    if set.steps.is_empty() {
        return Err(VerifySetError::EmptySteps {
            file: VERIFY_TOML_REL.to_owned(),
            name: name.to_owned(),
        });
    }
    let mut steps = Vec::with_capacity(set.steps.len());
    for (index, step) in set.steps.iter().enumerate() {
        if !VERIFY_STEP_COMMANDS.contains(&step.command.as_str()) {
            return Err(VerifySetError::UnknownStepCommand {
                file: VERIFY_TOML_REL.to_owned(),
                set: name.to_owned(),
                index,
                command: step.command.clone(),
            });
        }
        for scope in &step.scopes {
            if scope.is_empty() || scope.starts_with(':') {
                return Err(VerifySetError::BadStepScope {
                    file: VERIFY_TOML_REL.to_owned(),
                    set: name.to_owned(),
                    index,
                    scope: scope.clone(),
                });
            }
        }
        steps.push(ResolvedStep {
            command: step.command.clone(),
            scopes: step.scopes.clone(),
            bazel_options: step.bazel_options.clone(),
            required: step.required.unwrap_or(true),
        });
    }
    Ok(ResolvedSet {
        name: name.to_owned(),
        steps,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> dx_test_scratch::TempDir {
        dx_test_scratch::scratch(&format!("dx-verify-sets-{name}-"))
    }

    fn write(dir: &dx_test_scratch::TempDir, text: &str) {
        std::fs::write(dir.path().join(VERIFY_TOML_REL), text).expect("fixture");
    }

    const VALID: &str = r#"
schema = 1

[sets.pre-pr]
steps = [
  { command = "format", scopes = ["//cli/..."] },
  { command = "lint", scopes = ["//cli/..."], bazel_options = ["--jobs=4"] },
  { command = "test", required = false },
]
"#;

    #[test]
    fn valid_set_resolves_with_defaults() {
        let dir = scratch("valid");
        write(&dir, VALID);
        let set = load_verify_set(dir.path(), "pre-pr").expect("resolves");
        assert_eq!(set.name, "pre-pr");
        assert_eq!(set.steps.len(), 3);
        assert_eq!(set.steps[0].command, "format");
        assert_eq!(set.steps[0].scopes, vec!["//cli/...".to_owned()]);
        assert!(set.steps[0].bazel_options.is_empty());
        assert!(set.steps[0].required);
        assert_eq!(set.steps[1].bazel_options, vec!["--jobs=4".to_owned()]);
        assert!(set.steps[1].required);
        assert!(set.steps[2].scopes.is_empty());
        assert!(!set.steps[2].required);
    }

    #[test]
    fn missing_file_names_the_workspace() {
        let dir = scratch("missing");
        let error = load_verify_set(dir.path(), "pre-pr").expect_err("missing");
        assert!(
            matches!(error, VerifySetError::MissingFile { .. }),
            "{error}"
        );
        assert!(error.to_string().contains(VERIFY_TOML_REL));
        assert!(error
            .to_string()
            .contains(&dir.path().display().to_string()));
    }

    #[test]
    fn malformed_toml_never_parses() {
        let dir = scratch("malformed");
        write(&dir, "schema = [broken");
        let error = load_verify_set(dir.path(), "pre-pr").expect_err("malformed");
        assert!(matches!(error, VerifySetError::Malformed { .. }), "{error}");
    }

    #[test]
    fn wrong_schema_and_unknown_keys_fail_closed() {
        let dir = scratch("schema");
        write(&dir, "schema = 2\n\n[sets.a]\nsteps = []\n");
        let error = load_verify_set(dir.path(), "a").expect_err("schema");
        assert!(
            matches!(error, VerifySetError::UnsupportedSchema { found: 2, .. }),
            "{error}"
        );
        let dir = scratch("keys");
        write(
            &dir,
            "schema = 1\ntop = true\n\n[sets.a]\nsteps = [{ command = \"build\" }]\n",
        );
        let error = load_verify_set(dir.path(), "a").expect_err("keys");
        assert!(matches!(error, VerifySetError::Malformed { .. }), "{error}");
        let dir = scratch("nested");
        write(
            &dir,
            "schema = 1\n\n[sets.a]\nsteps = [{ command = \"build\", set = \"other\" }]\n",
        );
        let error = load_verify_set(dir.path(), "a").expect_err("nested");
        assert!(
            matches!(error, VerifySetError::Malformed { .. }),
            "nested sets stay rejected: {error}"
        );
    }

    #[test]
    fn unknown_set_lists_the_known_sets() {
        let dir = scratch("unknown");
        write(&dir, VALID);
        let error = load_verify_set(dir.path(), "other").expect_err("unknown");
        assert!(
            matches!(error, VerifySetError::UnknownSet { .. }),
            "{error}"
        );
        assert!(error.to_string().contains("pre-pr"));
    }

    #[test]
    fn empty_steps_and_unknown_commands_fail_closed() {
        let dir = scratch("empty");
        write(&dir, "schema = 1\n\n[sets.a]\nsteps = []\n");
        let error = load_verify_set(dir.path(), "a").expect_err("empty");
        assert!(
            matches!(error, VerifySetError::EmptySteps { .. }),
            "{error}"
        );
        let dir = scratch("command");
        write(
            &dir,
            "schema = 1\n\n[sets.a]\nsteps = [{ command = \"deploy\" }]\n",
        );
        let error = load_verify_set(dir.path(), "a").expect_err("command");
        assert!(
            matches!(error, VerifySetError::UnknownStepCommand { index: 0, .. }),
            "{error}"
        );
        assert!(error.to_string().contains("deploy"));
    }

    #[test]
    fn rejected_scopes_fail_at_load() {
        for scope in ["", ":target"] {
            let dir = scratch("scope");
            write(
                &dir,
                &format!("schema = 1\n\n[sets.a]\nsteps = [{{ command = \"build\", scopes = [\"{scope}\"] }}]\n"),
            );
            let error = load_verify_set(dir.path(), "a").expect_err("scope");
            assert!(
                matches!(error, VerifySetError::BadStepScope { .. }),
                "{scope:?}: {error}"
            );
        }
    }
}
