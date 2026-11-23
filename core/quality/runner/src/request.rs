use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use serde::Deserialize;

use crate::StageSpec;
use crate::real::{PathTool, REAL_TOOLS};

pub const REQUEST_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestTool {
    pub binary: Option<PathBuf>,
    pub config_rel: Option<String>,
    pub edition: Option<String>,
    pub dialect: Option<String>,
    /// Extra tool arguments from build settings; each tool decides which it accepts.
    pub args: Vec<String>,
    pub files: Vec<(String, String)>,
    /// Files of `rel<TAB>exec` lines, staged like `files`.
    pub file_lists: Vec<PathBuf>,
    /// Files of `rel<TAB>exec` lines for dependency files: each exec path is staged once, and
    /// never when it is a checked source.
    pub dep_lists: Vec<PathBuf>,
    pub env: Vec<(String, String)>,
    pub upstream: Vec<PathBuf>,
    pub search_paths: Vec<PathBuf>,
    /// Helper executables the tool finds by name on its own `PATH`.
    pub path_tools: Vec<PathTool>,
}

/// Helper names each tool may be given on its `PATH`. One row per tool that shells out.
pub const PATH_TOOL_ALLOWLIST: &[(&str, &[&str])] = &[
    ("vale", &["rst2html", "asciidoctor"]),
    ("helm", &["kubeconform"]),
    ("trivy_config", &["bicep"]),
    ("pkl_lsp", &["java", "pkl-lsp.jar", "pkl-lsp-natives.jar"]),
    ("buf", &["diff"]),
    ("actionlint", &["shellcheck"]),
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RealRequest {
    pub producer: String,
    pub capability: String,
    pub stages: Vec<StageSpec>,
    pub sources: Vec<(String, String)>,
    pub siblings: Vec<(String, String)>,
    pub resolves: Vec<(String, String)>,
    /// Files of `workspace<TAB>exec` lines, resolved like `resolves` unless a mapping holds the path.
    pub resolve_lists: Vec<ResolveList>,
    pub tools: BTreeMap<String, RequestTool>,
    pub scratch_parent: Option<String>,
    /// `dx <quality> --unsafe`: adapters may also apply the fixes the tool marks unsafe.
    pub unsafe_fixes: bool,
}

/// One resolve list file and the extensions it keeps; empty keeps every file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolveList {
    pub list: PathBuf,
    pub extensions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RequestError {
    #[error("malformed request: {detail}")]
    Json { detail: String },
    #[error("unsupported request schema {found}: want {REQUEST_SCHEMA_VERSION}")]
    UnsupportedSchema { found: u32 },
    #[error("empty producer: want a non-empty producer")]
    EmptyProducer,
    #[error("unknown capability {capability:?}: want lint, typecheck, format, or audit")]
    UnknownCapability { capability: String },
    #[error("empty stages: want at least one stage")]
    EmptyStages,
    #[error("empty tool id at stage {stage}")]
    EmptyToolId { stage: usize },
    #[error("unknown tool {tool:?}: want a real adapter tool")]
    UnknownTool { tool: String },
    #[error("duplicate stage for tool {tool:?}")]
    DuplicateStage { tool: String },
    #[error("empty class ids at stage {stage}")]
    EmptyClassIds { stage: usize },
    #[error("empty stage sources at stage {stage}")]
    EmptyStageSources { stage: usize },
    #[error("invalid {role} {path:?}: {reason}")]
    BadPath {
        role: String,
        path: String,
        reason: String,
    },
    #[error("tool {tool:?} may not be given {name:?} on its PATH")]
    UnknownPathTool { tool: String, name: String },
    #[error("empty exec path for {role} {workspace:?}")]
    EmptyExec { role: String, workspace: String },
    #[error("duplicate {role} {path:?}")]
    DuplicateMapping { role: String, path: String },
    #[error("{role} {path:?} shadows a checked source")]
    ShadowedMapping { role: String, path: String },
    #[error("stage source {path:?} has no declared source input")]
    MissingSource { path: String },
    #[error("stage tool {tool:?} has no declared tool entry")]
    MissingTool { tool: String },
    #[error("tool entry {tool:?} is used by no stage")]
    UnreferencedTool { tool: String },
    #[error("tool entry {tool:?} has neither a binary nor upstream diagnostics")]
    ToolWithoutBinary { tool: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
struct StageDoc {
    tool: String,
    classes: Vec<String>,
    sources: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
struct MappingDoc {
    workspace: String,
    exec: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
struct ToolFileDoc {
    mirror_rel: String,
    exec: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
struct ResolveListDoc {
    list: String,
    #[serde(default)]
    extensions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
struct EnvDoc {
    key: String,
    value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
struct ToolDoc {
    #[serde(default)]
    binary: Option<String>,
    #[serde(default)]
    config: Option<String>,
    #[serde(default)]
    edition: Option<String>,
    #[serde(default)]
    dialect: Option<String>,
    #[serde(default)]
    args: Vec<String>,
    #[serde(default)]
    files: Vec<ToolFileDoc>,
    #[serde(default)]
    file_lists: Vec<String>,
    #[serde(default)]
    dep_lists: Vec<String>,
    #[serde(default)]
    env: Vec<EnvDoc>,
    #[serde(default)]
    upstream: Vec<String>,
    #[serde(default)]
    search_paths: Vec<String>,
    #[serde(default)]
    path_tools: Vec<PathToolDoc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
struct PathToolDoc {
    name: String,
    binary: String,
    #[serde(default)]
    args: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
struct RequestDoc {
    schema_version: u32,
    producer: String,
    capability: String,
    stages: Vec<StageDoc>,
    sources: Vec<MappingDoc>,
    #[serde(default)]
    siblings: Vec<MappingDoc>,
    #[serde(default)]
    resolves: Vec<MappingDoc>,
    #[serde(default)]
    resolve_lists: Vec<ResolveListDoc>,
    #[serde(default)]
    tools: BTreeMap<String, ToolDoc>,
    #[serde(default)]
    scratch_parent: Option<String>,
    #[serde(default)]
    unsafe_fixes: bool,
}

fn check_workspace(role: &str, path: &str) -> Result<(), RequestError> {
    if let Some(reason) = dx_path::reject_reason(path) {
        return Err(RequestError::BadPath {
            role: role.to_owned(),
            path: path.to_owned(),
            reason: reason.to_owned(),
        });
    }
    Ok(())
}

fn check_mappings(
    role: &str,
    mappings: &[MappingDoc],
    seen: &mut BTreeSet<String>,
    forbid_shadow: bool,
) -> Result<Vec<(String, String)>, RequestError> {
    let mut out = Vec::with_capacity(mappings.len());
    let mut local = BTreeSet::new();
    for mapping in mappings {
        check_workspace(role, &mapping.workspace)?;
        if mapping.exec.is_empty() {
            return Err(RequestError::EmptyExec {
                role: role.to_owned(),
                workspace: mapping.workspace.clone(),
            });
        }
        if !local.insert(mapping.workspace.clone()) {
            return Err(RequestError::DuplicateMapping {
                role: role.to_owned(),
                path: mapping.workspace.clone(),
            });
        }
        if forbid_shadow && seen.contains(&mapping.workspace) {
            return Err(RequestError::ShadowedMapping {
                role: role.to_owned(),
                path: mapping.workspace.clone(),
            });
        }
        seen.insert(mapping.workspace.clone());
        out.push((mapping.workspace.clone(), mapping.exec.clone()));
    }
    Ok(out)
}

fn check_resolve_lists(docs: &[ResolveListDoc]) -> Result<Vec<ResolveList>, RequestError> {
    let mut out = Vec::with_capacity(docs.len());
    for doc in docs {
        if doc.list.is_empty() {
            return Err(RequestError::EmptyExec {
                role: "resolve list".to_owned(),
                workspace: String::new(),
            });
        }
        if doc.extensions.iter().any(String::is_empty) {
            return Err(RequestError::EmptyExec {
                role: "resolve list extension".to_owned(),
                workspace: doc.list.clone(),
            });
        }
        out.push(ResolveList {
            list: PathBuf::from(&doc.list),
            extensions: doc.extensions.clone(),
        });
    }
    Ok(out)
}

fn non_empty_paths(
    role: &str,
    tool_id: &str,
    values: &[String],
) -> Result<Vec<PathBuf>, RequestError> {
    let mut out = Vec::with_capacity(values.len());
    for value in values {
        if value.is_empty() {
            return Err(RequestError::EmptyExec {
                role: role.to_owned(),
                workspace: tool_id.to_owned(),
            });
        }
        out.push(PathBuf::from(value));
    }
    Ok(out)
}

fn check_path_tools(tool_id: &str, doc: &ToolDoc) -> Result<Vec<PathTool>, RequestError> {
    let mut path_tools: Vec<PathTool> = Vec::with_capacity(doc.path_tools.len());
    for helper in &doc.path_tools {
        let allowed = PATH_TOOL_ALLOWLIST
            .iter()
            .any(|(tool, names)| *tool == tool_id && names.contains(&helper.name.as_str()));
        if !allowed {
            return Err(RequestError::UnknownPathTool {
                tool: tool_id.to_owned(),
                name: helper.name.clone(),
            });
        }
        if helper.binary.is_empty() {
            return Err(RequestError::EmptyExec {
                role: "tool path helper".to_owned(),
                workspace: helper.name.clone(),
            });
        }
        if path_tools.iter().any(|tool| tool.name == helper.name) {
            return Err(RequestError::DuplicateMapping {
                role: "tool path helper".to_owned(),
                path: helper.name.clone(),
            });
        }
        path_tools.push(PathTool {
            name: helper.name.clone(),
            binary: PathBuf::from(&helper.binary),
            args: helper.args.clone(),
        });
    }
    Ok(path_tools)
}

fn check_tool(
    tool_id: &str,
    doc: &ToolDoc,
    staged: &BTreeSet<String>,
) -> Result<RequestTool, RequestError> {
    if !staged.contains(tool_id) {
        return Err(RequestError::UnreferencedTool {
            tool: tool_id.to_owned(),
        });
    }
    let binary = match &doc.binary {
        Some(path) if path.is_empty() => {
            return Err(RequestError::EmptyExec {
                role: "tool binary".to_owned(),
                workspace: tool_id.to_owned(),
            });
        }
        Some(path) => Some(PathBuf::from(path)),
        None => None,
    };
    if let Some(config) = &doc.config {
        check_workspace("tool config", config)?;
    }
    if let Some(edition) = &doc.edition
        && edition.is_empty()
    {
        return Err(RequestError::EmptyExec {
            role: "tool edition".to_owned(),
            workspace: tool_id.to_owned(),
        });
    }
    if doc.dialect.as_deref() == Some("") {
        return Err(RequestError::EmptyExec {
            role: "tool dialect".to_owned(),
            workspace: tool_id.to_owned(),
        });
    }
    let mut files = Vec::with_capacity(doc.files.len());
    for file in &doc.files {
        check_workspace("tool file", &file.mirror_rel)?;
        if file.exec.is_empty() {
            return Err(RequestError::EmptyExec {
                role: "tool file".to_owned(),
                workspace: file.mirror_rel.clone(),
            });
        }
        files.push((file.mirror_rel.clone(), file.exec.clone()));
    }
    let mut env = Vec::with_capacity(doc.env.len());
    for entry in &doc.env {
        if entry.key.is_empty() {
            return Err(RequestError::EmptyExec {
                role: "tool env".to_owned(),
                workspace: tool_id.to_owned(),
            });
        }
        env.push((entry.key.clone(), entry.value.clone()));
    }
    let upstream = non_empty_paths("upstream diagnostics", tool_id, &doc.upstream)?;
    let search_paths = non_empty_paths("tool search path", tool_id, &doc.search_paths)?;
    let file_lists = non_empty_paths("tool file list", tool_id, &doc.file_lists)?;
    let dep_lists = non_empty_paths("tool dependency list", tool_id, &doc.dep_lists)?;
    let path_tools = check_path_tools(tool_id, doc)?;
    if binary.is_none() && upstream.is_empty() {
        return Err(RequestError::ToolWithoutBinary {
            tool: tool_id.to_owned(),
        });
    }
    Ok(RequestTool {
        binary,
        config_rel: doc.config.clone(),
        edition: doc.edition.clone(),
        dialect: doc.dialect.clone(),
        args: doc.args.clone(),
        files,
        file_lists,
        dep_lists,
        env,
        upstream,
        search_paths,
        path_tools,
    })
}

/// Returns the result of the call.
///
/// # Errors
///
/// Returns the error describing the first step that failed.
pub fn parse_real_request(bytes: &[u8]) -> Result<RealRequest, RequestError> {
    let doc: RequestDoc = serde_json::from_slice(bytes).map_err(|error| RequestError::Json {
        detail: error.to_string(),
    })?;
    if doc.schema_version != REQUEST_SCHEMA_VERSION {
        return Err(RequestError::UnsupportedSchema {
            found: doc.schema_version,
        });
    }
    if doc.producer.is_empty() {
        return Err(RequestError::EmptyProducer);
    }
    match doc.capability.as_str() {
        "lint" | "typecheck" | "format" | "audit" => {}
        _ => {
            return Err(RequestError::UnknownCapability {
                capability: doc.capability.clone(),
            });
        }
    }
    if doc.stages.is_empty() {
        return Err(RequestError::EmptyStages);
    }
    let mut stages = Vec::with_capacity(doc.stages.len());
    let mut staged_tools = BTreeSet::new();
    for (index, stage) in doc.stages.iter().enumerate() {
        if stage.tool.is_empty() {
            return Err(RequestError::EmptyToolId { stage: index });
        }
        if !REAL_TOOLS.contains(&stage.tool.as_str()) {
            return Err(RequestError::UnknownTool {
                tool: stage.tool.clone(),
            });
        }
        if !staged_tools.insert(stage.tool.clone()) {
            return Err(RequestError::DuplicateStage {
                tool: stage.tool.clone(),
            });
        }
        if stage.classes.is_empty() || stage.classes.iter().any(String::is_empty) {
            return Err(RequestError::EmptyClassIds { stage: index });
        }
        if stage.sources.is_empty() {
            return Err(RequestError::EmptyStageSources { stage: index });
        }
        for path in &stage.sources {
            check_workspace("stage source", path)?;
        }
        stages.push(StageSpec {
            tool_id: stage.tool.clone(),
            class_ids: stage.classes.clone(),
            source_paths: stage.sources.clone(),
        });
    }
    let mut seen = BTreeSet::new();
    let sources = check_mappings("source", &doc.sources, &mut seen, false)?;
    let declared: BTreeSet<&str> = sources
        .iter()
        .map(|(workspace, _)| workspace.as_str())
        .collect();
    for stage in &stages {
        for path in &stage.source_paths {
            if !declared.contains(path.as_str()) {
                return Err(RequestError::MissingSource { path: path.clone() });
            }
        }
    }
    let siblings = check_mappings("sibling", &doc.siblings, &mut seen, true)?;
    let resolves = check_mappings("resolve", &doc.resolves, &mut seen, true)?;
    let resolve_lists = check_resolve_lists(&doc.resolve_lists)?;
    let mut tools = BTreeMap::new();
    for (tool_id, entry) in &doc.tools {
        tools.insert(tool_id.clone(), check_tool(tool_id, entry, &staged_tools)?);
    }
    for tool_id in &staged_tools {
        if !tools.contains_key(tool_id) {
            return Err(RequestError::MissingTool {
                tool: tool_id.clone(),
            });
        }
    }
    Ok(RealRequest {
        producer: doc.producer,
        capability: doc.capability,
        stages,
        sources,
        siblings,
        resolves,
        resolve_lists,
        tools,
        scratch_parent: doc.scratch_parent,
        unsafe_fixes: doc.unsafe_fixes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn skeleton(
        stages: serde_json::Value,
        sources: serde_json::Value,
        tools: serde_json::Value,
    ) -> Vec<u8> {
        let mut document = json!({
            "schema_version": 1,
            "producer": "//pkg:target",
            "capability": "lint",
            "siblings": [],
            "resolves": [],
        });
        document["stages"] = stages;
        document["sources"] = sources;
        document["tools"] = tools;
        serde_json::to_vec(&document).expect("skeleton encodes")
    }

    fn stage(tool: &str, classes: &[&str], sources: &[String]) -> serde_json::Value {
        json!({"tool": tool, "classes": classes, "sources": sources})
    }

    fn mapping(workspace: &str, exec: &str) -> serde_json::Value {
        json!({"workspace": workspace, "exec": exec})
    }

    fn binary_tool(binary: &str) -> serde_json::Value {
        json!({"binary": binary, "files": [], "env": [], "upstream": []})
    }

    fn one_source() -> (serde_json::Value, serde_json::Value, serde_json::Value) {
        (
            json!([{"tool": "ruff", "classes": ["python"], "sources": ["a.py"]}]),
            json!([{"workspace": "a.py", "exec": "exec/a.py"}]),
            json!({"ruff": {"binary": "bin/ruff", "files": [], "env": [], "upstream": []}}),
        )
    }

    fn parse_skeleton(
        stages: serde_json::Value,
        sources: serde_json::Value,
        tools: serde_json::Value,
    ) -> Result<RealRequest, RequestError> {
        parse_real_request(&skeleton(stages, sources, tools))
    }

    #[test]
    fn unsafe_fixes_default_off_and_parse_when_set() {
        let (stages, sources, tools) = one_source();
        let request = parse_skeleton(stages, sources, tools).expect("parses");
        assert!(!request.unsafe_fixes);

        let (stages, sources, tools) = one_source();
        let mut doc: serde_json::Value =
            serde_json::from_slice(&skeleton(stages, sources, tools)).expect("decodes");
        doc["unsafe_fixes"] = json!(true);
        let request =
            parse_real_request(&serde_json::to_vec(&doc).expect("encodes")).expect("parses");
        assert!(request.unsafe_fixes);
    }

    #[test]
    fn resolve_lists_parse_and_reject_empty_entries() {
        let (stages, sources, tools) = one_source();
        let request =
            parse_skeleton(stages.clone(), sources.clone(), tools.clone()).expect("parses");
        assert!(request.resolve_lists.is_empty());

        let mut doc: serde_json::Value =
            serde_json::from_slice(&skeleton(stages, sources, tools)).expect("decodes");
        doc["resolve_lists"] =
            json!([{"list": "out/ty-resolves.txt", "extensions": ["py", "pyi"]}]);
        let request =
            parse_real_request(&serde_json::to_vec(&doc).expect("encodes")).expect("parses");
        assert_eq!(
            request.resolve_lists,
            [ResolveList {
                list: PathBuf::from("out/ty-resolves.txt"),
                extensions: vec!["py".to_owned(), "pyi".to_owned()],
            }]
        );
        for bad in [
            json!({"list": ""}),
            json!({"list": "a.txt", "extensions": [""]}),
        ] {
            doc["resolve_lists"] = json!([bad]);
            assert!(matches!(
                parse_real_request(&serde_json::to_vec(&doc).expect("encodes")),
                Err(RequestError::EmptyExec { .. })
            ));
        }
    }

    #[test]
    fn unusual_filenames_survive_the_request_verbatim() {
        let weird: Vec<String> = [
            "a,b.py",
            "a;b.py",
            "a=b.py",
            "a b.py",
            "dir=x/a;b,c.py",
            "caf\u{e9}.py",
            "\u{65e5}\u{672c}\u{8a9e}.py",
        ]
        .iter()
        .map(|name| (*name).to_owned())
        .collect();
        let sources: Vec<serde_json::Value> = weird
            .iter()
            .enumerate()
            .map(|(index, name)| mapping(name, &format!("exec/{index}.py")))
            .collect();
        let request = parse_skeleton(
            json!([stage("ruff", &["python"], &weird)]),
            serde_json::Value::Array(sources),
            json!({"ruff": {
                "binary": "bin/ruff",
                "config": "ruff,;=.toml",
                "files": [{"mirror_rel": "conf,ig/x=y.toml", "exec": "exec/conf.toml"}],
                "env": [{"key": "EXTRA", "value": "a=b;c,d"}],
                "upstream": [],
            }}),
        )
        .expect("weird names parse");
        assert_eq!(request.stages[0].source_paths, weird);
        assert_eq!(
            request.sources,
            weird
                .iter()
                .enumerate()
                .map(|(index, name)| (name.clone(), format!("exec/{index}.py")))
                .collect::<Vec<(String, String)>>()
        );
        let tool = &request.tools["ruff"];
        assert_eq!(tool.config_rel.as_deref(), Some("ruff,;=.toml"));
        assert_eq!(
            tool.files,
            [("conf,ig/x=y.toml".to_owned(), "exec/conf.toml".to_owned())]
        );
        assert_eq!(tool.env, [("EXTRA".to_owned(), "a=b;c,d".to_owned())]);
    }

    #[test]
    fn schema_version_is_pinned_and_unknown_fields_rejected() {
        let (stages, sources, tools) = one_source();
        let mut raw: serde_json::Value =
            serde_json::from_slice(&skeleton(stages.clone(), sources.clone(), tools.clone()))
                .expect("skeleton decodes");
        raw["schema_version"] = json!(2);
        assert_eq!(
            parse_real_request(&serde_json::to_vec(&raw).expect("encodes")).expect_err("v2"),
            RequestError::UnsupportedSchema { found: 2 }
        );
        raw["schema_version"] = json!("1");
        assert!(
            matches!(
                parse_real_request(&serde_json::to_vec(&raw).expect("encodes")),
                Err(RequestError::Json { .. })
            ),
            "a string schema version must fail"
        );
        let mut raw: serde_json::Value =
            serde_json::from_slice(&skeleton(stages, sources, tools)).expect("skeleton decodes");
        raw["bogus"] = json!(true);
        assert!(
            matches!(
                parse_real_request(&serde_json::to_vec(&raw).expect("encodes")),
                Err(RequestError::Json { .. })
            ),
            "an unknown field must fail"
        );
        assert!(
            matches!(
                parse_real_request(b"{not json"),
                Err(RequestError::Json { .. })
            ),
            "non-JSON must fail"
        );
    }

    #[test]
    fn empty_envelope_fields_fail_before_any_tool_launch() {
        let (stages, sources, tools) = one_source();
        let mut raw: serde_json::Value =
            serde_json::from_slice(&skeleton(stages, sources, tools)).expect("skeleton decodes");
        raw["producer"] = json!("");
        assert_eq!(
            parse_real_request(&serde_json::to_vec(&raw).expect("encodes")).expect_err("producer"),
            RequestError::EmptyProducer
        );
        let (stages, sources, tools) = one_source();
        let mut raw: serde_json::Value =
            serde_json::from_slice(&skeleton(stages, sources, tools)).expect("skeleton decodes");
        raw["capability"] = json!("frobnicate");
        assert_eq!(
            parse_real_request(&serde_json::to_vec(&raw).expect("encodes"))
                .expect_err("capability"),
            RequestError::UnknownCapability {
                capability: "frobnicate".to_owned()
            }
        );
        assert_eq!(
            parse_skeleton(json!([]), json!([]), json!({})).expect_err("no stages"),
            RequestError::EmptyStages
        );
        assert_eq!(
            parse_skeleton(
                json!([{"tool": "", "classes": ["python"], "sources": ["a.py"]}]),
                json!([mapping("a.py", "exec/a.py")]),
                json!({"ruff": binary_tool("bin/ruff")}),
            )
            .expect_err("empty tool"),
            RequestError::EmptyToolId { stage: 0 }
        );
        assert_eq!(
            parse_skeleton(
                json!([{"tool": "ruff", "classes": [""], "sources": ["a.py"]}]),
                json!([mapping("a.py", "exec/a.py")]),
                json!({"ruff": binary_tool("bin/ruff")}),
            )
            .expect_err("empty class"),
            RequestError::EmptyClassIds { stage: 0 }
        );
        assert_eq!(
            parse_skeleton(
                json!([{"tool": "ruff", "classes": ["python"], "sources": []}]),
                json!([]),
                json!({"ruff": binary_tool("bin/ruff")}),
            )
            .expect_err("no sources"),
            RequestError::EmptyStageSources { stage: 0 }
        );
    }

    #[test]
    fn duplicates_and_shadows_fail() {
        let (stages, _sources, tools) = one_source();
        assert_eq!(
            parse_skeleton(
                stages.clone(),
                json!([
                    mapping("a.py", "exec/a.py"),
                    mapping("a.py", "exec/other.py")
                ]),
                tools.clone(),
            )
            .expect_err("duplicate source"),
            RequestError::DuplicateMapping {
                role: "source".to_owned(),
                path: "a.py".to_owned()
            }
        );
        let bytes = serde_json::to_vec(&json!({
            "schema_version": 1,
            "producer": "//pkg:target",
            "capability": "lint",
            "stages": stages,
            "sources": [mapping("a.py", "exec/a.py")],
            "siblings": [mapping("a.py", "exec/sib.py")],
            "resolves": [],
            "tools": tools,
        }))
        .expect("sibling shadow encodes");
        assert_eq!(
            parse_real_request(&bytes).expect_err("sibling shadow"),
            RequestError::ShadowedMapping {
                role: "sibling".to_owned(),
                path: "a.py".to_owned()
            }
        );
        let (stages, sources, tools) = one_source();
        let doubled = json!([stages[0].clone(), stages[0].clone()]);
        assert_eq!(
            parse_skeleton(doubled, sources, tools).expect_err("duplicate stage"),
            RequestError::DuplicateStage {
                tool: "ruff".to_owned()
            }
        );
    }

    #[test]
    fn stage_tools_must_be_declared_known_and_used() {
        let (stages, sources, _tools) = one_source();
        assert_eq!(
            parse_skeleton(stages, sources, json!({})).expect_err("no tool entry"),
            RequestError::MissingTool {
                tool: "ruff".to_owned()
            }
        );
        let (stages, sources, tools) = one_source();
        let mut with_extra: serde_json::Value = tools.clone();
        with_extra["ty"] = binary_tool("bin/ty");
        assert_eq!(
            parse_skeleton(stages, sources, with_extra).expect_err("unreferenced tool"),
            RequestError::UnreferencedTool {
                tool: "ty".to_owned()
            }
        );
        let (stages, sources, _tools) = one_source();
        assert_eq!(
            parse_skeleton(
                stages,
                sources,
                json!({"ruff": {"files": [], "env": [], "upstream": []}}),
            )
            .expect_err("no binary"),
            RequestError::ToolWithoutBinary {
                tool: "ruff".to_owned()
            }
        );
        assert_eq!(
            parse_skeleton(
                json!([{"tool": "frobnicate", "classes": ["python"], "sources": ["a.py"]}]),
                json!([mapping("a.py", "exec/a.py")]),
                json!({"frobnicate": binary_tool("bin/frobnicate")}),
            )
            .expect_err("unknown tool"),
            RequestError::UnknownTool {
                tool: "frobnicate".to_owned()
            }
        );
    }

    #[test]
    fn escaping_paths_are_rejected_at_every_mapping() {
        let (_stages, _sources, tools) = one_source();
        assert!(
            matches!(
                parse_skeleton(
                    json!([{"tool": "ruff", "classes": ["python"], "sources": ["../escape.py"]}]),
                    json!([mapping("../escape.py", "exec/escape.py")]),
                    tools.clone(),
                ),
                Err(RequestError::BadPath { role, .. }) if role == "stage source"
            ),
            "a '..' stage source must fail"
        );
        let (stages, _sources, tools) = one_source();
        assert!(
            matches!(
                parse_skeleton(
                    stages,
                    json!([mapping("/abs.py", "exec/abs.py")]),
                    tools,
                ),
                Err(RequestError::BadPath { role, .. }) if role == "source"
            ),
            "an absolute source must fail"
        );
        let (stages, sources, _tools) = one_source();
        assert!(
            matches!(
                parse_skeleton(
                    stages,
                    sources,
                    json!({"ruff": {
                        "binary": "bin/ruff",
                        "files": [{"mirror_rel": "a\\b.toml", "exec": "exec/b.toml"}],
                        "env": [],
                        "upstream": [],
                    }}),
                ),
                Err(RequestError::BadPath { role, .. }) if role == "tool file"
            ),
            "a backslash tool file must fail"
        );
        let (stages, sources, _tools) = one_source();
        assert!(
            matches!(
                parse_skeleton(
                    stages,
                    sources,
                    json!({"ruff": {
                        "binary": "",
                        "files": [],
                        "env": [],
                        "upstream": [],
                    }}),
                ),
                Err(RequestError::EmptyExec { .. })
            ),
            "an empty binary must fail"
        );
    }

    #[test]
    fn stage_sources_must_be_declared() {
        let (_stages, _sources, tools) = one_source();
        assert_eq!(
            parse_skeleton(
                json!([{"tool": "ruff", "classes": ["python"], "sources": ["absent.py"]}]),
                json!([mapping("a.py", "exec/a.py")]),
                tools,
            )
            .expect_err("absent source"),
            RequestError::MissingSource {
                path: "absent.py".to_owned()
            }
        );
    }

    #[test]
    fn tools_carry_allowlisted_path_helpers_only() {
        let doc = |tool: &str, helpers: serde_json::Value| {
            serde_json::to_vec(&json!({
                "schema_version": 1,
                "producer": "//pkg:target",
                "capability": "lint",
                "stages": [{"tool": tool, "classes": ["rst"], "sources": ["a.rst"]}],
                "sources": [{"workspace": "a.rst", "exec": "exec/a.rst"}],
                "siblings": [],
                "resolves": [],
                "tools": {tool: {"binary": "bin/t", "files": [], "env": [], "upstream": [], "path_tools": helpers}},
            }))
            .expect("encodes")
        };
        let helper = |name: &str, binary: &str, args: &[&str]| PathTool {
            name: name.to_owned(),
            binary: PathBuf::from(binary),
            args: args.iter().map(|arg| (*arg).to_owned()).collect(),
        };
        let ok = json!([{"name": "rst2html", "binary": "bin/rst2html"}]);
        let request = parse_real_request(&doc("vale", ok.clone())).expect("parses");
        assert_eq!(
            request.tools["vale"].path_tools,
            [helper("rst2html", "bin/rst2html", &[])]
        );
        assert!(
            parse_real_request(&doc("vale", json!([{"name": "ruby", "binary": "x"}]))).is_err()
        );
        assert!(
            parse_real_request(&doc("vale", json!([{"name": "rst2html", "binary": ""}]))).is_err()
        );
        assert!(parse_real_request(&doc("vale", json!([ok[0].clone(), ok[0].clone()]))).is_err());
        assert!(parse_real_request(&doc("chktex", ok.clone())).is_err());
        let diff = json!([{"name": "diff", "binary": "bin/dx", "args": ["buf_diff"]}]);
        assert_eq!(
            parse_real_request(&doc("buf", diff.clone()))
                .expect("parses")
                .tools["buf"]
                .path_tools,
            [helper("diff", "bin/dx", &["buf_diff"])]
        );
        assert!(parse_real_request(&doc("buf", ok)).is_err());
        assert!(parse_real_request(&doc("vale", diff)).is_err());
        let shellcheck = json!([{"name": "shellcheck", "binary": "bin/shellcheck"}]);
        assert_eq!(
            parse_real_request(&doc("actionlint", shellcheck.clone()))
                .expect("parses")
                .tools["actionlint"]
                .path_tools,
            [helper("shellcheck", "bin/shellcheck", &[])]
        );
        assert!(
            parse_real_request(&doc(
                "actionlint",
                json!([{"name": "diff", "binary": "bin/diff"}])
            ))
            .is_err()
        );
        assert!(parse_real_request(&doc("vale", shellcheck)).is_err());
    }

    #[test]
    fn tools_carry_a_dialect_and_reject_an_empty_one() {
        let doc = |dialect: &str| {
            serde_json::to_vec(&json!({
                "schema_version": 1,
                "producer": "//pkg:target",
                "capability": "format",
                "stages": [{"tool": "prettier", "classes": ["sql"], "sources": ["a.sql"]}],
                "sources": [{"workspace": "a.sql", "exec": "exec/a.sql"}],
                "siblings": [],
                "resolves": [],
                "tools": {"prettier": {"binary": "bin/prettier", "dialect": dialect, "files": [], "env": [], "upstream": []}},
            }))
            .expect("encodes")
        };
        let request = parse_real_request(&doc("postgresql")).expect("parses");
        assert_eq!(
            request.tools["prettier"].dialect.as_deref(),
            Some("postgresql")
        );
        assert!(parse_real_request(&doc("")).is_err());
    }

    #[test]
    fn tools_carry_args() {
        let doc = |tool: serde_json::Value| {
            serde_json::to_vec(&json!({
                "schema_version": 1,
                "producer": "//pkg:target",
                "capability": "lint",
                "stages": [{"tool": "clang_tidy", "classes": ["c"], "sources": ["a.c"]}],
                "sources": [{"workspace": "a.c", "exec": "exec/a.c"}],
                "siblings": [],
                "resolves": [],
                "tools": {"clang_tidy": tool},
            }))
            .expect("encodes")
        };
        let with = doc(
            json!({"binary": "bin/clang_tidy", "args": ["warning", "style"], "files": [], "env": [], "upstream": []}),
        );
        assert_eq!(
            parse_real_request(&with).expect("parses").tools["clang_tidy"].args,
            ["warning", "style"]
        );
        let without =
            doc(json!({"binary": "bin/clang_tidy", "files": [], "env": [], "upstream": []}));
        assert_eq!(
            parse_real_request(&without).expect("parses").tools["clang_tidy"].args,
            [] as [String; 0]
        );
    }

    #[test]
    fn tools_carry_search_paths() {
        let bytes = serde_json::to_vec(&json!({
            "schema_version": 1,
            "producer": "//pkg:target",
            "capability": "typecheck",
            "stages": [{"tool": "ty", "classes": ["python"], "sources": ["a.py"]}],
            "sources": [{"workspace": "a.py", "exec": "exec/a.py"}],
            "siblings": [],
            "resolves": [],
            "tools": {"ty": {"binary": "bin/ty", "files": [], "env": [], "upstream": [], "search_paths": ["wheels/click.install"]}},
        }))
        .expect("encodes");
        let request = parse_real_request(&bytes).expect("parses");
        assert_eq!(
            request.tools["ty"].search_paths,
            [PathBuf::from("wheels/click.install")]
        );
    }

    #[test]
    fn delegated_tools_carry_upstream_diagnostics_without_a_binary() {
        let bytes = serde_json::to_vec(&json!({
            "schema_version": 1,
            "producer": "//pkg:target",
            "capability": "lint",
            "stages": [{"tool": "clippy", "classes": ["rust"], "sources": ["src/lib.rs"]}],
            "sources": [{"workspace": "src/lib.rs", "exec": "exec/lib.rs"}],
            "siblings": [],
            "resolves": [],
            "tools": {"clippy": {"files": [], "env": [], "upstream": ["out/clippy.diag"]}},
        }))
        .expect("delegated encodes");
        let request = parse_real_request(&bytes).expect("delegated parses");
        let tool = &request.tools["clippy"];
        assert_eq!(tool.binary, None);
        assert_eq!(tool.upstream, [PathBuf::from("out/clippy.diag")]);
    }

    #[test]
    fn large_source_sets_parse_in_order() {
        let count = 3000;
        let names: Vec<String> = (0..count)
            .map(|index| format!("src/file{index:05}.py"))
            .collect();
        let mappings: Vec<serde_json::Value> = names
            .iter()
            .map(|name| mapping(name, &format!("exec/{name}")))
            .collect();
        let request = parse_skeleton(
            json!([stage("ruff", &["python"], &names)]),
            serde_json::Value::Array(mappings),
            json!({"ruff": binary_tool("bin/ruff")}),
        )
        .expect("large request parses");
        assert_eq!(request.stages[0].source_paths, names);
        assert_eq!(request.sources.len(), count);
        assert_eq!(request.sources[0].0, "src/file00000.py");
        assert_eq!(request.sources[count - 1].0, "src/file02999.py");
    }
}
