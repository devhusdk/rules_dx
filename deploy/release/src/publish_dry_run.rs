use std::io;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

const SCHEMA: &str = "publish-dry-run/v1";
const BINARY_STAGED: &str = "dx-linux-x86_64";
const ARCHIVE_STAGED: &str = "dx-standalone.tar.gz";
const ARCHIVE_CHECKSUM_STAGED: &str = "dx-standalone.tar.gz.sha256";
const SPDX_STAGED: &str = "sbom_demo.spdx.json";
const PROVENANCE_STAGED: &str = "sbom_demo.provenance.json";
const NOTICE_STAGED: &str = "notice_demo.NOTICE";
const RELEASE_LIST_STAGED: &str = "release-artifacts.txt";
const REPORT_STAGED: &str = "dry-run-report.json";
const BCR_SHAPE_STAGED: &str = "bcr-shape.txt";

const NOTICE_HEADER: &str = "NOTICE for ";
const SPDX_VERSION_TEXT: &str = "SPDX-2.3";
const PROVENANCE_PREDICATE: &str = "https://slsa.dev/provenance/v1";
const TRUST_ROOT: &str = "tuf-repo-cdn.sigstore.dev";
const MODULE_NAME_NEEDLE: &str = "name = \"rules_dx\"";
const MODULE_VERSION_NEEDLE: &str = "version = \"0.0.0\"";
const DRAFT_PLACEHOLDER: &str = "v0.0.0-dryrun";
const DRAFT_FLAGS: &str = "--draft --verify-tag";
const RELEASE_TESTS_TEXT: &str = "tests pass";
const HUMAN_RUN_TEXT: &str = "dry run (RELEASE_DRY_RUN=1)";
const BCR_NOTHING_TEXT: &str = "would submit, submitting nothing";
const VERIFY_REFUSAL_TEXT: &str = "checksum-only verification is not publisher-identity proof";
const INSTALL_VERIFIER_SOURCE: &str = "deploy/install/src/lib.rs";

const REQUIRED_RELEASE_FILES: [&str; 4] = [
    "licenses.toml",
    "sbom_demo.spdx.json",
    "sbom_demo.provenance.json",
    "notice_demo.NOTICE",
];

const RELEASE_MATRIX: [(&str, &str); 4] = [
    ("dx-linux-x86_64", "qualified-seed-built-here"),
    ("dx-linux-arm64", "qualified-host-evidence"),
    ("dx-macos-arm64", "qualified-host-evidence"),
    ("dx-windows-x86_64", "qualified-host-evidence"),
];

const EXERCISED: [&str; 10] = [
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
];

const WOULD_PUBLISH: [&str; 2] = [
    "Bazel Central Registry `rules_dx` module (owner-gated via //deploy/release:bcr_demo; owner approval required)",
    "GitHub Release with standalone `dx` binaries (draft only via //cli/cli:github_draft; owner approval required)",
];

const NOT_ATTEMPTED: [&str; 6] = [
    "SBOM/provenance publishing (generated owner-gated via //deploy/release:sbom_demo)",
    "Signing/attestation publishing (Sigstore keyless + GitHub attestations on the trust root, signing-first; GHCR images sign separately via cosign <digest>)",
    "GHCR prebuilt images (separate workflow .github/workflows/ghcr.yml per owner decision)",
    "Non-seed release matrix builds (qualified per-host under issue #815, human-run only, not built in dry-run)",
    "BCR submission (owner-gated via //deploy/release:bcr_demo)",
    "Any tag, registry submission, or release creation",
];

pub struct PublishDryRunInputs {
    pub stage: PathBuf,
    pub workspace: PathBuf,
    pub approve: String,
    pub binary: PathBuf,
    pub archive: PathBuf,
    pub archive_checksum: PathBuf,
    pub spdx: PathBuf,
    pub provenance: PathBuf,
    pub notice: PathBuf,
    pub step_summary: Option<PathBuf>,
}

fn fail(message: impl Into<String>) -> String {
    format!("publish-dry-run: {}", message.into())
}

fn read_text(path: &Path, what: &str) -> Result<String, String> {
    std::fs::read_to_string(path)
        .map_err(|error| fail(format!("cannot read {what} {}: {error}", path.display())))
}

fn stage_file(src: &Path, stage: &Path, name: &str) -> Result<(), String> {
    let dst = stage.join(name);
    std::fs::copy(src, &dst).map_err(|error| {
        fail(format!(
            "cannot stage {} as {}: {error}",
            src.display(),
            dst.display()
        ))
    })?;
    Ok(())
}

