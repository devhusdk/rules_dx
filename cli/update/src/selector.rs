use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use regex::Regex;

use super::sets::SetId;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Selector {
    Set(SetId),
    Package(SetId, String),
    Target(String),
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum SelectorError {
    #[error("empty selector")]
    Empty,
    #[error("unknown update selector {selector:?}; expected cargo|npm|maven|nuget|go|uv|npm-tools|uv-tools|npm-adopt|npm-adopt-polyglot|uv-adopt|uv-adopt-polyglot, set:package, or a label/path")]
    UnknownSelector { selector: String },
    #[error("invalid package {package:?} for set {set}: {reason}")]
    InvalidPackage {
        set: &'static str,
        package: String,
        reason: &'static str,
    },
    #[error(
        "no owning dependency set for {target:?} (non-dependency paths are out of update scope)"
    )]
    NoOwningSet { target: String },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SetRequest {
    Full,
    Packages(Vec<String>),
}

pub fn parse_selector(text: &str) -> Result<Selector, SelectorError> {
    if text.is_empty() {
        return Err(SelectorError::Empty);
    }
    if let Some((head, tail)) = text.split_once(':') {
        if let Some(set) = SetId::parse(head) {
            if tail.is_empty() {
                return Err(SelectorError::InvalidPackage {
                    set: set.name(),
                    package: tail.to_owned(),
                    reason: "package identity is empty",
                });
            }
            validate_package(set, tail)?;
            return Ok(Selector::Package(set, tail.to_owned()));
        }
    }
    if let Some(set) = SetId::parse(text) {
        return Ok(Selector::Set(set));
    }
    if is_target_shape(text) {
        return Ok(Selector::Target(text.to_owned()));
    }
    Err(SelectorError::UnknownSelector {
        selector: text.to_owned(),
    })
}

fn target_shape_re() -> Option<&'static Regex> {
    static RE: OnceLock<Regex> = OnceLock::new();
    if let Some(compiled) = RE.get() {
        return Some(compiled);
    }
    match Regex::new(r"^(//|@)|[/.]") {
        Ok(compiled) => {
            let _ = RE.set(compiled);
            RE.get()
        }
        Err(_) => None,
    }
}

fn is_target_shape(text: &str) -> bool {
    if let Some(re) = target_shape_re() {
        return text == "..." || re.is_match(text);
    }
    text.starts_with("//")
        || text.starts_with('@')
        || text.contains('/')
        || text.contains('.')
        || text == "..."
}

fn dotted_name_re() -> Option<&'static Regex> {
    static RE: OnceLock<Regex> = OnceLock::new();
    if let Some(compiled) = RE.get() {
        return Some(compiled);
    }
    match Regex::new(r"^[A-Za-z0-9_.-]+$") {
        Ok(compiled) => {
            let _ = RE.set(compiled);
            RE.get()
        }
        Err(_) => None,
    }
}

fn cargo_name_re() -> Option<&'static Regex> {
    static RE: OnceLock<Regex> = OnceLock::new();
    if let Some(compiled) = RE.get() {
        return Some(compiled);
    }
    match Regex::new(r"^[A-Za-z0-9_-]+$") {
        Ok(compiled) => {
            let _ = RE.set(compiled);
            RE.get()
        }
        Err(_) => None,
    }
}

fn scoped_npm_re() -> Option<&'static Regex> {
    static RE: OnceLock<Regex> = OnceLock::new();
    if let Some(compiled) = RE.get() {
        return Some(compiled);
    }
    match Regex::new(r"^@[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$") {
        Ok(compiled) => {
            let _ = RE.set(compiled);
            RE.get()
        }
        Err(_) => None,
    }
}

fn go_charset_re() -> Option<&'static Regex> {
    static RE: OnceLock<Regex> = OnceLock::new();
    if let Some(compiled) = RE.get() {
        return Some(compiled);
    }
    match Regex::new(r"^[A-Za-z0-9/._~+-]+$") {
        Ok(compiled) => {
            let _ = RE.set(compiled);
            RE.get()
        }
        Err(_) => None,
    }
}

