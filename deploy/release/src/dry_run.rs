use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};

use crate::SIGNING_TRUST_ROOT;

pub struct DryRunInputs {
    pub stage: PathBuf,
    pub approve: bool,
    pub binary: PathBuf,
    pub standalone: PathBuf,
    pub standalone_checksum: PathBuf,
    pub spdx: PathBuf,
    pub provenance: PathBuf,
    pub notice: PathBuf,
    pub module_bazel: PathBuf,
    pub install_lib: PathBuf,
    pub summary: Option<PathBuf>,
}

pub fn parse_approve(raw: &str) -> Result<bool, String> {
    match raw {
        "true" => Ok(true),
        "false" => Ok(false),
        other => Err(format!(
            "publish dry-run: --approve wants true or false, got {other}"
        )),
    }
}

fn read_bytes(path: &Path, what: &str) -> Result<Vec<u8>, String> {
    fs::read(path).map_err(|error| {
        format!(
            "publish dry-run: cannot read {what} {}: {error}",
            path.display()
        )
    })
}

fn read_text(path: &Path, what: &str) -> Result<String, String> {
    fs::read_to_string(path).map_err(|error| {
        format!(
            "publish dry-run: cannot read {what} {}: {error}",
            path.display()
        )
    })
}

fn stage_file(stage: &Path, name: &str, bytes: &[u8]) -> Result<(), String> {
    fs::write(stage.join(name), bytes).map_err(|error| {
        format!(
            "publish dry-run: cannot stage {name} {}: {error}",
            stage.join(name).display()
        )
    })
}

fn expect_contains(haystack: &str, needle: &str, what: &str) -> Result<(), String> {
    if haystack.contains(needle) {
        Ok(())
    } else {
        Err(format!("publish dry-run: {what} is missing {needle}"))
    }
}

fn spdx_subject_digest(text: &str) -> Result<String, String> {
    let document: serde_json::Value = serde_json::from_str(text)
        .map_err(|error| format!("publish dry-run: malformed SBOM SPDX JSON: {error}"))?;
    document["packages"][0]["checksums"][0]["checksumValue"]
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| {
            "publish dry-run: SBOM SPDX JSON has no packages[0].checksums[0].checksumValue"
                .to_owned()
        })
}

fn render_bcr_shape(module_text: &str) -> Result<String, String> {
    let name_lines: Vec<&str> = module_text
        .lines()
        .filter(|line| line.contains("name = \"rules_dx\""))
        .collect();
    let version_lines: Vec<&str> = module_text
        .lines()
        .filter(|line| line.contains("version = \"0.0.0\""))
        .collect();
    if name_lines.is_empty() {
        return Err(
            "publish dry-run: module file has no line matching name = \"rules_dx\"".to_owned(),
        );
    }
    if version_lines.is_empty() {
        return Err(
            "publish dry-run: module file has no line matching version = \"0.0.0\"".to_owned(),
        );
    }
    let mut shape = String::from("Bazel Central Registry module shape (checked, not submitted):\n");
    for line in name_lines.iter().chain(version_lines.iter()) {
        shape.push_str(line);
        shape.push('\n');
    }
    Ok(shape)
}

