use std::path::{Path, PathBuf};

use dx_release_tools::{
    dry_run_check, render_dry_run_console, render_dry_run_report, render_dry_run_summary,
    write_dry_run_outputs, DryRunInputs,
};

fn data(name: &str) -> PathBuf {
    let rel = std::env::var(name).unwrap_or_else(|_| panic!("{name} must name a test input"));
    dx_testing::resolve_runfiles(&rel)
}

fn scratch() -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix("dx-dry-run-report-")
        .tempdir()
        .unwrap_or_else(|error| panic!("test scratch: {error}"))
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()))
}

fn write(dir: &Path, name: &str, text: &str) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, text).unwrap_or_else(|error| panic!("write {}: {error}", path.display()));
    path
}

fn write_bytes(dir: &Path, name: &str, bytes: &[u8]) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, bytes)
        .unwrap_or_else(|error| panic!("write {}: {error}", path.display()));
    path
}

fn draft_log_text() -> String {
    "release: draft v0.0.0-dryrun\ncommand: gh release create v0.0.0-dryrun --draft --verify-tag\n"
        .to_owned()
}

fn signing_log_text() -> String {
    "signing: dry run (RELEASE_SIGN_DRY_RUN=1); would sign, publishing nothing:\n  trust root: https://tuf-repo-cdn.sigstore.dev\n  command: cosign sign-blob --bundle sbom_demo.spdx.json.bundle sbom_demo.spdx.json\n  asset: sbom_demo.provenance.json\n  asset: notice_demo.NOTICE\n"
        .to_owned()
}

fn bcr_log_text() -> String {
    "bcr: dry run (BCR_DRY_RUN=1); would submit, submitting nothing:\n  module: rules_dx\n  version: 0.0.0\n"
        .to_owned()
}

fn driver_log_text() -> String {
    "release: dry run (RELEASE_DRY_RUN=1); would release, publishing nothing:\n  tag: v0.0.0-dryrun\n"
        .to_owned()
}

fn tests_log_text() -> String {
    "INFO: Build completed successfully\n//deploy/release:all: tests pass\n".to_owned()
}

fn refusal_log_text() -> String {
    "verify: checksum-only verification is not publisher-identity proof; refusing before install\n"
        .to_owned()
}

fn module_text() -> String {
    "module(\n    name = \"rules_dx\",\n    version = \"0.0.0\",\n)\n".to_owned()
}

type GapBreak = fn(&mut DryRunInputs, &Path);

struct GapCase {
    label: &'static str,
    break_inputs: GapBreak,
    want: &'static str,
}

fn listing_text() -> String {
    "bazel-out/k8-opt/bin/deploy/release/licenses.toml\nbazel-out/k8-opt/bin/deploy/release/sbom_demo.spdx.json\nbazel-out/k8-opt/bin/deploy/release/sbom_demo.provenance.json\nbazel-out/k8-opt/bin/deploy/release/notice_demo.NOTICE\n".to_owned()
}

fn stage_inputs(dir: &Path, approve: bool) -> (DryRunInputs, PathBuf) {
    let stage = dir.join("stage");
    std::fs::create_dir_all(&stage).unwrap_or_else(|error| panic!("stage dir: {error}"));
    let binary = write_bytes(&stage, "dx-linux-x86_64", b"seed binary bytes\n");
    let archive = write_bytes(&stage, "dx-standalone.tar.gz", b"seed archive bytes\n");
    let listing = write(&stage, "release-artifacts.txt", &listing_text());
    let spdx = write(&stage, "sbom_demo.spdx.json", &read(&data("DX_SBOM_SPDX")));
    let provenance = write(
        &stage,
        "sbom_demo.provenance.json",
        &read(&data("DX_SBOM_PROVENANCE")),
    );
    let notice = write(&stage, "notice_demo.NOTICE", &read(&data("DX_NOTICE")));
    let draft = write(&stage, "draft-dry-run.log", &draft_log_text());
    let tests = write(&stage, "release-tests.log", &tests_log_text());
    let signing = write(&stage, "signing-dry-run.log", &signing_log_text());
    let bcr = write(&stage, "bcr-dry-run.log", &bcr_log_text());
    let driver = write(&stage, "human-run-dry-run.log", &driver_log_text());
    let refusal = write(&stage, "verify-refusal.log", &refusal_log_text());
    let module = write(&stage, "MODULE.bazel", &module_text());
    let inputs = DryRunInputs {
        approve,
        binary,
        standalone_archive: archive,
        release_artifacts: listing,
        spdx,
        provenance,
        notice,
        sbom_artifact: data("DX_SBOM_ARTIFACT"),
        notice_manifest: data("DX_NOTICE_MANIFEST"),
        notice_texts: vec![data("DX_NOTICE_TEXT_A"), data("DX_NOTICE_TEXT_B")],
        draft_log: draft,
        tests_log: tests,
        signing_log: signing,
        bcr_log: bcr,
        driver_log: driver,
        verify_refusal_log: refusal,
        module_file: module,
    };
    (inputs, stage)
}

