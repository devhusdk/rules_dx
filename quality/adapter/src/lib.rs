#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

use line_index::LineIndex;
use quality_result::proto::{Diagnostic, Severity};

pub mod commands;
pub mod exec;
pub mod launch;
pub mod parsers;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolSeverity {
    Warning,
    Error,
    Info,
}

impl ToolSeverity {
    pub fn proto(self) -> i32 {
        match self {
            ToolSeverity::Warning => Severity::Warning as i32,
            ToolSeverity::Error => Severity::Error as i32,
            ToolSeverity::Info => Severity::Info as i32,
        }
    }
}

pub const LINE_END: u64 = u64::MAX;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextPosition {
    pub line: u64,
    pub column: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Suggestion {
    pub start: u64,
    pub end: u64,
    pub replacement: Vec<u8>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ColumnEncoding {
    #[default]
    Scalars,
    Utf16,
    Utf8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub tool_id: String,
    pub rule_id: String,
    pub message: String,
    pub severity: ToolSeverity,
    pub start: TextPosition,
    pub end: Option<TextPosition>,
    pub suggestions: Vec<Suggestion>,
    pub encoding: ColumnEncoding,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PlaceError {
    #[error("position {line}:{column} is outside the checked bytes")]
    UnmappablePosition { line: u64, column: u64 },
    #[error("end position precedes start position")]
    InvertedRange,
}

pub fn line_col_to_byte(text: &str, line: u64, column: u64) -> Option<u64> {
    line_col_to_byte_in(text, line, column, ColumnEncoding::Scalars)
}

pub fn line_col_to_byte_in(
    text: &str,
    line: u64,
    column: u64,
    encoding: ColumnEncoding,
) -> Option<u64> {
    if line == 0 || column == 0 {
        return None;
    }
    if text.len() >= u32::MAX as usize {
        return legacy_line_col_to_byte(text, line, column, encoding);
    }
    let line_index = u32::try_from(line - 1).ok()?;
    let range = LineIndex::new(text).line(line_index)?;
    let start = u32::from(range.start()) as usize;
    let end = u32::from(range.end()) as usize;
    let line_text = text.get(start..end)?;
    let body = line_text.strip_suffix('\n').unwrap_or(line_text);
    Some((start + column_to_offset(body, column - 1, encoding)?) as u64)
}

fn column_to_offset(body: &str, units: u64, encoding: ColumnEncoding) -> Option<usize> {
    if encoding == ColumnEncoding::Utf8 {
        let offset = usize::try_from(units).ok()?;
        return body.is_char_boundary(offset).then_some(offset);
    }
    let mut seen = 0u64;
    for (offset, ch) in body.char_indices() {
        if seen == units {
            return Some(offset);
        }
        if seen > units {
            return None;
        }
        seen += match encoding {
            ColumnEncoding::Utf16 => ch.len_utf16() as u64,
            _ => 1,
        };
    }
    (seen == units).then_some(body.len())
}

pub fn line_end_to_byte(text: &str, line: u64) -> Option<u64> {
    let index = usize::try_from(line.checked_sub(1)?).ok()?;
    let mut offset = 0usize;
    for (number, content) in text.split_inclusive('\n').enumerate() {
        if number == index {
            let body = content.strip_suffix('\n').unwrap_or(content);
            let body = body.strip_suffix('\r').unwrap_or(body);
            return Some((offset + body.len()) as u64);
        }
        offset += content.len();
    }
    (index == 0 && text.is_empty()).then_some(0)
}

fn legacy_line_col_to_byte(
    text: &str,
    line: u64,
    column: u64,
    encoding: ColumnEncoding,
) -> Option<u64> {
    let mut offset = 0u64;
    for (index, content) in text.split_inclusive('\n').enumerate() {
        if index as u64 + 1 == line {
            let body = content.strip_suffix('\n').unwrap_or(content);
            return Some(offset + column_to_offset(body, column - 1, encoding)? as u64);
        }
        offset += content.len() as u64;
    }
    None
}

pub fn place_finding(finding: &Finding, path: &str, text: &str) -> Result<Diagnostic, PlaceError> {
    let start = line_col_to_byte_in(
        text,
        finding.start.line,
        finding.start.column,
        finding.encoding,
    )
    .ok_or(PlaceError::UnmappablePosition {
        line: finding.start.line,
        column: finding.start.column,
    })?;
    let end = match finding.end {
        Some(position) => {
            let byte = if position.column == LINE_END {
                line_end_to_byte(text, position.line)
            } else {
                line_col_to_byte_in(text, position.line, position.column, finding.encoding)
            };
            byte.ok_or(PlaceError::UnmappablePosition {
                line: position.line,
                column: position.column,
            })?
        }
        None => start,
    };
    if end < start {
        return Err(PlaceError::InvertedRange);
    }
    Ok(Diagnostic {
        severity: finding.severity.proto(),
        message: finding.message.clone(),
        tool_id: finding.tool_id.clone(),
        rule_id: finding.rule_id.clone(),
        path: path.to_owned(),
        start_byte: Some(start),
        end_byte: Some(end),
        fixable: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const CAFE: &str = "fn main() {\n    let caf\u{e9} = 1;\n}\n";

    #[test]
    fn line_col_covers_ascii_positions() {
        assert_eq!(line_col_to_byte("ab\ncd\n", 1, 1), Some(0));
        assert_eq!(line_col_to_byte("ab\ncd\n", 1, 3), Some(2));
        assert_eq!(line_col_to_byte("ab\ncd\n", 2, 1), Some(3));
        assert_eq!(line_col_to_byte("ab\ncd\n", 2, 3), Some(5));
    }

    #[test]
    fn line_col_counts_characters_not_bytes() {
        assert_eq!(line_col_to_byte(CAFE, 2, 9), Some(20));
        assert_eq!(line_col_to_byte(CAFE, 2, 12), Some(23));
    }

    #[test]
    fn line_col_rejects_zero_and_out_of_range() {
        assert_eq!(line_col_to_byte(CAFE, 0, 1), None);
        assert_eq!(line_col_to_byte(CAFE, 1, 0), None);
        assert_eq!(line_col_to_byte(CAFE, 5, 1), None);
        assert_eq!(line_col_to_byte(CAFE, 2, 99), None);
        assert_eq!(line_col_to_byte(CAFE, 4, 2), None);
    }

    #[test]
    fn line_col_covers_empty_trailing_crlf_and_astral() {
        assert_eq!(line_col_to_byte("", 1, 1), Some(0));
        assert_eq!(line_col_to_byte("", 1, 2), None);
        assert_eq!(line_col_to_byte("", 2, 1), None);
        assert_eq!(line_col_to_byte("ab\n", 1, 1), Some(0));
        assert_eq!(line_col_to_byte("ab\n", 1, 3), Some(2));
        assert_eq!(line_col_to_byte("ab\n", 1, 4), None);
        assert_eq!(line_col_to_byte("ab\n", 2, 1), Some(3));
        assert_eq!(line_col_to_byte("ab\n", 2, 2), None);
        assert_eq!(line_col_to_byte(CAFE, 4, 1), Some(CAFE.len() as u64));
        let crlf = "a\r\nb";
        assert_eq!(line_col_to_byte(crlf, 1, 1), Some(0));
        assert_eq!(line_col_to_byte(crlf, 1, 2), Some(1));
        assert_eq!(line_col_to_byte(crlf, 1, 3), Some(2));
        assert_eq!(line_col_to_byte(crlf, 1, 4), None);
        assert_eq!(line_col_to_byte(crlf, 2, 1), Some(3));
        assert_eq!(line_col_to_byte(crlf, 2, 2), Some(4));
        let astral = "a\u{1f496}b";
        assert_eq!(line_col_to_byte(astral, 1, 1), Some(0));
        assert_eq!(line_col_to_byte(astral, 1, 2), Some(1));
        assert_eq!(line_col_to_byte(astral, 1, 3), Some(5));
        assert_eq!(line_col_to_byte(astral, 1, 4), Some(6));
        assert_eq!(line_col_to_byte(astral, 1, 5), None);
    }

    #[test]
    fn place_finding_builds_point_and_covering_ranges() {
        let point = Finding {
            tool_id: "rustfmt".to_owned(),
            rule_id: String::new(),
            message: "file is not formatted".to_owned(),
            severity: ToolSeverity::Warning,
            start: TextPosition { line: 2, column: 1 },
            end: None,
            suggestions: Vec::new(),
            encoding: ColumnEncoding::Scalars,
        };
        let placed = place_finding(&point, "src/main.rs", CAFE).expect("mappable");
        assert_eq!(placed.start_byte, Some(12));
        assert_eq!(placed.end_byte, Some(12));
        assert_eq!(placed.severity, Severity::Warning as i32);
        assert!(!placed.fixable);

        let covering = Finding {
            end: Some(TextPosition {
                line: 2,
                column: 12,
            }),
            ..point
        };
        let placed = place_finding(&covering, "src/main.rs", CAFE).expect("mappable");
        assert_eq!((placed.start_byte, placed.end_byte), (Some(12), Some(23)));
    }

    #[test]
    fn line_end_resolves_from_the_text() {
        assert_eq!(line_end_to_byte("ab\ncd\n", 1), Some(2));
        assert_eq!(line_end_to_byte("ab\ncd\n", 2), Some(5));
        assert_eq!(line_end_to_byte("ab\ncd", 2), Some(5));
        assert_eq!(line_end_to_byte("a\r\nb\r\n", 1), Some(1));
        assert_eq!(line_end_to_byte("a\r\nb\r\n", 2), Some(4));
        assert_eq!(line_end_to_byte("", 1), Some(0));
        assert_eq!(line_end_to_byte("ab\n", 2), None);
        assert_eq!(line_end_to_byte("ab\n", 0), None);
        let finding = Finding {
            tool_id: "t".to_owned(),
            rule_id: String::new(),
            message: "m".to_owned(),
            severity: ToolSeverity::Warning,
            start: TextPosition { line: 1, column: 1 },
            end: Some(TextPosition {
                line: 2,
                column: LINE_END,
            }),
            suggestions: Vec::new(),
            encoding: ColumnEncoding::Scalars,
        };
        let placed = place_finding(&finding, "a", "ab\r\ncd\n").expect("placed");
        assert_eq!((placed.start_byte, placed.end_byte), (Some(0), Some(6)));
        assert!(place_finding(&finding, "a", "ab\n").is_err());
    }

    #[test]
    fn severity_and_error_display_are_stable() {
        assert_eq!(ToolSeverity::Warning.proto(), Severity::Warning as i32);
        assert_eq!(ToolSeverity::Error.proto(), Severity::Error as i32);
        assert_eq!(ToolSeverity::Info.proto(), Severity::Info as i32);
        assert_eq!(
            PlaceError::UnmappablePosition { line: 9, column: 1 }.to_string(),
            "position 9:1 is outside the checked bytes"
        );
        assert_eq!(
            PlaceError::InvertedRange.to_string(),
            "end position precedes start position"
        );
    }

    #[test]
    fn place_finding_rejects_bad_positions() {
        let base = Finding {
            tool_id: "taplo".to_owned(),
            rule_id: String::new(),
            message: "expected value".to_owned(),
            severity: ToolSeverity::Error,
            start: TextPosition { line: 9, column: 1 },
            end: None,
            suggestions: Vec::new(),
            encoding: ColumnEncoding::Scalars,
        };
        assert_eq!(
            place_finding(&base, "a.toml", CAFE),
            Err(PlaceError::UnmappablePosition { line: 9, column: 1 })
        );
        let inverted = Finding {
            start: TextPosition { line: 2, column: 5 },
            end: Some(TextPosition { line: 2, column: 3 }),
            ..base.clone()
        };
        assert_eq!(
            place_finding(&inverted, "a.toml", CAFE),
            Err(PlaceError::InvertedRange)
        );
        let bad_end = Finding {
            start: TextPosition { line: 1, column: 1 },
            end: Some(TextPosition { line: 9, column: 1 }),
            ..base.clone()
        };
        assert_eq!(
            place_finding(&bad_end, "a.toml", CAFE),
            Err(PlaceError::UnmappablePosition { line: 9, column: 1 })
        );
    }

    #[test]
    fn encodings_map_columns_after_astral_characters() {
        let text = "const s = \"\u{1f600}\"; missing();\n";
        let at = |line, col, enc| line_col_to_byte_in(text, line, col, enc);
        assert_eq!(at(1, 17, ColumnEncoding::Utf16), Some(18));
        assert_eq!(at(1, 24, ColumnEncoding::Utf16), Some(25));
        assert_eq!(at(1, 27, ColumnEncoding::Utf16), Some(28));
        assert_eq!(at(1, 28, ColumnEncoding::Utf16), None);
        assert_eq!(at(1, 16, ColumnEncoding::Scalars), Some(18));
        assert_eq!(at(1, 23, ColumnEncoding::Scalars), Some(25));
        assert_eq!(at(1, 19, ColumnEncoding::Utf8), Some(18));
        assert_eq!(at(1, 26, ColumnEncoding::Utf8), Some(25));
        assert_eq!(at(1, 15, ColumnEncoding::Utf8), None);
    }

    #[test]
    fn invalid_interior_columns_are_rejected() {
        let text = "a\u{1f600}b\r\nc";
        assert_eq!(line_col_to_byte_in(text, 1, 3, ColumnEncoding::Utf16), None);
        assert_eq!(
            line_col_to_byte_in(text, 1, 4, ColumnEncoding::Utf16),
            Some(5)
        );
        assert_eq!(line_col_to_byte_in(text, 1, 3, ColumnEncoding::Utf8), None);
        assert_eq!(
            line_col_to_byte_in(text, 1, 6, ColumnEncoding::Utf8),
            Some(5)
        );
        assert_eq!(
            line_col_to_byte_in(text, 1, 7, ColumnEncoding::Utf8),
            Some(6)
        );
        assert_eq!(
            line_col_to_byte_in(text, 1, 8, ColumnEncoding::Utf8),
            Some(7)
        );
        assert_eq!(line_col_to_byte_in(text, 1, 9, ColumnEncoding::Utf8), None);
        assert_eq!(
            line_col_to_byte_in(text, 2, 2, ColumnEncoding::Utf16),
            Some(9)
        );
        assert_eq!(line_col_to_byte_in(text, 2, 3, ColumnEncoding::Utf16), None);
    }
}
