use super::{as_text, code_name, finding, known, missing, FileFinding, ParseError};
use crate::{ColumnEncoding, ToolSeverity};

const TOOL: &str = "dart_analyze";

/// Exit 0 is clean; 1, 2 and 3 report infos, warnings and errors in the machine format.
pub fn parse_dart_analyze(
    stdout: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    let Some(exit @ 0..=3) = code else {
        return Err(shape(format!("unexpected exit {}", code_name(code))));
    };
    let mut findings = Vec::new();
    let mut worst = 0;
    for line in as_text(TOOL, stdout)?.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let fields = machine_fields(line);
        if fields.len() != 8 {
            return Err(missing(TOOL, "diagnostic", line));
        }
        let (severity, rank) = match fields[0].as_str() {
            "ERROR" => (ToolSeverity::Error, 3),
            "WARNING" => (ToolSeverity::Warning, 2),
            "INFO" => (ToolSeverity::Info, 1),
            _ => return Err(missing(TOOL, "severity", line)),
        };
        worst = worst.max(rank);
        let file = known(TOOL, files, &fields[3])?;
        let line_no = fields[4].parse().map_err(|_| missing(TOOL, "line", line))?;
        let column = fields[5]
            .parse()
            .map_err(|_| missing(TOOL, "column", line))?;
        if fields[2].is_empty() || fields[7].is_empty() {
            return Err(missing(TOOL, "code or message", line));
        }
        let mut found = finding(
            TOOL,
            file,
            fields[2].to_ascii_lowercase(),
            fields[7].clone(),
            severity,
            line_no,
            column,
        );
        found.finding.encoding = ColumnEncoding::Utf16;
        findings.push(found);
    }
    if worst != exit {
        return Err(shape(format!(
            "exit {exit} does not match the worst reported severity"
        )));
    }
    Ok(findings)
}

/// The `|`-separated fields of one machine-format line, with `\|` and `\\` unescaped.
fn machine_fields(line: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut field = String::new();
    let mut chars = line.chars();
    while let Some(c) = chars.next() {
        match c {
            '\\' => match chars.next() {
                Some(next) => field.push(next),
                None => field.push('\\'),
            },
            '|' => fields.push(std::mem::take(&mut field)),
            _ => field.push(c),
        }
    }
    fields.push(field);
    fields
}

fn shape(detail: String) -> ParseError {
    ParseError::Shape { tool: TOOL, detail }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILE: &str = "/tmp/dx-scratch/app/lib/a.dart";

    /// `dart analyze --format=machine --fatal-infos` stdout, verbatim from Dart 3.13.2.
    const ERRORS: &str = "ERROR|COMPILE_TIME_ERROR|INVALID_ASSIGNMENT|/tmp/dx-scratch/app/lib/a.dart|1|36|3|A value of type 'String' can't be assigned to a variable of type 'int'.\nWARNING|STATIC_WARNING|UNUSED_LOCAL_VARIABLE|/tmp/dx-scratch/app/lib/a.dart|1|19|1|The value of the local variable 'x' isn't used.\n";

    #[test]
    fn dart_analyze_reports_machine_diagnostics() {
        let findings = parse_dart_analyze(ERRORS.as_bytes(), Some(3), &[FILE]).expect("parsed");
        assert_eq!(findings.len(), 2);
        assert_eq!(findings[0].file, FILE);
        assert_eq!(findings[0].finding.rule_id, "invalid_assignment");
        assert_eq!(findings[0].finding.severity, ToolSeverity::Error);
        assert_eq!(findings[0].finding.start.line, 1);
        assert_eq!(findings[0].finding.start.column, 36);
        assert_eq!(findings[0].finding.encoding, ColumnEncoding::Utf16);
        assert_eq!(findings[1].finding.severity, ToolSeverity::Warning);
        assert_eq!(findings[1].finding.rule_id, "unused_local_variable");
        assert!(parse_dart_analyze(b"", Some(0), &[FILE])
            .expect("parsed")
            .is_empty());
    }

    #[test]
    fn dart_analyze_reports_infos_as_fatal() {
        let info = "INFO|LINT|PREFER_FINAL_LOCALS|/tmp/dx-scratch/app/lib/a.dart|2|3|3|Local variables should be final.\n";
        let findings = parse_dart_analyze(info.as_bytes(), Some(1), &[FILE]).expect("parsed");
        assert_eq!(findings[0].finding.severity, ToolSeverity::Info);
        assert_eq!(findings[0].finding.rule_id, "prefer_final_locals");
        assert!(parse_dart_analyze(info.as_bytes(), Some(0), &[FILE]).is_err());
    }

    #[test]
    fn dart_analyze_unescapes_pipes_and_backslashes() {
        let line = "INFO|LINT|X|/tmp/dx-scratch/app/lib/a.dart|1|1|1|a \\| b \\\\ c\n";
        let findings = parse_dart_analyze(line.as_bytes(), Some(1), &[FILE]).expect("parsed");
        assert_eq!(findings[0].finding.message, "a | b \\ c");
    }

    #[test]
    fn dart_analyze_rejects_outputs_outside_the_grammar() {
        assert!(parse_dart_analyze(b"", Some(3), &[FILE]).is_err());
        assert!(parse_dart_analyze(ERRORS.as_bytes(), Some(2), &[FILE]).is_err());
        assert!(parse_dart_analyze(b"", Some(64), &[FILE]).is_err());
        assert!(parse_dart_analyze(b"", None, &[FILE]).is_err());
        assert!(parse_dart_analyze(b"ERROR|X|Y\n", Some(3), &[FILE]).is_err());
        assert!(parse_dart_analyze(
            b"FATAL|X|Y|/tmp/dx-scratch/app/lib/a.dart|1|1|1|m\n",
            Some(3),
            &[FILE]
        )
        .is_err());
        assert!(parse_dart_analyze(
            b"ERROR|X|Y|/tmp/dx-scratch/app/lib/a.dart|one|1|1|m\n",
            Some(3),
            &[FILE]
        )
        .is_err());
        assert!(parse_dart_analyze(&[0xff], Some(0), &[FILE]).is_err());
    }

    #[test]
    fn dart_analyze_cannot_report_a_file_outside_the_run() {
        let err = parse_dart_analyze(
            b"ERROR|X|Y|/tmp/dx-scratch/_external/args/lib/args.dart|1|1|1|m\n",
            Some(3),
            &[FILE],
        )
        .expect_err("unchecked file fails");
        assert!(matches!(err, ParseError::UnknownFile { .. }));
    }
}
