use super::real_fixtures::*;
use super::*;
use crate::FileInput;
use quality_result::encode_validated;
use quality_result::proto::Convergence;

pub(super) fn markdown_links(
    argv: &[OsString],
    _cwd: &Path,
    env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    assert_hermetic(env);
    let mut out = String::new();
    let mut index = 0;
    while index < argv.len() {
        if argv[index] == "--source" {
            let mapping = argv[index + 1].to_string_lossy().into_owned();
            let (workspace, absolute) = mapping.split_once('=').expect("--source maps WS=ABS");
            let bytes = std::fs::read(absolute).expect("checked file is materialized");
            let text = String::from_utf8(bytes).expect("checked bytes are UTF-8");
            if text.contains("BROKEN") {
                out.push_str(&format!(
                    "{{\"path\":\"{workspace}\",\"line\":2,\"kind\":\"missing-file-target\",\"message\":\"link target nope.md does not match a checked source\"}}\n"
                ));
            }
            index += 1;
        }
        index += 1;
    }
    Ok(ChildOutput {
        code: Some(0),
        stdout: out.into_bytes(),
        stderr: Vec::new(),
    })
}

pub(super) fn markdown_broken(
    argv: &[OsString],
    _cwd: &Path,
    env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    assert_hermetic(env);
    let _ = last_file(argv);
    Ok(ChildOutput {
        code: Some(2),
        stdout: Vec::new(),
        stderr: b"markdown_check: bad usage".to_vec(),
    })
}

pub(super) fn rustfmt_hinted(
    argv: &[OsString],
    _cwd: &Path,
    env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    assert_hermetic(env);
    let config = argv
        .windows(2)
        .find(|pair| pair[0] == "--config-path")
        .map(|pair| pair[1].clone())
        .expect("rustfmt passes --config-path");
    assert!(
        config.to_string_lossy().ends_with("hint.toml"),
        "hinted config reaches the tool"
    );
    assert!(
        Path::new(&config).is_file(),
        "hinted config is materialized"
    );
    Ok(ChildOutput {
        code: Some(0),
        stdout: Vec::new(),
        stderr: Vec::new(),
    })
}

pub(super) fn rustfmt_defaults(
    argv: &[OsString],
    _cwd: &Path,
    env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    assert_hermetic(env);
    let config = argv
        .windows(2)
        .find(|pair| pair[0] == "--config-path")
        .map(|pair| pair[1].clone())
        .expect("rustfmt passes --config-path");
    assert!(
        config.to_string_lossy().ends_with(RUSTFMT_DEFAULTS_REL),
        "unhinted rustfmt gets the materialized defaults"
    );
    assert!(
        Path::new(&config).is_file(),
        "defaults file is materialized"
    );
    Ok(ChildOutput {
        code: Some(0),
        stdout: Vec::new(),
        stderr: Vec::new(),
    })
}

pub(super) fn failing_fix(
    argv: &[OsString],
    _cwd: &Path,
    env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    assert_hermetic(env);
    let _ = last_file(argv);
    Ok(ChildOutput {
        code: Some(1),
        stdout: Vec::new(),
        stderr: b"syntax error".to_vec(),
    })
}

pub(super) fn deleting_fix(
    argv: &[OsString],
    _cwd: &Path,
    env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    assert_hermetic(env);
    std::fs::remove_file(last_file(argv)).expect("fix removes the file");
    Ok(ChildOutput {
        code: Some(0),
        stdout: Vec::new(),
        stderr: Vec::new(),
    })
}

pub(super) fn binary_fix(
    argv: &[OsString],
    _cwd: &Path,
    env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    assert_hermetic(env);
    std::fs::write(last_file(argv), b"\xff").expect("fix writes bytes");
    Ok(ChildOutput {
        code: Some(0),
        stdout: Vec::new(),
        stderr: Vec::new(),
    })
}

#[test]
pub(super) fn real_tools_pin_the_dispatched_set() {
    assert_eq!(
        REAL_TOOLS,
        &[
            "ansible_lint",
            "biome",
            "buf",
            "buildifier",
            "capnp",
            "cfn_lint",
            "checkmake",
            "checkstyle",
            "cmakelint",
            "clang_format",
            "clang_syntax",
            "clang_tidy",
            "clippy",
            "cppcheck",
            "credo",
            "csc",
            "csharpier",
            "cue",
            "dart",
            "deadnix",
            "dfmt",
            "dialyzer",
            "dscanner",
            "djlint",
            "elvis",
            "erlfmt",
            "errcheck",
            "eslint",
            "fantomas",
            "flake8",
            "flang",
            "flatc",
            "fortitude",
            "fsc",
            "fsharplint",
            "gersemi",
            "gofumpt",
            "gotype",
            "google_java_format",
            "govet",
            "gplint",
            "graphql_eslint",
            "hadolint",
            "helm",
            "hlint",
            "javac",
            "jsonnet_lint",
            "jsonnetfmt",
            "julia_format",
            "keep_sorted",
            "latexindent",
            "ldc",
            "kubeconform",
            "ktfmt",
            "ktlint",
            "luals",
            "markdown_check",
            "mbake",
            "mix_format",
            "modfmt",
            "nixfmt",
            "notebook_outputs",
            "nvcc_syntax",
            "ormolu",
            "opa",
            "perlcritic",
            "perltidy",
            "pkl",
            "phpcs",
            "phpstan",
            "pmd",
            "prettier",
            "psscriptanalyzer",
            "pydoclint",
            "pylint",
            "qmlformat",
            "qmllint",
            "redocly",
            "regal",
            "remark",
            "roslyn",
            "rubocop",
            "ruff",
            "rustc",
            "rustfmt",
            "scalafix",
            "scalafmt",
            "shellcheck",
            "shfmt",
            "spotbugs",
            "sqlfluff",
            "standardrb",
            "staticcheck",
            "statix",
            "stylelint",
            "stylua",
            "taplo",
            "terraform",
            "tflint",
            "tsc",
            "ty",
            "typstyle",
            "vale",
            "yamlfmt",
            "yamllint",
            "zig",
            "zig_ast_check",
            "zig_fmt",
        ]
    );
}

#[test]
pub(super) fn real_spawn_reports_missing_binaries() {
    assert!(real_spawn(
        &[OsString::from("/nonexistent-dx-tool-binary")],
        Path::new("/tmp"),
        &[]
    )
    .is_err());
}

#[test]
pub(super) fn supports_follows_resolution() {
    let backend = backend_for("rustfmt", plain_tool(), rustfmt_defaults);
    assert!(backend.supports("rustfmt"));
    assert!(!backend.supports("lint-a"));
}