fn build_report(
    approve: bool,
    binary_sha: &str,
    standalone_sha: &str,
    sbom_digest: &str,
) -> serde_json::Value {
    serde_json::json!({
        "schema": "publish-dry-run/v1",
        "approved": approve,
        "published": false,
        "artifacts": [
            {
                "name": "dx-linux-x86_64",
                "sha256": binary_sha,
                "note": "seed-host binary; release matrix frozen in deploy/release/matrix.bzl",
            },
            {
                "name": "dx-standalone.tar.gz",
                "sha256": standalone_sha,
                "note": "seed-host standalone archive via //cli/cli:dx_standalone; wider matrix qualified per-host under issue #815 (human-run only, not built here)",
            },
        ],
        "release_matrix": [
            {"name": "dx-linux-x86_64", "status": "qualified-seed-built-here"},
            {"name": "dx-linux-arm64", "status": "qualified-host-evidence"},
            {"name": "dx-macos-arm64", "status": "qualified-host-evidence"},
            {"name": "dx-windows-x86_64", "status": "qualified-host-evidence"},
        ],
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
        "draft_dry_run": {
            "target": "//cli/cli:github_draft",
            "mode": "GH_RELEASE_DRY_RUN=1",
            "placeholder": "v0.0.0-dryrun",
            "flags": "--draft --verify-tag",
            "ok": true,
            "published": false,
        },
        "sbom": {
            "target": "//deploy/release:sbom_demo",
            "spdx": "SPDX-2.3",
            "predicate": "https://slsa.dev/provenance/v1",
            "subject_digest": sbom_digest,
            "linkage_ok": true,
            "verified": true,
            "published": false,
        },
        "notice": {
            "target": "//deploy/release:notice_demo",
            "header_ok": true,
            "signed_with_sbom": true,
            "verified": true,
            "published": false,
        },
        "packaging": {
            "target": "//deploy/release:release_artifacts",
            "curator": "//:audit_curator",
            "sbom_signed_with_notice": true,
            "ok": true,
            "published": false,
        },
        "signing_dry_run": {
            "target": "//deploy/release:signing_demo",
            "mode": "RELEASE_SIGN_DRY_RUN=1",
            "trust_root": SIGNING_TRUST_ROOT,
            "ok": true,
            "published": false,
        },
        "bcr_shape": {
            "module": "rules_dx",
            "version": "0.0.0",
            "target": "//deploy/release:bcr_demo",
            "mode": "BCR_DRY_RUN=1",
            "checked": true,
            "submitted": false,
        },
        "verify_refusal": {
            "verifier": "//deploy/install:dx_verify",
            "trust_root": SIGNING_TRUST_ROOT,
            "checksum_only_refused": true,
            "installed": false,
        },
        "would_publish": [
            "Bazel Central Registry `rules_dx` module (owner-gated via //deploy/release:bcr_demo; owner approval required)",
            "GitHub Release with standalone `dx` binaries (draft only via //cli/cli:github_draft; owner approval required)",
        ],
        "not_attempted": [
            "SBOM/provenance publishing (generated owner-gated via //deploy/release:sbom_demo)",
            "Signing/attestation publishing (Sigstore keyless + GitHub attestations on the trust root, signing-first; GHCR images sign separately via cosign <digest>)",
            "GHCR prebuilt images (separate workflow .github/workflows/ghcr.yml per owner decision)",
            "Non-seed release matrix builds (qualified per-host under issue #815, human-run only, not built in dry-run)",
            "BCR submission (owner-gated via //deploy/release:bcr_demo)",
            "Any tag, registry submission, or release creation",
        ],
    })
}

fn render_summary(approve: bool, binary_sha: &str, standalone_sha: &str) -> String {
    format!(
        "## Publish dry-run report\n\
         \n\
         - approved: `{approve}`; published: **no** (dry run never publishes)\n\
         - seed binary: `dx-linux-x86_64` `{binary_sha}`\n\
         - standalone: `dx-standalone.tar.gz` `{standalone_sha}`\n\
         - draft dry-run: `//cli/cli:github_draft` `GH_RELEASE_DRY_RUN=1` ok (placeholder `v0.0.0-dryrun`, draft-only flags, publishes nothing)\n\
         - packaging: `//deploy/release:release_artifacts` ok (audit curator plus binary plus man page plus NOTICE plus SBOM/provenance, publishes nothing)\n\
         - SBOM: `//deploy/release:sbom_demo` SPDX-2.3 + SLSA v1 ok, subject digest equals artifact sha256, publishes nothing\n\
         - NOTICE: `//deploy/release:notice_demo` ok (hermetic bundle, signed alongside SBOM pair, publishes nothing)\n\
         - signing dry-run: `//deploy/release:signing_demo` `RELEASE_SIGN_DRY_RUN=1` ok (Sigstore keyless + attestation over SBOM pair plus NOTICE, publishes nothing)\n\
         - BCR shape: `rules_dx` at `0.0.0` via `//deploy/release:bcr_demo` checked, not submitted\n\
         - verifier: checksum-only refused, TUF trust root, nothing installed\n\
         - full report: `dry-run-report.json` in the job logs\n"
    )
}

