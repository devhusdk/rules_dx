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
    parse_prettier_check_with_sources(stderr, code, files, |_| None)
}

/// Parses `prettier --check` stderr. `source` returns the original text of a checked file; it is
/// used only to place a `prettier-plugin-svelte` syntax error, whose code frame numbers the lines
/// of a copy with every `<script>` and `<style>` body collapsed onto one line. Without the source,
/// or when the frame does not match exactly one place in it, the error stays at 1:1.
pub fn parse_prettier_check_with_sources(
    stderr: &[u8],
    code: Option<i32>,
    files: &[&str],
    source: impl Fn(&str) -> Option<String>,
) -> Result<Vec<FileFinding>, ParseError> {
    check_output_size(TOOL, stderr)?;
    let text =
        std::str::from_utf8(stderr).map_err(|err| ParseError::shape(TOOL, err.to_string()))?;
    let mut findings = Vec::new();
    let mut frame: Option<SvelteFrame> = None;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed == "The system cannot find the path specified." {
            continue;
        }
        if let Some(error) = trimmed.strip_prefix("[error]") {
            let raw = error.strip_prefix(' ').unwrap_or(error);
            if svelte_frame(raw, &mut frame, &findings) {
                continue;
            }
            place_svelte_error(frame.take(), &mut findings, &source);
            syntax_error(error.trim_start(), files, &mut findings)?;
            continue;
        }
        place_svelte_error(frame.take(), &mut findings, &source);
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
    place_svelte_error(frame.take(), &mut findings, &source);
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

/// The Svelte compiler's code frame under the last syntax error: the shown lines in order, and
/// the index and display offset of the caret line once the `^` row has been seen.
#[derive(Debug, Default)]
struct SvelteFrame {
    lines: Vec<String>,
    caret: Option<(usize, usize)>,
    pending: Option<usize>,
}

const SNIPPED: &str = "\u{2702}prettier:content\u{2702}";

fn svelte_frame(raw: &str, frame: &mut Option<SvelteFrame>, findings: &[FileFinding]) -> bool {
    let Some(last) = findings.last() else {
        return false;
    };
    if last.finding.severity != ToolSeverity::Error {
        return false;
    }
    let text = raw.trim();
    if text.starts_with("https://svelte.dev/e/") {
        return true;
    }
    if text == "^" {
        let Some(open) = frame.as_mut() else {
            return false;
        };
        let Some(start) = open.pending.take() else {
            return false;
        };
        let caret = raw.len() - raw.trim_start().len();
        let Some(offset) = caret.checked_sub(start) else {
            return false;
        };
        if open.caret.is_some() || open.lines.is_empty() {
            return false;
        }
        open.caret = Some((open.lines.len() - 1, offset));
        return true;
    }
    let body = raw.trim_start();
    let digits = body.len() - body.trim_start_matches(|c: char| c.is_ascii_digit()).len();
    if digits == 0 {
        return false;
    }
    let rest = &body[digits..];
    let shown = if rest == ":" {
        ""
    } else if let Some(shown) = rest.strip_prefix(": ") {
        shown
    } else {
        return false;
    };
    let open = frame.get_or_insert_with(SvelteFrame::default);
    open.lines.push(shown.trim_end().to_owned());
    open.pending = Some(raw.len() - body.len() + digits + 2);
    true
}

/// Moves the last finding, a Svelte syntax error still at 1:1, to the original line and column
/// of its frame. The frame's lines between collapsed `<script>`/`<style>` rows are verbatim
/// source lines (leading tabs shown as two spaces), so they must match exactly one run of lines
/// in the original file; anything else leaves the error at 1:1.
fn place_svelte_error(
    frame: Option<SvelteFrame>,
    findings: &mut [FileFinding],
    source: &impl Fn(&str) -> Option<String>,
) {
    let Some(SvelteFrame {
        lines,
        caret: Some((index, offset)),
        ..
    }) = frame
    else {
        return;
    };
    let Some(last) = findings.last_mut() else {
        return;
    };
    if last.finding.severity != ToolSeverity::Error
        || last.finding.start.line != 1
        || last.finding.start.column != 1
    {
        return;
    }
    let first = lines[..index]
        .iter()
        .rposition(|line| line.contains(SNIPPED))
        .map_or(0, |at| at + 1);
    let end = lines[index..]
        .iter()
        .position(|line| line.contains(SNIPPED))
        .map_or(lines.len(), |at| index + at);
    let window = &lines[first..end];
    let Some(text) = source(&last.file) else {
        return;
    };
    let original: Vec<&str> = text.lines().collect();
    let shown: Vec<String> = original
        .iter()
        .map(|line| tabs_to_spaces(line).trim_end().to_owned())
        .collect();
    let mut matches = shown
        .windows(window.len())
        .enumerate()
        .filter(|(_, run)| *run == window)
        .map(|(at, _)| at);
    let (Some(at), None) = (matches.next(), matches.next()) else {
        return;
    };
    let row = at + index - first;
    let tabs = original[row].len() - original[row].trim_start_matches('\t').len();
    let column = if offset >= 2 * tabs {
        offset - tabs
    } else {
        offset / 2
    };
    *last = finding(
        TOOL,
        &last.file,
        String::new(),
        last.finding.message.clone(),
        ToolSeverity::Error,
        TextPosition {
            line: row as u64 + 1,
            column: column as u64 + 1,
        },
        ColumnEncoding::Scalars,
    );
}

fn tabs_to_spaces(line: &str) -> String {
    let body = line.trim_start_matches('\t');
    let tabs = line.len() - body.len();
    format!("{}{body}", "  ".repeat(tabs))
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

    fn svelte_sources(file: &str, text: &str) -> impl Fn(&str) -> Option<String> {
        let (file, text) = (file.to_owned(), text.to_owned());
        move |asked: &str| (asked == file).then(|| text.clone())
    }

    #[test]
    fn prettier_check_locates_a_svelte_syntax_error_beside_a_dirty_svelte_file() {
        let stderr = "[error] app/Broken.svelte: element_invalid_closing_tag: `</p>` attempted to close an element that was not open\n[error] https://svelte.dev/e/element_invalid_closing_tag\n[error] 1: <div>\n[error] 2:   <span>hi\n[error] 3: </p>\n[error]    ^\n[warn] app/Dirty.svelte\n";
        let source = svelte_sources("app/Broken.svelte", "<div>\n  <span>hi\n</p>\n");
        let findings = parse_prettier_check_with_sources(
            stderr.as_bytes(),
            Some(2),
            &["app/Broken.svelte", "app/Dirty.svelte"],
            source,
        )
        .expect("parsed");
        assert_eq!(findings.len(), 2);
        assert_eq!(findings[0].file, "app/Broken.svelte");
        assert_eq!(findings[0].finding.severity, ToolSeverity::Error);
        assert_eq!(findings[0].finding.start.line, 3);
        assert_eq!(findings[0].finding.start.column, 1);
        assert!(findings[0]
            .finding
            .message
            .contains("`</p>` attempted to close an element that was not open"));
        assert_eq!(findings[1].file, "app/Dirty.svelte");
        assert_eq!(findings[1].finding.message, "file is not formatted");
        let padded = "[error] app/Broken.svelte: element_unclosed: `<div>` was left open\n[error] https://svelte.dev/e/element_unclosed\n[error]  9: <p>\n[error] 10: let x =\n[error]            ^\n";
        let text = format!("{}<p>\nlet x =\n", "<b></b>\n".repeat(8));
        let findings = parse_prettier_check_with_sources(
            padded.as_bytes(),
            Some(2),
            &["app/Broken.svelte"],
            svelte_sources("app/Broken.svelte", &text),
        )
        .expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].finding.start.line, 10);
        assert_eq!(findings[0].finding.start.column, 8);
        let unlocated = "[error] app/Broken.svelte: element_unclosed: `<script>` was left open\n";
        let findings = parse_prettier_check(unlocated.as_bytes(), Some(2), &["app/Broken.svelte"])
            .expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].finding.severity, ToolSeverity::Error);
        assert_eq!(findings[0].finding.start.line, 1);
        assert_eq!(findings[0].finding.start.column, 1);
        assert!(parse_prettier_check(
            b"[error] https://svelte.dev/e/element_unclosed\n",
            Some(2),
            &["app/Broken.svelte"],
        )
        .is_err());
    }

    /// Real `prettier-plugin-svelte` 4.1.1 output: the frame shows blank source lines as a bare
    /// `N:` (trailing space trimmed), and numbers lines with the 3-line `<script>` collapsed.
    #[test]
    fn prettier_check_places_a_svelte_error_framed_by_blank_lines_after_a_script() {
        let stderr = "[error] app/A.svelte: element_invalid_closing_tag: `</p>` attempted to close an element that was not open\n[error] https://svelte.dev/e/element_invalid_closing_tag\n[error] 2: \n[error] 3: <div>\n[error] 4: </p>\n[error]    ^\n[error] 5: \n[error] 6: <b></b>\n[warn] app/Dirty.svelte\n";
        let text = "<script>\n  let x = 1;\n\n</script>\n\n<div>\n</p>\n\n<b></b>\n";
        let files = ["app/A.svelte", "app/Dirty.svelte"];
        let findings = parse_prettier_check_with_sources(
            stderr.as_bytes(),
            Some(2),
            &files,
            svelte_sources("app/A.svelte", text),
        )
        .expect("blank frame lines parse");
        assert_eq!(findings.len(), 2);
        assert_eq!(findings[0].file, "app/A.svelte");
        assert_eq!(findings[0].finding.severity, ToolSeverity::Error);
        assert_eq!(findings[0].finding.start.line, 7);
        assert_eq!(findings[0].finding.start.column, 1);
        assert_eq!(findings[1].file, "app/Dirty.svelte");
        let unsourced = parse_prettier_check(stderr.as_bytes(), Some(2), &files)
            .expect("parses without the source");
        assert_eq!(unsourced.len(), 2);
        assert_eq!(unsourced[0].finding.severity, ToolSeverity::Error);
        assert_eq!(unsourced[0].finding.start.line, 1);
        assert_eq!(unsourced[0].finding.start.column, 1);
    }

    #[test]
    fn prettier_check_maps_a_svelte_frame_past_a_multi_line_script_to_the_original_line() {
        let stderr = "[error] app/B.svelte: element_invalid_closing_tag: `</p>` attempted to close an element that was not open\n[error] https://svelte.dev/e/element_invalid_closing_tag\n[error] 2: <div>\n[error] 3: <span>x\n[error] 4: </p>\n[error]    ^\n";
        let text =
            "<script>\n  let a = 1;\n  let b = 2;\n  let c = 3;\n</script>\n<div>\n<span>x\n</p>\n";
        let findings = parse_prettier_check_with_sources(
            stderr.as_bytes(),
            Some(2),
            &["app/B.svelte"],
            svelte_sources("app/B.svelte", text),
        )
        .expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].finding.start.line, 8);
        assert_eq!(findings[0].finding.start.column, 1);

        let snipped = "[error] app/U.svelte: element_invalid_closing_tag: `</p>` attempted to close an element that was not open\n[error] https://svelte.dev/e/element_invalid_closing_tag\n[error] 1: <script \u{2702}prettier:content\u{2702}=\"CiAgbGV0IHggPSAxOwo=\">{}</script>\n[error] 2: <div>\n[error] 3: </p>\n[error]    ^\n[error] 4: <div>\n[error] 5: </p>\n";
        let text = "<script>\n  let x = 1;\n</script>\n<div>\n</p>\n<div>\n</p>\n";
        let findings = parse_prettier_check_with_sources(
            snipped.as_bytes(),
            Some(2),
            &["app/U.svelte"],
            svelte_sources("app/U.svelte", text),
        )
        .expect("parsed");
        assert_eq!(findings[0].finding.start.line, 5);
        assert_eq!(findings[0].finding.start.column, 1);

        let tabbed = "[error] app/T.svelte: element_invalid_closing_tag: `</p>` attempted to close an element that was not open\n[error] https://svelte.dev/e/element_invalid_closing_tag\n[error] 2: <div>\n[error] 3:   <span>\n[error] 4:     </p>\n[error]        ^\n[error] 5:   </span>\n[error] 6: </div>\n";
        let text =
            "<script>\n\tlet a = 1;\n</script>\n<div>\n\t<span>\n\t\t</p>\n\t</span>\n</div>\n";
        let findings = parse_prettier_check_with_sources(
            tabbed.as_bytes(),
            Some(2),
            &["app/T.svelte"],
            svelte_sources("app/T.svelte", text),
        )
        .expect("parsed");
        assert_eq!(findings[0].finding.start.line, 6);
        assert_eq!(findings[0].finding.start.column, 3);

        let ambiguous = "[error] app/V.svelte: element_invalid_closing_tag: `</p>` attempted to close an element that was not open\n[error] https://svelte.dev/e/element_invalid_closing_tag\n[error] 2: </p>\n[error]    ^\n";
        let text = "<script>\n  let x = 1;\n</script>\n</p>\n</p>\n";
        let findings = parse_prettier_check_with_sources(
            ambiguous.as_bytes(),
            Some(2),
            &["app/V.svelte"],
            svelte_sources("app/V.svelte", text),
        )
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
