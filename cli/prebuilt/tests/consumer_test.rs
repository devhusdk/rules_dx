use std::path::{Path, PathBuf};
use std::process::Command;

use dx_nested::Case;
use dx_testing::{resolve_runfiles, Run};

const HOST_ASSET: &str = "dx-linux-x86_64";

const EXTENSION: &str =
    "dx_prebuilt = use_extension(\"@rules_dx//cli/prebuilt:extension.bzl\", \"dx_prebuilt\")\n";

const PLATFORMS: &str = "bazel_dep(name = \"platforms\", version = \"1.1.0\")\n";

const RISCV_BUILD: &str = r#"platform(
    name = "riscv",
    constraint_values = [
        "@platforms//os:linux",
        "@platforms//cpu:riscv64",
    ],
)
"#;

fn dx_bin() -> PathBuf {
    let rel = std::env::var("DX_BIN").unwrap_or_else(|_| panic!("DX_BIN must name dx"));
    std::fs::canonicalize(resolve_runfiles(&rel)).unwrap_or_else(|error| panic!("{rel}: {error}"))
}

fn sha256(path: &Path) -> String {
    let out = Command::new("sha256sum")
        .arg(path)
        .output()
        .unwrap_or_else(|error| panic!("sha256sum: {error}"));
    assert!(out.status.success(), "sha256sum {path:?}");
    String::from_utf8_lossy(&out.stdout)
        .split_whitespace()
        .next()
        .expect("hash")
        .to_owned()
}

fn dx_version() -> String {
    let pins = dx_testing::read_text(&dx_testing::rules_dx_source().join("cli/prebuilt/pins.bzl"));
    let line = pins
        .lines()
        .find_map(|line| line.strip_prefix("DX_VERSION = \""))
        .expect("DX_VERSION in pins.bzl");
    line.trim_end_matches('"').to_owned()
}

fn bazel(case: &Case, args: &[&str]) -> Run {
    let mut all = vec![args[0], "--color=no", "--curses=no"];
    all.extend_from_slice(&args[1..]);
    case.bazel(&all)
}

fn ok(run: &Run) -> String {
    assert!(run.status.success(), "{}", run.combined());
    run.stdout.clone()
}

fn pinned_consumer(name: &str) -> Case {
    let bin = dx_bin();
    let tag = format!(
        "dx_prebuilt.asset(\n    name = \"{HOST_ASSET}\",\n    sha256 = \"{}\",\n    url = \"file://{}\",\n)\n",
        sha256(&bin),
        bin.display()
    );
    let case = Case::consumer(name, &format!("{PLATFORMS}{EXTENSION}{tag}"));
    case.write("BUILD.bazel", RISCV_BUILD);
    case
}

#[test]
fn a_consumer_runs_the_pinned_binary_and_builds_no_rust() {
    let case = pinned_consumer("prebuilt");

    let run = bazel(&case, &["run", "@rules_dx//:dx", "--", "--version"]);
    let version = ok(&run);
    assert!(
        version.contains(&format!("dx {}", dx_version())),
        "{}",
        run.combined()
    );

    let deps = ok(&bazel(&case, &["query", "deps(@rules_dx//:dx)"]));
    let leaked: Vec<&str> = deps
        .lines()
        .filter(|label| label.contains("rules_rust") || label.contains("crate"))
        .collect();
    assert!(leaked.is_empty(), "consumer dx graph has Rust: {leaked:?}");
    assert!(
        !deps.contains("//cli/cli:dx"),
        "consumer dx graph names the source dx:\n{deps}"
    );

    let rustc = ok(&bazel(
        &case,
        &["aquery", "mnemonic(\"Rustc\", deps(@rules_dx//:dx))"],
    ));
    assert!(
        rustc.trim().is_empty(),
        "consumer dx compiles Rust:\n{rustc}"
    );
}

#[test]
fn an_unsupported_platform_fails_with_the_remedies() {
    let case = pinned_consumer("unsupported");
    let run = bazel(&case, &["build", "--platforms=//:riscv", "@rules_dx//:dx"]);
    let text = run.combined();
    assert_eq!(run.status.code(), Some(1), "{text}");
    for needle in [
        "dx: no prebuilt binary for linux riscv64",
        "linux x86_64",
        "dx_prebuilt.from_source(), which requires Rust and C toolchains",
    ] {
        assert!(text.contains(needle), "{needle}\n{text}");
    }
}

#[test]
fn a_release_with_no_pins_builds_dx_from_source() {
    let case = Case::consumer("unpinned", "");
    case.write("BUILD.bazel", "");
    let deps = ok(&bazel(&case, &["query", "deps(@rules_dx//:dx, 2)"]));
    assert!(
        deps.lines().any(|label| label == "@rules_dx//cli/cli:dx"),
        "{deps}"
    );
}

#[test]
fn an_unpinned_host_asset_builds_from_source_and_downloads_nothing() {
    let tag = format!(
        "dx_prebuilt.asset(\n    name = \"dx-macos-arm64\",\n    sha256 = \"{}\",\n    url = \"file:///nonexistent/dx\",\n)\n",
        "0".repeat(64)
    );
    let case = Case::consumer("partly_pinned", &format!("{EXTENSION}{tag}"));
    case.write("BUILD.bazel", "");
    let deps = ok(&bazel(&case, &["cquery", "deps(@rules_dx//:dx, 2)"]));
    assert!(
        deps.lines().any(|label| label.contains("//cli/cli:dx ")),
        "{HOST_ASSET} is unpinned, so dx should come from source:\n{deps}"
    );
}

#[test]
fn from_source_points_the_public_label_at_the_source_binary() {
    let case = Case::consumer(
        "from_source",
        &format!("{EXTENSION}dx_prebuilt.from_source()\n"),
    );
    case.write("BUILD.bazel", "");
    let deps = ok(&bazel(&case, &["query", "deps(@rules_dx//:dx, 2)"]));
    assert!(
        deps.lines().any(|label| label == "@rules_dx//cli/cli:dx"),
        "{deps}"
    );
}

#[test]
fn a_non_root_module_cannot_set_tags() {
    let case = Case::consumer(
        "non_root",
        "bazel_dep(name = \"middle\", version = \"0.0.0\")\nlocal_path_override(module_name = \"middle\", path = \"middle\")\n",
    );
    case.write(
        "middle/MODULE.bazel",
        &format!(
            "module(name = \"middle\", version = \"0.0.0\")\n\nbazel_dep(name = \"rules_dx\", version = \"0.0.0\")\n{EXTENSION}dx_prebuilt.from_source()\n"
        ),
    );
    case.write("middle/BUILD.bazel", "");
    let run = bazel(&case, &["query", "deps(@rules_dx//:dx)"]);
    let text = run.combined();
    assert!(!run.status.success(), "{text}");
    assert!(
        text.contains("only the root module may set asset or from_source tags; middle sets them"),
        "{text}"
    );
}
