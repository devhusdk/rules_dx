use super::*;

fn checksum(digest: &str) -> SpdxChecksum {
    SpdxChecksum {
        algorithm: SHA256.to_owned(),
        value: digest.to_owned(),
    }
}

fn package(id: &str) -> SpdxPackage {
    SpdxPackage {
        id: id.to_owned(),
        name: "dx".to_owned(),
        download_location: NOASSERTION.to_owned(),
        ..SpdxPackage::default()
    }
}

fn valid() -> SpdxDocument {
    let mut document = Document::new("dx-test", "https://invalid.test/dx", &["Tool: dx"]);
    document.packages = vec![package("SPDXRef-Root-1"), package("SPDXRef-Package-1")];
    document.relationships = vec![
        describes("SPDXRef-Root-1"),
        contains("SPDXRef-Root-1", "SPDXRef-Package-1"),
    ];
    document
}

#[test]
fn document_new_sets_the_shared_header() {
    let document = Document::new("name", "ns", &["Tool: dx"]);
    assert_eq!(document.version, "SPDX-2.3");
    assert_eq!(document.data_license, "CC0-1.0");
    assert_eq!(document.id, "SPDXRef-DOCUMENT");
    assert_eq!(document.name, "name");
    assert_eq!(document.namespace, "ns");
    let info = document.creation_info.expect("creation info");
    assert_eq!(info.created, CREATED);
    assert_eq!(info.creators, vec!["Tool: dx".to_owned()]);
}

#[test]
fn builders_point_describes_from_the_document() {
    let rel = describes("SPDXRef-Package");
    assert_eq!(rel.from, DOCUMENT_ID);
    assert_eq!(rel.rel_type, DESCRIBES);
    assert_eq!(rel.to, "SPDXRef-Package");
    let rel = contains("SPDXRef-A", "SPDXRef-B");
    assert_eq!(
        (rel.from.as_str(), rel.rel_type.as_str()),
        ("SPDXRef-A", CONTAINS)
    );
    assert_eq!(rel.to, "SPDXRef-B");
}

#[test]
fn serializes_none_as_absent() {
    let mut document = valid();
    document.creation_info = None;
    let text = serde_json::to_string(&document).expect("render");
    assert!(!text.contains("null"), "{text}");
    assert!(!text.contains("creationInfo"), "{text}");
    assert!(!text.contains("filesAnalyzed"), "{text}");
    assert!(!text.contains("verificationCode"), "{text}");
    let mut document = valid();
    document.packages[0].files_analyzed = Some(false);
    let text = serde_json::to_string(&document).expect("render");
    assert!(text.contains("\"filesAnalyzed\":false"), "{text}");
    assert!(!text.contains("null"), "{text}");
}

#[test]
fn reads_legacy_release_sbom() {
    let text = r#"{
      "SPDXID": "SPDXRef-DOCUMENT",
      "creationInfo": null,
      "dataLicense": "CC0-1.0",
      "documentNamespace": "https://github.com/devhusdk/rules_dx/releases/a-b",
      "files": [{"SPDXID": "SPDXRef-File", "fileName": "a", "checksums": [{"algorithm": "SHA256", "checksumValue": "b"}]}],
      "name": "dx-a",
      "packages": [{
        "SPDXID": "SPDXRef-Package",
        "name": "dx",
        "downloadLocation": "NOASSERTION",
        "filesAnalyzed": null,
        "verificationCode": {"packageVerificationCodeValue": "b"},
        "checksums": [{"algorithm": "SHA256", "checksumValue": "b"}]
      }],
      "spdxVersion": "SPDX-2.3"
    }"#;
    let document: SpdxDocument = serde_json::from_str(text).expect("legacy document parses");
    assert_eq!(document.packages[0].files_analyzed, None);
    assert_eq!(sha256_of(&document.packages[0].checksums), Some("b"));
    assert!(document.relationships.is_empty());
}

#[test]
fn reference_errors_accept_a_valid_document() {
    assert_eq!(reference_errors(&valid()), Vec::<String>::new());
}

