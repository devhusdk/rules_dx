use std::path::{Path, PathBuf};

use dx_release_tools::{
    bcr_candidate_error, notice_verify_files, sbom_verify_files, write_bcr_source_for_archive,
    PROVENANCE_BUILDER_DRY_RUN,
};

fn data(name: &str) -> PathBuf {
    let rel = std::env::var(name).unwrap_or_else(|_| panic!("{name} must name a test input"));
    dx_testing::resolve_runfiles(&rel)
}

fn scratch() -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix("dx-release-verify-")
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

fn sbom_inputs() -> (PathBuf, PathBuf, PathBuf) {
    (
        data("DX_SBOM_ARTIFACT"),
        data("DX_SBOM_SPDX"),
        data("DX_SBOM_PROVENANCE"),
    )
}

fn notice_inputs() -> (PathBuf, PathBuf, Vec<PathBuf>) {
    (
        data("DX_NOTICE"),
        data("DX_NOTICE_MANIFEST"),
        vec![data("DX_NOTICE_TEXT_A"), data("DX_NOTICE_TEXT_B")],
    )
}

#[test]
fn the_sbom_pair_binds_the_archived_artifact() {
    let (artifact, spdx, provenance) = sbom_inputs();
    let report = sbom_verify_files(&artifact, &spdx, &provenance, PROVENANCE_BUILDER_DRY_RUN)
        .unwrap_or_else(|error| panic!("the generated SBOM pair must verify: {error}"));
    let digest = dx_digest::sha256_file_hex(&artifact)
        .unwrap_or_else(|error| panic!("hash the archived artifact: {error}"));
    assert_eq!(
        report,
        format!("sbom OK: SPDX-2.3 + SLSA v1 bind {digest}\n")
    );
}

#[test]
fn a_swapped_artifact_breaks_the_sbom_binding() {
    let dir = scratch();
    let (artifact, spdx, provenance) = sbom_inputs();
    let name = artifact
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("artifact.tar.gz")
        .to_owned();
    let staged = dir.path().join("swapped");
    std::fs::create_dir_all(&staged).unwrap_or_else(|error| panic!("stage dir: {error}"));
    let mut bytes =
        std::fs::read(&artifact).unwrap_or_else(|error| panic!("read artifact: {error}"));
    bytes.push(b'\n');
    let swapped = write_bytes(&staged, &name, &bytes);
    let error = sbom_verify_files(&swapped, &spdx, &provenance, PROVENANCE_BUILDER_DRY_RUN)
        .expect_err("a swapped artifact must not pass");
    assert!(
        error.to_string().contains("missing artifact digest"),
        "got {error}"
    );
}

#[test]
fn a_forged_provenance_builder_fails_closed() {
    let dir = scratch();
    let (artifact, spdx, provenance) = sbom_inputs();
    let text = read(&provenance);
    let real = "https://github.com/devhusdk/rules_dx/.github/workflows/publish-dry-run.yml";
    let prefixed = format!("{real}.evil");
    for forgery in ["https://example.test/builder", prefixed.as_str()] {
        let forged_text = text.replace(real, forgery);
        assert_ne!(
            forged_text, text,
            "the demo provenance names an allowlisted builder"
        );
        let forged = write(dir.path(), "forged.provenance.json", &forged_text);
        let error = sbom_verify_files(&artifact, &spdx, &forged, PROVENANCE_BUILDER_DRY_RUN)
            .expect_err("a forged builder must not pass");
        let shown = error.to_string();
        assert!(shown.contains("builder.id"), "got {shown}");
        assert!(shown.contains("is not an allowlisted"), "got {shown}");
    }
}

#[test]
fn provenance_without_the_slsa_predicate_fails() {
    let dir = scratch();
    let (artifact, spdx, provenance) = sbom_inputs();
    let untyped = write(
        dir.path(),
        "untyped.provenance.json",
        &read(&provenance).replace(
            "https://slsa.dev/provenance/v1",
            "https://slsa.dev/provenance",
        ),
    );
    let error = sbom_verify_files(&artifact, &spdx, &untyped, PROVENANCE_BUILDER_DRY_RUN)
        .expect_err("a bare SLSA predicate must not pass");
    assert!(
        error.to_string().contains("missing SLSA v1 predicate"),
        "got {error}"
    );
}

#[test]
fn the_bundled_notice_carries_every_inventory_entry_and_its_words() {
    let (notice, manifest, texts) = notice_inputs();
    let report = notice_verify_files(&notice, &manifest, &texts)
        .unwrap_or_else(|error| panic!("the generated NOTICE must verify: {error}"));
    assert_eq!(report, "notice OK: 2 entries bundled\n");
}

#[test]
fn a_notice_missing_an_entry_header_fails() {
    let dir = scratch();
    let (notice, manifest, texts) = notice_inputs();
    let text = read(&notice);
    let without_entry = text.replace("=== demo-lib-b 2.3.4 (Apache-2.0) ===\n", "");
    assert_ne!(
        without_entry, text,
        "the demo NOTICE carries two entry headers"
    );
    let stripped = write(dir.path(), "stripped.NOTICE", &without_entry);
    let error = notice_verify_files(&stripped, &manifest, &texts)
        .expect_err("a NOTICE missing an entry header must not pass");
    assert!(
        error.to_string().contains("NOTICE missing entry header"),
        "got {error}"
    );
}

