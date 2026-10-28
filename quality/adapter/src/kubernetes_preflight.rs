//! Offline checks dx runs on Kubernetes manifests before kubeconform sees them.

use std::collections::BTreeMap;

use saphyr_parser::{Event, Parser, ScalarStyle};

/// A file that does not parse as YAML or JSON, or is too large or not UTF-8.
pub const RULE_SYNTAX: &str = "dx/kubernetes-syntax";
/// A non-empty document with neither `apiVersion` nor `kind`.
pub const RULE_NOT_MANIFEST: &str = "dx/kubernetes-not-manifest";

/// The largest manifest file dx hands to kubeconform.
pub const MAX_MANIFEST_BYTES: usize = 2 * 1024 * 1024;

const BOM: char = '\u{feff}';

/// One problem the preflight found, placed on the file it was checking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    pub rule: &'static str,
    pub line: u64,
    pub column: u64,
    pub message: String,
}

/// One manifest document: its bytes, identity and pointer positions in the whole file.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Document {
    pub start: usize,
    pub end: usize,
    pub line: u64,
    pub column: u64,
    pub text: String,
    pub api_version: Option<String>,
    pub kind: Option<String>,
    pub name: Option<String>,
    pub pointers: BTreeMap<String, (u64, u64)>,
}

impl Document {
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
}

/// The documents of one file that go to kubeconform, and the problems that keep others back.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Preflight {
    pub documents: Vec<Document>,
    pub problems: Vec<Problem>,
}

/// Escapes one JSON pointer reference token.
pub fn escape(token: &str) -> String {
    token.replace('~', "~0").replace('/', "~1")
}

fn syntax(line: usize, col: usize, detail: &str) -> Problem {
    Problem {
        rule: RULE_SYNTAX,
        line: line.max(1) as u64,
        column: col as u64 + 1,
        message: format!("not valid YAML or JSON: {detail}"),
    }
}

enum Frame {
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

enum Node<'a> {
    Scalar(&'a str, ScalarStyle),
    Map,
    Seq,
    Alias,
}

#[derive(Default)]
struct Raw {
    start_char: usize,
    explicit: bool,
    root: Option<(u64, u64)>,
    empty: bool,
    pointers: BTreeMap<String, (u64, u64)>,
    captured: BTreeMap<&'static str, String>,
}

const CAPTURED: &[&str] = &["/apiVersion", "/kind", "/metadata/name"];

fn value_done(stack: &mut [Frame]) {
    if let Some(Frame::Map { expecting_key, .. }) = stack.last_mut() {
        *expecting_key = true;
    }
}

fn is_null(value: &str, style: ScalarStyle) -> bool {
    style == ScalarStyle::Plain && matches!(value, "" | "~" | "null" | "Null" | "NULL")
}

fn on_node(
    raw: &mut Raw,
    stack: &mut Vec<Frame>,
    node: Node<'_>,
    at: (u64, u64),
    line: usize,
    col: usize,
) -> Result<(), Problem> {
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
                    return Err(syntax(line, col, "a mapping, sequence or alias used as a key"));
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
            if let Some(slot) = CAPTURED.iter().find(|slot| **slot == pointer) {
                raw.captured.insert(slot, value.to_owned());
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
struct Offsets<'a> {
    text: &'a str,
    table: Option<Vec<usize>>,
}

impl<'a> Offsets<'a> {
    fn new(text: &'a str) -> Self {
        let table = (!text.is_ascii()).then(|| text.char_indices().map(|(at, _)| at).collect());
        Self { text, table }
    }

    fn byte(&self, char_index: usize) -> usize {
        match &self.table {
            None => char_index.min(self.text.len()),
            Some(table) => table.get(char_index).copied().unwrap_or(self.text.len()),
        }
    }
}

fn scan_yaml(text: &str) -> Result<Vec<Raw>, Problem> {
    let mut done = Vec::new();
    let mut current: Option<Raw> = None;
    let mut stack: Vec<Frame> = Vec::new();
    for item in Parser::new_from_str(text) {
        let (event, span) =
            item.map_err(|err| syntax(err.marker().line(), err.marker().col(), err.info()))?;
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
            on_node(raw, &mut stack, node, at, line, col)?;
        }
    }
    done.extend(current);
    Ok(done)
}

fn decode(bytes: &[u8]) -> Result<(&str, usize), Problem> {
    if bytes.len() > MAX_MANIFEST_BYTES {
        return Err(syntax(
            1,
            0,
            &format!("the file is larger than {MAX_MANIFEST_BYTES} bytes"),
        ));
    }
    let text = std::str::from_utf8(bytes).map_err(|_| syntax(1, 0, "the file is not UTF-8"))?;
    Ok(match text.strip_prefix(BOM) {
        Some(rest) => (rest, BOM.len_utf8()),
        None => (text, 0),
    })
}

fn finish(raw: Raw, start: usize, end: usize, text: &str, bom: usize) -> Document {
    let (line, column) = raw.root.unwrap_or((1, 1));
    let mut slice = &text[start..end];
    if raw.explicit {
        slice = slice.strip_prefix("---").unwrap_or(slice);
    }
    let mut pointers = raw.pointers;
    pointers.entry(String::new()).or_insert((line, column));
    let take = |captured: &BTreeMap<&str, String>, key: &str| captured.get(key).cloned();
    Document {
        start: start + bom,
        end: end + bom,
        line,
        column,
        text: slice.to_owned(),
        api_version: take(&raw.captured, "/apiVersion"),
        kind: take(&raw.captured, "/kind"),
        name: take(&raw.captured, "/metadata/name"),
        pointers,
    }
}

fn yaml_documents(text: &str, bom: usize) -> Result<Vec<(Document, bool)>, Problem> {
    let raws = scan_yaml(text)?;
    let offsets = Offsets::new(text);
    let starts: Vec<usize> = raws.iter().map(|raw| offsets.byte(raw.start_char)).collect();
    Ok(raws
        .into_iter()
        .enumerate()
        .map(|(index, raw)| {
            let start = starts[index];
            let end = starts.get(index + 1).copied().unwrap_or(text.len()).max(start);
            let empty = raw.empty;
            (finish(raw, start, end, text, bom), empty)
        })
        .collect())
}

fn json_document(text: &str, bom: usize) -> Result<Option<Document>, Problem> {
    if text.trim().is_empty() {
        return Ok(None);
    }
    let value: serde_json::Value = serde_json::from_str(text)
        .map_err(|err| syntax(err.line(), err.column().saturating_sub(1), &err.to_string()))?;
    let field = |pointer: &str| {
        value
            .pointer(pointer)
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned)
    };
    let mut document = match scan_yaml(text) {
        Ok(mut raws) if raws.len() == 1 => finish(raws.remove(0), 0, text.len(), text, bom),
        _ => Document {
            start: bom,
            end: text.len() + bom,
            line: 1,
            column: 1,
            pointers: BTreeMap::from([(String::new(), (1, 1))]),
            ..Document::default()
        },
    };
    document.text = text.to_owned();
    document.api_version = field("/apiVersion");
    document.kind = field("/kind");
    document.name = field("/metadata/name");
    Ok((!value.is_null()).then_some(document))
}

