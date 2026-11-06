//! Finds `deps` entries in BUILD file text and the bytes that remove them.

use std::collections::BTreeMap;

/// Byte range of the quoted entry of `deps` in the rule named `target` that names `dependency`.
pub fn locate(text: &str, target: &str, dependency: &str) -> Option<(usize, usize)> {
    let rule = text.find(&format!("name = \"{target}\""))?;
    let rule_end = text[rule..].find("\n)").map_or(text.len(), |at| rule + at);
    let mut from = rule;
    while let Some(at) = text[from..rule_end].find("deps = [") {
        let key = from + at;
        from = key + "deps = [".len();
        let before = text[..key].chars().next_back();
        if before.is_some_and(|c| c.is_alphanumeric() || c == '_') {
            continue;
        }
        let list_end = text[from..rule_end]
            .find(']')
            .map_or(rule_end, |at| from + at);
        return literals(&text[from..list_end], from)
            .into_iter()
            .find(|(start, end)| target_name(&text[start + 1..end - 1]) == dependency);
    }
    None
}

/// Whether the rule named `target` has `"dx-keep"` in its `tags`, which exempts it from dep checks.
pub fn has_keep_tag(text: &str, target: &str) -> bool {
    let Some(rule) = text.find(&format!("name = \"{target}\"")) else {
        return false;
    };
    let rule_end = text[rule..].find("\n)").map_or(text.len(), |at| rule + at);
    let mut from = rule;
    while let Some(at) = text[from..rule_end].find("tags = [") {
        let key = from + at;
        from = key + "tags = [".len();
        let before = text[..key].chars().next_back();
        if before.is_some_and(|c| c.is_alphanumeric() || c == '_') {
            continue;
        }
        let list_end = text[from..rule_end]
            .find(']')
            .map_or(rule_end, |at| from + at);
        return literals(&text[from..list_end], from)
            .into_iter()
            .any(|(start, end)| &text[start..end] == "\"dx-keep\"");
    }
    false
}

/// The quoted strings of a list body, as byte ranges including the quotes.
fn literals(body: &str, offset: usize) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut open: Option<usize> = None;
    let mut comment = false;
    for (at, c) in body.char_indices() {
        match (open, c) {
            (Some(start), '"') => {
                out.push((offset + start, offset + at + 1));
                open = None;
            }
            (Some(_), _) => {}
            (None, '\n') => comment = false,
            (None, '#') => comment = true,
            (None, '"') if !comment => open = Some(at),
            _ => {}
        }
    }
    out
}

fn target_name(label: &str) -> &str {
    match label.rsplit_once(':') {
        Some((_, name)) => name,
        None => label
            .rsplit('/')
            .next()
            .unwrap_or(label)
            .trim_start_matches('@'),
    }
}

/// The bytes to delete: the whole line when the entry is alone on it, else the entry and its comma.
pub fn removal(text: &str, start: usize, end: usize) -> (usize, usize) {
    let mut cut_end = end;
    if text[cut_end..].starts_with(',') {
        cut_end += 1;
    }
    let line_start = text[..start].rfind('\n').map_or(0, |at| at + 1);
    let line_end = text[cut_end..]
        .find('\n')
        .map_or(text.len(), |at| cut_end + at + 1);
    let alone =
        text[line_start..start].trim().is_empty() && text[cut_end..line_end].trim().is_empty();
    if alone {
        return (line_start, line_end);
    }
    let spaces = text[cut_end..].chars().take_while(|c| *c == ' ').count();
    (start, cut_end + spaces)
}

/// Byte range of the `deps = [` key of the rule named `target`, else of its `name =` line.
pub fn locate_deps_list(text: &str, target: &str) -> Option<(usize, usize)> {
    let rule = text.find(&format!("name = \"{target}\""))?;
    let (key, _) = deps_list(text, rule)?;
    if let Some(key) = key {
        return Some((key, key + "deps = [".len()));
    }
    let line_start = text[..rule].rfind('\n').map_or(0, |at| at + 1);
    let line_end = text[rule..].find('\n').map_or(text.len(), |at| rule + at);
    Some((line_start, line_end))
}

