//! Checks that every link, anchor and API page a docs site names really exists.

use std::collections::BTreeSet;
use std::path::Path;

/// The required and repeated inputs for one aggregate.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Inputs {
    pub book: Option<String>,
    pub api: Option<String>,
    pub shards: Vec<String>,
    pub prose: Vec<String>,
    pub data: Vec<String>,
}

/// The usage line shown for a rejected invocation.
pub fn usage() -> String {
    let mut line = String::from("usage: site_check --book FILE --api FILE --shard FILE");
    line.push_str(" [--shard FILE] [--prose FILE] [--data FILE]");
    line
}

/// Parses argv into inputs, or returns the usage rejection text.
pub fn parse_args(args: &[String]) -> Result<Inputs, (String, String)> {
    let mut inputs = Inputs::default();
    let mut index = 0;
    while index < args.len() {
        let flag = args[index].as_str();
        match flag {
            "--book" | "--api" | "--shard" | "--prose" | "--data" => {
                let Some(value) = args.get(index + 1) else {
                    return Err((format!("{flag} needs a FILE"), usage()));
                };
                match flag {
                    "--book" => inputs.book = Some(value.clone()),
                    "--api" => inputs.api = Some(value.clone()),
                    "--shard" => inputs.shards.push(value.clone()),
                    "--prose" => inputs.prose.push(value.clone()),
                    _ => inputs.data.push(value.clone()),
                }
            }
            _ => {
                return Err((format!("unknown argument '{flag}'"), usage()));
            }
        }
        index += 2;
    }
    if inputs.book.is_none() {
        return Err(("--book FILE is required".to_string(), usage()));
    }
    if inputs.api.is_none() {
        return Err(("--api FILE is required".to_string(), usage()));
    }
    if inputs.shards.is_empty() {
        return Err(("--shard FILE is required".to_string(), usage()));
    }
    if inputs.prose.is_empty() {
        return Err(("--prose FILE is required".to_string(), usage()));
    }
    Ok(inputs)
}

/// Returns the check refusal text, or an empty string when the site holds together.
pub fn check(inputs: &Inputs) -> String {
    let book_text = read(inputs.book.as_deref().unwrap_or_default());
    let api = inputs.api.as_deref().unwrap_or_default();
    let prose_texts: Vec<String> = inputs.prose.iter().map(|p| read(p)).collect();
    let api_text = read(api);
    let pages = || prose_texts.iter().map(String::as_str).collect::<Vec<_>>();
    let paths = || inputs.prose.iter().map(String::as_str).collect::<Vec<_>>();

    if !any_line(&book_text, |line| line.starts_with("title")) {
        return "book has no title".to_string();
    }
    for (path, text) in inputs.prose.iter().zip(&prose_texts) {
        if !any_line(text, |line| line.starts_with("# ")) {
            return format!("prose missing title '{path}'");
        }
    }

    let mut ids = BTreeSet::new();
    for shard in &inputs.shards {
        for line in read(shard).lines() {
            if let Some(rest) = line.strip_prefix("  id: \"") {
                ids.insert(rest.strip_suffix('"').unwrap_or(rest).to_string());
            }
        }
    }
    if ids.iter().all(String::is_empty) {
        return "shard names no symbols".to_string();
    }
    for id in &ids {
        if !api_text.contains(id.as_str()) {
            return format!("missing API page for {id}");
        }
    }

    if pages().iter().any(|text| has_empty_link_target(text)) {
        return "empty link target".to_string();
    }

    let anchors = anchors(pages(), &api_text);
    let api_paths: BTreeSet<String> = ids
        .iter()
        .map(|id| format!("api/{}.md", id.replace(':', "/")))
        .collect();
    let prose_bases = bases(&inputs.prose);
    let data_bases = bases(&inputs.data);
    let known: Vec<&str> = paths()
        .into_iter()
        .chain(inputs.data.iter().map(String::as_str))
        .chain(api_paths.iter().map(String::as_str))
        .collect();

    for target in link_targets(pages()) {
        if target.contains("://") || target.starts_with("mailto:") {
            continue;
        }
        if let Some(fragment) = target.strip_prefix('#') {
            if !anchors.contains(fragment) {
                return format!("dangling anchor '{target}'");
            }
            continue;
        }
        let (base, fragment) = split_fragment(&target);
        let (base, directory) = match base.strip_suffix('/') {
            Some(trimmed) => (trimmed.to_string(), true),
            None => (base.to_string(), false),
        };
        let name = normalize_link(&base);
        let found = if directory {
            paths()
                .iter()
                .any(|p| ends_with(p, &format!("{name}/README.md")))
        } else if known.iter().any(|p| ends_with(p, &name)) {
            true
        } else if !name.contains('/') {
            prose_bases.contains(name.as_str()) || data_bases.contains(name.as_str())
        } else {
            false
        } || name == "api.md"
            || name == "SUMMARY.md";
        if !found {
            return format!("dangling link '{target}'");
        }
        if !fragment.is_empty() {
            let normalized = fragment.to_lowercase();
            if !anchors.contains(&normalized) {
                return format!("dangling fragment '{target}'");
            }
        }
    }
    String::new()
}

