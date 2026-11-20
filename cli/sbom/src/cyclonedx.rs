//! The `CycloneDX` 1.6 JSON data model dx writes next to the SPDX one.

use std::collections::BTreeSet;
use std::sync::LazyLock;

use serde::{Deserialize, Serialize};

pub const BOM_FORMAT: &str = "CycloneDX";

pub const SPEC_VERSION: &str = "1.6";

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CdxLicense {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub id: String,
}

/// One `licenses[]` entry: an SPDX id under `license`, or an SPDX `expression`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CdxLicenseChoice {
    License { license: CdxLicense },
    Expression { expression: String },
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CdxComponent {
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(rename = "bom-ref")]
    pub bom_ref: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub version: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub purl: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub licenses: Vec<CdxLicenseChoice>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CdxTool {
    #[serde(rename = "type")]
    pub kind: String,
    pub name: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CdxTools {
    pub components: Vec<CdxTool>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CdxMetadata {
    pub timestamp: String,
    pub tools: CdxTools,
    pub component: CdxComponent,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CdxDependency {
    #[serde(rename = "ref")]
    pub reference: String,
    #[serde(rename = "dependsOn", default, skip_serializing_if = "Vec::is_empty")]
    pub depends_on: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CycloneDxDocument {
    #[serde(rename = "bomFormat")]
    pub bom_format: String,
    #[serde(rename = "specVersion")]
    pub spec_version: String,
    pub version: u32,
    pub metadata: CdxMetadata,
    pub components: Vec<CdxComponent>,
    pub dependencies: Vec<CdxDependency>,
}

impl CycloneDxDocument {
    /// Returns a document whose root `application` component is `root`, with no dependencies yet.
    #[must_use]
    pub fn new(root_name: &str, timestamp: &str, tool: &str) -> Self {
        Self {
            bom_format: BOM_FORMAT.to_owned(),
            spec_version: SPEC_VERSION.to_owned(),
            version: 1,
            metadata: CdxMetadata {
                timestamp: timestamp.to_owned(),
                tools: CdxTools {
                    components: vec![CdxTool {
                        kind: "application".to_owned(),
                        name: tool.to_owned(),
                    }],
                },
                component: CdxComponent {
                    kind: "application".to_owned(),
                    bom_ref: ROOT_REF.to_owned(),
                    name: root_name.to_owned(),
                    ..CdxComponent::default()
                },
            },
            ..Self::default()
        }
    }

    /// Adds a library component and lists it under the root in `dependencies`.
    pub fn add_library(&mut self, component: CdxComponent) {
        let root = &self.metadata.component.bom_ref;
        if !self
            .dependencies
            .iter()
            .any(|entry| &entry.reference == root)
        {
            self.dependencies.insert(
                0,
                CdxDependency {
                    reference: root.clone(),
                    depends_on: Vec::new(),
                },
            );
        }
        if let Some(entry) = self
            .dependencies
            .iter_mut()
            .find(|entry| entry.reference == self.metadata.component.bom_ref)
        {
            entry.depends_on.push(component.bom_ref.clone());
        }
        self.dependencies.push(CdxDependency {
            reference: component.bom_ref.clone(),
            depends_on: Vec::new(),
        });
        self.components.push(component);
    }
}

pub const ROOT_REF: &str = "root";

/// The `CycloneDX` `licenses` value for an SPDX expression: one id, an expression, or nothing.
#[must_use]
pub fn license_choices(expression: &str) -> Vec<CdxLicenseChoice> {
    let expression = expression.trim();
    if crate::license_error("license", expression).is_some()
        || matches!(
            expression,
            "" | crate::NOASSERTION | crate::NONE | "UNKNOWN"
        )
    {
        return Vec::new();
    }
    match spdx::license_id(expression) {
        Some(id) if SCHEMA_LICENSE_IDS.contains(id.name) => vec![CdxLicenseChoice::License {
            license: CdxLicense {
                id: id.name.to_owned(),
            },
        }],
        _ => vec![CdxLicenseChoice::Expression {
            expression: expression.to_owned(),
        }],
    }
}

#[derive(Deserialize)]
struct SpdxEnum {
    #[serde(rename = "enum")]
    ids: BTreeSet<String>,
}

// The `spdx` crate knows newer ids than the 1.6 schema enum, which rejects them under
// `license.id`. An unreadable schema leaves the set empty, so every id falls back to `expression`.
static SCHEMA_LICENSE_IDS: LazyLock<BTreeSet<String>> = LazyLock::new(|| {
    serde_json::from_str::<SpdxEnum>(include_str!("../schema/spdx.schema.json"))
        .map(|schema| schema.ids)
        .unwrap_or_default()
});

/// Graph rules the schema cannot express: non-empty unique bom-refs and known, unrepeated dependency refs.
#[must_use]
pub fn reference_errors(document: &CycloneDxDocument) -> Vec<String> {
    let mut errors = Vec::new();
    let mut known = BTreeSet::new();
    for component in std::iter::once(&document.metadata.component).chain(&document.components) {
        if component.bom_ref.is_empty() {
            errors.push(format!(
                "component '{}' has an empty bom-ref",
                component.name
            ));
        } else if !known.insert(component.bom_ref.as_str()) {
            errors.push(format!("bom-ref '{}' is declared twice", component.bom_ref));
        }
    }
    let mut listed = BTreeSet::new();
    for dependency in &document.dependencies {
        if !listed.insert(dependency.reference.as_str()) {
            errors.push(format!(
                "dependency '{}' is listed twice",
                dependency.reference
            ));
        }
        let mut targets = BTreeSet::new();
        for target in &dependency.depends_on {
            if !targets.insert(target.as_str()) {
                errors.push(format!(
                    "dependency '{}' depends on '{target}' twice",
                    dependency.reference
                ));
            }
        }
        for end in std::iter::once(&dependency.reference).chain(&dependency.depends_on) {
            if !known.contains(end.as_str()) {
                errors.push(format!("dependency names undeclared bom-ref '{end}'"));
            }
        }
    }
    errors
}
