//! Authoritative C/C++ compile context read from a Bazel aquery action record.
#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Mnemonic Bazel gives a configured C or C++ compile action.
pub const COMPILE_MNEMONIC: &str = "CppCompile";

/// File name clang-tidy and clangd read a compilation database from.
pub const DATABASE_FILE: &str = "compile_commands.json";

/// File name of the checked-in clang-tidy policy a consumer bundle carries.
pub const POLICY_FILE: &str = ".clang-tidy";

/// Config file extension a clang-tidy policy must use.
pub const POLICY_EXTENSION: &str = ".clang-tidy";

/// Everything that must fail closed instead of yielding a no-flags clean run.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum Error {
    #[error("compilation_db: malformed aquery record: {0}")]
    Malformed(String),
    #[error("compilation_db: no {mnemonic} action for label '{label}'")]
    NoCompileAction { mnemonic: String, label: String },
    #[error("compilation_db: compile action for '{target}' has no source flag before '{flag}'")]
    MissingSourceFlag { target: String, flag: String },
    #[error("compilation_db: response file '{path}' cannot be read: {reason}")]
    ResponseFile { path: String, reason: String },
    #[error("compilation_db: response file '{path}' ends inside a quoted argument")]
    UnterminatedQuote { path: String },
    #[error("compilation_db: source '{input}' has {count} conflicting compile commands")]
    ConflictingSource { input: String, count: usize },
    #[error("compilation_db: required source '{input}' has no compile command")]
    MissingSource { input: String },
    #[error("compilation_db: execroot '{path}' must be absolute")]
    RelativeExecroot { path: String },
    #[error("compilation_db: policy '{path}' must end in '{want}'")]
    WrongPolicyExtension { path: String, want: String },
    #[error("compilation_db: policy '{path}' cannot be read: {reason}")]
    UnreadablePolicy { path: String, reason: String },
    #[error("compilation_db: policy '{path}' is empty")]
    EmptyPolicy { path: String },
    #[error("compilation_db: output directory '{path}' cannot be created: {reason}")]
    UnwritableOutput { path: String, reason: String },
    #[error("compilation_db: database cannot be serialized: {reason}")]
    UnserializableDatabase { reason: String },
    #[error("compilation_db: {0}")]
    Usage(String),
}

/// One aquery action from a `--output=jsonproto` record.
#[derive(Debug, Deserialize)]
pub struct Action {
    #[serde(default)]
    pub mnemonic: String,
    #[serde(default)]
    pub arguments: Vec<String>,
    #[serde(default, rename = "inputDepSetIds")]
    pub input_dep_set_ids: Vec<i64>,
    #[serde(default, rename = "outputIds")]
    pub output_ids: Vec<i64>,
    #[serde(rename = "targetId", default)]
    pub target_id: i64,
    #[serde(rename = "configurationId", default)]
    pub configuration_id: i64,
    #[serde(default, rename = "executionPlatform")]
    pub execution_platform: String,
    #[serde(rename = "actionKey", default)]
    pub action_key: String,
}

/// One aquery artifact, addressed by execroot-relative path or path fragment.
#[derive(Debug, Deserialize)]
pub struct Artifact {
    pub id: i64,
    #[serde(default)]
    pub path: String,
    #[serde(rename = "pathFragmentId", default)]
    pub path_fragment_id: i64,
}

/// One aquery path fragment, linked to its parent fragment.
#[derive(Debug, Deserialize)]
pub struct PathFragment {
    pub id: i64,
    #[serde(default)]
    pub label: String,
    #[serde(rename = "parentId", default)]
    pub parent_id: i64,
}

/// One aquery file dep set; ids are recursive.
#[derive(Debug, Deserialize)]
pub struct DepSet {
    pub id: i64,
    #[serde(default, rename = "directArtifactIds")]
    pub direct_artifact_ids: Vec<i64>,
    #[serde(default, rename = "transitiveDepSetIds")]
    pub transitive_dep_set_ids: Vec<i64>,
}

/// One aquery target label.
#[derive(Debug, Deserialize)]
pub struct Target {
    pub id: i64,
    pub label: String,
}

/// One aquery configuration identity.
#[derive(Debug, Deserialize)]
pub struct Configuration {
    pub id: i64,
    #[serde(default)]
    pub mnemonic: String,
    #[serde(default)]
    pub checksum: String,
}

