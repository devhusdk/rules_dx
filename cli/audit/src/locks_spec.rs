//! Shared `name (version)` entry grammar for indentation-based lock files
//! (Paket `paket.lock`, Bundler `Gemfile.lock`).

/// Number of leading whitespace bytes of `line`.
pub(crate) fn indent_of(line: &str) -> usize {
    line.len() - line.trim_start().len()
}

/// Parses `name (version)` where `version` has no parentheses. The last
/// well-formed group wins, so names may themselves contain parentheses.
/// Trailing text after the closing parenthesis is ignored. Both parts are
/// trimmed and must be non-empty.
pub(crate) fn split_paren_spec(line: &str) -> Option<(&str, &str)> {
    for (open, _) in line.rmatch_indices('(') {
        let rest = &line[open + 1..];
        let Some(close) = rest.find(['(', ')']) else {
            continue;
        };
        if close == 0 || !rest[close..].starts_with(')') {
            continue;
        }
        let name = line[..open].trim();
        let version = rest[..close].trim();
        if name.is_empty() || version.is_empty() {
            return None;
        }
        return Some((name, version));
    }
    None
}
