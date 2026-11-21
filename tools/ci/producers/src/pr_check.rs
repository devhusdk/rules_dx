//! Analysis-only checks for a pull request on `linux_x86_64`, one named set at a time.

use std::process::{Command, Stdio};

const BAZEL: &str = "bazel";

pub const SETS: [&str; 8] = [
    "workflow",
    "preflight",
    "preflight-windows-arm64",
    "android-linux-arm64",
    "hermetic",
    "docs",
    "cli",
    "kotlin-bench",
];

pub const WORKFLOW_TARGETS: &str = "//tools/ci:actionlint_test //tools/ci:workflow_expressions_test
    //tools/ci:secret_flags_test //tools/ci:workflow_pins_test";

pub const PREFLIGHT_TARGETS: &str = "//cli/cli:dx //cc/tests/fixtures/hello:hello //cc/tests/fixtures/c_app/store:store_build_h
    //tools/testing:preflight_smoke_test //cli/env:bootstrap_test //cc/tests/fixtures/hello:hello_test
    //powershell/tests/fixtures/hello:hello_test //zig/tests/fixtures/zig_app/... //dart/tests/fixtures/dart_app/...
    //dart/tests/runtime:all //dart/private/tools:all //zig/tests/fixtures/hello:all //julia/tests/fixtures/hello:all
    //julia/tests/fixtures/julia_app/... //julia/private/tools/... //deploy/private:pypi_probe_verify //haskell/private/tools/weeder:weeder_build_test
    //core/quality/testdata:matrix_jupyter_format_pass //core/quality/testdata:matrix_jupyter_lint_fail
    //core/quality/testdata:matrix_jupyter_outputs_fail //core/quality/testdata:matrix_api_spec_openapi_pass
    //core/quality/testdata:matrix_yaml_format_pass //core/quality/testdata:matrix_yaml_format_fail //core/quality/testdata:matrix_yaml_format_syntax_error
    //core/quality/testdata:matrix_yaml_lint_pass //core/quality/testdata:matrix_yaml_lint_fail //core/quality/testdata:matrix_yaml_lint_syntax_error
    //core/quality/testdata:matrix_flatbuffers_lint_pass //core/quality/testdata:matrix_capnp_lint_pass //core/quality/testdata:matrix_thrift_lint_pass
    //core/quality/testdata:matrix_thrift_lint_warning_fail //core/quality/testdata:matrix_thrift_lint_syntax_error //core/quality/testdata:matrix_thrift_lint_semantic_fail
    //core/quality/testdata:matrix_thrift_lint_include_undeclared //core/quality/testdata:matrix_thrift_typecheck_pass //core/quality/testdata:matrix_thrift_typecheck_semantic_fail
    //core/quality/testdata:matrix_thrift_typecheck_warning_ignored //core/quality/testdata:matrix_avro_lint_avsc_pass //core/quality/testdata:matrix_avro_lint_avsc_undefined_name
    //core/quality/testdata:matrix_avro_lint_avsc_syntax_error //core/quality/testdata:matrix_avro_lint_avdl_import_pass //core/quality/testdata:matrix_avro_lint_avdl_syntax_error
    //core/quality/testdata:matrix_avro_typecheck_avsc_pass //core/quality/testdata:matrix_avro_typecheck_avsc_undefined_name";

pub const WINDOWS_ARM64_TARGETS: &str = "//cc/tests/fixtures/hello:hello //cc/tests/fixtures/hello:hello_test //powershell/tests/fixtures/hello:hello_test
    //cc/tests/fixtures/c_app/store:store_build_h
    //dart/tests/fixtures/dart_app/... //dart/tests/runtime:all //dart/private/tools:all
    //cli/cli:dx //tools/testing:preflight_smoke_test //cli/env:bootstrap_test //deploy/private:pypi_probe_verify
    //zig/tests/fixtures/hello:all //zig/tests/fixtures/zig_app/... //julia/tests/fixtures/hello:all
    //julia/tests/fixtures/julia_app/... //julia/private/tools/... //haskell/private/tools/weeder:weeder_build_test";

/// Preflight labels left out of `WINDOWS_ARM64_TARGETS`.
/// `runner_matrix_windows_arm64` has no erlang toolchain for `windows_arm64` (`//erlang/private/tools:dialyzer_bin`).
pub const WINDOWS_ARM64_BLOCKED: &str = "//core/quality/testdata:runner_matrix_windows_arm64";

