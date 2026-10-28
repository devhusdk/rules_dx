use super::real_fixtures::*;
use super::*;

#[test]
fn delegated_diagnostics_fail_closed_when_missing_or_malformed() {
    for tool_id in [
        "clippy",
        "rustc",
        "roslyn",
        "fsharplint",
        "qmllint",
        "govet",
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
        let capability = if tool_id == "rustc" {
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
        ("govet", "src/a.scala:1:1: finding\n"),
        ("errcheck", "src/a.scala:1:1: finding\n"),
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
        "dfmt",
        "djlint",
        "erlfmt",
        "fantomas",
        "gofumpt",
        "jsonnetfmt",
        "julia_format",
        "ktlint",
        "mix_format",
        "modfmt",
        "nixfmt",
        "ormolu",
        "opa",
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
        ("errcheck", ""),
        ("fsharplint", ""),
        ("govet", ""),
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
