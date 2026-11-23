use std::collections::BTreeMap;
use std::num::NonZeroU32;

use crate::LcovError;
use crate::xml::{attribute, tags, unescape};

pub fn cobertura_to_lcov(xml: &str) -> Result<String, LcovError> {
    let mut files: BTreeMap<String, BTreeMap<u32, u64>> = BTreeMap::new();
    let mut current: Option<String> = None;
    let mut in_methods = false;
    let mut source: Option<String> = None;
    for tag in tags(xml) {
        let tag = tag.map_err(|end| cobertura_error(&format!("unterminated {}", end.0)))?;
        let attrs = tag.attrs;
        match tag.name {
            "source" if source.is_none() => {
                let text = tag.tail.split('<').next().unwrap_or(tag.tail);
                source = Some(unescape(text.trim().trim_end_matches(['/', '\\'])));
            }
            "class" => {
                let path = attribute(attrs, "filename")
                    .filter(|path| !path.is_empty())
                    .ok_or_else(|| cobertura_error("class element without a filename"))?;
                let path = match &source {
                    Some(dir)
                        if !dx_path::is_absolute(&path)
                            && !path.starts_with('\\')
                            && !dir.is_empty() =>
                    {
                        format!("{dir}/{path}")
                    }
                    _ => path,
                };
                if path.contains(['\n', '\r']) {
                    return Err(cobertura_error(&format!(
                        "filename with a line break: {path:?}"
                    )));
                }
                files.entry(path.clone()).or_default();
                current = (!tag.empty).then_some(path);
            }
            "/class" => current = None,
            "methods" => in_methods = !tag.empty,
            "/methods" => in_methods = false,
            "line" if !in_methods => {
                let path = current
                    .as_ref()
                    .ok_or_else(|| cobertura_error("line element outside a class"))?;
                let number = numeric::<NonZeroU32>(attrs, "number")?.get();
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
        .ok_or_else(|| cobertura_error(&format!("line element without a valid {key}")))
}

fn cobertura_error(what: &str) -> LcovError {
    LcovError::Io {
        message: format!("malformed Cobertura report: {what}"),
    }
}

#[cfg(test)]
#[path = "cobertura_tests.rs"]
mod cobertura_tests;
