use crate::vuln::LockedPackage;
use dx_gomod::GoReplacementTarget;

#[path = "locks_bazel.rs"]
mod locks_bazel;
#[path = "locks_cabal.rs"]
mod locks_cabal;
#[path = "locks_cargo.rs"]
mod locks_cargo;
#[path = "locks_composer.rs"]
mod locks_composer;
#[path = "locks_conan.rs"]
mod locks_conan;
#[path = "locks_cpan.rs"]
mod locks_cpan;
#[path = "locks_dub.rs"]
mod locks_dub;
#[path = "locks_hex.rs"]
mod locks_hex;
#[path = "locks_julia.rs"]
mod locks_julia;
#[path = "locks_licenses.rs"]
mod locks_licenses;
#[path = "locks_luarocks.rs"]
mod locks_luarocks;
#[path = "locks_npm.rs"]
mod locks_npm;
#[path = "locks_opam.rs"]
mod locks_opam;
#[path = "locks_paket.rs"]
mod locks_paket;
#[path = "locks_pnpm.rs"]
mod locks_pnpm;
#[path = "locks_pub.rs"]
mod locks_pub;
#[path = "locks_ruby.rs"]
mod locks_ruby;
#[path = "locks_swift.rs"]
mod locks_swift;
#[path = "locks_uv.rs"]
mod locks_uv;

pub use locks_bazel::*;
pub use locks_cabal::*;
pub use locks_cargo::*;
pub use locks_composer::*;
pub use locks_conan::*;
pub use locks_cpan::*;
pub use locks_dub::*;
pub use locks_hex::*;
pub use locks_julia::*;
pub use locks_licenses::*;
pub use locks_luarocks::*;
pub use locks_npm::*;
pub use locks_opam::*;
pub use locks_paket::*;
pub use locks_pnpm::*;
pub use locks_pub::*;
pub use locks_ruby::*;
pub use locks_swift::*;
pub use locks_uv::*;

/// Reads the Go packages a `go.mod` pins, with its `replace` directives applied.
pub fn go_locked_packages(text: &str) -> Result<Vec<LockedPackage>, String> {
    let file = dx_gomod::parse_mod(text)?;
    if file.module.is_empty() {
        return Err("invalid go.mod: missing module directive".to_owned());
    }
    let mut out = Vec::new();
    for requirement in &file.require {
        match file
            .replacement_for(&requirement.module, &requirement.version)
            .map(|replacement| &replacement.target)
        {
            Some(GoReplacementTarget::FilePath(_)) => continue,
            Some(GoReplacementTarget::Module { module, version }) => {
                out.push(LockedPackage::new("go", module, version))
            }
            None => out.push(LockedPackage::new(
                "go",
                &requirement.module,
                &requirement.version,
            )),
        }
    }
    out.sort_by(|a, b| (&a.name, &a.version).cmp(&(&b.name, &b.version)));
    out.dedup_by(|b, a| a.name == b.name && a.version == b.version);
    Ok(out)
}

pub fn parse_maven_install(text: &str) -> Result<Vec<LockedPackage>, String> {
    let value: serde_json::Value = serde_json::from_str(text)
        .map_err(|error| format!("invalid maven_install.json: {error}"))?;
    let artifacts = value
        .get("artifacts")
        .and_then(|value| value.as_object())
        .ok_or_else(|| "invalid maven_install.json: missing artifacts".to_owned())?;
    let mut out = Vec::new();
    for (key, detail) in artifacts {
        let version = detail
            .get("version")
            .and_then(|value| value.as_str())
            .unwrap_or("")
            .trim();
        if key.trim().is_empty() || version.is_empty() {
            continue;
        }
        out.push(LockedPackage::new("maven", key.as_str(), version));
    }
    out.sort_by(|a, b| (&a.name, &a.version).cmp(&(&b.name, &b.version)));
    Ok(out)
}

pub fn parse_psgallery_lock(text: &str) -> Result<Vec<LockedPackage>, String> {
    let value: serde_json::Value = serde_json::from_str(text)
        .map_err(|error| format!("invalid PSGallery.lock.json: {error}"))?;
    let modules = value
        .get("modules")
        .and_then(|value| value.as_object())
        .ok_or_else(|| "invalid PSGallery.lock.json: missing modules".to_owned())?;
    let mut out = Vec::new();
    for (name, detail) in modules {
        let name = name.trim();
        let version = detail
            .get("version")
            .and_then(|value| value.as_str())
            .unwrap_or("")
            .trim();
        if name.is_empty() || version.is_empty() {
            return Err("invalid PSGallery.lock.json: module without a version".to_owned());
        }
        out.push(LockedPackage::new("powershell", name, version));
    }
    out.sort_by(|a, b| (&a.name, &a.version).cmp(&(&b.name, &b.version)));
    Ok(out)
}

/// Number of leading whitespace bytes of `line`.
pub(crate) fn indent_of(line: &str) -> usize {
    line.len() - line.trim_start().len()
}

/// Parses `name (version)` where `version` has no parentheses.
pub(crate) fn split_paren_spec(line: &str) -> Option<(&str, &str)> {
    for (open, _) in line.rmatch_indices('(') {
        let rest = &line[open + 1..];
        let Some(close) = rest.find(['(', ')']) else {
            continue;
        };
        if close == 0 || !rest[close..].starts_with(')') {
            continue;
        }
        let name = line[..open].trim();
        let version = rest[..close].trim();
        if name.is_empty() || version.is_empty() {
            return None;
        }
        return Some((name, version));
    }
    None
}

#[cfg(test)]
pub(crate) use locks_npm::package_lock_name;
#[cfg(test)]
pub(crate) use locks_paket::split_paket_line;
#[cfg(test)]
pub(crate) use locks_pnpm::{split_pnpm_scoped_fallback, split_pnpm_unscoped_fallback};

#[cfg(test)]
#[path = "locks_tests.rs"]
mod locks_tests;