/// The start of the `deps = [` key and the offset of its closing bracket.
fn deps_list(text: &str, rule: usize) -> Option<(Option<usize>, usize)> {
    let rule_end = text[rule..].find("\n)").map_or(text.len(), |at| rule + at);
    let mut from = rule;
    while let Some(at) = text[from..rule_end].find("deps = [") {
        let key = from + at;
        from = key + "deps = [".len();
        let before = text[..key].chars().next_back();
        if before.is_some_and(|c| c.is_alphanumeric() || c == '_') {
            continue;
        }
        let close = text[from..rule_end]
            .find(']')
            .map_or(rule_end, |at| from + at);
        return Some((Some(key), close));
    }
    Some((None, rule_end))
}

/// The offset and text that add `label` as the last entry of the target's `deps`.
/// None when the target has no `deps` list or already lists the label.
pub fn insertion(text: &str, target: &str, label: &str) -> Option<(usize, String)> {
    let rule = text.find(&format!("name = \"{target}\""))?;
    let (key, close) = deps_list(text, rule)?;
    let key = key?;
    let open = key + "deps = [".len();
    let entries = literals(&text[open..close], open);
    if entries
        .iter()
        .any(|(start, end)| &text[start + 1..end - 1] == label)
    {
        return None;
    }
    if !text[open..close].contains('\n') {
        return Some(match entries.last() {
            Some((_, end)) => (*end, format!(", \"{label}\"")),
            None => (open, format!("\"{label}\"")),
        });
    }
    let indent_of = |at: usize| -> String {
        let line = text[..at].rfind('\n').map_or(0, |n| n + 1);
        text[line..at]
            .chars()
            .take_while(|c| *c == ' ' || *c == '\t')
            .collect()
    };
    let Some((start, end)) = entries.last() else {
        let line_end = text[open..].find('\n').map_or(close, |at| open + at);
        return Some((line_end, format!("\n{}    \"{label}\",", indent_of(close))));
    };
    let indent = indent_of(*start);
    let tail = &text[*end..close];
    if tail.trim_start().starts_with(',') {
        let line_end = tail.find('\n').map_or(close, |at| end + at);
        Some((line_end, format!("\n{indent}\"{label}\",")))
    } else {
        Some((*end, format!(",\n{indent}\"{label}\",")))
    }
}

/// The repo of a canonical external label such as `@@maven//:guava`. Main-repo labels give None.
pub fn canonical_repo(label: &str) -> Option<&str> {
    let rest = label.strip_prefix("@@")?;
    let (repo, _) = rest.split_once("//")?;
    (!repo.is_empty()).then_some(repo)
}

