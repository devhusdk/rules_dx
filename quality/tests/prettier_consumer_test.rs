use std::path::PathBuf;

use dx_nested::{self as nested, Case};
use dx_test_scratch::scratch;
use dx_testing::{read, resolve_runfiles, rules_dx_source, write, Run};

const SETTINGS_JSON5: &str = "{\n  // service settings\n  name: \"app\",\n  retries: 3,\n}\n";

const SETTINGS_JSON5_DIRTY: &str =
    "{\n  // service settings\n  name:   \"app\",\n  retries: 3 ,\n}\n";

const TSCONFIG_JSONC: &str =
    "{\n  // compiler options\n  \"compilerOptions\": {\n    \"strict\": true,\n  },\n}\n";

const TSCONFIG_JSONC_DIRTY: &str =
    "{\n  // compiler options\n  \"compilerOptions\": {\n    \"strict\":   true\n  }\n}\n";

const SCHEMA_GRAPHQL: &str = "type Query {\n  greeting(name: String!): String!\n}\n";

const SCHEMA_GRAPHQL_DIRTY: &str = "type Query{\n  greeting(name:String!):String!\n}\n";

const QUERY_GRAPHQL: &str = "query Greet($name: String!) {\n  greeting(name: $name)\n}\n";

const EXTRA_GQL: &str = "fragment Name on Query {\n  greeting(name: \"x\")\n}\n";

const EXTRA_GQL_DIRTY: &str = "fragment Name on Query{\n  greeting(name:\"x\")\n}\n";

const INDEX_HTML: &str = "<!doctype html>\n<html lang=\"en\">\n  <head>\n    <meta charset=\"utf-8\" />\n    <title>pages</title>\n    <style>\n      body {\n        margin: 0;\n      }\n    </style>\n  </head>\n  <body>\n    <p>hello</p>\n    <script>\n      let greeting = \"hello\";\n    </script>\n  </body>\n</html>\n";

const INDEX_HTML_DIRTY: &str = "<!doctype html>\n<html lang=\"en\">\n<head><meta charset=\"utf-8\" />\n<title>pages</title>\n<style>body{margin:0}</style></head>\n<body>\n<p>hello</p>\n<script>let greeting=\"hello\"</script>\n</body>\n</html>\n";

const BROKEN_JSON5: &str = "{\n  name: \"app\",\n  retries: [1,\n";

const BROKEN_JSONC: &str = "{\n  \"strict\": [true,\n";

const BROKEN_GRAPHQL: &str = "type Query {\n  greeting: String\n";

const BROKEN_HTML: &str = "<div>\n  <p>hello</span>\n</div>\n";

const CLOSURE_REPOS: [&str; 2] = [
    "--repo=@@aspect_rules_js++npm+dx_npm_tools__prettier__3.9.6",
    "--repo=@@rules_nodejs++node+nodejs_linux_amd64",
];

const NODE_REPOS: [&str; 3] = [
    "--repo=@@aspect_rules_js++npm+prettier_consumer_npm__graphql__16.14.2",
    "--repo=@@aspect_rules_js++npm+prettier_consumer_npm__json5__2.2.3",
    "--repo=@@aspect_rules_js++npm+prettier_consumer_npm__jsonc-parser__3.3.1",
];

const NODE_MODULE: &str = r#"
bazel_dep(name = "aspect_rules_js", version = "3.5.2")

npm = use_extension("@aspect_rules_js//npm:extensions.bzl", "npm")
npm.npm_translate_lock(
    name = "prettier_consumer_npm",
    npmrc = "//:.npmrc",
    pnpm_lock = "//:pnpm-lock.yaml",
    verify_node_modules_ignored = "//:.bazelignore",
)
use_repo(npm, "prettier_consumer_npm")
"#;

const NODE_DATA_TEST: &str = r#"
load("@rules_dx//javascript/rules:defs.bzl", "javascript_js_test")

