//! Jupyter notebook scanner: preflight, location mapping and source splicing.
//!
//! Std plus `serde_json` only, so the module stays free of adapter types and
//! can move to its own crate. The scanner keeps byte spans into the original
//! text so a rewrite can touch only the bytes of the edited cells.

use std::fmt;

type Span = (usize, usize);

const MAX_DEPTH: usize = 128;

/// Why a notebook was rejected or could not be mapped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotebookError {
    /// The text is not valid JSON.
    Json(String),
    /// The JSON is not an nbformat 4 notebook.
    Format(String),
    /// The notebook language is not Python.
    Language(String),
    /// The original and the rewritten notebook cannot be spliced.
    Splice(String),
}

impl fmt::Display for NotebookError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(detail) => write!(f, "invalid notebook JSON: {detail}"),
            Self::Format(detail) => write!(f, "unsupported notebook: {detail}"),
            Self::Language(name) => write!(
                f,
                "notebook language is {name:?}; ruff checks only Python notebooks"
            ),
            Self::Splice(detail) => write!(f, "cannot splice notebook sources: {detail}"),
        }
    }
}

impl std::error::Error for NotebookError {}

/// What a successful preflight learned about a notebook.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Info {
    /// Number of cells of every type.
    pub cells: usize,
}

/// Spans of one cell, kept so other tools can edit single fields later.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CellIndex {
    /// The `cell_type` value.
    pub kind: String,
    /// Byte span of the `source` value.
    pub source: Option<(usize, usize)>,
    /// Byte span of the `outputs` value.
    pub outputs: Option<(usize, usize)>,
    /// Byte span of the `execution_count` value.
    pub execution_count: Option<(usize, usize)>,
}

#[derive(Debug, Clone)]
enum Node {
    Object(Span, Vec<(String, Node)>),
    Array(Span, Vec<Node>),
    Str(Span),
    Lit(Span),
}

impl Node {
    fn span(&self) -> Span {
        match self {
            Self::Object(span, _) | Self::Array(span, _) | Self::Str(span) | Self::Lit(span) => {
                *span
            }
        }
    }
}

struct Scanner<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl Scanner<'_> {
    fn skip_ws(&mut self) {
        while self
            .bytes
            .get(self.at)
            .is_some_and(|b| matches!(b, b' ' | b'\t' | b'\n' | b'\r'))
        {
            self.at += 1;
        }
    }

    fn fail<T>(&self, what: &str) -> Result<T, NotebookError> {
        Err(NotebookError::Json(format!("{what} at byte {}", self.at)))
    }

    fn value(&mut self, depth: usize) -> Result<Node, NotebookError> {
        if depth > MAX_DEPTH {
            return self.fail("nesting too deep");
        }
        self.skip_ws();
        match self.bytes.get(self.at) {
            Some(b'{') => self.object(depth),
            Some(b'[') => self.array(depth),
            Some(b'"') => Ok(Node::Str(self.string()?)),
            Some(_) => {
                let start = self.at;
                while self.bytes.get(self.at).is_some_and(|b| {
                    !matches!(b, b',' | b'}' | b']' | b' ' | b'\t' | b'\n' | b'\r')
                }) {
                    self.at += 1;
                }
                if self.at == start {
                    return self.fail("unexpected byte");
                }
                Ok(Node::Lit((start, self.at)))
            }
            None => self.fail("unexpected end"),
        }
    }

    fn string(&mut self) -> Result<Span, NotebookError> {
        let start = self.at;
        self.at += 1;
        loop {
            match self.bytes.get(self.at) {
                Some(b'"') => {
                    self.at += 1;
                    return Ok((start, self.at));
                }
                Some(b'\\') => self.at += 2,
                Some(_) => self.at += 1,
                None => return self.fail("unterminated string"),
            }
        }
    }

    fn object(&mut self, depth: usize) -> Result<Node, NotebookError> {
        let start = self.at;
        self.at += 1;
        let mut members = Vec::new();
        self.skip_ws();
        if self.bytes.get(self.at) == Some(&b'}') {
            self.at += 1;
            return Ok(Node::Object((start, self.at), members));
        }
        loop {
            self.skip_ws();
            if self.bytes.get(self.at) != Some(&b'"') {
                return self.fail("expected a key");
            }
            let key_span = self.string()?;
            let text = std::str::from_utf8(self.bytes)
                .map_err(|err| NotebookError::Json(err.to_string()))?;
            let key: String = decode_string(text, key_span)
                .into_iter()
                .map(|(c, _)| c)
                .collect();
            self.skip_ws();
            if self.bytes.get(self.at) != Some(&b':') {
                return self.fail("expected ':'");
            }
            self.at += 1;
            let value = self.value(depth + 1)?;
            members.push((key, value));
            self.skip_ws();
            match self.bytes.get(self.at) {
                Some(b',') => self.at += 1,
                Some(b'}') => {
                    self.at += 1;
                    return Ok(Node::Object((start, self.at), members));
                }
                _ => return self.fail("expected ',' or '}'"),
            }
        }
    }

    fn array(&mut self, depth: usize) -> Result<Node, NotebookError> {
        let start = self.at;
        self.at += 1;
        let mut items = Vec::new();
        self.skip_ws();
        if self.bytes.get(self.at) == Some(&b']') {
            self.at += 1;
            return Ok(Node::Array((start, self.at), items));
        }
        loop {
            items.push(self.value(depth + 1)?);
            self.skip_ws();
            match self.bytes.get(self.at) {
                Some(b',') => self.at += 1,
                Some(b']') => {
                    self.at += 1;
                    return Ok(Node::Array((start, self.at), items));
                }
                _ => return self.fail("expected ',' or ']'"),
            }
        }
    }
}

