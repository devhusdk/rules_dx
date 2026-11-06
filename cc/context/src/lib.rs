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
    #[error("compilation_db: response file '{path}' includes itself")]
    ResponseFileCycle { path: String },
    #[error("compilation_db: response file '{path}' is not valid UTF-8 or UTF-16LE")]
    ResponseFileEncoding { path: String },
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
    #[error("compilation_db: identity '{path}' is malformed: {reason}")]
    MalformedIdentity { path: String, reason: String },
    #[error("compilation_db: configuration '{configuration}' has conflicting compiler identities")]
    ConflictingIdentity { configuration: String },
    #[error(
        "compilation_db: {tool} cannot run '{target}': compiler '{compiler}' has no known driver"
    )]
    UnsupportedProjection {
        tool: String,
        target: String,
        compiler: String,
    },
    #[error("compilation_db: msvc action for '{target}' has no {missing} to project")]
    MissingMsvcContext { target: String, missing: String },
    #[error("compilation_db: '{target}' is not qualified: {reason}")]
    Unqualified { target: String, reason: String },
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
    #[serde(default, rename = "environmentVariables")]
    pub environment_variables: Vec<EnvironmentVariable>,
    #[serde(default, rename = "paramFiles")]
    pub param_files: Vec<ParamFile>,
}

/// One param file Bazel wrote for an action, as aquery reports it.
#[derive(Debug, Deserialize)]
pub struct ParamFile {
    #[serde(default, rename = "execPath")]
    pub exec_path: String,
    #[serde(default)]
    pub arguments: Vec<String>,
}

/// One aquery action environment entry.
#[derive(Debug, Deserialize)]
pub struct EnvironmentVariable {
    pub key: String,
    #[serde(default)]
    pub value: String,
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

/// How a compiler driver takes its flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Driver {
    Gnu,
    Msvc,
    #[serde(other)]
    Unknown,
}

/// The compiler Bazel selected for one configuration.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct CompilerIdentity {
    pub configuration: String,
    #[serde(default)]
    pub compiler: String,
    #[serde(default)]
    pub family: String,
    pub driver: Driver,
    #[serde(default)]
    pub abi: String,
    #[serde(default)]
    pub target: String,
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
    pub identity: Option<CompilerIdentity>,
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

    /// Drops compile actions that belong to none of `labels`, such as toolchain tools
    /// pulled in by `deps()`.
    pub fn retain_owned(&mut self, labels: &[String]) {
        let targets: BTreeMap<i64, &str> = self
            .targets
            .iter()
            .map(|target| (target.id, target.label.as_str()))
            .collect();
        let owned: BTreeSet<i64> = targets
            .iter()
            .filter(|(_, target)| labels.iter().any(|label| owns_label(target, label)))
            .map(|(id, _)| *id)
            .collect();
        self.actions
            .retain(|a| a.mnemonic != COMPILE_MNEMONIC || owned.contains(&a.target_id));
    }

    /// Collects every configured compile action, expanding response files.
    pub fn compile_commands(
        &self,
        execroot: &Path,
        identities: &[CompilerIdentity],
    ) -> Result<Vec<CompileCommand>, Error> {
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
        let mnemonics: BTreeMap<i64, &str> = self
            .configuration
            .iter()
            .map(|config| (config.id, config.mnemonic.as_str()))
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
            let identity = mnemonics
                .get(&action.configuration_id)
                .and_then(|mnemonic| {
                    identities
                        .iter()
                        .find(|identity| identity.configuration == *mnemonic)
                });
            let driver = identity.map_or(Driver::Unknown, |identity| identity.driver);
            let arguments = match rsp_quoting(&action.arguments, driver) {
                Some(quoting) => expand_response_files(&action.arguments, execroot, quoting)?,
                None => action.arguments.clone(),
            };
            let arguments = match identity {
                Some(identity) if identity.driver == Driver::Msvc => {
                    msvc_context_arguments(&target, action, identity, arguments)?
                }
                _ => arguments,
            };
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
                identity: identity.cloned(),
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
        for prefix in ["/Tc", "/Tp"] {
            if flag.len() > prefix.len() && flag.starts_with(prefix) {
                return Ok(&flag[prefix.len()..]);
            }
        }
        index += 1;
    }
    Err(missing(""))
}

/// How a driver splits the text of a response file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Quoting {
    Gnu,
    Windows,
}

