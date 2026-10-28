use std::path::{Path, PathBuf};

use quality_adapter::launch::ResolvedTool;
use quality_adapter::parsers::FileFinding;
use quality_adapter::{commands, parsers};

use super::*;

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
                                parsers::parse_ruff_format(&out.stdout, out.code, names),
                            )
                        } else {
                            parsed(tool_id, parsers::parse_ruff(&out.stdout, out.code, names))
                        }
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
                parsed(tool_id, parsers::parse_vale(&out.stdout, out.code, &strs))
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
            "cue" => {
                let invocation = commands::cue_check(&tool.binary, &refs);
                let out = self.run(tool_id, tool, &invocation, scratch)?;
                let cwd = scratch.root().join(&cwd_rel);
                parsed(
                    tool_id,
                    parsers::parse_cue(&out.stdout, &out.stderr, out.code, &strs, &cwd),
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
            "erlfmt" => {
                let invocation = commands::erlfmt_check(&tool.binary, &refs);
                let out = self.run(tool_id, tool, &invocation, scratch)?;
                parsed(
                    tool_id,
                    parsers::parse_erlfmt(&out.stdout, &out.stderr, out.code, &strs),
                )
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
            "zig_fmt" => {
                let invocation = commands::zig_fmt_check(&tool.binary, &refs);
                let out = self.run(tool_id, tool, &invocation, scratch)?;
                parsed(
                    tool_id,
                    parsers::parse_zig_fmt(&out.stdout, &out.stderr, out.code, &strs),
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
            "rubocop" | "yamllint" | "shellcheck" | "hadolint" => {
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
