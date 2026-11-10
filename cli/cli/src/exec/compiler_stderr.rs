use super::rustc_stderr::ActionDiagnostic;

/// Reads `path:line:col: severity: message` (gcc, clang, javac, kotlinc, swiftc),
/// Go's `path:line:col: message` and `path(line,col): error CODE: message` (tsc, csc).
pub(super) fn parse(stderr: &[u8]) -> Vec<ActionDiagnostic> {
    String::from_utf8_lossy(stderr)
        .lines()
        .filter_map(|line| colon_form(line).or_else(|| paren_form(line)))
        .collect()
}

fn severity_of(word: &str) -> Option<&'static str> {
    match word.to_ascii_lowercase().as_str() {
        "error" | "fatal error" => Some("error"),
        "warning" => Some("warning"),
        "note" => Some("note"),
        _ => None,
    }
}

fn diagnostic(
    severity: &'static str,
    file: &str,
    line: u64,
    column: Option<u64>,
    code: Option<String>,
    message: &str,
) -> ActionDiagnostic {
    ActionDiagnostic {
        severity,
        code,
        message: message.trim().to_owned(),
        file: Some(file.to_owned()),
        line: Some(line).filter(|line| *line > 0),
        column: column.filter(|column| *column > 0),
        related: Vec::new(),
    }
}

fn colon_form(line: &str) -> Option<ActionDiagnostic> {
    let skip = if dx_path::drive_rooted(line) { 2 } else { 0 };
    let mut from = skip;
    while let Some(at) = line[from..].find(':') {
        let colon = from + at;
        from = colon + 1;
        let file = &line[..colon];
        let mut rest = &line[colon + 1..];
        let Some((number, after)) = rest.split_once(':') else {
            continue;
        };
        let Ok(line_no) = number.parse::<u64>() else {
            continue;
        };
        rest = after;
        let mut column = None;
        if let Some((number, after)) = rest.split_once(':')
            && let Ok(value) = number.parse::<u64>()
        {
            column = Some(value);
            rest = after;
        }
        let rest = rest.strip_prefix(' ')?;
        if let Some((word, message)) = rest.split_once(": ")
            && let Some(severity) = severity_of(word)
        {
            if severity == "note" {
                return None;
            }
            let (message, code) = split_rule(message);
            return Some(diagnostic(severity, file, line_no, column, code, message));
        }
        if column.is_some() && !rest.trim().is_empty() {
            return Some(diagnostic("error", file, line_no, column, None, rest));
        }
        return None;
    }
    None
}

fn split_rule(message: &str) -> (&str, Option<String>) {
    let message = message.trim();
    match message.rfind(" [") {
        Some(start) if message.ends_with(']') => (
            message[..start].trim_end(),
            Some(message[start + 2..message.len() - 1].to_owned()),
        ),
        _ => (message, None),
    }
}

fn paren_form(line: &str) -> Option<ActionDiagnostic> {
    let open = line.find('(')?;
    let (position, rest) = line[open + 1..].split_once("): ")?;
    let (line_text, column_text) = position.split_once(',')?;
    let line_no = line_text.trim().parse::<u64>().ok()?;
    let column = column_text.trim().parse::<u64>().ok();
    let (word, message) = rest.split_once(' ')?;
    let severity = severity_of(word)?;
    if severity == "note" {
        return None;
    }
    let (code, message) = message.split_once(": ")?;
    Some(diagnostic(
        severity,
        &line[..open],
        line_no,
        column,
        Some(code.to_owned()),
        message,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one(text: &str) -> ActionDiagnostic {
        let found = parse(text.as_bytes());
        assert_eq!(found.len(), 1, "{found:?}");
        found.into_iter().next().expect("one")
    }

    #[test]
    fn gcc_error_has_path_line_column_and_message() {
        let found = one("src/a.c:3:5: error: 'x' undeclared (first use in this function)\n");
        assert_eq!(found.severity, "error");
        assert_eq!(found.file.as_deref(), Some("src/a.c"));
        assert_eq!((found.line, found.column), (Some(3), Some(5)));
        assert_eq!(found.message, "'x' undeclared (first use in this function)");
    }

    #[test]
    fn clang_warning_flag_becomes_the_code() {
        let found = one("a.cc:7:9: warning: unused variable 'y' [-Wunused-variable]");
        assert_eq!(found.severity, "warning");
        assert_eq!(found.code.as_deref(), Some("-Wunused-variable"));
        assert_eq!(found.message, "unused variable 'y'");
    }

    #[test]
    fn javac_has_a_line_but_no_column() {
        let text = "app/Main.java:4: error: cannot find symbol\n    foo();\n    ^\n  symbol: method foo()\n";
        let found = one(text);
        assert_eq!(found.file.as_deref(), Some("app/Main.java"));
        assert_eq!((found.line, found.column), (Some(4), None));
        assert_eq!(found.message, "cannot find symbol");
    }

    #[test]
    fn go_errors_have_no_severity_word() {
        let found = one("# example.com/app\n./main.go:9:2: undefined: foo\n");
        assert_eq!(found.severity, "error");
        assert_eq!(found.file.as_deref(), Some("./main.go"));
        assert_eq!((found.line, found.column), (Some(9), Some(2)));
        assert_eq!(found.message, "undefined: foo");
    }

    #[test]
    fn tsc_and_csc_use_parentheses_and_a_code() {
        let found = one("src/a.ts(3,5): error TS2322: Type 'string' is not assignable.");
        assert_eq!(found.code.as_deref(), Some("TS2322"));
        assert_eq!(found.file.as_deref(), Some("src/a.ts"));
        assert_eq!((found.line, found.column), (Some(3), Some(5)));
        let found = one("Program.cs(1,2): error CS0103: The name 'x' does not exist");
        assert_eq!(found.code.as_deref(), Some("CS0103"));
    }

    #[test]
    fn notes_and_plain_text_are_skipped() {
        let text = "In file included from a.c:1:\na.c:2:3: note: declared here\nmake failed: 2\n";
        assert!(parse(text.as_bytes()).is_empty());
    }

    #[test]
    fn a_windows_drive_path_is_kept_whole() {
        let found = one("C:\\src\\a.c(3,5): error C2065: 'x': undeclared identifier");
        assert_eq!(found.file.as_deref(), Some("C:\\src\\a.c"));
        let found = one("C:\\src\\a.c:3:5: error: bad");
        assert_eq!(found.file.as_deref(), Some("C:\\src\\a.c"));
    }
}
