use super::{FileFinding, ParseError, check_output_size, code_name};
use crate::{ColumnEncoding, Finding, TextPosition, ToolSeverity};

const TOOL: &str = "avro_tools";
const EXCEPTION: &str = "Exception in thread \"main\" ";

fn number(text: &str) -> Option<u64> {
    let digits: String = text.chars().take_while(char::is_ascii_digit).collect();
    digits.parse().ok()
}

/// Drops the leading `a.b.ClassName: ` of a Java exception message.
fn strip_class(mut text: &str) -> &str {
    while let Some((head, rest)) = text.split_once(": ") {
        let class = !head.is_empty()
            && head.contains('.')
            && head
                .chars()
                .all(|c| c.is_alphanumeric() || c == '.' || c == '$');
        if !class {
            break;
        }
        text = rest;
    }
    text
}

/// Reads `line: N, column: M` (Jackson) from a location line.
fn jackson_position(line: &str) -> Option<(u64, u64)> {
    let rest = line.split_once("line: ")?.1;
    let row = number(rest)?;
    let column = rest
        .split_once("column: ")
        .and_then(|(_, tail)| number(tail));
    Some((row, column.unwrap_or(1)))
}

/// Reads the `line N:M` an IDL syntax error starts with.
fn idl_position(message: &str) -> Option<(u64, u64)> {
    let rest = message.strip_prefix("line ")?;
    let row = number(rest)?;
    let tail = rest.split_once(':')?.1;
    Some((row, number(tail)?))
}

/// Parses stderr of one `avro-tools` compile or idl2schemata run over one schema.
///
/// A failed run prints a Java exception. Its message becomes one error on the
/// first checked file. Exit 0 is clean. Exit 1 without an exception line, or
/// any other exit, is a shape error and never clean.
pub fn parse_avro(
    stdout: &[u8],
    stderr: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    check_output_size(TOOL, stdout)?;
    check_output_size(TOOL, stderr)?;
    if !matches!(code, Some(0 | 1)) {
        return Err(ParseError::shape(
            TOOL,
            format!("unexpected exit {}", code_name(code)),
        ));
    }
    let err =
        std::str::from_utf8(stderr).map_err(|err| ParseError::shape(TOOL, err.to_string()))?;
    let first = files
        .first()
        .ok_or_else(|| ParseError::shape(TOOL, "no checked file".to_owned()))?;
    let mut lines = err.lines();
    let mut found: Option<(String, u64, u64)> = None;
    while let Some(line) = lines.next() {
        let Some(text) = line.strip_prefix(EXCEPTION) else {
            continue;
        };
        let message = strip_class(text).trim();
        let mut position = idl_position(message);
        if position.is_none() {
            position = lines.next().and_then(jackson_position);
        }
        let (row, column) = position.unwrap_or((1, 1));
        found = Some((message.to_owned(), row, column));
        break;
    }
    match (code, found) {
        (Some(0), None) => Ok(Vec::new()),
        (Some(0), Some(_)) => Err(ParseError::shape(TOOL, "exception with exit 0".to_owned())),
        (_, None) => Err(ParseError::shape(
            TOOL,
            "exit 1 without an exception".to_owned(),
        )),
        (_, Some((message, row, column))) => {
            if message.is_empty() {
                return Err(ParseError::shape(
                    TOOL,
                    "exception without a message".to_owned(),
                ));
            }
            Ok(vec![FileFinding {
                file: (*first).to_owned(),
                finding: Finding {
                    tool_id: TOOL.to_owned(),
                    rule_id: "error".to_owned(),
                    message,
                    severity: ToolSeverity::Error,
                    start: TextPosition {
                        line: row.max(1),
                        column: column.max(1),
                    },
                    end: None,
                    suggestions: Vec::new(),
                    encoding: ColumnEncoding::Utf8,
                    related: Vec::new(),
                },
            }])
        }
    }
}

/// The paths an `.avdl` source imports, as written.
pub fn avdl_imports(source: &str) -> Vec<String> {
    let mut paths = Vec::new();
    for line in source.lines() {
        if line.trim_start().starts_with("//") {
            continue;
        }
        for part in line.split("import ").skip(1) {
            let Some((kind, rest)) = part.split_once(' ') else {
                continue;
            };
            if !matches!(kind, "idl" | "protocol" | "schema") {
                continue;
            }
            if let Some(path) = rest
                .trim_start()
                .strip_prefix('"')
                .and_then(|quoted| quoted.split_once('"'))
            {
                paths.push(path.0.to_owned());
            }
        }
    }
    paths
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILE: &str = "/s/a.avsc";

    fn parse(err: &str, code: Option<i32>) -> Result<Vec<FileFinding>, ParseError> {
        parse_avro(b"", err.as_bytes(), code, &[FILE])
    }

    #[test]
    fn clean_run_has_no_findings() {
        assert!(
            parse("Input files to compile:\n  /s/a.avsc\n", Some(0))
                .expect("clean")
                .is_empty()
        );
    }

    #[test]
    fn undefined_name_reports_at_the_file() {
        let found = parse(
            "Exception in thread \"main\" org.apache.avro.AvroTypeException: Undefined schema: x.Nope\n\tat a.B(B.java:1)\n",
            Some(1),
        )
        .expect("finding");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].file, FILE);
        assert_eq!(found[0].finding.message, "Undefined schema: x.Nope");
        assert_eq!(found[0].finding.start.line, 1);
    }

    #[test]
    fn json_error_uses_the_jackson_position() {
        let found = parse(
            "Exception in thread \"main\" org.apache.avro.SchemaParseException: com.fasterxml.jackson.core.JsonParseException: Unexpected character ('\"' (code 34)): was expecting comma\n at [Source: REDACTED; line: 2, column: 13]\n\tat x.Y(Y.java:1)\n",
            Some(1),
        )
        .expect("finding");
        assert_eq!(found[0].finding.start.line, 2);
        assert_eq!(found[0].finding.start.column, 13);
        assert!(found[0].finding.message.starts_with("Unexpected character"));
    }

    #[test]
    fn idl_error_uses_the_line_and_column() {
        let found = parse(
            "Exception in thread \"main\" org.apache.avro.SchemaParseException: line 2:31 mismatched input '}'\n",
            Some(1),
        )
        .expect("finding");
        assert_eq!(found[0].finding.start.line, 2);
        assert_eq!(found[0].finding.start.column, 31);
    }

    #[test]
    fn unreadable_failure_is_a_shape_error() {
        assert!(parse("garbage\n", Some(1)).is_err());
        assert!(parse("", Some(1)).is_err());
        assert!(parse("", None).is_err());
        assert!(parse("", Some(2)).is_err());
        assert!(parse("Exception in thread \"main\" a.B: x\n", Some(0)).is_err());
    }

    #[test]
    fn imports_are_read_from_the_source() {
        let source = "protocol P { import schema \"base.avsc\"; }\nimport idl \"../x.avdl\";\n// import idl \"no.avdl\";\n";
        assert_eq!(avdl_imports(source), vec!["base.avsc", "../x.avdl"]);
    }
}
