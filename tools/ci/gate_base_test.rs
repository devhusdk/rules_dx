use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const ZEROS: &str = "0000000000000000000000000000000000000000";

fn commit(repo: &Path, name: &str) -> String {
    std::fs::write(repo.join(name), name).expect("write file");
    dx_test_scratch::git(repo, repo, &["add", name]);
    dx_test_scratch::git(repo, repo, &["commit", "-q", "-m", name]);
    dx_test_scratch::git(repo, repo, &["rev-parse", "HEAD"])
}

struct Repo {
    path: PathBuf,
    a: String,
    b: String,
    c: String,
    x: String,
}

fn repo(name: &str) -> Repo {
    let path = dx_test_scratch::scratch(name).path().join("repo");
    std::fs::create_dir_all(&path).expect("repo directory");
    dx_test_scratch::git(&path, &path, &["init", "-q", "-b", "main"]);
    let a = commit(&path, "a");
    let b = commit(&path, "b");
    let c = commit(&path, "c");
    dx_test_scratch::git(&path, &path, &["checkout", "-q", "-b", "other", &a]);
    let x = commit(&path, "x");
    dx_test_scratch::git(&path, &path, &["checkout", "-q", "main"]);
    Repo { path, a, b, c, x }
}

fn run(repo: &Repo, event: &str, before: &str, group: &str) -> (Output, String) {
    let env = repo.path.join("github_env");
    std::fs::write(&env, "").expect("env file");
    let binary =
        dx_testing::runfiles_root().join(std::env::var("DX_GATE_BASE").expect("DX_GATE_BASE"));
    let done = Command::new(binary)
        .current_dir(&repo.path)
        .env("GITHUB_ENV", &env)
        .env("EVENT_NAME", event)
        .env("PUSH_BEFORE", before)
        .env("MERGE_GROUP_BASE", group)
        .output()
        .expect("run gate_base_bin");
    let text = std::fs::read_to_string(&env).expect("read env file");
    let base = text
        .trim()
        .strip_prefix("GATE_BASE=")
        .unwrap_or("")
        .to_string();
    (done, base)
}

fn stderr(done: &Output) -> String {
    String::from_utf8_lossy(&done.stderr).into_owned()
}

#[test]
fn pull_request_uses_the_first_parent() {
    let r = repo("gate-base-pr");
    dx_test_scratch::git(
        &r.path,
        &r.path,
        &["merge", "-q", "--no-ff", "-m", "m", "other"],
    );
    let (done, base) = run(&r, "pull_request", "", "");
    assert!(done.status.success(), "{}", stderr(&done));
    assert_eq!(base, r.c);
    assert_eq!(
        dx_test_scratch::git(&r.path, &r.path, &["rev-parse", "refs/remotes/origin/main"]),
        r.c
    );
}

#[test]
fn push_uses_before() {
    let r = repo("gate-base-push");
    let (done, base) = run(&r, "push", &r.a, "");
    assert!(done.status.success(), "{}", stderr(&done));
    assert_eq!(base, r.a);
}

#[test]
fn push_with_zero_before_uses_the_first_parent() {
    let r = repo("gate-base-zero");
    let (done, base) = run(&r, "push", ZEROS, "");
    assert!(done.status.success(), "{}", stderr(&done));
    assert_eq!(base, r.b);
}

#[test]
fn force_push_resolves_through_the_merge_base() {
    let r = repo("gate-base-force");
    let (done, base) = run(&r, "push", &r.x, "");
    assert!(done.status.success(), "{}", stderr(&done));
    assert_eq!(base, r.a);
}

#[test]
fn merge_group_uses_base_sha() {
    let r = repo("gate-base-group");
    let (done, base) = run(&r, "merge_group", "", &r.a);
    assert!(done.status.success(), "{}", stderr(&done));
    assert_eq!(base, r.a);
}

#[test]
fn base_equal_to_head_fails() {
    let r = repo("gate-base-head");
    let (done, _) = run(&r, "push", &r.c, "");
    assert!(!done.status.success());
    assert!(stderr(&done).contains("equals HEAD"), "{}", stderr(&done));
}

#[test]
fn missing_merge_group_value_fails() {
    let r = repo("gate-base-missing");
    let (done, _) = run(&r, "merge_group", "", "");
    assert!(!done.status.success());
    assert!(stderr(&done).contains("base_sha"), "{}", stderr(&done));
}
