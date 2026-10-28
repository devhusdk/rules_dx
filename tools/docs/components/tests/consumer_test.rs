//! Builds a component API site through `dx docs` in a consumer that renames rules_dx.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use dx_nested::{self as nested, Case};
use dx_test_scratch::scratch;
use dx_testing::{read, resolve_runfiles, rules_dx_source, sha256_file, write, Run};

const MODULE: &str = r#"module(name = "renamed_component_docs_consumer")

bazel_dep(name = "rules_dx", version = "0.0.0", repo_name = "dxr")
local_path_override(module_name = "rules_dx", path = "RULES_DX")

bazel_dep(name = "aspect_rules_js", version = "RULES_JS")
bazel_dep(name = "rules_kotlin", version = "RULES_KOTLIN")
bazel_dep(name = "rules_rust", version = "0.74.0")

npm = use_extension("@aspect_rules_js//npm:extensions.bzl", "npm")
npm.npm_translate_lock(
    name = "npm",
    npmrc = "//:.npmrc",
    pnpm_lock = "//:pnpm-lock.yaml",
    verify_node_modules_ignored = "//:.bazelignore",
)
use_repo(npm, "npm")
"#;

const ROOT_BUILD: &str = r#"load("@dxr//tools/docs/site:site.bzl", "docs_site")
load("@npm//:defs.bzl", "npm_link_all_packages")

npm_link_all_packages(name = "node_modules")

docs_site(
    name = "site",
    book_toml = "book.toml",
    prose = ["prose.md"],
    shards = ["//ui:api"],
    source_url = "https://example.com/consumer/blob/main/",
)
"#;

const UI_BUILD: &str = r#"load("@dxr//tools/docs/components:defs.bzl", "vue_doc_shard")

vue_doc_shard(
    name = "api",
    srcs = glob(["src/*.vue"]) + ["tsconfig.base.json"],
    deps = DEPS,
    entry_points = [
        "src/Button.vue",
        "src/Swatch.vue",
    ],
    node_modules = "//:node_modules",
    package = "ui",
    tsconfig = "config/tsconfig.docs.json",
    visibility = ["//visibility:public"],
)
"#;

const DEPS: &str = r#"["//:node_modules/csstype"]"#;

const BOOK_TOML: &str =
    "[book]\ntitle = \"consumer\"\nauthors = [\"consumer\"]\nlanguage = \"en\"\nsrc = \"src\"\n";

const PROSE: &str = "# Consumer\n\nThe component API reference of the ui package.\n";

const TSCONFIG_BASE: &str = "{\n  \"compilerOptions\": {\n    \"target\": \"ES2022\",\n    \"module\": \"ESNext\",\n    \"moduleResolution\": \"Bundler\",\n    \"strict\": true,\n    \"noEmit\": true,\n    \"skipLibCheck\": true,\n    \"types\": []\n  }\n}\n";

const TSCONFIG_DOCS: &str =
    "{\n  \"extends\": \"../tsconfig.base.json\",\n  \"include\": [\"../src\"]\n}\n";

const BUTTON: &str = r#"<script setup lang="ts">
/** The props of the button. */
interface Props {
  /** Text shown inside the button. */
  label: string;
  /** Optional size in pixels. */
  size?: number;
}

const props = withDefaults(defineProps<Props>(), {
  size: 12,
});

const emit = defineEmits<{
  /** Fired when the button is clicked. */
  (e: "press", id: number): void;
}>();

defineSlots<{
  /** Content of the button. */
  default(props: { label: string }): unknown;
}>();

function press(): void {
  emit("press", props.size);
}
</script>

<template>
  <button @click="press"><slot :label="props.label" /></button>
</template>
"#;

const SWATCH: &str = r#"<script setup lang="ts">
import type * as CSS from "csstype";

defineProps<{
  /** Inline style of the swatch. */
  look: CSS.Properties;
}>();
</script>

<template>
  <span class="swatch" />
