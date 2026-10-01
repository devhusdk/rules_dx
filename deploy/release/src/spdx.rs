use serde::{Deserialize, Serialize};

pub const SPDX_VERSION: &str = "SPDX-2.3";

pub const DATA_LICENSE: &str = "CC0-1.0";

pub const DOCUMENT_ID: &str = "SPDXRef-DOCUMENT";

pub const SHA256: &str = "SHA256";

pub const NOASSERTION: &str = "NOASSERTION";

pub const NONE: &str = "NONE";

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct SpdxChecksum {
    pub algorithm: String,
    #[serde(rename = "checksumValue")]
    pub value: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct SpdxExternalRef {
    #[serde(rename = "referenceCategory")]
    pub category: String,
    #[serde(rename = "referenceType")]
    pub ref_type: String,
    #[serde(rename = "referenceLocator")]
    pub locator: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct SpdxVerificationCode {
    #[serde(rename = "packageVerificationCodeValue")]
    pub value: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct SpdxFile {
    #[serde(rename = "SPDXID", default, skip_serializing_if = "String::is_empty")]
    pub id: String,
    #[serde(rename = "fileName", default)]
    pub name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub checksums: Vec<SpdxChecksum>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct SpdxRelationship {
    #[serde(
        rename = "spdxElementId",
        default,
        skip_serializing_if = "String::is_empty"
    )]
    pub from: String,
    #[serde(
        rename = "relationshipType",
        default,
        skip_serializing_if = "String::is_empty"
    )]
    pub rel_type: String,
    #[serde(
        rename = "relatedSpdxElement",
        default,
        skip_serializing_if = "String::is_empty"
    )]
    pub to: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct SpdxCreationInfo {
    pub created: String,
    pub creators: Vec<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct SpdxPackage {
    #[serde(rename = "SPDXID", default, skip_serializing_if = "String::is_empty")]
    pub id: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    #[serde(
        default,
        rename = "versionInfo",
        skip_serializing_if = "String::is_empty"
    )]
    pub version: String,
    #[serde(
        default,
        rename = "licenseConcluded",
        skip_serializing_if = "String::is_empty"
    )]
    pub license_concluded: String,
    #[serde(
        default,
        rename = "licenseDeclared",
        skip_serializing_if = "String::is_empty"
    )]
    pub license_declared: String,
    #[serde(
        default,
        rename = "copyrightText",
        skip_serializing_if = "String::is_empty"
    )]
    pub copyright: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub supplier: String,
    #[serde(
        default,
        rename = "downloadLocation",
        skip_serializing_if = "String::is_empty"
    )]
    pub download_location: String,
    #[serde(default, rename = "filesAnalyzed")]
    pub files_analyzed: Option<bool>,
    #[serde(default, rename = "verificationCode")]
    pub verification_code: Option<SpdxVerificationCode>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub checksums: Vec<SpdxChecksum>,
    #[serde(
        default,
        rename = "externalRefs",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub external_refs: Vec<SpdxExternalRef>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct SpdxDocument {
    #[serde(rename = "spdxVersion")]
    pub version: String,
    #[serde(rename = "dataLicense")]
    pub data_license: String,
    #[serde(rename = "SPDXID", default, skip_serializing_if = "String::is_empty")]
    pub id: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    #[serde(
        default,
        rename = "documentNamespace",
        skip_serializing_if = "String::is_empty"
    )]
    pub namespace: String,
    #[serde(default, rename = "creationInfo")]
    pub creation_info: Option<SpdxCreationInfo>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub files: Vec<SpdxFile>,
    #[serde(default)]
    pub packages: Vec<SpdxPackage>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub relationships: Vec<SpdxRelationship>,
}

pub fn sha256_of(checksums: &[SpdxChecksum]) -> Option<&str> {
    checksums
        .iter()
        .find(|checksum| checksum.algorithm.eq_ignore_ascii_case(SHA256))
        .map(|checksum| checksum.value.as_str())
}

pub fn purl_name(locator: &str) -> Option<&str> {
    let rest = locator.strip_prefix("pkg:")?;
    let path = rest.split('@').next()?;
    path.rsplit('/').next().filter(|name| !name.is_empty())
}

pub fn purl_version(locator: &str) -> Option<&str> {
    let rest = locator.strip_prefix("pkg:")?;
    let (_, version) = rest.rsplit_once('@')?;
    Some(version).filter(|version| !version.is_empty())
}

fn license_error(field: &str, value: &str) -> Option<String> {
    if value.is_empty() || value == NOASSERTION || value == NONE {
        return None;
    }
    spdx::Expression::parse(value)
        .err()
        .map(|_| format!("sbom SPDX {field} '{value}' is not an SPDX license expression"))
}

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
            namespace: format!("https://github.com/ralvik/rules_dx/releases/{base}-{digest}"),
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
                verification_code: Some(SpdxVerificationCode {
                    value: digest.to_owned(),
                }),
                checksums: vec![checksum(digest)],
                external_refs: vec![purl("dx", digest)],
                ..SpdxPackage::default()
            }],
            relationships: Vec::new(),
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
        doc.packages[0].verification_code = None;
        doc.creation_info = None;
        doc.relationships = vec![SpdxRelationship {
            from: "SPDXRef-Package".to_owned(),
            rel_type: "DESCRIBES".to_owned(),
            to: DOCUMENT_ID.to_owned(),
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
    fn file_names_lose_any_directory_prefix() {
        assert_eq!(file_name("artifact.bin"), "artifact.bin");
        assert_eq!(file_name("./artifact.bin"), "artifact.bin");
        assert_eq!(file_name("dist/artifact.bin"), "artifact.bin");
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
}
