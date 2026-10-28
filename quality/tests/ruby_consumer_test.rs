use std::path::Path;

use dx_nested::{self as nested, Case};
use dx_test_scratch::scratch;
use dx_testing::{read, resolve_runfiles, rules_dx_source, write, Run};

const TYPES: &str =
    "# frozen_string_literal: true\n\n# Shared prefix.\nmodule Types\n  PREFIX = \"hello \"\nend\n";

const GREETER_DIRTY: &str = "# frozen_string_literal: true\n\n# Greets.\nmodule Core\n  def self.greet\n    'hello'\n  end\nend\n";

const GREETER_FORMATTED: &str = "# frozen_string_literal: true\n\n# Greets.\nmodule Core\n  def self.greet\n    \"hello\"\n  end\nend\n";

const LINTABLE: &str = "class Greeter\n  def greet\n    \"hello\"\n  end\nend\n";

const PROGRAM: &str = "# frozen_string_literal: true\n\nputs 42\n";

const SYNTAX_ERROR: &str =
    "# frozen_string_literal: true\n\nmodule Core\n  def self.greet\n    )\n  end\nend\n";

const RELAXED: &str =
    "Style/Documentation:\n  Enabled: false\nStyle/FrozenStringLiteralComment:\n  Enabled: false\nStyle/StringLiterals:\n  EnforcedStyle: double_quotes\n";

const MODULE: &str = r#"module(name = "consumer")

bazel_dep(name = "rules_dx", version = "0.0.0")
local_path_override(module_name = "rules_dx", path = "RULES_DX")

bazel_dep(name = "rules_rust", version = "0.75.0")
bazel_dep(name = "rules_ruby", version = "0.29.0")
bazel_dep(name = "rules_jvm_external", version = "7.1")

MAVEN_BLOCK

ruby = use_extension("@rules_ruby//ruby:extensions.bzl", "ruby")
ruby.toolchain(
    name = "ruby",
    portable_ruby = True,
    portable_ruby_checksums = {
        "ruby-3.4.12.arm64_darwin.tar.gz": "690e1eb8bb715a136029a62a60de50d5b463e275666a81246ca09a1e8f8ac1c2",
        "ruby-3.4.12.arm64_linux.tar.gz": "e36011c92255185e2e76f409cc1011bb652425bec6bb6e2800aadfb2e1494566",
        "ruby-3.4.12.x86_64_darwin.tar.gz": "4927c3f886fd89e37bcec36cc7f9f4b0367c6f81381912a0327aafa916971778",
        "ruby-3.4.12.x86_64_linux.tar.gz": "885185fabbc01bf695550d7375e610ae85084f5340474c837e5d00ec6c6cc234",
    },
    version = "3.4.12",
)
ruby.bundle_fetch(
    name = "dx_bundle",
    GEM_CHECKSUMS
    gemfile = "//:Gemfile",
    gemfile_lock = "//:Gemfile.lock",
)
use_repo(ruby, "dx_bundle", "ruby_toolchains")

register_toolchains("@ruby_toolchains//:all")
"#;

const BUILD: &str = r#"load("@rules_dx//quality:native_config.bzl", "rubocop_config")
load("@rules_dx//quality:policy.bzl", "quality_family", "workspace_policy")

quality_family(
    name = "ruby_family",
    family_id = "ruby",
    disabled = [
        "audit",
        "typecheck",
    ],
    format = ["standardrb"],
    lint = ["rubocop"],
)

workspace_policy(
    name = "policy",
    families = [":ruby_family"],
)

rubocop_config(
    name = "relaxed",
    src = "relaxed.rubocop.yml",
    visibility = ["//visibility:public"],
)
"#;

const CORE_BUILD: &str = r#"load("@rules_dx//ruby/rules:defs.bzl", "ruby_library", "ruby_test")

ruby_library(
    name = "core",
    aspect_hints = HINTS,
    srcs = [
        "greeter.rb",
        "types.rb",
    ],
    visibility = ["//visibility:public"],
)

ruby_test(
    name = "greeter_spec",
    aspect_hints = HINTS,
    srcs = ["greeter_spec.rb"],
    args = ["core/greeter_spec.rb"],
    main = "@dx_bundle//bin:rspec",
    deps = [
        ":core",
        "@dx_bundle",
    ],
)
"#;

const SPEC: &str = "# frozen_string_literal: true\n\nrequire_relative \"greeter\"\n\nRSpec.describe Core do\n  it \"greets\" do\n    expect(Core.greet).to eq(\"hello\")\n  end\nend\n";

