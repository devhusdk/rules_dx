use std::path::{Path, PathBuf};

use super::{notice_verify_files, sbom_verify_files};

pub const REPORT_SCHEMA: &str = "publish-dry-run/v1";

pub const SIGNING_TRUST_ROOT: &str = super::SIGNING_TRUST_ROOT;

pub struct PublishDryRunInputs {
    pub binary: PathBuf,
    pub archive: PathBuf,
    pub sbom_artifact: PathBuf,
    pub spdx: PathBuf,
    pub provenance: PathBuf,
    pub notice: PathBuf,
    pub notice_manifest: PathBuf,
    pub notice_texts: Vec<PathBuf>,
    pub release_artifacts_list: PathBuf,
    pub draft_log: PathBuf,
    pub signing_log: PathBuf,
    pub bcr_log: PathBuf,
    pub human_run_log: PathBuf,
    pub release_tests_log: PathBuf,
    pub module_bazel: PathBuf,
    pub verify_refusal_log: PathBuf,
    pub approve: bool,
}

#[derive(Debug)]
pub struct PublishDryRunOutcome {
    pub report: String,
    pub binary_digest: String,
    pub archive_digest: String,
    pub sbom_digest: String,
}

fn read_text(path: &Path, what: &str) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|error| {
        format!(
            "publish dry-run: cannot read {what} {}: {error}",
            path.display()
        )
    })
}

fn require_contains(haystack: &str, needle: &str, what: &str) -> Result<(), String> {
    if haystack.contains(needle) {
        return Ok(());
    }
    Err(format!("publish dry-run: {what} missing {needle:?}"))
}

fn sbom_subject_digest(spdx_path: &Path) -> Result<String, String> {
    let text = read_text(spdx_path, "SBOM SPDX")?;
    let value: serde_json::Value = serde_json::from_str(&text)
        .map_err(|error| format!("publish dry-run: SBOM SPDX is not JSON: {error}"))?;
    let digest = value
        .get("packages")
        .and_then(|packages| packages.as_array())
        .and_then(|packages| packages.first())
        .and_then(|package| package.get("checksums"))
        .and_then(|checksums| checksums.as_array())
        .and_then(|checksums| checksums.first())
        .and_then(|checksum| checksum.get("checksumValue"))
        .and_then(|digest| digest.as_str())
        .ok_or_else(|| {
            "publish dry-run: SBOM SPDX packages[0].checksums[0].checksumValue is missing"
                .to_owned()
        })?;
    if digest.is_empty() {
        return Err("publish dry-run: SBOM subject digest is empty".to_owned());
    }
    Ok(digest.to_owned())
}