#[test]
pub(super) fn rustfmt_pipeline_fixes_dirty_files_to_stable() {
    let backend = backend_for("rustfmt", rustfmt_tool(), roundtrip_rustfmt);
    let stages = vec![stage("rustfmt", &["rust"], &["src/main.rs"])];
    let files = vec![file("src/main.rs", "x  \ny\t")];
    let result = run_real_pipeline("//quality:test", "format", &stages, &files, &backend)
        .expect("real pipeline");
    assert_eq!(result.convergence, Convergence::Stable as i32);
    assert_eq!(result.completed_rounds, 2);
    assert_eq!(result.initial_diagnostics.len(), 1);
    assert_eq!(result.initial_diagnostics[0].tool_id, "rustfmt");
    assert_eq!(
        result.initial_diagnostics[0].message,
        "file is not formatted"
    );
    assert!(result.terminal_diagnostics.is_empty());
    assert!(result.initial_diagnostics.iter().all(|d| d.fixable));
    assert_eq!(result.replacements.len(), 1);
    assert_eq!(result.replacements[0].edits[0].start_byte, 1);
    assert_eq!(result.replacements[0].edits[0].end_byte, 6);
    assert_eq!(result.replacements[0].edits[0].replacement, b"\ny");
    assert!(encode_validated(&result).is_ok());
}

#[test]
pub(super) fn rustfmt_fix_keeps_module_siblings_available() {
    let backend = backend_for("rustfmt", rustfmt_tool(), module_rustfmt);
    let stages = vec![stage("rustfmt", &["rust"], &["src/lib.rs", "src/child.rs"])];
    let files = vec![
        file("src/lib.rs", "mod child;\npub fn f( ){ }  \n"),
        file("src/child.rs", "pub fn child() {}\n"),
    ];
    let result = run_real_pipeline("//quality:test", "format", &stages, &files, &backend)
        .expect("real pipeline");
    assert_eq!(result.convergence, Convergence::Stable as i32);
    assert_eq!(result.initial_diagnostics.len(), 1);
    assert!(result.terminal_diagnostics.is_empty());
    assert_eq!(result.replacements.len(), 1);
    assert_eq!(result.replacements[0].path, "src/lib.rs");
}

#[test]
pub(super) fn rustfmt_crate_root_with_unformatted_modules_is_fixable_for_every_producer() {
    let backend = backend_for("rustfmt", rustfmt_tool(), module_rustfmt);
    let stages = vec![stage(
        "rustfmt",
        &["rust"],
        &["src/lib.rs", "src/candidate.rs"],
    )];
    let files = vec![
        file("src/lib.rs", "pub mod candidate;\npub fn f( ){ }  \n"),
        file("src/candidate.rs", "pub fn candidate() {}  \n"),
    ];
    let forwarder = run_real_pipeline("//deploy/release:x", "format", &stages, &files, &backend)
        .expect("forwarder pipeline");
    assert_eq!(forwarder.convergence, Convergence::Stable as i32);
    assert_eq!(forwarder.initial_diagnostics.len(), 2);
    assert!(forwarder.initial_diagnostics.iter().all(|d| d.fixable));
    assert!(forwarder.terminal_diagnostics.is_empty());
    let paths: BTreeSet<&str> = forwarder
        .replacements
        .iter()
        .map(|r| r.path.as_str())
        .collect();
    assert_eq!(paths, BTreeSet::from(["src/candidate.rs", "src/lib.rs"]));
    let upstream = run_real_pipeline(
        "//deploy/release:x_upstream",
        "format",
        &stages,
        &files,
        &backend,
    )
    .expect("upstream pipeline");
    assert!(upstream.initial_diagnostics.iter().all(|d| d.fixable));
    assert_eq!(upstream.replacements, forwarder.replacements);
}

#[test]
pub(super) fn rustfmt_supporting_modules_resolve_but_are_never_owned() {
    let backend = backend_for("rustfmt", rustfmt_tool(), module_rustfmt);
    let stages = vec![stage("rustfmt", &["rust"], &["src/lib.rs"])];
    let files = vec![file("src/lib.rs", "mod child;  \n")];
    let siblings = vec![
        file("src/child.rs", "mod grand;  \n"),
        file("src/child/grand.rs", "pub fn g() {}  \n"),
    ];
    let result = run_real_pipeline_with_siblings(
        "//quality:test",
        "format",
        &stages,
        &files,
        &siblings,
        &backend,
    )
    .expect("real pipeline");
    assert_eq!(result.convergence, Convergence::Stable as i32);
    assert_eq!(result.initial_diagnostics.len(), 1);
    assert_eq!(result.initial_diagnostics[0].path, "src/lib.rs");
    assert!(result.terminal_diagnostics.is_empty());
    assert_eq!(result.replacements.len(), 1);
    assert_eq!(result.replacements[0].path, "src/lib.rs");
}

#[test]
pub(super) fn rustfmt_missing_module_fails_with_rustfmt_stderr() {
    let backend = backend_for("rustfmt", rustfmt_tool(), module_rustfmt);
    let stages = vec![stage("rustfmt", &["rust"], &["src/lib.rs"])];
    let files = vec![file("src/lib.rs", "mod child;\n")];
    let err = run_real_pipeline("//quality:test", "format", &stages, &files, &backend)
        .expect_err("missing module fails");
    assert!(matches!(err, RunnerError::ToolExecution { .. }), "{err}");
    assert_eq!(
        err.to_string(),
        format!(
            "tool execution failed for rustfmt: rustfmt failed with exit 1: Error writing files: \
             failed to resolve mod `child`: {} does not exist",
            Path::new("src").join("child.rs").display()
        )
    );
}

#[test]
pub(super) fn rustfmt_fix_without_context_reports_the_missing_module() {
    let backend = backend_for("rustfmt", rustfmt_tool(), module_rustfmt);
    let err = backend
        .apply_fix("rustfmt", "src/lib.rs", "mod child;  \n", "format")
        .expect_err("missing module is not a no-op fix");
    let missing = format!(
        "failed to resolve mod `child`: {} does not exist",
        Path::new("src").join("child.rs").display()
    );
    assert!(err.to_string().contains(&missing), "{err}");
}

#[test]
pub(super) fn rustfmt_fix_formats_the_passed_text_of_the_fixed_file() {
    let backend = backend_for("rustfmt", rustfmt_tool(), module_rustfmt);
    let known = BTreeSet::from(["src/lib.rs".to_owned(), "src/child.rs".to_owned()]);
    let fixed = backend
        .apply_fix_in_context("rustfmt", "src/lib.rs", "mod child;  \n", "format", &known)
        .expect("passed text wins");
    assert_eq!(fixed, "mod child;\n");
}

