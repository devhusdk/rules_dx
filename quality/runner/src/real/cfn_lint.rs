use std::path::Path;

use quality_adapter::cloudformation_preflight::{self, Problem};
use quality_adapter::parsers::FileFinding;
use quality_adapter::{commands, parsers, ColumnEncoding, Finding, TextPosition, ToolSeverity};

use super::*;

const EMPTY_CONFIG_REL: &str = "dx-cfn-lint/empty.yaml";

/// One preflight problem as an error finding on the owned file.
fn preflight_finding(file: &Path, problem: Problem) -> FileFinding {
    FileFinding {
        file: file.to_string_lossy().into_owned(),
        finding: Finding {
            tool_id: "cfn_lint".to_owned(),
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
    /// Preflights the config and each owned file, then lints the templates left in one cfn-lint run.
    pub(super) fn check_cfn_lint(
        &self,
        tool_id: &str,
        tool: &RealTool,
        staged: &StagedScratch,
        config: Option<&Path>,
    ) -> Result<Vec<FileFinding>, RunnerError> {
        if !tool.upstream_diagnostics.is_empty() {
            return Err(execution(
                tool_id,
                "cfn_lint has no delegated mode".to_owned(),
            ));
        }
        let scratch = &staged.scratch;
        let config_path = match config {
            Some(path) => {
                let text = std::fs::read_to_string(path)
                    .map_err(|err| execution(tool_id, format!("read config: {err}")))?;
                cloudformation_preflight::check_config(&text)
                    .map_err(|err| execution(tool_id, err))?;
                path.to_path_buf()
            }
            None => {
                let empty = scratch
                    .resolve(Path::new(EMPTY_CONFIG_REL))
                    .map_err(|err| execution(tool_id, format!("scratch config: {err}")))?;
                if let Some(parent) = empty.parent() {
                    std::fs::create_dir_all(parent)
                        .map_err(|err| execution(tool_id, format!("scratch config: {err}")))?;
                }
                std::fs::write(&empty, b"")
                    .map_err(|err| execution(tool_id, format!("scratch config: {err}")))?;
                empty
            }
        };
        let mut findings = Vec::new();
        let mut linted: Vec<&Path> = Vec::new();
        for (workspace, absolute) in &staged.pairs {
            let bytes = std::fs::read(absolute)
                .map_err(|err| execution(tool_id, format!("read {workspace}: {err}")))?;
            match cloudformation_preflight::check_template(workspace, &bytes) {
                Some(problem) => findings.push(preflight_finding(absolute, problem)),
                None => linted.push(absolute.as_path()),
            }
        }
        if linted.is_empty() {
            return Ok(findings);
        }
        for path in linted.iter().copied().chain([config_path.as_path()]) {
            if path.to_string_lossy().contains(['*', '?', '[']) {
                return Err(execution(
                    tool_id,
                    format!(
                        "cfn-lint would glob the path {}; move the workspace out of a directory with * ? [ in its name",
                        path.display()
                    ),
                ));
            }
        }
        let invocation = commands::cfn_lint_check(&tool.binary, &linted, &config_path);
        let out = self.run(tool_id, tool, &invocation, scratch)?;
        let names: Vec<String> = linted
            .iter()
            .map(|path| path.to_string_lossy().into_owned())
            .collect();
        let strs: Vec<&str> = names.iter().map(String::as_str).collect();
        findings.extend(parsed(
            tool_id,
            parsers::parse_cfn_lint(&out.stdout, &out.stderr, out.code, &strs),
        )?);
        Ok(findings)
    }
}
