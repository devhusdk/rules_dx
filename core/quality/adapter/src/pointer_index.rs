//! Positions of JSON pointers in YAML, JSON and TOML sources.
//!
//! Shared by the offline preflights and the JSON Schema adapter, so every finding that
//! names a data path can be placed on a line and column of the file it came from.

use std::collections::BTreeMap;

use saphyr_parser::{Event, Parser, ScalarStyle};

/// A syntax error at a zero-based line and column (the parser's own units).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntaxAt {
    pub line: usize,
    pub col: usize,
    pub detail: String,
}

impl SyntaxAt {
    pub fn new(line: usize, col: usize, detail: &str) -> Self {
        Self {
            line,
            col,
            detail: detail.to_owned(),
        }
    }
}

/// Escapes one JSON pointer reference token.
pub fn escape(token: &str) -> String {
    token.replace('~', "~0").replace('/', "~1")
}

pub enum Frame {
    Map {
        pointer: String,
        key: Option<String>,
        expecting_key: bool,
    },
    Seq {
        pointer: String,
        index: usize,
    },
}

pub enum Node<'a> {
    Scalar(&'a str, ScalarStyle),
    Map,
    Seq,
    Alias,
}

#[derive(Default)]
pub struct Raw {
    pub start_char: usize,
    pub explicit: bool,
    pub root: Option<(u64, u64)>,
    pub empty: bool,
    pub pointers: BTreeMap<String, (u64, u64)>,
    pub captured: BTreeMap<String, String>,
}

pub fn value_done(stack: &mut [Frame]) {
    if let Some(Frame::Map { expecting_key, .. }) = stack.last_mut() {
        *expecting_key = true;
    }
}

pub fn is_null(value: &str, style: ScalarStyle) -> bool {
    style == ScalarStyle::Plain && matches!(value, "" | "~" | "null" | "Null" | "NULL")
}

pub fn on_node(
    raw: &mut Raw,
    stack: &mut Vec<Frame>,
    node: Node<'_>,
    at: (u64, u64),
    line: usize,
    col: usize,
    captured: &[&str],
) -> Result<(), SyntaxAt> {
    let pointer = match stack.last_mut() {
        None => {
            raw.root = Some(at);
            raw.empty = matches!(node, Node::Scalar(value, style) if is_null(value, style));
            String::new()
        }
        Some(Frame::Map {
            pointer,
            key,
            expecting_key,
        }) => {
            if *expecting_key {
                let Node::Scalar(name, _) = node else {
                    return Err(SyntaxAt::new(
                        line,
                        col,
                        "a mapping, sequence or alias used as a key",
                    ));
                };
                let child = format!("{pointer}/{}", escape(name));
                raw.pointers.entry(child).or_insert(at);
                *key = Some(name.to_owned());
                *expecting_key = false;
                return Ok(());
            }
            format!("{pointer}/{}", escape(key.as_deref().unwrap_or_default()))
        }
        Some(Frame::Seq { pointer, index }) => {
            let child = format!("{pointer}/{index}");
            *index += 1;
            child
        }
    };
    raw.pointers.entry(pointer.clone()).or_insert(at);
    match node {
        Node::Scalar(value, _) => {
            if let Some(slot) = captured.iter().find(|slot| **slot == pointer) {
                raw.captured.insert((*slot).to_owned(), value.to_owned());
            }
            value_done(stack);
        }
        Node::Alias => value_done(stack),
        Node::Map => stack.push(Frame::Map {
            pointer,
            key: None,
            expecting_key: true,
        }),
        Node::Seq => stack.push(Frame::Seq { pointer, index: 0 }),
    }
    Ok(())
}

/// The byte offset of each char index the parser reports.
pub struct Offsets<'a> {
    text: &'a str,
    table: Option<Vec<usize>>,
}

impl<'a> Offsets<'a> {
    pub fn new(text: &'a str) -> Self {
        let table = (!text.is_ascii()).then(|| text.char_indices().map(|(at, _)| at).collect());
        Self { text, table }
    }

    pub fn byte(&self, char_index: usize) -> usize {
        match &self.table {
            None => char_index.min(self.text.len()),
            Some(table) => table.get(char_index).copied().unwrap_or(self.text.len()),
        }
    }
}

