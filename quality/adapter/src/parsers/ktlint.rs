use crate::ColumnEncoding;
use serde_sarif::sarif::Sarif;

use super::sarif::{read_sarif, sarif_findings};
use super::{FileFinding, ParseError};

const SRCROOT: &str = "%SRCROOT%";

fn path_reference(path: &str) -> String {
    let mut out = String::with_capacity(path.len() + 2);
    if !(path.starts_with('/') || (cfg!(windows) && path.starts_with('\\'))) {
        out.push_str("./");
    }
    for c in path.chars() {
        match c {
            '%' => out.push_str("%25"),
            '?' => out.push_str("%3F"),
            '#' => out.push_str("%23"),
            '\\' if cfg!(windows) => out.push('/'),
            '\\' => out.push_str("%5C"),
            other => out.push(other),
        }
    }
    out
}

fn anchor_to_cwd(log: &mut Sarif) {
    for run in &mut log.runs {
        let locations = run
            .results
            .iter_mut()
            .flatten()
            .flat_map(|result| result.locations.iter_mut().flatten());
        for location in locations {
            let Some(artifact) = location
                .physical_location
                .as_mut()
                .and_then(|physical| physical.artifact_location.as_mut())
            else {
                continue;
            };
            if artifact.uri_base_id.as_deref() == Some(SRCROOT) {
                artifact.uri_base_id = None;
                artifact.uri = artifact.uri.as_deref().map(path_reference);
            }
        }
    }
}

pub fn parse_ktlint(
    stdout: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    let mut log = read_sarif("ktlint", stdout)?;
    anchor_to_cwd(&mut log);
    sarif_findings("ktlint", &log, code, files, ColumnEncoding::Utf16)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SARIF: &str = r#"{"version":"2.1.0","runs":[{"tool":{"driver":{"name":"ktlint"}},"results":[{"ruleId":"no-unused-imports","level":"error","message":{"text":"Unused import."},"locations":[{"physicalLocation":{"artifactLocation":{"uri":"/s/Dirty.kt"},"region":{"startLine":1,"startColumn":8,"endLine":1,"endColumn":15}}}]}]}]}"#;

    #[test]
    fn ktlint_reports_sarif_ranges() {
        let findings = parse_ktlint(SARIF.as_bytes(), Some(1), &["/s/Dirty.kt"]).expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].finding.rule_id, "no-unused-imports");
        let clean =
            r#"{"version":"2.1.0","runs":[{"tool":{"driver":{"name":"ktlint"}},"results":[]}]}"#;
        let clean = parse_ktlint(clean.as_bytes(), Some(0), &["/s/Dirty.kt"]).expect("clean");
        assert!(clean.is_empty());
        assert!(parse_ktlint(b"not json", Some(1), &["/s/Dirty.kt"]).is_err());
    }

    fn relative_log(uri: &str) -> String {
        format!(
            r#"{{"version":"2.1.0","runs":[{{"originalUriBaseIds":{{"%SRCROOT%":{{"uri":"file:///home/user/"}}}},"tool":{{"driver":{{"name":"ktlint"}}}},"results":[{{"ruleId":"standard:indent","level":"error","message":{{"text":"Unexpected indentation (0) (should be 4)"}},"locations":[{{"physicalLocation":{{"artifactLocation":{{"uri":{uri:?},"uriBaseId":"%SRCROOT%"}},"region":{{"startColumn":1,"startLine":4}}}}}}]}}]}}]}}"#
        )
    }

    #[test]
    fn ktlint_relative_uris_name_the_checked_file_under_the_working_directory() {
        let sep = std::path::MAIN_SEPARATOR;
        let checked = format!("{sep}scratch{sep}quality{sep}testdata{sep}real_dirty.kt");
        let files = [checked.as_str()];
        let stdout = relative_log("quality/testdata/real_dirty.kt");
        let findings = parse_ktlint(stdout.as_bytes(), Some(1), &files).expect("ktlint sarif");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].file, checked);
        assert_eq!(findings[0].finding.start.line, 4);
        assert!(
            super::super::sarif::parse_sarif(
                "sarif-test",
                stdout.as_bytes(),
                Some(1),
                &files,
                ColumnEncoding::Utf16
            )
            .is_err(),
            "other tools keep resolving against the declared base"
        );

        let literal = format!("{sep}scratch{sep}src{sep}100%20.kt");
        let decoded = format!("{sep}scratch{sep}src{sep}100 .kt");
        let both = [literal.as_str(), decoded.as_str()];
        let stdout = relative_log("src/100%20.kt");
        let findings = parse_ktlint(stdout.as_bytes(), Some(1), &both).expect("raw path");
        assert_eq!(
            findings[0].file, literal,
            "ktlint paths are not percent-encoded"
        );

        let stdout = relative_log("src/Other.kt");
        assert!(matches!(
            parse_ktlint(stdout.as_bytes(), Some(1), &files),
            Err(ParseError::UnknownFile { .. })
        ));
    }
}