/// Decodes the JSON string at `span` (quotes included) into scalars paired
/// with the byte offset where each scalar starts in the raw text.
fn decode_string(text: &str, span: Span) -> Vec<(char, usize)> {
    let inner = &text[span.0 + 1..span.1 - 1];
    let base = span.0 + 1;
    let mut out = Vec::with_capacity(inner.len());
    let mut iter = inner.char_indices().peekable();
    while let Some((idx, c)) = iter.next() {
        if c != '\\' {
            out.push((c, base + idx));
            continue;
        }
        let Some((_, esc)) = iter.next() else { break };
        let decoded = match esc {
            'n' => '\n',
            't' => '\t',
            'r' => '\r',
            'b' => '\u{8}',
            'f' => '\u{c}',
            'u' => {
                let high = take_hex4(&mut iter);
                match high {
                    Some(0xD800..=0xDBFF) => {
                        let mut ahead = iter.clone();
                        let low = match (ahead.next(), ahead.next()) {
                            (Some((_, '\\')), Some((_, 'u'))) => take_hex4(&mut ahead),
                            _ => None,
                        };
                        match (high, low) {
                            (Some(h), Some(l @ 0xDC00..=0xDFFF)) => {
                                iter = ahead;
                                char::from_u32(0x10000 + ((h - 0xD800) << 10) + (l - 0xDC00))
                                    .unwrap_or('\u{FFFD}')
                            }
                            _ => '\u{FFFD}',
                        }
                    }
                    Some(code) => char::from_u32(code).unwrap_or('\u{FFFD}'),
                    None => '\u{FFFD}',
                }
            }
            other => other,
        };
        out.push((decoded, base + idx));
    }
    out
}

fn take_hex4(iter: &mut std::iter::Peekable<std::str::CharIndices<'_>>) -> Option<u32> {
    let mut value = 0u32;
    for _ in 0..4 {
        let (_, digit) = iter.next()?;
        value = value * 16 + digit.to_digit(16)?;
    }
    Some(value)
}

#[derive(Debug, Clone)]
enum Source {
    Single(Span),
    Lines(Span, Vec<Span>),
}

#[derive(Debug, Clone)]
struct Cell {
    kind: String,
    source: Option<Source>,
    outputs: Option<Span>,
    execution_count: Option<Span>,
}

