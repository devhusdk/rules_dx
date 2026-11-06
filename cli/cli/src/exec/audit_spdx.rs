use dx_audit::locks::{BAZEL_SET, LicensedPackage};

/// Builds the SPDX 2.3 document for one `dx license` run: a root package for the scope and one
/// package per resolved dependency, in the order given.
pub(super) fn license_spdx(
    scopes: &[String],
    packages: &[LicensedPackage],
    created: &str,
) -> Result<String, String> {
    let scope = if scopes.is_empty() {
        "workspace".to_owned()
    } else {
        scopes.join(",")
    };
    let mut document = dx_sbom::Document::new(
        &format!("dx-license-{scope}"),
        &format!("urn:dx:license:{}", id_part(&scope)),
        &["Tool: dx-license"],
    );
    if let Some(info) = document.creation_info.as_mut() {
        info.created = created.to_owned();
    }
    let root = "SPDXRef-Root";
    document.packages.push(dx_sbom::SpdxPackage {
        id: root.to_owned(),
        name: scope,
        download_location: dx_sbom::NOASSERTION.to_owned(),
        files_analyzed: Some(false),
        license_concluded: dx_sbom::NOASSERTION.to_owned(),
        license_declared: dx_sbom::NOASSERTION.to_owned(),
        ..dx_sbom::SpdxPackage::default()
    });
    document.relationships.push(dx_sbom::describes(root));
    for (index, package) in packages.iter().enumerate() {
        let id = format!("SPDXRef-Package-{index}-{}", id_part(&package.name));
        let license = license_value(&package.license);
        document.packages.push(dx_sbom::SpdxPackage {
            id: id.clone(),
            name: package.name.clone(),
            version: package.version.clone(),
            download_location: dx_sbom::NOASSERTION.to_owned(),
            files_analyzed: Some(false),
            license_concluded: license.clone(),
            license_declared: license,
            external_refs: vec![dx_sbom::SpdxExternalRef {
                category: "PACKAGE-MANAGER".to_owned(),
                ref_type: "purl".to_owned(),
                locator: purl(package),
            }],
            ..dx_sbom::SpdxPackage::default()
        });
        document.relationships.push(dx_sbom::contains(root, &id));
    }
    let errors = dx_sbom::reference_errors(&document);
    if !errors.is_empty() {
        return Err(errors.join("; "));
    }
    serde_json::to_string_pretty(&document).map_err(|error| error.to_string())
}

/// Formats `SOURCE_DATE_EPOCH` seconds, else the audit date, as the document creation time.
pub(super) fn created_time(epoch: Option<&str>, today: &str) -> String {
    epoch
        .and_then(|text| text.trim().parse::<i64>().ok())
        .and_then(|seconds| chrono::DateTime::from_timestamp(seconds, 0))
        .map(|time| time.format("%Y-%m-%dT%H:%M:%SZ").to_string())
        .unwrap_or_else(|| format!("{today}T00:00:00Z"))
}

pub(super) fn license_value(license: &str) -> String {
    if dx_sbom::license_error("license", license).is_some() || license == "UNKNOWN" {
        dx_sbom::NOASSERTION.to_owned()
    } else {
        license.to_owned()
    }
}

pub(super) fn id_part(text: &str) -> String {
    text.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' {
                c
            } else {
                '-'
            }
        })
        .collect()
}

pub(super) fn purl(package: &LicensedPackage) -> String {
    let kind = match package.set.as_str() {
        "cargo" => "cargo",
        "npm" => "npm",
        "go" => "golang",
        "maven" => "maven",
        "nuget" => "nuget",
        "ruby" => "gem",
        "uv" => "pypi",
        "composer" => "composer",
        "hex" => "hex",
        "pub" => "pub",
        "cpan" => "cpan",
        "conan" => "conan",
        BAZEL_SET => "generic",
        _ => "generic",
    };
    let name = if kind == "maven" {
        package.name.replacen(':', "/", 1)
    } else {
        package.name.clone()
    };
    let name = name.replace('@', "%40");
    format!("pkg:{kind}/{name}@{}", package.version)
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
    fn the_document_passes_the_schema_and_graph_rules() {
        let packages = vec![
            package("cargo", "serde", "1.0.100", "MIT"),
            package("npm", "@scope/left-pad", "1.3.0", "UNKNOWN"),
            package("maven", "junit:junit", "4.13.2", "EPL-1.0"),
        ];
        let text = license_spdx(&["//app".to_owned()], &packages, "2026-01-02T00:00:00Z")
            .expect("document");
        assert_eq!(dx_sbom_schema::spdx_errors(&text), Vec::<String>::new());
        let value: serde_json::Value = serde_json::from_str(&text).expect("json");
        assert_eq!(value["packages"].as_array().map(Vec::len), Some(4));
        assert_eq!(value["packages"][2]["licenseConcluded"], "NOASSERTION");
        assert_eq!(
            value["packages"][2]["externalRefs"][0]["referenceLocator"],
            "pkg:npm/%40scope/left-pad@1.3.0"
        );
        assert_eq!(
            value["packages"][3]["externalRefs"][0]["referenceLocator"],
            "pkg:maven/junit/junit@4.13.2"
        );
        assert_eq!(value["creationInfo"]["created"], "2026-01-02T00:00:00Z");
        assert_eq!(
            text,
            license_spdx(&["//app".to_owned()], &packages, "2026-01-02T00:00:00Z").expect("again")
        );
    }

    #[test]
    fn the_creation_time_prefers_source_date_epoch() {
        assert_eq!(
            created_time(Some("86400"), "2030-01-01"),
            "1970-01-02T00:00:00Z"
        );
        assert_eq!(
            created_time(Some("x"), "2030-01-01"),
            "2030-01-01T00:00:00Z"
        );
        assert_eq!(created_time(None, "2030-01-01"), "2030-01-01T00:00:00Z");
    }
}