/// Picks the response-file quoting from the selected driver, or None when it is unknown.
pub fn rsp_quoting(arguments: &[String], driver: Driver) -> Option<Quoting> {
    for argument in arguments {
        match argument.strip_prefix("--rsp-quoting=") {
            Some("windows") => return Some(Quoting::Windows),
            Some("posix") => return Some(Quoting::Gnu),
            _ => {}
        }
    }
    if arguments.iter().any(|a| a == "--driver-mode=cl") {
        return Some(Quoting::Windows);
    }
    match driver {
        Driver::Gnu => Some(Quoting::Gnu),
        Driver::Msvc => Some(Quoting::Windows),
        Driver::Unknown => None,
    }
}

/// Reads the compiler identity files Bazel wrote, one per configuration.
pub fn read_identities(paths: &[PathBuf]) -> Result<Vec<CompilerIdentity>, Error> {
    let mut identities: Vec<CompilerIdentity> = Vec::new();
    for path in paths {
        let malformed = |reason: String| Error::MalformedIdentity {
            path: path.to_string_lossy().into_owned(),
            reason,
        };
        let text = fs::read_to_string(path).map_err(|err| malformed(err.to_string()))?;
        let identity: CompilerIdentity =
            serde_json::from_str(&text).map_err(|err| malformed(err.to_string()))?;
        match identities
            .iter()
            .find(|known| known.configuration == identity.configuration)
        {
            Some(known) if *known != identity => {
                return Err(Error::ConflictingIdentity {
                    configuration: identity.configuration,
                });
            }
            Some(_) => {}
            None => identities.push(identity),
        }
    }
    Ok(identities)
}

/// Argv flags that mean an MSVC action already carries its own CRT and SDK includes.
const MSVC_ARGV_INCLUDE_FLAGS: [&str; 4] = ["/imsvc", "-imsvc", "/winsysroot", "-winsysroot"];

/// Argv0 and arguments of the version probe for a recorded compiler, with a relative
/// argv0 resolved against the execroot. `cl` takes no flag and prints its banner on stderr.
pub fn version_probe(execroot: &Path, argv0: &str) -> (PathBuf, Vec<String>) {
    let path = execroot.join(argv0);
    let is_cl = path
        .file_stem()
        .is_some_and(|stem| stem.eq_ignore_ascii_case("cl"));
    let args = if is_cl {
        Vec::new()
    } else {
        vec!["--version".to_string()]
    };
    (path, args)
}

/// Turns the hermetic MSVC action env into the flags clang-cl needs, right after argv0.
/// An action with no `INCLUDE` in its env but include flags in argv is kept unchanged.
fn msvc_context_arguments(
    target: &str,
    action: &Action,
    identity: &CompilerIdentity,
    arguments: Vec<String>,
) -> Result<Vec<String>, Error> {
    let env = |key: &str| {
        action
            .environment_variables
            .iter()
            .find(|variable| variable.key == key)
            .map_or("", |variable| variable.value.as_str())
    };
    let missing = |what: &str| Error::MissingMsvcContext {
        target: target.to_string(),
        missing: what.to_string(),
    };
    let mut flags: Vec<String> = ["INCLUDE", "EXTERNAL_INCLUDE"]
        .iter()
        .flat_map(|key| env(key).split(';'))
        .filter(|dir| !dir.is_empty())
        .map(|dir| format!("/imsvc{}", dir.replace('\\', "/")))
        .collect();
    if flags.is_empty() {
        let in_argv = arguments.iter().any(|argument| {
            MSVC_ARGV_INCLUDE_FLAGS
                .iter()
                .any(|flag| argument.starts_with(flag))
        });
        if in_argv {
            return Ok(arguments);
        }
        return Err(missing("INCLUDE in the action env or /imsvc in argv"));
    }
    flags.push("/X".to_string());
    let has_target = arguments
        .iter()
        .any(|argument| argument == "-target" || argument.starts_with("--target"));
    if !has_target {
        if identity.target.is_empty() {
            return Err(missing("target in the compiler identity"));
        }
        flags.push(format!("--target={}", identity.target));
    }
    let mut projected = arguments;
    let tail = projected.split_off(1.min(projected.len()));
    projected.extend(flags);
    projected.extend(tail);
    Ok(projected)
}

/// Fails when clang-tidy would run on an action whose driver Bazel did not identify.
pub fn clang_tidy_projection(commands: &[CompileCommand]) -> Result<(), Error> {
    for command in commands {
        if !matches!(&command.identity, Some(i) if i.driver != Driver::Unknown) {
            return Err(Error::UnsupportedProjection {
                tool: "clang-tidy".to_string(),
                target: command.target.clone(),
                compiler: command
                    .identity
                    .as_ref()
                    .map(|i| i.compiler.clone())
                    .or_else(|| command.arguments.first().cloned())
                    .unwrap_or_default(),
            });
        }
    }
    Ok(())
}

