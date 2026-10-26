use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ActionDiagnostic {
    pub(super) severity: &'static str,
    pub(super) code: Option<String>,
    pub(super) message: String,
    pub(super) file: Option<String>,
    pub(super) line: Option<u64>,
    pub(super) column: Option<u64>,
    pub(super) related: Vec<ActionRelated>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ActionRelated {
    pub(super) role: &'static str,
    pub(super) message: String,
    pub(super) file: Option<String>,
    pub(super) line: Option<u64>,
    pub(super) column: Option<u64>,
    pub(super) replacement: Option<String>,
    pub(super) applicability: Option<String>,
}

const MAX_RELATED: usize = 32;

#[derive(Deserialize)]
struct Message {
    message: String,
    code: Option<Code>,
    level: String,
    #[serde(default)]
    spans: Vec<Span>,
    #[serde(default)]
    children: Vec<serde_json::Value>,
}

#[derive(Deserialize)]
struct Code {
    code: String,
}

#[derive(Deserialize)]
struct Span {
    file_name: String,
    line_start: u64,
    column_start: u64,
    is_primary: bool,
    label: Option<String>,
    suggested_replacement: Option<String>,
    suggestion_applicability: Option<String>,
}

#[derive(Deserialize)]
struct Child {
    message: String,
    level: String,
    #[serde(default)]
    spans: Vec<Span>,
    #[serde(default)]
    children: Vec<serde_json::Value>,
}

fn first_line(text: &str) -> &str {
    text.lines().next().unwrap_or("").trim()
}

fn positive(value: u64) -> Option<u64> {
    Some(value).filter(|value| *value > 0)
}

fn span_label(span: &Span) -> Option<ActionRelated> {
    let message = first_line(span.label.as_deref()?);
    (!message.is_empty()).then(|| ActionRelated {
        role: "label",
        message: message.to_owned(),
        file: Some(span.file_name.clone()),
        line: positive(span.line_start),
        column: positive(span.column_start),
        replacement: None,
        applicability: None,
    })
}

fn collect_children(children: &[serde_json::Value], out: &mut Vec<ActionRelated>) {
    for value in children {
        if out.len() >= MAX_RELATED {
            return;
        }
        let Ok(child) = serde_json::from_value::<Child>(value.clone()) else {
            continue;
        };
        let role = match child.level.as_str() {
            "note" => "note",
            "help" => "help",
            _ => continue,
        };
        let message = first_line(&child.message);
        if !message.is_empty() {
            let span = child
                .spans
                .iter()
                .find(|span| span.suggested_replacement.is_some())
                .or_else(|| child.spans.iter().find(|span| span.is_primary))
                .or_else(|| child.spans.first());
            let replacement = span.and_then(|span| span.suggested_replacement.clone());
            let applicability = replacement
                .as_ref()
                .and_then(|_| span.and_then(|span| span.suggestion_applicability.clone()));
            out.push(ActionRelated {
                role,
                message: message.to_owned(),
                file: span.map(|span| span.file_name.clone()),
                line: span.and_then(|span| positive(span.line_start)),
                column: span.and_then(|span| positive(span.column_start)),
                replacement,
                applicability,
            });
        }
        collect_children(&child.children, out);
    }
}

fn collect_related(diagnostic: &Message) -> Vec<ActionRelated> {
    if diagnostic.message.starts_with("linking with") {
        return Vec::new();
    }
    let mut out: Vec<ActionRelated> = diagnostic
        .spans
        .iter()
        .filter(|span| span.is_primary)
        .chain(diagnostic.spans.iter().filter(|span| !span.is_primary))
        .filter_map(span_label)
        .collect();
    collect_children(&diagnostic.children, &mut out);
    out.truncate(MAX_RELATED);
    out
}

pub(super) fn parse(stderr: &[u8]) -> Vec<ActionDiagnostic> {
    let text = String::from_utf8_lossy(stderr);
    let mut found = Vec::new();
    let mut saw_json = false;
    for line in text.lines() {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if value
            .get("$message_type")
            .and_then(serde_json::Value::as_str)
            != Some("diagnostic")
        {
            continue;
        }
        saw_json = true;
        let Ok(diagnostic) = serde_json::from_value::<Message>(value) else {
            continue;
        };
        let severity = if diagnostic.level.starts_with("error") {
            "error"
        } else if diagnostic.level == "warning" {
            "warning"
        } else {
            continue;
        };
        let span = diagnostic
            .spans
            .iter()
            .find(|span| span.is_primary)
            .or_else(|| diagnostic.spans.first());
        if span.is_none()
            && (diagnostic.message.starts_with("aborting due to")
                || diagnostic.message.ends_with(" emitted"))
        {
            continue;
        }
        let related = collect_related(&diagnostic);
        found.push(ActionDiagnostic {
            severity,
            code: diagnostic.code.map(|code| code.code),
            message: diagnostic.message,
            file: span.map(|span| span.file_name.clone()),
            line: span.map(|span| span.line_start).filter(|line| *line > 0),
            column: span
                .map(|span| span.column_start)
                .filter(|column| *column > 0),
            related,
        });
    }
    if !saw_json {
        found = parse_rendered(&strip_ansi(&text));
    }
    found
}

fn strip_ansi(text: &str) -> String {
    let mut plain = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(ch) = chars.next() {
        if ch != '\u{1b}' {
            plain.push(ch);
            continue;
        }
        if chars.next() == Some('[') {
            for next in chars.by_ref() {
                if ('\u{40}'..='\u{7e}').contains(&next) {
                    break;
                }
            }
        }
    }
    plain
}

fn rendered_header(line: &str) -> Option<(&'static str, Option<String>, &str)> {
    let (severity, rest) = line
        .strip_prefix("error")
        .map(|rest| ("error", rest))
        .or_else(|| line.strip_prefix("warning").map(|rest| ("warning", rest)))?;
    let (code, rest) = match rest.strip_prefix('[') {
        Some(bracketed) => {
            let (code, tail) = bracketed.split_once(']')?;
            if code.is_empty() || code.contains(char::is_whitespace) {
                return None;
            }
            (Some(code.to_owned()), tail)
        }
        None => (None, rest),
    };
    let message = rest.strip_prefix(": ")?.trim_end();
    (!message.is_empty()).then_some((severity, code, message))
}

fn rendered_location(line: &str) -> Option<(String, u64, u64)> {
    let trimmed = line.trim_start();
    let location = trimmed
        .strip_prefix("--> ")
        .or_else(|| trimmed.strip_prefix("::: "))?
        .trim();
    let mut parts = location.rsplitn(3, ':');
    let column = parts.next()?.parse().ok()?;
    let line = parts.next()?.parse().ok()?;
    let file = parts.next().filter(|file| !file.is_empty())?;
    Some((file.to_owned(), line, column))
}

struct Gutter<'a> {
    number: Option<u64>,
    content: &'a str,
}

