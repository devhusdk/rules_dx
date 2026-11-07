//! Renders the standard and strict tier pages from the tier files.

use std::collections::BTreeMap;

use dx_diff::{FilePatch, PatchKind, render_patch};

pub const UPDATE_COMMAND: &str = "bazel run //docs:update_tiers";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tier {
    pub tool: String,
    pub source: String,
    pub native: String,
    pub standard_path: String,
    pub strict_path: String,
}

/// Reads one tab separated line per tool: tool, source, native file, standard path, strict path.
pub fn parse_manifest(text: &str) -> Result<Vec<Tier>, String> {
    let mut tiers = Vec::new();
    for line in text.lines().filter(|line| !line.is_empty()) {
        let fields: Vec<&str> = line.split('\t').collect();
        let [tool, source, native, standard_path, strict_path] = fields[..] else {
            return Err(format!("manifest line needs 5 fields: {line:?}"));
        };
        tiers.push(Tier {
            tool: tool.to_owned(),
            source: source.to_owned(),
            native: native.to_owned(),
            standard_path: standard_path.to_owned(),
            strict_path: strict_path.to_owned(),
        });
    }
    tiers.sort_by(|a, b| a.tool.cmp(&b.tool));
    Ok(tiers)
}

fn fence(body: &str) -> String {
    let mut longest = 0;
    let mut run = 0;
    for ch in body.chars() {
        run = if ch == '`' { run + 1 } else { 0 };
        longest = longest.max(run);
    }
    "`".repeat((longest + 1).max(3))
}

fn extension(path: &str) -> &str {
    path.rsplit_once('.').map_or("", |(_, ext)| ext)
}

fn header(tier: &Tier) -> String {
    let native = if tier.native.is_empty() {
        String::new()
    } else {
        format!(" Native file: `{}`.", tier.native)
    };
    format!(
        "## {}\n\nSource: {}.{native}\n\n",
        tier.tool,
        tier.source.trim_end_matches('.')
    )
}

fn block(out: &mut String, language: &str, body: &str) {
    let marks = fence(body);
    out.push_str(&marks);
    out.push_str(language);
    out.push('\n');
    out.push_str(body);
    if !body.ends_with('\n') {
        out.push('\n');
    }
    out.push_str(&marks);
    out.push_str("\n\n");
}

fn intro(title: &str, text: &str) -> String {
    format!(
        "# {title}\n\n{text} This page is generated. To refresh it, run `{UPDATE_COMMAND}`.\n\n"
    )
}

pub fn standard_page(tiers: &[Tier], files: &BTreeMap<String, String>) -> Result<String, String> {
    let mut out = intro(
        "Standard tiers",
        "This page lists the config each tool gets at the standard tier.",
    );
    for tier in tiers {
        let body = read(files, &tier.standard_path)?;
        out.push_str(&header(tier));
        block(&mut out, extension(&tier.standard_path), body);
    }
    Ok(trim(out))
}

pub fn strict_page(tiers: &[Tier], files: &BTreeMap<String, String>) -> Result<String, String> {
    let mut out = intro(
        "Strict tiers",
        "This page shows what the strict tier changes against the standard tier of each tool. A tier file is self-contained, so the diff is the whole difference.",
    );
    for tier in tiers {
        let standard = read(files, &tier.standard_path)?;
        let strict = read(files, &tier.strict_path)?;
        out.push_str(&header(tier));
        if standard == strict {
            out.push_str("Same as the standard tier.\n\n");
            continue;
        }
        let patch = render_patch(&[FilePatch {
            path: &tier.strict_path,
            kind: PatchKind::Modify,
            original: standard,
            candidate: strict,
        }])
        .map_err(|error| format!("{}: {error}", tier.tool))?;
        block(&mut out, "diff", &patch);
    }
    Ok(trim(out))
}

fn read<'a>(files: &'a BTreeMap<String, String>, path: &str) -> Result<&'a str, String> {
    files
        .get(path)
        .map(String::as_str)
        .ok_or_else(|| format!("missing tier file {path}"))
}

fn trim(mut page: String) -> String {
    while page.ends_with("\n\n") {
        page.pop();
    }
    page
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tier() -> Tier {
        Tier {
            tool: "demo".into(),
            source: "dx".into(),
            native: "demo.json".into(),
            standard_path: "demo/standard.json".into(),
            strict_path: "demo/strict.json".into(),
        }
    }

    fn files(standard: &str, strict: &str) -> BTreeMap<String, String> {
        BTreeMap::from([
            ("demo/standard.json".to_owned(), standard.to_owned()),
            ("demo/strict.json".to_owned(), strict.to_owned()),
        ])
    }

    #[test]
    fn manifest_is_sorted_by_tool() {
        let text = "b\tdx\t\tb/s.json\tb/t.json\na\tdx\ta.json\ta/s.json\ta/t.json\n";
        let tiers = parse_manifest(text).expect("manifest");
        assert_eq!(tiers[0].tool, "a");
        assert_eq!(tiers[1].tool, "b");
        assert!(parse_manifest("a\tb\n").is_err());
    }

    #[test]
    fn equal_pair_says_same_as_standard() {
        let files = files("{}\n", "{}\n");
        let strict = strict_page(&[tier()], &files).expect("page");
        assert!(strict.contains("Same as the standard tier."));
        let standard = standard_page(&[tier()], &files).expect("page");
        assert!(standard.contains("```json\n{}\n```"));
    }

    #[test]
    fn diff_pair_renders_the_difference() {
        let files = files("{\"a\": 1}\n", "{\"a\": 2}\n");
        let strict = strict_page(&[tier()], &files).expect("page");
        assert!(strict.contains("```diff\n--- a/demo/strict.json\n+++ b/demo/strict.json\n"));
        assert!(strict.contains("-{\"a\": 1}\n+{\"a\": 2}\n"));
        assert!(strict.ends_with("```\n"));
    }

    #[test]
    fn fence_outgrows_backticks_in_the_body() {
        assert_eq!(fence("a ``` b"), "````");
        assert_eq!(fence("plain"), "```");
    }

    #[test]
    fn missing_file_is_an_error() {
        let error = standard_page(&[tier()], &BTreeMap::new()).expect_err("missing");
        assert!(error.contains("demo/standard.json"));
    }
}