fn is_dotted_name(text: &str) -> bool {
    if let Some(re) = dotted_name_re() {
        return re.is_match(text);
    }
    !text.is_empty()
        && text
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
}

fn is_cargo_name(text: &str) -> bool {
    if let Some(re) = cargo_name_re() {
        return re.is_match(text);
    }
    !text.is_empty()
        && text
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

fn validate_package(set: SetId, package: &str) -> Result<(), SelectorError> {
    let invalid = |reason: &'static str| SelectorError::InvalidPackage {
        set: set.name(),
        package: package.to_owned(),
        reason,
    };
    match set {
        SetId::Cargo => {
            if !is_cargo_name(package) {
                return Err(invalid("cargo crate names use [A-Za-z0-9_-] only"));
            }
            Ok(())
        }
        SetId::Npm | SetId::NpmTools | SetId::NpmAdopt | SetId::NpmAdoptPolyglot => {
            if package.is_empty() || package.contains(':') || package.contains(' ') {
                return Err(invalid("npm package names never contain ':' or spaces"));
            }
            if let Some(rest) = package.strip_prefix('@') {
                if let Some(re) = scoped_npm_re() {
                    if re.is_match(package) {
                        return Ok(());
                    }
                    let (scope, slash, name) = match rest.find('/') {
                        Some(idx) => (&rest[..idx], true, &rest[idx + 1..]),
                        None => ("", false, ""),
                    };
                    let _ = slash;
                    if scope.is_empty() || name.is_empty() || !slash {
                        return Err(invalid("scoped npm names are @scope/name"));
                    }
                    return Err(invalid("npm scope/name use [A-Za-z0-9_.-] only"));
                }
                let (scope, slash, name) = match rest.find('/') {
                    Some(idx) => (&rest[..idx], true, &rest[idx + 1..]),
                    None => ("", false, ""),
                };
                let _ = slash;
                if scope.is_empty() || name.is_empty() || !slash {
                    return Err(invalid("scoped npm names are @scope/name"));
                }
                if !is_dotted_name(scope) || !is_dotted_name(name) {
                    return Err(invalid("npm scope/name use [A-Za-z0-9_.-] only"));
                }
                return Ok(());
            }
            if package.contains('/') {
                return Err(invalid("unscoped npm names never contain '/'"));
            }
            if !is_dotted_name(package) {
                return Err(invalid("npm names use [A-Za-z0-9_.-] only"));
            }
            Ok(())
        }
        SetId::Maven => {
            let (group, artifact) = match package.split_once(':') {
                Some((g, a)) => (g, a),
                None => {
                    return Err(invalid("maven identities are group:artifact"));
                }
            };
            if group.is_empty()
                || artifact.is_empty()
                || artifact.contains(':')
                || group.contains(' ')
                || artifact.contains(' ')
            {
                return Err(invalid("maven identities are group:artifact"));
            }
            Ok(())
        }
        SetId::NuGet => {
            if !is_dotted_name(package) {
                return Err(invalid("nuget ids use [A-Za-z0-9_.-] only"));
            }
            Ok(())
        }
        SetId::Uv | SetId::UvTools | SetId::UvAdopt | SetId::UvAdoptPolyglot => {
            if !is_dotted_name(package) {
                return Err(invalid("uv package names use [A-Za-z0-9_.-] only"));
            }
            Ok(())
        }
        SetId::Go => {
            if package.is_empty()
                || package.contains(':')
                || package.contains(' ')
                || package.starts_with('/')
                || package.ends_with('/')
                || package.contains("//")
            {
                return Err(invalid("go module paths never contain ':' or spaces"));
            }
            let charset_ok = if let Some(re) = go_charset_re() {
                re.is_match(package)
            } else {
                package.chars().all(|c| {
                    c.is_ascii_alphanumeric() || matches!(c, '/' | '.' | '-' | '_' | '~' | '+')
                })
            };
            if !charset_ok {
                return Err(invalid("go module paths use [A-Za-z0-9/_.-~+] only"));
            }
            Ok(())
        }
    }
}

