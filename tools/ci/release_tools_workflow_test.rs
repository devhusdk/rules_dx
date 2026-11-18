//! Pins the tool release build in release-tools.yml and its wiring in release.yml.

use ci_workflow::document;
use dx_testing::read_runfiles;

const TOOLS: &str = ".github/workflows/release-tools.yml";
const RELEASE: &str = ".github/workflows/release.yml";

const HOSTS: [(&str, &str); 4] = [
    ("linux-x86_64", "ubuntu-latest"),
    ("linux-arm64", "ubuntu-24.04-arm"),
    ("macos-arm64", "macos-15"),
    ("windows-x86_64", "windows-latest"),
];

const PYTHON_HOSTS: [&str; 5] = [
    "linux-x86_64",
    "linux-arm64",
    "macos-arm64",
    "windows-x86_64",
    "windows-arm64",
];

const ASSETS: [&str; 9] = [
    "dx-tool-nixfmt-1.5.0-LICENSE",
    "dx-tool-nixfmt-1.5.0-linux-arm64",
    "dx-tool-kcov-43-linux-x86_64",
    "dx-tool-kcov-43-linux-arm64",
    "dx-tool-deadnix-1.3.1-LICENSE",
    "dx-tool-selene-0.32.0-LICENSE",
    "dx-tool-kcov-43-LICENSE",
    "dx-tool-deadnix-1.3.1-source.crate",
    "dx-tool-kcov-43-source.tar.gz",
];

#[test]
fn matrix_pins_hosts_and_runners() {
    let workflow = document(TOOLS).unwrap();
    let include = workflow["jobs"]["tools"]["strategy"]["matrix"]["include"]
        .as_array()
        .expect("matrix include");
    let found: Vec<(&str, &str)> = include
        .iter()
        .map(|entry| {
            (
                entry["host"].as_str().expect("host"),
                entry["runner"].as_str().expect("runner"),
            )
        })
        .collect();
    assert_eq!(found, HOSTS);
}

#[test]
fn workflow_is_reusable_and_read_only() {
    let workflow = document(TOOLS).unwrap();
    assert!(read_runfiles(TOOLS).contains("\n  workflow_call:\n"));
    assert_eq!(workflow["permissions"]["contents"], "read");
    assert_eq!(workflow["permissions"].as_object().expect("permissions").len(), 1);
    assert_eq!(workflow["jobs"]["tools"]["permissions"]["contents"], "read");
    assert_eq!(workflow["jobs"]["python-tools"]["permissions"]["contents"], "read");
}

#[test]
fn each_host_uploads_its_artifact() {
    let text = read_runfiles(TOOLS);
    assert!(text.contains("name: release-tool-${{ matrix.host }}"));
    assert!(text.contains("//rust/private/quality/tools:deadnix //rust/private/quality/tools:selene"));
    assert!(text.contains("bazel test //tools/coverage/kcov:smoke_test //tools/coverage/kcov:static_smoke_test"));
    assert!(text.contains("bazel build //tools/coverage/kcov:kcov_static"));
    assert!(text.contains("kcov archive sha256 differs from pins.json"));
}

#[test]
fn python_tools_job_builds_every_bundle_host() {
    let workflow = document(TOOLS).unwrap();
    assert_eq!(workflow["jobs"]["python-tools"]["runs-on"], "ubuntu-latest");
    let text = read_runfiles(TOOLS);
    assert!(text.contains(&format!("hosts=({})", PYTHON_HOSTS.join(" "))));
    assert!(text.contains("//core/quality/tools/python_bundle:dx_python_tools_${host//-/_}"));
    assert!(text.contains("asset=\"$out/dx-python-tools-$host.tar.gz\""));
    assert!(text.contains("name: release-tool-python-tools"));
}

#[test]
fn release_calls_the_tools_workflow_before_the_source_job() {
    let workflow = document(RELEASE).unwrap();
    assert_eq!(
        workflow["jobs"]["tools"]["uses"],
        "./.github/workflows/release-tools.yml"
    );
    let needs = workflow["jobs"]["source"]["needs"].as_array().expect("needs");
    assert!(needs.iter().any(|need| need == "tools"));
}

#[test]
fn release_writes_and_checks_the_tool_manifest() {
    let text = read_runfiles(RELEASE);
    assert!(text.contains("pattern: release-tool-*"));
    assert!(text.contains("test \"$(wc -l < out/dx-tool-sha256sums.txt)\" -eq 22"));
    assert!(text.contains("sha256sum -c dx-tool-sha256sums.txt"));
    assert!(text.contains("out/dx-tool-sha256sums.txt; (cd out && sha256sum dx-sha256sums.txt dx-gazelle-sha256sums.txt dx-tool-sha256sums.txt)"));
    for asset in ASSETS {
        assert!(text.contains(asset), "{RELEASE} must name {asset}");
    }
    assert!(text.contains(&format!("for host in {}; do\n            tool_asset=\"dx-python-tools-$host.tar.gz\"", PYTHON_HOSTS.join(" "))));
    assert!(text.contains(" dx-tool-* dx-python-tools-*; do"));
    for tool in ["deadnix-1.3.1", "selene-0.32.0"] {
        assert!(text.contains(tool), "{RELEASE} must name {tool}");
    }
}