#[test]
pub(super) fn rustfmt_fix_stages_child_modules_as_empty_placeholders() {
    let backend = backend_for("rustfmt", rustfmt_tool(), placeholder_children_rustfmt);
    let stages = vec![stage("rustfmt", &["rust"], &["src/lib.rs", "src/child.rs"])];
    let files = vec![
        file("src/lib.rs", "mod child;  \n"),
        file("src/child.rs", "pub fn c() {}  \n"),
    ];
    let result = run_real_pipeline("//quality:test", "format", &stages, &files, &backend)
        .expect("lib.rs is fixed against a placeholder child");
    assert_eq!(result.convergence, Convergence::Stable as i32);
    let paths: Vec<&str> = result
        .replacements
        .iter()
        .map(|replacement| replacement.path.as_str())
        .collect();
    assert_eq!(paths, ["src/child.rs", "src/lib.rs"]);
}

#[test]
pub(super) fn placeholders_cover_the_other_known_files_with_the_extensions() {
    let known: BTreeSet<String> = [
        "src/lib.rs",
        "src/child.rs",
        "src/nested/mod.rs",
        "README.md",
        "app.py",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    assert_eq!(
        real_fix::placeholders("src/lib.rs", &known, &["rs"]),
        BTreeMap::from([
            ("src/child.rs".to_owned(), String::new()),
            ("src/nested/mod.rs".to_owned(), String::new()),
        ])
    );
}

#[test]
pub(super) fn rustfmt_parent_relative_path_module_is_formatted() {
    let backend = backend_for("rustfmt", rustfmt_tool(), module_rustfmt);
    let stages = vec![stage(
        "rustfmt",
        &["rust"],
        &["src/lib.rs", "shared/util.rs"],
    )];
    let files = vec![
        file("src/lib.rs", "#[path = \"../shared/util.rs\"]\nmod util;\n"),
        file("shared/util.rs", "pub fn u() {}  \n"),
    ];
    let result = run_real_pipeline("//quality:test", "format", &stages, &files, &backend)
        .expect("path module is matched");
    assert_eq!(result.convergence, Convergence::Stable as i32);
    let paths: Vec<&str> = result
        .replacements
        .iter()
        .map(|replacement| replacement.path.as_str())
        .collect();
    assert_eq!(paths, ["shared/util.rs"]);
}

#[test]
pub(super) fn rustfmt_syntax_errors_keep_the_input_and_stay_diagnostics() {
    let backend = backend_for("rustfmt", rustfmt_tool(), unclosed_rustfmt);
    let stages = vec![stage("rustfmt", &["rust"], &["src/lib.rs", "src/child.rs"])];
    let files = vec![
        file("src/lib.rs", "mod child;  \n"),
        file("src/child.rs", "fn f( {\n"),
    ];
    let result = run_real_pipeline("//quality:test", "format", &stages, &files, &backend)
        .expect("syntax errors are diagnostics");
    assert_eq!(result.convergence, Convergence::Stable as i32);
    let paths: Vec<&str> = result
        .replacements
        .iter()
        .map(|replacement| replacement.path.as_str())
        .collect();
    assert_eq!(paths, ["src/lib.rs"]);
    assert_eq!(result.terminal_diagnostics.len(), 1);
    assert!(result
        .terminal_diagnostics
        .iter()
        .all(|diagnostic| diagnostic.path == "src/child.rs"
            && diagnostic.message.contains("unclosed delimiter")));
}

#[test]
pub(super) fn rustfmt_unparsable_supporting_module_fails_the_check() {
    let backend = backend_for("rustfmt", rustfmt_tool(), unclosed_rustfmt);
    let stages = vec![stage("rustfmt", &["rust"], &["src/lib.rs"])];
    let files = vec![file("src/lib.rs", "mod child;\n")];
    let siblings = vec![file("src/child.rs", "fn f( {\n")];
    let err = run_real_pipeline_with_siblings(
        "//quality:test",
        "format",
        &stages,
        &files,
        &siblings,
        &backend,
    )
    .expect_err("supporting syntax errors are not hidden");
    assert!(
        err.to_string()
            .contains("rustfmt cannot parse supporting file src/child.rs"),
        "{err}"
    );
}

#[test]
pub(super) fn vale_pipeline_reports_without_rewriting() {
    let tool = RealTool {
        config_rel: Some("vdir/.vale.ini".to_owned()),
        tool_files: vec![(
            "vdir/.vale.ini".to_owned(),
            b"StylesPath = styles\n".to_vec(),
        )],
        ..plain_tool()
    };
    let backend = backend_for("vale", tool, vale_hinted);
    let stages = vec![stage("vale", &["markdown"], &["doc.md"])];
    let files = vec![file("doc.md", "aaa cotton bbb\n")];
    let result = run_real_pipeline("//quality:test", "lint", &stages, &files, &backend)
        .expect("real pipeline");
    assert_eq!(result.convergence, Convergence::Stable as i32);
    assert_eq!(result.completed_rounds, 1);
    assert_eq!(result.initial_diagnostics.len(), 1);
    assert_eq!(result.initial_diagnostics[0].rule_id, "Test.Cotton");
    assert_eq!(
        (
            result.initial_diagnostics[0].start_byte,
            result.initial_diagnostics[0].end_byte
        ),
        (Some(4), Some(10))
    );
    assert!(!result.initial_diagnostics[0].fixable);
    assert_eq!(result.terminal_diagnostics.len(), 1);
    assert!(result.replacements.is_empty());
    assert!(encode_validated(&result).is_ok());
}

#[test]
pub(super) fn markdown_check_reports_workspace_keyed_findings() {
    let backend = backend_for("markdown_check", plain_tool(), markdown_links);
    let mut inputs = BTreeMap::new();
    inputs.insert("doc/guide.md".to_owned(), "# Guide\n\nBROKEN\n".to_owned());
    inputs.insert("README.md".to_owned(), "# Readme\n".to_owned());
    let findings = backend
        .diagnose("markdown_check", "lint", &inputs)
        .expect("diagnosed");
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].tool_id, "markdown_check");
    assert_eq!(findings[0].rule_id, "missing-file-target");
    assert_eq!(findings[0].path, "doc/guide.md");
    assert_eq!(findings[0].start_byte, Some(8));
    assert_eq!(findings[0].end_byte, Some(8));
    assert!(!findings[0].fixable);
}

