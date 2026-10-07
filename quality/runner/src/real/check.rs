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
                let invocation = commands::buildifier_check(
                    &tool.binary,
                    &refs,
                    hint_dir(tool.config_rel.as_deref(), &cwd_rel),
                );
                let out = self.run(tool_id, tool, &invocation, scratch)?;
                let reported = parsed(
                    tool_id,
                    parsers::parse_buildifier(&out.stdout, &out.stderr, &strs),
                )?;
                let mut kept = Vec::with_capacity(reported.len());
                for found in reported {
                    if (capability == "format") == found.finding.rule_id.is_empty() {
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
            "rustfmt" => {
                let Some(cfg) = config.as_ref() else {
                    return Err(execution(tool_id, "rustfmt requires a config".to_owned()));
                };
                let invocation =
                    commands::rustfmt(&tool.binary, &refs, cfg, Self::rustfmt_edition(tool)?, true);
                let out = self.run(tool_id, tool, &invocation, scratch)?;
                parsed(
                    tool_id,
                    parsers::parse_rustfmt(&out.stdout, &out.stderr, out.code, &strs),
                )
            }
            "ruff" => {
                let config_ref = config.as_deref();
                let invocation = if capability == "format" {
                    commands::ruff_format_check(&tool.binary, &refs, config_ref)
                } else {
                    commands::ruff_check(&tool.binary, &refs, config_ref)
                };
                let out = self.run(tool_id, tool, &invocation, scratch)?;
                if capability == "format" {
                    parsed(
                        tool_id,
                        parsers::parse_ruff_format(&out.stdout, out.code, &strs),
                    )
                } else {
                    parsed(tool_id, parsers::parse_ruff(&out.stdout, out.code, &strs))
                }
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
            "pydoclint" => {
                let invocation = commands::pydoclint_check(&tool.binary, &refs);
                let out = self.run(tool_id, tool, &invocation, scratch)?;
                parsed(
                    tool_id,
                    parsers::parse_pydoclint(&out.stderr, out.code, &strs),
                )
            }
            "flake8" => {
                let invocation = commands::flake8_check(&tool.binary, &refs);
                let out = self.run(tool_id, tool, &invocation, scratch)?;
                parsed(tool_id, parsers::parse_flake8(&out.stdout, out.code, &strs))
            }
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
                    parsed(
                        tool_id,
                        parsers::parse_taplo_format_check(&out.stderr, out.code, &strs),
                    )
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
            "prettier" => {
                let launch = ResolvedTool::javascript(tool_id, &tool.binary);
                let invocation = commands::prettier_check(&launch, &refs)
                    .map_err(|err| execution(tool_id, err.to_string()))?;
                let out = self.run(tool_id, tool, &invocation, scratch)?;
                let workspaces: Vec<&str> = pairs
                    .iter()
                    .map(|(workspace, _)| workspace.as_str())
                    .collect();
                let report = parsers::parse_prettier_check(&out.stderr, out.code, &workspaces);
                let mut findings = parsed(tool_id, report)?;
                for found in &mut findings {
                    let absolute = reanchor(tool_id, pairs, &found.file)?;
                    found.file = absolute.to_string_lossy().into_owned();
                }
                Ok(findings)
            }
            "scalafmt" => {
                let invocation = commands::scalafmt_check(&tool.binary, &refs, config.as_deref());
                let out = self.run(tool_id, tool, &invocation, scratch)?;
                parsed(
                    tool_id,
                    parsers::parse_scalafmt(&out.stdout, out.code, &strs),
                )
            }
            "scalafix" => {
                if !tool.upstream_diagnostics.is_empty() {
                    self.check_scalafix_delegated(tool_id, tool, pairs)
                } else {
                    let invocation =
                        commands::scalafix_check(&tool.binary, &refs, None, None, None);
                    let out = self.run(tool_id, tool, &invocation, scratch)?;
                    parsed(
                        tool_id,
                        parsers::parse_scalafix(&out.stdout, out.code, &strs),
                    )
                }
            }
            "csharpier" => {
                let invocation = commands::csharpier_check(&tool.binary, &refs, config.as_deref());
                let out = self.run(tool_id, tool, &invocation, scratch)?;
                let workspaces: Vec<&str> = pairs
                    .iter()
                    .map(|(workspace, _)| workspace.as_str())
                    .collect();
                let mut findings = parsed(
                    tool_id,
                    parsers::parse_csharpier(&out.stdout, out.code, &workspaces),
                )?;
                for found in &mut findings {
                    let absolute = reanchor(tool_id, pairs, &found.file)?;
                    found.file = absolute.to_string_lossy().into_owned();
                }
                Ok(findings)
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
                    parsers::parse_fantomas(&out.stdout, out.code, &workspaces),
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
                    let invocation =
                        commands::fsharplint_check(&tool.binary, &refs, None, config.as_deref());
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
                let invocation = match config.as_ref() {
                    Some(cfg) => commands::checkstyle_check(&tool.binary, &refs, cfg),
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
                    parsers::parse_checkstyle(&out.stdout, out.code, &strs),
                )
            }
            "pmd" => {
                let invocation = commands::pmd_check(&tool.binary, &refs, config.as_deref());
                let out = self.run(tool_id, tool, &invocation, scratch)?;
                parsed(tool_id, parsers::parse_pmd(&out.stdout, out.code, &strs))
            }
            "spotbugs" => {
                let jar_targets: Vec<PathBuf> = tool
                    .tool_files
                    .iter()
                    .filter(|(rel, _)| rel.ends_with(".jar"))
                    .map(|(rel, _)| scratch.root().join(rel))
                    .collect();
                let targets: Vec<&Path> = jar_targets.iter().map(PathBuf::as_path).collect();
                let invocation = commands::spotbugs_check(&tool.binary, &targets);
                let out = self.run(tool_id, tool, &invocation, scratch)?;
                parsed(
                    tool_id,
                    parsers::parse_spotbugs(&out.stdout, out.code, &strs),
                )
            }
            "ktlint" => {
                let invocation = commands::ktlint_check(&tool.binary, &refs);
                let out = self.run(tool_id, tool, &invocation, scratch)?;
                parsed(tool_id, parsers::parse_ktlint(&out.stdout, out.code, &strs))
            }
            "clang_format" => {
                let invocation =
                    commands::clang_format_check(&tool.binary, &refs, config.as_deref());
                let out = self.run(tool_id, tool, &invocation, scratch)?;
                parsed(
                    tool_id,
                    parsers::parse_clang_format(&out.stdout, out.code, &strs),
                )
            }
            "gofumpt" => {
                let invocation = commands::gofumpt_check(&tool.binary, &refs);
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
                if capability == "format" {
                    let invocation = commands::buf_format_check(&tool.binary, &refs);
                    let out = self.run(tool_id, tool, &invocation, scratch)?;
                    parsed(
                        tool_id,
                        parsers::parse_buf_format(&out.stdout, out.code, &strs),
                    )
                } else if !tool.upstream_diagnostics.is_empty() {
                    self.check_buf_lint_delegated(tool_id, tool, pairs)
                } else {
                    let invocation = commands::buf_lint_check(&tool.binary, &refs);
                    let out = self.run(tool_id, tool, &invocation, scratch)?;
                    parsed(
                        tool_id,
                        parsers::parse_buf_lint(&out.stdout, out.code, &strs),
                    )
                }
            }
            "qmlformat" => {
                let invocation = commands::qmlformat_check(&tool.binary, &refs);
                let out = self.run(tool_id, tool, &invocation, scratch)?;
                let workspaces: Vec<&str> = pairs
                    .iter()
                    .map(|(workspace, _)| workspace.as_str())
                    .collect();
                let mut findings = parsed(
                    tool_id,
                    parsers::parse_qmlformat(&out.stdout, out.code, &workspaces),
                )?;
                for found in &mut findings {
                    let absolute = reanchor(tool_id, pairs, &found.file)?;
                    found.file = absolute.to_string_lossy().into_owned();
                }
                Ok(findings)
            }
            "qmllint" => {
                if !tool.upstream_diagnostics.is_empty() {
                    self.check_qmllint_delegated(tool_id, tool, pairs)
                } else {
                    let invocation = commands::qmllint_check(&tool.binary, &refs);
                    let out = self.run(tool_id, tool, &invocation, scratch)?;
                    let workspaces: Vec<&str> = pairs
                        .iter()
                        .map(|(workspace, _)| workspace.as_str())
                        .collect();
                    let mut findings = parsed(
                        tool_id,
                        parsers::parse_qmllint(&out.stdout, out.code, &workspaces),
                    )?;
                    for found in &mut findings {
                        let absolute = reanchor(tool_id, pairs, &found.file)?;
                        found.file = absolute.to_string_lossy().into_owned();
                    }
                    Ok(findings)
                }
            }
            "cue" => {
                let invocation = commands::cue_check(&tool.binary, &refs);
                let out = self.run(tool_id, tool, &invocation, scratch)?;
                parsed(tool_id, parsers::parse_cue(&out.stdout, out.code, &strs))
            }
            "jsonnetfmt" => {
                let invocation = commands::jsonnetfmt_check(&tool.binary, &refs);
                let out = self.run(tool_id, tool, &invocation, scratch)?;
                parsed(
                    tool_id,
                    parsers::parse_jsonnetfmt(&out.stdout, out.code, &strs),
                )
            }
            "pkl" => {
                let invocation = commands::pkl_check(&tool.binary, &refs);
                let out = self.run(tool_id, tool, &invocation, scratch)?;
                parsed(tool_id, parsers::parse_pkl(&out.stdout, out.code, &strs))
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
                    parsers::parse_terraform(&out.stdout, out.code, &strs, &cwd),
                )
            }
            "yamlfmt" => {
                let invocation = commands::yamlfmt_check(&tool.binary, &refs);
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
                let invocation = commands::standardrb_check(&tool.binary, &refs);
                let out = self.run(tool_id, tool, &invocation, scratch)?;
                parsed(
                    tool_id,
                    parsers::parse_standardrb(&out.stdout, out.code, &strs),
                )
            }
            "djlint" => {
                if capability == "format" {
                    let invocation = commands::djlint_format_check(&tool.binary, &refs);
                    let out = self.run(tool_id, tool, &invocation, scratch)?;
                    parsed(
                        tool_id,
                        parsers::parse_djlint_format(&out.stdout, out.code, &strs),
                    )
                } else if !tool.upstream_diagnostics.is_empty() {
                    self.check_file_family_delegated(tool_id, tool, pairs)
                } else {
                    let invocation = commands::djlint_check(&tool.binary, &refs, config.as_deref());
                    let out = self.run(tool_id, tool, &invocation, scratch)?;
                    parsed(tool_id, parsers::parse_djlint(&out.stdout, out.code, &strs))
                }
            }
            "stylelint" | "rubocop" | "psscriptanalyzer" | "yamllint" | "shellcheck"
            | "keep_sorted" => {
                if !tool.upstream_diagnostics.is_empty() {
                    self.check_file_family_delegated(tool_id, tool, pairs)
                } else {
                    let invocation = match tool_id {
                        "stylelint" => {
                            commands::stylelint_check(&tool.binary, &refs, config.as_deref())
                        }
                        "rubocop" => commands::rubocop_check(&tool.binary, &refs),
                        "psscriptanalyzer" => commands::psscriptanalyzer_check(&tool.binary, &refs),
                        "yamllint" => {
                            commands::yamllint_check(&tool.binary, &refs, config.as_deref())
                        }
                        "shellcheck" => commands::shellcheck_check(&tool.binary, &refs),
                        _ => commands::keep_sorted_check(&tool.binary, &refs),
                    };
                    let out = self.run(tool_id, tool, &invocation, scratch)?;
                    let report = match tool_id {
                        "stylelint" => parsers::parse_stylelint(&out.stdout, out.code, &strs),
                        "rubocop" => parsers::parse_rubocop(&out.stdout, out.code, &strs),
                        "psscriptanalyzer" => {
                            parsers::parse_psscriptanalyzer(&out.stdout, out.code, &strs)
                        }
                        "yamllint" => parsers::parse_yamllint(&out.stdout, out.code, &strs),
                        "shellcheck" => parsers::parse_shellcheck(&out.stdout, out.code, &strs),
                        _ => parsers::parse_keep_sorted(&out.stdout, out.code, &strs),
                    };
                    parsed(tool_id, report)
                }
            }
            _ => Err(execution(
                tool_id,
                format!("unsupported real tool: {tool_id}"),
            )),
        }
    }
}
