use std::collections::BTreeMap;

use super::LcovError;

pub const ELIGIBLE: &str = "eligible";
pub const SUPPORT: &str = "support";

pub const IGNORED_PIN: &str = "ignored=";

#[derive(Debug, Default, PartialEq, Eq, Clone)]
pub struct InventoryRow {
    pub disposition: String,
    pub expected_ignored: Option<u64>,
}

fn parse_ignored_pin(field: &str, lineno: usize, raw: &str) -> Result<u64, LcovError> {
    let digits = field
        .strip_prefix(IGNORED_PIN)
        .ok_or_else(|| LcovError::MalformedInventory {
            lineno,
            raw: raw.to_string(),
        })?;
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(LcovError::MalformedInventory {
            lineno,
            raw: raw.to_string(),
        });
    }
    digits
        .parse::<u64>()
        .map_err(|_| LcovError::MalformedInventory {
            lineno,
            raw: raw.to_string(),
        })
}

pub fn parse_inventory(text: &str) -> Result<BTreeMap<String, InventoryRow>, LcovError> {
    let mut inventory = BTreeMap::new();
    for (index, raw) in text.lines().enumerate() {
        let lineno = index + 1;
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let malformed = || LcovError::MalformedInventory {
            lineno,
            raw: raw.to_string(),
        };
        let mut parts = line.split_whitespace();
        let disposition = parts.next().unwrap_or_default().to_string();
        let path = parts.next().unwrap_or_default().to_string();
        if disposition.is_empty() || path.is_empty() {
            return Err(malformed());
        }
        let mut expected_ignored = None;
        for field in parts {
            if field.starts_with(IGNORED_PIN) {
                if disposition == SUPPORT || expected_ignored.is_some() {
                    return Err(malformed());
                }
                expected_ignored = Some(parse_ignored_pin(field, lineno, raw)?);
            } else {
                return Err(malformed());
            }
        }
        if inventory.contains_key(&path) {
            return Err(LcovError::DuplicateInventoryPath { lineno, path });
        }
        inventory.insert(
            path,
            InventoryRow {
                disposition,
                expected_ignored,
            },
        );
    }
    Ok(inventory)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_inventory_with_comments_and_blanks() {
        let inventory = parse_inventory("# comment\n\neligible a.rs\nsupport b.rs  \n").unwrap();
        assert_eq!(inventory["a.rs"].disposition, ELIGIBLE);
        assert_eq!(inventory["a.rs"].expected_ignored, None);
        assert_eq!(inventory["b.rs"].disposition, SUPPORT);
        assert_eq!(inventory["b.rs"].expected_ignored, None);
    }

    #[test]
    fn parses_eligible_ignored_pin() {
        let inventory = parse_inventory("eligible a.rs ignored=3\n").unwrap();
        assert_eq!(inventory["a.rs"].disposition, ELIGIBLE);
        assert_eq!(inventory["a.rs"].expected_ignored, Some(3));
    }

    #[test]
    fn parses_zero_ignored_pin() {
        let inventory = parse_inventory("eligible a.rs ignored=0\n").unwrap();
        assert_eq!(inventory["a.rs"].expected_ignored, Some(0));
    }

    #[test]
    fn rejects_malformed_inventory_lines() {
        assert!(parse_inventory("eligible\n").is_err());
        assert!(parse_inventory("eligible a.rs extra\n").is_err());
        assert!(parse_inventory("   \n lone\n").is_err());
        assert!(parse_inventory("eligible a.rs ignored=\n").is_err());
        assert!(parse_inventory("eligible a.rs ignored=many\n").is_err());
        assert!(parse_inventory("eligible a.rs ignored=-1\n").is_err());
        assert!(parse_inventory("eligible a.rs ignored=1 ignored=2\n").is_err());
        assert!(parse_inventory("support a.rs ignored=0\n").is_err());
    }

    #[test]
    fn rejects_duplicate_inventory_paths() {
        let error = parse_inventory("eligible a.rs\nsupport a.rs\n").unwrap_err();
        assert_eq!(
            error,
            LcovError::DuplicateInventoryPath {
                lineno: 2,
                path: "a.rs".to_string(),
            }
        );
    }

    #[test]
    fn rejects_duplicate_paths_with_matching_dispositions() {
        let error = parse_inventory("eligible a.rs\neligible a.rs\n").unwrap_err();
        assert_eq!(
            error,
            LcovError::DuplicateInventoryPath {
                lineno: 2,
                path: "a.rs".to_string(),
            }
        );
    }

    #[test]
    fn accepts_repeated_paths_across_comment_gaps() {
        let inventory = parse_inventory("eligible a.rs\n# comment\neligible b.rs\n").unwrap();
        assert_eq!(inventory.len(), 2);
    }
}
