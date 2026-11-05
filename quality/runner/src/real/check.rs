use std::path::{Path, PathBuf};

use quality_adapter::parsers::FileFinding;
use quality_adapter::{commands, parsers};

use super::*;

/// The bundled standard phpcs runs when the consumer binds no config.
const PHPCS_STANDARD: &str = "PSR12";

/// The rule level PHPStan runs at when the consumer binds no config.
const PHPSTAN_LEVEL: u8 = 5;

/// What a tool module needs to launch one check.
pub(super) struct CheckCall<'a> {
    pub tool_id: &'a str,
    pub tool: &'a RealTool,
    pub capability: &'a str,
    pub staged: &'a StagedScratch,
    pub config: Option<&'a Path>,
}

/// A tool that owns its launch and parse in its own module.
pub(super) struct ToolDef {
    pub id: &'static str,
    pub run: fn(&RealBackend, &CheckCall) -> Result<Vec<FileFinding>, RunnerError>,
}

pub(super) const TOOL_DEFS: &[&ToolDef] = &[
    &super::bicep::TOOL,
    &super::cfn_lint::TOOL,
    &super::cmakelint::TOOL,
    &super::helm::TOOL,
    &super::kubeconform::TOOL,
    &super::periphery::TOOL,
    &super::redocly::TOOL,
    &super::rustfmt::TOOL,
    &super::sqlfluff::TOOL,
];

