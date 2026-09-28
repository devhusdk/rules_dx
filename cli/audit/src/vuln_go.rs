pub(crate) fn strip_go_v(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    for (index, current) in chars.iter().enumerate() {
        if *current == 'v' || *current == 'V' {
            let prev_ok = index == 0
                || matches!(
                    chars[index - 1],
                    ' ' | '\t' | ',' | '<' | '>' | '=' | '~' | '^' | '!' | '('
                );
            let next_ok = chars
                .get(index + 1)
                .is_some_and(|next| next.is_ascii_digit());
            if prev_ok && next_ok {
                continue;
            }
        }
        out.push(*current);
    }
    out
}

pub fn go_in_scope(scope: &str, version: &str) -> bool {
    let scope_norm = strip_go_v(scope);
    let version_norm = strip_go_v(version);
    let requirements = match semver::VersionReq::parse(&scope_norm) {
        Ok(requirements) => requirements,
        Err(_) => return false,
    };
    let version = match semver::Version::parse(&version_norm) {
        Ok(version) => version,
        Err(_) => return false,
    };
    requirements
        .comparators
        .iter()
        .all(|comparator| go_matches_impl(comparator, &version))
}

fn go_matches_impl(comparator: &semver::Comparator, version: &semver::Version) -> bool {
    match comparator.op {
        semver::Op::Exact | semver::Op::Wildcard => go_matches_exact(comparator, version),
        semver::Op::Greater => go_matches_greater(comparator, version),
        semver::Op::GreaterEq => {
            go_matches_exact(comparator, version) || go_matches_greater(comparator, version)
        }
        semver::Op::Less => go_matches_less(comparator, version),
        semver::Op::LessEq => {
            go_matches_exact(comparator, version) || go_matches_less(comparator, version)
        }
        semver::Op::Tilde => go_matches_tilde(comparator, version),
        semver::Op::Caret => go_matches_caret(comparator, version),
        _ => false,
    }
}

fn go_matches_exact(comparator: &semver::Comparator, version: &semver::Version) -> bool {
    if version.major != comparator.major {
        return false;
    }
    if let Some(minor) = comparator.minor {
        if version.minor != minor {
            return false;
        }
    }
    if let Some(patch) = comparator.patch {
        if version.patch != patch {
            return false;
        }
    }
    version.pre == comparator.pre
}

fn go_matches_greater(comparator: &semver::Comparator, version: &semver::Version) -> bool {
    if version.major != comparator.major {
        return version.major > comparator.major;
    }
    let Some(minor) = comparator.minor else {
        return false;
    };
    if version.minor != minor {
        return version.minor > minor;
    }
    let Some(patch) = comparator.patch else {
        return false;
    };
    if version.patch != patch {
        return version.patch > patch;
    }
    version.pre > comparator.pre
}

fn go_matches_less(comparator: &semver::Comparator, version: &semver::Version) -> bool {
    if version.major != comparator.major {
        return version.major < comparator.major;
    }
    let Some(minor) = comparator.minor else {
        return false;
    };
    if version.minor != minor {
        return version.minor < minor;
    }
    let Some(patch) = comparator.patch else {
        return false;
    };
    if version.patch != patch {
        return version.patch < patch;
    }
    version.pre < comparator.pre
}

fn go_matches_tilde(comparator: &semver::Comparator, version: &semver::Version) -> bool {
    if version.major != comparator.major {
        return false;
    }
    if let Some(minor) = comparator.minor {
        if version.minor != minor {
            return false;
        }
    }
    if let Some(patch) = comparator.patch {
        if version.patch != patch {
            return version.patch > patch;
        }
    }
    version.pre >= comparator.pre
}

fn go_matches_caret(comparator: &semver::Comparator, version: &semver::Version) -> bool {
    if version.major != comparator.major {
        return false;
    }
    let Some(minor) = comparator.minor else {
        return true;
    };
    let Some(patch) = comparator.patch else {
        if comparator.major > 0 {
            return version.minor >= minor;
        }
        return version.minor == minor;
    };
    if comparator.major > 0 {
        if version.minor != minor {
            return version.minor > minor;
        }
        if version.patch != patch {
            return version.patch > patch;
        }
    } else if minor > 0 {
        if version.minor != minor {
            return false;
        }
        if version.patch != patch {
            return version.patch > patch;
        }
    } else if version.minor != minor || version.patch != patch {
        return false;
    }
    version.pre >= comparator.pre
}