#[test]
fn a_notice_missing_license_words_fails() {
    let dir = scratch();
    let (notice, manifest, texts) = notice_inputs();
    let text = read(&notice);
    let probe = read(&texts[0])
        .lines()
        .next()
        .unwrap_or_default()
        .trim()
        .to_owned();
    assert!(
        text.contains(&probe),
        "the demo NOTICE carries the first entry's words"
    );
    let redacted = write(
        dir.path(),
        "redacted.NOTICE",
        &text.replace(&probe, "REDACTED"),
    );
    let error = notice_verify_files(&redacted, &manifest, &texts)
        .expect_err("a NOTICE missing license words must not pass");
    assert!(
        error
            .to_string()
            .contains("NOTICE missing words for demo-lib-a@1.0.0 (MIT)"),
        "got {error}"
    );
}

#[test]
fn a_notice_with_drifted_words_after_the_first_line_fails() {
    let dir = scratch();
    let (notice, manifest, texts) = notice_inputs();
    let text = read(&notice);
    let next = "=== demo-lib-b 2.3.4 (Apache-2.0) ===";
    let drifted = text.replacen(next, &format!("Drifted extra line.\n{next}"), 1);
    assert_ne!(
        drifted, text,
        "the demo NOTICE carries the demo-lib-b header"
    );
    let drifted = write(dir.path(), "drifted.NOTICE", &drifted);
    let error = notice_verify_files(&drifted, &manifest, &texts)
        .expect_err("a NOTICE with drifted license words must not pass");
    assert!(
        error
            .to_string()
            .contains("NOTICE missing words for demo-lib-a@1.0.0 (MIT)"),
        "got {error}"
    );
}

#[test]
fn an_inventory_entry_without_license_words_fails() {
    let dir = scratch();
    let (notice, manifest, texts) = notice_inputs();
    let drifted = write(
        dir.path(),
        "drifted.manifest",
        &format!(
            "{}demo-lib-c|maven|3.0.0|MIT|absent.txt|{}\n",
            read(&manifest),
            "0".repeat(64)
        ),
    );
    let error = notice_verify_files(&notice, &drifted, &texts)
        .expect_err("an inventory entry with no bundled words must not pass");
    assert!(
        error.to_string().contains("missing-notice-text"),
        "got {error}"
    );
}

const CANDIDATE_VERSION: &str = "1.2.3-rc1";

fn candidate_source(dir: &Path) -> (PathBuf, String) {
    let (artifact, _, _) = sbom_inputs();
    let digest = dx_digest::sha256_file_hex(&artifact)
        .unwrap_or_else(|error| panic!("hash the archived artifact: {error}"));
    let dst = dir.join("candidate.source.json");
    write_bcr_source_for_archive(&dst, "rules_dx", CANDIDATE_VERSION, &artifact)
        .unwrap_or_else(|error| panic!("write candidate source.json: {error}"));
    (dst, digest)
}

#[test]
fn candidate_source_metadata_binds_the_staged_archive() {
    let dir = scratch();
    let (source, digest) = candidate_source(dir.path());
    let text = read(&source);
    assert!(
        text.contains("\"integrity\": \"sha256-"),
        "candidate source.json carries a real SRI integrity, got {text}"
    );
    bcr_candidate_error(&text, "rules_dx", CANDIDATE_VERSION, &digest)
        .unwrap_or_else(|error| panic!("candidate metadata must bind the staged archive: {error}"));
}

#[test]
fn candidate_source_rejects_a_replaced_archive() {
    let dir = scratch();
    let (source, _) = candidate_source(dir.path());
    let text = read(&source);
    let zeros = "0".repeat(64);
    let error = bcr_candidate_error(&text, "rules_dx", CANDIDATE_VERSION, &zeros)
        .expect_err("a replaced archive for one version must not pass");
    assert!(error.contains("integrity mismatch"), "got {error}");
}

#[test]
fn candidate_source_rejects_wrong_prefix_and_url() {
    let dir = scratch();
    let (source, digest) = candidate_source(dir.path());
    let text = read(&source);
    let error = bcr_candidate_error(&text, "rules_dx", "1.2.4", &digest)
        .expect_err("a wrong strip-prefix version must not pass");
    assert!(error.contains("strip_prefix mismatch"), "got {error}");
    let tampered = write(
        dir.path(),
        "tampered.source.json",
        &text.replace("v1.2.3-rc1/rules_dx-1.2.3-rc1", "v9.9.9/rules_dx-9.9.9"),
    );
    let error = bcr_candidate_error(&read(&tampered), "rules_dx", CANDIDATE_VERSION, &digest)
        .expect_err("a tampered url must not pass");
    assert!(error.contains("url mismatch"), "got {error}");
}

#[test]
fn interrupted_candidate_publication_fails_closed() {
    let dir = scratch();
    let (source, digest) = candidate_source(dir.path());
    let text = read(&source);
    let cut = write(dir.path(), "cut.source.json", &text[..text.len() / 2]);
    let error = bcr_candidate_error(&read(&cut), "rules_dx", CANDIDATE_VERSION, &digest)
        .expect_err("an interrupted publication must not pass");
    assert!(error.contains("not JSON"), "got {error}");
}