impl RealBackend {
    pub(super) fn run_check(
        &self,
        tool_id: &str,
        tool: &RealTool,
        capability: &str,
        staged: &StagedScratch,
    ) -> Result<Vec<FileFinding>, RunnerError> {
        let pairs = &staged.pairs;
        let resolve_pairs = &staged.resolve_pairs;
        let scratch = &staged.scratch;
        let refs: Vec<&Path> = pairs
            .iter()
            .map(|(_, absolute)| absolute.as_path())
            .collect();
        let names: Vec<String> = pairs
            .iter()
            .map(|(_, absolute)| absolute.to_string_lossy().into_owned())
            .collect();
        let strs: Vec<&str> = names.iter().map(String::as_str).collect();
        let config = self.config_abs(tool_id, tool, scratch)?;
        let cwd_rel = Self::cwd_rel(tool_id, tool.config_rel.as_deref());
        if let Some(def) = TOOL_DEFS
            .iter()
            .copied()
            .chain(super::delegated::TOOLS.iter())
            .chain(super::native::TOOLS.iter())
            .chain(super::schema::TOOLS.iter())
            .chain(super::single_run::TOOLS.iter())
            .chain(super::web::TOOLS.iter())
            .find(|def| def.id == tool_id)
        {
            let call = CheckCall {
                tool_id,
                tool,
                capability,
                staged,
                config: config.as_deref(),
            };
            return (def.run)(self, &call);
        }
        match tool_id {
            "ruff" => {
                let config_ref = config.as_deref();
                let format = capability == "format";
                let notebooks = Self::read_notebooks(tool_id, pairs)?;
                let notebook_refs: Vec<(&str, &str)> = notebooks
                    .iter()
                    .map(|(path, text)| (path.as_str(), text.as_str()))
                    .collect();
                self.check_files(
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
            "notebook_outputs" => {
                Self::read_notebooks_any(tool_id, pairs)?;
                self.check_files(
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
            "ty" => {
                // ty cannot read notebooks, so each one is checked as a script of its code
                // cells written beside the staged copy; findings map back to notebook cells.
                let notebooks = Self::read_notebooks(tool_id, pairs)?;
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
                let invocation = commands::ty_check(&tool.binary, &ty_refs, &dir_refs);
                let out = self.run(tool_id, tool, &invocation, scratch)?;
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
            "pydoclint" => self.check_files(
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
            ),
            "vulture" => {
                let roots = unused_roots(tool_id, tool, pairs)?;
                self.check_files(
                    tool_id,
                    tool,
                    scratch,
                    pairs,
                    |files| Ok(commands::vulture_check(&tool.binary, files)),
                    |out, names| {
                        parsed(
                            tool_id,
                            parsers::parse_vulture_unused(&out.stdout, out.code, names, &roots),
                        )
                    },
                )
            }
            "deadcode" => {
                let workspace: Vec<(String, PathBuf)> = pairs
                    .iter()
                    .map(|(name, _)| (name.clone(), PathBuf::from(name)))
                    .collect();
                let roots = unused_roots(tool_id, tool, &workspace)?;
                let Some((_, deadcode)) =
                    tool.path_tools.iter().find(|(name, _)| name == "deadcode")
                else {
                    return Err(execution(
                        tool_id,
                        "deadcode binary is not staged".to_owned(),
                    ));
                };
                let invocation = commands::deadcode_check(&tool.binary, deadcode);
                let out = self.run(tool_id, tool, &invocation, scratch)?;
                let names: Vec<&str> = pairs.iter().map(|(name, _)| name.as_str()).collect();
                let mut findings = parsed(
                    tool_id,
                    parsers::parse_deadcode(&out.stdout, out.code, &names, &roots),
                )?;
                for found in &mut findings {
                    let absolute = reanchor(tool_id, pairs, &found.file)?;
                    found.file = absolute.to_string_lossy().into_owned();
                }
                Ok(findings)
            }
            "flake8" => self.check_files(
                tool_id,
                tool,
                scratch,
                pairs,
                |files| Ok(commands::flake8_check(&tool.binary, files)),
                |out, names| parsed(tool_id, parsers::parse_flake8(&out.stdout, out.code, names)),
            ),
            "pylint" => {
                let invocation = commands::pylint_check(&tool.binary, &refs);
                let out = self.run(tool_id, tool, &invocation, scratch)?;
                let workspaces: Vec<&str> = pairs
                    .iter()
                    .map(|(workspace, _)| workspace.as_str())
                    .collect();
                let mut findings = parsed(
                    tool_id,
                    parsers::parse_pylint(&out.stdout, out.code, &workspaces),
                )?;
                for found in &mut findings {
                    let absolute = reanchor(tool_id, pairs, &found.file)?;
                    found.file = absolute.to_string_lossy().into_owned();
                }
                Ok(findings)
            }
            "scalafmt" => {
                let invocation = commands::scalafmt_check(&tool.binary, &refs, config.as_deref());
                let out = self.run(tool_id, tool, &invocation, scratch)?;
                let cwd = scratch.root().join(&cwd_rel);
                parsed(
                    tool_id,
                    parsers::parse_scalafmt(&out.stdout, &out.stderr, out.code, &strs, &cwd),
                )
            }
            "scalafix" => {
                let Some(cfg) = config.as_deref() else {
                    return Err(execution(tool_id, "scalafix requires a config".to_owned()));
                };
                let classpath = Self::scalafix_classpath(tool_id, tool, scratch)?;
                let jars: Vec<&Path> = classpath.iter().map(PathBuf::as_path).collect();
                let context = commands::ScalafixContext {
                    config: cfg,
                    sourceroot: scratch.root(),
                    classpath: &jars,
                };
                let invocation = commands::scalafix_check(&tool.binary, &refs, &context)
                    .map_err(|err| execution(tool_id, err.to_string()))?;
                let out = self.run(tool_id, tool, &invocation, scratch)?;
                parsed(
                    tool_id,
                    parsers::parse_scalafix(&out.stdout, &out.stderr, out.code, &strs),
                )
            }
            "csharpier" => {
                let invocation = commands::csharpier_check(&tool.binary, &refs, config.as_deref());
                let out = self.run(tool_id, tool, &invocation, scratch)?;
                parsed(
                    tool_id,
                    parsers::parse_csharpier(&out.stderr, out.code, &strs),
                )
            }
            "fantomas" => {
                let invocation = commands::fantomas_check(&tool.binary, &refs);
                let out = self.run(tool_id, tool, &invocation, scratch)?;
                let workspaces: Vec<&str> = pairs
                    .iter()
                    .map(|(workspace, _)| workspace.as_str())
                    .collect();
                let mut findings = parsed(
                    tool_id,
                    parsers::parse_fantomas(&out.stdout, out.code, &workspaces, scratch.root()),
                )?;
                for found in &mut findings {
                    let absolute = reanchor(tool_id, pairs, &found.file)?;
                    found.file = absolute.to_string_lossy().into_owned();
                }
                Ok(findings)
            }
            "fsharplint" => {
                if !tool.upstream_diagnostics.is_empty() {
                    self.check_fsharplint_delegated(tool_id, tool, pairs)
                } else {
                    let inputs = Self::fsharplint_context(tool_id, tool, scratch, pairs)?;
                    let compile: Vec<&Path> = inputs.compile.iter().map(PathBuf::as_path).collect();
                    let references: Vec<&Path> = inputs.refs.iter().map(PathBuf::as_path).collect();
                    let context = commands::FsharplintContext {
                        compile: &compile,
                        refs: &references,
                        defines: &inputs.defines,
                        langversion: &inputs.langversion,
                    };
                    let invocation = commands::fsharplint_check(
                        &tool.binary,
                        &refs,
                        &context,
                        config.as_deref(),
                    );
                    let out = self.run(tool_id, tool, &invocation, scratch)?;
                    parsed(
                        tool_id,
                        parsers::parse_fsharplint(&out.stdout, out.code, &strs),
                    )
                }
            }
            "google_java_format" => {
                let invocation = commands::google_java_format_check(&tool.binary, &refs);
                let out = self.run(tool_id, tool, &invocation, scratch)?;
                parsed(
                    tool_id,
                    parsers::parse_google_java_format(&out.stdout, out.code, &strs),
                )
            }
            "ktfmt" => {
                let invocation = commands::ktfmt_check(&tool.binary, &refs);
                let out = self.run(tool_id, tool, &invocation, scratch)?;
                parsed(tool_id, parsers::parse_ktfmt(&out.stdout, out.code, &strs))
            }
            "checkstyle" => {
                let (invocation, tab_width) = match config.as_ref() {
                    Some(cfg) => {
                        let bytes = std::fs::read(cfg).map_err(|err| {
                            execution(
                                tool_id,
                                format!("cannot read checkstyle config {}: {err}", cfg.display()),
                            )
                        })?;
                        (
                            commands::checkstyle_check(&tool.binary, &refs, cfg),
                            parsers::checkstyle_tab_width(&bytes),
                        )
                    }
                    None => {
                        return Err(execution(
                            tool_id,
                            "checkstyle requires a config".to_owned(),
                        ));
                    }
                };
                let out = self.run(tool_id, tool, &invocation, scratch)?;
                parsed(
                    tool_id,
                    parsers::parse_checkstyle(&out.stdout, out.code, &strs, tab_width),
                )
            }
            "pmd" => {
                let Some(ruleset) = config.as_deref() else {
                    return Err(execution(tool_id, "pmd needs a ruleset".to_owned()));
                };
                let invocation = commands::pmd_check(&tool.binary, &refs, ruleset);
                let out = self.run(tool_id, tool, &invocation, scratch)?;
                parsed(tool_id, parsers::parse_pmd(&out.stdout, out.code, &strs))
            }
            "spotbugs" => {
                let staged = |prefix: &str| -> Vec<PathBuf> {
                    tool.tool_files
                        .iter()
                        .filter(|(rel, _)| rel.starts_with(prefix) && rel.ends_with(".jar"))
                        .map(|(rel, _)| scratch.root().join(rel))
                        .collect()
                };
                let owned = staged("dx-spotbugs/owned/");
                let auxiliary = staged("dx-spotbugs/aux/");
                let targets: Vec<&Path> = owned.iter().map(PathBuf::as_path).collect();
                let aux_refs: Vec<&Path> = auxiliary.iter().map(PathBuf::as_path).collect();
                let invocation = commands::spotbugs_check(
                    &tool.binary,
                    &targets,
                    &aux_refs,
                    tool.edition.as_deref().unwrap_or("default"),
                    tool.dialect.as_deref().unwrap_or("medium"),
                )
                .map_err(|err| execution(tool_id, err.to_string()))?;
                let out = self.run(tool_id, tool, &invocation, scratch)?;
                parsed(
                    tool_id,
                    parsers::parse_spotbugs(&out.stdout, out.code, &strs),
                )
            }
            "dialyzer" => {
                let staged = |prefix: &str| -> Vec<PathBuf> {
                    let mut rels: Vec<&String> = tool
                        .tool_files
                        .iter()
                        .map(|(rel, _)| rel)
                        .filter(|rel| rel.starts_with(prefix))
                        .collect();
                    rels.sort();
                    rels.into_iter()
                        .map(|rel| scratch.root().join(rel))
                        .collect()
                };
                let plts = staged("dx-dialyzer/plt/");
                let beams: Vec<PathBuf> = staged("dx-dialyzer/owned/")
                    .into_iter()
                    .chain(staged("dx-dialyzer/aux/"))
                    .filter(|path| path.extension().is_some_and(|ext| ext == "beam"))
                    .collect();
                // A language's own compiler beams (Elixir's) go on the code path, so dialyzer can
                // read the debug info of that language's beams.
                let code_dir = scratch.root().join("dx-dialyzer/pa");
                let code_dirs: Vec<&Path> = if staged("dx-dialyzer/pa/").is_empty() {
                    Vec::new()
                } else {
                    vec![code_dir.as_path()]
                };
                let owned_beams = staged("dx-dialyzer/owned/")
                    .iter()
                    .any(|path| path.extension().is_some_and(|ext| ext == "beam"));
                if !owned_beams {
                    // An app with no sources has an empty ebin directory; there is nothing to analyse.
                    return Ok(Vec::new());
                }
                let plt_refs: Vec<&Path> = plts.iter().map(PathBuf::as_path).collect();
                let beam_refs: Vec<&Path> = beams.iter().map(PathBuf::as_path).collect();
                let name_refs: Vec<&str> = pairs.iter().map(|(rel, _)| rel.as_str()).collect();
                let invocation =
                    commands::dialyzer_check(&tool.binary, &code_dirs, &plt_refs, &beam_refs);
                let out = self.run(tool_id, tool, &invocation, scratch)?;
                let mut findings = parsed(
                    tool_id,
                    parsers::parse_dialyzer(&out.stdout, out.code, &name_refs),
                )?;
                // The parser names owned files by workspace path; later stages key them by the
                // staged absolute path.
                for found in &mut findings {
                    if let Some((_, absolute)) = pairs.iter().find(|(rel, _)| *rel == found.file) {
                        found.file = absolute.to_string_lossy().into_owned();
                    }
                }
                Ok(findings)
            }
            "xref" => {
                let staged = |prefix: &str| -> Vec<PathBuf> {
                    let mut rels: Vec<&String> = tool
                        .tool_files
                        .iter()
                        .map(|(rel, _)| rel)
                        .filter(|rel| rel.starts_with(prefix) && rel.ends_with(".beam"))
                        .collect();
                    rels.sort();
                    rels.into_iter()
                        .map(|rel| scratch.root().join(rel))
                        .collect()
                };
                let owned = staged("dx-xref/owned/");
                if owned.is_empty() {
                    return Ok(Vec::new());
                }
                let deps = staged("dx-xref/aux/");
                let code_dir = scratch.root().join("dx-xref/pa");
                let code_dirs: Vec<&Path> = if staged("dx-xref/pa/").is_empty() {
                    Vec::new()
                } else {
                    vec![code_dir.as_path()]
                };
                let script = scratch.root().join("dx-xref/dx_xref.escript");
                let dep_refs: Vec<&Path> = deps.iter().map(PathBuf::as_path).collect();
                let owned_refs: Vec<&Path> = owned.iter().map(PathBuf::as_path).collect();
                let name_refs: Vec<&str> = pairs.iter().map(|(rel, _)| rel.as_str()).collect();
                let invocation =
                    commands::xref_check(&tool.binary, &script, &code_dirs, &dep_refs, &owned_refs);
                let out = self.run(tool_id, tool, &invocation, scratch)?;
                let mut findings = parsed(
                    tool_id,
                    parsers::parse_xref(&out.stdout, out.code, &name_refs),
                )?;
                for found in &mut findings {
                    if let Some((_, absolute)) = pairs.iter().find(|(rel, _)| *rel == found.file) {
                        found.file = absolute.to_string_lossy().into_owned();
                    }
                }
                Ok(findings)
            }
            "phpcs" => {
                // phpcs reports the paths it was given, resolved; pass and match canonical ones.
                let mut owned: Vec<(PathBuf, &PathBuf)> = Vec::with_capacity(pairs.len());
                for (workspace, absolute) in pairs {
                    let real = std::fs::canonicalize(absolute)
                        .map_err(|err| execution(tool_id, format!("resolve {workspace}: {err}")))?;
                    owned.push((real, absolute));
                }
                // Always name a standard: without one phpcs would pick up a phpcs.xml from the
                // working directory.
                let standard: std::ffi::OsString = match config.as_deref() {
                    Some(path) => std::fs::canonicalize(path)
                        .map_err(|err| execution(tool_id, format!("resolve phpcs config: {err}")))?
                        .into_os_string(),
                    None => PHPCS_STANDARD.into(),
                };
                let path_refs: Vec<&Path> = owned.iter().map(|(real, _)| real.as_path()).collect();
                let invocation = commands::phpcs_check(&tool.binary, &standard, &path_refs);
                let out = self.run(tool_id, tool, &invocation, scratch)?;
                let real_names: Vec<String> = owned
                    .iter()
                    .map(|(real, _)| real.to_string_lossy().into_owned())
                    .collect();
                let real_strs: Vec<&str> = real_names.iter().map(String::as_str).collect();
                let mut findings = parsed(
                    tool_id,
                    parsers::parse_phpcs(&out.stdout, out.code, &real_strs),
                )?;
                for found in &mut findings {
                    if let Some((_, absolute)) = owned
                        .iter()
                        .find(|(real, _)| real.to_string_lossy() == found.file.as_str())
                    {
                        found.file = absolute.to_string_lossy().into_owned();
                    }
                }
                Ok(findings)
            }
            "jet" => {
                let root = std::fs::canonicalize(scratch.root()).map_err(|err| {
                    execution(tool_id, format!("canonicalize scratch root: {err}"))
                })?;
                let mut owned: Vec<(PathBuf, &PathBuf)> = Vec::with_capacity(pairs.len());
                for (workspace, absolute) in pairs {
                    let real = std::fs::canonicalize(absolute)
                        .map_err(|err| execution(tool_id, format!("resolve {workspace}: {err}")))?;
                    owned.push((real, absolute));
                }
                let work = root.join(".dx-jet");
                std::fs::create_dir_all(&work)
                    .map_err(|err| execution(tool_id, format!("stage jet work dir: {err}")))?;
                let report = work.join("report.json");
                let invocation = commands::jet_check(
                    &tool.binary,
                    &root.join("dx-jet/config.toml"),
                    &root,
                    &report,
                );
                let out = self.run(tool_id, tool, &invocation, scratch)?;
                let body = std::fs::read(&report).map_err(|err| {
                    execution(
                        tool_id,
                        format!(
                            "no report (exit {:?}): {err}: {}",
                            out.code,
                            String::from_utf8_lossy(&out.stderr)
                                .lines()
                                .next()
                                .unwrap_or("")
                        ),
                    )
                })?;
                let real_names: Vec<String> = owned
                    .iter()
                    .map(|(real, _)| real.to_string_lossy().into_owned())
                    .collect();
                let real_strs: Vec<&str> = real_names.iter().map(String::as_str).collect();
                let mut findings = parsed(tool_id, parsers::parse_jet(&body, &real_strs))?;
                for found in &mut findings {
                    if let Some((_, absolute)) = owned
                        .iter()
                        .find(|(real, _)| real.to_string_lossy() == found.file.as_str())
                    {
                        found.file = absolute.to_string_lossy().into_owned();
                    }
                }
                Ok(findings)
            }
            "pkl_lsp" => {
                let companion = |name: &str| {
                    tool.path_tools
                        .iter()
                        .find(|(helper, _)| helper == name)
                        .map(|(_, path)| path.clone())
                        .ok_or_else(|| {
                            execution(
                                tool_id,
                                format!("pkl_lsp needs the {name} companion; the aspect did not supply it"),
                            )
                        })
                };
                let java = companion("java")?;
                let java = real_launcher(&java);
                let jar = companion("pkl-lsp.jar")?;
                let home = scratch.root().join(PKL_LSP_HOME_REL);
                std::fs::create_dir_all(&home)
                    .map_err(|err| execution(tool_id, format!("stage pkl-lsp home: {err}")))?;
                let invocation = commands::pkl_lsp_check(&tool.binary, &java, &jar, &home, &refs);
                // The companions are named by absolute path, so the child needs no PATH shims.
                let bare = RealTool {
                    binary: tool.binary.clone(),
                    extra_env: tool.extra_env.clone(),
                    config_rel: None,
                    edition: None,
                    dialect: None,
                    tool_files: Vec::new(),
                    upstream_diagnostics: Vec::new(),
                    search_paths: Vec::new(),
                    path_tools: Vec::new(),
                };
                let out = self.run(tool_id, &bare, &invocation, scratch)?;
                parsed(
                    tool_id,
                    parsers::parse_pkl_lsp(&out.stdout, out.code, &strs),
                )
                .map_err(|err| with_stderr(err, &out.stderr))
            }
            "julialint" => {
                let work = scratch.root().join(".dx-julialint");
                std::fs::create_dir_all(&work).map_err(|err| {
                    execution(tool_id, format!("stage julialint work dir: {err}"))
                })?;
                let report = work.join("report.json");
                let invocation = commands::julialint_check(&tool.binary, &work, &report, &refs);
                let out = self.run(tool_id, tool, &invocation, scratch)?;
                let body = std::fs::read(&report).map_err(|err| {
                    execution(
                        tool_id,
                        format!(
                            "no report (exit {:?}): {err}: {}",
                            out.code,
                            String::from_utf8_lossy(&out.stderr)
                                .lines()
                                .next()
                                .unwrap_or("")
                        ),
                    )
                })?;
                parsed(tool_id, parsers::parse_julialint(&body, out.code, &strs))
            }
            "luals" => {
                // LuaLS reports file URIs of the real paths, so analyse and match canonical ones.
                let root = std::fs::canonicalize(scratch.root()).map_err(|err| {
                    execution(tool_id, format!("canonicalize scratch root: {err}"))
                })?;
                let mut owned: Vec<(PathBuf, &PathBuf)> = Vec::with_capacity(pairs.len());
                for (workspace, absolute) in pairs {
                    let real = std::fs::canonicalize(absolute)
                        .map_err(|err| execution(tool_id, format!("resolve {workspace}: {err}")))?;
                    owned.push((real, absolute));
                }
                let work = root.join(".dx-luals");
                std::fs::create_dir_all(&work)
                    .map_err(|err| execution(tool_id, format!("stage luals work dir: {err}")))?;
                let library = tool
                    .tool_files
                    .iter()
                    .any(|(rel, _)| rel.starts_with("dx-luals/mod/"))
                    .then(|| root.join("dx-luals/mod"));
                // A consumer config is a flat .luarc.json object; .jsonc is not read.
                let consumer: Option<serde_json::Value> = match config.as_deref() {
                    Some(path) => {
                        let text = std::fs::read_to_string(path).map_err(|err| {
                            execution(tool_id, format!("read luals config: {err}"))
                        })?;
                        Some(serde_json::from_str(&text).map_err(|err| {
                            execution(
                                tool_id,
                                format!(
                                    "luals config is not plain JSON (.jsonc is unsupported): {err}"
                                ),
                            )
                        })?)
                    }
                    None => None,
                };
                let luarc = work.join("luarc.json");
                std::fs::write(
                    &luarc,
                    commands::luals_luarc(
                        consumer.as_ref(),
                        library.as_deref(),
                        &["dx-luals", ".dx-luals"],
                    ),
                )
                .map_err(|err| execution(tool_id, format!("write luals config: {err}")))?;
                let report = work.join("check.json");
                let invocation = commands::luals_check(&tool.binary, &root, &work, &report, &luarc);
                let out = self.run(tool_id, tool, &invocation, scratch)?;
                // LuaLS exits 0 with findings, so only the report tells a run from a crash.
                let body = std::fs::read(&report).map_err(|err| {
                    execution(
                        tool_id,
                        format!(
                            "no report (exit {:?}): {err}: {}",
                            out.code,
                            String::from_utf8_lossy(&out.stderr)
                                .lines()
                                .next()
                                .unwrap_or("")
                        ),
                    )
                })?;
                let real_names: Vec<String> = owned
                    .iter()
                    .map(|(real, _)| real.to_string_lossy().into_owned())
                    .collect();
                let real_strs: Vec<&str> = real_names.iter().map(String::as_str).collect();
                let mut findings = parsed(tool_id, parsers::parse_luals(&body, &real_strs))?;
                for found in &mut findings {
                    if let Some((_, absolute)) = owned
                        .iter()
                        .find(|(real, _)| real.to_string_lossy() == found.file.as_str())
                    {
                        found.file = absolute.to_string_lossy().into_owned();
                    }
                }
                Ok(findings)
            }
            "phpstan" => {
                // PHPStan reports realpaths, so analyse and match the canonical ones.
                let root = std::fs::canonicalize(scratch.root()).map_err(|err| {
                    execution(tool_id, format!("canonicalize scratch root: {err}"))
                })?;
                let mut owned: Vec<(PathBuf, &PathBuf)> = Vec::with_capacity(pairs.len());
                for (workspace, absolute) in pairs {
                    let real = std::fs::canonicalize(absolute)
                        .map_err(|err| execution(tool_id, format!("resolve {workspace}: {err}")))?;
                    owned.push((real, absolute));
                }
                let work = root.join(".dx-phpstan");
                let tmp_dir = work.join("tmp");
                std::fs::create_dir_all(&tmp_dir)
                    .map_err(|err| execution(tool_id, format!("stage phpstan tmp: {err}")))?;
                let scan_dirs: Vec<PathBuf> = ["dx-phpstan/scan", "dx-phpstan/pkg"]
                    .iter()
                    .filter(|dir| {
                        tool.tool_files
                            .iter()
                            .any(|(rel, _)| rel.starts_with(&format!("{dir}/")))
                    })
                    .map(|dir| root.join(dir))
                    .collect();
                let scan_refs: Vec<&Path> = scan_dirs.iter().map(PathBuf::as_path).collect();
                let consumer = match config.as_deref() {
                    Some(path) => Some(std::fs::canonicalize(path).map_err(|err| {
                        execution(tool_id, format!("resolve phpstan config: {err}"))
                    })?),
                    None => None,
                };
                let extension = tool
                    .tool_files
                    .iter()
                    .any(|(rel, _)| rel.starts_with("dx-phpstan/ext/"))
                    .then(|| root.join("dx-phpstan/ext"));
                let autoload = extension.as_ref().map(|dir| dir.join("autoload.php"));
                let neon = work.join("dx.neon");
                std::fs::write(
                    &neon,
                    commands::phpstan_neon(
                        consumer.as_deref(),
                        PHPSTAN_LEVEL,
                        &tmp_dir,
                        &scan_refs,
                        extension.as_deref(),
                    ),
                )
                .map_err(|err| execution(tool_id, format!("write phpstan config: {err}")))?;
                let path_refs: Vec<&Path> = owned.iter().map(|(real, _)| real.as_path()).collect();
                let invocation =
                    commands::phpstan_check(&tool.binary, &neon, autoload.as_deref(), &path_refs);
                let out = self.run(tool_id, tool, &invocation, scratch)?;
                let real_names: Vec<String> = owned
                    .iter()
                    .map(|(real, _)| real.to_string_lossy().into_owned())
                    .collect();
                let real_strs: Vec<&str> = real_names.iter().map(String::as_str).collect();
                let mut findings = parsed(
                    tool_id,
                    parsers::parse_phpstan(&out.stdout, out.code, &real_strs),
                )?;
                if capability == "unused" {
                    findings = parsers::keep_unused_public(findings);
                }
                for found in &mut findings {
                    if let Some((_, absolute)) = owned
                        .iter()
                        .find(|(real, _)| real.to_string_lossy() == found.file.as_str())
                    {
                        found.file = absolute.to_string_lossy().into_owned();
                    }
                }
                Ok(findings)
            }
            "sorbet" => {
                // The owned sources and the staged dependency sources go on the command line, so
                // Sorbet resolves constants across them; only findings in the owned sources are
                // kept. It reads no config and writes no cache, so nothing outside the scratch
                // directory is touched.
                let mut deps: Vec<PathBuf> = tool
                    .tool_files
                    .iter()
                    .filter(|(rel, _)| rel.starts_with("dx-sorbet/dep/"))
                    .map(|(rel, _)| scratch.root().join(rel))
                    .collect();
                deps.sort();
                let mut inputs: Vec<&Path> = refs.clone();
                inputs.extend(deps.iter().map(PathBuf::as_path));
                let invocation = commands::sorbet_check(&tool.binary, &inputs);
                let out = self.run(tool_id, tool, &invocation, scratch)?;
                // Sorbet prints its errors on stderr.
                parsed(tool_id, parsers::parse_sorbet(&out.stderr, out.code, &strs))
            }
            "detekt" => {
                let Some(cfg) = config.as_deref() else {
                    return Err(execution(tool_id, "detekt needs a config path".to_owned()));
                };
                let work = scratch.root().join(".dx-detekt");
                std::fs::create_dir_all(&work)
                    .map_err(|err| execution(tool_id, format!("stage detekt work dir: {err}")))?;
                let report = work.join("report.xml");
                let invocation = commands::detekt_check(&tool.binary, &refs, cfg, &report);
                let out = self.run(tool_id, tool, &invocation, scratch)?;
                let body = std::fs::read(&report).map_err(|err| {
                    execution(
                        tool_id,
                        format!(
                            "no report (exit {:?}): {err}: {}",
                            out.code,
                            String::from_utf8_lossy(&out.stderr)
                                .lines()
                                .next()
                                .unwrap_or("")
                        ),
                    )
                })?;
                parsed(tool_id, parsers::parse_detekt(&body, out.code, &strs))
            }
            "ktlint" => {
                let invocation = commands::ktlint_check(&tool.binary, &refs, config.as_deref());
                let out = self.run(tool_id, tool, &invocation, scratch)?;
                parsed(tool_id, parsers::parse_ktlint(&out.stdout, out.code, &strs))
            }
            "cue" if capability == "typecheck" => self.check_cue_vet(tool_id, tool, staged),
            "pkl" if capability == "typecheck" => self.check_pkl_eval(tool_id, tool, staged),
            "terraform" if capability == "typecheck" => {
                self.check_terraform_validate(tool_id, tool, staged)
            }
            "cue" => {
                let invocation = commands::cue_check(&tool.binary, &refs);
                let out = self.run(tool_id, tool, &invocation, scratch)?;
                let cwd = scratch.root().join(&cwd_rel);
                parsed(
                    tool_id,
                    parsers::parse_cue(&out.stdout, &out.stderr, out.code, &strs, &cwd),
                )
            }
            "regal" => {
                let Some(config) = config.as_deref() else {
                    return Err(execution(tool_id, "regal needs a config path".to_owned()));
                };
                std::fs::create_dir_all(scratch.root().join(".regal"))
                    .map_err(|err| execution(tool_id, format!("stage .regal: {err}")))?;
                let relative: Vec<&Path> = pairs
                    .iter()
                    .map(|(workspace, _)| Path::new(workspace.as_str()))
                    .collect();
                let workspaces: Vec<&str> = pairs
                    .iter()
                    .map(|(workspace, _)| workspace.as_str())
                    .collect();
                let invocation = commands::regal_lint(&tool.binary, &relative, config);
                let out = self.run(tool_id, tool, &invocation, scratch)?;
                let mut findings = parsed(
                    tool_id,
                    parsers::parse_regal(&out.stdout, &out.stderr, out.code, &workspaces),
                )?;
                for item in &mut findings {
                    item.file = reanchor(tool_id, pairs, &item.file)?
                        .to_string_lossy()
                        .into_owned();
                }
                Ok(findings)
            }
            "opa" => {
                if capability == "format" {
                    let mut findings = Vec::new();
                    for (workspace, absolute) in pairs {
                        let invocation = commands::opa_fmt_check(&tool.binary, absolute);
                        let out = self.run(tool_id, tool, &invocation, scratch)?;
                        let input = std::fs::read(absolute).map_err(|err| {
                            execution(tool_id, format!("read {workspace}: {err}"))
                        })?;
                        let file = absolute.to_string_lossy();
                        findings.extend(parsed(
                            tool_id,
                            parsers::parse_opa_fmt(
                                &out.stdout,
                                &out.stderr,
                                out.code,
                                &input,
                                &file,
                            ),
                        )?);
                    }
                    Ok(findings)
                } else {
                    let mut all: Vec<&Path> = refs.clone();
                    for (_, absolute) in resolve_pairs {
                        if !all.contains(&absolute.as_path()) {
                            all.push(absolute.as_path());
                        }
                    }
                    let invocation = commands::opa_check(&tool.binary, &all);
                    let out = self.run(tool_id, tool, &invocation, scratch)?;
                    let known: Vec<String> = all
                        .iter()
                        .map(|path| path.to_string_lossy().into_owned())
                        .collect();
                    let known: Vec<&str> = known.iter().map(String::as_str).collect();
                    let mut findings = parsed(
                        tool_id,
                        parsers::parse_opa_check(&out.stdout, &out.stderr, out.code, &known),
                    )?;
                    let reported: Vec<String> = findings.iter().map(|f| f.file.clone()).collect();
                    findings.retain(|found| strs.contains(&found.file.as_str()));
                    if findings.is_empty() && !reported.is_empty() {
                        let mut names: Vec<String> = resolve_pairs
                            .iter()
                            .filter(|(_, absolute)| {
                                reported
                                    .iter()
                                    .any(|r| r.as_str() == absolute.to_string_lossy())
                            })
                            .map(|(workspace, _)| workspace.clone())
                            .collect();
                        names.sort();
                        names.dedup();
                        return Err(execution(
                            tool_id,
                            format!(
                                "opa check failed in dependencies before the owned files were type checked: {}",
                                names.join(", ")
                            ),
                        ));
                    }
                    Ok(findings)
                }
            }
            "perltidy" => {
                // One process per file: --standard-output takes a single file.
                let mut findings = Vec::new();
                for (workspace, absolute) in pairs {
                    let invocation = commands::perltidy_check(&tool.binary, absolute.as_path());
                    let out = self.run(tool_id, tool, &invocation, scratch)?;
                    let input = std::fs::read(absolute)
                        .map_err(|err| execution(tool_id, format!("read {workspace}: {err}")))?;
                    let name = absolute.to_string_lossy();
                    let found = parsed(
                        tool_id,
                        parsers::parse_perltidy(&out.stdout, &out.stderr, out.code, &input, &name),
                    )?;
                    findings.extend(found);
                }
                Ok(findings)
            }
            "elvis" => {
                let Some(config) = config.as_deref() else {
                    return Err(execution(tool_id, "elvis needs a config path".to_owned()));
                };
                let root = scratch.root();
                let relative = pairs
                    .iter()
                    .map(|(_, absolute)| {
                        absolute.strip_prefix(root).map_err(|_| {
                            execution(
                                tool_id,
                                format!("{} is outside the scratch", absolute.display()),
                            )
                        })
                    })
                    .collect::<Result<Vec<&Path>, RunnerError>>()?;
                let relative_names: Vec<String> = relative
                    .iter()
                    .map(|path| path.to_string_lossy().into_owned())
                    .collect();
                let relative_strs: Vec<&str> = relative_names.iter().map(String::as_str).collect();
                let invocation = commands::elvis_check(&tool.binary, &relative, config);
                let out = self.run(tool_id, tool, &invocation, scratch)?;
                let mut findings = parsed(
                    tool_id,
                    parsers::parse_elvis(&out.stdout, &out.stderr, out.code, &relative_strs),
                )?;
                for found in &mut findings {
                    if let Some(at) = relative_names.iter().position(|name| *name == found.file) {
                        found.file.clone_from(&names[at]);
                    }
                }
                Ok(findings)
            }
            "erlfmt" => {
                let invocation = commands::erlfmt_check(&tool.binary, &refs);
                let out = self.run(tool_id, tool, &invocation, scratch)?;
                parsed(
                    tool_id,
                    parsers::parse_erlfmt(&out.stdout, &out.stderr, out.code, &strs),
                )
            }
            "credo" => {
                let Some(config) = config.as_deref() else {
                    return Err(execution(tool_id, "credo needs a config path".to_owned()));
                };
                // Credo prints paths relative to its working directory, so it gets them that way.
                let cwd = scratch.root().join(&cwd_rel);
                let relative = refs
                    .iter()
                    .map(|absolute| {
                        absolute.strip_prefix(&cwd).map_err(|_| {
                            execution(
                                tool_id,
                                format!("{} is outside {}", absolute.display(), cwd.display()),
                            )
                        })
                    })
                    .collect::<Result<Vec<&Path>, RunnerError>>()?;
                let relative_names: Vec<String> = relative
                    .iter()
                    .map(|path| path.to_string_lossy().into_owned())
                    .collect();
                let relative_strs: Vec<&str> = relative_names.iter().map(String::as_str).collect();
                let invocation = commands::credo_check(&tool.binary, &relative, config);
                let out = self.run(tool_id, tool, &invocation, scratch)?;
                let mut findings = parsed(
                    tool_id,
                    parsers::parse_credo(&out.stdout, &out.stderr, out.code, &relative_strs),
                )?;
                for found in &mut findings {
                    if let Some(at) = relative_names.iter().position(|name| *name == found.file) {
                        found.file.clone_from(&names[at]);
                    }
                }
                Ok(findings)
            }
            "mix_format" => {
                let invocation = commands::mix_format_check(&tool.binary, &refs);
                let out = self.run(tool_id, tool, &invocation, scratch)?;
                let cwd = scratch.root().join(&cwd_rel);
                parsed(
                    tool_id,
                    parsers::parse_mix_format(&out.stdout, &out.stderr, out.code, &strs, &cwd),
                )
            }
            "selene" => {
                let invocation = commands::selene_check(&tool.binary, &refs, config.as_deref());
                let out = self.run(tool_id, tool, &invocation, scratch)?;
                parsed(tool_id, parsers::parse_selene(&out.stdout, out.code, &strs))
            }
            "stylua" => {
                let invocation = commands::stylua_check(&tool.binary, &refs, config.as_deref());
                let out = self.run(tool_id, tool, &invocation, scratch)?;
                parsed(
                    tool_id,
                    parsers::parse_stylua(&out.stdout, &out.stderr, out.code, &strs),
                )
            }
            "ormolu" => {
                // One process per file: ormolu formats files concurrently and interleaves
                // their diffs on stderr, and exits 102 on mixed results.
                let mut merged = Vec::new();
                for (name, path) in strs.iter().zip(refs.iter()) {
                    let invocation =
                        commands::ormolu_check(&tool.binary, std::slice::from_ref(path));
                    let out = self.run(tool_id, tool, &invocation, scratch)?;
                    let found = parsed(
                        tool_id,
                        parsers::parse_ormolu(&out.stdout, &out.stderr, out.code, &[*name]),
                    )
                    .map_err(|err| in_file(err, name))?;
                    merged.extend(found);
                }
                Ok(merged)
            }
            "pkl" => {
                let invocation = commands::pkl_check(&tool.binary, &refs);
                let out = self.run(tool_id, tool, &invocation, scratch)?;
                parsed(
                    tool_id,
                    parsers::parse_pkl(&out.stdout, &out.stderr, out.code, &strs),
                )
            }
            "modfmt" => {
                let invocation = commands::modfmt_check(&tool.binary, &refs);
                let out = self.run(tool_id, tool, &invocation, scratch)?;
                parsed(tool_id, parsers::parse_modfmt(&out.stdout, out.code, &strs))
            }
            "tflint" => {
                let Some(config) = config.as_deref() else {
                    return Err(execution(tool_id, "tflint needs a config path".to_owned()));
                };
                let mut merged = Vec::new();
                for (dir, members) in group_by_parent(scratch.root(), pairs) {
                    let names: Vec<&Path> = members
                        .iter()
                        .map(|(name, _, _)| Path::new(name.as_str()))
                        .collect();
                    let invocation = commands::tflint_check(&tool.binary, &dir, &names, config);
                    let out = self.run(tool_id, tool, &invocation, scratch)?;
                    let reported: Vec<&str> =
                        members.iter().map(|(_, rel, _)| rel.as_str()).collect();
                    let mut found = parsed(
                        tool_id,
                        parsers::parse_tflint(&out.stdout, out.code, &reported),
                    )?;
                    for entry in &mut found {
                        if let Some((_, _, absolute)) =
                            members.iter().find(|(_, rel, _)| *rel == entry.file)
                        {
                            entry.file.clone_from(absolute);
                        }
                    }
                    merged.extend(found);
                }
                Ok(merged)
            }
            "terraform" => {
                let invocation = commands::terraform_check(&tool.binary, &refs);
                let out = self.run(tool_id, tool, &invocation, scratch)?;
                let cwd = scratch.root().join(&cwd_rel);
                parsed(
                    tool_id,
                    parsers::parse_terraform(&out.stdout, &out.stderr, out.code, &strs, &cwd),
                )
            }
            "yamlfmt" => {
                let invocation = commands::yamlfmt_check(&tool.binary, &refs, config.as_deref());
                let out = self.run(tool_id, tool, &invocation, scratch)?;
                parsed(
                    tool_id,
                    parsers::parse_yamlfmt(&out.stderr, out.code, &strs),
                )
            }
            "shfmt" => {
                let invocation = commands::shfmt_check(&tool.binary, &refs);
                let out = self.run(tool_id, tool, &invocation, scratch)?;
                parsed(tool_id, parsers::parse_shfmt(&out.stdout, out.code, &strs))
            }
            "standardrb" => {
                let invocation = commands::standardrb_check(&tool.binary, &refs, config.as_deref());
                let out = self.run(tool_id, tool, &invocation, scratch)?;
                parsed(
                    tool_id,
                    parsers::parse_standardrb(
                        &out.stdout,
                        &out.stderr,
                        out.code,
                        &strs,
                        scratch.root(),
                    ),
                )
            }
            "psscriptanalyzer" => {
                if capability == "format" {
                    let invocation = commands::psscriptanalyzer_format_check(
                        &tool.binary,
                        &refs,
                        config.as_deref(),
                    );
                    let out = self.run(tool_id, tool, &invocation, scratch)?;
                    return parsed(
                        tool_id,
                        parsers::parse_psscriptanalyzer_format(&out.stdout, out.code, &strs),
                    );
                }
                let invocation =
                    commands::psscriptanalyzer_check(&tool.binary, &refs, config.as_deref());
                let out = self.run(tool_id, tool, &invocation, scratch)?;
                parsed(
                    tool_id,
                    parsers::parse_psscriptanalyzer(&out.stdout, out.code, &strs),
                )
            }
            "rubocop" | "yamllint" | "shellcheck" | "hadolint" | "checkmake" => {
                if !tool.upstream_diagnostics.is_empty() {
                    self.check_file_family_delegated(tool_id, tool, pairs)
                } else {
                    let invocation = match tool_id {
                        "rubocop" => {
                            commands::rubocop_check(&tool.binary, &refs, config.as_deref())
                        }
                        "yamllint" => {
                            commands::yamllint_check(&tool.binary, &refs, config.as_deref())
                        }
                        "checkmake" => {
                            let Some(config) = config.as_deref() else {
                                return Err(execution(
                                    tool_id,
                                    "checkmake needs a config path".to_owned(),
                                ));
                            };
                            commands::checkmake_check(&tool.binary, &refs, config)
                        }
                        "hadolint" => {
                            let Some(config) = config.as_deref() else {
                                return Err(execution(
                                    tool_id,
                                    "hadolint needs a config path".to_owned(),
                                ));
                            };
                            commands::hadolint_check(&tool.binary, &refs, config)
                        }
                        _ => commands::shellcheck_check(&tool.binary, &refs, config.as_deref()),
                    };
                    let out = self.run(tool_id, tool, &invocation, scratch)?;
                    let report = match tool_id {
                        "rubocop" => parsers::parse_rubocop(
                            &out.stdout,
                            &out.stderr,
                            out.code,
                            &strs,
                            scratch.root(),
                        ),
                        "yamllint" => parsers::parse_yamllint(&out.stdout, out.code, &strs),
                        "checkmake" => parsers::parse_checkmake(
                            &out.stdout,
                            &out.stderr,
                            out.code,
                            &strs,
                            config.as_deref().and_then(std::path::Path::to_str),
                        ),
                        "hadolint" => {
                            parsers::parse_hadolint(&out.stdout, &out.stderr, out.code, &strs)
                        }
                        _ => parsers::parse_shellcheck(&out.stdout, out.code, &strs),
                    };
                    parsed(tool_id, report)
                }
            }
            "statix" => {
                if !tool.upstream_diagnostics.is_empty() {
                    return self.check_file_family_delegated(tool_id, tool, pairs);
                }
                let Some(config) = config.as_deref() else {
                    return Err(execution(tool_id, "statix needs a config path".to_owned()));
                };
                let mut findings = Vec::new();
                for name in &strs {
                    let invocation = commands::statix_check(&tool.binary, Path::new(name), config);
                    let out = self.run(tool_id, tool, &invocation, scratch)?;
                    let report =
                        parsers::parse_statix(&out.stdout, &out.stderr, out.code, &[*name]);
                    findings.extend(parsed(tool_id, report)?);
                }
                Ok(findings)
            }
            "deadnix" => {
                if !tool.upstream_diagnostics.is_empty() {
                    return self.check_file_family_delegated(tool_id, tool, pairs);
                }
                self.check_files(
                    tool_id,
                    tool,
                    scratch,
                    pairs,
                    |files| Ok(commands::deadnix_check(&tool.binary, files)),
                    |out, names| {
                        parsed(
                            tool_id,
                            parsers::parse_deadnix(&out.stdout, &out.stderr, out.code, names),
                        )
                    },
                )
            }
            "debride" => self.check_files(
                tool_id,
                tool,
                scratch,
                pairs,
                |files| Ok(commands::debride_check(&tool.binary, files)),
                |out, names| {
                    parsed(
                        tool_id,
                        parsers::parse_debride(&out.stdout, &out.stderr, out.code, names),
                    )
                },
            ),
            "nixfmt" => self.check_files(
                tool_id,
                tool,
                scratch,
                pairs,
                |files| Ok(commands::nixfmt_check(&tool.binary, files)),
                |out, names| {
                    parsed(
                        tool_id,
                        parsers::parse_nixfmt(&out.stdout, &out.stderr, out.code, names),
                    )
                },
            ),
            "topiary" => self.check_files(
                tool_id,
                tool,
                scratch,
                pairs,
                |files| Ok(commands::topiary_check(&tool.binary, files)),
                |out, names| {
                    parsed(
                        tool_id,
                        parsers::parse_topiary(&out.stdout, out.code, names),
                    )
                },
            ),
            "ansible_lint" => {
                let roots = staging::ansible_collections_roots(
                    tool.tool_files.iter().map(|(rel, _)| rel.as_str()),
                )
                .map_err(|detail| execution(tool_id, detail))?;
                let mut collections = Vec::with_capacity(roots.len());
                for root in &roots {
                    collections.push(if root.is_empty() {
                        scratch.root().to_path_buf()
                    } else {
                        scratch
                            .resolve(Path::new(root))
                            .map_err(|err| execution(tool_id, format!("collections root: {err}")))?
                    });
                }
                let collection_refs: Vec<&Path> =
                    collections.iter().map(PathBuf::as_path).collect();
                let context = commands::AnsibleLintContext {
                    config: config.as_deref(),
                    collections: &collection_refs,
                    project_dir: scratch.root(),
                };
                let invocation = commands::ansible_lint_check(&tool.binary, &refs, &context);
                let out = self.run(tool_id, tool, &invocation, scratch)?;
                parsed(
                    tool_id,
                    parsers::parse_ansible_lint(&out.stdout, &out.stderr, out.code, &strs),
                )
            }
            "typos" => self.check_files(
                tool_id,
                tool,
                scratch,
                pairs,
                |files| Ok(commands::typos_check(&tool.binary, files)),
                |out, names| parsed(tool_id, parsers::parse_typos(&out.stdout, out.code, names)),
            ),
            "actionlint" => self.check_files(
                tool_id,
                tool,
                scratch,
                pairs,
                |files| Ok(commands::actionlint_check(&tool.binary, files)),
                |out, names| {
                    parsed(
                        tool_id,
                        parsers::parse_actionlint(&out.stdout, &out.stderr, out.code, names),
                    )
                },
            ),
            "lychee" => {
                let root = scratch.root();
                let online = tool
                    .extra_env
                    .iter()
                    .any(|(name, value)| name == "DX_LINT_ONLINE" && value == "1");
                let reported = self.check_files(
                    tool_id,
                    tool,
                    scratch,
                    pairs,
                    |files| Ok(commands::lychee_check(&tool.binary, files, root, online)),
                    |out, names| {
                        parsed(
                            tool_id,
                            parsers::parse_lychee(&out.stdout, &out.stderr, out.code, names),
                        )
                    },
                )?;
                Ok(reported
                    .into_iter()
                    .map(|mut found| {
                        found.finding.message = without_scratch_url(&found.finding.message, root);
                        found
                    })
                    .collect())
            }
            "keep_sorted" => {
                if !tool.upstream_diagnostics.is_empty() {
                    self.check_file_family_delegated(tool_id, tool, pairs)
                } else {
                    self.check_files(
                        tool_id,
                        tool,
                        scratch,
                        pairs,
                        |files| Ok(commands::keep_sorted_check(&tool.binary, files)),
                        |out, names| {
                            parsed(
                                tool_id,
                                parsers::parse_keep_sorted_lint(&out.stdout, out.code, names),
                            )
                        },
                    )
                }
            }
            _ => Err(execution(
                tool_id,
                format!("unsupported real tool: {tool_id}"),
            )),
        }
    }

    /// Knip analyses a whole package, so it runs once in each package root: the nearest
    /// directory at or above a staged file that holds a `package.json`.
    pub(super) fn check_knip(
        &self,
        tool_id: &str,
        tool: &RealTool,
        scratch: &Scratch,
        pairs: &[(String, PathBuf)],
    ) -> Result<Vec<FileFinding>, RunnerError> {
        let root = scratch.root();
        let mut roots: std::collections::BTreeMap<String, Vec<(String, &Path)>> =
            std::collections::BTreeMap::new();
        for (_, absolute) in pairs {
            let rel = absolute.strip_prefix(root).unwrap_or(absolute);
            let mut dir = rel.parent();
            let package = loop {
                match dir {
                    Some(dir) if root.join(dir).join("package.json").is_file() => break dir,
                    Some(dir) if dir.as_os_str().is_empty() => {
                        return Err(execution(
                            tool_id,
                            format!("no package.json at or above {}", rel.display()),
                        ));
                    }
                    Some(parent) => dir = parent.parent(),
                    None => break Path::new(""),
                }
            };
            let inside = rel.strip_prefix(package).unwrap_or(rel);
            roots
                .entry(package.to_string_lossy().replace('\\', "/"))
                .or_default()
                .push((inside.to_string_lossy().replace('\\', "/"), absolute));
        }
        let mut found = Vec::new();
        for (package, members) in &roots {
            let invocation = commands::knip_check(&tool.binary, package);
            let out = self.run(tool_id, tool, &invocation, scratch)?;
            let names: Vec<&str> = members.iter().map(|(inside, _)| inside.as_str()).collect();
            for mut hit in parsed(
                tool_id,
                parsers::parse_knip(&out.stdout, &out.stderr, out.code, &names),
            )? {
                if let Some((_, absolute)) = members.iter().find(|(inside, _)| *inside == hit.file)
                {
                    hit.file = absolute.to_string_lossy().into_owned();
                }
                found.push(hit);
            }
        }
        Ok(found)
    }

    /// Runs one tool over the selection, in argv-budget batches when its transport allows.
    ///
    /// Findings merge in a stable order, and a failing batch names itself when there are
    /// several so the error points at the spawn that broke.
    /// Reads every staged notebook and refuses any that Ruff would skip, so a
    /// non-Python or malformed notebook never reports clean.
    /// `cue vet -c=false` once per package, by import path from the enclosing CUE module root, so
    /// imports and ancestor-directory files of the same package resolve as in the real instance.
    /// zanuda reads the typed trees the compile wrote, not the sources: the aspect stages each
    /// `.cmt`/`.cmti` as `dx-zanuda/{impl,intf}/<source workspace path>.cmt[i]`. This batch lints
    /// the entries whose source is in `pairs` and fails when an owned source has none, so a
    /// target built with `-no-bin-annot` never reads as clean.
    pub(super) fn check_zanuda(
        &self,
        tool_id: &str,
        tool: &RealTool,
        scratch: &Scratch,
        pairs: &[(String, PathBuf)],
    ) -> Result<Vec<FileFinding>, RunnerError> {
        let mut lines = Vec::with_capacity(pairs.len());
        for (workspace, absolute) in pairs {
            let kind = if workspace.ends_with(".mli") {
                ("intf", "cmti")
            } else {
                ("impl", "cmt")
            };
            let rel = format!("dx-zanuda/{}/{workspace}.{}", kind.0, kind.1);
            if !tool.tool_files.iter().any(|(known, _)| *known == rel) {
                return Err(execution(
                    tool_id,
                    format!(
                        "no .{} for {workspace}; was it compiled with -no-bin-annot?",
                        kind.1
                    ),
                ));
            }
            lines.push(format!(
                "{}\t{}\t{}",
                kind.0,
                absolute.display(),
                scratch.root().join(&rel).display()
            ));
        }
        if lines.is_empty() {
            return Ok(Vec::new());
        }
        let dir = scratch.root().join("dx-zanuda");
        std::fs::create_dir_all(&dir)
            .map_err(|err| execution(tool_id, format!("create {}: {err}", dir.display())))?;
        let list = dir.join("list.tsv");
        let report = dir.join("out.jsonl");
        std::fs::write(&list, lines.join("\n") + "\n")
            .map_err(|err| execution(tool_id, format!("write {}: {err}", list.display())))?;
        let invocation = commands::zanuda_check(&tool.binary, &list, &report);
        let out = self.run(tool_id, tool, &invocation, scratch)?;
        if out.code != Some(0) {
            let tail = String::from_utf8_lossy(&out.stderr);
            return Err(execution(
                tool_id,
                format!(
                    "exit {}: {}",
                    out.code
                        .map_or_else(|| "by signal".to_owned(), |c| c.to_string()),
                    tail.trim()
                ),
            ));
        }
        let bytes = std::fs::read(&report)
            .map_err(|err| execution(tool_id, format!("read {}: {err}", report.display())))?;
        let names: Vec<String> = pairs
            .iter()
            .map(|(_, absolute)| absolute.to_string_lossy().into_owned())
            .collect();
        let strs: Vec<&str> = names.iter().map(String::as_str).collect();
        parsed(tool_id, parsers::parse_zanuda(&bytes, out.code, &strs))
    }

    fn check_cue_vet(
        &self,
        tool_id: &str,
        tool: &RealTool,
        staged: &StagedScratch,
    ) -> Result<Vec<FileFinding>, RunnerError> {
        let scratch = &staged.scratch;
        let modules: Vec<String> = staged
            .pairs
            .iter()
            .chain(staged.resolve_pairs.iter())
            .filter_map(|(ws, _)| {
                if ws == "cue.mod/module.cue" {
                    Some(String::new())
                } else {
                    ws.strip_suffix("/cue.mod/module.cue").map(str::to_owned)
                }
            })
            .collect();
        let mut groups: std::collections::BTreeSet<(String, String, String)> =
            std::collections::BTreeSet::new();
        for (ws, absolute) in &staged.pairs {
            if ws.starts_with("cue.mod/") || ws.contains("/cue.mod/") {
                continue;
            }
            let dir = ws.rsplit_once('/').map_or("", |(dir, _)| dir).to_owned();
            let root = modules
                .iter()
                .filter(|root| {
                    root.is_empty() || dir == **root || dir.starts_with(&format!("{root}/"))
                })
                .max_by_key(|root| root.len())
                .cloned()
                .unwrap_or_else(|| dir.clone());
            let text = std::fs::read_to_string(absolute)
                .map_err(|err| execution(tool_id, format!("read {ws}: {err}")))?;
            let package = cue_package_clause(&text);
            groups.insert((root, dir, package));
        }
        let owned: Vec<String> = staged
            .pairs
            .iter()
            .map(|(_, absolute)| absolute.to_string_lossy().into_owned())
            .collect();
        let owned: Vec<&str> = owned.iter().map(String::as_str).collect();
        let mut known_paths: Vec<String> = owned.iter().map(|path| (*path).to_owned()).collect();
        for (_, absolute) in &staged.resolve_pairs {
            let text = absolute.to_string_lossy().into_owned();
            if !known_paths.contains(&text) {
                known_paths.push(text);
            }
        }
        let known: Vec<&str> = known_paths.iter().map(String::as_str).collect();
        let mut findings = Vec::new();
        let mut reported = Vec::new();
        for (root, dir, package) in &groups {
            // Vet by import path so cue builds the real instance, which also unifies the staged
            // same-package files of ancestor directories up to the module root.
            let rel = if root.is_empty() {
                dir.as_str()
            } else {
                dir.strip_prefix(root.as_str())
                    .map_or("", |rest| rest.trim_start_matches('/'))
            };
            let mut target = if rel.is_empty() {
                ".".to_owned()
            } else {
                format!("./{rel}")
            };
            if !package.is_empty() {
                target.push(':');
                target.push_str(package);
            }
            let target = PathBuf::from(target);
            let invocation = commands::cue_vet(&tool.binary, &[target.as_path()], root);
            let out = self.run(tool_id, tool, &invocation, scratch)?;
            let cwd = scratch.root().join(root);
            let found = parsed(
                tool_id,
                parsers::parse_cue_vet(&out.stdout, &out.stderr, out.code, &owned, &known, &cwd),
            )?;
            for item in found {
                if owned.contains(&item.file.as_str()) {
                    findings.push(item);
                } else {
                    reported.push(item.file);
                }
            }
        }
        if findings.is_empty() && !reported.is_empty() {
            let mut names: Vec<String> = staged
                .resolve_pairs
                .iter()
                .filter(|(_, absolute)| {
                    reported
                        .iter()
                        .any(|r| r.as_str() == absolute.to_string_lossy())
                })
                .map(|(ws, _)| ws.clone())
                .collect();
            names.sort();
            names.dedup();
            return Err(execution(
                tool_id,
                format!(
                    "cue vet failed in dependencies before the owned files were checked: {}",
                    names.join(", ")
                ),
            ));
        }
        Ok(findings)
    }

    /// `pkl eval` once per owned module with imports staged beside it. Pkl stops at the first
    /// error, so a batched run would lose findings and misattribute them. Abstract modules are
    /// only loaded (`-x 1`), since rendering one reports its by-design unset properties. Values
    /// are lazy: only what the module renders is forced, so hidden or unused members are not
    /// checked. An error whose first frame is a non-owned file is an execution error.
    fn check_pkl_eval(
        &self,
        tool_id: &str,
        tool: &RealTool,
        staged: &StagedScratch,
    ) -> Result<Vec<FileFinding>, RunnerError> {
        let scratch = &staged.scratch;
        let owned: Vec<String> = staged
            .pairs
            .iter()
            .map(|(_, absolute)| absolute.to_string_lossy().into_owned())
            .collect();
        let owned: Vec<&str> = owned.iter().map(String::as_str).collect();
        let mut known_paths: Vec<String> = owned.iter().map(|path| (*path).to_owned()).collect();
        for (_, absolute) in &staged.resolve_pairs {
            let text = absolute.to_string_lossy().into_owned();
            if !known_paths.contains(&text) {
                known_paths.push(text);
            }
        }
        let known: Vec<&str> = known_paths.iter().map(String::as_str).collect();
        let mut findings: Vec<FileFinding> = Vec::new();
        let mut reported = Vec::new();
        for (ws, absolute) in &staged.pairs {
            let text = std::fs::read_to_string(absolute)
                .map_err(|err| execution(tool_id, format!("read {ws}: {err}")))?;
            let load_only = text
                .lines()
                .any(|line| line.trim_start().starts_with("abstract module"));
            let invocation = commands::pkl_eval(&tool.binary, absolute, scratch.root(), load_only);
            let out = self.run(tool_id, tool, &invocation, scratch)?;
            let found = parsed(
                tool_id,
                parsers::parse_pkl_eval(&out.stdout, &out.stderr, out.code, &known),
            )?;
            for item in found {
                if owned.contains(&item.file.as_str()) {
                    if !findings.contains(&item) {
                        findings.push(item);
                    }
                } else {
                    reported.push(item.file);
                }
            }
        }
        if findings.is_empty() && !reported.is_empty() {
            let mut names: Vec<String> = staged
                .resolve_pairs
                .iter()
                .filter(|(_, absolute)| {
                    reported
                        .iter()
                        .any(|r| r.as_str() == absolute.to_string_lossy())
                })
                .map(|(ws, _)| ws.clone())
                .collect();
            names.sort();
            names.dedup();
            return Err(execution(
                tool_id,
                format!(
                    "pkl eval failed in dependencies before the owned files were checked: {}",
                    names.join(", ")
                ),
            ));
        }
        Ok(findings)
    }

    /// `terraform init` then `terraform validate` once per directory that owns `.tf` files.
    ///
    /// Init is offline: every provider comes from the `-plugin-dir` mirrors the request names
    /// in `DX_TERRAFORM_PLUGIN_DIRS` (an empty directory when there are none, so init never
    /// reaches a registry). Init findings (syntax and configuration errors) skip validate for
    /// that directory. A failure that names no file is an execution error, never clean. The
    /// root run and a child module's own run both see the child's errors, so findings are
    /// de-duplicated. An error only in a non-owned file is an execution error, as for pkl.
    fn check_terraform_validate(
        &self,
        tool_id: &str,
        tool: &RealTool,
        staged: &StagedScratch,
    ) -> Result<Vec<FileFinding>, RunnerError> {
        let scratch = &staged.scratch;
        let cwd = std::env::current_dir()
            .map_err(|err| execution(tool_id, format!("current dir: {err}")))?;
        let mut mirrors: Vec<PathBuf> = tool
            .extra_env
            .iter()
            .filter(|(key, _)| key == TERRAFORM_PLUGIN_DIRS_ENV)
            .flat_map(|(_, value)| value.split('|'))
            .filter(|entry| !entry.is_empty())
            .map(|entry| cwd.join(entry))
            .collect();
        if mirrors.is_empty() {
            let empty = scratch.root().join(TERRAFORM_EMPTY_MIRROR_REL);
            std::fs::create_dir_all(&empty)
                .map_err(|err| execution(tool_id, format!("empty mirror: {err}")))?;
            mirrors.push(empty);
        }
        let mirror_refs: Vec<&Path> = mirrors.iter().map(PathBuf::as_path).collect();
        let owned: Vec<String> = staged
            .pairs
            .iter()
            .map(|(_, absolute)| absolute.to_string_lossy().into_owned())
            .collect();
        let mut known_paths = owned.clone();
        for (_, absolute) in &staged.resolve_pairs {
            let text = absolute.to_string_lossy().into_owned();
            if !known_paths.contains(&text) {
                known_paths.push(text);
            }
        }
        let known: Vec<&str> = known_paths.iter().map(String::as_str).collect();
        let sources: Vec<(String, PathBuf)> = staged
            .pairs
            .iter()
            .filter(|(ws, _)| ws.ends_with(".tf") || ws.ends_with(".tf.json"))
            .cloned()
            .collect();
        let mut findings: Vec<FileFinding> = Vec::new();
        let mut reported = Vec::new();
        for (dir, _) in group_by_parent(scratch.root(), &sources) {
            let abs_dir = if dir == "." {
                scratch.root().to_path_buf()
            } else {
                scratch.root().join(&dir)
            };
            let init = commands::terraform_init(&tool.binary, &abs_dir, &mirror_refs);
            let out = self.run(tool_id, tool, &init, scratch)?;
            let mut found = parsed(
                tool_id,
                parsers::parse_terraform_init(&out.stdout, &out.stderr, out.code, &abs_dir, &known),
            )?;
            if found.is_empty() {
                let validate = commands::terraform_validate(&tool.binary, &abs_dir);
                let out = self.run(tool_id, tool, &validate, scratch)?;
                found = parsed(
                    tool_id,
                    parsers::parse_terraform_validate(&out.stdout, out.code, &abs_dir, &known),
                )?;
            }
            for item in found {
                if owned.contains(&item.file) {
                    if !findings.contains(&item) {
                        findings.push(item);
                    }
                } else {
                    reported.push(item.file);
                }
            }
        }
        if findings.is_empty() && !reported.is_empty() {
            let mut names: Vec<String> = staged
                .resolve_pairs
                .iter()
                .filter(|(_, absolute)| {
                    reported
                        .iter()
                        .any(|r| r.as_str() == absolute.to_string_lossy())
                })
                .map(|(ws, _)| ws.clone())
                .collect();
            names.sort();
            names.dedup();
            return Err(execution(
                tool_id,
                format!(
                    "terraform reported errors in dependencies before the owned files were checked: {}",
                    names.join(", ")
                ),
            ));
        }
        Ok(findings)
    }

    fn read_notebooks(
        tool_id: &str,
        pairs: &[(String, PathBuf)],
    ) -> Result<Vec<(String, String)>, RunnerError> {
        Self::read_notebooks_with(tool_id, pairs, quality_notebook::preflight)
    }

    /// Like `read_notebooks`, for tools that accept any kernel language.
    fn read_notebooks_any(
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

    pub(super) fn check_files(
        &self,
        tool_id: &str,
        tool: &RealTool,
        scratch: &Scratch,
        pairs: &[(String, PathBuf)],
        build: impl Fn(&[&Path]) -> Result<Invocation, RunnerError>,
        parse: impl Fn(&ChildOutput, &[&str]) -> Result<Vec<FileFinding>, RunnerError>,
    ) -> Result<Vec<FileFinding>, RunnerError> {
        let refs: Vec<&Path> = pairs
            .iter()
            .map(|(_, absolute)| absolute.as_path())
            .collect();
        let names: Vec<String> = pairs
            .iter()
            .map(|(_, absolute)| absolute.to_string_lossy().into_owned())
            .collect();
        let strs: Vec<&str> = names.iter().map(String::as_str).collect();
        let batches = match commands::transport_for(tool_id) {
            commands::ArgTransport::Batched => {
                let head = spawn_argv(tool_id, &build(&[])?);
                commands::plan_batches(&head, &refs)
            }
            commands::ArgTransport::Single => vec![refs.clone()],
        };
        let total = batches.len();
        let mut offset = 0usize;
        let mut merged: Vec<FileFinding> = Vec::new();
        for (index, batch) in batches.iter().enumerate() {
            let batch_names = &strs[offset..offset + batch.len()];
            offset += batch.len();
            let report = build(batch.as_slice())
                .and_then(|invocation| self.run(tool_id, tool, &invocation, scratch))
                .and_then(|out| parse(&out, batch_names))
                .map_err(|err| in_batch(err, index + 1, total))?;
            merged.extend(report);
        }
        merged.sort_by(|left, right| {
            (
                left.file.as_str(),
                left.finding.start.line,
                left.finding.start.column,
                left.finding.rule_id.as_str(),
                left.finding.message.as_str(),
            )
                .cmp(&(
                    right.file.as_str(),
                    right.finding.start.line,
                    right.finding.start.column,
                    right.finding.rule_id.as_str(),
                    right.finding.message.as_str(),
                ))
        });
        Ok(merged)
    }
}

/// Reads the roots file the unused shard stages; without one nothing is kept alive.
fn unused_roots(
    tool_id: &str,
    tool: &RealTool,
    pairs: &[(String, PathBuf)],
) -> Result<parsers::UnusedRoots, RunnerError> {
    let mut roots = parsers::UnusedRoots::default();
    let Some((_, bytes)) = tool
        .tool_files
        .iter()
        .find(|(rel, _)| rel == "dx-unused/roots.json")
    else {
        return Ok(roots);
    };
    let lists: std::collections::BTreeMap<String, Vec<String>> = serde_json::from_slice(bytes)
        .map_err(|err| execution(tool_id, format!("unused roots: {err}")))?;
    let named = |workspace: &String| {
        pairs
            .iter()
            .find(|(path, _)| path == workspace)
            .map(|(_, absolute)| absolute.to_string_lossy().into_owned())
    };
    for key in ["mains", "public"] {
        roots
            .exported
            .extend(lists.get(key).into_iter().flatten().filter_map(named));
    }
    roots
        .keep
        .extend(lists.get("keep").into_iter().flatten().filter_map(named));
    Ok(roots)
}

/// Drops the scratch directory from `file://` links so messages read the same on every host.
fn without_scratch_url(message: &str, root: &Path) -> String {
    let root = root.to_string_lossy().replace('\\', "/");
    let root = root.trim_start_matches('/');
    message.replace(&format!("file:///{root}/"), "")
}

/// Names the file whose output failed to parse.
pub(super) fn in_file(err: RunnerError, name: &str) -> RunnerError {
    match err {
        RunnerError::ToolExecution { tool_id, detail } => RunnerError::ToolExecution {
            tool_id,
            detail: format!("{name}: {detail}"),
        },
        RunnerError::ToolOutput { tool_id, detail } => RunnerError::ToolOutput {
            tool_id,
            detail: format!("{name}: {detail}"),
        },
        other => other,
    }
}

/// Names the failing batch in an error when a selection ran as several spawns.
fn in_batch(err: RunnerError, index: usize, total: usize) -> RunnerError {
    if total <= 1 {
        return err;
    }
    match err {
        RunnerError::ToolExecution { tool_id, detail } => RunnerError::ToolExecution {
            tool_id,
            detail: format!("batch {index}/{total}: {detail}"),
        },
        RunnerError::ToolOutput { tool_id, detail } => RunnerError::ToolOutput {
            tool_id,
            detail: format!("batch {index}/{total}: {detail}"),
        },
        other => other,
    }
}

/// Groups staged files by their directory under the scratch root, in sorted directory order.
///
/// Each member is the file name inside its directory, its scratch-root-relative path
/// (the spelling a `--chdir` tool reports) and its absolute path. A file at the root
/// belongs to the directory `.`. Reusable by any directory-scoped linter.
pub(super) type DirGroups = std::collections::BTreeMap<String, Vec<(String, String, String)>>;

pub(super) fn group_by_parent(root: &Path, pairs: &[(String, PathBuf)]) -> DirGroups {
    let mut groups = DirGroups::new();
    for (_, absolute) in pairs {
        let rel = absolute.strip_prefix(root).unwrap_or(absolute);
        let dir = rel
            .parent()
            .map(|dir| dir.to_string_lossy().replace('\\', "/"))
            .filter(|dir| !dir.is_empty())
            .unwrap_or_else(|| ".".to_owned());
        let name = rel
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        let rel = if dir == "." {
            name.clone()
        } else {
            format!("{dir}/{name}")
        };
        groups
            .entry(dir)
            .or_default()
            .push((name, rel, absolute.to_string_lossy().into_owned()));
    }
    for members in groups.values_mut() {
        members.sort();
    }
    groups
}

/// The package name of a CUE file: skips blank lines, `//` comments and file-level `@attr(...)`
/// lines before the clause, and drops a trailing `//` comment. Empty when there is no clause.
fn cue_package_clause(text: &str) -> String {
    text.lines()
        .map(str::trim)
        .find(|line| !line.is_empty() && !line.starts_with("//") && !line.starts_with('@'))
        .and_then(|line| line.strip_prefix("package "))
        .map(|rest| {
            let rest = rest.split("//").next().unwrap_or(rest);
            rest.trim().to_owned()
        })
        .unwrap_or_default()
}

/// The JVM finds its modules file beside the launcher it was started from, and a
/// runfiles symlink on Windows does not lead there, so the launcher is named by its real path.
fn real_launcher(path: &Path) -> PathBuf {
    let Ok(real) = std::fs::canonicalize(path) else {
        return path.to_path_buf();
    };
    match real.to_str().and_then(|text| text.strip_prefix(r"\\?\")) {
        Some(plain) => PathBuf::from(plain),
        None => real,
    }
}

#[cfg(test)]
mod cue_package_clause_tests {
    use super::cue_package_clause;

    #[test]
    fn clause_ignores_comments_and_attributes() {
        assert_eq!(cue_package_clause("package app\n"), "app");
        assert_eq!(cue_package_clause("package app // main\n"), "app");
        assert_eq!(
            cue_package_clause("// hi\n@if(prod)\n@extern(embed)\n\npackage app\n"),
            "app"
        );
        assert_eq!(cue_package_clause("x: 1\n"), "");
    }
}