/// One exported command proven equal to Bazel's own action argv.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Qualified {
    pub target: String,
    pub configuration: String,
    pub compiler: String,
    pub driver: String,
    pub expected: Vec<String>,
    pub exported: Vec<String>,
}

/// Checks exported commands against the argv Bazel recorded, with param files
/// taken from aquery `paramFiles` and never from this crate's tokenizer.
/// Compile actions whose param file aquery does not report (the compiler param
/// file Bazel writes while executing) are read from the execroot, one argument
/// per line.
pub fn qualify(
    graph: &AqueryGraph,
    commands: &[CompileCommand],
    execroot: &Path,
) -> Result<Vec<Qualified>, Error> {
    let mut qualified = Vec::new();
    for command in commands {
        let fail = |reason: String| Error::Unqualified {
            target: command.target.clone(),
            reason,
        };
        let action = graph
            .actions
            .iter()
            .find(|action| {
                action.mnemonic == COMPILE_MNEMONIC && action.action_key == command.action_key
            })
            .ok_or_else(|| fail(format!("no action with key '{}'", command.action_key)))?;
        let identity = match &command.identity {
            Some(identity) if identity.driver != Driver::Unknown => identity,
            _ => return Err(fail("no compiler identity with a known driver".to_string())),
        };
        let mut expected = Vec::new();
        for argument in &action.arguments {
            match argument.strip_prefix('@') {
                Some(path) => {
                    let listed = action
                        .param_files
                        .iter()
                        .find(|file| file.exec_path.replace('\\', "/") == path.replace('\\', "/"));
                    match listed {
                        Some(file) => expected.extend(file.arguments.iter().cloned()),
                        None => {
                            let text = read_response_file(execroot, path)
                                .map_err(|err| fail(format!("param file '{path}': {err}")))?;
                            expected.extend(text.lines().map(unquote_line));
                        }
                    }
                }
                None => expected.push(argument.clone()),
            }
        }
        let exported = &command.arguments;
        if exported.first() != expected.first() {
            return Err(fail(format!(
                "argv0 {:?} differs from {:?}",
                exported.first(),
                expected.first()
            )));
        }
        match identity.driver {
            Driver::Msvc => {
                let tail = expected.get(1..).unwrap_or(&[]);
                let middle_len = exported.len().checked_sub(1 + tail.len());
                let middle = middle_len.and_then(|len| exported.get(1..1 + len));
                let ok = match middle {
                    Some(middle) => {
                        exported.ends_with(tail)
                            && middle.iter().all(|flag| {
                                flag.starts_with("/imsvc")
                                    || flag == "/X"
                                    || flag.starts_with("--target=")
                            })
                    }
                    None => false,
                };
                if !ok {
                    return Err(fail(format!(
                        "msvc argv differs from the action beyond the projected prefix: {}",
                        describe_mismatch(exported, &expected)
                    )));
                }
            }
            _ => {
                if *exported != expected {
                    return Err(fail(format!(
                        "argv differs from the action argv: {}",
                        describe_mismatch(exported, &expected)
                    )));
                }
            }
        }
        qualified.push(Qualified {
            target: command.target.clone(),
            configuration: command.configuration.clone(),
            compiler: identity.compiler.clone(),
            driver: format!("{:?}", identity.driver),
            expected,
            exported: exported.clone(),
        });
    }
    Ok(qualified)
}

fn unquote_line(line: &str) -> String {
    let line = line.trim_end_matches('\r');
    let inner = line
        .strip_prefix('"')
        .and_then(|rest| rest.strip_suffix('"'))
        .unwrap_or(line);
    inner.replace("\\\"", "\"")
}

/// Expands every `@response` argument against the execroot.
pub fn expand_response_files(
    arguments: &[String],
    execroot: &Path,
    quoting: Quoting,
) -> Result<Vec<String>, Error> {
    let mut expanded = Vec::with_capacity(arguments.len());
    let mut chain = Vec::new();
    expand_into(arguments, execroot, quoting, &mut chain, &mut expanded)?;
    Ok(expanded)
}

