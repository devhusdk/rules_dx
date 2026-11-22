use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::{Path, PathBuf};

use dx_atomic_fs::path_io::{self, PathIoError};
use dx_deploy_tools::import_allowed;
use serde::{Deserialize, Serialize};

pub const MATRIX_SCHEMA: &str = "release-matrix/v1";
pub const RECORD_SCHEMA: &str = "release-cell/v1";

/// The dir name `qualify` extracts into; it holds a space and non-ASCII letters.
pub const INSTALL_DIR: &str = "dx install \u{e9} \u{fc}";
/// The dir name `qualify` moves the install to before running `version` again.
pub const MOVED_DIR: &str = "moved \u{fc}";

/// The ASCII dir `qualify` stages the consumer workspace in, under the work dir.
pub const CONSUMER_DIR: &str = "consumer";
/// The module name the staged consumer MODULE.bazel declares.
pub const CONSUMER_MODULE: &str = "dx_release_consumer";
/// Template file and the name it takes in the staged consumer workspace.
pub const CONSUMER_FILES: [(&str, &str); 2] = [
    ("consumer.BUILD.bazel", "BUILD.bazel"),
    ("lib.rs", "src/lib.rs"),
];

/// The `//config` package the root-only `@@//config:toolchains_on` labels of the repeated patches name.
pub const CONSUMER_CONFIG_BUILD: &str = "alias(
    name = \"toolchains_on\",
    actual = \"@rules_dx//config:toolchains_on\",
    visibility = [\"//visibility:public\"],
)
";

/// The Windows flags the repository `.bazelrc` sets: a short output root and symlinks.
const WINDOWS_BAZELRC: &str = "startup --output_user_root=C:/bz
startup --windows_enable_symlinks
";

/// Every check `qualify` records, in run order.
pub const CHECKS: [&str; 12] = [
    "format",
    "digest",
    "determinism",
    "extract",
    "members",
    "mode",
    "identity",
    "binary",
    "imports",
    "run",
    "relocate",
    "consumer",
];

const OSES: [&str; 3] = ["linux", "macos", "windows"];
const MACHINES: [&str; 2] = ["x86_64", "aarch64"];
const FORMATS: [&str; 2] = ["tar.gz", "zip"];

/// One release cell from release-matrix/v1.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Cell {
    pub name: String,
    pub os: String,
    pub arch: String,
    pub machine: String,
    pub format: String,
    pub exe: String,
    pub runtime: Vec<String>,
    pub notices: Vec<String>,
}

impl Cell {
    /// Returns the archive file name `<name>.<format>`.
    #[must_use]
    pub fn archive_name(&self) -> String {
        format!("{}.{}", self.name, self.format)
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Matrix {
    schema: String,
    cells: Vec<Cell>,
}

/// A failed release-cell matrix, record, flag, command or staging step.
#[derive(Debug)]
pub enum CellError {
    /// A file operation failed.
    Io(PathIoError),
    /// Staging the consumer workspace failed.
    Stage(Box<CellError>),
    InsideCheckout {
        dir: PathBuf,
        checkout: PathBuf,
    },
    Clear {
        path: PathBuf,
        source: std::io::Error,
    },
    Copy {
        from: PathBuf,
        source: std::io::Error,
    },
    InitWroteNoModule(PathIoError),
    /// A command exited unsuccessfully; `stdout` is clipped and may be empty.
    Command {
        command: String,
        stderr: String,
        stdout: String,
    },
    MatrixJson(serde_json::Error),
    MatrixSchema {
        schema: String,
    },
    MatrixEmpty,
    MatrixDuplicate {
        cell: String,
    },
    MatrixCell {
        cell: String,
        reason: String,
    },
    RecordJson {
        path: PathBuf,
        source: serde_json::Error,
    },
    RecordSchema {
        path: PathBuf,
        schema: String,
    },
    SnippetRulesDx {
        snippet: String,
    },
    SnippetOverride,
    PatchNoTarget {
        label: String,
    },
    PatchOutsidePatches {
        label: String,
    },
    UnknownArgument {
        flag: String,
    },
    NeedsValue {
        flag: String,
    },
    GivenTwice {
        flag: String,
    },
    MissingFlag {
        name: String,
    },
    /// `--flag` was given without `--requires`.
    FlagRequires {
        flag: &'static str,
        requires: &'static str,
    },
    UnknownCell {
        name: String,
    },
    EmptyLicenseId {
        list: String,
    },
    NoVersionLine {
        stdout: String,
    },
    Render(dx_fingerprint::FingerprintError),
}

impl fmt::Display for CellError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(source) => write!(f, "{source}"),
            Self::Stage(source) => write!(f, "stage: {source}"),
            Self::InsideCheckout { dir, checkout } => write!(
                f,
                "{} is inside the checkout {}",
                dir.display(),
                checkout.display()
            ),
            Self::Clear { path, source } => write!(f, "cannot clear {}: {source}", path.display()),
            Self::Copy { from, source } => write!(f, "cannot copy {}: {source}", from.display()),
            Self::InitWroteNoModule(source) => write!(f, "dx init wrote no MODULE.bazel: {source}"),
            Self::Command {
                command,
                stderr,
                stdout,
            } => {
                write!(f, "{command} failed: {stderr}")?;
                if !stdout.is_empty() {
                    write!(f, "\nstdout: {stdout}")?;
                }
                Ok(())
            }
            Self::MatrixJson(source) => write!(f, "release matrix: {source}"),
            Self::MatrixSchema { schema } => write!(
                f,
                "release matrix: schema {schema:?}, want {MATRIX_SCHEMA:?}"
            ),
            Self::MatrixEmpty => f.write_str("release matrix: no cells"),
            Self::MatrixDuplicate { cell } => write!(f, "release matrix: duplicate cell {cell:?}"),
            Self::MatrixCell { cell, reason } => {
                write!(f, "release matrix: cell {cell:?}: {reason}")
            }
            Self::RecordJson { path, source } => write!(f, "record {}: {source}", path.display()),
            Self::RecordSchema { path, schema } => write!(
                f,
                "record {}: schema {schema:?}, want {RECORD_SCHEMA:?}",
                path.display()
            ),
            Self::SnippetRulesDx { snippet } => write!(
                f,
                "dx init snippet must declare rules_dx exactly once: {snippet}"
            ),
            Self::SnippetOverride => {
                f.write_str("dx init snippet already carries a local_path_override")
            }
            Self::PatchNoTarget { label } => write!(f, "override patch //{label} has no target"),
            Self::PatchOutsidePatches { label } => {
                write!(f, "override patch //{label} is outside //patches")
            }
            Self::UnknownArgument { flag } => write!(f, "unknown argument {flag:?}"),
            Self::NeedsValue { flag } => write!(f, "{flag} needs a value"),
            Self::GivenTwice { flag } => write!(f, "{flag} given twice"),
            Self::MissingFlag { name } => write!(f, "missing --{name}"),
            Self::FlagRequires { flag, requires } => write!(f, "--{flag} needs --{requires}"),
            Self::UnknownCell { name } => write!(f, "unknown cell {name:?}"),
            Self::EmptyLicenseId { list } => {
                write!(f, "--accept-licenses {list:?} has an empty id")
            }
            Self::NoVersionLine { stdout } => write!(f, "no `dx <version>` line in {stdout:?}"),
            Self::Render(source) => write!(f, "cannot render JSON: {source}"),
        }
    }
}

