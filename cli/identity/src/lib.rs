#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

//! Package identity validation the dependency commands share.

use std::sync::LazyLock;

use regex::Regex;

const NPM_CHARSET: &str = "npm names use [A-Za-z0-9_.-] only";
const NPM_SCOPE_CHARSET: &str = "npm scope/name use [A-Za-z0-9_.-] only";
const MAVEN_CHARSET: &str = "maven group/artifact use [A-Za-z0-9_.-] only";

fn dotted_name_re() -> Option<&'static Regex> {
    static RE: LazyLock<Option<Regex>> = LazyLock::new(|| Regex::new(r"^[A-Za-z0-9_.-]+$").ok());
    RE.as_ref()
}

fn cargo_name_re() -> Option<&'static Regex> {
    static RE: LazyLock<Option<Regex>> = LazyLock::new(|| Regex::new(r"^[A-Za-z0-9_-]+$").ok());
    RE.as_ref()
}

fn scoped_npm_re() -> Option<&'static Regex> {
    static RE: LazyLock<Option<Regex>> =
        LazyLock::new(|| Regex::new(r"^@[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$").ok());
    RE.as_ref()
}

fn go_charset_re() -> Option<&'static Regex> {
    static RE: LazyLock<Option<Regex>> = LazyLock::new(|| Regex::new(r"^[A-Za-z0-9/._~+-]+$").ok());
    RE.as_ref()
}

/// Accepts a name using only `[A-Za-z0-9_.-]`.
fn dotted_name(text: &str) -> bool {
    if let Some(re) = dotted_name_re() {
        return re.is_match(text);
    }
    !text.is_empty()
        && text
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
}

/// Accepts a crate name using only `[A-Za-z0-9_-]`.
fn cargo_name(text: &str) -> bool {
    if let Some(re) = cargo_name_re() {
        return re.is_match(text);
    }
    !text.is_empty()
        && text
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// Splits the part after `@` into two named halves.
fn scoped_npm_parts(rest: &str) -> Option<(&str, &str)> {
    let (scope, name) = rest.split_once('/')?;
    if scope.is_empty() || name.is_empty() {
        return None;
    }
    Some((scope, name))
}

/// Accepts a name using only `[A-Za-z0-9_.-]` or reports the caller's reason.
pub fn validate_dotted(package: &str, reason: &'static str) -> Result<(), &'static str> {
    if dotted_name(package) {
        Ok(())
    } else {
        Err(reason)
    }
}

/// Accepts two names using only `[A-Za-z0-9_.-]` or reports the caller's reason.
pub fn validate_dotted_pair(
    left: &str,
    right: &str,
    reason: &'static str,
) -> Result<(), &'static str> {
    if dotted_name(left) && dotted_name(right) {
        Ok(())
    } else {
        Err(reason)
    }
}

/// Accepts a cargo crate name or reports why it is refused.
pub fn validate_cargo(package: &str) -> Result<(), &'static str> {
    if cargo_name(package) {
        Ok(())
    } else {
        Err("cargo crate names use [A-Za-z0-9_-] only")
    }
}

/// Accepts an npm package name or reports why it is refused.
pub fn validate_npm(package: &str) -> Result<(), &'static str> {
    if package.is_empty() || package.contains(':') || package.contains(' ') {
        return Err("npm package names never contain ':' or spaces");
    }
    if let Some(rest) = package.strip_prefix('@') {
        if let Some(re) = scoped_npm_re() {
            if re.is_match(package) {
                return Ok(());
            }
            return match scoped_npm_parts(rest) {
                Some(_) => Err(NPM_SCOPE_CHARSET),
                None => Err("scoped npm names are @scope/name"),
            };
        }
        return match scoped_npm_parts(rest) {
            Some((scope, name)) => validate_dotted_pair(scope, name, NPM_SCOPE_CHARSET),
            None => Err("scoped npm names are @scope/name"),
        };
    }
    if package.contains('/') {
        return Err("unscoped npm names never contain '/'");
    }
    validate_dotted(package, NPM_CHARSET)
}

/// Accepts a go module path or reports why it is refused.
pub fn validate_go(package: &str) -> Result<(), &'static str> {
    if package.is_empty() || package.contains(':') || package.contains(' ') {
        return Err("go module paths never contain ':' or spaces");
    }
    if package.starts_with('/') || package.ends_with('/') || package.contains("//") {
        return Err("go module paths use single '/' segments");
    }
    let charset_ok = if let Some(re) = go_charset_re() {
        re.is_match(package)
    } else {
        package
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '.' | '-' | '_' | '~' | '+'))
    };
    if charset_ok {
        Ok(())
    } else {
        Err("go module paths use [A-Za-z0-9/_.-~+] only")
    }
}

/// Accepts a maven identity or reports why it is refused.
pub fn validate_maven(package: &str) -> Result<(), &'static str> {
    let shape = || "maven identities are group:artifact";
    let Some((group, artifact)) = package.split_once(':') else {
        return Err(shape());
    };
    if group.is_empty()
        || artifact.is_empty()
        || artifact.contains(':')
        || group.contains(' ')
        || artifact.contains(' ')
    {
        return Err(shape());
    }
    validate_dotted_pair(group, artifact, MAVEN_CHARSET)
}

