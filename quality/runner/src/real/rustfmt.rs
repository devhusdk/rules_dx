use std::collections::{BTreeMap, BTreeSet};

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

fn is_ident(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

fn declared_modules(text: &str) -> BTreeSet<&str> {
    let bytes = text.as_bytes();
    let mut names = BTreeSet::new();
    for (start, _) in text.match_indices("mod") {
        if start > 0 && is_ident(bytes[start - 1]) {
            continue;
        }
        let rest = &text[start + 3..];
        let name_part = rest.trim_start();
        if name_part.len() == rest.len() {
            continue;
        }
        let name_part = name_part.strip_prefix("r#").unwrap_or(name_part);
        let end = name_part
            .bytes()
            .position(|byte| !is_ident(byte))
            .unwrap_or(name_part.len());
        if end > 0 && name_part[end..].trim_start().starts_with(';') {
            names.insert(&name_part[..end]);
        }
    }
    names
}

fn path_file_names(text: &str) -> BTreeSet<&str> {
    let bytes = text.as_bytes();
    let mut names = BTreeSet::new();
    for (start, _) in text.match_indices("path") {
        if start > 0 && is_ident(bytes[start - 1]) {
            continue;
        }
        let Some(literal) = text[start + 4..]
            .trim_start()
            .strip_prefix('=')
            .map(str::trim_start)
            .and_then(|rest| rest.strip_prefix('"'))
            .and_then(|rest| rest.split_once('"'))
            .map(|(literal, _)| literal)
        else {
            continue;
        };
        let name = literal.rsplit(['/', '\\']).next().unwrap_or(literal);
        if !name.is_empty() {
            names.insert(name);
        }
    }
    names
}

fn module_name(path: &str) -> Option<&str> {
    let file = path.strip_suffix(".rs")?;
    let mut parts = file.rsplit(['/', '\\']);
    match parts.next()? {
        "mod" => parts.next(),
        stem => Some(stem),
    }
}

/// The known Rust files that may resolve a module declared in the fixed file.
pub(super) fn module_context<'a>(
    path: &str,
    text: &str,
    known: &'a BTreeSet<String>,
) -> BTreeSet<&'a str> {
    let modules = declared_modules(text);
    let targets = path_file_names(text);
    known
        .iter()
        .map(String::as_str)
        .filter(|candidate| *candidate != path)
        .filter(|candidate| {
            module_name(candidate).is_some_and(|name| modules.contains(name))
                || candidate
                    .rsplit(['/', '\\'])
                    .next()
                    .is_some_and(|name| name.ends_with(".rs") && targets.contains(name))
        })
        .collect()
}

fn failure(staged: &StagedScratch, out: &ChildOutput, detail: Option<String>) -> RunnerError {
    let root = staged.scratch.root().to_string_lossy().into_owned();
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
        Err(err) if out.code != Some(0) => {
            let detail = (matches!(err, ParseError::UnknownFile { .. })
                || !out.stdout.trim_ascii().is_empty())
            .then(|| err.to_string());
            Err(failure(staged, out, detail))
        }
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
        known: &BTreeSet<String>,
    ) -> Result<String, RunnerError> {
        let file = BTreeMap::from([(path.to_owned(), text.to_owned())]);
        let supporting: BTreeMap<String, String> = module_context(path, text, known)
            .into_iter()
            .map(|rel| (rel.to_owned(), String::new()))
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
                Ok(_) => Err(failure(&staged, &out, None)),
                Err(err) => Err(err),
            }
        };
        cleaned(TOOL_ID, staged.scratch, result)?
    }
}