#[test]
fn reference_errors_reject_a_label_as_element_id() {
    let mut document = valid();
    document.relationships.push(SpdxRelationship {
        from: "//services/payments:image".to_owned(),
        rel_type: DESCRIBES.to_owned(),
        to: DOCUMENT_ID.to_owned(),
    });
    document.packages.push(package("//a:b"));
    let errors = reference_errors(&document);
    assert!(
        errors.iter().any(|e| e.contains("'//a:b' is not SPDXRef")),
        "{errors:?}"
    );
    assert!(
        errors
            .iter()
            .any(|e| e.contains("undeclared element '//services/payments:image'")),
        "{errors:?}"
    );
}

#[test]
fn reference_errors_reject_a_dangling_target() {
    let mut document = valid();
    document
        .relationships
        .push(contains("SPDXRef-Root-1", "SPDXRef-Package-9"));
    let errors = reference_errors(&document);
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert!(errors[0].contains("'SPDXRef-Package-9'"), "{errors:?}");
}

#[test]
fn reference_errors_require_a_describes_root() {
    let mut document = valid();
    document.relationships.remove(0);
    let errors = reference_errors(&document);
    assert_eq!(
        errors,
        vec!["no SPDXRef-DOCUMENT DESCRIBES relationship".to_owned()]
    );
}

#[test]
fn reference_errors_reject_a_reversed_describes() {
    let mut document = valid();
    document.relationships[0] = SpdxRelationship {
        from: "SPDXRef-Root-1".to_owned(),
        rel_type: DESCRIBES.to_owned(),
        to: DOCUMENT_ID.to_owned(),
    };
    let errors = reference_errors(&document);
    assert!(
        errors.iter().any(|e| e.contains("DESCRIBES must run from")),
        "{errors:?}"
    );
    assert!(
        errors
            .iter()
            .any(|e| e.contains("no SPDXRef-DOCUMENT DESCRIBES")),
        "{errors:?}"
    );
}

#[test]
fn reference_errors_reject_duplicate_ids() {
    let mut document = valid();
    document.packages.push(package("SPDXRef-Root-1"));
    let errors = reference_errors(&document);
    assert!(
        errors.iter().any(|e| e.contains("declared twice")),
        "{errors:?}"
    );
}

#[test]
fn element_ids_follow_the_spdx_shape() {
    assert!(is_element_id("SPDXRef-Package-1"));
    assert!(is_element_id("SPDXRef-a.b-C"));
    assert!(!is_element_id("SPDXRef-"));
    assert!(!is_element_id("SPDXRef-a b"));
    assert!(!is_element_id("SPDXRef-a_b"));
    assert!(!is_element_id("//a:b"));
}

#[test]
fn license_error_accepts_sentinels_and_rejects_bad_expressions() {
    assert_eq!(license_error("f", ""), None);
    assert_eq!(license_error("f", NOASSERTION), None);
    assert_eq!(license_error("f", NONE), None);
    assert_eq!(license_error("f", "MIT OR Apache-2.0"), None);
    let err = license_error("licenseConcluded", "MIT AND").expect("bad expression");
    assert!(err.contains("licenseConcluded"), "{err}");
}

#[test]
fn purl_locators_split_into_name_and_version() {
    assert_eq!(
        purl_name("pkg:generic/dx@abc"),
        Some("dx"),
        "generic purl names the package"
    );
    assert_eq!(purl_version("pkg:generic/dx@abc"), Some("abc"));
    assert_eq!(purl_name("pkg:maven/junit/junit@4.13.2"), Some("junit"));
    assert_eq!(purl_version("pkg:maven/junit/junit@4.13.2"), Some("4.13.2"));
    assert_eq!(purl_name("pkg:generic/dx@"), Some("dx"), "name survives");
    assert_eq!(purl_version("pkg:generic/dx@"), None, "no version");
    assert_eq!(purl_name("pkg:generic/dx"), Some("dx"), "bare purl");
    assert_eq!(purl_version("pkg:generic/dx"), None);
    assert_eq!(purl_name("not-a-purl"), None);
}

#[test]
fn checksums_select_sha256_case_insensitively() {
    let checksums = vec![
        SpdxChecksum {
            algorithm: "MD5".to_owned(),
            value: "0123456789abcdef0123456789abcdef".to_owned(),
        },
        checksum("abc"),
    ];
    assert_eq!(sha256_of(&checksums), Some("abc"));
    assert_eq!(sha256_of(&[]), None);
}
