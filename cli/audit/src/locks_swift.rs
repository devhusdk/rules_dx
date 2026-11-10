use crate::vuln::LockedPackage;
use dx_error::DxError;

const SWIFT_SET: &str = "swifturl";

/// Reads the `pins` of a `Package.resolved` v2 or v3; pins with only a branch or revision have no
/// version to match and are skipped. Names follow OSV's SwiftURL: host and path, no scheme or `.git`.
pub fn parse_package_resolved(text: &str) -> Result<Vec<LockedPackage>, DxError> {
    let root: serde_json::Value = serde_json::from_str(text)
        .map_err(|error| DxError::failure(format!("invalid Package.resolved: {error}")))?;
    match root.get("version").and_then(serde_json::Value::as_u64) {
        Some(2 | 3) => {}
        Some(other) => {
            return Err(DxError::failure(format!(
                "unsupported Package.resolved version {other}; resolve with Swift 5.6 or newer (version 2 or 3)"
            )));
        }
        None => {
            return Err(DxError::failure(
                "invalid Package.resolved: missing version",
            ));
        }
    }
    let pins = root
        .get("pins")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| DxError::failure("invalid Package.resolved: pins is not a list"))?;
    let mut out = Vec::new();
    for pin in pins {
        let location = pin
            .get("location")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| DxError::failure("invalid Package.resolved: pin without location"))?;
        let Some(version) = pin
            .get("state")
            .and_then(|state| state.get("version"))
            .and_then(serde_json::Value::as_str)
        else {
            continue;
        };
        out.push(LockedPackage::new(
            SWIFT_SET,
            swift_url_name(location),
            version,
        ));
    }
    out.sort_by(|a, b| (&a.name, &a.version).cmp(&(&b.name, &b.version)));
    out.dedup_by(|b, a| a.name == b.name && a.version == b.version);
    Ok(out)
}

fn swift_url_name(location: &str) -> String {
    let url = location.trim().trim_end_matches('/');
    let url = url.strip_suffix(".git").unwrap_or(url);
    let url = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .or_else(|| url.strip_prefix("ssh://"))
        .unwrap_or(url);
    let url = url
        .strip_prefix("git@")
        .map_or_else(|| url.to_owned(), |scp| scp.replacen(':', "/", 1));
    url.split_once('@')
        .map_or(url.clone(), |(_, rest)| rest.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    const V2: &str = r#"{"pins":[
        {"identity":"a","kind":"remoteSourceControl","location":"https://github.com/apple/swift-argument-parser.git","state":{"revision":"x","version":"1.8.2"}},
        {"identity":"b","kind":"remoteSourceControl","location":"https://github.com/vapor/vapor","state":{"revision":"y","version":"4.50.0"}},
        {"identity":"c","kind":"remoteSourceControl","location":"https://github.com/o/branchy","state":{"branch":"main","revision":"z"}},
        {"identity":"d","kind":"remoteSourceControl","location":"https://github.com/o/rev","state":{"revision":"z"}}
    ],"version":2}"#;

    fn pairs(packages: &[LockedPackage]) -> Vec<(&str, &str)> {
        packages
            .iter()
            .map(|p| (p.name.as_str(), p.version.as_str()))
            .collect()
    }

    #[test]
    fn v2_strips_git_suffix_and_skips_branch_and_revision_pins() {
        let packages = parse_package_resolved(V2).expect("parses");
        assert_eq!(
            pairs(&packages),
            [
                ("github.com/apple/swift-argument-parser", "1.8.2"),
                ("github.com/vapor/vapor", "4.50.0")
            ]
        );
        assert!(packages.iter().all(|p| p.set == "swifturl"));
    }

    #[test]
    fn v3_reads_the_same_pins() {
        let text = V2.replace("\"version\":2}", "\"originHash\":\"abc\",\"version\":3}");
        assert_eq!(parse_package_resolved(&text).expect("parses").len(), 2);
    }

    #[test]
    fn v1_is_refused() {
        let error =
            parse_package_resolved(r#"{"object":{"pins":[]},"version":1}"#).expect_err("v1");
        assert!(error.message().contains("version 1"), "{error}");
    }

    #[test]
    fn scp_and_credentialed_locations_normalise() {
        assert_eq!(swift_url_name("git@github.com:o/r.git"), "github.com/o/r");
        assert_eq!(
            swift_url_name("https://u@github.com/o/r/"),
            "github.com/o/r"
        );
    }
}
