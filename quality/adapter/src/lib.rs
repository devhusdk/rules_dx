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
use quality_result::proto::{self, Diagnostic, Severity};

pub mod api_spec_preflight;
pub mod cloudformation_preflight;
pub mod commands;
pub mod exec;
pub mod kubernetes_preflight;
pub mod launch;
pub mod notebook;
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelatedLocation {
    pub file: String,
    pub start: TextPosition,
    pub end: Option<TextPosition>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Related {
    pub role: String,
    pub message: String,
    pub location: Option<RelatedLocation>,
    pub replacement: Option<String>,
    pub applicability: String,
}

impl Related {
    pub fn new(role: &str, message: &str) -> Self {
        Related {
            role: role.to_owned(),
            message: message.to_owned(),
            location: None,
            replacement: None,
            applicability: String::new(),
        }
    }
}

/// Units a tool reports columns in. Deliberately has no `Default`: every
/// parser must state the units its tool uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColumnEncoding {
    Scalars,
    Utf16,
    Utf8,
    /// Unicode scalars with tabs expanded to the next multiple of `width`,
    /// as Checkstyle reports columns. A width of 0 counts as 8.
    ScalarsTabs {
        width: u32,
    },
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
    pub related: Vec<Related>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PlaceError {
    #[error("position {line}:{column} is outside the checked bytes")]
    UnmappablePosition { line: u64, column: u64 },
    #[error("end position precedes start position")]
    InvertedRange,
}

pub fn line_col_to_byte(
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

/// Column units occupied after `ch`, given `seen` units before it.
fn column_step(seen: u64, ch: char, encoding: ColumnEncoding) -> u64 {
    match encoding {
        ColumnEncoding::Scalars => seen + 1,
        ColumnEncoding::Utf8 => seen + ch.len_utf8() as u64,
        ColumnEncoding::Utf16 => seen + ch.len_utf16() as u64,
        ColumnEncoding::ScalarsTabs { width } if ch == '\t' => {
            let w = if width == 0 { 8 } else { u64::from(width) };
            (seen / w + 1) * w
        }
        ColumnEncoding::ScalarsTabs { .. } => seen + 1,
    }
}

/// Width of `body` in `encoding` units, so `width + 1` is the column just
/// past its last character.
pub fn column_width(body: &str, encoding: ColumnEncoding) -> u64 {
    body.chars()
        .fold(0, |seen, ch| column_step(seen, ch, encoding))
}

fn column_to_offset(body: &str, units: u64, encoding: ColumnEncoding) -> Option<usize> {
    let mut seen = 0u64;
    for (offset, ch) in body.char_indices() {
        if seen == units {
            return Some(offset);
        }
        if seen > units {
            return None;
        }
        seen = column_step(seen, ch, encoding);
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
    place_finding_with(finding, path, text, |_| None)
}

pub fn place_finding_with<'t>(
    finding: &Finding,
    path: &str,
    text: &str,
    lookup: impl Fn(&str) -> Option<(String, &'t str)>,
) -> Result<Diagnostic, PlaceError> {
    let start = line_col_to_byte(
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
                line_col_to_byte(text, position.line, position.column, finding.encoding)
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
    let related = finding
        .related
        .iter()
        .map(|item| place_related(item, finding.encoding, &lookup))
        .collect();
    Ok(Diagnostic {
        severity: finding.severity.proto(),
        message: finding.message.clone(),
        tool_id: finding.tool_id.clone(),
        rule_id: finding.rule_id.clone(),
        path: path.to_owned(),
        start_byte: Some(start),
        end_byte: Some(end),
        fixable: false,
        unfixable_reason: String::new(),
        related,
    })
}

fn place_related<'t>(
    item: &Related,
    encoding: ColumnEncoding,
    lookup: &impl Fn(&str) -> Option<(String, &'t str)>,
) -> proto::Related {
    let mut placed = proto::Related {
        role: item.role.clone(),
        message: item.message.clone(),
        path: String::new(),
        start_byte: None,
        end_byte: None,
        replacement: item.replacement.clone(),
        applicability: item.applicability.clone(),
    };
    let Some(location) = &item.location else {
        return placed;
    };
    let Some((path, text)) = lookup(&location.file) else {
        return placed;
    };
    let start = line_col_to_byte(text, location.start.line, location.start.column, encoding);
    let end = match location.end {
        Some(position) if position.column == LINE_END => line_end_to_byte(text, position.line),
        Some(position) => line_col_to_byte(text, position.line, position.column, encoding),
        None => start,
    };
    if let (Some(start), Some(end)) = (start, end) {
        if start <= end {
            placed.path = path;
            placed.start_byte = Some(start);
            placed.end_byte = Some(end);
        }
    }
    placed
}

#[cfg(test)]
mod tests {
    use super::*;

    const CAFE: &str = "fn main() {\n    let caf\u{e9} = 1;\n}\n";

    #[test]
    fn line_col_covers_ascii_positions() {
        assert_eq!(
            line_col_to_byte("ab\ncd\n", 1, 1, ColumnEncoding::Scalars),
            Some(0)
        );
        assert_eq!(
            line_col_to_byte("ab\ncd\n", 1, 3, ColumnEncoding::Scalars),
            Some(2)
        );
        assert_eq!(
            line_col_to_byte("ab\ncd\n", 2, 1, ColumnEncoding::Scalars),
            Some(3)
        );
        assert_eq!(
            line_col_to_byte("ab\ncd\n", 2, 3, ColumnEncoding::Scalars),
            Some(5)
        );
    }

    #[test]
    fn line_col_counts_characters_not_bytes() {
        assert_eq!(
            line_col_to_byte(CAFE, 2, 9, ColumnEncoding::Scalars),
            Some(20)
        );
        assert_eq!(
            line_col_to_byte(CAFE, 2, 12, ColumnEncoding::Scalars),
            Some(23)
        );
    }

    #[test]
    fn line_col_rejects_zero_and_out_of_range() {
        assert_eq!(line_col_to_byte(CAFE, 0, 1, ColumnEncoding::Scalars), None);
        assert_eq!(line_col_to_byte(CAFE, 1, 0, ColumnEncoding::Scalars), None);
        assert_eq!(line_col_to_byte(CAFE, 5, 1, ColumnEncoding::Scalars), None);
        assert_eq!(line_col_to_byte(CAFE, 2, 99, ColumnEncoding::Scalars), None);
        assert_eq!(line_col_to_byte(CAFE, 4, 2, ColumnEncoding::Scalars), None);
    }

    #[test]
    fn line_col_covers_empty_trailing_crlf_and_astral() {
        assert_eq!(line_col_to_byte("", 1, 1, ColumnEncoding::Scalars), Some(0));
        assert_eq!(line_col_to_byte("", 1, 2, ColumnEncoding::Scalars), None);
        assert_eq!(line_col_to_byte("", 2, 1, ColumnEncoding::Scalars), None);
        assert_eq!(
            line_col_to_byte("ab\n", 1, 1, ColumnEncoding::Scalars),
            Some(0)
        );
        assert_eq!(
            line_col_to_byte("ab\n", 1, 3, ColumnEncoding::Scalars),
            Some(2)
        );
        assert_eq!(
            line_col_to_byte("ab\n", 1, 4, ColumnEncoding::Scalars),
            None
        );
        assert_eq!(
            line_col_to_byte("ab\n", 2, 1, ColumnEncoding::Scalars),
            Some(3)
        );
        assert_eq!(
            line_col_to_byte("ab\n", 2, 2, ColumnEncoding::Scalars),
            None
        );
        assert_eq!(
            line_col_to_byte(CAFE, 4, 1, ColumnEncoding::Scalars),
            Some(CAFE.len() as u64)
        );
        let crlf = "a\r\nb";
        assert_eq!(
            line_col_to_byte(crlf, 1, 1, ColumnEncoding::Scalars),
            Some(0)
        );
        assert_eq!(
            line_col_to_byte(crlf, 1, 2, ColumnEncoding::Scalars),
            Some(1)
        );
        assert_eq!(
            line_col_to_byte(crlf, 1, 3, ColumnEncoding::Scalars),
            Some(2)
        );
        assert_eq!(line_col_to_byte(crlf, 1, 4, ColumnEncoding::Scalars), None);
        assert_eq!(
            line_col_to_byte(crlf, 2, 1, ColumnEncoding::Scalars),
            Some(3)
        );
        assert_eq!(
            line_col_to_byte(crlf, 2, 2, ColumnEncoding::Scalars),
            Some(4)
        );
        let astral = "a\u{1f496}b";
        assert_eq!(
            line_col_to_byte(astral, 1, 1, ColumnEncoding::Scalars),
            Some(0)
        );
        assert_eq!(
            line_col_to_byte(astral, 1, 2, ColumnEncoding::Scalars),
            Some(1)
        );
        assert_eq!(
            line_col_to_byte(astral, 1, 3, ColumnEncoding::Scalars),
            Some(5)
        );
        assert_eq!(
            line_col_to_byte(astral, 1, 4, ColumnEncoding::Scalars),
            Some(6)
        );
        assert_eq!(
            line_col_to_byte(astral, 1, 5, ColumnEncoding::Scalars),
            None
        );
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
            related: Vec::new(),
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
            related: Vec::new(),
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
            related: Vec::new(),
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
        let at = |line, col, enc| line_col_to_byte(text, line, col, enc);
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
    fn tab_expanded_columns_map_to_bytes() {
        let text = "\t\treturn 42;\n";
        let at = |col, width| line_col_to_byte(text, 1, col, ColumnEncoding::ScalarsTabs { width });
        assert_eq!(at(24, 8), Some(9));
        assert_eq!(at(16, 4), Some(9));
        assert_eq!(at(1, 8), Some(0));
        assert_eq!(at(9, 8), Some(1));
        assert_eq!(at(9, 0), Some(1));
        assert_eq!(at(5, 8), None);
        assert_eq!(at(3, 4), None);
        let astral = "\t\u{1f600}x\n";
        let astral_at =
            |col| line_col_to_byte(astral, 1, col, ColumnEncoding::ScalarsTabs { width: 4 });
        assert_eq!(astral_at(5), Some(1));
        assert_eq!(astral_at(6), Some(5));
        assert_eq!(astral_at(7), Some(6));
        assert_eq!(astral_at(8), None);
    }

    #[test]
    fn invalid_interior_columns_are_rejected() {
        let text = "a\u{1f600}b\r\nc";
        assert_eq!(line_col_to_byte(text, 1, 3, ColumnEncoding::Utf16), None);
        assert_eq!(line_col_to_byte(text, 1, 4, ColumnEncoding::Utf16), Some(5));
        assert_eq!(line_col_to_byte(text, 1, 3, ColumnEncoding::Utf8), None);
        assert_eq!(line_col_to_byte(text, 1, 6, ColumnEncoding::Utf8), Some(5));
        assert_eq!(line_col_to_byte(text, 1, 7, ColumnEncoding::Utf8), Some(6));
        assert_eq!(line_col_to_byte(text, 1, 8, ColumnEncoding::Utf8), Some(7));
        assert_eq!(line_col_to_byte(text, 1, 9, ColumnEncoding::Utf8), None);
        assert_eq!(line_col_to_byte(text, 2, 2, ColumnEncoding::Utf16), Some(9));
        assert_eq!(line_col_to_byte(text, 2, 3, ColumnEncoding::Utf16), None);
    }

    #[test]
    fn related_locations_resolve_against_the_named_file_bytes() {
        let other = "a\r\n\u{e9}x\r\n";
        let mut finding = Finding {
            tool_id: "clippy".to_owned(),
            rule_id: String::new(),
            message: "m".to_owned(),
            severity: ToolSeverity::Warning,
            start: TextPosition { line: 2, column: 1 },
            end: None,
            suggestions: Vec::new(),
            encoding: ColumnEncoding::Scalars,
            related: Vec::new(),
        };
        let mut on_other = Related::new("label", "second");
        on_other.location = Some(RelatedLocation {
            file: "/s/b.rs".to_owned(),
            start: TextPosition { line: 2, column: 2 },
            end: Some(TextPosition { line: 2, column: 3 }),
        });
        let mut external = Related::new("note", "external");
        external.location = Some(RelatedLocation {
            file: "/rustc/lib.rs".to_owned(),
            start: TextPosition { line: 1, column: 1 },
            end: None,
        });
        let mut off_end = Related::new("note", "far");
        off_end.location = Some(RelatedLocation {
            file: "/s/b.rs".to_owned(),
            start: TextPosition { line: 9, column: 1 },
            end: None,
        });
        finding.related = vec![on_other, external, off_end, Related::new("help", "bare")];
        let placed = place_finding_with(&finding, "a.rs", CAFE, |file| {
            (file == "/s/b.rs").then(|| ("src/b.rs".to_owned(), other))
        })
        .expect("placed");
        assert_eq!(placed.related.len(), 4);
        assert_eq!(placed.related[0].path, "src/b.rs");
        assert_eq!(
            (placed.related[0].start_byte, placed.related[0].end_byte),
            (Some(5), Some(6))
        );
        for unlocated in &placed.related[1..] {
            assert_eq!(unlocated.path, "");
            assert_eq!(unlocated.start_byte, None);
        }
        assert_eq!(placed.related[3].role, "help");
        let plain = place_finding(&finding, "a.rs", CAFE).expect("placed");
        assert!(plain.related.iter().all(|item| item.path.is_empty()));
    }

    #[test]
    fn column_width_round_trips_for_every_encoding() {
        let body = "a\u{1F600}\u{e9}\tb";
        for (encoding, width) in [
            (ColumnEncoding::Scalars, 5),
            (ColumnEncoding::Utf16, 6),
            (ColumnEncoding::Utf8, 9),
            (ColumnEncoding::ScalarsTabs { width: 4 }, 5),
            (ColumnEncoding::ScalarsTabs { width: 0 }, 9),
        ] {
            assert_eq!(column_width(body, encoding), width, "{encoding:?}");
            for text in [
                body.to_owned(),
                format!("x\n{body}\n"),
                format!("x\r\n{body}\r\n"),
            ] {
                let line = if text == body { 1 } else { 2 };
                let line_start = if text == body {
                    0
                } else {
                    text.find('\n').unwrap() + 1
                };
                let line_body = text[line_start..].split('\n').next().unwrap();
                let end = column_width(line_body, encoding) + 1;
                assert_eq!(
                    line_col_to_byte(&text, line, end, encoding),
                    Some((line_start + line_body.len()) as u64),
                    "{encoding:?} {text:?}"
                );
                assert_eq!(line_col_to_byte(&text, line, end + 1, encoding), None);
            }
        }
    }
}