impl std::error::Error for CellError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(source) | Self::InitWroteNoModule(source) => Some(source),
            Self::Stage(source) => Some(source.as_ref()),
            Self::Clear { source, .. } | Self::Copy { source, .. } => Some(source),
            Self::MatrixJson(source) | Self::RecordJson { source, .. } => Some(source),
            Self::Render(source) => Some(source),
            Self::InsideCheckout { .. }
            | Self::Command { .. }
            | Self::MatrixSchema { .. }
            | Self::MatrixEmpty
            | Self::MatrixDuplicate { .. }
            | Self::MatrixCell { .. }
            | Self::RecordSchema { .. }
            | Self::SnippetRulesDx { .. }
            | Self::SnippetOverride
            | Self::PatchNoTarget { .. }
            | Self::PatchOutsidePatches { .. }
            | Self::UnknownArgument { .. }
            | Self::NeedsValue { .. }
            | Self::GivenTwice { .. }
            | Self::MissingFlag { .. }
            | Self::FlagRequires { .. }
            | Self::UnknownCell { .. }
            | Self::EmptyLicenseId { .. }
            | Self::NoVersionLine { .. } => None,
        }
    }
}

impl From<PathIoError> for CellError {
    fn from(source: PathIoError) -> Self {
        Self::Io(source)
    }
}

/// Parses and validates one release-matrix/v1 document.
///
/// # Errors
///
/// Returns an error when the operation fails.
pub fn parse_matrix(text: &str) -> Result<Vec<Cell>, CellError> {
    let matrix: Matrix = serde_json::from_str(text).map_err(CellError::MatrixJson)?;
    if matrix.schema != MATRIX_SCHEMA {
        return Err(CellError::MatrixSchema {
            schema: matrix.schema,
        });
    }
    if matrix.cells.is_empty() {
        return Err(CellError::MatrixEmpty);
    }
    let mut names = BTreeSet::new();
    for cell in &matrix.cells {
        if !names.insert(cell.name.as_str()) {
            return Err(CellError::MatrixDuplicate {
                cell: cell.name.clone(),
            });
        }
        if let Some(reason) = cell_error(cell) {
            return Err(CellError::MatrixCell {
                cell: cell.name.clone(),
                reason,
            });
        }
    }
    Ok(matrix.cells)
}

fn cell_error(cell: &Cell) -> Option<String> {
    let words = [
        ("os", &cell.os, &OSES[..]),
        ("machine", &cell.machine, &MACHINES[..]),
        ("format", &cell.format, &FORMATS[..]),
    ];
    for (field, value, allowed) in words {
        if !allowed.contains(&value.as_str()) {
            return Some(format!("{field} {value:?}, want one of {allowed:?}"));
        }
    }
    if cell.name.is_empty() || cell.arch.is_empty() {
        return Some("name and arch must be non-empty".to_owned());
    }
    let mut members: BTreeSet<&str> = ["LICENSE", "NOTICE", "IDENTITY.txt"].into_iter().collect();
    for member in std::iter::once(&cell.exe).chain(&cell.runtime) {
        if member.contains('/') {
            return Some(format!("member {member:?} must be a top-level name"));
        }
        if let Some(reason) = dx_deploy_tools::member_name_error(member) {
            return Some(reason);
        }
        if !members.insert(member) {
            return Some(format!("member {member:?} is listed twice"));
        }
    }
    let mut notices = BTreeSet::new();
    for notice in &cell.notices {
        if notice.contains('/') {
            return Some(format!("notice {notice:?} must be a top-level name"));
        }
        if let Some(reason) = dx_deploy_tools::member_name_error(notice) {
            return Some(reason);
        }
        if !notices.insert(notice) {
            return Some(format!("notice {notice:?} is listed twice"));
        }
    }
    None
}

