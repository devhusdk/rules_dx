//! Builds a release candidate archive from the working tree and consumes it through a local
//! registry in a nested Bazel workspace, with no override.

use std::path::{Path, PathBuf};
use std::process::Command;

use dx_nested::{self as nested, Case};
use dx_release_tools::candidate::{
    build_source_archive, source_archive_error, stage_local_registry, Candidate,
};
use dx_testing::{mkscratch, resolve_runfiles, rules_dx_source, Run};

const VERSION: &str = "1.2.3-rc1";

const BUILD: &str = r#"load("@my_dx//quality:policy.bzl", "quality_family", "workspace_policy")
load("@my_dx//rust/rules:defs.bzl", "rust_binary", "rust_test")

rust_binary(
    name = "app",
    srcs = ["main.rs"],
)

rust_test(
    name = "app_test",
    srcs = ["app_test.rs"],
)

quality_family(
    name = "rust_family",
    family_id = "rust",
    format = ["rustfmt"],
    lint = ["clippy"],
    typecheck = ["rustc"],
)

workspace_policy(
    name = "policy",
    families = [":rust_family"],
)
"#;

const DX_BUILD: &str = r#"alias(
    name = "generate",
    actual = "@my_dx//dx:generate",
    visibility = ["//visibility:public"],
)

alias(
    name = "generate_check",
    actual = "@my_dx//dx:generate_check",
    visibility = ["//visibility:public"],
)
"#;

fn write(path: &Path, bytes: &[u8]) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap_or_else(|error| panic!("mkdir {parent:?}: {error}"));
    }
    std::fs::write(path, bytes).unwrap_or_else(|error| panic!("write {path:?}: {error}"));
}