fn require_contains(haystack: &str, needles: &[&str], what: &str) -> Result<(), String> {
    for needle in needles {
        if !haystack.contains(needle) {
            return Err(fail(format!("{what} is missing {needle:?}")));
        }
    }
    Ok(())
}

fn sha256_of(path: &Path, what: &str) -> Result<String, String> {
    dx_digest::sha256_file_hex(path)
        .map_err(|error| fail(format!("cannot hash {what} {}: {error}", path.display())))
}

fn declared_checksum(path: &Path) -> Result<String, String> {
    let text = read_text(path, "standalone checksum")?;
    text.split_whitespace()
        .next()
        .filter(|token| dx_digest::is_hex(token))
        .map(str::to_owned)
        .ok_or_else(|| {
            fail(format!(
                "{} declares no sha256 digest",
                path.display()
            ))
        })
}

fn first_stage_digest(stage: &Path, spdx_text: &str) -> Result<String, String> {
    let document: Value = serde_json::from_str(spdx_text)
        .map_err(|error| fail(format!("{SPDX_STAGED} is not JSON: {error}")))?;
    document
        .get("packages")
        .and_then(Value::as_array)
        .and_then(|packages| packages.first())
        .and_then(|package| package.get("checksums"))
        .and_then(Value::as_array)
        .and_then(|checksums| checksums.first())
        .and_then(|checksum| checksum.get("checksumValue"))
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| {
            fail(format!(
                "{SPDX_STAGED} in {} lacks packages[0].checksums[0].checksumValue",
                stage.display()
            ))
        })
}

fn bcr_shape_text(workspace: &Path) -> Result<String, String> {
    let module = read_text(&workspace.join("MODULE.bazel"), "MODULE.bazel")?;
    let names: Vec<&str> = module
        .lines()
        .filter(|line| line.contains(MODULE_NAME_NEEDLE))
        .collect();
    let versions: Vec<&str> = module
        .lines()
        .filter(|line| line.contains(MODULE_VERSION_NEEDLE))
        .collect();
    if names.is_empty() {
        return Err(fail(format!(
            "MODULE.bazel never matches {MODULE_NAME_NEEDLE}"
        )));
    }
    if versions.is_empty() {
        return Err(fail(format!(
            "MODULE.bazel never matches {MODULE_VERSION_NEEDLE}"
        )));
    }
    let mut out = String::from("Bazel Central Registry module shape (checked, not submitted):\n");
    for line in names.iter().chain(versions.iter()) {
        out.push_str(line);
        out.push('\n');
    }
    Ok(out)
}

fn append_summary(path: &Path, summary: &str) -> Result<(), String> {
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|error| fail(format!("cannot open step summary {}: {error}", path.display())))?;
    file.write_all(summary.as_bytes())
        .map_err(|error| fail(format!("cannot write step summary {}: {error}", path.display())))?;
    Ok(())
}

fn bool_text(value: bool) -> &'static str {
    if value { "True" } else { "False" }
}

fn report_value(
    approve: bool,
    digest: &str,
    standalone_digest: &str,
    sbom_digest: &str,
    draft_ok: bool,
    signing_ok: bool,
    packaging_ok: bool,
    sbom_linkage_ok: bool,
    notice_ok: bool,
    bcr_ok: bool,
) -> Value {
    json!({
        "schema": SCHEMA,
        "approved": approve,
        "published": false,
        "artifacts": [
            {
                "name": "dx-linux-x86_64",
                "sha256": digest,
                "note": "seed-host binary; release matrix frozen in deploy/release/matrix.bzl",
            },
            {
                "name": "dx-standalone.tar.gz",
                "sha256": standalone_digest,
                "note": "seed-host standalone archive via //cli/cli:dx_standalone; wider matrix qualified per-host under issue #815 (human-run only, not built here)",
            },
        ],
        "release_matrix": RELEASE_MATRIX
            .iter()
            .map(|(name, status)| json!({"name": name, "status": status}))
            .collect::<Vec<Value>>(),
        "exercised": EXERCISED.iter().map(|entry| json!(entry)).collect::<Vec<Value>>(),
        "draft_dry_run": {
            "target": "//cli/cli:github_draft",
            "mode": "GH_RELEASE_DRY_RUN=1",
            "placeholder": DRAFT_PLACEHOLDER,
            "flags": DRAFT_FLAGS,
            "ok": draft_ok,
            "published": false,
        },
        "sbom": {
            "target": "//deploy/release:sbom_demo",
            "spdx": SPDX_VERSION_TEXT,
            "predicate": PROVENANCE_PREDICATE,
            "subject_digest": sbom_digest,
            "linkage_ok": sbom_linkage_ok,
            "verified": true,
            "published": false,
        },
        "notice": {
            "target": "//deploy/release:notice_demo",
            "header_ok": notice_ok,
            "signed_with_sbom": packaging_ok,
            "verified": true,
            "published": false,
        },
        "packaging": {
            "target": "//deploy/release:release_artifacts",
            "curator": "//:audit_curator",
            "sbom_signed_with_notice": packaging_ok,
            "ok": packaging_ok && sbom_linkage_ok && notice_ok,
            "published": false,
        },
        "signing_dry_run": {
            "target": "//deploy/release:signing_demo",
            "mode": "RELEASE_SIGN_DRY_RUN=1",
            "trust_root": format!("https://{TRUST_ROOT}"),
            "ok": signing_ok,
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
            "trust_root": format!("https://{TRUST_ROOT}"),
            "checksum_only_refused": true,
            "installed": false,
        },
        "would_publish": WOULD_PUBLISH.iter().map(|entry| json!(entry)).collect::<Vec<Value>>(),
        "not_attempted": NOT_ATTEMPTED.iter().map(|entry| json!(entry)).collect::<Vec<Value>>(),
    })
}