/// Splits one owned file into the documents kubeconform checks and the problems it never sees.
pub fn check_file(path: &str, bytes: &[u8]) -> Preflight {
    let mut out = Preflight::default();
    let (text, bom) = match decode(bytes) {
        Ok(decoded) => decoded,
        Err(problem) => {
            out.problems.push(problem);
            return out;
        }
    };
    let documents = if path.ends_with(".json") {
        json_document(text, bom).map(|document| document.map(|d| (d, false)).into_iter().collect())
    } else {
        yaml_documents(text, bom)
    };
    let documents = match documents {
        Ok(documents) => documents,
        Err(problem) => {
            out.problems.push(problem);
            return out;
        }
    };
    for (document, empty) in documents {
        if empty {
            continue;
        }
        if document.api_version.is_none() && document.kind.is_none() {
            out.problems.push(Problem {
                rule: RULE_NOT_MANIFEST,
                line: document.line,
                column: document.column,
                message: "not a Kubernetes manifest: the document has neither apiVersion nor kind"
                    .to_owned(),
            });
            continue;
        }
        out.documents.push(document);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const TWO: &str = "# lead\napiVersion: v1\nkind: Pod\nmetadata:\n  name: a\nspec:\n  containers:\n    - name: c\n      image: x\n---\napiVersion: apps/v1\nkind: Deployment\nmetadata:\n  name: \"b/c~d\"\n";

    #[test]
    fn splits_documents_with_identity_and_ranges() {
        let checked = check_file("m.yaml", TWO.as_bytes());
        assert!(checked.problems.is_empty(), "{:?}", checked.problems);
        let [first, second] = checked.documents.as_slice() else {
            panic!("two documents: {:?}", checked.documents);
        };
        assert_eq!(first.kind.as_deref(), Some("Pod"));
        assert_eq!(first.api_version.as_deref(), Some("v1"));
        assert_eq!(first.name.as_deref(), Some("a"));
        assert_eq!((first.line, first.column), (2, 1));
        assert_eq!(second.kind.as_deref(), Some("Deployment"));
        assert_eq!(second.name.as_deref(), Some("b/c~d"));
        assert_eq!(second.line, 11);
        assert!(first.text.contains("kind: Pod"));
        assert!(!first.text.contains("Deployment"));
        assert!(!second.text.contains("---"));
        assert!(second.text.contains("kind: Deployment"));
        assert_eq!(&TWO[second.start..second.start + 3], "---");
        assert_eq!(second.end, TWO.len());
    }

    #[test]
    fn maps_pointers_to_keys_and_items() {
        let checked = check_file("m.yaml", TWO.as_bytes());
        let first = &checked.documents[0];
        assert_eq!(first.pointers.get("/spec"), Some(&(6, 1)));
        assert_eq!(first.pointers.get("/spec/containers"), Some(&(7, 3)));
        assert_eq!(first.pointers.get("/spec/containers/0"), Some(&(8, 7)));
        assert_eq!(first.pointers.get("/spec/containers/0/image"), Some(&(9, 7)));
        assert_eq!(first.locate("/spec/containers/0/missing/deeper"), (8, 7));
        assert_eq!(first.locate(""), (2, 1));
        assert_eq!(first.locate("/nowhere"), (2, 1));
    }

    #[test]
    fn escapes_pointer_tokens() {
        let checked = check_file("m.yaml", b"apiVersion: v1\nkind: ConfigMap\ndata:\n  a/b~c: x\n");
        let document = &checked.documents[0];
        assert_eq!(document.pointers.get("/data/a~1b~0c"), Some(&(4, 3)));
    }

    #[test]
    fn positions_after_multibyte_text_stay_right() {
        let text = "# é\napiVersion: v1\nkind: Pod\n---\n# ü\napiVersion: v1\nkind: Service\n";
        let checked = check_file("m.yaml", text.as_bytes());
        let second = &checked.documents[1];
        assert_eq!(&text[second.start..second.start + 3], "---");
        assert_eq!(second.line, 6);
        assert!(second.text.contains("kind: Service"));
    }

    #[test]
    fn empty_documents_are_skipped() {
        let checked = check_file("m.yaml", b"---\n---\napiVersion: v1\nkind: Pod\n---\n");
        assert!(checked.problems.is_empty(), "{:?}", checked.problems);
        assert_eq!(checked.documents.len(), 1);
        assert!(check_file("m.yaml", b"").documents.is_empty());
        assert!(check_file("m.yaml", b"# only\n").documents.is_empty());
    }

    #[test]
    fn documents_without_identity_are_not_manifests() {
        let checked = check_file("m.yaml", b"apiVersion: v1\nkind: Pod\n---\nfoo: bar\n");
        assert_eq!(checked.documents.len(), 1);
        assert_eq!(checked.problems.len(), 1);
        assert_eq!(checked.problems[0].rule, RULE_NOT_MANIFEST);
        assert_eq!(checked.problems[0].line, 4);
        let only_kind = check_file("m.yaml", b"kind: Pod\n");
        assert!(only_kind.problems.is_empty());
        assert_eq!(only_kind.documents.len(), 1);
        let list = check_file("m.yaml", b"- a\n");
        assert_eq!(list.problems[0].rule, RULE_NOT_MANIFEST);
    }

    #[test]
    fn json_stays_one_document() {
        let text = "{\n  \"apiVersion\": \"v1\",\n  \"kind\": \"Pod\",\n  \"spec\": {\"x\": 1}\n}\n";
        let checked = check_file("p.json", text.as_bytes());
        assert!(checked.problems.is_empty(), "{:?}", checked.problems);
        let [document] = checked.documents.as_slice() else {
            panic!("one document");
        };
        assert_eq!(document.kind.as_deref(), Some("Pod"));
        assert_eq!(document.text, text);
        assert_eq!(document.locate("/spec/x").0, 4);
        let tabs = "{\n\t\"apiVersion\": \"v1\",\n\t\"kind\": \"Pod\"\n}\n";
        let checked = check_file("p.json", tabs.as_bytes());
        assert_eq!(checked.documents[0].kind.as_deref(), Some("Pod"));
        let plain = check_file("p.json", br#"{"name":"x"}"#);
        assert_eq!(plain.problems[0].rule, RULE_NOT_MANIFEST);
    }

    #[test]
    fn syntax_errors_are_located() {
        let checked = check_file("m.yaml", b"apiVersion: v1\nkind: [\n");
        assert_eq!(checked.problems[0].rule, RULE_SYNTAX);
        assert!(checked.documents.is_empty());
        let checked = check_file("p.json", b"{\n  \"a\": }");
        assert_eq!(checked.problems[0].rule, RULE_SYNTAX);
        assert_eq!(checked.problems[0].line, 2);
        let checked = check_file("m.yaml", b"? [a]\n: b\n");
        assert_eq!(checked.problems[0].rule, RULE_SYNTAX);
    }

    #[test]
    fn invalid_utf8_and_oversize_fail() {
        let checked = check_file("m.yaml", &[0xff, 0xfe, 0x00]);
        assert_eq!(checked.problems[0].rule, RULE_SYNTAX);
        let big = vec![b'a'; MAX_MANIFEST_BYTES + 1];
        assert_eq!(check_file("m.yaml", &big).problems[0].rule, RULE_SYNTAX);
    }

    #[test]
    fn a_bom_shifts_byte_ranges_only() {
        let text = "\u{feff}apiVersion: v1\nkind: Pod\n";
        let checked = check_file("m.yaml", text.as_bytes());
        let document = &checked.documents[0];
        assert_eq!(document.start, 3);
        assert_eq!(document.end, text.len());
        assert_eq!((document.line, document.column), (1, 1));
    }
}
