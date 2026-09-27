fn split_version_parts(value: &str, count: usize) -> Vec<String> {
    value
        .split(['.', '-'])
        .take(count)
        .map(|s| s.to_owned())
        .collect()
}

fn version_tuple(value: &str) -> Vec<i64> {
    let base = value.split('+').next().unwrap_or(value);
    base.split(['.', '-'])
        .take(3)
        .map(|part| {
            let digits: String = part.chars().take_while(|c| c.is_ascii_digit()).collect();
            digits.parse::<i64>().unwrap_or(0)
        })
        .collect()
}

pub(crate) fn versions_equal(spec: &str, locked: &str) -> bool {
    if spec.contains(['-', '+']) || locked.contains(['-', '+']) {
        return spec == locked;
    }
    fn numeric_parts(value: &str) -> Option<Vec<u64>> {
        let mut parts = Vec::new();
        for part in value.split('.') {
            if part.is_empty() || !part.bytes().all(|c| c.is_ascii_digit()) {
                return None;
            }
            parts.push(part.parse::<u64>().ok()?);
        }
        Some(parts)
    }
    match (numeric_parts(spec), numeric_parts(locked)) {
        (Some(mut a), Some(mut b)) => {
            while a.len() < b.len() {
                a.push(0);
            }
            while b.len() < a.len() {
                b.push(0);
            }
            a == b
        }
        _ => spec == locked,
    }
}

fn is_full_version(text: &str) -> bool {
    let mut parts = text.split(['.', '-']);
    let (Some(a), Some(b), Some(c)) = (parts.next(), parts.next(), parts.next()) else {
        return false;
    };
    !a.is_empty()
        && !b.is_empty()
        && !c.is_empty()
        && a.chars().all(|c| c.is_ascii_digit())
        && b.chars().all(|c| c.is_ascii_digit())
        && c.chars().next().is_some_and(|c| c.is_ascii_digit())
}

pub fn satisfies(spec: &str, locked: &str) -> bool {
    let mut s = spec
        .trim()
        .trim_matches(|c| c == '"' || c == '\'')
        .trim()
        .to_owned();
    if let Some(first) = s.split(';').next() {
        s = first.trim().to_owned();
    }
    if s.len() > 1
        && s.starts_with('v')
        && s[1..].chars().next().is_some_and(|c| c.is_ascii_digit())
    {
        s = s[1..].to_owned();
    }
    let locked_trim = locked.trim().to_owned();
    let locked_norm = if locked_trim.len() > 1
        && locked_trim.starts_with('v')
        && locked_trim[1..]
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_digit())
    {
        locked_trim[1..].to_owned()
    } else {
        locked_trim
    };
    let mut exact = false;
    if let Some(rest) = s.strip_prefix("==") {
        s = rest.trim().to_owned();
        exact = true;
    } else if let Some(rest) = s.strip_prefix('=') {
        s = rest.trim().to_owned();
        exact = true;
    } else if let Some(rest) = s.strip_prefix('^') {
        s = rest.trim().to_owned();
    } else if let Some(rest) = s.strip_prefix('~') {
        s = rest.trim().to_owned();
        let lv = locked_norm
            .split('+')
            .next()
            .unwrap_or(&locked_norm)
            .to_owned();
        let sp = split_version_parts(&s, 2);
        let lp = split_version_parts(&lv, 2);
        if sp.len() == 2 && lp.len() == 2 {
            let parse_pair = |pair: &[String]| -> Option<(i64, i64)> {
                let a = pair[0].parse::<i64>().ok()?;
                let b = pair[1].parse::<i64>().ok()?;
                Some((a, b))
            };
            if let (Some(a), Some(b)) = (parse_pair(&sp), parse_pair(&lp)) {
                return a == b;
            }
            return s == lv;
        }
        return s == lv;
    } else if s.starts_with(">=") || s.starts_with("<=") {
        let op = s[..2].to_owned();
        let mut rest = s[2..].trim().to_owned();
        if let Some(first) = rest.split(',').next() {
            rest = first.trim().to_owned();
        }
        let locked_t = version_tuple(&locked_norm);
        let want_t = version_tuple(&rest);
        if op == ">=" {
            return locked_t >= want_t;
        }
        return locked_t <= want_t;
    }
    if s == "*" || s.is_empty() {
        return true;
    }
    if exact || is_full_version(&s) {
        return versions_equal(&s, locked_norm.trim());
    }
    let smajor = s.split('.').next().unwrap_or("").trim().to_owned();
    let lmajor = locked_norm
        .split('.')
        .next()
        .unwrap_or("")
        .trim()
        .to_owned();
    if !smajor.is_empty() && smajor.chars().all(|c| c.is_ascii_digit()) {
        return smajor == lmajor;
    }
    smajor == lmajor
}
