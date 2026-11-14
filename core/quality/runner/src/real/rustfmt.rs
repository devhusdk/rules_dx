use quality_adapter::ToolSeverity;
use quality_adapter::commands;
use quality_adapter::exec::ChildOutput;
use quality_adapter::parsers::{self, FileFinding};

use super::check::ToolDef;
use super::real_fix::Outcome;
use super::*;

const TOOL_ID: &str = "rustfmt";

fn absolute_names(pairs: &[StagedPair]) -> Vec<String> {
    pairs
        .iter()
        .map(|(_, absolute)| absolute.to_string_lossy().into_owned())
        .collect()
}

/// The spelling rustfmt reports for a staged file: symlinks and short names resolved, no `\\?\` prefix.
fn resolved_name(name: &str) -> Option<String> {
    let canonical = std::fs::canonicalize(name).ok()?.to_string_lossy().into_owned();
    let plain = match canonical.strip_prefix(r"\\?\") {
        Some(rest) if rest.as_bytes().get(1) == Some(&b':') => rest.to_owned(),
        _ => canonical,
    };
    (plain != name).then_some(plain)
}

fn failure(root: &Path, out: &ChildOutput, detail: Option<String>) -> RunnerError {
    let root = root.to_string_lossy().into_owned();
    let stderr = String::from_utf8_lossy(&out.stderr).trim().to_owned();
    let message = detail
        .into_iter()
        .chain(Some(stderr).filter(|stderr| !stderr.is_empty()))
        .collect::<Vec<_>>()
        .join("; ")
        .replace(&format!("{root}/"), "")
        .replace(&format!("{root}\\"), "");
    let status = out
        .code
        .map_or_else(|| "a signal".to_owned(), |code| format!("exit {code}"));
    execution(TOOL_ID, format!("rustfmt failed with {status}: {message}"))
}

fn staged_names(staged: &StagedScratch) -> Vec<String> {
    absolute_names(&staged.pairs)
        .into_iter()
        .chain(absolute_names(&staged.sibling_pairs))
        .chain(absolute_names(&staged.resolve_pairs))
        .collect()
}

fn parse_staged(
    root: &Path,
    names: &[String],
    out: &ChildOutput,
) -> Result<Vec<FileFinding>, RunnerError> {
    let resolved: Vec<(String, &String)> = names
        .iter()
        .filter_map(|name| Some((resolved_name(name)?, name)))
        .collect();
    let known: Vec<&str> = names
        .iter()
        .map(String::as_str)
        .chain(resolved.iter().map(|(resolved, _)| resolved.as_str()))
        .collect();
    match parsers::parse_rustfmt(&out.stdout, &out.stderr, out.code, &known).map(|found| {
        found
            .into_iter()
            .map(|mut finding| {
                if let Some((_, original)) = resolved
                    .iter()
                    .find(|(resolved, _)| *resolved == finding.file)
                {
                    finding.file.clone_from(original);
                }
                finding
            })
            .collect::<Vec<_>>()
    }) {
        Ok(found) => Ok(found),
        Err(err) if out.code != Some(0) => {
            let detail = (matches!(err, ParseError::UnknownFile { .. })
                || !out.stdout.trim_ascii().is_empty())
            .then(|| err.to_string());
            Err(failure(root, out, detail))
        }
        Err(err) => parsed(TOOL_ID, Err(err)),
    }
}

pub(super) const TOOL: ToolDef = ToolDef {
    id: "rustfmt",
    run: |backend, call| backend.check_rustfmt(call.tool, call.staged),
};

impl RealBackend {
    pub(super) fn check_rustfmt(
        &self,
        tool: &RealTool,
        staged: &StagedScratch,
    ) -> Result<Vec<FileFinding>, RunnerError> {
        let Some(cfg) = self.config_abs(TOOL_ID, tool, &staged.scratch)? else {
            return Err(missing_tier_base(TOOL_ID));
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
        for found in parse_staged(staged.scratch.root(), &staged_names(staged), &out)? {
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

    pub(super) fn rustfmt_fix_invocation(
        &self,
        tool: &RealTool,
        refs: &[&Path],
        scratch: &Scratch,
    ) -> Result<Invocation, RunnerError> {
        let Some(cfg) = self.config_abs(TOOL_ID, tool, scratch)? else {
            return Err(missing_tier_base(TOOL_ID));
        };
        Ok(commands::rustfmt(
            &tool.binary,
            refs,
            &cfg,
            Self::rustfmt_edition(tool)?,
            false,
        ))
    }
}

/// Judges one rustfmt fix spawn: error-only parse findings keep the input.
pub(super) fn fix_outcome(root: &Path, names: &[String], out: &ChildOutput) -> Outcome {
    if out.code == Some(0) {
        return Outcome::Fixed;
    }
    match parse_staged(root, names, out) {
        Ok(found)
            if !found.is_empty()
                && found
                    .iter()
                    .all(|found| found.finding.severity == ToolSeverity::Error) =>
        {
            Outcome::Kept
        }
        Ok(_) => Outcome::Failed(failure(root, out, None)),
        Err(err) => Outcome::Failed(err),
    }
}
