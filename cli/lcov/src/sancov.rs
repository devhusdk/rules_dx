use std::collections::{BTreeMap, HashMap};

use crate::LcovError;

pub fn sancov_to_lcov(pcs: &str, symbolized: &str) -> Result<String, LcovError> {
    let mut ran: HashMap<u64, bool> = HashMap::new();
    for line in pcs.lines().filter(|line| !line.trim().is_empty()) {
        let mut fields = line.split_whitespace();
        let address = fields.next().and_then(parse_address);
        let hit = match fields.next() {
            Some("0") => Some(false),
            Some("1") => Some(true),
            _ => None,
        };
        match (address, hit, fields.next()) {
            (Some(address), Some(hit), None) => {
                let seen = ran.entry(address).or_insert(false);
                *seen |= hit;
            }
            _ => return Err(sancov_error(&format!("bad PC line {line:?}"))),
        }
    }
    let mut files: BTreeMap<String, BTreeMap<u32, u64>> = BTreeMap::new();
    for block in symbolized.replace("\r\n", "\n").split("\n\n") {
        let mut lines = block.lines().filter(|line| !line.is_empty());
        let Some(first) = lines.next() else {
            continue;
        };
        let address = parse_address(first)
            .ok_or_else(|| sancov_error(&format!("block starts without an address: {first:?}")))?;
        let hit = *ran
            .get(&address)
            .ok_or_else(|| sancov_error(&format!("symbolized address {first} is not a PC")))?;
        let frames: Vec<&str> = lines.collect();
        if !frames.len().is_multiple_of(2) {
            return Err(sancov_error(&format!("odd frame lines for {first}")));
        }
        for location in frames.iter().skip(1).step_by(2) {
            let Some((path, number)) = file_line(location) else {
                return Err(sancov_error(&format!("bad location {location:?}")));
            };
            if path.is_empty() || path == "??" || number == 0 {
                continue;
            }
            let count = files
                .entry(path.to_string())
                .or_default()
                .entry(number)
                .or_insert(0);
            *count = (*count).max(u64::from(hit));
        }
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

fn parse_address(text: &str) -> Option<u64> {
    let digits = text.strip_prefix("0x")?;
    u64::from_str_radix(digits, 16).ok()
}

fn file_line(location: &str) -> Option<(&str, u32)> {
    let mut parts = location.rsplitn(3, ':');
    let _column = parts.next()?;
    let line = parts.next()?.parse().ok()?;
    Some((parts.next()?, line))
}

fn sancov_error(what: &str) -> LcovError {
    LcovError::Io {
        message: format!("malformed sancov input: {what}"),
    }
}

#[cfg(test)]
#[path = "sancov_tests.rs"]
mod sancov_tests;
