use quality_adapter::commands;
use quality_adapter::parsers::{self, FileFinding};

use super::*;

/// What a `br:` module reference means for the user; Bicep itself only says "not restored".
const UNRESTORED_HINT: &str =
    " (dx lints without restoring modules, so registry modules are not supported yet)";

impl RealBackend {
    /// Formats (stdout compared to the file) or lints (SARIF) each owned Bicep file in turn.
    pub(super) fn check_bicep(
        &self,
        tool_id: &str,
        tool: &RealTool,
        capability: &str,
        staged: &StagedScratch,
    ) -> Result<Vec<FileFinding>, RunnerError> {
        if !tool.upstream_diagnostics.is_empty() {
            return Err(execution(tool_id, "bicep has no delegated mode".to_owned()));
        }
        let scratch = &staged.scratch;
        let mut findings = Vec::new();
        if capability == "format" {
            for (workspace, absolute) in &staged.pairs {
                let invocation = commands::bicep_format_check(&tool.binary, absolute);
                let out = self.run(tool_id, tool, &invocation, scratch)?;
                let input = std::fs::read(absolute)
                    .map_err(|err| execution(tool_id, format!("read {workspace}: {err}")))?;
                let file = absolute.to_string_lossy();
                findings.extend(parsed(
                    tool_id,
                    parsers::parse_bicep_format(&out.stdout, &out.stderr, out.code, &input, &file),
                )?);
            }
            return Ok(findings);
        }
        let mut known: Vec<String> = staged
            .pairs
            .iter()
            .chain(staged.resolve_pairs.iter())
            .map(|(_, absolute)| absolute.to_string_lossy().into_owned())
            .collect();
        known.dedup();
        let known: Vec<&str> = known.iter().map(String::as_str).collect();
        for (workspace, absolute) in &staged.pairs {
            let invocation = commands::bicep_lint_check(&tool.binary, absolute);
            let out = self.run(tool_id, tool, &invocation, scratch)?;
            let reported = parsed(
                tool_id,
                parsers::parse_bicep_lint(&out.stdout, out.code, &known),
            )?;
            let own = absolute.to_string_lossy();
            let had_results = !reported.is_empty();
            let mut mine: Vec<FileFinding> = reported
                .into_iter()
                .filter(|found| found.file == own)
                .collect();
            if mine.is_empty() && had_results && out.code != Some(0) {
                return Err(execution(
                    tool_id,
                    format!("bicep lint failed in a module that {workspace} references"),
                ));
            }
            for found in &mut mine {
                if found.finding.message.contains("has not been restored") {
                    found.finding.message.push_str(UNRESTORED_HINT);
                }
            }
            findings.extend(mine);
        }
        Ok(findings)
    }
}
