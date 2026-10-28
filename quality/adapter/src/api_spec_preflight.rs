//! Offline checks dx runs on OpenAPI and AsyncAPI documents before Redocly sees them.
//!
//! Redocly follows `$ref` over the network and lets a config turn `no-unresolved-refs`
//! off, so dx owns the version gate, the remote-ref ban and the missing-ref check.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use saphyr_parser::{Event, Parser};

/// A document that does not parse as YAML or JSON.
pub const RULE_SYNTAX: &str = "dx/api-spec-syntax";
/// A root without a supported `openapi` or `asyncapi` version key.
pub const RULE_ROOT: &str = "dx/api-spec-root";
/// A `$ref` that names a URL instead of a staged file.
pub const RULE_REMOTE_REF: &str = "dx/api-spec-remote-ref";
/// A `$ref` that names no staged root or fragment.
pub const RULE_UNRESOLVED_REF: &str = "dx/api-spec-unresolved-ref";

/// The Redocly rulesets a config may extend; anything else loads from disk or the network.
pub const BUILTIN_RULESETS: &[&str] = &[
    "all",
    "minimal",
    "recommended",
    "recommended-strict",
    "spec",
];

/// Config keys that load code or send credentials, so dx refuses them.
const FORBIDDEN_CONFIG_KEYS: &[&str] = &["plugins", "resolve"];

/// One problem the preflight found, placed on the root it was checking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    pub rule: &'static str,
    pub line: u64,
    pub column: u64,
    pub message: String,
}

#[derive(Debug, Default)]
struct Scanned {
    root_keys: Vec<(String, Option<String>)>,
    refs: Vec<(String, u64, u64)>,
}

enum Frame {
    Map {
        expecting_key: bool,
        pending: Option<String>,
    },
    Seq {
        extends: bool,
    },
}

/// The key a mapping value belongs to, consumed as the value arrives.
fn take_key(stack: &mut [Frame]) -> Option<String> {
    match stack.last_mut() {
        Some(Frame::Map {
            expecting_key,
            pending,
        }) => {
            if *expecting_key {
                *expecting_key = false;
                *pending = Some(String::new());
                None
            } else {
                *expecting_key = true;
                pending.take()
            }
        }
        _ => None,
    }
}

/// Records the scalar key whose value comes next.
fn set_key(stack: &mut [Frame], key: String) {
    if let Some(Frame::Map {
        expecting_key,
        pending,
    }) = stack.last_mut()
    {
        *expecting_key = false;
        *pending = Some(key);
    }
}

fn wants_key(stack: &[Frame]) -> bool {
    matches!(
        stack.last(),
        Some(Frame::Map {
            expecting_key: true,
            ..
        })
    )
}

fn syntax(line: usize, col: usize, detail: &str) -> Problem {
    Problem {
        rule: RULE_SYNTAX,
        line: line.max(1) as u64,
        column: col as u64 + 1,
        message: format!("not valid YAML or JSON: {detail}"),
    }
}

/// The root keys and every scalar `$ref` of one document, or the syntax problem that stopped it.
fn scan(text: &str) -> Result<Scanned, Problem> {
    let mut out = Scanned::default();
    let mut stack: Vec<Frame> = Vec::new();
    let mut documents = 0usize;
    for item in Parser::new_from_str(text) {
        let (event, span) =
            item.map_err(|err| syntax(err.marker().line(), err.marker().col(), err.info()))?;
        match event {
            Event::DocumentStart(_) => {
                documents += 1;
                if documents > 1 {
                    return Err(syntax(
                        span.start.line(),
                        span.start.col(),
                        "more than one document in the stream",
                    ));
                }
            }
            Event::Scalar(value, ..) => {
                if wants_key(&stack) {
                    set_key(&mut stack, value.to_string());
                } else if let Some(key) = take_key(&mut stack) {
                    if key == "$ref" {
                        out.refs.push((
                            value.to_string(),
                            span.start.line() as u64,
                            span.start.col() as u64 + 1,
                        ));
                    }
                    if stack.len() == 1 {
                        out.root_keys.push((key, Some(value.to_string())));
                    }
                }
            }
            Event::Alias(_) => {
                if let Some(key) = take_key(&mut stack) {
                    if stack.len() == 1 {
                        out.root_keys.push((key, None));
                    }
                }
            }
            Event::MappingStart(..) | Event::SequenceStart(..) => {
                if let Some(key) = take_key(&mut stack) {
                    if stack.len() == 1 {
                        out.root_keys.push((key, None));
                    }
                }
                stack.push(if matches!(event, Event::MappingStart(..)) {
                    Frame::Map {
                        expecting_key: true,
                        pending: None,
                    }
                } else {
                    Frame::Seq { extends: false }
                });
            }
            Event::MappingEnd | Event::SequenceEnd => {
                stack.pop();
            }
            _ => {}
        }
    }
    Ok(out)
}

