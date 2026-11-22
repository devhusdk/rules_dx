//! Renders the standard and strict tier pages from the tier files.

#![deny(clippy::indexing_slicing, clippy::string_slice)]

use std::collections::BTreeMap;
use std::fmt;

use dx_diff::{DiffError, FilePatch, PatchKind, render_patch};

pub const UPDATE_COMMAND: &str = "bazel run //docs:update_tiers";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tier {
    pub tool: String,
    pub source: String,
    pub native: String,
    pub standard_path: String,
    pub strict_path: String,
}

/// Why the tier manifest or a tier page is bad.
#[derive(Debug)]
pub enum TiersError {
    ManifestLine { line: String },
    MissingFile { path: String },
    Patch { tool: String, source: DiffError },
}

impl fmt::Display for TiersError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ManifestLine { line } => write!(f, "manifest line needs 5 fields: {line:?}"),
            Self::MissingFile { path } => write!(f, "missing tier file {path}"),
            Self::Patch { tool, source } => write!(f, "{tool}: {source}"),
        }
    }
}

impl std::error::Error for TiersError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Patch { source, .. } => Some(source),
            Self::ManifestLine { .. } | Self::MissingFile { .. } => None,
        }
    }
}

/// Reads one tab separated line per tool: tool, source, native file, standard path, strict path.
///
/// # Errors
///
/// Returns [`TiersError::ManifestLine`] for a line without exactly 5 fields.
pub fn parse_manifest(text: &str) -> Result<Vec<Tier>, TiersError> {
    let mut tiers = Vec::new();
    for line in text.lines().filter(|line| !line.is_empty()) {
        let fields: Vec<&str> = line.split('\t').collect();
        let [tool, source, native, standard_path, strict_path] = fields[..] else {
            return Err(TiersError::ManifestLine {
                line: line.to_owned(),
            });
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
        "### {}\n\nSource: {}.{native}\n\n",
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

// Both pages share the steps for picking a tier; only the opening line, the
// example `dx.toml` and the line before the per-tool reference differ.
fn intro(title: &str, purpose: &str, tier: &str, reference: &str) -> String {
    format!(
        "# {title}\n\n{purpose}\n\n\
         ## Pick the tier\n\n\
         1. Open `dx.toml` at the root of your workspace, the folder that holds `MODULE.bazel`. Create it if it is missing.\n\
         2. Set `tier`:\n\n\
         \x20  ```toml\n   schema_version = 1\n   tier = \"{tier}\"\n   ```\n\n\
         3. Run `tools/dx config <tool>`, such as `tools/dx config ruff`, to see the files that tool now reads.\n\n\
         Without `dx.toml`, every tool runs at the strict tier. Set `tier = \"standard\"` to opt down. To give one family, such as `rust`, its own tier, set `tier` under `[families.rust]`. The [config reference](cli/commands/config.md#dxtoml) lists every key.\n\n\
         To change a single setting, edit the tool's own config file at the root of your repo. The [quality page](cli/commands/quality.md) lists the file each tool reads.\n\n\
         ## Tools\n\n\
         {reference} Each section names the tool, where its tier config comes from, and the native config file you edit to change it. This page is generated. To refresh it, run `{UPDATE_COMMAND}`.\n\n"
    )
}

/// # Errors
///
/// Returns [`TiersError::MissingFile`] when a standard tier file is not in `files`.
pub fn standard_page(
    tiers: &[Tier],
    files: &BTreeMap<String, String>,
) -> Result<String, TiersError> {
    let mut out = intro(
        "Standard tiers",
        "This page shows the config each lint and format tool runs with when you opt down to the standard tier.",
        "standard",
        "Each section below shows the full standard config of one tool.",
    );
    for tier in tiers {
        let body = read(files, &tier.standard_path)?;
        out.push_str(&header(tier));
        block(&mut out, extension(&tier.standard_path), body);
    }
    Ok(trim(out))
}

/// # Errors
///
/// Returns [`TiersError::MissingFile`] when a tier file is not in `files`, and
/// [`TiersError::Patch`] when the diff cannot be rendered.
pub fn strict_page(tiers: &[Tier], files: &BTreeMap<String, String>) -> Result<String, TiersError> {
    let mut out = intro(
        "Strict tiers",
        "This page shows what each lint and format tool checks on top of the standard tier. Strict is the default tier.",
        "strict",
        "Each section below is a diff from the standard config of one tool to its strict config. Each tier file is complete on its own, so the diff is the whole difference.",
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
        .map_err(|source| TiersError::Patch {
            tool: tier.tool.clone(),
            source,
        })?;
        block(&mut out, "diff", &patch);
    }
    Ok(trim(out))
}

fn read<'a>(files: &'a BTreeMap<String, String>, path: &str) -> Result<&'a str, TiersError> {
    files
        .get(path)
        .map(String::as_str)
        .ok_or_else(|| TiersError::MissingFile {
            path: path.to_owned(),
        })
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
        let error = parse_manifest("a\tb\n").expect_err("short line");
        assert!(matches!(&error, TiersError::ManifestLine { line } if line == "a\tb"));
        assert_eq!(error.to_string(), "manifest line needs 5 fields: \"a\\tb\"");
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
        assert!(matches!(&error, TiersError::MissingFile { path } if path == "demo/standard.json"));
        assert_eq!(error.to_string(), "missing tier file demo/standard.json");
    }
}
