use quality_adapter::{commands, parsers, TextPosition};

use super::check::{group_by_parent, without_scratch_url, ToolDef};
use super::*;

/// Config, policy and text tools that build their own invocation and parse.
pub(super) const TOOLS: &[ToolDef] = &[
    ToolDef {
        id: "cue",
        run: |backend, call| {
            let tool_id = call.tool_id;
            let tool = call.tool;
            let scratch = &call.staged.scratch;
            let (refs, names) = super::single_run::paths(call);
            let strs: Vec<&str> = names.iter().map(String::as_str).collect();
            let cwd_rel = RealBackend::cwd_rel(call.tool_id, call.tool.config_rel.as_deref());
            if call.capability == "typecheck" {
                return backend.check_cue_vet(call.tool_id, call.tool, call.staged);
            }
            let invocation = commands::cue_check(&tool.binary, &refs);
            let out = backend.run(tool_id, tool, &invocation, scratch)?;
            let cwd = scratch.root().join(&cwd_rel);
            parsed(
                tool_id,
                parsers::parse_cue(&out.stdout, &out.stderr, out.code, &strs, &cwd),
            )
        },
    },
    ToolDef {
        id: "regal",
        run: |backend, call| {
            let tool_id = call.tool_id;
            let tool = call.tool;
            let pairs = &call.staged.pairs;
            let scratch = &call.staged.scratch;
            let config = call.config;
            let Some(config) = config else {
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
            let out = backend.run(tool_id, tool, &invocation, scratch)?;
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
        },
    },
    ToolDef {
        id: "opa",
        run: |backend, call| {
            let tool_id = call.tool_id;
            let tool = call.tool;
            let capability = call.capability;
            let pairs = &call.staged.pairs;
            let resolve_pairs = &call.staged.resolve_pairs;
            let scratch = &call.staged.scratch;
            let (refs, names) = super::single_run::paths(call);
            let strs: Vec<&str> = names.iter().map(String::as_str).collect();
            if capability == "format" {
                let mut findings = Vec::new();
                for (workspace, absolute) in pairs {
                    let invocation = commands::opa_fmt_check(&tool.binary, absolute);
                    let out = backend.run(tool_id, tool, &invocation, scratch)?;
                    let input = std::fs::read(absolute)
                        .map_err(|err| execution(tool_id, format!("read {workspace}: {err}")))?;
                    let file = absolute.to_string_lossy();
                    findings.extend(parsed(
                        tool_id,
                        parsers::parse_opa_fmt(&out.stdout, &out.stderr, out.code, &input, &file),
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
                let out = backend.run(tool_id, tool, &invocation, scratch)?;
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
        },
    },
    ToolDef {
        id: "pkl",
        run: |backend, call| {
            let tool_id = call.tool_id;
            let tool = call.tool;
            let scratch = &call.staged.scratch;
            let (refs, names) = super::single_run::paths(call);
            let strs: Vec<&str> = names.iter().map(String::as_str).collect();
            if call.capability == "typecheck" {
                return backend.check_pkl_eval(call.tool_id, call.tool, call.staged);
            }
            let invocation = commands::pkl_check(&tool.binary, &refs);
            let out = backend.run(tool_id, tool, &invocation, scratch)?;
            parsed(
                tool_id,
                parsers::parse_pkl(&out.stdout, &out.stderr, out.code, &strs),
            )
        },
    },
    ToolDef {
        id: "modfmt",
        run: |backend, call| {
            let tool_id = call.tool_id;
            let tool = call.tool;
            let scratch = &call.staged.scratch;
            let (refs, names) = super::single_run::paths(call);
            let strs: Vec<&str> = names.iter().map(String::as_str).collect();
            let invocation = commands::modfmt_check(&tool.binary, &refs);
            let out = backend.run(tool_id, tool, &invocation, scratch)?;
            parsed(tool_id, parsers::parse_modfmt(&out.stdout, out.code, &strs))
        },
    },
    ToolDef {
        id: "tflint",
        run: |backend, call| {
            let tool_id = call.tool_id;
            let tool = call.tool;
            let pairs = &call.staged.pairs;
            let scratch = &call.staged.scratch;
            let config = call.config;
            let Some(config) = config else {
                return Err(execution(tool_id, "tflint needs a config path".to_owned()));
            };
            let mut merged = Vec::new();
            for (dir, members) in group_by_parent(scratch.root(), pairs) {
                let names: Vec<&Path> = members
                    .iter()
                    .map(|(name, _, _)| Path::new(name.as_str()))
                    .collect();
                let invocation = commands::tflint_check(&tool.binary, &dir, &names, config);
                let out = backend.run(tool_id, tool, &invocation, scratch)?;
                let reported: Vec<&str> = members.iter().map(|(_, rel, _)| rel.as_str()).collect();
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
        },
    },
    ToolDef {
        id: "terraform",
        run: |backend, call| {
            let tool_id = call.tool_id;
            let tool = call.tool;
            let scratch = &call.staged.scratch;
            let (refs, names) = super::single_run::paths(call);
            let strs: Vec<&str> = names.iter().map(String::as_str).collect();
            let cwd_rel = RealBackend::cwd_rel(call.tool_id, call.tool.config_rel.as_deref());
            if call.capability == "typecheck" {
                return backend.check_terraform_validate(call.tool_id, call.tool, call.staged);
            }
            let invocation = commands::terraform_check(&tool.binary, &refs);
            let out = backend.run(tool_id, tool, &invocation, scratch)?;
            let cwd = scratch.root().join(&cwd_rel);
            parsed(
                tool_id,
                parsers::parse_terraform(&out.stdout, &out.stderr, out.code, &strs, &cwd),
            )
        },
    },
    ToolDef {
        id: "yamlfmt",
        run: |backend, call| {
            let tool_id = call.tool_id;
            let tool = call.tool;
            let scratch = &call.staged.scratch;
            let config = call.config;
            let (refs, names) = super::single_run::paths(call);
            let strs: Vec<&str> = names.iter().map(String::as_str).collect();
            let invocation = commands::yamlfmt_check(&tool.binary, &refs, config);
            let out = backend.run(tool_id, tool, &invocation, scratch)?;
            parsed(
                tool_id,
                parsers::parse_yamlfmt(&out.stderr, out.code, &strs),
            )
        },
    },
    ToolDef {
        id: "shfmt",
        run: |backend, call| {
            let tool_id = call.tool_id;
            let tool = call.tool;
            let scratch = &call.staged.scratch;
            let (refs, names) = super::single_run::paths(call);
            let strs: Vec<&str> = names.iter().map(String::as_str).collect();
            let invocation = commands::shfmt_check(&tool.binary, &refs);
            let out = backend.run(tool_id, tool, &invocation, scratch)?;
            parsed(tool_id, parsers::parse_shfmt(&out.stdout, out.code, &strs))
        },
    },
    ToolDef {
        id: "psscriptanalyzer",
        run: |backend, call| {
            let tool_id = call.tool_id;
            let tool = call.tool;
            let capability = call.capability;
            let scratch = &call.staged.scratch;
            let config = call.config;
            let (refs, names) = super::single_run::paths(call);
            let strs: Vec<&str> = names.iter().map(String::as_str).collect();
            if capability == "format" {
                let invocation =
                    commands::psscriptanalyzer_format_check(&tool.binary, &refs, config);
                let out = backend.run(tool_id, tool, &invocation, scratch)?;
                return parsed(
                    tool_id,
                    parsers::parse_psscriptanalyzer_format(&out.stdout, out.code, &strs),
                );
            }
            let invocation = commands::psscriptanalyzer_check(&tool.binary, &refs, config);
            let out = backend.run(tool_id, tool, &invocation, scratch)?;
            parsed(
                tool_id,
                parsers::parse_psscriptanalyzer(&out.stdout, out.code, &strs),
            )
        },
    },
    ToolDef {
        id: "statix",
        run: |backend, call| {
            let tool_id = call.tool_id;
            let tool = call.tool;
            let pairs = &call.staged.pairs;
            let scratch = &call.staged.scratch;
            let config = call.config;
            let (_, names) = super::single_run::paths(call);
            let strs: Vec<&str> = names.iter().map(String::as_str).collect();
            if !tool.upstream_diagnostics.is_empty() {
                return backend.check_file_family_delegated(tool_id, tool, pairs);
            }
            let Some(config) = config else {
                return Err(execution(tool_id, "statix needs a config path".to_owned()));
            };
            let mut findings = Vec::new();
            for name in &strs {
                let invocation = commands::statix_check(&tool.binary, Path::new(name), config);
                let out = backend.run(tool_id, tool, &invocation, scratch)?;
                let report = parsers::parse_statix(&out.stdout, &out.stderr, out.code, &[*name]);
                findings.extend(parsed(tool_id, report)?);
            }
            Ok(findings)
        },
    },
    ToolDef {
        id: "trivy_config",
        run: |backend, call| {
            let tool_id = call.tool_id;
            let tool = call.tool;
            let scratch = &call.staged.scratch;
            let cache = scratch.root().join("dx-trivy-cache");
            if let Some(chart_root) = helm_chart_root(tool) {
                return scan_chart(backend, call, &cache, &chart_root);
            }
            if let Some((_, bicep)) = tool.path_tools.iter().find(|(name, _)| name == "bicep") {
                return scan_bicep(backend, call, &cache, bicep);
            }
            let (_, names) = super::single_run::paths(call);
            let mut findings = Vec::new();
            for name in &names {
                let invocation =
                    commands::trivy_config_check(&tool.binary, Path::new(name), &cache);
                let out = backend.run(tool_id, tool, &invocation, scratch)?;
                // A single-file scan names the target by its file name alone.
                let base = Path::new(name)
                    .file_name()
                    .map_or(name.as_str(), |base| base.to_str().unwrap_or(name));
                let report = parsers::parse_trivy_config(
                    &out.stdout,
                    &out.stderr,
                    out.code,
                    &[name.as_str(), base],
                );
                for mut found in parsed(tool_id, report)? {
                    found.file = name.clone();
                    findings.push(found);
                }
            }
            Ok(findings)
        },
    },
    ToolDef {
        id: "deadnix",
        run: |backend, call| {
            let tool_id = call.tool_id;
            let tool = call.tool;
            let pairs = &call.staged.pairs;
            let scratch = &call.staged.scratch;
            if !tool.upstream_diagnostics.is_empty() {
                return backend.check_file_family_delegated(tool_id, tool, pairs);
            }
            backend.check_files(
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
        },
    },
    ToolDef {
        id: "nixfmt",
        run: |backend, call| {
            let tool_id = call.tool_id;
            let tool = call.tool;
            let pairs = &call.staged.pairs;
            let scratch = &call.staged.scratch;
            backend.check_files(
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
            )
        },
    },
    ToolDef {
        id: "topiary",
        run: |backend, call| {
            let tool_id = call.tool_id;
            let tool = call.tool;
            let pairs = &call.staged.pairs;
            let scratch = &call.staged.scratch;
            backend.check_files(
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
            )
        },
    },
    ToolDef {
        id: "ansible_lint",
        run: |backend, call| {
            let tool_id = call.tool_id;
            let tool = call.tool;
            let scratch = &call.staged.scratch;
            let config = call.config;
            let (refs, names) = super::single_run::paths(call);
            let strs: Vec<&str> = names.iter().map(String::as_str).collect();
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
            let collection_refs: Vec<&Path> = collections.iter().map(PathBuf::as_path).collect();
            let context = commands::AnsibleLintContext {
                config,
                collections: &collection_refs,
                project_dir: scratch.root(),
            };
            let invocation = commands::ansible_lint_check(&tool.binary, &refs, &context);
            let out = backend.run(tool_id, tool, &invocation, scratch)?;
            parsed(
                tool_id,
                parsers::parse_ansible_lint(&out.stdout, &out.stderr, out.code, &strs),
            )
        },
    },
    ToolDef {
        id: "typos",
        run: |backend, call| {
            let tool_id = call.tool_id;
            let tool = call.tool;
            let pairs = &call.staged.pairs;
            let scratch = &call.staged.scratch;
            backend.check_files(
                tool_id,
                tool,
                scratch,
                pairs,
                |files| Ok(commands::typos_check(&tool.binary, files)),
                |out, names| parsed(tool_id, parsers::parse_typos(&out.stdout, out.code, names)),
            )
        },
    },
    ToolDef {
        id: "zizmor",
        run: |backend, call| {
            let tool_id = call.tool_id;
            let tool = call.tool;
            let scratch = &call.staged.scratch;
            let (refs, names) = super::single_run::paths(call);
            let strs: Vec<&str> = names.iter().map(String::as_str).collect();
            let invocation = commands::zizmor_check(&tool.binary, &refs);
            let out = backend.run(tool_id, tool, &invocation, scratch)?;
            parsed(
                tool_id,
                parsers::parse_zizmor(&out.stdout, &out.stderr, out.code, &strs),
            )
        },
    },
    ToolDef {
        id: "actionlint",
        run: |backend, call| {
            let tool_id = call.tool_id;
            let tool = call.tool;
            let pairs = &call.staged.pairs;
            let scratch = &call.staged.scratch;
            backend.check_files(
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
            )
        },
    },
    ToolDef {
        id: "lychee",
        run: |backend, call| {
            let tool_id = call.tool_id;
            let tool = call.tool;
            let pairs = &call.staged.pairs;
            let scratch = &call.staged.scratch;
            let root = scratch.root();
            let online = tool
                .extra_env
                .iter()
                .any(|(name, value)| name == "DX_LINT_ONLINE" && value == "1");
            let reported = backend.check_files(
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
        },
    },
    ToolDef {
        id: "keep_sorted",
        run: |backend, call| {
            let tool_id = call.tool_id;
            let tool = call.tool;
            let pairs = &call.staged.pairs;
            let scratch = &call.staged.scratch;
            if !tool.upstream_diagnostics.is_empty() {
                backend.check_file_family_delegated(tool_id, tool, pairs)
            } else {
                backend.check_files(
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
        },
    },
];

/// The chart root from the helm context the aspect stages for a helm_chart target.
fn helm_chart_root(tool: &RealTool) -> Option<String> {
    let (_, bytes) = tool
        .tool_files
        .iter()
        .find(|(rel, _)| rel == "dx-helm/context.json")?;
    let doc: serde_json::Value = serde_json::from_slice(bytes).ok()?;
    Some(doc.get("chart_root")?.as_str()?.to_owned())
}

/// Trivy renders a chart itself, so the chart directory is scanned once and each finding
/// lands on the chart file it names. Lines refer to the rendered manifest, so they reset to 1.
fn scan_chart(
    backend: &RealBackend,
    call: &super::check::CheckCall,
    cache: &Path,
    chart_root: &str,
) -> Result<Vec<parsers::FileFinding>, RunnerError> {
    let tool_id = call.tool_id;
    let scratch = &call.staged.scratch;
    let chart_abs = if chart_root.is_empty() {
        scratch.root().to_path_buf()
    } else {
        scratch
            .resolve(Path::new(chart_root))
            .map_err(|err| execution(tool_id, format!("chart root: {err}")))?
    };
    let staged: Vec<(String, &Path)> = call
        .staged
        .pairs
        .iter()
        .filter_map(|(_, absolute)| {
            let rel = absolute.strip_prefix(&chart_abs).ok()?;
            Some((rel.to_string_lossy().replace('\\', "/"), absolute.as_path()))
        })
        .collect();
    let relative: Vec<&str> = staged.iter().map(|(rel, _)| rel.as_str()).collect();
    let invocation = commands::trivy_config_check(&call.tool.binary, &chart_abs, cache);
    let out = backend.run(tool_id, call.tool, &invocation, scratch)?;
    let report = parsers::parse_trivy_config(&out.stdout, &out.stderr, out.code, &relative);
    let mut findings = parsed(tool_id, report)?;
    for found in &mut findings {
        if let Some((_, absolute)) = staged.iter().find(|(rel, _)| *rel == found.file) {
            found.file = absolute.to_string_lossy().into_owned();
        }
        found.finding.start = TextPosition { line: 1, column: 1 };
        found.finding.end = None;
    }
    Ok(findings)
}

const ARM_DIR_REL: &str = "dx-bicep-arm";

/// Trivy does not read Bicep, so each file is built to ARM JSON and the staged templates are
/// scanned together. ARM lines do not map to Bicep lines, so findings land on line 1.
fn scan_bicep(
    backend: &RealBackend,
    call: &super::check::CheckCall,
    cache: &Path,
    bicep: &Path,
) -> Result<Vec<parsers::FileFinding>, RunnerError> {
    let tool_id = call.tool_id;
    let scratch = &call.staged.scratch;
    let arm_dir = scratch.root().join(ARM_DIR_REL);
    std::fs::create_dir_all(&arm_dir)
        .map_err(|err| execution(tool_id, format!("arm dir: {err}")))?;
    let mut built: Vec<(String, &Path)> = Vec::new();
    for (index, (workspace, absolute)) in call.staged.pairs.iter().enumerate() {
        let invocation = commands::bicep_build_stdout(bicep, absolute);
        let out = backend.run(tool_id, call.tool, &invocation, scratch)?;
        if out.code != Some(0) {
            return Err(execution(
                tool_id,
                format!(
                    "bicep build {workspace} failed: {}",
                    String::from_utf8_lossy(&out.stderr).trim()
                ),
            ));
        }
        let name = format!("{index}.json");
        std::fs::write(arm_dir.join(&name), &out.stdout)
            .map_err(|err| execution(tool_id, format!("write {name}: {err}")))?;
        built.push((name, absolute.as_path()));
    }
    if built.is_empty() {
        return Ok(Vec::new());
    }
    let names: Vec<&str> = built.iter().map(|(name, _)| name.as_str()).collect();
    let invocation = commands::trivy_config_check(&call.tool.binary, &arm_dir, cache);
    let out = backend.run(tool_id, call.tool, &invocation, scratch)?;
    let report = parsers::parse_trivy_config(&out.stdout, &out.stderr, out.code, &names);
    let mut findings = parsed(tool_id, report)?;
    for found in &mut findings {
        if let Some((_, absolute)) = built.iter().find(|(name, _)| *name == found.file) {
            found.file = absolute.to_string_lossy().into_owned();
        }
        found.finding.start = TextPosition { line: 1, column: 1 };
        found.finding.end = None;
    }
    Ok(findings)
}
