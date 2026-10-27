use super::{
    check_output_size, code_name, finding, known_native, row_col, unformatted_at, FileFinding,
    ParseError,
};
use crate::ToolSeverity;
use crate::{ColumnEncoding, TextPosition};

const TOOL: &str = "prettier";

pub fn parse_prettier_check(
    stderr: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    check_output_size(TOOL, stderr)?;
    let text =
        std::str::from_utf8(stderr).map_err(|err| ParseError::shape(TOOL, err.to_string()))?;
    let mut findings = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed == "The system cannot find the path specified." {
            continue;
        }
        if let Some(error) = trimmed.strip_prefix("[error]") {
            syntax_error(error.trim_start(), files, &mut findings)?;
            continue;
        }
        let Some(path) = trimmed.strip_prefix("[warn] ") else {
            return Err(ParseError::shape(
                TOOL,
                format!("unexpected stderr line: {trimmed:?}"),
            ));
        };
        if path.starts_with("Code style issues") {
            continue;
        }
        let normalized = path.strip_prefix("./").unwrap_or(path);
        let checked = known_native(TOOL, files, normalized)?;
        findings.push(unformatted_at(TOOL, checked, 1, 1, ColumnEncoding::Scalars));
    }
    if findings.is_empty() && code != Some(0) {
        return Err(ParseError::shape(
            TOOL,
            format!("exit {} with no warn lines", code_name(code)),
        ));
    }
    Ok(findings)
}

fn syntax_error(
    text: &str,
    files: &[&str],
    findings: &mut Vec<FileFinding>,
) -> Result<(), ParseError> {
    let named = files.iter().find_map(|file| {
        let native = if cfg!(windows) {
            file.replace('/', "\\")
        } else {
            (*file).to_owned()
        };
        let message = [*file, native.as_str()].into_iter().find_map(|spelling| {
            text.strip_prefix(spelling)
                .or_else(|| {
                    text.strip_prefix("./")
                        .and_then(|t| t.strip_prefix(spelling))
                })?
                .strip_prefix(": ")
        })?;
        Some((*file, message))
    });
    let Some((file, message)) = named else {
        if is_run_error(text) {
            return Err(ParseError::shape(TOOL, format!("run error: {text:?}")));
        }
        if let Some((line, column)) = parenthesised_position(text) {
            if let Some(last) = findings.last_mut() {
                if last.finding.severity == ToolSeverity::Error
                    && last.finding.message.ends_with(':')
                {
                    let relocated = finding(
                        TOOL,
                        &last.file,
                        String::new(),
                        format!("{} {}", last.finding.message, text),
                        ToolSeverity::Error,
                        TextPosition { line, column },
                        ColumnEncoding::Scalars,
                    );
                    *last = relocated;
                    return Ok(());
                }
            }
        }
        return match findings.last() {
            Some(last)
                if last.finding.severity == ToolSeverity::Error
                    && (is_frame(text) || last.finding.message.starts_with("Error: ")) =>
            {
                Ok(())
            }
            _ => Err(ParseError::shape(
                TOOL,
                format!("error without a file: {text:?}"),
            )),
        };
    };
    let (line, column) = error_position(message).unwrap_or((1, 1));
    findings.push(finding(
        TOOL,
        file,
        String::new(),
        message.trim().to_owned(),
        ToolSeverity::Error,
        TextPosition { line, column },
        ColumnEncoding::Scalars,
    ));
    Ok(())
}

fn is_run_error(text: &str) -> bool {
    [
        "No parser could be inferred",
        "No files matching the pattern",
        "Invalid ",
        "Cannot find",
    ]
    .iter()
    .any(|prefix| text.starts_with(prefix))
}

fn is_frame(text: &str) -> bool {
    if text.is_empty() {
        return true;
    }
    if text.starts_with("at ") {
        return true;
    }
    let rest = text.strip_prefix('>').unwrap_or(text).trim_start();
    let rest = rest
        .trim_start_matches(|c: char| c.is_ascii_digit())
        .trim_start();
    rest.starts_with('|')
}

fn parenthesised_position(text: &str) -> Option<(u64, u64)> {
    let (position, _) = text.strip_prefix('(')?.split_once("): ")?;
    row_col(position, ':')
}

fn line_column_words(head: &str) -> Option<(u64, u64)> {
    let (_, tail) = head.rsplit_once(" at line ")?;
    row_col(
        &tail.trim_end_matches('.').replacen(" column ", ":", 1),
        ':',
    )
}