#[test]
pub(super) fn markdown_check_pipeline_is_stable_without_rewriting() {
    let backend = backend_for("markdown_check", plain_tool(), markdown_links);
    let stages = vec![stage("markdown_check", &["markdown"], &["doc/guide.md"])];
    let files = vec![file("doc/guide.md", "# Guide\n\nBROKEN\n")];
    let result = run_real_pipeline("//quality:test", "lint", &stages, &files, &backend)
        .expect("real pipeline");
    assert_eq!(result.convergence, Convergence::Stable as i32);
    assert_eq!(result.completed_rounds, 1);
    assert_eq!(result.initial_diagnostics.len(), 1);
    assert_eq!(result.initial_diagnostics[0].tool_id, "markdown_check");
    assert_eq!(result.terminal_diagnostics.len(), 1);
    assert!(result.replacements.is_empty());
    assert!(encode_validated(&result).is_ok());
}

pub(super) fn markdown_sibling_links(
    argv: &[OsString],
    _cwd: &Path,
    env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    assert_hermetic(env);
    let mut out = String::new();
    let seen_sibling = argv.iter().any(|arg| arg == "--sibling");
    let mut index = 0;
    while index < argv.len() {
        if argv[index] == "--source" {
            let mapping = argv[index + 1].to_string_lossy().into_owned();
            let (workspace, absolute) = mapping.split_once('=').expect("--source maps WS=ABS");
            let bytes = std::fs::read(absolute).expect("checked file is materialized");
            let text = String::from_utf8(bytes).expect("checked bytes are UTF-8");
            if text.contains("BROKEN") && !seen_sibling {
                out.push_str(&format!(
                    "{{\"path\":\"{workspace}\",\"line\":2,\"kind\":\"missing-file-target\",\"message\":\"link target nope.md does not match a checked source\"}}\n"
                ));
            }
            index += 1;
        }
        index += 1;
    }
    Ok(ChildOutput {
        code: Some(0),
        stdout: out.into_bytes(),
        stderr: Vec::new(),
    })
}

#[test]
pub(super) fn markdown_siblings_reach_the_checker_invocation() {
    let backend = backend_for("markdown_check", plain_tool(), markdown_sibling_links);
    let mut siblings = BTreeMap::new();
    siblings.insert("LICENSE".to_owned(), "license text\n".to_owned());
    let without = backend
        .diagnose_with_siblings(
            "markdown_check",
            "lint",
            &single("doc/guide.md", "# Guide\n\nBROKEN\n"),
            &BTreeMap::new(),
        )
        .expect("diagnosed");
    assert_eq!(without.len(), 1);
    let with = backend
        .diagnose_with_siblings(
            "markdown_check",
            "lint",
            &single("doc/guide.md", "# Guide\n\nBROKEN\n"),
            &siblings,
        )
        .expect("diagnosed");
    assert!(with.is_empty());
}

#[test]
pub(super) fn markdown_siblings_stay_out_of_snapshots_and_stages() {
    let backend = backend_for("markdown_check", plain_tool(), markdown_links);
    let stages = vec![stage("markdown_check", &["markdown"], &["doc/guide.md"])];
    let files = vec![file("doc/guide.md", "# Guide\n")];
    let siblings = vec![file("LICENSE", "license text\n")];
    let result = run_real_pipeline_with_siblings(
        "//quality:test",
        "lint",
        &stages,
        &files,
        &siblings,
        &backend,
    )
    .expect("real pipeline");
    assert!(result.initial_diagnostics.is_empty());
    assert!(result.terminal_diagnostics.is_empty());
    assert_eq!(result.original_snapshot.len(), 1);
    assert_eq!(result.original_snapshot[0].path, "doc/guide.md");
    assert_eq!(result.terminal_snapshot.len(), 1);
    assert!(result.replacements.is_empty());
    assert!(encode_validated(&result).is_ok());
}

#[test]
fn resolve_inputs_are_staged_but_never_become_quality_sources() {
    let backend = backend_for("markdown_check", plain_tool(), markdown_links);
    let stages = vec![stage("markdown_check", &["markdown"], &["doc/guide.md"])];
    let files = vec![file("doc/guide.md", "# Guide\n")];
    let siblings = vec![file("LICENSE", "license\n")];
    let resolve = vec![file("doc/target.md", "# Target\n")];
    let result = run_real_pipeline_with_resolve(
        "//quality:test",
        "lint",
        &stages,
        &files,
        &siblings,
        &resolve,
        &backend,
    )
    .expect("resolve pipeline");
    assert_eq!(result.original_snapshot.len(), 1);
    assert_eq!(result.original_snapshot[0].path, "doc/guide.md");
    assert!(result.initial_diagnostics.is_empty());
    for duplicate in [
        vec![file("doc/guide.md", "duplicate")],
        vec![file("LICENSE", "duplicate")],
        vec![
            file("doc/target.md", "first"),
            file("doc/target.md", "second"),
        ],
    ] {
        assert!(matches!(
            run_real_pipeline_with_resolve(
                "//quality:test",
                "lint",
                &stages,
                &files,
                &siblings,
                &duplicate,
                &backend
            ),
            Err(RunnerError::DuplicateFile { .. })
        ));
    }
    let invalid = vec![FileInput {
        path: "doc/target.md".to_owned(),
        bytes: vec![0xff],
    }];
    assert_eq!(
        run_real_pipeline_with_resolve(
            "//quality:test",
            "lint",
            &stages,
            &files,
            &siblings,
            &invalid,
            &backend
        ),
        Err(RunnerError::InvalidUtf8 {
            path: "doc/target.md".to_owned()
        })
    );
}

#[test]
pub(super) fn sibling_colliding_with_a_source_fails() {
    let backend = backend_for("markdown_check", plain_tool(), markdown_links);
    let stages = vec![stage("markdown_check", &["markdown"], &["doc/guide.md"])];
    let files = vec![file("doc/guide.md", "# Guide\n")];
    let siblings = vec![file("doc/guide.md", "other\n")];
    assert_eq!(
        run_real_pipeline_with_siblings(
            "//quality:test",
            "lint",
            &stages,
            &files,
            &siblings,
            &backend
        ),
        Err(RunnerError::DuplicateFile {
            path: "doc/guide.md".to_owned(),
        })
    );
}