/// A check outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Verdict {
    Ok,
    Failed,
    Skipped,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Check {
    pub name: String,
    pub verdict: Verdict,
    pub detail: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArchiveFacts {
    pub name: String,
    pub sha256: String,
    pub bytes: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BinaryFacts {
    pub sha256: String,
    pub bytes: u64,
    pub format: String,
    pub machine: String,
    pub abi: String,
    pub needed: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Member {
    pub name: String,
    pub mode: u32,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunnerFacts {
    pub os: String,
    pub arch: String,
}

/// One measured release-cell/v1 record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CellRecord {
    pub schema: String,
    pub cell: String,
    pub archive: ArchiveFacts,
    pub binary: BinaryFacts,
    pub members: Vec<Member>,
    pub sha: String,
    pub version: String,
    pub workflow_ref: String,
    pub workflow_sha: String,
    pub run_id: String,
    pub job: String,
    pub runner: RunnerFacts,
    pub checks: Vec<Check>,
    pub result: Verdict,
}

impl CellRecord {
    /// Renders the record as pretty ASCII JSON.
    ///
    /// # Errors
    ///
    /// Returns an error when the record cannot be rendered as JSON.
    pub fn render(&self) -> Result<String, CellError> {
        crate::pretty_json(self).map_err(CellError::Render)
    }
}

/// What one command printed and whether it succeeded.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RunOutput {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
}

/// Runs one command; tests inject a fake so no binary or Bazel runs.
pub trait Runner {
    fn run(&self, program: &Path, args: &[&str], cwd: &Path) -> RunOutput;
}

/// The consumer template and the `rules_dx` checkout the staged workspace depends on.
#[derive(Debug, Clone)]
pub struct Consumer {
    pub template: PathBuf,
    pub checkout: PathBuf,
    /// Licence ids `dx init` accepts in the staged consumer. The release workflow owns the
    /// list, so this crate never names a licence itself.
    pub accept_licenses: Vec<String>,
}

/// The debug symbols archive of a Windows cell and its independent rebuild.
#[derive(Debug, Clone)]
pub struct Symbols {
    pub archive: PathBuf,
    pub rebuild: PathBuf,
}

/// Everything `qualify` needs to measure one cell on its native runner.
#[derive(Debug, Clone)]
pub struct QualifyInputs {
    pub cell: Cell,
    pub archive: PathBuf,
    pub rebuild: PathBuf,
    pub work: PathBuf,
    pub sha: String,
    pub version: String,
    pub workflow_ref: String,
    pub workflow_sha: String,
    pub run_id: String,
    pub job: String,
    pub runner_os: String,
    pub runner_arch: String,
    pub consumer: Option<Consumer>,
    pub symbols: Option<Symbols>,
}

struct Verdicts(Vec<Check>);

impl Verdicts {
    fn note(&mut self, name: &str, outcome: Result<(), String>) {
        let (verdict, detail) = match outcome {
            Ok(()) => (Verdict::Ok, String::new()),
            Err(detail) => (Verdict::Failed, detail),
        };
        self.0.push(Check {
            name: name.to_owned(),
            verdict,
            detail,
        });
    }
}

fn magic_format(bytes: &[u8]) -> &'static str {
    if bytes.starts_with(b"PK\x03\x04") {
        "zip"
    } else if bytes.starts_with(b"\x1f\x8b") {
        "tar.gz"
    } else {
        "unknown"
    }
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn sha256_file(path: &Path) -> Result<(String, u64), String> {
    let read_error = |error: std::io::Error| format!("cannot read {}: {error}", path.display());
    let digest = dx_digest::sha256_file_hex(path).map_err(read_error)?;
    let size = std::fs::metadata(path).map_err(read_error)?.len();
    Ok((digest, size))
}

fn container_for(os: &str) -> &'static str {
    match os {
        "linux" => "elf",
        "macos" => "macho",
        _ => "pe",
    }
}

const STDOUT_HEAD: usize = 3000;
const STDOUT_TAIL: usize = 1000;

/// Keeps the head and the tail of long output, cutting on char boundaries.
fn clip(text: &str) -> String {
    if text.len() <= STDOUT_HEAD + STDOUT_TAIL {
        return text.to_owned();
    }
    let mut head = STDOUT_HEAD;
    while !text.is_char_boundary(head) {
        head -= 1;
    }
    let mut tail = text.len() - STDOUT_TAIL;
    while !text.is_char_boundary(tail) {
        tail += 1;
    }
    format!(
        "{}\n...\n{}",
        text.get(..head).unwrap_or(""),
        text.get(tail..).unwrap_or("")
    )
}

fn ran(
    runner: &dyn Runner,
    program: &Path,
    args: &[&str],
    cwd: &Path,
) -> Result<String, CellError> {
    let out = runner.run(program, args, cwd);
    if out.success {
        return Ok(out.stdout);
    }
    // `dx check` prints its findings on stdout and only the progress log on stderr.
    Err(CellError::Command {
        command: format!("{} {}", program.display(), args.join(" ")),
        stderr: out.stderr.trim().to_owned(),
        stdout: clip(out.stdout.trim()),
    })
}

fn starlark_string(text: &str) -> String {
    format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\""))
}

/// The one place that says where the consumer gets `rules_dx` from.
fn rules_dx_override(checkout: &Path) -> String {
    format!(
        "local_path_override(\n    module_name = \"rules_dx\",\n    path = {},\n)\n",
        starlark_string(&checkout.to_string_lossy()),
    )
}

const OVERRIDE_CALLS: [&str; 2] = ["archive_override(", "single_version_override("];

fn string_arg<'a>(call: &'a str, key: &str) -> Option<&'a str> {
    let rest = call.split_once(&format!("{key} = \""))?.1;
    rest.split_once('"').map(|(value, _)| value)
}

/// The modules of the non-dev `bazel_dep` calls of the `rules_dx` root MODULE.bazel.
fn shipped_deps(root_module: &str) -> Vec<&str> {
    root_module
        .lines()
        .filter(|line| line.starts_with("bazel_dep(") && !line.contains("dev_dependency = True"))
        .filter_map(|line| string_arg(line, "name"))
        .collect()
}

/// The top-level `archive_override` and `single_version_override` calls of the `rules_dx` root
/// MODULE.bazel whose module is a non-dev `bazel_dep` of it.
#[must_use]
pub fn root_overrides(root_module: &str) -> Vec<String> {
    let shipped = shipped_deps(root_module);
    let mut blocks = Vec::new();
    let mut open: Option<Vec<&str>> = None;
    for line in root_module.lines() {
        if let Some(block) = open.as_mut() {
            block.push(line);
            if line.trim_end() == ")" {
                blocks.extend(open.take().map(|block| format!("{}\n", block.join("\n"))));
            }
        } else if OVERRIDE_CALLS.iter().any(|call| line.starts_with(call)) {
            if line.trim_end().ends_with(')') {
                blocks.push(format!("{line}\n"));
            } else {
                open = Some(vec![line]);
            }
        }
    }
    blocks.retain(|block| {
        string_arg(block, "module_name").is_some_and(|module| shipped.contains(&module))
    });
    blocks
}

/// The `(package, file)` of each `//patches/...` label in `overrides`.
///
/// # Errors
///
/// Returns an error when the operation fails.
pub fn override_patches(overrides: &[String]) -> Result<Vec<(String, String)>, CellError> {
    let mut patches = Vec::new();
    for block in overrides {
        for label in block.split('"').skip(1).step_by(2) {
            let Some(label) = label.strip_prefix("//") else {
                continue;
            };
            let Some((package, file)) = label.split_once(':') else {
                return Err(CellError::PatchNoTarget {
                    label: label.to_owned(),
                });
            };
            if !package.starts_with("patches/") || file.contains('/') {
                return Err(CellError::PatchOutsidePatches {
                    label: label.to_owned(),
                });
            }
            patches.push((package.to_owned(), file.to_owned()));
        }
    }
    Ok(patches)
}

/// Builds the consumer MODULE.bazel from the `dx init` snippet and the `rules_dx` checkout.
///
/// # Errors
///
/// Returns an error when the operation fails.
pub fn consumer_module(
    init_module: &str,
    root_module: &str,
    checkout: &Path,
) -> Result<String, CellError> {
    let mut kept = Vec::new();
    let mut in_module = false;
    for line in init_module.lines() {
        if line.starts_with("module(") {
            in_module = !line.trim_end().ends_with(')');
        } else if in_module {
            in_module = line.trim_end() != ")";
        } else {
            kept.push(line);
        }
    }
    let snippet = kept.join("\n");
    let snippet = snippet.trim();
    if snippet.matches("bazel_dep(name = \"rules_dx\", ").count() != 1 {
        return Err(CellError::SnippetRulesDx {
            snippet: snippet.to_owned(),
        });
    }
    if snippet.contains("local_path_override(") {
        return Err(CellError::SnippetOverride);
    }
    Ok(format!(
        "module(name = {})\n\n{}\n{}{}",
        starlark_string(CONSUMER_MODULE),
        snippet.trim_end(),
        rules_dx_override(checkout),
        root_overrides(root_module).concat(),
    ))
}

