use std::collections::BTreeMap;
use std::fmt::Write as _;

use crate::LcovError;
use crate::xml::{attribute, tags};

/// Converts a Clover XML report (`PHPUnit`, Xdebug, pcov) to LCOV text.
///
/// Only `stmt` lines count. Files appear in path order, lines in line order.
pub fn clover_to_lcov(xml: &str) -> Result<String, LcovError> {
    let mut files: BTreeMap<String, BTreeMap<u32, u64>> = BTreeMap::new();
    let mut current: Option<String> = None;
    for tag in tags(xml) {
        let tag = tag.map_err(|end| clover_error(&format!("unterminated {}", end.0)))?;
        let attrs = tag.attrs;
        match tag.name {
            "file" => {
                let path = attribute(attrs, "name")
                    .filter(|path| !path.is_empty())
                    .ok_or_else(|| clover_error("file element without a name"))?;
                files.entry(path.clone()).or_default();
                current = Some(path);
            }
            "/file" => current = None,
            "line" if attribute(attrs, "type").as_deref() == Some("stmt") => {
                let path = current
                    .as_ref()
                    .ok_or_else(|| clover_error("line element outside a file"))?;
                let num = number(attrs, "num")?;
                let count = number(attrs, "count")?;
                let hits = files
                    .entry(path.clone())
                    .or_default()
                    .entry(num)
                    .or_insert(0);
                *hits = hits.saturating_add(count);
            }
            _ => {}
        }
    }
    if files.is_empty() {
        return Err(clover_error("no file elements"));
    }
    let mut out = String::new();
    for (path, lines) in &files {
        writeln!(out, "SF:{path}").ok();
        for (num, count) in lines {
            writeln!(out, "DA:{num},{count}").ok();
        }
        let hit = lines.values().filter(|count| **count > 0).count();
        writeln!(out, "LF:{}\nLH:{hit}\nend_of_record", lines.len()).ok();
    }
    Ok(out)
}

fn clover_error(what: &str) -> LcovError {
    LcovError::Io {
        message: format!("malformed Clover report: {what}"),
    }
}

fn number<T: std::str::FromStr>(attrs: &str, key: &str) -> Result<T, LcovError> {
    attribute(attrs, key)
        .and_then(|value| value.parse().ok())
        .ok_or_else(|| clover_error(&format!("line element without a numeric {key}")))
}

#[cfg(test)]
#[path = "clover_tests.rs"]
mod clover_tests;
