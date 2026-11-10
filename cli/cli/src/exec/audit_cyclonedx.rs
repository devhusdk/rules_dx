use dx_audit::locks::LicensedPackage;
use dx_sbom::cyclonedx::{CdxComponent, CycloneDxDocument, license_choices};

use super::audit_spdx::{id_part, license_value, purl};

/// Builds the CycloneDX 1.6 document for one `dx license` run: a root component for the scope and
/// one library per resolved dependency, in the order given.
pub(super) fn license_cyclonedx(
    scopes: &[String],
    packages: &[LicensedPackage],
    created: &str,
) -> Result<String, dx_error::DxError> {
    let scope = if scopes.is_empty() {
        "workspace".to_owned()
    } else {
        scopes.join(",")
    };
    let mut document = CycloneDxDocument::new(&scope, created, "dx-license");
    for (index, package) in packages.iter().enumerate() {
        document.add_library(CdxComponent {
            kind: "library".to_owned(),
            bom_ref: format!("pkg-{index}-{}", id_part(&package.name)),
            name: package.name.clone(),
            version: package.version.clone(),
            purl: purl(package),
            licenses: license_choices(&license_value(&package.license)),
        });
    }
    let errors = dx_sbom::cyclonedx::reference_errors(&document);
    if !errors.is_empty() {
        return Err(dx_error::DxError::failure(errors.join("; ")));
    }
    serde_json::to_string_pretty(&document)
        .map_err(|error| dx_error::DxError::failure(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn package(set: &str, name: &str, version: &str, license: &str) -> LicensedPackage {
        LicensedPackage {
            set: set.to_owned(),
            name: name.to_owned(),
            version: version.to_owned(),
            license: license.to_owned(),
        }
    }

    #[test]
    fn the_document_passes_the_schema_and_is_repeatable() {
        let packages = vec![
            package("cargo", "serde", "1.0.100", "MIT"),
            package("npm", "@scope/left-pad", "1.3.0", "UNKNOWN"),
            package("maven", "junit:junit", "4.13.2", "EPL-1.0 OR MIT"),
        ];
        let scopes = ["//app".to_owned()];
        let text = license_cyclonedx(&scopes, &packages, "2026-01-02T00:00:00Z").expect("document");
        assert_eq!(
            dx_sbom_schema::cyclonedx_errors(&text),
            Vec::<String>::new()
        );
        let value: serde_json::Value = serde_json::from_str(&text).expect("json");
        assert_eq!(value["specVersion"], "1.6");
        assert_eq!(
            value["components"][0]["licenses"][0]["license"]["id"],
            "MIT"
        );
        assert!(value["components"][1].get("licenses").is_none());
        assert_eq!(
            value["components"][2]["licenses"][0]["expression"],
            "EPL-1.0 OR MIT"
        );
        assert_eq!(
            value["components"][1]["purl"],
            "pkg:npm/%40scope/left-pad@1.3.0"
        );
        assert_eq!(value["metadata"]["timestamp"], "2026-01-02T00:00:00Z");
        assert_eq!(
            text,
            license_cyclonedx(&scopes, &packages, "2026-01-02T00:00:00Z").expect("again")
        );
    }
}