/// Accepts a nuget id or reports why it is refused.
pub fn validate_nuget(package: &str) -> Result<(), &'static str> {
    validate_dotted(package, "nuget ids use [A-Za-z0-9_.-] only")
}

#[cfg(test)]
mod tests {
    use super::*;

    const DOTTED: [&str; 5] = ["anyhow", "rules_dx", "a-b_c.d", "FSharp.Core", "x"];

    #[test]
    fn dotted_name_takes_letters_digits_dots_dashes_and_underscores() {
        for name in DOTTED {
            assert!(dotted_name(name), "{name}");
        }
        for name in ["", "bad name", "bad!name", "with/slash", "a:b"] {
            assert!(!dotted_name(name), "{name}");
        }
    }

    #[test]
    fn cargo_names_drop_the_dot() {
        assert!(validate_cargo("anyhow").is_ok());
        assert!(validate_cargo("a-b_c").is_ok());
        assert_eq!(
            validate_cargo("rules.dx"),
            Err("cargo crate names use [A-Za-z0-9_-] only")
        );
        assert_eq!(
            validate_cargo("bad name"),
            Err("cargo crate names use [A-Za-z0-9_-] only")
        );
    }

    #[test]
    fn npm_names_cover_plain_scoped_and_refused_shapes() {
        for name in [
            "anyhow",
            "@astrojs/compiler",
            "@scope/name.with-dots_and_underscores",
        ] {
            assert_eq!(validate_npm(name), Ok(()), "{name}");
        }
        assert_eq!(
            validate_npm("with:colon"),
            Err("npm package names never contain ':' or spaces")
        );
        assert_eq!(
            validate_npm("@scope"),
            Err("scoped npm names are @scope/name")
        );
        assert_eq!(
            validate_npm("@scope/"),
            Err("scoped npm names are @scope/name")
        );
        assert_eq!(
            validate_npm("@scope/bad!"),
            Err("npm scope/name use [A-Za-z0-9_.-] only")
        );
        assert_eq!(
            validate_npm("@scope/a/b"),
            Err("npm scope/name use [A-Za-z0-9_.-] only")
        );
        assert_eq!(
            validate_npm("with/slash"),
            Err("unscoped npm names never contain '/'")
        );
        assert_eq!(
            validate_npm("bad!name"),
            Err("npm names use [A-Za-z0-9_.-] only")
        );
    }

    #[test]
    fn go_paths_refuse_empty_colon_space_and_bad_edges() {
        for path in [
            "github.com/google/go-cmp",
            "gopkg.in/yaml.v3",
            "example.com/a~b+c",
        ] {
            assert_eq!(validate_go(path), Ok(()), "{path}");
        }
        for path in ["/leading", "trailing/", "double//slash"] {
            assert_eq!(
                validate_go(path),
                Err("go module paths use single '/' segments"),
                "{path}"
            );
        }
        for path in ["with:colon", "with space", ""] {
            assert_eq!(
                validate_go(path),
                Err("go module paths never contain ':' or spaces"),
                "{path}"
            );
        }
        assert_eq!(
            validate_go("bad!name"),
            Err("go module paths use [A-Za-z0-9/_.-~+] only")
        );
    }

    #[test]
    fn maven_identities_need_both_halves_inside_the_dotted_charset() {
        for identity in [
            "junit:junit",
            "org.junit.jupiter:junit-jupiter-api",
            "com.example:my_artifact-1.0",
        ] {
            assert_eq!(validate_maven(identity), Ok(()), "{identity}");
        }
        for identity in [":artifact", "group:", "group:art:ifact"] {
            assert_eq!(
                validate_maven(identity),
                Err("maven identities are group:artifact"),
                "{identity}"
            );
        }
        assert_eq!(
            validate_maven("gr oup:artifact"),
            Err("maven identities are group:artifact")
        );
        for identity in ["gr/oup:artifact", "gr@oup:artifact", "group:artifact!"] {
            assert_eq!(
                validate_maven(identity),
                Err("maven group/artifact use [A-Za-z0-9_.-] only"),
                "{identity}"
            );
        }
    }

    #[test]
    fn nuget_ids_follow_the_dotted_charset() {
        assert_eq!(validate_nuget("Newtonsoft.Json"), Ok(()));
        assert_eq!(
            validate_nuget("bad!name"),
            Err("nuget ids use [A-Za-z0-9_.-] only")
        );
    }

    #[test]
    fn a_caller_reason_survives_the_dotted_checks() {
        const REASON: &str = "ruby gem names use [A-Za-z0-9_.-] only";
        assert_eq!(validate_dotted("rake", REASON), Ok(()));
        assert_eq!(validate_dotted("bad!name", REASON), Err(REASON));
        assert_eq!(validate_dotted("", REASON), Err(REASON));
        assert_eq!(validate_dotted_pair("owner", "repo.name-1", REASON), Ok(()));
        assert_eq!(validate_dotted_pair("own!er", "repo", REASON), Err(REASON));
        assert_eq!(validate_dotted_pair("owner", "re po", REASON), Err(REASON));
    }
}
