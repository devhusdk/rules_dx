//! Plans a writable mirror of a runfiles tree; language-neutral.

use std::collections::BTreeSet;

/// One runfiles entry: its rlocation and the file it points at (None for an empty file).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub rlocation: String,
    pub target: Option<String>,
}

/// How each entry lands in the mirror.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct MirrorPlan {
    /// Entries copied as real, writable files.
    pub copy: Vec<Entry>,
    /// Entries linked to the original file.
    pub link: Vec<Entry>,
    /// Entries that are empty files.
    pub empty: Vec<String>,
}

fn unescape(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('s') => out.push(' '),
            Some('n') => out.push('\n'),
            Some('b') => out.push('\\'),
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
            None => out.push('\\'),
        }
    }
    out
}

fn escape(raw: &str) -> String {
    raw.replace('\\', "\\b")
        .replace('\n', "\\n")
        .replace(' ', "\\s")
}

/// Parses a Bazel runfiles MANIFEST, including its escaped-line form.
pub fn parse_manifest(text: &str) -> Vec<Entry> {
    let mut entries = Vec::new();
    for line in text.lines() {
        if line.is_empty() {
            continue;
        }
        let (rlocation, target) = if let Some(escaped) = line.strip_prefix(' ') {
            match escaped.split_once(' ') {
                Some((key, value)) => (unescape(key), unescape(value)),
                None => (unescape(escaped), String::new()),
            }
        } else {
            match line.split_once(' ') {
                Some((key, value)) => (key.to_string(), value.to_string()),
                None => (line.to_string(), String::new()),
            }
        };
        entries.push(Entry {
            rlocation,
            target: if target.is_empty() {
                None
            } else {
                Some(target)
            },
        });
    }
    entries
}

/// Renders a MANIFEST whose entries point into the mirror rooted at `root`.
pub fn render_manifest(entries: &[Entry], root: &str) -> String {
    let mut out = String::new();
    for entry in entries {
        let target = if entry.target.is_some() {
            format!("{}/{}", root, entry.rlocation)
        } else {
            String::new()
        };
        if entry.rlocation.contains([' ', '\n', '\\']) || target.contains(['\n', '\\']) {
            out.push(' ');
            out.push_str(&escape(&entry.rlocation));
            out.push(' ');
            out.push_str(&escape(&target));
        } else {
            out.push_str(&entry.rlocation);
            out.push(' ');
            out.push_str(&target);
        }
        out.push('\n');
    }
    out
}

/// Splits entries into the copy, link and empty sets.
pub fn plan(entries: &[Entry], copies: &BTreeSet<String>) -> MirrorPlan {
    let mut plan = MirrorPlan::default();
    for entry in entries {
        match entry.target {
            None => plan.empty.push(entry.rlocation.clone()),
            Some(_) if copies.contains(&entry.rlocation) => plan.copy.push(entry.clone()),
            Some(_) => plan.link.push(entry.clone()),
        }
    }
    plan
}

/// Returns the rlocations that must be real copies: each assembly plus its PDB when present.
pub fn copy_set(entries: &[Entry], assemblies: &[String]) -> BTreeSet<String> {
    let present: BTreeSet<&str> = entries.iter().map(|e| e.rlocation.as_str()).collect();
    let mut out = BTreeSet::new();
    for assembly in assemblies {
        out.insert(assembly.clone());
        if let Some(stem) = assembly
            .strip_suffix(".dll")
            .or(assembly.strip_suffix(".exe"))
        {
            let pdb = format!("{stem}.pdb");
            if present.contains(pdb.as_str()) {
                out.insert(pdb);
            }
        }
    }
    out
}

/// Returns the parent rlocation directory of one rlocation.
pub fn parent(rlocation: &str) -> &str {
    rlocation.rsplit_once('/').map_or("", |(dir, _)| dir)
}

/// Returns the assembly name coverlet filters on: the file name without its extension.
pub fn assembly_name(rlocation: &str) -> &str {
    let file = rlocation.rsplit_once('/').map_or(rlocation, |(_, f)| f);
    file.rsplit_once('.').map_or(file, |(stem, _)| stem)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(r: &str, t: Option<&str>) -> Entry {
        Entry {
            rlocation: r.to_string(),
            target: t.map(str::to_string),
        }
    }

    #[test]
    fn parses_plain_and_empty_lines() {
        let entries =
            parse_manifest("_main/a.dll /out/a.dll\n_main/__init__.py \n_repo_mapping /x/rm\n");
        assert_eq!(
            entries,
            vec![
                entry("_main/a.dll", Some("/out/a.dll")),
                entry("_main/__init__.py", None),
                entry("_repo_mapping", Some("/x/rm")),
            ]
        );
    }

    #[test]
    fn parses_escaped_lines() {
        let entries = parse_manifest(" _main/a\\sb.cs C:\\bout\\bspaced\\sdir\\ba.cs\n");
        assert_eq!(
            entries,
            vec![entry("_main/a b.cs", Some("C:\\out\\spaced dir\\a.cs"))]
        );
    }

    #[test]
    fn renders_mirror_manifest_with_escapes() {
        let entries = vec![
            entry("_main/a.dll", Some("/out/a.dll")),
            entry("_main/a b.cs", Some("/out/a b.cs")),
            entry("_main/empty", None),
        ];
        let text = render_manifest(&entries, "/tmp/m");
        assert_eq!(
            text,
            "_main/a.dll /tmp/m/_main/a.dll\n _main/a\\sb.cs /tmp/m/_main/a\\sb.cs\n_main/empty \n"
        );
        assert_eq!(
            parse_manifest(&text)[1],
            entry("_main/a b.cs", Some("/tmp/m/_main/a b.cs"))
        );
    }

    #[test]
    fn copies_only_instrumented_assemblies_and_their_pdbs() {
        let entries = vec![
            entry("_main/p/lib/lib.dll", Some("/o/lib.dll")),
            entry("_main/p/lib/lib.pdb", Some("/o/lib.pdb")),
            entry("_main/p/t/t.dll", Some("/o/t.dll")),
            entry("nuget/xunit.dll", Some("/o/xunit.dll")),
            entry("_main/p/empty", None),
        ];
        let copies = copy_set(
            &entries,
            &[
                "_main/p/lib/lib.dll".to_string(),
                "_main/p/t/t.dll".to_string(),
            ],
        );
        assert_eq!(
            copies.iter().map(String::as_str).collect::<Vec<_>>(),
            vec![
                "_main/p/lib/lib.dll",
                "_main/p/lib/lib.pdb",
                "_main/p/t/t.dll"
            ]
        );
        let plan = plan(&entries, &copies);
        assert_eq!(plan.copy.len(), 3);
        assert_eq!(
            plan.link,
            vec![entry("nuget/xunit.dll", Some("/o/xunit.dll"))]
        );
        assert_eq!(plan.empty, vec!["_main/p/empty".to_string()]);
    }

    #[test]
    fn names_assemblies_and_parents() {
        assert_eq!(
            assembly_name("_main/p/net10.0/greeter_lib.dll"),
            "greeter_lib"
        );
        assert_eq!(parent("_main/p/net10.0/greeter_lib.dll"), "_main/p/net10.0");
        assert_eq!(parent("top.dll"), "");
    }
}
