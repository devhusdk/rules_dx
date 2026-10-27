use std::path::{Path, PathBuf};

use dx_testing::nested::{self, Case};
use dx_testing::{mkscratch, resolve_runfiles, Run};

const SCHEMA: &str =
    "type Query {\n  user(id: ID!): User\n}\n\ntype User {\n  id: ID!\n  name: String\n}\n";

const FRAGMENTS: &str = "fragment UserFields on User {\n  id\n  name\n}\n";

const QUERY: &str = "query GetUser($id: ID!) {\n  user(id: $id) {\n    ...UserFields\n  }\n}\n";

const UNKNOWN_FIELD: &str = "query GetUser($id: ID!) {\n  user(id: $id) {\n    email\n  }\n}\n";

const UNKNOWN_TYPE: &str = "query GetUser($id: Missing!) {\n  user(id: $id) {\n    id\n  }\n}\n";

const UNKNOWN_FRAGMENT: &str =
    "query GetUser($id: ID!) {\n  user(id: $id) {\n    ...Missing\n  }\n}\n";

const BROKEN: &str = "query GetUser($id: ID!) {\n  user(id: $id {\n    id\n  }\n}\n";

const UNFORMATTED: &str = "query GetUser($id: ID!) { user(id: $id) { ...UserFields } }\n";

const CONFIG: &str = r#"{
  "schema": "api/schema.graphql",
  "documents": ["api/fragments/fragments.graphql"],
  "rules": {
    "schema": {
      "@graphql-eslint/known-type-names": "error"
    },
    "operations": {
      "@graphql-eslint/fields-on-correct-type": "error",
      "@graphql-eslint/known-fragment-names": "error",
      "@graphql-eslint/known-type-names": "error"
    }
  }
}
"#;

const UNKNOWN_KEY_CONFIG: &str =
    "{\n  \"schema\": \"api/schema.graphql\",\n  \"extends\": \"recommended\"\n}\n";

const CLOSURE_REPOS: [&str; 4] = [
    "--repo=@@aspect_rules_js++npm+npm_tools__at_graphql-eslint_eslint-plugin__4.4.2",
    "--repo=@@aspect_rules_js++npm+npm_tools__eslint__10.10.0",
    "--repo=@@aspect_rules_js++npm+npm_tools__graphql__16.14.2",
    "--repo=@@rules_nodejs++node+nodejs_linux_amd64",
];

const MODULE: &str = r#"module(name = "consumer")

bazel_dep(name = "rules_dx", version = "0.0.0")
local_path_override(module_name = "rules_dx", path = "RULES_DX")
"#;

const BUILD: &str = r#"load("@rules_dx//quality:native_config.bzl", "graphql_eslint_config")
load("@rules_dx//quality:policy.bzl", "quality_family", "workspace_policy")

graphql_eslint_config(
    name = "graphql_lint",
    src = "graphql_lint.json",
    data = [
        "//api:schema.graphql",
        "//api/fragments:fragments.graphql",
    ],
    visibility = ["//visibility:public"],
)

quality_family(
    name = "graphql_family",
    disabled = ["typecheck"],
    family_id = "graphql",
    format = ["prettier"],
    lint = ["graphql_eslint"],
)

workspace_policy(
    name = "policy",
    families = [":graphql_family"],
)
"#;

const API_BUILD: &str = r#"load("@rules_dx//javascript/rules:defs.bzl", "javascript_library")

exports_files(["schema.graphql"])

javascript_library(
    name = "api",
    srcs = [
        "query.graphql",
        "schema.graphql",
    ],
    aspect_hints = ["//:graphql_lint"],
    deps = ["//api/fragments"],
)
"#;

const FRAGMENTS_BUILD: &str = r#"load("@rules_dx//javascript/rules:defs.bzl", "javascript_library")

exports_files(["fragments.graphql"])

javascript_library(
    name = "fragments",
    srcs = ["fragments.graphql"],
    aspect_hints = ["//:graphql_lint"],
    visibility = ["//visibility:public"],
)
"#;

const BARE_BUILD: &str = r#"load("@rules_dx//javascript/rules:defs.bzl", "javascript_library")

javascript_library(
    name = "bare",
    srcs = ["bare.graphql"],
)
"#;

fn write(path: &Path, text: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap_or_else(|error| panic!("mkdir: {error}"));
    }
    std::fs::write(path, text).unwrap_or_else(|error| panic!("write {path:?}: {error}"));
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|error| panic!("read {path:?}: {error}"))
}

fn rules_dx_root() -> PathBuf {
    let marker = resolve_runfiles("quality/tests/graphql_eslint_consumer_test.rs");
    let real = std::fs::canonicalize(&marker).unwrap_or_else(|error| panic!("{marker:?}: {error}"));
    real.ancestors()
        .nth(3)
        .unwrap_or_else(|| panic!("no workspace above {real:?}"))
        .to_path_buf()
}

fn consumer() -> Case {
    let rules = rules_dx_root();
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
    write(&root.join("graphql_lint.json"), CONFIG);
    write(&root.join("api/BUILD.bazel"), API_BUILD);
    write(&root.join("api/schema.graphql"), SCHEMA);
    write(&root.join("api/query.graphql"), QUERY);
    write(&root.join("api/fragments/BUILD.bazel"), FRAGMENTS_BUILD);
    write(&root.join("api/fragments/fragments.graphql"), FRAGMENTS);
    root
}