</template>
"#;

const VITE_CONFIG: &str = "import { writeFileSync } from \"node:fs\";\n\nwriteFileSync(\"vite_config_ran.marker\", \"ran\");\n\nexport default {};\n";

fn pin(text: &str, name: &str) -> String {
    let marker = format!("{name}\", version = \"");
    let start = text
        .find(&marker)
        .unwrap_or_else(|| panic!("{name} missing"))
        + marker.len();
    let end = text[start..].find('"').expect("closing quote") + start;
    text[start..end].to_owned()
}

/// The version a `single_version_override` of the rules_dx root forces; overrides do not
/// reach a consumer, which must ask for the same version.
fn override_pin(text: &str, name: &str) -> String {
    let marker = format!("module_name = \"{name}\",\n    version = \"");
    let start = text
        .find(&marker)
        .unwrap_or_else(|| panic!("{name} override missing"))
        + marker.len();
    let end = text[start..].find('"').expect("closing quote") + start;
    text[start..end].to_owned()
}

/// A consumer sets up fresh: the build keeps the old `bazel-bin/site`, so each case is its own.
fn consumer(name: &str, deps: &str) -> Case {
    let rules = rules_dx_source();
    let module = read(&rules.join("MODULE.bazel"));
    let root = nested::case(name);
    write(
        &root.join(".bazelversion"),
        &read(&rules.join(".bazelversion")),
    );
    write(
        &root.join("MODULE.bazel"),
        &MODULE
            .replace("RULES_DX", &rules.display().to_string())
            .replace("RULES_KOTLIN", &override_pin(&module, "rules_kotlin"))
            .replace(
                "RULES_JS",
                &pin(&module, "bazel_dep(name = \"aspect_rules_js"),
            ),
    );
    write(&root.join(".bazelrc"), "common --enable_bzlmod\n");
    write(&root.join(".bazelignore"), "node_modules\n");

    let rel = std::env::var("DX_COMPONENT_NODE_FIXTURE")
        .unwrap_or_else(|_| panic!("DX_COMPONENT_NODE_FIXTURE must name the pinned lock"));
    let lock = resolve_runfiles(&rel);
    let dir = lock
        .parent()
        .unwrap_or_else(|| panic!("{lock:?} has no parent"))
        .to_path_buf();
    for file in [
        "package.json",
        "pnpm-lock.yaml",
        "pnpm-workspace.yaml",
        ".npmrc",
    ] {
        write(&root.join(file), &read(&dir.join(file)));
    }

    write(&root.join("BUILD.bazel"), ROOT_BUILD);
    write(&root.join("book.toml"), BOOK_TOML);
    write(&root.join("prose.md"), PROSE);
    write(
        &root.join("ui/BUILD.bazel"),
        &UI_BUILD.replace("DEPS", deps),
    );
    write(&root.join("ui/tsconfig.base.json"), TSCONFIG_BASE);
    write(&root.join("ui/config/tsconfig.docs.json"), TSCONFIG_DOCS);
    write(&root.join("ui/src/Button.vue"), BUTTON);
    write(&root.join("ui/src/Swatch.vue"), SWATCH);
    write(&root.join("ui/vite.config.ts"), VITE_CONFIG);
    root
}

fn docs(root: &Case) -> Run {
    let rel = std::env::var("DX_BIN").unwrap_or_else(|_| panic!("DX_BIN must name dx"));
    let workspace = root.to_string_lossy().into_owned();
    let home_dir = scratch("component-docs-home");
    let home = home_dir.path().to_string_lossy().into_owned();
    root.run(
        &resolve_runfiles(&rel),
        &["docs", "--workspace", workspace.as_str(), "//:site"],
        &[("HOME", home.as_str()), ("PATH", "/usr/bin:/bin")],
    )
}