/// The decoded aquery action graph.
#[derive(Debug, Default, Deserialize)]
pub struct AqueryGraph {
    #[serde(default)]
    pub actions: Vec<Action>,
    #[serde(default)]
    pub artifacts: Vec<Artifact>,
    #[serde(default, rename = "depSetOfFiles")]
    pub dep_sets: Vec<DepSet>,
    #[serde(default)]
    pub targets: Vec<Target>,
    #[serde(default)]
    pub configuration: Vec<Configuration>,
    #[serde(default, rename = "pathFragments")]
    pub path_fragments: Vec<PathFragment>,
}

/// One configured compile action with its authoritative argv and declared inputs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompileCommand {
    pub target: String,
    pub configuration: String,
    pub execution_platform: String,
    pub action_key: String,
    pub source: String,
    pub arguments: Vec<String>,
    pub inputs: Vec<String>,
    pub outputs: Vec<String>,
}

/// One compilation database record, in the format clang tooling consumes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    pub directory: String,
    pub file: String,
    pub arguments: Vec<String>,
}

impl AqueryGraph {
    /// Decodes an aquery `--output=jsonproto` record.
    pub fn parse(text: &str) -> Result<Self, Error> {
        serde_json::from_str(text).map_err(|err| Error::Malformed(err.to_string()))
    }

    /// Collects every configured compile action, expanding response files.
    pub fn compile_commands(&self, execroot: &Path) -> Result<Vec<CompileCommand>, Error> {
        let fragments: BTreeMap<i64, &PathFragment> = self
            .path_fragments
            .iter()
            .map(|fragment| (fragment.id, fragment))
            .collect();
        let artifact_paths: BTreeMap<i64, String> = self
            .artifacts
            .iter()
            .map(|artifact| {
                let path = if artifact.path.is_empty() {
                    fragment_path(&fragments, artifact.path_fragment_id)
                } else {
                    artifact.path.clone()
                };
                (artifact.id, path)
            })
            .collect();
        let artifacts: BTreeMap<i64, &str> = artifact_paths
            .iter()
            .map(|(id, path)| (*id, path.as_str()))
            .collect();
        let targets: BTreeMap<i64, &str> = self
            .targets
            .iter()
            .map(|target| (target.id, target.label.as_str()))
            .collect();
        let configurations: BTreeMap<i64, String> = self
            .configuration
            .iter()
            .map(|config| {
                (
                    config.id,
                    format!("{} ({})", config.mnemonic, config.checksum),
                )
            })
            .collect();
        let dep_sets: BTreeMap<i64, &DepSet> = self
            .dep_sets
            .iter()
            .map(|dep_set| (dep_set.id, dep_set))
            .collect();
        let mut commands = Vec::new();
        for action in self
            .actions
            .iter()
            .filter(|a| a.mnemonic == COMPILE_MNEMONIC)
        {
            let target = targets
                .get(&action.target_id)
                .copied()
                .unwrap_or("")
                .to_string();
            let arguments = expand_response_files(&action.arguments, execroot)?;
            let source = source_argument(&target, &arguments)?.to_string();
            commands.push(CompileCommand {
                configuration: configurations
                    .get(&action.configuration_id)
                    .cloned()
                    .unwrap_or_default(),
                execution_platform: action.execution_platform.clone(),
                action_key: action.action_key.clone(),
                source,
                arguments,
                inputs: resolve_inputs(action, &dep_sets, &artifacts),
                outputs: sorted_paths(action.output_ids.as_slice(), &artifacts),
                target,
            });
        }
        Ok(commands)
    }
}

/// Reports whether a compile action belongs to the requested top-level label.
pub fn owns_label(action_target: &str, label: &str) -> bool {
    action_target == label || action_target == format!("{label}_upstream")
}

/// Reads the source a compile action compiles from its own argv.
fn source_argument<'a>(target: &str, arguments: &'a [String]) -> Result<&'a str, Error> {
    let missing = |flag: &str| Error::MissingSourceFlag {
        target: target.to_string(),
        flag: flag.to_string(),
    };
    let mut index = 0;
    while index < arguments.len() {
        let flag = arguments[index].as_str();
        if matches!(flag, "-c" | "/c" | "/Tc" | "/Tp") {
            let source = arguments.get(index + 1).ok_or_else(|| missing(flag))?;
            if source.starts_with('-') || source.starts_with('/') || source.starts_with('@') {
                return Err(missing(flag));
            }
            return Ok(source);
        }
        index += 1;
    }
    Err(missing(""))
}

