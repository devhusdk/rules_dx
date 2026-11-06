//! Appends accepted findings to `security.toml` and `licenses.toml`, keeping the file's comments and order.

use toml_edit::{value, ArrayOfTables, DocumentMut, Item, Table};

/// One `[[table]]` entry to append.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Entry {
    pub table: &'static str,
    /// The keys that name the finding; an entry with the same identity is not added again.
    pub identity: Vec<(&'static str, String)>,
    /// The remaining keys, written after the identity.
    pub rest: Vec<(&'static str, String)>,
}

/// The new text of a policy file with `entries` appended, or `None` when every entry is already there.
/// A missing file starts with `header` and `schema_version = 1`.
pub fn append_entries(
    text: Option<&str>,
    header: &str,
    entries: &[Entry],
) -> Result<Option<String>, String> {
    let existing = text.filter(|text| !text.trim().is_empty());
    let mut doc: DocumentMut = match existing {
        Some(text) => text
            .parse()
            .map_err(|error| format!("invalid TOML: {error}"))?,
        None => format!("{header}\nschema_version = 1\n")
            .parse()
            .map_err(|error| format!("invalid TOML: {error}"))?,
    };
    let mut added = false;
    for entry in entries {
        if holds(&doc, entry) {
            continue;
        }
        let mut table = Table::new();
        for (key, text) in entry.identity.iter().chain(&entry.rest) {
            table.insert(key, value(text.as_str()));
        }
        let item = doc
            .entry(entry.table)
            .or_insert_with(|| Item::ArrayOfTables(ArrayOfTables::new()));
        let tables = item
            .as_array_of_tables_mut()
            .ok_or_else(|| format!("{} is not an array of tables", entry.table))?;
        tables.push(table);
        added = true;
    }
    Ok(added.then(|| doc.to_string()))
}

fn holds(doc: &DocumentMut, entry: &Entry) -> bool {
    let Some(tables) = doc.get(entry.table).and_then(Item::as_array_of_tables) else {
        return false;
    };
    tables.iter().any(|table| {
        entry
            .identity
            .iter()
            .all(|(key, text)| table.get(key).and_then(Item::as_str) == Some(text.as_str()))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn exception(advisory: &str) -> Entry {
        Entry {
            table: "exception",
            identity: vec![
                ("advisory", advisory.to_owned()),
                ("package", "left-pad".to_owned()),
                ("set", "npm".to_owned()),
                ("versions", "=1.0.0".to_owned()),
            ],
            rest: vec![
                ("reason", "waiting on upstream".to_owned()),
                ("expires", "2030-01-01".to_owned()),
            ],
        }
    }

    #[test]
    fn a_missing_file_starts_with_the_header_and_schema_version() {
        let out = append_entries(None, "#:schema https://example/x.json", &[exception("A-1")])
            .expect("appends")
            .expect("changed");
        assert!(out.starts_with("#:schema https://example/x.json\nschema_version = 1\n"));
        assert!(out.contains("[[exception]]"));
        assert!(out.contains("versions = \"=1.0.0\""));
    }

    #[test]
    fn comments_and_order_are_kept() {
        let before = "# top\nschema_version = 1\n\n# keep me\nfail_on = [\"vulnerability\"]\n";
        let out = append_entries(Some(before), "", &[exception("A-1")])
            .expect("appends")
            .expect("changed");
        assert!(out.starts_with(before));
    }

    #[test]
    fn an_existing_entry_is_not_duplicated() {
        let first = append_entries(None, "", &[exception("A-1")])
            .expect("appends")
            .expect("changed");
        assert_eq!(
            append_entries(Some(&first), "", &[exception("A-1")]).expect("appends"),
            None
        );
        let second = append_entries(Some(&first), "", &[exception("A-2")])
            .expect("appends")
            .expect("changed");
        assert_eq!(second.matches("[[exception]]").count(), 2);
    }
}
