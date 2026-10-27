//! The one SPDX 2.3 JSON data model shared by every dx SBOM writer and reader.
//!
//! Writers build documents through [`Document::new`] so the version, data license, document id
//! and creation info come from one place. [`reference_errors`] checks the graph rules the JSON
//! schema cannot express: element ids, dangling relationship ends, and the DESCRIBES root.

#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

pub const SPDX_VERSION: &str = "SPDX-2.3";

pub const DATA_LICENSE: &str = "CC0-1.0";

pub const DOCUMENT_ID: &str = "SPDXRef-DOCUMENT";

pub const SHA256: &str = "SHA256";

pub const NOASSERTION: &str = "NOASSERTION";

pub const NONE: &str = "NONE";

pub const DESCRIBES: &str = "DESCRIBES";

pub const CONTAINS: &str = "CONTAINS";

/// Fixed creation time, so the same inputs render the same bytes.
pub const CREATED: &str = "1970-01-01T00:00:00Z";

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpdxChecksum {
    pub algorithm: String,
    #[serde(rename = "checksumValue")]
    pub value: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpdxExternalRef {
    #[serde(rename = "referenceCategory")]
    pub category: String,
    #[serde(rename = "referenceType")]
    pub ref_type: String,
    #[serde(rename = "referenceLocator")]
    pub locator: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpdxFile {
    #[serde(rename = "SPDXID", default, skip_serializing_if = "String::is_empty")]
    pub id: String,
    #[serde(rename = "fileName", default)]
    pub name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub checksums: Vec<SpdxChecksum>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpdxCreationInfo {
    pub created: String,
    pub creators: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
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
    #[serde(
        default,
        rename = "filesAnalyzed",
        skip_serializing_if = "Option::is_none"
    )]
    pub files_analyzed: Option<bool>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub checksums: Vec<SpdxChecksum>,
    #[serde(
        default,
        rename = "externalRefs",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub external_refs: Vec<SpdxExternalRef>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
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
    #[serde(
        default,
        rename = "creationInfo",
        skip_serializing_if = "Option::is_none"
    )]
    pub creation_info: Option<SpdxCreationInfo>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub files: Vec<SpdxFile>,
    #[serde(default)]
    pub packages: Vec<SpdxPackage>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub relationships: Vec<SpdxRelationship>,
}

/// The document type every dx writer builds through [`SpdxDocument::new`].
pub type Document = SpdxDocument;

impl SpdxDocument {
    /// Returns an empty SPDX 2.3 document with version, data license, id and creation info set.
    pub fn new(name: &str, namespace: &str, creators: &[&str]) -> Self {
        Self {
            version: SPDX_VERSION.to_owned(),
            data_license: DATA_LICENSE.to_owned(),
            id: DOCUMENT_ID.to_owned(),
            name: name.to_owned(),
            namespace: namespace.to_owned(),
            creation_info: Some(SpdxCreationInfo {
                created: CREATED.to_owned(),
                creators: creators
                    .iter()
                    .map(|creator| (*creator).to_owned())
                    .collect(),
            }),
            ..Self::default()
        }
    }
}

/// `SPDXRef-DOCUMENT DESCRIBES <id>`: the document names `id` as one of its roots.
pub fn describes(id: &str) -> SpdxRelationship {
    SpdxRelationship {
        from: DOCUMENT_ID.to_owned(),
        rel_type: DESCRIBES.to_owned(),
        to: id.to_owned(),
    }
}

/// `<from> CONTAINS <to>`.
pub fn contains(from: &str, to: &str) -> SpdxRelationship {
    SpdxRelationship {
        from: from.to_owned(),
        rel_type: CONTAINS.to_owned(),
        to: to.to_owned(),
    }
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

/// Returns why `value` is not an SPDX license expression, or `None` when it is (or is a sentinel).
pub fn license_error(field: &str, value: &str) -> Option<String> {
    if value.is_empty() || value == NOASSERTION || value == NONE {
        return None;
    }
    spdx::Expression::parse(value)
        .err()
        .map(|_| format!("sbom SPDX {field} '{value}' is not an SPDX license expression"))
}

/// True when `id` has the `SPDXRef-[A-Za-z0-9.-]+` shape SPDX 2.3 requires of element ids.
pub fn is_element_id(id: &str) -> bool {
    id.strip_prefix("SPDXRef-").is_some_and(|rest| {
        !rest.is_empty()
            && rest
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
    })
}

/// Graph rules the SPDX 2.3 JSON schema does not check: every element id is well formed and
/// unique, every relationship end is the document or a declared element, and the document
/// DESCRIBES at least one element.
pub fn reference_errors(document: &SpdxDocument) -> Vec<String> {
    let mut errors = Vec::new();
    if document.id != DOCUMENT_ID {
        errors.push(format!(
            "document SPDXID '{}' is not {DOCUMENT_ID}",
            document.id
        ));
    }
    let mut declared = BTreeSet::new();
    let ids = document
        .packages
        .iter()
        .map(|package| &package.id)
        .chain(document.files.iter().map(|file| &file.id));
    for id in ids {
        if !is_element_id(id) {
            errors.push(format!(
                "element SPDXID '{id}' is not SPDXRef-[A-Za-z0-9.-]+"
            ));
        } else if id == DOCUMENT_ID {
            errors.push(format!("element SPDXID '{id}' reuses the document id"));
        }
        if !declared.insert(id.as_str()) {
            errors.push(format!("element SPDXID '{id}' is declared twice"));
        }
    }
    let known = |id: &str| id == DOCUMENT_ID || declared.contains(id);
    let mut describes_any = false;
    for relationship in &document.relationships {
        for end in [&relationship.from, &relationship.to] {
            if !known(end) {
                errors.push(format!(
                    "relationship {} {} {} names undeclared element '{end}'",
                    relationship.from, relationship.rel_type, relationship.to
                ));
            }
        }
        if relationship.rel_type == DESCRIBES {
            if relationship.from == DOCUMENT_ID && relationship.to != DOCUMENT_ID {
                describes_any = true;
            } else {
                errors.push(format!(
                    "DESCRIBES must run from {DOCUMENT_ID} to an element, got {} DESCRIBES {}",
                    relationship.from, relationship.to
                ));
            }
        }
    }
    if !describes_any {
        errors.push(format!("no {DOCUMENT_ID} DESCRIBES relationship"));
    }
    errors
}

#[cfg(test)]
mod tests;