javascript_js_test(
    name = "data_test",
    srcs = ["data_test.mjs"],
    args = [
        "$(rootpath //config:settings)",
        "$(rootpath //config:tsconfig)",
        "$(rootpath //api:schema)",
        "$(rootpath //api:query)",
        "$(rootpath //api:fragment)",
    ],
    data = [
        "//:node_modules/graphql",
        "//:node_modules/json5",
        "//:node_modules/jsonc-parser",
        "//api:fragment",
        "//api:query",
        "//api:schema",
        "//config:settings",
        "//config:tsconfig",
    ],
    entry_point = "data_test.mjs",
    package_json = "//:package_json",
    tags = ["no-format", "no-lint", "no-typecheck"],
)
"#;

const MODULE: &str = r#"module(name = "consumer")

bazel_dep(name = "rules_dx", version = "0.0.0")
local_path_override(module_name = "rules_dx", path = "RULES_DX")

bazel_dep(name = "rules_rust", version = "0.75.0")
bazel_dep(name = "rules_rust_wasm_bindgen", version = "0.75.0")
"#;

const BUILD: &str = r#"load("@rules_dx//quality:policy.bzl", "quality_family", "workspace_policy")

quality_family(
    name = "html_family",
    disabled = [
        "lint",
        "typecheck",
    ],
    family_id = "html",
    format = ["prettier"],
)

quality_family(
    name = "graphql_family",
    disabled = [
        "lint",
        "typecheck",
    ],
    family_id = "graphql",
    format = ["prettier"],
)

quality_family(
    name = "json_family",
    disabled = [
        "lint",
        "typecheck",
    ],
    family_id = "json",
    format = ["prettier"],
)

workspace_policy(
    name = "policy",
    families = [
        ":graphql_family",
        ":html_family",
        ":json_family",
    ],
)
"#;

const CONFIG_BUILD: &str = r#"load("@rules_dx//javascript/rules:defs.bzl", "javascript_library")

javascript_library(
    name = "settings",
    srcs = ["settings.json5"],
    visibility = ["//visibility:public"],
)

javascript_library(
    name = "tsconfig",
    srcs = ["tsconfig.jsonc"],
    visibility = ["//visibility:public"],
)
"#;

const API_BUILD: &str = r#"load("@rules_dx//javascript/rules:defs.bzl", "javascript_library")

javascript_library(
    name = "schema",
    srcs = ["schema.graphql"],
    visibility = ["//visibility:public"],
)

javascript_library(
    name = "query",
    srcs = ["query.graphql"],
    visibility = ["//visibility:public"],
)

javascript_library(
    name = "fragment",
    srcs = ["fragment.gql"],
    visibility = ["//visibility:public"],
)
"#;

const PAGES_BUILD: &str = r#"load("@rules_dx//javascript/rules:defs.bzl", "javascript_library")

javascript_library(
    name = "pages",
    srcs = ["index.html"],
    visibility = ["//visibility:public"],
)

exports_files(["index.html"])
"#;

const APP_BUILD: &str = r#"load("@rules_dx//javascript/rules:web.bzl", "javascript_web_app")
load(":stub.bzl", "stub_bindgen")

stub_bindgen(name = "bindgen")

javascript_web_app(
    name = "app",
    assets = {
        "//api:query": "assets/query.graphql",
        "//api:schema": "assets/schema.graphql",
        "//config:settings": "assets/settings.json5",
        "//config:tsconfig": "assets/tsconfig.jsonc",
    },
    bindgen = ":bindgen",
    html = "//pages:index.html",
)
"#;

const STUB_BZL: &str = r#"load("@rules_rust_wasm_bindgen//:providers.bzl", "RustWasmBindgenInfo")

def _impl(ctx):
    js = ctx.actions.declare_file(ctx.label.name + "/app.js")
    wasm = ctx.actions.declare_file(ctx.label.name + "/app_bg.wasm")
    ts = ctx.actions.declare_file(ctx.label.name + "/app.d.ts")
    snippets = ctx.actions.declare_directory(ctx.label.name + "/snippets")
    ctx.actions.run_shell(
        outputs = [js, wasm, ts, snippets],
        command = "echo 'export default 1;' > {js}; printf '\\0asm\\1\\0\\0\\0' > {wasm}; echo 'export {{}};' > {ts}; mkdir -p {snippets}".format(
            js = js.path,
            wasm = wasm.path,
            ts = ts.path,
            snippets = snippets.path,
        ),
    )
    return [
        DefaultInfo(files = depset([js, wasm, ts, snippets])),
        RustWasmBindgenInfo(
            js = depset([js]),
            root = ctx.label.name,
            snippets = snippets,
            ts = depset([ts]),
            wasm = wasm,
        ),
    ]

