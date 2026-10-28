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
    assert!(upload.contains("for file in $files \"$src\" \"$src.sha256\" source.json; do"));
    assert!(upload.contains("files=\"$files $asset $asset.sha256 $archive $archive.sha256\""));
    assert!(upload.contains("files=\"dx-sha256sums.txt dx-sha256sums.txt.sigstore.json\""));
}

#[test]
fn the_checksum_manifest_is_built_checked_and_uploaded() {
    let text = workflow();
    for needle in [
        "cat \"out/$asset.sha256\" \"out/$archive.sha256\" >> sums.tmp",
        "LC_ALL=C sort -k2 sums.tmp > out/dx-sha256sums.txt",
        "test \"$(wc -l < out/dx-sha256sums.txt)\" -eq 10",
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

const PINS: &str = "cli/prebuilt/pins.bzl";

fn pins_version(pins: &str) -> String {
    pins.lines()
        .find_map(|line| line.strip_prefix("DX_VERSION = \""))
        .and_then(|rest| rest.strip_suffix('"'))
        .unwrap_or_else(|| panic!("{PINS} has no DX_VERSION line"))
        .to_owned()
}

fn pinned_assets(pins: &str) -> Vec<String> {
    let start = pins.find("SHA256 = {\n").expect("SHA256 map") + "SHA256 = {\n".len();
    let end = pins[start..].find("\n}").expect("SHA256 map end") + start;
    pins[start..end]
        .lines()
        .map(|line| {
            let (name, hash) = line
                .trim()
                .split_once(": ")
                .unwrap_or_else(|| panic!("{PINS} entry {line:?}"));
            let hash = hash.trim_end_matches(',').trim_matches('"');
            assert!(
                hash.is_empty()
                    || (hash.len() == 64
                        && hash.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))),
                "{PINS} pin for {name} is not a sha256"
            );
            name.trim_matches('"').to_owned()
        })
        .collect()
}

#[test]
fn the_prebuilt_pins_name_every_asset_once_in_order() {
    let pins = read_runfiles(PINS);
    let names = pinned_assets(&pins);
    assert_eq!(
        names,
        sorted(names.clone()),
        "{PINS} entries are not sorted"
    );
    assert_eq!(names, expected(), "{PINS} keys differ from ASSETS");
}

#[test]
fn the_prebuilt_version_matches_the_module_and_the_cli() {
    let version = pins_version(&read_runfiles(PINS));
    let module = read_runfiles("MODULE.bazel");
    assert!(
        module.contains(&format!(
            "module(\n    name = \"rules_dx\",\n    version = \"{version}\",\n)"
        )),
        "{PINS} DX_VERSION {version} differs from MODULE.bazel"
    );
    let cli = read_runfiles("cli/adopt/src/version.rs");
    assert!(
        cli.contains(&format!("pub const DX_VERSION: &str = \"{version}\";")),
        "{PINS} DX_VERSION {version} differs from cli/adopt/src/version.rs"
    );
}

#[test]
fn the_release_build_checks_the_binary_version_against_the_tag() {
    let text = workflow();
    let check = text
        .find("want=\"dx ${RELEASE_TAG#v}\"")
        .expect("version check");
    let hash = text.find("hash=\"$(sha256sum").expect("hash");
    assert!(
        check < hash,
        "{WORKFLOW} hashes before checking the version"
    );
}

#[test]
fn the_source_job_refuses_a_tag_the_committed_version_does_not_match() {
    let text = workflow();
    let source = text.find("\n  source:\n").expect("source job");
    let gate = text
        .find("[ \"$committed\" != \"${RELEASE_TAG#v}\" ]")
        .expect("version gate");
    let driver = text.find("release_driver").expect("driver");
    assert!(source < gate && gate < driver);
    assert!(text.contains("land the version bump first"));
}

const TRUST: &str = "deploy/install/src/trust.rs";

fn trust_const(name: &str) -> String {
    let trust = read_runfiles(TRUST);
    let at = trust
        .find(&format!("pub const {name}: &str = \""))
        .unwrap_or_else(|| panic!("{TRUST} lacks {name}"));
    let rest = &trust[at..];
    let start = rest.find('"').expect("open quote") + 1;
    let end = start + rest[start..].find('"').expect("close quote");
    rest[start..end].to_owned()
}

