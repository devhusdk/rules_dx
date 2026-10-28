use std::path::{Path, PathBuf};

use quality_adapter::kubernetes_preflight::{self, Document, Problem};
use quality_adapter::parsers::{FileFinding, KubeconformTarget};
use quality_adapter::{commands, parsers, ColumnEncoding, Finding, TextPosition, ToolSeverity};

use super::*;

const DOCS_REL: &str = "dx-kubeconform/docs";

/// One preflight problem as an error finding on the owned file.
fn preflight_finding(file: &Path, problem: Problem) -> FileFinding {
    FileFinding {
        file: file.to_string_lossy().into_owned(),
        finding: Finding {
            tool_id: "kubeconform".to_owned(),
            rule_id: problem.rule.to_owned(),
            message: problem.message,
            severity: ToolSeverity::Error,
            start: TextPosition {
                line: problem.line,
                column: problem.column,
            },
            end: None,
            suggestions: Vec::new(),
            encoding: ColumnEncoding::Scalars,
            related: Vec::new(),
        },
    }
}

impl RealBackend {
    /// Preflights each owned file, then validates every document in one kubeconform run.
    pub(super) fn check_kubeconform(
        &self,
        tool_id: &str,
        tool: &RealTool,
        staged: &StagedScratch,
        config: Option<&Path>,
    ) -> Result<Vec<FileFinding>, RunnerError> {
        let scratch = &staged.scratch;
        let delegated = !tool.upstream_diagnostics.is_empty();
        let mut findings = Vec::new();
        let mut owned: Vec<(String, String, Vec<Document>)> = Vec::new();
        for (workspace, absolute) in &staged.pairs {
            let bytes = std::fs::read(absolute)
                .map_err(|err| execution(tool_id, format!("read {workspace}: {err}")))?;
            let checked = kubernetes_preflight::check_file(workspace, &bytes);
            if !delegated {
                findings.extend(
                    checked
                        .problems
                        .into_iter()
                        .map(|problem| preflight_finding(absolute, problem)),
                );
            }
            owned.push((
                workspace.clone(),
                absolute.to_string_lossy().into_owned(),
                checked.documents,
            ));
        }
        if delegated {
            let targets: Vec<KubeconformTarget<'_>> = owned
                .iter()
                .map(|(workspace, absolute, documents)| KubeconformTarget {
                    reported: workspace,
                    file: absolute,
                    documents,
                })
                .collect();
            for path in &tool.upstream_diagnostics {
                let bytes = std::fs::read(path)
                    .map_err(|err| execution(tool_id, format!("upstream diagnostics: {err}")))?;
                let code = i32::from(failing_upstream(&bytes));
                findings.extend(parsed(
                    tool_id,
                    parsers::parse_kubeconform(&bytes, &[], Some(code), &targets),
                )?);
            }
            return Ok(findings);
        }
        let Some(config_path) = config else {
            return Err(execution(tool_id, "kubeconform needs a config".to_owned()));
        };
        let config_bytes = std::fs::read(config_path)
            .map_err(|err| execution(tool_id, format!("read config: {err}")))?;
        let config = commands::kubeconform_config(config_path, &config_bytes)
            .map_err(|err| execution(tool_id, err))?;
        let docs_dir = scratch
            .resolve(Path::new(DOCS_REL))
            .map_err(|err| execution(tool_id, format!("scratch docs: {err}")))?;
        std::fs::create_dir_all(&docs_dir)
            .map_err(|err| execution(tool_id, format!("scratch docs: {err}")))?;
        let mut written: Vec<(String, usize, usize)> = Vec::new();
        for (index, (_, _, documents)) in owned.iter().enumerate() {
            for (at, document) in documents.iter().enumerate() {
                let path: PathBuf = docs_dir.join(format!("{}.yaml", written.len()));
                std::fs::write(&path, &document.text)
                    .map_err(|err| execution(tool_id, format!("write document: {err}")))?;
                written.push((path.to_string_lossy().into_owned(), index, at));
            }
        }
        if written.is_empty() {
            return Ok(findings);
        }
        let targets: Vec<KubeconformTarget<'_>> = written
            .iter()
            .map(|(reported, index, at)| {
                let (_, absolute, documents) = &owned[*index];
                KubeconformTarget {
                    reported,
                    file: absolute,
                    documents: std::slice::from_ref(&documents[*at]),
                }
            })
            .collect();
        let refs: Vec<&Path> = written
            .iter()
            .map(|(reported, _, _)| Path::new(reported.as_str()))
            .collect();
        let invocation = commands::kubeconform_check(&tool.binary, &config, &refs)
            .map_err(|err| execution(tool_id, err))?;
        let out = self.run(tool_id, tool, &invocation, scratch)?;
        findings.extend(parsed(
            tool_id,
            parsers::parse_kubeconform(&out.stdout, &out.stderr, out.code, &targets),
        )?);
        Ok(findings)
    }
}

/// Whether delegated kubeconform output names a failing resource.
fn failing_upstream(bytes: &[u8]) -> bool {
    let text = String::from_utf8_lossy(bytes);
    text.contains("\"statusInvalid\"") || text.contains("\"statusError\"")
}
