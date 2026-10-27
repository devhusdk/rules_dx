//! The optional parts `dx init` scaffolds and the committed record of which ones a repo chose.

use std::path::Path;

use super::{AdoptError, Applied};

/// One optional group of files `dx init` writes only when selected.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum InitPart {
    Envrc,
    Devcontainer,
    Vscode,
    GithubCi,
    Agents,
}

/// Every part name, in canonical order.
pub const INIT_PART_NAMES: [&str; 5] = ["envrc", "devcontainer", "vscode", "github-ci", "agents"];

/// The committed record of the parts `dx init --apply` wrote.
pub const INIT_RECORD_REL: &str = ".dx/init.toml";

const RECORD_KEY: &str = "with";

const RECORD_HEADER: &str = "# Written by dx init --apply; remove a name to stop checking that part.\n# dx init and dx status read this file; dx new and dx init --full select every part.\n";

impl InitPart {
    /// Every part, in canonical order.
    pub const ALL: [InitPart; 5] = [
        InitPart::Envrc,
        InitPart::Devcontainer,
        InitPart::Vscode,
        InitPart::GithubCi,
        InitPart::Agents,
    ];

    /// The name `--with` and the record use.
    pub fn name(self) -> &'static str {
        INIT_PART_NAMES[self as usize]
    }

    /// The part a name selects.
    pub fn from_name(name: &str) -> Option<InitPart> {
        Self::ALL.into_iter().find(|part| part.name() == name)
    }

    /// The scaffold files this part adds to the init plan.
    ///
    /// The agents part writes no scaffold file; its hooks, AGENTS.md block and CLAUDE.md
    /// import come from the wiring step.
    pub fn scaffold_paths(self) -> &'static [&'static str] {
        match self {
            InitPart::Envrc => &[".envrc"],
            InitPart::Devcontainer => &[".devcontainer/devcontainer.json"],
            InitPart::Vscode => &[".vscode/settings.json", ".vscode/extensions.json"],
            InitPart::GithubCi => &[".github/workflows/ci.yml"],
            InitPart::Agents => &[],
        }
    }
}

/// A set of init parts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct InitParts(u8);

impl InitParts {
    /// No optional part: the essentials only.
    pub const NONE: InitParts = InitParts(0);
    /// Every part: the full scaffold `dx new` and `dx init --full` write.
    pub const ALL: InitParts = InitParts(0b1_1111);

    fn bit(part: InitPart) -> u8 {
        1 << (part as u8)
    }

    /// This set with `part` added.
    #[must_use]
    pub fn with(self, part: InitPart) -> InitParts {
        InitParts(self.0 | Self::bit(part))
    }

    pub fn contains(self, part: InitPart) -> bool {
        self.0 & Self::bit(part) != 0
    }

    #[must_use]
    pub fn union(self, other: InitParts) -> InitParts {
        InitParts(self.0 | other.0)
    }

    pub fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// The parts in this set, in canonical order.
    pub fn parts(self) -> impl Iterator<Item = InitPart> {
        InitPart::ALL
            .into_iter()
            .filter(move |part| self.contains(*part))
    }

    /// The part names in this set, in canonical order.
    pub fn names(self) -> Vec<&'static str> {
        self.parts().map(InitPart::name).collect()
    }

    /// Parses `--with` values; each may hold a comma-separated list.
    pub fn parse_list(values: &[String]) -> Result<InitParts, AdoptError> {
        let mut parts = InitParts::NONE;
        for name in values.iter().flat_map(|value| value.split(',')) {
            let name = name.trim();
            if name.is_empty() {
                continue;
            }
            let part = InitPart::from_name(name).ok_or_else(|| AdoptError::InitUnknownPart {
                name: name.to_owned(),
            })?;
            parts = parts.with(part);
        }
        Ok(parts)
    }
}

/// What `.dx/init.toml` records.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct InitRecord {
    /// The known parts it selects.
    pub parts: InitParts,
    /// Names this dx does not know, kept so a newer record survives an older dx.
    pub unknown: Vec<String>,
}

