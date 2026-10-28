use dx_nested::{self as nested, Case};
use dx_testing::{mkscratch, read, resolve_runfiles, rules_dx_source, write, Run};

const BASE_CSS: &str = ":root {\n  --gap: 4px;\n}\n\nbody {\n  margin: 0;\n}\n";

const THEME_SCSS: &str = "// theme tokens\n$accent: #336699;\n\n.theme {\n  color: $accent;\n\n  &:hover {\n    color: darken($accent, 10%);\n  }\n}\n";

const WIDGETS_LESS: &str = "// widgets\n@gap: 4px;\n\n.widget {\n  padding: @gap;\n\n  .title {\n    margin: @gap;\n  }\n}\n";

const BAD_CSS: &str = ":root {\n  --gap: 4px;\n}\n\nbody {\n  colr: red;\n}\n";

const BAD_SCSS: &str = "// theme tokens\n$accent: #336699;\n\n.theme {\n  colr: $accent;\n}\n";

const BAD_LESS: &str = "// widgets\n@gap: 4px;\n\n.widget {\n  colr: @gap;\n}\n";

const BROKEN_CSS: &str = "body {\n  color: red;\n";

const UNFORMATTED_CSS: &str = ":root{--gap:4px}\n\nbody{margin:0}\n";

const OVERRIDE_JSON: &str = r#"{
  "rules": {
    "color-no-invalid-hex": true
  },
  "overrides": [
    {
      "files": ["*.scss"],
      "customSyntax": "postcss-scss"
    },
    {
      "files": ["*.less"],
      "customSyntax": "postcss-less"
    }
  ]
}
"#;

const UNKNOWN_RULE_JSON: &str = "{\n  \"rules\": {\n    \"no-such-rule-anywhere\": true\n  }\n}\n";

const CLOSURE_REPOS: [&str; 6] = [
    "--repo=@@aspect_rules_js++npm+dx_npm_tools__stylelint__17.14.1",
    "--repo=@@aspect_rules_js++npm+dx_npm_tools__postcss-scss__4.0.9_postcss_8.5.31",
    "--repo=@@aspect_rules_js++npm+dx_npm_tools__postcss-less__6.0.0_postcss_8.5.31",
    "--repo=@@aspect_rules_js++npm+dx_npm_tools__postcss__8.5.31",
    "--repo=@@aspect_rules_js++npm+dx_npm_tools__postcss-value-parser__4.2.0",
    "--repo=@@rules_nodejs++node+nodejs_linux_amd64",
];

const MODULE: &str = r#"module(name = "consumer")

bazel_dep(name = "rules_dx", version = "0.0.0")
local_path_override(module_name = "rules_dx", path = "RULES_DX")

bazel_dep(name = "rules_rust", version = "0.74.0")
bazel_dep(name = "rules_rust_wasm_bindgen", version = "0.74.0")
"#;

const BUILD: &str = r#"load("@rules_dx//quality:native_config.bzl", "stylelint_config")
load("@rules_dx//quality:policy.bzl", "quality_family", "workspace_policy")

stylelint_config(
    name = "lint_override",
    src = "stylelint.json",
    visibility = ["//visibility:public"],
)

quality_family(
    name = "css_family",
    family_id = "css",
    format = ["prettier"],
    lint = ["stylelint"],
)

workspace_policy(
    name = "policy",
    families = [":css_family"],
)
"#;

const BASE_BUILD: &str = r#"load("@rules_dx//javascript/rules:defs.bzl", "javascript_library")

javascript_library(
    name = "base",
    srcs = ["base.css"],
    visibility = ["//visibility:public"],
)
"#;

const THEME_BUILD: &str = r#"load("@rules_dx//javascript/rules:defs.bzl", "javascript_library")

javascript_library(
    name = "theme",
    srcs = ["theme.scss"],
    visibility = ["//visibility:public"],
    deps = ["//styles/base"],
)
"#;

const WIDGETS_BUILD: &str = r#"load("@rules_dx//javascript/rules:defs.bzl", "javascript_library")