#[test]
pub(super) fn sibling_non_utf8_fails() {
    let backend = backend_for("markdown_check", plain_tool(), markdown_links);
    let stages = vec![stage("markdown_check", &["markdown"], &["doc/guide.md"])];
    let files = vec![file("doc/guide.md", "# Guide\n")];
    let siblings = vec![FileInput {
        path: "LICENSE".to_owned(),
        bytes: vec![0xff],
    }];
    assert_eq!(
        run_real_pipeline_with_siblings(
            "//quality:test",
            "lint",
            &stages,
            &files,
            &siblings,
            &backend
        ),
        Err(RunnerError::InvalidUtf8 {
            path: "LICENSE".to_owned(),
        })
    );
}

#[test]
pub(super) fn stage_naming_a_sibling_fails_missing_file() {
    let backend = backend_for("markdown_check", plain_tool(), markdown_links);
    let stages = vec![stage(
        "markdown_check",
        &["markdown"],
        &["doc/guide.md", "LICENSE"],
    )];
    let files = vec![file("doc/guide.md", "# Guide\n")];
    let siblings = vec![file("LICENSE", "license text\n")];
    assert_eq!(
        run_real_pipeline_with_siblings(
            "//quality:test",
            "lint",
            &stages,
            &files,
            &siblings,
            &backend
        ),
        Err(RunnerError::MissingFile {
            path: "LICENSE".to_owned(),
        })
    );
}

#[test]
pub(super) fn markdown_check_failure_fails_the_action() {
    let backend = backend_for("markdown_check", plain_tool(), markdown_broken);
    let err = backend
        .diagnose(
            "markdown_check",
            "lint",
            &single("doc/guide.md", "# Guide\n"),
        )
        .expect_err("exit 2 fails");
    assert!(matches!(err, RunnerError::ToolOutput { .. }));
    assert!(err.to_string().contains("findings exist only on exit 0"));
}

#[test]
pub(super) fn unknown_stage_tool_fails_real_validation() {
    let backend = backend_for("rustfmt", plain_tool(), rustfmt_defaults);
    let stages = vec![stage("nope", &["rust"], &["src/main.rs"])];
    let files = vec![file("src/main.rs", "x\n")];
    assert_eq!(
        run_real_pipeline("//quality:test", "format", &stages, &files, &backend),
        Err(RunnerError::UnknownTool {
            tool_id: "nope".to_owned(),
        })
    );
}

#[test]
pub(super) fn buildifier_lint_keeps_only_categorized_warnings() {
    let backend = backend_for("buildifier", plain_tool(), buildifier_plain);
    let findings = backend
        .diagnose("buildifier", "lint", &single("a.bzl", "x = 1\n"))
        .expect("diagnosed");
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].rule_id, "module-docstring");
    assert_eq!(findings[0].start_byte, Some(0));
}

#[test]
pub(super) fn buildifier_format_keeps_only_format_findings() {
    let backend = backend_for("buildifier", plain_tool(), buildifier_plain);
    let findings = backend
        .diagnose("buildifier", "format", &single("a.bzl", "x = 1\n"))
        .expect("diagnosed");
    assert_eq!(findings.len(), 1);
    assert!(findings[0].rule_id.is_empty());
    assert_eq!(findings[0].message, "file is not formatted");
}

#[test]
pub(super) fn buildifier_syntax_errors_survive_both_capabilities() {
    for capability in ["lint", "format"] {
        let backend = backend_for("buildifier", plain_tool(), buildifier_invalid);
        let findings = backend
            .diagnose("buildifier", capability, &single("BUILD.bazel", "load(\n"))
            .expect("diagnosed");
        assert_eq!(findings.len(), 1, "{capability}");
        assert!(findings[0].message.contains("syntax error"), "{capability}");
        assert_eq!(findings[0].start_byte, Some(5), "{capability}");
    }
}

#[test]
pub(super) fn buildifier_hint_runs_from_the_config_dir() {
    let tool = RealTool {
        config_rel: Some("cfg/.buildifier.json".to_owned()),
        tool_files: vec![("cfg/.buildifier.json".to_owned(), b"{}".to_vec())],
        ..plain_tool()
    };
    let backend = backend_for("buildifier", tool, buildifier_hinted);
    let findings = backend
        .diagnose("buildifier", "lint", &single("a.bzl", "x = 1\n"))
        .expect("diagnosed");
    assert_eq!(findings.len(), 1);
}

#[test]
pub(super) fn taplo_lint_reports_syntax_blocks() {
    let backend = backend_for("taplo", plain_tool(), taplo_either);
    let findings = backend
        .diagnose("taplo", "lint", &single("a.toml", "a = 1\nb = \n"))
        .expect("diagnosed");
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].tool_id, "taplo");
}

#[test]
pub(super) fn taplo_format_reports_unformatted_files() {
    let tool = RealTool {
        config_rel: Some("taplo.toml".to_owned()),
        tool_files: vec![("taplo.toml".to_owned(), b"".to_vec())],
        ..plain_tool()
    };
    let backend = backend_for("taplo", tool, taplo_either);
    let findings = backend
        .diagnose("taplo", "format", &single("a.toml", "a=1\n"))
        .expect("diagnosed");
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].message, "file is not formatted");
}

#[test]
pub(super) fn taplo_config_that_is_a_source_uses_pipeline_text() {
    let tool = RealTool {
        config_rel: Some(".taplo.toml".to_owned()),
        tool_files: vec![(".taplo.toml".to_owned(), b"a = 1\n".to_vec())],
        ..plain_tool()
    };
    let backend = backend_for("taplo", tool, taplo_reads_disk);
    let findings = backend
        .diagnose("taplo", "format", &single(".taplo.toml", "a=1\n"))
        .expect("diagnosed");
    assert_eq!(findings.len(), 1);
}

#[test]
pub(super) fn taplo_fix_of_a_config_source_uses_pipeline_text() {
    let tool = RealTool {
        config_rel: Some(".taplo.toml".to_owned()),
        tool_files: vec![(".taplo.toml".to_owned(), b"b=2\n".to_vec())],
        ..plain_tool()
    };
    let backend = backend_for("taplo", tool, taplo_reads_disk);
    let fixed = backend
        .apply_fix("taplo", ".taplo.toml", "a=1\n", "format")
        .expect("fixed");
    assert_eq!(fixed, "a = 1\n");
}

