//! Pins the tool release build in release-tools.yml and its wiring in release.yml and
//! release-consumer.yml.

use ci_workflow::document;
use dx_testing::read_runfiles;

const TOOLS: &str = ".github/workflows/release-tools.yml";
const RELEASE: &str = ".github/workflows/release.yml";
const CONSUMER: &str = ".github/workflows/release-consumer.yml";
const RELEASE_JARS: &str = "tools/ci/release_jars.txt";

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

const JARS: [&str; 8] = [
    "dx-avro-union-any.jar",
    "dx-dokka-extract-any.jar",
    "dx-javac-check-any.jar",
    "dx-javadoc-extract-any.jar",
    "dx-jmh-generate-any.jar",
    "dx-kotlinc-check-any.jar",
    "dx-scalac-check-any.jar",
    "dx-scaladoc-extract-any.jar",
];

const ASSETS: [&str; 9] = [
    "dx-nixfmt-LICENSE",
    "dx-nixfmt-linux-arm64",
    "dx-kcov-linux-x86_64",
    "dx-kcov-linux-arm64",
    "dx-deadnix-LICENSE",
    "dx-selene-LICENSE",
    "dx-kcov-LICENSE",
    "dx-deadnix-source.crate",
    "dx-kcov-source.tar.gz",
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
    assert_eq!(
        workflow["permissions"]
            .as_object()
            .expect("permissions")
            .len(),
        1
    );
    assert_eq!(workflow["jobs"]["tools"]["permissions"]["contents"], "read");
    assert_eq!(
        workflow["jobs"]["python-tools"]["permissions"]["contents"],
        "read"
    );
    assert_eq!(workflow["jobs"]["go"]["permissions"]["contents"], "read");
}

