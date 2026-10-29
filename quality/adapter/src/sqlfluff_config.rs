//! Merges nested `.sqlfluff` files the way sqlfluff does, so dx can hand sqlfluff one config.

use std::collections::BTreeMap;

/// One effective config for a set of files.
#[derive(Debug, PartialEq, Eq)]
pub struct Group {
    /// The nested config rels applied over the bound config, shallowest first.
    pub layers: Vec<String>,
    /// The merged config text; `None` when the bound config applies unchanged.
    pub merged: Option<String>,
    /// The files that share this config, sorted.
    pub files: Vec<String>,
}

type Leaves = BTreeMap<Vec<String>, String>;

/// Parses an ini config into leaf paths and raw values; mirrors sqlfluff's `load_ini_string`.
fn parse(text: &str) -> Result<Leaves, String> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut leaves = Leaves::new();
    let mut section: Option<Vec<String>> = None;
    let mut current: Option<Vec<String>> = None;
    let mut indent_level = 0usize;
    for (index, raw) in text.lines().enumerate() {
        let number = index + 1;
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }
        let indent = raw.len() - raw.trim_start().len();
        if let Some(path) = &current {
            if indent > indent_level {
                if let Some(value) = leaves.get_mut(path) {
                    value.push('\n');
                    value.push_str(line);
                }
                continue;
            }
        }
        indent_level = indent;
        if let Some(rest) = line.strip_prefix('[') {
            let Some(name) = rest.strip_suffix(']') else {
                return Err(format!("line {number}: unterminated section"));
            };
            current = None;
            section = if name == "sqlfluff" {
                Some(vec!["core".to_owned()])
            } else {
                name.strip_prefix("sqlfluff:")
                    .map(|tail| tail.split(':').map(str::to_owned).collect())
            };
            continue;
        }
        let Some(split) = line.find('=') else {
            return Err(format!("line {number}: expected `key = value`"));
        };
        let key = line[..split].trim();
        let value = line[split + 1..].trim();
        let Some(base) = &section else {
            current = None;
            continue;
        };
        let mut path = base.clone();
        path.extend(key.split('.').map(str::to_owned));
        if leaves.insert(path.clone(), value.to_owned()).is_some() {
            return Err(format!("line {number}: duplicate option `{key}`"));
        }
        current = Some(path);
    }
    Ok(leaves)
}

/// Renders leaves as ini text, sections and keys in sorted order.
fn render(leaves: &Leaves) -> String {
    let mut sections: BTreeMap<String, Vec<(String, &String)>> = BTreeMap::new();
    for (path, value) in leaves {
        let (section, key) = if path[0] == "core" {
            ("sqlfluff".to_owned(), path[1..].join("."))
        } else {
            let head = &path[..path.len() - 1];
            (
                format!("sqlfluff:{}", head.join(":")),
                path[path.len() - 1].clone(),
            )
        };
        sections.entry(section).or_default().push((key, value));
    }
    let mut out = String::new();
    for (section, keys) in sections {
        out.push_str(&format!("[{section}]\n"));
        for (key, value) in keys {
            let value = value.replace('\n', "\n    ");
            if value.is_empty() {
                out.push_str(&format!("{key} =\n"));
            } else {
                out.push_str(&format!("{key} = {value}\n"));
            }
        }
        out.push('\n');
    }
    out
}

/// Merges config layers leaf by leaf; a later layer wins.
pub fn merge(layers: &[&str]) -> Result<String, String> {
    let mut merged = Leaves::new();
    for text in layers {
        merged.extend(parse(text)?);
    }
    Ok(render(&merged))
}

fn normalise(rel: &str) -> String {
    rel.replace('\\', "/").trim_start_matches("./").to_owned()
}

fn dir_of(rel: &str) -> &str {
    rel.rsplit_once('/').map_or("", |(dir, _)| dir)
}

fn depth(dir: &str) -> usize {
    if dir.is_empty() {
        0
    } else {
        dir.split('/').count()
    }
}

/// Groups files by the chain of nested configs above them and merges each chain over the bound
/// config. `nested` holds `(config rel, text)`; a config applies to files below its directory.
pub fn group(
    bound: &str,
    nested: &[(String, String)],
    files: &[String],
) -> Result<Vec<Group>, String> {
    let mut layers: Vec<(String, String, &str)> = nested
        .iter()
        .map(|(rel, text)| {
            let rel = normalise(rel);
            let dir = dir_of(&rel).to_owned();
            (dir, rel, text.as_str())
        })
        .collect();
    layers.sort_by(|a, b| (depth(&a.0), &a.1).cmp(&(depth(&b.0), &b.1)));
    let mut chains: BTreeMap<Vec<usize>, Vec<String>> = BTreeMap::new();
    for file in files {
        let file = normalise(file);
        let chain: Vec<usize> = layers
            .iter()
            .enumerate()
            .filter(|(_, (dir, _, _))| {
                dir.is_empty()
                    || file
                        .strip_prefix(dir.as_str())
                        .is_some_and(|rest| rest.starts_with('/'))
            })
            .map(|(index, _)| index)
            .collect();
        chains.entry(chain).or_default().push(file);
    }
    let mut groups = Vec::with_capacity(chains.len());
    for (chain, mut files) in chains {
        files.sort();
        files.dedup();
        let merged = if chain.is_empty() {
            None
        } else {
            let mut texts = vec![bound];
            texts.extend(chain.iter().map(|&index| layers[index].2));
            Some(merge(&texts)?)
        };
        groups.push(Group {
            layers: chain.iter().map(|&index| layers[index].1.clone()).collect(),
            merged,
            files,
        });
    }
    Ok(groups)
}