/// Builds the consumer workspace the way a user adopts dx and returns its dir.
///
/// The template supplies sources only; `exe init` writes the toolchain wiring.
///
/// # Errors
///
/// Returns an error when the operation fails.
pub fn stage_consumer(
    consumer: &Consumer,
    exe: &Path,
    work: &Path,
    windows: bool,
    runner: &dyn Runner,
) -> Result<PathBuf, CellError> {
    stage_consumer_in(consumer, exe, work, windows, runner)
        .map_err(|error| CellError::Stage(Box::new(error)))
}

fn stage_consumer_in(
    consumer: &Consumer,
    exe: &Path,
    work: &Path,
    windows: bool,
    runner: &dyn Runner,
) -> Result<PathBuf, CellError> {
    let dir = work.join(CONSUMER_DIR);
    if dir.starts_with(&consumer.checkout) {
        return Err(CellError::InsideCheckout {
            dir,
            checkout: consumer.checkout.clone(),
        });
    }
    if dir.exists() {
        std::fs::remove_dir_all(&dir).map_err(|source| CellError::Clear {
            path: dir.clone(),
            source,
        })?;
    }
    path_io::create_dir_all(&dir)?;
    let copy = |from: PathBuf, to: &str| {
        let to = dir.join(to);
        if let Some(parent) = to.parent() {
            path_io::create_dir_all(parent)?;
        }
        std::fs::copy(&from, to)
            .map(drop)
            .map_err(|source| CellError::Copy { from, source })
    };
    let workspace = dir.to_string_lossy().into_owned();
    let mut init = vec![
        "init",
        "--apply",
        "--no-generate",
        "--workspace",
        &workspace,
    ];
    for id in &consumer.accept_licenses {
        init.extend(["--accept-license", id.as_str()]);
    }
    ran(runner, exe, &init, &dir)?;
    let module_path = dir.join("MODULE.bazel");
    let written = path_io::read_to_string(&module_path).map_err(CellError::InitWroteNoModule)?;
    let root = path_io::read_to_string(&consumer.checkout.join("MODULE.bazel"))?;
    let module = consumer_module(&written, &root, &consumer.checkout)?;
    path_io::write_scratch(&module_path, module.as_bytes())?;
    for (package, file) in override_patches(&root_overrides(&root))? {
        let build = dir.join(&package).join("BUILD.bazel");
        if !build.exists() {
            path_io::create_dir_all(&dir.join(&package))?;
            path_io::write_scratch(&build, b"")?;
        }
        copy(
            consumer.checkout.join(&package).join(&file),
            &format!("{package}/{file}"),
        )?;
    }
    path_io::create_dir_all(&dir.join("config"))?;
    path_io::write_scratch(
        &dir.join("config/BUILD.bazel"),
        CONSUMER_CONFIG_BUILD.as_bytes(),
    )?;
    for (source, dest) in CONSUMER_FILES {
        copy(consumer.template.join(source), dest)?;
    }
    copy(consumer.checkout.join(".bazelversion"), ".bazelversion")?;
    if windows {
        let rc = dir.join(".bazelrc");
        let mut text = std::fs::read_to_string(&rc).unwrap_or_default();
        if !text.is_empty() && !text.ends_with('\n') {
            text.push('\n');
        }
        text.push_str(WINDOWS_BAZELRC);
        path_io::write_scratch(&rc, text.as_bytes())?;
    }
    Ok(dir)
}

fn version_check(runner: &dyn Runner, exe: &Path, cwd: &Path, version: &str) -> Result<(), String> {
    let stdout = ran(runner, exe, &["--version"], cwd).map_err(|error| error.to_string())?;
    if stdout.contains(version) {
        return Ok(());
    }
    Err(format!(
        "version printed {:?}, want it to contain {version:?}",
        stdout.trim()
    ))
}

fn parse_identity(text: &str) -> BTreeMap<String, String> {
    text.lines()
        .filter_map(|line| line.split_once(": "))
        .map(|(key, value)| (key.to_owned(), value.to_owned()))
        .collect()
}

#[cfg(unix)]
fn installed_mode(path: &Path) -> Option<u32> {
    use std::os::unix::fs::PermissionsExt as _;
    std::fs::metadata(path)
        .ok()
        .map(|meta| meta.permissions().mode() & 0o7777)
}

#[cfg(not(unix))]
fn installed_mode(_path: &Path) -> Option<u32> {
    Some(0o755)
}

/// Notes the member list and file modes of an extracted install.
fn note_layout(
    cell: &Cell,
    listing: &[(String, u32, String)],
    exe: &Path,
    verdicts: &mut Verdicts,
) {
    let have: BTreeSet<String> = listing.iter().map(|entry| entry.0.clone()).collect();
    let want: BTreeSet<String> = ["LICENSE", "NOTICE", "IDENTITY.txt", cell.exe.as_str()]
        .into_iter()
        .chain(cell.runtime.iter().map(String::as_str))
        .map(str::to_owned)
        .chain(
            cell.notices
                .iter()
                .map(|notice| format!("notices/{notice}")),
        )
        .collect();
    verdicts.note(
        "members",
        if have == want {
            Ok(())
        } else {
            Err(format!(
                "missing {:?}, unexpected {:?}",
                want.difference(&have).collect::<Vec<_>>(),
                have.difference(&want).collect::<Vec<_>>()
            ))
        },
    );
    let mut wrong: Vec<String> = listing
        .iter()
        .filter(|(name, mode, _)| *mode != if *name == cell.exe { 0o755 } else { 0o644 })
        .map(|(name, mode, _)| format!("{name} has mode {mode:o}"))
        .collect();
    if have.contains(cell.exe.as_str()) && installed_mode(exe) != Some(0o755) {
        wrong.push(format!("extracted {} is not mode 755", cell.exe));
    }
    verdicts.note(
        "mode",
        if wrong.is_empty() {
            Ok(())
        } else {
            Err(wrong.join("; "))
        },
    );
}

