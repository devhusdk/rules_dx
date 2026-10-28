use dx_nested::{self as nested, Case};
use dx_test_scratch::scratch;
use dx_testing::{read, resolve_runfiles, rules_dx_source, write, Run};

const APP: &str = "defaults: &defaults\n  retries: 3\n  region: eu\nservice:\n  settings: *defaults\n  name: api\n---\nsecond: 2\n";

const EXTRA: &str = "list:\n  - a\n  - b\n";

const DIRTY: &str = "list:   [a,b]\nname: api\n";

const FORMATTED_DIRTY: &str = "list: [a, b]\nname: api\n";

const UNOWNED: &str = "key:    value\nother: [1,2]\n";

const TRAILING: &str = "name: api   \n";

const BROKEN: &str = "key: [unclosed\n";

const INDENT4: &str = "formatter:\n  indent: 4\n";

const LINT_DEFAULT: &str = "extends: default\nrules:\n  document-start: disable\n";

const LINT_RELAXED: &str =
    "extends: default\nrules:\n  document-start: disable\n  trailing-spaces: disable\n";

const YAMLFMT_REPO: &str = "--repo=@@rules_dx++dx_tools+dx_yamlfmt_linux_x86_64";

const CLOSURE_REPOS: [&str; 5] = [
    "--repo=@@aspect_rules_py++uv+whl__yamllint__fc394a5b3be980a4",
    "--repo=@@aspect_rules_py++uv+whl__pyyaml__ba1cc08a7ccde2d2",
    "--repo=@@aspect_rules_py++uv+whl__pathspec__a00ce642f577bf7f",
    "--repo=@@aspect_rules_py++python_interpreters+python_3_12_x86_64_unknown_linux_gnu",
    "--repo=@@aspect_rules_py++http_file+toml2json_linux_amd64",
];

const MODULE: &str = r#"module(name = "consumer")

bazel_dep(name = "rules_dx", version = "0.0.0")
local_path_override(module_name = "rules_dx", path = "RULES_DX")

bazel_dep(name = "rules_rust", version = "0.75.0")
bazel_dep(name = "rules_python", version = "1.9.0")
bazel_dep(name = "aspect_rules_py", version = "2.0.0-alpha.6")

python_interpreters = use_extension("@aspect_rules_py//py:extensions.bzl", "python_interpreters")
python_interpreters.toolchain(python_version = "3.12")
use_repo(python_interpreters, "python_interpreters")

register_toolchains("@python_interpreters//:all")

python = use_extension("@rules_python//python/extensions:python.bzl", "python")
python.toolchain(python_version = "3.12")
use_repo(python, "python_3_12")
"#;

const BUILD: &str = r#"load("@rules_dx//quality:native_config.bzl", "yamlfmt_config", "yamllint_config")
load("@rules_dx//quality:policy.bzl", "quality_family", "workspace_policy")

quality_family(
    name = "yaml_family",
    family_id = "yaml",
    disabled = [
        "audit",
        "typecheck",
    ],
    format = ["yamlfmt"],
    lint = ["yamllint"],
)

workspace_policy(
    name = "policy",
    families = [":yaml_family"],
)

yamlfmt_config(
    name = "indent4",
    src = "indent4.yamlfmt",
    visibility = ["//visibility:public"],
)

yamllint_config(
    name = "lint_default",
    src = "default.yamllint",
    visibility = ["//visibility:public"],
)

yamllint_config(
    name = "lint_relaxed",
    src = "relaxed.yamllint",
    visibility = ["//visibility:public"],
)
"#;

const APP_BUILD: &str = r#"load("@rules_dx//yaml/rules:defs.bzl", "yaml_library")

yaml_library(
    name = "app",
    aspect_hints = HINTS,
    srcs = [
        "app.yaml",
        "extra.yml",
    ],
    deps = ["//lib:base"],
)
"#;

const LIB_BUILD: &str = r#"load("@rules_dx//yaml/rules:defs.bzl", "yaml_library")

yaml_library(
    name = "base",
    aspect_hints = ["//:lint_default"],
    srcs = ["base.yaml"],
    visibility = ["//visibility:public"],
)
"#;

const BASE: &str = "base: 1\n";

const HOST_CONFIG: &str =
    "extends: default\nrules:\n  document-start: disable\n  trailing-spaces: disable\n";

fn set_hints(root: &Case, hints: &str) {
    write(
        &root.join("app/BUILD.bazel"),
        &APP_BUILD.replace("HINTS", hints),
    );
}

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
    write(&root.join("indent4.yamlfmt"), INDENT4);
    write(&root.join("default.yamllint"), LINT_DEFAULT);
    write(&root.join("relaxed.yamllint"), LINT_RELAXED);
    set_hints(&root, "[\"//:lint_default\"]");
    write(&root.join("app/app.yaml"), APP);
    write(&root.join("app/extra.yml"), EXTRA);
    write(&root.join("lib/BUILD.bazel"), LIB_BUILD);
    write(&root.join("lib/base.yaml"), BASE);
    write(&root.join("docs/unowned.yaml"), UNOWNED);
    root
}

