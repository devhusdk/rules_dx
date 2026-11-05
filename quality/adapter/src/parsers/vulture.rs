use super::{check_output_size, code_name, located, point, FileFinding, ParseError};
use crate::{ColumnEncoding, Finding, ToolSeverity};
use std::collections::BTreeSet;

/// Vulture exits 3 when it reports unused code and 0 when it reports none.
const DEAD_CODE: i32 = 3;

/// `unused function 'f' (60% confidence)` becomes the rule `unused-function`.
fn vulture_rule(message: &str) -> Result<String, ParseError> {
    const TOOL: &str = "vulture";
    let kind = message.split(" '").next().unwrap_or_default();
    if kind.is_empty() || !message.ends_with("% confidence)") {
        return Err(ParseError::Shape {
            tool: TOOL,
            detail: format!("malformed message: {message:?}"),
        });
    }
    Ok(kind.split_whitespace().collect::<Vec<_>>().join("-"))
}

/// Vulture prints a path relative to its working directory when the file sits below it.
fn relative<'a>(line: &'a str, files: &[&'a str]) -> Option<(&'a str, &'a str)> {
    files.iter().copied().find_map(|file| {
        let rest = (0..file.len())
            .filter(|&at| file.is_char_boundary(at) && file[..at].ends_with(['/', '\\']))
            .find_map(|at| line.strip_prefix(&file[at..]))?;
        rest.strip_prefix(':').map(|rest| (file, rest))
    })
}

/// `path:line: message`; vulture reports no columns, so every finding starts at column 1.
fn vulture_line<'a>(
    line: &'a str,
    files: &[&'a str],
) -> Result<(&'a str, u64, &'a str), ParseError> {
    const TOOL: &str = "vulture";
    let (path, rest) = match located(TOOL, line, files) {
        Ok(found) => found,
        Err(err) => relative(line, files).ok_or(err)?,
    };
    let (row_text, message) = rest.split_once(": ").ok_or_else(|| ParseError::Shape {
        tool: TOOL,
        detail: format!("malformed finding: {line:?}"),
    })?;
    match row_text.parse::<u64>() {
        Ok(row) if row >= 1 => Ok((path, row, message)),
        _ => Err(ParseError::Shape {
            tool: TOOL,
            detail: format!("malformed line number: {line:?}"),
        }),
    }
}

/// Files whose symbols other code may use: findings in `exported` files are dropped for public
/// names, and every finding in a `keep` file is dropped. Both hold the names given to vulture.
#[derive(Debug, Default)]
pub struct UnusedRoots {
    pub exported: BTreeSet<String>,
    pub keep: BTreeSet<String>,
}

impl UnusedRoots {
    /// Drops every finding in a main, public or keep file, for tools that report whole
    /// definitions (exports, methods, files) rather than names.
    pub fn drop_rooted(&self, findings: &mut Vec<FileFinding>) {
        findings.retain(|found| {
            !self.exported.contains(&found.file) && !self.keep.contains(&found.file)
        });
    }
}

/// `unused function 'foo' (60% confidence)` names `foo`.
fn flagged_name(message: &str) -> &str {
    let rest = message.split_once('\'').map_or("", |(_, rest)| rest);
    rest.rsplit_once('\'').map_or("", |(name, _)| name)
}

/// Parses `vulture --min-confidence 60 <files>` over a whole closure and drops what the roots
/// keep alive: anything in a keep file, and public names in a main or public file.
pub fn parse_vulture_unused(
    stdout: &[u8],
    code: Option<i32>,
    files: &[&str],
    roots: &UnusedRoots,
) -> Result<Vec<FileFinding>, ParseError> {
    let mut findings = parse_vulture(stdout, code, files)?;
    findings.retain(|found| {
        if roots.keep.contains(&found.file) {
            return false;
        }
        let name = flagged_name(&found.finding.message);
        !(roots.exported.contains(&found.file) && !name.starts_with('_'))
    });
    Ok(findings)
}

