use dx_testing::{read_runfiles, runfiles_root};

const BAZELVERSION: &str = ".bazelversion";
const DOCKERFILE: &str = ".devcontainer/Dockerfile.prebuilt";
const GITHUB_BUILD: &str = ".github/BUILD.bazel";
const VERSIONS_BZL: &str = "modules/versions.bzl";

fn starlark_const(text: &str, name: &str) -> String {
    dx_testing::starlark_const(text, name, VERSIONS_BZL)
}

fn bazelisk_pin() -> String {
    starlark_const(&read_runfiles(VERSIONS_BZL), "BAZELISK_VERSION")
}

fn workflow_exports() -> Vec<String> {
    dx_testing::workflow_exports(&read_runfiles(GITHUB_BUILD), GITHUB_BUILD)
}

fn workflows() -> Vec<(String, String)> {
    workflow_exports()
        .into_iter()
        .map(|name| {
            let text = std::fs::read_to_string(runfiles_root().join(".github").join(&name))
                .unwrap_or_else(|_| panic!("{name} must ship as bazelisk_pins_test data"));
            (name, text)
        })
        .collect()
}

fn setup_bazel_workflows() -> Vec<(String, String)> {
    let users: Vec<(String, String)> = workflows()
        .into_iter()
        .filter(|(_, text)| text.contains("- uses: bazel-contrib/setup-bazel@"))
        .collect();
    assert!(!users.is_empty(), "no workflow installs Bazelisk");
    users
}

#[test]
fn every_setup_bazel_workflow_declares_the_canonical_bazelisk_pin() {
    let declaration = format!("BAZELISK_VERSION: \"{}\"", bazelisk_pin());
    for (name, text) in setup_bazel_workflows() {
        assert_eq!(
            text.matches(&declaration).count(),
            1,
            "{name} must declare {declaration} exactly once"
        );
    }
}

#[test]
fn every_setup_bazel_step_reads_the_workflow_pin() {
    for (name, text) in setup_bazel_workflows() {
        for line in text.lines() {
            let Some(value) = line.trim().strip_prefix("bazelisk-version:") else {
                continue;
            };
            assert_eq!(
                value.trim(),
                "${{ env.BAZELISK_VERSION }}",
                "{name} hardcodes a Bazelisk version instead of its BAZELISK_VERSION pin"
            );
        }
        assert!(
            !text.contains("bazelisk/releases/download/"),
            "{name} downloads Bazelisk itself instead of taking the pin from {VERSIONS_BZL}"
        );
    }
}

#[test]
fn the_devcontainer_fetches_the_canonical_bazelisk_version() {
    let url = format!(
        "https://github.com/bazelbuild/bazelisk/releases/download/v{}/bazelisk-linux-amd64",
        bazelisk_pin()
    );
    assert_eq!(
        read_runfiles(DOCKERFILE).matches(&url).count(),
        1,
        "{DOCKERFILE} must fetch {url} exactly once"
    );
}

#[test]
fn the_devcontainer_runs_the_pinned_bazel_version() {
    let bazel = starlark_const(&read_runfiles(VERSIONS_BZL), "BAZEL_VERSION");
    assert!(
        read_runfiles(DOCKERFILE).contains(&format!("USE_BAZEL_VERSION={bazel}")),
        "{DOCKERFILE} must set USE_BAZEL_VERSION={bazel} to the {VERSIONS_BZL} pin"
    );
    assert_eq!(
        read_runfiles(BAZELVERSION).trim(),
        bazel,
        "{BAZELVERSION} must carry the {VERSIONS_BZL} Bazel version"
    );
}

#[test]
fn the_ghcr_summary_reports_the_pins_it_built() {
    let versions = read_runfiles(VERSIONS_BZL);
    let text = read_runfiles(".github/workflows/ghcr.yml");
    for claim in [
        format!(
            "Bazelisk v{}",
            starlark_const(&versions, "BAZELISK_VERSION")
        ),
        format!("Bazel {}", starlark_const(&versions, "BAZEL_VERSION")),
    ] {
        assert!(
            text.contains(&claim),
            "the ghcr job summary must report {claim} from {VERSIONS_BZL}"
        );
    }
}
