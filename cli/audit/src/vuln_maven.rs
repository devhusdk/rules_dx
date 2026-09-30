use mvn_version::ComparableVersion;

const MAX_VERSION_LEN: usize = 256;
const MAX_SCOPE_LEN: usize = 4096;

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

pub fn maven_compare(left: &str, right: &str) -> std::cmp::Ordering {
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
    maven_compare(left_trimmed, right_trimmed) == std::cmp::Ordering::Equal
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
                && lower.len() <= MAX_VERSION_LEN
                && upper.len() <= MAX_VERSION_LEN
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
