use std::collections::BTreeMap;
use std::path::Path;

use quality_adapter::api_spec_preflight::{self, Problem};
use quality_adapter::launch::ResolvedTool;
use quality_adapter::parsers::FileFinding;
use quality_adapter::{commands, parsers, ColumnEncoding, Finding, TextPosition, ToolSeverity};

use super::*;

/// One preflight problem as an error finding on the root it was found from.
fn preflight_finding(root: &Path, problem: Problem) -> FileFinding {
    FileFinding {
        file: root.to_string_lossy().into_owned(),
        finding: Finding {
            tool_id: "redocly".to_owned(),
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
    /// Lints each owned root on its own, after the offline preflight clears it.
    pub(super) fn check_redocly(
        &self,
        tool_id: &str,
        tool: &RealTool,
        staged: &StagedScratch,
        config: Option<&Path>,
    ) -> Result<Vec<FileFinding>, RunnerError> {
        let scratch = &staged.scratch;
        let Some(cfg) = config else {
            return Err(execution(tool_id, "redocly has no config".to_owned()));
        };
        let cfg_text = std::fs::read_to_string(cfg)
            .map_err(|err| execution(tool_id, format!("read config: {err}")))?;
        api_spec_preflight::check_config(&cfg_text).map_err(|err| execution(tool_id, err))?;
        let every: Vec<&StagedPair> = staged
            .pairs
            .iter()
            .chain(staged.sibling_pairs.iter())
            .chain(staged.resolve_pairs.iter())
            .collect();
        let mut docs = BTreeMap::new();
        for (workspace, absolute) in &every {
            let bytes = std::fs::read(absolute)
                .map_err(|err| execution(tool_id, format!("read {workspace}: {err}")))?;
            docs.insert(
                workspace.clone(),
                String::from_utf8_lossy(&bytes).into_owned(),
            );
        }
        let names: Vec<String> = every
            .iter()
            .map(|(_, absolute)| absolute.to_string_lossy().into_owned())
            .collect();
        let strs: Vec<&str> = names.iter().map(String::as_str).collect();
        let launch = ResolvedTool::javascript(tool_id, &tool.binary);
        let mut findings = Vec::new();
        for (workspace, absolute) in &staged.pairs {
            let problems = api_spec_preflight::check_root(workspace, &docs);
            if !problems.is_empty() {
                findings.extend(
                    problems
                        .into_iter()
                        .map(|problem| preflight_finding(absolute, problem)),
                );
                continue;
            }
            let invocation = commands::redocly_check(&launch, Path::new(workspace), cfg)
                .map_err(|err| execution(tool_id, err.to_string()))?;
            let out = self.run(tool_id, tool, &invocation, scratch)?;
            let blank = out.stdout.iter().all(u8::is_ascii_whitespace);
            if blank || !matches!(out.code, Some(0 | 1)) {
                let code = out
                    .code
                    .map_or_else(|| "signal".to_owned(), |code| code.to_string());
                let stderr = String::from_utf8_lossy(&out.stderr);
                return Err(execution(
                    tool_id,
                    format!("{workspace}: exit {code}: {}", stderr.trim()),
                ));
            }
            let cwd = scratch.root().join(&invocation.cwd_rel);
            findings.extend(parsed(
                tool_id,
                parsers::parse_redocly(
                    &out.stdout,
                    out.code,
                    &absolute.to_string_lossy(),
                    &cwd,
                    &strs,
                ),
            )?);
        }
        Ok(findings)
    }
}