fn read(path: &str) -> String {
    std::fs::read_to_string(Path::new(path)).unwrap_or_default()
}

fn any_line(text: &str, wanted: impl Fn(&str) -> bool) -> bool {
    text.lines().any(wanted)
}

fn has_empty_link_target(text: &str) -> bool {
    let bytes = text.as_bytes();
    for index in 0..bytes.len() {
        if bytes[index] != b']' || index + 2 >= bytes.len() {
            continue;
        }
        if bytes[index + 1] != b'(' || bytes[index + 2] != b')' {
            continue;
        }
        let mut back = index;
        while back > 0 {
            back -= 1;
            match bytes[back] {
                b']' => break,
                b'[' => return true,
                _ => {}
            }
        }
    }
    false
}

fn anchors(prose: Vec<&str>, api: &str) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    for text in prose {
        for line in text.lines().filter(|l| l.starts_with('#')) {
            found.insert(anchor_name(line));
        }
    }
    for line in api.lines().filter(|l| l.starts_with("## ")) {
        found.insert(anchor_name(line));
    }
    found
}

fn anchor_name(line: &str) -> String {
    let heading = line.trim_start_matches('#').trim_start();
    let lowered = heading.to_lowercase();
    let cleaned: String = lowered
        .chars()
        .filter(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == ' ' || *c == '-')
        .collect();
    let trimmed = cleaned.trim();
    let dashed = trimmed.replace(' ', "-");
    let mut collapsed = String::new();
    let mut dash = false;
    for c in dashed.chars() {
        if c == '-' {
            if !dash {
                collapsed.push(c);
            }
            dash = true;
        } else {
            collapsed.push(c);
            dash = false;
        }
    }
    collapsed
}

fn bases(paths: &[String]) -> BTreeSet<String> {
    paths
        .iter()
        .map(|p| {
            Path::new(p)
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default()
        })
        .collect()
}

fn link_targets(prose: Vec<&str>) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    for text in prose {
        for line in text.lines() {
            for inner in inline_targets(line) {
                let tidied = tidy(&inner);
                found.insert(first_field(tidied.as_str()));
            }
            if let Some((_, rest)) = split_reference(line) {
                found.insert(first_field(&trim_wrapping(rest.trim_start())));
            }
        }
    }
    found
}

fn inline_targets(line: &str) -> Vec<String> {
    let bytes = line.as_bytes();
    let mut found = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] != b']' || index + 1 >= bytes.len() || bytes[index + 1] != b'(' {
            index += 1;
            continue;
        }
        let mut back = index;
        let mut opened = false;
        while back > 0 {
            back -= 1;
            match bytes[back] {
                b']' => break,
                b'[' => {
                    opened = true;
                    break;
                }
                _ => {}
            }
        }
        if !opened {
            index += 1;
            continue;
        }
        let start = index + 2;
        match line[start..].find(')') {
            Some(offset) => {
                found.push(line[start..start + offset].to_string());
                index = start + offset;
            }
            None => break,
        }
    }
    found
}

fn split_reference(line: &str) -> Option<(usize, &str)> {
    let trimmed = line.trim_start();
    if !trimmed.starts_with('[') {
        return None;
    }
    let close = trimmed.find(']')?;
    if !trimmed[close + 1..].starts_with(':') {
        return None;
    }
    Some((close + 1, &trimmed[close + 2..]))
}

fn tidy(value: &str) -> String {
    let trimmed = value.trim();
    let unbracketed = trimmed.strip_prefix('<').unwrap_or(trimmed);
    let unbracketed = unbracketed.strip_suffix('>').unwrap_or(unbracketed);
    if unbracketed.starts_with('"') {
        return String::new();
    }
    let cut = match unbracketed.find('"') {
        Some(at) => &unbracketed[..at],
        None => unbracketed,
    };
    cut.trim().to_string()
}

fn trim_wrapping(value: &str) -> String {
    let unbracketed = value.strip_prefix('<').unwrap_or(value);
    let unbracketed = unbracketed.strip_suffix('>').unwrap_or(unbracketed);
    let unquoted = unbracketed.strip_prefix('"').unwrap_or(unbracketed);
    unquoted.strip_suffix('"').unwrap_or(unquoted).to_string()
}

fn first_field(value: &str) -> String {
    value.split(' ').next().unwrap_or("").to_string()
}

fn split_fragment(target: &str) -> (&str, &str) {
    match target.split_once('#') {
        Some((base, fragment)) => (base, fragment),
        None => (target, ""),
    }
}

fn normalize_link(base: &str) -> String {
    let mut name = base.to_string();
    while let Some(rest) = name.strip_prefix("../") {
        name = rest.to_string();
    }
    name.strip_prefix("./").unwrap_or(&name).to_string()
}

fn ends_with(candidate: &str, name: &str) -> bool {
    candidate == name
        || (candidate.len() > name.len()
            && candidate.ends_with(name)
            && candidate.as_bytes()[candidate.len() - name.len() - 1] == b'/')
}