#[test]
pub(super) fn taplo_pipeline_formats_a_config_source_to_stable() {
    let tool = RealTool {
        config_rel: Some(".taplo.toml".to_owned()),
        tool_files: vec![(".taplo.toml".to_owned(), b"a = 1\n".to_vec())],
        ..plain_tool()
    };
    let backend = backend_for("taplo", tool, taplo_reads_disk);
    let stages = vec![stage("taplo", &["format"], &[".taplo.toml"])];
    let files = vec![file(".taplo.toml", "a=1\n")];
    let result = run_real_pipeline("//quality:test", "format", &stages, &files, &backend)
        .expect("real pipeline");
    assert_eq!(result.convergence, Convergence::Stable as i32);
    assert!(result.terminal_diagnostics.is_empty());
    assert_eq!(result.initial_diagnostics.len(), 1);
    assert!(result.initial_diagnostics.iter().all(|d| d.fixable));
    assert_eq!(result.replacements.len(), 1);
    assert!(encode_validated(&result).is_ok());
}

#[test]
pub(super) fn rustfmt_hinted_config_reaches_the_tool() {
    let tool = RealTool {
        config_rel: Some("hint.toml".to_owned()),
        edition: Some("2021".to_owned()),
        tool_files: vec![("hint.toml".to_owned(), b"".to_vec())],
        ..plain_tool()
    };
    let backend = backend_for("rustfmt", tool, rustfmt_hinted);
    let findings = backend
        .diagnose(
            "rustfmt",
            "format",
            &single("src/main.rs", "fn main() {}\n"),
        )
        .expect("diagnosed");
    assert!(findings.is_empty());
}

#[test]
pub(super) fn rustfmt_without_hint_gets_materialized_defaults() {
    let backend = backend_for("rustfmt", rustfmt_tool(), rustfmt_defaults);
    let findings = backend
        .diagnose(
            "rustfmt",
            "format",
            &single("src/main.rs", "fn main() {}\n"),
        )
        .expect("diagnosed");
    assert!(findings.is_empty());
}

pub(super) fn rustfmt_edition_passthrough(
    argv: &[OsString],
    _cwd: &Path,
    env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    assert_hermetic(env);
    let edition = argv
        .windows(2)
        .find(|pair| pair[0] == "--edition")
        .map(|pair| pair[1].clone())
        .expect("rustfmt passes --edition");
    assert_eq!(
        edition,
        OsString::from("2018"),
        "caller edition reaches the tool"
    );
    Ok(ChildOutput {
        code: Some(0),
        stdout: Vec::new(),
        stderr: Vec::new(),
    })
}

#[test]
pub(super) fn rustfmt_caller_edition_reaches_the_tool() {
    let tool = RealTool {
        edition: Some("2018".to_owned()),
        ..plain_tool()
    };
    let backend = backend_for("rustfmt", tool, rustfmt_edition_passthrough);
    let findings = backend
        .diagnose(
            "rustfmt",
            "format",
            &single("src/main.rs", "fn main() {}\n"),
        )
        .expect("diagnosed");
    assert!(findings.is_empty());
}

#[test]
pub(super) fn rustfmt_without_edition_fails_the_action() {
    let backend = backend_for("rustfmt", plain_tool(), rustfmt_defaults);
    let err = backend
        .diagnose(
            "rustfmt",
            "format",
            &single("src/main.rs", "fn main() {}\n"),
        )
        .expect_err("missing edition fails");
    assert!(matches!(err, RunnerError::ToolExecution { .. }));
    assert!(err.to_string().contains("request tool edition"));
    let err = backend
        .apply_fix("rustfmt", "src/main.rs", "fn main() {}\n", "format")
        .expect_err("missing edition fails");
    assert!(matches!(err, RunnerError::ToolExecution { .. }));
    assert!(err.to_string().contains("request tool edition"));
}

pub(super) fn gofumpt_lang_passthrough(
    argv: &[OsString],
    _cwd: &Path,
    env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    assert_hermetic(env);
    assert_eq!(
        argv.get(1),
        Some(&OsString::from("-lang=go1.26")),
        "the aspect's Go language version reaches gofumpt"
    );
    Ok(ChildOutput {
        code: Some(0),
        stdout: Vec::new(),
        stderr: Vec::new(),
    })
}

#[test]
pub(super) fn gofumpt_check_and_fix_share_the_caller_lang() {
    let backend = backend_for("gofumpt", gofumpt_tool(), gofumpt_lang_passthrough);
    let findings = backend
        .diagnose("gofumpt", "format", &single("a.go", "package a\n"))
        .expect("diagnosed");
    assert!(findings.is_empty());
    backend
        .apply_fix("gofumpt", "a.go", "package a\n", "format")
        .expect("fixed");
}

#[test]
pub(super) fn gofumpt_without_lang_fails_the_action() {
    let backend = backend_for("gofumpt", plain_tool(), gofumpt_lang_passthrough);
    let err = backend
        .diagnose("gofumpt", "format", &single("a.go", "package a\n"))
        .expect_err("missing lang fails");
    assert!(matches!(err, RunnerError::ToolExecution { .. }));
    assert!(err.to_string().contains("--tool-edition"));
    let err = backend
        .apply_fix("gofumpt", "a.go", "package a\n", "format")
        .expect_err("missing lang fails");
    assert!(matches!(err, RunnerError::ToolExecution { .. }));
    assert!(err.to_string().contains("--tool-edition"));
}

pub(super) fn ktlint_defaults(
    argv: &[OsString],
    _cwd: &Path,
    env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    assert_hermetic(env);
    let config = argv
        .windows(2)
        .find(|pair| pair[0] == "--editorconfig")
        .map(|pair| pair[1].clone())
        .expect("ktlint passes --editorconfig");
    assert!(
        Path::new(&config).ends_with(KTLINT_DEFAULTS_REL),
        "unhinted ktlint gets the materialized defaults"
    );
    let text = std::fs::read_to_string(&config).expect("defaults file is materialized");
    assert!(text.contains("ktlint_code_style = intellij_idea"));
    assert!(text.contains("ktlint_standard_function-signature = disabled"));
    let _ = last_file(argv);
    let stdout = if argv.iter().any(|arg| arg == "--format") {
        Vec::new()
    } else {
        br#"{"version":"2.1.0","runs":[{"tool":{"driver":{"name":"ktlint"}},"results":[]}]}"#
            .to_vec()
    };
    Ok(ChildOutput {
        code: Some(0),
        stdout,
        stderr: Vec::new(),
    })
}

#[test]
pub(super) fn ktlint_runs_with_the_ktfmt_compatible_defaults() {
    let backend = backend_for("ktlint", plain_tool(), ktlint_defaults);
    let findings = backend
        .diagnose("ktlint", "lint", &single("src/A.kt", "class A\n"))
        .expect("ktlint diagnose");
    assert!(findings.is_empty());
    let fixed = backend
        .apply_fix("ktlint", "src/A.kt", "class A\n", "lint")
        .expect("ktlint fix");
    assert_eq!(fixed, "class A\n");
}

