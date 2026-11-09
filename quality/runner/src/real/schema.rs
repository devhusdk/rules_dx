use quality_adapter::parsers;

use super::check::ToolDef;
use super::*;

/// Schema and IDL compilers.
pub(super) const TOOLS: &[ToolDef] = &[
    ToolDef {
        id: "buf",
        run: |backend, call| {
            let pairs = &call.staged.pairs;
            let resolve_pairs = &call.staged.resolve_pairs;
            let capability = call.capability;
            let cwd_rel = RealBackend::cwd_rel(call.tool_id, call.tool.config_rel.as_deref());

            let root = call.staged.scratch.root().join(&cwd_rel);
            if capability == "format" {
                let mut findings = Vec::new();
                for (workspace, absolute) in pairs {
                    let invocation = commands::buf_format_check(
                        &call.tool.binary,
                        &[absolute.as_path()],
                        &root,
                        call.config,
                    );
                    let out =
                        backend.run(call.tool_id, call.tool, &invocation, &call.staged.scratch)?;
                    let input = std::fs::read(absolute).map_err(|err| {
                        execution(call.tool_id, format!("read {workspace}: {err}"))
                    })?;
                    let found = parsed(
                        call.tool_id,
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
                let typecheck = capability == "typecheck";
                let mut linted: Vec<(PathBuf, &Path)> = Vec::with_capacity(pairs.len());
                for (workspace, absolute) in pairs {
                    let import_copy =
                        import_path_copy(call.tool_id, workspace, absolute, resolve_pairs)?;
                    linted.push((import_copy.unwrap_or_else(|| absolute.clone()), absolute));
                }
                let targets: Vec<&Path> = linted.iter().map(|(copy, _)| copy.as_path()).collect();
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
                        context.push((relative.to_string_lossy().into_owned(), import.as_str()));
                    }
                }
                let context: Vec<(&str, &str)> = context
                    .iter()
                    .map(|(copy, import)| (copy.as_str(), *import))
                    .collect();
                let image_dir = call.staged.scratch.root().join(".dx-buf-out");
                std::fs::create_dir_all(&image_dir)
                    .map_err(|err| execution(call.tool_id, format!("create output dir: {err}")))?;
                let invocation = if typecheck {
                    commands::buf_build_check(
                        &call.tool.binary,
                        &targets,
                        &root,
                        call.config,
                        &image_dir.join("image.binpb"),
                    )
                } else {
                    commands::buf_lint_check(&call.tool.binary, &targets, &root, call.config)
                };
                let out =
                    backend.run(call.tool_id, call.tool, &invocation, &call.staged.scratch)?;
                let mut found = parsed(
                    call.tool_id,
                    parsers::parse_buf_lint(
                        if typecheck { &out.stderr } else { &out.stdout },
                        out.code,
                        &names,
                        &context,
                    ),
                )?;
                let base = call
                    .tool
                    .extra_env
                    .iter()
                    .find(|(name, _)| name == "DX_BUF_BREAKING_AGAINST")
                    .map(|(_, image)| image.as_str());
                if let Some(image) = base.filter(|_| !typecheck) {
                    let invocation = commands::buf_breaking_check(
                        &call.tool.binary,
                        Path::new(image),
                        &root,
                        call.config,
                    );
                    let out =
                        backend.run(call.tool_id, call.tool, &invocation, &call.staged.scratch)?;
                    found.extend(parsed(
                        call.tool_id,
                        parsers::parse_buf_lint(&out.stdout, out.code, &names, &context),
                    )?);
                }
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
        },
    },
    ToolDef {
        id: "capnp",
        run: |backend, call| {
            let pairs = &call.staged.pairs;
            let capability = call.capability;

            let root = std::fs::canonicalize(call.staged.scratch.root()).map_err(|err| {
                execution(call.tool_id, format!("canonicalize scratch root: {err}"))
            })?;
            let mut findings = Vec::new();
            for (workspace, absolute) in pairs {
                // The run starts at the scratch root and names the file relative to it,
                // so the compiler's import root is the scratch root and nothing above it.
                let invocation =
                    commands::capnp_check(&call.tool.binary, Path::new(workspace), &root);
                let out =
                    backend.run(call.tool_id, call.tool, &invocation, &call.staged.scratch)?;
                // Exit 1 is a schema error; the crash banner for an escaping `..` import
                // also exits 1 and is reported by the parser. Anything else is not a verdict.
                if !matches!(out.code, Some(0 | 1)) {
                    let code = out
                        .code
                        .map_or_else(|| "signal".to_owned(), |code| code.to_string());
                    let stderr = String::from_utf8_lossy(&out.stderr);
                    return Err(execution(
                        call.tool_id,
                        format!("exit {code}: {}", stderr.trim()),
                    ));
                }
                let found = parsed(
                    call.tool_id,
                    parsers::parse_capnp(&out.stderr, out.code, &[workspace.as_str()]),
                )?;
                let scratch_prefix = format!("{}{}", root.display(), std::path::MAIN_SEPARATOR);
                for mut item in found {
                    // Typecheck is the error-only view of the same capnp run.
                    if capability == "typecheck"
                        && !matches!(item.finding.severity, quality_adapter::ToolSeverity::Error)
                    {
                        continue;
                    }
                    item.file = absolute.to_string_lossy().into_owned();
                    item.finding.message = item.finding.message.replace(&scratch_prefix, "");
                    findings.push(item);
                }
            }
            Ok(findings)
        },
    },
    ToolDef {
        id: "flatc",
        run: |backend, call| {
            let pairs = &call.staged.pairs;
            let capability = call.capability;

            let root = std::fs::canonicalize(call.staged.scratch.root()).map_err(|err| {
                execution(call.tool_id, format!("canonicalize scratch root: {err}"))
            })?;
            let out_dir = root.join(".dx-flatc-out");
            let mut findings = Vec::new();
            for (workspace, absolute) in pairs {
                // flatc prints realpaths, so check and report the canonical one.
                let file = std::fs::canonicalize(absolute).map_err(|err| {
                    execution(call.tool_id, format!("resolve {workspace}: {err}"))
                })?;
                let invocation = commands::flatc_check(&call.tool.binary, &file, &root, &out_dir);
                let out =
                    backend.run(call.tool_id, call.tool, &invocation, &call.staged.scratch)?;
                if !matches!(out.code, Some(0 | 1)) {
                    let code = out
                        .code
                        .map_or_else(|| "signal".to_owned(), |code| code.to_string());
                    let stderr = String::from_utf8_lossy(&out.stderr);
                    return Err(execution(
                        call.tool_id,
                        format!("exit {code}: {}", stderr.trim()),
                    ));
                }
                let name = file.to_string_lossy().into_owned();
                let report = parsed(
                    call.tool_id,
                    parsers::parse_flatc(&out.stdout, &out.stderr, out.code, &[name.as_str()]),
                )?;
                declared_input_escape(call.tool_id, workspace, &root, &report.dependencies)?;
                let scratch_prefix = format!("{}{}", root.display(), std::path::MAIN_SEPARATOR);
                for mut item in report.findings {
                    // Typecheck is the error-only view of the same flatc run.
                    if capability == "typecheck"
                        && !matches!(item.finding.severity, quality_adapter::ToolSeverity::Error)
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
        },
    },
    ToolDef {
        id: "thrift",
        run: |backend, call| {
            let pairs = &call.staged.pairs;
            let capability = call.capability;

            let root = std::fs::canonicalize(call.staged.scratch.root()).map_err(|err| {
                execution(call.tool_id, format!("canonicalize scratch root: {err}"))
            })?;
            let out_dir = root.join(".dx-thrift-out");
            std::fs::create_dir_all(&out_dir)
                .map_err(|err| execution(call.tool_id, format!("create output dir: {err}")))?;
            let mut findings = Vec::new();
            for (workspace, absolute) in pairs {
                // thrift prints realpaths of the staged copy, so check and report that one.
                let file = std::fs::canonicalize(root.join(workspace)).map_err(|err| {
                    execution(call.tool_id, format!("resolve {workspace}: {err}"))
                })?;
                let invocation = commands::thrift_check(&call.tool.binary, &file, &root, &out_dir);
                let out =
                    backend.run(call.tool_id, call.tool, &invocation, &call.staged.scratch)?;
                // Exit 1 is a schema error; a usage error also exits 1 and the parser
                // rejects it. Anything else is not a verdict.
                if !matches!(out.code, Some(0 | 1)) {
                    let code = out
                        .code
                        .map_or_else(|| "signal".to_owned(), |code| code.to_string());
                    let stderr = String::from_utf8_lossy(&out.stderr);
                    return Err(execution(
                        call.tool_id,
                        format!("exit {code}: {}", stderr.trim()),
                    ));
                }
                let name = file.to_string_lossy().into_owned();
                let report = parsed(
                    call.tool_id,
                    parsers::parse_thrift(&out.stdout, &out.stderr, out.code, &[name.as_str()]),
                )?;
                declared_input_escape(call.tool_id, workspace, &root, &report.dependencies)?;
                let scratch_prefix = format!("{}{}", root.display(), std::path::MAIN_SEPARATOR);
                for mut item in report.findings {
                    // Typecheck is the error-only view of the same thrift run.
                    if capability == "typecheck"
                        && !matches!(item.finding.severity, quality_adapter::ToolSeverity::Error)
                    {
                        continue;
                    }
                    item.file = absolute.to_string_lossy().into_owned();
                    item.finding.message = item.finding.message.replace(&scratch_prefix, "");
                    findings.push(item);
                }
            }
            Ok(findings)
        },
    },
    ToolDef {
        id: "avro_tools",
        run: |backend, call| {
            let pairs = &call.staged.pairs;
            let resolve_pairs = &call.staged.resolve_pairs;
            let sibling_pairs = &call.staged.sibling_pairs;

            let root = std::fs::canonicalize(call.staged.scratch.root()).map_err(|err| {
                execution(call.tool_id, format!("canonicalize scratch root: {err}"))
            })?;
            let out_dir = root.join(".dx-avro-out");
            std::fs::create_dir_all(&out_dir)
                .map_err(|err| execution(call.tool_id, format!("create output dir: {err}")))?;
            let mut staged: Vec<PathBuf> = pairs
                .iter()
                .map(|(workspace, _)| root.join(workspace))
                .collect();
            staged.extend(sibling_pairs.iter().map(|(_, copy)| copy.clone()));
            staged.extend(resolve_pairs.iter().map(|(_, copy)| copy.clone()));
            let mut findings = Vec::new();
            for (workspace, absolute) in pairs {
                let file = std::fs::canonicalize(root.join(workspace)).map_err(|err| {
                    execution(call.tool_id, format!("resolve {workspace}: {err}"))
                })?;
                let deps: Vec<&Path> = staged
                    .iter()
                    .filter(|dep| dep.extension().is_some_and(|ext| ext == "avsc"))
                    .filter(|dep| **dep != file)
                    .map(PathBuf::as_path)
                    .collect();
                if file.extension().is_some_and(|ext| ext == "avdl") {
                    let source = std::fs::read_to_string(&file).map_err(|err| {
                        execution(call.tool_id, format!("read {workspace}: {err}"))
                    })?;
                    let outside = avdl_imports_outside(&source, &file, &root);
                    declared_input_escape(call.tool_id, workspace, &root, &outside)?;
                }
                let invocation = commands::avro_check(&call.tool.binary, &file, &deps, &out_dir);
                let out =
                    backend.run(call.tool_id, call.tool, &invocation, &call.staged.scratch)?;
                let name = file.to_string_lossy().into_owned();
                let found = parsed(
                    call.tool_id,
                    parsers::parse_avro(&out.stdout, &out.stderr, out.code, &[name.as_str()]),
                )?;
                let scratch_prefix = format!("{}{}", root.display(), std::path::MAIN_SEPARATOR);
                for mut item in found {
                    item.file = absolute.to_string_lossy().into_owned();
                    item.finding.message = item.finding.message.replace(&scratch_prefix, "");
                    findings.push(item);
                }
            }
            Ok(findings)
        },
    },
];

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

/// The imports of an `.avdl` source that resolve outside the scratch `root`.
fn avdl_imports_outside(source: &str, file: &Path, root: &Path) -> Vec<String> {
    let dir = file.parent().unwrap_or(root);
    parsers::avdl_imports(source)
        .into_iter()
        .map(|import| lexical_normal(&dir.join(import)))
        .filter(|path| !path.starts_with(root))
        .map(|path| path.to_string_lossy().into_owned())
        .collect()
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

#[cfg(test)]
mod declared_input_tests {
    use super::{avdl_imports_outside, declared_input_escape};

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
        let up = vec![
            inner
                .join("pkg/../../outside.fbs")
                .to_string_lossy()
                .into_owned(),
        ];
        let err = declared_input_escape("flatc", "pkg/a.fbs", &inner, &up).expect_err("escape");
        assert!(
            err.to_string()
                .contains("is outside the declared inputs of pkg/a.fbs")
        );
        let missing = vec![inner.join("pkg/none.fbs").to_string_lossy().into_owned()];
        assert!(declared_input_escape("flatc", "pkg/a.fbs", &inner, &missing).is_err());
    }

    #[test]
    fn avdl_imports_outside_the_root_fail() {
        let dir = tempfile::tempdir().expect("scratch");
        let root = std::fs::canonicalize(dir.path()).expect("root");
        std::fs::create_dir_all(root.join("pkg")).expect("pkg");
        let file = root.join("pkg/a.avdl");
        let source = "protocol P { import schema \"b.avsc\"; import schema \"../../outside.avsc\"; import idl \"/etc/hosts\"; }";
        let outside = avdl_imports_outside(source, &file, &root);
        assert_eq!(outside.len(), 2, "{outside:?}");
        let err =
            declared_input_escape("avro_tools", "pkg/a.avdl", &root, &outside).expect_err("escape");
        assert!(
            err.to_string()
                .contains("is outside the declared inputs of pkg/a.avdl")
        );
        assert!(avdl_imports_outside("import schema \"b.avsc\";", &file, &root).is_empty());
    }

    #[test]
    fn thrift_absolute_and_parent_includes_outside_the_root_fail() {
        let dir = tempfile::tempdir().expect("scratch");
        let root = std::fs::canonicalize(dir.path()).expect("root");
        std::fs::create_dir_all(root.join("root/pkg")).expect("pkg");
        std::fs::write(root.join("root/pkg/a.thrift"), "struct A {}").expect("a");
        std::fs::write(root.join("outside.thrift"), "struct O {}").expect("outside");
        let inner = root.join("root");
        let own = vec![inner.join("pkg/a.thrift").to_string_lossy().into_owned()];
        declared_input_escape("thrift", "pkg/a.thrift", &inner, &own).expect("inside");
        let absolute = vec![root.join("outside.thrift").to_string_lossy().into_owned()];
        let err = declared_input_escape("thrift", "pkg/a.thrift", &inner, &absolute)
            .expect_err("absolute include escapes");
        assert!(
            err.to_string()
                .contains("is outside the declared inputs of pkg/a.thrift")
        );
        let parent = vec![
            inner
                .join("pkg/../../outside.thrift")
                .to_string_lossy()
                .into_owned(),
        ];
        assert!(declared_input_escape("thrift", "pkg/a.thrift", &inner, &parent).is_err());
    }
}
