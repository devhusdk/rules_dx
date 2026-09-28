pub fn npm_in_scope(scope: &str, version: &str) -> bool {
    let scope_trimmed = scope.trim();
    let version_trimmed = version.trim();
    if scope_trimmed.is_empty() || version_trimmed.is_empty() {
        return false;
    }
    if scope_trimmed.len() > 4096 || version_trimmed.len() > 256 {
        return false;
    }
    let locked = match parse_npm_locked(version_trimmed) {
        Some(locked) => locked,
        None => return false,
    };
    let branches: Vec<&str> = scope_trimmed.split("||").collect();
    if branches.len() > 64 {
        return false;
    }
    for branch in branches {
        if npm_branch_matches(branch.trim(), &locked) {
            return true;
        }
    }
    false
}

fn parse_npm_locked(version: &str) -> Option<semver::Version> {
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
    semver::Version::parse(text).ok()
}

struct NpmPartial {
    major: String,
    minor: Option<String>,
    patch: Option<String>,
    prerelease: Option<String>,
}

fn parse_npm_partial(text: &str) -> Option<NpmPartial> {
    let text = text.trim();
    let text = text
        .strip_prefix('v')
        .or_else(|| text.strip_prefix('V'))
        .unwrap_or(text);
    let text = text.trim();
    if text.is_empty() || text.len() > 256 {
        return None;
    }
    let core_and_pre = match text.split_once('+') {
        Some((before, _)) => before,
        None => text,
    };
    if core_and_pre.is_empty() {
        return None;
    }
    let (core, prerelease) = match core_and_pre.split_once('-') {
        Some((core, pre)) => (core, Some(pre)),
        None => (core_and_pre, None),
    };
    let raw: Vec<&str> = core.split('.').collect();
    if raw.is_empty() || raw.len() > 3 {
        return None;
    }
    let mut parts: Vec<Option<String>> = Vec::new();
    for part in raw {
        let part = part.trim();
        if part.is_empty() {
            return None;
        }
        if part == "x" || part == "X" || part == "*" {
            parts.push(None);
        } else if is_npm_numeric(part) {
            parts.push(Some(part.to_owned()));
        } else {
            return None;
        }
    }
    let mut seen_wildcard = false;
    for part in &parts {
        if part.is_none() {
            seen_wildcard = true;
        } else if seen_wildcard {
            return None;
        }
    }
    while parts.len() < 3 {
        parts.push(None);
    }
    let prerelease = match prerelease {
        None => None,
        Some(pre) => {
            if parts.iter().any(Option::is_none) || pre.is_empty() {
                return None;
            }
            for label in pre.split('.') {
                if label.is_empty()
                    || !label
                        .bytes()
                        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
                {
                    return None;
                }
            }
            Some(pre.to_owned())
        }
    };
    Some(NpmPartial {
        major: parts[0].clone()?,
        minor: parts[1].clone(),
        patch: parts[2].clone(),
        prerelease,
    })
}

fn is_npm_numeric(part: &str) -> bool {
    if part.is_empty() || !part.bytes().all(|byte| byte.is_ascii_digit()) {
        return false;
    }
    part.len() == 1 || !part.starts_with('0')
}

fn increment_npm_numeric(part: &str) -> Option<String> {
    if !is_npm_numeric(part) {
        return None;
    }
    let mut digits: Vec<u8> = part.bytes().map(|byte| byte - b'0').collect();
    let mut carry = true;
    for digit in digits.iter_mut().rev() {
        if carry {
            if *digit == 9 {
                *digit = 0;
            } else {
                *digit += 1;
                carry = false;
            }
        }
    }
    let mut out = String::new();
    if carry {
        out.push('1');
    }
    for digit in digits {
        out.push((digit + b'0') as char);
    }
    Some(out)
}

fn npm_partial_lower(partial: &NpmPartial) -> Option<String> {
    let mut bound = format!(
        ">={}.{}.{}",
        partial.major,
        partial.minor.as_deref().unwrap_or("0"),
        partial.patch.as_deref().unwrap_or("0")
    );
    if let Some(pre) = &partial.prerelease {
        bound.push('-');
        bound.push_str(pre);
    }
    Some(bound)
}

fn npm_partial_upper(partial: &NpmPartial) -> Option<(String, bool)> {
    if let Some(patch) = &partial.patch {
        let mut bound = format!(
            "{}.{}.{}",
            partial.major,
            partial.minor.as_deref().unwrap_or("0"),
            patch
        );
        if let Some(pre) = &partial.prerelease {
            bound.push('-');
            bound.push_str(pre);
        }
        return Some((bound, true));
    }
    if let Some(minor) = &partial.minor {
        let bumped = increment_npm_numeric(minor)?;
        return Some((format!("<{}.{}.0", partial.major, bumped), false));
    }
    let bumped = increment_npm_numeric(&partial.major)?;
    Some((format!("<{bumped}.0.0"), false))
}