/// Android is unsupported on `linux_arm64`, so every target under it must be skipped there.
pub const ANDROID_LINUX_ARM64_FLAGS: &str = "--platforms=//tools/platforms:linux_arm64
    --host_platform=//tools/platforms:linux_arm64 //android/...";

/// Path prefixes whose changes can make an Android target compatible with `linux_arm64`.
pub const ANDROID_LINUX_ARM64_PATHS: [&str; 3] = ["android/", "core/platforms/", "cc/"];

pub const KOTLIN_BENCH_TARGETS: &str = "//kotlin/tests/fixtures/bench:all";

pub const WINDOWS_ARM64_PLATFORM: &str = "--platforms=@llvm//platforms:windows_aarch64";

pub const CRATE_LOCK_CHECK: &str =
    "--repo_env=CARGO_BAZEL_DIGEST_CHECK=1 @crates @dx_quality_crates @dx_thrift_crates";

pub const HERMETIC_QUERY: &str = r#"attr(tags, hermetic-consumer, //...) except filter("_upstream$", //...) except filter("_msvc$", //...)"#;

const HERMETIC_TARGETS: [&str; 2] = [
    "//tools/testing/hermetic:isolation_probe_test",
    "//tools/testing/hermetic:consumer_repos_test",
];

/// One command of a set; a query step appends the labels it prints to the next `Run` step.
/// A `Silent` step discards its stdout and takes no query labels.
/// A `Changed` step ends the set early when the last commit touches none of its path prefixes.
#[derive(Debug, PartialEq, Eq)]
pub enum Step {
    Run(&'static str, Vec<String>),
    Query(Vec<String>),
    Silent(&'static str, Vec<String>),
    Changed(&'static [&'static str]),
}

fn words(list: &[&str], flags: &[String], tail: &str) -> Vec<String> {
    let mut args: Vec<String> = list.iter().map(ToString::to_string).collect();
    args.extend(flags.iter().cloned());
    args.extend(tail.split_whitespace().map(str::to_owned));
    args
}

/// Returns the commands of `set` with the extra bazel `flags`.
///
/// # Errors
///
/// Returns `Err` when `set` names no set.
pub fn plan(set: &str, flags: &[String]) -> Result<Vec<Step>, String> {
    let steps = match set {
        "workflow" => vec![Step::Run(BAZEL, words(&["test"], flags, WORKFLOW_TARGETS))],
        "preflight" => vec![
            Step::Run(
                BAZEL,
                words(
                    &["build", "--nobuild", "--lockfile_mode=error"],
                    flags,
                    PREFLIGHT_TARGETS,
                ),
            ),
            Step::Silent(
                BAZEL,
                words(
                    &["mod", "show_repo", "--lockfile_mode=error"],
                    flags,
                    CRATE_LOCK_CHECK,
                ),
            ),
        ],
        "preflight-windows-arm64" => {
            let mut args = words(
                &[
                    "build",
                    "--nobuild",
                    "--skip_incompatible_explicit_targets",
                    "--lockfile_mode=error",
                ],
                flags,
                WINDOWS_ARM64_PLATFORM,
            );
            args.extend(
                WINDOWS_ARM64_TARGETS
                    .split_whitespace()
                    .filter(|label| {
                        !WINDOWS_ARM64_BLOCKED
                            .split_whitespace()
                            .any(|b| b == *label)
                    })
                    .map(str::to_owned),
            );
            vec![Step::Run(BAZEL, args)]
        }
        "android-linux-arm64" => vec![
            Step::Changed(&ANDROID_LINUX_ARM64_PATHS),
            Step::Run(
                BAZEL,
                words(
                    &[
                        "build",
                        "--nobuild",
                        "--keep_going",
                        "--lockfile_mode=error",
                    ],
                    flags,
                    ANDROID_LINUX_ARM64_FLAGS,
                ),
            ),
        ],
        "hermetic" => {
            let mut query = words(&["query", "--lockfile_mode=error"], flags, "");
            query.extend([HERMETIC_QUERY.to_owned(), "--output=label".to_owned()]);
            vec![
                Step::Query(query),
                Step::Run(
                    BAZEL,
                    words(
                        &["build", "--nobuild", "--lockfile_mode=error"],
                        flags,
                        &HERMETIC_TARGETS.join(" "),
                    ),
                ),
            ]
        }
        "docs" => vec![
            Step::Run(
                "tools/dx",
                words(
                    &["lint", "//docs/...", "//tools/docs/...", "--", "--nobuild"],
                    flags,
                    "",
                ),
            ),
            Step::Run(
                BAZEL,
                words(&["build"], flags, "//tools/docs/site:user_api"),
            ),
        ],
        "cli" => vec![Step::Run(
            BAZEL,
            words(&["test"], flags, "//cli/cli:dx_cli_test"),
        )],
        "kotlin-bench" => vec![Step::Run(
            BAZEL,
            words(&["build"], flags, KOTLIN_BENCH_TARGETS),
        )],
        other => {
            return Err(format!(
                "unknown set: {other}; pick one of {} or all",
                SETS.join(", ")
            ));
        }
    };
    Ok(steps)
}

/// Returns whether a path in the newline-separated `diff` starts with one of `prefixes`.
fn touches(diff: &str, prefixes: &[&str]) -> bool {
    diff.lines()
        .any(|path| prefixes.iter().any(|prefix| path.starts_with(prefix)))
}

fn execute(steps: Vec<Step>) -> Result<(), String> {
    let mut found: Vec<String> = Vec::new();
    for step in steps {
        match step {
            Step::Changed(prefixes) => {
                // The checkout holds two commits: the PR merge commit and its base.
                let out = Command::new("git")
                    .args(["diff", "--name-only", "HEAD^1", "HEAD"])
                    .output()
                    .map_err(|error| format!("spawn git: {error}"))?;
                if !out.status.success() {
                    return Err(format!(
                        "git diff --name-only HEAD^1 HEAD failed: {}",
                        String::from_utf8_lossy(&out.stderr)
                    ));
                }
                if !touches(&String::from_utf8_lossy(&out.stdout), prefixes) {
                    println!("skipped: no change under {}", prefixes.join(" "));
                    return Ok(());
                }
            }
            Step::Query(args) => {
                let out = Command::new(BAZEL)
                    .args(&args)
                    .output()
                    .map_err(|error| format!("spawn bazel: {error}"))?;
                if !out.status.success() {
                    return Err(format!(
                        "bazel {} failed: {}",
                        args.join(" "),
                        String::from_utf8_lossy(&out.stderr)
                    ));
                }
                found = String::from_utf8_lossy(&out.stdout)
                    .split_whitespace()
                    .map(str::to_owned)
                    .collect();
                if found.is_empty() {
                    return Err(format!("bazel {} matched no targets", args.join(" ")));
                }
            }
            Step::Run(program, mut args) => {
                args.append(&mut found);
                let status = Command::new(program)
                    .args(&args)
                    .status()
                    .map_err(|error| format!("spawn {program}: {error}"))?;
                if !status.success() {
                    return Err(format!("{program} {} failed: {status}", args.join(" ")));
                }
            }
            Step::Silent(program, args) => {
                let status = Command::new(program)
                    .args(&args)
                    .stdout(Stdio::null())
                    .status()
                    .map_err(|error| format!("spawn {program}: {error}"))?;
                if !status.success() {
                    return Err(format!("{program} {} failed: {status}", args.join(" ")));
                }
            }
        }
    }
    Ok(())
}

/// Runs `pr-check SET [BAZEL_FLAGS...]`, where SET is one of `SETS` or `all`.
///
/// # Errors
///
/// Returns `Err` when the set is unknown or one of its commands fails.
pub fn check(args: &[String]) -> Result<(), String> {
    let Some((set, flags)) = args.split_first() else {
        return Err(format!(
            "usage: producers_bin --pr-check <{}|all> [bazel flags...]",
            SETS.join("|")
        ));
    };
    if set != "all" {
        return execute(plan(set, flags)?);
    }
    for each in SETS {
        println!("::group::{each}");
        execute(plan(each, flags)?)?;
        println!("::endgroup::");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{ANDROID_LINUX_ARM64_PATHS, CRATE_LOCK_CHECK, SETS, Step, plan, touches};

    fn flags() -> Vec<String> {
        vec!["--config=ci".to_owned()]
    }

    #[test]
    fn every_set_has_a_plan() {
        for set in SETS {
            assert!(!plan(set, &flags()).unwrap().is_empty(), "{set}");
        }
        assert!(plan("nope", &[]).unwrap_err().contains("unknown set: nope"));
    }

    #[test]
    fn flags_follow_the_command_options() {
        let steps = plan("preflight", &flags()).unwrap();
        let Some(Step::Run("bazel", args)) = steps.first() else {
            panic!("{steps:?}");
        };
        assert_eq!(
            args.get(..4).unwrap(),
            ["build", "--nobuild", "--lockfile_mode=error", "--config=ci"]
        );
        assert!(args.contains(&"//cli/cli:dx".to_owned()));
    }

    #[test]
    fn preflight_checks_the_crate_lockfiles() {
        let steps = plan("preflight", &flags()).unwrap();
        assert_eq!(
            steps.get(1),
            Some(&Step::Silent(
                "bazel",
                ["mod", "show_repo", "--lockfile_mode=error", "--config=ci"]
                    .into_iter()
                    .chain(CRATE_LOCK_CHECK.split(' '))
                    .map(str::to_owned)
                    .collect()
            ))
        );
    }

    #[test]
    fn windows_arm64_targets_the_platform() {
        let steps = plan("preflight-windows-arm64", &[]).unwrap();
        let Some(Step::Run("bazel", args)) = steps.first() else {
            panic!("{steps:?}");
        };
        for want in [
            "--platforms=@llvm//platforms:windows_aarch64",
            "--skip_incompatible_explicit_targets",
            "//dart/private/tools:all",
        ] {
            assert!(args.contains(&want.to_owned()), "{want}");
        }
    }

    #[test]
    fn android_linux_arm64_analyses_android_after_the_path_check() {
        let steps = plan("android-linux-arm64", &flags()).unwrap();
        assert_eq!(
            steps.first(),
            Some(&Step::Changed(&ANDROID_LINUX_ARM64_PATHS))
        );
        let Some(Step::Run("bazel", args)) = steps.get(1) else {
            panic!("{steps:?}");
        };
        assert_eq!(
            args.get(..5).unwrap(),
            [
                "build",
                "--nobuild",
                "--keep_going",
                "--lockfile_mode=error",
                "--config=ci"
            ]
        );
        for want in [
            "--platforms=//tools/platforms:linux_arm64",
            "--host_platform=//tools/platforms:linux_arm64",
            "//android/...",
        ] {
            assert!(args.contains(&want.to_owned()), "{want}");
        }
    }

    #[test]
    fn touches_matches_path_prefixes() {
        let prefixes = ANDROID_LINUX_ARM64_PATHS;
        assert!(touches("README.md\nandroid/defs.bzl\n", &prefixes));
        assert!(touches("core/platforms/BUILD.bazel", &prefixes));
        assert!(touches("cc/private/x.bzl", &prefixes));
        assert!(!touches("docs/cc/x.md\nrust/android/y.rs\n", &prefixes));
        assert!(!touches("", &prefixes));
    }

    #[test]
    fn kotlin_bench_builds_the_bench_fixtures() {
        let steps = plan("kotlin-bench", &flags()).unwrap();
        assert_eq!(
            steps,
            [Step::Run(
                "bazel",
                ["build", "--config=ci", "//kotlin/tests/fixtures/bench:all"]
                    .map(str::to_owned)
                    .to_vec()
            )]
        );
    }

    #[test]
    fn the_consumer_set_queries_then_builds() {
        let steps = plan("hermetic", &flags()).unwrap();
        assert!(matches!(steps.first(), Some(Step::Query(_))));
        let Some(Step::Run("bazel", args)) = steps.get(1) else {
            panic!("{steps:?}");
        };
        assert_eq!(args.get(..2).unwrap(), ["build", "--nobuild"]);
    }

    #[test]
    fn docs_lints_without_building() {
        let steps = plan("docs", &flags()).unwrap();
        assert_eq!(
            steps.first(),
            Some(&Step::Run(
                "tools/dx",
                [
                    "lint",
                    "//docs/...",
                    "//tools/docs/...",
                    "--",
                    "--nobuild",
                    "--config=ci"
                ]
                .map(str::to_owned)
                .to_vec()
            ))
        );
    }
}
