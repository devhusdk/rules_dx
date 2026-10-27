use super::VersionScope;

fn compare_nuget_numeric(left: &str, right: &str) -> std::cmp::Ordering {
    if left.len() != right.len() {
        return left.len().cmp(&right.len());
    }
    left.cmp(right)
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum NugetPrereleaseLabel {
    Numeric(String),
    Alpha(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct NugetVersion {
    parts: [String; 4],
    prerelease: Option<Vec<NugetPrereleaseLabel>>,
}

fn parse_nuget_version(version: &str) -> Option<NugetVersion> {
    let trimmed = version.trim();
    if trimmed.is_empty() || trimmed.len() > 256 {
        return None;
    }
    if trimmed.contains('*') {
        return None;
    }
    let (without_metadata, _) = match trimmed.split_once('+') {
        Some((before, _)) => (before, true),
        None => (trimmed, false),
    };
    let without_metadata = without_metadata.trim();
    if without_metadata.is_empty() {
        return None;
    }
    let (core_str, pre_str) = match without_metadata.split_once('-') {
        Some((core, pre)) => (core.trim(), Some(pre.trim())),
        None => (without_metadata, None),
    };
    if core_str.is_empty() {
        return None;
    }
    let raw_parts: Vec<&str> = core_str.split('.').collect();
    if raw_parts.is_empty() || raw_parts.len() > 4 {
        return None;
    }
    let mut normalized: Vec<String> = Vec::new();
    for part in raw_parts {
        let part = part.trim();
        if part.is_empty() || !part.bytes().all(|byte| byte.is_ascii_digit()) {
            return None;
        }
        let stripped = part.trim_start_matches('0');
        normalized.push(if stripped.is_empty() {
            "0".to_owned()
        } else {
            stripped.to_owned()
        });
    }
    while normalized.len() < 4 {
        normalized.push("0".to_owned());
    }
    let parts: [String; 4] = [
        normalized[0].clone(),
        normalized[1].clone(),
        normalized[2].clone(),
        normalized[3].clone(),
    ];
    let prerelease = match pre_str {
        None => None,
        Some(pre) => {
            if pre.is_empty() {
                return None;
            }
            if pre.contains([
                '[', ']', '(', ')', ',', ' ', '\t', '\n', '\r', '+', '>', '<', '=', '^', '~', '!',
                '|', '&', ':', '/',
            ]) {
                return None;
            }
            let mut labels = Vec::new();
            for label in pre.split('.') {
                let label = label.trim();
                if label.is_empty() {
                    return None;
                }
                if !label
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
                {
                    return None;
                }
                if label.bytes().all(|byte| byte.is_ascii_digit()) {
                    let stripped = label.trim_start_matches('0');
                    labels.push(NugetPrereleaseLabel::Numeric(if stripped.is_empty() {
                        "0".to_owned()
                    } else {
                        stripped.to_owned()
                    }));
                } else {
                    labels.push(NugetPrereleaseLabel::Alpha(label.to_ascii_lowercase()));
                }
            }
            if labels.is_empty() {
                return None;
            }
            Some(labels)
        }
    };
    Some(NugetVersion { parts, prerelease })
}

pub fn nuget_compare(left: &str, right: &str) -> std::cmp::Ordering {
    let left_parsed = parse_nuget_version(left);
    let right_parsed = parse_nuget_version(right);
    match (left_parsed, right_parsed) {
        (Some(left_version), Some(right_version)) => {
            for index in 0..4 {
                match compare_nuget_numeric(&left_version.parts[index], &right_version.parts[index])
                {
                    std::cmp::Ordering::Equal => continue,
                    other => return other,
                }
            }
            match (left_version.prerelease, right_version.prerelease) {
                (None, None) => std::cmp::Ordering::Equal,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (Some(_), None) => std::cmp::Ordering::Less,
                (Some(left_labels), Some(right_labels)) => {
                    let common = left_labels.len().min(right_labels.len());
                    for index in 0..common {
                        match (&left_labels[index], &right_labels[index]) {
                            (
                                NugetPrereleaseLabel::Numeric(left_num),
                                NugetPrereleaseLabel::Numeric(right_num),
                            ) => match compare_nuget_numeric(left_num, right_num) {
                                std::cmp::Ordering::Equal => continue,
                                other => return other,
                            },
                            (NugetPrereleaseLabel::Numeric(_), NugetPrereleaseLabel::Alpha(_)) => {
                                return std::cmp::Ordering::Less;
                            }
                            (NugetPrereleaseLabel::Alpha(_), NugetPrereleaseLabel::Numeric(_)) => {
                                return std::cmp::Ordering::Greater;
                            }
                            (
                                NugetPrereleaseLabel::Alpha(left_text),
                                NugetPrereleaseLabel::Alpha(right_text),
                            ) => match left_text.cmp(right_text) {
                                std::cmp::Ordering::Equal => continue,
                                other => return other,
                            },
                        }
                    }
                    left_labels.len().cmp(&right_labels.len())
                }
            }
        }
        _ => std::cmp::Ordering::Equal,
    }
}

pub fn nuget_version_eq(left: &str, right: &str) -> bool {
    let left_trimmed = left.trim();
    let right_trimmed = right.trim();
    if left_trimmed.is_empty() || right_trimmed.is_empty() {
        return false;
    }
    if left_trimmed.len() > 256 || right_trimmed.len() > 256 {
        return false;
    }
    if parse_nuget_version(left_trimmed).is_none() || parse_nuget_version(right_trimmed).is_none() {
        return false;
    }
    nuget_compare(left_trimmed, right_trimmed) == std::cmp::Ordering::Equal
}

pub fn nuget_in_scope(scope: &str, version: &str) -> bool {
    nuget_assess(scope, version) == VersionScope::Affected
}

pub fn nuget_assess(scope: &str, version: &str) -> VersionScope {
    let scope_trimmed = scope.trim();
    let version_trimmed = version.trim();
    if scope_trimmed.is_empty() {
        return VersionScope::Unaffected;
    }
    if version_trimmed.is_empty() {
        return VersionScope::Incomplete;
    }
    if scope_trimmed.len() > 4096 || version_trimmed.len() > 256 {
        return VersionScope::Incomplete;
    }
    if scope_trimmed.contains('*') {
        return VersionScope::Incomplete;
    }
    if parse_nuget_version(version_trimmed).is_none() {
        return VersionScope::Incomplete;
    }
    let has_brackets = scope_trimmed.contains('[')
        || scope_trimmed.contains('(')
        || scope_trimmed.contains(']')
        || scope_trimmed.contains(')');
    if !has_brackets {
        if parse_nuget_version(scope_trimmed).is_none() {
            return VersionScope::Incomplete;
        }
        return scope_from(nuget_version_eq(scope_trimmed, version_trimmed));
    }
    let chars: Vec<char> = scope_trimmed.chars().collect();
    if chars.len() < 3 {
        return VersionScope::Incomplete;
    }
    if (chars[0] != '[' && chars[0] != '(')
        || (chars[chars.len() - 1] != ']' && chars[chars.len() - 1] != ')')
    {
        return VersionScope::Incomplete;
    }
    let inner: String = chars[1..chars.len() - 1].iter().collect();
    if inner.contains('[') || inner.contains('(') || inner.contains(']') || inner.contains(')') {
        return VersionScope::Incomplete;
    }
    let parts: Vec<&str> = inner.split(',').collect();
    if parts.len() == 1 {
        let bound = parts[0].trim();
        if bound.is_empty() {
            return VersionScope::Incomplete;
        }
        if chars[0] != '[' || chars[chars.len() - 1] != ']' {
            return VersionScope::Incomplete;
        }
        if bound.len() > 256 || parse_nuget_version(bound).is_none() {
            return VersionScope::Incomplete;
        }
        return scope_from(nuget_version_eq(bound, version_trimmed));
    }
    if parts.len() != 2 {
        return VersionScope::Incomplete;
    }
    let lower = parts[0].trim().to_owned();
    let upper = parts[1].trim().to_owned();
    if lower.is_empty() && upper.is_empty() {
        return VersionScope::Incomplete;
    }
    if lower.len() > 256 || upper.len() > 256 {
        return VersionScope::Incomplete;
    }
    if !lower.is_empty() && parse_nuget_version(&lower).is_none() {
        return VersionScope::Incomplete;
    }
    if !upper.is_empty() && parse_nuget_version(&upper).is_none() {
        return VersionScope::Incomplete;
    }
    scope_from(nuget_interval_matches(
        &lower,
        &upper,
        chars[0] == '[',
        chars[chars.len() - 1] == ']',
        version_trimmed,
    ))
}

fn scope_from(affected: bool) -> VersionScope {
    if affected {
        VersionScope::Affected
    } else {
        VersionScope::Unaffected
    }
}

fn nuget_interval_matches(
    lower: &str,
    upper: &str,
    lower_inclusive: bool,
    upper_inclusive: bool,
    version: &str,
) -> bool {
    if !lower.is_empty() {
        match nuget_compare(version, lower) {
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
        match nuget_compare(version, upper) {
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
