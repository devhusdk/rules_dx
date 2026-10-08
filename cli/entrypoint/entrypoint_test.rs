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
fn consumer_dependency_sets_plan_update_from_their_own_directories() {
    let dryrun = read("DX_SETS_DRYRUN");
    assert!(
        dryrun.contains("Running update for frontend, worker"),
        "bare update selects exactly the configured sets: {dryrun}"
    );
    assert!(
        dryrun.contains("Would update frontend: uv lock --directory apps/frontend"),
        "frontend plans from its own directory: {dryrun}"
    );
    assert!(
        dryrun.contains("Would update worker: uv lock --directory services/worker"),
        "worker plans from its own directory: {dryrun}"
    );
    for phantom in [
        "cargo",
        "pnpm",
        "maven",
        "nuget",
        "examples/",
        "third_party/",
    ] {
        assert!(
            !dryrun.contains(phantom),
            "no rules_dx builtin set may appear: {dryrun}"
        );
    }
    let one = read("DX_SETS_DRYRUN_ONE");
    assert!(
        one.contains("Running update for worker"),
        "selective update narrows to one set: {one}"
    );
    assert!(!one.contains("frontend"), "selective update leaks: {one}");
    let check = read("DX_SETS_CHECK_DRYRUN");
    assert!(
        check.contains("Would check frontend: uv lock --check --directory apps/frontend"),
        "check mode plans read-only: {check}"
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
fn consumer_dependency_set_fixtures_fail_closed() {
    let overlap = read("DX_SETS_OVERLAP");
    assert!(
        overlap.contains("Running update for left, right"),
        "an overlapping scope selects every owner: {overlap}"
    );
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
