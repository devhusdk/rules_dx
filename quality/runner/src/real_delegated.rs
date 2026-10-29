use super::real_fixtures::*;
use super::*;

#[test]
fn delegated_diagnostics_fail_closed_when_missing_or_malformed() {
    for tool_id in [
        "clippy",
        "rustc",
        "roslyn",
        "csc",
        "fsc",
        "fsharplint",
        "qmllint",
        "govet",
        "gotype",
        "errcheck",
        "staticcheck",
        "clang_tidy",
        "cppcheck",
        "rubocop",
        "shellcheck",
        "yamllint",
        "hadolint",
        "statix",
        "deadnix",
        "kubeconform",
        "checkmake",
    ] {
        let scratch = tempfile::tempdir().expect("scratch");
        let tool = delegated_tool(scratch.path().join("missing"));
        let backend = backend_for(tool_id, tool, must_not_spawn);
        let capability = if tool_id == "rustc" || tool_id == "gotype" {
            "typecheck"
        } else {
            "lint"
        };
        let error = backend
            .diagnose(tool_id, capability, &single("src/a.txt", "x\n"))
            .expect_err("missing diagnostics");
        assert!(
            error.to_string().contains("upstream diagnostics"),
            "{tool_id}: {error}"
        );
        let report = upstream_file(tool_id, "{<error malformed");
        let backend = backend_for(
            tool_id,
            delegated_tool(report.path().to_path_buf()),
            must_not_spawn,
        );
        assert!(
            backend
                .diagnose(tool_id, capability, &single("src/a.txt", "x\n"))
                .is_err(),
            "{tool_id}"
        );
    }
}

#[test]
fn delegated_findings_are_reanchored_to_the_staged_source() {
    for (tool_id, report) in [
        (
            "qmllint",
            r#"{"files":[{"filename":"src/a.scala","success":false,"warnings":[{"column":1,"id":"demo","line":1,"message":"finding","type":"warning"}]}],"revision":4}"#,
        ),
        (
            "roslyn",
            r#"{"version":"2.1.0","runs":[{"tool":{"driver":{"name":"csc"}},"results":[{"ruleId":"CA1822","level":"warning","message":{"text":"finding"},"locations":[{"physicalLocation":{"artifactLocation":{"uri":"src/a.scala"},"region":{"startLine":1,"startColumn":1,"endLine":1,"endColumn":2}}}]}]}]}"#,
        ),
        ("shellcheck", "src/a.scala:1:1: warning: finding [SC2086]\n"),
        (
            "checkmake",
            r#"[{"rule":"minphony","violation":"finding","file_name":"src/a.scala","line_number":1}]"#,
        ),
        (
            "hadolint",
            r#"[{"code":"DL4006","column":1,"file":"src/a.scala","level":"warning","line":1,"message":"finding"}]"#,
        ),
        (
            "statix",
            r#"{"file":"src/a.scala","report":[{"note":"finding","code":2,"severity":"Warn","diagnostics":[{"at":{"from":{"line":1,"column":1},"to":{"line":1,"column":2}},"message":"finding","suggestion":null}]}]}"#,
        ),
        (
            "deadnix",
            r#"{"file":"src/a.scala","results":[{"column":1,"endColumn":2,"line":1,"message":"Unused let binding: finding"}]}"#,
        ),
        (
            "kubeconform",
            r#"{"resources":[{"filename":"src/a.scala","kind":"Pod","name":"a","version":"v1","status":"statusInvalid","msg":"finding","validationErrors":[{"path":"/spec","msg":"finding"}]}]}"#,
        ),
        (
            "yamllint",
            "src/a.scala:1:1: [warning] finding (trailing-spaces)\n",
        ),
        (
            "govet",
            r#"{"p":{"printf":[{"posn":"src/a.scala:1:1","message":"finding"}]}}"#,
        ),
        (
            "errcheck",
            r#"{"p":{"errcheck":[{"posn":"src/a.scala:1:1","message":"finding"}]}}"#,
        ),
        (
            "clang_tidy",
            "src/a.scala:1:1: warning: finding [demo-check]\n",
        ),
        (
            "cppcheck",
            r#"<results version="2"><errors><error id="demo" severity="warning" msg="finding"><location file="src/a.scala" line="1" column="1"/></error></errors></results>"#,
        ),
        (
            "staticcheck",
            r#"{"code":"S1000","severity":"warning","location":{"file":"src/a.scala","line":1,"column":1},"message":"finding"}"#,
        ),
        (
            "fsharplint",
            r#"{"path":"src/a.scala","startLine":1,"startColumn":1,"endLine":1,"endColumn":2,"rule":"Rule","message":"finding"}"#,
        ),
    ] {
        let upstream = upstream_file(tool_id, report);
        let backend = backend_for(
            tool_id,
            delegated_tool(upstream.path().to_path_buf()),
            must_not_spawn,
        );
        let findings = backend
            .diagnose(tool_id, "lint", &single("src/a.scala", "xx\n"))
            .expect("delegated finding");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].path, "src/a.scala");
    }
}

fn empty_report(
    argv: &[OsString],
    _cwd: &Path,
    env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    assert_hermetic(env);
    let _ = last_file(argv);
    Ok(ChildOutput {
        code: Some(0),
        stdout: Vec::new(),
        stderr: Vec::new(),
    })
}

fn must_not_spawn(
    argv: &[OsString],
    _cwd: &Path,
    env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    assert_hermetic(env);
    panic!("delegated tool spawned: {:?}", argv);
}

#[test]
pub(super) fn every_real_tool_reaches_a_check_arm() {
    for &tool_id in REAL_TOOLS {
        let backend = backend_for(tool_id, plain_tool(), empty_report);
        for capability in ["lint", "format"] {
            match backend.diagnose(tool_id, capability, &single("a.txt", "x\n")) {
                Ok(_) => {}
                Err(err) => assert!(
                    !err.to_string().contains("unsupported real tool"),
                    "{tool_id} fell through the dispatch: {err}"
                ),
            }
        }
    }
}

#[test]
pub(super) fn every_real_fix_tool_reaches_a_fix_round() {
    for tool_id in [
        "buf",
        "clang_format",
        "cue",
        "csharpier",
        "dart",
        "dfmt",
        "djlint",
        "erlfmt",
        "fantomas",
        "ffmt",
        "gersemi",
        "gofumpt",
        "jsonnetfmt",
        "julia_format",
        "latexindent",
        "ktlint",
        "mbake",
        "mix_format",
        "modfmt",
        "nixfmt",
        "ormolu",
        "opa",
        "perltidy",
        "pkl",
        "psscriptanalyzer",
        "qmlformat",
        "scalafmt",
        "shfmt",
        "standardrb",
        "stylua",
        "terraform",
        "typstyle",
        "yamlfmt",
        "zig_fmt",
    ] {
        let tool = if tool_id == "gofumpt" {
            gofumpt_tool()
        } else {
            plain_tool()
        };
        let backend = backend_for(tool_id, tool, empty_report);
        let fixed = backend
            .apply_fix(tool_id, "a.txt", "x\n", "format")
            .expect("fix round reaches the tool");
        assert_eq!(fixed, "x\n");
    }
}

