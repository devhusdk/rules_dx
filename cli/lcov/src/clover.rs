use std::collections::BTreeMap;
use std::fmt::Write as _;

use crate::LcovError;

/// Converts a Clover XML report (`PHPUnit`, Xdebug, pcov) to LCOV text.
///
/// Only `stmt` lines count. Files appear in path order, lines in line order.
pub fn clover_to_lcov(xml: &str) -> Result<String, LcovError> {
    let mut files: BTreeMap<String, BTreeMap<u32, u64>> = BTreeMap::new();
    let mut current: Option<String> = None;
    let mut rest = xml;
    while let Some((_, after)) = rest.split_once('<') {
        let (tag, tail) = after
            .split_once('>')
            .ok_or_else(|| clover_error("unterminated tag"))?;
        let tag = tag.trim_end_matches('/');
        rest = tail;
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

pub(crate) fn attribute(attrs: &str, key: &str) -> Option<String> {
    let mut rest = attrs;
    while let Some((name, tail)) = rest.split_once('=') {
        let name = name.trim();
        let mut quoted = tail.trim_start().chars();
        let quote = quoted.next().filter(|c| *c == '"' || *c == '\'')?;
        let (value, after) = quoted.as_str().split_once(quote)?;
        if name == key {
            return Some(unescape(value));
        }
        rest = after;
    }
    None
}

fn unescape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some((head, tail)) = rest.split_once('&') {
        out.push_str(head);
        let decoded = tail
            .split_once(';')
            .and_then(|(name, after)| entity(name).map(|ch| (ch, after)));
        if let Some((ch, after)) = decoded {
            out.push(ch);
            rest = after;
        } else {
            out.push('&');
            rest = tail;
        }
    }
    out.push_str(rest);
    out
}

fn entity(name: &str) -> Option<char> {
    match name {
        "lt" => Some('<'),
        "gt" => Some('>'),
        "quot" => Some('"'),
        "apos" => Some('\''),
        "amp" => Some('&'),
        _ => {
            let digits = name.strip_prefix('#')?;
            let code = match digits.strip_prefix(['x', 'X']) {
                Some(hex) => u32::from_str_radix(hex, 16).ok()?,
                None => digits.parse().ok()?,
            };
            char::from_u32(code)
        }
    }
}

#[cfg(test)]
#[path = "clover_tests.rs"]
mod clover_tests;