/// Notes the identity, binary format and imports of an extracted install.
fn note_binary(
    inputs: &QualifyInputs,
    install: &Path,
    exe: &Path,
    verdicts: &mut Verdicts,
    binary: &mut BinaryFacts,
) {
    let cell = &inputs.cell;
    let identity =
        parse_identity(&std::fs::read_to_string(install.join("IDENTITY.txt")).unwrap_or_default());
    let mismatched: Vec<String> = [
        ("name", &cell.name),
        ("arch", &cell.machine),
        ("os", &cell.os),
    ]
    .into_iter()
    .filter(|(key, want)| identity.get(*key) != Some(*want))
    .map(|(key, want)| format!("IDENTITY {key} is {:?}, want {want:?}", identity.get(key)))
    .collect();
    verdicts.note(
        "identity",
        if mismatched.is_empty() {
            Ok(())
        } else {
            Err(mismatched.join("; "))
        },
    );
    let image = std::fs::read(exe)
        .map_err(|error| format!("cannot read {}: {error}", cell.exe))
        .and_then(|bytes| {
            binary.sha256 = dx_digest::sha256_hex(&bytes);
            binary.bytes = bytes.len() as u64;
            dx_binfmt::inspect(&bytes).map_err(|error| error.to_string())
        })
        .and_then(|images| match <[dx_binfmt::Image; 1]>::try_from(images) {
            Ok([image]) => Ok(image),
            Err(images) => Err(format!("want one image, found {}", images.len())),
        });
    if let Ok(image) = &image {
        image.format.name().clone_into(&mut binary.format);
        binary.machine.clone_from(&image.machine);
        binary.abi = image.abi.name();
        binary.needed.clone_from(&image.needed);
    }
    verdicts.note(
        "binary",
        image.as_ref().map_err(String::clone).and_then(|image| {
            let want = container_for(&cell.os);
            if image.format.name() != want {
                return Err(format!("container {}, want {want}", image.format.name()));
            }
            if image.machine != cell.machine {
                return Err(format!("machine {}, want {}", image.machine, cell.machine));
            }
            Ok(())
        }),
    );
    verdicts.note(
        "imports",
        image
            .map_err(|error| format!("no image: {error}"))
            .and_then(|image| {
                let missing: Vec<&String> = image
                    .needed
                    .iter()
                    .filter(|needed| !import_allowed(&cell.os, needed, &cell.runtime))
                    .collect();
                if missing.is_empty() {
                    Ok(())
                } else {
                    Err(format!("not shipped and not a system import: {missing:?}"))
                }
            }),
    );
}

/// Notes the sidecar digest and the rebuild determinism of the archive.
fn note_digests(
    inputs: &QualifyInputs,
    archive: &ArchiveFacts,
    readable: Result<(), String>,
    verdicts: &mut Verdicts,
) {
    let mut sidecar = inputs.archive.clone().into_os_string();
    sidecar.push(".sha256");
    verdicts.note(
        "digest",
        readable.and_then(|()| {
            let text = std::fs::read_to_string(&sidecar)
                .map_err(|error| format!("cannot read the .sha256 sidecar: {error}"))?;
            let listed = text.split_whitespace().next().unwrap_or_default();
            let listed = listed.strip_prefix('\\').unwrap_or(listed);
            if listed == archive.sha256 {
                Ok(())
            } else {
                Err(format!(
                    "sidecar lists {listed:?}, archive is {}",
                    archive.sha256
                ))
            }
        }),
    );
    verdicts.note(
        "determinism",
        sha256_file(&inputs.rebuild)
            .and_then(|(rebuilt, _)| {
                if rebuilt == archive.sha256 {
                    Ok(())
                } else {
                    Err(format!(
                        "rebuild is {rebuilt}, archive is {}; {}",
                        archive.sha256,
                        differing_members(inputs)
                    ))
                }
            })
            .and_then(|()| match &inputs.symbols {
                Some(symbols) => symbols_match(symbols),
                None => Ok(()),
            }),
    );
}

/// Builds, checks and runs one extracted install; called only after a clean extraction.
fn installed_checks(
    inputs: &QualifyInputs,
    listing: &[(String, u32, String)],
    runner: &dyn Runner,
    verdicts: &mut Verdicts,
    binary: &mut BinaryFacts,
) {
    let cell = &inputs.cell;
    let install = inputs.work.join(INSTALL_DIR);
    let exe = install.join(&cell.exe);
    note_layout(cell, listing, &exe, verdicts);
    note_binary(inputs, &install, &exe, verdicts, binary);
    verdicts.note(
        "run",
        ran(runner, &exe, &["help"], &install)
            .map_err(|error| error.to_string())
            .and_then(|_| version_check(runner, &exe, &install, &inputs.version)),
    );
    let moved = inputs.work.join(MOVED_DIR);
    let renamed = rename_with_retry(&install, &moved, RENAME_ATTEMPTS, RENAME_BACKOFF)
        .map_err(|error| format!("cannot move the install: {error}"));
    let home = if renamed.is_ok() { moved } else { install };
    let moved_exe = home.join(&cell.exe);
    verdicts.note(
        "relocate",
        renamed.and_then(|()| version_check(runner, &moved_exe, &home, &inputs.version)),
    );
    let Some(consumer) = &inputs.consumer else {
        verdicts.0.push(Check {
            name: "consumer".to_owned(),
            verdict: Verdict::Skipped,
            detail: "no --consumer".to_owned(),
        });
        return;
    };
    verdicts.note(
        "consumer",
        stage_consumer(
            consumer,
            &moved_exe,
            &inputs.work,
            cell.os == "windows",
            runner,
        )
        .map_err(|error| error.to_string())
        .and_then(|dir| {
            [
                vec!["build", "//..."],
                vec!["test", "//..."],
                vec!["check", "--diff", "//..."],
            ]
            .into_iter()
            .try_for_each(|args| {
                ran(runner, &moved_exe, &args, &dir)
                    .map(drop)
                    .map_err(|error| error.to_string())
            })
        }),
    );
}

const RENAME_ATTEMPTS: u32 = 8;
const RENAME_BACKOFF: std::time::Duration = std::time::Duration::from_millis(100);

/// Renames `from` to `to`, retrying with a doubling backoff. On Windows a lingering image
/// handle or a virus scan can make the rename fail briefly with "Access is denied".
fn rename_with_retry(
    from: &Path,
    to: &Path,
    attempts: u32,
    first_backoff: std::time::Duration,
) -> std::io::Result<()> {
    let mut backoff = first_backoff;
    let mut attempt = 1;
    loop {
        match std::fs::rename(from, to) {
            Err(_) if attempt < attempts => {
                std::thread::sleep(backoff);
                backoff = backoff.saturating_mul(2);
                attempt += 1;
            }
            result => return result,
        }
    }
}