fn version_ok(value: &str, majors: &[&str]) -> bool {
    majors.iter().any(|major| {
        value == *major
            || value.strip_prefix(major).is_some_and(|rest| {
                rest.strip_prefix('.').is_some_and(|patch| {
                    !patch.is_empty() && patch.bytes().all(|b| b.is_ascii_digit())
                })
            })
    })
}

/// The problem with a root's version key, if it has one.
fn root_problem(keys: &[(String, Option<String>)]) -> Option<String> {
    let find = |name: &str| keys.iter().find(|(key, _)| key == name).map(|(_, v)| v);
    if find("swagger").is_some() {
        return Some(
            "Swagger 2.0 is not supported: convert the document to OpenAPI 3.0, 3.1 or 3.2"
                .to_owned(),
        );
    }
    match (find("openapi"), find("asyncapi")) {
        (Some(_), Some(_)) => Some("a root names both openapi and asyncapi".to_owned()),
        (Some(Some(value)), None) if version_ok(value, &["3.0", "3.1", "3.2"]) => None,
        (Some(value), None) => Some(format!(
            "unsupported openapi version {}: want 3.0, 3.1 or 3.2",
            value.as_deref().unwrap_or("(not a scalar)")
        )),
        (None, Some(Some(value))) if version_ok(value, &["2.6", "3.0"]) => None,
        (None, Some(value)) => Some(format!(
            "unsupported asyncapi version {}: want 2.6 or 3.0",
            value.as_deref().unwrap_or("(not a scalar)")
        )),
        (None, None) => Some(
            "an api_spec root needs an openapi (3.0, 3.1, 3.2) or asyncapi (2.6, 3.0) root key"
                .to_owned(),
        ),
    }
}

/// The problem with a root that is not an OpenAPI 3.0, 3.1 or 3.2 document, for code generation.
pub fn openapi_root_problem(text: &str) -> Option<Problem> {
    let message = match scan(text) {
        Err(problem) => return Some(problem),
        Ok(scanned) => {
            let is_openapi = scanned.root_keys.iter().any(|(key, _)| key == "openapi");
            match root_problem(&scanned.root_keys) {
                Some(message) => message,
                None if is_openapi => return None,
                None => {
                    "code generation needs an OpenAPI root; AsyncAPI is not supported".to_owned()
                }
            }
        }
    };
    Some(Problem {
        rule: RULE_ROOT,
        line: 1,
        column: 1,
        message,
    })
}

/// Whether a `$ref` names a URL (`scheme:` or `//host`) rather than a relative file.
pub fn is_remote(reference: &str) -> bool {
    if reference.starts_with("//") || reference.starts_with("\\\\") {
        return true;
    }
    let Some((scheme, _)) = reference.split_once(':') else {
        return false;
    };
    let mut chars = scheme.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphabetic())
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
        && !scheme.contains('/')
        && !scheme.contains('#')
}

/// The staged workspace path a relative `$ref` in `from` names, or why it names none.
pub fn ref_target(from: &str, reference: &str) -> Result<Option<String>, String> {
    let path = reference.split('#').next().unwrap_or_default();
    if path.is_empty() {
        return Ok(None);
    }
    if path.starts_with('/') || path.contains('\\') {
        return Err(format!(
            "$ref {reference} is not a relative path with forward slashes"
        ));
    }
    let mut parts: Vec<&str> = from.split('/').collect();
    parts.pop();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                if parts.pop().is_none() {
                    return Err(format!("$ref {reference} escapes the workspace"));
                }
            }
            other => parts.push(other),
        }
    }
    Ok(Some(parts.join("/")))
}