#[test]
fn the_manifest_and_bundle_names_match_the_install_engine() {
    let text = workflow();
    let manifest = trust_const("MANIFEST_NAME");
    let bundle = trust_const("BUNDLE_NAME");
    assert!(text.contains(&format!("out/{manifest}")));
    assert!(text.contains(&format!("out/{bundle}")));
    assert!(text.contains(&format!("files=\"{manifest} {bundle}\"")));
}

#[test]
fn the_archives_match_the_install_engine_cells() {
    let trust = read_runfiles(TRUST);
    let text = workflow();
    for asset in ASSETS {
        let suffix = if asset.contains("windows") {
            ".zip"
        } else {
            ".tar.gz"
        };
        assert!(
            trust.contains(&format!("name: \"{asset}\",")),
            "{TRUST} lacks {asset}"
        );
        assert!(
            trust.contains(&format!("suffix: \"{suffix}\",")),
            "{TRUST} lacks {suffix}"
        );
        let format = suffix.trim_start_matches('.');
        assert!(
            text.contains(&format!("- name: {asset}\n            runner:"))
                && text.contains(&format!("            format: {format}\n")),
            "{WORKFLOW} format for {asset}"
        );
    }
    assert!(text.contains("dx-windows-*) archive=\"$asset.zip\""));
    assert!(text.contains("*) archive=\"$asset.tar.gz\""));
}

#[test]
fn the_attestation_is_signed_once_from_the_tag_in_the_upload_job() {
    let text = workflow();
    let step = "uses: actions/attest@1e69f48acb82d1966a394da916b4c1698aa569d6 # v4.2.2";
    assert_eq!(text.matches("actions/attest@").count(), 1);
    let upload = text.find("\n  upload:\n").expect("upload job");
    let at = text.find(step).expect("pinned attest step");
    assert!(at > upload);
    let head = &text[text[..at].rfind("      - name:").expect("step")..at];
    assert!(head.contains("steps.plan.outputs.attest == 'true'"));
    assert!(head.contains("github.ref == format('refs/tags/{0}', env.RELEASE_TAG)"));
    assert!(text.contains("subject-checksums: out/dx-sha256sums.txt"));
    assert!(text.contains("grep -Fxq dx-sha256sums.txt.sigstore.json"));
    assert!(text.contains("dispatch with --ref $RELEASE_TAG"));
    for permission in [
        "id-token: write",
        "attestations: write",
        "artifact-metadata: write",
    ] {
        assert_eq!(text.matches(permission).count(), 1, "{permission}");
        assert!(text.find(permission).expect("permission") > upload);
    }
}

#[test]
fn the_archives_are_verified_with_the_install_engine_flags() {
    let text = workflow();
    let trust = read_runfiles(TRUST);
    for flag in [
        "--bundle",
        "--repo",
        "--cert-identity",
        "--cert-oidc-issuer",
        "--source-ref",
        "--deny-self-hosted-runners",
    ] {
        assert!(
            trust.contains(&format!("\"{flag}\"")),
            "{TRUST} lacks {flag}"
        );
        assert!(text.contains(&format!("{flag}")), "{WORKFLOW} lacks {flag}");
    }
    assert!(text.contains("release.yml@refs/tags/$RELEASE_TAG"));
    assert!(text.contains("--source-ref \"refs/tags/$RELEASE_TAG\""));
    let verify = text.find("gh attestation verify").expect("verify");
    let put = text.find("gh release upload").expect("upload");
    assert!(verify < put);
}

#[test]
fn a_published_release_is_checked_with_dx_install() {
    let text = workflow();
    let download = text
        .find("--pattern 'dx-*' --dir verify")
        .expect("download");
    let install = text
        .find("verify/dx install --tag \"$RELEASE_TAG\" --archive verify/dx-linux-x86_64.tar.gz --output json")
        .expect("dx install check");
    assert!(download < install);
    assert!(!text.contains("--apply"));
}