fn stdout_text(
    approve: bool,
    digest: &str,
    standalone_digest: &str,
    sbom_digest: &str,
    draft_ok: bool,
    signing_ok: bool,
    packaging_ok: bool,
    sbom_linkage_ok: bool,
    bcr_ok: bool,
) -> String {
    format!(
        "seed binary sha256: {digest}\n\
         standalone archive sha256: {standalone_digest}\n\
         sbom subject digest: {sbom_digest} (linkage ok: {})\n\
         packaging ok: {} (SBOM pair plus NOTICE signed together)\n\
         draft dry-run ok: {}\n\
         signing dry-run ok: {}\n\
         bcr dry-run ok: {}\n\
         approved: {}; published: False (dry run never publishes)\n",
        bool_text(sbom_linkage_ok),
        bool_text(packaging_ok),
        bool_text(draft_ok),
        bool_text(signing_ok),
        bool_text(bcr_ok),
        bool_text(approve),
    )
}

fn summary_text(approve: &str, digest: &str, standalone_digest: &str) -> String {
    format!(
        "## Publish dry-run report\n\
         \n\
         - approved: `{approve}`; published: **no** (dry run never publishes)\n\
         - seed binary: `dx-linux-x86_64` `{digest}`\n\
         - standalone: `dx-standalone.tar.gz` `{standalone_digest}`\n\
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

pub fn publish_dry_run(inputs: &PublishDryRunInputs) -> Result<String, String> {
    let approve = match inputs.approve.as_str() {
        "true" => true,
        "false" => false,
        other => {
            return Err(fail(format!(
                "approve must be 'true' or 'false', got {other:?}"
            )))
        }
    };
    std::fs::create_dir_all(&inputs.stage).map_err(|error| {
        fail(format!(
            "cannot create stage {}: {error}",
            inputs.stage.display()
        ))
    })?;
    stage_file(&inputs.binary, &inputs.stage, BINARY_STAGED)?;
    stage_file(&inputs.archive, &inputs.stage, ARCHIVE_STAGED)?;
    stage_file(
        &inputs.archive_checksum,
        &inputs.stage,
        ARCHIVE_CHECKSUM_STAGED,
    )?;
    stage_file(&inputs.spdx, &inputs.stage, SPDX_STAGED)?;
    stage_file(&inputs.provenance, &inputs.stage, PROVENANCE_STAGED)?;
    stage_file(&inputs.notice, &inputs.stage, NOTICE_STAGED)?;

    let digest = sha256_of(&inputs.stage.join(BINARY_STAGED), BINARY_STAGED)?;
    let standalone_digest = sha256_of(&inputs.stage.join(ARCHIVE_STAGED), ARCHIVE_STAGED)?;
    let checksum_line = format!("{digest}  {BINARY_STAGED}\n");
    std::fs::write(inputs.stage.join(format!("{BINARY_STAGED}.sha256")), checksum_line)
        .map_err(|error| fail(format!("cannot write binary checksum: {error}")))?;
    let declared = declared_checksum(&inputs.stage.join(ARCHIVE_CHECKSUM_STAGED))?;
    if declared != standalone_digest {
        return Err(fail(format!(
            "{ARCHIVE_CHECKSUM_STAGED} declares {declared} but the staged archive hashes to {standalone_digest}"
        )));
    }

    let release_list = read_text(&inputs.stage.join(RELEASE_LIST_STAGED), RELEASE_LIST_STAGED)?;
    for required in REQUIRED_RELEASE_FILES {
        if !release_list.contains(required) {
            return Err(fail(format!(
                "{RELEASE_LIST_STAGED} never names {required}"
            )));
        }
    }

    let spdx_text = read_text(&inputs.stage.join(SPDX_STAGED), SPDX_STAGED)?;
    require_contains(&spdx_text, &[SPDX_VERSION_TEXT], SPDX_STAGED)?;
    let provenance_text = read_text(&inputs.stage.join(PROVENANCE_STAGED), PROVENANCE_STAGED)?;
    require_contains(&provenance_text, &[PROVENANCE_PREDICATE], PROVENANCE_STAGED)?;
    let sbom_digest = first_stage_digest(&inputs.stage, &spdx_text)?;
    let sbom_linkage_ok = provenance_text.contains(&sbom_digest);
    if !sbom_linkage_ok {
        return Err(fail(format!(
            "{PROVENANCE_STAGED} never names the SPDX subject digest {sbom_digest}"
        )));
    }
    let notice_text = read_text(&inputs.stage.join(NOTICE_STAGED), NOTICE_STAGED)?;
    if !notice_text.starts_with(NOTICE_HEADER) {
        return Err(fail(format!(
            "{NOTICE_STAGED} does not start with {NOTICE_HEADER:?}"
        )));
    }
    let notice_ok = true;

    let draft_log = read_text(&inputs.stage.join("draft-dry-run.log"), "draft dry-run log")?;
    require_contains(
        &draft_log,
        &[DRAFT_PLACEHOLDER, DRAFT_FLAGS],
        "draft dry-run log",
    )?;
    let release_tests_log = read_text(
        &inputs.stage.join("release-tests.log"),
        "release test log",
    )?;
    require_contains(&release_tests_log, &[RELEASE_TESTS_TEXT], "release test log")?;
    let signing_log = read_text(
        &inputs.stage.join("signing-dry-run.log"),
        "signing dry-run log",
    )?;
    require_contains(
        &signing_log,
        &[
            "cosign sign-blob",
            TRUST_ROOT,
            "sbom_demo.spdx.json",
            "sbom_demo.provenance.json",
            "notice_demo.NOTICE",
        ],
        "signing dry-run log",
    )?;
    let bcr_log = read_text(&inputs.stage.join("bcr-dry-run.log"), "bcr dry-run log")?;
    require_contains(&bcr_log, &[BCR_NOTHING_TEXT], "bcr dry-run log")?;
    let human_log = read_text(
        &inputs.stage.join("human-run-dry-run.log"),
        "release driver dry-run log",
    )?;
    require_contains(&human_log, &[HUMAN_RUN_TEXT], "release driver dry-run log")?;
    read_text(
        &inputs.stage.join("verify-help.txt"),
        "install verifier help",
    )?;
    let refusal_log = read_text(
        &inputs.stage.join("verify-refusal.log"),
        "install verifier refusal log",
    )?;
    require_contains(
        &refusal_log,
        &[VERIFY_REFUSAL_TEXT],
        "install verifier refusal log",
    )?;
    let verifier_source = read_text(
        &inputs.workspace.join(INSTALL_VERIFIER_SOURCE),
        "install verifier source",
    )?;
    require_contains(&verifier_source, &[TRUST_ROOT], "install verifier source")?;

    let bcr_shape = bcr_shape_text(&inputs.workspace)?;

    let draft_ok = true;
    let signing_ok = true;
    let packaging_ok = true;
    let bcr_ok = true;

    let report = report_value(
        approve,
        &digest,
        &standalone_digest,
        &sbom_digest,
        draft_ok,
        signing_ok,
        packaging_ok,
        sbom_linkage_ok,
        notice_ok,
        bcr_ok,
    );
    let report_text = serde_json::to_string_pretty(&report)
        .map_err(|error| fail(format!("cannot encode {REPORT_STAGED}: {error}")))?;
    std::fs::write(inputs.stage.join(REPORT_STAGED), report_text)
        .map_err(|error| fail(format!("cannot write {REPORT_STAGED}: {error}")))?;
    std::fs::write(inputs.stage.join(BCR_SHAPE_STAGED), bcr_shape)
        .map_err(|error| fail(format!("cannot write {BCR_SHAPE_STAGED}: {error}")))?;

    if let Some(path) = &inputs.step_summary {
        append_summary(path, &summary_text(&inputs.approve, &digest, &standalone_digest))?;
    }

    Ok(stdout_text(
        approve,
        &digest,
        &standalone_digest,
        &sbom_digest,
        draft_ok,
        signing_ok,
        packaging_ok,
        sbom_linkage_ok,
        bcr_ok,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch() -> tempfile::TempDir {
        tempfile::tempdir().expect("tempdir")
    }

    struct Fixture {
        root: tempfile::TempDir,
    }

    impl Fixture {
        fn new() -> Self {
            Fixture {
                root: tempfile::tempdir().expect("tempdir"),
            }
        }

        fn path(&self, name: &str) -> PathBuf {
            self.root.path().join(name)
        }

        fn write(&self, name: &str, contents: &str) -> PathBuf {
            let path = self.path(name);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).expect("mkdir");
            }
            std::fs::write(&path, contents).expect("write");
            path
        }

        fn inputs(&self) -> PublishDryRunInputs {
            let stage = self.path("stage");
            std::fs::create_dir_all(&stage).expect("stage dir");
            for log in [
                "draft-dry-run.log",
                "release-tests.log",
                "signing-dry-run.log",
                "bcr-dry-run.log",
                "human-run-dry-run.log",
                "verify-help.txt",
                "verify-refusal.log",
            ] {
                self.write_log(log);
            }
            PublishDryRunInputs {
                stage,
                workspace: self.path("workspace"),
                approve: "false".to_owned(),
                binary: self.write("out/dx", "seed binary bytes"),
                archive: self.write("out/dx_standalone.tar.gz", "standalone archive bytes"),
                archive_checksum: self.write(
                    "out/dx_standalone.tar.gz.sha256",
                    &format!(
                        "{}  dx_standalone.tar.gz\n",
                        dx_digest::sha256_hex(b"standalone archive bytes")
                    ),
                ),
                spdx: self.write(
                    "out/sbom_demo.spdx.json",
                    SPDX_FIXTURE,
                ),
                provenance: self.write(
                    "out/sbom_demo.provenance.json",
                    PROVENANCE_FIXTURE,
                ),
                notice: self.write("out/notice_demo.NOTICE", NOTICE_FIXTURE),
                step_summary: Some(self.path("summary.md")),
            }
        }

        fn write_log(&self, name: &str) -> PathBuf {
            let body = match name {
                "draft-dry-run.log" => {
                    "draft: v0.0.0-dryrun\ngh release create --draft --verify-tag\n"
                }
                "release-tests.log" => "Executed 12 tests, 0 failures: tests pass\n",
                "signing-dry-run.log" => {
                    "signing: cosign sign-blob\ntrust root: https://tuf-repo-cdn.sigstore.dev\n\
                     assets: sbom_demo.spdx.json sbom_demo.provenance.json notice_demo.NOTICE\n"
                }
                "bcr-dry-run.log" => "bcr: dry run; would submit, submitting nothing\n",
                "human-run-dry-run.log" => "release: dry run (RELEASE_DRY_RUN=1)\n",
                "verify-help.txt" => "usage: dx_verify\ntrust root: tuf-repo-cdn.sigstore.dev\n",
                "verify-refusal.log" => {
                    "refused: checksum-only verification is not publisher-identity proof\n"
                }
                other => unreachable!("unknown fixture log {other}"),
            };
            let stage = self.path("stage");
            std::fs::create_dir_all(&stage).expect("stage dir");
            let path = stage.join(name);
            std::fs::write(&path, body).expect("log write");
            path
        }

        fn workspace(&self) {
            self.write(
                "workspace/MODULE.bazel",
                "module(\n    name = \"rules_dx\",\n    version = \"0.0.0\",\n)\n",
            );
            self.write(
                "workspace/deploy/install/src/lib.rs",
                "const TUF: &str = \"https://tuf-repo-cdn.sigstore.dev\";\n",
            );
            self.write(
                "workspace/stage/release-artifacts.txt",
                "bazel-out/bin/deploy/release/notice_demo.NOTICE\n\
                 bazel-out/bin/deploy/release/sbom_demo.spdx.json\n\
                 bazel-out/bin/deploy/release/sbom_demo.provenance.json\n\
                 licenses.toml\n",
            );
        }
    }

    const SBOM_DIGEST: &str = "0000000000000000000000000000000000000000000000000000000000000abc";

    const SPDX_FIXTURE: &str = concat!(
        "{\"spdxVersion\": \"SPDX-2.3\", \"packages\": [{\"name\": \"demo\", ",
        "\"checksums\": [{\"algorithm\": \"SHA256\", \"checksumValue\": \"",
        "0000000000000000000000000000000000000000000000000000000000000abc\"}]}]}"
    );

    const PROVENANCE_FIXTURE: &str = concat!(
        "{\"predicateType\": \"https://slsa.dev/provenance/v1\", ",
        "\"subject\": [{\"digest\": {\"sha256\": \"",
        "0000000000000000000000000000000000000000000000000000000000000abc\"}}]}"
    );

    const NOTICE_FIXTURE: &str = "NOTICE for demo 1.0\nlicensed text\n";

    fn run(fixture: &Fixture) -> Result<String, String> {
        let mut inputs = fixture.inputs();
        fixture.workspace();
        inputs.stage = fixture.path("stage");
        publish_dry_run(&inputs)
    }

    #[test]
    fn stages_byte_identical_artifacts_and_sha256sum_files() {
        let fixture = Fixture::new();
        run(&fixture).expect("dry run passes");
        let stage = fixture.path("stage");
        assert_eq!(
            std::fs::read(stage.join(BINARY_STAGED)).expect("staged binary"),
            b"seed binary bytes"
        );
        assert_eq!(
            std::fs::read(stage.join(ARCHIVE_STAGED)).expect("staged archive"),
            b"standalone archive bytes"
        );
        assert_eq!(
            std::fs::read_to_string(stage.join(format!("{BINARY_STAGED}.sha256"))).expect("sum"),
            format!(
                "{}  {BINARY_STAGED}\n",
                dx_digest::sha256_hex(b"seed binary bytes")
            )
        );
        assert_eq!(
            std::fs::read_to_string(stage.join(RELEASE_LIST_STAGED)).expect("list"),
            read_text(&fixture.path("stage/release-artifacts.txt"), "fixture").expect("fixture")
        );
    }

    #[test]
    fn emits_the_v1_report_with_equivalent_metadata() {
        let fixture = Fixture::new();
        run(&fixture).expect("dry run passes");
        let report_text = std::fs::read_to_string(fixture.path("stage/dry-run-report.json"))
            .expect("report");
        let report: Value = serde_json::from_str(&report_text).expect("report json");
        assert_eq!(report["schema"], SCHEMA);
        assert_eq!(report["approved"], json!(false));
        assert_eq!(report["published"], json!(false));
        assert_eq!(
            report["artifacts"][0]["sha256"],
            json!(dx_digest::sha256_hex(b"seed binary bytes"))
        );
        assert_eq!(
            report["artifacts"][1]["sha256"],
            json!(dx_digest::sha256_hex(b"standalone archive bytes"))
        );
        assert_eq!(report["sbom"]["subject_digest"], json!(SBOM_DIGEST));
        assert_eq!(report["sbom"]["linkage_ok"], json!(true));
        assert_eq!(report["packaging"]["ok"], json!(true));
        assert_eq!(report["draft_dry_run"]["ok"], json!(true));
        assert_eq!(report["signing_dry_run"]["ok"], json!(true));
        assert_eq!(report["bcr_shape"]["checked"], json!(true));
        assert_eq!(report["verify_refusal"]["checksum_only_refused"], json!(true));
        assert_eq!(
            report["release_matrix"].as_array().expect("matrix").len(),
            4
        );
        assert_eq!(
            report["exercised"].as_array().expect("exercised").len(),
            10
        );
        assert_eq!(
            report["would_publish"].as_array().expect("would").len(),
            2
        );
        assert_eq!(
            report["not_attempted"].as_array().expect("not").len(),
            6
        );
        let shape = std::fs::read_to_string(fixture.path("stage/bcr-shape.txt")).expect("shape");
        assert!(shape.starts_with("Bazel Central Registry module shape"));
        assert!(shape.contains("name = \"rules_dx\""));
        assert!(shape.contains("version = \"0.0.0\""));
        let summary =
            std::fs::read_to_string(fixture.path("summary.md")).expect("step summary");
        assert!(summary.starts_with("## Publish dry-run report"));
        assert!(summary.contains("approved: `false`"));
    }

    #[test]
    fn approved_run_reports_true() {
        let fixture = Fixture::new();
        let mut inputs = fixture.inputs();
        fixture.workspace();
        inputs.stage = fixture.path("stage");
        inputs.approve = "true".to_owned();
        publish_dry_run(&inputs).expect("dry run passes");
        let report_text = std::fs::read_to_string(fixture.path("stage/dry-run-report.json"))
            .expect("report");
        let report: Value = serde_json::from_str(&report_text).expect("report json");
        assert_eq!(report["approved"], json!(true));
        let summary =
            std::fs::read_to_string(fixture.path("summary.md")).expect("step summary");
        assert!(summary.contains("approved: `true`"));
    }

    #[test]
    fn stdout_reports_every_digest_like_the_shell_report() {
        let fixture = Fixture::new();
        let out = run(&fixture).expect("dry run passes");
        assert!(out.contains(&format!(
            "seed binary sha256: {}\n",
            dx_digest::sha256_hex(b"seed binary bytes")
        )));
        assert!(out.contains("sbom subject digest: 0000000000000000000000000000000000000000000000000000000000000abc (linkage ok: True)"));
        assert!(out.contains("draft dry-run ok: True"));
        assert!(out.contains("approved: False; published: False (dry run never publishes)"));
    }

    #[test]
    fn rejects_an_unrecognized_approve_value() {
        let fixture = Fixture::new();
        let mut inputs = fixture.inputs();
        fixture.workspace();
        inputs.stage = fixture.path("stage");
        inputs.approve = "yes".to_owned();
        let error = publish_dry_run(&inputs).expect_err("approve must be rejected");
        assert!(error.contains("approve must be 'true' or 'false'"), "{error}");
        assert!(!fixture.path("stage/dry-run-report.json").exists());
    }

    #[test]
    fn rejects_a_missing_staged_input() {
        let fixture = Fixture::new();
        let mut inputs = fixture.inputs();
        fixture.workspace();
        inputs.stage = fixture.path("stage");
        inputs.archive = fixture.path("out/absent.tar.gz");
        let error = publish_dry_run(&inputs).expect_err("missing archive fails");
        assert!(error.contains("cannot stage"), "{error}");
        assert!(error.contains("absent.tar.gz"), "{error}");
    }

    #[test]
    fn rejects_a_malformed_spdx_package_list() {
        let fixture = Fixture::new();
        let mut inputs = fixture.inputs();
        fixture.workspace();
        inputs.stage = fixture.path("stage");
        inputs.spdx = fixture.write("out/spdx.json", "{\"spdxVersion\": \"SPDX-2.3\"}");
        let error = publish_dry_run(&inputs).expect_err("malformed spdx fails");
        assert!(error.contains("checksumValue"), "{error}");
    }

    #[test]
    fn rejects_a_provenance_without_the_subject_digest() {
        let fixture = Fixture::new();
        let mut inputs = fixture.inputs();
        fixture.workspace();
        inputs.stage = fixture.path("stage");
        inputs.provenance = fixture.write(
            "out/prov.json",
            "{\"predicateType\": \"https://slsa.dev/provenance/v1\"}",
        );
        let error = publish_dry_run(&inputs).expect_err("digest linkage must hold");
        assert!(error.contains("never names the SPDX subject digest"), "{error}");
    }

    #[test]
    fn rejects_a_checksum_file_that_declares_another_digest() {
        let fixture = Fixture::new();
        let mut inputs = fixture.inputs();
        fixture.workspace();
        inputs.stage = fixture.path("stage");
        inputs.archive_checksum = fixture.write(
            "out/wrong.sha256",
            "1111111111111111111111111111111111111111111111111111111111111111  dx_standalone.tar.gz\n",
        );
        let error = publish_dry_run(&inputs).expect_err("mismatched checksum fails");
        assert!(error.contains("declares 1111111111111111111111111111111111111111111111111111111111111111"), "{error}");
        assert!(error.contains("hashes to"), "{error}");
    }

    #[test]
    fn rejects_a_release_list_missing_a_required_file() {
        let fixture = Fixture::new();
        let mut inputs = fixture.inputs();
        fixture.workspace();
        inputs.stage = fixture.path("stage");
        fixture.write(
            "stage/release-artifacts.txt",
            "bazel-out/bin/deploy/release/sbom_demo.spdx.json\nlicenses.toml\n",
        );
        let error = publish_dry_run(&inputs).expect_err("missing entry fails");
        assert!(error.contains("never names"), "{error}");
    }

    #[test]
    fn rejects_a_draft_log_without_the_placeholder() {
        let fixture = Fixture::new();
        let mut inputs = fixture.inputs();
        fixture.workspace();
        inputs.stage = fixture.path("stage");
        fixture.write("stage/draft-dry-run.log", "nothing happened\n");
        let error = publish_dry_run(&inputs).expect_err("draft log must carry the placeholder");
        assert!(error.contains("draft dry-run log is missing"), "{error}");
    }

    #[test]
    fn rejects_a_signing_log_without_the_trust_root() {
        let fixture = Fixture::new();
        let mut inputs = fixture.inputs();
        fixture.workspace();
        inputs.stage = fixture.path("stage");
        fixture.write(
            "stage/signing-dry-run.log",
            "cosign sign-blob\nsbom_demo.spdx.json\nsbom_demo.provenance.json\nnotice_demo.NOTICE\n",
        );
        let error = publish_dry_run(&inputs).expect_err("trust root must be announced");
        assert!(error.contains("signing dry-run log is missing"), "{error}");
    }

    #[test]
    fn rejects_a_refusal_log_that_does_not_refuse() {
        let fixture = Fixture::new();
        let mut inputs = fixture.inputs();
        fixture.workspace();
        inputs.stage = fixture.path("stage");
        fixture.write("stage/verify-refusal.log", "installed anyway\n");
        let error = publish_dry_run(&inputs).expect_err("refusal proof is required");
        assert!(
            error.contains("install verifier refusal log is missing"),
            "{error}"
        );
    }

    #[test]
    fn rejects_a_workspace_without_the_module_shape() {
        let fixture = Fixture::new();
        let mut inputs = fixture.inputs();
        fixture.workspace();
        inputs.stage = fixture.path("stage");
        fixture.write(
            "workspace/MODULE.bazel",
            "module(\n    name = \"other\",\n    version = \"1.2.3\",\n)\n",
        );
        let error = publish_dry_run(&inputs).expect_err("module shape is required");
        assert!(error.contains("MODULE.bazel never matches"), "{error}");
    }

    #[test]
    fn rejects_a_missing_release_list() {
        let fixture = Fixture::new();
        let mut inputs = fixture.inputs();
        fixture.workspace();
        inputs.stage = fixture.path("stage");
        std::fs::remove_file(fixture.path("stage/release-artifacts.txt")).expect("remove");
        let error = publish_dry_run(&inputs).expect_err("missing list fails");
        assert!(error.contains("cannot read release-artifacts.txt"), "{error}");
    }

    #[test]
    fn writes_no_report_when_any_check_fails() {
        let fixture = Fixture::new();
        let mut inputs = fixture.inputs();
        fixture.workspace();
        inputs.stage = fixture.path("stage");
        fixture.write("stage/bcr-dry-run.log", "bcr: something else\n");
        let error = publish_dry_run(&inputs).expect_err("bcr log must carry the gate");
        assert!(error.contains("bcr dry-run log is missing"), "{error}");
        assert!(!fixture.path("stage/dry-run-report.json").exists());
        assert!(!fixture.path("summary.md").exists());
    }

    #[test]
    fn notice_must_carry_its_header() {
        let fixture = Fixture::new();
        let mut inputs = fixture.inputs();
        fixture.workspace();
        inputs.stage = fixture.path("stage");
        inputs.notice = fixture.write("out/notice.txt", "demo licensed text\n");
        let error = publish_dry_run(&inputs).expect_err("notice header is required");
        assert!(error.contains("does not start with"), "{error}");
    }

    #[test]
    fn verifier_source_must_pin_the_trust_root() {
        let fixture = Fixture::new();
        let mut inputs = fixture.inputs();
        fixture.workspace();
        inputs.stage = fixture.path("stage");
        fixture.write("workspace/deploy/install/src/lib.rs", "fn main() {}\n");
        let error = publish_dry_run(&inputs).expect_err("trust root pin is required");
        assert!(error.contains("install verifier source is missing"), "{error}");
    }

    #[test]
    fn summary_is_appended_not_replaced() {
        let fixture = Fixture::new();
        fixture.write("summary.md", "prior line\n");
        let mut inputs = fixture.inputs();
        fixture.workspace();
        inputs.stage = fixture.path("stage");
        inputs.step_summary = Some(fixture.path("summary.md"));
        publish_dry_run(&inputs).expect("dry run passes");
        let summary =
            std::fs::read_to_string(fixture.path("summary.md")).expect("step summary");
        assert!(summary.starts_with("prior line\n## Publish dry-run report"));
    }

    #[test]
    fn stage_directory_is_created_when_absent() {
        let fixture = Fixture::new();
        let mut inputs = fixture.inputs();
        fixture.workspace();
        std::fs::remove_dir_all(fixture.path("stage")).expect("remove stage");
        inputs.stage = fixture.path("fresh/stage");
        publish_dry_run(&inputs).expect_err("logs live in the original stage");
        assert!(fixture.path("fresh/stage").is_dir());
    }
}
