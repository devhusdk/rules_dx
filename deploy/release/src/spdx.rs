//! Binds a release SPDX document to the artifact it describes; the model lives in dx_sbom.

pub use dx_sbom::{
    describes, license_error, purl_name, purl_version, sha256_of, Document, SpdxChecksum,
    SpdxDocument, SpdxExternalRef, SpdxFile, SpdxPackage, DOCUMENT_ID, NOASSERTION, SHA256,
    SPDX_VERSION,
};

pub fn document_error(document: &SpdxDocument, base: &str, digest: &str) -> Result<(), String> {
    if document.version != SPDX_VERSION {
        return Err("sbom spdxVersion not SPDX-2.3".to_owned());
    }
    if document.id != DOCUMENT_ID {
        return Err(format!("sbom SPDX document id is not {DOCUMENT_ID}"));
    }
    if let Some(error) = license_error("dataLicense", &document.data_license) {
        return Err(error);
    }
    for package in &document.packages {
        if package.name.is_empty() {
            return Err("sbom SPDX names a package with no name".to_owned());
        }
        if let Some(error) = license_error("licenseConcluded", &package.license_concluded) {
            return Err(error);
        }
        if let Some(error) = license_error("licenseDeclared", &package.license_declared) {
            return Err(error);
        }
    }
    identity_error(document, base, digest)
}

fn identity_error(document: &SpdxDocument, base: &str, digest: &str) -> Result<(), String> {
    let file = document
        .files
        .iter()
        .find(|file| file_name(&file.name) == base)
        .ok_or_else(|| format!("sbom SPDX does not name artifact {base}"))?;
    if sha256_of(&file.checksums) != Some(digest) {
        return Err(format!(
            "sbom SPDX missing artifact digest {digest} for file {base}"
        ));
    }
    let package = document
        .packages
        .iter()
        .find(|package| {
            package_purls(package).any(|purl| {
                purl_version(purl) == Some(digest) && purl_name(purl) == Some(&package.name)
            })
        })
        .ok_or_else(|| format!("sbom SPDX names no package for artifact digest {digest}"))?;
    if sha256_of(&package.checksums) != Some(digest) {
        return Err(format!(
            "sbom SPDX missing artifact digest {digest} for package {}",
            package.name
        ));
    }
    Ok(())
}

fn file_name(recorded: &str) -> &str {
    let trimmed = recorded.strip_prefix("./").unwrap_or(recorded);
    trimmed.rsplit('/').next().unwrap_or(trimmed)
}

fn package_purls(package: &SpdxPackage) -> impl Iterator<Item = &str> {
    package
        .external_refs
        .iter()
        .filter(|reference| reference.ref_type == "purl")
        .map(|reference| reference.locator.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;
    use dx_sbom::{SpdxCreationInfo, SpdxRelationship, DATA_LICENSE};

    fn checksum(digest: &str) -> SpdxChecksum {
        SpdxChecksum {
            algorithm: SHA256.to_owned(),
            value: digest.to_owned(),
        }
    }

    fn purl(name: &str, digest: &str) -> SpdxExternalRef {
        SpdxExternalRef {
            category: "PACKAGE-MANAGER".to_owned(),
            ref_type: "purl".to_owned(),
            locator: format!("pkg:generic/{name}@{digest}"),
        }
    }

    fn document(base: &str, digest: &str) -> SpdxDocument {
        SpdxDocument {
            version: SPDX_VERSION.to_owned(),
            data_license: DATA_LICENSE.to_owned(),
            id: DOCUMENT_ID.to_owned(),
            name: format!("dx-{base}"),
            namespace: format!("https://github.com/devhusdk/rules_dx/releases/{base}-{digest}"),
            creation_info: Some(SpdxCreationInfo {
                created: "1970-01-01T00:00:00Z".to_owned(),
                creators: vec!["Tool: rules_dx-sbom-1.0".to_owned()],
            }),
            files: vec![SpdxFile {
                id: "SPDXRef-File".to_owned(),
                name: base.to_owned(),
                checksums: vec![checksum(digest)],
            }],
            packages: vec![SpdxPackage {
                id: "SPDXRef-Package".to_owned(),
                name: "dx".to_owned(),
                supplier: "Organization: rules_dx".to_owned(),
                download_location: NOASSERTION.to_owned(),
                files_analyzed: Some(false),
                checksums: vec![checksum(digest)],
                external_refs: vec![purl("dx", digest)],
                ..SpdxPackage::default()
            }],
            relationships: vec![describes("SPDXRef-Package")],
        }
    }

    #[test]
    fn binds_a_document_to_its_artifact() {
        let digest = "a948904f2f0f479b8f8197694b30184b0d2ed1c1cd2a1ec0fb85d299a192a447";
        let doc = document("artifact.bin", digest);
        assert!(document_error(&doc, "artifact.bin", digest).is_ok());
        let err = document_error(&doc, "artifact.bin", "0").expect_err("wrong digest");
        assert!(err.contains("missing artifact digest"), "got {err}");
        let err = document_error(&doc, "other.bin", digest).expect_err("wrong file");
        assert!(
            err.contains("does not name artifact other.bin"),
            "got {err}"
        );
    }

    #[test]
    fn reads_the_license_audit_subset_of_the_schema() {
        let digest = "a948904f2f0f479b8f8197694b30184b0d2ed1c1cd2a1ec0fb85d299a192a447";
        let mut doc = document("artifact.bin", digest);
        doc.packages[0].version = "1.0.0".to_owned();
        doc.packages[0].license_concluded = "MIT".to_owned();
        doc.packages[0].license_declared = "MIT".to_owned();
        doc.packages[0].copyright = NOASSERTION.to_owned();
        doc.packages[0].supplier = String::new();
        doc.packages[0].download_location = String::new();
        doc.packages[0].files_analyzed = None;
        doc.creation_info = None;
        doc.relationships = vec![SpdxRelationship {
            from: DOCUMENT_ID.to_owned(),
            rel_type: "DESCRIBES".to_owned(),
            to: "SPDXRef-Package".to_owned(),
        }];
        let text = serde_json::to_string(&doc).expect("render audit document");
        let parsed: SpdxDocument = serde_json::from_str(&text).expect("read audit document");
        assert_eq!(parsed.packages[0].license_concluded, "MIT");
        assert_eq!(parsed.packages[0].version, "1.0.0");
        assert!(document_error(&parsed, "artifact.bin", digest).is_ok());
    }

    #[test]
    fn rejects_an_unparsable_license_through_the_spdx_crate() {
        let digest = "a948904f2f0f479b8f8197694b30184b0d2ed1c1cd2a1ec0fb85d299a192a447";
        let mut doc = document("artifact.bin", digest);
        doc.packages[0].license_concluded = "MIT AND".to_owned();
        let err = document_error(&doc, "artifact.bin", digest).expect_err("bad expression");
        assert!(err.contains("licenseConcluded"), "got {err}");
        doc.packages[0].license_concluded = "MIT".to_owned();
        doc.packages[0].license_declared = NOASSERTION.to_owned();
        assert!(document_error(&doc, "artifact.bin", digest).is_ok());
    }

    #[test]
    fn file_names_lose_any_directory_prefix() {
        assert_eq!(file_name("artifact.bin"), "artifact.bin");
        assert_eq!(file_name("./artifact.bin"), "artifact.bin");
        assert_eq!(file_name("dist/artifact.bin"), "artifact.bin");
    }
}