#[test]
pub(super) fn delegated_tools_read_diagnostics_without_spawning() {
    for (tool_id, report) in [
        ("clang_tidy", ""),
        (
            "cppcheck",
            r#"<results version="2"><errors></errors></results>"#,
        ),
        ("djlint", ""),
        ("errcheck", "{}"),
        ("fsharplint", ""),
        ("gotype", "{}"),
        ("govet", "{}"),
        ("keep_sorted", ""),
        (
            "qmllint",
            r#"{"files":[{"filename":"a.txt","success":true,"warnings":[]}],"revision":4}"#,
        ),
        (
            "roslyn",
            r#"{"version":"2.1.0","runs":[{"tool":{"driver":{"name":"csc"}},"results":[]}]}"#,
        ),
        ("rubocop", r#"{"files":[{"path":"a.txt","offenses":[]}]}"#),
        ("shellcheck", ""),
        ("hadolint", "[]"),
        ("statix", ""),
        ("deadnix", ""),
        ("kubeconform", r#"{"resources":[]}"#),
        ("checkmake", ""),
        ("staticcheck", ""),
        ("yamllint", ""),
    ] {
        let upstream = upstream_file(tool_id, report);
        let tool = delegated_tool(upstream.path().to_path_buf());
        let backend = backend_for(tool_id, tool, must_not_spawn);
        let found = backend
            .diagnose(tool_id, "lint", &single("a.txt", "x\n"))
            .unwrap_or_else(|err| panic!("{tool_id}: {err}"));
        assert!(found.is_empty(), "{tool_id}");
    }
}

fn erlfmt_rewrites(
    argv: &[OsString],
    _cwd: &Path,
    env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    assert_hermetic(env);
    let file = last_file(argv);
    let broken = std::fs::read_to_string(&file)
        .expect("read staged file")
        .contains("broken");
    assert_eq!(argv[1], OsString::from("--write"));
    std::fs::write(&file, "-module(a).\n").expect("fix writes back");
    Ok(ChildOutput {
        code: Some(0),
        stdout: Vec::new(),
        stderr: if broken {
            format!("{file}:2:12: syntax error before: '.'\n").into_bytes()
        } else {
            Vec::new()
        },
    })
}

#[test]
pub(super) fn erlfmt_fix_keeps_a_file_it_reported_an_error_for() {
    let backend = backend_for("erlfmt", plain_tool(), erlfmt_rewrites);
    let fixed = backend
        .apply_fix("erlfmt", "a.erl", "-module( a ).\n", "format")
        .expect("fix round");
    assert_eq!(fixed, "-module(a).\n");
    let broken = "-module( a ).\nbroken(X) -> X +.\n";
    let kept = backend
        .apply_fix("erlfmt", "a.erl", broken, "format")
        .expect("fix round");
    assert_eq!(kept, broken);
}

fn dfmt_behavior(
    argv: &[OsString],
    cwd: &Path,
    env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    assert_hermetic(env);
    assert_eq!(argv[1], OsString::from("--config"));
    assert_eq!(argv[2], OsString::from("dx-dfmt-default"));
    let staged = std::fs::read_to_string(cwd.join("dx-dfmt-default/.editorconfig"))
        .expect("the staged dfmt default");
    assert!(staged.starts_with("root = true\n"), "{staged}");
    let file = last_file(argv);
    let text = std::fs::read_to_string(&file).expect("read staged file");
    let broken = text.contains("broken");
    let warned = text.contains("warned");
    let inplace = argv[3] == OsString::from("--inplace");
    let formatted = text.replace("(){", "()\n{");
    let stderr = if broken {
        format!("{file}(2:5)[error]: Primary expression expected\n").into_bytes()
    } else if warned {
        format!("{file}(1:1)[warn]: Empty declaration\n").into_bytes()
    } else {
        Vec::new()
    };
    if inplace {
        std::fs::write(&file, &formatted).expect("fix writes back");
        return Ok(ChildOutput {
            code: Some(0),
            stdout: Vec::new(),
            stderr,
        });
    }
    Ok(ChildOutput {
        code: Some(0),
        stdout: formatted.into_bytes(),
        stderr,
    })
}

#[test]
pub(super) fn dfmt_checks_one_file_at_a_time_and_never_trusts_a_run_that_printed_an_error() {
    let backend = backend_for("dfmt", plain_tool(), dfmt_behavior);
    let clean = backend
        .diagnose("dfmt", "format", &single("a.d", "void f()\n{}\n"))
        .expect("check");
    assert!(clean.is_empty());
    let dirty = backend
        .diagnose("dfmt", "format", &single("a.d", "void f(){}\n"))
        .expect("check");
    assert_eq!(dirty.len(), 1);
    let broken = backend
        .diagnose("dfmt", "format", &single("a.d", "void f(){\nbroken }\n"))
        .expect("check");
    assert_eq!(broken.len(), 1);
    assert_eq!(broken[0].rule_id, "syntax");
}

fn jsonnet_lint_behavior(
    argv: &[OsString],
    _cwd: &Path,
    env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    assert_hermetic(env);
    let file = last_file(argv);
    let text = std::fs::read_to_string(&file).expect("read staged file");
    let (code, stderr) = if text.contains("crash") {
        (1, "ERROR: boom\n".to_owned())
    } else if text.contains("unused") {
        (
            2,
            format!("{file}:1:7-17 Unused variable: x\n\nlocal x = 1;\n\n\nProblems found!\n"),
        )
    } else {
        (0, String::new())
    };
    Ok(ChildOutput {
        code: Some(code),
        stdout: Vec::new(),
        stderr: stderr.into_bytes(),
    })
}

#[test]
pub(super) fn jsonnet_lint_reports_problems_and_never_trusts_a_failed_run() {
    let backend = backend_for("jsonnet_lint", plain_tool(), jsonnet_lint_behavior);
    let lint = |text: &str| backend.diagnose("jsonnet_lint", "lint", &single("a.jsonnet", text));
    assert!(lint("{}\n").expect("check").is_empty());
    let warned = lint("local x = 1; unused\n").expect("check");
    assert_eq!(warned.len(), 1);
    assert_eq!(warned[0].message, "Unused variable: x");
    assert!(lint("crash\n").is_err());
}

fn fortitude_behavior(
    argv: &[OsString],
    _cwd: &Path,
    env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    assert_hermetic(env);
    let file = last_file(argv);
    let text = std::fs::read_to_string(&file).expect("read staged file");
    let (code, stdout) = if text.contains("crash") {
        (2, "error: boom\n".to_owned())
    } else if text.contains("implicit") {
        (
            1,
            format!(
                r#"[{{"code":"C001","severity":"error","filename":"{file}","location":{{"column":1,"row":1}},"end_location":{{"column":1,"row":2}},"message":"program uses implicit typing"}}]"#
            ),
        )
    } else {
        (0, "[]".to_owned())
    };
    Ok(ChildOutput {
        code: Some(code),
        stdout: stdout.into_bytes(),
        stderr: Vec::new(),
    })
}

#[test]
pub(super) fn fortitude_reports_findings_skips_fixed_form_and_never_trusts_a_failed_run() {
    let backend = backend_for("fortitude", plain_tool(), fortitude_behavior);
    let lint = |name: &str, text: &str| backend.diagnose("fortitude", "lint", &single(name, text));
    assert!(lint("a.f90", "program p\n").expect("check").is_empty());
    let warned = lint("a.f90", "program implicit\n").expect("check");
    assert_eq!(warned.len(), 1);
    assert_eq!(warned[0].rule_id, "C001");
    // Fixed-form sources are not handed to the tool, so even a crash text is not run.
    assert!(lint("a.f", "crash\n").expect("check").is_empty());
    assert!(lint("a.f90", "crash\n").is_err());
}

fn hlint_behavior(
    argv: &[OsString],
    _cwd: &Path,
    env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    assert_hermetic(env);
    assert_eq!(argv[1], OsString::from("--json"));
    let config = argv[2].to_string_lossy().into_owned();
    assert!(
        config.starts_with("--hint=") && config.ends_with("dx-hlint-default/hlint.yaml"),
        "{config}"
    );
    assert_eq!(argv[3], OsString::from("--"));
    let staged = std::fs::read(config.trim_start_matches("--hint=")).expect("staged config");
    assert_eq!(staged, b"[]\n");
    let file = last_file(argv);
    let text = std::fs::read_to_string(&file).expect("read staged file");
    let (code, stdout) = if text.contains("crash") {
        (1, String::new())
    } else if text.contains("redundant") {
        (
            1,
            format!(
                r#"[{{"module":[],"decl":[],"severity":"Warning","hint":"Use id","file":"{file}","startLine":1,"startColumn":5,"endLine":1,"endColumn":12,"from":"x","to":"id","note":[],"refactorings":"[]"}}]"#
            ),
        )
    } else {
        (0, "[]".to_owned())
    };
    Ok(ChildOutput {
        code: Some(code),
        stdout: stdout.into_bytes(),
        stderr: Vec::new(),
    })
}

#[test]
pub(super) fn hlint_reports_findings_with_the_staged_default_and_never_trusts_a_failed_run() {
    let backend = backend_for("hlint", plain_tool(), hlint_behavior);
    let lint = |text: &str| backend.diagnose("hlint", "lint", &single("A.hs", text));
    assert!(lint("main = pure ()\n").expect("check").is_empty());
    let warned = lint("f = redundant\n").expect("check");
    assert_eq!(warned.len(), 1);
    assert_eq!(warned[0].rule_id, "Use id");
    assert!(lint("crash\n").is_err());
}

fn zig_ast_check_behavior(
    argv: &[OsString],
    _cwd: &Path,
    env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    assert_hermetic(env);
    // Exactly one file: `ast-check`, `--color`, `off`, then the file.
    assert_eq!(argv.len(), 5, "one file per process: {argv:?}");
    assert_eq!(argv[1], OsString::from("ast-check"));
    let file = last_file(argv);
    let text = std::fs::read_to_string(&file).expect("read staged file");
    let (code, stderr) = if text.contains("crash") {
        (
            1,
            "error: unable to open file 'x' for ast-check: FileNotFound\n".to_owned(),
        )
    } else if text.contains("unused") {
        (
            1,
            format!("{file}:1:1: error: unused local variable\nunused\n^\n"),
        )
    } else {
        (0, String::new())
    };
    Ok(ChildOutput {
        code: Some(code),
        stdout: Vec::new(),
        stderr: stderr.into_bytes(),
    })
}

#[test]
pub(super) fn zig_ast_check_runs_one_process_per_file_and_never_trusts_a_failed_run() {
    let backend = backend_for("zig_ast_check", plain_tool(), zig_ast_check_behavior);
    let lint = |text: &str| backend.diagnose("zig_ast_check", "lint", &single("a.zig", text));
    assert!(lint("const a = 1;\n").expect("check").is_empty());
    let warned = lint("unused\n").expect("check");
    assert_eq!(warned.len(), 1);
    assert_eq!(warned[0].rule_id, "ast_check");
    assert!(lint("crash\n").is_err());
    let mut two = single("a.zig", "unused\n");
    two.extend(single("b.zig", "unused\n"));
    let both = backend
        .diagnose("zig_ast_check", "lint", &two)
        .expect("check");
    assert_eq!(both.len(), 2);
}

fn dart_behavior(
    argv: &[OsString],
    _cwd: &Path,
    env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    assert_hermetic(env);
    assert_eq!(argv[1], OsString::from("--suppress-analytics"));
    assert_eq!(argv[2], OsString::from("format"));
    let value = |key: &str| {
        env.iter()
            .find(|(known, _)| known == key)
            .map(|(_, value)| value.clone())
    };
    assert_eq!(
        value("DART_SUPPRESS_ANALYTICS"),
        Some(OsString::from("true"))
    );
    assert!(value("HOME").is_none(), "dart format needs no HOME");
    let writing = argv[3] == OsString::from("--output=write");
    let file = last_file(argv);
    let text = std::fs::read_to_string(&file).expect("read staged file");
    let (code, stdout, stderr) = if text.contains("broken") {
        (
            65,
            "Formatted no files in 0.00 seconds.\n".to_owned(),
            format!(
                "Could not format because the source could not be parsed:\n\nline 2, column 1 of {file}: Expected an identifier.\n"
            ),
        )
    } else if text.contains("silent") {
        (
            1,
            "Formatted 1 file (0 changed) in 0.00 seconds.\n".to_owned(),
            String::new(),
        )
    } else if text.contains("{ }") && writing {
        std::fs::write(&file, text.replace("{ }", "{}")).expect("rewrite staged file");
        (
            0,
            "Formatted 1 file (1 changed) in 0.00 seconds.\n".to_owned(),
            String::new(),
        )
    } else if text.contains("{ }") {
        (
            1,
            format!("Changed {file}\nFormatted 1 file (1 changed) in 0.00 seconds.\n"),
            String::new(),
        )
    } else {
        (
            0,
            "Formatted 1 file (0 changed) in 0.00 seconds.\n".to_owned(),
            String::new(),
        )
    };
    Ok(ChildOutput {
        code: Some(code),
        stdout: stdout.into_bytes(),
        stderr: stderr.into_bytes(),
    })
}

#[test]
pub(super) fn dart_formats_without_a_home_and_never_trusts_a_silent_exit() {
    let backend = backend_for("dart", plain_tool(), dart_behavior);
    let check = |text: &str| backend.diagnose("dart", "format", &single("a.dart", text));
    assert!(check("void main() {}\n").expect("check").is_empty());
    let dirty = check("void main() { }\n").expect("check");
    assert_eq!(dirty.len(), 1);
    assert_eq!(dirty[0].message, "file is not formatted");
    let broken = check("broken\n").expect("check");
    assert_eq!(broken.len(), 1);
    assert_eq!(broken[0].rule_id, "syntax");
    assert!(check("silent\n").is_err());
    let fixed = backend
        .apply_fix("dart", "a.dart", "void main() { }\n", "format")
        .expect("fix round");
    assert_eq!(fixed, "void main() {}\n");
    let kept = backend
        .apply_fix("dart", "a.dart", "broken\n", "format")
        .expect("fix round");
    assert_eq!(kept, "broken\n");
}

fn dart_analyze_behavior(
    argv: &[OsString],
    _cwd: &Path,
    env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    assert_hermetic(env);
    assert_eq!(argv[2], OsString::from("analyze"));
    assert_eq!(argv[3], OsString::from("--format=json"));
    assert!(argv[4].to_string_lossy().starts_with("--cache="));
    let options = argv[5].to_string_lossy().into_owned();
    assert!(options.ends_with("analysis_options.yaml"));
    let file = last_file(argv);
    let text = std::fs::read_to_string(&file).expect("read staged file");
    let diagnostic = |path: &str, code: &str, severity: &str, kind: &str| {
        format!(
            r#"{{"code":"{code}","severity":"{severity}","type":"{kind}","location":{{"file":"{path}","range":{{"start":{{"offset":0,"line":1,"column":1}},"end":{{"offset":1,"line":1,"column":2}}}}}},"problemMessage":"msg."}}"#
        )
    };
    let (code, diagnostics) = if text.contains("badoptions") {
        (
            3,
            vec![diagnostic(
                &options,
                "undefined_lint",
                "ERROR",
                "COMPILE_TIME_ERROR",
            )],
        )
    } else if text.contains("mismatch") {
        (
            0,
            vec![diagnostic(&file, "unused_import", "WARNING", "HINT")],
        )
    } else if text.contains("both") {
        (
            3,
            vec![
                diagnostic(&file, "unused_import", "WARNING", "HINT"),
                diagnostic(&file, "invalid_assignment", "ERROR", "COMPILE_TIME_ERROR"),
            ],
        )
    } else {
        (0, Vec::new())
    };
    Ok(ChildOutput {
        code: Some(code),
        stdout: format!(
            r#"{{"version":1,"diagnostics":[{}]}}"#,
            diagnostics.join(",")
        )
        .into_bytes(),
        stderr: Vec::new(),
    })
}

#[test]
pub(super) fn dart_analyze_splits_lint_from_typecheck_and_never_trusts_a_bad_run() {
    let backend = backend_for("dart", plain_tool(), dart_analyze_behavior);
    let run = |capability: &str, text: &str| {
        backend.diagnose("dart", capability, &single("a.dart", text))
    };
    assert!(run("lint", "ok\n").expect("lint").is_empty());
    let lint = run("lint", "both\n").expect("lint");
    let types = run("typecheck", "both\n").expect("typecheck");
    assert_eq!(lint.len(), 1);
    assert_eq!(lint[0].rule_id, "unused_import");
    assert_eq!(types.len(), 1);
    assert_eq!(types[0].rule_id, "invalid_assignment");
    assert!(run("lint", "badoptions\n").is_err());
    assert!(run("typecheck", "mismatch\n").is_err());
    let kept = backend
        .apply_fix("dart", "a.dart", "both\n", "lint")
        .expect("fix round");
    assert_eq!(kept, "both\n");
}

fn dscanner_behavior(
    argv: &[OsString],
    cwd: &Path,
    env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    assert_hermetic(env);
    assert_eq!(argv[2], OsString::from("--config"));
    assert_eq!(argv[3], OsString::from("dx-dscanner-default/dscanner.ini"));
    let staged = std::fs::read_to_string(cwd.join("dx-dscanner-default/dscanner.ini"))
        .expect("the staged dscanner default");
    assert!(
        staged.starts_with("[analysis.config.StaticAnalysisConfig]\n"),
        "{staged}"
    );
    let file = last_file(argv);
    let text = std::fs::read_to_string(&file).expect("read staged file");
    let (code, stdout) = if text.contains("silent") {
        (1, String::new())
    } else if text.contains("phantom") {
        (0, format!("dx\t{file}\t1\t1\t1\t2\twarn\tx\tm\n"))
    } else if text.contains("broken") {
        (
            1,
            format!("dx\t{file}\t2\t5\t2\t5\terror\t\tPrimary expression expected\n"),
        )
    } else if text.contains("unused") {
        (
            1,
            format!(
                "dx\t{file}\t1\t9\t1\t10\twarn\tunused_variable_check\tVariable x is never used.\n"
            ),
        )
    } else {
        (0, String::new())
    };
    Ok(ChildOutput {
        code: Some(code),
        stdout: stdout.into_bytes(),
        stderr: Vec::new(),
    })
}

#[test]
pub(super) fn dscanner_lints_a_batch_and_never_trusts_an_exit_that_disagrees_with_its_output() {
    let backend = backend_for("dscanner", plain_tool(), dscanner_behavior);
    let lint = |text: &str| backend.diagnose("dscanner", "lint", &single("a.d", text));
    assert!(lint("void f()\n{}\n").expect("check").is_empty());
    let warned = lint("void f() { int unused; }\n").expect("check");
    assert_eq!(warned.len(), 1);
    assert_eq!(warned[0].rule_id, "unused_variable_check");
    let broken = lint("void f(){\nbroken }\n").expect("check");
    assert_eq!(broken.len(), 1);
    assert_eq!(broken[0].rule_id, "syntax");
    assert!(lint("silent\n").is_err());
    assert!(lint("phantom\n").is_err());
}

#[test]
pub(super) fn dfmt_fix_keeps_a_file_it_reported_an_error_for() {
    let backend = backend_for("dfmt", plain_tool(), dfmt_behavior);
    let fixed = backend
        .apply_fix("dfmt", "a.d", "void f(){}\n", "format")
        .expect("fix round");
    assert_eq!(fixed, "void f()\n{}\n");
    let broken = "void f(){\nbroken }\n";
    let kept = backend
        .apply_fix("dfmt", "a.d", broken, "format")
        .expect("fix round");
    assert_eq!(kept, broken);
    let fixed = backend
        .apply_fix("dfmt", "a.d", "void f(){} // warned\n", "format")
        .expect("fix round");
    assert_eq!(fixed, "void f()\n{} // warned\n");
}

fn perltidy_behavior(
    argv: &[OsString],
    _cwd: &Path,
    env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    assert_hermetic(env);
    assert_eq!(argv[1], OsString::from("--noprofile"));
    let file = last_file(argv);
    let text = std::fs::read_to_string(&file).expect("read staged file");
    let inplace = argv.contains(&OsString::from("-b"));
    if text.contains("broken") {
        return Ok(ChildOutput {
            code: Some(2),
            stdout: if inplace {
                Vec::new()
            } else {
                text.into_bytes()
            },
            stderr: format!("{file}:1: final indentation level: 1\n").into_bytes(),
        });
    }
    let formatted = text.replace("f{", "f {");
    if inplace {
        std::fs::write(&file, &formatted).expect("fix writes back");
        return Ok(ChildOutput {
            code: Some(0),
            stdout: Vec::new(),
            stderr: Vec::new(),
        });
    }
    Ok(ChildOutput {
        code: Some(0),
        stdout: formatted.into_bytes(),
        stderr: Vec::new(),
    })
}

#[test]
pub(super) fn perltidy_checks_one_file_at_a_time_and_reports_a_syntax_error() {
    let backend = backend_for("perltidy", plain_tool(), perltidy_behavior);
    let clean = backend
        .diagnose("perltidy", "format", &single("a.pm", "sub f {}\n"))
        .expect("check");
    assert!(clean.is_empty());
    let dirty = backend
        .diagnose("perltidy", "format", &single("a.pm", "sub f{}\n"))
        .expect("check");
    assert_eq!(dirty.len(), 1);
    let broken = backend
        .diagnose("perltidy", "format", &single("a.pm", "sub f { broken\n"))
        .expect("check");
    assert_eq!(broken.len(), 1);
    assert_eq!(broken[0].rule_id, "syntax");
}

#[test]
pub(super) fn perltidy_fix_keeps_a_file_it_could_not_parse() {
    let backend = backend_for("perltidy", plain_tool(), perltidy_behavior);
    let fixed = backend
        .apply_fix("perltidy", "a.pm", "sub f{}\n", "format")
        .expect("fix round");
    assert_eq!(fixed, "sub f {}\n");
    let broken = "sub f { broken\n";
    let kept = backend
        .apply_fix("perltidy", "a.pm", broken, "format")
        .expect("fix round");
    assert_eq!(kept, broken);
}

fn perlcritic_behavior(
    argv: &[OsString],
    _cwd: &Path,
    env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    assert_hermetic(env);
    assert_eq!(argv[1], OsString::from("--noprofile"));
    let file = last_file(argv);
    let text = std::fs::read_to_string(&file).expect("read staged file");
    let (code, stdout) = if text.contains("silent") {
        (2, String::new())
    } else if text.contains("phantom") {
        (0, format!("dx\t{file}\t1\t1\t5\tP::Q\tm\n"))
    } else if text.contains("unreadable") {
        (3, String::new())
    } else if text.contains("no strict") {
        (
            2,
            format!(
                "dx\t{file}\t1\t1\t5\tTestingAndDebugging::RequireUseStrict\tCode before strictures are enabled\n"
            ),
        )
    } else {
        (0, String::new())
    };
    Ok(ChildOutput {
        code: Some(code),
        stdout: stdout.into_bytes(),
        stderr: Vec::new(),
    })
}

#[test]
pub(super) fn perlcritic_lints_and_never_trusts_an_exit_that_disagrees_with_its_output() {
    let backend = backend_for("perlcritic", plain_tool(), perlcritic_behavior);
    let lint = |text: &str| backend.diagnose("perlcritic", "lint", &single("a.pl", text));
    assert!(lint("use strict;\n").expect("check").is_empty());
    let found = lint("# no strict\n").expect("check");
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].rule_id, "TestingAndDebugging::RequireUseStrict");
    assert!(lint("silent\n").is_err());
    assert!(lint("phantom\n").is_err());
    assert!(lint("unreadable\n").is_err());
}