/// A scanned notebook borrowing its original text.
#[derive(Debug)]
pub struct Notebook<'a> {
    text: &'a str,
    cells: Vec<Cell>,
    python: Option<String>,
}

fn member<'n>(node: &'n Node, key: &str) -> Option<&'n Node> {
    let Node::Object(_, members) = node else {
        return None;
    };
    members
        .iter()
        .rev()
        .find(|(name, _)| name == key)
        .map(|(_, value)| value)
}

impl<'a> Notebook<'a> {
    /// Parses a notebook without checking its format version or language.
    pub fn parse(text: &'a str) -> Result<Self, NotebookError> {
        serde_json::from_str::<serde::de::IgnoredAny>(text)
            .map_err(|err| NotebookError::Json(err.to_string()))?;
        let mut scanner = Scanner {
            bytes: text.as_bytes(),
            at: 0,
        };
        let root = scanner.value(0)?;
        Self::from_root(text, &root)
    }

    fn lit(&self, node: &Node) -> &'a str {
        let (start, end) = node.span();
        &self.text[start..end]
    }

    fn from_root(text: &'a str, root: &Node) -> Result<Self, NotebookError> {
        let probe = Self {
            text,
            cells: Vec::new(),
            python: None,
        };
        if !matches!(root, Node::Object(..)) {
            return Err(NotebookError::Format(
                "top level is not an object".to_owned(),
            ));
        }
        let nbformat = probe.present(member(root, "nbformat"));
        match nbformat.map(|node| probe.lit(node)) {
            Some("4") => {}
            Some(other) => {
                return Err(NotebookError::Format(format!(
                    "nbformat {other} (only nbformat 4 is supported)"
                )))
            }
            None => return Err(NotebookError::Format("missing nbformat".to_owned())),
        }
        let Some(Node::Array(_, raw_cells)) = member(root, "cells") else {
            return Err(NotebookError::Format("missing cells array".to_owned()));
        };
        let mut cells = Vec::with_capacity(raw_cells.len());
        for raw in raw_cells {
            cells.push(probe.cell(raw)?);
        }
        let python = probe.language(root);
        Ok(Self {
            text,
            cells,
            python,
        })
    }

    fn present<'n>(&self, node: Option<&'n Node>) -> Option<&'n Node> {
        node.filter(|n| !(matches!(n, Node::Lit(_)) && self.lit(n) == "null"))
    }

    fn string_value(&self, node: &Node) -> Option<String> {
        match node {
            Node::Str(span) => Some(
                decode_string(self.text, *span)
                    .into_iter()
                    .map(|(c, _)| c)
                    .collect(),
            ),
            _ => None,
        }
    }

    /// Follows Ruff's `is_python_notebook`: `language_info.name`, then
    /// `kernelspec.language`, then Python by default. Returns the offending
    /// language name when it is not Python.
    fn language(&self, root: &Node) -> Option<String> {
        let metadata = self.present(member(root, "metadata"));
        let name = |node: &Node, key: &str| {
            member(node, key)
                .and_then(|value| self.string_value(value))
                .unwrap_or_default()
        };
        let info = metadata.and_then(|m| self.present(member(m, "language_info")));
        let spec = metadata.and_then(|m| self.present(member(m, "kernelspec")));
        let found = match (info, spec) {
            (Some(info), _) => name(info, "name"),
            (None, Some(spec)) => name(spec, "language"),
            (None, None) => return None,
        };
        if found == "python" {
            None
        } else if found.is_empty() {
            Some("unknown".to_owned())
        } else {
            Some(found)
        }
    }

    fn cell(&self, raw: &Node) -> Result<Cell, NotebookError> {
        if !matches!(raw, Node::Object(..)) {
            return Err(NotebookError::Format("a cell is not an object".to_owned()));
        }
        let kind = member(raw, "cell_type")
            .and_then(|node| self.string_value(node))
            .ok_or_else(|| NotebookError::Format("a cell has no cell_type".to_owned()))?;
        let source = match member(raw, "source") {
            None => None,
            Some(Node::Str(span)) => Some(Source::Single(*span)),
            Some(Node::Array(span, items)) => {
                let mut lines = Vec::with_capacity(items.len());
                for item in items {
                    let Node::Str(line) = item else {
                        return Err(NotebookError::Format(
                            "a source array holds a non-string".to_owned(),
                        ));
                    };
                    lines.push(*line);
                }
                Some(Source::Lines(*span, lines))
            }
            Some(_) => {
                return Err(NotebookError::Format(
                    "a cell source is neither a string nor an array".to_owned(),
                ))
            }
        };
        Ok(Cell {
            kind,
            source,
            outputs: member(raw, "outputs").map(Node::span),
            execution_count: member(raw, "execution_count").map(Node::span),
        })
    }

    /// Per-cell spans, in file order.
    pub fn index(&self) -> Vec<CellIndex> {
        self.cells
            .iter()
            .map(|cell| CellIndex {
                kind: cell.kind.clone(),
                source: cell.source.as_ref().map(source_span),
                outputs: cell.outputs,
                execution_count: cell.execution_count,
            })
            .collect()
    }

    /// The decoded source of a cell with each scalar's raw byte offset, plus
    /// the raw offset just past the last scalar.
    fn source_chars(&self, cell: &Cell) -> Option<(Vec<(char, usize)>, usize)> {
        match cell.source.as_ref()? {
            Source::Single(span) => Some((decode_string(self.text, *span), span.1 - 1)),
            Source::Lines(span, lines) => {
                let mut chars = Vec::new();
                for line in lines {
                    chars.extend(decode_string(self.text, *line));
                }
                let end = lines.last().map_or(span.0 + 1, |line| line.1 - 1);
                Some((chars, end))
            }
        }
    }

    fn source_text(&self, cell: &Cell) -> Option<String> {
        self.source_chars(cell)
            .map(|(chars, _)| chars.into_iter().map(|(c, _)| c).collect())
    }

    /// Maps a Ruff `(cell, row, column)` (all 1-based, columns in scalars) to
    /// a 1-based line and scalar column in the notebook file. A row or column
    /// past the end clamps to the end of the cell source; anything else out of
    /// range returns `None`.
    pub fn locate(&self, cell: u64, row: u64, column: u64) -> Option<(u64, u64)> {
        let index = usize::try_from(cell.checked_sub(1)?).ok()?;
        let target = self.cells.get(index)?;
        if target.kind != "code" || row == 0 || column == 0 {
            return None;
        }
        let (chars, end) = self.source_chars(target)?;
        let mut starts = vec![0usize];
        for (at, (c, _)) in chars.iter().enumerate() {
            let lone_cr = *c == '\r' && chars.get(at + 1).map(|(n, _)| *n) != Some('\n');
            if *c == '\n' || lone_cr {
                starts.push(at + 1);
            }
        }
        let offset = match usize::try_from(row - 1)
            .ok()
            .and_then(|r| starts.get(r).copied())
        {
            None => end,
            Some(line_start) => {
                let mut content_end = starts
                    .iter()
                    .copied()
                    .find(|start| *start > line_start)
                    .unwrap_or(chars.len());
                while content_end > line_start
                    && content_end < chars.len() + 1
                    && matches!(
                        chars.get(content_end - 1).map(|(c, _)| *c),
                        Some('\n' | '\r')
                    )
                {
                    content_end -= 1;
                }
                let wanted = line_start.saturating_add(usize::try_from(column - 1).ok()?);
                let at = wanted.min(content_end);
                chars.get(at).map_or(end, |(_, raw)| *raw)
            }
        };
        let before = &self.text[..offset];
        let line = before.matches('\n').count() as u64 + 1;
        let line_start = before.rfind('\n').map_or(0, |at| at + 1);
        let col = before[line_start..].chars().count() as u64 + 1;
        Some((line, col))
    }
}

