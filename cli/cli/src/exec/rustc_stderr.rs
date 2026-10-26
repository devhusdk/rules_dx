use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ActionDiagnostic {
    pub(super) severity: &'static str,
    pub(super) code: Option<String>,
    pub(super) message: String,
    pub(super) file: Option<String>,
    pub(super) line: Option<u64>,
    pub(super) column: Option<u64>,
}

#[derive(Deserialize)]
struct Message {
    message: String,
    code: Option<Code>,
    level: String,
    #[serde(default)]
    spans: Vec<Span>,
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
        found.push(ActionDiagnostic {
            severity,
            code: diagnostic.code.map(|code| code.code),
            message: diagnostic.message,
            file: span.map(|span| span.file_name.clone()),
            line: span.map(|span| span.line_start).filter(|line| *line > 0),
            column: span
                .map(|span| span.column_start)
                .filter(|column| *column > 0),
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
    let location = line.trim_start().strip_prefix("--> ")?.trim();
    let mut parts = location.rsplitn(3, ':');
    let column = parts.next()?.parse().ok()?;
    let line = parts.next()?.parse().ok()?;
    let file = parts.next().filter(|file| !file.is_empty())?;
    Some((file.to_owned(), line, column))
}

fn parse_rendered(text: &str) -> Vec<ActionDiagnostic> {
    let mut found = Vec::new();
    let mut open = false;
    for line in text.lines() {
        if let Some((severity, code, message)) = rendered_header(line) {
            found.push(ActionDiagnostic {
                severity,
                code,
                message: message.to_owned(),
                file: None,
                line: None,
                column: None,
            });
            open = true;
            continue;
        }
        if line.trim().is_empty() {
            open = false;
            continue;
        }
        if !open {
            continue;
        }
        if let Some((file, line, column)) = rendered_location(line) {
            if let Some(last) = found.last_mut() {
                last.file = Some(file);
                last.line = Some(line).filter(|line| *line > 0);
                last.column = Some(column).filter(|column| *column > 0);
            }
            open = false;
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
    }

    #[test]
    fn action_output_keeps_rendered_spanless_link_errors() {
        let input = "error: linking with `cc` failed: exit status: 1\n  |\n  = note: ld: missing\n\nerror: aborting due to 1 previous error\n";
        let found = parse(input.as_bytes());
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].message, "linking with `cc` failed: exit status: 1");
        assert_eq!((found[0].file.as_deref(), found[0].line), (None, None));
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
