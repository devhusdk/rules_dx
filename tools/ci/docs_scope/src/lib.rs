//! Validates the docs scope input and builds the dx lint arguments for it.

#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

pub const SCOPE_ENV: &str = "DX_DOCS_SCOPE";

fn is_package_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || "_.+@=,/-".contains(c)
}

fn is_pattern(pattern: &str) -> bool {
    let mut rest = pattern;
    if let Some(after) = rest.strip_prefix('@') {
        let after = after.strip_prefix('@').unwrap_or(after);
        let end = after
            .find(|c: char| !(c.is_ascii_alphanumeric() || "_.+-".contains(c)))
            .unwrap_or(after.len());
        rest = &after[end..];
    }
    let Some(rest) = rest.strip_prefix("//") else {
        return false;
    };
    let (package, name) = match rest.split_once(':') {
        Some((package, name)) => (package, Some(name)),
        None => (rest, None),
    };
    package.chars().all(is_package_char)
        && name.is_none_or(|n| !n.is_empty() && n.chars().all(is_package_char))
}

/// Returns the ordered scope patterns, or the diagnostic.
pub fn parse_scope(text: &str) -> Result<Vec<String>, String> {
    if text.trim_matches(' ').is_empty() {
        return Err(format!(
            "{SCOPE_ENV} is empty. Write patterns such as //... separated by spaces."
        ));
    }
    let patterns: Vec<String> = text
        .split(' ')
        .filter(|part| !part.is_empty())
        .map(str::to_string)
        .collect();
    if let Some(bad) = patterns.iter().find(|p| !is_pattern(p)) {
        return Err(format!(
            "{SCOPE_ENV} holds {bad:?}, which is not a target pattern. Write patterns such as //... or //pkg:target separated by spaces."
        ));
    }
    Ok(patterns)
}

/// Returns the bazel arguments that run dx lint over the scope.
pub fn lint_argv(scope: &str, config: &[String]) -> Result<Vec<String>, String> {
    let patterns = parse_scope(scope)?;
    let mut argv: Vec<String> = vec!["run".to_string()];
    argv.extend_from_slice(config);
    argv.extend(["@rules_dx//:dx", "--", "lint"].map(str::to_string));
    argv.extend(patterns);
    argv.push("--".to_string());
    argv.extend_from_slice(config);
    Ok(argv)
}