pub fn publish_dry_run_check(inputs: &PublishDryRunInputs) -> Result<PublishDryRunOutcome, String> {
    let binary_digest = dx_digest::sha256_file_hex(&inputs.binary)
        .map_err(|error| format!("publish dry-run: cannot hash staged binary: {error}"))?;
    let archive_digest = dx_digest::sha256_file_hex(&inputs.archive)
        .map_err(|error| format!("publish dry-run: cannot hash staged archive: {error}"))?;
    sbom_verify_files(&inputs.sbom_artifact, &inputs.spdx, &inputs.provenance)
        .map_err(|error| format!("publish dry-run: SBOM validation failed: {error}"))?;
    notice_verify_files(
        &inputs.notice,
        &inputs.notice_manifest,
        &inputs.notice_texts,
    )
    .map_err(|error| format!("publish dry-run: NOTICE validation failed: {error}"))?;
    let artifacts_list = read_text(&inputs.release_artifacts_list, "release artifacts list")?;
    for required in [
        "licenses.toml",
        "sbom_demo.spdx.json",
        "sbom_demo.provenance.json",
        "notice_demo.NOTICE",
    ] {
        require_contains(&artifacts_list, required, "release artifacts list")?;
    }
    let draft_log = read_text(&inputs.draft_log, "draft dry-run log")?;
    require_contains(&draft_log, "v0.0.0-dryrun", "draft dry-run log")?;
    require_contains(&draft_log, "--draft --verify-tag", "draft dry-run log")?;
    let signing_log = read_text(&inputs.signing_log, "signing dry-run log")?;
    require_contains(&signing_log, "cosign sign-blob", "signing dry-run log")?;
    require_contains(&signing_log, SIGNING_TRUST_ROOT, "signing dry-run log")?;
    require_contains(&signing_log, "sbom_demo.spdx.json", "signing dry-run log")?;
    require_contains(
        &signing_log,
        "sbom_demo.provenance.json",
        "signing dry-run log",
    )?;
    require_contains(&signing_log, "notice_demo.NOTICE", "signing dry-run log")?;
    let bcr_log = read_text(&inputs.bcr_log, "BCR dry-run log")?;
    require_contains(
        &bcr_log,
        "would submit, submitting nothing",
        "BCR dry-run log",
    )?;
    let human_run_log = read_text(&inputs.human_run_log, "release driver dry-run log")?;
    require_contains(
        &human_run_log,
        "dry run (RELEASE_DRY_RUN=1)",
        "release driver dry-run log",
    )?;
    let release_tests_log = read_text(&inputs.release_tests_log, "release tests log")?;
    require_contains(&release_tests_log, "tests pass", "release tests log")?;
    let module_text = read_text(&inputs.module_bazel, "MODULE.bazel")?;
    require_contains(&module_text, "name = \"rules_dx\"", "MODULE.bazel")?;
    require_contains(&module_text, "version = \"0.0.0\"", "MODULE.bazel")?;
    let verify_log = read_text(&inputs.verify_refusal_log, "install verifier refusal log")?;
    require_contains(
        &verify_log,
        "checksum-only verification is not publisher-identity proof",
        "install verifier refusal log",
    )?;
    let sbom_digest = sbom_subject_digest(&inputs.spdx)?;
    let report = super::render_pretty(&serde_json::json!({
        "approved": inputs.approve,
        "artifacts": [
            {
                "name": "dx-linux-x86_64",
                "note": "seed-host binary; release matrix frozen in deploy/release/matrix.bzl",
                "sha256": binary_digest,
            },
            {
                "name": "dx-standalone.tar.gz",
                "note": "seed-host standalone archive via //cli/cli:dx_standalone; wider matrix qualified per-host under issue #815 (human-run only, not built here)",
                "sha256": archive_digest,
            },
        ],
        "bcr_shape": {
            "checked": true,
            "mode": "BCR_DRY_RUN=1",
            "module": "rules_dx",
            "submitted": false,
            "target": "//deploy/release:bcr_demo",
            "version": "0.0.0",
        },
        "draft_dry_run": {
            "flags": "--draft --verify-tag",
            "mode": "GH_RELEASE_DRY_RUN=1",
            "ok": true,
            "placeholder": "v0.0.0-dryrun",
            "published": false,
            "target": "//cli/cli:github_draft",
        },
        "exercised": [
            "seed binary build //cli/cli:dx with sha256 staged under RUNNER_TEMP",
            "seed standalone archive //cli/cli:dx_standalone with sha256 staged under RUNNER_TEMP",
            "seed releasable unit //deploy/release:release_artifacts (audit curator plus binary plus man page plus NOTICE plus SBOM/provenance, publishes nothing)",
            "SBOM/provenance generation //deploy/release:sbom_demo (SPDX-2.3 + SLSA v1, subject digest equal to artifact sha256, owner-gated, publishes nothing)",
            "aggregated NOTICE //deploy/release:notice_demo (hermetic notice_bundle, byte-identical, missing-notice-text fails, publishes nothing)",
            "draft-only publisher //cli/cli:github_draft in GH_RELEASE_DRY_RUN=1 mode (placeholder v0.0.0-dryrun, draft-only flags, publishes nothing)",
            "owner-gated signing //deploy/release:signing_demo in RELEASE_SIGN_DRY_RUN=1 mode (Sigstore keyless + attestation over SBOM pair plus NOTICE, publishes nothing)",
            "Bazel Central Registry shape via //deploy/release:bcr_demo in BCR_DRY_RUN=1 mode (rules_dx at 0.0.0, checked not submitted)",
            "human-run driver //deploy/release:release_driver dry run (owner approval + tag ceiling, publishes nothing)",
            "install verifier //deploy/install:dx_verify refusal proof (checksum-only rejected, TUF trust root, fails before install)",
        ],
        "not_attempted": [
            "SBOM/provenance publishing (generated owner-gated via //deploy/release:sbom_demo)",
            "Signing/attestation publishing (Sigstore keyless + GitHub attestations on the trust root, signing-first; GHCR images sign separately via cosign <digest>)",
            "GHCR prebuilt images (separate workflow .github/workflows/ghcr.yml per owner decision)",
            "Non-seed release matrix builds (qualified per-host under issue #815, human-run only, not built in dry-run)",
            "BCR submission (owner-gated via //deploy/release:bcr_demo)",
            "Any tag, registry submission, or release creation",
        ],
        "notice": {
            "header_ok": true,
            "signed_with_sbom": true,
            "target": "//deploy/release:notice_demo",
            "verified": true,
            "published": false,
        },
        "packaging": {
            "curator": "//:audit_curator",
            "ok": true,
            "published": false,
            "sbom_signed_with_notice": true,
            "target": "//deploy/release:release_artifacts",
        },
        "published": false,
        "release_matrix": [
            {"name": "dx-linux-x86_64", "status": "qualified-seed-built-here"},
            {"name": "dx-linux-arm64", "status": "qualified-host-evidence"},
            {"name": "dx-macos-arm64", "status": "qualified-host-evidence"},
            {"name": "dx-windows-x86_64", "status": "qualified-host-evidence"},
        ],
        "sbom": {
            "linkage_ok": true,
            "predicate": "https://slsa.dev/provenance/v1",
            "published": false,
            "spdx": "SPDX-2.3",
            "subject_digest": sbom_digest,
            "target": "//deploy/release:sbom_demo",
            "verified": true,
        },
        "schema": REPORT_SCHEMA,
        "signing_dry_run": {
            "mode": "RELEASE_SIGN_DRY_RUN=1",
            "ok": true,
            "published": false,
            "target": "//deploy/release:signing_demo",
            "trust_root": SIGNING_TRUST_ROOT,
        },
        "verify_refusal": {
            "checksum_only_refused": true,
            "installed": false,
            "trust_root": SIGNING_TRUST_ROOT,
            "verifier": "//deploy/install:dx_verify",
        },
        "would_publish": [
            "Bazel Central Registry `rules_dx` module (owner-gated via //deploy/release:bcr_demo; owner approval required)",
            "GitHub Release with standalone `dx` binaries (draft only via //cli/cli:github_draft; owner approval required)",
        ],
    }));
    if report.is_empty() {
        return Err("publish dry-run: rendered report is empty".to_owned());
    }
    Ok(PublishDryRunOutcome {
        report,
        binary_digest,
        archive_digest,
        sbom_digest,
    })
}