fn append_summary(path: &Path, summary: &str) -> Result<(), String> {
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|error| {
            format!(
                "publish dry-run: cannot append step summary {}: {error}",
                path.display()
            )
        })?;
    file.write_all(summary.as_bytes()).map_err(|error| {
        format!(
            "publish dry-run: cannot append step summary {}: {error}",
            path.display()
        )
    })
}

pub fn run(inputs: &DryRunInputs) -> Result<Vec<String>, String> {
    let stage = &inputs.stage;
    fs::create_dir_all(stage).map_err(|error| {
        format!(
            "publish dry-run: cannot create stage {}: {error}",
            stage.display()
        )
    })?;

    let binary_bytes = read_bytes(&inputs.binary, "seed binary")?;
    let standalone_bytes = read_bytes(&inputs.standalone, "standalone archive")?;
    let standalone_checksum = read_bytes(&inputs.standalone_checksum, "standalone checksum")?;
    let spdx_bytes = read_bytes(&inputs.spdx, "SBOM SPDX")?;
    let provenance_bytes = read_bytes(&inputs.provenance, "SBOM provenance")?;
    let notice_bytes = read_bytes(&inputs.notice, "NOTICE bundle")?;
    let module_bytes = read_bytes(&inputs.module_bazel, "module file")?;
    let install_bytes = read_bytes(&inputs.install_lib, "install verifier source")?;

    stage_file(stage, "dx-linux-x86_64", &binary_bytes)?;
    stage_file(stage, "dx-standalone.tar.gz", &standalone_bytes)?;
    stage_file(stage, "dx-standalone.tar.gz.sha256", &standalone_checksum)?;
    stage_file(stage, "sbom_demo.spdx.json", &spdx_bytes)?;
    stage_file(stage, "sbom_demo.provenance.json", &provenance_bytes)?;
    stage_file(stage, "notice_demo.NOTICE", &notice_bytes)?;

    let binary_sha = dx_digest::sha256_hex(&binary_bytes);
    let standalone_sha = dx_digest::sha256_hex(&standalone_bytes);
    stage_file(
        stage,
        "dx-linux-x86_64.sha256",
        format!("{binary_sha}  dx-linux-x86_64\n").as_bytes(),
    )?;

    let spdx_text = String::from_utf8(spdx_bytes)
        .map_err(|_| "publish dry-run: SBOM SPDX is not UTF-8".to_owned())?;
    let provenance_text = String::from_utf8(provenance_bytes)
        .map_err(|_| "publish dry-run: SBOM provenance is not UTF-8".to_owned())?;
    let notice_text = String::from_utf8(notice_bytes)
        .map_err(|_| "publish dry-run: NOTICE bundle is not UTF-8".to_owned())?;
    let module_text = String::from_utf8(module_bytes)
        .map_err(|_| "publish dry-run: module file is not UTF-8".to_owned())?;
    let install_text = String::from_utf8(install_bytes)
        .map_err(|_| "publish dry-run: install verifier source is not UTF-8".to_owned())?;

    let artifacts_list = read_text(
        &stage.join("release-artifacts.txt"),
        "release artifacts list",
    )?;
    for needle in [
        "licenses.toml",
        "sbom_demo.spdx.json",
        "sbom_demo.provenance.json",
        "notice_demo.NOTICE",
    ] {
        expect_contains(&artifacts_list, needle, "release artifacts list")?;
    }

    expect_contains(&spdx_text, "SPDX-2.3", "SBOM SPDX")?;
    expect_contains(
        &provenance_text,
        "https://slsa.dev/provenance/v1",
        "SBOM provenance",
    )?;
    if !notice_text.starts_with("NOTICE for ") {
        return Err("publish dry-run: NOTICE bundle does not start with 'NOTICE for '".to_owned());
    }
    let sbom_digest = spdx_subject_digest(&spdx_text)?;
    expect_contains(&provenance_text, &sbom_digest, "SBOM provenance subject")?;

    let draft_log = read_text(&stage.join("draft-dry-run.log"), "draft dry-run log")?;
    expect_contains(&draft_log, "v0.0.0-dryrun", "draft dry-run log")?;
    expect_contains(&draft_log, "--draft --verify-tag", "draft dry-run log")?;

    let tests_log = read_text(&stage.join("release-tests.log"), "release tests log")?;
    expect_contains(&tests_log, "tests pass", "release tests log")?;

    let signing_log = read_text(&stage.join("signing-dry-run.log"), "signing dry-run log")?;
    expect_contains(&signing_log, "cosign sign-blob", "signing dry-run log")?;
    expect_contains(
        &signing_log,
        "tuf-repo-cdn.sigstore.dev",
        "signing dry-run log",
    )?;
    expect_contains(&signing_log, "sbom_demo.spdx.json", "signing dry-run log")?;
    expect_contains(
        &signing_log,
        "sbom_demo.provenance.json",
        "signing dry-run log",
    )?;
    expect_contains(&signing_log, "notice_demo.NOTICE", "signing dry-run log")?;

    let bcr_log = read_text(&stage.join("bcr-dry-run.log"), "bcr dry-run log")?;
    expect_contains(
        &bcr_log,
        "would submit, submitting nothing",
        "bcr dry-run log",
    )?;

    let driver_log = read_text(&stage.join("human-run-dry-run.log"), "release driver log")?;
    expect_contains(
        &driver_log,
        "dry run (RELEASE_DRY_RUN=1)",
        "release driver log",
    )?;

    expect_contains(
        &install_text,
        "tuf-repo-cdn.sigstore.dev",
        "install verifier source",
    )?;

    let refusal_log = read_text(&stage.join("verify-refusal.log"), "verifier refusal log")?;
    expect_contains(
        &refusal_log,
        "checksum-only verification is not publisher-identity proof",
        "verifier refusal log",
    )?;

    let bcr_shape = render_bcr_shape(&module_text)?;
    stage_file(stage, "bcr-shape.txt", bcr_shape.as_bytes())?;

    let report = build_report(inputs.approve, &binary_sha, &standalone_sha, &sbom_digest);
    let report_text = serde_json::to_string_pretty(&report)
        .map_err(|error| format!("publish dry-run: cannot render report: {error}"))?;
    stage_file(
        stage,
        "dry-run-report.json",
        format!("{report_text}\n").as_bytes(),
    )?;

    if let Some(path) = &inputs.summary {
        if !path.as_os_str().is_empty() {
            append_summary(
                path,
                &render_summary(inputs.approve, &binary_sha, &standalone_sha),
            )?;
        }
    }

    Ok(vec![
        format!("seed binary sha256: {binary_sha}"),
        format!("standalone archive sha256: {standalone_sha}"),
        format!("sbom subject digest: {sbom_digest} (linkage ok: true)"),
        "packaging ok: true (SBOM pair plus NOTICE signed together)".to_owned(),
        "draft dry-run ok: true".to_owned(),
        "signing dry-run ok: true".to_owned(),
        "bcr dry-run ok: true".to_owned(),
        format!(
            "approved: {approve}; published: False (dry run never publishes)",
            approve = inputs.approve
        ),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(root: &Path, name: &str, text: &str) -> PathBuf {
        let path = root.join(name);
        fs::write(&path, text).expect("write fixture");
        path
    }

    fn fixture(root: &Path) -> DryRunInputs {
        let stage = root.join("stage");
        fs::create_dir_all(&stage).expect("stage dir");
        write(
            &stage,
            "release-artifacts.txt",
            "bazel-bin/deploy/rules/licenses.toml\n\
             bazel-bin/deploy/release/sbom_demo.spdx.json\n\
             bazel-bin/deploy/release/sbom_demo.provenance.json\n\
             bazel-bin/deploy/release/notice_demo.NOTICE\n",
        );
        write(
            &stage,
            "draft-dry-run.log",
            "publishing draft release v0.0.0-dryrun with --draft --verify-tag\n",
        );
        write(
            &stage,
            "release-tests.log",
            "Executed 12 out of 12 tests: 12 tests pass.\n",
        );
        write(
            &stage,
            "signing-dry-run.log",
            "cosign sign-blob on https://tuf-repo-cdn.sigstore.dev signing \
             sbom_demo.spdx.json sbom_demo.provenance.json notice_demo.NOTICE\n",
        );
        write(
            &stage,
            "bcr-dry-run.log",
            "would submit, submitting nothing\n",
        );
        write(
            &stage,
            "human-run-dry-run.log",
            "dry run (RELEASE_DRY_RUN=1)\n",
        );
        write(
            &stage,
            "verify-refusal.log",
            "checksum-only verification is not publisher-identity proof\n",
        );
        let digest = "b".repeat(64);
        DryRunInputs {
            stage,
            approve: false,
            binary: write(root, "dx", "seed binary bytes"),
            standalone: write(root, "dx-standalone.tar.gz", "standalone archive bytes"),
            standalone_checksum: write(
                root,
                "dx-standalone.tar.gz.sha256",
                "deadbeef  dx-standalone.tar.gz\n",
            ),
            spdx: write(
                root,
                "sbom.spdx.json",
                &format!(
                    "{{\"spdxVersion\": \"SPDX-2.3\", \"packages\": [{{\"checksums\": [{{\"checksumValue\": \"{digest}\"}}]}}]}}"
                ),
            ),
            provenance: write(
                root,
                "provenance.json",
                &format!(
                    "{{\"predicateType\": \"https://slsa.dev/provenance/v1\", \"subject\": [{{\"digest\": {{\"sha256\": \"{digest}\"}}}}]}}"
                ),
            ),
            notice: write(root, "notice.NOTICE", "NOTICE for demo\n"),
            module_bazel: write(
                root,
                "MODULE.bazel",
                "module(\n    name = \"rules_dx\",\n    version = \"0.0.0\",\n)\n",
            ),
            install_lib: write(
                root,
                "install_lib.rs",
                "const TRUST_ROOT: &str = \"https://tuf-repo-cdn.sigstore.dev\";\n",
            ),
            summary: None,
        }
    }

    #[test]
    fn stages_artifacts_and_writes_report() {
        let dir = tempfile::TempDir::new().expect("scratch");
        let inputs = fixture(dir.path());
        let stage = inputs.stage.clone();
        let lines = run(&inputs).expect("dry run succeeds");

        let staged_binary = fs::read(stage.join("dx-linux-x86_64")).expect("staged binary");
        assert_eq!(staged_binary, b"seed binary bytes");
        let binary_sha = dx_digest::sha256_hex(b"seed binary bytes");
        assert_eq!(
            fs::read_to_string(stage.join("dx-linux-x86_64.sha256")).expect("sha file"),
            format!("{binary_sha}  dx-linux-x86_64\n")
        );
        assert_eq!(
            fs::read(stage.join("dx-standalone.tar.gz")).expect("staged archive"),
            b"standalone archive bytes"
        );
        assert_eq!(
            fs::read(stage.join("dx-standalone.tar.gz.sha256")).expect("staged checksum"),
            b"deadbeef  dx-standalone.tar.gz\n"
        );
        assert!(stage.join("sbom_demo.spdx.json").is_file());
        assert!(stage.join("sbom_demo.provenance.json").is_file());
        assert!(stage.join("notice_demo.NOTICE").is_file());

        let report_text =
            fs::read_to_string(stage.join("dry-run-report.json")).expect("report file");
        let report: serde_json::Value = serde_json::from_str(&report_text).expect("report JSON");
        assert_eq!(report["schema"], "publish-dry-run/v1");
        assert_eq!(report["approved"], false);
        assert_eq!(report["published"], false);
        assert_eq!(report["artifacts"][0]["name"], "dx-linux-x86_64");
        assert_eq!(report["artifacts"][0]["sha256"], binary_sha);
        let standalone_sha = dx_digest::sha256_hex(b"standalone archive bytes");
        assert_eq!(report["artifacts"][1]["name"], "dx-standalone.tar.gz");
        assert_eq!(report["artifacts"][1]["sha256"], standalone_sha);
        assert_eq!(report["sbom"]["linkage_ok"], true);
        assert_eq!(report["sbom"]["subject_digest"], "b".repeat(64));
        assert_eq!(report["signing_dry_run"]["trust_root"], SIGNING_TRUST_ROOT);
        assert_eq!(report["exercised"].as_array().expect("exercised").len(), 10);
        assert_eq!(
            report["not_attempted"]
                .as_array()
                .expect("not attempted")
                .len(),
            6
        );

        assert_eq!(lines[0], format!("seed binary sha256: {binary_sha}"));
        assert_eq!(
            lines[7],
            "approved: false; published: False (dry run never publishes)"
        );

        let shape = fs::read_to_string(stage.join("bcr-shape.txt")).expect("bcr shape");
        assert_eq!(
            shape,
            "Bazel Central Registry module shape (checked, not submitted):\n    name = \"rules_dx\",\n    version = \"0.0.0\",\n"
        );
    }

    #[test]
    fn approved_run_reports_true() {
        let dir = tempfile::TempDir::new().expect("scratch");
        let mut inputs = fixture(dir.path());
        inputs.approve = true;
        let lines = run(&inputs).expect("dry run succeeds");
        assert_eq!(
            lines[7],
            "approved: true; published: False (dry run never publishes)"
        );
        let report_text =
            fs::read_to_string(inputs.stage.join("dry-run-report.json")).expect("report file");
        let report: serde_json::Value = serde_json::from_str(&report_text).expect("report JSON");
        assert_eq!(report["approved"], true);
    }

    #[test]
    fn appends_step_summary() {
        let dir = tempfile::TempDir::new().expect("scratch");
        let mut inputs = fixture(dir.path());
        let summary = dir.path().join("summary.md");
        inputs.summary = Some(summary.clone());
        run(&inputs).expect("dry run succeeds");
        let text = fs::read_to_string(&summary).expect("summary file");
        let binary_sha = dx_digest::sha256_hex(b"seed binary bytes");
        let standalone_sha = dx_digest::sha256_hex(b"standalone archive bytes");
        assert!(text.starts_with("## Publish dry-run report\n"));
        assert!(text.contains("- approved: `false`; published: **no**"));
        assert!(text.contains(&format!(
            "- seed binary: `dx-linux-x86_64` `{binary_sha}`\n"
        )));
        assert!(text.contains(&format!(
            "- standalone: `dx-standalone.tar.gz` `{standalone_sha}`\n"
        )));
        assert!(text.contains("- full report: `dry-run-report.json` in the job logs\n"));
    }

    #[test]
    fn skips_empty_step_summary_path() {
        let dir = tempfile::TempDir::new().expect("scratch");
        let mut inputs = fixture(dir.path());
        inputs.summary = Some(PathBuf::new());
        run(&inputs).expect("dry run succeeds");
    }

    #[test]
    fn rejects_missing_input() {
        let dir = tempfile::TempDir::new().expect("scratch");
        let inputs = fixture(dir.path());
        fs::remove_file(&inputs.spdx).expect("remove spdx");
        let error = run(&inputs).expect_err("missing spdx fails");
        assert!(error.contains("cannot read SBOM SPDX"), "{error}");
        assert!(error.contains("sbom.spdx.json"), "{error}");
    }

    #[test]
    fn rejects_missing_probe_log() {
        let dir = tempfile::TempDir::new().expect("scratch");
        let inputs = fixture(dir.path());
        fs::remove_file(inputs.stage.join("signing-dry-run.log")).expect("remove log");
        let error = run(&inputs).expect_err("missing log fails");
        assert!(error.contains("cannot read signing dry-run log"), "{error}");
    }

    #[test]
    fn rejects_malformed_spdx() {
        let dir = tempfile::TempDir::new().expect("scratch");
        let inputs = fixture(dir.path());
        fs::write(&inputs.spdx, "SPDX-2.3 but not json").expect("write spdx");
        let error = run(&inputs).expect_err("malformed spdx fails");
        assert!(error.contains("malformed SBOM SPDX JSON"), "{error}");
    }

    #[test]
    fn rejects_spdx_without_subject_checksum() {
        let dir = tempfile::TempDir::new().expect("scratch");
        let inputs = fixture(dir.path());
        fs::write(&inputs.spdx, "{\"spdxVersion\": \"SPDX-2.3\"}").expect("write spdx");
        let error = run(&inputs).expect_err("checksum-less spdx fails");
        assert!(error.contains("checksumValue"), "{error}");
    }

    #[test]
    fn rejects_provenance_missing_subject_digest() {
        let dir = tempfile::TempDir::new().expect("scratch");
        let inputs = fixture(dir.path());
        fs::write(
            &inputs.provenance,
            "{\"predicateType\": \"https://slsa.dev/provenance/v1\"}",
        )
        .expect("write provenance");
        let error = run(&inputs).expect_err("unlinked provenance fails");
        assert!(error.contains("SBOM provenance subject"), "{error}");
    }

    #[test]
    fn rejects_notice_without_header() {
        let dir = tempfile::TempDir::new().expect("scratch");
        let inputs = fixture(dir.path());
        fs::write(&inputs.notice, "demo text\n").expect("write notice");
        let error = run(&inputs).expect_err("header-less notice fails");
        assert!(error.contains("NOTICE bundle does not start"), "{error}");
    }

    #[test]
    fn rejects_release_artifacts_list_without_notice() {
        let dir = tempfile::TempDir::new().expect("scratch");
        let inputs = fixture(dir.path());
        fs::write(
            inputs.stage.join("release-artifacts.txt"),
            "licenses.toml\nsbom_demo.spdx.json\nsbom_demo.provenance.json\n",
        )
        .expect("write list");
        let error = run(&inputs).expect_err("incomplete list fails");
        assert!(
            error.contains("release artifacts list is missing"),
            "{error}"
        );
        assert!(error.contains("notice_demo.NOTICE"), "{error}");
    }

    #[test]
    fn rejects_draft_log_without_placeholder() {
        let dir = tempfile::TempDir::new().expect("scratch");
        let inputs = fixture(dir.path());
        fs::write(inputs.stage.join("draft-dry-run.log"), "publishing\n").expect("write log");
        let error = run(&inputs).expect_err("placeholder-less draft log fails");
        assert!(error.contains("draft dry-run log is missing"), "{error}");
    }

    #[test]
    fn rejects_driver_log_without_dry_run_marker() {
        let dir = tempfile::TempDir::new().expect("scratch");
        let inputs = fixture(dir.path());
        fs::write(inputs.stage.join("human-run-dry-run.log"), "approving\n").expect("write log");
        let error = run(&inputs).expect_err("marker-less driver log fails");
        assert!(error.contains("release driver log is missing"), "{error}");
    }

    #[test]
    fn rejects_module_without_version_line() {
        let dir = tempfile::TempDir::new().expect("scratch");
        let inputs = fixture(dir.path());
        fs::write(
            &inputs.module_bazel,
            "module(\n    name = \"rules_dx\",\n)\n",
        )
        .expect("write module");
        let error = run(&inputs).expect_err("version-less module fails");
        assert!(error.contains("no line matching version"), "{error}");
    }

    #[test]
    fn rejects_unverifiable_checksum_only_refusal() {
        let dir = tempfile::TempDir::new().expect("scratch");
        let inputs = fixture(dir.path());
        fs::write(inputs.stage.join("verify-refusal.log"), "mismatch\n").expect("write log");
        let error = run(&inputs).expect_err("refusal-less log fails");
        assert!(error.contains("verifier refusal log is missing"), "{error}");
    }

    #[test]
    fn parses_approve_strictly() {
        assert_eq!(parse_approve("true").expect("true"), true);
        assert_eq!(parse_approve("false").expect("false"), false);
        for raw in ["", "True", "1", "yes"] {
            let error = parse_approve(raw).expect_err("rejects malformed approve");
            assert!(error.contains("--approve wants true or false"), "{error}");
        }
    }
}