javascript_library(
    name = "widgets",
    srcs = ["widgets.less"],
    visibility = ["//visibility:public"],
    deps = ["//styles/theme"],
)
"#;

const APP_BUILD: &str = r#"load("@rules_dx//javascript/rules:web.bzl", "javascript_web_app")
load(":stub.bzl", "stub_bindgen")

stub_bindgen(name = "bindgen")

javascript_web_app(
    name = "app",
    assets = {
        "//styles/base": "assets/base.css",
        "//styles/theme": "assets/theme.scss",
        "//styles/widgets": "assets/widgets.less",
    },
    bindgen = ":bindgen",
    html = "index.html",
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

const INDEX_HTML: &str = "<!doctype html>\n<html lang=\"en\">\n  <head>\n    <meta charset=\"utf-8\" />\n    <title>styles</title>\n  </head>\n  <body></body>\n</html>\n";

const BAD_BUILD: &str = r#"load("@rules_dx//javascript/rules:defs.bzl", "javascript_library")

javascript_library(
    name = "bad",
    srcs = ["bad.EXT"],
    aspect_hints = HINTS,
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
    write(&root.join("stylelint.json"), OVERRIDE_JSON);
    write(&root.join("styles/base/BUILD.bazel"), BASE_BUILD);
    write(&root.join("styles/base/base.css"), BASE_CSS);
    write(&root.join("styles/theme/BUILD.bazel"), THEME_BUILD);
    write(&root.join("styles/theme/theme.scss"), THEME_SCSS);
    write(&root.join("styles/widgets/BUILD.bazel"), WIDGETS_BUILD);
    write(&root.join("styles/widgets/widgets.less"), WIDGETS_LESS);
    write(&root.join("app/BUILD.bazel"), APP_BUILD);
    write(&root.join("app/stub.bzl"), STUB_BZL);
    write(&root.join("app/index.html"), INDEX_HTML);
    root
}

fn dx_with(root: &Case, command: &str, args: &[&str], flags: &[&str]) -> Run {
    let rel = std::env::var("DX_BIN").unwrap_or_else(|_| panic!("DX_BIN must name dx"));
    let workspace = root.to_string_lossy().into_owned();
    let home = mkscratch("stylelint-home").unwrap_or_else(|error| panic!("scratch: {error}"));
    let home = home.to_string_lossy().into_owned();
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

fn add_bad(root: &Case, ext: &str, text: &str, hint: &str) {
    write(
        &root.join("bad/BUILD.bazel"),
        &BAD_BUILD.replace("EXT", ext).replace("HINTS", hint),
    );
    write(&root.join(format!("bad/bad.{ext}")), text);
}

#[test]
fn clean_stylesheets_pass_and_the_web_app_builds() {
    let root = consumer();
    let clean = dx(&root, "lint", &[]);
    assert_eq!(clean.status.code(), Some(0), "{}", clean.combined());
    let built = root.bazel(&["build", "//app:app"]);
    assert!(built.status.success(), "{}", built.combined());
}

#[test]
fn dirty_css_scss_and_less_are_reported_with_rule_and_position() {
    let root = consumer();
    for (ext, text, line) in [
        ("css", BAD_CSS, "bad/bad.css:6:"),
        ("scss", BAD_SCSS, "bad/bad.scss:5:"),
        ("less", BAD_LESS, "bad/bad.less:5:"),
    ] {
        add_bad(&root, ext, text, "[]");
        let out = dx(&root, "lint", &[]);
        assert_eq!(out.status.code(), Some(1), "{ext}: {}", out.combined());
        assert!(out.combined().contains(line), "{ext}: {}", out.combined());
        assert!(
            out.combined().contains("property-no-unknown"),
            "{ext}: {}",
            out.combined()
        );
        std::fs::remove_dir_all(root.join("bad")).unwrap_or_else(|error| panic!("rm: {error}"));
    }
}

#[test]
fn a_stylelint_config_override_replaces_the_default_rules() {
    let root = consumer();
    add_bad(&root, "scss", BAD_SCSS, "[\"//:lint_override\"]");
    let out = dx(&root, "lint", &[]);
    assert_eq!(out.status.code(), Some(0), "{}", out.combined());
}

#[test]
fn a_broken_stylesheet_is_reported_and_never_clean() {
    let root = consumer();
    add_bad(&root, "css", BROKEN_CSS, "[]");
    let out = dx(&root, "lint", &[]);
    assert_ne!(out.status.code(), Some(0), "{}", out.combined());
    assert!(
        out.combined().contains("bad/bad.css:"),
        "{}",
        out.combined()
    );
}

#[test]
fn an_invalid_config_is_reported_and_never_clean() {
    let root = consumer();
    write(&root.join("stylelint.json"), UNKNOWN_RULE_JSON);
    add_bad(&root, "css", BAD_CSS, "[\"//:lint_override\"]");
    let out = dx(&root, "lint", &[]);
    assert_ne!(out.status.code(), Some(0), "{}", out.combined());
    assert!(
        out.combined().contains("no-such-rule-anywhere"),
        "{}",
        out.combined()
    );
}

#[test]
fn format_stays_with_prettier_and_stylelint_never_rewrites() {
    let root = consumer();
    let base = root.join("styles/base/base.css");
    write(&base, UNFORMATTED_CSS);

    let dirty = dx(&root, "format", &[]);
    assert_eq!(dirty.status.code(), Some(1), "{}", dirty.combined());
    assert!(
        dirty.combined().contains("styles/base/base.css"),
        "{}",
        dirty.combined()
    );
    assert_eq!(read(&base), UNFORMATTED_CSS);

    let applied = dx(&root, "format", &["--apply"]);
    assert_eq!(applied.status.code(), Some(0), "{}", applied.combined());
    assert_eq!(read(&base), BASE_CSS);

    add_bad(&root, "css", BAD_CSS, "[]");
    let fixed = dx(&root, "lint", &["--apply"]);
    assert_ne!(fixed.status.code(), Some(0), "{}", fixed.combined());
    assert_eq!(read(&root.join("bad/bad.css")), BAD_CSS);
}

#[test]
fn the_declared_stylelint_is_acquired_from_the_cache_with_the_network_denied() {
    let root = consumer();
    let warm = dx(&root, "lint", &[]);
    assert_eq!(warm.status.code(), Some(0), "{}", warm.combined());

    let scratch = mkscratch("stylelint-denied").unwrap_or_else(|error| panic!("scratch: {error}"));
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

const MODULES_BUILD: &str = r#"load("@rules_dx//javascript/rules:defs.bzl", "javascript_library")

javascript_library(
    name = "modules",
    srcs = ["modules.css"],
    visibility = ["//visibility:public"],
)
"#;

const MODULES_CSS: &str = ".card :global(.theme) {\n  composes: base from \"./base.css\";\n  color: red;\n}\n\n:local(.title) {\n  margin: 0;\n}\n\n:export {\n  accent: #336699;\n}\n";

#[test]
fn css_modules_syntax_lints_clean_and_typos_are_still_flagged() {
    let root = consumer();
    write(&root.join("modules/BUILD.bazel"), MODULES_BUILD);
    write(&root.join("modules/modules.css"), MODULES_CSS);
    let clean = dx(&root, "lint", &[]);
    assert_eq!(clean.status.code(), Some(0), "{}", clean.combined());

    write(
        &root.join("modules/modules.css"),
        &format!("{MODULES_CSS}\n.card {{\n  colr: red;\n}}\n"),
    );
    let dirty = dx(&root, "lint", &[]);
    assert_eq!(dirty.status.code(), Some(1), "{}", dirty.combined());
    assert!(
        dirty.combined().contains("modules/modules.css:"),
        "{}",
        dirty.combined()
    );
    assert!(
        dirty.combined().contains("property-no-unknown"),
        "{}",
        dirty.combined()
    );
}
