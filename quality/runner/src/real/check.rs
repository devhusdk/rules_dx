use std::path::{Path, PathBuf};

use quality_adapter::launch::ResolvedTool;
use quality_adapter::parsers::FileFinding;
use quality_adapter::{commands, parsers};

use super::*;

/// The rule level PHPStan runs at when the consumer binds no config.
const PHPSTAN_LEVEL: u8 = 5;

impl RealBackend {
    pub(super) fn run_check(
        &self,
        tool_id: &str,
        tool: &RealTool,
        capability: &str,
        staged: &StagedScratch,
    ) -> Result<Vec<FileFinding>, RunnerError> {
        let pairs = &staged.pairs;
        let sibling_pairs = &staged.sibling_pairs;
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
        match tool_id {
            "buildifier" => {
                let hint = hint_dir(tool.config_rel.as_deref(), &cwd_rel);
                let reported = self.check_files(
                    tool_id,
                    tool,
                    scratch,
                    pairs,
                    |files| Ok(commands::buildifier_check(&tool.binary, files, hint)),
                    |out, names| {
                        parsed(
                            tool_id,
                            parsers::parse_buildifier(&out.stdout, &out.stderr, names),
                        )
                    },
                )?;
                let mut kept = Vec::with_capacity(reported.len());
                for mut found in reported {
                    let syntax = found.finding.severity == quality_adapter::ToolSeverity::Error;
                    if syntax {
                        clamp_to_text(&mut found);
                        kept.push(found);
                    } else if (capability == "format") == found.finding.rule_id.is_empty() {
                        kept.push(found);
                    }
                }
                Ok(kept)
            }
            "clippy" => self.check_clippy_delegated(tool_id, tool, pairs),
            "rustc" => self.check_rustc_delegated(tool_id, tool, pairs),
            "markdown_check" => {
                let specs: Vec<(&str, &Path)> = pairs
                    .iter()
                    .map(|(workspace, absolute)| (workspace.as_str(), absolute.as_path()))
                    .collect();
                let sibling_specs: Vec<(&str, &Path)> = sibling_pairs
                    .iter()
                    .map(|(workspace, absolute)| (workspace.as_str(), absolute.as_path()))
                    .collect();
                let invocation = commands::markdown_check(&tool.binary, &specs, &sibling_specs);
                let out = self.run(tool_id, tool, &invocation, scratch)?;
                let workspaces: Vec<&str> = pairs
                    .iter()
                    .map(|(workspace, _)| workspace.as_str())
                    .collect();
                let reported = parsed(
                    tool_id,
                    parsers::parse_markdown_findings(&out.stdout, out.code, &workspaces),
                )?;
                let mut rerooted = Vec::with_capacity(reported.len());
                for found in reported {
                    rerooted.push(FileFinding {
                        file: reanchor(tool_id, pairs, &found.file)?
                            .to_string_lossy()
                            .into_owned(),
                        finding: found.finding,
                    });
                }
                Ok(rerooted)
            }
            "rustfmt" => self.check_rustfmt(tool, staged),
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
                let mut dirs: Vec<PathBuf> = Vec::new();
                for (_, absolute) in pairs.iter().chain(resolve_pairs.iter()) {
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
                let invocation = commands::ty_check(&tool.binary, &refs, &dir_refs);
                let out = self.run(tool_id, tool, &invocation, scratch)?;
                let workspaces: Vec<&str> = pairs
                    .iter()
                    .chain(resolve_pairs.iter())
                    .map(|(workspace, _)| workspace.as_str())
                    .collect();
                let mut findings = parsed(
                    tool_id,
                    parsers::parse_ty(&out.stdout, out.code, &workspaces),
                )?;
                let checked: std::collections::BTreeSet<&str> = pairs
                    .iter()
                    .map(|(workspace, _)| workspace.as_str())
                    .collect();
                findings.retain(|found| checked.contains(found.file.as_str()));
                for found in &mut findings {
                    let absolute = reanchor(tool_id, pairs, &found.file)?;
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
            "taplo" => {
                let invocation = if capability == "format" {
                    commands::taplo_format(&tool.binary, &refs, config.as_deref(), true)
                } else {
                    commands::taplo_lint(&tool.binary, &refs, config.as_deref())
                };
                let out = self.run(tool_id, tool, &invocation, scratch)?;
                if capability == "format" {
                    let batch = parsed(
                        tool_id,
                        parsers::parse_taplo_format_check(&out.stderr, out.code, &strs),
                    );
                    if batch.is_ok() || refs.len() < 2 {
                        return batch;
                    }
                    // Re-check each file alone to name the file taplo rejected.
                    let mut merged = Vec::new();
                    for (name, path) in strs.iter().zip(refs.iter()) {
                        let single = commands::taplo_format(
                            &tool.binary,
                            std::slice::from_ref(path),
                            config.as_deref(),
                            true,
                        );
                        let out = self.run(tool_id, tool, &single, scratch)?;
                        merged.extend(parsed(
                            tool_id,
                            parsers::parse_taplo_format_check(&out.stderr, out.code, &[*name]),
                        )?);
                    }
                    Ok(merged)
                } else {
                    parsed(
                        tool_id,
                        parsers::parse_taplo_lint(&out.stderr, out.code, &strs),
                    )
                }
            }
            "vale" => {
                let invocation = match config.as_ref() {
                    Some(ini) => commands::vale_check(&tool.binary, ini, &refs, &cwd_rel),
                    None => return Err(execution(tool_id, "vale requires a config".to_owned())),
                };
                let out = self.run(tool_id, tool, &invocation, scratch)?;
                // Vale prints a runtime error envelope on stderr when stdout is empty.
                let report = if out.stdout.iter().all(u8::is_ascii_whitespace) {
                    &out.stderr
                } else {
                    &out.stdout
                };
                parsed(tool_id, parsers::parse_vale(report, out.code, &strs))
            }
            "biome" => {
                let config_dir = Self::biome_config_dir(tool, scratch)?;
                let invocation = if capability == "format" {
                    commands::biome_format_check(&tool.binary, &refs, &config_dir)
                } else {
                    commands::biome_lint_check(&tool.binary, &refs, &config_dir)
                };
                let out = self.run(tool_id, tool, &invocation, scratch)?;
                let workspaces: Vec<&str> = pairs
                    .iter()
                    .map(|(workspace, _)| workspace.as_str())
                    .collect();
                let report = if capability == "format" {
                    parsers::parse_biome_format(&out.stdout, out.code, &workspaces)
                } else {
                    parsers::parse_biome_lint(&out.stdout, out.code, &workspaces)
                };
                let mut findings = parsed(tool_id, report)?;
                for found in &mut findings {
                    let absolute = reanchor(tool_id, pairs, &found.file)?;
                    found.file = absolute.to_string_lossy().into_owned();
                }
                Ok(findings)
            }
            "eslint" => {
                let launch = ResolvedTool::javascript(tool_id, &tool.binary);
                let invocation = match config.as_ref() {
                    Some(cfg) => commands::eslint_check(&launch, &refs, cfg)
                        .map_err(|err| execution(tool_id, err.to_string()))?,
                    None => {
                        return Err(execution(tool_id, "eslint requires a config".to_owned()));
                    }
                };
                let out = self.run(tool_id, tool, &invocation, scratch)?;
                parsed(tool_id, parsers::parse_eslint(&out.stdout, out.code, &strs))
            }
            "stylelint" => {
                let launch = ResolvedTool::javascript(tool_id, &tool.binary);
                let Some(cfg) = config.as_ref() else {
                    return Err(execution(tool_id, "stylelint requires a config".to_owned()));
                };
                let invocation = commands::stylelint_check(&launch, &refs, cfg)
                    .map_err(|err| execution(tool_id, err.to_string()))?;
                let out = self.run(tool_id, tool, &invocation, scratch)?;
                let report = if out.stdout.is_empty() {
                    &out.stderr
                } else {
                    &out.stdout
                };
                parsed(tool_id, parsers::parse_stylelint(report, out.code, &strs))
            }
            "redocly" => self.check_redocly(tool_id, tool, staged, config.as_deref()),
            "graphql_eslint" => {
                let launch = ResolvedTool::javascript(tool_id, &tool.binary);
                let Some(cfg) = config.as_ref() else {
                    return Err(execution(
                        tool_id,
                        "graphql_eslint requires a config".to_owned(),
                    ));
                };
                let invocation =
                    commands::graphql_eslint_check(&launch, &refs, cfg, scratch.root())
                        .map_err(|err| execution(tool_id, err.to_string()))?;
                let out = self.run(tool_id, tool, &invocation, scratch)?;
                if !matches!(out.code, Some(0 | 1)) {
                    let code = out
                        .code
                        .map_or_else(|| "signal".to_owned(), |code| code.to_string());
                    let stderr = String::from_utf8_lossy(&out.stderr);
                    return Err(execution(
                        tool_id,
                        format!("exit {code}: {}", stderr.trim()),
                    ));
                }
                parsed(
                    tool_id,
                    parsers::parse_graphql_eslint(&out.stdout, out.code, &strs),
                )
            }
            "prettier" => {
                let launch = ResolvedTool::javascript(tool_id, &tool.binary);
                let invocation = commands::prettier_check(
                    &launch,
                    &refs,
                    config.as_deref(),
                    tool.dialect.as_deref(),
                )
                .map_err(|err| execution(tool_id, err.to_string()))?;
                let out = self.run(tool_id, tool, &invocation, scratch)?;
                let workspaces: Vec<&str> = pairs
                    .iter()
                    .map(|(workspace, _)| workspace.as_str())
                    .collect();
                let report = parsers::parse_prettier_check_with_sources(
                    &out.stderr,
                    out.code,
                    &workspaces,
                    |workspace| {
                        let (_, absolute) = pairs.iter().find(|(known, _)| known == workspace)?;
                        std::fs::read_to_string(absolute).ok()
                    },
                );
                let mut findings = parsed(tool_id, report)?;
                for found in &mut findings {
                    let absolute = reanchor(tool_id, pairs, &found.file)?;
                    found.file = absolute.to_string_lossy().into_owned();
                }
                Ok(findings)
            }
            "remark" => {
                let launch = ResolvedTool::javascript(tool_id, &tool.binary);
                let invocation = commands::remark_check(&launch, &refs, config.as_deref())
                    .map_err(|err| execution(tool_id, err.to_string()))?;
                let out = self.run(tool_id, tool, &invocation, scratch)?;
                parsed(
                    tool_id,
                    parsers::parse_remark(&out.stdout, &out.stderr, out.code, &strs),
                )
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
            "roslyn" => self.check_roslyn_delegated(tool_id, tool, pairs),
            "csc" => self.check_csc_delegated(tool_id, tool, pairs),
            "clang_syntax" => self.check_clang_syntax_delegated(tool_id, tool, pairs),
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
                    .map_err(|err| with_stderr(err, &out.stderr))
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
                let invocation = commands::pmd_check(&tool.binary, &refs, config.as_deref());
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
                let invocation = commands::spotbugs_check(&tool.binary, &targets, &aux_refs)
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
                let neon = work.join("dx.neon");
                std::fs::write(
                    &neon,
                    commands::phpstan_neon(
                        consumer.as_deref(),
                        PHPSTAN_LEVEL,
                        &tmp_dir,
                        &scan_refs,
                    ),
                )
                .map_err(|err| execution(tool_id, format!("write phpstan config: {err}")))?;
                let path_refs: Vec<&Path> = owned.iter().map(|(real, _)| real.as_path()).collect();
                let invocation = commands::phpstan_check(&tool.binary, &neon, &path_refs);
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
            "ktlint" => {
                let invocation = commands::ktlint_check(&tool.binary, &refs, config.as_deref());
                let out = self.run(tool_id, tool, &invocation, scratch)?;
                parsed(tool_id, parsers::parse_ktlint(&out.stdout, out.code, &strs))
            }
            "clang_format" => {
                let invocation =
                    commands::clang_format_check(&tool.binary, &refs, config.as_deref());
                let out = self.run(tool_id, tool, &invocation, scratch)?;
                parsed(
                    tool_id,
                    parsers::parse_clang_format(&out.stderr, out.code, &strs),
                )
            }
            "gofumpt" => {
                let lang = Self::gofumpt_lang(tool)?;
                let invocation = commands::gofumpt_check(&tool.binary, &refs, lang);
                let out = self.run(tool_id, tool, &invocation, scratch)?;
                parsed(
                    tool_id,
                    parsers::parse_gofumpt(&out.stdout, out.code, &strs),
                )
            }
            "clang_tidy" => {
                if !tool.upstream_diagnostics.is_empty() {
                    self.check_clang_tidy_delegated(tool_id, tool, pairs)
                } else {
                    Err(execution(
                        tool_id,
                        "clang_tidy requires upstream diagnostics from a declared delegated action: direct execution without the authoritative compilation context is rejected".to_owned(),
                    ))
                }
            }
            "cppcheck" => {
                if !tool.upstream_diagnostics.is_empty() {
                    self.check_cppcheck_delegated(tool_id, tool, pairs)
                } else {
                    let invocation =
                        commands::cppcheck_check(&tool.binary, &refs, config.as_deref());
                    let out = self.run(tool_id, tool, &invocation, scratch)?;
                    parsed(
                        tool_id,
                        parsers::parse_cppcheck(&out.stderr, out.code, &strs),
                    )
                }
            }
            "staticcheck" => {
                if !tool.upstream_diagnostics.is_empty() {
                    self.check_staticcheck_delegated(tool_id, tool, pairs)
                } else {
                    let invocation = commands::staticcheck_check(
                        &tool.binary,
                        &refs,
                        hint_dir(tool.config_rel.as_deref(), &cwd_rel),
                    );
                    let out = self.run(tool_id, tool, &invocation, scratch)?;
                    parsed(
                        tool_id,
                        parsers::parse_staticcheck(&out.stdout, out.code, &strs),
                    )
                }
            }
            "govet" => {
                if !tool.upstream_diagnostics.is_empty() {
                    self.check_govet_delegated(tool_id, tool, pairs)
                } else {
                    let invocation = commands::govet_check(&tool.binary, &refs);
                    let out = self.run(tool_id, tool, &invocation, scratch)?;
                    parsed(tool_id, parsers::parse_govet(&out.stderr, out.code, &strs))
                }
            }
            "errcheck" => {
                if !tool.upstream_diagnostics.is_empty() {
                    self.check_errcheck_delegated(tool_id, tool, pairs)
                } else {
                    let invocation = commands::errcheck_check(&tool.binary, &refs);
                    let out = self.run(tool_id, tool, &invocation, scratch)?;
                    parsed(
                        tool_id,
                        parsers::parse_errcheck(&out.stdout, out.code, &strs),
                    )
                }
            }
            "buf" => {
                let root = scratch.root().join(&cwd_rel);
                if capability == "format" {
                    let mut findings = Vec::new();
                    for (workspace, absolute) in pairs {
                        let invocation = commands::buf_format_check(
                            &tool.binary,
                            &[absolute.as_path()],
                            &root,
                            config.as_deref(),
                        );
                        let out = self.run(tool_id, tool, &invocation, scratch)?;
                        let input = std::fs::read(absolute).map_err(|err| {
                            execution(tool_id, format!("read {workspace}: {err}"))
                        })?;
                        let found = parsed(
                            tool_id,
                            parsers::parse_buf_format(
                                &out.stdout,
                                &out.stderr,
                                out.code,
                                &input,
                                workspace,
                            ),
                        )?;
                        for mut item in found {
                            item.file = absolute.to_string_lossy().into_owned();
                            findings.push(item);
                        }
                    }
                    Ok(findings)
                } else {
                    let mut linted: Vec<(PathBuf, &Path)> = Vec::with_capacity(pairs.len());
                    for (workspace, absolute) in pairs {
                        let import_copy =
                            import_path_copy(tool_id, workspace, absolute, resolve_pairs)?;
                        linted.push((import_copy.unwrap_or_else(|| absolute.clone()), absolute));
                    }
                    let targets: Vec<&Path> =
                        linted.iter().map(|(copy, _)| copy.as_path()).collect();
                    let names: Vec<String> = targets
                        .iter()
                        .map(|copy| copy.to_string_lossy().into_owned())
                        .collect();
                    let names: Vec<&str> = names.iter().map(String::as_str).collect();
                    let mut context: Vec<(String, &str)> = Vec::new();
                    for (import, copy) in resolve_pairs {
                        if linted.iter().any(|(linted, _)| linted == copy) {
                            continue;
                        }
                        context.push((copy.to_string_lossy().into_owned(), import.as_str()));
                        if let Ok(relative) = copy.strip_prefix(&root) {
                            context
                                .push((relative.to_string_lossy().into_owned(), import.as_str()));
                        }
                    }
                    let context: Vec<(&str, &str)> = context
                        .iter()
                        .map(|(copy, import)| (copy.as_str(), *import))
                        .collect();
                    let invocation =
                        commands::buf_lint_check(&tool.binary, &targets, &root, config.as_deref());
                    let out = self.run(tool_id, tool, &invocation, scratch)?;
                    let found = parsed(
                        tool_id,
                        parsers::parse_buf_lint(&out.stdout, out.code, &names, &context),
                    )?;
                    let mut owned = Vec::with_capacity(found.len());
                    for mut item in found {
                        if let Some((_, absolute)) = linted
                            .iter()
                            .find(|(copy, _)| copy.to_string_lossy() == item.file)
                        {
                            item.file = absolute.to_string_lossy().into_owned();
                        }
                        owned.push(item);
                    }
                    Ok(owned)
                }
            }
            "flatc" => {
                let root = std::fs::canonicalize(scratch.root()).map_err(|err| {
                    execution(tool_id, format!("canonicalize scratch root: {err}"))
                })?;
                let out_dir = root.join(".dx-flatc-out");
                let mut findings = Vec::new();
                for (workspace, absolute) in pairs {
                    // flatc prints realpaths, so check and report the canonical one.
                    let file = std::fs::canonicalize(absolute)
                        .map_err(|err| execution(tool_id, format!("resolve {workspace}: {err}")))?;
                    let invocation = commands::flatc_check(&tool.binary, &file, &root, &out_dir);
                    let out = self.run(tool_id, tool, &invocation, scratch)?;
                    if !matches!(out.code, Some(0 | 1)) {
                        let code = out
                            .code
                            .map_or_else(|| "signal".to_owned(), |code| code.to_string());
                        let stderr = String::from_utf8_lossy(&out.stderr);
                        return Err(execution(
                            tool_id,
                            format!("exit {code}: {}", stderr.trim()),
                        ));
                    }
                    let name = file.to_string_lossy().into_owned();
                    let report = parsed(
                        tool_id,
                        parsers::parse_flatc(&out.stdout, &out.stderr, out.code, &[name.as_str()]),
                    )?;
                    declared_input_escape(tool_id, workspace, &root, &report.dependencies)?;
                    let scratch_prefix = format!("{}{}", root.display(), std::path::MAIN_SEPARATOR);
                    for mut item in report.findings {
                        // Typecheck is the error-only view of the same flatc run.
                        if capability == "typecheck"
                            && !matches!(
                                item.finding.severity,
                                quality_adapter::ToolSeverity::Error
                            )
                        {
                            continue;
                        }
                        item.file = absolute.to_string_lossy().into_owned();
                        // Messages name other schemas by scratch path; keep them workspace-relative.
                        item.finding.message = item.finding.message.replace(&scratch_prefix, "");
                        findings.push(item);
                    }
                }
                Ok(findings)
            }
            "qmlformat" => {
                let mut findings = Vec::new();
                for (workspace, absolute) in pairs {
                    let invocation = commands::qmlformat_check(
                        &tool.binary,
                        &[absolute.as_path()],
                        config.as_deref(),
                    );
                    let out = self.run(tool_id, tool, &invocation, scratch)?;
                    let input = std::fs::read(absolute)
                        .map_err(|err| execution(tool_id, format!("read {workspace}: {err}")))?;
                    let found = parsed(
                        tool_id,
                        parsers::parse_qmlformat(&out.stdout, out.code, &input, workspace),
                    )?;
                    for mut item in found {
                        item.file = absolute.to_string_lossy().into_owned();
                        findings.push(item);
                    }
                }
                Ok(findings)
            }
            "qmllint" => {
                if !tool.upstream_diagnostics.is_empty() {
                    self.check_qmllint_delegated(tool_id, tool, pairs)
                } else {
                    let roots = super::staging::qml_import_roots(tool_id, resolve_pairs)?;
                    let root_refs: Vec<&Path> = roots.iter().map(PathBuf::as_path).collect();
                    if let Some(settings) = config.as_deref() {
                        super::staging::place_qmllint_settings(tool_id, settings, pairs)?;
                    }
                    let invocation =
                        commands::qmllint_check(&tool.binary, &refs, &root_refs, config.is_some());
                    let out = self.run(tool_id, tool, &invocation, scratch)?;
                    parsed(
                        tool_id,
                        parsers::parse_qmllint(&out.stdout, out.code, &strs),
                    )
                }
            }
            "cue" if capability == "typecheck" => self.check_cue_vet(tool_id, tool, staged),
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
            "fortitude" => {
                // fortitude parses free-form source only; it reports a valid fixed-form `.f`
                // or `.F` file as a syntax error, so those are not handed to it.
                let free_form: Vec<&Path> = refs
                    .iter()
                    .copied()
                    .filter(|path| {
                        !path
                            .extension()
                            .is_some_and(|ext| ext.eq_ignore_ascii_case("f"))
                    })
                    .collect();
                if free_form.is_empty() {
                    return Ok(Vec::new());
                }
                let invocation = commands::fortitude_check(&tool.binary, &free_form);
                let out = self.run(tool_id, tool, &invocation, scratch)?;
                let free_strs: Vec<&str> = free_form.iter().filter_map(|p| p.to_str()).collect();
                parsed(
                    tool_id,
                    parsers::parse_fortitude(&out.stdout, out.code, &free_strs),
                )
            }
            "jsonnet_lint" => {
                let invocation = commands::jsonnet_lint_check(&tool.binary, &refs);
                let out = self.run(tool_id, tool, &invocation, scratch)?;
                parsed(
                    tool_id,
                    parsers::parse_jsonnet_lint(&out.stdout, &out.stderr, out.code, &strs),
                )
            }
            "jsonnetfmt" => {
                let invocation = commands::jsonnetfmt_check(&tool.binary, &refs);
                let out = self.run(tool_id, tool, &invocation, scratch)?;
                parsed(
                    tool_id,
                    parsers::parse_jsonnetfmt(&out.stdout, &out.stderr, out.code, &strs),
                )
            }
            "dfmt" => {
                // One process per file: with two or more files dfmt edits them in place.
                let mut findings = Vec::new();
                for (workspace, absolute) in pairs {
                    let invocation = commands::dfmt_check(&tool.binary, absolute.as_path());
                    let out = self.run(tool_id, tool, &invocation, scratch)?;
                    let input = std::fs::read(absolute)
                        .map_err(|err| execution(tool_id, format!("read {workspace}: {err}")))?;
                    let name = absolute.to_string_lossy();
                    let found = parsed(
                        tool_id,
                        parsers::parse_dfmt(&out.stdout, &out.stderr, out.code, &input, &name),
                    )?;
                    findings.extend(found);
                }
                Ok(findings)
            }
            "dscanner" => {
                // dscanner silently skips a file it cannot read, so read each one first.
                for (workspace, absolute) in pairs {
                    std::fs::read(absolute)
                        .map_err(|err| execution(tool_id, format!("read {workspace}: {err}")))?;
                }
                let invocation = commands::dscanner_check(&tool.binary, &refs);
                let out = self.run(tool_id, tool, &invocation, scratch)?;
                parsed(
                    tool_id,
                    parsers::parse_dscanner(&out.stdout, out.code, &strs),
                )
            }
            "typstyle" => {
                let invocation = commands::typstyle_check(&tool.binary, &refs);
                let out = self.run(tool_id, tool, &invocation, scratch)?;
                let cwd = scratch.root().join(&cwd_rel);
                parsed(
                    tool_id,
                    parsers::parse_typstyle(&out.stdout, &out.stderr, out.code, &strs, &cwd),
                )
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
            "dart" if capability != "format" => {
                // The analyzer prints canonical paths, so check and report the canonical ones.
                let root = std::fs::canonicalize(scratch.root()).map_err(|err| {
                    execution(tool_id, format!("canonicalize scratch root: {err}"))
                })?;
                let mut files: Vec<(String, PathBuf)> = Vec::with_capacity(pairs.len());
                for (workspace, absolute) in pairs {
                    let file = std::fs::canonicalize(absolute)
                        .map_err(|err| execution(tool_id, format!("resolve {workspace}: {err}")))?;
                    files.push((workspace.clone(), file));
                }
                let file_refs: Vec<&Path> = files.iter().map(|(_, file)| file.as_path()).collect();
                let file_names: Vec<String> = files
                    .iter()
                    .map(|(_, file)| file.to_string_lossy().into_owned())
                    .collect();
                let file_strs: Vec<&str> = file_names.iter().map(String::as_str).collect();
                let options = root.join("analysis_options.yaml");
                let cache = root.join(".dx-dart-analysis");
                let invocation = commands::dart_analyze(&tool.binary, &cache, &options, &file_refs);
                let out = self.run(tool_id, tool, &invocation, scratch)?;
                let view = if capability == "typecheck" {
                    parsers::DartAnalyzeView::Typecheck
                } else {
                    parsers::DartAnalyzeView::Lint
                };
                let found = parsed(
                    tool_id,
                    parsers::parse_dart_analyze(
                        &out.stdout,
                        &out.stderr,
                        out.code,
                        &file_strs,
                        &options.to_string_lossy(),
                        view,
                    ),
                )?;
                let scratch_prefix = format!("{}{}", root.display(), std::path::MAIN_SEPARATOR);
                let mut owned = Vec::with_capacity(found.len());
                for mut item in found {
                    if let Some(at) = file_names.iter().position(|name| *name == item.file) {
                        item.file = names[at].clone();
                    }
                    item.finding.message = item.finding.message.replace(&scratch_prefix, "");
                    owned.push(item);
                }
                Ok(owned)
            }
            "dart" => {
                let invocation = commands::dart_format_check(&tool.binary, &refs);
                let out = self.run(tool_id, tool, &invocation, scratch)?;
                parsed(
                    tool_id,
                    parsers::parse_dart_format(&out.stdout, &out.stderr, out.code, &strs),
                )
            }
            "julia_format" => {
                let invocation = commands::julia_format_check(&tool.binary, &refs);
                let out = self.run(tool_id, tool, &invocation, scratch)?;
                parsed(
                    tool_id,
                    parsers::parse_julia_format(&out.stdout, &out.stderr, out.code, &strs),
                )
            }
            "zig_fmt" => {
                let invocation = commands::zig_fmt_check(&tool.binary, &refs);
                let out = self.run(tool_id, tool, &invocation, scratch)?;
                parsed(
                    tool_id,
                    parsers::parse_zig_fmt(&out.stdout, &out.stderr, out.code, &strs),
                )
            }
            "latexindent" => {
                // One process per file: latexindent takes a single document. Its log and
                // backups go to a scratch directory, never next to the source.
                let cruft = Self::latexindent_cruft(tool_id, scratch)?;
                let mut merged = Vec::new();
                for (name, path) in strs.iter().zip(refs.iter()) {
                    let invocation =
                        commands::latexindent_check(&tool.binary, path, &cruft, config.as_deref());
                    let out = self.run(tool_id, tool, &invocation, scratch)?;
                    let found = parsed(
                        tool_id,
                        parsers::parse_latexindent(&out.stdout, &out.stderr, out.code, &[*name]),
                    )
                    .map_err(|err| in_file(err, name))?;
                    merged.extend(found);
                }
                Ok(merged)
            }
            "gersemi" => {
                let Some(config) = config.as_deref() else {
                    return Err(execution(tool_id, "gersemi needs a config path".to_owned()));
                };
                let invocation = commands::gersemi_check(&tool.binary, &refs, config);
                let out = self.run(tool_id, tool, &invocation, scratch)?;
                parsed(
                    tool_id,
                    parsers::parse_gersemi(&out.stdout, &out.stderr, out.code, &strs),
                )
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
            "djlint" => {
                if capability != "format" && !tool.upstream_diagnostics.is_empty() {
                    return self.check_file_family_delegated(tool_id, tool, pairs);
                }
                let mut findings = Vec::new();
                for name in &strs {
                    let file = [Path::new(name)];
                    let one = [*name];
                    if capability == "format" {
                        let invocation =
                            commands::djlint_format_check(&tool.binary, &file, config.as_deref());
                        let out = self.run(tool_id, tool, &invocation, scratch)?;
                        let report = parsers::djlint_ran(&out.stderr).and_then(|()| {
                            parsers::parse_djlint_format(&out.stdout, out.code, &one)
                        });
                        findings.extend(parsed(tool_id, report)?);
                    } else {
                        let invocation =
                            commands::djlint_check(&tool.binary, &file, config.as_deref());
                        let out = self.run(tool_id, tool, &invocation, scratch)?;
                        let report = parsers::djlint_ran(&out.stderr)
                            .and_then(|()| parsers::parse_djlint(&out.stdout, out.code, &one));
                        findings.extend(parsed(tool_id, report)?);
                    }
                }
                Ok(findings)
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
            "kubeconform" => self.check_kubeconform(tool_id, tool, staged, config.as_deref()),
            "helm" => self.check_helm(tool_id, tool, staged, config.as_deref()),
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

    /// Runs one tool over the selection, in argv-budget batches when its transport allows.
    ///
    /// Findings merge in a stable order, and a failing batch names itself when there are
    /// several so the error points at the spawn that broke.
    /// Reads every staged notebook and refuses any that Ruff would skip, so a
    /// non-Python or malformed notebook never reports clean.
    /// `cue vet -c=false` once per package, by import path from the enclosing CUE module root, so
    /// imports and ancestor-directory files of the same package resolve as in the real instance.
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

    fn check_files(
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

/// Names the file whose output failed to parse.
fn in_file(err: RunnerError, name: &str) -> RunnerError {
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

/// Lexically resolves `.` and `..` without touching the filesystem.
fn lexical_normal(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for part in path.components() {
        match part {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// Fails when a dependency a tool reports from a read-only dependency scan
/// lies outside the canonical scratch `root`, so a lookup that escaped the
/// declared inputs is never reported clean.
fn declared_input_escape(
    tool_id: &str,
    source: &str,
    root: &Path,
    dependencies: &[String],
) -> Result<(), RunnerError> {
    for dependency in dependencies {
        let normal = lexical_normal(Path::new(dependency));
        let inside = std::fs::canonicalize(&normal).is_ok_and(|real| real.starts_with(root));
        if !inside {
            return Err(execution(
                tool_id,
                format!("include '{dependency}' is outside the declared inputs of {source}"),
            ));
        }
    }
    Ok(())
}

fn import_path_copy(
    tool_id: &str,
    workspace: &str,
    absolute: &Path,
    resolve_pairs: &[(String, PathBuf)],
) -> Result<Option<PathBuf>, RunnerError> {
    let bytes = std::fs::read(absolute)
        .map_err(|err| execution(tool_id, format!("read {workspace}: {err}")))?;
    let name = Path::new(workspace).file_name();
    let mut same = Vec::new();
    for (path, copy) in resolve_pairs {
        if Path::new(path).file_name() != name {
            continue;
        }
        let other =
            std::fs::read(copy).map_err(|err| execution(tool_id, format!("read {path}: {err}")))?;
        if other == bytes {
            same.push((path, copy));
        }
    }
    let chosen = same
        .iter()
        .find(|(path, _)| workspace.ends_with(path.as_str()))
        .or(same.first());
    Ok(chosen.map(|(_, copy)| (*copy).clone()))
}

/// Buildifier reports an unexpected end of file one line past the last one.
fn clamp_to_text(found: &mut FileFinding) {
    let Ok(text) = std::fs::read_to_string(&found.file) else {
        return;
    };
    let start = found.finding.start;
    let encoding = found.finding.encoding;
    if quality_adapter::line_col_to_byte(&text, start.line, start.column, encoding).is_some() {
        return;
    }
    let lines = text.split_inclusive('\n').count().max(1);
    let last = text.split_inclusive('\n').next_back().unwrap_or("");
    let body = last.strip_suffix('\n').unwrap_or(last);
    found.finding.start = quality_adapter::TextPosition {
        line: lines as u64,
        column: quality_adapter::column_width(body, encoding) + 1,
    };
}

#[cfg(test)]
mod clamp_tests {
    use quality_adapter::{ColumnEncoding, Finding, TextPosition, ToolSeverity};

    use super::{clamp_to_text, FileFinding};

    fn clamped(text: &str, encoding: ColumnEncoding) -> (TextPosition, Option<u64>) {
        let dir = tempfile::tempdir().expect("scratch");
        let path = dir.path().join("BUILD.bazel");
        std::fs::write(&path, text).expect("write");
        let mut found = FileFinding {
            file: path.to_string_lossy().into_owned(),
            finding: Finding {
                tool_id: "buildifier".to_owned(),
                rule_id: String::new(),
                message: "unexpected end of file".to_owned(),
                severity: ToolSeverity::Error,
                start: TextPosition {
                    line: 99,
                    column: 1,
                },
                end: None,
                suggestions: Vec::new(),
                encoding,
                related: Vec::new(),
            },
        };
        clamp_to_text(&mut found);
        let start = found.finding.start;
        let byte = quality_adapter::line_col_to_byte(text, start.line, start.column, encoding);
        (start, byte)
    }

    #[test]
    fn clamp_lands_on_a_non_ascii_last_line_in_the_finding_units() {
        let text = "x = 1\ny = \"\u{1F600}\u{e9}\"";
        let (start, byte) = clamped(text, ColumnEncoding::Utf16);
        assert_eq!((start.line, start.column), (2, 10));
        assert_eq!(byte, Some(text.len() as u64));
        for encoding in [
            ColumnEncoding::Scalars,
            ColumnEncoding::Utf8,
            ColumnEncoding::ScalarsTabs { width: 4 },
        ] {
            let (_, byte) = clamped(&format!("{text}\t\r"), encoding);
            assert_eq!(byte, Some(text.len() as u64 + 2), "{encoding:?}");
        }
    }
}

#[cfg(test)]
mod declared_input_tests {
    use super::declared_input_escape;

    #[test]
    fn dependencies_inside_the_root_pass_and_escapes_fail() {
        let dir = tempfile::tempdir().expect("scratch");
        let root = std::fs::canonicalize(dir.path()).expect("root");
        std::fs::create_dir_all(root.join("root/pkg")).expect("pkg");
        std::fs::write(root.join("root/pkg/a.fbs"), "table A {}").expect("a");
        std::fs::write(root.join("outside.fbs"), "table O {}").expect("outside");
        let inner = root.join("root");
        let ok = vec![inner.join("pkg/a.fbs").to_string_lossy().into_owned()];
        declared_input_escape("flatc", "pkg/a.fbs", &inner, &ok).expect("inside");
        let up = vec![inner
            .join("pkg/../../outside.fbs")
            .to_string_lossy()
            .into_owned()];
        let err = declared_input_escape("flatc", "pkg/a.fbs", &inner, &up).expect_err("escape");
        assert!(err
            .to_string()
            .contains("is outside the declared inputs of pkg/a.fbs"));
        let missing = vec![inner.join("pkg/none.fbs").to_string_lossy().into_owned()];
        assert!(declared_input_escape("flatc", "pkg/a.fbs", &inner, &missing).is_err());
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
