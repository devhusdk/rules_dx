use quality_adapter::{commands, parsers};

use super::check::ToolDef;
use super::*;

/// JVM and .NET formatters and linters.
pub(super) const TOOLS: &[ToolDef] = &[
    ToolDef {
        id: "scalafmt",
        run: |backend, call| {
            let names: Vec<String> = call
                .staged
                .pairs
                .iter()
                .map(|(_, absolute)| absolute.to_string_lossy().into_owned())
                .collect();
            let strs: Vec<&str> = names.iter().map(String::as_str).collect();
            let tool_id = call.tool_id;
            let tool = call.tool;
            let scratch = &call.staged.scratch;
            let pairs = &call.staged.pairs;
            let refs: Vec<&Path> = pairs
                .iter()
                .map(|(_, absolute)| absolute.as_path())
                .collect();
            let cwd_rel = RealBackend::cwd_rel(tool_id, tool.config_rel.as_deref());
            {
                let invocation = commands::scalafmt_check(&tool.binary, &refs, call.config);
                let out = backend.run(tool_id, tool, &invocation, scratch)?;
                let cwd = scratch.root().join(&cwd_rel);
                parsed(
                    tool_id,
                    parsers::parse_scalafmt(&out.stdout, &out.stderr, out.code, &strs, &cwd),
                )
            }
        },
    },
    ToolDef {
        id: "scalafix",
        run: |backend, call| {
            let names: Vec<String> = call
                .staged
                .pairs
                .iter()
                .map(|(_, absolute)| absolute.to_string_lossy().into_owned())
                .collect();
            let strs: Vec<&str> = names.iter().map(String::as_str).collect();
            let tool_id = call.tool_id;
            let tool = call.tool;
            let scratch = &call.staged.scratch;
            let pairs = &call.staged.pairs;
            let refs: Vec<&Path> = pairs
                .iter()
                .map(|(_, absolute)| absolute.as_path())
                .collect();
            {
                let Some(cfg) = call.config else {
                    return Err(execution(tool_id, "scalafix requires a config".to_owned()));
                };
                let classpath = RealBackend::scalafix_classpath(tool_id, tool, scratch)?;
                let jars: Vec<&Path> = classpath.iter().map(PathBuf::as_path).collect();
                let context = commands::ScalafixContext {
                    config: cfg,
                    sourceroot: scratch.root(),
                    classpath: &jars,
                };
                let invocation = commands::scalafix_check(&tool.binary, &refs, &context)
                    .map_err(|err| execution(tool_id, err.to_string()))?;
                let out = backend.run(tool_id, tool, &invocation, scratch)?;
                parsed(
                    tool_id,
                    parsers::parse_scalafix(&out.stdout, &out.stderr, out.code, &strs),
                )
            }
        },
    },
    ToolDef {
        id: "csharpier",
        run: |backend, call| {
            let names: Vec<String> = call
                .staged
                .pairs
                .iter()
                .map(|(_, absolute)| absolute.to_string_lossy().into_owned())
                .collect();
            let strs: Vec<&str> = names.iter().map(String::as_str).collect();
            let tool_id = call.tool_id;
            let tool = call.tool;
            let scratch = &call.staged.scratch;
            let pairs = &call.staged.pairs;
            let refs: Vec<&Path> = pairs
                .iter()
                .map(|(_, absolute)| absolute.as_path())
                .collect();
            {
                let invocation = commands::csharpier_check(&tool.binary, &refs, call.config);
                let out = backend.run(tool_id, tool, &invocation, scratch)?;
                parsed(
                    tool_id,
                    parsers::parse_csharpier(&out.stderr, out.code, &strs),
                )
            }
        },
    },
    ToolDef {
        id: "fantomas",
        run: |backend, call| {
            let tool_id = call.tool_id;
            let tool = call.tool;
            let scratch = &call.staged.scratch;
            let pairs = &call.staged.pairs;
            let refs: Vec<&Path> = pairs
                .iter()
                .map(|(_, absolute)| absolute.as_path())
                .collect();
            {
                let invocation = commands::fantomas_check(&tool.binary, &refs);
                let out = backend.run(tool_id, tool, &invocation, scratch)?;
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
        },
    },
    ToolDef {
        id: "fsharplint",
        run: |backend, call| {
            let names: Vec<String> = call
                .staged
                .pairs
                .iter()
                .map(|(_, absolute)| absolute.to_string_lossy().into_owned())
                .collect();
            let strs: Vec<&str> = names.iter().map(String::as_str).collect();
            let tool_id = call.tool_id;
            let tool = call.tool;
            let scratch = &call.staged.scratch;
            let pairs = &call.staged.pairs;
            let refs: Vec<&Path> = pairs
                .iter()
                .map(|(_, absolute)| absolute.as_path())
                .collect();
            {
                if !tool.upstream_diagnostics.is_empty() {
                    backend.check_fsharplint_delegated(tool_id, tool, pairs)
                } else {
                    let inputs = RealBackend::fsharplint_context(tool_id, tool, scratch, pairs)?;
                    let compile: Vec<&Path> = inputs.compile.iter().map(PathBuf::as_path).collect();
                    let references: Vec<&Path> = inputs.refs.iter().map(PathBuf::as_path).collect();
                    let context = commands::FsharplintContext {
                        compile: &compile,
                        refs: &references,
                        defines: &inputs.defines,
                        langversion: &inputs.langversion,
                    };
                    let invocation =
                        commands::fsharplint_check(&tool.binary, &refs, &context, call.config);
                    let out = backend.run(tool_id, tool, &invocation, scratch)?;
                    parsed(
                        tool_id,
                        parsers::parse_fsharplint(&out.stdout, out.code, &strs),
                    )
                }
            }
        },
    },
    ToolDef {
        id: "google_java_format",
        run: |backend, call| {
            let names: Vec<String> = call
                .staged
                .pairs
                .iter()
                .map(|(_, absolute)| absolute.to_string_lossy().into_owned())
                .collect();
            let strs: Vec<&str> = names.iter().map(String::as_str).collect();
            let tool_id = call.tool_id;
            let tool = call.tool;
            let scratch = &call.staged.scratch;
            let pairs = &call.staged.pairs;
            let refs: Vec<&Path> = pairs
                .iter()
                .map(|(_, absolute)| absolute.as_path())
                .collect();
            {
                let invocation = commands::google_java_format_check(&tool.binary, &refs);
                let out = backend.run(tool_id, tool, &invocation, scratch)?;
                parsed(
                    tool_id,
                    parsers::parse_google_java_format(&out.stdout, out.code, &strs),
                )
            }
        },
    },
    ToolDef {
        id: "ktfmt",
        run: |backend, call| {
            let names: Vec<String> = call
                .staged
                .pairs
                .iter()
                .map(|(_, absolute)| absolute.to_string_lossy().into_owned())
                .collect();
            let strs: Vec<&str> = names.iter().map(String::as_str).collect();
            let tool_id = call.tool_id;
            let tool = call.tool;
            let scratch = &call.staged.scratch;
            let pairs = &call.staged.pairs;
            let refs: Vec<&Path> = pairs
                .iter()
                .map(|(_, absolute)| absolute.as_path())
                .collect();
            {
                let invocation = commands::ktfmt_check(&tool.binary, &refs);
                let out = backend.run(tool_id, tool, &invocation, scratch)?;
                parsed(tool_id, parsers::parse_ktfmt(&out.stdout, out.code, &strs))
            }
        },
    },
    ToolDef {
        id: "checkstyle",
        run: |backend, call| {
            let names: Vec<String> = call
                .staged
                .pairs
                .iter()
                .map(|(_, absolute)| absolute.to_string_lossy().into_owned())
                .collect();
            let strs: Vec<&str> = names.iter().map(String::as_str).collect();
            let tool_id = call.tool_id;
            let tool = call.tool;
            let scratch = &call.staged.scratch;
            let pairs = &call.staged.pairs;
            let refs: Vec<&Path> = pairs
                .iter()
                .map(|(_, absolute)| absolute.as_path())
                .collect();
            {
                let (invocation, tab_width) = match call.config {
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
                let out = backend.run(tool_id, tool, &invocation, scratch)?;
                parsed(
                    tool_id,
                    parsers::parse_checkstyle(&out.stdout, out.code, &strs, tab_width),
                )
            }
        },
    },
    ToolDef {
        id: "pmd",
        run: |backend, call| {
            let names: Vec<String> = call
                .staged
                .pairs
                .iter()
                .map(|(_, absolute)| absolute.to_string_lossy().into_owned())
                .collect();
            let strs: Vec<&str> = names.iter().map(String::as_str).collect();
            let tool_id = call.tool_id;
            let tool = call.tool;
            let scratch = &call.staged.scratch;
            let pairs = &call.staged.pairs;
            let refs: Vec<&Path> = pairs
                .iter()
                .map(|(_, absolute)| absolute.as_path())
                .collect();
            {
                let Some(ruleset) = call.config else {
                    return Err(execution(tool_id, "pmd needs a ruleset".to_owned()));
                };
                let invocation = commands::pmd_check(&tool.binary, &refs, ruleset);
                let out = backend.run(tool_id, tool, &invocation, scratch)?;
                parsed(tool_id, parsers::parse_pmd(&out.stdout, out.code, &strs))
            }
        },
    },
    ToolDef {
        id: "spotbugs",
        run: |backend, call| {
            let names: Vec<String> = call
                .staged
                .pairs
                .iter()
                .map(|(_, absolute)| absolute.to_string_lossy().into_owned())
                .collect();
            let strs: Vec<&str> = names.iter().map(String::as_str).collect();
            let tool_id = call.tool_id;
            let tool = call.tool;
            let scratch = &call.staged.scratch;
            {
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
                let out = backend.run(tool_id, tool, &invocation, scratch)?;
                parsed(
                    tool_id,
                    parsers::parse_spotbugs(&out.stdout, out.code, &strs),
                )
            }
        },
    },
    ToolDef {
        id: "detekt",
        run: |backend, call| {
            let names: Vec<String> = call
                .staged
                .pairs
                .iter()
                .map(|(_, absolute)| absolute.to_string_lossy().into_owned())
                .collect();
            let strs: Vec<&str> = names.iter().map(String::as_str).collect();
            let tool_id = call.tool_id;
            let tool = call.tool;
            let scratch = &call.staged.scratch;
            let pairs = &call.staged.pairs;
            let refs: Vec<&Path> = pairs
                .iter()
                .map(|(_, absolute)| absolute.as_path())
                .collect();
            {
                let Some(cfg) = call.config else {
                    return Err(execution(tool_id, "detekt needs a config path".to_owned()));
                };
                let work = scratch.root().join(".dx-detekt");
                std::fs::create_dir_all(&work)
                    .map_err(|err| execution(tool_id, format!("stage detekt work dir: {err}")))?;
                let report = work.join("report.xml");
                let invocation = commands::detekt_check(&tool.binary, &refs, cfg, &report);
                let out = backend.run(tool_id, tool, &invocation, scratch)?;
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
        },
    },
];