/// The preflight problems of one root and every file its `$ref` graph reaches.
///
/// `docs` maps each staged workspace path (roots, refs and dep files) to its text.
/// A problem in a fragment is placed on the root at line 1 and names the fragment.
pub fn check_root(root: &str, docs: &BTreeMap<String, String>) -> Vec<Problem> {
    let mut problems = Vec::new();
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut queue: VecDeque<String> = VecDeque::from([root.to_owned()]);
    seen.insert(root.to_owned());
    while let Some(file) = queue.pop_front() {
        let is_root = file == root;
        let place = |problem: Problem| -> Problem {
            if is_root {
                problem
            } else {
                Problem {
                    rule: problem.rule,
                    line: 1,
                    column: 1,
                    message: format!(
                        "{file}:{}:{}: {}",
                        problem.line, problem.column, problem.message
                    ),
                }
            }
        };
        let Some(text) = docs.get(&file) else {
            continue;
        };
        let scanned = match scan(text) {
            Ok(scanned) => scanned,
            Err(problem) => {
                problems.push(place(problem));
                continue;
            }
        };
        if is_root {
            if let Some(message) = root_problem(&scanned.root_keys) {
                problems.push(Problem {
                    rule: RULE_ROOT,
                    line: 1,
                    column: 1,
                    message,
                });
            }
        }
        for (reference, line, column) in scanned.refs {
            if is_remote(&reference) {
                problems.push(place(Problem {
                    rule: RULE_REMOTE_REF,
                    line,
                    column,
                    message: format!(
                        "$ref {reference} names a remote document; dx lints API specs offline, so vendor it and list it in refs"
                    ),
                }));
                continue;
            }
            match ref_target(&file, &reference) {
                Ok(None) => {}
                Ok(Some(target)) if docs.contains_key(&target) => {
                    if seen.insert(target.clone()) {
                        queue.push_back(target);
                    }
                }
                Ok(Some(target)) => problems.push(place(Problem {
                    rule: RULE_UNRESOLVED_REF,
                    line,
                    column,
                    message: format!(
                        "$ref {reference} names {target}, which is not a staged api_spec source, ref or dep"
                    ),
                })),
                Err(message) => problems.push(place(Problem {
                    rule: RULE_UNRESOLVED_REF,
                    line,
                    column,
                    message,
                })),
            }
        }
    }
    problems
}

/// Rejects a Redocly config that loads plugins, sends resolve headers, or extends a non-builtin ruleset.
pub fn check_config(text: &str) -> Result<(), String> {
    let mut stack: Vec<Frame> = Vec::new();
    let mut documents = 0usize;
    for item in Parser::new_from_str(text) {
        let (event, _) = item.map_err(|err| format!("redocly config is not valid YAML: {err}"))?;
        let in_extends = matches!(stack.last(), Some(Frame::Seq { extends: true }));
        match event {
            Event::DocumentStart(_) => {
                documents += 1;
                if documents > 1 {
                    return Err("redocly config holds more than one YAML document".to_owned());
                }
            }
            Event::Scalar(value, ..) => {
                if wants_key(&stack) {
                    if FORBIDDEN_CONFIG_KEYS.contains(&value.as_ref()) {
                        return Err(format!(
                            "redocly config key `{value}` is not allowed: dx runs Redocly offline, without plugins or resolver headers"
                        ));
                    }
                    set_key(&mut stack, value.to_string());
                } else if in_extends || take_key(&mut stack).as_deref() == Some("extends") {
                    builtin(&value)?;
                }
            }
            Event::Alias(_) => {
                if in_extends || take_key(&mut stack).as_deref() == Some("extends") {
                    return Err(EXTENDS_BY_NAME.to_owned());
                }
            }
            Event::MappingStart(..) | Event::SequenceStart(..) => {
                if in_extends {
                    return Err(EXTENDS_BY_NAME.to_owned());
                }
                let extends = take_key(&mut stack).as_deref() == Some("extends");
                if matches!(event, Event::MappingStart(..)) {
                    if extends {
                        return Err(EXTENDS_BY_NAME.to_owned());
                    }
                    stack.push(Frame::Map {
                        expecting_key: true,
                        pending: None,
                    });
                } else {
                    stack.push(Frame::Seq { extends });
                }
            }
            Event::MappingEnd | Event::SequenceEnd => {
                stack.pop();
            }
            _ => {}
        }
    }
    Ok(())
}