fn opa_behavior(
    argv: &[OsString],
    _cwd: &Path,
    env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    assert_hermetic(env);
    let file = last_file(argv);
    let text = std::fs::read_to_string(&file).expect("read staged file");
    let done = |code, stdout: &str, stderr: String| {
        Ok(ChildOutput {
            code: Some(code),
            stdout: stdout.as_bytes().to_vec(),
            stderr: stderr.into_bytes(),
        })
    };
    if argv[1] == OsString::from("check") {
        if text.contains("noise") {
            return done(0, "", "warning: noise\n".to_owned());
        }
        if text.contains("missing") {
            let report = format!(
                "{{\"errors\":[{{\"message\":\"undefined function data.lib.missing\",\"code\":\"rego_type_error\",\"location\":{{\"file\":\"{file}\",\"row\":3,\"col\":6}}}}]}}"
            );
            return done(1, "", report);
        }
        return done(0, "", String::new());
    }
    if text.contains(":= :=") {
        return done(
            2,
            "",
            format!("failed to format Rego source file: 1 error occurred:\n{file}:2: rego_parse_error: unexpected assign token\n"),
        );
    }
    let formatted = "package a\n\nx := 1\n";
    if argv[2] == OsString::from("-w") {
        std::fs::write(&file, formatted).expect("fix writes back");
        return done(0, "", String::new());
    }
    done(0, formatted, String::new())
}

