use std::path::{Path, PathBuf};

use dx_testing::nested;
use dx_testing::resolve_runfiles;

const COMMON_NAMES: &[(&str, &str)] = &[
    ("npm", "@npm//:all"),
    ("ruby", "@ruby_toolchains//:all"),
    ("bundle", "@bundle//:all"),
];

const MODULE: &str = r#"module(name = "consumer")

bazel_dep(name = "rules_dx", version = "0.0.0")
local_path_override(module_name = "rules_dx", path = "RULES_DX")

bazel_dep(name = "aspect_rules_js", version = "3.5.2")
bazel_dep(name = "rules_ruby", version = "0.28.0")

npm = use_extension("@aspect_rules_js//npm:extensions.bzl", "npm")
npm.npm_translate_lock(
    name = "npm",
    npmrc = "//:.npmrc",
    pnpm_lock = "//:pnpm-lock.yaml",
    verify_node_modules_ignored = "//:.bazelignore",
)
use_repo(npm, "npm")

ruby = use_extension("@rules_ruby//ruby:extensions.bzl", "ruby")
ruby.toolchain(
    name = "ruby",
    portable_ruby = True,
    version = "3.3.6",
)
ruby.bundle_fetch(
    name = "bundle",
    GEM_CHECKSUMS
    gemfile = "//:Gemfile",
    gemfile_lock = "//:Gemfile.lock",
)
use_repo(ruby, "bundle", "ruby", "ruby_toolchains")

register_toolchains("@ruby_toolchains//:all")
"#;

const PNPM_LOCK: &str = "lockfileVersion: '9.0'\n\nsettings:\n  autoInstallPeers: true\n  excludeLinksFromLockfile: false\n\nimporters:\n\n  .: {}\n";

const BUILD: &str = "load(\"@npm//:defs.bzl\", \"npm_link_all_packages\")\n\nnpm_link_all_packages(name = \"node_modules\")\n";

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|error| panic!("read {path:?}: {error}"))
}

fn write(path: &Path, text: &str) {
    std::fs::write(path, text).unwrap_or_else(|error| panic!("write {path:?}: {error}"));
}

fn rules_dx_root() -> PathBuf {
    let source = resolve_runfiles("modules/consumer_names_test.rs");
    let real = std::fs::canonicalize(&source).unwrap_or_else(|error| panic!("{source:?}: {error}"));
    real.ancestors()
        .nth(2)
        .unwrap_or_else(|| panic!("no workspace above {real:?}"))
        .to_path_buf()
}

fn gem_checksums(rules: &Path) -> String {
    let module = read(&rules.join("MODULE.bazel"));
    let start = module
        .find("gem_checksums = {")
        .unwrap_or_else(|| panic!("no gem_checksums in MODULE.bazel"));
    let end = module[start..]
        .find("},")
        .unwrap_or_else(|| panic!("unterminated gem_checksums"));
    module[start..start + end + 2].to_string()
}

#[test]
fn common_upstream_repo_names_resolve_next_to_rules_dx() {
    let case = nested::case("consumer-names");
    let root = case.root();
    let rules = rules_dx_root();
    write(
        &root.join(".bazelversion"),
        &read(&rules.join(".bazelversion")),
    );
    write(
        &root.join("MODULE.bazel"),
        &MODULE
            .replace("RULES_DX", &rules.display().to_string())
            .replace("GEM_CHECKSUMS", &gem_checksums(&rules)),
    );
    write(
        &root.join(".bazelrc"),
        &format!(
            "{}build --java_runtime_version=remotejdk_21\nbuild --tool_java_runtime_version=remotejdk_21\n",
            read(&rules.join("tools/bazelrc/preset.bazelrc"))
        ),
    );
    write(&root.join(".npmrc"), "hoist=false\n");
    write(&root.join(".bazelignore"), "node_modules\n");
    write(&root.join("pnpm-lock.yaml"), PNPM_LOCK);
    write(
        &root.join("pnpm-workspace.yaml"),
        "packages:\n  - .\nallowBuilds: {}\n",
    );
    write(
        &root.join("package.json"),
        "{\n  \"name\": \"consumer\",\n  \"private\": true\n}\n",
    );
    write(&root.join("BUILD.bazel"), BUILD);
    write(
        &root.join("Gemfile"),
        &read(&rules.join("third_party/ruby/Gemfile")),
    );
    write(
        &root.join("Gemfile.lock"),
        &read(&rules.join("third_party/ruby/Gemfile.lock")),
    );

    for (name, pattern) in COMMON_NAMES {
        let run = case.bazel(&["query", "--color=no", "--curses=no", pattern]);
        assert!(run.status.success(), "{name}: {}", run.combined());
        assert!(!run.stdout.trim().is_empty(), "{name}: {}", run.combined());
    }

    let run = case.bazel(&[
        "run",
        "--color=no",
        "--curses=no",
        "@rules_dx//:dx",
        "--",
        "--help",
    ]);
    assert!(run.status.success(), "{}", run.combined());
    assert!(run.combined().contains("Usage: dx"), "{}", run.combined());
}