const EXTENDS_BY_NAME: &str = "redocly config extends must list builtin rulesets by name";

fn builtin(name: &str) -> Result<(), String> {
    if BUILTIN_RULESETS.contains(&name) {
        Ok(())
    } else {
        Err(format!(
            "redocly config extends {name}, which is not a builtin ruleset: want one of {}",
            BUILTIN_RULESETS.join(", ")
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn docs(files: &[(&str, &str)]) -> BTreeMap<String, String> {
        files
            .iter()
            .map(|(path, text)| ((*path).to_owned(), (*text).to_owned()))
            .collect()
    }

    fn rules(problems: &[Problem]) -> Vec<&'static str> {
        problems.iter().map(|problem| problem.rule).collect()
    }

    #[test]
    fn openapi_root_problem_accepts_only_openapi() {
        assert_eq!(openapi_root_problem("openapi: 3.1.0\n"), None);
        for text in [
            "asyncapi: 3.0.0\n",
            "swagger: '2.0'\n",
            "info: {}\n",
            "openapi: 2.0.0\n",
        ] {
            let problem = openapi_root_problem(text);
            assert_eq!(problem.map(|p| p.rule), Some(RULE_ROOT), "{text}");
        }
        let syntax = openapi_root_problem("openapi: [\n");
        assert_eq!(syntax.map(|p| p.rule), Some(RULE_SYNTAX));
    }

    #[test]
    fn api_spec_supported_roots_pass() {
        for text in [
            "openapi: 3.0.3\ninfo: {}\n",
            "openapi: 3.1.0\n",
            "openapi: '3.2.0'\n",
            "{\"openapi\": \"3.1.1\", \"paths\": {}}",
            "asyncapi: 2.6.0\n",
            "asyncapi: 3.0.0\n",
        ] {
            let problems = check_root("api/root.yaml", &docs(&[("api/root.yaml", text)]));
            assert!(problems.is_empty(), "{text}: {problems:?}");
        }
    }

    #[test]
    fn api_spec_rejects_swagger_and_unknown_roots() {
        for (text, wanted) in [
            ("swagger: '2.0'\n", "Swagger 2.0"),
            ("openapi: 2.0.0\n", "unsupported openapi version 2.0.0"),
            ("openapi: 3.1.x\n", "unsupported openapi version"),
            ("openapi: {major: 3}\n", "(not a scalar)"),
            ("asyncapi: 2.5.0\n", "unsupported asyncapi version"),
            ("openapi: 3.1.0\nasyncapi: 3.0.0\n", "both"),
            ("info: {}\n", "needs an openapi"),
            ("", "needs an openapi"),
        ] {
            let problems = check_root("r.yaml", &docs(&[("r.yaml", text)]));
            assert_eq!(rules(&problems), [RULE_ROOT], "{text}");
            assert!(problems[0].message.contains(wanted), "{problems:?}");
        }
    }

    #[test]
    fn api_spec_nested_version_keys_do_not_count() {
        let problems = check_root("r.yaml", &docs(&[("r.yaml", "info:\n  openapi: 3.1.0\n")]));
        assert_eq!(rules(&problems), [RULE_ROOT]);
    }

    #[test]
    fn api_spec_syntax_errors_and_streams_fail() {
        let problems = check_root("r.yaml", &docs(&[("r.yaml", "openapi: [3.1\n")]));
        assert_eq!(rules(&problems), [RULE_SYNTAX]);
        let problems = check_root(
            "r.yaml",
            &docs(&[("r.yaml", "openapi: 3.1.0\n---\nx: 1\n")]),
        );
        assert_eq!(rules(&problems), [RULE_SYNTAX]);
        assert_eq!(problems[0].line, 2);
    }

    #[test]
    fn api_spec_follows_refs_through_fragments() {
        let files = docs(&[
            (
                "api/root.yaml",
                "openapi: 3.1.0\npaths:\n  /pets:\n    $ref: paths/pets.yaml\n",
            ),
            (
                "api/paths/pets.yaml",
                "get:\n  responses:\n    '200':\n      $ref: '../schemas/pet.yaml#/Pet'\n",
            ),
            (
                "api/schemas/pet.yaml",
                "Pet:\n  $ref: '#/Other'\nOther: {}\n",
            ),
        ]);
        assert!(check_root("api/root.yaml", &files).is_empty());
    }

    #[test]
    fn api_spec_reports_fragment_problems_on_the_root() {
        let files = docs(&[
            ("api/root.yaml", "openapi: 3.1.0\nx:\n  $ref: frag.yaml\n"),
            ("api/frag.yaml", "a:\n  b:\n    $ref: missing.yaml\n"),
        ]);
        let problems = check_root("api/root.yaml", &files);
        assert_eq!(rules(&problems), [RULE_UNRESOLVED_REF]);
        assert_eq!((problems[0].line, problems[0].column), (1, 1));
        assert!(
            problems[0].message.starts_with("api/frag.yaml:3:"),
            "{problems:?}"
        );
        assert!(problems[0].message.contains("api/missing.yaml"));
    }

    #[test]
    fn api_spec_rejects_remote_escaping_and_absolute_refs() {
        for reference in [
            "https://example.com/pet.yaml",
            "//example.com/pet.yaml",
            "file:///etc/passwd",
        ] {
            let text = format!("openapi: 3.1.0\nx:\n  $ref: '{reference}'\n");
            let problems = check_root("r.yaml", &docs(&[("r.yaml", &text)]));
            assert_eq!(rules(&problems), [RULE_REMOTE_REF], "{reference}");
            assert_eq!(problems[0].line, 3);
        }
        for reference in ["../../etc/passwd", "/etc/passwd", "a\\\\b.yaml"] {
            let text = format!("openapi: 3.1.0\nx:\n  $ref: \"{reference}\"\n");
            let problems = check_root("api/r.yaml", &docs(&[("api/r.yaml", &text)]));
            assert_eq!(rules(&problems), [RULE_UNRESOLVED_REF], "{reference}");
        }
    }

    #[test]
    fn api_spec_ref_cycles_terminate() {
        let files = docs(&[
            ("r.yaml", "openapi: 3.1.0\nx:\n  $ref: a.yaml\n"),
            ("a.yaml", "y:\n  $ref: r.yaml\n"),
        ]);
        assert!(check_root("r.yaml", &files).is_empty());
    }

    #[test]
    fn api_spec_remote_detection() {
        assert!(is_remote("http://x/y"));
        assert!(is_remote("C:/x"));
        assert!(!is_remote("schemas/pet.yaml#/Pet"));
        assert!(!is_remote("#/components/schemas/Pet"));
        assert!(!is_remote("dir/a:b.yaml"));
        assert_eq!(
            ref_target("api/r.yaml", "./s/../p.yaml#/x"),
            Ok(Some("api/p.yaml".to_owned()))
        );
        assert_eq!(ref_target("api/r.yaml", "#/x"), Ok(None));
    }

    #[test]
    fn api_spec_config_accepts_builtin_extends() {
        let curated = "extends:\n  - spec\nrules:\n  no-unresolved-refs: error\ntelemetry: off\n";
        assert_eq!(check_config(curated), Ok(()));
        assert_eq!(check_config("extends: [recommended, minimal]\n"), Ok(()));
        assert_eq!(check_config("extends: recommended-strict\n"), Ok(()));
        assert_eq!(check_config(""), Ok(()));
    }

    #[test]
    fn api_spec_config_rejects_plugins_resolve_and_foreign_extends() {
        for (text, wanted) in [
            ("plugins:\n  - ./p.js\n", "`plugins`"),
            ("resolve:\n  http:\n    headers: []\n", "`resolve`"),
            ("apis:\n  main:\n    plugins: [x]\n", "`plugins`"),
            ("extends:\n  - ./local.yaml\n", "not a builtin ruleset"),
            ("extends:\n  - https://x/y.yaml\n", "not a builtin ruleset"),
            (
                "apis:\n  a:\n    extends: [p/all]\n",
                "not a builtin ruleset",
            ),
            ("extends:\n  - {name: x}\n", EXTENDS_BY_NAME),
            ("extends: {a: b}\n", EXTENDS_BY_NAME),
            ("extends: [spec\n", "not valid YAML"),
            ("a: 1\n---\nb: 2\n", "more than one"),
        ] {
            let err = check_config(text).expect_err(text);
            assert!(err.contains(wanted), "{text}: {err}");
        }
    }
}
