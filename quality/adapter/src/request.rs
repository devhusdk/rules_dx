use std::collections::{BTreeMap, BTreeSet};

pub const REQUEST_SCHEMA: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PathMapping {
    pub path: String,
    pub exec: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RequestStage {
    pub tool: String,
    pub classes: Vec<String>,
    pub sources: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RequestToolFile {
    pub mirror: String,
    pub exec: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RequestTool {
    pub binary: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edition: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub env: Vec<(String, String)>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub files: Vec<RequestToolFile>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RequestUpstream {
    pub tool: String,
    pub exec: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ActionRequest {
    pub schema: u32,
    pub producer: String,
    pub capability: String,
    pub real: bool,
    pub stages: Vec<RequestStage>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sources: Vec<PathMapping>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub siblings: Vec<PathMapping>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub resolves: Vec<PathMapping>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub tools: BTreeMap<String, RequestTool>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub upstream: Vec<RequestUpstream>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RequestError {
    #[error("malformed request: {detail}")]
    Malformed { detail: String },
    #[error("unsupported request schema {got}: want 1")]
    BadSchema { got: u32 },
    #[error("empty producer: want a non-empty producer")]
    EmptyProducer,
    #[error("empty capability: want a non-empty capability")]
    EmptyCapability,
    #[error("empty stages: want at least one stage")]
    EmptyStages,
    #[error("empty tool id at stage {stage}")]
    EmptyToolId { stage: usize },
    #[error("empty class ids at stage {stage}")]
    EmptyClassIds { stage: usize },
    #[error("empty stage sources at stage {stage}")]
    EmptyStageSources { stage: usize },
    #[error("unknown tool {tool:?}: declare it under tools")]
    UnknownTool { tool: String },
    #[error("duplicate source path {path:?}")]
    DuplicateSource { path: String },
    #[error("duplicate sibling path {path:?}")]
    DuplicateSibling { path: String },
    #[error("sibling {path:?} shadows a checked source")]
    SiblingShadowsSource { path: String },
    #[error("duplicate resolve path {path:?}")]
    DuplicateResolve { path: String },
    #[error("resolve {path:?} shadows a checked source or sibling")]
    ResolveShadows { path: String },
    #[error("invalid {role} path {path:?}: want a relative workspace path without '..'")]
    BadPath { role: String, path: String },
    #[error("empty exec path for {role} {path:?}")]
    EmptyExec { role: String, path: String },
    #[error("empty binary for tool {tool:?}")]
    EmptyToolBinary { tool: String },
    #[error("empty config for tool {tool:?}")]
    EmptyToolConfig { tool: String },
    #[error("empty edition for tool {tool:?}")]
    EmptyToolEdition { tool: String },
    #[error("empty env key for tool {tool:?}")]
    EmptyEnvKey { tool: String },
    #[error("duplicate tool file {mirror:?} for tool {tool:?}")]
    DuplicateToolFile { tool: String, mirror: String },
    #[error("empty upstream tool id at entry {entry}")]
    EmptyUpstreamTool { entry: usize },
    #[error("empty upstream exec path for tool {tool:?}")]
    EmptyUpstreamExec { tool: String },
}

fn check_logical_path(role: &str, path: &str) -> Result<(), RequestError> {
    if path.is_empty() || path.starts_with('/') || path.split('/').any(|part| part == "..") {
        return Err(RequestError::BadPath {
            role: role.to_owned(),
            path: path.to_owned(),
        });
    }
    Ok(())
}

fn check_mapping(role: &str, mapping: &PathMapping) -> Result<(), RequestError> {
    check_logical_path(role, &mapping.path)?;
    if mapping.exec.is_empty() {
        return Err(RequestError::EmptyExec {
            role: role.to_owned(),
            path: mapping.path.clone(),
        });
    }
    Ok(())
}

fn check_stages(request: &ActionRequest) -> Result<(), RequestError> {
    if request.stages.is_empty() {
        return Err(RequestError::EmptyStages);
    }
    for (index, stage) in request.stages.iter().enumerate() {
        if stage.tool.is_empty() {
            return Err(RequestError::EmptyToolId { stage: index });
        }
        if !request.tools.contains_key(&stage.tool) {
            return Err(RequestError::UnknownTool {
                tool: stage.tool.clone(),
            });
        }
        if stage.classes.is_empty() {
            return Err(RequestError::EmptyClassIds { stage: index });
        }
        if stage.sources.is_empty() {
            return Err(RequestError::EmptyStageSources { stage: index });
        }
        for path in &stage.sources {
            check_logical_path("stage source", path)?;
        }
    }
    Ok(())
}

fn check_mappings(request: &ActionRequest) -> Result<(), RequestError> {
    let mut sources = BTreeSet::new();
    for mapping in &request.sources {
        check_mapping("source", mapping)?;
        if !sources.insert(mapping.path.clone()) {
            return Err(RequestError::DuplicateSource {
                path: mapping.path.clone(),
            });
        }
    }
    let mut siblings = BTreeSet::new();
    for mapping in &request.siblings {
        check_mapping("sibling", mapping)?;
        if sources.contains(&mapping.path) {
            return Err(RequestError::SiblingShadowsSource {
                path: mapping.path.clone(),
            });
        }
        if !siblings.insert(mapping.path.clone()) {
            return Err(RequestError::DuplicateSibling {
                path: mapping.path.clone(),
            });
        }
    }
    let mut resolves = BTreeSet::new();
    for mapping in &request.resolves {
        check_mapping("resolve", mapping)?;
        if sources.contains(&mapping.path) || siblings.contains(&mapping.path) {
            return Err(RequestError::ResolveShadows {
                path: mapping.path.clone(),
            });
        }
        if !resolves.insert(mapping.path.clone()) {
            return Err(RequestError::DuplicateResolve {
                path: mapping.path.clone(),
            });
        }
    }
    Ok(())
}

fn check_tools(request: &ActionRequest) -> Result<(), RequestError> {
    for (tool, spec) in &request.tools {
        if spec.binary.is_empty() {
            return Err(RequestError::EmptyToolBinary { tool: tool.clone() });
        }
        if spec.config.as_deref() == Some("") {
            return Err(RequestError::EmptyToolConfig { tool: tool.clone() });
        }
        if spec.edition.as_deref() == Some("") {
            return Err(RequestError::EmptyToolEdition { tool: tool.clone() });
        }
        for (key, _) in &spec.env {
            if key.is_empty() {
                return Err(RequestError::EmptyEnvKey { tool: tool.clone() });
            }
        }
        let mut mirrors = BTreeSet::new();
        for file in &spec.files {
            check_logical_path("tool file", &file.mirror)?;
            if file.exec.is_empty() {
                return Err(RequestError::EmptyExec {
                    role: "tool file".to_owned(),
                    path: file.mirror.clone(),
                });
            }
            if !mirrors.insert(file.mirror.clone()) {
                return Err(RequestError::DuplicateToolFile {
                    tool: tool.clone(),
                    mirror: file.mirror.clone(),
                });
            }
        }
    }
    for (entry, item) in request.upstream.iter().enumerate() {
        if item.tool.is_empty() {
            return Err(RequestError::EmptyUpstreamTool { entry });
        }
        if item.exec.is_empty() {
            return Err(RequestError::EmptyUpstreamExec {
                tool: item.tool.clone(),
            });
        }
    }
    Ok(())
}

fn validate(request: &ActionRequest) -> Result<(), RequestError> {
    if request.producer.is_empty() {
        return Err(RequestError::EmptyProducer);
    }
    if request.capability.is_empty() {
        return Err(RequestError::EmptyCapability);
    }
    check_stages(request)?;
    check_mappings(request)?;
    check_tools(request)?;
    Ok(())
}

pub fn parse_request(bytes: &[u8]) -> Result<ActionRequest, RequestError> {
    let request: ActionRequest =
        serde_json::from_slice(bytes).map_err(|error| RequestError::Malformed {
            detail: error.to_string(),
        })?;
    if request.schema != REQUEST_SCHEMA {
        return Err(RequestError::BadSchema {
            got: request.schema,
        });
    }
    validate(&request)?;
    Ok(request)
}

pub fn serialize_request(request: &ActionRequest) -> Result<Vec<u8>, RequestError> {
    serde_json::to_vec(request).map_err(|error| RequestError::Malformed {
        detail: error.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const WEIRD_SOURCES: &[&str] = &[
        "pkg/a,b.py",
        "pkg/c;d.py",
        "pkg/e=f.py",
        "pkg/g h.py",
        "pkg/ünïcodé.py",
    ];

    fn tool_with(binary: &str) -> RequestTool {
        RequestTool {
            binary: binary.to_owned(),
            config: Some("cfg/ruff.toml".to_owned()),
            edition: None,
            env: vec![("KEY".to_owned(), "a=b;c,d".to_owned())],
            files: vec![RequestToolFile {
                mirror: "cfg/ruff.toml".to_owned(),
                exec: "bazel-out/cfg/ruff.toml".to_owned(),
            }],
        }
    }

    fn request_with(sources: &[&str]) -> ActionRequest {
        let mut tools = BTreeMap::new();
        tools.insert("ruff".to_owned(), tool_with("bin/ruff"));
        ActionRequest {
            schema: REQUEST_SCHEMA,
            producer: "//pkg:target".to_owned(),
            capability: "lint".to_owned(),
            real: true,
            stages: vec![RequestStage {
                tool: "ruff".to_owned(),
                classes: vec!["python".to_owned()],
                sources: sources.iter().map(|path| (*path).to_owned()).collect(),
            }],
            sources: sources
                .iter()
                .map(|path| PathMapping {
                    path: (*path).to_owned(),
                    exec: "exec/".to_owned() + path,
                })
                .collect(),
            siblings: vec![PathMapping {
                path: "pkg/README.md".to_owned(),
                exec: "exec/pkg/README.md".to_owned(),
            }],
            resolves: vec![],
            tools,
            upstream: vec![RequestUpstream {
                tool: "clippy".to_owned(),
                exec: "exec/clippy.diag".to_owned(),
            }],
        }
    }

    fn parse(text: &str) -> Result<ActionRequest, RequestError> {
        parse_request(text.as_bytes())
    }

    fn malformed(request: ActionRequest) -> String {
        match parse_request(&serialize_request(&request).expect("serializable")) {
            Ok(_) => panic!("request must be rejected"),
            Err(error) => error.to_string(),
        }
    }

    #[test]
    fn round_trip_preserves_delimiter_filenames_verbatim() {
        let request = request_with(WEIRD_SOURCES);
        let bytes = serialize_request(&request).expect("serializable");
        let parsed = parse_request(&bytes).expect("parseable");
        assert_eq!(parsed, request);
        assert_eq!(parsed.stages[0].sources, WEIRD_SOURCES);
        let again = serialize_request(&parsed).expect("serializable");
        assert_eq!(again, bytes);
    }

    #[test]
    fn large_requests_parse_with_compact_output() {
        let sources: Vec<String> = (0..2000).map(|index| "pkg/f".to_owned() + &index.to_string() + ".py").collect();
        let refs: Vec<&str> = sources.iter().map(String::as_str).collect();
        let request = request_with(&refs);
        let bytes = serialize_request(&request).expect("serializable");
        let parsed = parse_request(&bytes).expect("parseable");
        assert_eq!(parsed.stages[0].sources.len(), 2000);
        assert_eq!(parsed.sources.len(), 2000);
        assert_eq!(serialize_request(&parsed).expect("serializable"), bytes);
    }

    #[test]
    fn schema_and_envelope_fields_are_required() {
        let error = parse("{\"producer\":\"//p:t\",\"capability\":\"lint\",\"real\":true,\"stages\":[],\"tools\":{}}")
            .expect_err("missing schema");
        assert!(
            error.to_string().starts_with("malformed request: missing field `schema`"),
            "{error}"
        );
        let mut versioned = request_with(&["pkg/a.py"]);
        versioned.schema = 0;
        assert_eq!(
            parse_request(&serialize_request(&versioned).expect("serializable"))
                .expect_err("schema 0"),
            RequestError::BadSchema { got: 0 }
        );
        versioned.schema = 2;
        assert_eq!(
            parse_request(&serialize_request(&versioned).expect("serializable"))
                .expect_err("schema 2"),
            RequestError::BadSchema { got: 2 }
        );
        assert!(parse("{not json}").expect_err("syntax").to_string().starts_with("malformed request: "));
        let mut request = request_with(&["pkg/a.py"]);
        request.producer.clear();
        assert_eq!(
            parse_request(&serialize_request(&request).expect("serializable")).expect_err("producer"),
            RequestError::EmptyProducer
        );
        request = request_with(&["pkg/a.py"]);
        request.capability.clear();
        assert_eq!(
            parse_request(&serialize_request(&request).expect("serializable")).expect_err("capability"),
            RequestError::EmptyCapability
        );
    }

    #[test]
    fn stage_shapes_reject_empties_and_unknown_tools() {
        let mut request = request_with(&["pkg/a.py"]);
        request.stages.clear();
        assert_eq!(
            parse_request(&serialize_request(&request).expect("serializable")).expect_err("stages"),
            RequestError::EmptyStages
        );
        request = request_with(&["pkg/a.py"]);
        request.stages[0].tool.clear();
        assert_eq!(
            parse_request(&serialize_request(&request).expect("serializable")).expect_err("tool"),
            RequestError::EmptyToolId { stage: 0 }
        );
        request = request_with(&["pkg/a.py"]);
        request.stages[0].tool = "ty".to_owned();
        assert_eq!(
            parse_request(&serialize_request(&request).expect("serializable")).expect_err("unknown"),
            RequestError::UnknownTool { tool: "ty".to_owned() }
        );
        request = request_with(&["pkg/a.py"]);
        request.stages[0].classes.clear();
        assert_eq!(
            parse_request(&serialize_request(&request).expect("serializable")).expect_err("classes"),
            RequestError::EmptyClassIds { stage: 0 }
        );
        request = request_with(&["pkg/a.py"]);
        request.stages[0].sources.clear();
        assert_eq!(
            parse_request(&serialize_request(&request).expect("serializable")).expect_err("sources"),
            RequestError::EmptyStageSources { stage: 0 }
        );
    }

    #[test]
    fn mapping_roles_reject_duplicates_shadows_and_escapes() {
        let mut request = request_with(&["pkg/a.py"]);
        request.sources.push(PathMapping {
            path: "pkg/a.py".to_owned(),
            exec: "exec/other".to_owned(),
        });
        assert_eq!(
            parse_request(&serialize_request(&request).expect("serializable")).expect_err("dup source"),
            RequestError::DuplicateSource { path: "pkg/a.py".to_owned() }
        );
        request = request_with(&["pkg/a.py"]);
        request.siblings.push(PathMapping {
            path: "pkg/a.py".to_owned(),
            exec: "exec/other".to_owned(),
        });
        assert_eq!(
            parse_request(&serialize_request(&request).expect("serializable")).expect_err("shadow"),
            RequestError::SiblingShadowsSource { path: "pkg/a.py".to_owned() }
        );
        request = request_with(&["pkg/a.py"]);
        request.siblings.push(PathMapping {
            path: "pkg/README.md".to_owned(),
            exec: "exec/other".to_owned(),
        });
        assert_eq!(
            parse_request(&serialize_request(&request).expect("serializable")).expect_err("dup sibling"),
            RequestError::DuplicateSibling { path: "pkg/README.md".to_owned() }
        );
        request = request_with(&["pkg/a.py"]);
        request.resolves.push(PathMapping {
            path: "pkg/README.md".to_owned(),
            exec: "exec/other".to_owned(),
        });
        assert_eq!(
            parse_request(&serialize_request(&request).expect("serializable")).expect_err("resolve shadow"),
            RequestError::ResolveShadows { path: "pkg/README.md".to_owned() }
        );
        request = request_with(&["pkg/a.py"]);
        request.resolves.push(PathMapping {
            path: "pkg/dep.py".to_owned(),
            exec: "exec/dep".to_owned(),
        });
        request.resolves.push(PathMapping {
            path: "pkg/dep.py".to_owned(),
            exec: "exec/other".to_owned(),
        });
        assert_eq!(
            parse_request(&serialize_request(&request).expect("serializable")).expect_err("dup resolve"),
            RequestError::DuplicateResolve { path: "pkg/dep.py".to_owned() }
        );
    }

    #[test]
    fn every_mapping_role_rejects_escaping_and_empty_paths() {
        for bad in ["", "/abs.py", "pkg/../../evil.py", ".."] {
            let mut request = request_with(&["pkg/a.py"]);
            request.sources.push(PathMapping {
                path: bad.to_owned(),
                exec: "exec/x".to_owned(),
            });
            assert_eq!(
                parse_request(&serialize_request(&request).expect("serializable"))
                    .expect_err("bad source must fail"),
                RequestError::BadPath {
                    role: "source".to_owned(),
                    path: bad.to_owned(),
                }
            );
            request = request_with(&["pkg/a.py"]);
            request.siblings = vec![PathMapping {
                path: bad.to_owned(),
                exec: "exec/x".to_owned(),
            }];
            assert_eq!(
                parse_request(&serialize_request(&request).expect("serializable"))
                    .expect_err("bad sibling must fail"),
                RequestError::BadPath {
                    role: "sibling".to_owned(),
                    path: bad.to_owned(),
                }
            );
            request = request_with(&["pkg/a.py"]);
            request.resolves = vec![PathMapping {
                path: bad.to_owned(),
                exec: "exec/x".to_owned(),
            }];
            assert_eq!(
                parse_request(&serialize_request(&request).expect("serializable"))
                    .expect_err("bad resolve must fail"),
                RequestError::BadPath {
                    role: "resolve".to_owned(),
                    path: bad.to_owned(),
                }
            );
        }
        let mut request = request_with(&["pkg/a.py"]);
        request.stages[0].sources = vec!["pkg/../../evil.py".to_owned()];
        assert_eq!(
            parse_request(&serialize_request(&request).expect("serializable")).expect_err("stage escape"),
            RequestError::BadPath {
                role: "stage source".to_owned(),
                path: "pkg/../../evil.py".to_owned(),
            }
        );
        request = request_with(&["pkg/a.py"]);
        request.sources[0].exec.clear();
        assert_eq!(
            parse_request(&serialize_request(&request).expect("serializable")).expect_err("empty exec"),
            RequestError::EmptyExec {
                role: "source".to_owned(),
                path: "pkg/a.py".to_owned(),
            }
        );
    }

    #[test]
    fn tool_entries_reject_empties_duplicates_and_bad_mirrors() {
        let mut request = request_with(&["pkg/a.py"]);
        request.tools["ruff"].binary.clear();
        assert_eq!(
            parse_request(&serialize_request(&request).expect("serializable")).expect_err("binary"),
            RequestError::EmptyToolBinary { tool: "ruff".to_owned() }
        );
        request = request_with(&["pkg/a.py"]);
        request.tools["ruff"].config = Some(String::new());
        assert_eq!(
            parse_request(&serialize_request(&request).expect("serializable")).expect_err("config"),
            RequestError::EmptyToolConfig { tool: "ruff".to_owned() }
        );
        request = request_with(&["pkg/a.py"]);
        request.tools["ruff"].edition = Some(String::new());
        assert_eq!(
            parse_request(&serialize_request(&request).expect("serializable")).expect_err("edition"),
            RequestError::EmptyToolEdition { tool: "ruff".to_owned() }
        );
        request = request_with(&["pkg/a.py"]);
        request.tools["ruff"].env.push((String::new(), "1".to_owned()));
        assert_eq!(
            parse_request(&serialize_request(&request).expect("serializable")).expect_err("env key"),
            RequestError::EmptyEnvKey { tool: "ruff".to_owned() }
        );
        request = request_with(&["pkg/a.py"]);
        request.tools["ruff"].files.push(RequestToolFile {
            mirror: "cfg/ruff.toml".to_owned(),
            exec: "exec/other".to_owned(),
        });
        assert_eq!(
            parse_request(&serialize_request(&request).expect("serializable")).expect_err("dup mirror"),
            RequestError::DuplicateToolFile {
                tool: "ruff".to_owned(),
                mirror: "cfg/ruff.toml".to_owned(),
            }
        );
        request = request_with(&["pkg/a.py"]);
        request.tools["ruff"].files[0].mirror = "../evil.toml".to_owned();
        assert_eq!(
            parse_request(&serialize_request(&request).expect("serializable")).expect_err("mirror escape"),
            RequestError::BadPath {
                role: "tool file".to_owned(),
                path: "../evil.toml".to_owned(),
            }
        );
        request = request_with(&["pkg/a.py"]);
        request.tools["ruff"].files[0].exec.clear();
        assert_eq!(
            parse_request(&serialize_request(&request).expect("serializable")).expect_err("file exec"),
            RequestError::EmptyExec {
                role: "tool file".to_owned(),
                path: "cfg/ruff.toml".to_owned(),
            }
        );
    }

    #[test]
    fn upstream_entries_reject_empties() {
        let mut request = request_with(&["pkg/a.py"]);
        request.upstream[0].tool.clear();
        assert_eq!(
            parse_request(&serialize_request(&request).expect("serializable")).expect_err("tool"),
            RequestError::EmptyUpstreamTool { entry: 0 }
        );
        request = request_with(&["pkg/a.py"]);
        request.upstream[0].exec.clear();
        assert_eq!(
            parse_request(&serialize_request(&request).expect("serializable")).expect_err("exec"),
            RequestError::EmptyUpstreamExec { tool: "clippy".to_owned() }
        );
    }

    #[test]
    fn error_display_names_the_offending_value() {
        assert_eq!(
            RequestError::BadSchema { got: 9 }.to_string(),
            "unsupported request schema 9: want 1"
        );
        assert_eq!(
            RequestError::UnknownTool { tool: "ty".to_owned() }.to_string(),
            "unknown tool \"ty\": declare it under tools"
        );
        assert_eq!(
            RequestError::BadPath {
                role: "source".to_owned(),
                path: "../x".to_owned(),
            }
            .to_string(),
            "invalid source path \"../x\": want a relative workspace path without '..'"
        );
        let mut request = request_with(&["pkg/a.py"]);
        request.stages.clear();
        assert_eq!(malformed(request), "empty stages: want at least one stage");
    }
}
