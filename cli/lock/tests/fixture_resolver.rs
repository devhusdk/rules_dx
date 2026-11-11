//! A resolver for the lock test: resolves `fixture.json` against a registry index file.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use serde_json::{Value, json};

fn names(value: &Value) -> Vec<String> {
    value
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|name| name.as_str().map(str::to_owned))
        .collect()
}

fn closure(index: &Value, roots: &[String]) -> Result<BTreeSet<String>, String> {
    let mut found = BTreeSet::new();
    let mut pending = roots.to_vec();
    while let Some(name) = pending.pop() {
        let entry = index["packages"]
            .get(&name)
            .ok_or(format!("{name} is not in the registry"))?;
        if found.insert(name) {
            pending.extend(names(&entry["deps"]));
        }
    }
    Ok(found)
}

/// The scratch directory a variable points at, when it is a directory beside the working directory.
fn scratch_dir(variable: &str, cwd: &Path) -> Result<PathBuf, String> {
    let dir = PathBuf::from(std::env::var(variable).map_err(|_| format!("{variable} is not set"))?);
    if dir.is_dir() && dir.parent() == cwd.parent() {
        Ok(dir)
    } else {
        Err(format!(
            "{variable} is {}, outside the scratch directory",
            dir.display()
        ))
    }
}

fn resolve(index_path: &str) -> Result<String, String> {
    let cwd = std::env::current_dir().map_err(|error| error.to_string())?;
    let home = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    for variable in [home, "XDG_CACHE_HOME", "XDG_CONFIG_HOME", "TMPDIR"] {
        scratch_dir(variable, &cwd)?;
    }
    let read = |path: &Path| -> Result<Value, String> {
        let text = std::fs::read_to_string(path)
            .map_err(|error| format!("{}: {error}", path.display()))?;
        serde_json::from_str(&text).map_err(|error| format!("{}: {error}", path.display()))
    };
    let index = read(Path::new(index_path))?;
    let manifest = read(&cwd.join("fixture.json"))?;
    let required = closure(&index, &names(&manifest["require"]))?;
    let dev = closure(&index, &names(&manifest["dev"]))?;
    let mut packages = BTreeMap::new();
    for name in dev.union(&required).collect::<Vec<_>>().into_iter().rev() {
        let entry = &index["packages"][name];
        let archive = entry["archive"].as_str().unwrap_or_default();
        packages.insert(
            name.clone(),
            json!({
                "name": name,
                "version": entry["version"],
                "archives": [{
                    "urls": [format!("https://registry.example/{archive}")],
                    "sha256": entry["sha256"].as_str().unwrap_or_default(),
                }],
                "deps": entry["deps"],
                "provides": entry["provides"],
                "dev": !required.contains(name),
            }),
        );
    }
    let state: String = packages
        .iter()
        .map(|(name, package)| {
            format!(
                "{name} {}\n",
                package["version"].as_str().unwrap_or_default()
            )
        })
        .collect();
    std::fs::write(cwd.join("fixture.state"), state).map_err(|error| error.to_string())?;
    let listed: Vec<&Value> = packages.values().rev().collect();
    Ok(json!({ "packages": listed }).to_string())
}

fn main() -> ExitCode {
    let index = std::env::args().nth(1).unwrap_or_default();
    match resolve(&index) {
        Ok(resolution) => {
            println!("{resolution}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("fixture_resolver: {error}");
            ExitCode::FAILURE
        }
    }
}