pub fn scan_yaml(text: &str, captured: &[&str]) -> Result<Vec<Raw>, SyntaxAt> {
    let mut done = Vec::new();
    let mut current: Option<Raw> = None;
    let mut stack: Vec<Frame> = Vec::new();
    for item in Parser::new_from_str(text) {
        let (event, span) =
            item.map_err(|err| SyntaxAt::new(err.marker().line(), err.marker().col(), err.info()))?;
        let (line, col) = (span.start.line(), span.start.col());
        let at = (line.max(1) as u64, col as u64 + 1);
        let node = match &event {
            Event::DocumentStart(explicit) => {
                current = Some(Raw {
                    start_char: span.start.index(),
                    explicit: *explicit,
                    ..Raw::default()
                });
                stack.clear();
                None
            }
            Event::DocumentEnd => {
                done.extend(current.take());
                None
            }
            Event::Scalar(value, style, ..) => Some(Node::Scalar(value, *style)),
            Event::MappingStart(..) => Some(Node::Map),
            Event::SequenceStart(..) => Some(Node::Seq),
            Event::Alias(_) => Some(Node::Alias),
            Event::MappingEnd | Event::SequenceEnd => {
                stack.pop();
                value_done(&mut stack);
                None
            }
            _ => None,
        };
        if let (Some(node), Some(raw)) = (node, current.as_mut()) {
            on_node(raw, &mut stack, node, at, line, col, captured)?;
        }
    }
    done.extend(current);
    Ok(done)
}

/// Pointer positions of one document, one-based line and char-based column.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Index {
    pub line: u64,
    pub column: u64,
    pub pointers: BTreeMap<String, (u64, u64)>,
}

impl Index {
    /// The position of a JSON pointer, falling back to its nearest known ancestor.
    pub fn locate(&self, pointer: &str) -> (u64, u64) {
        let mut at = pointer;
        loop {
            if let Some(found) = self.pointers.get(at) {
                return *found;
            }
            match at.rfind('/') {
                Some(cut) => at = &at[..cut],
                None => return (self.line, self.column),
            }
        }
    }

    fn from_raw(raw: Raw) -> Self {
        let (line, column) = raw.root.unwrap_or((1, 1));
        let mut pointers = raw.pointers;
        pointers.entry(String::new()).or_insert((line, column));
        Self {
            line,
            column,
            pointers,
        }
    }

    fn root_only() -> Self {
        Self {
            line: 1,
            column: 1,
            pointers: BTreeMap::from([(String::new(), (1, 1))]),
        }
    }
}

/// Indexes every document of a YAML stream.
pub fn yaml_documents(text: &str) -> Result<Vec<Index>, SyntaxAt> {
    Ok(scan_yaml(text, &[])?
        .into_iter()
        .map(Index::from_raw)
        .collect())
}

/// Indexes a JSON text. JSON is read through the YAML scanner; when that fails the
/// index only knows the root, so lookups land on line 1.
pub fn json_index(text: &str) -> Index {
    match scan_yaml(text, &[]) {
        Ok(mut raws) if raws.len() == 1 => Index::from_raw(raws.remove(0)),
        _ => Index::root_only(),
    }
}

/// Byte offsets to one-based line and char-based column.
struct Lines<'a> {
    text: &'a str,
    starts: Vec<usize>,
}

impl<'a> Lines<'a> {
    fn new(text: &'a str) -> Self {
        let mut starts = vec![0];
        starts.extend(text.match_indices('\n').map(|(at, _)| at + 1));
        Self { text, starts }
    }

    fn at(&self, offset: usize) -> (u64, u64) {
        let offset = offset.min(self.text.len());
        let line = self.starts.partition_point(|start| *start <= offset) - 1;
        let from = self.starts[line];
        let mut end = offset;
        while !self.text.is_char_boundary(end) {
            end -= 1;
        }
        let col = self.text[from..end].chars().count();
        (line as u64 + 1, col as u64 + 1)
    }
}

/// Indexes a TOML text. Tables, arrays of tables (`/a/0`), dotted and inline keys are covered.
pub fn toml_index(text: &str) -> Result<Index, SyntaxAt> {
    let lines = Lines::new(text);
    let doc = toml_edit::Document::parse(text).map_err(|err| {
        let at = err.span().map_or(0, |span| span.start);
        let (line, column) = lines.at(at);
        SyntaxAt::new(line as usize, column as usize - 1, err.message())
    })?;
    let mut index = Index::root_only();
    if let toml_edit::Item::Table(root) = doc.as_item() {
        walk_table(root, "", (1, 1), &lines, &mut index.pointers);
    }
    Ok(index)
}

fn start(span: Option<std::ops::Range<usize>>, lines: &Lines<'_>, or: (u64, u64)) -> (u64, u64) {
    span.map_or(or, |span| lines.at(span.start))
}

fn walk_table(
    table: &toml_edit::Table,
    pointer: &str,
    parent: (u64, u64),
    lines: &Lines<'_>,
    out: &mut BTreeMap<String, (u64, u64)>,
) {
    for (name, item) in table.iter() {
        let key_at = table.key(name).and_then(|key| key.span());
        let at = start(key_at.or_else(|| item.span()), lines, parent);
        walk_item(item, &format!("{pointer}/{}", escape(name)), at, lines, out);
    }
}

