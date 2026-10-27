use crate::vuln::{any_scope, VersionScope};
use nodejs_semver::{Range, Version};

pub fn npm_in_scope(scope: &str, version: &str) -> bool {
    npm_assess(scope, version) == VersionScope::Affected
}

pub fn npm_assess(scope: &str, version: &str) -> VersionScope {
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
    let Some(locked) = parse_npm_locked(version_trimmed) else {
        return VersionScope::Incomplete;
    };
    let branches: Vec<&str> = scope_trimmed.split("||").collect();
    if branches.len() > 64 {
        return VersionScope::Incomplete;
    }
    any_scope(
        branches
            .into_iter()
            .map(|branch| npm_branch_assess(branch.trim(), &locked)),
    )
}

fn parse_npm_locked(version: &str) -> Option<Version> {
    let mut text = version.trim();
    loop {
        if let Some(rest) = text.strip_prefix('v').or_else(|| text.strip_prefix('V')) {
            text = rest.trim_start();
            continue;
        }
        if let Some(rest) = text.strip_prefix('=') {
            text = rest.trim_start();
            continue;
        }
        break;
    }
    if text.is_empty() || text.len() > 256 {
        return None;
    }
    Version::parse(text).ok()
}

fn npm_branch_assess(branch: &str, locked: &Version) -> VersionScope {
    if branch.is_empty() || branch.len() > 4096 {
        return VersionScope::Incomplete;
    }
    let normalized = npm_normalize_branch(branch);
    if normalized.is_empty() {
        return VersionScope::Incomplete;
    }
    let tokens: Vec<&str> = normalized.split_whitespace().collect();
    let hyphen = tokens.len() == 3 && tokens[1] == "-";
    for bound in npm_bounds(&tokens, hyphen) {
        if !npm_wildcard_is_trailing(bound) || npm_has_leading_zero(bound) {
            return VersionScope::Incomplete;
        }
    }
    if !hyphen {
        for token in &tokens {
            let (operator, rest) = npm_split_operator(token);
            if operator.is_some() && npm_is_bare_wildcard(rest) {
                return VersionScope::Incomplete;
            }
            if tokens.len() > 1 && npm_is_bare_wildcard(token) {
                return VersionScope::Incomplete;
            }
        }
    }
    match Range::parse(&normalized) {
        Ok(range) if range.satisfies(locked) => VersionScope::Affected,
        Ok(_) => VersionScope::Unaffected,
        Err(_) => VersionScope::Incomplete,
    }
}

fn npm_bounds<'t>(tokens: &'t [&'t str], hyphen: bool) -> Vec<&'t str> {
    if hyphen {
        vec![tokens[0], tokens[2]]
    } else {
        tokens.to_vec()
    }
}

fn npm_normalize_branch(branch: &str) -> String {
    let source = branch.replace(',', " ");
    let tokens: Vec<&str> = source.split_whitespace().collect();
    if tokens.len() == 3 && tokens[1] == "-" && npm_is_bare_wildcard(tokens[2]) {
        let bound = if npm_is_bare_wildcard(tokens[0]) {
            tokens[2]
        } else {
            tokens[0]
        };
        return format!(">={bound}");
    }
    if tokens.len() == 1 && npm_is_bare_wildcard(tokens[0]) {
        return "*".to_owned();
    }
    let mut merged: Vec<String> = Vec::new();
    let mut index = 0;
    while index < tokens.len() {
        let token = tokens[index];
        if npm_is_operator(token) && index + 1 < tokens.len() {
            merged.push(format!("{token}{}", tokens[index + 1]));
            index += 2;
        } else {
            merged.push(token.to_owned());
            index += 1;
        }
    }
    merged.join(" ")
}

fn npm_is_operator(token: &str) -> bool {
    matches!(token, "<" | ">" | "<=" | ">=" | "=")
}

fn npm_split_operator(token: &str) -> (Option<&str>, &str) {
    for operator in [">=", "<=", ">", "<", "="] {
        if let Some(rest) = token.strip_prefix(operator) {
            return (Some(operator), rest);
        }
    }
    (None, token)
}

fn npm_wildcard_is_trailing(token: &str) -> bool {
    let core = npm_core(token);
    match core.find(['x', 'X', '*']) {
        None => true,
        Some(at) => core[at..]
            .chars()
            .all(|ch| ch == '.' || npm_is_wildcard(&ch.to_string())),
    }
}

fn npm_has_leading_zero(token: &str) -> bool {
    npm_core(token).split('.').any(|part| {
        let part = part.trim();
        part.len() > 1 && part.starts_with('0') && part.bytes().all(|byte| byte.is_ascii_digit())
    })
}

fn npm_core(token: &str) -> &str {
    let token = token.trim();
    let token = token
        .strip_prefix('v')
        .or_else(|| token.strip_prefix('V'))
        .unwrap_or(token);
    let token = token.split_once('+').map_or(token, |(before, _)| before);
    token.split_once('-').map_or(token, |(before, _)| before)
}

fn npm_is_bare_wildcard(token: &str) -> bool {
    npm_is_wildcard(npm_core(token).trim())
}

fn npm_is_wildcard(part: &str) -> bool {
    part == "x" || part == "X" || part == "*"
}
