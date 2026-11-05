use super::VersionScope;
use std::cmp::Ordering;

/// The OSV families whose version order is not covered by the older matchers.
pub const VERSION_FAMILIES: &[&str] = &[
    "hex",
    "pub",
    "packagist",
    "hackage",
    "swifturl",
    "julia",
    "opam",
    "cran",
];

pub fn is_version_family(set: &str) -> bool {
    VERSION_FAMILIES.contains(&set)
}

/// Matches one locked version against a scope such as `>=1.0, <1.4` or `=2.0.1`.
pub fn family_assess(set: &str, scope: &str, version: &str) -> VersionScope {
    match set {
        "hex" | "pub" | "swifturl" | "julia" => assess_with(scope, version, semver_key),
        "packagist" => assess_with(scope, version, composer_key),
        "hackage" | "cran" => assess_with(scope, version, dotted_key),
        "opam" => assess_with(scope, version, |text| Some(Debian(text.to_owned()))),
        "cpan" => assess_with(scope, version, cpan_key),
        _ => VersionScope::Incomplete,
    }
}

fn assess_with<K: Ord>(
    scope: &str,
    version: &str,
    key: impl Fn(&str) -> Option<K>,
) -> VersionScope {
    let scope = scope.trim();
    if scope.is_empty() {
        return VersionScope::Unaffected;
    }
    let Some(locked) = key(version.trim()) else {
        return VersionScope::Incomplete;
    };
    if scope == "*" {
        return VersionScope::Affected;
    }
    let exact_only = !scope.contains(',') && scope.starts_with('=');
    for part in scope.split(',') {
        let Some((op, text)) = split_clause(part) else {
            return VersionScope::Incomplete;
        };
        let Some(bound) = key(text) else {
            if exact_only {
                return VersionScope::Unaffected;
            }
            return VersionScope::Incomplete;
        };
        let order = locked.cmp(&bound);
        let holds = match op {
            ">=" => order != Ordering::Less,
            "<=" => order != Ordering::Greater,
            "!=" => order != Ordering::Equal,
            ">" => order == Ordering::Greater,
            "<" => order == Ordering::Less,
            _ => order == Ordering::Equal,
        };
        if !holds {
            return VersionScope::Unaffected;
        }
    }
    VersionScope::Affected
}

fn split_clause(part: &str) -> Option<(&str, &str)> {
    let part = part.trim();
    [">=", "<=", "!=", "==", ">", "<", "="]
        .into_iter()
        .find_map(|op| part.strip_prefix(op).map(|rest| (op, rest.trim())))
        .filter(|(_, rest)| !rest.is_empty())
}

fn strip_v(text: &str) -> &str {
    text.strip_prefix(['v', 'V']).unwrap_or(text)
}

fn semver_key(text: &str) -> Option<semver::Version> {
    semver::Version::parse(strip_v(text)).ok()
}

fn trim_zeros(mut parts: Vec<u64>) -> Vec<u64> {
    while parts.last() == Some(&0) {
        parts.pop();
    }
    parts
}

/// Dotted integers as in PVP and CRAN, where `1.0` and `1.0.0` are the same version.
fn dotted_key(text: &str) -> Option<Vec<u64>> {
    let parts = text
        .split(['.', '-'])
        .map(|part| part.parse::<u64>().ok())
        .collect::<Option<Vec<_>>>()?;
    Some(trim_zeros(parts))
}

/// CPAN order: `v1.2.3` is dotted, `1.23` is decimal read as `1.230`.
fn cpan_key(text: &str) -> Option<Vec<u64>> {
    let text = text.replace('_', "");
    let dotted = text.starts_with(['v', 'V']) || text.matches('.').count() > 1;
    let text = strip_v(&text);
    if dotted {
        return text
            .split('.')
            .map(|part| part.parse::<u64>().ok())
            .collect::<Option<Vec<_>>>()
            .map(trim_zeros);
    }
    let (whole, fraction) = text.split_once('.').unwrap_or((text, ""));
    let mut parts = vec![whole.parse::<u64>().ok()?];
    if !fraction.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    for chunk in fraction.as_bytes().chunks(3) {
        let mut padded = String::from_utf8_lossy(chunk).into_owned();
        while padded.len() < 3 {
            padded.push('0');
        }
        parts.push(padded.parse().ok()?);
    }
    Some(trim_zeros(parts))
}

/// Composer order: numeric parts, then dev < alpha < beta < RC < stable < patch.
fn composer_key(text: &str) -> Option<(Vec<u64>, u8, u64)> {
    let lowered = text.to_ascii_lowercase();
    let text = strip_v(lowered.split('+').next().unwrap_or(""));
    let split = text
        .find(|ch: char| !(ch.is_ascii_digit() || ch == '.'))
        .unwrap_or(text.len());
    let (main, rest) = text.split_at(split);
    let parts = main
        .trim_end_matches('.')
        .split('.')
        .map(|part| part.parse::<u64>().ok())
        .collect::<Option<Vec<_>>>()?;
    let rest = rest.trim_start_matches(['-', '_', '.']);
    if rest.is_empty() {
        return Some((trim_zeros(parts), 4, 0));
    }
    let word_end = rest
        .find(|ch: char| !ch.is_ascii_alphabetic())
        .unwrap_or(rest.len());
    let (word, number) = rest.split_at(word_end);
    let rank = match word {
        "dev" => 0,
        "alpha" | "a" => 1,
        "beta" | "b" => 2,
        "rc" => 3,
        "stable" => 4,
        "patch" | "pl" | "p" => 5,
        _ => return None,
    };
    let number = number.trim_start_matches(['-', '_', '.']);
    let number = if number.is_empty() {
        0
    } else {
        number.parse().ok()?
    };
    Some((trim_zeros(parts), rank, number))
}

