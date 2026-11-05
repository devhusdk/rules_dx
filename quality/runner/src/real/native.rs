use quality_adapter::parsers::{self, FileFinding};

use super::check::{in_file, CheckCall, ToolDef};
use super::*;

fn paths<'a>(call: &'a CheckCall<'a>) -> (Vec<&'a Path>, Vec<String>) {
    let refs = call
        .staged
        .pairs
        .iter()
        .map(|(_, absolute)| absolute.as_path())
        .collect();
    let names = call
        .staged
        .pairs
        .iter()
        .map(|(_, absolute)| absolute.to_string_lossy().into_owned())
        .collect();
    (refs, names)
}

fn run(
    backend: &RealBackend,
    call: &CheckCall,
    invocation: &Invocation,
) -> Result<quality_adapter::exec::ChildOutput, RunnerError> {
    backend.run(call.tool_id, call.tool, invocation, &call.staged.scratch)
}

fn need_config<'a>(call: &CheckCall<'a>) -> Result<&'a Path, RunnerError> {
    call.config.ok_or_else(|| {
        execution(
            call.tool_id,
            format!("{} needs a config path", call.tool_id),
        )
    })
}

/// Runs one process per file, parsing each file's output on its own.
fn per_file(
    backend: &RealBackend,
    call: &CheckCall,
    build: impl Fn(&Path) -> Invocation,
    parse: impl Fn(&quality_adapter::exec::ChildOutput, &str) -> Result<Vec<FileFinding>, RunnerError>,
) -> Result<Vec<FileFinding>, RunnerError> {
    let (refs, names) = paths(call);
    let mut merged = Vec::new();
    for (name, path) in names.iter().zip(refs.iter()) {
        let out = run(backend, call, &build(path))?;
        merged.extend(parse(&out, name).map_err(|err| in_file(err, name))?);
    }
    Ok(merged)
}

