use std::path::{Path, PathBuf};

use dx_nested::{self as nested, Case};
use dx_testing::resolve_runfiles;

const MODULE: &str = r#"module(name = "consumer")

bazel_dep(name = "rules_dx", version = "0.0.0", repo_name = "acme_rules")
local_path_override(module_name = "rules_dx", path = "RULES_DX")

bazel_dep(name = "platforms", version = "1.1.0")
bazel_dep(name = "rules_rust", version = "0.75.0")
bazel_dep(name = "rules_jvm_external", version = "7.1")
bazel_dep(name = "aspect_rules_js", version = "ASPECT_RULES_JS")

npm = use_extension("@aspect_rules_js//npm:extensions.bzl", "npm")
npm.npm_translate_lock(
    name = "npm",
    pnpm_lock = "//:pnpm-lock.yaml",
)
use_repo(npm, "npm")

MAVEN_BLOCK
"#;

const FILES: [&str; 14] = [
    "package.json",
    "pnpm-lock.yaml",
    "pnpm-workspace.yaml",
    "tsconfig.json",
    "vite.config.mjs",
    "index.html",
    "public/robots.txt",
    "site_client.test.js",
    "site_ssr.test.js",
    "src/App.svelte",
    "src/entry-server.ts",
    "src/env.d.ts",
    "src/main.ts",
    "src/components/Counter.svelte",
];

const MORE_FILES: [&str; 2] = ["src/components/Header.svelte", "src/assets/logo.png"];

fn read(path: &Path) -> Vec<u8> {
    std::fs::read(path).unwrap_or_else(|error| panic!("read {path:?}: {error}"))
}

fn write(path: &Path, bytes: &[u8]) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap_or_else(|error| panic!("mkdir: {error}"));
    }
    std::fs::write(path, bytes).unwrap_or_else(|error| panic!("write {path:?}: {error}"));
}

fn rules_dx_root() -> PathBuf {
    let source = resolve_runfiles("svelte/tests/consumer/app_consumer_test.rs");
    let real = std::fs::canonicalize(&source).unwrap_or_else(|error| panic!("{source:?}: {error}"));
    real.ancestors()
        .nth(4)
        .unwrap_or_else(|| panic!("no workspace above {real:?}"))
        .to_path_buf()
}

fn pin(text: &str, name: &str) -> String {
    let marker = format!("{name} = \"");
    let start = text
        .find(&marker)
        .unwrap_or_else(|| panic!("{name} missing from versions.bzl"))
        + marker.len();
    let end = text[start..].find('"').expect("closing quote") + start;
    text[start..end].to_owned()
}

fn consumer() -> (Case, Vec<(PathBuf, Vec<u8>)>) {
    let rules = rules_dx_root();
    let versions = String::from_utf8_lossy(&read(&rules.join("modules/versions.bzl"))).into_owned();
    let case = nested::case("svelte-consumer");
    let root = case.root().to_path_buf();
    write(
        &root.join(".bazelversion"),
        &read(&rules.join(".bazelversion")),
    );
    write(
        &root.join("MODULE.bazel"),
        MODULE
            .replace("MAVEN_BLOCK", nested::CONSUMER_MAVEN)
            .replace("RULES_DX", &rules.display().to_string())
            .replace(
                "ASPECT_RULES_JS",
                &pin(&versions, "ASPECT_RULES_JS_VERSION"),
            )
            .as_bytes(),
    );
    nested::seed_rules_dx_lock(&root, &rules);
    write(
        &root.join(".bazelrc"),
        dx_testing::preset_rc(&rules).as_bytes(),
    );
    nested::stage_consumer_maven_lock(&root);
    let build = String::from_utf8_lossy(&read(&resolve_runfiles(
        "svelte/tests/fixtures/svelte-app/consumer.BUILD.bazel",
    )))
    .replace("@rules_dx//", "@acme_rules//");
    write(&root.join("BUILD.bazel"), build.as_bytes());
    let mut own = Vec::new();
    for file in FILES.iter().chain(MORE_FILES.iter()) {
        let bytes = read(&resolve_runfiles(&format!("svelte/tests/fixtures/svelte-app/{file}")));
        write(&root.join(file), &bytes);
        own.push((root.join(file), bytes));
    }
    (case, own)
}

fn bazel(case: &Case, verb: &str, targets: &[&str]) -> (Option<i32>, String) {
    let mut args = vec![verb, "--color=no", "--curses=no", "--test_output=errors"];
    args.extend_from_slice(targets);
    let run = case.bazel(&args);
    (run.status.code(), run.combined())
}

#[test]
fn the_app_builds_checks_and_tests_through_a_renamed_rules_dx() {
    let (case, _) = consumer();
    let (code, text) = bazel(
        &case,
        "test",
        &[
            "//:check",
            "//:components",
            "//:site",
            "//:site_ssr_test",
            "//:site_client_test",
        ],
    );
    assert_eq!(code, Some(0), "{text}");
    for name in ["//:site_ssr_test", "//:site_client_test"] {
        let line = text
            .lines()
            .find(|line| line.starts_with(name))
            .unwrap_or_else(|| panic!("{name} did not run:\n{text}"));
        assert!(line.contains("PASSED"), "{line}");
    }
}

#[test]
fn dx_check_leaves_the_consumer_sources_unchanged() {
    let (case, own) = consumer();
    let workspace = case.to_string_lossy().into_owned();
    let dx = resolve_runfiles(&std::env::var("DX_BIN").expect("DX_BIN"));
    let run = case.run(
        &dx,
        &["check", "--apply", "--workspace", &workspace, "//:all"],
        &[],
    );
    assert_eq!(run.status.code(), Some(0), "{}", run.combined());
    for (path, bytes) in own {
        assert_eq!(read(&path), bytes, "{path:?} changed");
    }
}
