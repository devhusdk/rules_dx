use std::path::Path;

use serde_sarif::sarif::Location;

use super::sarif::{read_sarif, sarif_findings};
use super::{FileFinding, ParseError, check_output_size};
use crate::ColumnEncoding;

fn file_name(path: &str) -> Option<&str> {
    Path::new(path).file_name()?.to_str()
}

/// The checked file a package-relative SpotBugs URI names when its directory does not
/// follow the package.
fn package_relative(uri: &str, files: &[&str]) -> Option<String> {
    if uri.starts_with('/') || uri.contains(':') || uri.split('/').any(|part| part == "..") {
        return None;
    }
    let suffix = format!("/{uri}");
    let spelled = |file: &&str| file.replace('\\', "/");
    if files
        .iter()
        .any(|file| spelled(file) == uri || spelled(file).ends_with(&suffix))
    {
        return None;
    }
    let name = uri.rsplit('/').next()?;
    let mut hits = files.iter().filter(|file| file_name(file) == Some(name));
    let hit = hits.next()?;
    hits.next().is_none().then(|| (*hit).to_owned())
}

fn relocate(locations: Option<&mut Vec<Location>>, files: &[&str]) {
    for location in locations.into_iter().flatten() {
        let Some(artifact) = location
            .physical_location
            .as_mut()
            .and_then(|physical| physical.artifact_location.as_mut())
        else {
            continue;
        };
        if artifact.uri_base_id.is_some() {
            continue;
        }
        if let Some(file) = artifact
            .uri
            .as_deref()
            .and_then(|uri| package_relative(uri, files))
        {
            artifact.uri = Some(file);
        }
    }
}

pub fn parse_spotbugs(
    stdout: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    check_output_size("spotbugs", stdout)?;
    let mut log = read_sarif("spotbugs", stdout)?;
    for run in &mut log.runs {
        for result in run.results.iter_mut().flatten() {
            relocate(result.locations.as_mut(), files);
            relocate(result.related_locations.as_mut(), files);
        }
    }
    sarif_findings("spotbugs", &log, code, files, ColumnEncoding::Utf16)
}
#[cfg(test)]
mod tests {
    use super::*;

    const SARIF: &str = r#"{"version":"2.1.0","runs":[{"tool":{"driver":{"name":"SpotBugs"}},"results":[{"ruleId":"NP_NULL_ON_SOME_PATH","level":"warning","message":{"text":"Possible null pointer dereference."},"locations":[{"physicalLocation":{"artifactLocation":{"uri":"/s/Dirty.java"},"region":{"startLine":5,"startColumn":5,"endLine":5,"endColumn":9}}}]}]}]}"#;

    #[test]
    fn spotbugs_reports_sarif_ranges() {
        let findings =
            parse_spotbugs(SARIF.as_bytes(), Some(1), &["/s/Dirty.java"]).expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].finding.rule_id, "NP_NULL_ON_SOME_PATH");
        let clean =
            r#"{"version":"2.1.0","runs":[{"tool":{"driver":{"name":"SpotBugs"}},"results":[]}]}"#;
        let clean = parse_spotbugs(clean.as_bytes(), Some(0), &["/s/Dirty.java"]).expect("clean");
        assert!(clean.is_empty());
        let missing = r#"{"version":"2.1.0","runs":[{"tool":{"driver":{"name":"SpotBugs"}},"results":[],"invocations":[{"executionSuccessful":false,"toolConfigurationNotifications":[{"level":"error","message":{"text":"Classes needed for analysis were missing: [example.Dependency]"}}]}]}]}"#;
        let err = parse_spotbugs(missing.as_bytes(), Some(0), &["/s/Dirty.java"])
            .expect_err("incomplete");
        assert!(err.to_string().contains("example.Dependency"), "{err}");
        assert!(parse_spotbugs(b"not json", Some(1), &["/s/Dirty.java"]).is_err());
    }

    const PACKAGED: &str = r#"{"version":"2.1.0","runs":[{"tool":{"driver":{"name":"SpotBugs"}},"results":[{"ruleId":"R","message":{"text":"m"},"locations":[{"physicalLocation":{"artifactLocation":{"uri":"dx/a/Dirty.java"},"region":{"startLine":5}}}]}]}]}"#;

    #[test]
    fn a_package_relative_uri_resolves_to_the_checked_file_outside_the_package_dir() {
        let findings =
            parse_spotbugs(PACKAGED.as_bytes(), Some(1), &["/s/tests/Dirty.java"]).expect("parsed");
        assert_eq!(findings[0].file, "/s/tests/Dirty.java");
        let packaged = parse_spotbugs(
            PACKAGED.as_bytes(),
            Some(1),
            &["/s/tests/Dirty.java", "/s/src/dx/a/Dirty.java"],
        )
        .expect("packaged");
        assert_eq!(packaged[0].file, "/s/src/dx/a/Dirty.java");
        let err = parse_spotbugs(
            PACKAGED.as_bytes(),
            Some(1),
            &["/s/one/Dirty.java", "/s/two/Dirty.java"],
        )
        .expect_err("ambiguous");
        assert!(matches!(err, ParseError::UnknownFile { .. }), "{err}");
    }
}