#[test]
pub(super) fn opa_format_reports_unformatted_files_and_parse_errors() {
    let backend = backend_for("opa", plain_tool(), opa_behavior);
    let clean = single("a.rego", "package a\n\nx := 1\n");
    assert!(backend
        .diagnose("opa", "format", &clean)
        .expect("clean")
        .is_empty());
    let findings = backend
        .diagnose(
            "opa",
            "format",
            &single("a.rego", "package a\n\nx   := 1\n"),
        )
        .expect("unformatted");
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].message, "file is not formatted");
    let broken = single("a.rego", "package a\nx := := 1\n");
    let findings = backend
        .diagnose("opa", "format", &broken)
        .expect("parse error");
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].rule_id, "rego_parse_error");
}

#[test]
pub(super) fn opa_fix_rewrites_an_owned_file() {
    let backend = backend_for("opa", plain_tool(), opa_behavior);
    let fixed = backend
        .apply_fix("opa", "a.rego", "package a\n\nx   := 1\n", "format")
        .expect("fix round");
    assert_eq!(fixed, "package a\n\nx := 1\n");
}

#[test]
pub(super) fn opa_typecheck_reports_errors_and_never_counts_noise_as_clean() {
    let backend = backend_for("opa", plain_tool(), opa_behavior);
    let clean = single("a.rego", "package a\n\nx := 1\n");
    assert!(backend
        .diagnose("opa", "typecheck", &clean)
        .expect("clean")
        .is_empty());
    let findings = backend
        .diagnose(
            "opa",
            "typecheck",
            &single("a.rego", "package a\n\nx := data.lib.missing(1)\n"),
        )
        .expect("type error");
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].rule_id, "rego_type_error");
    assert_eq!(findings[0].path, "a.rego");
    assert!(backend
        .diagnose("opa", "typecheck", &single("a.rego", "noise\n"))
        .is_err());
}