/// Runs git with a fixed identity and no user or system config.
fn git(dir: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args([
            "-c",
            "commit.gpgsign=false",
            "-c",
            "user.name=candidate",
            "-c",
            "user.email=candidate@example.com",
        ])
        .args(args)
        .current_dir(dir)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        // Keep `git status` from refreshing and rewriting the checkout's index.
        .env("GIT_OPTIONAL_LOCKS", "0")
        .output()
        .unwrap_or_else(|error| panic!("git {args:?}: {error}"));
    assert!(
        output.status.success(),
        "git {args:?} in {dir:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

/// What the checkout looks like to git, to prove the snapshot leaves it alone.
#[derive(Debug, PartialEq, Eq)]
struct RepoState {
    head: String,
    branch: String,
    status: String,
    index: (u64, Option<std::time::SystemTime>),
}

fn repo_state(root: &Path) -> RepoState {
    let index = git(
        root,
        &["rev-parse", "--path-format=absolute", "--git-path", "index"],
    );
    let meta = std::fs::metadata(&index).ok();
    RepoState {
        head: git(root, &["rev-parse", "HEAD"]),
        // Prints `HEAD` on a detached checkout instead of failing.
        branch: git(root, &["rev-parse", "--abbrev-ref", "HEAD"]),
        status: git(root, &["--no-optional-locks", "status", "--porcelain=v1"]),
        index: (
            meta.as_ref().map_or(0, |meta| meta.len()),
            meta.and_then(|meta| meta.modified().ok()),
        ),
    }
}

/// Commits the checkout's working tree in a scratch git dir that borrows the real objects.
/// Returns the scratch git dir and the commit.
fn snapshot_commit(root: &Path, scratch: &Path) -> (PathBuf, String) {
    let common = git(
        root,
        &["rev-parse", "--path-format=absolute", "--git-common-dir"],
    );
    let git_dir = scratch.join("snapshot.git");
    std::fs::create_dir_all(&git_dir).unwrap_or_else(|error| panic!("mkdir: {error}"));
    git(&git_dir, &["init", "--bare", "--quiet", "."]);
    write(
        &git_dir.join("objects/info/alternates"),
        format!("{common}/objects\n").as_bytes(),
    );
    let exclude = Path::new(&common).join("info/exclude");
    if let Ok(bytes) = std::fs::read(&exclude) {
        write(&git_dir.join("info/exclude"), &bytes);
    }
    let tree_root = root.to_string_lossy().into_owned();
    git(&git_dir, &["config", "core.bare", "false"]);
    git(&git_dir, &["config", "core.worktree", tree_root.as_str()]);
    let parent = git(root, &["rev-parse", "HEAD"]);
    git(&git_dir, &["read-tree", parent.as_str()]);
    git(&git_dir, &["add", "-A"]);
    let tree = git(&git_dir, &["write-tree"]);
    let sha = git(
        &git_dir,
        &[
            "commit-tree",
            tree.as_str(),
            "-p",
            parent.as_str(),
            "-m",
            "candidate",
        ],
    );
    (git_dir, sha)
}

/// Stages a registry for the archive and returns its directory.
fn stage_registry(candidate: &Candidate, archive: &Path) -> PathBuf {
    let dir = mkscratch("candidate-registry").unwrap_or_else(|error| panic!("scratch: {error}"));
    stage_local_registry(&dir, candidate, archive).unwrap_or_else(|error| panic!("{error}"))
}

/// Writes a consumer that reaches rules_dx only through the staged registry.
fn consumer(candidate: &Candidate, archive: &Path, registry: &Path) -> Case {
    let prefix = candidate.strip_prefix();
    let member = |rel: &str| {
        dx_deploy_tools::archive_read_member(archive, &format!("{prefix}/{rel}"))
            .unwrap_or_else(|error| panic!("archive {rel}: {error}"))
    };
    let case = nested::case("candidate_consumer");
    let root = case.root().to_path_buf();
    write(&root.join(".bazelversion"), &member(".bazelversion"));
    let mut rc = member("tools/bazelrc/preset.bazelrc");
    rc.extend_from_slice(
        format!(
            "\ncommon --registry=file://{}\ncommon --registry=https://bcr.bazel.build\n",
            registry.display()
        )
        .as_bytes(),
    );
    write(&root.join(".bazelrc"), &rc);
    let module = format!(
        "module(name = \"candidate_consumer\")\n\nbazel_dep(name = \"rules_dx\", version = \"{}\", repo_name = \"my_dx\")\n",
        candidate.version()
    );
    assert!(!module.contains("override"), "{module}");
    write(&root.join("MODULE.bazel"), module.as_bytes());
    write(&root.join("BUILD.bazel"), BUILD.as_bytes());
    write(&root.join("main.rs"), b"fn main() {}\n");
    write(&root.join("app_test.rs"), b"#[test]\nfn passes() {}\n");
    write(&root.join("dx/BUILD.bazel"), DX_BUILD.as_bytes());
    case
}

fn dx(case: &Case, command: &str, args: &[&str]) -> Run {
    let rel = std::env::var("DX_BIN").unwrap_or_else(|_| panic!("DX_BIN must name dx"));
    let workspace = case.root().to_string_lossy().into_owned();
    let home = mkscratch("candidate-home").unwrap_or_else(|error| panic!("scratch: {error}"));
    let home = home.to_string_lossy().into_owned();
    let mut argv: Vec<&str> = vec![command, "--workspace", workspace.as_str()];
    argv.extend_from_slice(args);
    case.run(
        &resolve_runfiles(&rel),
        &argv,
        &[("HOME", home.as_str()), ("PATH", "/usr/bin:/bin")],
    )
}

fn ok(run: &Run, what: &str) {
    assert_eq!(run.status.code(), Some(0), "{what}: {}", run.combined());
}

/// Consumes the archive through the registry and checks what Bazel fetched.
fn assert_registry_consumer(candidate: &Candidate, archive: &Path, built_integrity: &str) {
    let registry = stage_registry(candidate, archive);
    let case = consumer(candidate, archive, &registry);

    ok(&case.bazel(&["build", "//..."]), "build");
    ok(&case.bazel(&["test", "//..."]), "test");

    let explain = case.bazel(&["mod", "explain", "rules_dx"]);
    ok(&explain, "mod explain");
    assert!(
        explain.stdout.contains(&format!("rules_dx@{VERSION}")),
        "{}",
        explain.combined()
    );

    let repo = case.bazel(&["mod", "show_repo", "@my_dx"]);
    ok(&repo, "mod show_repo");
    let shown = repo.stdout.clone();
    let mirror = format!("file://{}/mirror/github.com/", registry.display());
    for want in [
        built_integrity.to_owned(),
        candidate.strip_prefix(),
        mirror,
        format!(
            "{}/modules/rules_dx/{VERSION}/MODULE.bazel",
            registry.display()
        ),
    ] {
        assert!(shown.contains(&want), "show_repo lacks {want:?}: {shown}");
    }

    let base = case.bazel(&["info", "output_base"]);
    ok(&base, "info output_base");
    let fetched = Path::new(base.stdout.trim()).join("external/rules_dx+/MODULE.bazel");
    let staged = registry.join(format!("modules/rules_dx/{VERSION}/MODULE.bazel"));
    assert_eq!(
        std::fs::read(&fetched).unwrap_or_else(|error| panic!("{fetched:?}: {error}")),
        std::fs::read(&staged).unwrap_or_else(|error| panic!("{staged:?}: {error}")),
        "the fetched MODULE.bazel is not the staged registry file"
    );

    let policy = "--@my_dx//config:workspace=//:policy";
    ok(&dx(&case, "check", &["//...", "--", policy]), "dx check");
}

#[test]
fn a_candidate_archive_builds_tests_and_checks_a_registry_consumer() {
    let root = rules_dx_source();
    let before = repo_state(&root);
    let scratch =
        mkscratch("candidate-snapshot").unwrap_or_else(|error| panic!("scratch: {error}"));
    let (git_dir, sha) = snapshot_commit(&root, &scratch);
    assert_eq!(
        repo_state(&root),
        before,
        "the snapshot touched the checkout"
    );

    let candidate = Candidate::new(VERSION, &sha).unwrap_or_else(|error| panic!("{error}"));
    let archive = scratch.join("rules_dx.tar.gz");
    let built = build_source_archive(&git_dir, &candidate, &archive)
        .unwrap_or_else(|error| panic!("{error}"));
    source_archive_error(&archive, &git_dir, &candidate).unwrap_or_else(|error| panic!("{error}"));

    assert_registry_consumer(&candidate, &archive, &built.integrity);
    assert_eq!(repo_state(&root), before, "the test touched the checkout");
}