#[cfg(test)]
mod tests {
    use super::*;

    const BOUND: &str = "[sqlfluff]\ndialect = ansi\ntemplater = raw\n[sqlfluff:rules:capitalisation.keywords]\ncapitalisation_policy = upper\n";

    fn nested(rel: &str, text: &str) -> (String, String) {
        (rel.to_owned(), text.to_owned())
    }

    #[test]
    fn child_overrides_a_key_and_inherits_the_rest() {
        let merged = merge(&[BOUND, "[sqlfluff]\ntemplater = placeholder\n"]).unwrap();
        assert!(merged.contains("dialect = ansi"));
        assert!(merged.contains("templater = placeholder"));
        assert!(!merged.contains("templater = raw"));
        assert!(merged.contains("capitalisation_policy = upper"));
    }

    #[test]
    fn dotted_key_collides_with_section_path() {
        let merged = merge(&["[sqlfluff:a:b]\nc = 1\n", "[sqlfluff:a]\nb.c = 2\n"]).unwrap();
        assert_eq!(
            merged.matches("c = ").count() + merged.matches("b.c = ").count(),
            1
        );
        assert!(merged.contains("= 2"));
    }

    #[test]
    fn continuation_lines_and_case_are_kept() {
        let text = "[sqlfluff]\nIgnore = a,\n    b\n\tc\nDialect = ansi\n";
        let leaves = parse(text).unwrap();
        assert_eq!(
            leaves[&vec!["core".to_owned(), "Ignore".to_owned()]],
            "a,\nb\nc"
        );
        assert!(leaves.contains_key(&vec!["core".to_owned(), "Dialect".to_owned()]));
        let again = parse(&render(&leaves)).unwrap();
        assert_eq!(again, leaves);
    }

    #[test]
    fn bom_crlf_and_foreign_sections() {
        let text = "\u{feff}[sqlfluff]\r\ndialect = ansi\r\n[flake8]\r\nmax = 3\r\n";
        let leaves = parse(text).unwrap();
        assert_eq!(leaves.len(), 1);
        assert_eq!(leaves.values().next().unwrap(), "ansi");
    }

    #[test]
    fn siblings_form_separate_groups() {
        let groups = group(
            BOUND,
            &[
                nested("a/.sqlfluff", "[sqlfluff]\ntemplater = jinja\n"),
                nested("b/.sqlfluff", "[sqlfluff]\ntemplater = placeholder\n"),
            ],
            &[
                "b/x.sql".into(),
                "a/y.sql".into(),
                "c/z.sql".into(),
                "a/w.sql".into(),
            ],
        )
        .unwrap();
        assert_eq!(groups.len(), 3);
        let by: Vec<_> = groups.iter().map(|g| g.files.clone()).collect();
        assert!(by.contains(&vec!["a/w.sql".to_owned(), "a/y.sql".to_owned()]));
        assert!(by.contains(&vec!["c/z.sql".to_owned()]));
        let plain = groups.iter().find(|g| g.files == ["c/z.sql"]).unwrap();
        assert!(plain.merged.is_none());
    }

    #[test]
    fn bound_config_is_the_base_wherever_it_lives() {
        let groups = group(
            BOUND,
            &[nested("x/.sqlfluff", "[sqlfluff]\ntemplater = jinja\n")],
            &["x/q.sql".into()],
        )
        .unwrap();
        let merged = groups[0].merged.as_deref().unwrap();
        assert!(merged.contains("dialect = ansi"));
        assert!(merged.contains("templater = jinja"));
    }

    #[test]
    fn deep_chain_applies_shallowest_first() {
        let groups = group(
            BOUND,
            &[
                nested("a/b/.sqlfluff", "[sqlfluff]\ntemplater = placeholder\n"),
                nested("a/.sqlfluff", "[sqlfluff]\ntemplater = jinja\n"),
            ],
            &["a/b/f.sql".into(), "a/bc/g.sql".into()],
        )
        .unwrap();
        let deep = groups.iter().find(|g| g.files == ["a/b/f.sql"]).unwrap();
        assert_eq!(deep.layers, ["a/.sqlfluff", "a/b/.sqlfluff"]);
        assert!(deep
            .merged
            .as_deref()
            .unwrap()
            .contains("templater = placeholder"));
        let near = groups.iter().find(|g| g.files == ["a/bc/g.sql"]).unwrap();
        assert_eq!(near.layers, ["a/.sqlfluff"]);
    }

    #[test]
    fn bad_ini_is_an_error() {
        assert!(parse("[sqlfluff]\nnot a pair\n").is_err());
        assert!(parse("[sqlfluff]\na = 1\na = 2\n").is_err());
    }
}