fn report_json(report: &dx_release_tools::DryRunReport) -> serde_json::Value {
    serde_json::from_str(&render_dry_run_report(report))
        .unwrap_or_else(|error| panic!("report is JSON: {error}"))
}

#[test]
fn the_happy_path_binds_digests_and_never_publishes() {
    let dir = scratch();
    let (inputs, stage) = stage_inputs(dir.path(), false);
    let report = dry_run_check(&inputs).unwrap_or_else(|error| panic!("happy path: {error}"));
    let binary_digest = dx_digest::sha256_file_hex(&inputs.binary).expect("hash binary");
    let standalone_digest =
        dx_digest::sha256_file_hex(&inputs.standalone_archive).expect("hash archive");
    assert_eq!(report.binary_digest, binary_digest);
    assert_eq!(report.standalone_digest, standalone_digest);
    assert!(!report.approved);
    write_dry_run_outputs(&stage, &report).unwrap_or_else(|error| panic!("write outputs: {error}"));
    assert_eq!(
        read(&stage.join("dx-linux-x86_64.sha256")),
        format!("{binary_digest}  dx-linux-x86_64\n")
    );
    assert_eq!(
        read(&stage.join("dx-standalone.tar.gz.sha256")),
        format!("{standalone_digest}  dx-standalone.tar.gz\n")
    );
    let document = report_json(&report);
    assert_eq!(document["schema"], serde_json::json!("publish-dry-run/v1"));
    assert_eq!(document["approved"], serde_json::json!(false));
    assert_eq!(document["published"], serde_json::json!(false));
    assert_eq!(
        document["artifacts"][0]["sha256"],
        serde_json::json!(binary_digest)
    );
    assert_eq!(
        document["artifacts"][1]["sha256"],
        serde_json::json!(standalone_digest)
    );
    let spdx: serde_json::Value = serde_json::from_str(&read(&inputs.spdx)).expect("spdx JSON");
    assert_eq!(
        document["sbom"]["subject_digest"],
        spdx["packages"][0]["checksums"][0]["checksumValue"]
    );
    for section in [
        "draft_dry_run",
        "sbom",
        "notice",
        "packaging",
        "signing_dry_run",
    ] {
        assert_eq!(
            document[section]["published"],
            serde_json::json!(false),
            "{section} must not publish"
        );
    }
    assert_eq!(document["bcr_shape"]["submitted"], serde_json::json!(false));
    assert_eq!(
        document["verify_refusal"]["installed"],
        serde_json::json!(false)
    );
    let summary = render_dry_run_summary(&report);
    assert!(
        summary.contains(&binary_digest),
        "summary names the binary digest"
    );
    assert!(
        summary.contains(&standalone_digest),
        "summary names the archive digest"
    );
    assert!(
        summary.contains("published: **no**"),
        "summary says nothing publishes"
    );
    let console = render_dry_run_console(&report);
    assert!(console.contains(&format!("seed binary sha256: {binary_digest}")));
    assert!(console.contains("approved: false; published: False (dry run never publishes)"));
}

#[test]
fn approval_is_recorded_not_gated() {
    let dir = scratch();
    let (inputs, _) = stage_inputs(dir.path(), true);
    let report = dry_run_check(&inputs).unwrap_or_else(|error| panic!("approved run: {error}"));
    assert!(report.approved);
    assert_eq!(report_json(&report)["approved"], serde_json::json!(true));
    assert!(render_dry_run_summary(&report).contains("- approved: `true`"));
    assert!(render_dry_run_console(&report).contains("approved: true;"));
}

#[test]
fn the_real_module_bazel_carries_the_bcr_shape() {
    let text = read(&data("DX_MODULE_BAZEL"));
    assert!(
        text.contains("name = \"rules_dx\""),
        "module names rules_dx"
    );
    assert!(text.contains("version = \"0.0.0\""), "module pins 0.0.0");
}