const APP_BUILD: &str = r#"load("@rules_dx//ruby/rules:defs.bzl", "ruby_binary")

ruby_binary(
    name = "app",
    srcs = ["main.rb"],
    main = "main.rb",
    deps = ["//core"],
)
"#;

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

const TOOL_FLAGS: &str = "build --@rules_dx//quality/tools/ruby:rubocop=@dx_bundle//bin:rubocop\nbuild --@rules_dx//quality/tools/ruby:standardrb=@dx_bundle//bin:standardrb\n";

fn consumer() -> Case {
    consumer_with("")
}

fn consumer_with(extra_rc: &str) -> Case {
    let rules = rules_dx_source();
    let root = nested::case("consumer");
    write(
        &root.join(".bazelversion"),
        &read(&rules.join(".bazelversion")),
    );
    write(
        &root.join("MODULE.bazel"),
        &MODULE
            .replace("MAVEN_BLOCK", nested::CONSUMER_MAVEN)
            .replace("RULES_DX", &rules.display().to_string())
            .replace("GEM_CHECKSUMS", &gem_checksums(&rules)),
    );
    write(
        &root.join("Gemfile"),
        &read(&rules.join("third_party/ruby/Gemfile")),
    );
    write(
        &root.join("Gemfile.lock"),
        &read(&rules.join("third_party/ruby/Gemfile.lock")),
    );
    write(
        &root.join(".bazelrc"),
        &format!("common --enable_bzlmod\nbuild --java_runtime_version=remotejdk_21\nbuild --tool_java_runtime_version=remotejdk_21\n{TOOL_FLAGS}{extra_rc}"),
    );
    write(&root.join("BUILD.bazel"), BUILD);
    nested::stage_consumer_maven_lock(&root);
    write(&root.join("relaxed.rubocop.yml"), RELAXED);
    write(
        &root.join("core/BUILD.bazel"),
        &CORE_BUILD.replace("HINTS", "[]"),
    );
    write(&root.join("core/types.rb"), TYPES);
    write(&root.join("core/greeter.rb"), GREETER_DIRTY);
    write(&root.join("core/greeter_spec.rb"), SPEC);
    write(&root.join("app/BUILD.bazel"), APP_BUILD);
    write(&root.join("app/main.rb"), PROGRAM);
    root
}

fn dx(root: &Case, command: &str, args: &[&str]) -> Run {
    dx_with_env(root, command, args, &[], &[])
}

fn dx_with_env(
    root: &Case,
    command: &str,
    args: &[&str],
    flags: &[&str],
    envs: &[(&str, &str)],
) -> Run {
    let rel = std::env::var("DX_BIN").unwrap_or_else(|_| panic!("DX_BIN must name dx"));
    let workspace = root.to_string_lossy().into_owned();
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
    root.run(&resolve_runfiles(&rel), &argv, envs)
}

#[test]
fn format_keeps_bytes_apply_edits_owned_files_and_recheck_passes() {
    let root = consumer();
    let greeter = root.join("core/greeter.rb");
    let program = root.join("app/main.rb");
    let types = root.join("core/types.rb");

    let dirty = dx(&root, "format", &[]);
    assert_eq!(dirty.status.code(), Some(1), "{}", dirty.combined());
    assert!(
        dirty.combined().contains("core/greeter.rb:"),
        "{}",
        dirty.combined()
    );
    assert!(
        !dirty.combined().contains("core/types.rb"),
        "{}",
        dirty.combined()
    );
    assert_eq!(read(&greeter), GREETER_DIRTY);
    assert_eq!(read(&types), TYPES);
    assert_eq!(read(&program), PROGRAM);

    let applied = dx(&root, "format", &["--apply"]);
    assert_eq!(applied.status.code(), Some(0), "{}", applied.combined());
    assert_eq!(read(&greeter), GREETER_FORMATTED);
    assert_eq!(read(&types), TYPES);
    assert_eq!(read(&program), PROGRAM);

    let clean = dx(&root, "format", &[]);
    assert_eq!(clean.status.code(), Some(0), "{}", clean.combined());
    assert_eq!(read(&greeter), GREETER_FORMATTED);
}

