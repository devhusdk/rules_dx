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
bazel_dep(name = "rules_py", version = "RULES_PY")
archive_override(
    module_name = "rules_py",
    integrity = "RULES_PY_INTEGRITY",
    strip_prefix = "rules_py-RULES_PY",
    urls = ["https://github.com/aspect-build/rules_py/releases/download/vRULES_PY/rules_py-vRULES_PY.tar.gz"],
)

single_version_override(
    module_name = "rules_julia",
    patch_strip = 1,
    patches = [
        "//patches:package_name.patch",
        "//patches:portable_cpu_target.patch",
    ],
    version = "RULES_JULIA",
)

npm = use_extension("@aspect_rules_js//npm:extensions.bzl", "npm")
npm.npm_translate_lock(
    name = "npm",
    pnpm_lock = "//:pnpm-lock.yaml",
)
use_repo(npm, "npm")

MAVEN_BLOCK
"#;

const FILES: [&str; 15] = [
    "biome.json",
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
    let module = String::from_utf8_lossy(&read(&rules.join("MODULE.bazel"))).into_owned();
    let rules_py = &module[module
        .find("module_name = \"rules_py\"")
        .expect("rules_py override")..];
    let rules_py_integrity = pin(rules_py, "integrity");
    let rules_julia = &module[module
        .find("module_name = \"rules_julia\"")
        .expect("rules_julia override")..];
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
            .replace("RULES_PY_INTEGRITY", &rules_py_integrity)
            .replace("RULES_JULIA", &pin(rules_julia, "version"))
            .replace("RULES_PY", &pin(&versions, "RULES_PY_VERSION"))
            .replace(
                "ASPECT_RULES_JS",
                &pin(&versions, "ASPECT_RULES_JS_VERSION"),
            )
            .as_bytes(),
    );
    write(
        &root.join("patches/BUILD.bazel"),
        b"exports_files(glob([\"*.patch\"]))\n",
    );
    for patch in ["package_name.patch", "portable_cpu_target.patch"] {
        let bytes = read(&rules.join("patches/rules_julia").join(patch));
        write(&root.join("patches").join(patch), &bytes);
    }
    nested::seed_rules_dx_lock(&root, &rules);
    write(
        &root.join(".bazelrc"),
        dx_testing::preset_rc(&rules)
            .replace("@rules_dx//", "@acme_rules//")
            .as_bytes(),
    );
    nested::stage_consumer_maven_lock(&root);
    let build = String::from_utf8_lossy(&read(&resolve_runfiles(
        "svelte/tests/fixtures/svelte-app/consumer.BUILD.bazel",
    )))
    .replace("@rules_dx//", "@acme_rules//");
    write(&root.join("BUILD.bazel"), build.as_bytes());
    let mut own = Vec::new();
    for file in FILES.iter().chain(MORE_FILES.iter()) {
        let bytes = read(&resolve_runfiles(&format!(
            "svelte/tests/fixtures/svelte-app/{file}"
        )));
        write(&root.join(file), &bytes);
        own.push((root.join(file), bytes));
    }
    (case, own)
}

fn bazel(case: &Case, verb: &str, extra: &[&str], targets: &[&str]) -> (Option<i32>, String) {
    let mut args = vec![verb, "--color=no", "--curses=no"];
    if verb == "test" {
        args.push("--test_output=errors");
    }
    args.extend_from_slice(extra);
    args.extend_from_slice(targets);
    let run = case.bazel(&args);
    (run.status.code(), run.combined())
}

#[test]
fn the_app_builds_checks_and_tests_through_a_renamed_rules_dx() {
    let (case, _) = consumer();
    let targets = [
        "//:check",
        "//:components",
        "//:site",
        "//:site_ssr_test",
        "//:site_client_test",
    ];
    let (code, text) = bazel(&case, "fetch", &[], &targets);
    assert_eq!(code, Some(0), "fetch failed:\n{text}");
    let (code, text) = bazel(&case, "test", &["--repository_disable_download"], &targets);
    assert_eq!(code, Some(0), "offline test failed:\n{text}");
    for name in ["//:site_ssr_test", "//:site_client_test"] {
        let line = text
            .lines()
            .find(|line| line.starts_with(name))
            .unwrap_or_else(|| panic!("{name} did not run:\n{text}"));
        assert!(line.contains("PASSED"), "{line}");
    }
}

const UNFORMATTED: &str = r#"<script lang="ts">
let   count:number=0
  function  inc(){count+=1}
</script>

<button   on:click={inc}>
clicked {count}
</button>

<style>
button{color:red;
    margin:0}
</style>
"#;

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
    let seen = std::fs::read_to_string(case.root().join("BUILD.bazel")).unwrap_or_default();
    let files: Vec<_> = ["src", "src/components", "src/assets"]
        .iter()
        .map(|dir| {
            let mut names: Vec<_> = std::fs::read_dir(case.root().join(dir))
                .map(|entries| entries.flatten().map(|e| e.file_name()).collect())
                .unwrap_or_default();
            names.sort();
            (dir, names)
        })
        .collect();
    assert_eq!(
        run.status.code(),
        Some(0),
        "{}\nBUILD.bazel:\n{seen}\nsources present: {files:?}",
        run.combined()
    );
    for (path, bytes) in own {
        assert_eq!(read(&path), bytes, "{path:?} changed");
    }
}

#[test]
fn dx_format_applies_to_the_consumer_svelte_files() {
    let (case, own) = consumer();
    let workspace = case.to_string_lossy().into_owned();
    let dx = resolve_runfiles(&std::env::var("DX_BIN").expect("DX_BIN"));
    let target = case.root().join("src/components/Unformatted.svelte");
    write(&target, UNFORMATTED.as_bytes());
    let run = case.run(
        &dx,
        &["format", "--apply", "--workspace", &workspace, "//:all"],
        &[],
    );
    assert_eq!(run.status.code(), Some(0), "{}", run.combined());
    assert_ne!(read(&target), UNFORMATTED.as_bytes(), "not formatted");
    for (path, bytes) in own {
        assert_eq!(read(&path), bytes, "{path:?} changed");
    }
    let run = case.run(&dx, &["format", "--workspace", &workspace, "//:all"], &[]);
    assert_eq!(run.status.code(), Some(0), "{}", run.combined());
}
