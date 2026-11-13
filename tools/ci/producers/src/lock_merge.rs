//! Merges the per-platform `MODULE.bazel.lock` files of `lock-refresh.yml` into one lock.
//! Each platform lock contributes only the module extension entries keyed by its own factor.

use serde_json::{Map, Value};
use std::path::Path;

const PLATFORMS: [(&str, &str); 5] = [
    ("linux_x86_64", "os:linux,arch:amd64"),
    ("linux_arm64", "os:linux,arch:aarch64"),
    ("macos_arm64", "os:osx,arch:aarch64"),
    ("windows_x86_64", "os:windows,arch:amd64"),
    ("windows_arm64", "os:windows,arch:aarch64"),
];

fn read_lock(path: &Path) -> Result<Value, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|error| format!("lock_merge: missing {}: {error}", path.display()))?;
    serde_json::from_str(&text)
        .map_err(|error| format!("lock_merge: {} is not JSON: {error}", path.display()))
}

fn deep_merge(target: &mut Value, source: &Value) {
    match (target, source) {
        (Value::Object(target), Value::Object(source)) => {
            for (key, value) in source {
                match target.get_mut(key) {
                    Some(existing) => deep_merge(existing, value),
                    None => {
                        target.insert(key.clone(), value.clone());
                    }
                }
            }
        }
        (target, source) => *target = source.clone(),
    }
}

fn check_directories(locks: &Path) -> Result<(), String> {
    let entries = std::fs::read_dir(locks)
        .map_err(|error| format!("lock_merge: cannot read {}: {error}", locks.display()))?;
    for entry in entries {
        let entry = entry.map_err(|error| error.to_string())?;
        if !entry.path().is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        let known = name
            .strip_prefix("lock-")
            .is_some_and(|platform| PLATFORMS.iter().any(|(known, _)| *known == platform));
        if !known {
            return Err(format!("lock_merge: unknown platform directory {name}"));
        }
    }
    Ok(())
}

fn check_keys(platform: &str, lock: &Value) -> Result<(), String> {
    let extensions = lock.get("moduleExtensions").and_then(Value::as_object);
    let mut unknown: Vec<&str> = extensions
        .into_iter()
        .flat_map(Map::values)
        .filter_map(Value::as_object)
        .flat_map(Map::keys)
        .map(String::as_str)
        .filter(|key| *key != "general" && PLATFORMS.iter().all(|(_, factor)| factor != key))
        .collect();
    unknown.sort_unstable();
    unknown.dedup();
    if unknown.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "lock_merge: lock-{platform} has extension keys that are not general or a platform factor: {}",
            unknown.join(" ")
        ))
    }
}

pub fn merge(args: &[String]) -> Result<(), String> {
    let [locks, out] = args else {
        return Err("usage: producers_bin --lock-merge <locks_dir> <out>".to_owned());
    };
    let locks = Path::new(locks);
    check_directories(locks)?;
    let mut loaded = Vec::new();
    for (platform, factor) in PLATFORMS {
        let lock = read_lock(&locks.join(format!("lock-{platform}")).join("MODULE.bazel.lock"))?;
        loaded.push((platform, factor, lock));
    }
    for (platform, _, lock) in &loaded {
        check_keys(platform, lock)?;
    }
    let Some(((_, _, first), rest)) = loaded.split_first() else {
        return Err("lock merge has no platforms".to_owned());
    };
    let mut merged = first.clone();
    for (_, factor, lock) in rest {
        let extensions = lock.get("moduleExtensions").and_then(Value::as_object);
        for (key, value) in extensions.into_iter().flatten() {
            let Some(entry) = value.get(*factor) else {
                continue;
            };
            let slot = merged
                .as_object_mut()
                .map(|root| root.entry("moduleExtensions").or_insert_with(|| Value::Object(Map::new())))
                .and_then(Value::as_object_mut)
                .map(|extensions| extensions.entry(key.clone()).or_insert_with(|| Value::Object(Map::new())))
                .and_then(Value::as_object_mut)
                .ok_or_else(|| "lock_merge: the lock is not an object".to_owned())?;
            slot.insert((*factor).to_owned(), entry.clone());
        }
        if let (Some(facts), Some(root)) = (lock.get("facts"), merged.as_object_mut()) {
            deep_merge(
                root.entry("facts").or_insert_with(|| Value::Object(Map::new())),
                facts,
            );
        }
    }
    let text = serde_json::to_string_pretty(&merged).map_err(|error| error.to_string())?;
    std::fs::write(out, text).map_err(|error| format!("lock_merge: cannot write {out}: {error}"))
}