fn npm_branch_matches(branch: &str, locked: &semver::Version) -> bool {
    if branch.is_empty() || branch.len() > 4096 {
        return false;
    }
    let normalized = branch.replace(',', " ");
    let tokens: Vec<&str> = normalized.split_whitespace().collect();
    if tokens.is_empty() || tokens.len() > 64 {
        return false;
    }
    if tokens.len() == 3 && tokens[1] == "-" {
        return npm_hyphen_matches(tokens[0], tokens[2], locked);
    }
    let mut merged: Vec<String> = Vec::new();
    let mut index = 0;
    while index < tokens.len() {
        let token = tokens[index];
        if (token == "<" || token == ">" || token == "<=" || token == ">=" || token == "=")
            && index + 1 < tokens.len()
        {
            merged.push(format!("{}{}", token, tokens[index + 1]));
            index += 2;
        } else {
            merged.push(token.to_owned());
            index += 1;
        }
    }
    let mut comparators: Vec<String> = Vec::new();
    for token in &merged {
        match normalize_npm_comparator(token) {
            Some(expanded) => comparators.extend(expanded),
            None => return false,
        }
    }
    if comparators.is_empty() {
        return false;
    }
    match semver::VersionReq::parse(&comparators.join(", ")) {
        Ok(requirements) => requirements.matches(locked),
        Err(_) => false,
    }
}

fn npm_hyphen_matches(left: &str, right: &str, locked: &semver::Version) -> bool {
    let left_trimmed = left.trim();
    let right_trimmed = right.trim();
    if left_trimmed.is_empty() || right_trimmed.is_empty() {
        return false;
    }
    let lower = if is_npm_wildcard(left_trimmed) {
        None
    } else {
        match parse_npm_partial(left_trimmed) {
            Some(partial) => npm_partial_lower(&partial),
            None => return false,
        }
    };
    let upper = if is_npm_wildcard(right_trimmed) {
        None
    } else {
        match parse_npm_partial(right_trimmed) {
            Some(partial) => match npm_partial_upper(&partial) {
                Some((bound, true)) => Some(format!("<={bound}")),
                Some((bound, false)) => Some(bound),
                None => return false,
            },
            None => return false,
        }
    };
    match (lower, upper) {
        (None, None) => false,
        (Some(low), None) => match semver::VersionReq::parse(&low) {
            Ok(requirements) => requirements.matches(locked),
            Err(_) => false,
        },
        (None, Some(high)) => match semver::VersionReq::parse(&high) {
            Ok(requirements) => requirements.matches(locked),
            Err(_) => false,
        },
        (Some(low), Some(high)) => match semver::VersionReq::parse(&format!("{low}, {high}")) {
            Ok(requirements) => requirements.matches(locked),
            Err(_) => false,
        },
    }
}

fn is_npm_wildcard(text: &str) -> bool {
    let text = text.trim();
    let text = text
        .strip_prefix('v')
        .or_else(|| text.strip_prefix('V'))
        .unwrap_or(text);
    text.trim() == "*" || text.trim() == "x" || text.trim() == "X"
}

fn normalize_npm_comparator(token: &str) -> Option<Vec<String>> {
    if token.is_empty() || token.len() > 256 {
        return None;
    }
    let (operator, rest) = if let Some(rest) = token.strip_prefix(">=") {
        (">=", rest)
    } else if let Some(rest) = token.strip_prefix("<=") {
        ("<=", rest)
    } else if let Some(rest) = token.strip_prefix('>') {
        (">", rest)
    } else if let Some(rest) = token.strip_prefix('<') {
        ("<", rest)
    } else if let Some(rest) = token.strip_prefix('=') {
        ("=", rest)
    } else if let Some(rest) = token.strip_prefix('^') {
        ("^", rest)
    } else if let Some(rest) = token.strip_prefix('~') {
        ("~", rest)
    } else {
        ("", token)
    };
    if rest.is_empty() {
        return None;
    }
    if rest.starts_with(['=', '>', '<', '^', '~', '!']) {
        return None;
    }
    let rest = rest
        .strip_prefix('v')
        .or_else(|| rest.strip_prefix('V'))
        .unwrap_or(rest);
    if rest.is_empty() {
        return None;
    }
    if rest == "*" || rest == "x" || rest == "X" {
        if operator.is_empty() {
            return Some(vec!["*".to_owned()]);
        }
        return None;
    }
    let partial = parse_npm_partial(rest)?;
    let full = partial.minor.is_some() && partial.patch.is_some();
    match operator {
        "" | "=" => {
            if full {
                let mut exact = format!(
                    "{}.{}.{}",
                    partial.major,
                    partial.minor.as_deref().unwrap_or("0"),
                    partial.patch.as_deref().unwrap_or("0")
                );
                if let Some(pre) = &partial.prerelease {
                    exact.push('-');
                    exact.push_str(pre);
                }
                Some(vec![format!("={exact}")])
            } else {
                npm_partial_range(&partial)
            }
        }
        ">=" => {
            if full {
                Some(vec![format!(">={}", npm_full_text(&partial))])
            } else {
                Some(vec![format!(">={}", npm_fill_zero(&partial))])
            }
        }
        "<=" => {
            if full {
                Some(vec![format!("<={}", npm_full_text(&partial))])
            } else {
                npm_exclusive_upper(&partial).map(|bound| vec![bound])
            }
        }
        ">" => {
            if full {
                Some(vec![format!(">{}", npm_full_text(&partial))])
            } else {
                npm_inclusive_next(&partial).map(|bound| vec![bound])
            }
        }
        "<" => {
            if full {
                Some(vec![format!("<{}", npm_full_text(&partial))])
            } else {
                Some(vec![format!("<{}", npm_fill_zero(&partial))])
            }
        }
        "^" => npm_caret_range(&partial),
        "~" => npm_tilde_range(&partial),
        _ => None,
    }
}