fn read_record_table(root: &Path) -> Result<Option<toml::Table>, AdoptError> {
    let path = root.join(INIT_RECORD_REL);
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => {
            return Err(AdoptError::InitRecord {
                detail: e.to_string(),
            })
        }
    };
    text.parse::<toml::Table>()
        .map(Some)
        .map_err(|e| AdoptError::InitRecord {
            detail: e.to_string(),
        })
}

fn record_names(table: &toml::Table) -> Result<Vec<String>, AdoptError> {
    let Some(value) = table.get(RECORD_KEY) else {
        return Ok(Vec::new());
    };
    let bad = || AdoptError::InitRecord {
        detail: format!("`{RECORD_KEY}` must be an array of part names"),
    };
    value
        .as_array()
        .ok_or_else(bad)?
        .iter()
        .map(|item| item.as_str().map(str::to_owned).ok_or_else(bad))
        .collect()
}

fn split_names(names: &[String]) -> InitRecord {
    let mut record = InitRecord::default();
    for name in names {
        match InitPart::from_name(name) {
            Some(part) => record.parts = record.parts.with(part),
            None if !record.unknown.contains(name) => record.unknown.push(name.clone()),
            None => {}
        }
    }
    record
}

/// Reads `root/.dx/init.toml`. A missing file records no part.
///
/// Unknown names and keys are kept, not errors, so a record from a newer dx still reads.
pub fn read_init_record(root: &Path) -> Result<InitRecord, AdoptError> {
    match read_record_table(root)? {
        Some(table) => Ok(split_names(&record_names(&table)?)),
        None => Ok(InitRecord::default()),
    }
}

