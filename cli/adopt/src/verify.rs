use std::collections::BTreeMap;
use std::path::Path;

pub const VERIFY_TOML_REL: &str = "dx.verify.toml";

pub const VERIFY_SCHEMA: u32 = 1;

pub const VERIFY_COMMANDS: [&str; 7] = [
    "format",
    "lint",
    "typecheck",
    "generate",
    "build",
    "test",
    "coverage",
];

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

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum VerifyError {
    #[error(
        "no verification sets: workspace has no dx.verify.toml (commit one at the workspace root)"
    )]
    MissingFile,
    #[error("read dx.verify.toml: {detail}")]
    Unreadable { detail: String },
    #[error("invalid dx.verify.toml: {detail}")]
    Malformed { detail: String },
    #[error("unsupported dx.verify.toml schema {found}: want schema = 1")]
    UnsupportedSchema { found: u32 },
    #[error("unknown verification set {name:?}: want one of {available}")]
    UnknownSet { name: String, available: String },
    #[error("verification set {name:?} declares no steps")]
    EmptySet { name: String },
    #[error("verification set {name:?} step {index} names unknown command {command:?}: want one of format, lint, typecheck, generate, build, test, coverage")]
    UnknownCommand {
        name: String,
        index: usize,
        command: String,
    },
    #[error("verification set {name:?} step {index} declares no scopes: spell explicit scopes such as \"//...\"")]
    EmptyScopes { name: String, index: usize },
    #[error("verification set {name:?} step {index} has unsupported scope {scope:?}: want // or @ labels, or workspace-relative file and directory paths")]
    BadScope {
        name: String,
        index: usize,
        scope: String,
    },
    #[error("verification set {name:?} step {index} has an empty bazel option")]
    EmptyBazelOption { name: String, index: usize },
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct VerifyFile {
    schema: u32,
    sets: BTreeMap<String, VerifySet>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct VerifySet {
    steps: Vec<VerifyStep>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct VerifyStep {
    command: String,
    scopes: Vec<String>,
    #[serde(default)]
    bazel_options: Vec<String>,
    #[serde(default = "default_required")]
    required: bool,
}

fn default_required() -> bool {
    true
}

fn available_sets(sets: &BTreeMap<String, VerifySet>) -> String {
    if sets.is_empty() {
        return "(no sets declared)".to_owned();
    }
    sets.keys()
        .map(|name| format!("{name:?}"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn resolve_step(name: &str, index: usize, step: VerifyStep) -> Result<ResolvedStep, VerifyError> {
    if !VERIFY_COMMANDS.contains(&step.command.as_str()) {
        return Err(VerifyError::UnknownCommand {
            name: name.to_owned(),
            index,
            command: step.command,
        });
    }
    if step.scopes.is_empty() {
        return Err(VerifyError::EmptyScopes {
            name: name.to_owned(),
            index,
        });
    }
    for scope in &step.scopes {
        if scope.is_empty() || scope == "-" || scope.starts_with(':') {
            return Err(VerifyError::BadScope {
                name: name.to_owned(),
                index,
                scope: scope.clone(),
            });
        }
    }
    for option in &step.bazel_options {
        if option.is_empty() {
            return Err(VerifyError::EmptyBazelOption {
                name: name.to_owned(),
                index,
            });
        }
    }
    Ok(ResolvedStep {
        command: step.command,
        scopes: step.scopes,
        bazel_options: step.bazel_options,
        required: step.required,
    })
}

pub fn load_verify_set(workspace: &Path, name: &str) -> Result<ResolvedSet, VerifyError> {
    let path = workspace.join(VERIFY_TOML_REL);
    let text = std::fs::read_to_string(&path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            VerifyError::MissingFile
        } else {
            VerifyError::Unreadable {
                detail: error.to_string(),
            }
        }
    })?;
    let mut file: VerifyFile = toml::from_str(&text).map_err(|error| VerifyError::Malformed {
        detail: error.to_string(),
    })?;
    if file.schema != VERIFY_SCHEMA {
        return Err(VerifyError::UnsupportedSchema { found: file.schema });
    }
    let Some(set) = file.sets.remove(name) else {
        return Err(VerifyError::UnknownSet {
            name: name.to_owned(),
            available: available_sets(&file.sets),
        });
    };
    if set.steps.is_empty() {
        return Err(VerifyError::EmptySet {
            name: name.to_owned(),
        });
    }
    let mut steps = Vec::with_capacity(set.steps.len());
    for (offset, step) in set.steps.into_iter().enumerate() {
        steps.push(resolve_step(name, offset + 1, step)?);
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
        dx_test_scratch::scratch(name)
    }

    fn write_sets(dir: &Path, text: &str) {
        std::fs::write(dir.join(VERIFY_TOML_REL), text).expect("write sets");
    }

    fn minimal_set(steps: &str) -> String {
        format!("schema = 1\n[sets.pre-pr]\nsteps = [\n{steps}\n]\n")
    }

    fn step(command: &str, scopes: &str) -> String {
        format!("{{ command = \"{command}\", scopes = [{scopes}] }}")
    }

    #[test]
    fn missing_file_names_the_committed_path() {
        let dir = scratch("dx-verify-missing-");
        assert_eq!(
            load_verify_set(dir.path(), "pre-pr"),
            Err(VerifyError::MissingFile)
        );
        assert_eq!(
            VerifyError::MissingFile.to_string(),
            "no verification sets: workspace has no dx.verify.toml (commit one at the workspace root)"
        );
    }

    #[test]
    fn happy_path_applies_documented_defaults() {
        let dir = scratch("dx-verify-happy-");
        write_sets(
            dir.path(),
            &minimal_set(&format!(
                "{},\n{{ command = \"test\", scopes = [\"//a:one\"], bazel_options = [\"--jobs=4\"], required = false }}",
                step("format", "\"//...\"")
            )),
        );
        let set = load_verify_set(dir.path(), "pre-pr").expect("loads");
        assert_eq!(set.name, "pre-pr");
        assert_eq!(
            set.steps,
            vec![
                ResolvedStep {
                    command: "format".to_owned(),
                    scopes: vec!["//...".to_owned()],
                    bazel_options: Vec::new(),
                    required: true,
                },
                ResolvedStep {
                    command: "test".to_owned(),
                    scopes: vec!["//a:one".to_owned()],
                    bazel_options: vec!["--jobs=4".to_owned()],
                    required: false,
                },
            ]
        );
    }

    #[test]
    fn malformed_toml_and_schema_fail_closed() {
        let dir = scratch("dx-verify-malformed-");
        write_sets(dir.path(), "schema = [");
        assert!(matches!(
            load_verify_set(dir.path(), "pre-pr"),
            Err(VerifyError::Malformed { .. })
        ));
        write_sets(dir.path(), "schema = 2\n[sets.pre-pr]\nsteps = []\n");
        assert_eq!(
            load_verify_set(dir.path(), "pre-pr"),
            Err(VerifyError::UnsupportedSchema { found: 2 })
        );
    }

    #[test]
    fn unknown_set_names_the_available_sets() {
        let dir = scratch("dx-verify-unknown-set-");
        write_sets(dir.path(), &minimal_set(&step("format", "\"//...\"")));
        let error = load_verify_set(dir.path(), "other").expect_err("unknown set");
        assert_eq!(
            error,
            VerifyError::UnknownSet {
                name: "other".to_owned(),
                available: "\"pre-pr\"".to_owned(),
            }
        );
        assert_eq!(
            error.to_string(),
            "unknown verification set \"other\": want one of \"pre-pr\""
        );
    }

    #[test]
    fn empty_steps_and_unknown_keys_fail() {
        let dir = scratch("dx-verify-empty-");
        write_sets(dir.path(), "schema = 1\n[sets.empty]\nsteps = []\n");
        assert_eq!(
            load_verify_set(dir.path(), "empty"),
            Err(VerifyError::EmptySet {
                name: "empty".to_owned(),
            })
        );
        write_sets(
            dir.path(),
            "schema = 1\n[sets.pre-pr]\nsteps = [\n{ command = \"format\", scopes = [\"//...\"], verify = \"other\" }\n]\n",
        );
        assert!(matches!(
            load_verify_set(dir.path(), "pre-pr"),
            Err(VerifyError::Malformed { .. })
        ));
        write_sets(
            dir.path(),
            "schema = 1\n[sets.pre-pr]\nsteps = [\n{ command = \"run\", scopes = [\"//:bin\"] }\n]\n",
        );
        assert_eq!(
            load_verify_set(dir.path(), "pre-pr"),
            Err(VerifyError::UnknownCommand {
                name: "pre-pr".to_owned(),
                index: 1,
                command: "run".to_owned(),
            })
        );
    }

    #[test]
    fn bad_scopes_and_options_fail_with_step_index() {
        let dir = scratch("dx-verify-scopes-");
        write_sets(
            dir.path(),
            &minimal_set("{ command = \"format\", scopes = [] }"),
        );
        assert_eq!(
            load_verify_set(dir.path(), "pre-pr"),
            Err(VerifyError::EmptyScopes {
                name: "pre-pr".to_owned(),
                index: 1,
            })
        );
        for (case, scopes) in [
            ("blank", "\"\""),
            ("relative", "\":corpus\""),
            ("dash", "\"-\""),
        ] {
            write_sets(dir.path(), &minimal_set(&step("format", scopes)));
            let error = load_verify_set(dir.path(), "pre-pr").expect_err(case);
            assert!(
                matches!(
                    error,
                    VerifyError::EmptyScopes { .. } | VerifyError::BadScope { .. }
                ),
                "{case}: {error}"
            );
        }
        write_sets(
            dir.path(),
            &minimal_set("{ command = \"build\", scopes = [\"//...\"], bazel_options = [\"\"] }"),
        );
        assert_eq!(
            load_verify_set(dir.path(), "pre-pr"),
            Err(VerifyError::EmptyBazelOption {
                name: "pre-pr".to_owned(),
                index: 1,
            })
        );
    }

    #[test]
    fn every_advertised_step_command_resolves() {
        let dir = scratch("dx-verify-commands-");
        let body = VERIFY_COMMANDS
            .iter()
            .map(|command| step(command, "\"//...\""))
            .collect::<Vec<_>>()
            .join(",\n");
        write_sets(dir.path(), &minimal_set(&body));
        let set = load_verify_set(dir.path(), "pre-pr").expect("loads");
        assert_eq!(set.steps.len(), VERIFY_COMMANDS.len());
        for (resolved, command) in set.steps.iter().zip(VERIFY_COMMANDS.iter()) {
            assert_eq!(&resolved.command, command);
            assert!(resolved.required);
        }
    }
}
