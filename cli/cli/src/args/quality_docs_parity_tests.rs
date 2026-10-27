//! Parity between `docs/cli/commands/quality.md` and the quality registry.
//!
//! The page's "Default policy" and "Wired tools" tables are written by hand.
//! These tests read `quality/adapters.bzl` and `quality/BUILD.bazel` and fail
//! when a table drifts. The parsers are deliberately strict: anything they
//! cannot read is a panic, never a silent skip.

use std::collections::BTreeMap;

fn read(relative: &str) -> String {
    std::fs::read_to_string(dx_testing::runfiles_root().join(relative))
        .unwrap_or_else(|error| panic!("{relative} must ship as test data: {error}"))
}

/// The text from `open` (an opening bracket at `start`) through its match.
fn balanced(text: &str, start: usize) -> &str {
    let bytes = text.as_bytes();
    let (open, close) = match bytes[start] {
        b'{' => (b'{', b'}'),
        b'[' => (b'[', b']'),
        b'(' => (b'(', b')'),
        other => panic!("not an opening bracket: {}", other as char),
    };
    let mut depth = 0usize;
    let mut quoted = false;
    for (offset, byte) in bytes[start..].iter().enumerate() {
        match *byte {
            b'"' => quoted = !quoted,
            b if !quoted && b == open => depth += 1,
            b if !quoted && b == close => {
                depth -= 1;
                if depth == 0 {
                    return &text[start..=start + offset];
                }
            }
            _ => {}
        }
    }
    panic!("unbalanced `{}` in quality source", open as char);
}

/// Parses `NAME = { ... }` from a `.bzl` file as JSON after dropping trailing
/// commas. The literal must stay pure: a comment, call or `+` fails here.
fn bzl_literal(source: &str, name: &str) -> serde_json::Value {
    let marker = format!("\n{name} = {{");
    let at = source
        .find(&marker)
        .unwrap_or_else(|| panic!("quality/adapters.bzl has no `{name} = {{` literal"))
        + marker.len()
        - 1;
    let literal = balanced(source, at);
    let mut json = String::new();
    let chars: Vec<char> = literal.chars().collect();
    for (index, ch) in chars.iter().enumerate() {
        if *ch == ',' {
            let next = chars[index + 1..].iter().find(|c| !c.is_whitespace());
            if matches!(next, Some('}') | Some(']')) {
                continue;
            }
        }
        json.push(*ch);
    }
    serde_json::from_str(&json).unwrap_or_else(|error| {
        panic!("{name} in quality/adapters.bzl must stay a pure literal (no comments, calls or `+`): {error}")
    })
}

fn strings_of(value: &serde_json::Value) -> Vec<String> {
    value
        .as_array()
        .expect("a list")
        .iter()
        .map(|item| item.as_str().expect("a string").to_owned())
        .collect()
}

struct Family {
    family_id: String,
    format: Vec<String>,
    lint: Vec<String>,
    typecheck: Vec<String>,
    disabled: Vec<String>,
}

const FAMILY_KEYS: [&str; 6] = [
    "name",
    "family_id",
    "format",
    "lint",
    "typecheck",
    "disabled",
];

fn quoted(text: &str) -> Vec<String> {
    text.split('"')
        .skip(1)
        .step_by(2)
        .map(str::to_owned)
        .collect()
}

/// Every `quality_family(...)` call in the BUILD file, by target name.
fn families(build: &str) -> BTreeMap<String, Family> {
    let mut out = BTreeMap::new();
    let mut rest = 0;
    while let Some(found) = build[rest..].find("\nquality_family(") {
        let open = rest + found + "\nquality_family".len();
        let call = balanced(build, open);
        rest = open + call.len();
        let mut attrs: BTreeMap<String, String> = BTreeMap::new();
        let inner = &call[1..call.len() - 1];
        let mut cursor = 0;
        while cursor < inner.len() {
            let line_end = inner[cursor..]
                .find('\n')
                .map_or(inner.len(), |n| cursor + n);
            let line = &inner[cursor..line_end];
            let top_level = line.starts_with("    ") && !line.starts_with("     ");
            if top_level && line.contains(" = ") {
                let key = line.trim().split(" = ").next().expect("key").to_owned();
                assert!(
                    FAMILY_KEYS.contains(&key.as_str()),
                    "quality_family has an attribute the docs parity test cannot read: {line}"
                );
                let value_at = cursor + line.find(" = ").expect("equals") + 3;
                let value = if inner[value_at..].starts_with('[') {
                    balanced(inner, value_at).to_owned()
                } else {
                    line[line.find(" = ").expect("equals") + 3..]
                        .trim_end_matches(',')
                        .to_owned()
                };
                cursor = value_at + value.len();
                attrs.insert(key, value);
                continue;
            }
            cursor = line_end + 1;
        }
        let list = |key: &str| attrs.get(key).map(|v| quoted(v)).unwrap_or_default();
        let name = list("name")
            .pop()
            .unwrap_or_else(|| panic!("quality_family without a name: {call}"));
        let family_id = list("family_id")
            .pop()
            .unwrap_or_else(|| panic!("quality_family {name} has no family_id"));
        out.insert(
            name,
            Family {
                family_id,
                format: list("format"),
                lint: list("lint"),
                typecheck: list("typecheck"),
                disabled: list("disabled"),
            },
        );
    }
    assert!(!out.is_empty(), "no quality_family blocks found");
    out
}