pub(super) fn no_spawn(
    _: &[OsString],
    _: &Path,
    _: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    panic!("delegated check must not spawn")
}

#[test]
pub(super) fn clippy_delegated_parses_upstream_file_without_spawning() {
    let fixture = upstream_file("warn", DELEGATED_CLIPPY_WARN);
    let path = fixture.path().to_path_buf();
    let backend = backend_for("clippy", delegated_tool(path.clone()), no_spawn);
    let findings = backend
        .diagnose(
            "clippy",
            "lint",
            &single("src/main.rs", "let y = v.len() == 0;\n"),
        )
        .expect("diagnosed");
    std::fs::remove_file(&path).expect("remove upstream fixture");
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].rule_id, "clippy::len_zero");
    assert_eq!(
        (findings[0].start_byte, findings[0].end_byte),
        (Some(8), Some(20))
    );
    assert_eq!(findings[0].path, "src/main.rs");
    assert!(!findings[0].fixable);
}

#[test]
pub(super) fn clippy_delegated_related_context_resolves_against_declared_files() {
    let report = concat!(
        r#"{"$message_type":"diagnostic","message":"mismatched types","code":{"code":"E0308","explanation":null},"level":"warning","spans":["#,
        r#"{"file_name":"src/a.rs","byte_start":0,"byte_end":1,"line_start":1,"line_end":1,"column_start":1,"column_end":2,"is_primary":true,"text":[],"label":"expected `i32`","suggested_replacement":null,"suggestion_applicability":null,"expansion":null},"#,
        r#"{"file_name":"src/b.rs","byte_start":0,"byte_end":1,"line_start":2,"line_end":2,"column_start":2,"column_end":3,"is_primary":false,"text":[],"label":"defined here","suggested_replacement":null,"suggestion_applicability":null,"expansion":null}],"#,
        r#""children":[{"message":"see std","code":null,"level":"note","spans":[{"file_name":"/rustc/x/core.rs","byte_start":0,"byte_end":1,"line_start":1,"line_end":1,"column_start":1,"column_end":2,"is_primary":true,"text":[],"label":null,"suggested_replacement":null,"suggestion_applicability":null,"expansion":null}],"children":[],"rendered":null}],"rendered":null}"#
    );
    let fixture = upstream_file("related", report);
    let path = fixture.path().to_path_buf();
    let backend = backend_for("clippy", delegated_tool(path.clone()), no_spawn);
    let mut files = single("src/a.rs", "let x;\n");
    files.insert("src/b.rs".to_owned(), "a\r\n\u{e9}y\n".to_owned());
    let findings = backend
        .diagnose("clippy", "lint", &files)
        .expect("diagnosed");
    std::fs::remove_file(&path).expect("remove upstream fixture");
    assert_eq!(findings.len(), 1);
    let related = &findings[0].related;
    assert_eq!(related.len(), 3);
    assert_eq!(related[0].path, "src/a.rs");
    assert_eq!(related[1].path, "src/b.rs");
    assert_eq!(
        (related[1].start_byte, related[1].end_byte),
        (Some(5), Some(6))
    );
    assert_eq!(related[2].path, "");
    assert_eq!(related[2].message, "see std");
    assert!(!findings[0].fixable);
}

#[test]
pub(super) fn clippy_delegated_fix_is_check_only() {
    let fixture = upstream_file("fix", DELEGATED_CLIPPY_WARN);
    let path = fixture.path().to_path_buf();
    let backend = backend_for("clippy", delegated_tool(path.clone()), no_spawn);
    let patched = backend
        .apply_fix("clippy", "src/main.rs", "let y = v.len() == 0;\n", "lint")
        .expect("check-only keeps input");
    std::fs::remove_file(&path).expect("remove upstream fixture");
    assert_eq!(patched, "let y = v.len() == 0;\n");
}

#[test]
pub(super) fn clippy_delegated_missing_file_fails_the_action() {
    let scratch = tempfile::Builder::new()
        .prefix("dx-delegated-clippy-")
        .tempdir_in(std::env::temp_dir())
        .expect("scratch");
    let missing = scratch.path().join("absent");
    let backend = backend_for("clippy", delegated_tool(missing), no_spawn);
    backend
        .diagnose(
            "clippy",
            "lint",
            &single("src/main.rs", "let y = v.len() == 0;\n"),
        )
        .expect_err("missing upstream diagnostics fail");
}

#[test]
pub(super) fn clippy_delegated_empty_file_reports_no_findings() {
    let fixture = upstream_file("empty", "");
    let path = fixture.path().to_path_buf();
    let backend = backend_for("clippy", delegated_tool(path.clone()), no_spawn);
    let findings = backend
        .diagnose(
            "clippy",
            "lint",
            &single("src/main.rs", "let y = v.len() == 0;\n"),
        )
        .expect("diagnosed");
    std::fs::remove_file(&path).expect("remove upstream fixture");
    assert!(findings.is_empty());
}

#[test]
pub(super) fn rustc_delegated_parses_upstream_file_without_spawning() {
    let fixture = upstream_file("rustc-warn", DELEGATED_RUSTC_WARN);
    let path = fixture.path().to_path_buf();
    let backend = backend_for("rustc", delegated_tool(path.clone()), no_spawn);
    let findings = backend
        .diagnose(
            "rustc",
            "typecheck",
            &single("src/main.rs", "fn f(x: i32) {}\nfn g() { f(\"oops\"); }\n"),
        )
        .expect("diagnosed");
    std::fs::remove_file(&path).expect("remove upstream fixture");
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].rule_id, "E0308");
    assert_eq!(
        (findings[0].start_byte, findings[0].end_byte),
        (Some(28), Some(32))
    );
    assert_eq!(findings[0].path, "src/main.rs");
    assert!(!findings[0].fixable);
}

#[test]
pub(super) fn rustc_delegated_fix_is_check_only() {
    let fixture = upstream_file("rustc-fix", DELEGATED_RUSTC_WARN);
    let path = fixture.path().to_path_buf();
    let backend = backend_for("rustc", delegated_tool(path.clone()), no_spawn);
    let text = "fn f(x: i32) {}\nfn g() { f(\"oops\"); }\n";
    let patched = backend
        .apply_fix("rustc", "src/main.rs", text, "typecheck")
        .expect("check-only keeps input");
    std::fs::remove_file(&path).expect("remove upstream fixture");
    assert_eq!(patched, text);
}