fn dx(root: &Case, command: &str, args: &[&str]) -> Run {
    let rel = std::env::var("DX_BIN").unwrap_or_else(|_| panic!("DX_BIN must name dx"));
    let workspace = root.to_string_lossy().into_owned();
    let home = mkscratch("graphql-eslint-home").unwrap_or_else(|error| panic!("scratch: {error}"));
    let home = home.to_string_lossy().into_owned();
    let mut argv: Vec<&str> = vec![command];
    argv.extend_from_slice(args);
    argv.extend_from_slice(&["--workspace", workspace.as_str(), "//...", "--"]);
    argv.push("--@rules_dx//config:workspace=//:policy");
    root.run(
        &resolve_runfiles(&rel),
        &argv,
        &[("HOME", home.as_str()), ("PATH", "/usr/bin:/bin")],
    )
}

#[test]
fn a_clean_schema_and_operations_pass() {
    let root = consumer();
    let clean = dx(&root, "lint", &[]);
    assert_eq!(clean.status.code(), Some(0), "{}", clean.combined());
}

#[test]
fn unknown_fields_types_and_fragments_are_reported_with_position() {
    let root = consumer();
    let query = root.join("api/query.graphql");
    for (text, at, rule) in [
        (
            UNKNOWN_FIELD,
            "api/query.graphql:3:5",
            "fields-on-correct-type",
        ),
        (UNKNOWN_TYPE, "api/query.graphql:1:20", "known-type-names"),
        (
            UNKNOWN_FRAGMENT,
            "api/query.graphql:3:8",
            "known-fragment-names",
        ),
    ] {
        write(&query, text);
        let out = dx(&root, "lint", &[]);
        assert_eq!(out.status.code(), Some(1), "{rule}: {}", out.combined());
        assert!(out.combined().contains(at), "{rule}: {}", out.combined());
        assert!(out.combined().contains(rule), "{rule}: {}", out.combined());
    }
}

#[test]
fn a_syntax_error_and_an_invalid_config_are_never_clean() {
    let root = consumer();
    write(&root.join("api/query.graphql"), BROKEN);
    let broken = dx(&root, "lint", &[]);
    assert_ne!(broken.status.code(), Some(0), "{}", broken.combined());
    assert!(
        broken.combined().contains("api/query.graphql:2:"),
        "{}",
        broken.combined()
    );

    write(&root.join("api/query.graphql"), QUERY);
    write(&root.join("graphql_lint.json"), UNKNOWN_KEY_CONFIG);
    let invalid = dx(&root, "lint", &[]);
    assert_ne!(invalid.status.code(), Some(0), "{}", invalid.combined());
    assert!(
        invalid.combined().contains("unknown config key extends"),
        "{}",
        invalid.combined()
    );
}

#[test]
fn lint_never_rewrites_and_format_stays_with_prettier() {
    let root = consumer();
    let query = root.join("api/query.graphql");
    write(&query, UNKNOWN_FIELD);
    let fixed = dx(&root, "lint", &["--apply"]);
    assert_ne!(fixed.status.code(), Some(0), "{}", fixed.combined());
    assert_eq!(read(&query), UNKNOWN_FIELD);

    write(&query, UNFORMATTED);
    let dirty = dx(&root, "format", &["--check"]);
    assert_eq!(dirty.status.code(), Some(1), "{}", dirty.combined());
    assert_eq!(read(&query), UNFORMATTED);
    let applied = dx(&root, "format", &["--apply"]);
    assert_eq!(applied.status.code(), Some(0), "{}", applied.combined());
    assert_eq!(read(&query), QUERY);
}

#[test]
fn selecting_graphql_eslint_without_a_config_is_an_actionable_error() {
    let root = consumer();
    write(&root.join("bare/BUILD.bazel"), BARE_BUILD);
    write(&root.join("bare/bare.graphql"), QUERY);
    let out = dx(&root, "lint", &[]);
    assert_ne!(out.status.code(), Some(0), "{}", out.combined());
    assert!(
        out.combined()
            .contains("graphql-eslint requires declared config"),
        "{}",
        out.combined()
    );
}

#[test]
fn the_declared_graphql_eslint_is_acquired_from_the_cache_with_the_network_denied() {
    let root = consumer();
    let warm = dx(&root, "lint", &[]);
    assert_eq!(warm.status.code(), Some(0), "{}", warm.combined());

    let scratch =
        mkscratch("graphql-eslint-denied").unwrap_or_else(|error| panic!("scratch: {error}"));
    let denied = scratch.join("downloader.cfg");
    write(&denied, "allow bcr.bazel.build\nblock *\n");
    let denied_flag = format!("--downloader_config={}", denied.display());

    let expunged = root.bazel(&["clean", "--expunge"]);
    assert!(expunged.status.success(), "{}", expunged.combined());
    let mut fetch: Vec<&str> = vec!["fetch"];
    fetch.extend(CLOSURE_REPOS);
    fetch.push(denied_flag.as_str());
    let offline = root.bazel(&fetch);
    assert!(offline.status.success(), "{}", offline.combined());

    let empty = scratch.join("empty-cache");
    let cache_flag = format!("--repository_cache={}", empty.display());
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