pub fn owning_sets(target: &str) -> Vec<SetId> {
    if target == "//..." {
        return SetId::ALL.to_vec();
    }
    if target.contains("MODULE.bazel") {
        return SetId::ALL.to_vec();
    }
    let package = package_path(target);
    if package.is_empty()
        || (!target.starts_with("//") && !target.starts_with('@') && !target.contains('/'))
    {
        return root_owning_sets(target);
    }
    if has_prefix(&package, "quality/tools/javascript") {
        return vec![SetId::NpmTools];
    }
    if has_prefix(&package, "quality/tools/python") {
        return vec![SetId::UvTools];
    }
    if has_prefix(&package, "python/tests/fixtures/hello") {
        return vec![SetId::Uv];
    }
    if has_prefix(&package, "examples/adopt-python") {
        return vec![SetId::UvAdopt];
    }
    if has_prefix(&package, "examples/adopt-js-ts") {
        return vec![SetId::NpmAdopt];
    }
    if has_prefix(&package, "examples/adopt-rust") {
        return vec![SetId::Cargo];
    }
    if has_prefix(&package, "examples/adopt-java") || has_prefix(&package, "examples/adopt-kotlin")
    {
        return vec![SetId::Maven];
    }
    if has_prefix(&package, "examples/adopt-go") {
        return vec![SetId::Go];
    }
    if has_prefix(&package, "examples/adopt-polyglot") {
        return vec![SetId::NpmAdoptPolyglot, SetId::UvAdoptPolyglot];
    }
    if has_prefix(&package, "python") {
        return vec![SetId::Uv];
    }
    if has_prefix(&package, "docs/ir") {
        return vec![SetId::Cargo];
    }
    for prefix in ["rust", "cli", "generation", "env", "quality"] {
        if has_prefix(&package, prefix) {
            return vec![SetId::Cargo];
        }
    }
    for prefix in ["javascript", "typescript", "astro", "svelte", "vue", "mdx"] {
        if has_prefix(&package, prefix) {
            return vec![SetId::Npm];
        }
    }
    for prefix in ["java", "kotlin", "scala", "third_party/jvm"] {
        if has_prefix(&package, prefix) {
            return vec![SetId::Maven];
        }
    }
    for prefix in ["csharp", "fsharp", "third_party/dotnet", "paket-files"] {
        if has_prefix(&package, prefix) {
            return vec![SetId::NuGet];
        }
    }
    if has_prefix(&package, "go") {
        return vec![SetId::Go];
    }
    vec![]
}

fn recursive_suffix_re() -> Option<&'static Regex> {
    static RE: OnceLock<Regex> = OnceLock::new();
    if let Some(compiled) = RE.get() {
        return Some(compiled);
    }
    match Regex::new(r"/\.\.\.$") {
        Ok(compiled) => {
            let _ = RE.set(compiled);
            RE.get()
        }
        Err(_) => None,
    }
}

fn trailing_slash_re() -> Option<&'static Regex> {
    static RE: OnceLock<Regex> = OnceLock::new();
    if let Some(compiled) = RE.get() {
        return Some(compiled);
    }
    match Regex::new(r"/+$") {
        Ok(compiled) => {
            let _ = RE.set(compiled);
            RE.get()
        }
        Err(_) => None,
    }
}

fn dot_slash_prefix_re() -> Option<&'static Regex> {
    static RE: OnceLock<Regex> = OnceLock::new();
    if let Some(compiled) = RE.get() {
        return Some(compiled);
    }
    match Regex::new(r"^(?:\./)+") {
        Ok(compiled) => {
            let _ = RE.set(compiled);
            RE.get()
        }
        Err(_) => None,
    }
}

fn strip_recursive_suffix(text: &str) -> Option<String> {
    if let Some(re) = recursive_suffix_re() {
        if re.is_match(text) {
            let stripped = re.replacen(text, 1, "");
            return Some(stripped.into_owned());
        }
        return None;
    }
    text.strip_suffix("/...").map(str::to_owned)
}

