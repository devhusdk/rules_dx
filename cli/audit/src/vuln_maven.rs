use mvn_version::ComparableVersion;
use std::cmp::Ordering;

const MAX_VERSION_LEN: usize = 256;
const MAX_SCOPE_LEN: usize = 4096;

struct MavenInterval {
    lower: Option<String>,
    upper: Option<String>,
    lower_inclusive: bool,
    upper_inclusive: bool,
}

fn maven_dot_qualifiers_are_dashes(version: &str) -> String {
    let lower = version.to_ascii_lowercase();
    let bytes = lower.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len() + 4);
    let mut index: usize = 0;
    while index < bytes.len() {
        let byte = bytes[index];
        if byte == b'.'
            && bytes
                .get(index + 1)
                .is_some_and(|next| !next.is_ascii_digit())
        {
            let mut after = index + 1;
            while after < bytes.len() && !bytes[after].is_ascii_digit() && bytes[after] != b'-' {
                after += 1;
            }
            if after == bytes.len() || bytes[after].is_ascii_digit() {
                out.push(b'-');
                index += 1;
                continue;
            }
        }
        out.push(byte);
        index += 1;
    }
    String::from_utf8(out).unwrap_or_default()
}

fn comparable_maven(version: &str) -> ComparableVersion {
    ComparableVersion::new(maven_dot_qualifiers_are_dashes(version.trim()).as_str())
}

pub fn maven_compare(left: &str, right: &str) -> Ordering {
    comparable_maven(left).cmp(&comparable_maven(right))
}

pub fn maven_version_eq(left: &str, right: &str) -> bool {
    let left_trimmed = left.trim();
    let right_trimmed = right.trim();
    if left_trimmed.is_empty() || right_trimmed.is_empty() {
        return false;
    }
    if left_trimmed.len() > MAX_VERSION_LEN || right_trimmed.len() > MAX_VERSION_LEN {
        return false;
    }
    maven_compare(left_trimmed, right_trimmed) == Ordering::Equal
}

fn maven_interval(
    content: &str,
    lower_inclusive: bool,
    upper_inclusive: bool,
) -> Option<MavenInterval> {
    let parts: Vec<&str> = content.split(',').collect();
    let (lower, upper) = match parts.as_slice() {
        [bound] => {
            let bound = bound.trim();
            if bound.is_empty() {
                return None;
            }
            (bound.to_owned(), bound.to_owned())
        }
        [low, high] => {
            let low = low.trim();
            let high = high.trim();
            if low.is_empty() && high.is_empty() {
                return None;
            }
            (low.to_owned(), high.to_owned())
        }
        _ => return None,
    };
    if lower.len() > MAX_VERSION_LEN || upper.len() > MAX_VERSION_LEN {
        return None;
    }
    Some(MavenInterval {
        lower: maven_bound(&lower),
        upper: maven_bound(&upper),
        lower_inclusive,
        upper_inclusive,
    })
}

fn maven_bound(bound: &str) -> Option<String> {
    if bound.is_empty() {
        None
    } else {
        Some(bound.to_owned())
    }
}

fn maven_intervals(scope: &str) -> Option<Vec<MavenInterval>> {
    let chars: Vec<char> = scope.chars().collect();
    let mut intervals: Vec<MavenInterval> = Vec::new();
    let mut index: usize = 0;
    while index < chars.len() {
        let current = chars[index];
        if current == '[' || current == '(' {
            let mut close_index = index + 1;
            while close_index < chars.len()
                && chars[close_index] != ']'
                && chars[close_index] != ')'
            {
                close_index += 1;
            }
            if close_index >= chars.len() {
                return None;
            }
            let content: String = chars[index + 1..close_index].iter().collect();
            if let Some(interval) =
                maven_interval(&content, current == '[', chars[close_index] == ']')
            {
                intervals.push(interval);
            }
            index = close_index + 1;
        } else if current == ',' || current.is_whitespace() {
            index += 1;
        } else {
            return None;
        }
    }
    if intervals.is_empty() {
        return None;
    }
    Some(intervals)
}

fn interval_matches(interval: &MavenInterval, version: &ComparableVersion) -> bool {
    if let Some(lower) = &interval.lower {
        match version.cmp(&comparable_maven(lower)) {
            Ordering::Less => return false,
            Ordering::Equal if !interval.lower_inclusive => return false,
            Ordering::Equal | Ordering::Greater => {}
        }
    }
    if let Some(upper) = &interval.upper {
        match version.cmp(&comparable_maven(upper)) {
            Ordering::Greater => return false,
            Ordering::Equal if !interval.upper_inclusive => return false,
            Ordering::Less | Ordering::Equal => {}
        }
    }
    true
}

pub fn maven_in_scope(scope: &str, version: &str) -> bool {
    let scope_trimmed = scope.trim();
    let version_trimmed = version.trim();
    if scope_trimmed.is_empty() || version_trimmed.is_empty() {
        return false;
    }
    if scope_trimmed.len() > MAX_SCOPE_LEN || version_trimmed.len() > MAX_VERSION_LEN {
        return false;
    }
    if !scope_trimmed.contains(['[', '(', ']', ')']) {
        return maven_version_eq(scope_trimmed, version_trimmed);
    }
    let Some(intervals) = maven_intervals(scope_trimmed) else {
        return false;
    };
    let version = comparable_maven(version_trimmed);
    intervals
        .iter()
        .any(|interval| interval_matches(interval, &version))
}
