//! Versioned request document for one quality runner action.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use dx_path::classify;

use crate::StageSpec;

pub const REQUEST_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub version: u32,
    pub producer: String,
    pub capability: String,
    pub output: String,
    pub real: bool,
    pub scratch_parent: Option<String>,
    pub stages: Vec<Stage>,
    pub sources: Vec<Mapping>,
    #[serde(default)]
    pub siblings: Vec<Mapping>,
    #[serde(default)]
    pub resolves: Vec<Mapping>,
    #[serde(default)]
    pub tools: Vec<Tool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Stage {
    pub tool: String,
    pub classes: Vec<String>,
    pub sources: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Mapping {
    pub path: String,
    #[serde(rename = "exec")]
    pub exec_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnvVar {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tool {
    pub id: String,
    pub binary: Option<String>,
    pub config: Option<String>,
    pub edition: Option<String>,
    #[serde(default)]
    pub files: Vec<Mapping>,
    #[serde(default)]
    pub env: Vec<EnvVar>,
    #[serde(default)]
    pub upstream_diagnostics: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Invocation {
    pub producer: String,
    pub capability: String,
    pub output: String,
    pub stages: Vec<StageSpec>,
    pub sources: Vec<(String, String)>,
    pub siblings: Vec<(String, String)>,
    pub resolves: Vec<(String, String)>,
    pub real: bool,
    pub scratch_parent: Option<String>,
    pub binaries: Vec<(String, PathBuf)>,
    pub configs: Vec<(String, String)>,
    pub editions: Vec<(String, String)>,
    pub tool_files: Vec<(String, String, String)>,
    pub tool_env: Vec<(String, String, String)>,
    pub upstream: Vec<(String, PathBuf)>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RequestError {
    #[error("cannot read request {source:?}: {detail}")]
    Unreadable { source: String, detail: String },
    #[error("malformed request {source}: {detail}")]
    Malformed { source: String, detail: String },
    #[error("unsupported request version {found}: want {want}")]
    UnsupportedVersion { found: u32, want: u32 },
    #[error("empty {field}: want a non-empty value")]
    Empty { field: &'static str },
    #[error("duplicate {field} {value:?}")]
    Duplicate { field: &'static str, value: String },
    #[error("invalid {field} {path:?}: {reason}")]
    BadPath {
        field: &'static str,
        path: String,
        reason: &'static str,
    },
    #[error("tool {tool:?} sets {setting} without a binary; declare the binary first")]
    ToolWithoutBinary {
        tool: String,
        setting: &'static str,
    },
    #[error("stage tool {tool:?} is not declared in tools")]
    UndeclaredStageTool { tool: String },
    #[error("stage source {path:?} is not declared in sources")]
    UndeclaredStageSource { path: String },
}

pub fn load(path: &Path) -> Result<Request, RequestError> {
    let source = path.display().to_string();
    let text = std::fs::read_to_string(path).map_err(|error| RequestError::Unreadable {
        source: source.clone(),
        detail: error.to_string(),
    })?;
    parse(&text, &source)
}

pub fn parse(text: &str, source: &str) -> Result<Request, RequestError> {
    let request: Request =
        serde_json::from_str(text).map_err(|error| RequestError::Malformed {
            source: source.to_owned(),
            detail: error.to_string(),
        })?;
    check(&request)?;
    Ok(request)
}

impl Request {
    pub fn invocation(self) -> Invocation {
        let mut binaries = Vec::new();
        let mut configs = Vec::new();
        let mut editions = Vec::new();
        let mut tool_files = Vec::new();
        let mut tool_env = Vec::new();
        let mut upstream = Vec::new();
        for tool in &self.tools {
            if let Some(binary) = &tool.binary {
                binaries.push((tool.id.clone(), PathBuf::from(binary)));
            }
            if let Some(config) = &tool.config {
                configs.push((tool.id.clone(), config.clone()));
            }
            if let Some(edition) = &tool.edition {
                editions.push((tool.id.clone(), edition.clone()));
            }
            for file in &tool.files {
                tool_files.push((tool.id.clone(), file.path.clone(), file.exec_path.clone()));
            }
            for var in &tool.env {
                tool_env.push((tool.id.clone(), var.key.clone(), var.value.clone()));
            }
            for diagnostic in &tool.upstream_diagnostics {
                upstream.push((tool.id.clone(), PathBuf::from(diagnostic)));
            }
        }
        Invocation {
            producer: self.producer,
            capability: self.capability,
            output: self.output,
            stages: self
                .stages
                .iter()
                .map(|stage| StageSpec {
                    tool_id: stage.tool.clone(),
                    class_ids: stage.classes.clone(),
                    source_paths: stage.sources.clone(),
                })
                .collect(),
            sources: self.sources.iter().map(pair).collect(),
            siblings: self.siblings.iter().map(pair).collect(),
            resolves: self.resolves.iter().map(pair).collect(),
            real: self.real,
            scratch_parent: self.scratch_parent.clone(),
            binaries,
            configs,
            editions,
            tool_files,
            tool_env,
            upstream,
        }
    }
}

fn pair(mapping: &Mapping) -> (String, String) {
    (mapping.path.clone(), mapping.exec_path.clone())
}

fn check_path(field: &'static str, path: &str) -> Result<(), RequestError> {
    if let Some(problem) = classify(path) {
        return Err(RequestError::BadPath {
            field,
            path: path.to_owned(),
            reason: problem.reason(),
        });
    }
    Ok(())
}

fn check_non_empty(field: &'static str, value: &str) -> Result<(), RequestError> {
    if value.is_empty() {
        return Err(RequestError::Empty { field });
    }
    Ok(())
}

fn check_mappings(field: &'static str, mappings: &[Mapping]) -> Result<(), RequestError> {
    let mut seen = BTreeSet::new();
    for mapping in mappings {
        check_path(field, &mapping.path)?;
        if !seen.insert(mapping.path.clone()) {
            return Err(RequestError::Duplicate {
                field,
                value: mapping.path.clone(),
            });
        }
        check_non_empty("mapping exec", &mapping.exec_path)?;
    }
    Ok(())
}

fn check_tools(tools: &[Tool]) -> Result<(), RequestError> {
    let mut seen = BTreeSet::new();
    for tool in tools {
        check_non_empty("tool id", &tool.id)?;
        if !seen.insert(tool.id.clone()) {
            return Err(RequestError::Duplicate {
                field: "tool id",
                value: tool.id.clone(),
            });
        }
        if let Some(binary) = &tool.binary {
            check_non_empty("tool binary", binary)?;
        } else if tool.config.is_some() {
            return Err(RequestError::ToolWithoutBinary {
                tool: tool.id.clone(),
                setting: "config",
            });
        } else if tool.edition.is_some() {
            return Err(RequestError::ToolWithoutBinary {
                tool: tool.id.clone(),
                setting: "edition",
            });
        } else if !tool.files.is_empty() {
            return Err(RequestError::ToolWithoutBinary {
                tool: tool.id.clone(),
                setting: "files",
            });
        } else if !tool.env.is_empty() {
            return Err(RequestError::ToolWithoutBinary {
                tool: tool.id.clone(),
                setting: "env",
            });
        }
        for file in &tool.files {
            check_path("tool file path", &file.path)?;
            check_non_empty("tool file exec", &file.exec_path)?;
        }
        for var in &tool.env {
            check_non_empty("tool env key", &var.key)?;
        }
        for diagnostic in &tool.upstream_diagnostics {
            check_non_empty("upstream diagnostics", diagnostic)?;
        }
    }
    Ok(())
}

fn check_stages(
    request: &Request,
    declared_tools: &BTreeSet<&str>,
    declared_sources: &BTreeSet<&str>,
) -> Result<(), RequestError> {
    for stage in &request.stages {
        check_non_empty("stage tool", &stage.tool)?;
        if stage.classes.is_empty() {
            return Err(RequestError::Empty {
                field: "stage classes",
            });
        }
        if stage.sources.is_empty() {
            return Err(RequestError::Empty {
                field: "stage sources",
            });
        }
        if request.real && !declared_tools.contains(stage.tool.as_str()) {
            return Err(RequestError::UndeclaredStageTool {
                tool: stage.tool.clone(),
            });
        }
        for class in &stage.classes {
            check_non_empty("stage class", class)?;
        }
        for source in &stage.sources {
            check_path("stage source", source)?;
            if !declared_sources.contains(source.as_str()) {
                return Err(RequestError::UndeclaredStageSource {
                    path: source.clone(),
                });
            }
        }
    }
    Ok(())
}

fn check(request: &Request) -> Result<(), RequestError> {
    if request.version != REQUEST_VERSION {
        return Err(RequestError::UnsupportedVersion {
            found: request.version,
            want: REQUEST_VERSION,
        });
    }
    check_non_empty("producer", &request.producer)?;
    check_non_empty("capability", &request.capability)?;
    check_non_empty("output", &request.output)?;
    if request.stages.is_empty() {
        return Err(RequestError::Empty { field: "stages" });
    }
    check_mappings("source", &request.sources)?;
    check_mappings("sibling", &request.siblings)?;
    check_mappings("resolve", &request.resolves)?;
    check_tools(&request.tools)?;
    let declared_tools = request.tools.iter().map(|tool| tool.id.as_str()).collect();
    let declared_sources = request
        .sources
        .iter()
        .map(|mapping| mapping.path.as_str())
        .collect();
    check_stages(request, &declared_tools, &declared_sources)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc(stages: &str, sources: &str, tools: &str, real: bool) -> String {
        format!(
            "{{\n  \"version\": 1,\n  \"producer\": \"//pkg:lib\",\n  \"capability\": \"format\",\n  \"output\": \"out.pb\",\n  \"real\": {real},\n  \"scratch_parent\": null,\n  \"stages\": [{stages}],\n  \"sources\": [{sources}],\n  \"siblings\": [],\n  \"resolves\": [],\n  \"tools\": [{tools}]\n}}"
        )
    }

    const STAGE: &str =
        "{\"tool\": \"ruff\", \"classes\": [\"python\"], \"sources\": [\"src/main.py\"]}";
    const SOURCE: &str = "{\"path\": \"src/main.py\", \"exec\": \"pkg/main.py\"}";
    const TOOL: &str = "{\"id\": \"ruff\", \"binary\": \"tools/ruff\"}";

    fn error_of(text: &str) -> RequestError {
        parse(text, "<test>").expect_err("request must be rejected")
    }

    #[test]
    fn a_valid_v1_request_loads_and_converts() {
        let request = parse(&doc(STAGE, SOURCE, TOOL, true), "<test>").expect("request");
        let invocation = request.invocation();
        assert_eq!(invocation.producer, "//pkg:lib");
        assert_eq!(invocation.capability, "format");
        assert_eq!(invocation.output, "out.pb");
        assert!(invocation.real);
        assert_eq!(invocation.scratch_parent, None);
        assert_eq!(
            invocation.stages,
            [StageSpec {
                tool_id: "ruff".to_owned(),
                class_ids: vec!["python".to_owned()],
                source_paths: vec!["src/main.py".to_owned()],
            }]
        );
        assert_eq!(
            invocation.sources,
            [("src/main.py".to_owned(), "pkg/main.py".to_owned())]
        );
        assert_eq!(
            invocation.binaries,
            [("ruff".to_owned(), PathBuf::from("tools/ruff"))]
        );
        assert!(invocation.configs.is_empty());
        assert!(invocation.tool_files.is_empty());
        assert!(invocation.upstream.is_empty());
    }

    #[test]
    fn unusual_filenames_stay_unambiguous() {
        let names = [
            "src/a,b.py",
            "src/c;d.py",
            "src/e=f.py",
            "src/g h.py",
            "src/日本語.py",
        ];
        let mut sources = String::new();
        let mut stage_sources = String::new();
        for name in names {
            if !sources.is_empty() {
                sources.push(',');
            }
            sources.push_str(&format!(
                "{{\"path\": \"{name}\", \"exec\": \"exec/{name}\"}}"
            ));
            if !stage_sources.is_empty() {
                stage_sources.push(',');
            }
            stage_sources.push_str(&format!("\"{name}\""));
        }
        let stage = format!(
            "{{\"tool\": \"lint-a\", \"classes\": [\"python\"], \"sources\": [{stage_sources}]}}"
        );
        let request = parse(&doc(&stage, &sources, "", false), "<test>").expect("request");
        let invocation = request.invocation();
        let paths: Vec<&str> = invocation
            .sources
            .iter()
            .map(|(path, _)| path.as_str())
            .collect();
        assert_eq!(paths, names);
        assert_eq!(invocation.stages[0].source_paths, names);
        assert_eq!(
            invocation.sources[0].1,
            format!("exec/{}", names[0]),
            "exec path keeps its own spelling"
        );
    }

    #[test]
    fn unsupported_versions_and_malformed_documents_fail() {
        let document = doc(STAGE, SOURCE, TOOL, true);
        let bumped = document.replace("\"version\": 1", "\"version\": 2");
        assert_eq!(
            error_of(&bumped),
            RequestError::UnsupportedVersion {
                found: 2,
                want: REQUEST_VERSION,
            }
        );
        assert!(matches!(
            error_of("{\"version\": 1}"),
            RequestError::Malformed { .. }
        ));
        assert!(matches!(error_of("not json"), RequestError::Malformed { .. }));
        let unknown = document.replace("\"version\": 1,", "\"version\": 1, \"unknown\": 7,");
        match error_of(&unknown) {
            RequestError::Malformed { source, detail } => {
                assert_eq!(source, "<test>");
                assert!(detail.contains("unknown field"), "{detail}");
            }
            other => panic!("unexpected error: {other:?}"),
        }
        let missing = document.replace("  \"output\": \"out.pb\",\n", "");
        match error_of(&missing) {
            RequestError::Malformed { detail, .. } => {
                assert!(detail.contains("missing field `output`"), "{detail}");
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn duplicate_mappings_and_tool_ids_fail() {
        let sources = format!("{SOURCE},{SOURCE}");
        assert_eq!(
            error_of(&doc(STAGE, &sources, TOOL, true)),
            RequestError::Duplicate {
                field: "source",
                value: "src/main.py".to_owned(),
            }
        );
        let tools = format!("{TOOL},{TOOL}");
        assert_eq!(
            error_of(&doc(STAGE, SOURCE, &tools, true)),
            RequestError::Duplicate {
                field: "tool id",
                value: "ruff".to_owned(),
            }
        );
    }

    #[test]
    fn escaping_workspace_paths_fail() {
        for (path, reason) in [
            ("../escape.py", "path must have no '..' component"),
            ("/escape.py", "path must be workspace-relative, not absolute"),
            ("back\\slash.py", "path must use forward slashes"),
        ] {
            let json_path = path.replace('\\', "\\\\");
            let sources = format!("{{\"path\": \"{json_path}\", \"exec\": \"pkg/x.py\"}}");
            let stage = format!(
                "{{\"tool\": \"ruff\", \"classes\": [\"python\"], \"sources\": [\"{json_path}\"]}}"
            );
            assert_eq!(
                error_of(&doc(&stage, &sources, TOOL, true)),
                RequestError::BadPath {
                    field: "source",
                    path: path.to_owned(),
                    reason,
                },
                "path: {path}"
            );
        }
    }

    #[test]
    fn tool_settings_without_a_binary_fail() {
        for (tool, setting) in [
            ("{\"id\": \"ruff\", \"config\": \"ruff.toml\"}", "config"),
            ("{\"id\": \"ruff\", \"edition\": \"2021\"}", "edition"),
            (
                "{\"id\": \"ruff\", \"files\": [{\"path\": \"ruff.toml\", \"exec\": \"pkg/ruff.toml\"}]}",
                "files",
            ),
            (
                "{\"id\": \"ruff\", \"env\": [{\"key\": \"KEY\", \"value\": \"1\"}]}",
                "env",
            ),
        ] {
            assert_eq!(
                error_of(&doc(STAGE, SOURCE, tool, true)),
                RequestError::ToolWithoutBinary {
                    tool: "ruff".to_owned(),
                    setting,
                },
                "setting: {setting}"
            );
        }
    }

    #[test]
    fn undeclared_stage_tools_and_sources_fail() {
        let stage = "{\"tool\": \"taplo\", \"classes\": [\"toml\"], \"sources\": [\"src/main.py\"]}";
        assert_eq!(
            error_of(&doc(stage, SOURCE, TOOL, true)),
            RequestError::UndeclaredStageTool {
                tool: "taplo".to_owned(),
            }
        );
        let stage = "{\"tool\": \"ruff\", \"classes\": [\"python\"], \"sources\": [\"src/other.py\"]}";
        assert_eq!(
            error_of(&doc(stage, SOURCE, TOOL, true)),
            RequestError::UndeclaredStageSource {
                path: "src/other.py".to_owned(),
            }
        );
        let synthetic = "{\"tool\": \"lint-a\", \"classes\": [\"python\"], \"sources\": [\"src/main.py\"]}";
        let request = parse(&doc(synthetic, SOURCE, "", false), "<test>")
            .expect("non-real requests do not need declared tools");
        assert!(!request.real);
    }

    #[test]
    fn empty_required_fields_fail() {
        let document = doc(STAGE, SOURCE, TOOL, true);
        let producer = document.replace("\"producer\": \"//pkg:lib\"", "\"producer\": \"\"");
        assert_eq!(
            error_of(&producer),
            RequestError::Empty {
                field: "producer",
            }
        );
        assert_eq!(
            error_of(&doc("", SOURCE, TOOL, true)),
            RequestError::Empty { field: "stages" }
        );
        let stage = "{\"tool\": \"ruff\", \"classes\": [], \"sources\": [\"src/main.py\"]}";
        assert_eq!(
            error_of(&doc(stage, SOURCE, TOOL, true)),
            RequestError::Empty {
                field: "stage classes",
            }
        );
        let stage = "{\"tool\": \"ruff\", \"classes\": [\"python\"], \"sources\": []}";
        assert_eq!(
            error_of(&doc(stage, SOURCE, TOOL, true)),
            RequestError::Empty {
                field: "stage sources",
            }
        );
        let tools =
            "{\"id\": \"ruff\", \"binary\": \"tools/ruff\", \"env\": [{\"key\": \"\", \"value\": \"1\"}]}";
        assert_eq!(
            error_of(&doc(STAGE, SOURCE, tools, true)),
            RequestError::Empty {
                field: "tool env key",
            }
        );
        let sources = "{\"path\": \"src/main.py\", \"exec\": \"\"}";
        assert_eq!(
            error_of(&doc(STAGE, sources, TOOL, true)),
            RequestError::Empty {
                field: "mapping exec",
            }
        );
    }

    #[test]
    fn load_reads_the_document_and_names_an_unreadable_path() {
        let dir = tempfile::tempdir().expect("scratch");
        let path = dir.path().join("request.json");
        std::fs::write(&path, doc(STAGE, SOURCE, TOOL, true)).expect("request file");
        let request = load(&path).expect("request");
        assert_eq!(request.version, REQUEST_VERSION);
        let missing = dir.path().join("absent.json");
        match load(&missing).expect_err("missing request") {
            RequestError::Unreadable { source, detail } => {
                assert!(source.contains("absent.json"), "{source}");
                assert!(!detail.is_empty(), "{detail}");
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }
}
