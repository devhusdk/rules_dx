#[derive(Clone, Debug, Eq, PartialEq)]
enum MavenToken {
    Numeric(String),
    Qualifier(String),
}

fn is_maven_null_token(token: &MavenToken) -> bool {
    match token {
        MavenToken::Numeric(value) => value == "0",
        MavenToken::Qualifier(value) => value.is_empty(),
    }
}

fn compare_maven_numeric(left: &str, right: &str) -> std::cmp::Ordering {
    if left.len() != right.len() {
        return left.len().cmp(&right.len());
    }
    left.cmp(right)
}

fn compare_maven_qualifier(left: &str, right: &str) -> std::cmp::Ordering {
    const KNOWN: [&str; 7] = ["alpha", "beta", "milestone", "rc", "snapshot", "", "sp"];
    let mut left_index: Option<usize> = None;
    let mut right_index: Option<usize> = None;
    for (index, known) in KNOWN.iter().enumerate() {
        if *known == left {
            left_index = Some(index);
        }
        if *known == right {
            right_index = Some(index);
        }
    }
    match (left_index, right_index) {
        (Some(left_pos), Some(right_pos)) => left_pos.cmp(&right_pos),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => left.cmp(right),
    }
}

fn maven_token_vs_null(token: &MavenToken) -> std::cmp::Ordering {
    match token {
        MavenToken::Numeric(value) => {
            if value == "0" {
                std::cmp::Ordering::Equal
            } else {
                std::cmp::Ordering::Greater
            }
        }
        MavenToken::Qualifier(value) => compare_maven_qualifier(value, ""),
    }
}

fn tokenize_maven_raw(version: &str) -> Vec<(char, String, bool)> {
    let mut out: Vec<(char, String, bool)> = Vec::new();
    let mut current = String::new();
    let mut current_is_digit: Option<bool> = None;
    let mut next_sep: char = ' ';
    for c in version.chars() {
        if c == '.' || c == '-' || c == '_' {
            match current_is_digit {
                None => {
                    out.push((next_sep, "0".to_owned(), true));
                }
                Some(is_digit) => {
                    out.push((next_sep, std::mem::take(&mut current), is_digit));
                }
            }
            current_is_digit = None;
            next_sep = if c == '.' { '.' } else { '-' };
        } else if c.is_ascii_digit() {
            match current_is_digit {
                Some(false) => {
                    out.push((next_sep, std::mem::take(&mut current), false));
                    next_sep = '-';
                    current.push(c);
                    current_is_digit = Some(true);
                }
                _ => {
                    current.push(c);
                    current_is_digit = Some(true);
                }
            }
        } else if current_is_digit == Some(true) {
            out.push((next_sep, std::mem::take(&mut current), true));
            next_sep = '-';
            current.push(c);
            current_is_digit = Some(false);
        } else {
            current.push(c);
            current_is_digit = Some(false);
        }
    }
    match current_is_digit {
        None => {
            if !out.is_empty() {
                out.push((next_sep, "0".to_owned(), true));
            }
        }
        Some(is_digit) => {
            out.push((next_sep, current, is_digit));
        }
    }
    out
}

