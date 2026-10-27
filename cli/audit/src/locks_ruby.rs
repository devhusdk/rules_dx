use crate::locks::locks_spec::{indent_of, split_paren_spec};
use crate::vuln::{split_gem_lock_version, LockedPackage};

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
        let indent = indent_of(raw);
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
        let Some((name, locked)) = split_gem_spec(trimmed) else {
            continue;
        };
        let (version, platform) = split_gem_lock_version(&locked);
        let (version, platform) = (version.to_owned(), platform.map(str::to_owned));
        out.push(LockedPackage {
            is_git: source == Some(Source::Git),
            is_private: source == Some(Source::Path),
            platform,
            ..LockedPackage::new("ruby", name, version)
        });
    }
    if !saw_section {
        return Err("invalid Gemfile.lock: missing GEM section".to_owned());
    }
    out.sort_by(|a, b| (&a.name, &a.version, &a.platform).cmp(&(&b.name, &b.version, &b.platform)));
    out.dedup_by(|b, a| a.name == b.name && a.version == b.version && a.platform == b.platform);
    Ok(out)
}

fn split_gem_spec(trimmed: &str) -> Option<(String, String)> {
    let (name, version) = split_paren_spec(trimmed)?;
    if name.contains(char::is_whitespace) || version.contains(char::is_whitespace) {
        return None;
    }
    Some((name.to_owned(), version.to_owned()))
}
