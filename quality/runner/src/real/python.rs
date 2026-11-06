use quality_adapter::{commands, parsers};

use super::check::ToolDef;
use super::*;

/// Python linters, formatters and notebook checks.
pub(super) const TOOLS: &[ToolDef] = &[
    ToolDef {
        id: "ruff",
        run: |backend, call| {
            let tool_id = call.tool_id;
            let tool = call.tool;
            let capability = call.capability;
            let scratch = &call.staged.scratch;
            let pairs = &call.staged.pairs;
            {
                let config_ref = call.config;
                let format = capability == "format";
                let notebooks = RealBackend::read_notebooks(tool_id, pairs)?;
                let notebook_refs: Vec<(&str, &str)> = notebooks
                    .iter()
                    .map(|(path, text)| (path.as_str(), text.as_str()))
                    .collect();
                backend.check_files(
                    tool_id,
                    tool,
                    scratch,
                    pairs,
                    |files| {
                        Ok(if format {
                            commands::ruff_format_check(&tool.binary, files, config_ref)
                        } else {
                            commands::ruff_check(&tool.binary, files, config_ref)
                        })
                    },
                    |out, names| {
                        if format {
                            parsed(
                                tool_id,
                                parsers::parse_ruff_format(
                                    &out.stdout,
                                    out.code,
                                    names,
                                    &notebook_refs,
                                ),
                            )
                        } else {
                            parsed(
                                tool_id,
                                parsers::parse_ruff(&out.stdout, out.code, names, &notebook_refs),
                            )
                        }
                    },
                )
            }
        },
    },
    ToolDef {
        id: "notebook_outputs",
        run: |backend, call| {
            let tool_id = call.tool_id;
            let tool = call.tool;
            let scratch = &call.staged.scratch;
            let pairs = &call.staged.pairs;
            {
                RealBackend::read_notebooks_any(tool_id, pairs)?;
                backend.check_files(
                    tool_id,
                    tool,
                    scratch,
                    pairs,
                    |files| Ok(commands::notebook_outputs_check(&tool.binary, files)),
                    |out, names| {
                        parsed(
                            tool_id,
                            parsers::parse_notebook_outputs(&out.stdout, out.code, names),
                        )
                    },
                )
            }
        },
    },
    ToolDef {
        id: "ty",
        run: |backend, call| {
            let tool_id = call.tool_id;
            let tool = call.tool;
            let scratch = &call.staged.scratch;
            let pairs = &call.staged.pairs;
            let resolve_pairs = &call.staged.resolve_pairs;
            {
                // ty cannot read notebooks, so each one is checked as a script of its code
                // cells written beside the staged copy; findings map back to notebook cells.
                let notebooks = RealBackend::read_notebooks(tool_id, pairs)?;
                let mut scripts: Vec<(String, String, quality_notebook::PythonScript)> = Vec::new();
                let mut checked_pairs: Vec<(String, PathBuf)> = Vec::new();
                for (workspace, absolute) in pairs {
                    if absolute.extension().and_then(|ext| ext.to_str()) != Some("ipynb") {
                        checked_pairs.push((workspace.clone(), absolute.clone()));
                        continue;
                    }
                    let key = absolute.to_string_lossy().into_owned();
                    let text = notebooks
                        .iter()
                        .find(|(path, _)| *path == key)
                        .map(|(_, text)| text.as_str())
                        .unwrap_or_default();
                    let script = quality_notebook::Notebook::parse(text)
                        .map_err(|err| execution(tool_id, format!("{workspace}: {err}")))?
                        .python_script()
                        .ok_or_else(|| {
                            execution(tool_id, format!("{workspace}: unreadable cell source"))
                        })?;
                    let mut script_path = absolute.clone().into_os_string();
                    script_path.push(".py");
                    let script_path = PathBuf::from(script_path);
                    std::fs::write(&script_path, &script.source).map_err(|err| {
                        execution(tool_id, format!("{workspace}: write cell script: {err}"))
                    })?;
                    checked_pairs.push((format!("{workspace}.py"), script_path));
                    scripts.push((workspace.clone(), text.to_owned(), script));
                }
                let ty_refs: Vec<&Path> = checked_pairs
                    .iter()
                    .map(|(_, absolute)| absolute.as_path())
                    .collect();
                let mut dirs: Vec<PathBuf> = Vec::new();
                for (_, absolute) in checked_pairs.iter().chain(resolve_pairs.iter()) {
                    if let Some(parent) = absolute.parent() {
                        if !dirs.contains(&parent.to_path_buf()) {
                            dirs.push(parent.to_path_buf());
                        }
                    }
                }
                for extra in &tool.search_paths {
                    if !dirs.contains(extra) {
                        dirs.push(extra.clone());
                    }
                }
                let dir_refs: Vec<&Path> = dirs.iter().map(|dir| dir.as_path()).collect();
                let invocation = commands::ty_check(&tool.binary, &ty_refs, &dir_refs, call.config);
                let out = backend.run(tool_id, tool, &invocation, scratch)?;
                let workspaces: Vec<&str> = checked_pairs
                    .iter()
                    .chain(resolve_pairs.iter())
                    .map(|(workspace, _)| workspace.as_str())
                    .collect();
                let mut findings = parsed(
                    tool_id,
                    parsers::parse_ty(&out.stdout, out.code, &workspaces),
                )?;
                let checked: std::collections::BTreeSet<&str> = checked_pairs
                    .iter()
                    .map(|(workspace, _)| workspace.as_str())
                    .collect();
                findings.retain(|found| checked.contains(found.file.as_str()));
                for found in &mut findings {
                    let notebook = found
                        .file
                        .strip_suffix(".py")
                        .and_then(|name| scripts.iter().find(|(ws, _, _)| ws == name));
                    if let Some((workspace, text, script)) = notebook {
                        let parsed_nb = quality_notebook::Notebook::parse(text)
                            .map_err(|err| execution(tool_id, format!("{workspace}: {err}")))?;
                        let unplaced = || RunnerError::UnplaceableFinding {
                            tool_id: tool_id.to_owned(),
                            detail: format!("{workspace}: finding outside a code cell"),
                        };
                        let start = found.finding.start;
                        let (cell, row) = script.cell_row(start.line).ok_or_else(unplaced)?;
                        let place = |position: quality_adapter::TextPosition| {
                            let (end_cell, end_row) =
                                script.cell_row(position.line).ok_or_else(unplaced)?;
                            // A span running into a later cell ends at the end of this one.
                            let (row, column) = if end_cell == cell {
                                (end_row, position.column)
                            } else {
                                (u64::MAX / 2, 1)
                            };
                            parsed_nb
                                .locate(cell, row, column)
                                .map(|(line, column)| quality_adapter::TextPosition {
                                    line,
                                    column,
                                })
                                .ok_or_else(unplaced)
                        };
                        found.finding.start = place(start)?;
                        if let Some(end) = found.finding.end {
                            found.finding.end = Some(place(end)?);
                        }
                        found.finding.message =
                            format!("cell {cell}, line {row}: {}", found.finding.message);
                        found.file = reanchor(tool_id, pairs, workspace)?
                            .to_string_lossy()
                            .into_owned();
                        continue;
                    }
                    let absolute = reanchor(tool_id, &checked_pairs, &found.file)?;
                    found.file = absolute.to_string_lossy().into_owned();
                }
                Ok(findings)
            }
        },
    },
    ToolDef {
        id: "pydoclint",
        run: |backend, call| {
            let tool_id = call.tool_id;
            let tool = call.tool;
            let scratch = &call.staged.scratch;
            let pairs = &call.staged.pairs;
            backend.check_files(
                tool_id,
                tool,
                scratch,
                pairs,
                |files| Ok(commands::pydoclint_check(&tool.binary, files)),
                |out, names| {
                    parsed(
                        tool_id,
                        parsers::parse_pydoclint(&out.stderr, out.code, names),
                    )
                },
            )
        },
    },
];

