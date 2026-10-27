use std::path::PathBuf;

use dx_testing::{mkscratch, Run};

mod support;

use support::{commit, dx_in, fixture, write};

fn repo(name: &str) -> PathBuf {
    let root = mkscratch(name).unwrap_or_else(|error| panic!("scratch: {error}"));
    fixture(&root);
    commit(&root);
    root
}

fn nested_repo(name: &str, module: &str) -> (PathBuf, PathBuf) {
    let root = mkscratch(name).unwrap_or_else(|error| panic!("scratch: {error}"));
    let workspace = root.join("sub");
    fixture(&workspace);
    write(&workspace.join("MODULE.bazel"), module);
    write(&root.join("frontend/x.ts"), "x\n");
    write(
        &root.join("lib/MODULE.bazel"),
        "module(name = \"lib\", version = \"0\")\n",
    );
    write(&root.join("lib/BUILD.bazel"), "");
    write(&root.join("lib/l.txt"), "l\n");
    commit(&root);
    (root, workspace)
}

fn lines(run: &Run) -> Vec<String> {
    run.stdout.lines().map(str::to_owned).collect()
}

const FIXTURE_TESTS: [&str; 3] = ["//a:a_test", "//b:b_test", "//c:c_test"];

/// Asserts `dx test --affected` ran exactly `want` among the fixture's tests.
fn assert_ran(run: &Run, want: &[&str]) {
    assert_eq!(run.status.code(), Some(0), "{}", run.combined());
    let all = run.combined();
    for label in FIXTURE_TESTS {
        assert_eq!(all.contains(label), want.contains(&label), "{label}: {all}");
    }
}

#[test]
fn test_affected_runs_changed_lib_tests_and_reverse_deps_only() {
    let root = repo("dx-affected-lib");
    write(&root.join("a/a.txt"), "changed\n");

    let ran = dx_in(&root, &["test", "--affected"]);
    assert_ran(&ran, &["//a:a_test", "//b:b_test"]);
    let resolving = ran
        .stderr
        .find("dx: affected: resolving changes since ")
        .unwrap_or_else(|| panic!("{}", ran.combined()));
    let summary = ran
        .stderr
        .find("dx: affected: 2 targets")
        .unwrap_or_else(|| panic!("{}", ran.combined()));
    assert!(resolving < summary, "{}", ran.combined());
}

#[test]
fn test_affected_build_only_edit_runs_the_package_tests_and_reverse_deps() {
    let root = repo("dx-affected-build");
    let build = root.join("a/BUILD.bazel");
    let text = std::fs::read_to_string(&build).unwrap_or_else(|error| panic!("read: {error}"));
    write(&build, &format!("{text}# edit\n"));

    let listed = dx_in(&root, &["test", "--affected"]);
    assert_ran(&listed, &["//a:a_test", "//b:b_test"]);
    assert!(
        listed
            .stderr
            .contains("widened to //a:all (build file a/BUILD.bazel)"),
        "{}",
        listed.combined()
    );
}

#[test]
fn test_affected_json_reports_the_affected_object() {
    let root = repo("dx-affected-json");
    write(&root.join("b/b.txt"), "changed\n");
    let json = dx_in(&root, &["test", "--affected", "--output=json"]);
    assert_eq!(json.status.code(), Some(0), "{}", json.combined());
    let first = json.stdout.lines().next().unwrap_or_default();
    assert!(first.contains("\"command_started\""), "{first}");
    assert!(first.contains("\"changed_files\":[\"b/b.txt\"]"), "{first}");
    assert!(first.contains("\"widened\":[]"), "{first}");
    assert!(first.contains("//b:b_test"), "{first}");
    assert!(!first.contains("//a:a_test"), "{first}");
    assert!(!first.contains("\"tests\""), "{first}");
    assert!(
        !json.stderr.contains("resolving changes"),
        "{}",
        json.combined()
    );
}

