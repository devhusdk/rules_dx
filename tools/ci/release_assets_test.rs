use dx_testing::read_runfiles;

const WORKFLOW: &str = ".github/workflows/release.yml";
const CONSUMER: &str = ".github/workflows/release-consumer.yml";
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

fn sorted(mut names: Vec<String>) -> Vec<String> {
    names.sort();
    names
}

fn expected() -> Vec<String> {
    sorted(ASSETS.iter().map(|asset| (*asset).to_owned()).collect())
}

#[test]
fn the_workflow_uploads_every_asset_the_launchers_can_ask_for() {
    let text = workflow();
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
    assert!(
        launcher.contains("[ \"$asset\" = \"dx-$os-$arch\" ]"),
        "{LAUNCHER} shell launcher does not pick the pin by asset name"
    );
    assert!(launcher.contains("os=linux") && launcher.contains("os=macos"));
    assert!(launcher.contains("arch=x86_64") && launcher.contains("arch=arm64"));
    assert!(launcher.contains("set \"ARCH=x86_64\"") && launcher.contains("set \"ARCH=arm64\""));
}

#[test]
fn the_sidecar_is_hash_two_spaces_name_beside_the_asset() {
    let text = workflow();
    assert!(
        text.contains("printf '%s  %s\\n' \"$hash\" \"$asset\" > \"out/$asset.sha256\""),
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
}

#[test]
fn the_assets_go_through_the_release_driver_without_clobber() {
    let text = workflow();
    assert!(!text.contains("--clobber"), "{WORKFLOW} clobbers");
    assert!(!text.contains("gh release upload"));
    assert!(!text.contains("cmp -s"));
    let assets = &text[text.find("\n  assets:\n").expect("assets job")..];
    let call = "RELEASE_DRY_RUN=0 RELEASE_APPROVE=1 bazel run //deploy/release:release_driver -- \"$RELEASE_TAG\" \"$CANDIDATE_COMMIT\" \"${paths[@]}\" > \"$RUNNER_TEMP/ledger.json\" || status=$?";
    let driver = assets.find(call).expect("driver call with the assets");
    assert!(assets.contains("echo '```json'; cat \"$RUNNER_TEMP/ledger.json\"; echo '```'"));
    assert!(assets.contains(">> \"$GITHUB_STEP_SUMMARY\""));
    assert!(assets.contains("exit \"$status\""));
    assert!(
        assets.contains("name: release-ledger\n          path: ${{ runner.temp }}/ledger.json")
    );
    assert!(assets.contains("if: ${{ always() && !inputs.dry_run }}"));
    assert!(assets.contains("echo \"$PWD/out/$file\" >> \"$RUNNER_TEMP/paths.txt\""));
    assert!(assets.contains("mapfile -t paths < \"$RUNNER_TEMP/paths.txt\""));
    assert!(
        assets.contains(
            "for file in $(cat out/files.list) \"$src\" \"$src.sha256\" \"$src.intoto.jsonl\" $sources; do"
        )
    );
    assert!(
        driver
            < assets
                .find("sha256sum -c dx-sha256sums.txt")
                .expect("check")
    );
    let source = &text[text.find("\n  source:\n").expect("source job")..];
    assert!(source.contains(
        "files=\"dx-sha256sums.txt dx-sha256sums.txt.sigstore.json dx-gazelle-sha256sums.txt\""
    ));
    assert!(source.contains(
        "files=\"$files $asset $asset.sha256 $archive $archive.sha256 $archive.spdx.json\""
    ));
}

#[test]
fn only_the_attest_job_can_write_ids_and_only_the_assets_job_can_write_contents() {
    let text = workflow();
    let attest = text.find("\n  attest:\n").expect("attest job");
    let assets = text.find("\n  assets:\n").expect("assets job");
    let pins = text.find("\n  pins:\n").expect("pins job");
    assert!(attest < assets && assets < pins);
    let source = text.find("\n  source:\n").expect("source job");
    assert!(!text[source..pins].contains("\n    if: ${{ !inputs.dry_run }}"));
    let (attest_job, assets_job) = (&text[attest..assets], &text[assets..pins]);
    assert!(attest_job.contains("contents: read"));
    assert!(!attest_job.contains("contents: write"));
    assert!(!attest_job.contains("bazel"));
    assert!(assets_job.contains("needs: [cells, attest]"));
    assert!(assets_job.contains("contents: write"));
    assert!(!assets_job.contains("id-token"));
    assert!(!assets_job.contains("attestations"));
}

#[test]
fn the_missing_release_hint_points_to_the_runbook() {
    assert!(workflow().contains("Create the draft first (see RELEASING.md)"));
    let runbook = read_runfiles("RELEASING.md");
    assert!(runbook.contains("GH_RELEASE_APPROVED=1 tools/dx deploy --apply"));
}

#[test]
fn the_checksum_manifest_is_built_checked_and_uploaded() {
    let text = workflow();
    for needle in [
        "cat \"out/$asset.sha256\" \"out/$archive.sha256\" >> sums.tmp",
        "\"$archive.spdx.json\" >> sums.tmp",
        "for file in \"$archive\" \"$archive.sha256\" \"$archive.spdx.json\"; do",
        "LC_ALL=C sort -k2 sums.tmp > out/dx-sha256sums.txt",
        "test \"$(wc -l < out/dx-sha256sums.txt)\" -eq 19",
        "(cd out && sha256sum -c dx-sha256sums.txt)",
        "--pattern 'dx-*' --dir verify",
        "sha256sum -c dx-sha256sums.txt)",
        "unzip -p \"out/$archive\" dx.exe",
        "pdb=\"$asset-pdb.zip\"",
        "files=\"$files $pdb $pdb.sha256 $pdb.spdx.json\"",
        "dx-windows-x86_64-pdb.zip dx-windows-arm64-pdb.zip; do",
        "tar -xzOf \"out/$archive\" dx",
    ] {
        assert!(text.contains(needle), "{WORKFLOW} lacks {needle:?}");
    }
}

#[test]
fn the_gazelle_manifest_is_separate_checked_attested_and_filled() {
    let text = workflow();
    for needle in [
        "gazelle=\"dx-gazelle-${asset#dx-}\"",
        "cat \"out/$gazelle.sha256\" >> out/dx-gazelle-sha256sums.txt",
        "files=\"$files $gazelle $gazelle.sha256\"",
        "test \"$(wc -l < out/dx-gazelle-sha256sums.txt)\" -eq 5",
        "(cd out && sha256sum -c dx-gazelle-sha256sums.txt)",
        "--gazelle-manifest \"$PWD/out/dx-gazelle-sha256sums.txt\"",
        "cat out/dx-sha256sums.txt out/dx-gazelle-sha256sums.txt; (cd out && sha256sum dx-sha256sums.txt dx-gazelle-sha256sums.txt); }",
        "sha256sum -c dx-sha256sums.txt && sha256sum -c dx-gazelle-sha256sums.txt)",
    ] {
        assert!(text.contains(needle), "{WORKFLOW} lacks {needle:?}");
    }
    assert!(
        !text.contains("$gazelle.sha256\" >> sums.tmp"),
        "gazelle lines must stay out of dx-sha256sums.txt: old launchers reject unknown assets"
    );
    for asset in ASSETS {
        let gazelle = format!("dx-gazelle-{}", asset.trim_start_matches("dx-"));
        assert!(
            text.contains(&format!(" {gazelle}")),
            "{WORKFLOW} does not copy {gazelle}"
        );
    }
}

#[test]
fn every_cell_builds_and_compares_the_pure_gazelle() {
    let text = read_runfiles(CELL_SCRIPT);
    for needle in [
        "gazelle_flags=(--config=dx_build_release --@rules_go//go/config:pure)",
        "gazelle_asset=\"dx-gazelle-${CELL#dx-}\"",
        "cmp \"$out/$gazelle_asset\" \"$rebuilt_gazelle\"",
    ] {
        assert!(text.contains(needle), "{CELL_SCRIPT} lacks {needle:?}");
    }
}

#[test]
fn the_gazelle_pins_sit_after_the_dx_pins_under_one_version() {
    let pins = read_runfiles(PINS);
    let dx = pins.find("\nSHA256 = {\n").expect("SHA256 map");
    let gazelle = pins
        .find("\nGAZELLE_SHA256 = {\n")
        .expect("GAZELLE_SHA256 map");
    assert!(dx < gazelle);
    assert_eq!(pins.matches("_VERSION = ").count(), 1);
    for asset in ASSETS {
        let name = format!("\"dx-gazelle-{}\": ", asset.trim_start_matches("dx-"));
        assert!(pins[gazelle..].contains(&name), "{PINS} lacks {name}");
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
    let upload = text.find("\n  attest:\n").expect("attest job");
    assert!(source < upload);
    assert!(
        text[source..upload].contains("contents: read"),
        "{WORKFLOW} source job must stay read-only"
    );
    assert!(!text[source..upload].contains("contents: write"));
    assert!(text[upload..].contains("needs: [cells, source]"));
}

#[test]
fn the_source_archive_is_uploaded_without_clobber() {
    let text = workflow();
    assert!(text.contains("test -s \"out/$src\""));
    assert!(
        text.contains("for file in $(cat out/files.list) \"$src\" \"$src.sha256\" \"$src.intoto.jsonl\" $sources; do")
    );
    assert!(!text.contains("--clobber"));
    assert!(text.contains("sha256sum -c \"$src.sha256\""));
}

const CELLS_WORKFLOW: &str = ".github/workflows/release-cells.yml";
const EVIDENCE_WORKFLOW: &str = ".github/workflows/evidence.yml";
const CELL_SCRIPT: &str = "tools/ci/release_cell.sh";

#[test]
fn both_workflows_run_the_one_release_cell_script() {
    for path in [CELLS_WORKFLOW, EVIDENCE_WORKFLOW] {
        let text = read_runfiles(path);
        assert_eq!(
            text.matches(CELL_SCRIPT).count(),
            1,
            "{path} calls {CELL_SCRIPT} once"
        );
        assert!(
            !text.contains("\"$tool\" qualify"),
            "{path} must not copy the cell procedure"
        );
    }
    assert!(read_runfiles(CELL_SCRIPT).contains("\"$tool\" qualify"));
}

#[test]
fn the_evidence_release_cell_runs_uncached_and_only_on_request() {
    let text = read_runfiles(EVIDENCE_WORKFLOW);
    let step = text
        .find("- name: Build and qualify the release cell")
        .expect("release cell step");
    let cache = text
        .find("- name: Use the BuildBuddy remote cache\n        env:\n          DX_PLATFORM")
        .expect("evidence BuildBuddy step");
    assert!(
        step < cache,
        "the cell must build before the remote cache is configured"
    );
    assert!(text[step..cache].contains("if: ${{ inputs.release_cell }}"));
}

fn upload_job(text: &str) -> &str {
    let start = text.find("\n  attest:\n").expect("upload job");
    &text[start..]
}

/// The `name:` or `pattern:` values that follow each `uses: actions/<action>@` line.
fn artifact_names(text: &str, action: &str, keys: [&str; 2]) -> Vec<String> {
    let mut names = Vec::new();
    let mut in_step = false;
    for line in text.lines() {
        let new_step = line.trim_start().starts_with("- ");
        let line = line.trim().trim_start_matches("- ");
        if line.starts_with("uses:") {
            in_step = line.contains(action);
        } else if new_step {
            in_step = false;
        } else if in_step {
            for key in keys {
                if let Some(value) = line.strip_prefix(key).and_then(|v| v.strip_prefix(": ")) {
                    names.push(value.trim().replace("${{ matrix.name }}", "*"));
                }
            }
        }
    }
    names
}

/// Names match when equal, or when a `*` leaves one a prefix of the other.
fn names_match(download: &str, upload: &str) -> bool {
    if !download.contains('*') && !upload.contains('*') {
        return download == upload;
    }
    let (d, u) = (download.trim_end_matches('*'), upload.trim_end_matches('*'));
    d.starts_with(u) || u.starts_with(d)
}

fn unmatched_downloads(release: &str, cells: &str) -> Vec<String> {
    let uploads: Vec<String> = [release, cells]
        .iter()
        .flat_map(|text| artifact_names(text, "upload-artifact@", ["name", "name"]))
        .collect();
    artifact_names(
        upload_job(release),
        "download-artifact@",
        ["name", "pattern"],
    )
    .into_iter()
    .filter(|download| !uploads.iter().any(|up| names_match(download, up)))
    .collect()
}

#[test]
fn the_upload_job_downloads_the_source_artifact() {
    let text = workflow();
    let upload = upload_job(&text);
    let end = upload
        .find("Check the release inputs")
        .expect("collect step");
    assert!(upload[..end].contains("name: source\n"));
    assert!(upload[..end].contains("path: out\n"));
}

#[test]
fn the_assets_job_downloads_the_release_assets_into_out() {
    let text = workflow();
    let assets = &text[text.find("\n  assets:\n").expect("assets job")..];
    let assets = &assets[..assets.find("\n  pins:\n").expect("pins job")];
    assert!(assets.contains("name: release-assets\n          path: out\n"));
    assert!(!assets.contains("path: .\n"));
}

#[test]
fn every_upload_download_matches_an_uploaded_artifact() {
    let (release, cells) = (workflow(), read_runfiles(CELLS_WORKFLOW));
    assert_eq!(unmatched_downloads(&release, &cells), Vec::<String>::new());
    let broken = release.replacen(
        "          name: source\n          path: out\n",
        "          name: missing\n          path: out\n",
        1,
    );
    assert_eq!(
        unmatched_downloads(&broken, &cells),
        vec!["missing".to_owned()]
    );
}

#[test]
fn the_dry_run_report_runs_every_check_before_failing() {
    let text = workflow();
    let start = text.find("Emit dry-run report").expect("dry-run step");
    let end = start
        + text[start..]
            .find("Prove checkout left clean")
            .expect("next step");
    let step = &text[start..end];
    assert!(!step.contains("set -euo pipefail"));
    assert!(step.contains("failed steps"));
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
            "module(\n    name = \"rules_dx\",\n    version = \"{version}\","
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
fn the_upload_checks_the_cells_version_and_commit_before_hashing() {
    let text = workflow();
    let check = text
        .find("test \"$CELLS_VERSION\" = \"${RELEASE_TAG#v}\"")
        .expect("version check");
    assert!(text.contains("test \"$CELLS_SHA\" = \"$CANDIDATE_COMMIT\""));
    let hash = text.find("hash=\"$(sha256sum").expect("hash");
    assert!(
        check < hash,
        "{WORKFLOW} hashes before checking the version"
    );
}

#[test]
fn the_upload_checks_name_the_wanted_and_actual_values() {
    let text = workflow();
    for needle in [
        "|| { echo \"want ${RELEASE_TAG#v}, got $CELLS_VERSION\" >&2; exit 1; }",
        "|| { echo \"want $CANDIDATE_COMMIT, got $CELLS_SHA\" >&2; exit 1; }",
        "|| { echo \"want a non-empty out/$file\" >&2; exit 1; }",
        "|| { echo \"want 19 manifest lines, got $(wc -l < out/dx-sha256sums.txt)\" >&2; exit 1; }",
    ] {
        assert!(text.contains(needle), "{WORKFLOW} lacks {needle:?}");
    }
}

#[test]
fn the_source_job_refuses_a_tag_the_committed_versions_do_not_match() {
    let text = workflow();
    let source = text.find("\n  source:\n").expect("source job");
    let gate = text.find("tag=\"${RELEASE_TAG#v}\"").expect("version gate");
    let driver = text.find("release_driver").expect("driver");
    assert!(source < gate && gate < driver);
    assert!(text.contains("land the version bump first"));
}

#[test]
fn the_version_gate_names_all_four_version_files_before_building() {
    let text = workflow();
    let gate = text
        .find("Check the committed versions equal the tag")
        .expect("version gate step");
    let archive = text.find("source_archive").expect("source archive");
    let step = &text[gate..archive];
    for check in [
        "check \"cli/prebuilt/pins.bzl DX_VERSION\"",
        "check \"MODULE.bazel module version\"",
        "check \"cli/adopt/src/version.rs DX_VERSION\"",
        "check \"cli/adopt/src/version.rs MODULE_VERSION\"",
    ] {
        assert!(
            step.contains(check),
            "{WORKFLOW} version gate lacks {check}"
        );
    }
}

#[test]
fn the_archives_are_named_per_platform() {
    let text = workflow();
    assert!(text.contains("dx-windows-*) archive=\"$asset.zip\""));
    assert!(text.contains("*) archive=\"$asset.tar.gz\""));
}

#[test]
fn the_attestation_is_signed_once_from_the_tag_in_the_upload_job() {
    let text = workflow();
    let step = "uses: actions/attest@1e69f48acb82d1966a394da916b4c1698aa569d6 # v4.2.2";
    assert_eq!(text.matches("actions/attest@").count(), 2);
    assert!(text.contains("subject-path: out/rules_dx-${{ needs.cells.outputs.version }}.tar.gz"));
    assert!(text.contains(".tar.gz.intoto.jsonl\"\n"));
    let upload = text.find("\n  attest:\n").expect("attest job");
    for (at, _) in text.match_indices(step) {
        assert!(at > upload);
        let head = &text[text[..at].rfind("      - name:").expect("step")..at];
        assert!(head.contains("steps.plan.outputs.attest == 'true'"));
        assert!(head.contains("github.ref == format('refs/tags/{0}', env.RELEASE_TAG)"));
    }
    assert!(text.contains("subject-checksums: ${{ runner.temp }}/subjects.txt"));
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
fn the_archives_are_verified_with_the_expected_flags() {
    let text = workflow();
    for flag in [
        "--bundle",
        "--repo",
        "--cert-identity",
        "--cert-oidc-issuer",
        "--source-ref",
        "--deny-self-hosted-runners",
    ] {
        assert!(text.contains(&format!("{flag}")), "{WORKFLOW} lacks {flag}");
    }
    assert!(text.contains("release.yml@refs/tags/$RELEASE_TAG"));
    assert!(text.contains("--source-ref \"refs/tags/$RELEASE_TAG\""));
    let verify = text.find("gh attestation verify").expect("verify");
    let put = text
        .find("release_driver -- \"$RELEASE_TAG\" \"$CANDIDATE_COMMIT\" \"${paths")
        .expect("upload");
    assert!(verify < put);
}

#[test]
fn a_published_release_is_checked_by_running_the_downloaded_dx() {
    let text = workflow();
    let text = &text[text.find("\n  assets:\n").expect("assets job")..];
    let download = text
        .find("--pattern 'dx-*' --dir verify")
        .expect("download");
    let check = text
        .find("sha256sum -c dx-sha256sums.txt && sha256sum -c dx-gazelle-sha256sums.txt)")
        .expect("manifest check");
    let run = text
        .find("tar -xzf verify/dx-linux-x86_64.tar.gz -C verify dx")
        .expect("extract");
    assert!(download < check && check < run);
    assert!(text.contains("verify/dx --version"));
    assert!(!text.contains("--apply"));
}

#[test]
fn pull_requests_that_touch_pins_run_the_attested_check() {
    let text = read_runfiles(".github/workflows/ci.yml");
    assert!(text.contains("^cli/prebuilt/pins.bzl$"));
    assert!(text.contains("//tools/ci:fill_pins_bin -- \"v$version\" --check"));
}

#[test]
fn the_source_job_fills_the_pins_into_the_archive_and_the_pins_job_checks_them() {
    let text = workflow();
    let source = text.find("\n  source:\n").expect("source job");
    let attest = text.find("\n  attest:\n").expect("attest job");
    let source_job = &text[source..attest];
    assert!(source_job.contains("needs: cells"));
    let fill = source_job
        .find("fill_pins_bin -- \"$RELEASE_TAG\" --manifest")
        .expect("fill_pins_bin --manifest");
    let archive = source_job
        .find("source_archive -- \"$RELEASE_TAG\" \"$CANDIDATE_COMMIT\" \"$SOURCE_DIR\" --pins")
        .expect("source_archive --pins");
    assert!(fill < archive);
    assert!(source_job.contains("cp cli/prebuilt/pins.bzl \"$pins\""));
    assert!(!source_job.contains("contents: write"));
    let pins = &text[text.find("\n  pins:\n").expect("pins job")..];
    assert!(pins.contains("--check --filled --pins"));
    assert!(pins.contains("cli/prebuilt/pins.bzl"));
}

#[test]
fn the_release_and_ci_workflows_need_no_python() {
    for path in [WORKFLOW, ".github/workflows/ci.yml"] {
        assert!(
            !read_runfiles(path).contains("python3"),
            "{path} uses python3"
        );
    }
}

#[test]
fn publishing_runs_the_consumer_against_the_prebuilt_download() {
    let text = read_runfiles(CONSUMER);
    assert!(text.contains("types: [published]"));
    assert!(text.contains("permissions:\n  contents: read\n"));
    assert!(text.contains("@rules_dx//:dx -- --version"));
    assert!(text.contains("dx_prebuilt/$KEY/dx"));
    assert!(!text.contains("dx_prebuilt_"));
}

#[test]
fn the_consumer_matrix_keys_match_the_hub_outputs() {
    let text = read_runfiles(CONSUMER);
    for (cell, key) in [
        ("dx-linux-x86_64", "linux_x86_64"),
        ("dx-linux-arm64", "linux_aarch64"),
        ("dx-macos-arm64", "macos_aarch64"),
        ("dx-windows-x86_64", "windows_x86_64"),
        ("dx-windows-arm64", "windows_aarch64"),
    ] {
        assert!(
            text.contains(&format!("- name: {cell}\n            key: {key}\n")),
            "{cell} must map to {key}"
        );
    }
}

fn consumer_job(name: &str) -> String {
    let text = read_runfiles(CONSUMER);
    let start = text.find(&format!("\n  {name}:\n")).expect("job") + 1;
    let rest = &text[start..];
    let end = rest
        .match_indices("\n  ")
        .map(|(i, _)| i)
        .find(|&i| rest[i + 3..].chars().next().is_some_and(|c| c != ' '))
        .unwrap_or(rest.len());
    rest[..end].to_owned()
}

#[test]
fn the_bcr_submission_waits_for_the_consumer_check_and_alone_can_attest() {
    let bcr = consumer_job("bcr");
    assert!(bcr.contains("needs: consumer\n"));
    for permission in ["contents: write", "id-token: write", "attestations: write"] {
        assert!(bcr.contains(permission), "{permission}");
    }
    assert!(bcr.contains("attest: true"));
    let text = read_runfiles(CONSUMER);
    assert!(text.contains("permissions:\n  contents: read\n"));
    for permission in ["contents: write", "id-token: write", "attestations: write"] {
        assert_eq!(text.matches(permission).count(), 1, "{permission}");
    }
    let uses = bcr
        .lines()
        .find_map(|line| line.trim().strip_prefix("uses: "))
        .expect("uses");
    let (_, rest) = uses.split_once('@').expect("pinned");
    let sha = rest.split_whitespace().next().expect("sha");
    assert!(
        sha.len() == 40 && sha.bytes().all(|b| b.is_ascii_hexdigit()),
        "bcr uses must be SHA-pinned: {uses}"
    );
    assert!(!bcr.contains("run:"));
}

#[test]
fn the_bcr_ledger_follows_the_bcr_job_and_records_failures() {
    let ledger = consumer_job("bcr-ledger");
    assert!(ledger.contains("needs: bcr\n"));
    assert!(ledger.contains("if: always() && needs.bcr.result != 'skipped'"));
    assert!(ledger.contains("contents: read"));
    assert!(!ledger.contains("contents: write"));
    assert!(ledger.contains("//deploy/release:bcr_ledger"));
    assert!(ledger.contains("name: bcr-ledger"));
    assert!(!ledger.contains("secrets."));
}

fn job_steps(job: &str) -> Vec<&str> {
    job.split("\n      - ").skip(1).collect()
}

#[test]
fn a_dry_run_never_signs_or_publishes() {
    let text = workflow();
    let assets = text.find("\n  assets:\n").expect("assets job");
    let pins = text.find("\n  pins:\n").expect("pins job");
    let attest = text.find("\n  attest:\n").expect("attest job");
    for job in [&text[attest..assets], &text[assets..pins]] {
        for step in job_steps(job) {
            let writes = [
                "gh release",
                "actions/attest@",
                "gh attestation",
                "RELEASE_DRY_RUN=0",
                "actions/checkout",
            ]
            .iter()
            .any(|needle| step.contains(needle));
            if writes {
                let cond = step
                    .lines()
                    .find(|line| line.trim_start().starts_with("if:"))
                    .unwrap_or_else(|| panic!("no if: in step\n{step}"));
                assert!(cond.contains("!inputs.dry_run"), "ungated step\n{step}");
            }
        }
    }
}

#[test]
fn the_asset_paths_are_collected_on_a_dry_run_too() {
    let text = workflow();
    let assets = &text[text.find("\n  assets:\n").expect("assets job")..];
    let step = job_steps(assets)
        .into_iter()
        .find(|step| step.starts_with("name: Collect the release asset paths"))
        .expect("collect step");
    assert!(!step.contains("if:"));
    assert!(step.contains("DRY_RUN: ${{ inputs.dry_run }}"));
    assert!(step.contains("test -s \"out/$file\""));
}

#[test]
fn the_tag_is_derived_once_in_three_job_envs() {
    let text = workflow();
    let expr = "RELEASE_TAG: ${{ inputs.dry_run && format('v{0}', needs.cells.outputs.version) || inputs.tag || github.ref_name }}";
    assert_eq!(text.matches(expr).count(), 3);
    let pins = text.find("\n  pins:\n").expect("pins job");
    assert!(text[pins..].contains("\n    if: ${{ !inputs.dry_run }}"));
}
