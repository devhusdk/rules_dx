//! Builds a release candidate archive from the working tree and consumes it through a local
//! registry in a nested Bazel workspace, with no rules_dx override.

use std::path::{Path, PathBuf};

use dx_nested::compose::{
    rules_flutter_override, rules_py_override, rules_rust_override, write_rules_py_patches,
    write_rules_rust_patches,
};
use dx_nested::{self as nested, Case};
use dx_release_tools::candidate::{
    Candidate, build_source_archive, file_url, source_archive_error, stage_local_registry,
};
use dx_testing::{Run, resolve_runfiles, rules_dx_source};

const VERSION: &str = "1.2.3-rc1";

const BUILD: &str = r#"load("@my_dx//core/quality/private:generated_policy.bzl", "dx_generated_policy")
load("@my_dx//rust:defs.bzl", "rust_binary", "rust_test")

rust_binary(
    name = "app",
    srcs = ["main.rs"],
)

rust_test(
    name = "app_test",
    srcs = ["app_test.rs"],
)

dx_generated_policy(
    name = "policy",
    defaults = False,
    families = {
        "rust": {
            "format": ["rustfmt"],
            "lint": ["clippy"],
            "typecheck": ["rustc"],
        },
    },
    tier = "standard",
)
"#;

fn write(path: &Path, bytes: &[u8]) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap_or_else(|error| panic!("mkdir {parent:?}: {error}"));
    }
    std::fs::write(path, bytes).unwrap_or_else(|error| panic!("write {path:?}: {error}"));
}

/// What the checkout looks like to git, to prove the snapshot leaves it alone.
#[derive(Debug, PartialEq, Eq)]
struct RepoState {
    head: String,
    branch: String,
    status: String,
    index: (u64, Option<std::time::SystemTime>),
}

fn has_git(root: &Path) -> bool {
    root.join(".git").exists()
}

fn repo_state(root: &Path, sandbox: &Path) -> RepoState {
    if !has_git(root) {
        return RepoState {
            head: String::new(),
            branch: String::new(),
            status: String::new(),
            index: (0, None),
        };
    }
    let git = |dir: &Path, args: &[&str]| dx_test_scratch::git(dir, sandbox, args);
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
    let git = |dir: &Path, args: &[&str]| dx_test_scratch::git(dir, scratch, args);
    let git_dir = scratch.join("snapshot.git");
    std::fs::create_dir_all(&git_dir).unwrap_or_else(|error| panic!("mkdir: {error}"));
    git(&git_dir, &["init", "--bare", "--quiet", "."]);
    let parent = has_git(root).then(|| {
        let common = git(
            root,
            &["rev-parse", "--path-format=absolute", "--git-common-dir"],
        );
        write(
            &git_dir.join("objects/info/alternates"),
            format!("{common}/objects\n").as_bytes(),
        );
        if let Ok(bytes) = std::fs::read(Path::new(&common).join("info/exclude")) {
            write(&git_dir.join("info/exclude"), &bytes);
        }
        git(root, &["rev-parse", "HEAD"])
    });
    let tree_root = root.to_string_lossy().into_owned();
    git(&git_dir, &["config", "core.bare", "false"]);
    git(&git_dir, &["config", "core.worktree", tree_root.as_str()]);
    if let Some(parent) = parent.as_deref() {
        git(&git_dir, &["read-tree", parent]);
    }
    git(&git_dir, &["add", "-A"]);
    let tree = git(&git_dir, &["write-tree"]);
    let mut args = vec!["commit-tree", tree.as_str()];
    if let Some(parent) = parent.as_deref() {
        args.extend(["-p", parent]);
    }
    args.extend(["-m", "candidate"]);
    let sha = git(&git_dir, &args);
    (git_dir, sha)
}

const GCC_BUILD: &str = r#"load("@my_dx//cc:defs.bzl", "cc_binary")

cc_binary(
    name = "app",
    srcs = ["main.cc"],
    tags = ["manual"],
)

cc_binary(
    name = "c_app",
    srcs = ["main.c"],
    tags = ["manual"],
)
"#;

