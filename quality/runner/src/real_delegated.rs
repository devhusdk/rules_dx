use super::real_fixtures::*;
use super::*;

#[test]
fn delegated_diagnostics_fail_closed_when_missing_or_malformed() {
    for tool_id in [
        "clippy",
        "rustc",
        "roslyn",
        "scalafix",
        "fsharplint",
        "buf",
        "qmllint",
        "govet",
        "errcheck",
        "staticcheck",
        "clang_tidy",
        "cppcheck",
        "rubocop",
        "psscriptanalyzer",
        "shellcheck",
        "stylelint",
        "yamllint",
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
            "buf",
            r#"{"path":"src/a.scala","start_line":1,"start_column":1,"type":"PACKAGE_DEFINED","message":"finding"}"#,
        ),
        (
            "qmllint",
            r#"{"diagnostics":[{"file":"src/a.scala","line":1,"column":1,"rule":"demo","message":"finding","severity":"warning"}]}"#,
        ),
        (
            "roslyn",
            r#"{"version":"2.1.0","runs":[{"results":[{"ruleId":"CA1822","level":"warning","message":{"text":"finding"},"locations":[{"physicalLocation":{"artifactLocation":{"uri":"src/a.scala"},"region":{"startLine":1,"startColumn":1,"endLine":1,"endColumn":2}}}]}]}]}"#,
        ),
        ("shellcheck", "src/a.scala:1:1: warning: finding [SC2086]\n"),
        ("yamllint", "src/a.scala:1:1: [trailing-spaces] finding\n"),
        ("govet", "src/a.scala:1:1: finding\n"),
        ("errcheck", "src/a.scala:1:1: finding\n"),
        (
            "clang_tidy",
            "src/a.scala:1:1: warning: finding [demo-check]\n",
        ),
        (
            "cppcheck",
            r#"<error id="demo" severity="warning" msg="finding"><location file="src/a.scala" line="1" column="1"/></error>"#,
        ),
        (
            "staticcheck",
            r#"[{"code":"S1000","severity":"warning","location":{"file":"src/a.scala","line":1,"column":1},"message":"finding"}]"#,
        ),
        ("psscriptanalyzer", "src/a.scala:1:1: [Demo] finding\n"),
        (
            "scalafix",
            r#"{"path":"src/a.scala","line":1,"column":1,"rule":"Rule","message":"finding","severity":"warning"}"#,
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
    env: &[(String, String)],
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
    env: &[(String, String)],
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
        "djlint",
        "fantomas",
        "gofumpt",
        "jsonnetfmt",
        "ktlint",
        "modfmt",
        "pkl",
        "qmlformat",
        "scalafmt",
        "shfmt",
        "standardrb",
        "terraform",
        "yamlfmt",
    ] {
        let backend = backend_for(tool_id, plain_tool(), empty_report);
        let fixed = backend
            .apply_fix(tool_id, "a.txt", "x\n", "format")
            .expect("fix round reaches the tool");
        assert_eq!(fixed, "x\n");
    }
}

#[test]
pub(super) fn delegated_tools_read_diagnostics_without_spawning() {
    for tool_id in [
        "buf",
        "clang_tidy",
        "cppcheck",
        "djlint",
        "errcheck",
        "fsharplint",
        "govet",
        "keep_sorted",
        "psscriptanalyzer",
        "qmllint",
        "roslyn",
        "rubocop",
        "scalafix",
        "shellcheck",
        "staticcheck",
        "stylelint",
        "yamllint",
    ] {
        let upstream = upstream_file(tool_id, "");
        let tool = delegated_tool(upstream.path().to_path_buf());
        let backend = backend_for(tool_id, tool, must_not_spawn);
        let _ = backend.diagnose(tool_id, "lint", &single("a.txt", "x\n"));
    }
}