fn regal_behavior(
    argv: &[OsString],
    cwd: &Path,
    env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    assert_hermetic(env);
    assert!(cwd.join(".regal").is_dir(), "regal config dir is staged");
    let config = argv
        .iter()
        .position(|arg| arg == "--config-file")
        .map(|index| PathBuf::from(&argv[index + 1]))
        .expect("regal is always given a config file");
    assert!(config.is_file(), "the config file exists");
    let file = last_file(argv);
    assert!(
        Path::new(&file).is_relative(),
        "paths are workspace-relative"
    );
    let text = std::fs::read_to_string(cwd.join(&file)).expect("read staged file");
    let report = |level: &str, count: u64| {
        format!(
            "{{\"violations\":[{{\"title\":\"use-assignment-operator\",\"description\":\"Prefer :=\",\"category\":\"style\",\"level\":\"{level}\",\"location\":{{\"file\":\"{file}\",\"row\":3,\"col\":3}}}}],\"summary\":{{\"files_scanned\":1,\"files_failed\":1,\"rules_skipped\":0,\"num_violations\":{count}}}}}"
        )
    };
    let done = |code: i32, stdout: String, stderr: &str| {
        Ok(ChildOutput {
            code: Some(code),
            stdout: stdout.into_bytes(),
            stderr: stderr.as_bytes().to_vec(),
        })
    };
    if text.contains("errors") {
        return done(3, report("error", 1), "");
    }
    if text.contains("warnings") {
        return done(2, report("warning", 1), "");
    }
    if text.contains("exit0") {
        return done(0, report("error", 1), "");
    }
    if text.contains("garbage") {
        return done(0, "not json".to_owned(), "");
    }
    if text.contains("crash") {
        return done(1, "{\"errors\":[\"parse failure\"]}".to_owned(), "boom\n");
    }
    done(
        0,
        "{\"violations\":[],\"summary\":{\"files_scanned\":1,\"files_failed\":0,\"rules_skipped\":0,\"num_violations\":0}}"
            .to_owned(),
        "",
    )
}

#[test]
pub(super) fn regal_lint_reports_errors_and_warnings_and_never_counts_noise_as_clean() {
    let mut tool = plain_tool();
    tool.extra_env = vec![("REGAL_DISABLE_VERSION_CHECK".to_owned(), "1".to_owned())];
    let backend = backend_for("regal", tool, regal_behavior);
    assert!(backend
        .diagnose("regal", "lint", &single("p/a.rego", "package p\n"))
        .expect("clean")
        .is_empty());
    let findings = backend
        .diagnose(
            "regal",
            "lint",
            &single("p/a.rego", "package p\n\nerrors = 1\n"),
        )
        .expect("errors");
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].rule_id, "style/use-assignment-operator");
    assert_eq!(findings[0].path, "p/a.rego");
    let findings = backend
        .diagnose(
            "regal",
            "lint",
            &single("p/a.rego", "package p\n\nwarnings = 1\n"),
        )
        .expect("warnings are findings");
    assert_eq!(findings.len(), 1);
    for body in ["package p\n\nexit0 = 1\n", "garbage\n", "crash\n"] {
        assert!(
            backend
                .diagnose("regal", "lint", &single("p/a.rego", body))
                .is_err(),
            "{body}"
        );
    }
}

#[test]
pub(super) fn regal_lint_with_a_missing_binary_is_an_error() {
    let backend = backend_for("regal", plain_tool(), |_, _, _| {
        Err(io::Error::new(io::ErrorKind::NotFound, "no regal"))
    });
    assert!(backend
        .diagnose("regal", "lint", &single("a.rego", "package a\n"))
        .is_err());
}

fn opa_dependency_behavior(
    argv: &[OsString],
    _cwd: &Path,
    env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    assert_hermetic(env);
    let mut errors = Vec::new();
    for arg in &argv[2..] {
        let file = arg.to_string_lossy().into_owned();
        if !Path::new(&file).is_file() {
            continue;
        }
        let text = std::fs::read_to_string(&file).expect("read staged file");
        if text.contains("broken") {
            errors.push(format!(
                "{{\"message\":\"unexpected token\",\"code\":\"rego_parse_error\",\"location\":{{\"file\":\"{file}\",\"row\":2,\"col\":1}}}}"
            ));
        }
    }
    let (code, stderr) = if errors.is_empty() {
        (0, String::new())
    } else {
        (1, format!("{{\"errors\":[{}]}}", errors.join(",")))
    };
    Ok(ChildOutput {
        code: Some(code),
        stdout: Vec::new(),
        stderr: stderr.into_bytes(),
    })
}

#[test]
pub(super) fn opa_typecheck_never_reports_clean_when_only_a_dependency_is_broken() {
    let backend = backend_for("opa", plain_tool(), opa_dependency_behavior);
    let owned = single("app/app.rego", "package app\n\nx := 1\n");
    let clean = single("lib/base.rego", "package lib\n\ny := 1\n");
    let found = backend
        .diagnose_with_resolve("opa", "typecheck", &owned, &BTreeMap::new(), &clean)
        .expect("clean dependency");
    assert!(found.is_empty());
    let broken = single("lib/base.rego", "package lib\nbroken\n");
    let error = backend
        .diagnose_with_resolve("opa", "typecheck", &owned, &BTreeMap::new(), &broken)
        .expect_err("broken dependency is never clean");
    assert!(error.to_string().contains("lib/base.rego"), "{error}");
    let both = single("app/app.rego", "package app\nbroken\n");
    let found = backend
        .diagnose_with_resolve("opa", "typecheck", &both, &BTreeMap::new(), &broken)
        .expect("owned error");
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].path, "app/app.rego");
}

fn mix_format_behavior(
    argv: &[OsString],
    cwd: &Path,
    env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    assert_hermetic(env);
    let file = last_file(argv);
    let text = std::fs::read_to_string(&file).expect("read staged file");
    let relative = Path::new(&file)
        .strip_prefix(cwd)
        .expect("staged under cwd")
        .to_string_lossy()
        .into_owned();
    let check = argv[1] == OsString::from("--check-formatted");
    if text.contains('*') {
        return Ok(ChildOutput {
            code: Some(1),
            stdout: Vec::new(),
            stderr: format!(
                "mix format failed for file: {relative}\n** (SyntaxError) invalid syntax found on {relative}:2:3:\n    error: syntax error before: '*'\n"
            )
            .into_bytes(),
        });
    }
    if check {
        return Ok(ChildOutput {
            code: Some(1),
            stdout: Vec::new(),
            stderr: format!(
                "** (Mix) mix format failed due to --check-formatted.\nThe following files are not formatted:\n\n\u{1b}[1m\u{1b}[31m{file}\n\u{1b}[0m\n1   -|x=1\n  1 +|x = 1\n"
            )
            .into_bytes(),
        });
    }
    assert_eq!(argv.len(), 2);
    std::fs::write(&file, "x = 1\n").expect("fix writes back");
    Ok(ChildOutput {
        code: Some(0),
        stdout: Vec::new(),
        stderr: Vec::new(),
    })
}

#[test]
pub(super) fn mix_format_reports_files_relative_to_its_working_directory() {
    let backend = backend_for("mix_format", plain_tool(), mix_format_behavior);
    let findings = backend
        .diagnose("mix_format", "format", &single("lib/a.ex", "x=1\n"))
        .expect("unformatted");
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].path, "lib/a.ex");
    assert_eq!(findings[0].message, "file is not formatted");
    let findings = backend
        .diagnose("mix_format", "format", &single("lib/a.ex", "x = 1\ny *\n"))
        .expect("syntax error");
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].path, "lib/a.ex");
    assert_eq!(findings[0].rule_id, "syntax");
    assert_eq!(findings[0].message, "syntax error before: '*'");
    assert_eq!(findings[0].start_byte, Some(8));
}

#[test]
pub(super) fn mix_format_fix_keeps_a_file_it_could_not_parse() {
    let backend = backend_for("mix_format", plain_tool(), mix_format_behavior);
    let fixed = backend
        .apply_fix("mix_format", "a.ex", "x=1\n", "format")
        .expect("fix round");
    assert_eq!(fixed, "x = 1\n");
    let broken = "x = 1\ny *\n";
    let kept = backend
        .apply_fix("mix_format", "a.ex", broken, "format")
        .expect("fix round");
    assert_eq!(kept, broken);
}

const ROSLYN_CLEAN: &str =
    r#"{"version":"2.1.0","runs":[{"tool":{"driver":{"name":"csc"}},"results":[]}]}"#;

#[test]
fn delegated_roslyn_fails_closed_without_the_csc_exit_status() {
    for exit in [None, Some("oops")] {
        let report = upstream_file("roslyn", ROSLYN_CLEAN);
        let mut tool = delegated_tool(report.path().to_path_buf());
        tool.tool_files = exit
            .map(|text| vec![(roslyn_exit_key(report.path()), text.as_bytes().to_vec())])
            .unwrap_or_default();
        let backend = backend_for("roslyn", tool, must_not_spawn);
        let error = backend
            .diagnose("roslyn", "lint", &single("src/a.cs", "x\n"))
            .expect_err("missing exit status");
        assert!(error.to_string().contains("csc exit status"), "{error}");
    }
}

#[test]
fn delegated_roslyn_reports_a_nonzero_exit_without_errors() {
    let report = upstream_file("roslyn", ROSLYN_CLEAN);
    let mut tool = delegated_tool(report.path().to_path_buf());
    tool.tool_files = vec![(roslyn_exit_key(report.path()), b"1\r\n".to_vec())];
    let backend = backend_for("roslyn", tool, must_not_spawn);
    let error = backend
        .diagnose("roslyn", "lint", &single("src/a.cs", "x\n"))
        .expect_err("incomplete");
    assert!(error.to_string().contains("csc exit 1"), "{error}");
}

