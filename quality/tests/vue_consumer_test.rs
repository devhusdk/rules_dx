use std::path::{Path, PathBuf};

use dx_nested::{self as nested, Case};
use dx_testing::{mkscratch, resolve_runfiles, Run};

const HELLO_VUE: &str = "<template>\n  <div class=\"hello\">{{ message }}</div>\n</template>\n\n<script setup lang=\"ts\">\nconst message: string = \"world\";\n</script>\n\n<style scoped>\n.hello {\n  color: black;\n}\n</style>\n";

const HELLO_VUE_DIRTY: &str = "<template>\n<div   class=\"hello\">{{message}}</div>\n</template>\n\n<script setup lang=\"ts\">\nconst message: string = 'world'\n</script>\n\n<style scoped>\n.hello { color: black }\n</style>\n";

const HELLO_VUE_BROKEN: &str =
    "<template>\n  <div>\n    <span>hi\n</template>\n\n<script setup lang=\"ts\">\nconst message = 1;\n</script>\n";

const HELLO_VUE_UNUSED: &str = "<template>\n  <ul>\n    <li v-for=\"item in items\" :key=\"items.length\">entry</li>\n  </ul>\n</template>\n\n<script setup lang=\"ts\">\nconst items: string[] = [\"a\"];\n</script>\n";

const ESLINT_CONFIG: &str = "import pluginVue from \"eslint-plugin-vue\";\n\nexport default [\n  ...pluginVue.configs[\"flat/essential\"],\n  {\n    files: [\"**/*.vue\"],\n    languageOptions: {\n      parserOptions: {\n        parser: \"@typescript-eslint/parser\",\n      },\n    },\n  },\n];\n";

const MODULE: &str = r#"module(name = "renamed_vue_consumer")

bazel_dep(name = "rules_dx", version = "0.0.0")
local_path_override(module_name = "rules_dx", path = "RULES_DX")
"#;

const BUILD: &str = r#"load("@rules_dx//quality:native_config.bzl", "eslint_config")
load("@rules_dx//quality:policy.bzl", "quality_family", "workspace_policy")

quality_family(
    name = "vue_family",
    disabled = [
        "audit",
        "typecheck",
    ],
    family_id = "vue",
    format = ["prettier"],
    lint = ["eslint"],
)

workspace_policy(
    name = "policy",
    families = [":vue_family"],
)

eslint_config(
    name = "eslint",
    src = "eslint.config.mjs",
    visibility = ["//visibility:public"],
)
"#;

const APP_BUILD: &str = r#"load("@rules_dx//vue/rules:defs.bzl", "vue_library")

vue_library(
    name = "hello",
    srcs = ["HelloWorld.vue"],
    aspect_hints = ["//:eslint"],
)
"#;

const HELLO: &str = "app/HelloWorld.vue";

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
    let marker = resolve_runfiles("quality/tests/vue_consumer_test.rs");
    let real = std::fs::canonicalize(&marker).unwrap_or_else(|error| panic!("{marker:?}: {error}"));
    real.ancestors()
        .nth(3)
        .unwrap_or_else(|| panic!("no workspace above {real:?}"))
        .to_path_buf()
}

fn consumer(hello: &str) -> Case {
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
    write(&root.join("app/BUILD.bazel"), APP_BUILD);
    write(&root.join(HELLO), hello);
    root
}

fn dx(root: &Case, command: &str, args: &[&str]) -> Run {
    let rel = std::env::var("DX_BIN").unwrap_or_else(|_| panic!("DX_BIN must name dx"));
    let workspace = root.to_string_lossy().into_owned();
    let home = mkscratch("vue-home").unwrap_or_else(|error| panic!("scratch: {error}"));
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
fn a_clean_sfc_passes_format_and_lint() {
    let root = consumer(HELLO_VUE);
    let format = dx(&root, "format", &[]);
    assert_eq!(format.status.code(), Some(0), "{}", format.combined());
    let check = dx(&root, "check", &[]);
    assert_eq!(check.status.code(), Some(0), "{}", check.combined());
    assert_eq!(read(&root.join(HELLO)), HELLO_VUE);
}

#[test]
fn check_keeps_bytes_apply_rewrites_and_recheck_passes() {
    let root = consumer(HELLO_VUE_DIRTY);
    let check = dx(&root, "check", &[]);
    assert_eq!(check.status.code(), Some(1), "{}", check.combined());
    assert!(check.combined().contains(HELLO), "{}", check.combined());
    assert_eq!(read(&root.join(HELLO)), HELLO_VUE_DIRTY);

    let applied = dx(&root, "format", &["--apply"]);
    assert_eq!(applied.status.code(), Some(0), "{}", applied.combined());
    assert_eq!(read(&root.join(HELLO)), HELLO_VUE);

    let again = dx(&root, "format", &["--apply"]);
    assert_eq!(again.status.code(), Some(0), "{}", again.combined());
    assert_eq!(read(&root.join(HELLO)), HELLO_VUE);
    let check = dx(&root, "check", &[]);
    assert_eq!(check.status.code(), Some(0), "{}", check.combined());
}

#[test]
fn a_broken_template_is_located_and_never_clean() {
    let root = consumer(HELLO_VUE_BROKEN);
    let out = dx(&root, "format", &[]);
    assert_eq!(out.status.code(), Some(1), "{}", out.combined());
    assert!(
        out.combined().contains("app/HelloWorld.vue:4:1: error"),
        "{}",
        out.combined()
    );
    let applied = dx(&root, "format", &["--apply"]);
    assert_ne!(applied.status.code(), Some(0), "{}", applied.combined());
    assert_eq!(read(&root.join(HELLO)), HELLO_VUE_BROKEN);
}

#[test]
fn eslint_plugin_vue_reports_at_the_original_sfc() {
    let root = consumer(HELLO_VUE_UNUSED);
    let out = dx(&root, "lint", &[]);
    assert_eq!(out.status.code(), Some(1), "{}", out.combined());
    assert!(
        out.combined().contains("app/HelloWorld.vue:3:16"),
        "{}",
        out.combined()
    );
    assert!(
        out.combined().contains("vue/no-unused-vars"),
        "{}",
        out.combined()
    );
    assert_eq!(read(&root.join(HELLO)), HELLO_VUE_UNUSED);
}
