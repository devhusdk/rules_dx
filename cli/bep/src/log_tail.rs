/// The last `max_lines` lines of a test log. When those exceed `max_bytes`, the oldest whole
/// lines are dropped; a single oversize line is cut at a char boundary instead. Blank logs have
/// no tail.
#[must_use]
pub fn log_tail(bytes: &[u8], max_lines: usize, max_bytes: usize) -> Option<String> {
    let text = String::from_utf8_lossy(bytes);
    let mut lines: Vec<&str> = text.lines().collect();
    let start = lines.len().saturating_sub(max_lines);
    let mut tail = lines.split_off(start).join("\n");
    if tail.len() > max_bytes {
        let mut from = tail.len() - max_bytes;
        while !tail.is_char_boundary(from) {
            from += 1;
        }
        let Some((head, window)) = tail.split_at_checked(from) else {
            return (!tail.trim().is_empty()).then_some(tail);
        };
        let at_line_start = head.ends_with('\n');
        tail = match window.split_once('\n') {
            _ if at_line_start => window.to_owned(),
            Some((_, rest)) => rest.to_owned(),
            None => window.to_owned(),
        };
    }
    (!tail.trim().is_empty()).then_some(tail)
}

#[cfg(test)]
mod tests {
    use super::log_tail;

    #[test]
    fn keeps_the_requested_line_count() {
        let text: String = (1..=10).map(|n| format!("{n}\n")).collect();
        assert_eq!(
            log_tail(text.as_bytes(), 3, 1024),
            Some("8\n9\n10".to_owned())
        );
    }

    #[test]
    fn drops_whole_lines_to_fit_the_byte_cap() {
        assert_eq!(
            log_tail(b"aaaa\nbbbb\ncccc\n", 3, 8),
            Some("cccc".to_owned())
        );
    }
}