fn roslyn_result(rule: &str, line: u32, message: &str) -> String {
    format!(
        r#"{{"ruleId":"{rule}","level":"warning","message":{{"text":"{message}"}},"locations":[{{"physicalLocation":{{"artifactLocation":{{"uri":"src/a.cs"}},"region":{{"startLine":{line},"startColumn":1,"endLine":{line},"endColumn":2}}}}}}]}}"#
    )
}

fn roslyn_sarif(results: &[String]) -> String {
    format!(
        r#"{{"version":"2.1.0","runs":[{{"tool":{{"driver":{{"name":"csc"}}}},"results":[{}]}}]}}"#,
        results.join(",")
    )
}

/// One Roslyn tool fed a SARIF log and exit status per target framework.
fn roslyn_pivots(scratch: &Path, pivots: &[(&str, &str, Option<&str>)]) -> RealTool {
    let mut tool = delegated_tool(PathBuf::new());
    tool.upstream_diagnostics.clear();
    tool.tool_files.clear();
    for (tfm, sarif, exit) in pivots {
        let path = scratch.join(format!("{tfm}.sarif"));
        std::fs::write(&path, sarif).expect("write pivot SARIF");
        if let Some(exit) = exit {
            tool.tool_files
                .push((format!("dx-roslyn/{tfm}/exit"), exit.as_bytes().to_vec()));
        }
        tool.upstream_diagnostics.push(path);
    }
    tool
}

const ROSLYN_SOURCE: &str = "a\nb\nc\n";

#[test]
fn delegated_roslyn_reports_a_finding_shared_by_every_framework_once() {
    let scratch = tempfile::tempdir().expect("scratch");
    let shared = roslyn_sarif(&[roslyn_result("CA1822", 1, "shared")]);
    let tool = roslyn_pivots(
        scratch.path(),
        &[
            ("net8.0", &shared, Some("0")),
            ("net10.0", &shared, Some("0")),
        ],
    );
    let findings = backend_for("roslyn", tool, must_not_spawn)
        .diagnose("roslyn", "lint", &single("src/a.cs", ROSLYN_SOURCE))
        .expect("merged findings");
    assert_eq!(findings.len(), 1, "{findings:?}");
    assert_eq!(findings[0].message, "shared");
}

#[test]
fn delegated_roslyn_marks_a_finding_with_the_frameworks_that_report_it() {
    let scratch = tempfile::tempdir().expect("scratch");
    let net8 = roslyn_sarif(&[
        roslyn_result("CA1822", 1, "shared"),
        roslyn_result("CA1822", 2, "only net8"),
    ]);
    let net10 = roslyn_sarif(&[roslyn_result("CA1822", 1, "shared")]);
    let tool = roslyn_pivots(
        scratch.path(),
        &[("net8.0", &net8, Some("0")), ("net10.0", &net10, Some("0"))],
    );
    let findings = backend_for("roslyn", tool, must_not_spawn)
        .diagnose("roslyn", "lint", &single("src/a.cs", ROSLYN_SOURCE))
        .expect("merged findings");
    let mut messages: Vec<&str> = findings
        .iter()
        .map(|found| found.message.as_str())
        .collect();
    messages.sort_unstable();
    assert_eq!(messages, ["only net8 [net8.0]", "shared"]);
}

#[test]
fn delegated_roslyn_fails_closed_without_a_framework_exit_status() {
    let scratch = tempfile::tempdir().expect("scratch");
    let tool = roslyn_pivots(
        scratch.path(),
        &[
            ("net8.0", ROSLYN_CLEAN, Some("0")),
            ("net10.0", ROSLYN_CLEAN, None),
        ],
    );
    let error = backend_for("roslyn", tool, must_not_spawn)
        .diagnose("roslyn", "lint", &single("src/a.cs", ROSLYN_SOURCE))
        .expect_err("missing net10.0 exit status");
    assert!(
        error.to_string().contains("exit status for net10.0"),
        "{error}"
    );
}

#[test]
fn delegated_roslyn_clean_framework_never_hides_a_failed_one() {
    let scratch = tempfile::tempdir().expect("scratch");
    let tool = roslyn_pivots(
        scratch.path(),
        &[
            ("net8.0", ROSLYN_CLEAN, Some("1")),
            ("net10.0", ROSLYN_CLEAN, Some("0")),
        ],
    );
    let error = backend_for("roslyn", tool, must_not_spawn)
        .diagnose("roslyn", "lint", &single("src/a.cs", ROSLYN_SOURCE))
        .expect_err("net8.0 csc failed");
    assert!(error.to_string().contains("csc exit 1"), "{error}");
}

#[test]
fn delegated_roslyn_fails_closed_on_one_malformed_framework_log() {
    let scratch = tempfile::tempdir().expect("scratch");
    let tool = roslyn_pivots(
        scratch.path(),
        &[
            ("net8.0", "{<error", Some("0")),
            ("net10.0", ROSLYN_CLEAN, Some("0")),
        ],
    );
    assert!(backend_for("roslyn", tool, must_not_spawn)
        .diagnose("roslyn", "lint", &single("src/a.cs", ROSLYN_SOURCE))
        .is_err());
}

#[test]
fn delegated_csc_keeps_compiler_diagnostics_and_drops_analyzers() {
    let scratch = tempfile::tempdir().expect("scratch");
    let sarif = roslyn_sarif(&[
        roslyn_result("CS0029", 1, "cannot convert"),
        roslyn_result("CA1822", 2, "static"),
    ]);
    let tool = roslyn_pivots(scratch.path(), &[("net8.0", &sarif, Some("0"))]);
    let findings = backend_for("csc", tool, must_not_spawn)
        .diagnose("csc", "typecheck", &single("src/a.cs", ROSLYN_SOURCE))
        .expect("compiler findings");
    assert_eq!(findings.len(), 1, "{findings:?}");
    assert_eq!(findings[0].message, "cannot convert");
}

#[test]
fn delegated_csc_requires_the_exit_status() {
    let scratch = tempfile::tempdir().expect("scratch");
    let sarif = roslyn_sarif(&[]);
    let tool = roslyn_pivots(scratch.path(), &[("net8.0", &sarif, None)]);
    let error = backend_for("csc", tool, must_not_spawn)
        .diagnose("csc", "typecheck", &single("src/a.cs", ROSLYN_SOURCE))
        .expect_err("missing exit");
    assert!(error.to_string().contains("csc"), "{error}");
}

/// One fsc tool fed an fsc log and exit status per target framework.
fn fsc_pivots(scratch: &Path, pivots: &[(&str, &str, Option<&str>)]) -> RealTool {
    let mut tool = delegated_tool(PathBuf::new());
    tool.upstream_diagnostics.clear();
    tool.tool_files.clear();
    for (tfm, log, exit) in pivots {
        let path = scratch.join(format!("{tfm}.log"));
        std::fs::write(&path, log).expect("write fsc log");
        if let Some(exit) = exit {
            tool.tool_files
                .push((format!("dx-fsc/{tfm}/exit"), exit.as_bytes().to_vec()));
        }
        tool.upstream_diagnostics.push(path);
    }
    tool
}

const FSC_ERROR: &str = "/work/root/src/a.fs(2,1): error FS0001: boom\n";

#[test]
fn delegated_fsc_reports_errors_reanchored_to_the_staged_source() {
    let scratch = tempfile::tempdir().expect("scratch");
    let tool = fsc_pivots(scratch.path(), &[("net10.0", FSC_ERROR, Some("1"))]);
    let findings = backend_for("fsc", tool, must_not_spawn)
        .diagnose("fsc", "typecheck", &single("src/a.fs", ROSLYN_SOURCE))
        .expect("compiler findings");
    assert_eq!(findings.len(), 1, "{findings:?}");
    assert_eq!(findings[0].rule_id, "FS0001");
    assert!(findings[0].path.ends_with("src/a.fs"), "{:?}", findings[0]);
}

#[test]
fn delegated_fsc_requires_the_exit_status() {
    let scratch = tempfile::tempdir().expect("scratch");
    let tool = fsc_pivots(scratch.path(), &[("net10.0", "", None)]);
    let error = backend_for("fsc", tool, must_not_spawn)
        .diagnose("fsc", "typecheck", &single("src/a.fs", ROSLYN_SOURCE))
        .expect_err("missing exit");
    assert!(error.to_string().contains("fsc exit status"), "{error}");
}