fn hashes(dir: &Path) -> BTreeMap<PathBuf, String> {
    let mut out = BTreeMap::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(next) = stack.pop() {
        for entry in std::fs::read_dir(&next).unwrap_or_else(|error| panic!("{next:?}: {error}")) {
            let path = entry
                .unwrap_or_else(|error| panic!("entry: {error}"))
                .path();
            if path.is_dir() {
                stack.push(path);
            } else {
                let hash = sha256_file(&path).unwrap_or_else(|error| panic!("{error}"));
                out.insert(path, hash);
            }
        }
    }
    out
}

fn site(root: &Case) -> PathBuf {
    root.join("bazel-bin/site")
}

/// Every rendered page under one language directory of the site, joined.
fn pages(root: &Case, language: &str) -> String {
    let dir = site(root).join("api").join(language);
    let mut text = String::new();
    let mut stack = vec![dir.clone()];
    while let Some(next) = stack.pop() {
        for entry in std::fs::read_dir(&next).unwrap_or_else(|error| panic!("{next:?}: {error}")) {
            let path = entry
                .unwrap_or_else(|error| panic!("entry: {error}"))
                .path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|ext| ext == "html") {
                text.push_str(&read(&path));
            }
        }
    }
    text
}

/// The 1-based line of the first line of `text` that contains `needle`.
fn line_of(text: &str, needle: &str) -> usize {
    text.lines()
        .position(|line| line.contains(needle))
        .unwrap_or_else(|| panic!("{needle} missing"))
        + 1
}

#[test]
fn dx_docs_renders_vue_components_without_running_consumer_code() {
    let root = consumer("component_docs_render", DEPS);
    let before = hashes(&root.join("ui"));
    let built = docs(&root);
    assert_eq!(built.status.code(), Some(0), "{}", built.combined());
    assert_eq!(hashes(&root.join("ui")), before, "dx docs changed a source");
    assert!(
        !root.join("vite_config_ran.marker").exists(),
        "the consumer's vite config ran"
    );

    let pages = pages(&root, "vue");
    for expected in [
        "Text shown inside the button.",
        "Optional size in pixels.",
        "id: number",
        "Content of the button.",
        "Inline style of the swatch.",
        "Properties",
    ] {
        assert!(pages.contains(expected), "{expected} missing\n{pages}");
    }
    let line = line_of(BUTTON, "label: string;");
    let link = format!("href=\"https://example.com/consumer/blob/main/ui/src/Button.vue#L{line}\"");
    assert!(pages.contains(&link), "{link} missing\n{pages}");
    let index = read(&site(&root).join("searchindex.js"));
    assert!(
        index.contains("api/vue/"),
        "the search index never names a Vue page"
    );
}

#[test]
fn a_type_error_fails_without_a_stub_page() {
    let root = consumer("component_docs_type_error", DEPS);
    let broken = BUTTON.replace("size: 12,", "size: \"twelve\",");
    write(&root.join("ui/src/Button.vue"), &broken);
    let before = hashes(&root.join("ui"));
    let built = docs(&root);
    assert_ne!(built.status.code(), Some(0), "{}", built.combined());
    assert!(
        built.combined().contains("src/Button.vue: TS2322"),
        "{}",
        built.combined()
    );
    assert!(
        !site(&root).join("api/vue").exists(),
        "a failed build left Vue pages behind"
    );
    assert_eq!(hashes(&root.join("ui")), before, "dx docs changed a source");
}

#[test]
fn a_missing_type_dependency_fails() {
    let root = consumer("component_docs_missing_dep", "[]");
    let before = hashes(&root.join("ui"));
    let built = docs(&root);
    assert_ne!(built.status.code(), Some(0), "{}", built.combined());
    let output = built.combined();
    assert!(output.contains("Swatch.vue"), "{output}");
    assert!(output.contains("TS2307"), "{output}");
    assert!(
        !site(&root).join("api/vue").exists(),
        "a failed build left Vue pages behind"
    );
    assert_eq!(hashes(&root.join("ui")), before, "dx docs changed a source");
}