fn package_path(target: &str) -> String {
    if let Some(rest) = target.strip_prefix("//") {
        if let Some(colon) = rest.find(':') {
            return rest[..colon].to_owned();
        }
        if let Some(prefix) = strip_recursive_suffix(rest) {
            return prefix;
        }
        return rest.to_owned();
    }
    if let Some(rest) = target.strip_prefix('@') {
        let _ = rest;
        return String::new();
    }
    let mut path = target.to_owned();
    if let Some(re) = dot_slash_prefix_re() {
        if re.is_match(&path) {
            path = re.replacen(&path, 1, "").into_owned();
        }
    } else {
        while let Some(rest) = path.strip_prefix("./") {
            path = rest.to_owned();
        }
    }
    if let Some(re) = trailing_slash_re() {
        if path.len() > 1 && re.is_match(&path) {
            path = re.replacen(&path, 1, "").into_owned();
            if path.is_empty() {
                path = "/".to_owned();
            }
        }
    } else {
        while path.ends_with('/') && path.len() > 1 {
            path.pop();
        }
    }
    if let Some(prefix) = strip_recursive_suffix(&path) {
        return prefix;
    }
    path
}

fn root_npm_re() -> Option<&'static Regex> {
    static RE: OnceLock<Regex> = OnceLock::new();
    if let Some(compiled) = RE.get() {
        return Some(compiled);
    }
    match Regex::new(
        r"(?i)package\.json|pnpm-lock\.yaml|pnpm-workspace\.yaml|\.npmrc|node_modules|package_json",
    ) {
        Ok(compiled) => {
            let _ = RE.set(compiled);
            RE.get()
        }
        Err(_) => None,
    }
}

fn root_cargo_re() -> Option<&'static Regex> {
    static RE: OnceLock<Regex> = OnceLock::new();
    if let Some(compiled) = RE.get() {
        return Some(compiled);
    }
    match Regex::new(r"(?i)cargo-bazel-lock\.json|cargo\.toml|cargo\.lock") {
        Ok(compiled) => {
            let _ = RE.set(compiled);
            RE.get()
        }
        Err(_) => None,
    }
}

fn root_owning_sets(target: &str) -> Vec<SetId> {
    if let (Some(npm), Some(cargo)) = (root_npm_re(), root_cargo_re()) {
        if npm.is_match(target) {
            return vec![SetId::Npm];
        }
        if cargo.is_match(target) {
            return vec![SetId::Cargo];
        }
        return vec![];
    }
    let lower = target.to_ascii_lowercase();
    if lower.contains("package.json")
        || lower.contains("pnpm-lock.yaml")
        || lower.contains("pnpm-workspace.yaml")
        || lower.contains(".npmrc")
        || lower.contains("node_modules")
        || lower.contains("package_json")
    {
        return vec![SetId::Npm];
    }
    if lower.contains("cargo-bazel-lock.json")
        || lower.contains("cargo.toml")
        || lower.contains("cargo.lock")
    {
        return vec![SetId::Cargo];
    }
    vec![]
}

fn has_prefix(package: &str, prefix: &str) -> bool {
    let pattern = format!(r"^{}(?:/|$)", regex::escape(prefix));
    match Regex::new(&pattern) {
        Ok(re) => re.is_match(package),
        Err(_) => package == prefix || package.starts_with(&format!("{prefix}/")),
    }
}

