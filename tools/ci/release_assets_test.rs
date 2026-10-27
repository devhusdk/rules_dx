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

/// Assets named by the one `for asset in ...; do` line of the upload job.
fn upload_loop_assets(text: &str) -> Vec<String> {
    let loops: Vec<&str> = text
        .lines()
        .filter_map(|line| line.trim().strip_prefix("for asset in "))
        .collect();
    assert_eq!(
        loops.len(),
        1,
        "want exactly one `for asset in` loop, found {}",
        loops.len()
    );
    let (list, _) = loops[0]
        .split_once("; do")
        .expect("upload loop is `for asset in ...; do`");
    list.split_whitespace().map(str::to_owned).collect()
}

/// Assets named by the build matrix `- name: dx-*` cells.
fn matrix_assets(text: &str) -> Vec<String> {
    text.lines()
        .filter_map(|line| line.trim().strip_prefix("- name: "))
        .filter(|name| name.starts_with("dx-"))
        .map(str::to_owned)
        .collect()
}

fn sorted(mut names: Vec<String>) -> Vec<String> {
    names.sort();
    names
}

fn expected() -> Vec<String> {
    sorted(ASSETS.iter().map(|asset| (*asset).to_owned()).collect())
}

#[test]
fn the_workflow_builds_every_asset_the_launchers_can_ask_for() {
    let text = workflow();
    assert_eq!(
        sorted(matrix_assets(&text)),
        expected(),
        "{WORKFLOW} matrix cells differ from ASSETS"
    );
    assert_eq!(
        sorted(upload_loop_assets(&text)),
        expected(),
        "{WORKFLOW} upload loop differs from ASSETS"
    );
}

#[test]
fn an_upload_loop_missing_an_asset_is_caught() {
    let text = "      run: |\n        for asset in dx-linux-x86_64 dx-linux-arm64 dx-macos-arm64 dx-windows-x86_64; do\n          echo \"$asset\"\n        done\n";
    assert_ne!(sorted(upload_loop_assets(text)), expected());
}

#[test]
#[should_panic(expected = "exactly one")]
fn a_workflow_with_two_upload_loops_is_refused() {
    upload_loop_assets("for asset in dx-a; do\nfor asset in dx-b; do\n");
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
    for asset in ASSETS {
        let platform = asset.trim_start_matches("dx-");
        let var = format!("SHA256_{}", platform.replace('-', "_").to_uppercase());
        assert!(
            launcher.contains(&format!("  {platform}) want=${var} ;;")),
            "{LAUNCHER} shell launcher does not pick the {asset} pin"
        );
    }
    assert!(launcher.contains("os=linux") && launcher.contains("os=macos"));
    assert!(launcher.contains("arch=x86_64") && launcher.contains("arch=arm64"));
    assert!(launcher.contains("set \"ARCH=x86_64\"") && launcher.contains("set \"ARCH=arm64\""));
}

#[test]
fn the_sidecar_is_hash_two_spaces_name_beside_the_asset() {
    let text = workflow();
    assert!(
        text.contains("printf '%s  %s\\n' \"$hash\" \"$ASSET\" > \"out/$ASSET.sha256\""),
        "{WORKFLOW} sidecar is not '<hash>  <name>' in <asset>.sha256"
    );
    let source = read_runfiles(LAUNCHER);
    let launcher = source
        .split("#[cfg(test)]")
        .next()
        .expect("launcher templates");
    assert!(
        !launcher.contains(".sha256\""),
        "launcher fetches a sidecar"
    );
    assert!(
        !launcher.contains("'.sha256'"),
        "cmd launcher fetches a sidecar"
    );
    for asset in expected() {
        let var = format!(
            "SHA256_{}",
            asset
                .trim_start_matches("dx-")
                .replace('-', "_")
                .to_uppercase()
        );
        assert!(
            launcher.contains(&format!("{var}=\"@{var}@\"")),
            "{LAUNCHER} has no pin for {asset}"
        );
    }
}

#[test]
fn the_upload_never_overwrites() {
    let text = workflow();
    let upload = &text[text.find("\n  upload:\n").expect("upload job")..];
    assert!(!upload.contains("--clobber"), "{WORKFLOW} upload clobbers");
    let uploads: Vec<&str> = upload
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with("gh release upload"))
        .collect();
    assert_eq!(
        uploads,
        ["gh release upload \"$RELEASE_TAG\" \"out/$file\""]
    );
    let guard = upload
        .find("if ! cmp -s \"existing/$file\" \"out/$file\"; then")
        .expect("cmp guard");
    let put = upload.find("gh release upload").expect("upload");
    assert!(guard < put);
    assert!(upload
        .contains("for file in $files dx-sha256sums.txt \"$src\" \"$src.sha256\" source.json; do"));
    assert!(upload.contains("files=\"$files $asset $asset.sha256\""));
}

#[test]
fn the_checksum_manifest_is_built_checked_and_uploaded() {
    let text = workflow();
    for needle in [
        "cat \"out/$asset.sha256\" >> sums.tmp",
        "LC_ALL=C sort -k2 sums.tmp > out/dx-sha256sums.txt",
        "test \"$(wc -l < out/dx-sha256sums.txt)\" -eq 5",
        "(cd out && sha256sum -c dx-sha256sums.txt)",
        "--pattern 'dx-*' --dir verify",
        "sha256sum -c dx-sha256sums.txt)",
    ] {
        assert!(text.contains(needle), "{WORKFLOW} lacks {needle:?}");
    }
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
