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
fn purl_qualifiers_and_subpath_stay_out_of_name_and_version() {
    assert_eq!(
        purl_version("pkg:cargo/serde@1.0.0?arch=x86"),
        Some("1.0.0")
    );
    assert_eq!(
        purl_version("pkg:golang/github.com/a/b@v1#sub/dir"),
        Some("v1")
    );
    assert_eq!(purl_name("pkg:golang/github.com/a/b@v1#sub/dir"), Some("b"));
    assert_eq!(purl_name("pkg:cargo/serde?path=a/b"), Some("serde"));
    assert_eq!(purl_name("pkg:cargo/serde#a/b"), Some("serde"));
    assert_eq!(
        purl_version("pkg:maven/g/a?repository_url=https://u@host/r"),
        None,
        "an @ inside a qualifier is not a version"
    );
    assert_eq!(purl_version("pkg:generic/dx@?arch=x"), None);
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

#[test]
fn spdx_package_count_reads_the_packages_array() {
    let scratch = dx_test_scratch::scratch("dx_sbom_count_").unwrap();
    let dir = scratch.path().to_path_buf();
    let good = dir.join("g.spdx.json");
    std::fs::write(&good, br#"{"packages":[{"name":"a"},{"name":"b"}]}"#).expect("write");
    let bad = dir.join("b.spdx.json");
    std::fs::write(&bad, b"not json").expect("write");
    assert_eq!(spdx_package_count(&good), Some(2));
    assert_eq!(spdx_package_count(&bad), None);
    assert_eq!(spdx_package_count(&dir.join("missing")), None);
}

#[test]
fn purl_parts_survive_an_unencoded_at_in_the_namespace() {
    assert_eq!(purl_name("pkg:npm/@angular/core@16.0.0"), Some("core"));
    assert_eq!(purl_version("pkg:npm/@angular/core@16.0.0"), Some("16.0.0"));
    assert_eq!(purl_name("pkg:npm/@angular/core"), Some("core"));
    assert_eq!(purl_version("pkg:npm/@angular/core"), None);
}

#[test]
fn purl_without_a_name_has_no_name_or_version() {
    assert_eq!(purl_name("pkg:generic"), None);
    assert_eq!(purl_name("pkg:dx@abc"), None);
    assert_eq!(purl_version("pkg:dx@abc"), None);
    assert_eq!(purl_name("pkg:/dx@abc"), None);
}

#[test]
fn reference_errors_accept_none_and_noassertion_targets() {
    let mut document = valid();
    document.relationships.push(SpdxRelationship {
        from: "SPDXRef-Package-1".to_owned(),
        rel_type: "DEPENDS_ON".to_owned(),
        to: NOASSERTION.to_owned(),
    });
    document
        .relationships
        .push(contains("SPDXRef-Package-1", NONE));
    assert_eq!(reference_errors(&document), Vec::<String>::new());
}

#[test]
fn describes_none_is_not_a_root() {
    let mut document = valid();
    document.relationships = vec![describes(NONE)];
    let errors = reference_errors(&document);
    assert!(
        errors
            .iter()
            .any(|e| e.contains("no SPDXRef-DOCUMENT DESCRIBES")),
        "{errors:?}"
    );
}

#[test]
fn cyclonedx_reference_errors_reject_an_empty_bom_ref() {
    let mut document = cyclonedx::CycloneDxDocument::new("app", CREATED, "dx");
    document.add_library(cyclonedx::CdxComponent {
        kind: "library".to_owned(),
        name: "a".to_owned(),
        ..cyclonedx::CdxComponent::default()
    });
    let errors = cyclonedx::reference_errors(&document);
    assert!(
        errors.iter().any(|e| e.contains("empty bom-ref")),
        "{errors:?}"
    );
}

#[test]
fn cyclonedx_reference_errors_reject_repeated_dependency_refs() {
    let mut document = cyclonedx::CycloneDxDocument::new("app", CREATED, "dx");
    document.add_library(cyclonedx::CdxComponent {
        kind: "library".to_owned(),
        bom_ref: "a".to_owned(),
        name: "a".to_owned(),
        ..cyclonedx::CdxComponent::default()
    });
    let mut repeated = document.clone();
    repeated.dependencies[0].depends_on.push("a".to_owned());
    let errors = cyclonedx::reference_errors(&repeated);
    assert!(
        errors.iter().any(|e| e.contains("depends on 'a' twice")),
        "{errors:?}"
    );
    let mut repeated = document;
    repeated.dependencies.push(cyclonedx::CdxDependency {
        reference: "a".to_owned(),
        depends_on: Vec::new(),
    });
    let errors = cyclonedx::reference_errors(&repeated);
    assert!(
        errors
            .iter()
            .any(|e| e.contains("dependency 'a' is listed twice")),
        "{errors:?}"
    );
}

#[test]
fn cyclonedx_add_library_hangs_off_the_root_entry() {
    let mut document = cyclonedx::CycloneDxDocument::new("app", CREATED, "dx");
    document.dependencies.push(cyclonedx::CdxDependency {
        reference: "other".to_owned(),
        depends_on: Vec::new(),
    });
    document.add_library(cyclonedx::CdxComponent {
        kind: "library".to_owned(),
        bom_ref: "a".to_owned(),
        name: "a".to_owned(),
        ..cyclonedx::CdxComponent::default()
    });
    let root = document
        .dependencies
        .iter()
        .find(|dependency| dependency.reference == cyclonedx::ROOT_REF)
        .expect("root entry");
    assert_eq!(root.depends_on, vec!["a".to_owned()]);
    assert!(
        document
            .dependencies
            .iter()
            .filter(|dependency| dependency.reference != cyclonedx::ROOT_REF)
            .all(|dependency| dependency.depends_on.is_empty()),
        "{:?}",
        document.dependencies
    );
}

#[test]
fn cyclonedx_license_choices_trim_whitespace() {
    assert_eq!(
        cyclonedx::license_choices(" MIT "),
        cyclonedx::license_choices("MIT")
    );
}