/// The label as written in a BUILD file of `package`.
/// A canonical external label is written by its apparent name, so it needs `apparent`, a map
/// from canonical to apparent repo name; without a name for the repo the result is None.
pub fn short_label(
    package: &str,
    label: &str,
    apparent: Option<&BTreeMap<String, String>>,
) -> Option<String> {
    if let Some(repo) = canonical_repo(label) {
        let name = apparent?.get(repo)?;
        let (_, rest) = label.split_once("//")?;
        return Some(format!("@{name}//{rest}"));
    }
    let rest = label
        .strip_prefix("@@")
        .or_else(|| label.strip_prefix('@'))
        .unwrap_or(label);
    let rest = rest.strip_prefix("//")?;
    let (pkg, name) = match rest.split_once(':') {
        Some(parts) => parts,
        None => (rest, rest.rsplit('/').next().unwrap_or(rest)),
    };
    if pkg == package {
        Some(format!(":{name}"))
    } else {
        Some(format!("//{pkg}:{name}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BUILD: &str = concat!(
        "rust_library(\n",
        "    name = \"lib\",\n",
        "    srcs = [\"lib.rs\"],\n",
        "    deps = [\n",
        "        \"//util:used\",\n",
        "        \"@crates//:serde\",\n",
        "    ],\n",
        ")\n",
    );

    fn cut(text: &str, target: &str, dependency: &str) -> Option<String> {
        let (start, end) = locate(text, target, dependency)?;
        let (from, to) = removal(text, start, end);
        let mut fixed = text.to_owned();
        fixed.replace_range(from..to, "");
        Some(fixed)
    }

    #[test]
    fn reads_the_dx_keep_tag_of_one_rule() {
        let text = "lib(\n    name = \"a\",\n    tags = [\"x\", \"dx-keep\"],\n)\nlib(\n    name = \"b\",\n    tags = [\"x\"],\n)\n";
        assert!(has_keep_tag(text, "a"));
        assert!(!has_keep_tag(text, "b"));
        assert!(!has_keep_tag(text, "c"));
    }

    #[test]
    fn finds_the_entry_and_removes_its_line() {
        let (start, end) = locate(BUILD, "lib", "serde").expect("located");
        assert_eq!(&BUILD[start..end], "\"@crates//:serde\"");
        let fixed = cut(BUILD, "lib", "serde").expect("cut");
        assert!(!fixed.contains("serde"));
        assert!(fixed.contains("\"//util:used\",\n    ],"));
    }

    #[test]
    fn removes_an_inline_entry_with_its_comma() {
        let build = "rust_library(name = \"lib\", deps = [\"//a:a\", \"//b:b\"])\n";
        assert_eq!(
            cut(build, "lib", "a").expect("cut"),
            "rust_library(name = \"lib\", deps = [\"//b:b\"])\n"
        );
    }

    #[test]
    fn leaves_proc_macro_deps_alone() {
        let build = "rust_library(name = \"lib\", proc_macro_deps = [\"//a:a\"])\n";
        assert!(locate(build, "lib", "a").is_none());
    }

    #[test]
    fn matches_a_full_label_and_not_a_short_one() {
        let build = concat!(
            "java_library(\n",
            "    name = \"user\",\n",
            "    deps = [\n",
            "        \":kept_helper\",\n",
            "        \"//quality/testdata/jvm_owned:dropped_helper\",\n",
            "    ],\n",
            ")\n",
        );
        let (start, end) = locate(build, "user", "dropped_helper").expect("located");
        assert_eq!(
            &build[start..end],
            "\"//quality/testdata/jvm_owned:dropped_helper\""
        );
        let fixed = cut(build, "user", "dropped_helper").expect("cut");
        assert!(!fixed.contains("dropped_helper"));
        assert!(fixed.contains(":kept_helper"));
    }

    fn insert(text: &str, target: &str, label: &str) -> Option<String> {
        let (at, add) = insertion(text, target, label)?;
        let mut fixed = text.to_owned();
        fixed.insert_str(at, &add);
        Some(fixed)
    }

    #[test]
    fn inserts_into_a_multi_line_list() {
        let fixed = insert(BUILD, "lib", "//q:needed").expect("inserted");
        assert!(fixed.contains("\"@crates//:serde\",\n        \"//q:needed\",\n    ],"));
    }

    #[test]
    fn inserts_into_an_inline_list() {
        let build = "rust_library(name = \"lib\", deps = [\"//a:a\"])\n";
        assert_eq!(
            insert(build, "lib", "//b:b").expect("inserted"),
            "rust_library(name = \"lib\", deps = [\"//a:a\", \"//b:b\"])\n"
        );
        let empty = "rust_library(name = \"lib\", deps = [])\n";
        assert_eq!(
            insert(empty, "lib", "//b:b").expect("inserted"),
            "rust_library(name = \"lib\", deps = [\"//b:b\"])\n"
        );
    }

    #[test]
    fn does_not_insert_without_deps_or_a_second_time() {
        let build = "rust_library(name = \"lib\")\n";
        assert!(insert(build, "lib", "//b:b").is_none());
        assert!(insert(BUILD, "lib", "//util:used").is_none());
    }

    #[test]
    fn locates_the_deps_key_else_the_name_line() {
        let (start, end) = locate_deps_list(BUILD, "lib").expect("located");
        assert_eq!(&BUILD[start..end], "deps = [");
        let build = "rust_library(\n    name = \"lib\",\n)\n";
        let (start, end) = locate_deps_list(build, "lib").expect("located");
        assert_eq!(&build[start..end], "    name = \"lib\",");
    }

    #[test]
    fn shortens_main_repo_labels() {
        assert_eq!(short_label("p", "//p:a", None).as_deref(), Some(":a"));
        assert_eq!(short_label("p", "@//p:a", None).as_deref(), Some(":a"));
        assert_eq!(short_label("p", "@@//q:a", None).as_deref(), Some("//q:a"));
        assert_eq!(short_label("p", "@@x//:y", None), None);
        assert_eq!(short_label("p", "@x//:y", None), None);
        let apparent = BTreeMap::from([("+maven+maven".to_owned(), "maven".to_owned())]);
        assert_eq!(
            short_label("p", "@@+maven+maven//:guava", Some(&apparent)).as_deref(),
            Some("@maven//:guava")
        );
        assert_eq!(short_label("p", "@@+other//:x", Some(&apparent)), None);
    }
}
