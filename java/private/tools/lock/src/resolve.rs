//! Runs the rules_jvm_external Maven resolver on a version catalog with the pinned JDK.
#![deny(clippy::indexing_slicing, clippy::string_slice)]

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{Value, json};

/// The JDK and resolver jar the lock step runs.
pub struct Tools {
    /// The `java` executable.
    pub java: PathBuf,
    /// The resolver deploy jar.
    pub jar: PathBuf,
}

impl Tools {
    /// Reads `DX_MAVEN_JAVA` and `DX_MAVEN_RESOLVER` as runfiles paths.
    pub fn from_env() -> Result<Self, String> {
        Ok(Tools {
            java: runfile("DX_MAVEN_JAVA")?,
            jar: runfile("DX_MAVEN_RESOLVER")?,
        })
    }
}

fn runfile(var: &str) -> Result<PathBuf, String> {
    let value = std::env::var(var).map_err(|_| format!("{var} is not set"))?;
    let relative = value.strip_prefix("../").unwrap_or(&value);
    let root = match std::env::var_os("RUNFILES_DIR") {
        Some(root) => PathBuf::from(root),
        None => std::env::current_dir()
            .map_err(|error| error.to_string())?
            .parent()
            .map(Path::to_path_buf)
            .ok_or_else(|| "no runfiles directory".to_owned())?,
    };
    let path = root.join(relative);
    if path.exists() {
        Ok(path)
    } else {
        Err(format!("{var}: {} does not exist", path.display()))
    }
}

/// One lock step for one dependency set.
pub struct Request {
    /// The set directory.
    pub dir: PathBuf,
    /// The manifest file name.
    pub manifest: String,
    /// The lock file name.
    pub lock: String,
    /// Repository URLs; empty means Maven Central.
    pub repositories: Vec<String>,
    /// Compare only.
    pub check: bool,
}

/// What the lock step did.
#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    /// The lock already matched.
    Fresh,
    /// The lock was written.
    Written,
    /// `check` found the lock missing or different.
    Stale,
}

fn text<'a>(table: &'a toml::Table, key: &str) -> Option<&'a str> {
    table.get(key).and_then(toml::Value::as_str)
}

fn coordinates(alias: &str, value: &toml::Value, versions: &toml::Table) -> Result<(String, Value), String> {
    let table = match value {
        toml::Value::String(coords) => {
            let mut parts = coords.split(':');
            let (Some(group), Some(artifact), Some(version), None) =
                (parts.next(), parts.next(), parts.next(), parts.next())
            else {
                return Err(format!("library {alias}: {coords} is not group:artifact:version"));
            };
            return Ok((
                format!("{group}:{artifact}"),
                json!({"group": group, "artifact": artifact, "version": version}),
            ));
        }
        toml::Value::Table(table) => table,
        _ => return Err(format!("library {alias} is neither a string nor a table")),
    };
    let (group, artifact) = match (text(table, "module"), text(table, "group"), text(table, "name")) {
        (Some(module), _, _) => module
            .split_once(':')
            .ok_or_else(|| format!("library {alias}: module {module} is not group:artifact"))?,
        (None, Some(group), Some(name)) => (group, name),
        _ => return Err(format!("library {alias} needs module, or group and name")),
    };
    let version = match table.get("version") {
        Some(toml::Value::String(version)) => version.as_str(),
        Some(toml::Value::Table(version)) => {
            let reference = text(version, "ref").ok_or_else(|| format!("library {alias}: version has no ref"))?;
            versions
                .get(reference)
                .and_then(toml::Value::as_str)
                .ok_or_else(|| format!("library {alias}: no version {reference}"))?
        }
        _ => return Err(format!("library {alias} has no version")),
    };
    let mut artifact_json = json!({"group": group, "artifact": artifact, "version": version});
    if let Some(packaging) = text(table, "package").or_else(|| text(table, "packaging"))
        && packaging != "jar"
        && let Some(object) = artifact_json.as_object_mut()
    {
        object.insert("packaging".to_owned(), json!(packaging));
    }
    let bom = text(table, "is_bom").is_some_and(|flag| flag.eq_ignore_ascii_case("true"))
        || table.get("is_bom").and_then(toml::Value::as_bool) == Some(true);
    if bom && let Some(object) = artifact_json.as_object_mut() {
        object.insert("bom".to_owned(), json!(true));
    }
    Ok((format!("{group}:{artifact}"), artifact_json))
}