#[test]
fn lint_reports_cops_and_a_bound_config_changes_them() {
    let root = consumer();
    let greeter = root.join("core/greeter.rb");
    write(&greeter, LINTABLE);

    let dirty = dx(&root, "lint", &[]);
    assert_eq!(dirty.status.code(), Some(1), "{}", dirty.combined());
    assert!(
        dirty.combined().contains("core/greeter.rb:"),
        "{}",
        dirty.combined()
    );
    assert!(
        dirty.combined().contains("Style/Documentation"),
        "{}",
        dirty.combined()
    );
    assert_eq!(read(&greeter), LINTABLE);

    write(
        &root.join("core/BUILD.bazel"),
        &CORE_BUILD.replace("HINTS", "[\"//:relaxed\"]"),
    );
    let relaxed = dx(&root, "lint", &[]);
    assert_eq!(relaxed.status.code(), Some(0), "{}", relaxed.combined());
}

#[test]
fn a_syntax_error_is_reported_with_its_severity_and_line() {
    let root = consumer();
    let greeter = root.join("core/greeter.rb");
    write(&greeter, SYNTAX_ERROR);

    for command in ["lint", "format"] {
        let out = dx(&root, command, &[]);
        let text = out.combined();
        assert_eq!(out.status.code(), Some(1), "{command}: {text}");
        let line = text
            .lines()
            .find(|line| line.contains("core/greeter.rb:5:"))
            .unwrap_or_else(|| panic!("{command}: no finding at line 5: {text}"));
        assert!(line.contains("unexpected token"), "{command}: {line}");
        assert!(line.to_lowercase().contains("error"), "{command}: {line}");
        assert_eq!(read(&greeter), SYNTAX_ERROR);
    }
}

#[test]
fn an_unreadable_config_fails_the_run_and_never_counts_as_clean() {
    let root = consumer();
    write(&root.join("relaxed.rubocop.yml"), "Style/Documentation: [");
    write(
        &root.join("core/BUILD.bazel"),
        &CORE_BUILD.replace("HINTS", "[\"//:relaxed\"]"),
    );
    let out = dx(&root, "lint", &[]);
    let text = out.combined();
    assert_eq!(out.status.code(), Some(1), "{text}");
    assert!(text.contains("invalid tool output for rubocop"), "{text}");
    assert!(text.contains("analysis_failed"), "{text}");
    assert!(!text.contains("core/greeter.rb:"), "{text}");
}

#[test]
fn tools_ignore_ambient_gems_and_bundler_and_install_nothing() {
    let home_dir = scratch("ruby-home");
    let home = home_dir.path();
    let home_text = home.to_string_lossy().into_owned();
    let gems = home.join("gems").to_string_lossy().into_owned();
    let rc = format!(
        "build --action_env=GEM_HOME={gems}\nbuild --action_env=GEM_PATH={gems}\nbuild --action_env=BUNDLE_GEMFILE=/nonexistent/Gemfile\nbuild --action_env=BUNDLE_APP_CONFIG={home_text}\nbuild --action_env=RUBYLIB=/nonexistent\nbuild --action_env=RUBYOPT=-r/nonexistent/hook\n"
    );
    let root = consumer_with(&rc);

    let dirty = dx(&root, "format", &[]);
    assert_eq!(dirty.status.code(), Some(1), "{}", dirty.combined());
    assert!(
        dirty.combined().contains("core/greeter.rb:"),
        "{}",
        dirty.combined()
    );
    write(&root.join("core/greeter.rb"), LINTABLE);
    let linted = dx(&root, "lint", &[]);
    assert_eq!(linted.status.code(), Some(1), "{}", linted.combined());
    assert!(
        linted.combined().contains("Style/Documentation"),
        "{}",
        linted.combined()
    );
    assert!(!home.join("gems").exists());
    assert!(!home.join(".bundle").exists());
}

#[test]
fn rspec_runs_from_the_pinned_bundle() {
    let root = consumer();
    let out = root.bazel(&["test", "//core:greeter_spec"]);
    assert_eq!(out.status.code(), Some(0), "{}", out.combined());
    assert!(out.combined().contains("PASSED"), "{}", out.combined());
}

#[test]
fn an_uncached_bundle_with_downloads_blocked_fails_without_ambient_gems() {
    let root = consumer();
    let expunged = root.bazel(&["clean", "--expunge"]);
    assert!(expunged.status.success(), "{}", expunged.combined());
    let cache_dir = scratch("ruby-empty-cache");
    let cache = cache_dir.path();
    let cache_flag = format!("--repository_cache={}", cache.join("repos").display());
    let out = dx_with_env(
        &root,
        "format",
        &[],
        &[
            cache_flag.as_str(),
            "--experimental_repository_disable_download",
        ],
        &[],
    );
    let text = out.combined();
    assert_eq!(out.status.code(), Some(1), "{text}");
    assert!(!text.contains("core/greeter.rb:"), "{text}");
}