fn default_policy_families(build: &str) -> Vec<String> {
    let marker = "name = \"default_workspace_policy\"";
    let at = build.find(marker).expect("default_workspace_policy target");
    let start = build[..at].rfind("workspace_policy(").expect("call start");
    let call = balanced(build, start + "workspace_policy".len());
    let list_at = call.find("families = [").expect("families list") + "families = ".len();
    quoted(balanced(call, list_at))
        .into_iter()
        .map(|label| label.trim_start_matches(':').to_owned())
        .collect()
}

/// The table rows (cells, trimmed) under `### <heading>`.
fn table_rows(page: &str, heading: &str) -> Vec<Vec<String>> {
    let mut in_section = false;
    let mut rows = Vec::new();
    for line in page.lines() {
        if let Some(title) = line.strip_prefix("### ") {
            in_section = title.trim() == heading;
            continue;
        }
        if line.starts_with("## ") {
            in_section = false;
        }
        if in_section && line.starts_with('|') {
            rows.push(
                line.trim()
                    .trim_matches('|')
                    .split('|')
                    .map(|cell| cell.trim().to_owned())
                    .collect(),
            );
        }
    }
    assert!(rows.len() > 2, "quality.md has no `### {heading}` table");
    rows.drain(..2);
    rows
}

fn code(tools: &[String]) -> String {
    if tools.is_empty() {
        "none".to_owned()
    } else {
        tools
            .iter()
            .map(|tool| format!("`{tool}`"))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

#[test]
fn quality_page_wired_tools_table_matches_real_adapters() {
    let source = read("quality/adapters.bzl");
    let adapters = bzl_literal(&source, "REAL_ADAPTERS");
    let class_to_family = bzl_literal(&source, "REAL_CLASS_TO_FAMILY");
    let mut expected: BTreeMap<String, (String, String)> = BTreeMap::new();
    for (tool, verbs) in adapters.as_object().expect("adapter map") {
        let mut names: Vec<String> = verbs.as_object().expect("verbs").keys().cloned().collect();
        names.sort();
        let mut families: Vec<String> = verbs
            .as_object()
            .expect("verbs")
            .values()
            .flat_map(strings_of)
            .map(|class| {
                class_to_family[&class]
                    .as_str()
                    .unwrap_or_else(|| panic!("class {class} has no family"))
                    .to_owned()
            })
            .collect();
        families.sort();
        families.dedup();
        expected.insert(tool.clone(), (names.join(", "), families.join(", ")));
    }
    let page = read("docs/cli/commands/quality.md");
    let rows = table_rows(&page, "Wired tools");
    let mut documented: BTreeMap<String, (String, String)> = BTreeMap::new();
    let mut order = Vec::new();
    for row in rows {
        assert_eq!(row.len(), 3, "Wired tools row needs 3 cells: {row:?}");
        let tool = row[0].trim_matches('`').to_owned();
        order.push(tool.clone());
        assert!(
            documented
                .insert(tool.clone(), (row[1].clone(), row[2].clone()))
                .is_none(),
            "Wired tools lists `{tool}` twice"
        );
    }
    let mut sorted = order.clone();
    sorted.sort();
    assert_eq!(order, sorted, "Wired tools rows must be sorted by tool ID");
    assert_eq!(
        documented, expected,
        "quality.md Wired tools must match REAL_ADAPTERS and REAL_CLASS_TO_FAMILY exactly"
    );
}

#[test]
fn quality_page_default_policy_table_matches_default_workspace_policy() {
    let build = read("quality/BUILD.bazel");
    let all = families(&build);
    let mut expected: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let listed = default_policy_families(&build);
    assert!(
        !listed.is_empty(),
        "default_workspace_policy lists no family"
    );
    for name in listed {
        let family = all
            .get(&name)
            .unwrap_or_else(|| panic!("default_workspace_policy names unknown family {name}"));
        let cell = |capability: &str, tools: &[String]| {
            if family.disabled.iter().any(|d| d == capability) {
                "disabled".to_owned()
            } else {
                code(tools)
            }
        };
        expected.insert(
            family.family_id.clone(),
            vec![
                cell("format", &family.format),
                cell("lint", &family.lint),
                cell("typecheck", &family.typecheck),
            ],
        );
    }
    let page = read("docs/cli/commands/quality.md");
    let mut documented: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for row in table_rows(&page, "Default policy") {
        assert_eq!(row.len(), 4, "Default policy row needs 4 cells: {row:?}");
        let family = row[0].trim_matches('`').to_owned();
        assert!(
            documented
                .insert(family.clone(), row[1..].to_vec())
                .is_none(),
            "Default policy lists `{family}` twice"
        );
    }
    assert_eq!(
        documented, expected,
        "quality.md Default policy must match default_workspace_policy exactly"
    );
}

#[test]
fn quality_page_baselines_holds_only_baseline_content() {
    let page = read("docs/cli/commands/quality.md");
    let mut in_baselines = false;
    for line in page.lines() {
        if line.starts_with("## ") {
            in_baselines = line.trim() == "## Baselines";
            continue;
        }
        assert!(
            !(in_baselines && line.starts_with("### ")),
            "quality.md `## Baselines` holds a tool section: {line}"
        );
    }
}