/// Stages a registry for the archive and returns its guard and directory.
fn stage_registry(candidate: &Candidate, archive: &Path) -> (dx_test_scratch::TempDir, PathBuf) {
    let dir = dx_test_scratch::scratch("candidate-registry");
    let registry = stage_local_registry(dir.path(), candidate, archive)
        .unwrap_or_else(|error| panic!("{error}"));
    (dir, registry)
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
    let mut rc: Vec<u8> = dx_testing::preset_rc()
        .replace("@rules_dx//", "@my_dx//")
        .into_bytes();
    rc.extend_from_slice(
        format!(
            "\ncommon --registry={}\ncommon --registry=https://bcr.bazel.build\n",
            file_url(registry)
        )
        .as_bytes(),
    );
    rc.extend_from_slice(crate::quickstart_consumer_test::WINDOWS_LICENCES.as_bytes());
    write(&root.join(".bazelrc"), &rc);
    let module = format!(
        "module(name = \"candidate_consumer\")\n\nbazel_dep(name = \"rules_dx\", version = \"{}\", repo_name = \"my_dx\")\n{}{}{}",
        candidate.version(),
        rules_py_override(),
        rules_flutter_override(),
        rules_rust_override(),
    );
    assert!(!module.contains("module_name = \"rules_dx\""), "{module}");
    write_rules_py_patches(&root, &rules_dx_source()).unwrap_or_else(|error| panic!("{error}"));
    write_rules_rust_patches(&root, &rules_dx_source()).unwrap_or_else(|error| panic!("{error}"));
    write(&root.join("MODULE.bazel"), module.as_bytes());
    write(&root.join("BUILD.bazel"), BUILD.as_bytes());
    write(&root.join("main.rs"), b"fn main() {}\n");
    write(&root.join("app_test.rs"), b"#[test]\nfn passes() {}\n");
    write(&root.join("gcc/BUILD.bazel"), GCC_BUILD.as_bytes());
    write(
        &root.join("gcc/main.cc"),
        b"#include <iostream>\n\nint main() {\n  std::cout << \"gcc consumer\\n\";\n}\n",
    );
    write(
        &root.join("gcc/main.c"),
        b"#include <stdio.h>\n\nint main(void) {\n  puts(\"gcc c consumer\");\n  return 0;\n}\n",
    );
    case
}

fn dx(case: &Case, command: &str, args: &[&str]) -> Run {
    let rel = std::env::var("DX_BIN").unwrap_or_else(|_| panic!("DX_BIN must name dx"));
    let workspace = case.root().to_string_lossy().into_owned();
    let home_dir = dx_test_scratch::scratch("candidate-home");
    let home = home_dir.path().to_string_lossy().into_owned();
    let applets = dx_testing::env_files("DX_COREUTILS");
    let path = applets[0]
        .parent()
        .expect("applet directory")
        .to_string_lossy()
        .into_owned();
    let mut argv: Vec<&str> = vec![command, "--workspace", workspace.as_str()];
    argv.extend_from_slice(args);
    case.run(
        &resolve_runfiles(&rel),
        &argv,
        &[("HOME", home.as_str()), ("PATH", path.as_str())],
    )
}

/// Names the step about to run on the uncaptured stderr, so a test killed mid-step shows it.
fn stage(name: &str) {
    use std::io::Write;
    let _ = writeln!(std::io::stderr(), "candidate_consumer_test: {name}");
}

/// Runs nested Bazel with the `PATH` of `ldd_path`.
fn bazel(case: &Case, args: &[&str]) -> Run {
    let path = crate::quickstart_consumer_test::ldd_path();
    case.bazel_with(args, &[("PATH", path.as_str())])
}

fn ok(run: &Run, what: &str) {
    assert_eq!(run.status.code(), Some(0), "{what}: {}", run.combined());
}

/// Consumes the archive through the registry and checks what Bazel fetched.
fn assert_registry_consumer(candidate: &Candidate, archive: &Path, built_integrity: &str) {
    stage("stage registry");
    let (_registry_guard, registry) = stage_registry(candidate, archive);
    stage("nested case");
    let case = consumer(candidate, archive, &registry);

    stage("bazel build");
    ok(&bazel(&case, &["build", "//..."]), "build");
    stage("bazel test");
    ok(&bazel(&case, &["test", "//..."]), "test");
    stage("bazel mod");

    let explain = bazel(&case, &["mod", "explain", "rules_dx"]);
    ok(&explain, "mod explain");
    assert!(
        explain.stdout.contains(&format!("rules_dx@{VERSION}")),
        "{}",
        explain.combined()
    );

    let repo = bazel(&case, &["mod", "show_repo", "@my_dx"]);
    ok(&repo, "mod show_repo");
    let shown = repo.stdout.clone();
    let registry_url = file_url(&registry);
    let mirror = format!("{registry_url}/mirror/github.com/");
    for want in [
        built_integrity.to_owned(),
        candidate.strip_prefix(),
        mirror,
        format!("{registry_url}/modules/rules_dx/{VERSION}/MODULE.bazel"),
    ] {
        assert!(shown.contains(&want), "show_repo lacks {want:?}: {shown}");
    }

    stage("bazel info");
    let base = bazel(&case, &["info", "output_base"]);
    ok(&base, "info output_base");
    let fetched = Path::new(base.stdout.trim()).join("external/rules_dx+/MODULE.bazel");
    let staged = registry.join(format!("modules/rules_dx/{VERSION}/MODULE.bazel"));
    assert_eq!(
        std::fs::read(&fetched).unwrap_or_else(|error| panic!("{fetched:?}: {error}")),
        std::fs::read(&staged).unwrap_or_else(|error| panic!("{staged:?}: {error}")),
        "the fetched MODULE.bazel is not the staged registry file"
    );

    stage("dx check");
    let policy = "--@my_dx//config:workspace=//:policy";
    ok(&dx(&case, "check", &["//...", "--", policy]), "dx check");

    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        assert_gcc_consumer(&case);
        assert_swift_consumer(&case);
    }
}