#[test]
fn untracked_files_count_and_a_clean_tree_selects_nothing() {
    let root = repo("dx-affected-clean");
    let clean = dx_in(&root, &["test", "--affected"]);
    assert_eq!(clean.status.code(), Some(0), "{}", clean.combined());
    assert_eq!(clean.stdout.trim(), "no affected targets");

    for args in [
        &["fix", "--affected"][..],
        &["fix", "--apply", "--affected"][..],
        &["check", "--affected"][..],
    ] {
        let run = dx_in(&root, args);
        assert_eq!(run.status.code(), Some(0), "{args:?}: {}", run.combined());
        assert_eq!(run.stdout.trim(), "no affected targets", "{args:?}");
        assert!(
            !run.stderr.lines().any(|line| line.contains("phase")),
            "{args:?}: {}",
            run.combined()
        );
    }

    let json = dx_in(&root, &["fix", "--affected", "--output=json"]);
    assert_eq!(json.status.code(), Some(0), "{}", json.combined());
    let events = lines(&json);
    assert_eq!(events.len(), 2, "{}", json.combined());
    assert!(events[0].contains("\"command_started\""), "{}", events[0]);
    assert!(events[0].contains("\"affected\":"), "{}", events[0]);
    assert!(events[0].contains("\"mode\":\"check\""), "{}", events[0]);
    assert!(events[1].contains("\"command_finished\""), "{}", events[1]);

    write(&root.join("c/c.txt"), "changed\n");
    let dirty = dx_in(&root, &["test", "--affected"]);
    assert_ran(&dirty, &["//c:c_test"]);
}

#[test]
fn module_change_widens_to_the_whole_workspace() {
    let root = repo("dx-affected-widen");
    write(
        &root.join("MODULE.bazel"),
        "module(name = \"fixture\", repo_name = \"rules_dx\", version = \"1\")\n",
    );
    let listed = dx_in(&root, &["test", "--affected"]);
    assert_ran(&listed, &FIXTURE_TESTS);
    assert!(
        listed.stderr.contains("widened to //..."),
        "{}",
        listed.combined()
    );
}

#[test]
fn affected_conflicts_with_scopes_and_here() {
    let root = repo("dx-affected-usage");
    let scoped = dx_in(&root, &["test", "--affected", "//a:a_test"]);
    assert_eq!(scoped.status.code(), Some(2), "{}", scoped.combined());
    let here = dx_in(&root, &["test", "--affected", "--here"]);
    assert_eq!(here.status.code(), Some(2), "{}", here.combined());
}

#[test]
fn workspace_below_the_git_root_maps_changes_without_widening() {
    let (_root, workspace) = nested_repo(
        "dx-affected-sub",
        "module(name = \"fixture\", repo_name = \"rules_dx\")\n",
    );
    write(&workspace.join("a/a.txt"), "changed\n");
    let json = dx_in(&workspace, &["test", "--affected", "--output=json"]);
    assert_eq!(json.status.code(), Some(0), "{}", json.combined());
    let first = json.stdout.lines().next().unwrap_or_default();
    assert!(first.contains("\"changed_files\":[\"a/a.txt\"]"), "{first}");
    assert!(first.contains("\"widened\":[]"), "{first}");
    assert!(first.contains("//a:a_test"), "{first}");
    assert!(first.contains("//b:b_test"), "{first}");
    assert!(!first.contains("//c:c_test"), "{first}");
}

#[test]
fn changes_outside_a_nested_workspace_select_nothing() {
    let (root, workspace) = nested_repo(
        "dx-affected-outside",
        "module(name = \"fixture\", repo_name = \"rules_dx\")\n",
    );
    write(&root.join("frontend/x.ts"), "changed\n");
    write(&root.join("frontend/new.ts"), "new\n");
    let ran = dx_in(&workspace, &["test", "--affected"]);
    assert_eq!(ran.status.code(), Some(0), "{}", ran.combined());
    assert_eq!(
        ran.stdout.trim(),
        "no affected targets",
        "{}",
        ran.combined()
    );
    assert!(!ran.stderr.contains("widened"), "{}", ran.combined());
}

#[test]
fn local_path_override_change_outside_the_workspace_widens() {
    let (root, workspace) = nested_repo(
        "dx-affected-override",
        "module(name = \"fixture\", repo_name = \"rules_dx\")\nbazel_dep(name = \"lib\", version = \"0\")\nlocal_path_override(module_name = \"lib\", path = \"../lib\")\n",
    );
    write(&root.join("lib/l.txt"), "changed\n");
    let listed = dx_in(&workspace, &["test", "--affected"]);
    assert_ran(&listed, &FIXTURE_TESTS);
    assert!(
        listed
            .stderr
            .contains("widened to //... (local_path_override ../lib/l.txt)"),
        "{}",
        listed.combined()
    );
}
