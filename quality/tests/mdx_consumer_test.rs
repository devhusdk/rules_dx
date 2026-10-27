use std::path::{Path, PathBuf};

use dx_testing::nested::{self, Case};
use dx_testing::{mkscratch, resolve_runfiles, Run};

const HELLO_MDX: &str = "import { Chart } from \"./chart.js\";\n\nexport const meta = { title: \"Report\" };\n\n# Quarterly report\n\nRevenue grew in _this_ quarter, see **below**.\n\n* First item\n* Second item\n\n<Chart data={[1, 2, 3]} label=\"Revenue\">\n  The chart shows the trend.\n</Chart>\n\n```js\nconst total = 1 + 2;\n```\n";

const HELLO_MDX_DIRTY: &str = "import { Chart } from \"./chart.js\";\n\nexport const meta = { title: \"Report\" };\n\nQuarterly report\n================\n\nRevenue grew in *this* quarter, see __below__.\n\n- First item\n- Second item\n\n<Chart data={[1, 2, 3]} label=\"Revenue\">\n  The chart shows the trend.\n</Chart>\n\n~~~js\nconst total = 1 + 2;\n~~~\n";

const HELLO_MDX_UNUSED: &str = "import { Chart } from \"./chart.js\";\nimport { Legend } from \"./legend.js\";\n\n# Report\n\n<Chart data={[1, 2, 3]} />\n";

const BROKEN_MDX: &str =
    "# Broken\n\nSome text before the tag.\n\n<Chart data={[1, 2, 3]}>\n  The chart never closes.\n";

const REMARK_CONFIG: &str = "{\n  \"bullet\": \"*\"\n}\n";

const ESLINT_CONFIG: &str = "import * as mdx from \"eslint-plugin-mdx\";\n\nexport default [\n  {\n    ...mdx.flat,\n    files: [\"**/*.mdx\"],\n    languageOptions: {\n      ...mdx.flat.languageOptions,\n      parserOptions: { ...mdx.flat.languageOptions.parserOptions, ignoreRemarkConfig: true },\n    },\n    rules: { ...mdx.flat.rules, \"mdx/remark\": \"off\", \"no-unused-vars\": \"error\" },\n  },\n];\n";

const MODULE: &str = r#"module(name = "renamed_mdx_consumer")

bazel_dep(name = "rules_dx", version = "0.0.0")
local_path_override(module_name = "rules_dx", path = "RULES_DX")
"#;

const BUILD: &str = r#"load("@rules_dx//quality:native_config.bzl", "eslint_config", "remark_config")
load("@rules_dx//quality:policy.bzl", "quality_family", "workspace_policy")

quality_family(
    name = "mdx_family",
    disabled = [
        "audit",
        "typecheck",
    ],
    family_id = "mdx",
    format = ["remark"],
    lint = ["eslint"],
)

workspace_policy(
    name = "policy",
    families = [":mdx_family"],
)

eslint_config(
    name = "eslint",
    src = "eslint.config.mjs",
    visibility = ["//visibility:public"],
)

remark_config(
    name = "remark",
    src = "remark.json",
    visibility = ["//visibility:public"],
)
"#;

const APP_BUILD: &str = r#"load("@rules_dx//mdx/rules:defs.bzl", "mdx_library")

mdx_library(
    name = "docs",
    srcs = glob(["*.mdx"]),
    aspect_hints = [
        "//:eslint",
        "//:remark",
    ],
)
"#;

const HELLO: &str = "app/Hello.mdx";
const BROKEN: &str = "app/Broken.mdx";

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
    let marker = resolve_runfiles("quality/tests/mdx_consumer_test.rs");
    let real = std::fs::canonicalize(&marker).unwrap_or_else(|error| panic!("{marker:?}: {error}"));
    real.ancestors()
        .nth(3)
        .unwrap_or_else(|| panic!("no workspace above {real:?}"))
        .to_path_buf()
}

fn consumer(files: &[(&str, &str)]) -> Case {
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
    write(&root.join("eslint.config.mjs"), ESLINT_CONFIG);
    write(&root.join("remark.json"), REMARK_CONFIG);
    write(&root.join("app/BUILD.bazel"), APP_BUILD);
    for (path, text) in files {
        write(&root.join(path), text);
    }
    root
}

fn dx(root: &Case, command: &str, args: &[&str]) -> Run {
    let rel = std::env::var("DX_BIN").unwrap_or_else(|_| panic!("DX_BIN must name dx"));
    let workspace = root.to_string_lossy().into_owned();
    let home = mkscratch("mdx-home").unwrap_or_else(|error| panic!("scratch: {error}"));
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
    root.run(
        &resolve_runfiles(&rel),
        &argv,
        &[("HOME", home.as_str()), ("PATH", "/usr/bin:/bin")],
    )
}

#[test]
fn a_clean_document_passes_check_unchanged() {
    let root = consumer(&[(HELLO, HELLO_MDX)]);
    let check = dx(&root, "check", &[]);
    assert_eq!(check.status.code(), Some(0), "{}", check.combined());
    assert_eq!(read(&root.join(HELLO)), HELLO_MDX);
}

#[test]
fn check_keeps_bytes_and_fix_formats_with_the_bound_settings() {
    let root = consumer(&[(HELLO, HELLO_MDX_DIRTY)]);
    let check = dx(&root, "check", &[]);
    assert_eq!(check.status.code(), Some(1), "{}", check.combined());
    assert!(check.combined().contains(HELLO), "{}", check.combined());
    assert_eq!(read(&root.join(HELLO)), HELLO_MDX_DIRTY);

    let fixed = dx(&root, "fix", &["--apply"]);
    assert_eq!(fixed.status.code(), Some(0), "{}", fixed.combined());
    assert_eq!(read(&root.join(HELLO)), HELLO_MDX);

    let again = dx(&root, "fix", &["--apply"]);
    assert_eq!(again.status.code(), Some(0), "{}", again.combined());
    assert_eq!(read(&root.join(HELLO)), HELLO_MDX);
    let check = dx(&root, "check", &[]);
    assert_eq!(check.status.code(), Some(0), "{}", check.combined());
}

#[test]
fn a_parse_error_is_located_and_never_rewritten() {
    let root = consumer(&[(BROKEN, BROKEN_MDX)]);
    let out = dx(&root, "format", &[]);
    assert_eq!(out.status.code(), Some(1), "{}", out.combined());
    assert!(
        out.combined().contains("app/Broken.mdx:5:1"),
        "{}",
        out.combined()
    );
    let applied = dx(&root, "format", &["--apply"]);
    assert_ne!(applied.status.code(), Some(0), "{}", applied.combined());
    assert_eq!(read(&root.join(BROKEN)), BROKEN_MDX);
}

#[test]
fn eslint_plugin_mdx_reports_at_the_original_line() {
    let root = consumer(&[(HELLO, HELLO_MDX_UNUSED)]);
    let out = dx(&root, "lint", &[]);
    assert_eq!(out.status.code(), Some(1), "{}", out.combined());
    assert!(
        out.combined().contains("app/Hello.mdx:2:10"),
        "{}",
        out.combined()
    );
    assert!(
        out.combined().contains("no-unused-vars"),
        "{}",
        out.combined()
    );
    assert_eq!(read(&root.join(HELLO)), HELLO_MDX_UNUSED);
}