fn parse_maven_version(version: &str) -> Vec<(char, MavenToken)> {
    let trimmed = version.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }
    let raw = tokenize_maven_raw(trimmed);
    if raw.is_empty() {
        return Vec::new();
    }
    let mut mapped: Vec<(char, MavenToken)> = Vec::new();
    for (index, (sep, text, is_digit)) in raw.iter().enumerate() {
        if *is_digit {
            let stripped = text.trim_start_matches('0');
            let normalized = if stripped.is_empty() {
                "0".to_owned()
            } else {
                stripped.to_owned()
            };
            mapped.push((*sep, MavenToken::Numeric(normalized)));
        } else {
            let lower = text.to_ascii_lowercase();
            let aliased = if lower == "ga" || lower == "final" || lower == "release" {
                String::new()
            } else if lower == "cr" {
                "rc".to_owned()
            } else if (lower == "a" || lower == "b" || lower == "m")
                && index + 1 < raw.len()
                && raw[index + 1].2
            {
                if lower == "a" {
                    "alpha".to_owned()
                } else if lower == "b" {
                    "beta".to_owned()
                } else {
                    "milestone".to_owned()
                }
            } else {
                lower
            };
            mapped.push((*sep, MavenToken::Qualifier(aliased)));
        }
    }
    let mut segments: Vec<Vec<(char, MavenToken)>> = Vec::new();
    let mut current_seg: Vec<(char, MavenToken)> = Vec::new();
    for (sep, token) in mapped {
        if sep == '-' && !current_seg.is_empty() {
            segments.push(std::mem::take(&mut current_seg));
            current_seg.push(('-', token));
        } else {
            current_seg.push((sep, token));
        }
    }
    if !current_seg.is_empty() {
        segments.push(current_seg);
    }
    for segment in segments.iter_mut() {
        while let Some((_, token)) = segment.last() {
            if is_maven_null_token(token) {
                segment.pop();
            } else {
                break;
            }
        }
    }
    segments.retain(|segment| !segment.is_empty());
    let mut out: Vec<(char, MavenToken)> = Vec::new();
    for (seg_index, segment) in segments.into_iter().enumerate() {
        for (tok_index, (_, token)) in segment.into_iter().enumerate() {
            if seg_index == 0 && tok_index == 0 {
                out.push((' ', token));
            } else if tok_index == 0 {
                out.push(('-', token));
            } else {
                out.push(('.', token));
            }
        }
    }
    out
}

pub fn maven_compare(left: &str, right: &str) -> std::cmp::Ordering {
    let left_tokens = parse_maven_version(left);
    let right_tokens = parse_maven_version(right);
    let common = left_tokens.len().min(right_tokens.len());
    for index in 0..common {
        let (sep_left, token_left) = &left_tokens[index];
        let (sep_right, token_right) = &right_tokens[index];
        match (token_left, token_right) {
            (MavenToken::Numeric(left_num), MavenToken::Numeric(right_num)) => {
                if sep_left == sep_right {
                    match compare_maven_numeric(left_num, right_num) {
                        std::cmp::Ordering::Equal => continue,
                        other => return other,
                    }
                } else {
                    let left_rank = if *sep_left == '-' { 0 } else { 1 };
                    let right_rank = if *sep_right == '-' { 0 } else { 1 };
                    if left_rank != right_rank {
                        if left_rank < right_rank {
                            return std::cmp::Ordering::Less;
                        }
                        return std::cmp::Ordering::Greater;
                    }
                    match compare_maven_numeric(left_num, right_num) {
                        std::cmp::Ordering::Equal => continue,
                        other => return other,
                    }
                }
            }
            (MavenToken::Qualifier(left_q), MavenToken::Qualifier(right_q)) => {
                match compare_maven_qualifier(left_q, right_q) {
                    std::cmp::Ordering::Equal => continue,
                    other => return other,
                }
            }
            (MavenToken::Qualifier(_), MavenToken::Numeric(_)) => {
                return std::cmp::Ordering::Less;
            }
            (MavenToken::Numeric(_), MavenToken::Qualifier(_)) => {
                return std::cmp::Ordering::Greater;
            }
        }
    }
    if left_tokens.len() == right_tokens.len() {
        return std::cmp::Ordering::Equal;
    }
    if left_tokens.len() > right_tokens.len() {
        for (_, token) in left_tokens.iter().skip(common) {
            match maven_token_vs_null(token) {
                std::cmp::Ordering::Equal => continue,
                other => return other,
            }
        }
        return std::cmp::Ordering::Equal;
    }
    for (_, token) in right_tokens.iter().skip(common) {
        match maven_token_vs_null(token) {
            std::cmp::Ordering::Equal => continue,
            std::cmp::Ordering::Less => return std::cmp::Ordering::Greater,
            std::cmp::Ordering::Greater => return std::cmp::Ordering::Less,
        }
    }
    std::cmp::Ordering::Equal
}