fn walk_inline(
    table: &toml_edit::InlineTable,
    pointer: &str,
    parent: (u64, u64),
    lines: &Lines<'_>,
    out: &mut BTreeMap<String, (u64, u64)>,
) {
    for (name, value) in table.iter() {
        let key_at = table.key(name).and_then(|key| key.span());
        let at = start(key_at.or_else(|| value.span()), lines, parent);
        walk_value(
            value,
            &format!("{pointer}/{}", escape(name)),
            at,
            lines,
            out,
        );
    }
}

fn walk_item(
    item: &toml_edit::Item,
    pointer: &str,
    at: (u64, u64),
    lines: &Lines<'_>,
    out: &mut BTreeMap<String, (u64, u64)>,
) {
    out.entry(pointer.to_owned()).or_insert(at);
    match item {
        toml_edit::Item::Table(table) => walk_table(table, pointer, at, lines, out),
        toml_edit::Item::ArrayOfTables(tables) => {
            for (n, table) in tables.iter().enumerate() {
                let here = start(table.span(), lines, at);
                let pointer = format!("{pointer}/{n}");
                out.entry(pointer.clone()).or_insert(here);
                walk_table(table, &pointer, here, lines, out);
            }
        }
        toml_edit::Item::Value(value) => walk_value(value, pointer, at, lines, out),
        toml_edit::Item::None => {}
    }
}

fn walk_value(
    value: &toml_edit::Value,
    pointer: &str,
    at: (u64, u64),
    lines: &Lines<'_>,
    out: &mut BTreeMap<String, (u64, u64)>,
) {
    out.entry(pointer.to_owned()).or_insert(at);
    match value {
        toml_edit::Value::Array(array) => {
            for (n, member) in array.iter().enumerate() {
                let here = start(member.span(), lines, at);
                walk_value(member, &format!("{pointer}/{n}"), here, lines, out);
            }
        }
        toml_edit::Value::InlineTable(table) => walk_inline(table, pointer, at, lines, out),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn yaml_anchors_aliases_and_styles() {
        let text = "base: &b\n  x: 1\nuse: *b\nflow: {a: [1, 2], b: c}\n";
        let docs = yaml_documents(text).expect("yaml");
        assert_eq!(docs.len(), 1);
        let doc = &docs[0];
        assert_eq!(doc.locate("/base/x"), (2, 3));
        assert_eq!(doc.locate("/use"), (3, 1));
        assert_eq!(doc.locate("/flow/a/1"), (4, 15));
        assert_eq!(doc.locate("/flow/b"), (4, 19));
        assert_eq!(doc.locate("/flow/missing/deeper"), (4, 1));
    }

    #[test]
    fn yaml_columns_count_chars() {
        let docs = yaml_documents("\u{e5}\u{e5}: 1\nk: \u{f8}\u{f8}\nz: 2\n").expect("yaml");
        assert_eq!(docs[0].locate("/z"), (3, 1));
        let docs = yaml_documents("k: { \u{f8}: 1, b: 2 }\n").expect("yaml");
        assert_eq!(docs[0].locate("/k/b"), (1, 12));
    }

    #[test]
    fn yaml_syntax_errors_and_many_documents() {
        assert!(yaml_documents("a: [1\nb: 2\n").is_err());
        assert_eq!(yaml_documents("a: 1\n---\nb: 2\n").expect("yaml").len(), 2);
    }

    #[test]
    fn json_goes_through_the_yaml_scanner_with_a_root_fallback() {
        let index = json_index("{\n  \"a\": [\n    1\n  ]\n}");
        assert_eq!(index.locate("/a/0"), (3, 5));
        let broken = json_index("{\"a\": ");
        assert_eq!(broken.locate("/a"), (1, 1));
    }

    #[test]
    fn toml_tables_arrays_dotted_and_inline_keys() {
        let text = "a.b = 1\n[t]\nx = 2\n[[h]]\nn = 1\n[[h]]\nn = 2\ni = { p = 3 }\n";
        let index = toml_index(text).expect("toml");
        assert_eq!(index.locate("/a/b"), (1, 3));
        assert_eq!(index.locate("/t/x"), (3, 1));
        assert_eq!(index.locate("/h/0/n"), (5, 1));
        assert_eq!(index.locate("/h/1/n"), (7, 1));
        assert_eq!(index.locate("/h/1/i/p"), (8, 7));
        assert!(toml_index("a = \n").is_err());
    }

    #[test]
    fn escape_follows_json_pointer() {
        assert_eq!(escape("a/b~c"), "a~1b~0c");
    }
}
