//! A marked block of text that is added or replaced in place.

use crate::Error;

/// The byte range of the block, including one trailing line break.
pub fn span(text: &str, begin: &str, end: &str) -> Option<(usize, usize)> {
    let start = text.find(begin)?;
    let stop = start + text[start..].find(end)? + end.len();
    let stop = if text[stop..].starts_with("\r\n") {
        stop + 2
    } else if text[stop..].starts_with('\n') {
        stop + 1
    } else {
        stop
    };
    Some((start, stop))
}

/// Whether the text has the begin marker but no end marker after it.
pub fn orphaned(text: &str, begin: &str, end: &str) -> bool {
    text.contains(begin) && span(text, begin, end).is_none()
}

/// The line break the text uses.
pub fn eol(text: &str) -> &'static str {
    if text.contains("\r\n") { "\r\n" } else { "\n" }
}

/// The text with `block` (LF, markers included) replaced or appended and every other byte kept.
pub fn merge(text: &str, begin: &str, end: &str, block: &str) -> Result<String, Error> {
    if orphaned(text, begin, end) {
        return Err(Error::OrphanedMarker {
            begin: begin.to_owned(),
            end: end.to_owned(),
        });
    }
    let eol = eol(text);
    let block = block.replace('\n', eol);
    Ok(match span(text, begin, end) {
        Some((start, stop)) => format!("{}{}{}", &text[..start], block, &text[stop..]),
        None => {
            let mut out = text.to_owned();
            if !out.is_empty() && !out.ends_with('\n') {
                out.push_str(eol);
            }
            if !out.is_empty() {
                out.push_str(eol);
            }
            out.push_str(&block);
            out
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const B: &str = "<!-- b -->";
    const E: &str = "<!-- e -->";
    const BLOCK: &str = "<!-- b -->\nbody\n<!-- e -->\n";

    #[test]
    fn appends_after_a_blank_line() {
        let out = merge("# Mine", B, E, BLOCK).unwrap();
        assert_eq!(out, format!("# Mine\n\n{BLOCK}"));
    }

    #[test]
    fn empty_text_is_just_the_block() {
        assert_eq!(merge("", B, E, BLOCK).unwrap(), BLOCK);
    }

    #[test]
    fn replaces_only_the_block() {
        let old = "top\r\n<!-- b -->\r\nold\r\n<!-- e -->\r\nbottom\r\n";
        let out = merge(old, B, E, BLOCK).unwrap();
        assert_eq!(out, "top\r\n<!-- b -->\r\nbody\r\n<!-- e -->\r\nbottom\r\n");
    }

    #[test]
    fn merging_twice_changes_nothing() {
        let once = merge("# Mine\nkeep", B, E, BLOCK).unwrap();
        assert_eq!(merge(&once, B, E, BLOCK).unwrap(), once);
    }

    #[test]
    fn a_begin_marker_alone_is_an_error() {
        let error = merge("x\n<!-- b -->\n", B, E, BLOCK).unwrap_err();
        assert!(matches!(error, Error::OrphanedMarker { .. }));
    }

    #[test]
    fn bytes_outside_the_block_survive() {
        let old = "\u{feff}  odd \t\n<!-- b -->\nold\n<!-- e -->\n\n\ttail";
        let out = merge(old, B, E, BLOCK).unwrap();
        assert!(out.starts_with("\u{feff}  odd \t\n"));
        assert!(out.ends_with("\n\n\ttail"));
    }
}
