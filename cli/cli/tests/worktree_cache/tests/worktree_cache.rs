#![cfg(unix)]

use std::path::{Path, PathBuf};
use std::process::Command;

use dx_nested::{self as nested, Case};
use dx_testing::serde_json::{Deserializer, Value};
use dx_testing::{resolve_runfiles, Run};

const BAZELRC: &str = "common:dx_dev --define=dx=dev\ncommon:dx_release --define=dx=release\n";

const PKG_BUILD: &str = r#"genrule(
    name = "gen_a",
    outs = ["a.txt"],
    cmd = "echo a > $@",
)

genrule(
    name = "gen_b",
    srcs = [":gen_a"],
    outs = ["b.txt"],
    cmd = "cat $(location :gen_a) > $@ && echo b >> $@",
)
"#;

const HIT: &str = "disk cache hit";

struct Scratch {
    repo: PathBuf,
    home: PathBuf,
    cache: PathBuf,
}

fn write(path: &Path, text: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap_or_else(|error| panic!("mkdir: {error}"));
    }
    std::fs::write(path, text).unwrap_or_else(|error| panic!("write {path:?}: {error}"));
}

fn git(root: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args([
            "-c",
            "user.name=dx",
            "-c",
            "user.email=dx@example.com",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .current_dir(root)
        .status()
        .unwrap_or_else(|error| panic!("git: {error}"));
    assert!(status.success(), "git {args:?}");
}

fn fixture(case: &Case) -> Scratch {
    let scratch = Scratch {
        repo: case.root().join("repo"),
        home: case.root().join("home"),
        cache: case.root().join("cache"),
    };
    let repo = &scratch.repo;
    write(&repo.join("MODULE.bazel"), &nested::rules_dx_module("", ""));
    write(&repo.join(".bazelrc"), BAZELRC);
    write(&repo.join("BUILD.bazel"), "");
    nested::write_rules_dx_config(repo);
    write(&repo.join("pkg/BUILD.bazel"), PKG_BUILD);
    write(&repo.join(".gitignore"), "/bazel-*\n/MODULE.bazel.lock\n");
    std::fs::create_dir_all(&scratch.home).unwrap_or_else(|error| panic!("mkdir: {error}"));
    std::fs::create_dir_all(&scratch.cache).unwrap_or_else(|error| panic!("mkdir: {error}"));
    git(repo, &["init", "--quiet", "--initial-branch=main"]);
    git(repo, &["add", "."]);
    git(repo, &["commit", "--quiet", "-m", "base"]);
    scratch
}

/// Returns the bazelisk cache of the outer environment so nested dx calls never download Bazel.
fn bazelisk_home() -> String {
    if let Some(home) = std::env::var_os("BAZELISK_HOME").filter(|home| !home.is_empty()) {
        return home.to_string_lossy().into_owned();
    }
    let cache = std::env::var_os("XDG_CACHE_HOME")
        .filter(|cache| !cache.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cache")))
        .unwrap_or_else(|| panic!("HOME must be set"));
    cache.join("bazelisk").to_string_lossy().into_owned()
}

/// Runs dx locally in `dir` with the scratch home and cache.
fn dx(case: &Case, scratch: &Scratch, dir: &Path, args: &[&str]) -> Run {
    let bin = resolve_runfiles(
        &std::env::var("DX_BIN").unwrap_or_else(|_| panic!("DX_BIN must name dx")),
    );
    let home = scratch.home.to_string_lossy().into_owned();
    let cache = scratch.cache.to_string_lossy().into_owned();
    let bazelisk = bazelisk_home();
    let mut command = case.command(
        &bin,
        &[
            ("HOME", home.as_str()),
            ("XDG_CACHE_HOME", cache.as_str()),
            ("CLAUDE_CODE_SESSION_ID", "worktree-cache"),
            ("BAZELISK_HOME", bazelisk.as_str()),
        ],
    );
    command
        .env_remove("BAZEL_TEST")
        .env_remove("CI")
        .env_remove("DX_BUILD_SLOT")
        .env_remove("DX_BUILD_SHARE")
        .args(args)
        .current_dir(dir);
    let output = command
        .output()
        .unwrap_or_else(|error| panic!("run dx: {error}"));
    Run {
        status: output.status,
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

fn ok(run: &Run, what: &str) {
    assert!(run.status.success(), "{what} failed:\n{}", run.combined());
}

fn add(case: &Case, scratch: &Scratch, branch: &str) -> PathBuf {
    let added = dx(
        case,
        scratch,
        &scratch.repo,
        &["worktree", "add", branch, "--create=main", "--output=path"],
    );
    ok(&added, &format!("worktree add {branch}"));
    PathBuf::from(added.stdout.trim())
}

fn build(case: &Case, scratch: &Scratch, slot: &Path, log: &Path) -> Vec<Value> {
    let flag = format!("--execution_log_json_file={}", log.display());
    let built = dx(case, scratch, slot, &["build", "//pkg:all", "--", &flag]);
    ok(&built, &format!("build in {}", slot.display()));
    let text = std::fs::read_to_string(log).unwrap_or_else(|error| panic!("read {log:?}: {error}"));
    Deserializer::from_str(&text)
        .into_iter::<Value>()
        .map(|spawn| spawn.unwrap_or_else(|error| panic!("parse {log:?}: {error}")))
        .collect()
}

fn runner(spawn: &Value) -> &str {
    spawn["runner"].as_str().unwrap_or_default()
}

fn dir_has_files(dir: &Path) -> bool {
    std::fs::read_dir(dir).is_ok_and(|entries| {
        entries.filter_map(Result::ok).any(|entry| {
            let path = entry.path();
            path.is_file() || dir_has_files(&path)
        })
    })
}

#[test]
fn a_second_slot_on_the_same_commit_hits_the_shared_disk_cache() {
    let case = nested::case("worktree-cache");
    let scratch = fixture(&case);

    let one = add(&case, &scratch, "one");
    let two = add(&case, &scratch, "two");
    assert_ne!(one, two, "both branches got the same slot");

    let first = build(&case, &scratch, &one, &case.root().join("one.json"));
    let disk_cache = scratch.cache.join("dx/disk-cache");
    assert!(
        dir_has_files(&disk_cache),
        "{} is empty after the first build; an rc such as /etc/bazel.bazelrc may set --disk_cache",
        disk_cache.display()
    );
    let executed = first.iter().filter(|spawn| runner(spawn) != HIT).count();
    assert!(
        executed >= 2,
        "slot one executed {executed} spawns: {first:#?}"
    );

    let second = build(&case, &scratch, &two, &case.root().join("two.json"));
    assert_eq!(
        second.len(),
        first.len(),
        "slot two ran other spawns: {second:#?}"
    );
    let missed: Vec<&Value> = second.iter().filter(|spawn| runner(spawn) != HIT).collect();
    assert!(
        missed.is_empty(),
        "slot two missed the disk cache: {missed:#?}"
    );

    let listed = dx(
        &case,
        &scratch,
        &scratch.repo,
        &["worktree", "list", "--disk", "--output=json"],
    );
    ok(&listed, "worktree list");
    let summary = listed
        .stdout
        .lines()
        .filter_map(|line| dx_testing::serde_json::from_str::<Value>(line).ok())
        .find(|event| event["event"] == "summary")
        .unwrap_or_else(|| panic!("no summary event:\n{}", listed.combined()));
    assert!(
        summary["disk_cache_bytes"]
            .as_u64()
            .is_some_and(|bytes| bytes > 0),
        "{summary}"
    );

    for slot in [&one, &two] {
        let released = dx(&case, &scratch, slot, &["worktree", "release", "--force"]);
        ok(&released, &format!("release {}", slot.display()));
    }
    let left = nested::servers_in_workspaces(&scratch.cache);
    assert!(left.is_empty(), "slot servers survived release: {left:?}");
}

#[test]
fn changing_local_resources_keeps_the_analysis_cache() {
    let case = nested::case("worktree-cache-resources");
    let scratch = fixture(&case);
    let slot = add(&case, &scratch, "one");

    let first = dx(
        &case,
        &scratch,
        &slot,
        &["build", "//pkg:all", "--", "--local_resources=cpu=2"],
    );
    ok(&first, "first build");
    let second = dx(
        &case,
        &scratch,
        &slot,
        &["build", "//pkg:all", "--", "--local_resources=cpu=1"],
    );
    ok(&second, "second build");
    let output = second.combined();
    assert!(
        output.contains("0 targets configured"),
        "changing --local_resources re-analyzed the targets:\n{output}"
    );
    assert!(
        !output.contains("Discarding analysis cache"),
        "changing --local_resources discarded the analysis cache:\n{output}"
    );

    let released = dx(&case, &scratch, &slot, &["worktree", "release", "--force"]);
    ok(&released, "release");
}
