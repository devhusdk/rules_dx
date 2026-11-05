use std::collections::BTreeMap;

use crate::clover::attribute;
use crate::LcovError;

/// Converts a Cobertura XML report (kcov, coverage.py) to LCOV text.
///
/// Only class-level `line` elements count; lines listed under `methods` repeat them.
/// Files appear in path order, lines in line order.
pub fn cobertura_to_lcov(xml: &str) -> Result<String, LcovError> {
    let mut files: BTreeMap<String, BTreeMap<u32, u64>> = BTreeMap::new();
    let mut current: Option<String> = None;
    let mut in_methods = false;
    let mut rest = xml;
    while let Some(start) = rest.find('<') {
        let after = &rest[start + 1..];
        let end = after
            .find('>')
            .ok_or_else(|| cobertura_error("unterminated tag"))?;
        let tag = after[..end].trim_end_matches('/');
        rest = &after[end + 1..];
        let (name, attrs) = tag.split_once(char::is_whitespace).unwrap_or((tag, ""));
        match name {
            "class" => {
                let path = attribute(attrs, "filename")
                    .filter(|path| !path.is_empty())
                    .ok_or_else(|| cobertura_error("class element without a filename"))?;
                files.entry(path.clone()).or_default();
                current = Some(path);
            }
            "/class" => current = None,
            "methods" => in_methods = true,
            "/methods" => in_methods = false,
            "line" if !in_methods => {
                let path = current
                    .as_ref()
                    .ok_or_else(|| cobertura_error("line element outside a class"))?;
                let number = numeric(attrs, "number")?;
                let hits = numeric(attrs, "hits")?;
                let total = files
                    .entry(path.clone())
                    .or_default()
                    .entry(number)
                    .or_insert(0);
                *total = total.saturating_add(hits);
            }
            _ => {}
        }
    }
    if files.is_empty() {
        return Err(cobertura_error("no class elements"));
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

fn numeric<T: std::str::FromStr>(attrs: &str, key: &str) -> Result<T, LcovError> {
    attribute(attrs, key)
        .and_then(|value| value.parse().ok())
        .ok_or_else(|| cobertura_error(&format!("line element without a numeric {key}")))
}

fn cobertura_error(what: &str) -> LcovError {
    LcovError::Io {
        message: format!("malformed Cobertura report: {what}"),
    }
}

#[cfg(test)]
#[path = "cobertura_tests.rs"]
mod cobertura_tests;
