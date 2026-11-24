use crate::Error;

#[must_use]
pub fn span(text: &str, begin: &str, end: &str) -> Option<(usize, usize)> {
    let start = text.find(begin)?;
    let stop = start + text.get(start..)?.find(end)? + end.len();
    let after = text.get(stop..)?;
    let stop = if after.starts_with("\r\n") {
        stop + 2
    } else if after.starts_with('\n') {
        stop + 1
    } else {
        stop
    };
    Some((start, stop))
}

#[must_use]
pub fn orphaned(text: &str, begin: &str, end: &str) -> bool {
    text.contains(begin) && span(text, begin, end).is_none()
}

#[must_use]
pub fn eol(text: &str) -> &'static str {
    if text.contains("\r\n") { "\r\n" } else { "\n" }
}

pub fn merge(text: &str, begin: &str, end: &str, block: &str) -> Result<String, Error> {
    if orphaned(text, begin, end) {
        return Err(Error::OrphanedMarker {
            begin: begin.to_owned(),
            end: end.to_owned(),
        });
    }
    let eol = eol(text);
    let mut block = block.replace("\r\n", "\n").replace('\n', eol);
    Ok(if let Some((start, stop)) = span(text, begin, end) {
        let rest = text.get(stop..).unwrap_or_default();
        if !rest.is_empty() && !block.is_empty() && !block.ends_with('\n') {
            block.push_str(eol);
        }
        format!("{}{}{}", text.get(..start).unwrap_or_default(), block, rest)
    } else {
        let mut out = text.to_owned();
        if !out.is_empty() && !out.ends_with('\n') {
            out.push_str(eol);
        }
        if !out.is_empty() && !out.ends_with("\n\n") && !out.ends_with("\n\r\n") {
            out.push_str(eol);
        }
        out.push_str(&block);
        out
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

    #[test]
    fn a_crlf_block_stays_single_crlf() {
        let old = "top\r\n<!-- b -->\r\nold\r\n<!-- e -->\r\n";
        let block = "<!-- b -->\r\nbody\r\n<!-- e -->\r\n";
        let out = merge(old, B, E, block).unwrap();
        assert_eq!(out, "top\r\n<!-- b -->\r\nbody\r\n<!-- e -->\r\n");
    }

    #[test]
    fn a_block_without_a_line_break_does_not_join_the_next_line() {
        let old = "top\n<!-- b -->\nold\n<!-- e -->\nbottom\n";
        let out = merge(old, B, E, "<!-- b -->\nbody\n<!-- e -->").unwrap();
        assert_eq!(out, "top\n<!-- b -->\nbody\n<!-- e -->\nbottom\n");
    }

    #[test]
    fn a_trailing_blank_line_is_not_doubled() {
        assert_eq!(
            merge("# Mine\n\n", B, E, BLOCK).unwrap(),
            format!("# Mine\n\n{BLOCK}")
        );
        let out = merge("# Mine\r\n\r\n", B, E, BLOCK).unwrap();
        assert!(out.starts_with("# Mine\r\n\r\n<!-- b -->"), "{out:?}");
    }
}