stub_bindgen = rule(implementation = _impl)
"#;

const FILES: [(&str, &str, &str); 6] = [
    (
        "config/settings.json5",
        SETTINGS_JSON5,
        SETTINGS_JSON5_DIRTY,
    ),
    (
        "config/tsconfig.jsonc",
        TSCONFIG_JSONC,
        TSCONFIG_JSONC_DIRTY,
    ),
    ("api/schema.graphql", SCHEMA_GRAPHQL, SCHEMA_GRAPHQL_DIRTY),
    ("api/query.graphql", QUERY_GRAPHQL, QUERY_GRAPHQL),
    ("api/fragment.gql", EXTRA_GQL, EXTRA_GQL_DIRTY),
    ("pages/index.html", INDEX_HTML, INDEX_HTML_DIRTY),
];

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
    write(&root.join("config/BUILD.bazel"), CONFIG_BUILD);
    write(&root.join("api/BUILD.bazel"), API_BUILD);
    write(&root.join("pages/BUILD.bazel"), PAGES_BUILD);
    write(&root.join("app/BUILD.bazel"), APP_BUILD);
    write(&root.join("app/stub.bzl"), STUB_BZL);
    for (path, clean, _) in FILES {
        write(&root.join(path), clean);
    }
    root
}

fn with_node(root: &Case) {
    let rel = std::env::var("DX_PRETTIER_NODE_FIXTURE")
        .unwrap_or_else(|_| panic!("DX_PRETTIER_NODE_FIXTURE must name the pinned lock"));
    let lock = resolve_runfiles(&rel);
    let dir = lock
        .parent()
        .unwrap_or_else(|| panic!("{lock:?} has no parent"))
        .to_path_buf();
    for name in [
        "package.json",
        "pnpm-lock.yaml",
        "pnpm-workspace.yaml",
        ".npmrc",
    ] {
        write(&root.join(name), &read(&dir.join(name)));
    }
    write(
        &root.join("app/data_test.mjs"),
        &read(&dir.join("data_test.mjs")),
    );
    write(&root.join(".bazelignore"), "node_modules\n");
    let module = read(&root.join("MODULE.bazel"));
    write(
        &root.join("MODULE.bazel"),
        &format!("{module}{NODE_MODULE}"),
    );
    let build = read(&dir.join("app_data.BUILD.bazel"));
    let top = read(&root.join("BUILD.bazel"));
    let top = top.replacen(
        "load(\"@rules_dx//quality:policy.bzl\"",
        "load(\"@aspect_rules_js//js:defs.bzl\", \"js_library\")\nload(\"@prettier_consumer_npm//:defs.bzl\", \"npm_link_all_packages\")\nload(\"@rules_dx//quality:policy.bzl\"",
        1,
    );
    let _ = build;
    write(
        &root.join("BUILD.bazel"),
        &format!(
            "{top}\njs_library(\n    name = \"package_json\",\n    srcs = [\"package.json\"],\n    visibility = [\"//visibility:public\"],\n)\n\nnpm_link_all_packages(name = \"node_modules\")\n"
        ),
    );
    let app = read(&root.join("app/BUILD.bazel"));
    write(
        &root.join("app/BUILD.bazel"),
        &format!("{app}{NODE_DATA_TEST}"),
    );
}