pub fn resolve(selectors: &[String]) -> Result<BTreeMap<SetId, SetRequest>, SelectorError> {
    if selectors.is_empty() {
        return Ok(SetId::ALL
            .iter()
            .map(|set| (*set, SetRequest::Full))
            .collect());
    }
    let mut full: BTreeSet<SetId> = BTreeSet::new();
    let mut packages: BTreeMap<SetId, Vec<String>> = BTreeMap::new();
    for raw in selectors {
        match parse_selector(raw)? {
            Selector::Set(set) => {
                full.insert(set);
            }
            Selector::Package(set, package) => {
                packages.entry(set).or_default().push(package);
            }
            Selector::Target(target) => {
                let owners = owning_sets(&target);
                if owners.is_empty() {
                    return Err(SelectorError::NoOwningSet { target });
                }
                for set in owners {
                    full.insert(set);
                }
            }
        }
    }
    let mut resolved = BTreeMap::new();
    for set in full {
        resolved.insert(set, SetRequest::Full);
    }
    for (set, mut names) in packages {
        if resolved.contains_key(&set) {
            continue;
        }
        names.sort();
        names.dedup();
        resolved.insert(set, SetRequest::Packages(names));
    }
    Ok(resolved)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(words: &[&str]) -> Vec<String> {
        words.iter().map(|word| (*word).to_owned()).collect()
    }

    #[test]
    fn parses_sets_packages_and_targets() {
        assert_eq!(parse_selector("cargo"), Ok(Selector::Set(SetId::Cargo)));
        assert_eq!(
            parse_selector("npm:react"),
            Ok(Selector::Package(SetId::Npm, "react".to_owned()))
        );
        assert_eq!(
            parse_selector("maven:junit:junit"),
            Ok(Selector::Package(SetId::Maven, "junit:junit".to_owned()))
        );
        assert_eq!(
            parse_selector("//rust/tests/fixtures/hello:hello"),
            Ok(Selector::Target(
                "//rust/tests/fixtures/hello:hello".to_owned()
            ))
        );
        assert_eq!(
            parse_selector("rust/tests/fixtures/hello/Cargo.toml"),
            Ok(Selector::Target(
                "rust/tests/fixtures/hello/Cargo.toml".to_owned()
            ))
        );
    }

    #[test]
    fn scoped_npm_and_nuget_identities_validate() {
        assert!(parse_selector("npm:@astrojs/compiler").is_ok());
        assert!(parse_selector("nuget:FSharp.Core").is_ok());
        assert!(parse_selector("cargo:anyhow").is_ok());
        assert!(parse_selector("go:rules_dx/go/tests/fixtures/hello").is_ok());
    }

    #[test]
    fn invalid_identities_fail_closed() {
        for (set, packages) in [
            (
                SetId::Npm,
                vec![
                    "",
                    "a:b",
                    "a b",
                    "@scope",
                    "@/name",
                    "@scope/",
                    "@scope/bad!",
                    "a/b",
                    "bad!",
                ],
            ),
            (SetId::NuGet, vec!["bad!", ""]),
            (SetId::Go, vec!["", "a:b", "a b", "/a", "a/", "a//b", "a!b"]),
        ] {
            for package in packages {
                assert!(
                    matches!(
                        validate_package(set, package),
                        Err(SelectorError::InvalidPackage { .. })
                    ),
                    "{}:{package}",
                    set.name()
                );
            }
        }
        assert!(matches!(
            parse_selector("cargo:"),
            Err(SelectorError::InvalidPackage { .. })
        ));
        assert!(matches!(
            parse_selector("cargo:bad name"),
            Err(SelectorError::InvalidPackage { .. })
        ));
        assert!(matches!(
            parse_selector("npm:"),
            Err(SelectorError::InvalidPackage { .. })
        ));
        assert!(matches!(
            parse_selector("maven:junit"),
            Err(SelectorError::InvalidPackage { .. })
        ));
        assert!(matches!(
            parse_selector("maven::artifact"),
            Err(SelectorError::InvalidPackage { .. })
        ));
        assert!(matches!(
            parse_selector("nuget:"),
            Err(SelectorError::InvalidPackage { .. })
        ));
        assert!(matches!(
            parse_selector("go:"),
            Err(SelectorError::InvalidPackage { .. })
        ));
        assert!(matches!(parse_selector(""), Err(SelectorError::Empty)));
        assert!(matches!(
            parse_selector("crates"),
            Err(SelectorError::UnknownSelector { .. })
        ));
        assert!(matches!(
            parse_selector("cargo-lock"),
            Err(SelectorError::UnknownSelector { .. })
        ));
    }

    #[test]
    fn bazel_labels_are_targets_not_packages() {
        assert!(matches!(
            parse_selector("//foo:bar"),
            Ok(Selector::Target(_))
        ));
        assert!(matches!(parse_selector("//..."), Ok(Selector::Target(_))));
    }

    #[test]
    fn owning_sets_cover_all_families() {
        for (path, expected) in [
            ("examples/adopt-rust/crates", vec![SetId::Cargo]),
            ("examples/adopt-js-ts/app", vec![SetId::NpmAdopt]),
            ("examples/adopt-python/app", vec![SetId::UvAdopt]),
            ("examples/adopt-java/greet", vec![SetId::Maven]),
            ("examples/adopt-kotlin/greet", vec![SetId::Maven]),
            ("examples/adopt-go/greet", vec![SetId::Go]),
            (
                "examples/adopt-polyglot/frontend",
                vec![SetId::NpmAdoptPolyglot, SetId::UvAdoptPolyglot],
            ),
            ("docs/ir/ir", vec![SetId::Cargo]),
        ] {
            assert_eq!(owning_sets(path), expected, "{path}");
        }
        assert_eq!(owning_sets("//..."), SetId::ALL.to_vec());
        assert_eq!(owning_sets("MODULE.bazel"), SetId::ALL.to_vec());
        assert_eq!(
            owning_sets("//rust/tests/fixtures/hello:hello"),
            vec![SetId::Cargo]
        );
        assert_eq!(
            owning_sets("rust/tests/fixtures/hello/Cargo.toml"),
            vec![SetId::Cargo]
        );
        assert_eq!(owning_sets("cli/cli/src/exec.rs"), vec![SetId::Cargo]);
        assert_eq!(
            owning_sets("quality/tools/javascript/package.json"),
            vec![SetId::NpmTools]
        );
        assert_eq!(
            owning_sets("quality/tools/python/pyproject.toml"),
            vec![SetId::UvTools]
        );
        assert_eq!(
            owning_sets("python/tests/fixtures/hello/pyproject.toml"),
            vec![SetId::Uv]
        );
        assert_eq!(
            owning_sets("python/tests/fixtures/hello/hello.py"),
            vec![SetId::Uv]
        );
        assert_eq!(
            owning_sets("//javascript/tests/fixtures/hello:hello"),
            vec![SetId::Npm]
        );
        assert_eq!(owning_sets("package.json"), vec![SetId::Npm]);
        assert_eq!(
            owning_sets("//third_party/jvm:maven_install"),
            vec![SetId::Maven]
        );
        assert_eq!(
            owning_sets("//csharp/tests/fixtures/hello:hello"),
            vec![SetId::NuGet]
        );
        assert_eq!(
            owning_sets("//go/tests/fixtures/hello:hello"),
            vec![SetId::Go]
        );
        assert_eq!(owning_sets("go/tests/fixtures/hello"), vec![SetId::Go]);
        assert!(owning_sets("docs/cli/README.md").is_empty());
        assert!(owning_sets("libs/starlark/defs.bzl").is_empty());
    }

    #[test]
    fn bare_resolves_to_all_sets_full() {
        let resolved = resolve(&[]).expect("bare");
        assert_eq!(resolved.len(), 12);
        for set in SetId::ALL {
            assert_eq!(resolved.get(&set), Some(&SetRequest::Full));
        }
    }

    #[test]
    fn full_wins_over_packages_for_the_same_set() {
        let resolved = resolve(&strings(&["npm", "npm:react"])).expect("full wins");
        assert_eq!(resolved.get(&SetId::Npm), Some(&SetRequest::Full));
    }

    #[test]
    fn package_only_sets_stay_selective_and_sorted() {
        let resolved = resolve(&strings(&["npm:react", "npm:jest"])).expect("packages");
        assert_eq!(
            resolved.get(&SetId::Npm),
            Some(&SetRequest::Packages(vec![
                "jest".to_owned(),
                "react".to_owned()
            ]))
        );
    }

    #[test]
    fn maven_selective_parses_group_artifact_and_stays_selective() {
        assert_eq!(
            parse_selector("maven:org.junit.jupiter:junit-jupiter-api"),
            Ok(Selector::Package(
                SetId::Maven,
                "org.junit.jupiter:junit-jupiter-api".to_owned()
            ))
        );
        let resolved = resolve(&strings(&[
            "maven:junit:junit",
            "maven:org.junit.jupiter:junit-jupiter-api",
        ]))
        .expect("maven packages");
        assert_eq!(
            resolved.get(&SetId::Maven),
            Some(&SetRequest::Packages(vec![
                "junit:junit".to_owned(),
                "org.junit.jupiter:junit-jupiter-api".to_owned()
            ]))
        );
        let resolved = resolve(&strings(&[
            "maven",
            "maven:org.junit.jupiter:junit-jupiter-api",
        ]))
        .expect("maven full wins");
        assert_eq!(resolved.get(&SetId::Maven), Some(&SetRequest::Full));
    }

    #[test]
    fn go_selective_parses_module_path_and_stays_selective() {
        assert_eq!(
            parse_selector("go:github.com/google/go-cmp/cmp"),
            Ok(Selector::Package(
                SetId::Go,
                "github.com/google/go-cmp/cmp".to_owned()
            ))
        );
        let resolved = resolve(&strings(&[
            "go:github.com/google/go-cmp/cmp",
            "go:rules_dx/go/tests/fixtures/hello",
        ]))
        .expect("go packages");
        assert_eq!(
            resolved.get(&SetId::Go),
            Some(&SetRequest::Packages(vec![
                "github.com/google/go-cmp/cmp".to_owned(),
                "rules_dx/go/tests/fixtures/hello".to_owned()
            ]))
        );
        let resolved =
            resolve(&strings(&["go", "go:github.com/google/go-cmp/cmp"])).expect("go full wins");
        assert_eq!(resolved.get(&SetId::Go), Some(&SetRequest::Full));
    }

    #[test]
    fn targets_expand_to_owning_sets_full() {
        let resolved = resolve(&strings(&["//go/tests/fixtures/hello:hello"])).expect("target");
        assert_eq!(resolved.get(&SetId::Go), Some(&SetRequest::Full));
        let resolved = resolve(&strings(&["//..."])).expect("repo");
        assert_eq!(resolved.len(), 12);
    }

    #[test]
    fn unknown_and_unowned_targets_fail_closed() {
        assert!(matches!(
            resolve(&strings(&["crates"])),
            Err(SelectorError::UnknownSelector { .. })
        ));
        assert!(matches!(
            resolve(&strings(&["docs/cli/README.md"])),
            Err(SelectorError::NoOwningSet { .. })
        ));
    }

    #[test]
    fn regex_prefix_never_matches_sibling_names() {
        assert!(has_prefix("go", "go"));
        assert!(has_prefix("go/tests/fixtures/hello", "go"));
        assert!(!has_prefix("gold", "go"));
        assert!(!has_prefix("rusty", "rust"));
        assert!(!has_prefix("quality-tools/x", "quality"));
        assert!(has_prefix("quality/tools/x", "quality"));
    }

    #[test]
    fn regex_package_path_normalizes_dotslash_and_suffixes() {
        assert_eq!(
            package_path("./go/tests/fixtures/hello"),
            "go/tests/fixtures/hello"
        );
        assert_eq!(
            package_path("go/tests/fixtures/hello/"),
            "go/tests/fixtures/hello"
        );
        assert_eq!(
            package_path("//go/tests/fixtures/hello/..."),
            "go/tests/fixtures/hello"
        );
        assert_eq!(package_path("//:target"), "");
        assert_eq!(package_path("@crates//:lock"), "");
    }

    #[test]
    fn regex_root_owning_sets_match_case_insensitively() {
        assert_eq!(owning_sets("PACKAGE.JSON"), vec![SetId::Npm]);
        assert_eq!(owning_sets("Cargo.TOML"), vec![SetId::Cargo]);
        assert!(owning_sets("libs/starlark/defs.bzl").is_empty());
    }
}
