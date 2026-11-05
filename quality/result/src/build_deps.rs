//! Finds `deps` entries in BUILD file text and the bytes that remove them.

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
        assert_eq!(&build[start..end], "\"//quality/testdata/jvm_owned:dropped_helper\"");
        let fixed = cut(build, "user", "dropped_helper").expect("cut");
        assert!(!fixed.contains("dropped_helper"));
        assert!(fixed.contains(":kept_helper"));
    }
}
