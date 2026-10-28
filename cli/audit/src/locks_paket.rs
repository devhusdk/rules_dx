use crate::locks::locks_spec::split_paren_spec;
use crate::vuln::LockedPackage;

pub fn parse_paket_lock(text: &str) -> Result<Vec<LockedPackage>, String> {
    let mut out = Vec::new();
    let mut in_nuget = false;
    let mut in_git = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed == "NUGET" {
            in_nuget = true;
            in_git = false;
            continue;
        }
        if trimmed == "GIT" {
            in_nuget = false;
            in_git = true;
            continue;
        }
        if trimmed == "HTTP" || trimmed == "GITHUB" {
            in_nuget = false;
            in_git = false;
            continue;
        }
        if !in_nuget && !in_git {
            continue;
        }
        if trimmed.is_empty()
            || trimmed.starts_with("remote:")
            || trimmed.starts_with("GROUP")
            || trimmed.starts_with("RESTRICTION:")
        {
            continue;
        }
        if let Some((name, version)) = split_paket_line(trimmed) {
            out.push(LockedPackage {
                is_git: in_git,
                ..LockedPackage::new("nuget", name, version)
            });
        }
    }
    out.sort_by_cached_key(|package| {
        (
            crate::vuln::package_identity("nuget", &package.name),
            package.name.clone(),
            package.version.clone(),
        )
    });
    out.dedup_by(|b, a| {
        a.is_git == b.is_git
            && a.version == b.version
            && crate::vuln::package_identity("nuget", &a.name)
                == crate::vuln::package_identity("nuget", &b.name)
    });
    Ok(out)
}

pub(crate) fn split_paket_line(trimmed: &str) -> Option<(String, String)> {
    let (name, version) = split_paren_spec(trimmed)?;
    if name.contains("remote") {
        return None;
    }
    Some((name.to_owned(), version.to_owned()))
}
