use super::*;
use serde_json::json;

fn minimal() -> Value {
    json!({
        "spdxVersion": "SPDX-2.3",
        "dataLicense": "CC0-1.0",
        "SPDXID": "SPDXRef-DOCUMENT",
        "name": "dx-test",
        "documentNamespace": "https://invalid.test/dx",
        "creationInfo": {"created": "1970-01-01T00:00:00Z", "creators": ["Tool: dx"]},
        "packages": [{
            "SPDXID": "SPDXRef-Package",
            "name": "dx",
            "downloadLocation": "NOASSERTION"
        }],
        "relationships": [{
            "spdxElementId": "SPDXRef-DOCUMENT",
            "relationshipType": "DESCRIBES",
            "relatedSpdxElement": "SPDXRef-Package"
        }]
    })
}

fn errors_of(value: &Value) -> Vec<String> {
    spdx_errors(&value.to_string())
}

#[test]
fn a_minimal_document_passes() {
    assert_eq!(errors_of(&minimal()), Vec::<String>::new());
}

#[test]
fn the_builder_output_passes() {
    let mut document = dx_sbom::Document::new("dx-test", "https://invalid.test/dx", &["Tool: dx"]);
    document.packages.push(dx_sbom::SpdxPackage {
        id: "SPDXRef-Package".to_owned(),
        name: "dx".to_owned(),
        download_location: dx_sbom::NOASSERTION.to_owned(),
        files_analyzed: Some(false),
        ..dx_sbom::SpdxPackage::default()
    });
    document
        .relationships
        .push(dx_sbom::describes("SPDXRef-Package"));
    let text = serde_json::to_string(&document).expect("render");
    assert_eq!(spdx_errors(&text), Vec::<String>::new());
}

#[test]
fn verification_code_fails_the_schema() {
    let mut value = minimal();
    value["packages"][0]["verificationCode"] = json!({"packageVerificationCodeValue": "abc"});
    let errors = schema_errors(&value);
    assert!(
        errors.iter().any(|e| e.contains("verificationCode")),
        "{errors:?}"
    );
}

#[test]
fn a_missing_creation_info_fails_the_schema() {
    let mut value = minimal();
    value
        .as_object_mut()
        .expect("object")
        .remove("creationInfo");
    let errors = schema_errors(&value);
    assert!(
        errors.iter().any(|e| e.contains("creationInfo")),
        "{errors:?}"
    );
}

#[test]
fn a_package_without_download_location_fails_the_schema() {
    let mut value = minimal();
    value["packages"][0]
        .as_object_mut()
        .expect("object")
        .remove("downloadLocation");
    let errors = schema_errors(&value);
    assert!(
        errors.iter().any(|e| e.contains("downloadLocation")),
        "{errors:?}"
    );
}

#[test]
fn a_null_field_fails_the_schema() {
    let mut value = minimal();
    value["packages"][0]["filesAnalyzed"] = Value::Null;
    assert!(!schema_errors(&value).is_empty());
}

#[test]
fn a_label_as_element_id_passes_the_schema_but_fails_the_reference_check() {
    let mut value = minimal();
    value["relationships"][0] = json!({
        "spdxElementId": "//services/payments:image",
        "relationshipType": "DESCRIBES",
        "relatedSpdxElement": "SPDXRef-DOCUMENT"
    });
    assert_eq!(schema_errors(&value), Vec::<String>::new());
    let errors = errors_of(&value);
    assert!(
        errors
            .iter()
            .any(|e| e.contains("//services/payments:image")),
        "{errors:?}"
    );
}