#[test]
fn a_release_listing_missing_a_member_fails_named() {
    let dir = scratch();
    let (mut inputs, _) = stage_inputs(dir.path(), false);
    let partial = dir.path().join("partial.txt");
    std::fs::write(
        &partial,
        "bazel-out/k8-opt/bin/deploy/release/licenses.toml\n",
    )
    .expect("partial listing");
    inputs.release_artifacts = partial;
    let error = dry_run_check(&inputs).expect_err("a partial listing must not pass");
    assert!(
        error.contains("release artifacts missing sbom_demo.spdx.json"),
        "got {error}"
    );
}

#[test]
fn a_swapped_spdx_digest_breaks_the_provenance_linkage() {
    let dir = scratch();
    let (mut inputs, _) = stage_inputs(dir.path(), false);
    let zeros = "0".repeat(64);
    let doctored = read(&inputs.spdx).replace(&inputs_digest(&inputs), &zeros);
    assert_ne!(
        doctored,
        read(&inputs.spdx),
        "the demo SPDX binds a real digest"
    );
    let path = write(dir.path(), "doctored.spdx.json", &doctored);
    inputs.spdx = path;
    let error = dry_run_check(&inputs).expect_err("a swapped SPDX must not pass");
    assert!(
        error.contains("SBOM provenance missing SPDX subject digest"),
        "got {error}"
    );
}

fn inputs_digest(inputs: &DryRunInputs) -> String {
    let text = read(&inputs.spdx);
    dx_release_tools::dry_run_spdx_subject(&text).expect("demo SPDX has a subject digest")
}

#[test]
fn a_forged_provenance_builder_fails_closed() {
    let dir = scratch();
    let (mut inputs, _) = stage_inputs(dir.path(), false);
    let forged = read(&inputs.provenance).replace(
        "https://github.com/ralvik/rules_dx/.github/workflows/publish-dry-run.yml",
        "https://example.test/builder",
    );
    let path = write(dir.path(), "forged.provenance.json", &forged);
    inputs.provenance = path;
    let error = dry_run_check(&inputs).expect_err("a forged builder must not pass");
    assert!(
        error.contains("builder.id is not an allowlisted"),
        "got {error}"
    );
}

#[test]
fn a_headerless_notice_fails() {
    let dir = scratch();
    let (mut inputs, _) = stage_inputs(dir.path(), false);
    let path = write(dir.path(), "headerless.NOTICE", "tampered bytes\n");
    inputs.notice = path;
    let error = dry_run_check(&inputs).expect_err("a headerless NOTICE must not pass");
    assert!(
        error.contains("NOTICE bundle missing header"),
        "got {error}"
    );
}

#[test]
fn a_notice_missing_an_entry_fails() {
    let dir = scratch();
    let (mut inputs, _) = stage_inputs(dir.path(), false);
    let text = read(&inputs.notice);
    let first_header = text
        .lines()
        .find(|line| line.starts_with("=== "))
        .expect("entry header");
    let stripped = text.replacen(&format!("{first_header}\n"), "", 1);
    assert_ne!(stripped, text, "the demo NOTICE has entry headers");
    let path = write(dir.path(), "stripped.NOTICE", &stripped);
    inputs.notice = path;
    let error = dry_run_check(&inputs).expect_err("a stripped NOTICE must not pass");
    assert!(error.contains("NOTICE"), "got {error}");
}