fn gutter(line: &str) -> Option<Gutter<'_>> {
    let trimmed = line.trim_start();
    let digits = trimmed.chars().take_while(char::is_ascii_digit).count();
    let content = trimmed[digits..].trim_start().strip_prefix('|')?;
    let number = if digits > 0 {
        Some(trimmed[..digits].parse().ok()?)
    } else {
        None
    };
    Some(Gutter { number, content })
}

fn underline_label(content: &str) -> Option<&str> {
    let mut rest = content.trim();
    if !rest.starts_with(['^', '-']) {
        return None;
    }
    while rest.starts_with(['^', '-']) {
        rest = rest.trim_start_matches(['^', '-']).trim_start();
    }
    let text = first_line(rest);
    (!text.is_empty()).then_some(text)
}

fn rendered_related(role: &'static str, message: &str) -> ActionRelated {
    ActionRelated {
        role,
        message: message.to_owned(),
        file: None,
        line: None,
        column: None,
        replacement: None,
        applicability: None,
    }
}

fn parse_rendered(text: &str) -> Vec<ActionDiagnostic> {
    let mut found: Vec<ActionDiagnostic> = Vec::new();
    let mut open = false;
    let mut file: Option<String> = None;
    let mut source_line: Option<u64> = None;
    let mut pending: Option<usize> = None;
    let mut connector = false;
    for line in text.lines() {
        if let Some((severity, code, message)) = rendered_header(line) {
            found.push(ActionDiagnostic {
                severity,
                code,
                message: message.to_owned(),
                file: None,
                line: None,
                column: None,
                related: Vec::new(),
            });
            open = true;
            file = None;
            source_line = None;
            pending = None;
            connector = false;
            continue;
        }
        if line.trim().is_empty() {
            open = false;
            continue;
        }
        if !open {
            continue;
        }
        let Some(last) = found.last_mut() else {
            continue;
        };
        let was_connector = std::mem::take(&mut connector);
        if let Some((path, row, column)) = rendered_location(line) {
            file = Some(path.clone());
            source_line = None;
            if let Some(index) = pending.take() {
                if let Some(item) = last.related.get_mut(index) {
                    item.file = Some(path);
                    item.line = positive(row);
                    item.column = positive(column);
                }
            } else if last.file.is_none() {
                last.file = Some(path);
                last.line = positive(row);
                last.column = positive(column);
            }
            continue;
        }
        let trimmed = line.trim_start();
        if let Some(message) = trimmed
            .strip_prefix("= note: ")
            .map(|m| ("note", m))
            .or_else(|| trimmed.strip_prefix("= help: ").map(|m| ("help", m)))
        {
            push_related(last, rendered_related(message.0, first_line(message.1)));
            continue;
        }
        if let Some(message) = line
            .strip_prefix("note: ")
            .map(|m| ("note", m))
            .or_else(|| line.strip_prefix("help: ").map(|m| ("help", m)))
        {
            if push_related(last, rendered_related(message.0, first_line(message.1))) {
                pending = Some(last.related.len() - 1);
            }
            continue;
        }
        let Some(gutter) = gutter(line) else {
            continue;
        };
        if let Some(number) = gutter.number {
            source_line = Some(number);
            continue;
        }
        let content = gutter.content.trim();
        let label = underline_label(content).or_else(|| {
            (was_connector && !content.is_empty() && !content.starts_with('='))
                .then(|| first_line(content))
        });
        if let Some(label) = label {
            let mut item = rendered_related("label", label);
            item.file = file.clone();
            item.line = source_line;
            push_related(last, item);
        }
        connector = !content.is_empty() && content.chars().all(|c| c == '|' || c == ' ');
    }
    for item in &mut found {
        if item.message.starts_with("linking with") {
            item.related.clear();
        }
    }
    found.retain(|item| {
        !item.message.starts_with("could not compile")
            && (item.file.is_some()
                || !(item.message.starts_with("aborting due to")
                    || item.message.ends_with(" emitted")))
    });
    found
}

