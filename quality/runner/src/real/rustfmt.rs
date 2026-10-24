use std::collections::BTreeMap;

use quality_adapter::commands;
use quality_adapter::exec::ChildOutput;
use quality_adapter::parsers::{self, FileFinding};
use quality_adapter::ToolSeverity;

use super::*;

const TOOL_ID: &str = "rustfmt";

fn absolute_names(pairs: &[StagedPair]) -> Vec<String> {
    pairs
        .iter()
        .map(|(_, absolute)| absolute.to_string_lossy().into_owned())
        .collect()
}

fn failure(staged: &StagedScratch, out: &ChildOutput) -> RunnerError {
    let root = staged.scratch.root().to_string_lossy().into_owned();
    let stderr = String::from_utf8_lossy(&out.stderr)
        .trim()
        .replace(&format!("{root}/"), "")
        .replace(&format!("{root}\\"), "");
    let status = out
        .code
        .map_or_else(|| "a signal".to_owned(), |code| format!("exit {code}"));
    execution(TOOL_ID, format!("rustfmt failed with {status}: {stderr}"))
}

fn parse_staged(
    staged: &StagedScratch,
    out: &ChildOutput,
) -> Result<Vec<FileFinding>, RunnerError> {
    let names: Vec<String> = absolute_names(&staged.pairs)
        .into_iter()
        .chain(absolute_names(&staged.sibling_pairs))
        .chain(absolute_names(&staged.resolve_pairs))
        .collect();
    let known: Vec<&str> = names.iter().map(String::as_str).collect();
    match parsers::parse_rustfmt(&out.stdout, &out.stderr, out.code, &known) {
        Ok(found) => Ok(found),
        Err(_) if out.code != Some(0) => Err(failure(staged, out)),
        Err(err) => parsed(TOOL_ID, Err(err)),
    }
}

impl RealBackend {
    pub(super) fn check_rustfmt(
        &self,
        tool: &RealTool,
        staged: &StagedScratch,
    ) -> Result<Vec<FileFinding>, RunnerError> {
        let Some(cfg) = self.config_abs(TOOL_ID, tool, &staged.scratch)? else {
            return Err(execution(TOOL_ID, "rustfmt requires a config".to_owned()));
        };
        let refs: Vec<&Path> = staged
            .pairs
            .iter()
            .map(|(_, absolute)| absolute.as_path())
            .collect();
        let invocation = commands::rustfmt(
            &tool.binary,
            &refs,
            &cfg,
            Self::rustfmt_edition(tool)?,
            true,
        );
        let out = self.run(TOOL_ID, tool, &invocation, &staged.scratch)?;
        let owned = absolute_names(&staged.pairs);
        let mut kept = Vec::new();
        for found in parse_staged(staged, &out)? {
            if owned.contains(&found.file) {
                if !kept.contains(&found) {
                    kept.push(found);
                }
            } else if found.finding.severity == ToolSeverity::Error {
                let workspace = staged
                    .sibling_pairs
                    .iter()
                    .chain(&staged.resolve_pairs)
                    .find(|(_, absolute)| absolute.as_os_str() == found.file.as_str())
                    .map_or(found.file.as_str(), |(workspace, _)| workspace.as_str());
                return Err(execution(
                    TOOL_ID,
                    format!(
                        "rustfmt cannot parse supporting file {workspace}: {}",
                        found.finding.message
                    ),
                ));
            }
        }
        Ok(kept)
    }

    pub(super) fn fix_rustfmt(
        &self,
        tool: &RealTool,
        path: &str,
        text: &str,
        context: &BTreeMap<String, String>,
    ) -> Result<String, RunnerError> {
        let file = BTreeMap::from([(path.to_owned(), text.to_owned())]);
        let supporting: BTreeMap<String, String> = context
            .iter()
            .filter(|(rel, _)| rel.as_str() != path)
            .map(|(rel, body)| (rel.clone(), body.clone()))
            .collect();
        let staged = self.stage_scratch(TOOL_ID, tool, &file, &supporting, &BTreeMap::new())?;
        let Some(cfg) = self.config_abs(TOOL_ID, tool, &staged.scratch)? else {
            return Err(execution(TOOL_ID, "rustfmt requires a config".to_owned()));
        };
        let Some((_, absolute)) = staged.pairs.first().cloned() else {
            return cleaned(
                TOOL_ID,
                staged.scratch,
                Err(execution(TOOL_ID, format!("rustfmt did not stage {path}"))),
            )?;
        };
        let invocation = commands::rustfmt(
            &tool.binary,
            &[absolute.as_path()],
            &cfg,
            Self::rustfmt_edition(tool)?,
            false,
        );
        let out = self.run(TOOL_ID, tool, &invocation, &staged.scratch)?;
        let result = if out.code == Some(0) {
            Self::reread_fixed(TOOL_ID, &absolute)
        } else {
            match parse_staged(&staged, &out) {
                Ok(found)
                    if !found.is_empty()
                        && found
                            .iter()
                            .all(|found| found.finding.severity == ToolSeverity::Error) =>
                {
                    Ok(text.to_owned())
                }
                Ok(_) => Err(failure(&staged, &out)),
                Err(err) => Err(err),
            }
        };
        cleaned(TOOL_ID, staged.scratch, result)?
    }
}