fn buildifier(backend: &RealBackend, call: &CheckCall) -> Result<Vec<FileFinding>, RunnerError> {
    let tool = call.tool;
    let cwd_rel = RealBackend::cwd_rel(call.tool_id, tool.config_rel.as_deref());
    let hint = hint_dir(tool.config_rel.as_deref(), &cwd_rel);
    let reported = backend.check_files(
        call.tool_id,
        tool,
        &call.staged.scratch,
        &call.staged.pairs,
        |files| Ok(commands::buildifier_check(&tool.binary, files, hint)),
        |out, names| {
            parsed(
                call.tool_id,
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
        } else if (call.capability == "format") == found.finding.rule_id.is_empty() {
            kept.push(found);
        }
    }
    Ok(kept)
}

fn qmlformat(backend: &RealBackend, call: &CheckCall) -> Result<Vec<FileFinding>, RunnerError> {
    let mut findings = Vec::new();
    for (workspace, absolute) in &call.staged.pairs {
        let invocation =
            commands::qmlformat_check(&call.tool.binary, &[absolute.as_path()], call.config);
        let out = run(backend, call, &invocation)?;
        let input = std::fs::read(absolute)
            .map_err(|err| execution(call.tool_id, format!("read {workspace}: {err}")))?;
        let found = parsed(
            call.tool_id,
            parsers::parse_qmlformat(&out.stdout, out.code, &input, workspace),
        )?;
        for mut item in found {
            item.file = absolute.to_string_lossy().into_owned();
            findings.push(item);
        }
    }
    Ok(findings)
}

fn qmllint(backend: &RealBackend, call: &CheckCall) -> Result<Vec<FileFinding>, RunnerError> {
    let view = if call.capability == "typecheck" {
        parsers::QmllintView::Typecheck
    } else {
        parsers::QmllintView::Lint
    };
    let pairs = &call.staged.pairs;
    if !call.tool.upstream_diagnostics.is_empty() {
        return backend.check_qmllint_delegated(call.tool_id, call.tool, pairs, view);
    }
    let (refs, names) = paths(call);
    let strs: Vec<&str> = names.iter().map(String::as_str).collect();
    let roots = super::staging::qml_import_roots(call.tool_id, &call.staged.resolve_pairs)?;
    let root_refs: Vec<&Path> = roots.iter().map(PathBuf::as_path).collect();
    if let Some(settings) = call.config {
        super::staging::place_qmllint_settings(call.tool_id, settings, pairs)?;
    }
    let invocation =
        commands::qmllint_check(&call.tool.binary, &refs, &root_refs, call.config.is_some());
    let out = run(backend, call, &invocation)?;
    parsed(
        call.tool_id,
        parsers::parse_qmllint(&out.stdout, out.code, &strs, view),
    )
}

fn dart(backend: &RealBackend, call: &CheckCall) -> Result<Vec<FileFinding>, RunnerError> {
    let tool_id = call.tool_id;
    let (refs, names) = paths(call);
    let strs: Vec<&str> = names.iter().map(String::as_str).collect();
    if call.capability == "format" {
        let out = run(
            backend,
            call,
            &commands::dart_format_check(&call.tool.binary, &refs),
        )?;
        return parsed(
            tool_id,
            parsers::parse_dart_format(&out.stdout, &out.stderr, out.code, &strs),
        );
    }
    // The analyzer prints canonical paths, so check and report the canonical ones.
    let root = std::fs::canonicalize(call.staged.scratch.root())
        .map_err(|err| execution(tool_id, format!("canonicalize scratch root: {err}")))?;
    let mut files: Vec<(String, PathBuf)> = Vec::with_capacity(call.staged.pairs.len());
    for (workspace, absolute) in &call.staged.pairs {
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
    let invocation = commands::dart_analyze(&call.tool.binary, &cache, &options, &file_refs);
    let out = run(backend, call, &invocation)?;
    let view = if call.capability == "typecheck" {
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

/// Tools for C, C++, Go, Qt, Fortran, D, Typst, Swift, Dart, Julia, Zig, LaTeX, CMake and Make.
pub(super) const TOOLS: &[ToolDef] = &[
    ToolDef {
        id: "buildifier",
        run: buildifier,
    },
    ToolDef {
        id: "clang_format",
        run: |backend, call| {
            let (refs, names) = paths(call);
            let strs: Vec<&str> = names.iter().map(String::as_str).collect();
            let invocation = commands::clang_format_check(&call.tool.binary, &refs, call.config);
            let out = run(backend, call, &invocation)?;
            parsed(
                call.tool_id,
                parsers::parse_clang_format(&out.stderr, out.code, &strs),
            )
        },
    },
    ToolDef {
        id: "gofumpt",
        run: |backend, call| {
            let (refs, names) = paths(call);
            let strs: Vec<&str> = names.iter().map(String::as_str).collect();
            let lang = RealBackend::gofumpt_lang(call.tool)?;
            let out = run(
                backend,
                call,
                &commands::gofumpt_check(&call.tool.binary, &refs, lang),
            )?;
            parsed(
                call.tool_id,
                parsers::parse_gofumpt(&out.stdout, out.code, &strs),
            )
        },
    },
    ToolDef {
        id: "clang_tidy",
        run: |backend, call| {
            if call.tool.upstream_diagnostics.is_empty() {
                return Err(execution(
                    call.tool_id,
                    "clang_tidy requires upstream diagnostics from a declared delegated action: direct execution without the authoritative compilation context is rejected".to_owned(),
                ));
            }
            backend.check_clang_tidy_delegated(call.tool_id, call.tool, &call.staged.pairs)
        },
    },
    ToolDef {
        id: "cppcheck",
        run: |backend, call| {
            if !call.tool.upstream_diagnostics.is_empty() {
                return backend.check_cppcheck_delegated(
                    call.tool_id,
                    call.tool,
                    &call.staged.pairs,
                );
            }
            let (refs, names) = paths(call);
            let strs: Vec<&str> = names.iter().map(String::as_str).collect();
            let unused = call.capability == "unused";
            let invocation = commands::cppcheck_check(
                &call.tool.binary,
                &refs,
                call.config,
                unused,
                &call.tool.args,
            );
            let out = run(backend, call, &invocation)?;
            let found = parsed(
                call.tool_id,
                parsers::parse_cppcheck(&out.stderr, out.code, &strs),
            )?;
            Ok(if unused {
                parsers::keep_unused_function(found)
            } else {
                found
            })
        },
    },
    ToolDef {
        id: "staticcheck",
        run: |backend, call| {
            if !call.tool.upstream_diagnostics.is_empty() {
                return backend.check_staticcheck_delegated(
                    call.tool_id,
                    call.tool,
                    &call.staged.pairs,
                );
            }
            let (refs, names) = paths(call);
            let strs: Vec<&str> = names.iter().map(String::as_str).collect();
            let cwd_rel = RealBackend::cwd_rel(call.tool_id, call.tool.config_rel.as_deref());
            let invocation = commands::staticcheck_check(
                &call.tool.binary,
                &refs,
                hint_dir(call.tool.config_rel.as_deref(), &cwd_rel),
            );
            let out = run(backend, call, &invocation)?;
            parsed(
                call.tool_id,
                parsers::parse_staticcheck(&out.stdout, out.code, &strs),
            )
        },
    },
    ToolDef {
        id: "qmlformat",
        run: qmlformat,
    },
    ToolDef {
        id: "qmllint",
        run: qmllint,
    },
    ToolDef {
        id: "ffmt",
        run: |backend, call| {
            // Fixed-form `.f` and `.F` and the `.f08` family never reach ffmt; an empty
            // set skips the run because ffmt exits 2 when it finds no Fortran file.
            let (refs, _) = paths(call);
            let accepted: Vec<&Path> = refs
                .iter()
                .copied()
                .filter(|path| ffmt_accepts(path))
                .collect();
            if accepted.is_empty() {
                return Ok(Vec::new());
            }
            let dir = RealBackend::ffmt_config_dir(call.tool)?;
            let invocation = commands::ffmt_check(&call.tool.binary, &accepted, &dir);
            let out = run(backend, call, &invocation)?;
            let accepted_strs: Vec<&str> = accepted.iter().filter_map(|p| p.to_str()).collect();
            parsed(
                call.tool_id,
                parsers::parse_ffmt(&out.stdout, out.code, &accepted_strs),
            )
        },
    },
    ToolDef {
        id: "fortitude",
        run: |backend, call| {
            // fortitude parses free-form source only; it reports a valid fixed-form `.f`
            // or `.F` file as a syntax error, so those are not handed to it.
            let (refs, _) = paths(call);
            let free_form: Vec<&Path> = refs
                .iter()
                .copied()
                .filter(|path| fortran_free_form(path))
                .collect();
            if free_form.is_empty() {
                return Ok(Vec::new());
            }
            let config = backend.config_abs(call.tool_id, call.tool, &call.staged.scratch)?;
            let invocation =
                commands::fortitude_check(&call.tool.binary, &free_form, config.as_deref());
            let out = run(backend, call, &invocation)?;
            let free_strs: Vec<&str> = free_form.iter().filter_map(|p| p.to_str()).collect();
            parsed(
                call.tool_id,
                parsers::parse_fortitude(&out.stdout, out.code, &free_strs),
            )
        },
    },
    ToolDef {
        id: "dfmt",
        run: |backend, call| {
            // One process per file: with two or more files dfmt edits them in place.
            let mut findings = Vec::new();
            for (workspace, absolute) in &call.staged.pairs {
                let invocation = commands::dfmt_check(&call.tool.binary, absolute.as_path());
                let out = run(backend, call, &invocation)?;
                let input = std::fs::read(absolute)
                    .map_err(|err| execution(call.tool_id, format!("read {workspace}: {err}")))?;
                let name = absolute.to_string_lossy();
                findings.extend(parsed(
                    call.tool_id,
                    parsers::parse_dfmt(&out.stdout, &out.stderr, out.code, &input, &name),
                )?);
            }
            Ok(findings)
        },
    },
    ToolDef {
        id: "tinymist",
        run: |backend, call| {
            // One process per file: each run compiles one entry file.
            let (refs, names) = paths(call);
            let strs: Vec<&str> = names.iter().map(String::as_str).collect();
            let mut findings = Vec::new();
            for absolute in &refs {
                let invocation = commands::tinymist_lint(
                    &call.tool.binary,
                    call.staged.scratch.root(),
                    absolute,
                );
                let out = run(backend, call, &invocation)?;
                findings.extend(parsed(
                    call.tool_id,
                    parsers::parse_tinymist(&out.stdout, &out.stderr, out.code, &strs),
                )?);
            }
            Ok(findings)
        },
    },
    ToolDef {
        id: "swift_format",
        run: |backend, call| {
            // One process per file: the check prints the formatted text of one file.
            let config = backend.config_abs(call.tool_id, call.tool, &call.staged.scratch)?;
            let mut findings = Vec::new();
            for (workspace, absolute) in &call.staged.pairs {
                let invocation = commands::swift_format_check(
                    &call.tool.binary,
                    absolute.as_path(),
                    config.as_deref(),
                );
                let out = run(backend, call, &invocation)?;
                let input = std::fs::read(absolute)
                    .map_err(|err| execution(call.tool_id, format!("read {workspace}: {err}")))?;
                let name = absolute.to_string_lossy();
                findings.extend(parsed(
                    call.tool_id,
                    parsers::parse_swift_format(&out.stdout, &out.stderr, out.code, &input, &name),
                )?);
            }
            Ok(findings)
        },
    },
    ToolDef {
        id: "swift_lint",
        run: |backend, call| {
            let (refs, names) = paths(call);
            let strs: Vec<&str> = names.iter().map(String::as_str).collect();
            let config = backend.config_abs(call.tool_id, call.tool, &call.staged.scratch)?;
            let out = run(
                backend,
                call,
                &commands::swift_lint(&call.tool.binary, &refs, config.as_deref()),
            )?;
            parsed(
                call.tool_id,
                parsers::parse_swift_lint(&out.stdout, &out.stderr, out.code, &strs),
            )
        },
    },
    ToolDef {
        id: "dart",
        run: dart,
    },
    ToolDef {
        id: "julia_format",
        run: |backend, call| {
            let (refs, names) = paths(call);
            let strs: Vec<&str> = names.iter().map(String::as_str).collect();
            let invocation = commands::julia_format_check(&call.tool.binary, &refs);
            let out = run(backend, call, &invocation)?;
            parsed(
                call.tool_id,
                parsers::parse_julia_format(&out.stdout, &out.stderr, out.code, &strs),
            )
        },
    },
    ToolDef {
        id: "zig_ast_check",
        run: |backend, call| {
            // One process per file: `zig ast-check` rejects a second positional file.
            per_file(
                backend,
                call,
                |path| commands::zig_ast_check(&call.tool.binary, path),
                |out, name| {
                    parsed(
                        call.tool_id,
                        parsers::parse_zig_ast_check(&out.stdout, &out.stderr, out.code, &[name]),
                    )
                },
            )
        },
    },
    ToolDef {
        id: "zig_fmt",
        run: |backend, call| {
            let (refs, names) = paths(call);
            let strs: Vec<&str> = names.iter().map(String::as_str).collect();
            let out = run(
                backend,
                call,
                &commands::zig_fmt_check(&call.tool.binary, &refs),
            )?;
            parsed(
                call.tool_id,
                parsers::parse_zig_fmt(&out.stdout, &out.stderr, out.code, &strs),
            )
        },
    },
    ToolDef {
        id: "chktex",
        run: |backend, call| {
            // One process per file: chktex checks a single document and never follows `\input`.
            per_file(
                backend,
                call,
                |path| commands::chktex_check(&call.tool.binary, path),
                |out, name| {
                    parsed(
                        call.tool_id,
                        parsers::parse_chktex(&out.stdout, &out.stderr, out.code, &[name]),
                    )
                },
            )
        },
    },
    ToolDef {
        id: "latexindent",
        run: |backend, call| {
            // One process per file: latexindent takes a single document. Its log and
            // backups go to a scratch directory, never next to the source.
            let cruft = RealBackend::latexindent_cruft(call.tool_id, &call.staged.scratch)?;
            per_file(
                backend,
                call,
                |path| commands::latexindent_check(&call.tool.binary, path, &cruft, call.config),
                |out, name| {
                    parsed(
                        call.tool_id,
                        parsers::parse_latexindent(&out.stdout, &out.stderr, out.code, &[name]),
                    )
                },
            )
        },
    },
    ToolDef {
        id: "gersemi",
        run: |backend, call| {
            let config = need_config(call)?;
            let (refs, names) = paths(call);
            let strs: Vec<&str> = names.iter().map(String::as_str).collect();
            let invocation = commands::gersemi_check(&call.tool.binary, &refs, config);
            let out = run(backend, call, &invocation)?;
            parsed(
                call.tool_id,
                parsers::parse_gersemi(&out.stdout, &out.stderr, out.code, &strs),
            )
        },
    },
    ToolDef {
        id: "mbake",
        run: |backend, call| {
            let config = need_config(call)?;
            let (refs, names) = paths(call);
            let strs: Vec<&str> = names.iter().map(String::as_str).collect();
            let invocation = commands::mbake_check(&call.tool.binary, &refs, config);
            let out = run(backend, call, &invocation)?;
            parsed(
                call.tool_id,
                parsers::parse_mbake(&out.stdout, &out.stderr, out.code, &strs),
            )
        },
    },
];

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
