use std::path::PathBuf;

fn read(key: &str) -> String {
    let rel = std::env::var(key).unwrap_or_else(|_| panic!("{key} must name a captured output"));
    let path: PathBuf = dx_testing::resolve_runfiles(&rel);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read {}: {error}", path.display()))
}

#[test]
fn the_public_label_answers_help_from_the_consumer_repository() {
    let help = read("DX_PUBLIC_HELP");
    assert!(
        help.starts_with("dx - Run Bazel workflows"),
        "the public launcher must print its own help: {help}"
    );
    assert!(help.contains("Usage: dx [COMMAND]"), "help: {help}");
}

#[test]
fn the_public_and_crate_labels_run_one_binary() {
    assert_eq!(
        read("DX_PUBLIC_HELP"),
        read("DX_CRATE_HELP"),
        "@rules_dx//:dx must stay the same binary as @rules_dx//cli/cli:dx"
    );
}

#[test]
fn the_public_label_runs_real_commands_over_a_consumer_workspace() {
    let version = read("DX_PUBLIC_VERSION");
    assert!(version.contains("dx 0.0.0"), "version: {version}");
    let status = read("DX_PUBLIC_STATUS");
    assert!(status.contains("pin: ok"), "status: {status}");
    let completion = read("DX_PUBLIC_COMPLETION");
    assert!(
        completion.contains("_clap_complete_dx"),
        "completion: {completion}"
    );
}

#[test]
fn the_public_label_reports_a_drifted_consumer_pin() {
    let drift = read("DX_PUBLIC_DRIFT");
    assert!(
        drift.contains("status_pin_mismatch"),
        "a drifted pin must fail through the public label: {drift}"
    );
}

#[test]
fn consumer_dependency_sets_audit_and_reject_bump() {
    let security = read("DX_SETS_SECURITY");
    assert!(
        security.contains("no advisory coverage for frontend, worker"),
        "audit names both configured sets: {security}"
    );
    let license = read("DX_SETS_LICENSE");
    assert!(
        license.contains("audit_license_clean"),
        "license passes with no packages: {license}"
    );
    let bump = read("DX_SETS_BUMP");
    assert!(
        bump.contains("bump is not supported for ecosystem uv set frontend"),
        "bump names the unsupported operation: {bump}"
    );
}

#[test]
fn consumer_new_applies_a_stdlib_cpp_project() {
    let apply = read("DX_NEW_APPLY");
    let wrote: Vec<&str> = apply
        .lines()
        .filter(|line| line.starts_with("wrote myproj/"))
        .collect();
    assert_eq!(
        wrote.len(),
        11,
        "new --apply writes init wiring plus two C++ sources: {apply}"
    );
    for path in [
        "wrote myproj/.dx/version",
        "wrote myproj/MODULE.bazel.snippet",
        "wrote myproj/hello.cc",
        "wrote myproj/hello.h",
    ] {
        assert!(apply.contains(path), "apply lacks {path}: {apply}");
    }
    assert!(
        apply.contains("exit=0"),
        "apply records its exit code: {apply}"
    );
    let cc = read("DX_SCAFFOLD_HELLO_CC");
    assert!(
        cc.contains("#include \"hello.h\""),
        "hello.cc includes its own header: {cc}"
    );
    let header = read("DX_SCAFFOLD_HELLO_H");
    assert!(
        header.contains("#pragma once"),
        "hello.h is a guarded header: {header}"
    );
    assert!(
        header.contains("#include <string>"),
        "hello.h is stdlib-only: {header}"
    );
    let snippet = read("DX_SCAFFOLD_SNIPPET");
    assert!(
        snippet.contains("register_toolchains(\"@llvm//toolchain:all\")"),
        "snippet registers hermetic LLVM: {snippet}"
    );
    assert!(
        snippet.contains("register_toolchains(\"@rust_toolchains//:all\")"),
        "snippet registers hermetic Rust: {snippet}"
    );
    assert!(
        !read("DX_SCAFFOLD_VERSION").trim().is_empty(),
        "the scaffolded project pins a dx version"
    );
}

#[test]
fn consumer_new_checks_before_applying_and_refuses_to_overwrite() {
    let check = read("DX_NEW_CHECK");
    assert!(
        check.contains("new drift: missing"),
        "check mode reports drift instead of writing: {check}"
    );
    assert!(
        check.contains("checkonly/.dx/version"),
        "check mode names the missing scaffold: {check}"
    );
    assert!(check.contains("exit=1"), "check mode fails: {check}");
    let reapply = read("DX_NEW_REAPPLY");
    assert!(
        reapply.contains("absent-only, left untouched"),
        "re-apply refuses existing files: {reapply}"
    );
    assert!(
        reapply.contains("exit=0"),
        "re-apply still succeeds: {reapply}"
    );
    let invalid = read("DX_NEW_INVALID");
    assert!(
        invalid.contains("unknown language"),
        "an unknown language fails: {invalid}"
    );
    assert!(invalid.contains("exit=2"), "usage errors exit 2: {invalid}");
}

#[test]
fn consumer_init_applies_and_rechecks_a_foreign_tree() {
    let apply = read("DX_INIT_APPLY");
    let wrote: Vec<&str> = apply
        .lines()
        .filter(|line| line.starts_with("wrote "))
        .collect();
    assert_eq!(
        wrote.len(),
        9,
        "init --apply writes the nine wiring files: {apply}"
    );
    assert!(
        apply.contains("wrote MODULE.bazel.snippet"),
        "init scaffolds the consumer setup: {apply}"
    );
    assert!(
        apply.contains("exit=0"),
        "apply records its exit code: {apply}"
    );
    let check = read("DX_INIT_CHECK");
    assert!(
        check.contains("init drift: missing"),
        "check mode reports drift instead of writing: {check}"
    );
    assert!(check.contains("exit=1"), "check mode fails: {check}");
    let clean = read("DX_INIT_CLEAN");
    assert!(
        clean.contains("init current: demo"),
        "an applied tree rechecks clean: {clean}"
    );
    assert!(clean.contains("exit=0"), "a clean tree passes: {clean}");
    let bad = read("DX_INIT_BAD");
    assert!(
        bad.contains("invalid module name"),
        "an invalid module fails: {bad}"
    );
    assert!(bad.contains("exit=2"), "usage errors exit 2: {bad}");
    let snippet = read("DX_INIT_SNIPPET");
    assert!(
        snippet.contains("register_toolchains(\"@llvm//toolchain:all\")"),
        "the init snippet matches the hermetic setup: {snippet}"
    );
    assert!(
        !read("DX_INIT_VERSION").trim().is_empty(),
        "the wired tree pins a dx version"
    );
}

#[test]
fn consumer_dependency_set_fixtures_fail_closed() {
    let bad = read("DX_SETS_BAD_KIND");
    assert!(
        bad.contains("unknown ecosystem"),
        "an unknown backend kind fails closed: {bad}"
    );
    let nolock = read("DX_SETS_NOLOCK");
    assert!(
        nolock.contains("failed to assess frontend"),
        "a missing lock fails the license audit: {nolock}"
    );
    assert!(
        nolock.contains("apps/frontend/uv.lock"),
        "a missing lock names its path: {nolock}"
    );
}
