use quality_adapter::launch::ResolvedTool;
use quality_adapter::parsers::FileFinding;
use quality_adapter::{commands, parsers};

use super::check::ToolDef;
use super::*;

/// The standard tier's kinds, used when the consumer binds no markdown_check config.
const MARKDOWN_CHECK_DEFAULT_KINDS: &str =
    include_str!("../../../tiers/markdown_check/standard.txt");

/// Web, docs and config linters that build their own invocation and parse.
pub(super) const TOOLS: &[ToolDef] = &[
    ToolDef {
        id: "markdown_check",
        run: |backend, call| {
            let tool_id = call.tool_id;
            let tool = call.tool;
            let scratch = &call.staged.scratch;
            let pairs = &call.staged.pairs;
            let specs: Vec<(&str, &Path)> = pairs
                .iter()
                .map(|(workspace, absolute)| (workspace.as_str(), absolute.as_path()))
                .collect();
            let sibling_specs: Vec<(&str, &Path)> = call
                .staged
                .sibling_pairs
                .iter()
                .map(|(workspace, absolute)| (workspace.as_str(), absolute.as_path()))
                .collect();
            let kinds = match call.config {
                Some(path) => std::fs::read_to_string(path)
                    .map_err(|err| execution(tool_id, format!("read {}: {err}", path.display())))?,
                None => MARKDOWN_CHECK_DEFAULT_KINDS.to_owned(),
            };
            let kinds: Vec<&str> = kinds
                .lines()
                .filter(|line| !line.trim().is_empty())
                .collect();
            let invocation = commands::markdown_check(&tool.binary, &specs, &sibling_specs, &kinds);
            let out = backend.run(tool_id, tool, &invocation, scratch)?;
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
        },
    },
    ToolDef {
        id: "taplo",
        run: |backend, call| {
            let tool_id = call.tool_id;
            let tool = call.tool;
            let scratch = &call.staged.scratch;
            let pairs = &call.staged.pairs;
            let refs: Vec<&Path> = pairs
                .iter()
                .map(|(_, absolute)| absolute.as_path())
                .collect();
            let names: Vec<String> = pairs
                .iter()
                .map(|(_, absolute)| absolute.to_string_lossy().into_owned())
                .collect();
            let strs: Vec<&str> = names.iter().map(String::as_str).collect();
            let config = call.config;
            let capability = call.capability;
            let invocation = if capability == "format" {
                commands::taplo_format(&tool.binary, &refs, config, true)
            } else if capability == "typecheck" {
                commands::taplo_typecheck(&tool.binary, &refs, config)
            } else {
                commands::taplo_lint(&tool.binary, &refs, config)
            };
            let out = backend.run(tool_id, tool, &invocation, scratch)?;
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
                        config,
                        true,
                    );
                    let out = backend.run(tool_id, tool, &single, scratch)?;
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
        },
    },
    ToolDef {
        id: "vale",
        run: |backend, call| {
            let tool_id = call.tool_id;
            let tool = call.tool;
            let scratch = &call.staged.scratch;
            let pairs = &call.staged.pairs;
            let refs: Vec<&Path> = pairs
                .iter()
                .map(|(_, absolute)| absolute.as_path())
                .collect();
            let names: Vec<String> = pairs
                .iter()
                .map(|(_, absolute)| absolute.to_string_lossy().into_owned())
                .collect();
            let strs: Vec<&str> = names.iter().map(String::as_str).collect();
            let config = call.config;
            let cwd_rel = RealBackend::cwd_rel(tool_id, tool.config_rel.as_deref());
            let invocation = match config {
                Some(ini) => commands::vale_check(&tool.binary, ini, &refs, &cwd_rel),
                None => return Err(missing_tier_base(tool_id)),
            };
            let out = backend.run(tool_id, tool, &invocation, scratch)?;
            // Vale prints a runtime error envelope on stderr when stdout is empty.
            let report = if out.stdout.iter().all(u8::is_ascii_whitespace) {
                &out.stderr
            } else {
                &out.stdout
            };
            parsed(tool_id, parsers::parse_vale(report, out.code, &strs))
        },
    },
    ToolDef {
        id: "biome",
        run: |backend, call| {
            let tool_id = call.tool_id;
            let tool = call.tool;
            let scratch = &call.staged.scratch;
            let pairs = &call.staged.pairs;
            let refs: Vec<&Path> = pairs
                .iter()
                .map(|(_, absolute)| absolute.as_path())
                .collect();
            let capability = call.capability;
            let config_dir = RealBackend::biome_config_dir(tool, scratch)?;
            let invocation = if capability == "format" {
                commands::biome_format_check(&tool.binary, &refs, &config_dir)
            } else {
                commands::biome_lint_check(&tool.binary, &refs, &config_dir)
            };
            let out = backend.run(tool_id, tool, &invocation, scratch)?;
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
        },
    },
    ToolDef {
        id: "eslint",
        run: |backend, call| {
            let tool_id = call.tool_id;
            let tool = call.tool;
            let scratch = &call.staged.scratch;
            let pairs = &call.staged.pairs;
            let refs: Vec<&Path> = pairs
                .iter()
                .map(|(_, absolute)| absolute.as_path())
                .collect();
            let names: Vec<String> = pairs
                .iter()
                .map(|(_, absolute)| absolute.to_string_lossy().into_owned())
                .collect();
            let strs: Vec<&str> = names.iter().map(String::as_str).collect();
            let config = call.config;
            let launch = ResolvedTool::javascript(tool_id, &tool.binary);
            let invocation = match config {
                Some(cfg) => commands::eslint_check(&launch, &refs, cfg)
                    .map_err(|err| execution(tool_id, err.to_string()))?,
                None => {
                    return Err(missing_tier_base(tool_id));
                }
            };
            let out = backend.run(tool_id, tool, &invocation, scratch)?;
            parsed(tool_id, parsers::parse_eslint(&out.stdout, out.code, &strs))
        },
    },
    ToolDef {
        id: "gplint",
        run: |backend, call| {
            let tool_id = call.tool_id;
            let tool = call.tool;
            let scratch = &call.staged.scratch;
            let pairs = &call.staged.pairs;
            let config = call.config;
            // gplint reports realpaths, so lint and match the canonical ones.
            let launch = ResolvedTool::javascript(tool_id, &tool.binary);
            let Some(cfg) = config else {
                return Err(missing_tier_base(tool_id));
            };
            let mut owned: Vec<(PathBuf, &PathBuf)> = Vec::with_capacity(pairs.len());
            for (workspace, absolute) in pairs {
                let real = std::fs::canonicalize(absolute)
                    .map_err(|err| execution(tool_id, format!("resolve {workspace}: {err}")))?;
                owned.push((real, absolute));
            }
            let path_refs: Vec<&Path> = owned.iter().map(|(real, _)| real.as_path()).collect();
            let invocation = commands::gplint_check(&launch, &path_refs, cfg)
                .map_err(|err| execution(tool_id, err.to_string()))?;
            let out = backend.run(tool_id, tool, &invocation, scratch)?;
            if out.stdout.is_empty() {
                // A crash or a configuration error: gplint prints no report, and a crash exits 0.
                return Err(execution(
                    tool_id,
                    format!(
                        "gplint printed no report (exit {:?}): {}",
                        out.code,
                        String::from_utf8_lossy(&out.stderr).trim()
                    ),
                ));
            }
            let real_names: Vec<String> = owned
                .iter()
                .map(|(real, _)| real.to_string_lossy().into_owned())
                .collect();
            let real_strs: Vec<&str> = real_names.iter().map(String::as_str).collect();
            let mut findings = parsed(
                tool_id,
                parsers::parse_gplint(&out.stdout, out.code, &real_strs),
            )?;
            for found in &mut findings {
                // Messages such as the duplicate-name rules quote realpaths; name the workspace path.
                for ((real, _), (workspace, _)) in owned.iter().zip(pairs.iter()) {
                    found.finding.message = found
                        .finding
                        .message
                        .replace(real.to_string_lossy().as_ref(), workspace);
                }
                if let Some((_, absolute)) = owned
                    .iter()
                    .find(|(real, _)| real.to_string_lossy() == found.file.as_str())
                {
                    found.file = absolute.to_string_lossy().into_owned();
                }
            }
            Ok(findings)
        },
    },
    ToolDef {
        id: "stylelint",
        run: |backend, call| {
            let tool_id = call.tool_id;
            let tool = call.tool;
            let scratch = &call.staged.scratch;
            let pairs = &call.staged.pairs;
            let refs: Vec<&Path> = pairs
                .iter()
                .map(|(_, absolute)| absolute.as_path())
                .collect();
            let names: Vec<String> = pairs
                .iter()
                .map(|(_, absolute)| absolute.to_string_lossy().into_owned())
                .collect();
            let strs: Vec<&str> = names.iter().map(String::as_str).collect();
            let config = call.config;
            let launch = ResolvedTool::javascript(tool_id, &tool.binary);
            let Some(cfg) = config else {
                return Err(missing_tier_base(tool_id));
            };
            let invocation = commands::stylelint_check(&launch, &refs, cfg)
                .map_err(|err| execution(tool_id, err.to_string()))?;
            let out = backend.run(tool_id, tool, &invocation, scratch)?;
            let report = if out.stdout.is_empty() {
                &out.stderr
            } else {
                &out.stdout
            };
            parsed(tool_id, parsers::parse_stylelint(report, out.code, &strs))
        },
    },
    ToolDef {
        id: "graphql_eslint",
        run: |backend, call| {
            let tool_id = call.tool_id;
            let tool = call.tool;
            let scratch = &call.staged.scratch;
            let pairs = &call.staged.pairs;
            let refs: Vec<&Path> = pairs
                .iter()
                .map(|(_, absolute)| absolute.as_path())
                .collect();
            let names: Vec<String> = pairs
                .iter()
                .map(|(_, absolute)| absolute.to_string_lossy().into_owned())
                .collect();
            let strs: Vec<&str> = names.iter().map(String::as_str).collect();
            let config = call.config;
            let launch = ResolvedTool::javascript(tool_id, &tool.binary);
            let Some(cfg) = config else {
                return Err(missing_tier_base(tool_id));
            };
            let invocation = if call.capability == "typecheck" {
                commands::graphql_typecheck(&launch, &refs, cfg, scratch.root())
            } else {
                commands::graphql_eslint_check(&launch, &refs, cfg, scratch.root())
            }
            .map_err(|err| execution(tool_id, err.to_string()))?;
            let out = backend.run(tool_id, tool, &invocation, scratch)?;
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
        },
    },
    ToolDef {
        id: "prettier",
        run: |backend, call| {
            let tool_id = call.tool_id;
            let tool = call.tool;
            let scratch = &call.staged.scratch;
            let pairs = &call.staged.pairs;
            let refs: Vec<&Path> = pairs
                .iter()
                .map(|(_, absolute)| absolute.as_path())
                .collect();
            let config = call.config;
            let launch = ResolvedTool::javascript(tool_id, &tool.binary);
            let invocation = commands::prettier_check(&launch, &refs, config)
                .map_err(|err| execution(tool_id, err.to_string()))?;
            let out = backend.run(tool_id, tool, &invocation, scratch)?;
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
        },
    },
    ToolDef {
        id: "remark",
        run: |backend, call| {
            let tool_id = call.tool_id;
            let tool = call.tool;
            let scratch = &call.staged.scratch;
            let pairs = &call.staged.pairs;
            let refs: Vec<&Path> = pairs
                .iter()
                .map(|(_, absolute)| absolute.as_path())
                .collect();
            let names: Vec<String> = pairs
                .iter()
                .map(|(_, absolute)| absolute.to_string_lossy().into_owned())
                .collect();
            let strs: Vec<&str> = names.iter().map(String::as_str).collect();
            let config = call.config;
            let launch = ResolvedTool::javascript(tool_id, &tool.binary);
            let invocation = commands::remark_check(&launch, &refs, config)
                .map_err(|err| execution(tool_id, err.to_string()))?;
            let out = backend.run(tool_id, tool, &invocation, scratch)?;
            parsed(
                tool_id,
                parsers::parse_remark(&out.stdout, &out.stderr, out.code, &strs),
            )
        },
    },
    ToolDef {
        id: "djlint",
        run: |backend, call| {
            let tool_id = call.tool_id;
            let tool = call.tool;
            let scratch = &call.staged.scratch;
            let pairs = &call.staged.pairs;
            let names: Vec<String> = pairs
                .iter()
                .map(|(_, absolute)| absolute.to_string_lossy().into_owned())
                .collect();
            let strs: Vec<&str> = names.iter().map(String::as_str).collect();
            let config = call.config;
            let capability = call.capability;
            if capability != "format" && !tool.upstream_diagnostics.is_empty() {
                return backend.check_file_family_delegated(tool_id, tool, pairs);
            }
            let mut findings = Vec::new();
            for name in &strs {
                let file = [Path::new(name)];
                let one = [*name];
                if capability == "format" {
                    let invocation = commands::djlint_format_check(&tool.binary, &file, config);
                    let out = backend.run(tool_id, tool, &invocation, scratch)?;
                    let report = parsers::djlint_ran(&out.stderr)
                        .and_then(|()| parsers::parse_djlint_format(&out.stdout, out.code, &one));
                    findings.extend(parsed(tool_id, report)?);
                } else {
                    let invocation = commands::djlint_check(&tool.binary, &file, config);
                    let out = backend.run(tool_id, tool, &invocation, scratch)?;
                    let report = parsers::djlint_ran(&out.stderr)
                        .and_then(|()| parsers::parse_djlint(&out.stdout, out.code, &one));
                    findings.extend(parsed(tool_id, report)?);
                }
            }
            Ok(findings)
        },
    },
];