impl RealBackend {
    pub(super) fn read_notebooks(
        tool_id: &str,
        pairs: &[(String, PathBuf)],
    ) -> Result<Vec<(String, String)>, RunnerError> {
        Self::read_notebooks_with(tool_id, pairs, quality_notebook::preflight)
    }

    /// Like `read_notebooks`, for tools that accept any kernel language.
    pub(super) fn read_notebooks_any(
        tool_id: &str,
        pairs: &[(String, PathBuf)],
    ) -> Result<Vec<(String, String)>, RunnerError> {
        Self::read_notebooks_with(tool_id, pairs, quality_notebook::validate)
    }

    fn read_notebooks_with(
        tool_id: &str,
        pairs: &[(String, PathBuf)],
        validate: fn(&str) -> Result<quality_notebook::Info, quality_notebook::NotebookError>,
    ) -> Result<Vec<(String, String)>, RunnerError> {
        let mut notebooks = Vec::new();
        for (workspace, absolute) in pairs {
            if absolute.extension().and_then(|ext| ext.to_str()) != Some("ipynb") {
                continue;
            }
            let bytes = std::fs::read(absolute)
                .map_err(|err| execution(tool_id, format!("{workspace}: read notebook: {err}")))?;
            let text = String::from_utf8(bytes).map_err(|err| {
                execution(
                    tool_id,
                    format!("{workspace}: notebook is not UTF-8: {err}"),
                )
            })?;
            validate(&text).map_err(|err| execution(tool_id, format!("{workspace}: {err}")))?;
            notebooks.push((absolute.to_string_lossy().into_owned(), text));
        }
        Ok(notebooks)
    }
}