fn push_related(diagnostic: &mut ActionDiagnostic, item: ActionRelated) -> bool {
    if item.message.is_empty() || diagnostic.related.len() >= MAX_RELATED {
        return false;
    }
    diagnostic.related.push(item);
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    const CLIPPY_BROKEN: &str = r#"{"$message_type":"diagnostic","message":"this file contains an unclosed delimiter","code":null,"level":"error","spans":[{"file_name":"/s/broken.rs","byte_start":11,"byte_end":11,"line_start":1,"line_end":1,"column_start":12,"column_end":12,"is_primary":true,"text":[],"label":null,"suggested_replacement":null,"suggestion_applicability":null,"expansion":null}],"children":[],"rendered":null}
{"$message_type":"diagnostic","message":"aborting due to 1 previous error","code":null,"level":"error","spans":[],"children":[],"rendered":null}"#;

    #[test]
    fn action_output_reads_errors_and_skips_text_and_summaries() {
        let mut input = String::from("compiling\n");
        input.push_str(CLIPPY_BROKEN);
        input.push_str("\nnot json\n");
        let found = parse(input.as_bytes());
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].severity, "error");
        assert_eq!(found[0].file.as_deref(), Some("/s/broken.rs"));
        assert_eq!((found[0].line, found[0].column), (Some(1), Some(12)));
    }

    #[test]
    fn action_output_keeps_spanless_link_errors() {
        let line = r#"{"$message_type":"diagnostic","message":"linking with `cc` failed: exit status: 1","code":null,"level":"error","spans":[],"children":[],"rendered":null}"#;
        let found = parse(line.as_bytes());
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].file, None);
        assert_eq!(found[0].line, None);
        assert!(found[0].related.is_empty());
    }

    #[test]
    fn json_link_errors_carry_no_related_items() {
        let line = r#"{"$message_type":"diagnostic","message":"linking with `cc` failed: exit status: 1","code":null,"level":"error","spans":[],"children":[{"message":"\"cc\" \"-m64\"","level":"note","spans":[],"children":[]}],"rendered":null}"#;
        let found = parse(line.as_bytes());
        assert_eq!(found.len(), 1);
        assert!(found[0].related.is_empty(), "{found:?}");
    }

    const JSON_E0308: &str = r#"{"$message_type":"diagnostic","message":"mismatched types","code":{"code":"E0308","explanation":null},"level":"error","spans":[{"file_name":"src/other.rs","line_start":3,"column_start":14,"is_primary":false,"label":"expected due to this","suggested_replacement":null,"suggestion_applicability":null},{"file_name":"src/main.rs","line_start":4,"column_start":18,"is_primary":true,"label":"expected `i32`, found `&str`","suggested_replacement":null,"suggestion_applicability":null}],"children":[{"message":"the item is defined here\nsecond line","level":"note","spans":[],"children":[]},{"message":"try this","level":"help","spans":[{"file_name":"src/main.rs","line_start":4,"column_start":18,"is_primary":true,"label":null,"suggested_replacement":"1","suggestion_applicability":"MaybeIncorrect"}],"children":[]},{"message":"bad","level":"help","spans":"nope"},42],"rendered":null}"#;

    #[test]
    fn json_errors_carry_labels_notes_and_helps() {
        let found = parse(JSON_E0308.as_bytes());
        assert_eq!(found.len(), 1, "{found:?}");
        let related = &found[0].related;
        assert_eq!(related.len(), 4, "{related:?}");
        assert_eq!(related[0].role, "label");
        assert_eq!(related[0].message, "expected `i32`, found `&str`");
        assert_eq!(related[0].file.as_deref(), Some("src/main.rs"));
        assert_eq!((related[0].line, related[0].column), (Some(4), Some(18)));
        assert_eq!(related[1].message, "expected due to this");
        assert_eq!(related[1].file.as_deref(), Some("src/other.rs"));
        assert_eq!(related[2].role, "note");
        assert_eq!(related[2].message, "the item is defined here");
        assert_eq!(related[2].file, None);
        assert_eq!(related[3].role, "help");
        assert_eq!(related[3].replacement.as_deref(), Some("1"));
        assert_eq!(related[3].applicability.as_deref(), Some("MaybeIncorrect"));
        assert_eq!(related[0].applicability, None);
    }

    #[test]
    fn rendered_text_handles_crlf_and_non_ascii_paths() {
        let input = "error[E0308]: mismatched types\r\n --> src/æøå.rs:4:18\r\n  |\r\n4 | let x: i32 = \"a\";\r\n  |              ^^^ expected `i32`, found `&str`\r\n\r\n";
        let found = parse(input.as_bytes());
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].file.as_deref(), Some("src/æøå.rs"));
        assert_eq!(found[0].related.len(), 1);
        assert_eq!(found[0].related[0].file.as_deref(), Some("src/æøå.rs"));
        assert_eq!(found[0].related[0].line, Some(4));
    }

    #[test]
    fn rendered_related_does_not_leak_into_the_next_diagnostic() {
        let input = "warning: a\n --> x.rs:1:1\n  |\n1 | a\n  | ^ one\n\nwarning: b\n --> y.rs:2:2\n  = note: two\n";
        let found = parse(input.as_bytes());
        assert_eq!(found.len(), 2, "{found:?}");
        assert_eq!(found[0].related.len(), 1);
        assert_eq!(found[1].related.len(), 1);
        assert_eq!(found[1].related[0].message, "two");
    }

    const RENDERED_E0308: &str = "\u{1b}[0m\u{1b}[1m\u{1b}[38;5;9merror[E0308]\u{1b}[0m\u{1b}[0m\u{1b}[1m: mismatched types\u{1b}[0m
