use crate::vuln::LockedPackage;

pub fn strip_go_comment(line: &str) -> &str {
    let bytes = line.as_bytes();
    let mut index = 0usize;
    while index + 1 < bytes.len() {
        if bytes[index] == b'/' && bytes[index + 1] == b'/' {
            if index == 0 || bytes[index - 1].is_ascii_whitespace() {
                return line[..index].trim_end();
            }
            index += 2;
            continue;
        }
        index += 1;
    }
    line
}

pub fn split_go_replace_rhs(rhs: &str) -> Option<(String, Option<String>)> {
    let mut tokens = rhs.split_whitespace();
    let path = tokens.next()?.trim().to_owned();
    if path.is_empty() {
        return None;
    }
    let version = tokens.next().map(str::trim).filter(|v| !v.is_empty());
    Some((path, version.map(str::to_owned)))
}

pub fn parse_go_mod(text: &str) -> Result<Vec<LockedPackage>, String> {
    let mut has_module = false;
    let mut in_require = false;
    let mut in_replace = false;
    let mut in_skip_block = false;
    let mut requires: Vec<(String, String)> = Vec::new();
    let mut replaces: std::collections::BTreeMap<String, (String, Option<String>)> =
        std::collections::BTreeMap::new();
    for raw in text.lines() {
        let line = strip_go_comment(raw).trim().to_owned();
        if line.is_empty() {
            continue;
        }
        if in_skip_block {
            if line == ")" {
                in_skip_block = false;
            }
            continue;
        }
        if in_require {
            if line == ")" {
                in_require = false;
                continue;
            }
            let mut tokens = line.split_whitespace();
            if let (Some(name), Some(version)) = (tokens.next(), tokens.next()) {
                if !name.is_empty() && !version.is_empty() {
                    requires.push((name.to_owned(), version.to_owned()));
                }
            }
            continue;
        }
        if in_replace {
            if line == ")" {
                in_replace = false;
                continue;
            }
            if let Some((lhs, rhs)) = line.split_once("=>") {
                let old_path = lhs.split_whitespace().next().unwrap_or("").trim();
                if let Some((new_path, new_version)) = split_go_replace_rhs(rhs.trim()) {
                    if !old_path.is_empty() {
                        replaces.insert(old_path.to_owned(), (new_path, new_version));
                    }
                }
            }
            continue;
        }
        if line == ")" {
            return Err("invalid go.mod: unexpected closing paren".to_owned());
        }
        if line.starts_with("module ") || line == "module" {
            has_module = true;
            continue;
        }
        if line == "require (" || line.starts_with("require (") {
            in_require = true;
            continue;
        }
        if line.starts_with("require ") || line.starts_with("require\t") {
            let rest = line["require".len()..].trim();
            let mut tokens = rest.split_whitespace();
            if let (Some(name), Some(version)) = (tokens.next(), tokens.next()) {
                if !name.is_empty() && !version.is_empty() {
                    requires.push((name.to_owned(), version.to_owned()));
                }
            }
            continue;
        }
        if line == "replace (" || line.starts_with("replace (") {
            in_replace = true;
            continue;
        }
        if line.starts_with("replace ") || line.starts_with("replace\t") {
            let rest = line["replace".len()..].trim();
            if let Some((lhs, rhs)) = rest.split_once("=>") {
                let old_path = lhs.split_whitespace().next().unwrap_or("").trim();
                if let Some((new_path, new_version)) = split_go_replace_rhs(rhs.trim()) {
                    if !old_path.is_empty() {
                        replaces.insert(old_path.to_owned(), (new_path, new_version));
                    }
                }
            }
            continue;
        }
        if line == "exclude ("
            || line.starts_with("exclude (")
            || line == "retract ("
            || line.starts_with("retract (")
        {
            in_skip_block = true;
            continue;
        }
    }
    if !has_module {
        return Err("invalid go.mod: missing module directive".to_owned());
    }
    let mut out = Vec::new();
    for (name, version) in requires {
        match replaces.get(&name) {
            Some((_, None)) => continue,
            Some((new_path, Some(new_version))) => out.push(LockedPackage {
                name: new_path.clone(),
                version: new_version.clone(),
                set: "go".to_owned(),
                is_git: false,
                is_private: false,
            }),
            None => out.push(LockedPackage {
                name,
                version,
                set: "go".to_owned(),
                is_git: false,
                is_private: false,
            }),
        }
    }
    out.sort_by(|a, b| (&a.name, &a.version).cmp(&(&b.name, &b.version)));
    out.dedup_by(|b, a| a.name == b.name && a.version == b.version);
    Ok(out)
}