#[test]
fn delegated_fsc_merges_frameworks_and_fails_on_a_silent_failure() {
    let scratch = tempfile::tempdir().expect("scratch");
    let tool = fsc_pivots(
        scratch.path(),
        &[
            ("net8.0", FSC_ERROR, Some("1")),
            ("net10.0", FSC_ERROR, Some("1")),
        ],
    );
    let findings = backend_for("fsc", tool, must_not_spawn)
        .diagnose("fsc", "typecheck", &single("src/a.fs", ROSLYN_SOURCE))
        .expect("merged");
    assert_eq!(findings.len(), 1, "{findings:?}");
    let tool = fsc_pivots(
        scratch.path(),
        &[("net8.0", FSC_ERROR, Some("1")), ("net10.0", "", Some("1"))],
    );
    assert!(backend_for("fsc", tool, must_not_spawn)
        .diagnose("fsc", "typecheck", &single("src/a.fs", ROSLYN_SOURCE))
        .is_err());
}

fn clang_syntax_tool(log: &Path, exit: Option<&str>) -> RealTool {
    let stem = log.file_stem().and_then(|s| s.to_str()).expect("stem");
    let mut tool = delegated_tool(log.to_path_buf());
    tool.tool_files = exit
        .map(|text| {
            vec![(
                format!("dx-clang-syntax/{stem}/exit"),
                text.as_bytes().to_vec(),
            )]
        })
        .unwrap_or_default();
    tool
}

fn nvcc_syntax_tool(log: &Path, exit: Option<&str>) -> RealTool {
    let stem = log.file_stem().and_then(|s| s.to_str()).expect("stem");
    let mut tool = delegated_tool(log.to_path_buf());
    tool.tool_files = exit
        .map(|text| {
            vec![(
                format!("dx-nvcc-syntax/{stem}/exit"),
                text.as_bytes().to_vec(),
            )]
        })
        .unwrap_or_default();
    tool
}

fn compile_check_tool(tool_id: &str, log: &Path, exit: Option<&str>) -> RealTool {
    let stem = log.file_stem().and_then(|s| s.to_str()).expect("stem");
    let mut tool = delegated_tool(log.to_path_buf());
    tool.tool_files = exit
        .map(|text| {
            vec![(
                format!("dx-compile-check/{stem}/exit"),
                text.as_bytes().to_vec(),
            )]
        })
        .unwrap_or_default();
    let _ = tool_id;
    tool
}

const COMPILE_CHECK_SOURCE: &str = "abcdefghij\nabcdefghij\nabcdefghij\nabcdefghij\n";

const COMPILE_CHECK_CASES: [(&str, &str, &str, &str); 2] = [
    (
        "flang",
        "src/a.f90",
        "src/a.f90:3:7: error: No explicit type declared for 'y'\nerror: Semantic errors in src/a.f90\n",
        "src/gen.f90:3:7: error: No explicit type declared for 'y'\n",
    ),
    (
        "ldc",
        "src/a.d",
        "src/a.d:4:9: Error: undefined identifier `y`\n",
        "src/gen.d:4:9: Error: undefined identifier `y`\n",
    ),
];

#[test]
fn delegated_compile_check_reads_clean_and_owned_errors() {
    for (tool_id, path, owned, _) in COMPILE_CHECK_CASES {
        let clean = upstream_file(tool_id, "");
        let found = backend_for(
            tool_id,
            compile_check_tool(tool_id, clean.path(), Some("0\n")),
            must_not_spawn,
        )
        .diagnose(tool_id, "typecheck", &single(path, COMPILE_CHECK_SOURCE))
        .expect("clean");
        assert!(found.is_empty(), "{tool_id}");

        let dirty = upstream_file(tool_id, owned);
        let found = backend_for(
            tool_id,
            compile_check_tool(tool_id, dirty.path(), Some("1\n")),
            must_not_spawn,
        )
        .diagnose(tool_id, "typecheck", &single(path, COMPILE_CHECK_SOURCE))
        .expect("dirty");
        assert_eq!(found.len(), 1, "{tool_id}");
        assert_eq!(found[0].path, path);
    }
}

#[test]
fn delegated_compile_check_fails_closed() {
    for (tool_id, path, _, foreign) in COMPILE_CHECK_CASES {
        let empty = upstream_file(tool_id, "");
        let foreign_log = upstream_file(tool_id, foreign);
        for (tool, needle) in [
            (
                compile_check_tool(tool_id, empty.path(), None),
                "exit status",
            ),
            (
                compile_check_tool(tool_id, empty.path(), Some("1")),
                "exit 1",
            ),
            (
                compile_check_tool(tool_id, foreign_log.path(), Some("1")),
                tool_id,
            ),
        ] {
            let error = backend_for(tool_id, tool, must_not_spawn)
                .diagnose(tool_id, "typecheck", &single(path, COMPILE_CHECK_SOURCE))
                .expect_err("fail closed");
            assert!(error.to_string().contains(needle), "{tool_id}: {error}");
        }
        let error = backend_for(tool_id, plain_tool(), must_not_spawn)
            .diagnose(tool_id, "typecheck", &single(path, COMPILE_CHECK_SOURCE))
            .expect_err("no upstream");
        assert!(error.to_string().contains("requires upstream diagnostics"));
    }
}

const TSC_SOURCE: &str = "export const n: number = 'x';\n";
const TSC_OWNED: &str =
    "src/a.ts(1,14): error TS2322: Type 'string' is not assignable to type 'number'.\n";

fn tsc_tool(report: &Path, exit: Option<&str>) -> RealTool {
    let mut tool = delegated_tool(report.to_path_buf());
    tool.tool_files = exit
        .map(|text| vec![("dx-tsc/exit".to_owned(), text.as_bytes().to_vec())])
        .unwrap_or_default();
    tool
}

#[test]
fn delegated_clang_syntax_reads_clean_and_dirty_logs() {
    let clean = upstream_file("clang_syntax", "");
    let backend = backend_for(
        "clang_syntax",
        clang_syntax_tool(clean.path(), Some("0\n")),
        must_not_spawn,
    );
    let found = backend
        .diagnose("clang_syntax", "typecheck", &single("src/a.c", "x\n"))
        .expect("clean");
    assert!(found.is_empty());

    let dirty = upstream_file(
        "clang_syntax",
        "src/a.c:1:1: error: unknown type name 'x'\n",
    );
    let backend = backend_for(
        "clang_syntax",
        clang_syntax_tool(dirty.path(), Some("1\n")),
        must_not_spawn,
    );
    let found = backend
        .diagnose("clang_syntax", "typecheck", &single("src/a.c", "x\n"))
        .expect("dirty");
    assert_eq!(found.len(), 1);
}

#[test]
fn delegated_clang_syntax_fails_closed() {
    let log = upstream_file("clang_syntax", "");
    for (tool, needle) in [
        (clang_syntax_tool(log.path(), None), "exit status"),
        (clang_syntax_tool(log.path(), Some("1")), "exit 1"),
    ] {
        let error = backend_for("clang_syntax", tool, must_not_spawn)
            .diagnose("clang_syntax", "typecheck", &single("src/a.c", "x\n"))
            .expect_err("fail closed");
        assert!(error.to_string().contains(needle), "{error}");
    }
    let error = backend_for("clang_syntax", plain_tool(), must_not_spawn)
        .diagnose("clang_syntax", "typecheck", &single("src/a.c", "x\n"))
        .expect_err("no upstream");
    assert!(error.to_string().contains("requires upstream diagnostics"));
}

#[test]
fn delegated_nvcc_syntax_reads_clean_and_dirty_logs() {
    let clean = upstream_file("nvcc_syntax", "");
    let backend = backend_for(
        "nvcc_syntax",
        nvcc_syntax_tool(clean.path(), Some("0\n")),
        must_not_spawn,
    );
    let found = backend
        .diagnose("nvcc_syntax", "typecheck", &single("src/a.cu", "x\n"))
        .expect("clean");
    assert!(found.is_empty());

    let dirty = upstream_file(
        "nvcc_syntax",
        "src/a.cu(1): error: identifier \"x\" is undefined\n",
    );
    let backend = backend_for(
        "nvcc_syntax",
        nvcc_syntax_tool(dirty.path(), Some("1\n")),
        must_not_spawn,
    );
    let found = backend
        .diagnose("nvcc_syntax", "typecheck", &single("src/a.cu", "x\n"))
        .expect("dirty");
    assert_eq!(found.len(), 1);
}

#[test]
fn delegated_nvcc_syntax_fails_closed() {
    let log = upstream_file("nvcc_syntax", "");
    for (tool, needle) in [
        (nvcc_syntax_tool(log.path(), None), "exit status"),
        (nvcc_syntax_tool(log.path(), Some("1")), "exit 1"),
    ] {
        let error = backend_for("nvcc_syntax", tool, must_not_spawn)
            .diagnose("nvcc_syntax", "typecheck", &single("src/a.cu", "x\n"))
            .expect_err("fail closed");
        assert!(error.to_string().contains(needle), "{error}");
    }
    let error = backend_for("nvcc_syntax", plain_tool(), must_not_spawn)
        .diagnose("nvcc_syntax", "typecheck", &single("src/a.cu", "x\n"))
        .expect_err("no upstream");
    assert!(error.to_string().contains("requires upstream diagnostics"));
}

