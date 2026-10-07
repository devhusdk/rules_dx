//! The matrix manifest the rule writes and the runner argv it describes.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::Error;

pub const MANIFEST_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub schema_version: u32,
    pub name: String,
    pub producer: String,
    pub capability: String,
    pub stages: Vec<String>,
    pub runner: String,
    pub printer: String,
    pub expected: String,
    pub snapshot_dir: String,
    #[serde(default)]
    pub sources: Vec<Mapping>,
    #[serde(default)]
    pub siblings: Vec<Mapping>,
    #[serde(default)]
    pub tools: Vec<Tool>,
    #[serde(default)]
    pub tool_configs: Vec<ToolConfig>,
    #[serde(default)]
    pub tool_editions: Vec<ToolEdition>,
    #[serde(default)]
    pub tool_files: Vec<ToolFile>,
    #[serde(default)]
    pub tool_env: Vec<ToolEnv>,
    #[serde(default)]
    pub upstream: Vec<Upstream>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Mapping {
    pub workspace: String,
    pub rlocation: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tool {
    pub tool: String,
    pub rlocation: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolConfig {
    pub tool: String,
    pub config: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolEdition {
    pub tool: String,
    pub edition: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolFile {
    pub tool: String,
    pub rel: String,
    pub rlocation: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolEnv {
    pub tool: String,
    pub key: String,
    pub value: EnvValue,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(untagged)]
pub enum EnvValue {
    RunfilesRoot { runfiles_root: bool },
    Literal { value: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Upstream {
    pub tool: String,
    pub rlocation: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StageSpec {
    pub tool_id: String,
    pub class_ids: Vec<String>,
    pub source_paths: Vec<String>,
}

/// Resolves one runfiles rlocation to the file on disk.
pub trait Rlocation {
    fn path(&self, rlocation: &str) -> Result<PathBuf, Error>;
}

impl<F> Rlocation for F
where
    F: Fn(&str) -> Result<PathBuf, Error>,
{
    fn path(&self, rlocation: &str) -> Result<PathBuf, Error> {
        self(rlocation)
    }
}

impl Manifest {
    /// Reads one manifest written by the matrix rule.
    pub fn parse(bytes: &[u8]) -> Result<Self, Error> {
        let manifest: Self =
            serde_json::from_slice(bytes).map_err(|error| Error::Malformed(error.to_string()))?;
        if manifest.schema_version != MANIFEST_SCHEMA_VERSION {
            return Err(Error::UnsupportedSchema {
                found: manifest.schema_version,
            });
        }
        Ok(manifest)
    }

    /// Splits every declared stage into its tool, classes and sources.
    pub fn stages(&self) -> Result<Vec<StageSpec>, Error> {
        self.stages.iter().map(|spec| parse_stage(spec)).collect()
    }

    /// Builds the runner argv for one case, in the order the runner reads.
    pub fn runner_argv<L: Rlocation>(
        &self,
        output: &Path,
        scratch: &Path,
        runfiles_root: &Path,
        lookup: &L,
    ) -> Result<Vec<OsString>, Error> {
        let mut argv = vec![
            OsString::from(lookup.path(&self.runner)?),
            flag("--producer"),
            OsString::from(&self.producer),
            flag("--capability"),
            OsString::from(&self.capability),
            flag("--output"),
            output.into(),
        ];
        for stage in &self.stages {
            argv.push(flag("--stage"));
            argv.push(OsString::from(stage));
        }
        for (flag_name, mappings) in [("--source", &self.sources), ("--sibling", &self.siblings)] {
            for mapping in mappings {
                argv.push(flag(flag_name));
                argv.push(OsString::from(format!(
                    "{}={}",
                    mapping.workspace,
                    display(&lookup.path(&mapping.rlocation)?)
                )));
            }
        }
        argv.push(flag("--real"));
        argv.push(flag("--scratch-parent"));
        argv.push(scratch.into());
        for tool in &self.tools {
            argv.push(flag("--tool-binary"));
            argv.push(OsString::from(format!(
                "{}={}",
                tool.tool,
                display(&lookup.path(&tool.rlocation)?)
            )));
        }
        for config in &self.tool_configs {
            argv.push(flag("--tool-config"));
            argv.push(OsString::from(format!("{}={}", config.tool, config.config)));
        }
        for edition in &self.tool_editions {
            argv.push(flag("--tool-edition"));
            argv.push(OsString::from(format!(
                "{}={}",
                edition.tool, edition.edition
            )));
        }
        for file in &self.tool_files {
            argv.push(flag("--tool-file"));
            argv.push(OsString::from(format!(
                "{}={}={}",
                file.tool,
                file.rel,
                display(&lookup.path(&file.rlocation)?)
            )));
        }
        for entry in &self.tool_env {
            argv.push(flag("--tool-env"));
            argv.push(OsString::from(format!(
                "{}={}={}",
                entry.tool,
                entry.key,
                entry.value.text(runfiles_root)?
            )));
        }
        for entry in &self.upstream {
            argv.push(flag("--upstream-diagnostics"));
            argv.push(OsString::from(format!(
                "{}={}",
                entry.tool,
                display(&lookup.path(&entry.rlocation)?)
            )));
        }
        Ok(argv)
    }

    /// Builds the printer argv for one result protobuf.
    pub fn printer_argv<L: Rlocation>(
        &self,
        output: &Path,
        lookup: &L,
    ) -> Result<Vec<OsString>, Error> {
        Ok(vec![lookup.path(&self.printer)?.into(), output.into()])
    }

    /// The workspace path one staged update belongs to.
    pub fn snapshot_path(&self) -> String {
        format!("{}/{}.expected.txt", self.snapshot_dir, self.name)
    }

    /// The artifact name one staged update is written under.
    pub fn update_name(&self) -> String {
        format!("{}.expected.update", self.name)
    }
}

fn flag(name: &str) -> OsString {
    OsString::from(name)
}

fn display(path: &Path) -> String {
    path.display().to_string()
}

fn parse_stage(spec: &str) -> Result<StageSpec, Error> {
    let (tool_id, rest) = spec
        .split_once(';')
        .ok_or_else(|| Error::Malformed(format!("malformed stage {spec:?}")))?;
    let (classes, paths) = rest
        .split_once(';')
        .ok_or_else(|| Error::Malformed(format!("malformed stage {spec:?}")))?;
    Ok(StageSpec {
        tool_id: tool_id.to_owned(),
        class_ids: classes.split(',').map(str::to_owned).collect(),
        source_paths: paths.split(',').map(str::to_owned).collect(),
    })
}

impl EnvValue {
    /// The text one tool environment entry carries.
    pub fn text(&self, runfiles_root: &Path) -> Result<String, Error> {
        match self {
            EnvValue::RunfilesRoot {
                runfiles_root: wanted,
            } => {
                if *wanted {
                    Ok(display(runfiles_root))
                } else {
                    Err(Error::Malformed(
                        "runfiles_root must be true when the field is present".to_owned(),
                    ))
                }
            }
            EnvValue::Literal { value } => Ok(value.clone()),
        }
    }
}

/// The root the runfiles tree and manifest are read from.
pub fn runfiles_root() -> PathBuf {
    for key in ["RUNFILES_DIR", "TEST_SRCDIR"] {
        if let Some(dir) = std::env::var_os(key) {
            if !dir.is_empty() {
                return PathBuf::from(dir);
            }
        }
    }
    std::env::temp_dir()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn minimal() -> serde_json::Value {
        serde_json::json!({
            "schema_version": 1,
            "name": "matrix_case",
            "producer": "//quality/testdata:matrix_case",
            "capability": "lint",
            "stages": ["tool;rust;src/lib.rs"],
            "runner": "_main/quality/runner/tool",
            "printer": "_main/quality/result/tool",
            "expected": "_main/quality/testdata/matrix/case.expected.txt",
            "snapshot_dir": "quality/testdata/matrix",
        })
    }

    #[test]
    fn minimal_manifest_parses_with_empty_sections() {
        let manifest = Manifest::parse(serde_json::to_vec(&minimal()).unwrap().as_slice()).unwrap();
        assert_eq!(manifest.name, "matrix_case");
        assert!(manifest.sources.is_empty());
        assert!(manifest.tool_env.is_empty());
    }

    #[test]
    fn unknown_field_is_rejected() {
        let mut value = minimal();
        value["surprise"] = serde_json::json!(true);
        let error = Manifest::parse(serde_json::to_vec(&value).unwrap().as_slice()).unwrap_err();
        assert!(matches!(error, Error::Malformed(_)), "error: {error}");
    }

    #[test]
    fn wrong_schema_version_is_rejected() {
        let mut value = minimal();
        value["schema_version"] = serde_json::json!(2);
        let error = Manifest::parse(serde_json::to_vec(&value).unwrap().as_slice()).unwrap_err();
        assert_eq!(error, Error::UnsupportedSchema { found: 2 });
    }

    #[test]
    fn stage_spec_splits_tool_classes_and_sources() {
        let mut value = minimal();
        value["stages"] = serde_json::json!(["rustfmt;rust;src/a.rs,src/b.rs"]);
        let manifest = Manifest::parse(serde_json::to_vec(&value).unwrap().as_slice()).unwrap();
        assert_eq!(
            manifest.stages().unwrap(),
            vec![StageSpec {
                tool_id: "rustfmt".to_owned(),
                class_ids: vec!["rust".to_owned()],
                source_paths: vec!["src/a.rs".to_owned(), "src/b.rs".to_owned()],
            }]
        );
    }

    #[test]
    fn malformed_stage_is_rejected() {
        let mut value = minimal();
        value["stages"] = serde_json::json!(["rustfmt"]);
        let manifest = Manifest::parse(serde_json::to_vec(&value).unwrap().as_slice()).unwrap();
        assert!(matches!(
            manifest.stages().unwrap_err(),
            Error::Malformed(_)
        ));
    }

    #[test]
    fn runner_argv_orders_sources_before_tools() {
        let mut value = minimal();
        value["sources"] =
            serde_json::json!([{"workspace": "src/lib.rs", "rlocation": "_main/r/a.rs"}]);
        value["tools"] = serde_json::json!([{"tool": "rustfmt", "rlocation": "_main/r/fmt"}]);
        let manifest = Manifest::parse(serde_json::to_vec(&value).unwrap().as_slice()).unwrap();
        let argv = manifest
            .runner_argv(
                Path::new("/work/out.pb"),
                Path::new("/work/scratch"),
                Path::new("/runfiles"),
                &|key: &str| Ok::<PathBuf, Error>(PathBuf::from(key)),
            )
            .unwrap()
            .into_iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        let source = argv.iter().position(|arg| arg == "--source").unwrap();
        let tool = argv.iter().position(|arg| arg == "--tool-binary").unwrap();
        assert!(source < tool, "argv: {argv:?}");
        assert!(argv.contains(&"--real".to_owned()));
        assert!(argv.contains(&"src/lib.rs=_main/r/a.rs".to_owned()));
    }

    #[test]
    fn tool_env_runfiles_root_resolves_to_the_tree() {
        let value = EnvValue::RunfilesRoot {
            runfiles_root: true,
        };
        assert_eq!(value.text(Path::new("/runfiles")).unwrap(), "/runfiles");
        assert!(matches!(
            EnvValue::RunfilesRoot {
                runfiles_root: false
            }
            .text(Path::new("/runfiles")),
            Err(Error::Malformed(_))
        ));
        assert_eq!(
            EnvValue::Literal {
                value: "1".to_owned()
            }
            .text(Path::new("/runfiles"))
            .unwrap(),
            "1"
        );
    }

    #[test]
    fn snapshot_paths_name_the_case() {
        let manifest = Manifest::parse(serde_json::to_vec(&minimal()).unwrap().as_slice()).unwrap();
        assert_eq!(
            manifest.snapshot_path(),
            "quality/testdata/matrix/matrix_case.expected.txt"
        );
        assert_eq!(manifest.update_name(), "matrix_case.expected.update");
    }
}