/// Expands every clang `@response` argument against the execroot.
pub fn expand_response_files(arguments: &[String], execroot: &Path) -> Result<Vec<String>, Error> {
    let mut expanded: Vec<String> = Vec::with_capacity(arguments.len());
    let mut stack: Vec<String> = arguments.to_vec();
    stack.reverse();
    while let Some(argument) = stack.pop() {
        let path = match argument.strip_prefix('@') {
            None => {
                expanded.push(argument);
                continue;
            }
            Some("") => {
                expanded.push(argument);
                continue;
            }
            Some(path) => path.to_string(),
        };
        let response = read_response_file(execroot, &path)?;
        let mut tokens = tokenize_response(&path, &response)?;
        tokens.reverse();
        stack.extend(tokens);
    }
    Ok(expanded)
}

fn read_response_file(execroot: &Path, path: &str) -> Result<String, Error> {
    let full = execroot.join(path);
    fs::read_to_string(&full).map_err(|err| Error::ResponseFile {
        path: full.to_string_lossy().into_owned(),
        reason: err.to_string(),
    })
}

/// Splits a clang response file into arguments the way the clang driver does.
pub fn tokenize_response(path: &str, text: &str) -> Result<Vec<String>, Error> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    let mut escaped = false;
    for ch in text.chars() {
        if escaped {
            current.push(ch);
            escaped = false;
        } else if ch == '\\' {
            escaped = true;
        } else if ch == '"' {
            quoted = !quoted;
        } else if ch.is_whitespace() && !quoted {
            if !current.is_empty() {
                tokens.push(std::mem::take(&mut current));
            }
        } else {
            current.push(ch);
        }
    }
    if quoted || escaped {
        return Err(Error::UnterminatedQuote {
            path: path.to_string(),
        });
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    Ok(tokens)
}

/// Rebuilds one execroot-relative artifact path from its fragment chain.
pub fn fragment_path(fragments: &BTreeMap<i64, &PathFragment>, id: i64) -> String {
    let mut parts: Vec<&str> = Vec::new();
    let mut current = id;
    let mut guard = 0;
    while guard <= fragments.len() {
        let Some(fragment) = fragments.get(&current) else {
            break;
        };
        if !fragment.label.is_empty() {
            parts.push(fragment.label.as_str());
        }
        if fragment.parent_id == 0 {
            break;
        }
        current = fragment.parent_id;
        guard += 1;
    }
    parts.reverse();
    parts.join("/")
}

fn resolve_inputs(
    action: &Action,
    dep_sets: &BTreeMap<i64, &DepSet>,
    artifacts: &BTreeMap<i64, &str>,
) -> Vec<String> {
    let mut seen: BTreeSet<i64> = BTreeSet::new();
    let mut paths: BTreeSet<&str> = BTreeSet::new();
    let mut pending: Vec<i64> = action.input_dep_set_ids.clone();
    while let Some(id) = pending.pop() {
        if !seen.insert(id) {
            continue;
        }
        let Some(dep_set) = dep_sets.get(&id) else {
            continue;
        };
        for artifact_id in dep_set.direct_artifact_ids.iter() {
            if let Some(path) = artifacts.get(artifact_id) {
                paths.insert(path);
            }
        }
        for nested in dep_set.transitive_dep_set_ids.iter() {
            pending.push(*nested);
        }
    }
    paths.into_iter().map(str::to_string).collect()
}

fn sorted_paths(ids: &[i64], artifacts: &BTreeMap<i64, &str>) -> Vec<String> {
    let mut paths: BTreeSet<&str> = BTreeSet::new();
    for id in ids {
        if let Some(path) = artifacts.get(id) {
            paths.insert(path);
        }
    }
    paths.into_iter().map(str::to_string).collect()
}