#[test]
fn delegated_tsc_is_clean_on_exit_zero_and_reports_owned_findings() {
    let clean = upstream_file("tsc", "");
    let findings = backend_for("tsc", tsc_tool(clean.path(), Some("0")), must_not_spawn)
        .diagnose("tsc", "typecheck", &single("src/a.ts", TSC_SOURCE))
        .expect("clean");
    assert!(findings.is_empty());
    let dirty = upstream_file("tsc", TSC_OWNED);
    let findings = backend_for("tsc", tsc_tool(dirty.path(), Some("2\n")), must_not_spawn)
        .diagnose("tsc", "typecheck", &single("src/a.ts", TSC_SOURCE))
        .expect("findings");
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].path, "src/a.ts");
    assert_eq!(findings[0].rule_id, "TS2322");
}

#[test]
fn delegated_tsc_never_reports_a_failing_project_clean() {
    let dependency_only = upstream_file(
        "tsc",
        "lib/dep.ts(3,1): error TS2322: Type 'string' is not assignable to type 'number'.\n",
    );
    let global = upstream_file(
        "tsc",
        "error TS18003: No inputs were found in config file.\n",
    );
    let silent = upstream_file("tsc", "");
    for report in [&dependency_only, &global, &silent] {
        let result = backend_for("tsc", tsc_tool(report.path(), Some("2")), must_not_spawn)
            .diagnose("tsc", "typecheck", &single("src/a.ts", TSC_SOURCE));
        assert!(result.is_err(), "{result:?}");
    }
}

#[test]
fn delegated_tsc_fails_closed_without_its_inputs() {
    for exit in [None, Some("oops")] {
        let report = upstream_file("tsc", "");
        let error = backend_for("tsc", tsc_tool(report.path(), exit), must_not_spawn)
            .diagnose("tsc", "typecheck", &single("src/a.ts", TSC_SOURCE))
            .expect_err("missing exit status");
        assert!(error.to_string().contains("exit status"), "{error}");
    }
    let mut tool = tsc_tool(Path::new(""), Some("0"));
    tool.upstream_diagnostics.clear();
    let error = backend_for("tsc", tool, must_not_spawn)
        .diagnose("tsc", "typecheck", &single("src/a.ts", TSC_SOURCE))
        .expect_err("missing diagnostics");
    assert!(error.to_string().contains("diagnostics"), "{error}");
}

const JAVAC_SOURCE: &str = "class A {\n  int f() { return \"s\"; }\n}\n";

fn javac_log(exit: i32, file: &str) -> String {
    let diagnostics = if exit == 0 {
        String::new()
    } else {
        format!(
            r#"{{"file":"{file}","line":2,"column":22,"severity":"error","code":"compiler.err.prob.found.req","message":"incompatible types"}}"#
        )
    };
    format!(r#"{{"version":1,"tool":"javac","exit":{exit},"diagnostics":[{diagnostics}]}}"#)
}

#[test]
fn delegated_javac_is_clean_and_reports_owned_errors() {
    let clean = upstream_file("javac", &javac_log(0, ""));
    let findings = backend_for(
        "javac",
        delegated_tool(clean.path().to_path_buf()),
        must_not_spawn,
    )
    .diagnose("javac", "typecheck", &single("j/A.java", JAVAC_SOURCE))
    .expect("clean");
    assert!(findings.is_empty());
    let dirty = upstream_file("javac", &javac_log(1, "j/A.java"));
    let findings = backend_for(
        "javac",
        delegated_tool(dirty.path().to_path_buf()),
        must_not_spawn,
    )
    .diagnose("javac", "typecheck", &single("j/A.java", JAVAC_SOURCE))
    .expect("findings");
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].path, "j/A.java");
    assert_eq!(findings[0].rule_id, "compiler.err.prob.found.req");
}

#[test]
fn delegated_javac_never_reports_a_failing_target_clean() {
    let generated = upstream_file("javac", &javac_log(1, "bazel-out/k8/bin/j/Gen.java"));
    let result = backend_for(
        "javac",
        delegated_tool(generated.path().to_path_buf()),
        must_not_spawn,
    )
    .diagnose("javac", "typecheck", &single("j/A.java", JAVAC_SOURCE));
    assert!(result.is_err(), "{result:?}");
}

#[test]
fn delegated_javac_fails_closed_without_its_log() {
    let mut tool = delegated_tool(PathBuf::new());
    tool.upstream_diagnostics.clear();
    let error = backend_for("javac", tool, must_not_spawn)
        .diagnose("javac", "typecheck", &single("j/A.java", JAVAC_SOURCE))
        .expect_err("missing log");
    assert!(error.to_string().contains("check log"), "{error}");
}

const KOTLINC_SOURCE: &str = "fun f(): Int {\n  return \"s\"\n}\n";

fn kotlinc_log(exit: i32, file: &str) -> String {
    let diagnostics = if exit == 0 {
        String::new()
    } else {
        format!(
            r#"{{"file":"{file}","line":2,"column":10,"severity":"error","code":"RETURN_TYPE_MISMATCH","message":"type mismatch"}}"#
        )
    };
    format!(r#"{{"version":1,"tool":"kotlinc","exit":{exit},"diagnostics":[{diagnostics}]}}"#)
}

#[test]
fn delegated_kotlinc_is_clean_and_reports_owned_errors() {
    let clean = upstream_file("kotlinc", &kotlinc_log(0, ""));
    let findings = backend_for(
        "kotlinc",
        delegated_tool(clean.path().to_path_buf()),
        must_not_spawn,
    )
    .diagnose("kotlinc", "typecheck", &single("k/A.kt", KOTLINC_SOURCE))
    .expect("clean");
    assert!(findings.is_empty());
    let dirty = upstream_file("kotlinc", &kotlinc_log(1, "k/A.kt"));
    let findings = backend_for(
        "kotlinc",
        delegated_tool(dirty.path().to_path_buf()),
        must_not_spawn,
    )
    .diagnose("kotlinc", "typecheck", &single("k/A.kt", KOTLINC_SOURCE))
    .expect("findings");
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].path, "k/A.kt");
    assert_eq!(findings[0].rule_id, "RETURN_TYPE_MISMATCH");
}

#[test]
fn delegated_kotlinc_never_reports_a_failing_target_clean() {
    let generated = upstream_file("kotlinc", &kotlinc_log(1, "bazel-out/k8/bin/k/Gen.kt"));
    let result = backend_for(
        "kotlinc",
        delegated_tool(generated.path().to_path_buf()),
        must_not_spawn,
    )
    .diagnose("kotlinc", "typecheck", &single("k/A.kt", KOTLINC_SOURCE));
    assert!(result.is_err(), "{result:?}");
}

#[test]
fn delegated_kotlinc_fails_closed_without_its_log() {
    let mut tool = delegated_tool(PathBuf::new());
    tool.upstream_diagnostics.clear();
    let error = backend_for("kotlinc", tool, must_not_spawn)
        .diagnose("kotlinc", "typecheck", &single("k/A.kt", KOTLINC_SOURCE))
        .expect_err("missing log");
    assert!(error.to_string().contains("check log"), "{error}");
}

#[test]
fn gotype_reports_findings_and_fails_closed_on_driver_errors() {
    let report = upstream_file(
        "gotype",
        r#"{"p":{"typecheck":[{"posn":"a.txt:1:5","message":"undefined: x"}]}}"#,
    );
    let backend = backend_for(
        "gotype",
        delegated_tool(report.path().to_path_buf()),
        must_not_spawn,
    );
    let found = backend
        .diagnose("gotype", "typecheck", &single("a.txt", "var x = 1\n"))
        .expect("findings");
    assert_eq!(found.len(), 1);
    let failed = upstream_file(
        "gotype",
        r#"{"p":{"typecheck":{"error":"no export data"}}}"#,
    );
    let backend = backend_for(
        "gotype",
        delegated_tool(failed.path().to_path_buf()),
        must_not_spawn,
    );
    assert!(backend
        .diagnose("gotype", "typecheck", &single("a.txt", "x\n"))
        .is_err());
}

#[test]
fn go_lint_tools_fail_closed_without_exactly_one_report() {
    for tool_id in ["govet", "errcheck"] {
        let mut tool = delegated_tool(std::path::PathBuf::new());
        tool.upstream_diagnostics.clear();
        let backend = backend_for(tool_id, tool, must_not_spawn);
        let error = backend
            .diagnose(tool_id, "lint", &single("a.go", "package a\n"))
            .expect_err("no upstream report");
        assert!(
            error.to_string().contains("did not supply it"),
            "{tool_id}: {error}"
        );
    }
}