#[test]
fn each_host_uploads_its_artifact() {
    let text = read_runfiles(TOOLS);
    assert!(text.contains("name: release-tool-${{ matrix.host }}"));
    assert!(
        text.contains("//rust/private/quality/tools:deadnix //rust/private/quality/tools:selene")
    );
    assert!(text.contains(
        "bazel test //tools/coverage/kcov:smoke_test //tools/coverage/kcov:static_smoke_test"
    ));
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
fn go_job_cross_builds_every_host() {
    let workflow = document(TOOLS).unwrap();
    assert_eq!(workflow["jobs"]["go"]["runs-on"], "ubuntu-latest");
    let text = read_runfiles(TOOLS);
    assert!(text.contains("tools=(go:dx_go oapi-codegen:oapi_codegen)"));
    assert!(text.contains("//go/private/tools/analysis/dx_go:${tool#*:}_${host//-/_}"));
    assert!(text.contains("asset=\"$out/dx-${tool%%:*}-$host\""));
    assert!(text.contains("\"$out/dx-oapi-codegen-LICENSE\""));
    assert!(text.contains("\"$out/dx-oapi-codegen-linux-x86_64\" -version"));
    assert!(text.contains("cp \"$out/dx-go-linux-x86_64\" \"$check/dx_modfmt\""));
    assert!(text.contains("./dx_modfmt -c"));
    assert!(text.contains("name: release-tool-go"));
}

#[test]
fn jars_job_builds_every_jar_asset() {
    let workflow = document(TOOLS).unwrap();
    assert_eq!(workflow["jobs"]["jars"]["runs-on"], "ubuntu-latest");
    assert_eq!(workflow["jobs"]["jars"]["permissions"]["contents"], "read");
    let tools = read_runfiles(TOOLS);
    let release = read_runfiles(RELEASE);
    assert!(tools.contains("name: release-tool-jars"));
    for jar in JARS {
        assert!(tools.contains(jar), "{TOOLS} must build {jar}");
        assert!(release.contains(jar), "{RELEASE} must list {jar}");
    }
    let release_jars = read_runfiles(RELEASE_JARS);
    assert_eq!(release_jars.lines().count(), JARS.len());
    for line in release_jars.lines() {
        assert!(
            tools.contains(&format!("\"{line}\"")),
            "{TOOLS} must build \"{line}\""
        );
    }
    assert!(release.contains("test \"$(wc -l < out/dx-jar-sha256sums.txt)\" -eq 8"));
    assert!(release.contains("sha256sum -c dx-jar-sha256sums.txt"));
    assert!(release.contains("--manifest \"$PWD/out/dx-jar-sha256sums.txt\""));
}

#[test]
fn haskell_job_builds_every_haskell_asset() {
    let workflow = document(TOOLS).unwrap();
    assert_eq!(
        workflow["jobs"]["haskell"]["permissions"]["contents"],
        "read"
    );
    let tools = read_runfiles(TOOLS);
    let release = read_runfiles(RELEASE);
    assert!(tools.contains("name: release-tool-haskell-${{ matrix.host }}"));
    let include = workflow["jobs"]["haskell"]["strategy"]["matrix"]["include"]
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
    assert!(tools.contains("linux-x86_64) flags=(--platforms=//tools/platforms:linux_x86_64_musl --features=fully_static_link) ;;"));
    assert!(tools.contains("//haskell/private/coverage:hpc_lcov_source dx-hpc-lcov-$HOST"));
    assert!(tools.contains("//haskell/private/tools/weeder/bin:source dx-weeder-$HOST"));
    assert!(release.contains("for tool in dx-hpc-lcov dx-weeder; do"));
    assert!(release.contains("dx-weeder-LICENSE >> out/dx-haskell-sha256sums.txt"));
    assert!(release.contains("files=\"$files dx-weeder-LICENSE\""));
    assert!(release.contains("files=\"$files dx-haskell-sha256sums.txt\""));
    assert!(tools.contains("{ echo \"weeder archive sha256 differs from haskell/private/tools/weeder/cabal.lock.json\" >&2; exit 1; }"));
    assert!(tools.contains("\"weeder-$weeder_version/LICENSE\" > \"$out/dx-weeder-LICENSE\""));
    assert!(release.contains("sha256sum -c dx-haskell-sha256sums.txt"));
    assert!(release.contains("--manifest \"$PWD/out/dx-haskell-sha256sums.txt\""));
}

#[test]
fn hackage_job_builds_hlint_and_ormolu() {
    let workflow = document(TOOLS).unwrap();
    assert_eq!(
        workflow["jobs"]["hackage"]["permissions"]["contents"],
        "read"
    );
    let tools = read_runfiles(TOOLS);
    let release = read_runfiles(RELEASE);
    assert!(tools.contains("name: release-tool-haskell-${{ matrix.tool }}-${{ matrix.host }}"));
    let include = workflow["jobs"]["hackage"]["strategy"]["matrix"]["include"]
        .as_array()
        .expect("matrix include");
    let found: Vec<[&str; 5]> = include
        .iter()
        .map(|entry| {
            ["tool", "host", "runner", "target", "lock"]
                .map(|field| entry[field].as_str().expect("matrix field"))
        })
        .collect();
    assert_eq!(
        found,
        [
            [
                "hlint",
                "linux-arm64",
                "ubuntu-24.04-arm",
                "@haskell_hlint_hackage//:hlint-exe-hlint",
                "haskell/private/tools/hlint/cabal.lock.json",
            ],
            [
                "hlint",
                "macos-arm64",
                "macos-15",
                "@haskell_hlint_hackage//:hlint-exe-hlint",
                "haskell/private/tools/hlint/cabal.lock.json",
            ],
            [
                "ormolu",
                "linux-arm64",
                "ubuntu-24.04-arm",
                "@haskell_ormolu_hackage//:ormolu-exe-ormolu",
                "haskell/private/tools/ormolu/cabal.lock.json",
            ],
        ]
    );
    assert!(tools.contains("asset=\"dx-$TOOL-$HOST\""));
    assert!(tools.contains("> \"$out/dx-$TOOL-LICENSE\""));
    assert!(release.contains("for asset in dx-hlint-linux-arm64 dx-hlint-macos-arm64 dx-ormolu-linux-arm64 dx-hlint-LICENSE dx-ormolu-LICENSE dx-haskell-gmp-LICENSE dx-haskell-gmp-source.tar.xz dx-haskell-gmp-source.patch; do"));
    assert!(release.contains("test \"$(wc -l < out/dx-haskell-sha256sums.txt)\" -eq 17"));
    assert!(release.contains("dx-hlint-* dx-ormolu-* dx-haskell-gmp-*"));
}

#[test]
fn haskell_job_ships_the_gmp_licence_and_source() {
    let tools = read_runfiles(TOOLS);
    let release = read_runfiles(RELEASE);
    let pins: serde_json::Value =
        serde_json::from_str(&read_runfiles("haskell/pins.json")).expect("haskell/pins.json");
    for (pin, asset) in [
        ("gmp_license", "dx-haskell-gmp-LICENSE"),
        ("gmp_source", "dx-haskell-gmp-source.tar.xz"),
        ("gmp_source_patch", "dx-haskell-gmp-source.patch"),
    ] {
        assert!(tools.contains(&format!("\"{pin} {asset}\"")), "{asset}");
        assert!(release.contains(&format!(" {asset}")), "{asset}");
        let entry = &pins["archives"][pin];
        assert_eq!(entry["sha256"].as_str().map(str::len), Some(64), "{pin}");
        assert!(
            entry["urls"][0]
                .as_str()
                .is_some_and(|url| url.starts_with("https://gmplib.org/")
                    || url.starts_with("https://www.gnu.org/")),
            "{pin}"
        );
    }
    assert!(tools.contains("sha256 differs from haskell/pins.json $pin"));
    for cell in ["linux_arm64_musl", "linux_x86_64_musl"] {
        assert!(
            pins["archives"][format!("apk__gmp-dev__{cell}")]["urls"][0]
                .as_str()
                .is_some_and(|url| url.ends_with("/gmp-dev-6.2.1-r3.apk")),
            "{cell}"
        );
    }
    assert!(
        pins["archives"]["gmp_source"]["urls"][0]
            .as_str()
            .is_some_and(|url| url.ends_with("/gmp-6.2.1.tar.xz"))
    );
}

#[test]
fn release_calls_the_tools_workflow_before_the_source_job() {
    let workflow = document(RELEASE).unwrap();
    assert_eq!(
        workflow["jobs"]["tools"]["uses"],
        "./.github/workflows/release-tools.yml"
    );
    let needs = workflow["jobs"]["source"]["needs"]
        .as_array()
        .expect("needs");
    assert!(needs.iter().any(|need| need == "tools"));
}

#[test]
fn release_writes_and_checks_the_tool_manifest() {
    let text = read_runfiles(RELEASE);
    assert!(text.contains("pattern: release-tool-*"));
    assert!(text.contains("test \"$(wc -l < out/dx-tool-sha256sums.txt)\" -eq 22"));
    assert!(text.contains("sha256sum -c dx-tool-sha256sums.txt"));
    assert!(text.contains("out/dx-tool-sha256sums.txt; (cd out && sha256sum dx-sha256sums.txt dx-gazelle-sha256sums.txt dx-jar-sha256sums.txt dx-haskell-sha256sums.txt dx-tool-sha256sums.txt)"));
    for asset in ASSETS {
        assert!(text.contains(asset), "{RELEASE} must name {asset}");
    }
    assert!(text.contains(&format!(
        "for host in {}; do\n            tool_asset=\"dx-python-tools-$host.tar.gz\"",
        PYTHON_HOSTS.join(" ")
    )));
    assert!(text.contains(" dx-deadnix-* dx-selene-* dx-nixfmt-* dx-kcov-* dx-python-tools-*; do"));
    assert!(text.contains(
        "for tool in dx-deadnix dx-selene; do\n            for host in linux-x86_64 linux-arm64 macos-arm64 windows-x86_64; do\n              tool_asset=\"$tool-$host\""
    ));
    assert!(text.contains("--manifest \"$PWD/out/dx-tool-sha256sums.txt\""));
    assert!(text.contains("test \"$(wc -l < out/dx-sha256sums.txt)\" -eq 63"));
    assert!(text.contains("for tool in capnp flatc go oapi-codegen runtime thrift; do"));
    assert!(text.contains("files=\"$files dx-oapi-codegen-LICENSE\""));
    assert!(text.contains(" dx-go-* dx-oapi-codegen-* "));
    assert!(text.contains("cd out\n            for file in dx-sha256sums.txt "));
    assert!(text.contains("cp \"$file\" \"$SOURCE_DIR/\""));
}

#[test]
fn macos_tools_ship_signed() {
    let text = read_runfiles(TOOLS);
    assert!(text.contains(
        "for tool in deadnix selene topiary progenitor; do\n              file=\"$out/dx-$tool-$host\""
    ));
    assert!(text.contains(
        "codesign --verify --strict \"$file\" || codesign --sign - \"$file\"\n              codesign --verify --strict \"$file\""
    ));
}

#[test]
fn release_consumer_checks_prebuilt_quality_tools() {
    let text = read_runfiles(CONSUMER);
    assert!(text.contains("for tool in topiary progenitor; do"));
    assert!(text.contains(
        "tool_files=\"$(bazel cquery --output=files \"@rules_dx//rust/private/quality/tools:${tool}_tool\")\""
    ));
    assert!(text.contains("*\"dx_prebuilt/$KEY/$tool\"*) ;;"));
    assert!(text.contains("- $tool: $tool_files"));
    assert!(text.contains("bazel cquery --output=files @rules_dx//config:tool_govet"));
    assert!(text.contains("*\"dx_prebuilt/$KEY/dx_govet\"*) ;;"));
    assert!(text.contains("bazel build @rules_dx//config:tool_modfmt"));
    assert!(text.contains("\"$modfmt\" -c"));
}

#[test]
fn release_verifies_every_manifest_against_the_bundle() {
    let text = read_runfiles(RELEASE);
    assert!(text.contains(
        "for manifest in dx-sha256sums.txt dx-gazelle-sha256sums.txt dx-jar-sha256sums.txt dx-haskell-sha256sums.txt dx-tool-sha256sums.txt; do\n            gh attestation verify \"out/$manifest\" \\\n              --bundle out/dx-sha256sums.txt.sigstore.json"
    ));
}
