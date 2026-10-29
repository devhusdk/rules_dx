use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use quality_adapter::parsers::FileFinding;
use quality_adapter::{commands, parsers, sqlfluff_config, sqlfluff_preflight};

use super::*;

/// One sqlfluff run: a config and the files it applies to.
pub(super) struct SqlfluffRun {
    pub config: PathBuf,
    pub files: Vec<PathBuf>,
}

/// Preflights the bound config and every nested `.sqlfluff`, then groups `files` by the chain of
/// nested configs above them. Without nested configs every file runs under the bound config.
pub(super) fn plan_sqlfluff(
    tool_id: &str,
    tool: &RealTool,
    scratch: &Scratch,
    bound: &Path,
    files: &[&Path],
) -> Result<Vec<SqlfluffRun>, RunnerError> {
    let text = std::fs::read_to_string(bound)
        .map_err(|err| execution(tool_id, format!("read config: {err}")))?;
    sqlfluff_preflight::check_config(&text).map_err(|err| execution(tool_id, err))?;
    let mut nested = Vec::new();
    for (rel, bytes) in &tool.tool_files {
        let basename = rel.rsplit(['/', '\\']).next().unwrap_or(rel);
        if basename != ".sqlfluff" || tool.config_rel.as_deref() == Some(rel.as_str()) {
            continue;
        }
        let nested_text = String::from_utf8(bytes.clone())
            .map_err(|err| execution(tool_id, format!("{rel}: not UTF-8: {err}")))?;
        sqlfluff_preflight::check_nested_config(&nested_text)
            .map_err(|err| execution(tool_id, format!("{rel}: {err}")))?;
        nested.push((rel.replace('\\', "/"), nested_text));
    }
    if nested.is_empty() || files.is_empty() {
        if files.is_empty() {
            return Ok(Vec::new());
        }
        return Ok(vec![SqlfluffRun {
            config: bound.to_path_buf(),
            files: files.iter().map(|file| file.to_path_buf()).collect(),
        }]);
    }
    let mut by_rel: BTreeMap<String, &Path> = BTreeMap::new();
    for file in files {
        let rel = file.strip_prefix(scratch.root()).map_err(|_| {
            execution(
                tool_id,
                format!("{} is outside the scratch", file.display()),
            )
        })?;
        by_rel.insert(rel.to_string_lossy().replace('\\', "/"), file);
    }
    let rels: Vec<String> = by_rel.keys().cloned().collect();
    let groups = sqlfluff_config::group(&text, &nested, &rels)
        .map_err(|err| execution(tool_id, format!("merge nested sqlfluff configs: {err}")))?;
    let dir = scratch.root().join(".dx-sqlfluff");
    std::fs::create_dir_all(&dir)
        .map_err(|err| execution(tool_id, format!("create {}: {err}", dir.display())))?;
    let mut runs = Vec::with_capacity(groups.len());
    for (index, group) in groups.iter().enumerate() {
        let config = match &group.merged {
            None => bound.to_path_buf(),
            Some(merged) => {
                sqlfluff_preflight::check_config(merged).map_err(|err| {
                    execution(
                        tool_id,
                        format!(
                            "{} merged with {}: {err}",
                            bound.display(),
                            group.layers.join(", ")
                        ),
                    )
                })?;
                let path = dir.join(format!("{index}.sqlfluff"));
                std::fs::write(&path, merged).map_err(|err| {
                    execution(tool_id, format!("write {}: {err}", path.display()))
                })?;
                path
            }
        };
        runs.push(SqlfluffRun {
            config,
            files: group
                .files
                .iter()
                .map(|rel| by_rel[rel].to_path_buf())
                .collect(),
        });
    }
    Ok(runs)
}

impl RealBackend {
    /// Preflights the bound and nested configs, then lints every owned file, one sqlfluff run per
    /// distinct config chain. A format
    /// check lints with only the rules `sqlfluff format` applies.
    pub(super) fn check_sqlfluff(
        &self,
        tool_id: &str,
        tool: &RealTool,
        staged: &StagedScratch,
        config: &Path,
        format: bool,
    ) -> Result<Vec<FileFinding>, RunnerError> {
        if !tool.upstream_diagnostics.is_empty() {
            return Err(execution(
                tool_id,
                "sqlfluff has no delegated mode".to_owned(),
            ));
        }
        let files: Vec<&Path> = staged
            .pairs
            .iter()
            .map(|(_, absolute)| absolute.as_path())
            .collect();
        let runs = plan_sqlfluff(tool_id, tool, &staged.scratch, config, &files)?;
        for file in &files {
            // Unreadable or non-UTF-8 files are left to sqlfluff to report.
            if let Ok(text) = std::fs::read_to_string(file) {
                sqlfluff_preflight::check_inline_config(&text)
                    .map_err(|err| execution(tool_id, format!("{err} (in {})", file.display())))?;
            }
        }
        let mut findings = Vec::new();
        for run in &runs {
            let run_files: Vec<&Path> = run.files.iter().map(PathBuf::as_path).collect();
            let invocation = if format {
                commands::sqlfluff_format_check(&tool.binary, &run_files, &run.config)
            } else {
                commands::sqlfluff_check(&tool.binary, &run_files, &run.config)
            };
            let out = self.run(tool_id, tool, &invocation, &staged.scratch)?;
            let names: Vec<String> = run_files
                .iter()
                .map(|path| path.to_string_lossy().into_owned())
                .collect();
            let strs: Vec<&str> = names.iter().map(String::as_str).collect();
            findings.extend(parsed(
                tool_id,
                parsers::parse_sqlfluff(&out.stdout, &out.stderr, out.code, &strs),
            )?);
        }
        Ok(findings)
    }
}
