use crate::vuln::LockedPackage;

#[derive(Clone, Copy, Eq, PartialEq)]
enum Source {
    Registry,
    Git,
    Path,
}

pub fn parse_gemfile_lock(text: &str) -> Result<Vec<LockedPackage>, String> {
    let mut source: Option<Source> = None;
    let mut in_specs = false;
    let mut saw_section = false;
    let mut out = Vec::new();
    for raw in text.lines() {
        if raw.trim().is_empty() {
            continue;
        }
        let indent = raw.len() - raw.trim_start().len();
        let trimmed = raw.trim();
        if indent == 0 {
            source = match trimmed {
                "GEM" => {
                    saw_section = true;
                    Some(Source::Registry)
                }
                "GIT" => {
                    saw_section = true;
                    Some(Source::Git)
                }
                "PATH" => {
                    saw_section = true;
                    Some(Source::Path)
                }
                _ => None,
            };
            in_specs = false;
            continue;
        }
        if source.is_none() {
            continue;
        }
        if indent == 2 {
            in_specs = trimmed == "specs:";
            continue;
        }
        if !in_specs || indent != 4 {
            continue;
        }
        let Some((name, version)) = split_gem_spec(trimmed) else {
            continue;
        };
        out.push(LockedPackage {
            name,
            version,
            set: "ruby".to_owned(),
            is_git: source == Some(Source::Git),
            is_private: source == Some(Source::Path),
        });
    }
    if !saw_section {
        return Err("invalid Gemfile.lock: missing GEM section".to_owned());
    }
    out.sort_by(|a, b| (&a.name, &a.version).cmp(&(&b.name, &b.version)));
    out.dedup_by(|b, a| a.name == b.name && a.version == b.version);
    Ok(out)
}

fn split_gem_spec(trimmed: &str) -> Option<(String, String)> {
    let open = trimmed.find('(')?;
    let close = trimmed.rfind(')')?;
    if close < open {
        return None;
    }
    let name = trimmed[..open].trim().to_owned();
    let version = trimmed[open + 1..close].trim().to_owned();
    if name.is_empty()
        || version.is_empty()
        || name.contains(char::is_whitespace)
        || version.contains(char::is_whitespace)
    {
        return None;
    }
    Some((name, version))
}
