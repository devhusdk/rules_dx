use std::collections::BTreeMap;
use std::num::NonZeroU32;

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
                if path.contains(['\n', '\r']) {
                    return Err(clover_error(&format!(
                        "file name with a line break: {path:?}"
                    )));
                }
                files.entry(path.clone()).or_default();
                current = (!tag.empty).then_some(path);
            }
            "/file" => current = None,
            "line" if attribute(attrs, "type").as_deref() == Some("stmt") => {
                let path = current
                    .as_ref()
                    .ok_or_else(|| clover_error("line element outside a file"))?;
                let num = number::<NonZeroU32>(attrs, "num")?.get();
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
        out.push_str(&format!("SF:{path}\n"));
        for (num, count) in lines {
            out.push_str(&format!("DA:{num},{count}\n"));
        }
        let hit = lines.values().filter(|count| **count > 0).count();
        out.push_str(&format!("LF:{}\nLH:{hit}\nend_of_record\n", lines.len()));
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
        .ok_or_else(|| clover_error(&format!("line element without a valid {key}")))
}

#[cfg(test)]
#[path = "clover_tests.rs"]
mod clover_tests;