fn error_position(message: &str) -> Option<(u64, u64)> {
    let head = message.lines().next()?.trim_end();
    if let Some(position) = line_column_words(head) {
        return Some(position);
    }
    let tail = head
        .strip_suffix(')')
        .and_then(|rest| rest.rsplit_once('('))
        .map(|(_, position)| position)
        .or_else(|| head.rsplit_once(" at ").map(|(_, position)| position))?;
    row_col(tail, ':')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prettier_check_reports_warn_lines() {
        let stderr = "[warn] src/a.js\n[warn] Code style issues found in the above file. Run Prettier with --write to fix.\n";
        let findings =
            parse_prettier_check(stderr.as_bytes(), Some(1), &["src/a.js"]).expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].file, "src/a.js");
        assert_eq!(findings[0].finding.message, "file is not formatted");
        let clean = parse_prettier_check(b"", Some(0), &["src/a.js"]).expect("parsed");
        assert!(clean.is_empty());
        assert!(parse_prettier_check(b"", Some(1), &["src/a.js"]).is_err());
        assert!(parse_prettier_check(b"unexpected\n", Some(1), &["src/a.js"]).is_err());
        assert!(parse_prettier_check(stderr.as_bytes(), Some(1), &["src/other.js"]).is_err());
        assert!(parse_prettier_check(&[0xff], Some(1), &["src/a.js"]).is_err());
        let padded = "[warn] src/a.js\n\n";
        let findings =
            parse_prettier_check(padded.as_bytes(), Some(1), &["src/a.js"]).expect("parsed");
        assert_eq!(findings.len(), 1);
        let windows_noise =
            "The system cannot find the path specified.\n[warn] src/a.js\n[warn] Code style issues found in the above file. Run Prettier with --write to fix.\n";
        let findings =
            parse_prettier_check(windows_noise.as_bytes(), Some(1), &["src/a.js"]).expect("parsed");
        assert_eq!(findings.len(), 1);
        let clean_noise = parse_prettier_check(
            b"The system cannot find the path specified.\n",
            Some(0),
            &["src/a.js"],
        )
        .expect("parsed");
        assert!(clean_noise.is_empty());
    }

    #[test]
    fn prettier_check_locates_a_syntax_error_beside_unformatted_files() {
        let stderr = "[error] a/bad.json5: SyntaxError: JSON5: invalid end of input at 2:1\n[error] > 1 | { \"a\": [1,\n[error]     |           ^\n[warn] a/dirty.html\n[warn] Code style issues found in the above file. Run Prettier with --write to fix.\n";
        let findings =
            parse_prettier_check(stderr.as_bytes(), Some(2), &["a/bad.json5", "a/dirty.html"])
                .expect("parsed");
        assert_eq!(findings.len(), 2);
        assert_eq!(findings[0].file, "a/bad.json5");
        assert_eq!(findings[0].finding.start.line, 2);
        assert_eq!(findings[0].finding.start.column, 1);
        assert_eq!(findings[0].finding.severity, ToolSeverity::Error);
        assert!(findings[0].finding.message.contains("invalid end of input"));
        assert_eq!(findings[1].file, "a/dirty.html");
    }

    #[test]
    fn prettier_check_reads_a_parenthesised_position() {
        let stderr = "[error] x.graphql: SyntaxError: Expected Name, found <EOF>. (1:17)\n";
        let findings =
            parse_prettier_check(stderr.as_bytes(), Some(2), &["x.graphql"]).expect("parsed");
        assert_eq!(findings[0].finding.start.line, 1);
        assert_eq!(findings[0].finding.start.column, 17);
        assert!(parse_prettier_check(b"[error] other: boom\n", Some(2), &["x.graphql"]).is_err());
    }

    #[test]
    fn prettier_check_locates_a_vue_template_error_beside_a_dirty_vue_file() {
        let stderr = "[error] app/Broken.vue: SyntaxError: Unexpected closing tag \"template\". It may happen when the tag has already been closed by another tag. For more info see https://www.w3.org/TR/html5/syntax.html#closing-elements-that-have-implied-end-tags (4:1)\n[error]   2 |   <div>\n[error]   3 |     <span>hi\n[error] > 4 | </template>\n[error]     | ^^^^^^^^^^^\n[error]   5 |\n[warn] app/Dirty.vue\n[warn] Code style issues found in the above file. Run Prettier with --write to fix.\n";
        let findings = parse_prettier_check(
            stderr.as_bytes(),
            Some(2),
            &["app/Broken.vue", "app/Dirty.vue"],
        )
        .expect("parsed");
        assert_eq!(findings.len(), 2);
        assert_eq!(findings[0].file, "app/Broken.vue");
        assert_eq!(findings[0].finding.severity, ToolSeverity::Error);
        assert_eq!(findings[0].finding.start.line, 4);
        assert_eq!(findings[0].finding.start.column, 1);
        assert!(findings[0]
            .finding
            .message
            .contains("Unexpected closing tag \"template\""));
        assert_eq!(findings[1].file, "app/Dirty.vue");
        assert_eq!(findings[1].finding.message, "file is not formatted");
    }

    #[test]
    fn prettier_check_locates_an_astro_syntax_error_beside_a_dirty_astro_file() {
        let stderr = "[error] app/Broken.astro: SyntaxError: Closing tag '</p>' has no matching opening tag. (3:3)\n[error]   1 | <div>\n[error]   2 |   <span>hi\n[error] > 3 | </p>\n[error]     |   ^\n[error]   4 |\n[warn] app/Dirty.astro\n";
        let findings = parse_prettier_check(
            stderr.as_bytes(),
            Some(2),
            &["app/Broken.astro", "app/Dirty.astro"],
        )
        .expect("parsed");
        assert_eq!(findings.len(), 2);
        assert_eq!(findings[0].file, "app/Broken.astro");
        assert_eq!(findings[0].finding.severity, ToolSeverity::Error);
        assert_eq!(findings[0].finding.start.line, 3);
        assert_eq!(findings[0].finding.start.column, 3);
        assert!(findings[0]
            .finding
            .message
            .contains("Closing tag '</p>' has no matching opening tag."));
        assert_eq!(findings[1].file, "app/Dirty.astro");
        assert_eq!(findings[1].finding.message, "file is not formatted");
        let unlocated = "[error] app/Broken.astro: SyntaxError: Unexpected end of frontmatter\n";
        let findings = parse_prettier_check(unlocated.as_bytes(), Some(2), &["app/Broken.astro"])
            .expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].finding.severity, ToolSeverity::Error);
        assert_eq!(findings[0].finding.start.line, 1);
        assert_eq!(findings[0].finding.start.column, 1);
    }

    #[test]
    fn prettier_check_fails_on_an_unnamed_error_after_a_syntax_error() {
        let stderr = "[error] a/bad.json5: SyntaxError: JSON5: invalid end of input at 2:1\n[error] No parser could be inferred for file \"/w/a/x.foo\".\n";
        assert!(
            parse_prettier_check(stderr.as_bytes(), Some(2), &["a/bad.json5", "a/x.foo"]).is_err()
        );
    }

    #[test]
    fn prettier_check_fails_on_an_uninferred_file_after_a_plugin_error() {
        let stderr = "[error] q.sql: Error: Parse error: Unexpected \"'abc from \" at line 1 column 8.\n[error] This likely happens because you're using the default \"sql\" dialect.\n[error] If possible, please select a more specific dialect (like sqlite, postgresql, etc).\n[error]     at TokenizerEngine.createParseError (file:///r/sql-formatter/dist/esm/lexer/TokenizerEngine.js:40:16)\n[error] No parser could be inferred for file \"/w/x.foo\".\n";
        assert!(parse_prettier_check(stderr.as_bytes(), Some(2), &["q.sql", "x.foo"]).is_err());
    }

    #[test]
    fn prettier_check_fails_when_a_plugin_does_not_load() {
        let missing = "[error] Cannot find module '/nonexistent/prettier-plugin-sql.js' imported from /r/prettier/index.mjs\n";
        assert!(parse_prettier_check(missing.as_bytes(), Some(2), &["q.sql"]).is_err());
        assert!(parse_prettier_check(missing.as_bytes(), Some(1), &["q.sql"]).is_err());
        let dropped = "[warn] Ignored unknown option { formatter: \"sql-formatter\" }.\n[warn] Ignored unknown option { language: \"sql\" }.\n[warn] Ignored unknown option { keywordCase: \"preserve\" }.\n[error] No parser could be inferred for file \"/w/q.sql\".\n";
        assert!(parse_prettier_check(dropped.as_bytes(), Some(2), &["q.sql"]).is_err());
    }

    #[test]
    fn prettier_check_locates_plugin_errors() {
        let sql = "[error] q.sql: Error: Parse error: Unexpected \"'abc\" at line 3 column 8.\n[error]     at TokenizerEngine.createParseError (file:///x.js:1:1)\n";
        let findings = parse_prettier_check(sql.as_bytes(), Some(2), &["q.sql"]).expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].finding.start.line, 3);
        assert_eq!(findings[0].finding.start.column, 8);
        let gherkin = "[error] a.feature: Error: Parser errors:\n[error] (1:1): expected: #EOF, got 'Scenario: x'\n[error]     at Parser.parse (file:///x.js:1:1)\n";
        let findings =
            parse_prettier_check(gherkin.as_bytes(), Some(2), &["a.feature"]).expect("parsed");
        assert_eq!(findings.len(), 1);
        assert!(findings[0].finding.message.contains("got 'Scenario: x'"));
        let nearley = "[error] q.sql: Error: Parse error at token: ) at line 1 column 8\n[error] Unexpected CLOSE_PAREN token: {}. Instead, I was expecting\n[error] \n[error]     all_columns_asterisk \u{2192}  \u{25cf} %ASTERISK\n";
        let findings =
            parse_prettier_check(nearley.as_bytes(), Some(2), &["q.sql"]).expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].finding.start.column, 8);
        let gherkin = "[error] a.feature: Error: Parser errors:\n[error] (4:3): expected: #EOF, got 'x'\n[error] (6:1): expected: #EOF, got 'y'\n";
        let findings =
            parse_prettier_check(gherkin.as_bytes(), Some(2), &["a.feature"]).expect("parsed");
        assert_eq!(findings[0].finding.start.line, 4);
        assert_eq!(findings[0].finding.start.column, 3);
    }
}
