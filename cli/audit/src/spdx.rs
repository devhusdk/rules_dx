use packageurl::PackageUrl;
use serde::{Deserialize, Serialize};

pub const SPDX_VERSION: &str = "2.3";

pub const DATA_LICENSE: &str = "CC0-1.0";

pub const DESCRIBES: &str = "DESCRIBES";

pub const CONTAINS: &str = "CONTAINS";

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SpdxPackage {
    #[serde(rename = "SPDXID")]
    pub id: String,
    pub name: String,
    #[serde(rename = "versionInfo")]
    pub version: String,
    #[serde(rename = "licenseConcluded")]
    pub license: String,
    #[serde(rename = "licenseDeclared")]
    pub declared: String,
    #[serde(rename = "copyrightText")]
    pub copyright: String,
    #[serde(rename = "externalRefs")]
    pub external: Vec<SpdxExternalRef>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SpdxExternalRef {
    #[serde(rename = "referenceCategory")]
    pub category: String,
    #[serde(rename = "referenceType")]
    pub ref_type: String,
    #[serde(rename = "referenceLocator")]
    pub locator: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SpdxRelationship {
    #[serde(rename = "spdxElementId")]
    pub from: String,
    #[serde(rename = "relationshipType")]
    pub rel_type: String,
    #[serde(rename = "relatedSpdxElement")]
    pub to: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SpdxDocument {
    #[serde(rename = "spdxVersion")]
    pub version: String,
    #[serde(rename = "dataLicense")]
    pub data_license: String,
    #[serde(rename = "SPDXID")]
    pub id: String,
    pub name: String,
    #[serde(rename = "documentNamespace")]
    pub namespace: String,
    pub packages: Vec<SpdxPackage>,
    pub relationships: Vec<SpdxRelationship>,
}

pub fn package_url(set: &str, name: &str, version: &str) -> String {
    fn split_namespace(full: &str) -> (Option<&str>, &str) {
        match full.rsplit_once('/') {
            Some((ns, base)) if !ns.is_empty() && !base.is_empty() => (Some(ns), base),
            _ => (None, full),
        }
    }

    fn build(ty: &str, namespace: Option<&str>, name: &str, version: &str) -> Option<String> {
        let mut purl = PackageUrl::new(ty, name).ok()?;
        if let Some(ns) = namespace {
            purl.with_namespace(ns).ok()?;
        }
        purl.with_version(version).ok()?;
        Some(purl.to_string())
    }

    match set {
        "maven" => {
            if let Some((group, artifact)) = name.split_once(':') {
                if !group.is_empty() && !artifact.is_empty() && !artifact.contains('/') {
                    if let Some(text) = build("maven", Some(group), artifact, version) {
                        return text;
                    }
                } else if !artifact.is_empty() && artifact.contains('/') {
                    let (ns_extra, base) = split_namespace(artifact);
                    if !base.is_empty() {
                        let ns = match ns_extra {
                            Some(extra) => format!("{group}/{extra}"),
                            None => group.to_owned(),
                        };
                        if let Some(text) = build("maven", Some(&ns), base, version) {
                            return text;
                        }
                    }
                }
                format!("pkg:maven/{group}/{artifact}@{version}")
            } else {
                build("maven", None, name, version)
                    .unwrap_or_else(|| format!("pkg:maven/{name}@{version}"))
            }
        }
        "npm" => {
            let (ns, base) = split_namespace(name);
            build("npm", ns, base, version).unwrap_or_else(|| format!("pkg:npm/{name}@{version}"))
        }
        "cargo" => build("cargo", None, name, version)
            .unwrap_or_else(|| format!("pkg:cargo/{name}@{version}")),
        "nuget" => build("nuget", None, name, version)
            .unwrap_or_else(|| format!("pkg:nuget/{name}@{version}")),
        "go" => {
            let (ns, base) = split_namespace(name);
            build("golang", ns, base, version)
                .unwrap_or_else(|| format!("pkg:golang/{name}@{version}"))
        }
        _ => {
            let (ns, base) = split_namespace(name);
            build("generic", ns, base, version)
                .unwrap_or_else(|| format!("pkg:generic/{name}@{version}"))
        }
    }
}

/// Returns the namespace URI for one document's own content, so the same inputs render the same bytes.
pub fn document_namespace(
    roots: &[String],
    packages: &[SpdxPackage],
    contains: &[(String, String)],
) -> String {
    let mut sorted_roots = roots.to_vec();
    sorted_roots.sort();
    let mut sorted_packages = packages.to_vec();
    sorted_packages.sort_by(|a, b| a.id.cmp(&b.id));
    let mut sorted_contains = contains.to_vec();
    sorted_contains.sort();
    let identity =
        serde_json::to_vec(&(sorted_roots, sorted_packages, sorted_contains)).unwrap_or_default();
    format!(
        "https://dx-audit.local/{}",
        dx_digest::sha256_hex(&identity)
    )
}

pub fn render_spdx(
    roots: &[String],
    packages: &[SpdxPackage],
    contains: &[(String, String)],
    namespace: &str,
) -> String {
    let mut sorted_packages = packages.to_vec();
    sorted_packages.sort_by(|a, b| a.id.cmp(&b.id));
    let mut sorted_roots = roots.to_vec();
    sorted_roots.sort();
    let mut sorted_contains = contains.to_vec();
    sorted_contains.sort();
    let mut relationships = Vec::new();
    for root in &sorted_roots {
        relationships.push(SpdxRelationship {
            from: root.clone(),
            rel_type: DESCRIBES.to_owned(),
            to: "SPDXRef-DOCUMENT".to_owned(),
        });
    }
    for (from, to) in &sorted_contains {
        relationships.push(SpdxRelationship {
            from: from.clone(),
            rel_type: CONTAINS.to_owned(),
            to: to.clone(),
        });
    }
    let document = SpdxDocument {
        version: format!("SPDX-{SPDX_VERSION}"),
        data_license: DATA_LICENSE.to_owned(),
        id: "SPDXRef-DOCUMENT".to_owned(),
        name: "dx-audit-license".to_owned(),
        namespace: namespace.to_owned(),
        packages: sorted_packages,
        relationships,
    };
    serde_json::to_string_pretty(&document).unwrap_or_else(|_| "{}".to_owned())
}

pub fn spdx_package(
    set: &str,
    name: &str,
    version: &str,
    license: &str,
    index: usize,
) -> SpdxPackage {
    let locator = package_url(set, name, version);
    SpdxPackage {
        id: format!("SPDXRef-Package-{index}"),
        name: name.to_owned(),
        version: version.to_owned(),
        license: license.to_owned(),
        declared: license.to_owned(),
        copyright: "NOASSERTION".to_owned(),
        external: vec![SpdxExternalRef {
            category: "PACKAGE-MANAGER".to_owned(),
            ref_type: "purl".to_owned(),
            locator,
        }],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_NAMESPACE: &str = "https://invalid.test/dx-audit-1";

    #[test]
    fn package_urls_follow_purl_shapes() {
        assert_eq!(
            package_url("cargo", "serde", "1.0.100"),
            "pkg:cargo/serde@1.0.100"
        );
        assert_eq!(
            package_url("npm", "react", "18.2.0"),
            "pkg:npm/react@18.2.0"
        );
        assert_eq!(
            package_url("maven", "junit:junit", "4.13.2"),
            "pkg:maven/junit/junit@4.13.2"
        );
        assert_eq!(
            package_url("nuget", "FSharp.Core", "8.0.0"),
            "pkg:nuget/FSharp.Core@8.0.0"
        );
        assert_eq!(
            package_url("go", "example.com/hello", "1.0.0"),
            "pkg:golang/example.com/hello@1.0.0"
        );
    }

    #[test]
    fn package_urls_round_trip_through_packageurl() {
        use std::str::FromStr;
        let vectors: &[(&str, &str, &str, &str, Option<&str>, &str, &str, &str)] = &[
            (
                "cargo",
                "serde",
                "1.0.100",
                "cargo",
                None,
                "serde",
                "1.0.100",
                "pkg:cargo/serde@1.0.100",
            ),
            (
                "npm",
                "react",
                "18.2.0",
                "npm",
                None,
                "react",
                "18.2.0",
                "pkg:npm/react@18.2.0",
            ),
            (
                "maven",
                "junit:junit",
                "4.13.2",
                "maven",
                Some("junit"),
                "junit",
                "4.13.2",
                "pkg:maven/junit/junit@4.13.2",
            ),
            (
                "nuget",
                "FSharp.Core",
                "8.0.0",
                "nuget",
                None,
                "FSharp.Core",
                "8.0.0",
                "pkg:nuget/FSharp.Core@8.0.0",
            ),
            (
                "go",
                "example.com/hello",
                "1.0.0",
                "golang",
                Some("example.com"),
                "hello",
                "1.0.0",
                "pkg:golang/example.com/hello@1.0.0",
            ),
            (
                "go",
                "github.com/gorilla/mux",
                "1.8.0",
                "golang",
                Some("github.com/gorilla"),
                "mux",
                "1.8.0",
                "pkg:golang/github.com/gorilla/mux@1.8.0",
            ),
            (
                "npm",
                "@angular/animation",
                "12.3.1",
                "npm",
                Some("@angular"),
                "animation",
                "12.3.1",
                "pkg:npm/%40angular/animation@12.3.1",
            ),
            (
                "maven",
                "single",
                "1.0.0",
                "maven",
                None,
                "single",
                "1.0.0",
                "pkg:maven/single@1.0.0",
            ),
            (
                "other",
                "mytool",
                "2.0.0",
                "generic",
                None,
                "mytool",
                "2.0.0",
                "pkg:generic/mytool@2.0.0",
            ),
        ];
        for (set, name, version, ty, ns, base, ver, expected) in vectors {
            let text = package_url(set, name, version);
            assert_eq!(&text, expected, "stable purl for {set}:{name}@{version}");
            let parsed = PackageUrl::from_str(&text).expect("parse-back valid purl");
            assert_eq!(parsed.ty(), *ty);
            assert_eq!(parsed.namespace(), *ns);
            assert_eq!(parsed.name(), *base);
            assert_eq!(parsed.version(), Some(*ver));
            assert_eq!(parsed.to_string(), text, "Display idempotent");
        }
    }

    #[test]
    fn maven_package_urls_cover_grouped_slash_and_degenerate_shapes() {
        assert_eq!(
            package_url("maven", "com.example:foo/bar", "1.0.0"),
            "pkg:maven/com.example/foo/bar@1.0.0"
        );
        assert_eq!(
            package_url("maven", "g:a/", "1.0.0"),
            "pkg:maven/g/a%2F@1.0.0"
        );
        assert_eq!(package_url("maven", "g:", "1.0.0"), "pkg:maven/g/@1.0.0");
        assert_eq!(package_url("maven", ":a", "1.0.0"), "pkg:maven//a@1.0.0");
    }

    #[test]
    fn spdx_document_pins_version_package_urls_and_relations() {
        let roots = vec!["//services/payments:image".to_owned()];
        let packages = vec![
            spdx_package("cargo", "serde", "1.0.100", "MIT", 1),
            spdx_package("npm", "react", "18.2.0", "MIT", 2),
        ];
        let contains = vec![(
            "SPDXRef-Package-1".to_owned(),
            "SPDXRef-Package-2".to_owned(),
        )];
        let text = render_spdx(&roots, &packages, &contains, TEST_NAMESPACE);
        let value: serde_json::Value = serde_json::from_str(&text).expect("valid JSON");
        assert_eq!(value["spdxVersion"], serde_json::json!("SPDX-2.3"));
        assert_eq!(value["dataLicense"], serde_json::json!("CC0-1.0"));
        assert_eq!(value["SPDXID"], serde_json::json!("SPDXRef-DOCUMENT"));
        let pkgs = value["packages"].as_array().expect("packages");
        assert_eq!(pkgs.len(), 2);
        assert_eq!(
            pkgs[0]["externalRefs"][0]["referenceLocator"],
            serde_json::json!("pkg:cargo/serde@1.0.100")
        );
        let rels = value["relationships"].as_array().expect("relationships");
        assert_eq!(rels[0]["relationshipType"], serde_json::json!("DESCRIBES"));
        assert_eq!(
            rels[0]["spdxElementId"],
            serde_json::json!("//services/payments:image")
        );
        assert_eq!(rels[1]["relationshipType"], serde_json::json!("CONTAINS"));
    }

    #[test]
    fn the_document_namespace_names_the_content_not_the_process() {
        let roots = vec!["//b:two".to_owned(), "//a:one".to_owned()];
        let packages = vec![
            spdx_package("npm", "react", "18.2.0", "MIT", 2),
            spdx_package("cargo", "serde", "1.0.100", "MIT", 1),
        ];
        let contains = vec![
            ("//b:two".to_owned(), "//a:one".to_owned()),
            ("//a:one".to_owned(), "//b:two".to_owned()),
        ];
        let first = document_namespace(&roots, &packages, &contains);
        let shuffled_roots = vec!["//a:one".to_owned(), "//b:two".to_owned()];
        let mut shuffled_packages = packages.clone();
        shuffled_packages.reverse();
        let shuffled_contains = vec![
            ("//a:one".to_owned(), "//b:two".to_owned()),
            ("//b:two".to_owned(), "//a:one".to_owned()),
        ];
        assert_eq!(
            first,
            document_namespace(&shuffled_roots, &shuffled_packages, &shuffled_contains),
            "input order does not change the namespace"
        );
        let digest = first
            .strip_prefix("https://dx-audit.local/")
            .expect("namespace prefix");
        assert!(dx_digest::is_lower_hex(digest, 32), "{digest}");

        let other = document_namespace(&roots, &packages[1..], &contains);
        assert_ne!(first, other, "different content names a different document");
        let strip = |text: String| text.replace(&first, "NS").replace(&other, "NS");
        assert_eq!(
            strip(render_spdx(&roots, &packages, &contains, &first)),
            strip(render_spdx(&roots, &packages, &contains, &other)),
            "the namespace is the only thing that differs between two renders"
        );
    }

    #[test]
    fn spdx_render_is_deterministic_over_input_order() {
        let roots = vec!["//b:two".to_owned(), "//a:one".to_owned()];
        let packages = vec![
            spdx_package("npm", "react", "18.2.0", "MIT", 2),
            spdx_package("cargo", "serde", "1.0.100", "MIT", 1),
        ];
        let first = render_spdx(&roots, &packages, &[], "ns");
        let mut rev_roots = roots.clone();
        rev_roots.reverse();
        let mut rev_pkgs = packages.clone();
        rev_pkgs.reverse();
        let second = render_spdx(&rev_roots, &rev_pkgs, &[], "ns");
        assert_eq!(first, second);
    }

    #[test]
    fn frozen_shape_constants() {
        assert_eq!(SPDX_VERSION, "2.3");
        assert_eq!(DATA_LICENSE, "CC0-1.0");
        assert_eq!(DESCRIBES, "DESCRIBES");
        assert_eq!(CONTAINS, "CONTAINS");
    }

    #[test]
    fn spdx_golden_pins_full_document_shape() {
        let roots = vec!["//b:two".to_owned(), "//a:one".to_owned()];
        let packages = vec![
            spdx_package("maven", "junit:junit", "4.13.2", "EPL-1.0", 2),
            spdx_package("cargo", "serde", "1.0.100", "MIT", 1),
            spdx_package("go", "example.com/hello", "1.0.0", "NOASSERTION", 3),
        ];
        let text = render_spdx(&roots, &packages, &[], TEST_NAMESPACE);
        let value: serde_json::Value = serde_json::from_str(&text).expect("valid JSON");
        assert_eq!(value["spdxVersion"], serde_json::json!("SPDX-2.3"));
        assert_eq!(value["dataLicense"], serde_json::json!("CC0-1.0"));
        assert_eq!(value["SPDXID"], serde_json::json!("SPDXRef-DOCUMENT"));
        assert_eq!(value["name"], serde_json::json!("dx-audit-license"));
        assert_eq!(
            value["documentNamespace"],
            serde_json::json!(TEST_NAMESPACE)
        );
        let pkgs = value["packages"].as_array().expect("packages");
        assert_eq!(pkgs.len(), 3);
        assert_eq!(pkgs[0]["SPDXID"], serde_json::json!("SPDXRef-Package-1"));
        assert_eq!(pkgs[0]["name"], serde_json::json!("serde"));
        assert_eq!(pkgs[0]["versionInfo"], serde_json::json!("1.0.100"));
        assert_eq!(pkgs[0]["licenseConcluded"], serde_json::json!("MIT"));
        assert_eq!(pkgs[0]["licenseDeclared"], serde_json::json!("MIT"));
        assert_eq!(pkgs[0]["copyrightText"], serde_json::json!("NOASSERTION"));
        assert_eq!(
            pkgs[0]["externalRefs"],
            serde_json::json!([{
                "referenceCategory": "PACKAGE-MANAGER",
                "referenceType": "purl",
                "referenceLocator": "pkg:cargo/serde@1.0.100",
            }])
        );
        assert_eq!(
            pkgs[1]["externalRefs"][0]["referenceLocator"],
            serde_json::json!("pkg:maven/junit/junit@4.13.2")
        );
        assert_eq!(
            pkgs[2]["externalRefs"][0]["referenceLocator"],
            serde_json::json!("pkg:golang/example.com/hello@1.0.0")
        );
        let rels = value["relationships"].as_array().expect("relationships");
        assert_eq!(rels.len(), 2);
        assert_eq!(
            rels[0],
            serde_json::json!({
                "spdxElementId": "//a:one",
                "relationshipType": "DESCRIBES",
                "relatedSpdxElement": "SPDXRef-DOCUMENT",
            })
        );
        assert_eq!(
            rels[1],
            serde_json::json!({
                "spdxElementId": "//b:two",
                "relationshipType": "DESCRIBES",
                "relatedSpdxElement": "SPDXRef-DOCUMENT",
            })
        );
        let with_contains = render_spdx(
            &["//a:one".to_owned()],
            &packages[..2],
            &[(
                "SPDXRef-Package-2".to_owned(),
                "SPDXRef-Package-1".to_owned(),
            )],
            "ns",
        );
        let with_value: serde_json::Value =
            serde_json::from_str(&with_contains).expect("valid JSON");
        let with_rels = with_value["relationships"].as_array().expect("rels");
        assert_eq!(with_rels.len(), 2);
        assert_eq!(
            with_rels[0]["relationshipType"],
            serde_json::json!("DESCRIBES")
        );
        assert_eq!(
            with_rels[1]["relationshipType"],
            serde_json::json!("CONTAINS")
        );
    }
}