/// Parses `vulture --min-confidence 60 <files>`: every finding is a warning, since dead-code
/// tools flag reflection, plugin and entry-point users too.
pub fn parse_vulture(
    stdout: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    const TOOL: &str = "vulture";
    check_output_size(TOOL, stdout)?;
    let text = std::str::from_utf8(stdout).map_err(|err| ParseError::Shape {
        tool: TOOL,
        detail: err.to_string(),
    })?;
    let mut findings = Vec::new();
    for line in text.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let (path, row, message) = vulture_line(line, files)?;
        let (start, end) = point(row, 1);
        findings.push(FileFinding {
            file: path.to_owned(),
            finding: Finding {
                tool_id: TOOL.to_owned(),
                rule_id: vulture_rule(message)?,
                message: message.to_owned(),
                severity: ToolSeverity::Warning,
                start,
                end,
                suggestions: Vec::new(),
                encoding: ColumnEncoding::Scalars,
                related: Vec::new(),
            },
        });
    }
    let clean = code == Some(0) && findings.is_empty();
    let dead_code = code == Some(DEAD_CODE) && !findings.is_empty();
    if clean || dead_code {
        return Ok(findings);
    }
    Err(ParseError::Shape {
        tool: TOOL,
        detail: format!("exit {} with {} findings", code_name(code), findings.len()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TextPosition;

    const VULTURE_DIRTY: &str = concat!(
        "/s/a.py:8: unused function 'unused_fn' (60% confidence)\n",
        "/s/a.py:1: unused import 'os' (90% confidence)\n",
    );

    #[test]
    fn vulture_reports_unused_code_as_info_findings() {
        let findings =
            parse_vulture(VULTURE_DIRTY.as_bytes(), Some(3), &["/s/a.py"]).expect("parsed");
        assert_eq!(findings.len(), 2);
        assert_eq!(findings[0].file, "/s/a.py");
        assert_eq!(findings[0].finding.tool_id, "vulture");
        assert_eq!(findings[0].finding.rule_id, "unused-function");
        assert_eq!(findings[0].finding.severity, ToolSeverity::Warning);
        assert_eq!(
            (findings[0].finding.start, findings[0].finding.end),
            (TextPosition { line: 8, column: 1 }, None)
        );
        assert_eq!(
            findings[0].finding.message,
            "unused function 'unused_fn' (60% confidence)"
        );
        assert_eq!(findings[1].finding.rule_id, "unused-import");
    }

    #[test]
    fn unused_roots_drop_public_names_and_keep_files() {
        let stdout = concat!(
            "/s/lib.py:1: unused function 'api' (60% confidence)\n",
            "/s/lib.py:5: unused function '_helper' (60% confidence)\n",
            "/s/main.py:2: unused function 'run' (60% confidence)\n",
            "/s/keep.py:3: unused function '_x' (60% confidence)\n",
            "/s/priv.py:4: unused function 'dead' (60% confidence)\n",
        );
        let files = ["/s/lib.py", "/s/main.py", "/s/keep.py", "/s/priv.py"];
        let roots = UnusedRoots {
            exported: ["/s/lib.py", "/s/main.py"].map(String::from).into(),
            keep: ["/s/keep.py"].map(String::from).into(),
        };
        let findings =
            parse_vulture_unused(stdout.as_bytes(), Some(3), &files, &roots).expect("parsed");
        let kept: Vec<_> = findings
            .iter()
            .map(|found| (found.file.as_str(), found.finding.start.line))
            .collect();
        assert_eq!(kept, vec![("/s/lib.py", 5), ("/s/priv.py", 4)]);
    }

    #[test]
    fn drop_rooted_removes_every_finding_in_a_root_file() {
        let stdout = concat!(
            "/s/lib.py:5: unused function 'api' (60% confidence)\n",
            "/s/keep.py:2: unused function 'kept' (60% confidence)\n",
            "/s/priv.py:4: unused function 'dead' (60% confidence)\n",
        );
        let files = ["/s/lib.py", "/s/keep.py", "/s/priv.py"];
        let roots = UnusedRoots {
            exported: ["/s/lib.py"].map(String::from).into(),
            keep: ["/s/keep.py"].map(String::from).into(),
        };
        let mut findings = parse_vulture(stdout.as_bytes(), Some(3), &files).expect("parsed");
        roots.drop_rooted(&mut findings);
        let kept: Vec<_> = findings.iter().map(|found| found.file.as_str()).collect();
        assert_eq!(kept, vec!["/s/priv.py"]);
    }

    #[test]
    fn unused_roots_still_check_the_exit_code_before_dropping() {
        let roots = UnusedRoots::default();
        assert!(parse_vulture_unused(b"", Some(3), &["/s/a.py"], &roots).is_err());
    }

    #[test]
    fn vulture_maps_paths_relative_to_its_working_directory() {
        let files = ["/s/pkg/a.py", "/s/b.py"];
        let stdout = "pkg/a.py:3: unused function 'f' (60% confidence)\n";
        let findings = parse_vulture(stdout.as_bytes(), Some(3), &files).expect("parsed");
        assert_eq!(findings[0].file, "/s/pkg/a.py");
    }

    #[test]
    fn vulture_multi_word_kinds_join_with_dashes() {
        let stdout = "/s/a.py:3: unreachable code after 'return' (100% confidence)\n";
        let findings = parse_vulture(stdout.as_bytes(), Some(3), &["/s/a.py"]).expect("parsed");
        assert_eq!(findings[0].finding.rule_id, "unreachable-code-after");
    }

    #[test]
    fn vulture_exit_codes_must_match_the_findings() {
        assert!(parse_vulture(b"", Some(0), &["/s/a.py"])
            .expect("parsed")
            .is_empty());
        assert!(parse_vulture(b"", Some(3), &["/s/a.py"]).is_err());
        assert!(parse_vulture(VULTURE_DIRTY.as_bytes(), Some(0), &["/s/a.py"]).is_err());
        assert!(parse_vulture(VULTURE_DIRTY.as_bytes(), Some(1), &["/s/a.py"]).is_err());
        assert!(parse_vulture(VULTURE_DIRTY.as_bytes(), None, &["/s/a.py"]).is_err());
    }

    #[test]
    fn vulture_syntax_errors_fail_closed() {
        let stdout = "/s/a.py:1: invalid syntax at \"def broken(:\"\n";
        assert!(parse_vulture(stdout.as_bytes(), Some(1), &["/s/a.py"]).is_err());
    }

    #[test]
    fn vulture_rejects_unknown_files_and_malformed_lines() {
        assert!(parse_vulture(VULTURE_DIRTY.as_bytes(), Some(3), &["/s/b.py"]).is_err());
        for line in [
            "/s/a.py:x: unused function 'f' (60% confidence)\n",
            "/s/a.py:0: unused function 'f' (60% confidence)\n",
            "/s/a.py:3: unused function 'f'\n",
            "/s/a.py:3 unused function 'f' (60% confidence)\n",
        ] {
            assert!(
                parse_vulture(line.as_bytes(), Some(3), &["/s/a.py"]).is_err(),
                "{line}"
            );
        }
        assert!(parse_vulture(&[0xff], Some(3), &["/s/a.py"]).is_err());
    }

    /// Vulture prints the paths it was given, so Windows drive paths come back unchanged.
    #[test]
    fn vulture_reports_windows_paths_as_given() {
        let checked = r"C:\tmp\dx-scratch-a\a.py";
        let stdout = format!("{checked}:2: unused variable 'x' (60% confidence)\n");
        let findings = parse_vulture(stdout.as_bytes(), Some(3), &[checked]).expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].file, checked);
        assert_eq!(findings[0].finding.rule_id, "unused-variable");
    }
}
