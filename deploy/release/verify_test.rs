use std::path::{Path, PathBuf};

use dx_release_tools::{notice_verify_files, sbom_verify_files};

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
    let report = sbom_verify_files(&artifact, &spdx, &provenance)
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
    let error = sbom_verify_files(&swapped, &spdx, &provenance)
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
    let forged_text = text.replace(
        "https://github.com/ralvik/rules_dx/.github/workflows/publish-dry-run.yml",
        "https://example.test/builder",
    );
    assert_ne!(
        forged_text, text,
        "the demo provenance names an allowlisted builder"
    );
    let forged = write(dir.path(), "forged.provenance.json", &forged_text);
    let error =
        sbom_verify_files(&artifact, &spdx, &forged).expect_err("a forged builder must not pass");
    assert!(
        error
            .to_string()
            .contains("builder.id is not an allowlisted"),
        "got {error}"
    );
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
    let error = sbom_verify_files(&artifact, &spdx, &untyped)
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
fn an_inventory_entry_without_license_words_fails() {
    let dir = scratch();
    let (notice, manifest, texts) = notice_inputs();
    let drifted = write(
        dir.path(),
        "drifted.manifest",
        &format!("{}demo-lib-c|maven|3.0.0|MIT|absent.txt\n", read(&manifest)),
    );
    let error = notice_verify_files(&notice, &drifted, &texts)
        .expect_err("an inventory entry with no bundled words must not pass");
    assert!(
        error.to_string().contains("missing-notice-text"),
        "got {error}"
    );
}