/// A version compared the way dpkg compares them, which opam uses.
#[derive(Clone, Debug, Eq, PartialEq)]
struct Debian(String);

impl Ord for Debian {
    fn cmp(&self, other: &Self) -> Ordering {
        debian_compare(self.0.as_bytes(), other.0.as_bytes())
    }
}

impl PartialOrd for Debian {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

fn debian_weight(byte: Option<&u8>) -> i32 {
    match byte {
        None => 0,
        Some(b'~') => -1,
        Some(byte) if byte.is_ascii_alphabetic() => i32::from(*byte),
        Some(byte) => i32::from(*byte) + 256,
    }
}

fn debian_compare(mut left: &[u8], mut right: &[u8]) -> Ordering {
    while !left.is_empty() || !right.is_empty() {
        let left_text = left
            .iter()
            .take_while(|byte| !byte.is_ascii_digit())
            .count();
        let right_text = right
            .iter()
            .take_while(|byte| !byte.is_ascii_digit())
            .count();
        for index in 0..left_text.max(right_text) {
            let order = debian_weight(left[..left_text].get(index))
                .cmp(&debian_weight(right[..right_text].get(index)));
            if order != Ordering::Equal {
                return order;
            }
        }
        left = &left[left_text..];
        right = &right[right_text..];
        let left_digits = left.iter().take_while(|byte| byte.is_ascii_digit()).count();
        let right_digits = right
            .iter()
            .take_while(|byte| byte.is_ascii_digit())
            .count();
        let left_number = digits_value(&left[..left_digits]);
        let right_number = digits_value(&right[..right_digits]);
        let order = left_number
            .len()
            .cmp(&right_number.len())
            .then_with(|| left_number.cmp(right_number));
        if order != Ordering::Equal {
            return order;
        }
        left = &left[left_digits..];
        right = &right[right_digits..];
    }
    Ordering::Equal
}

fn digits_value(digits: &[u8]) -> &[u8] {
    let start = digits.iter().take_while(|byte| **byte == b'0').count();
    &digits[start..]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn affected(set: &str, scope: &str, version: &str) -> bool {
        family_assess(set, scope, version) == VersionScope::Affected
    }

    #[test]
    fn semver_families_order_prereleases_and_strip_v() {
        for set in ["hex", "pub", "swifturl", "julia"] {
            assert!(affected(set, ">=1.0.0, <1.4.2", "1.3.9"), "{set}");
            assert!(!affected(set, ">=1.0.0, <1.4.2", "1.4.2"), "{set}");
            assert!(affected(set, "<1.0.0", "1.0.0-rc.1"), "{set}");
            assert!(affected(set, "=2.0.1", "2.0.1"), "{set}");
        }
        assert!(affected("swifturl", "<1.5.0", "v1.4.0"));
        assert_eq!(
            family_assess("hex", "<1.0.0", "main"),
            VersionScope::Incomplete
        );
    }

    #[test]
    fn composer_order_uses_stability_suffixes() {
        assert!(affected("packagist", "<2.0.0", "v1.9.9"));
        assert!(affected("packagist", "<1.0.0", "1.0.0-RC1"));
        assert!(affected("packagist", ">=1.0.0-beta2, <1.0.0", "1.0.0-RC1"));
        assert!(!affected("packagist", "<1.0.0", "1.0.0"));
        assert!(!affected("packagist", "<1.0.0", "1.0.0-patch1"));
        assert!(affected("packagist", "<1.0.1", "1.0.0.5"));
        assert_eq!(
            family_assess("packagist", "<1.0.0", "dev-master"),
            VersionScope::Incomplete
        );
    }

    #[test]
    fn pvp_and_cran_compare_dotted_integers() {
        assert!(affected("hackage", "<1.10", "1.9.3"));
        assert!(!affected("hackage", "<1.0", "1.0.0"));
        assert!(affected("hackage", "=1.0", "1.0.0.0"));
        assert!(affected("cran", ">=1.2-3, <1.2-10", "1.2.9"));
        assert!(!affected("cran", "<1.2-3", "1.2-3"));
    }

    #[test]
    fn opam_follows_debian_ordering() {
        assert!(affected("opam", "<1.0.0", "1.0.0~beta1"));
        assert!(!affected("opam", "<1.0.0", "1.0.0"));
        assert!(affected("opam", "<1.10", "1.9"));
        assert!(affected("opam", ">=2.0, <2.1.0", "2.0+dev"));
        assert!(!affected("opam", "<2.0", "2.0+dev"));
        assert!(affected("opam", "=4.14.1", "4.14.1"));
    }

    #[test]
    fn unparsable_exact_scope_never_matches() {
        assert_eq!(
            family_assess("hex", "=main", "1.0.0"),
            VersionScope::Unaffected
        );
        assert_eq!(
            family_assess("hex", ">=main", "1.0.0"),
            VersionScope::Incomplete
        );
        assert_eq!(family_assess("hex", "", "1.0.0"), VersionScope::Unaffected);
        assert!(affected("hex", "*", "9.9.9"));
    }
}
