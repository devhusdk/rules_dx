use crate::vuln::LockedPackage;
use dx_gomod::GoReplacementTarget;

/// Reads the Go packages a `go.mod` pins, with its `replace` directives applied.
pub fn go_locked_packages(text: &str) -> Result<Vec<LockedPackage>, String> {
    let file = dx_gomod::parse_mod(text)?;
    if file.module.is_empty() {
        return Err("invalid go.mod: missing module directive".to_owned());
    }
    let mut out = Vec::new();
    for requirement in &file.require {
        match file
            .replace
            .iter()
            .rev()
            .find(|replacement| replacement.module == requirement.module)
            .map(|replacement| &replacement.target)
        {
            Some(GoReplacementTarget::FilePath(_)) => continue,
            Some(GoReplacementTarget::Module { module, version }) => {
                out.push(locked(module, version))
            }
            None => out.push(locked(&requirement.module, &requirement.version)),
        }
    }
    out.sort_by(|a, b| (&a.name, &a.version).cmp(&(&b.name, &b.version)));
    out.dedup_by(|b, a| a.name == b.name && a.version == b.version);
    Ok(out)
}

fn locked(name: &str, version: &str) -> LockedPackage {
    LockedPackage {
        name: name.to_owned(),
        version: version.to_owned(),
        set: "go".to_owned(),
        is_git: false,
        is_private: false,
    }
}