fn dx_with(root: &Case, command: &str, args: &[&str], flags: &[&str]) -> Run {
    let rel = std::env::var("DX_BIN").unwrap_or_else(|_| panic!("DX_BIN must name dx"));
    let workspace = root.to_string_lossy().into_owned();
    let home_dir = scratch("yaml-home");
    let home = home_dir.path().to_string_lossy().into_owned();
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
fn format_check_keeps_bytes_apply_edits_owned_files_and_recheck_passes() {
    let root = consumer();
    let clean = dx(&root, "format", &[]);
    assert_eq!(clean.status.code(), Some(0), "{}", clean.combined());

    let extra = root.join("app/extra.yml");
    write(&extra, DIRTY);

    let dirty = dx(&root, "format", &[]);
    assert_eq!(dirty.status.code(), Some(1), "{}", dirty.combined());
    assert!(
        dirty.combined().contains("app/extra.yml"),
        "{}",
        dirty.combined()
    );
    assert!(
        !dirty.combined().contains("docs/unowned.yaml"),
        "{}",
        dirty.combined()
    );
    assert_eq!(read(&extra), DIRTY);
    assert_eq!(read(&root.join("app/app.yaml")), APP);
    assert_eq!(read(&root.join("docs/unowned.yaml")), UNOWNED);

    let applied = dx(&root, "format", &["--apply"]);
    assert_eq!(applied.status.code(), Some(0), "{}", applied.combined());
    assert_eq!(read(&extra), FORMATTED_DIRTY);
    assert_eq!(read(&root.join("app/app.yaml")), APP);
    assert_eq!(read(&root.join("docs/unowned.yaml")), UNOWNED);

    let again = dx(&root, "format", &[]);
    assert_eq!(again.status.code(), Some(0), "{}", again.combined());
}

#[test]
fn multiple_documents_and_anchors_pass_both_tools() {
    let root = consumer();
    let format = dx(&root, "format", &[]);
    assert_eq!(format.status.code(), Some(0), "{}", format.combined());
    let lint = dx(&root, "lint", &[]);
    assert_eq!(lint.status.code(), Some(0), "{}", lint.combined());
    assert_eq!(read(&root.join("app/app.yaml")), APP);
}

#[test]
fn a_bound_yamlfmt_config_changes_the_formatting() {
    let root = consumer();
    set_hints(&root, "[\"//:lint_default\", \"//:indent4\"]");
    let dirty = dx(&root, "format", &[]);
    assert_eq!(dirty.status.code(), Some(1), "{}", dirty.combined());
    assert!(
        dirty.combined().contains("app/app.yaml"),
        "{}",
        dirty.combined()
    );
    assert_eq!(read(&root.join("app/app.yaml")), APP);

    let applied = dx(&root, "format", &["--apply"]);
    assert_eq!(applied.status.code(), Some(0), "{}", applied.combined());
    assert!(
        read(&root.join("app/app.yaml")).contains("\n    retries: 3\n"),
        "{}",
        read(&root.join("app/app.yaml"))
    );
    let again = dx(&root, "format", &[]);
    assert_eq!(again.status.code(), Some(0), "{}", again.combined());
}

#[test]
fn lint_reports_rules_and_a_bound_config_changes_them() {
    let root = consumer();
    let extra = root.join("app/extra.yml");
    write(&extra, TRAILING);

    let dirty = dx(&root, "lint", &[]);
    assert_eq!(dirty.status.code(), Some(1), "{}", dirty.combined());
    assert!(
        dirty.combined().contains("app/extra.yml:"),
        "{}",
        dirty.combined()
    );
    assert!(
        dirty.combined().contains("trailing-spaces"),
        "{}",
        dirty.combined()
    );
    assert_eq!(read(&extra), TRAILING);

    set_hints(&root, "[\"//:lint_relaxed\"]");
    let relaxed = dx(&root, "lint", &[]);
    assert_eq!(relaxed.status.code(), Some(0), "{}", relaxed.combined());
}

#[test]
fn lint_is_check_only_and_never_edits_files() {
    let root = consumer();
    let extra = root.join("app/extra.yml");
    write(&extra, TRAILING);
    let out = dx(&root, "lint", &["--apply"]);
    assert_ne!(out.status.code(), Some(0), "{}", out.combined());
    assert!(
        out.combined().contains("trailing-spaces") || out.combined().contains("check-only"),
        "{}",
        out.combined()
    );
    assert_eq!(read(&extra), TRAILING);
}

#[test]
fn a_parse_error_is_reported_by_both_tools_and_never_clean() {
    let root = consumer();
    let extra = root.join("app/extra.yml");
    write(&extra, BROKEN);

    for command in ["format", "lint"] {
        let out = dx(&root, command, &[]);
        let text = out.combined();
        assert_eq!(out.status.code(), Some(1), "{command}: {text}");
        assert!(text.contains("app/extra.yml"), "{command}: {text}");
        assert_eq!(read(&extra), BROKEN);
    }
    let applied = dx(&root, "format", &["--apply"]);
    assert_ne!(applied.status.code(), Some(0), "{}", applied.combined());
    assert_eq!(read(&extra), BROKEN);
}

#[test]
fn lint_includes_dependency_libraries() {
    let root = consumer();
    let base = root.join("lib/base.yaml");
    write(&base, "base: 1   \n");
    let out = dx(&root, "lint", &[]);
    assert_eq!(out.status.code(), Some(1), "{}", out.combined());
    assert!(
        out.combined().contains("lib/base.yaml:"),
        "{}",
        out.combined()
    );
    assert!(
        out.combined().contains("trailing-spaces"),
        "{}",
        out.combined()
    );
    assert_eq!(read(&base), "base: 1   \n");
}

#[test]
fn lint_without_a_bound_config_ignores_host_configs() {
    let root = consumer();
    set_hints(&root, "[]");
    write(&root.join(".yamllint"), HOST_CONFIG);
    let extra = root.join("app/extra.yml");
    write(&extra, TRAILING);
    let out = dx(&root, "lint", &[]);
    assert_eq!(out.status.code(), Some(1), "{}", out.combined());
    assert!(
        out.combined().contains("trailing-spaces"),
        "{}",
        out.combined()
    );
}

#[test]
fn the_declared_yamlfmt_and_yamllint_are_acquired_from_the_cache_with_the_network_denied() {
    let root = consumer();
    let fresh = root.bazel(&["clean", "--expunge"]);
    assert!(fresh.status.success(), "{}", fresh.combined());
    let warm = dx(&root, "format", &[]);
    assert_eq!(warm.status.code(), Some(0), "{}", warm.combined());
    let warm = dx(&root, "lint", &[]);
    assert_eq!(warm.status.code(), Some(0), "{}", warm.combined());

    let scratch_dir = scratch("yaml-denied");
    let scratch = scratch_dir.path();
    let denied = scratch.join("downloader.cfg");
    write(&denied, "allow bcr.bazel.build\nblock *\n");
    let denied_flag = format!("--downloader_config={}", denied.display());

    write(&root.join("app/extra.yml"), DIRTY);
    for command in ["format", "lint"] {
        let built = dx_with(&root, command, &[], &[denied_flag.as_str()]);
        assert_eq!(built.status.code(), Some(1), "{}", built.combined());
        assert!(
            built.combined().contains("app/extra.yml"),
            "{}",
            built.combined()
        );
    }

    let lock = read(&root.join("MODULE.bazel.lock"));
    assert!(
        lock.contains("py:extensions.bzl%python_interpreters"),
        "{lock}"
    );

    let expunged = root.bazel(&["clean", "--expunge"]);
    assert!(expunged.status.success(), "{}", expunged.combined());
    let mut fetch: Vec<&str> = vec!["fetch", YAMLFMT_REPO];
    fetch.extend(CLOSURE_REPOS);
    fetch.push(denied_flag.as_str());
    let offline = root.bazel(&fetch);
    assert!(offline.status.success(), "{}", offline.combined());

    let empty = scratch.join("empty-cache");
    let cache_flag = format!("--repository_cache={}", empty.display());
    let expunged = root.bazel(&["clean", "--expunge"]);
    assert!(expunged.status.success(), "{}", expunged.combined());
    let miss = root.bazel(&[
        "fetch",
        YAMLFMT_REPO,
        cache_flag.as_str(),
        denied_flag.as_str(),
    ]);
    assert_ne!(miss.status.code(), Some(0), "{}", miss.combined());
    assert!(
        miss.combined().contains("yamlfmt_0.21.0_Linux_x86_64"),
        "{}",
        miss.combined()
    );

    let expunged = root.bazel(&["clean", "--expunge"]);
    assert!(expunged.status.success(), "{}", expunged.combined());
    let mut cold: Vec<&str> = vec!["fetch"];
    cold.extend(CLOSURE_REPOS);
    cold.push(cache_flag.as_str());
    cold.push(denied_flag.as_str());
    let miss = root.bazel(&cold);
    assert_ne!(miss.status.code(), Some(0), "{}", miss.combined());
    assert!(
        miss.combined().contains("Cache miss"),
        "{}",
        miss.combined()
    );
}