#[test]
fn log_and_shape_gaps_fail_with_their_role() {
    let dir = scratch();
    let cases = vec![
        GapCase {
            label: "draft placeholder",
            break_inputs: |inputs: &mut DryRunInputs, scratch: &Path| {
                inputs.draft_log = write(scratch, "draft.log", "release: draft\n");
            },
            want: "draft dry-run log missing",
        },
        GapCase {
            label: "release tests",
            break_inputs: |inputs: &mut DryRunInputs, scratch: &Path| {
                inputs.tests_log =
                    write(scratch, "tests.log", "INFO: Build completed successfully\n");
            },
            want: "release tests log missing",
        },
        GapCase {
            label: "signing trust root",
            break_inputs: |inputs: &mut DryRunInputs, scratch: &Path| {
                inputs.signing_log = write(scratch, "signing.log", "signing: cosign sign-blob\n");
            },
            want: "signing dry-run log missing",
        },
        GapCase {
            label: "signing packaging",
            break_inputs: |inputs: &mut DryRunInputs, scratch: &Path| {
                inputs.signing_log = write(
                    scratch,
                    "signing.log",
                    "signing: cosign sign-blob on https://tuf-repo-cdn.sigstore.dev\n",
                );
            },
            want: "signing dry-run log missing",
        },
        GapCase {
            label: "bcr gate",
            break_inputs: |inputs: &mut DryRunInputs, scratch: &Path| {
                inputs.bcr_log = write(scratch, "bcr.log", "bcr: submitted\n");
            },
            want: "BCR dry-run log missing",
        },
        GapCase {
            label: "driver gate",
            break_inputs: |inputs: &mut DryRunInputs, scratch: &Path| {
                inputs.driver_log = write(scratch, "driver.log", "release: live\n");
            },
            want: "release driver dry-run log missing",
        },
        GapCase {
            label: "refusal proof",
            break_inputs: |inputs: &mut DryRunInputs, scratch: &Path| {
                inputs.verify_refusal_log = write(scratch, "refusal.log", "verify: ok\n");
            },
            want: "install verifier refusal log missing",
        },
        GapCase {
            label: "module name",
            break_inputs: |inputs: &mut DryRunInputs, scratch: &Path| {
                inputs.module_file = write(
                    scratch,
                    "MODULE.bazel",
                    "module(\n    version = \"0.0.0\",\n)\n",
                );
            },
            want: "module file missing",
        },
        GapCase {
            label: "module version",
            break_inputs: |inputs: &mut DryRunInputs, scratch: &Path| {
                inputs.module_file = write(
                    scratch,
                    "MODULE.bazel",
                    "module(\n    name = \"rules_dx\",\n)\n",
                );
            },
            want: "module file missing",
        },
    ];
    for case in cases {
        let (mut inputs, _) = stage_inputs(dir.path(), false);
        (case.break_inputs)(&mut inputs, dir.path());
        let error = dry_run_check(&inputs).expect_err(&format!("{} must not pass", case.label));
        assert!(error.contains(case.want), "{}: got {error}", case.label);
    }
}

#[test]
fn a_missing_input_file_names_its_role_and_path() {
    let dir = scratch();
    let (mut inputs, _) = stage_inputs(dir.path(), false);
    inputs.binary = dir.path().join("absent-dx-linux-x86_64");
    let error = dry_run_check(&inputs).expect_err("a missing binary must not pass");
    assert!(error.contains("cannot hash staged binary"), "got {error}");
    assert!(error.contains("absent-dx-linux-x86_64"), "got {error}");
}

#[test]
fn a_malformed_spdx_fails_actionable() {
    let dir = scratch();
    let (mut inputs, _) = stage_inputs(dir.path(), false);
    let path = write(
        dir.path(),
        "broken.spdx.json",
        "{\"spdxVersion\": \"SPDX-2.3\", broken",
    );
    inputs.spdx = path;
    let error = dry_run_check(&inputs).expect_err("malformed SPDX must not pass");
    assert!(error.contains("SBOM SPDX is not JSON"), "got {error}");
    let path = write(
        dir.path(),
        "empty.spdx.json",
        "{\"spdxVersion\": \"SPDX-2.3\"}",
    );
    inputs.spdx = path;
    let error = dry_run_check(&inputs).expect_err("subjectless SPDX must not pass");
    assert!(
        error.contains("no packages[0].checksums[0].checksumValue"),
        "got {error}"
    );
}

#[test]
fn report_outputs_land_beside_the_staged_artifacts() {
    let dir = scratch();
    let (inputs, stage) = stage_inputs(dir.path(), false);
    let report = dry_run_check(&inputs).unwrap_or_else(|error| panic!("happy path: {error}"));
    write_dry_run_outputs(&stage, &report).unwrap_or_else(|error| panic!("write outputs: {error}"));
    for name in [
        "dx-linux-x86_64.sha256",
        "dx-standalone.tar.gz.sha256",
        "dry-run-report.json",
        "dry-run-summary.md",
    ] {
        assert!(stage.join(name).is_file(), "stage holds {name}");
    }
    let missing = stage.join("nowhere");
    std::fs::create_dir_all(&missing).expect("dir as file target");
    let error = write_dry_run_outputs(&missing.join("child"), &report)
        .expect_err("an unwritable stage must fail");
    assert!(
        error.to_string().contains("dry-run: cannot write"),
        "got {error}"
    );
}
