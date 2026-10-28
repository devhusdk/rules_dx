use dx_nested::{self as nested, Case};
use dx_testing::{mkscratch, read, resolve_runfiles, rules_dx_source, write, Run};

const CLEAN: &str = "FROM scratch\nCOPY hello /hello\nENTRYPOINT [\"/hello\"]\n";

// DL3006: untagged base image.
const DIRTY: &str = "FROM ubuntu\nCOPY hello /hello\n";

const HADOLINT_REPO: &str = "--repo=@@rules_dx++dx_tools+dx_hadolint_linux_x86_64";

const MODULE: &str = r#"module(name = "consumer")

bazel_dep(name = "rules_dx", version = "0.0.0")
local_path_override(module_name = "rules_dx", path = "RULES_DX")

bazel_dep(name = "rules_rust", version = "0.74.0")
"#;

const BUILD: &str = r#"load("@rules_dx//quality:policy.bzl", "quality_family", "workspace_policy")

quality_family(
    name = "dockerfile_family",
    family_id = "dockerfile",
    disabled = [
        "audit",
        "typecheck",
        "format",
    ],
    lint = ["hadolint"],
)

workspace_policy(
    name = "policy",
    families = [":dockerfile_family"],
)
"#;

const APP_BUILD: &str = r#"load("@rules_dx//dockerfile/rules:defs.bzl", "dockerfile_library")

dockerfile_library(
    name = "app",
    srcs = [
        "Dockerfile",
        "app.Dockerfile",
    ],
)
"#;

fn consumer() -> Case {
    let rules = rules_dx_source();
    let root = nested::case("consumer");
    write(
        &root.join(".bazelversion"),
        &read(&rules.join(".bazelversion")),
    );
    write(
        &root.join("MODULE.bazel"),
        &MODULE.replace("RULES_DX", &rules.display().to_string()),
    );
    write(&root.join(".bazelrc"), "common --enable_bzlmod\n");
    write(&root.join("BUILD.bazel"), BUILD);
    write(&root.join("app/BUILD.bazel"), APP_BUILD);
    write(&root.join("app/Dockerfile"), CLEAN);
    write(&root.join("app/app.Dockerfile"), CLEAN);
    write(&root.join("docs/Dockerfile"), DIRTY);
    root
}

fn dx_with(root: &Case, command: &str, args: &[&str], flags: &[&str]) -> Run {
    let rel = std::env::var("DX_BIN").unwrap_or_else(|_| panic!("DX_BIN must name dx"));
    let workspace = root.to_string_lossy().into_owned();
    let home = mkscratch("dockerfile-home").unwrap_or_else(|error| panic!("scratch: {error}"));
    let home = home.to_string_lossy().into_owned();
    let mut argv: Vec<&str> = vec![command];
    argv.extend_from_slice(args);
    argv.extend_from_slice(&[
        "--workspace",
        workspace.as_str(),
        "//...",
        "--",
        "--@rules_dx//config:workspace=//:policy",
    ]);
    argv.extend_from_slice(flags);
    root.run(
        &resolve_runfiles(&rel),
        &argv,
        &[("HOME", home.as_str()), ("PATH", "/usr/bin:/bin")],
    )
}

fn dx(root: &Case, command: &str, args: &[&str]) -> Run {
    dx_with(root, command, args, &[])
}

#[test]
fn clean_dockerfiles_pass_and_unowned_ones_are_ignored() {
    let root = consumer();
    let out = dx(&root, "lint", &[]);
    assert_eq!(out.status.code(), Some(0), "{}", out.combined());
    assert_eq!(read(&root.join("docs/Dockerfile")), DIRTY);
}

#[test]
fn lint_reports_rules_for_owned_dockerfiles_and_never_edits() {
    let root = consumer();
    for name in ["app/Dockerfile", "app/app.Dockerfile"] {
        write(&root.join(name), DIRTY);
        let out = dx(&root, "lint", &[]);
        assert_eq!(out.status.code(), Some(1), "{name}: {}", out.combined());
        assert!(out.combined().contains(name), "{name}: {}", out.combined());
        assert!(out.combined().contains("DL3006"), "{}", out.combined());
        assert!(
            !out.combined().contains("docs/Dockerfile"),
            "{}",
            out.combined()
        );
        assert_eq!(read(&root.join(name)), DIRTY);
        write(&root.join(name), CLEAN);
    }
}

#[test]
fn lint_is_check_only_and_never_edits_files() {
    let root = consumer();
    let file = root.join("app/Dockerfile");
    write(&file, DIRTY);
    let out = dx(&root, "lint", &["--apply"]);
    assert_ne!(out.status.code(), Some(0), "{}", out.combined());
    assert_eq!(read(&file), DIRTY);
}

#[test]
fn the_declared_hadolint_is_acquired_from_the_cache_with_the_network_denied() {
    let root = consumer();
    let fresh = root.bazel(&["clean", "--expunge"]);
    assert!(fresh.status.success(), "{}", fresh.combined());
    let warm = dx(&root, "lint", &[]);
    assert_eq!(warm.status.code(), Some(0), "{}", warm.combined());

    let scratch = mkscratch("dockerfile-denied").unwrap_or_else(|error| panic!("scratch: {error}"));
    let denied = scratch.join("downloader.cfg");
    write(&denied, "allow bcr.bazel.build\nblock *\n");
    let denied_flag = format!("--downloader_config={}", denied.display());

    write(&root.join("app/Dockerfile"), DIRTY);
    let built = dx_with(&root, "lint", &[], &[denied_flag.as_str()]);
    assert_eq!(built.status.code(), Some(1), "{}", built.combined());
    assert!(
        built.combined().contains("app/Dockerfile"),
        "{}",
        built.combined()
    );

    let expunged = root.bazel(&["clean", "--expunge"]);
    assert!(expunged.status.success(), "{}", expunged.combined());
    let offline = root.bazel(&["fetch", HADOLINT_REPO, denied_flag.as_str()]);
    assert!(offline.status.success(), "{}", offline.combined());

    let empty = scratch.join("empty-cache");
    let cache_flag = format!("--repository_cache={}", empty.display());
    let expunged = root.bazel(&["clean", "--expunge"]);
    assert!(expunged.status.success(), "{}", expunged.combined());
    let miss = root.bazel(&[
        "fetch",
        HADOLINT_REPO,
        cache_flag.as_str(),
        denied_flag.as_str(),
    ]);
    assert_ne!(miss.status.code(), Some(0), "{}", miss.combined());
    assert!(
        miss.combined().contains("hadolint-linux-x86_64"),
        "{}",
        miss.combined()
    );
}