fn expand_into(
    arguments: &[String],
    execroot: &Path,
    quoting: Quoting,
    chain: &mut Vec<String>,
    expanded: &mut Vec<String>,
) -> Result<(), Error> {
    for argument in arguments {
        let path = match argument.strip_prefix('@') {
            None | Some("") => {
                expanded.push(argument.clone());
                continue;
            }
            Some(path) => path,
        };
        if chain.iter().any(|open| open == path) {
            return Err(Error::ResponseFileCycle {
                path: path.to_string(),
            });
        }
        let response = read_response_file(execroot, path)?;
        let tokens = tokenize_response(path, &response, quoting)?;
        chain.push(path.to_string());
        expand_into(&tokens, execroot, quoting, chain, expanded)?;
        chain.pop();
    }
    Ok(())
}

fn read_response_file(execroot: &Path, path: &str) -> Result<String, Error> {
    let full = execroot.join(path);
    let full_text = full.to_string_lossy().into_owned();
    let bytes = fs::read(&full).map_err(|err| Error::ResponseFile {
        path: full_text.clone(),
        reason: err.to_string(),
    })?;
    let encoding = || Error::ResponseFileEncoding {
        path: full_text.clone(),
    };
    if let Some(rest) = bytes.strip_prefix(&[0xFF, 0xFE]) {
        if rest.len() % 2 != 0 {
            return Err(encoding());
        }
        let units: Vec<u16> = rest
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| u16::from_le_bytes(*pair))
            .collect();
        return String::from_utf16(&units).map_err(|_| encoding());
    }
    let bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(&bytes);
    String::from_utf8(bytes.to_vec()).map_err(|_| encoding())
}

/// Splits a response file into arguments the way the selected driver does.
pub fn tokenize_response(path: &str, text: &str, quoting: Quoting) -> Result<Vec<String>, Error> {
    match quoting {
        Quoting::Gnu => tokenize_gnu(path, text),
        Quoting::Windows => tokenize_windows(path, text),
    }
}

fn tokenize_gnu(path: &str, text: &str) -> Result<Vec<String>, Error> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_token = false;
    let mut quote: Option<char> = None;
    let mut escaped = false;
    for ch in text.chars() {
        if escaped {
            current.push(ch);
            escaped = false;
        } else if ch == '\\' && quote != Some('\'') {
            escaped = true;
            in_token = true;
        } else if let Some(open) = quote {
            if ch == open {
                quote = None;
            } else {
                current.push(ch);
            }
        } else if ch == '"' || ch == '\'' {
            quote = Some(ch);
            in_token = true;
        } else if ch.is_whitespace() {
            if in_token {
                tokens.push(std::mem::take(&mut current));
                in_token = false;
            }
        } else {
            current.push(ch);
            in_token = true;
        }
    }
    if quote.is_some() || escaped {
        return Err(Error::UnterminatedQuote {
            path: path.to_string(),
        });
    }
    if in_token {
        tokens.push(current);
    }
    Ok(tokens)
}

fn tokenize_windows(path: &str, text: &str) -> Result<Vec<String>, Error> {
    let chars: Vec<char> = text.chars().collect();
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_token = false;
    let mut quoted = false;
    let mut index = 0;
    while index < chars.len() {
        let ch = chars[index];
        if ch == '\\' {
            let mut slashes = 0;
            while chars.get(index) == Some(&'\\') {
                slashes += 1;
                index += 1;
            }
            in_token = true;
            if chars.get(index) == Some(&'"') {
                current.extend(std::iter::repeat_n('\\', slashes / 2));
                if slashes % 2 == 1 {
                    current.push('"');
                    index += 1;
                }
            } else {
                current.extend(std::iter::repeat_n('\\', slashes));
            }
            continue;
        }
        if ch == '"' {
            in_token = true;
            if quoted && chars.get(index + 1) == Some(&'"') {
                current.push('"');
                index += 1;
            } else {
                quoted = !quoted;
            }
        } else if ch.is_whitespace() && !quoted {
            if in_token {
                tokens.push(std::mem::take(&mut current));
                in_token = false;
            }
        } else {
            current.push(ch);
            in_token = true;
        }
        index += 1;
    }
    if quoted {
        return Err(Error::UnterminatedQuote {
            path: path.to_string(),
        });
    }
    if in_token {
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
fn describe_mismatch(exported: &[String], expected: &[String]) -> String {
    let at = exported
        .iter()
        .zip(expected)
        .position(|(left, right)| left != right)
        .unwrap_or(exported.len().min(expected.len()));
    format!(
        "exported has {} arguments, action has {}; first difference at {at}: exported {:?}, action {:?}",
        exported.len(),
        expected.len(),
        exported.get(at),
        expected.get(at)
    )
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod lib_tests;