fn npm_full_text(partial: &NpmPartial) -> String {
    let mut out = format!(
        "{}.{}.{}",
        partial.major,
        partial.minor.as_deref().unwrap_or("0"),
        partial.patch.as_deref().unwrap_or("0")
    );
    if let Some(pre) = &partial.prerelease {
        out.push('-');
        out.push_str(pre);
    }
    out
}

fn npm_fill_zero(partial: &NpmPartial) -> String {
    format!(
        "{}.{}.{}",
        partial.major,
        partial.minor.as_deref().unwrap_or("0"),
        partial.patch.as_deref().unwrap_or("0")
    )
}

fn npm_partial_range(partial: &NpmPartial) -> Option<Vec<String>> {
    Some(vec![
        format!(">={}", npm_fill_zero(partial)),
        npm_exclusive_upper(partial)?,
    ])
}

fn npm_exclusive_upper(partial: &NpmPartial) -> Option<String> {
    if let Some(minor) = &partial.minor {
        Some(format!(
            "<{}.{}.0",
            partial.major,
            increment_npm_numeric(minor)?
        ))
    } else {
        Some(format!("<{}.0.0", increment_npm_numeric(&partial.major)?))
    }
}

fn npm_inclusive_next(partial: &NpmPartial) -> Option<String> {
    if let Some(minor) = &partial.minor {
        Some(format!(
            ">={}.{}.0",
            partial.major,
            increment_npm_numeric(minor)?
        ))
    } else {
        Some(format!(">={}.0.0", increment_npm_numeric(&partial.major)?))
    }
}

fn npm_caret_range(partial: &NpmPartial) -> Option<Vec<String>> {
    let minor = partial.minor.as_deref().unwrap_or("0");
    let patch = partial.patch.as_deref().unwrap_or("0");
    let mut lower = format!(">={}.{}.{}", partial.major, minor, patch);
    if let Some(pre) = &partial.prerelease {
        lower.push('-');
        lower.push_str(pre);
    }
    let upper = if partial.major != "0" {
        let bumped = increment_npm_numeric(&partial.major)?;
        format!("<{bumped}.0.0")
    } else if partial.minor.as_deref().unwrap_or("0") != "0" {
        let bumped = increment_npm_numeric(minor)?;
        format!("<0.{bumped}.0")
    } else if partial.patch.is_some() {
        let bumped = increment_npm_numeric(patch)?;
        format!("<0.0.{bumped}")
    } else if partial.minor.is_some() {
        let bumped = increment_npm_numeric(minor)?;
        format!("<0.{bumped}.0")
    } else {
        let bumped = increment_npm_numeric(&partial.major)?;
        format!("<{bumped}.0.0")
    };
    Some(vec![lower, upper])
}

fn npm_tilde_range(partial: &NpmPartial) -> Option<Vec<String>> {
    let minor = partial.minor.as_deref().unwrap_or("0");
    let patch = partial.patch.as_deref().unwrap_or("0");
    let mut lower = format!(">={}.{}.{}", partial.major, minor, patch);
    if let Some(pre) = &partial.prerelease {
        lower.push('-');
        lower.push_str(pre);
    }
    let upper = match &partial.minor {
        Some(minor_value) => {
            let bumped = increment_npm_numeric(minor_value)?;
            format!("<{}.{}.0", partial.major, bumped)
        }
        None => {
            let bumped = increment_npm_numeric(&partial.major)?;
            format!("<{bumped}.0.0")
        }
    };
    Some(vec![lower, upper])
}
