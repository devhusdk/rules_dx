use std::collections::BTreeMap;

use crate::LcovError;

/// Converts a Clover XML report (PHPUnit, Xdebug, pcov) to LCOV text.
///
/// Only `stmt` lines count. Files appear in path order, lines in line order.
pub fn clover_to_lcov(xml: &str) -> Result<String, LcovError> {
    let mut files: BTreeMap<String, BTreeMap<u32, u64>> = BTreeMap::new();
    let mut current: Option<String> = None;
    let mut rest = xml;
    while let Some(start) = rest.find('<') {
        let after = &rest[start + 1..];
        let end = after
            .find('>')
            .ok_or_else(|| clover_error("unterminated tag"))?;
        let tag = after[..end].trim_end_matches('/');
        rest = &after[end + 1..];
        let (name, attrs) = tag.split_once(char::is_whitespace).unwrap_or((tag, ""));
        match name {
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
        .ok_or_else(|| clover_error(&format!("line element without a numeric {key}")))
}

pub(crate) fn attribute(attrs: &str, key: &str) -> Option<String> {
    let mut rest = attrs;
    while let Some(eq) = rest.find('=') {
        let name = rest[..eq].trim();
        let quoted = rest[eq + 1..].trim_start();
        let quote = quoted.chars().next().filter(|c| *c == '"' || *c == '\'')?;
        let body = &quoted[1..];
        let close = body.find(quote)?;
        if name == key {
            return Some(unescape(&body[..close]));
        }
        rest = &body[close + 1..];
    }
    None
}

fn unescape(text: &str) -> String {
    text.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

#[cfg(test)]
#[path = "clover_tests.rs"]
mod clover_tests;