fn source_span(source: &Source) -> Span {
    match source {
        Source::Single(span) | Source::Lines(span, _) => *span,
    }
}

/// Checks that `text` is an nbformat 4 notebook in Python.
pub fn preflight(text: &str) -> Result<Info, NotebookError> {
    let notebook = Notebook::parse(text)?;
    if let Some(language) = notebook.python {
        return Err(NotebookError::Language(language));
    }
    Ok(Info {
        cells: notebook.cells.len(),
    })
}

/// One-shot form of [`Notebook::locate`].
pub fn locate(text: &str, cell: u64, row: u64, column: u64) -> Option<(u64, u64)> {
    Notebook::parse(text).ok()?.locate(cell, row, column)
}

fn encode_string(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Splits text into lines that keep their `\n`, the nbformat array layout.
fn split_lines(text: &str) -> Vec<&str> {
    text.split_inclusive('\n').collect()
}

fn strip_final_newline(text: &str) -> &str {
    text.strip_suffix("\r\n")
        .or_else(|| text.strip_suffix('\n'))
        .unwrap_or(text)
}

fn line_indent(text: &str, at: usize) -> &str {
    let start = text[..at].rfind('\n').map_or(0, |i| i + 1);
    let rest = &text[start..at];
    let len = rest.len() - rest.trim_start_matches([' ', '\t']).len();
    &rest[..len]
}

fn indent_unit(text: &str) -> &str {
    text.lines()
        .map(|line| {
            let len = line.len() - line.trim_start_matches([' ', '\t']).len();
            &line[..len]
        })
        .find(|indent| !indent.is_empty())
        .unwrap_or(" ")
}

fn render_lines(original: &str, source: &Source, new_text: &str, eol: &str) -> String {
    let Source::Lines(span, lines) = source else {
        return encode_string(new_text);
    };
    let lines_new = split_lines(new_text);
    if lines_new.is_empty() {
        return "[]".to_owned();
    }
    let multiline = lines
        .first()
        .is_some_and(|first| original[span.0..first.0].contains('\n'));
    let (elem, close) = if multiline {
        let first = lines[0];
        let closing = line_indent(original, span.1 - 1);
        (
            line_indent(original, first.0).to_owned(),
            closing.to_owned(),
        )
    } else {
        let key = line_indent(original, span.0).to_owned();
        (format!("{key}{}", indent_unit(original)), key)
    };
    let body: Vec<String> = lines_new
        .iter()
        .map(|line| format!("{elem}{}", encode_string(line)))
        .collect();
    format!("[{eol}{}{eol}{close}]", body.join(&format!(",{eol}")))
}

/// Copies the sources of changed code cells from `rewritten` into
/// `original` and leaves every other byte alone.
pub fn splice_sources(original: &str, rewritten: &str) -> Result<String, NotebookError> {
    let before = Notebook::parse(original)?;
    let after = Notebook::parse(rewritten)?;
    if before.cells.len() != after.cells.len() {
        return Err(NotebookError::Splice(format!(
            "cell count changed from {} to {}",
            before.cells.len(),
            after.cells.len()
        )));
    }
    let eol = if original.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    let mut edits: Vec<(Span, String)> = Vec::new();
    let mut expected: Vec<Option<String>> = Vec::with_capacity(before.cells.len());
    for (at, (old, new)) in before.cells.iter().zip(&after.cells).enumerate() {
        if old.kind != new.kind {
            return Err(NotebookError::Splice(format!(
                "cell {} changed type from {} to {}",
                at + 1,
                old.kind,
                new.kind
            )));
        }
        if old.kind != "code" {
            expected.push(None);
            continue;
        }
        let old_text = before.source_text(old).unwrap_or_default();
        let mut new_text = after.source_text(new).unwrap_or_default();
        if strip_final_newline(&old_text) == strip_final_newline(&new_text) {
            expected.push(Some(old_text));
            continue;
        }
        if old_text.ends_with('\n') && !new_text.ends_with('\n') {
            new_text.push('\n');
        }
        let Some(source) = old.source.as_ref() else {
            return Err(NotebookError::Splice(format!(
                "cell {} has no source to replace",
                at + 1
            )));
        };
        edits.push((
            source_span(source),
            render_lines(original, source, &new_text, eol),
        ));
        expected.push(Some(new_text));
    }
    let mut out = original.to_owned();
    edits.sort_by_key(|(span, _)| std::cmp::Reverse(span.0));
    for ((start, end), replacement) in edits {
        out.replace_range(start..end, &replacement);
    }
    let check = Notebook::parse(&out)
        .map_err(|err| NotebookError::Splice(format!("result is not a notebook: {err}")))?;
    for (at, (cell, want)) in check.cells.iter().zip(&expected).enumerate() {
        if let Some(want) = want {
            if check.source_text(cell).as_deref() != Some(want.as_str()) {
                return Err(NotebookError::Splice(format!(
                    "cell {} did not round-trip",
                    at + 1
                )));
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nb(cells: &str, meta: &str) -> String {
        format!(
            "{{\n \"cells\": [{cells}],\n \"metadata\": {meta},\n \"nbformat\": 4,\n \"nbformat_minor\": 5\n}}\n"
        )
    }

    fn md() -> &'static str {
        "{\"cell_type\": \"markdown\", \"metadata\": {}, \"source\": [\"# hi\"]}"
    }

    fn code(source: &str) -> String {
        format!(
            "{{\"cell_type\": \"code\", \"execution_count\": null, \"metadata\": {{}}, \"outputs\": [], \"source\": {source}}}"
        )
    }

    #[test]
    fn markdown_cell_before_code_cell_counts_in_the_index() {
        let text = nb(
            &format!("{},{}", md(), code("[\"import os\\n\", \"x = 1\"]")),
            "{}",
        );
        assert_eq!(preflight(&text).unwrap().cells, 2);
        let found = locate(&text, 2, 2, 3).unwrap();
        let line_text: Vec<&str> = text.lines().collect();
        assert!(line_text[found.0 as usize - 1].contains("x = 1"));
        assert_eq!(locate(&text, 1, 1, 1), None);
        assert_eq!(locate(&text, 3, 1, 1), None);
        assert_eq!(locate(&text, 0, 1, 1), None);
        assert_eq!(locate(&text, 2, 0, 1), None);
    }

    #[test]
    fn single_string_source_with_newline_escapes() {
        let text = nb(&code("\"a = 1\\nb = 2\\n\""), "{}");
        let (line, col) = locate(&text, 1, 2, 1).unwrap();
        let raw = text.lines().nth(line as usize - 1).unwrap();
        assert!(raw[col as usize - 1..].starts_with("b = 2\\n\"}"));
    }

    #[test]
    fn escapes_and_non_ascii_count_as_scalars() {
        let text = nb(&code("[\"s = \\\"é\\\" + \\u00e9 + x\"]"), "{}");
        // s = "é" + é + x  ->  `x` is the 15th scalar.
        let (line, col) = locate(&text, 1, 1, 15).unwrap();
        let raw = text.lines().nth(line as usize - 1).unwrap();
        let tail: String = raw.chars().skip(col as usize - 1).collect();
        assert!(tail.starts_with("x\"]}"));
    }

    #[test]
    fn surrogate_pair_decodes_to_one_scalar() {
        let text = nb(&code("[\"a = \\ud83d\\ude00; b\"]"), "{}");
        let (line, col) = locate(&text, 1, 1, 8).unwrap();
        let raw = text.lines().nth(line as usize - 1).unwrap();
        let tail: String = raw.chars().skip(col as usize - 1).collect();
        assert!(tail.starts_with("b\"]}"));
    }

    #[test]
    fn out_of_range_row_and_column_clamp_to_the_cell_end() {
        let text = nb(&code("[\"a\\n\", \"b\"]"), "{}");
        let end = locate(&text, 1, 2, 2).unwrap();
        assert_eq!(locate(&text, 1, 9, 1), Some(end));
        assert_eq!(locate(&text, 1, 2, 99), Some(end));
    }

    #[test]
    fn empty_source_array_locates_inside_the_brackets() {
        let text = nb(&code("[]"), "{}");
        let (line, col) = locate(&text, 1, 1, 1).unwrap();
        let raw = text.lines().nth(line as usize - 1).unwrap();
        assert_eq!(&raw[col as usize - 1..col as usize], "]");
    }

    #[test]
    fn invalid_json_is_rejected() {
        assert!(matches!(
            preflight("{\"cells\": ["),
            Err(NotebookError::Json(_))
        ));
        assert!(matches!(preflight(""), Err(NotebookError::Json(_))));
    }

    #[test]
    fn non_python_kernel_and_old_format_fail_closed() {
        let r = nb(&code("[\"x\"]"), "{\"kernelspec\": {\"language\": \"R\"}}");
        assert_eq!(preflight(&r), Err(NotebookError::Language("R".to_owned())));
        let julia = nb(&code("[\"x\"]"), "{\"language_info\": {\"name\": \"julia\"}, \"kernelspec\": {\"language\": \"python\"}}");
        assert!(matches!(preflight(&julia), Err(NotebookError::Language(_))));
        let v3 = nb(&code("[\"x\"]"), "{}").replace("\"nbformat\": 4", "\"nbformat\": 3");
        assert!(matches!(preflight(&v3), Err(NotebookError::Format(_))));
        let py = nb(
            &code("[\"x\"]"),
            "{\"language_info\": {\"name\": \"python\"}}",
        );
        assert!(preflight(&py).is_ok());
        assert!(preflight(&nb(&code("[\"x\"]"), "{}")).is_ok());
    }

    /// Re-serializes the way Ruff does: sorted keys, 1-space indent, ids.
    fn ruff_style(source: &str) -> String {
        format!(
            "{{\n \"cells\": [\n  {{\n   \"cell_type\": \"markdown\",\n   \"id\": \"a1\",\n   \"metadata\": {{}},\n   \"source\": [\n    \"# hi\"\n   ]\n  }},\n  {{\n   \"cell_type\": \"code\",\n   \"execution_count\": null,\n   \"id\": \"b2\",\n   \"metadata\": {{}},\n   \"outputs\": [],\n   \"source\": {source}\n  }}\n ],\n \"metadata\": {{}},\n \"nbformat\": 4,\n \"nbformat_minor\": 5\n}}\n"
        )
    }

    #[test]
    fn splice_changes_only_the_edited_cell_with_two_space_indent() {
        let original = "{\n  \"cells\": [\n    {\n      \"cell_type\": \"markdown\",\n      \"metadata\": {},\n      \"source\": [\n        \"# hi\"\n      ]\n    },\n    {\n      \"cell_type\": \"code\",\n      \"execution_count\": 3,\n      \"metadata\": {\"tags\": [\"x\"]},\n      \"outputs\": [{\"name\": \"stdout\", \"output_type\": \"stream\", \"text\": [\"1\\n\"]}],\n      \"source\": [\n        \"import os\\n\",\n        \"x=1\"\n      ]\n    }\n  ],\n  \"metadata\": {},\n  \"nbformat\": 4,\n  \"nbformat_minor\": 5\n}\n";
        let rewritten = ruff_style("[\n    \"x = 1\"\n   ]");
        let out = splice_sources(original, &rewritten).unwrap();
        let expected = original.replace(
            "\"source\": [\n        \"import os\\n\",\n        \"x=1\"\n      ]",
            "\"source\": [\n        \"x = 1\"\n      ]",
        );
        assert_eq!(out, expected);
    }

    #[test]
    fn splice_keeps_one_space_indent_and_single_string_kind() {
        let original = nb(&format!("{},{}", md(), code("\"x=1\\ny=2\\n\"")), "{}");
        let rewritten = ruff_style("[\n    \"x = 1\\n\",\n    \"y = 2\"\n   ]");
        let out = splice_sources(&original, &rewritten).unwrap();
        assert_eq!(out, original.replace("x=1\\ny=2\\n", "x = 1\\ny = 2\\n"));
    }

    #[test]
    fn splice_keeps_one_space_array_elements() {
        let original = "{\n \"cells\": [\n  {\n   \"cell_type\": \"code\",\n   \"execution_count\": null,\n   \"metadata\": {},\n   \"outputs\": [],\n   \"source\": [\n    \"x=1\"\n   ]\n  }\n ],\n \"metadata\": {},\n \"nbformat\": 4,\n \"nbformat_minor\": 5\n}\n";
        let rewritten = original.replace("x=1", "x = 1\\n\",\n    \"y = 2");
        let out = splice_sources(original, &rewritten).unwrap();
        assert!(out.contains("   \"source\": [\n    \"x = 1\\n\",\n    \"y = 2\"\n   ]"));
        assert!(serde_json::from_str::<serde_json::Value>(&out).is_ok());
    }

    #[test]
    fn splice_ignores_key_order_indent_and_ids() {
        let original = nb(
            &format!("{},{}", md(), code("[\"import os\\n\", \"x=1\"]")),
            "{}",
        );
        let rewritten = ruff_style("[\n    \"x = 1\"\n   ]");
        let out = splice_sources(&original, &rewritten).unwrap();
        assert!(!out.contains("\"id\""));
        assert!(out.contains("\"# hi\""));
        assert!(out.starts_with(&original[..original.find("\"import os").unwrap()]));
        assert!(out.contains("[\n  \"x = 1\"\n ]") || out.contains("\"x = 1\""));
        let again = splice_sources(&out, &rewritten).unwrap();
        assert_eq!(again, out);
    }

    #[test]
    fn splice_keeps_crlf_line_endings() {
        let original =
            nb(&format!("{},{}", md(), code("[\n  \"x=1\"\n ]")), "{}").replace('\n', "\r\n");
        let rewritten = ruff_style("[\n    \"x = 1\\n\",\n    \"y = 2\"\n   ]");
        let out = splice_sources(&original, &rewritten).unwrap();
        assert!(out.contains("\"x = 1\\n\",\r\n"));
        assert!(!out.replace("\r\n", "").contains('\n'));
    }

    #[test]
    fn splice_leaves_an_unchanged_notebook_byte_identical() {
        let original = nb(&format!("{},{}", md(), code("[\"x = 1\\n\"]")), "{}");
        let rewritten = ruff_style("[\n    \"x = 1\"\n   ]");
        assert_eq!(splice_sources(&original, &rewritten).unwrap(), original);
    }

    #[test]
    fn splice_fills_an_empty_source_array() {
        let original = nb(&format!("{},{}", md(), code("[]")), "{}");
        let rewritten = ruff_style("[\n    \"x = 1\"\n   ]");
        let out = splice_sources(&original, &rewritten).unwrap();
        assert!(out.contains("\"x = 1\""));
        assert!(serde_json::from_str::<serde_json::Value>(&out).is_ok());
    }

    #[test]
    fn splice_rejects_changed_shape() {
        let original = nb(&code("[\"x\"]"), "{}");
        let fewer = nb("", "{}");
        assert!(matches!(
            splice_sources(&original, &fewer),
            Err(NotebookError::Splice(_))
        ));
        let retyped = original.replace("\"code\"", "\"raw\"");
        assert!(matches!(
            splice_sources(&original, &retyped),
            Err(NotebookError::Splice(_))
        ));
        assert!(matches!(
            splice_sources(&original, "nope"),
            Err(NotebookError::Json(_))
        ));
    }

    #[test]
    fn index_keeps_output_and_execution_count_spans() {
        let text = nb(&code("[\"x\"]"), "{}");
        let index = Notebook::parse(&text).unwrap().index();
        let cell = &index[0];
        let (s, e) = cell.outputs.unwrap();
        assert_eq!(&text[s..e], "[]");
        let (s, e) = cell.execution_count.unwrap();
        assert_eq!(&text[s..e], "null");
    }
}