fn dx_with(root: &Case, command: &str, args: &[&str], flags: &[&str]) -> Run {
    let rel = std::env::var("DX_BIN").unwrap_or_else(|_| panic!("DX_BIN must name dx"));
    let workspace = root.to_string_lossy().into_owned();
    let home_dir = scratch("prettier-home");
    let home = home_dir.path().to_string_lossy().into_owned();
    let mut argv: Vec<&str> = vec![command];
    argv.extend_from_slice(args);
    argv.extend_from_slice(&["--workspace", workspace.as_str(), "//...", "--"]);
    argv.push("--@rules_dx//config:workspace=//:policy");
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
fn format_check_keeps_bytes_apply_rewrites_and_recheck_passes() {
    let root = consumer();
    let clean = dx(&root, "format", &[]);
    assert_eq!(clean.status.code(), Some(0), "{}", clean.combined());

    for (path, _, dirty) in FILES {
        write(&root.join(path), dirty);
    }
    let run = dx(&root, "format", &[]);
    assert_eq!(run.status.code(), Some(1), "{}", run.combined());
    for (path, clean, dirty) in FILES {
        if clean != dirty {
            assert!(run.combined().contains(path), "{path}: {}", run.combined());
        }
    }
    assert!(
        !run.combined().contains("api/query.graphql"),
        "{}",
        run.combined()
    );
    for (path, clean, dirty) in FILES {
        let want = if clean == dirty { clean } else { dirty };
        assert_eq!(read(&root.join(path)), want, "{path} changed in check");
    }

    let applied = dx(&root, "format", &["--apply"]);
    assert_eq!(applied.status.code(), Some(0), "{}", applied.combined());
    for (path, clean, _) in FILES {
        assert_eq!(read(&root.join(path)), clean, "{path}");
    }

    let again = dx(&root, "format", &[]);
    assert_eq!(again.status.code(), Some(0), "{}", again.combined());
}

#[test]
fn an_ignored_owned_file_is_still_checked_and_rewritten() {
    let root = consumer();
    write(&root.join(".gitignore"), "api/fragment.gql\n");
    write(&root.join(".prettierignore"), "api/fragment.gql\npages/\n");
    write(&root.join("api/fragment.gql"), EXTRA_GQL_DIRTY);
    write(&root.join("pages/index.html"), INDEX_HTML_DIRTY);
    let run = dx(&root, "format", &[]);
    assert_eq!(run.status.code(), Some(1), "{}", run.combined());
    for path in ["api/fragment.gql", "pages/index.html"] {
        assert!(run.combined().contains(path), "{path}: {}", run.combined());
    }
    let applied = dx(&root, "format", &["--apply"]);
    assert_eq!(applied.status.code(), Some(0), "{}", applied.combined());
    assert_eq!(read(&root.join("api/fragment.gql")), EXTRA_GQL);
    assert_eq!(read(&root.join("pages/index.html")), INDEX_HTML);
}

#[test]
fn a_syntax_error_is_located_and_never_clean_in_every_class() {
    for (path, broken, line) in [
        (
            "config/settings.json5",
            BROKEN_JSON5,
            "config/settings.json5:4:1: error",
        ),
        (
            "config/tsconfig.jsonc",
            BROKEN_JSONC,
            "config/tsconfig.jsonc:3:1: error",
        ),
        (
            "api/schema.graphql",
            BROKEN_GRAPHQL,
            "api/schema.graphql:3:1: error",
        ),
        (
            "pages/index.html",
            BROKEN_HTML,
            "pages/index.html:2:11: error",
        ),
    ] {
        let root = consumer();
        write(&root.join(path), broken);
        let out = dx(&root, "format", &[]);
        assert_eq!(out.status.code(), Some(1), "{path}: {}", out.combined());
        assert!(out.combined().contains(line), "{path}: {}", out.combined());
        let applied = dx(&root, "format", &["--apply"]);
        assert_ne!(
            applied.status.code(),
            Some(0),
            "{path}: {}",
            applied.combined()
        );
        assert_eq!(read(&root.join(path)), broken, "{path}");
        let again = dx(&root, "format", &[]);
        assert_ne!(again.status.code(), Some(0), "{path}: {}", again.combined());
    }
}

#[test]
fn the_formatted_page_is_served_by_the_built_web_app() {
    let root = consumer();
    write(&root.join("pages/index.html"), INDEX_HTML_DIRTY);
    let applied = dx(&root, "format", &["--apply"]);
    assert_eq!(applied.status.code(), Some(0), "{}", applied.combined());
    let built = root.bazel(&["build", "//app:app"]);
    assert!(built.status.success(), "{}", built.combined());
    let base = root.bazel(&["info", "bazel-bin"]);
    assert!(base.status.success(), "{}", base.combined());
    let staged = PathBuf::from(base.stdout.trim());
    let page = std::fs::read_dir(staged.join("app"))
        .unwrap_or_else(|error| panic!("{staged:?}: {error}"))
        .flatten()
        .map(|entry| entry.path())
        .find_map(|dir| std::fs::read_to_string(dir.join("index.html")).ok())
        .or_else(|| std::fs::read_to_string(staged.join("app/index.html")).ok())
        .unwrap_or_else(|| panic!("no staged index.html under {staged:?}"));
    assert_eq!(page, INDEX_HTML);
}

#[test]
fn the_declared_prettier_is_acquired_from_the_cache_with_the_network_denied() {
    let root = consumer();
    with_node(&root);
    let warm = dx(&root, "format", &[]);
    assert_eq!(warm.status.code(), Some(0), "{}", warm.combined());

    let scratch_dir = scratch("prettier-denied");
    let scratch = scratch_dir.path();
    let denied = scratch.join("downloader.cfg");
    write(&denied, "allow bcr.bazel.build\nblock *\n");
    let denied_flag = format!("--downloader_config={}", denied.display());

    let expunged = root.bazel(&["clean", "--expunge"]);
    assert!(expunged.status.success(), "{}", expunged.combined());
    let mut fetch: Vec<&str> = vec!["fetch"];
    fetch.extend(CLOSURE_REPOS);
    fetch.extend(NODE_REPOS);
    fetch.push(denied_flag.as_str());
    let offline = root.bazel(&fetch);
    assert!(offline.status.success(), "{}", offline.combined());

    let empty = scratch.join("empty-cache");
    let cache_flag = format!("--repository_cache={}", empty.display());
    let expunged = root.bazel(&["clean", "--expunge"]);
    assert!(expunged.status.success(), "{}", expunged.combined());
    let mut cold: Vec<&str> = vec!["fetch"];
    cold.extend(CLOSURE_REPOS);
    cold.extend(NODE_REPOS);
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

const GRAPHQL_OVERRIDE: &str =
    "{\"overrides\": [{\"files\": \"*.graphql\", \"options\": {\"tabWidth\": 4}}]}\n";

const SCHEMA_GRAPHQL_TAB4: &str = "type Query {\n    greeting(name: String!): String!\n}\n";

const QUERY_GRAPHQL_TAB4: &str = "query Greet($name: String!) {\n    greeting(name: $name)\n}\n";

fn bind_config(root: &Case, config: &str, data: &[(&str, &str)]) {
    write(&root.join(".prettierrc.json"), config);
    let mut names = String::new();
    for (path, text) in data {
        write(&root.join(path), text);
        names.push_str(&format!("\"{path}\", "));
    }
    let build = format!(
        "load(\"@rules_dx//quality:native_config.bzl\", \"prettier_config\")\n{}\nprettier_config(\n    name = \"prettier\",\n    src = \".prettierrc.json\",\n    data = [{names}],\n    visibility = [\"//visibility:public\"],\n)\n",
        BUILD
    );
    write(&root.join("BUILD.bazel"), &build);
    for package in ["config", "api", "pages"] {
        let path = root.join(package).join("BUILD.bazel");
        let text = read(&path).replace(
            "    srcs = [",
            "    aspect_hints = [\"//:prettier\"],\n    srcs = [",
        );
        write(&path, &text);
    }
}

#[test]
fn a_bound_prettier_config_changes_the_formatting() {
    let root = consumer();
    bind_config(&root, GRAPHQL_OVERRIDE, &[]);
    let run = dx(&root, "format", &[]);
    assert_eq!(run.status.code(), Some(1), "{}", run.combined());
    assert!(
        run.combined().contains("api/schema.graphql"),
        "{}",
        run.combined()
    );
    assert!(
        run.combined().contains("api/query.graphql"),
        "{}",
        run.combined()
    );
    assert!(
        !run.combined().contains("api/fragment.gql"),
        "{}",
        run.combined()
    );
    assert_eq!(read(&root.join("api/schema.graphql")), SCHEMA_GRAPHQL);
    assert_eq!(read(&root.join("api/query.graphql")), QUERY_GRAPHQL);

    let applied = dx(&root, "format", &["--apply"]);
    assert_eq!(applied.status.code(), Some(0), "{}", applied.combined());
    assert_eq!(read(&root.join("api/schema.graphql")), SCHEMA_GRAPHQL_TAB4);
    assert_eq!(read(&root.join("api/query.graphql")), QUERY_GRAPHQL_TAB4);

    let again = dx(&root, "format", &[]);
    assert_eq!(again.status.code(), Some(0), "{}", again.combined());
}

#[test]
fn an_unknown_parser_in_the_config_fails_and_is_never_clean() {
    let root = consumer();
    bind_config(
        &root,
        "{\"overrides\": [{\"files\": \"*.json5\", \"options\": {\"parser\": \"nope\"}}]}\n",
        &[],
    );
    let run = dx(&root, "format", &[]);
    assert_ne!(run.status.code(), Some(0), "{}", run.combined());
    assert!(run.combined().contains("nope"), "{}", run.combined());
    let applied = dx(&root, "format", &["--apply"]);
    assert_ne!(applied.status.code(), Some(0), "{}", applied.combined());
    assert_eq!(read(&root.join("config/settings.json5")), SETTINGS_JSON5);
    let again = dx(&root, "format", &[]);
    assert_ne!(again.status.code(), Some(0), "{}", again.combined());
}

#[test]
fn a_prettierignore_in_the_config_data_does_not_hide_a_file() {
    let root = consumer();
    bind_config(
        &root,
        "{}\n",
        &[(".prettierignore", "config/settings.json5\n")],
    );
    write(&root.join("config/settings.json5"), SETTINGS_JSON5_DIRTY);
    let run = dx(&root, "format", &[]);
    assert_eq!(run.status.code(), Some(1), "{}", run.combined());
    assert!(
        run.combined().contains("config/settings.json5"),
        "{}",
        run.combined()
    );
}

#[test]
fn an_invalid_json_config_fails_the_run() {
    let root = consumer();
    bind_config(&root, "{\"tabWidth\": \n", &[]);
    let run = dx(&root, "format", &[]);
    assert_ne!(run.status.code(), Some(0), "{}", run.combined());
    let applied = dx(&root, "format", &["--apply"]);
    assert_ne!(applied.status.code(), Some(0), "{}", applied.combined());
}

const QUERY_UNKNOWN_FIELD: &str =
    "query Greet($name: String!) {\n  greeting(name: $name)\n  nope\n}\n";

fn node_consumer() -> Case {
    let root = consumer();
    with_node(&root);
    root
}

#[test]
fn the_formatted_data_is_parsed_by_a_declared_test() {
    let root = node_consumer();
    for (path, _, dirty) in FILES {
        write(&root.join(path), dirty);
    }
    let applied = dx(&root, "format", &["--apply"]);
    assert_eq!(applied.status.code(), Some(0), "{}", applied.combined());
    for (path, clean, _) in FILES {
        assert_eq!(read(&root.join(path)), clean, "{path}");
    }
    let test = root.bazel(&["test", "//app:data_test", "--test_output=errors"]);
    assert!(test.status.success(), "{}", test.combined());
    let again = dx(&root, "format", &[]);
    assert_eq!(again.status.code(), Some(0), "{}", again.combined());
}

#[test]
fn a_query_the_schema_rejects_fails_the_declared_test() {
    let root = node_consumer();
    write(&root.join("api/query.graphql"), QUERY_UNKNOWN_FIELD);
    let run = dx(&root, "format", &[]);
    assert_eq!(run.status.code(), Some(0), "{}", run.combined());
    let test = root.bazel(&["test", "//app:data_test", "--test_output=errors"]);
    assert_ne!(test.status.code(), Some(0), "{}", test.combined());
    assert!(
        test.combined().contains("Cannot query field"),
        "{}",
        test.combined()
    );
}

#[test]
fn a_broken_json5_fails_format_and_the_declared_test() {
    let root = node_consumer();
    write(&root.join("config/settings.json5"), BROKEN_JSON5);
    let run = dx(&root, "format", &[]);
    assert_ne!(run.status.code(), Some(0), "{}", run.combined());
    let test = root.bazel(&["test", "//app:data_test", "--test_output=errors"]);
    assert_ne!(test.status.code(), Some(0), "{}", test.combined());
}