\u{1b}[0m   \u{1b}[0m\u{1b}[0m\u{1b}[1m\u{1b}[38;5;12m--> \u{1b}[0m\u{1b}[0mcli/cli/src/main.rs:12:26\u{1b}[0m
\u{1b}[0m   \u{1b}[0m\u{1b}[0m\u{1b}[1m\u{1b}[38;5;12m|\u{1b}[0m
\u{1b}[0m\u{1b}[1m\u{1b}[38;5;12m12\u{1b}[0m \u{1b}[0m\u{1b}[0m\u{1b}[1m\u{1b}[38;5;12m|\u{1b}[0m fn _dx_broken() -> u32 { \"x\" }
\u{1b}[0m   \u{1b}[0m\u{1b}[0m\u{1b}[1m\u{1b}[38;5;12m|\u{1b}[0m                    \u{1b}[0m\u{1b}[0m\u{1b}[1m\u{1b}[38;5;9m^^^\u{1b}[0m expected `u32`, found `&str`

\u{1b}[0m\u{1b}[1m\u{1b}[38;5;9merror\u{1b}[0m\u{1b}[0m\u{1b}[1m: aborting due to 1 previous error\u{1b}[0m

\u{1b}[0m\u{1b}[1mFor more information about this error, try `rustc --explain E0308`.\u{1b}[0m
";

    #[test]
    fn action_output_reads_rendered_errors_with_ansi_codes() {
        let found = parse(RENDERED_E0308.as_bytes());
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].severity, "error");
        assert_eq!(found[0].code.as_deref(), Some("E0308"));
        assert_eq!(found[0].message, "mismatched types");
        assert_eq!(found[0].file.as_deref(), Some("cli/cli/src/main.rs"));
        assert_eq!((found[0].line, found[0].column), (Some(12), Some(26)));
        assert_eq!(found[0].related.len(), 1, "{found:?}");
        let label = &found[0].related[0];
        assert_eq!(label.role, "label");
        assert_eq!(label.message, "expected `u32`, found `&str`");
        assert_eq!(label.file.as_deref(), Some("cli/cli/src/main.rs"));
        assert_eq!((label.line, label.column), (Some(12), None));
    }

    #[test]
    fn action_output_reads_rendered_warnings_and_errors_in_order() {
        let input = "warning: length comparison to zero
 --> src/len.rs:2:8
  |
2 |     if \"x\".len() == 0 {}
  |        ^^^^^^^^^^^^^^ help: using `is_empty` is clearer and more explicit
  = note: `#[warn(clippy::len_zero)]` on by default

error[E0425]: cannot find value `y` in this scope
   --> C:\\work\\src\\lib.rs:7:5
    |
note: the item is defined here
   --> src/other.rs:1:1

warning: 1 warning emitted

error: could not compile `app` (lib) due to 1 previous error
";
        let found = parse(input.as_bytes());
        assert_eq!(found.len(), 2, "{found:?}");
        assert_eq!(found[0].severity, "warning");
        assert_eq!(found[0].code, None);
        assert_eq!(found[0].message, "length comparison to zero");
        assert_eq!(found[0].file.as_deref(), Some("src/len.rs"));
        assert_eq!((found[0].line, found[0].column), (Some(2), Some(8)));
        assert_eq!(found[1].severity, "error");
        assert_eq!(found[1].code.as_deref(), Some("E0425"));
        assert_eq!(found[1].file.as_deref(), Some("C:\\work\\src\\lib.rs"));
        assert_eq!((found[1].line, found[1].column), (Some(7), Some(5)));
        let first: Vec<_> = found[0]
            .related
            .iter()
            .map(|r| (r.role, r.message.as_str(), r.file.as_deref(), r.line))
            .collect();
        assert_eq!(
            first,
            vec![
                (
                    "label",
                    "help: using `is_empty` is clearer and more explicit",
                    Some("src/len.rs"),
                    Some(2)
                ),
                (
                    "note",
                    "`#[warn(clippy::len_zero)]` on by default",
                    None,
                    None
                ),
            ]
        );
        assert_eq!(found[1].related.len(), 1, "{found:?}");
        let note = &found[1].related[0];
        assert_eq!(note.role, "note");
        assert_eq!(note.message, "the item is defined here");
        assert_eq!(note.file.as_deref(), Some("src/other.rs"));
        assert_eq!((note.line, note.column), (Some(1), Some(1)));
    }

    #[test]
    fn action_output_keeps_rendered_spanless_link_errors() {
        let input = "error: linking with `cc` failed: exit status: 1\n  |\n  = note: ld: missing\n\nerror: aborting due to 1 previous error\n";
        let found = parse(input.as_bytes());
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].message, "linking with `cc` failed: exit status: 1");
        assert_eq!((found[0].file.as_deref(), found[0].line), (None, None));
        assert!(found[0].related.is_empty(), "{found:?}");
    }

    #[test]
    fn action_output_ignores_indented_and_unrelated_text() {
        let input = "  error: from a build script\nerror in build script\nnote: --> a.rs:1:1\n";
        let found = parse(input.as_bytes());
        assert!(found.is_empty(), "{found:?}");
    }

    #[test]
    fn action_output_prefers_json_over_rendered_lines() {
        let mut input = String::from("error[E0308]: rendered copy\n --> src/x.rs:9:9\n\n");
        input.push_str(CLIPPY_BROKEN);
        let found = parse(input.as_bytes());
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].message, "this file contains an unclosed delimiter");
        assert_eq!(found[0].file.as_deref(), Some("/s/broken.rs"));
    }
}