pub fn sha256sum_line(digest: &str, name: &str) -> String {
    format!("{digest}  {name}\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch() -> tempfile::TempDir {
        tempfile::TempDir::new().expect("scratch")
    }

    fn write_bytes(dir: &Path, name: &str, bytes: &[u8]) -> PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, bytes).expect("write fixture");
        path
    }

    fn write_text(dir: &Path, name: &str, text: &str) -> PathBuf {
        write_bytes(dir, name, text.as_bytes())
    }

    fn notice_fixture(dir: &Path) -> (PathBuf, Vec<PathBuf>) {
        let manifest = write_text(
            dir,
            "inventory.txt",
            "demo-lib-b|npm|2.3.4|Apache-2.0|demo-lib-b.txt\ndemo-lib-a|cargo|1.0.0|MIT|demo-lib-a.txt\n",
        );
        let a = write_text(
            dir,
            "demo-lib-a.txt",
            "Fixture words for demo-lib-a (MIT).\n",
        );
        let b = write_text(
            dir,
            "demo-lib-b.txt",
            "Fixture words for demo-lib-b (Apache-2.0).\n",
        );
        (manifest, vec![a, b])
    }

    fn valid_inputs(dir: &Path) -> PublishDryRunInputs {
        let binary = write_bytes(dir, "dx-linux-x86_64", b"seed binary\n");
        let archive = write_bytes(dir, "dx-standalone.tar.gz", b"seed archive\n");
        let sbom_artifact = write_bytes(dir, "demo.tar.gz", b"demo artifact\n");
        let digest = dx_digest::sha256_file_hex(&sbom_artifact).expect("digest demo");
        let spdx_text = crate::render_spdx("demo.tar.gz", &digest, "dx", "rules_dx");
        let spdx = write_text(dir, "sbom_demo.spdx.json", &spdx_text);
        let prov_text =
            crate::render_provenance("demo.tar.gz", &digest, crate::PROVENANCE_BUILDER_DRY_RUN);
        let provenance = write_text(dir, "sbom_demo.provenance.json", &prov_text);
        let (notice_manifest, notice_texts) = notice_fixture(dir);
        let joined = crate::join_notice_entries(
            &std::fs::read_to_string(&notice_manifest).expect("manifest"),
            &notice_texts,
        )
        .expect("join");
        let notice_text = crate::render_notice("//demo:root", &joined);
        let notice = write_text(dir, "notice_demo.NOTICE", &notice_text);
        let release_artifacts_list = write_text(
            dir,
            "release-artifacts.txt",
            "licenses.toml\nsbom_demo.spdx.json\nsbom_demo.provenance.json\nnotice_demo.NOTICE\n",
        );
        let draft_log = write_text(
            dir,
            "draft-dry-run.log",
            "v0.0.0-dryrun\n--draft --verify-tag\n",
        );
        let signing_log = write_text(
            dir,
            "signing-dry-run.log",
            "https://tuf-repo-cdn.sigstore.dev\ncosign sign-blob\nsbom_demo.spdx.json\nsbom_demo.provenance.json\nnotice_demo.NOTICE\n",
        );
        let bcr_log = write_text(dir, "bcr-dry-run.log", "would submit, submitting nothing\n");
        let human_run_log = write_text(
            dir,
            "human-run-dry-run.log",
            "dry run (RELEASE_DRY_RUN=1)\n",
        );
        let release_tests_log = write_text(dir, "release-tests.log", "9 tests pass\n");
        let module_bazel = write_text(
            dir,
            "MODULE.bazel",
            "name = \"rules_dx\"\nversion = \"0.0.0\"\n",
        );
        let verify_refusal_log = write_text(
            dir,
            "verify-refusal.log",
            "checksum-only verification is not publisher-identity proof\n",
        );
        PublishDryRunInputs {
            binary,
            archive,
            sbom_artifact,
            spdx,
            provenance,
            notice,
            notice_manifest,
            notice_texts,
            release_artifacts_list,
            draft_log,
            signing_log,
            bcr_log,
            human_run_log,
            release_tests_log,
            module_bazel,
            verify_refusal_log,
            approve: false,
        }
    }

    #[test]
    fn staged_bytes_verify_and_report_without_host_tools() {
        let dir = scratch();
        let inputs = valid_inputs(dir.path());
        let outcome = publish_dry_run_check(&inputs).expect("valid stage verifies");
        let binary_want = dx_digest::sha256_file_hex(&inputs.binary).expect("digest binary");
        let archive_want = dx_digest::sha256_file_hex(&inputs.archive).expect("digest archive");
        assert_eq!(outcome.binary_digest, binary_want);
        assert_eq!(outcome.archive_digest, archive_want);
        assert!(!outcome.sbom_digest.is_empty());
        let report: serde_json::Value =
            serde_json::from_str(&outcome.report).expect("report is JSON");
        assert_eq!(report["schema"], serde_json::json!(REPORT_SCHEMA));
        assert_eq!(report["approved"], serde_json::json!(false));
        assert_eq!(report["published"], serde_json::json!(false));
        assert_eq!(
            report["artifacts"][0]["sha256"],
            serde_json::json!(binary_want)
        );
        assert_eq!(
            report["artifacts"][1]["sha256"],
            serde_json::json!(archive_want)
        );
        assert_eq!(
            report["sbom"]["subject_digest"],
            serde_json::json!(outcome.sbom_digest)
        );
        assert_eq!(report["sbom"]["linkage_ok"], serde_json::json!(true));
        assert_eq!(report["notice"]["header_ok"], serde_json::json!(true));
        assert_eq!(report["packaging"]["ok"], serde_json::json!(true));
        assert_eq!(report["draft_dry_run"]["ok"], serde_json::json!(true));
        assert_eq!(report["signing_dry_run"]["ok"], serde_json::json!(true));
        assert!(outcome.report.ends_with('\n'));
    }

    #[test]
    fn approved_flag_is_recorded_without_publishing() {
        let dir = scratch();
        let mut inputs = valid_inputs(dir.path());
        inputs.approve = true;
        let outcome = publish_dry_run_check(&inputs).expect("approved stage verifies");
        let report: serde_json::Value =
            serde_json::from_str(&outcome.report).expect("report is JSON");
        assert_eq!(report["approved"], serde_json::json!(true));
        assert_eq!(report["published"], serde_json::json!(false));
        assert_eq!(report["sbom"]["published"], serde_json::json!(false));
        assert_eq!(
            report["signing_dry_run"]["published"],
            serde_json::json!(false)
        );
    }

    #[test]
    fn missing_staged_binary_fails_before_any_digest() {
        let dir = scratch();
        let mut inputs = valid_inputs(dir.path());
        inputs.binary = dir.path().join("absent-binary");
        let error = publish_dry_run_check(&inputs).expect_err("missing binary must fail");
        assert!(error.contains("cannot hash staged binary"), "got {error}");
    }

    #[test]
    fn swapped_archive_breaks_its_reported_digest() {
        let dir = scratch();
        let inputs = valid_inputs(dir.path());
        let before = publish_dry_run_check(&inputs).expect("valid stage verifies");
        let swapped = write_bytes(dir.path(), "dx-standalone.tar.gz", b"tampered archive\n");
        assert_ne!(
            dx_digest::sha256_file_hex(&swapped).expect("digest swapped"),
            before.archive_digest
        );
    }

    #[test]
    fn mismatched_sbom_digest_fails_closed() {
        let dir = scratch();
        let mut inputs = valid_inputs(dir.path());
        let zeros = "0".repeat(64);
        let bad = crate::render_spdx("demo.tar.gz", &zeros, "dx", "rules_dx");
        inputs.spdx = write_text(dir.path(), "bad.spdx.json", &bad);
        let error = publish_dry_run_check(&inputs).expect_err("mismatched SBOM must fail");
        assert!(error.contains("SBOM validation failed"), "got {error}");
    }

    #[test]
    fn malformed_spdx_json_fails_actionable() {
        let dir = scratch();
        let mut inputs = valid_inputs(dir.path());
        inputs.spdx = write_text(dir.path(), "broken.spdx.json", "{not json");
        let error = publish_dry_run_check(&inputs).expect_err("malformed SPDX must fail");
        assert!(error.contains("SBOM validation failed"), "got {error}");
    }

    #[test]
    fn forged_provenance_builder_is_unapproved() {
        let dir = scratch();
        let mut inputs = valid_inputs(dir.path());
        let digest = dx_digest::sha256_file_hex(&inputs.sbom_artifact).expect("digest");
        let forged =
            crate::render_provenance("demo.tar.gz", &digest, "https://example.test/builder");
        inputs.provenance = write_text(dir.path(), "forged.prov.json", &forged);
        let error = publish_dry_run_check(&inputs).expect_err("forged builder must fail");
        assert!(error.contains("SBOM validation failed"), "got {error}");
    }

    #[test]
    fn redacted_notice_fails_closed() {
        let dir = scratch();
        let mut inputs = valid_inputs(dir.path());
        let text = std::fs::read_to_string(&inputs.notice).expect("read notice");
        let redacted = text.replace("Fixture words for demo-lib-a", "REDACTED");
        inputs.notice = write_text(dir.path(), "redacted.NOTICE", &redacted);
        let error = publish_dry_run_check(&inputs).expect_err("redacted NOTICE must fail");
        assert!(error.contains("NOTICE validation failed"), "got {error}");
    }

    #[test]
    fn release_list_missing_an_entry_fails() {
        let dir = scratch();
        let mut inputs = valid_inputs(dir.path());
        inputs.release_artifacts_list = write_text(
            dir.path(),
            "short.txt",
            "licenses.toml\nsbom_demo.spdx.json\n",
        );
        let error = publish_dry_run_check(&inputs).expect_err("short list must fail");
        assert!(error.contains("release artifacts list"), "got {error}");
    }

    #[test]
    fn draft_log_without_placeholder_fails() {
        let dir = scratch();
        let mut inputs = valid_inputs(dir.path());
        inputs.draft_log = write_text(dir.path(), "draft.log", "--draft --verify-tag\n");
        let error = publish_dry_run_check(&inputs).expect_err("draft without placeholder fails");
        assert!(error.contains("draft dry-run log"), "got {error}");
    }

    #[test]
    fn signing_log_without_cosign_fails() {
        let dir = scratch();
        let mut inputs = valid_inputs(dir.path());
        inputs.signing_log = write_text(
            dir.path(),
            "signing.log",
            "https://tuf-repo-cdn.sigstore.dev\nsbom_demo.spdx.json\nsbom_demo.provenance.json\nnotice_demo.NOTICE\n",
        );
        let error = publish_dry_run_check(&inputs).expect_err("signing without cosign fails");
        assert!(error.contains("signing dry-run log"), "got {error}");
    }

    #[test]
    fn sha256sum_lines_match_host_format() {
        assert_eq!(
            sha256sum_line("abc", "dx-linux-x86_64"),
            "abc  dx-linux-x86_64\n"
        );
    }
}