/// Builds the database for one label, refusing any source with conflicting context.
pub fn database(
    commands: &[CompileCommand],
    execroot: &Path,
    label: &str,
    required_sources: &[String],
) -> Result<Vec<Entry>, Error> {
    if !execroot.is_absolute() {
        return Err(Error::RelativeExecroot {
            path: execroot.to_string_lossy().into_owned(),
        });
    }
    let owned = commands
        .iter()
        .filter(|command| owns_label(&command.target, label))
        .count();
    if owned == 0 {
        return Err(Error::NoCompileAction {
            mnemonic: COMPILE_MNEMONIC.to_string(),
            label: label.to_string(),
        });
    }
    let directory = execroot.to_string_lossy().into_owned();
    let mut by_source: BTreeMap<&str, &CompileCommand> = BTreeMap::new();
    let mut conflicts: BTreeMap<&str, usize> = BTreeMap::new();
    for command in commands {
        let previous = by_source.insert(command.source.as_str(), command);
        if let Some(previous) = previous {
            if previous.arguments != command.arguments {
                let count = conflicts.entry(command.source.as_str()).or_insert(1);
                *count += 1;
            }
        }
    }
    if let Some((source, count)) = conflicts.iter().next() {
        return Err(Error::ConflictingSource {
            input: (*source).to_string(),
            count: *count,
        });
    }
    let mut entries: Vec<Entry> = by_source
        .values()
        .map(|command| Entry {
            directory: directory.clone(),
            file: execroot
                .join(&command.source)
                .to_string_lossy()
                .into_owned(),
            arguments: command.arguments.clone(),
        })
        .collect();
    entries.sort_by(|left, right| left.file.cmp(&right.file));
    for source in required_sources {
        if !entries
            .iter()
            .any(|entry| same_source(&entry.file, source, execroot))
        {
            return Err(Error::MissingSource {
                input: source.clone(),
            });
        }
    }
    Ok(entries)
}

fn same_source(file: &str, source: &str, execroot: &Path) -> bool {
    let source_path = Path::new(source);
    if source_path.is_absolute() {
        return file == source;
    }
    Path::new(file) == execroot.join(source_path)
}

/// Reads and validates a checked-in clang-tidy policy.
pub fn read_policy(path: &Path) -> Result<String, Error> {
    let text = path.to_string_lossy().into_owned();
    if !text.ends_with(POLICY_EXTENSION) {
        return Err(Error::WrongPolicyExtension {
            path: text,
            want: POLICY_EXTENSION.to_string(),
        });
    }
    let body = fs::read_to_string(path).map_err(|err| Error::UnreadablePolicy {
        path: text.clone(),
        reason: err.to_string(),
    })?;
    if body.trim().is_empty() {
        return Err(Error::EmptyPolicy { path: text });
    }
    Ok(body)
}

/// Writes the compilation database and the bound policy into one consumer bundle.
pub fn write_bundle(directory: &Path, entries: &[Entry], policy: &str) -> Result<(), Error> {
    fs::create_dir_all(directory).map_err(|err| Error::UnwritableOutput {
        path: directory.to_string_lossy().into_owned(),
        reason: err.to_string(),
    })?;
    let rendered =
        serde_json::to_string_pretty(entries).map_err(|err| Error::UnserializableDatabase {
            reason: err.to_string(),
        })?;
    let database = directory.join(DATABASE_FILE);
    fs::write(&database, format!("{rendered}\n")).map_err(|err| Error::UnwritableOutput {
        path: database.to_string_lossy().into_owned(),
        reason: err.to_string(),
    })?;
    let policy_path = directory.join(POLICY_FILE);
    fs::write(&policy_path, policy).map_err(|err| Error::UnwritableOutput {
        path: policy_path.to_string_lossy().into_owned(),
        reason: err.to_string(),
    })?;
    Ok(())
}

/// Reads a whole file, or stdin when the path is `-`.
pub fn read_input(path: &str) -> Result<String, Error> {
    if path == "-" {
        let mut text = String::new();
        std::io::Read::read_to_string(&mut std::io::stdin(), &mut text).map_err(|err| {
            Error::UnreadablePolicy {
                path: "stdin".to_string(),
                reason: err.to_string(),
            }
        })?;
        return Ok(text);
    }
    fs::read_to_string(PathBuf::from(path)).map_err(|err| Error::ResponseFile {
        path: path.to_string(),
        reason: err.to_string(),
    })
}
#[cfg(test)]
#[path = "lib_tests.rs"]
mod lib_tests;
