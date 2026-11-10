use quality_adapter::{TextPosition, commands, parsers};

use super::check::{ToolDef, group_by_parent, without_scratch_url};
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
                    return Err(dependency_failure(
                        tool_id,
                        "opa check failed in dependencies before the owned files were type checked",
                        &resolve_pairs,
                        &reported,
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
        id: "trivy_config",
        run: |backend, call| {
            let tool_id = call.tool_id;
            let tool = call.tool;
            let scratch = &call.staged.scratch;
            let cache = scratch.root().join("dx-trivy-cache");
            let severities = tier_lines(tool_id, call.config, TRIVY_DEFAULT_SEVERITIES)?;
            let severities: Vec<&str> = severities.iter().map(String::as_str).collect();
            if let Some(chart_root) = helm_chart_root(tool) {
                return scan_chart(backend, call, &cache, &severities, &chart_root);
            }
            if let Some((_, bicep)) = tool.path_tools.iter().find(|(name, _)| name == "bicep") {
                return scan_bicep(backend, call, &cache, &severities, bicep);
            }
            let (_, names) = super::single_run::paths(call);
            let mut findings = Vec::new();
            for name in &names {
                let invocation = commands::trivy_config_check(
                    &tool.binary,
                    Path::new(name),
                    &cache,
                    &severities,
                );
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
        id: "trivy_image_config",
        run: |backend, call| {
            let tool_id = call.tool_id;
            let tool = call.tool;
            let scratch = &call.staged.scratch;
            let cache = scratch.root().join("dx-trivy-cache");
            let Some(layout) = tool.args.first() else {
                return Err(execution(
                    tool_id,
                    "trivy_image_config needs the image layout path".to_owned(),
                ));
            };
            let source = std::env::current_dir()
                .map_err(|err| execution(tool_id, format!("working directory: {err}")))?
                .join(layout);
            let layout = scratch.root().join(OCI_LAYOUT_DIR_REL);
            copy_layout(&source, &layout).map_err(|err| {
                execution(tool_id, format!("copy {}: {err}", source.display()))
            })?;
            let (_, names) = super::single_run::paths(call);
            let Some(file) = names.first() else {
                return Ok(Vec::new());
            };
            let name = source
                .file_name()
                .map_or("image", |base| base.to_str().unwrap_or("image"));
            let invocation = commands::trivy_image_config_check(&tool.binary, &layout, &cache);
            let out = backend.run(tool_id, tool, &invocation, scratch)?;
            parsed(
                tool_id,
                parsers::parse_trivy_image_config(
                    &out.stdout,
                    &out.stderr,
                    out.code,
                    &layout.to_string_lossy(),
                    name,
                    file,
                ),
            )
        },
    },
    ToolDef {
        id: "container_structure_test",
        run: |backend, call| {
            let tool_id = call.tool_id;
            let tool = call.tool;
            let scratch = &call.staged.scratch;
            let Some((layout, configs)) = tool.args.split_first() else {
                return Err(execution(
                    tool_id,
                    "container_structure_test needs the image layout path".to_owned(),
                ));
            };
            if configs.is_empty() {
                return Ok(Vec::new());
            }
            let cwd = std::env::current_dir()
                .map_err(|err| execution(tool_id, format!("working directory: {err}")))?;
            let source = cwd.join(layout);
            let archive = scratch.root().join(DOCKER_ARCHIVE_REL);
            layout_to_docker_archive(&source, &archive).map_err(|err| {
                execution(tool_id, format!("archive {}: {err}", source.display()))
            })?;
            let (_, names) = super::single_run::paths(call);
            let Some(file) = names.first() else {
                return Ok(Vec::new());
            };
            let name = source
                .file_name()
                .map_or("image", |base| base.to_str().unwrap_or("image"));
            let absolute: Vec<PathBuf> = configs.iter().map(|config| cwd.join(config)).collect();
            let refs: Vec<&Path> = absolute.iter().map(PathBuf::as_path).collect();
            let invocation =
                commands::container_structure_test_check(&tool.binary, &archive, &refs);
            let out = backend.run(tool_id, tool, &invocation, scratch)?;
            parsed(
                tool_id,
                parsers::parse_container_structure_test(
                    &out.stdout,
                    &out.stderr,
                    out.code,
                    name,
                    file,
                ),
            )
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
            let checks = tier_lines(tool_id, call.config, DEADNIX_STANDARD)?;
            let reports = |name: &str| checks.iter().any(|check| check == name);
            let (lambda_arg, lambda_pattern_names) =
                (reports("lambda-arg"), reports("lambda-pattern-names"));
            backend.check_files(
                tool_id,
                tool,
                scratch,
                pairs,
                |files| {
                    Ok(commands::deadnix_check(
                        &tool.binary,
                        files,
                        lambda_arg,
                        lambda_pattern_names,
                    ))
                },
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
        id: "helmfmt",
        run: |backend, call| {
            let tool_id = call.tool_id;
            let tool = call.tool;
            let staged = call.staged;
            let scratch = &staged.scratch;
            let known = |path: &str| {
                staged
                    .pairs
                    .iter()
                    .chain(&staged.sibling_pairs)
                    .any(|(ws, _)| ws == path)
                    || scratch.root().join(path).is_file()
            };
            let templates: Vec<(String, std::path::PathBuf)> = staged
                .pairs
                .iter()
                .filter(|(ws, _)| commands::is_helm_template(ws, known))
                .cloned()
                .collect();
            if templates.is_empty() {
                return Ok(Vec::new());
            }
            backend.check_files(
                tool_id,
                tool,
                scratch,
                &templates,
                |files| Ok(commands::helmfmt_check(&tool.binary, files)),
                |out, names| {
                    parsed(
                        tool_id,
                        parsers::parse_helmfmt(&out.stdout, &out.stderr, out.code, names),
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
                |files| Ok(commands::typos_check(&tool.binary, files, call.config)),
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
            let persona = tier_lines(tool_id, call.config, ZIZMOR_DEFAULT_PERSONA)?.remove(0);
            let invocation = commands::zizmor_check(&tool.binary, &refs, &persona);
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
                |files| {
                    Ok(commands::lychee_check(
                        &tool.binary,
                        files,
                        root,
                        online,
                        call.config,
                    ))
                },
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
    severities: &[&str],
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
    let invocation = commands::trivy_config_check(&call.tool.binary, &chart_abs, cache, severities);
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

const TRIVY_DEFAULT_SEVERITIES: &str = include_str!("../../../tiers/trivy_config/standard.txt");
const ZIZMOR_DEFAULT_PERSONA: &str = include_str!("../../../tiers/zizmor/standard.txt");

/// The non-empty lines of the bound tier file, or of the standard tier when none is bound.
fn tier_lines(
    tool_id: &str,
    config: Option<&Path>,
    standard: &str,
) -> Result<Vec<String>, RunnerError> {
    let text = match config {
        Some(path) => std::fs::read_to_string(path)
            .map_err(|err| execution(tool_id, format!("read {}: {err}", path.display())))?,
        None => standard.to_owned(),
    };
    Ok(text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect())
}

const ARM_DIR_REL: &str = "dx-bicep-arm";

const DOCKER_ARCHIVE_REL: &str = "dx-oci-image.tar";

fn invalid(detail: String) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidData, detail)
}

fn layout_blob(layout: &Path, digest: &str) -> std::io::Result<(String, PathBuf)> {
    let hex = digest
        .strip_prefix("sha256:")
        .filter(|hex| !hex.is_empty() && hex.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .ok_or_else(|| invalid(format!("unsupported digest {digest:?}")))?;
    Ok((
        format!("blobs/sha256/{hex}"),
        layout.join("blobs").join("sha256").join(hex),
    ))
}

fn digest_of<'a>(value: &'a serde_json::Value, what: &str) -> std::io::Result<&'a str> {
    value
        .get("digest")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| invalid(format!("{what} has no digest")))
}

fn ustar_header(name: &str, size: u64) -> std::io::Result<[u8; 512]> {
    if name.len() > 100 {
        return Err(invalid(format!("member name too long: {name}")));
    }
    let mut header = [0u8; 512];
    header[..name.len()].copy_from_slice(name.as_bytes());
    header[100..108].copy_from_slice(b"0000644\0");
    header[108..116].copy_from_slice(b"0000000\0");
    header[116..124].copy_from_slice(b"0000000\0");
    header[124..136].copy_from_slice(format!("{size:011o}\0").as_bytes());
    header[136..148].copy_from_slice(b"00000000000\0");
    header[148..156].copy_from_slice(b"        ");
    header[156] = b'0';
    header[257..263].copy_from_slice(b"ustar\0");
    header[263..265].copy_from_slice(b"00");
    let sum: u32 = header.iter().map(|byte| u32::from(*byte)).sum();
    header[148..156].copy_from_slice(format!("{sum:06o}\0 ").as_bytes());
    Ok(header)
}

fn append_member(
    out: &mut impl std::io::Write,
    name: &str,
    size: u64,
    body: &mut impl std::io::Read,
) -> std::io::Result<()> {
    out.write_all(&ustar_header(name, size)?)?;
    let copied = std::io::copy(body, out)?;
    if copied != size {
        return Err(invalid(format!("{name} changed while archiving")));
    }
    let pad = (512 - size % 512) % 512;
    out.write_all(&vec![0u8; pad as usize])
}

/// Writes the first image of an OCI layout as the docker archive the tar driver reads: a
/// `manifest.json` naming the config and layer blobs, followed by those blobs. An index is
/// followed to its first manifest.
fn layout_to_docker_archive(layout: &Path, archive: &Path) -> std::io::Result<()> {
    let read_json = |path: &Path| -> std::io::Result<serde_json::Value> {
        serde_json::from_slice(&std::fs::read(path)?)
            .map_err(|err| invalid(format!("{}: {err}", path.display())))
    };
    let mut document = read_json(&layout.join("index.json"))?;
    let manifest = loop {
        let Some(next) = document
            .get("manifests")
            .and_then(serde_json::Value::as_array)
        else {
            break document;
        };
        let first = next
            .first()
            .ok_or_else(|| invalid("index lists no manifest".to_owned()))?;
        let (_, path) = layout_blob(layout, digest_of(first, "index entry")?)?;
        document = read_json(&path)?;
    };
    let config = manifest
        .get("config")
        .ok_or_else(|| invalid("manifest has no config".to_owned()))?;
    let layers = manifest
        .get("layers")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| invalid("manifest has no layers".to_owned()))?;
    let mut members = vec![layout_blob(layout, digest_of(config, "config")?)?];
    for layer in layers {
        let member = layout_blob(layout, digest_of(layer, "layer")?)?;
        if !members.iter().any(|(name, _)| *name == member.0) {
            members.push(member);
        }
    }
    let listing = serde_json::json!([{
        "Config": members[0].0,
        "RepoTags": [],
        "Layers": layers
            .iter()
            .map(|layer| digest_of(layer, "layer").and_then(|digest| layout_blob(layout, digest)))
            .map(|member| member.map(|(name, _)| name))
            .collect::<std::io::Result<Vec<_>>>()?,
    }])
    .to_string();
    let mut out = std::io::BufWriter::new(std::fs::File::create(archive)?);
    append_member(
        &mut out,
        "manifest.json",
        listing.len() as u64,
        &mut listing.as_bytes(),
    )?;
    for (name, path) in &members {
        let size = std::fs::metadata(path)?.len();
        append_member(&mut out, name, size, &mut std::fs::File::open(path)?)?;
    }
    std::io::Write::write_all(&mut out, &[0u8; 1024])?;
    std::io::Write::flush(&mut out)
}

const OCI_LAYOUT_DIR_REL: &str = "dx-oci-layout";

/// Copies the layout with its symlinked blobs resolved to regular files.
fn copy_layout(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if std::fs::metadata(entry.path())?.is_dir() {
            copy_layout(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

/// Trivy does not read Bicep, so each file is built to ARM JSON and the staged templates are
/// scanned together. ARM lines do not map to Bicep lines, so findings land on line 1.
fn scan_bicep(
    backend: &RealBackend,
    call: &super::check::CheckCall,
    cache: &Path,
    severities: &[&str],
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
    let invocation = commands::trivy_config_check(&call.tool.binary, &arm_dir, cache, severities);
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
