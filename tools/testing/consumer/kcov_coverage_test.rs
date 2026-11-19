//! A Zig consumer gets kcov coverage on Linux only from a `dx_prebuilt` kcov pin.
//!
//! With the pin, `bazel coverage` writes an LCOV report that names the library source. With no
//! pin the hub serves no kcov: the test still passes, writes no report, and the consumer fetches
//! no kcov source build.

use std::path::{Path, PathBuf};

use dx_nested::Case;

/// The zig_test of `zig/tests/consumer`.
const TEST: &str = "//:greet_test";

/// The library source the report must name.
const SOURCE: &str = "greet.zig";

/// Repository name parts of the kcov source build and its build dependencies.
const SOURCE_BUILD_REPOS: &[&str] = &["kcov", "elfutils", "rules_m4"];

fn consumer(name: &str, extra_module: &str) -> Case {
    Case::language_consumer(name, "zig", extra_module).unwrap_or_else(|error| panic!("{error}"))
}

/// Runs `bazel coverage` on the zig_test and returns its coverage.dat text, empty when absent.
fn coverage(case: &Case) -> String {
    let run = case.bazel(&["coverage", "--color=no", "--curses=no", TEST]);
    assert!(
        run.status.success(),
        "bazel coverage {TEST}: {}",
        run.combined()
    );
    let testlogs = info(case, "bazel-testlogs");
    let report = testlogs.join("greet_test").join("coverage.dat");
    std::fs::read_to_string(&report).unwrap_or_default()
}

fn info(case: &Case, key: &str) -> PathBuf {
    let run = case.bazel(&["info", key]);
    assert!(run.status.success(), "bazel info {key}: {}", run.combined());
    PathBuf::from(run.stdout.trim())
}

/// The source files an LCOV report names on its `SF:` lines.
fn report_sources(lcov: &str) -> Vec<&str> {
    lcov.lines()
        .filter_map(|line| line.strip_prefix("SF:"))
        .collect()
}

/// The fetched repositories whose canonical name has a part in `SOURCE_BUILD_REPOS`.
fn source_build_repos(external: &Path) -> Vec<String> {
    let entries = std::fs::read_dir(external)
        .unwrap_or_else(|error| panic!("{}: {error}", external.display()));
    let mut found: Vec<String> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| names_source_build(name))
        .collect();
    found.sort();
    found
}

fn names_source_build(repo: &str) -> bool {
    repo.split(['+', '~'])
        .any(|part| SOURCE_BUILD_REPOS.contains(&part))
}

#[test]
fn a_pinned_kcov_writes_an_lcov_report_for_a_zig_test() {
    let kcov = dx_testing::rlocation(dx_testing::env_text("DX_KCOV_STATIC").trim())
        .unwrap_or_else(|error| panic!("DX_KCOV_STATIC: {error}"));
    let sha256 = dx_testing::sha256_file(&kcov).unwrap_or_else(|error| panic!("{error}"));
    let url = format!("file://{}", kcov.to_string_lossy());
    let extra = format!(
        "dx_prebuilt = use_extension(\"@rules_dx//core:extensions.bzl\", \"dx_prebuilt\")\n\
         dx_prebuilt.asset(name = \"kcov\", os = \"linux\", cpu = \"x86_64\", url = \"{url}\", sha256 = \"{sha256}\")\n"
    );
    let case = consumer("zig_kcov_pinned", &extra);
    let lcov = coverage(&case);
    let sources = report_sources(&lcov);
    assert!(
        sources.iter().any(|source| source.ends_with(SOURCE)),
        "the kcov report names no {SOURCE}:\n{lcov}"
    );
}

#[test]
fn an_unpinned_kcov_runs_the_zig_test_without_coverage() {
    let case = consumer("zig_kcov_unpinned", "");
    let lcov = coverage(&case);
    assert!(
        report_sources(&lcov).is_empty(),
        "a consumer with no kcov pin wrote a report:\n{lcov}"
    );
    let found = source_build_repos(&info(&case, "output_base").join("external"));
    assert!(
        found.is_empty(),
        "a consumer with no kcov pin fetched the kcov source build: {}",
        found.join(", ")
    );
}

#[test]
fn source_build_repos_match_whole_name_parts() {
    assert!(names_source_build("kcov"));
    assert!(names_source_build("elfutils+"));
    assert!(names_source_build("rules_dx++dx_pins+kcov"));
    assert!(names_source_build("rules_m4+"));
    assert!(!names_source_build("rules_dx++dx_prebuilt+dx_prebuilt"));
    assert!(!names_source_build("kcov_tools"));
}

#[test]
fn report_sources_reads_sf_lines() {
    assert_eq!(
        report_sources("TN:\nSF:greet.zig\nDA:1,1\nend_of_record\nSF:b.zig\n"),
        ["greet.zig", "b.zig"]
    );
}
