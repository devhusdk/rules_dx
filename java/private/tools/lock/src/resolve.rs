//! Runs the `rules_jvm_external` Maven resolver on a version catalog with the pinned JDK.
#![deny(
    clippy::indexing_slicing,
    clippy::map_err_ignore,
    clippy::string_slice,
    clippy::unused_result_ok
)]

use std::path::PathBuf;

use dx_lock::{RESOLVER_TIMEOUT, Scratch};
use serde_json::{Value, json};

/// The JDK and resolver jar the lock step runs.
pub struct Tools {
    /// The `java` executable.
    pub java: PathBuf,
    /// The resolver deploy jar.
    pub jar: PathBuf,
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

fn coordinates(
    alias: &str,
    value: &toml::Value,
    versions: &toml::Table,
) -> Result<(String, Value), String> {
    let table = match value {
        toml::Value::String(coords) => {
            let mut parts = coords.split(':');
            let (Some(group), Some(artifact), Some(version), None) =
                (parts.next(), parts.next(), parts.next(), parts.next())
            else {
                return Err(format!(
                    "library {alias}: {coords} is not group:artifact:version"
                ));
            };
            return Ok((
                format!("{group}:{artifact}"),
                json!({"group": group, "artifact": artifact, "version": version}),
            ));
        }
        toml::Value::Table(table) => table,
        toml::Value::Integer(_)
        | toml::Value::Float(_)
        | toml::Value::Boolean(_)
        | toml::Value::Datetime(_)
        | toml::Value::Array(_) => {
            return Err(format!("library {alias} is neither a string nor a table"));
        }
    };
    let (group, artifact) = match (
        text(table, "module"),
        text(table, "group"),
        text(table, "name"),
    ) {
        (Some(module), _, _) => module
            .split_once(':')
            .ok_or_else(|| format!("library {alias}: module {module} is not group:artifact"))?,
        (None, Some(group), Some(name)) => (group, name),
        _ => return Err(format!("library {alias} needs module, or group and name")),
    };
    let version = match table.get("version") {
        Some(toml::Value::String(version)) => version.as_str(),
        Some(toml::Value::Table(version)) => {
            let reference = text(version, "ref")
                .ok_or_else(|| format!("library {alias}: version has no ref"))?;
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
///
/// # Errors
///
/// Returns an error when the operation fails.
pub fn config(manifest: &str, repositories: &[String]) -> Result<String, String> {
    let parsed: toml::Table = manifest
        .parse()
        .map_err(|error| format!("version catalog: {error}"))?;
    let empty = toml::Table::new();
    let versions = parsed
        .get("versions")
        .and_then(toml::Value::as_table)
        .unwrap_or(&empty);
    let libraries = parsed
        .get("libraries")
        .and_then(toml::Value::as_table)
        .unwrap_or(&empty);
    let excluded = parsed
        .get("excluded_artifacts")
        .and_then(toml::Value::as_array)
        .map(|items| {
            items
                .iter()
                .map(|item| {
                    item.as_str().map(str::to_owned).ok_or_else(|| {
                        "excluded_artifacts holds only group:artifact strings".to_owned()
                    })
                })
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()?
        .unwrap_or_default();
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
        if bom {
            boms.push(entry);
        } else {
            artifacts.push(entry);
        }
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
        "globalExclusions": excluded,
        "fetchSources": false,
        "fetchJavadoc": false,
    });
    serde_json::to_string_pretty(&config).map_err(|error| error.to_string())
}

/// The top-level lock key that records the sha256 of the manifest the lock was resolved from.
pub const MARKER: &str = "__DX_INPUTS_SHA256";

fn recorded(lock: &[u8]) -> Option<String> {
    let parsed: Value = serde_json::from_slice(lock).ok()?;
    parsed.get(MARKER)?.as_str().map(str::to_owned)
}

/// Resolves the set's manifest and writes, or with `check` compares the lock's marker to the
/// manifest.
///
/// A lock whose marker matches the manifest digest is fresh without running the resolver.
///
/// # Errors
///
/// Returns an error when the operation fails.
pub fn lock(tools: &Tools, request: &Request) -> Result<Outcome, String> {
    let manifest_path = request.dir.join(&request.manifest);
    let manifest = std::fs::read_to_string(&manifest_path)
        .map_err(|error| format!("{}: {error}", manifest_path.display()))?;
    let digest = dx_digest::sha256_hex(manifest.as_bytes());
    let lock_path = request.dir.join(&request.lock);
    let current = std::fs::read(&lock_path).ok();
    if current.as_deref().and_then(recorded).as_deref() == Some(digest.as_str()) {
        return Ok(Outcome::Fresh);
    }
    if request.check {
        return Ok(Outcome::Stale);
    }
    let config = config(&manifest, &request.repositories)?;
    let resolved = resolve(tools, &config)?;
    let Some(body) = resolved.strip_prefix(b"{\n") else {
        return Err("the maven resolver output does not start with {".to_owned());
    };
    let mut marked = format!("{{\n  \"{MARKER}\": \"{digest}\",\n").into_bytes();
    marked.extend_from_slice(body);
    std::fs::write(&lock_path, &marked)
        .map_err(|error| format!("{}: {error}", lock_path.display()))?;
    Ok(Outcome::Written)
}

fn resolve(tools: &Tools, config: &str) -> Result<Vec<u8>, String> {
    let scratch = Scratch::new().map_err(|error| error.to_string())?;
    let config_path = scratch.set().join("config.json");
    let output = scratch.set().join("lock.json");
    std::fs::write(&config_path, config).map_err(|error| error.to_string())?;
    let mut command = scratch.command(&tools.java);
    command
        .env("RJE_VERBOSE", "false")
        .arg(format!("-Duser.home={}", scratch.home().display()))
        .arg(format!("-Djava.io.tmpdir={}", scratch.tmp().display()))
        .arg("-jar")
        .arg(&tools.jar)
        .arg("--argsfile")
        .arg(&config_path)
        .arg("--use_isolated_cache")
        .arg("--output")
        .arg(&output);
    Scratch::run(&mut command, RESOLVER_TIMEOUT)
        .map_err(|error| format!("the maven resolver failed: {error}"))?;
    std::fs::read(&output).map_err(|error| format!("{}: {error}", output.display()))
}

fn parse(args: &[String]) -> Result<(Tools, dx_lock::Args), dx_lock::Error> {
    let (paths, rest) = dx_lock::tool_paths(args, &["--java", "--resolver-jar"])?;
    let mut paths = paths.into_iter();
    let (Some(java), Some(jar)) = (paths.next(), paths.next()) else {
        return Err(dx_lock::Error::new(
            "--java and --resolver-jar are required",
        ));
    };
    let args = dx_lock::Args::parse(rest)?;
    if !args.upgrade.is_empty() {
        return Err(dx_lock::Error::new("--upgrade is not supported"));
    }
    Ok((Tools { java, jar }, args))
}

/// Runs the lock step on `args`, the arguments after the subcommand name:
/// `--java=<path> --resolver-jar=<path> --dir <set directory> [--check]`.
/// Returns 0 when the lock is fresh or written, 1 when stale or the resolver fails, 2 on bad
/// arguments.
pub fn run_process(args: &[String]) -> i32 {
    let (tools, args) = match parse(args) {
        Ok(parsed) => parsed,
        Err(error) => {
            eprintln!("java_lock: {error}");
            return 2;
        }
    };
    let Some(hub) = dx_families::hub("maven") else {
        eprintln!("maven has no registry entry");
        return 2;
    };
    let request = Request {
        dir: args.dir,
        manifest: hub.manifest.clone(),
        lock: hub.lock.clone().unwrap_or_default(),
        repositories: Vec::new(),
        check: args.check,
    };
    match lock(&tools, &request) {
        Ok(Outcome::Fresh | Outcome::Written) => 0,
        Ok(Outcome::Stale) => {
            eprintln!("{} is stale; run dx generate --apply", request.lock);
            1
        }
        Err(error) => {
            eprintln!("{error}");
            1
        }
    }
}