/// Names the members that differ between the archive and its rebuild.
fn differing_members(inputs: &QualifyInputs) -> String {
    let list = |archive: &Path, dir: &str| {
        dx_deploy_tools::archive_extract(archive, &inputs.work.join(dir))
            .map_err(|error| format!("cannot extract {}: {error}", archive.display()))
    };
    let (first, second) = match (
        list(&inputs.archive, "determinism-archive"),
        list(&inputs.rebuild, "determinism-rebuild"),
    ) {
        (Ok(first), Ok(second)) => (first, second),
        (Err(error), _) | (_, Err(error)) => return error,
    };
    let mut names = Vec::new();
    for (name, mode, sha256) in &first {
        match second.iter().find(|(other, _, _)| other == name) {
            None => names.push(format!("{name} (missing from rebuild)")),
            Some((_, other_mode, other)) if other != sha256 || other_mode != mode => {
                names.push(name.clone());
            }
            Some(_) => {}
        }
    }
    for (name, _, _) in &second {
        if !first.iter().any(|(other, _, _)| other == name) {
            names.push(format!("{name} (only in rebuild)"));
        }
    }
    if names.is_empty() {
        "all members match, so the archive framing differs".to_owned()
    } else {
        format!("differing members: {}", names.join(", "))
    }
}

/// Checks that the symbols archive equals its rebuild.
fn symbols_match(symbols: &Symbols) -> Result<(), String> {
    let (first, _) = sha256_file(&symbols.archive)?;
    let (second, _) = sha256_file(&symbols.rebuild)?;
    if first == second {
        Ok(())
    } else {
        Err(format!(
            "symbols rebuild is {second}, symbols archive is {first}"
        ))
    }
}

/// Measures one cell archive on its native runner and returns a record with a verdict per check.
pub fn qualify(inputs: &QualifyInputs, runner: &dyn Runner) -> CellRecord {
    let cell = &inputs.cell;
    let mut verdicts = Verdicts(Vec::new());
    let bytes = std::fs::read(&inputs.archive)
        .map_err(|error| format!("cannot read {}: {error}", inputs.archive.display()));
    let name = file_name(&inputs.archive);
    let archive = ArchiveFacts {
        name: name.clone(),
        sha256: bytes
            .as_ref()
            .map(|bytes| dx_digest::sha256_hex(bytes))
            .unwrap_or_default(),
        bytes: bytes.as_ref().map_or(0, |bytes| bytes.len() as u64),
    };
    verdicts.note(
        "format",
        bytes.as_ref().map_err(String::clone).and_then(|bytes| {
            let found = magic_format(bytes);
            if found != cell.format {
                return Err(format!("archive is {found}, want {}", cell.format));
            }
            if name != cell.archive_name() {
                return Err(format!(
                    "archive is named {name}, want {}",
                    cell.archive_name()
                ));
            }
            Ok(())
        }),
    );
    note_digests(
        inputs,
        &archive,
        bytes.as_ref().map(drop).map_err(String::clone),
        &mut verdicts,
    );
    let listing = dx_deploy_tools::archive_extract(&inputs.archive, &inputs.work.join(INSTALL_DIR))
        .map_err(|error| format!("cannot extract: {error}"));
    verdicts.note("extract", listing.as_ref().map(drop).map_err(String::clone));
    let mut binary = BinaryFacts::default();
    let members = if let Ok(listing) = &listing {
        installed_checks(inputs, listing, runner, &mut verdicts, &mut binary);
        listing
            .iter()
            .map(|(name, mode, sha256)| Member {
                name: name.clone(),
                mode: *mode,
                sha256: sha256.clone(),
            })
            .collect()
    } else {
        for name in &CHECKS[4..] {
            verdicts.note(name, Err("blocked: extract failed".to_owned()));
        }
        Vec::new()
    };
    let result = if verdicts.0.iter().all(|check| check.verdict == Verdict::Ok) {
        Verdict::Ok
    } else {
        Verdict::Failed
    };
    CellRecord {
        schema: RECORD_SCHEMA.to_owned(),
        cell: cell.name.clone(),
        archive,
        binary,
        members,
        sha: inputs.sha.clone(),
        version: inputs.version.clone(),
        workflow_ref: inputs.workflow_ref.clone(),
        workflow_sha: inputs.workflow_sha.clone(),
        run_id: inputs.run_id.clone(),
        job: inputs.job.clone(),
        runner: RunnerFacts {
            os: inputs.runner_os.clone(),
            arch: inputs.runner_arch.clone(),
        },
        checks: verdicts.0,
        result,
    }
}

/// The release identity every record must carry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Expected {
    pub sha: String,
    pub version: String,
    pub workflow_ref: String,
}

fn record_errors(
    cell: &Cell,
    record: &CellRecord,
    archives: &Path,
    expected: &Expected,
) -> Vec<String> {
    let mut errors = Vec::new();
    let pairs = [
        ("sha", &record.sha, &expected.sha),
        ("version", &record.version, &expected.version),
        ("workflow_ref", &record.workflow_ref, &expected.workflow_ref),
        ("runner os", &record.runner.os, &cell.os),
        ("runner arch", &record.runner.arch, &cell.machine),
        ("binary machine", &record.binary.machine, &cell.machine),
        ("archive name", &record.archive.name, &cell.archive_name()),
    ];
    for (field, have, want) in pairs {
        if have != want {
            errors.push(format!("{field} is {have:?}, want {want:?}"));
        }
    }
    match sha256_file(&archives.join(&record.archive.name)) {
        Ok((sha256, bytes)) if sha256 == record.archive.sha256 && bytes == record.archive.bytes => {
        }
        Ok((sha256, _)) => errors.push(format!(
            "archive {} hashes to {sha256}, record says {}",
            record.archive.name, record.archive.sha256
        )),
        Err(error) => errors.push(error),
    }
    let names: Vec<&str> = record
        .checks
        .iter()
        .map(|check| check.name.as_str())
        .collect();
    if names != CHECKS {
        errors.push(format!("checks are {names:?}, want {CHECKS:?}"));
    }
    for check in record
        .checks
        .iter()
        .filter(|check| check.verdict != Verdict::Ok)
    {
        errors.push(format!(
            "check {} is {:?}: {}",
            check.name, check.verdict, check.detail
        ));
    }
    if record.result != Verdict::Ok {
        errors.push(format!("result is {:?}", record.result));
    }
    errors
}