/// Builds and tests `swift/tests/consumer` on the unpatched BCR rules_swift.
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn assert_swift_consumer(case: &Case) {
    stage("swift consumer");
    for file in ["BUILD.bazel", "Greet.swift", "GreetTests.swift"] {
        let text = dx_testing::read_runfiles(&format!("swift/tests/consumer/{file}"))
            .replace("@rules_dx//", "@my_dx//");
        write(&case.root().join("swift").join(file), text.as_bytes());
    }
    let module = dx_testing::read_text(&rules_dx_source().join("MODULE.bazel"));
    let version = module
        .split("bazel_dep(name = \"rules_swift\", version = \"")
        .nth(1)
        .and_then(|rest| rest.split('"').next())
        .expect("rules_swift version in MODULE.bazel");

    let explain = case.bazel(&["mod", "explain", "rules_swift"]);
    ok(&explain, "mod explain rules_swift");
    assert!(
        explain.stdout.contains(&format!("rules_swift@{version}")),
        "{}",
        explain.combined()
    );
    ok(
        &case.bazel(&["test", "//swift:greet_tests", "--test_output=errors"]),
        "swift test",
    );
}

/// Builds with the hermetic GCC toolchain while host compilers on PATH fail.
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn assert_gcc_consumer(case: &Case) {
    use std::os::unix::fs::PermissionsExt;

    let gcc_dir = dx_test_scratch::scratch("candidate-gcc-shim");
    let shim = gcc_dir.path();
    let ldd = Path::new(&crate::quickstart_consumer_test::ldd_path()).join("ldd");
    std::os::unix::fs::symlink(&ldd, shim.join("ldd"))
        .unwrap_or_else(|error| panic!("link ldd into {shim:?}: {error}"));
    for name in ["gcc", "g++", "cc", "c++"] {
        let path = shim.join(name);
        write(&path, b"#!/bin/sh\nexit 97\n");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
            .unwrap_or_else(|error| panic!("chmod {path:?}: {error}"));
    }
    let path = format!("PATH={}", shim.display());
    let repo_env = format!("--repo_env={path}");
    let action_env = format!("--action_env={path}");
    let flags = [
        "--@my_dx//cc:compiler=gcc",
        repo_env.as_str(),
        action_env.as_str(),
    ];
    let with = |head: &[&'static str], target: &'static str| -> Vec<&str> {
        head.iter().copied().chain(flags).chain([target]).collect()
    };
    let root = bazel(case, &["info", "execution_root"]);
    ok(&root, "info execution_root");

    for (target, expected) in [
        ("//gcc:app", "gcc consumer\n"),
        ("//gcc:c_app", "gcc c consumer\n"),
    ] {
        ok(&bazel(case, &with(&["build"], target)), "gcc build");
        let files = bazel(case, &with(&["cquery", "--output=files"], target));
        ok(&files, "gcc cquery");
        let binary = Path::new(root.stdout.trim()).join(files.stdout.trim());
        let bytes = std::fs::read(&binary).unwrap_or_else(|error| panic!("{binary:?}: {error}"));
        let has = |needle: &str| bytes.windows(needle.len()).any(|w| w == needle.as_bytes());
        assert!(has("GCC: ("), "{binary:?} has no GCC comment");
        assert!(has("16.2.0"), "{binary:?} is not GCC 16.2.0");
        assert!(!has("clang version"), "{binary:?} was built by clang");

        let output = std::process::Command::new(&binary)
            .env_clear()
            .env("PATH", shim)
            .output()
            .unwrap_or_else(|error| panic!("run {binary:?}: {error}"));
        assert!(
            output.status.success(),
            "{binary:?} exited {:?}",
            output.status
        );
        assert_eq!(
            String::from_utf8_lossy(&output.stdout),
            expected,
            "{binary:?}"
        );
    }
}

#[test]
fn a_candidate_archive_builds_tests_and_checks_a_registry_consumer() {
    let root = rules_dx_source();
    let scratch_dir = dx_test_scratch::scratch("candidate-snapshot");
    let scratch = scratch_dir.path();
    stage("repo state");
    let before = repo_state(&root, scratch);
    stage("snapshot commit");
    let (git_dir, sha) = snapshot_commit(&root, scratch);
    stage("repo state");
    assert_eq!(
        repo_state(&root, scratch),
        before,
        "the snapshot touched the checkout"
    );

    let candidate = Candidate::new(VERSION, &sha).unwrap_or_else(|error| panic!("{error}"));
    let archive = scratch.join("rules_dx.tar.gz");
    stage("build source archive");
    let built = build_source_archive(&git_dir, &candidate, &archive, None)
        .unwrap_or_else(|error| panic!("{error}"));
    stage("check source archive");
    source_archive_error(&archive, &git_dir, &candidate, None)
        .unwrap_or_else(|error| panic!("{error}"));

    assert_registry_consumer(&candidate, &archive, &built.integrity);
    stage("repo state");
    assert_eq!(
        repo_state(&root, scratch),
        before,
        "the test touched the checkout"
    );
}
