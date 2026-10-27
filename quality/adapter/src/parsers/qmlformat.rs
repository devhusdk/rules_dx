use super::{as_text, code_name, unformatted_at, FileFinding, ParseError};
use crate::ColumnEncoding;

pub fn parse_qmlformat(
    stdout: &[u8],
    code: Option<i32>,
    input: &[u8],
    file: &str,
) -> Result<Vec<FileFinding>, ParseError> {
    const TOOL: &str = "qmlformat";
    as_text(TOOL, stdout)?;
    if code != Some(0) {
        return Err(ParseError::Shape {
            tool: TOOL,
            detail: format!("exit {} (want 0)", code_name(code)),
        });
    }
    if stdout == input {
        return Ok(Vec::new());
    }
    Ok(vec![unformatted_at(
        TOOL,
        file,
        1,
        1,
        ColumnEncoding::Scalars,
    )])
}

#[cfg(test)]
mod tests {
    use super::*;

    const FORMATTED: &[u8] = b"import QtQuick\n\nItem {\n}\n";

    #[test]
    fn qmlformat_reports_a_file_whose_formatted_output_differs() {
        let findings = parse_qmlformat(FORMATTED, Some(0), b"import QtQuick\nItem {}\n", "a.qml")
            .expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].file, "a.qml");
        let clean = parse_qmlformat(FORMATTED, Some(0), FORMATTED, "a.qml").expect("parsed");
        assert!(clean.is_empty());
        assert!(parse_qmlformat(FORMATTED, Some(1), FORMATTED, "a.qml").is_err());
        assert!(parse_qmlformat(FORMATTED, None, FORMATTED, "a.qml").is_err());
        assert!(parse_qmlformat(&[0xff], Some(0), b"", "a.qml").is_err());
    }
}