fn read_record(path: &Path) -> Result<CellRecord, CellError> {
    let text = path_io::read_to_string(path)?;
    let record: CellRecord =
        serde_json::from_str(&text).map_err(|source| CellError::RecordJson {
            path: path.to_path_buf(),
            source,
        })?;
    if record.schema != RECORD_SCHEMA {
        return Err(CellError::RecordSchema {
            path: path.to_path_buf(),
            schema: record.schema,
        });
    }
    Ok(record)
}

/// What verification found for one matrix cell.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CellOutcome {
    pub cell: String,
    pub record: Option<CellRecord>,
    pub reasons: Vec<String>,
}

impl CellOutcome {
    #[must_use]
    pub fn qualified(&self) -> bool {
        self.reasons.is_empty()
    }
}

/// Per-cell outcomes in matrix order, plus problems no cell owns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Verification {
    pub cells: Vec<CellOutcome>,
    pub stray: Vec<String>,
}

impl Verification {
    pub fn ok(&self) -> bool {
        self.stray.is_empty() && self.cells.iter().all(CellOutcome::qualified)
    }
}

/// Checks every `*.json` record in `records` against the matrix, the archives and the release identity,
/// and reports each cell on its own instead of stopping at the first rejection.
#[must_use]
pub fn verify_each(
    matrix: &[Cell],
    records: &Path,
    archives: &Path,
    expected: &Expected,
) -> Verification {
    let mut cells: Vec<CellOutcome> = matrix
        .iter()
        .map(|cell| CellOutcome {
            cell: cell.name.clone(),
            record: None,
            reasons: Vec::new(),
        })
        .collect();
    let mut stray = Vec::new();
    let mut paths: Vec<PathBuf> = match std::fs::read_dir(records) {
        Ok(entries) => entries
            .filter_map(|entry| entry.ok().map(|entry| entry.path()))
            .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
            .filter(|path| !path.to_string_lossy().ends_with(".spdx.json"))
            .collect(),
        Err(error) => {
            stray.push(format!(
                "cannot read records {}: {error}",
                records.display()
            ));
            Vec::new()
        }
    };
    paths.sort();
    for path in paths {
        let record = match read_record(&path) {
            Ok(record) => record,
            Err(error) => {
                stray.push(error.to_string());
                continue;
            }
        };
        let Some((outcome, cell)) = cells
            .iter_mut()
            .zip(matrix)
            .find(|(_, cell)| cell.name == record.cell)
        else {
            stray.push(format!(
                "record {}: unknown cell {:?}",
                path.display(),
                record.cell
            ));
            continue;
        };
        if outcome.record.is_some() {
            outcome
                .reasons
                .push(format!("duplicate record {}", path.display()));
            continue;
        }
        outcome
            .reasons
            .extend(record_errors(cell, &record, archives, expected));
        outcome.record = Some(record);
    }
    for outcome in cells.iter_mut().filter(|outcome| outcome.record.is_none()) {
        outcome.reasons.push("no record".to_string());
    }
    Verification { cells, stray }
}

/// Checks every record like `verify_each` and fails on the first problem found anywhere.
///
/// # Errors
///
/// Returns an error when the operation fails.
pub fn verify(
    matrix: &[Cell],
    records: &Path,
    archives: &Path,
    expected: &Expected,
) -> Result<Vec<CellRecord>, Vec<String>> {
    let verification = verify_each(matrix, records, archives, expected);
    if verification.ok() {
        return Ok(verification
            .cells
            .into_iter()
            .filter_map(|outcome| outcome.record)
            .collect());
    }
    let mut errors = verification.stray;
    for outcome in &verification.cells {
        errors.extend(
            outcome
                .reasons
                .iter()
                .map(|reason| format!("cell {}: {reason}", outcome.cell)),
        );
    }
    Err(errors)
}

fn parse_flags<'a>(
    args: &'a [String],
    required: &[&str],
    optional: &[&str],
) -> Result<BTreeMap<&'a str, &'a str>, CellError> {
    let mut flags = BTreeMap::new();
    let mut rest = args;
    while let [flag, tail @ ..] = rest {
        let Some(name) = flag
            .strip_prefix("--")
            .filter(|name| required.contains(name) || optional.contains(name))
        else {
            return Err(CellError::UnknownArgument { flag: flag.clone() });
        };
        let [value, after @ ..] = tail else {
            return Err(CellError::NeedsValue { flag: flag.clone() });
        };
        if flags.insert(name, value.as_str()).is_some() {
            return Err(CellError::GivenTwice { flag: flag.clone() });
        }
        rest = after;
    }
    if let Some(missing) = required.iter().find(|name| !flags.contains_key(*name)) {
        return Err(CellError::MissingFlag {
            name: (*missing).to_owned(),
        });
    }
    Ok(flags)
}

fn flag(flags: &BTreeMap<&str, &str>, name: &str) -> String {
    flags.get(name).copied().unwrap_or_default().to_owned()
}

fn read_matrix(path: &str) -> Result<Vec<Cell>, CellError> {
    parse_matrix(&path_io::read_to_string(Path::new(path))?)
}

pub const QUALIFY_USAGE: &str = "qualify --matrix <release_matrix.json> --cell <name> --archive <path> --rebuild <path> --work <dir> --sha <commit> --version <version> --workflow-ref <ref> --workflow-sha <commit> --run-id <id> --job <name> --out <record.json> [--consumer <template dir> --checkout <rules_dx dir> [--accept-licenses <id,id>]] [--symbols <zip> --symbols-rebuild <zip>]";

pub const VERIFY_USAGE: &str = "verify --matrix <release_matrix.json> --records <dir> --archives <dir> --sha <commit> --version <version> --workflow-ref <ref>";

