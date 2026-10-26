use dx_testing::read_runfiles;

const WORKFLOW: &str = ".github/workflows/release.yml";
const LAUNCHER: &str = "cli/adopt/src/launcher.rs";
const ASSETS: [&str; 5] = [
    "dx-linux-x86_64",
    "dx-linux-arm64",
    "dx-macos-arm64",
    "dx-windows-x86_64",
    "dx-windows-arm64",
];

fn workflow() -> String {
    read_runfiles(WORKFLOW)
}

#[test]
fn the_workflow_builds_every_asset_the_launchers_can_ask_for() {
    let text = workflow();
    for asset in ASSETS {
        assert!(
            text.contains(&format!("- name: {asset}\n")),
            "{WORKFLOW} has no matrix cell named {asset}"
        );
        assert!(
            text.contains(asset),
            "{WORKFLOW} upload check misses {asset}"
        );
    }
}

#[test]
fn the_launchers_download_the_names_the_workflow_uploads() {
    let launcher = read_runfiles(LAUNCHER);
    assert!(
        launcher.contains("releases/download/v$VERSION/dx-$os-$arch\""),
        "{LAUNCHER} shell URL scheme changed"
    );
    assert!(
        launcher.contains("releases/download/v%VERSION%/dx-windows-%ARCH%\""),
        "{LAUNCHER} cmd URL scheme changed"
    );
    assert!(
        launcher.contains("linux-x86_64 | linux-arm64 | macos-arm64"),
        "{LAUNCHER} supported unix platforms changed"
    );
    assert!(launcher.contains("os=linux") && launcher.contains("os=macos"));
    assert!(launcher.contains("arch=x86_64") && launcher.contains("arch=arm64"));
    assert!(launcher.contains("set \"ARCH=x86_64\"") && launcher.contains("set \"ARCH=arm64\""));
    for asset in ASSETS {
        let (os, arch) = asset
            .strip_prefix("dx-")
            .and_then(|rest| rest.split_once('-'))
            .expect("asset name is dx-<os>-<arch>");
        assert!(["linux", "macos", "windows"].contains(&os), "{asset}");
        assert!(["x86_64", "arm64"].contains(&arch), "{asset}");
    }
}

#[test]
fn the_sidecar_is_hash_two_spaces_name_beside_the_asset() {
    let text = workflow();
    assert!(
        text.contains("printf '%s  %s\\n' \"$hash\" \"$ASSET\" > \"out/$ASSET.sha256\""),
        "{WORKFLOW} sidecar is not '<hash>  <name>' in <asset>.sha256"
    );
    let launcher = read_runfiles(LAUNCHER);
    assert!(launcher.contains("fetch \"$url.sha256\""));
    assert!(launcher.contains("IFS=' ' read -r want _"));
}

#[test]
fn the_workflow_runs_on_version_tags() {
    let text = workflow();
    assert!(
        text.contains("tags:\n      - \"v*\""),
        "{WORKFLOW} tag filter"
    );
    assert!(
        read_runfiles(LAUNCHER).contains("download/v$VERSION/"),
        "{LAUNCHER} no longer downloads from a v-prefixed tag"
    );
}

#[test]
fn windows_bash_does_not_rewrite_bazel_labels() {
    assert!(workflow().contains("MSYS2_ARG_CONV_EXCL: \"//\""));
}

#[test]
fn the_upload_job_never_creates_the_release() {
    assert!(!workflow().contains("gh release create"));
}

#[test]
fn the_source_job_checks_the_tag_against_the_commit_before_building_the_archive() {
    let text = workflow();
    assert!(text.contains("CANDIDATE_COMMIT: ${{ inputs.commit || github.sha }}"));
    let driver = text
        .find("RELEASE_DRY_RUN=0 RELEASE_APPROVE=1 bazel run //deploy/release:release_driver -- \"$RELEASE_TAG\" \"$CANDIDATE_COMMIT\"")
        .expect("release_driver runs with the tag and the commit");
    let archive = text
        .find("bazel run //deploy/release:source_archive -- \"$RELEASE_TAG\" \"$CANDIDATE_COMMIT\"")
        .expect("source_archive runs with the tag and the commit");
    assert!(
        driver < archive,
        "the driver must pass before the archive is built"
    );
}

#[test]
fn the_source_job_is_read_only_and_gates_the_upload() {
    let text = workflow();
    let source = text.find("\n  source:\n").expect("source job");
    let upload = text.find("\n  upload:\n").expect("upload job");
    assert!(source < upload);
    assert!(
        text[source..upload].contains("contents: read"),
        "{WORKFLOW} source job must stay read-only"
    );
    assert!(!text[source..upload].contains("contents: write"));
    assert!(text[upload..].contains("needs: [build, source]"));
}

#[test]
fn the_source_archive_is_uploaded_without_clobber() {
    let text = workflow();
    assert!(text.contains("test -s \"out/$src\""));
    assert!(text.contains("gh release upload \"$RELEASE_TAG\" \"out/$file\"\n"));
    assert!(!text.contains("rules_dx-* --clobber"));
    assert!(!text.contains("\"out/$file\" --clobber"));
    assert!(text.contains("sha256sum -c \"$src.sha256\""));
}