#[test]
pub(super) fn clang_tidy_without_upstream_context_fails_closed() {
    let backend = backend_for("clang_tidy", plain_tool(), no_spawn);
    let error = backend
        .diagnose(
            "clang_tidy",
            "lint",
            &single("a.c", "int check(int value) {\n  return value;\n}\n"),
        )
        .expect_err("direct clang-tidy without compilation context");
    assert_eq!(
        error,
        RunnerError::ToolExecution {
            tool_id: "clang_tidy".to_owned(),
            detail: "clang_tidy requires upstream diagnostics from a declared delegated action: direct execution without the authoritative compilation context is rejected"
                .to_owned(),
        }
    );
}

#[test]
pub(super) fn cppcheck_without_a_declared_binary_fails_closed() {
    let scratch = tempfile::Builder::new()
        .prefix("dx-cppcheck-missing-")
        .tempdir_in(std::env::temp_dir())
        .expect("scratch");
    let tool = RealTool {
        binary: scratch.path().join("cppcheck"),
        ..plain_tool()
    };
    let backend = backend_for("cppcheck", tool, real_spawn);
    let error = backend
        .diagnose(
            "cppcheck",
            "lint",
            &single("src/a.c", "int check(int value) {\n  return value;\n}\n"),
        )
        .expect_err("cppcheck without a binary is rejected");
    assert!(
        matches!(error, RunnerError::ToolExecution { .. }),
        "{error}"
    );
    assert!(error.to_string().contains("cppcheck"), "{error}");
}

#[test]
pub(super) fn rustc_delegated_missing_file_fails_the_action() {
    let scratch = tempfile::Builder::new()
        .prefix("dx-delegated-rustc-")
        .tempdir_in(std::env::temp_dir())
        .expect("scratch");
    let missing = scratch.path().join("absent");
    let backend = backend_for("rustc", delegated_tool(missing), no_spawn);
    backend
        .diagnose(
            "rustc",
            "typecheck",
            &single("src/main.rs", "fn f(x: i32) {}\nfn g() { f(\"oops\"); }\n"),
        )
        .expect_err("missing upstream diagnostics fail");
}

#[test]
pub(super) fn rustc_delegated_empty_file_reports_no_findings() {
    let fixture = upstream_file("rustc-empty", "");
    let path = fixture.path().to_path_buf();
    let backend = backend_for("rustc", delegated_tool(path.clone()), no_spawn);
    let findings = backend
        .diagnose(
            "rustc",
            "typecheck",
            &single("src/main.rs", "fn f(x: i32) {}\nfn g() { f(\"oops\"); }\n"),
        )
        .expect("diagnosed");
    std::fs::remove_file(&path).expect("remove upstream fixture");
    assert!(findings.is_empty());
}

#[test]
pub(super) fn rustc_apply_is_check_only() {
    let fixture = upstream_file("rustc-check-only", DELEGATED_RUSTC_WARN);
    let path = fixture.path().to_path_buf();
    let backend = backend_for("rustc", delegated_tool(path.clone()), no_spawn);
    let text = "fn f(x: i32) {}\nfn g() { f(\"oops\"); }\n";
    let patched = backend
        .apply_fix("rustc", "src/main.rs", text, "typecheck")
        .expect("unchanged");
    std::fs::remove_file(&path).expect("remove upstream fixture");
    assert_eq!(patched, text);
}

#[test]
pub(super) fn diagnose_unknown_tool_fails() {
    let backend = backend_for("rustfmt", plain_tool(), rustfmt_defaults);
    assert_eq!(
        backend.diagnose("nope", "format", &single("src/main.rs", "x\n")),
        Err(RunnerError::UnknownTool {
            tool_id: "nope".to_owned(),
        })
    );
    assert_eq!(
        backend.apply_fix("nope", "src/main.rs", "x\n", "lint"),
        Err(RunnerError::UnknownTool {
            tool_id: "nope".to_owned(),
        })
    );
}

#[test]
pub(super) fn custom_tool_ids_have_no_dispatch() {
    let backend = backend_for("custom", plain_tool(), rustfmt_defaults);
    let err = backend
        .diagnose("custom", "format", &single("src/main.rs", "x\n"))
        .expect_err("no dispatch");
    assert!(matches!(err, RunnerError::ToolExecution { .. }));
    assert!(err.to_string().contains("unsupported real tool"));
    let err = backend
        .apply_fix("custom", "src/main.rs", "x\n", "lint")
        .expect_err("no dispatch");
    assert!(matches!(err, RunnerError::ToolExecution { .. }));
}

#[test]
pub(super) fn spawn_failure_fails_the_action() {
    let backend = backend_for("rustfmt", rustfmt_tool(), missing_spawn);
    let err = backend
        .diagnose("rustfmt", "format", &single("src/main.rs", "x\n"))
        .expect_err("spawn fails");
    assert!(matches!(err, RunnerError::ToolExecution { .. }));
    assert!(err.to_string().contains("spawn"));
}

#[test]
pub(super) fn grammar_failure_fails_the_action() {
    let backend = backend_for("buildifier", plain_tool(), garbage_stdout);
    let err = backend
        .diagnose("buildifier", "lint", &single("a.bzl", "x = 1\n"))
        .expect_err("grammar fails");
    assert!(matches!(err, RunnerError::ToolOutput { .. }));
}

#[test]
pub(super) fn taplo_grammar_failure_fails_the_action() {
    let backend = backend_for("taplo", plain_tool(), taplo_garbage);
    let err = backend
        .diagnose("taplo", "lint", &single("a.toml", "a = 1\n"))
        .expect_err("grammar fails");
    assert!(matches!(err, RunnerError::ToolOutput { .. }));
    let err = backend
        .apply_fix("clippy", "src/main.rs", "x\n", "lint")
        .expect_err("clippy without resolution fails");
    assert!(matches!(err, RunnerError::UnknownTool { .. }));
}

#[test]
pub(super) fn unplaceable_findings_fail_the_action() {
    let backend = backend_for("buildifier", plain_tool(), buildifier_far);
    let err = backend
        .diagnose("buildifier", "lint", &single("a.bzl", "x = 1\n"))
        .expect_err("placement fails");
    assert!(matches!(err, RunnerError::UnplaceableFinding { .. }));
}

#[test]
pub(super) fn vale_without_config_fails_fast() {
    let backend = backend_for("vale", plain_tool(), vale_hinted);
    let err = backend
        .diagnose("vale", "lint", &single("doc.md", "cotton\n"))
        .expect_err("vale needs a config");
    assert!(matches!(err, RunnerError::ToolExecution { .. }));
    assert!(err.to_string().contains("requires a config"));
}
