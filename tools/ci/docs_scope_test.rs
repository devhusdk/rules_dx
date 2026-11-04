use docs_scope::lint_argv;

const PROBE: &str = "DX_SCOPE_EXECUTED";

fn args(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| s.to_string()).collect()
}

#[test]
fn valid_scopes_reach_bazel_as_separate_arguments() {
    let argv = lint_argv("//... //docs:all @dep//pkg:tgt", &args(&["--config=ci"]));
    assert_eq!(
        argv,
        Ok(args(&[
            "run",
            "--config=ci",
            "@rules_dx//:dx",
            "--",
            "lint",
            "//...",
            "//docs:all",
            "@dep//pkg:tgt",
            "--",
            "--config=ci",
        ]))
    );
}

#[test]
fn more_valid_patterns_are_accepted() {
    for scope in [
        "//",
        "//a/b-c_d.e:f+g",
        "@@repo+1//x:y",
        "@//:z",
        "  //...   //a  ",
    ] {
        assert!(lint_argv(scope, &[]).is_ok(), "{scope}");
    }
}

#[test]
fn tilde_repo_names_are_rejected() {
    assert!(lint_argv("@@repo~1//x:y", &[]).is_err());
}

#[test]
fn substitution_is_rejected() {
    let error = lint_argv(&format!("//...$(printf {PROBE} >&2)"), &[]).unwrap_err();
    assert!(error.contains("not a target pattern"), "{error}");
}

#[test]
fn separators_quotes_and_globs_are_rejected() {
    for scope in [
        "//...;id",
        "//...\n//other",
        "//...\t//other",
        "//... && id",
        "//a|b",
        "\"//...\"",
        "'//...'",
        "//*",
        "//...`id`",
        "--fix",
        "-//foo",
        "foo",
        "//a:",
        "",
        "   ",
    ] {
        assert!(lint_argv(scope, &[]).is_err(), "{scope:?}");
    }
}

#[cfg(unix)]
#[test]
fn the_binary_runs_bazel_without_a_shell() {
    use std::os::unix::fs::PermissionsExt;
    let scratch = dx_test_scratch::scratch("docs-scope");
    let bin = scratch.path().join("bin");
    dx_test_scratch::write(
        &bin,
        "bazel",
        "#!/bin/sh\nfor arg in \"$@\"; do printf '%s\\n' \"$arg\"; done > \"$DX_ARGV_LOG\"\n",
    );
    std::fs::set_permissions(bin.join("bazel"), std::fs::Permissions::from_mode(0o755)).unwrap();
    let log = scratch.path().join("argv.log");
    let path = format!(
        "{}:{}",
        bin.display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let exe = dx_testing::runfiles_root().join(std::env::var("DX_DOCS_SCOPE_BIN").unwrap());
    let done = std::process::Command::new(exe)
        .current_dir(scratch.path())
        .env("DX_DOCS_SCOPE", "//... //docs:all")
        .env("DX_ARGV_LOG", &log)
        .env("BAZEL_CONFIG", "--config=ci")
        .env("BB_ARGS", "")
        .env("PATH", path)
        .output()
        .unwrap();
    assert!(
        done.status.success(),
        "{}",
        String::from_utf8_lossy(&done.stderr)
    );
    let recorded = std::fs::read_to_string(&log).unwrap();
    assert_eq!(
        recorded.lines().collect::<Vec<_>>(),
        [
            "run",
            "--config=ci",
            "@rules_dx//:dx",
            "--",
            "lint",
            "//...",
            "//docs:all",
            "--",
            "--config=ci"
        ]
    );
    let bad = std::process::Command::new(
        dx_testing::runfiles_root().join(std::env::var("DX_DOCS_SCOPE_BIN").unwrap()),
    )
    .current_dir(scratch.path())
    .env("DX_DOCS_SCOPE", "//...;id")
    .output()
    .unwrap();
    assert_eq!(bad.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&bad.stderr).starts_with("dx-ci: "));
}