pub fn maven_version_eq(left: &str, right: &str) -> bool {
    let left_trimmed = left.trim();
    let right_trimmed = right.trim();
    if left_trimmed.is_empty() || right_trimmed.is_empty() {
        return false;
    }
    if left_trimmed.len() > 256 || right_trimmed.len() > 256 {
        return false;
    }
    maven_compare(left_trimmed, right_trimmed) == std::cmp::Ordering::Equal
}

pub fn maven_in_scope(scope: &str, version: &str) -> bool {
    let scope_trimmed = scope.trim();
    let version_trimmed = version.trim();
    if scope_trimmed.is_empty() || version_trimmed.is_empty() {
        return false;
    }
    if scope_trimmed.len() > 4096 || version_trimmed.len() > 256 {
        return false;
    }
    let has_brackets = scope_trimmed.contains('[')
        || scope_trimmed.contains('(')
        || scope_trimmed.contains(']')
        || scope_trimmed.contains(')');
    if !has_brackets {
        return maven_version_eq(scope_trimmed, version_trimmed);
    }
    let chars: Vec<char> = scope_trimmed.chars().collect();
    let mut index: usize = 0;
    let mut matched = false;
    let mut found_interval = false;
    while index < chars.len() {
        let current = chars[index];
        if current == '[' || current == '(' {
            let start = current;
            let mut close_index = index + 1;
            while close_index < chars.len()
                && chars[close_index] != ']'
                && chars[close_index] != ')'
            {
                close_index += 1;
            }
            if close_index >= chars.len() {
                return false;
            }
            let end = chars[close_index];
            let content: String = chars[index + 1..close_index].iter().collect();
            found_interval = true;
            let parts: Vec<&str> = content.split(',').collect();
            let (lower, upper, lower_inclusive, upper_inclusive, valid) = if parts.len() == 1 {
                let bound = parts[0].trim();
                if bound.is_empty() {
                    (String::new(), String::new(), false, false, false)
                } else {
                    (
                        bound.to_owned(),
                        bound.to_owned(),
                        start == '[',
                        end == ']',
                        true,
                    )
                }
            } else if parts.len() == 2 {
                let low = parts[0].trim().to_owned();
                let high = parts[1].trim().to_owned();
                if low.is_empty() && high.is_empty() {
                    (String::new(), String::new(), false, false, false)
                } else {
                    (low, high, start == '[', end == ']', true)
                }
            } else {
                (String::new(), String::new(), false, false, false)
            };
            if valid
                && lower.len() <= 256
                && upper.len() <= 256
                && interval_matches(
                    &lower,
                    &upper,
                    lower_inclusive,
                    upper_inclusive,
                    version_trimmed,
                )
            {
                matched = true;
            }
            index = close_index + 1;
        } else if current == ',' || current.is_whitespace() {
            index += 1;
        } else {
            return false;
        }
    }
    if !found_interval {
        return false;
    }
    matched
}

fn interval_matches(
    lower: &str,
    upper: &str,
    lower_inclusive: bool,
    upper_inclusive: bool,
    version: &str,
) -> bool {
    if !lower.is_empty() {
        match maven_compare(version, lower) {
            std::cmp::Ordering::Less => return false,
            std::cmp::Ordering::Equal => {
                if !lower_inclusive {
                    return false;
                }
            }
            std::cmp::Ordering::Greater => {}
        }
    }
    if !upper.is_empty() {
        match maven_compare(version, upper) {
            std::cmp::Ordering::Greater => return false,
            std::cmp::Ordering::Equal => {
                if !upper_inclusive {
                    return false;
                }
            }
            std::cmp::Ordering::Less => {}
        }
    }
    true
}