/// Parses `qualify` flags into inputs for the host runner and the record path.
///
/// # Errors
///
/// Returns an error when the operation fails.
pub fn parse_qualify_args(
    args: &[String],
    runner_os: &str,
    runner_arch: &str,
) -> Result<(QualifyInputs, PathBuf), CellError> {
    let flags = parse_flags(
        args,
        &[
            "matrix",
            "cell",
            "archive",
            "rebuild",
            "work",
            "sha",
            "version",
            "workflow-ref",
            "workflow-sha",
            "run-id",
            "job",
            "out",
        ],
        &[
            "consumer",
            "checkout",
            "accept-licenses",
            "symbols",
            "symbols-rebuild",
        ],
    )?;
    let symbols = match (flags.get("symbols"), flags.get("symbols-rebuild")) {
        (Some(archive), Some(rebuild)) => Some(Symbols {
            archive: PathBuf::from(*archive),
            rebuild: PathBuf::from(*rebuild),
        }),
        (Some(_), None) => {
            return Err(CellError::FlagRequires {
                flag: "symbols",
                requires: "symbols-rebuild",
            });
        }
        (None, Some(_)) => {
            return Err(CellError::FlagRequires {
                flag: "symbols-rebuild",
                requires: "symbols",
            });
        }
        (None, None) => None,
    };
    let accept_licenses = match flags.get("accept-licenses") {
        Some(list) => parse_license_ids(list)?,
        None => Vec::new(),
    };
    let consumer = match (flags.get("consumer"), flags.get("checkout")) {
        (Some(template), Some(checkout)) => Some(Consumer {
            template: PathBuf::from(*template),
            checkout: PathBuf::from(*checkout),
            accept_licenses,
        }),
        (Some(_), None) => {
            return Err(CellError::FlagRequires {
                flag: "consumer",
                requires: "checkout",
            });
        }
        (None, Some(_)) => {
            return Err(CellError::FlagRequires {
                flag: "checkout",
                requires: "consumer",
            });
        }
        (None, None) if flags.contains_key("accept-licenses") => {
            return Err(CellError::FlagRequires {
                flag: "accept-licenses",
                requires: "consumer",
            });
        }
        (None, None) => None,
    };
    let name = flag(&flags, "cell");
    let cell = read_matrix(&flag(&flags, "matrix"))?
        .into_iter()
        .find(|cell| cell.name == name)
        .ok_or(CellError::UnknownCell { name })?;
    let inputs = QualifyInputs {
        cell,
        archive: PathBuf::from(flag(&flags, "archive")),
        rebuild: PathBuf::from(flag(&flags, "rebuild")),
        work: PathBuf::from(flag(&flags, "work")),
        sha: flag(&flags, "sha"),
        version: flag(&flags, "version"),
        workflow_ref: flag(&flags, "workflow-ref"),
        workflow_sha: flag(&flags, "workflow-sha"),
        run_id: flag(&flags, "run-id"),
        job: flag(&flags, "job"),
        runner_os: runner_os.to_owned(),
        runner_arch: runner_arch.to_owned(),
        consumer,
        symbols,
    };
    Ok((inputs, PathBuf::from(flag(&flags, "out"))))
}

/// Splits the comma-separated `--accept-licenses` value; an empty id is a typo, not "none".
fn parse_license_ids(list: &str) -> Result<Vec<String>, CellError> {
    list.split(',')
        .map(|id| {
            let id = id.trim();
            if id.is_empty() {
                Err(CellError::EmptyLicenseId {
                    list: list.to_owned(),
                })
            } else {
                Ok(id.to_owned())
            }
        })
        .collect()
}

/// Parsed `verify` flags.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifyArgs {
    pub matrix: Vec<Cell>,
    pub records: PathBuf,
    pub archives: PathBuf,
    pub expected: Expected,
}

/// Parses `verify` flags and reads the matrix.
///
/// # Errors
///
/// Returns an error when the operation fails.
pub fn parse_verify_args(args: &[String]) -> Result<VerifyArgs, CellError> {
    let flags = parse_flags(
        args,
        &[
            "matrix",
            "records",
            "archives",
            "sha",
            "version",
            "workflow-ref",
        ],
        &[],
    )?;
    Ok(VerifyArgs {
        matrix: read_matrix(&flag(&flags, "matrix"))?,
        records: PathBuf::from(flag(&flags, "records")),
        archives: PathBuf::from(flag(&flags, "archives")),
        expected: Expected {
            sha: flag(&flags, "sha"),
            version: flag(&flags, "version"),
            workflow_ref: flag(&flags, "workflow-ref"),
        },
    })
}

/// Runs `qualify` from flags, writes the record and returns it.
///
/// # Errors
///
/// Returns an error when the operation fails.
pub fn qualify_main(args: &[String], runner: &dyn Runner) -> Result<CellRecord, CellError> {
    let (inputs, out) = parse_qualify_args(args, std::env::consts::OS, std::env::consts::ARCH)?;
    let record = qualify(&inputs, runner);
    path_io::write_scratch(&out, record.render()?.as_bytes())?;
    Ok(record)
}

/// Renders one block per cell in matrix order, then the problems no cell owns.
#[must_use]
pub fn render_verification(verification: &Verification) -> String {
    let mut text = String::new();
    for outcome in &verification.cells {
        if let (Some(record), true) = (&outcome.record, outcome.qualified()) {
            {
                text.push_str(&format!(
                    "{} qualified {} sha256 {} sha {} version {} job {} run {}\n",
                    record.cell,
                    record.archive.name,
                    record.archive.sha256,
                    record.sha,
                    record.version,
                    record.job,
                    record.run_id
                ));
            }
        } else {
            text.push_str(&format!("{} rejected\n", outcome.cell));
            for reason in &outcome.reasons {
                text.push_str(&format!("  {reason}\n"));
            }
        }
    }
    for problem in &verification.stray {
        text.push_str(&format!("stray: {problem}\n"));
    }
    text
}

/// Runs `verify` from flags and returns whether every cell qualified, with the per-cell text.
/// `Err` is only for bad flags or a bad matrix.
///
/// # Errors
///
/// Returns an error when the operation fails.
pub fn verify_main(args: &[String]) -> Result<(bool, String), CellError> {
    let parsed = parse_verify_args(args)?;
    let verification = verify_each(
        &parsed.matrix,
        &parsed.records,
        &parsed.archives,
        &parsed.expected,
    );
    Ok((verification.ok(), render_verification(&verification)))
}

pub const VERSION_USAGE: &str = "version --exe <path>";

/// Takes the version from the first `dx <version>` line of `dx --version` output.
///
/// # Errors
///
/// Returns an error when the operation fails.
pub fn printed_version(stdout: &str) -> Result<String, CellError> {
    stdout
        .lines()
        .find_map(|line| line.strip_prefix("dx "))
        .map(|rest| rest.trim().to_owned())
        .filter(|version| !version.is_empty())
        .ok_or_else(|| CellError::NoVersionLine {
            stdout: stdout.trim().to_owned(),
        })
}

/// Runs `version` from flags: asks the executable for its version and returns it.
///
/// # Errors
///
/// Returns an error when the operation fails.
pub fn version_main(args: &[String], runner: &dyn Runner) -> Result<String, CellError> {
    let flags = parse_flags(args, &["exe"], &[])?;
    let stdout = ran(
        runner,
        Path::new(&flag(&flags, "exe")),
        &["--version"],
        Path::new("."),
    )?;
    printed_version(&stdout)
}

#[cfg(test)]
#[path = "cell_tests.rs"]
pub(crate) mod tests;