/// Renders the resolver's config file for one manifest.
pub fn config(manifest: &str, repositories: &[String]) -> Result<String, String> {
    let parsed: toml::Table = manifest.parse().map_err(|error| format!("version catalog: {error}"))?;
    let empty = toml::Table::new();
    let versions = parsed.get("versions").and_then(toml::Value::as_table).unwrap_or(&empty);
    let libraries = parsed.get("libraries").and_then(toml::Value::as_table).unwrap_or(&empty);
    let mut artifacts = Vec::new();
    let mut boms = Vec::new();
    let mut entries = libraries
        .iter()
        .map(|(alias, value)| coordinates(alias, value, versions))
        .collect::<Result<Vec<_>, _>>()?;
    entries.sort_by(|left, right| left.0.cmp(&right.0));
    for (_, mut entry) in entries {
        let bom = entry
            .as_object_mut()
            .and_then(|object| object.remove("bom"))
            .is_some();
        if bom { boms.push(entry) } else { artifacts.push(entry) }
    }
    let repositories = if repositories.is_empty() {
        vec!["https://repo1.maven.org/maven2/".to_owned()]
    } else {
        repositories.to_vec()
    };
    let config = json!({
        "repositories": repositories,
        "artifacts": artifacts,
        "boms": boms,
        "globalExclusions": [],
        "fetchSources": false,
        "fetchJavadoc": false,
    });
    serde_json::to_string_pretty(&config).map_err(|error| error.to_string())
}

/// Resolves the set's manifest and writes, or with `check` compares, its lock.
pub fn lock(tools: &Tools, request: &Request) -> Result<Outcome, String> {
    let manifest_path = request.dir.join(&request.manifest);
    let manifest = std::fs::read_to_string(&manifest_path)
        .map_err(|error| format!("{}: {error}", manifest_path.display()))?;
    let config = config(&manifest, &request.repositories)?;
    let scratch = std::env::temp_dir().join(format!("dx-maven-lock-{}", std::process::id()));
    std::fs::create_dir_all(scratch.join("home")).map_err(|error| error.to_string())?;
    let result = resolve(tools, &config, &scratch);
    let outcome = result.and_then(|resolved| {
        let lock_path = request.dir.join(&request.lock);
        let current = std::fs::read(&lock_path).ok();
        if current.as_deref() == Some(resolved.as_slice()) {
            Ok(Outcome::Fresh)
        } else if request.check {
            Ok(Outcome::Stale)
        } else {
            std::fs::write(&lock_path, &resolved).map_err(|error| format!("{}: {error}", lock_path.display()))?;
            Ok(Outcome::Written)
        }
    });
    let _ = std::fs::remove_dir_all(&scratch);
    outcome
}

fn resolve(tools: &Tools, config: &str, scratch: &Path) -> Result<Vec<u8>, String> {
    let config_path = scratch.join("config.json");
    let output = scratch.join("lock.json");
    let home = scratch.join("home");
    std::fs::write(&config_path, config).map_err(|error| error.to_string())?;
    let status = Command::new(&tools.java)
        .env_clear()
        .env("HOME", &home)
        .env("TMPDIR", scratch)
        .env("RJE_VERBOSE", "false")
        .arg(format!("-Duser.home={}", home.display()))
        .arg(format!("-Djava.io.tmpdir={}", scratch.display()))
        .arg("-jar")
        .arg(&tools.jar)
        .arg("--argsfile")
        .arg(&config_path)
        .arg("--use_isolated_cache")
        .arg("--output")
        .arg(&output)
        .current_dir(scratch)
        .status()
        .map_err(|error| format!("{}: {error}", tools.java.display()))?;
    if !status.success() {
        return Err(format!("the maven resolver failed: {status}"));
    }
    std::fs::read(&output).map_err(|error| format!("{}: {error}", output.display()))
}