/// Adds `parts` to the record, keeping its unknown names and keys.
///
/// Writes only when the content changes and never creates the file for an empty selection.
pub fn merge_init_record(
    root: &Path,
    parts: InitParts,
    applied: &mut Applied,
) -> Result<(), AdoptError> {
    let existing = read_record_table(root)?;
    if existing.is_none() && parts.is_empty() {
        return Ok(());
    }
    let mut table = existing.clone().unwrap_or_default();
    let record = split_names(&record_names(&table)?);
    let merged = record.parts.union(parts);
    if existing.is_some() && merged == record.parts {
        return Ok(());
    }
    let mut names: Vec<toml::Value> = merged
        .names()
        .into_iter()
        .map(|name| toml::Value::String(name.to_owned()))
        .collect();
    names.extend(record.unknown.into_iter().map(toml::Value::String));
    table.insert(RECORD_KEY.to_owned(), toml::Value::Array(names));
    let body = toml::to_string(&table).map_err(|e| AdoptError::InitRecord {
        detail: e.to_string(),
    })?;
    let dest = super::scaffold::scaffold_dest_within_root(root, INIT_RECORD_REL)?;
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(|e| AdoptError::CreateParent {
            parent: parent.display().to_string(),
            detail: e.to_string(),
        })?;
    }
    dx_atomic_fs::write_atomic(&dest, format!("{RECORD_HEADER}{body}").as_ref()).map_err(|e| {
        AdoptError::WriteFile {
            path: dest.display().to_string(),
            detail: e.to_string(),
        }
    })?;
    applied.wrote.push(INIT_RECORD_REL.to_owned());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    #[test]
    fn names_follow_the_canonical_order() {
        let names: Vec<_> = InitPart::ALL.into_iter().map(InitPart::name).collect();
        assert_eq!(names, INIT_PART_NAMES);
        assert_eq!(InitParts::ALL.names(), INIT_PART_NAMES);
    }

    #[test]
    fn parse_list_dedupes_and_orders() {
        let parts = InitParts::parse_list(&strings(&["agents,vscode", "vscode"])).expect("parse");
        assert_eq!(parts.names(), ["vscode", "agents"]);
        assert!(!parts.contains(InitPart::Envrc));
        assert_eq!(InitParts::parse_list(&[]).expect("empty"), InitParts::NONE);
    }

    #[test]
    fn parse_list_rejects_an_unknown_name_and_lists_the_five() {
        let error = InitParts::parse_list(&strings(&["bogus"])).expect_err("unknown");
        let text = error.to_string();
        assert!(text.contains("bogus"), "{text}");
        for name in INIT_PART_NAMES {
            assert!(text.contains(name), "{text}");
        }
    }

    #[test]
    fn missing_record_selects_nothing() {
        let scratch = dx_test_scratch::scratch("dx-init-record-missing-");
        let record = read_init_record(scratch.path()).expect("read");
        assert_eq!(record, InitRecord::default());
    }

    #[test]
    fn empty_selection_writes_no_record() {
        let scratch = dx_test_scratch::scratch("dx-init-record-empty-");
        let mut applied = Applied::default();
        merge_init_record(scratch.path(), InitParts::NONE, &mut applied).expect("merge");
        assert!(!scratch.path().join(INIT_RECORD_REL).exists());
        assert!(applied.wrote.is_empty());
    }

    #[test]
    fn merge_only_adds_and_writes_on_change() {
        let scratch = dx_test_scratch::scratch("dx-init-record-merge-");
        let root = scratch.path();
        let vscode = InitParts::NONE.with(InitPart::Vscode);
        let mut applied = Applied::default();
        merge_init_record(root, vscode, &mut applied).expect("first");
        assert_eq!(applied.wrote, [INIT_RECORD_REL]);
        let text = std::fs::read_to_string(root.join(INIT_RECORD_REL)).expect("record");
        assert!(text.starts_with("# Written by dx init --apply"), "{text}");
        assert!(text.contains("with = [\"vscode\"]"), "{text}");
        let mut again = Applied::default();
        merge_init_record(root, vscode, &mut again).expect("same");
        merge_init_record(root, InitParts::NONE, &mut again).expect("none");
        assert!(again.wrote.is_empty(), "{again:?}");
        merge_init_record(root, InitParts::NONE.with(InitPart::Envrc), &mut again).expect("add");
        let record = read_init_record(root).expect("read");
        assert_eq!(record.parts.names(), ["envrc", "vscode"]);
    }

    #[test]
    fn unknown_names_and_keys_survive_a_rewrite() {
        let scratch = dx_test_scratch::scratch("dx-init-record-unknown-");
        let root = scratch.path();
        std::fs::create_dir_all(root.join(".dx")).expect("dx dir");
        std::fs::write(
            root.join(INIT_RECORD_REL),
            "with = [\"future\", \"vscode\"]\nrust_targets = [\"x86_64-unknown-linux-gnu\"]\n",
        )
        .expect("record");
        let record = read_init_record(root).expect("read");
        assert_eq!(record.parts.names(), ["vscode"]);
        assert_eq!(record.unknown, ["future"]);
        let mut applied = Applied::default();
        merge_init_record(root, InitParts::NONE.with(InitPart::Agents), &mut applied)
            .expect("merge");
        let text = std::fs::read_to_string(root.join(INIT_RECORD_REL)).expect("record");
        assert!(text.contains("rust_targets"), "{text}");
        let record = read_init_record(root).expect("reread");
        assert_eq!(record.parts.names(), ["vscode", "agents"]);
        assert_eq!(record.unknown, ["future"]);
    }

    #[test]
    fn invalid_record_is_an_error() {
        let scratch = dx_test_scratch::scratch("dx-init-record-invalid-");
        let root = scratch.path();
        std::fs::create_dir_all(root.join(".dx")).expect("dx dir");
        std::fs::write(root.join(INIT_RECORD_REL), "with = [\n").expect("record");
        assert!(read_init_record(root).is_err());
        std::fs::write(root.join(INIT_RECORD_REL), "with = \"vscode\"\n").expect("record");
        assert!(read_init_record(root).is_err());
    }
}
