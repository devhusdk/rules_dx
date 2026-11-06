use super::{FileFinding, ParseError, check_output_size, code_name, point, vulture::UnusedRoots};
use crate::{ColumnEncoding, Finding, ToolSeverity};
use serde::Deserialize;

const TOOL: &str = "deadcode";

#[derive(Deserialize)]
struct Package {
    #[serde(rename = "Funcs")]
    funcs: Vec<Function>,
}

#[derive(Deserialize)]
struct Function {
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "Position")]
    position: Position,
}

#[derive(Deserialize)]
struct Position {
    #[serde(rename = "File")]
    file: String,
    #[serde(rename = "Line")]
    line: u64,
    #[serde(rename = "Col")]
    col: u64,
}

/// `(*T).Method` and `pkg.Func` are exported when the last `.`-segment starts upper case.
fn is_exported(name: &str) -> bool {
    name.rsplit('.')
        .next()
        .and_then(|last| last.chars().next())
        .is_some_and(char::is_uppercase)
}

/// Parses `dx_deadcode`, which prints deadcode's `-json` with workspace file names. Every
/// finding is a warning, since reflection and plugin users call functions the graph cannot see.
/// Findings in a keep file are dropped, and so are exported names in a main or public file.
pub fn parse_deadcode(
    stdout: &[u8],
    code: Option<i32>,
    files: &[&str],
    roots: &UnusedRoots,
) -> Result<Vec<FileFinding>, ParseError> {
    check_output_size(TOOL, stdout)?;
    if code != Some(0) {
        return Err(ParseError::shape(TOOL, format!("exit {}", code_name(code))));
    }
    let packages: Vec<Package> =
        serde_json::from_slice(stdout).map_err(|err| ParseError::Json {
            tool: TOOL,
            detail: err.to_string(),
        })?;
    let mut findings = Vec::new();
    for function in packages.into_iter().flat_map(|package| package.funcs) {
        let position = function.position;
        if !files.contains(&position.file.as_str()) {
            return Err(ParseError::shape(
                TOOL,
                format!("finding in unknown file {:?}", position.file),
            ));
        }
        if position.line == 0 || position.col == 0 {
            return Err(ParseError::shape(
                TOOL,
                format!("finding {} carries a zero position", function.name),
            ));
        }
        if roots.keep.contains(&position.file)
            || (roots.exported.contains(&position.file) && is_exported(&function.name))
        {
            continue;
        }
        let (start, end) = point(position.line, position.col);
        findings.push(FileFinding {
            file: position.file,
            finding: Finding {
                tool_id: TOOL.to_owned(),
                rule_id: "unreachable-func".to_owned(),
                message: format!("unreachable func: {}", function.name),
                severity: ToolSeverity::Warning,
                start,
                end,
                suggestions: Vec::new(),
                encoding: ColumnEncoding::Scalars,
                related: Vec::new(),
            },
        });
    }
    Ok(findings)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TextPosition;

    const OUTPUT: &str = r#"[
      {"Name":"lib","Path":"x/lib","Funcs":[
        {"Name":"Uncalled","Position":{"File":"a/lib.go","Line":3,"Col":6}},
        {"Name":"helper","Position":{"File":"a/lib.go","Line":7,"Col":6}}]},
      {"Name":"main","Path":"x/app","Funcs":[
        {"Name":"(*T).Run","Position":{"File":"b/main.go","Line":4,"Col":13}},
        {"Name":"unused","Position":{"File":"b/main.go","Line":9,"Col":6}}]},
      {"Name":"keep","Path":"x/keep","Funcs":[
        {"Name":"unused","Position":{"File":"c/keep.go","Line":2,"Col":6}}]}
    ]"#;
    const FILES: [&str; 3] = ["a/lib.go", "b/main.go", "c/keep.go"];

    fn roots(exported: &[&str], keep: &[&str]) -> UnusedRoots {
        UnusedRoots {
            exported: exported.iter().map(|name| name.to_string()).collect(),
            keep: keep.iter().map(|name| name.to_string()).collect(),
        }
    }

    #[test]
    fn reports_unreachable_functions_as_warnings() {
        let found =
            parse_deadcode(OUTPUT.as_bytes(), Some(0), &FILES, &roots(&[], &[])).expect("parsed");
        assert_eq!(found.len(), 5);
        assert_eq!(found[0].file, "a/lib.go");
        assert_eq!(found[0].finding.tool_id, "deadcode");
        assert_eq!(found[0].finding.rule_id, "unreachable-func");
        assert_eq!(found[0].finding.severity, ToolSeverity::Warning);
        assert_eq!(found[0].finding.message, "unreachable func: Uncalled");
        assert_eq!(found[0].finding.start, TextPosition { line: 3, column: 6 });
        assert!(found[0].finding.suggestions.is_empty());
    }

    #[test]
    fn roots_drop_keep_files_and_exported_names_in_public_files() {
        let roots = roots(&["a/lib.go", "b/main.go"], &["c/keep.go"]);
        let found = parse_deadcode(OUTPUT.as_bytes(), Some(0), &FILES, &roots).expect("parsed");
        let kept: Vec<_> = found
            .iter()
            .map(|found| (found.file.as_str(), found.finding.start.line))
            .collect();
        assert_eq!(kept, vec![("a/lib.go", 7), ("b/main.go", 9)]);
    }

    #[test]
    fn exported_names_follow_the_last_segment() {
        assert!(is_exported("Uncalled"));
        assert!(is_exported("(*T).Run"));
        assert!(is_exported("T.Run"));
        assert!(!is_exported("run"));
        assert!(!is_exported("(*T).run"));
        assert!(!is_exported("T.Run.func1.helper"));
    }

    #[test]
    fn a_failed_run_is_an_error() {
        let none = roots(&[], &[]);
        assert!(parse_deadcode(b"", Some(1), &FILES, &none).is_err());
        assert!(parse_deadcode(b"[]", None, &FILES, &none).is_err());
    }

    #[test]
    fn clean_output_has_no_findings() {
        let none = roots(&[], &[]);
        assert!(
            parse_deadcode(b"[]", Some(0), &FILES, &none)
                .expect("parsed")
                .is_empty()
        );
    }

    #[test]
    fn unknown_files_and_malformed_output_fail_closed() {
        let none = roots(&[], &[]);
        assert!(parse_deadcode(OUTPUT.as_bytes(), Some(0), &["a/lib.go"], &none).is_err());
        assert!(parse_deadcode(b"not json", Some(0), &FILES, &none).is_err());
        let zero = r#"[{"Funcs":[{"Name":"F","Position":{"File":"a/lib.go","Line":0,"Col":1}}]}]"#;
        assert!(parse_deadcode(zero.as_bytes(), Some(0), &FILES, &none).is_err());
    }
}
