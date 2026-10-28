use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

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
    ("lib.rs", "lib.rs"),
];

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

/// System libraries a shipped binary may import without shipping them; a trailing '*' is a prefix.
pub const SYSTEM_IMPORTS: [(&str, &[&str]); 3] = [
    (
        "linux",
        &[
            "ld-linux-aarch64.so.1",
            "ld-linux-x86-64.so.2",
            "libc.so.6",
            "libdl.so.2",
            "libgcc_s.so.1",
            "libm.so.6",
            "libpthread.so.0",
            "librt.so.1",
        ],
    ),
    (
        "macos",
        &[
            "/System/Library/Frameworks/*",
            "/usr/lib/libSystem.B.dylib",
            "/usr/lib/libc++.1.dylib",
            "/usr/lib/libiconv.2.dylib",
            "/usr/lib/libobjc.A.dylib",
            "/usr/lib/libresolv.9.dylib",
        ],
    ),
    (
        "windows",
        &[
            "advapi32.dll",
            "api-ms-win-*",
            "bcrypt.dll",
            "bcryptprimitives.dll",
            "crypt32.dll",
            "dbghelp.dll",
            "iphlpapi.dll",
            "kernel32.dll",
            "ncrypt.dll",
            "ntdll.dll",
            "ole32.dll",
            "oleaut32.dll",
            "psapi.dll",
            "secur32.dll",
            "shell32.dll",
            "synchronization.dll",
            "ucrtbase.dll",
            "user32.dll",
            "userenv.dll",
            "ws2_32.dll",
        ],
    ),
];

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
    #[serde(default)]
    pub runtime: Vec<String>,
}

impl Cell {
    /// Returns the archive file name `<name>.<format>`.
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

/// Parses and validates one release-matrix/v1 document.
pub fn parse_matrix(text: &str) -> Result<Vec<Cell>, String> {
    let matrix: Matrix =
        serde_json::from_str(text).map_err(|error| format!("release matrix: {error}"))?;
    if matrix.schema != MATRIX_SCHEMA {
        return Err(format!(
            "release matrix: schema {:?}, want {MATRIX_SCHEMA:?}",
            matrix.schema
        ));
    }
    if matrix.cells.is_empty() {
        return Err("release matrix: no cells".to_owned());
    }
    let mut names = BTreeSet::new();
    for cell in &matrix.cells {
        if !names.insert(cell.name.as_str()) {
            return Err(format!("release matrix: duplicate cell {:?}", cell.name));
        }
        cell_error(cell).map_or(Ok(()), |error| {
            Err(format!("release matrix: cell {:?}: {error}", cell.name))
        })?;
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
    let mut members: BTreeSet<&str> = ["NOTICE", "IDENTITY.txt"].into_iter().collect();
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
    pub fn render(&self) -> String {
        crate::render_pretty(&serde_json::to_value(self).unwrap_or_default())
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

/// The consumer template and the rules_dx checkout the staged workspace depends on.
#[derive(Debug, Clone)]
pub struct Consumer {
    pub template: PathBuf,
    pub checkout: PathBuf,
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
    let bytes =
        std::fs::read(path).map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    Ok((dx_digest::sha256_hex(&bytes), bytes.len() as u64))
}

fn container_for(os: &str) -> &'static str {
    match os {
        "linux" => "elf",
        "macos" => "macho",
        _ => "pe",
    }
}

/// Returns whether `needed` is a system import on `os` or one of the shipped runtime members.
pub fn import_allowed(os: &str, needed: &str, runtime: &[String]) -> bool {
    let fold = |text: &str| {
        if os == "windows" {
            text.to_ascii_lowercase()
        } else {
            text.to_owned()
        }
    };
    let needed = fold(needed);
    let base = needed.rsplit('/').next().unwrap_or_default();
    if runtime.iter().any(|member| fold(member) == base) {
        return true;
    }
    SYSTEM_IMPORTS
        .iter()
        .filter(|(system, _)| *system == os)
        .flat_map(|(_, names)| names.iter())
        .any(|pattern| match pattern.strip_suffix('*') {
            Some(prefix) => needed.starts_with(prefix),
            None => needed == *pattern,
        })
}

fn ran(runner: &dyn Runner, program: &Path, args: &[&str], cwd: &Path) -> Result<String, String> {
    let out = runner.run(program, args, cwd);
    if out.success {
        return Ok(out.stdout);
    }
    Err(format!(
        "{} {} failed: {}",
        program.display(),
        args.join(" "),
        out.stderr.trim()
    ))
}

fn starlark_string(text: &str) -> String {
    format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\""))
}

/// The one place that says where the consumer gets rules_dx from.
fn rules_dx_override(checkout: &Path) -> String {
    format!(
        "local_path_override(\n    module_name = \"rules_dx\",\n    path = {},\n)\n",
        starlark_string(&checkout.to_string_lossy()),
    )
}

/// Builds the consumer MODULE.bazel from the `dx init` snippet and the rules_dx checkout.
pub fn consumer_module(snippet: &str, checkout: &Path) -> Result<String, String> {
    if snippet.matches("bazel_dep(name = \"rules_dx\", ").count() != 1 {
        return Err(format!(
            "dx init snippet must declare rules_dx exactly once: {snippet}"
        ));
    }
    if snippet.contains("local_path_override(") {
        return Err("dx init snippet already carries a local_path_override".to_owned());
    }
    Ok(format!(
        "module(name = {})\n\n{}\n{}",
        starlark_string(CONSUMER_MODULE),
        snippet.trim_end(),
        rules_dx_override(checkout),
    ))
}

/// Builds the consumer workspace the way a user adopts dx and returns its dir.
///
/// The template supplies sources only; `exe init` writes the toolchain wiring.
pub fn stage_consumer(
    consumer: &Consumer,
    exe: &Path,
    work: &Path,
    runner: &dyn Runner,
) -> Result<PathBuf, String> {
    let dir = work.join(CONSUMER_DIR);
    if dir.starts_with(&consumer.checkout) {
        return Err(format!(
            "stage: {} is inside the checkout {}",
            dir.display(),
            consumer.checkout.display()
        ));
    }
    if dir.exists() {
        std::fs::remove_dir_all(&dir)
            .map_err(|error| format!("stage: cannot clear {}: {error}", dir.display()))?;
    }
    std::fs::create_dir_all(&dir)
        .map_err(|error| format!("stage: cannot create {}: {error}", dir.display()))?;
    let copy = |from: PathBuf, to: &str| {
        std::fs::copy(&from, dir.join(to))
            .map(drop)
            .map_err(|error| format!("stage: cannot copy {}: {error}", from.display()))
    };
    for (source, dest) in CONSUMER_FILES {
        copy(consumer.template.join(source), dest)?;
    }
    copy(consumer.checkout.join(".bazelversion"), ".bazelversion")?;
    let workspace = dir.to_string_lossy().into_owned();
    ran(
        runner,
        exe,
        &["init", "--apply", "--workspace", &workspace],
        &dir,
    )
    .map_err(|error| format!("stage: {error}"))?;
    let snippet = std::fs::read_to_string(dir.join("MODULE.bazel.snippet"))
        .map_err(|error| format!("stage: dx init wrote no MODULE.bazel.snippet: {error}"))?;
    let module =
        consumer_module(&snippet, &consumer.checkout).map_err(|error| format!("stage: {error}"))?;
    std::fs::write(dir.join("MODULE.bazel"), module)
        .map_err(|error| format!("stage: cannot write MODULE.bazel: {error}"))?;
    Ok(dir)
}

fn version_check(runner: &dyn Runner, exe: &Path, cwd: &Path, version: &str) -> Result<(), String> {
    let stdout = ran(runner, exe, &["version"], cwd)?;
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
    let have: BTreeSet<&str> = listing.iter().map(|entry| entry.0.as_str()).collect();
    let want: BTreeSet<&str> = ["NOTICE", "IDENTITY.txt", cell.exe.as_str()]
        .into_iter()
        .chain(cell.runtime.iter().map(String::as_str))
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
    if have.contains(cell.exe.as_str()) && installed_mode(&exe) != Some(0o755) {
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
    let image = std::fs::read(&exe)
        .map_err(|error| format!("cannot read {}: {error}", cell.exe))
        .and_then(|bytes| {
            binary.sha256 = dx_digest::sha256_hex(&bytes);
            dx_binfmt::inspect(&bytes).map_err(|error| error.to_string())
        })
        .and_then(|images| match <[dx_binfmt::Image; 1]>::try_from(images) {
            Ok([image]) => Ok(image),
            Err(images) => Err(format!("want one image, found {}", images.len())),
        });
    if let Ok(image) = &image {
        binary.format = image.format.name().to_owned();
        binary.machine = image.machine.clone();
        binary.abi = image.abi.name();
        binary.needed = image.needed.clone();
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
    verdicts.note(
        "run",
        ran(runner, &exe, &["help"], &install)
            .and_then(|_| version_check(runner, &exe, &install, &inputs.version)),
    );
    let moved = inputs.work.join(MOVED_DIR);
    let renamed = std::fs::rename(&install, &moved)
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
        stage_consumer(consumer, &moved_exe, &inputs.work, runner).and_then(|dir| {
            ["build", "test", "check"]
                .into_iter()
                .try_for_each(|command| {
                    ran(runner, &moved_exe, &[command, "//..."], &dir).map(drop)
                })
        }),
    );
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
        bytes: bytes.as_ref().map(|bytes| bytes.len() as u64).unwrap_or(0),
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
    let mut sidecar = inputs.archive.clone().into_os_string();
    sidecar.push(".sha256");
    verdicts.note(
        "digest",
        bytes.as_ref().map_err(String::clone).and_then(|_| {
            let text = std::fs::read_to_string(&sidecar)
                .map_err(|error| format!("cannot read the .sha256 sidecar: {error}"))?;
            let listed = text.split_whitespace().next().unwrap_or_default();
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
        sha256_file(&inputs.rebuild).and_then(|(rebuilt, _)| {
            if rebuilt == archive.sha256 {
                Ok(())
            } else {
                Err(format!(
                    "rebuild is {rebuilt}, archive is {}",
                    archive.sha256
                ))
            }
        }),
    );
    let listing = dx_deploy_tools::archive_extract(&inputs.archive, &inputs.work.join(INSTALL_DIR))
        .map_err(|error| format!("cannot extract: {error}"));
    verdicts.note("extract", listing.as_ref().map(drop).map_err(String::clone));
    let mut binary = BinaryFacts::default();
    let members = match &listing {
        Ok(listing) => {
            installed_checks(inputs, listing, runner, &mut verdicts, &mut binary);
            listing
                .iter()
                .map(|(name, mode, sha256)| Member {
                    name: name.clone(),
                    mode: *mode,
                    sha256: sha256.clone(),
                })
                .collect()
        }
        Err(_) => {
            for name in &CHECKS[4..] {
                verdicts.note(name, Err("blocked: extract failed".to_owned()));
            }
            Vec::new()
        }
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

fn read_record(path: &Path) -> Result<CellRecord, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    let record: CellRecord = serde_json::from_str(&text)
        .map_err(|error| format!("record {}: {error}", path.display()))?;
    if record.schema != RECORD_SCHEMA {
        return Err(format!(
            "record {}: schema {:?}, want {RECORD_SCHEMA:?}",
            path.display(),
            record.schema
        ));
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
                stray.push(error);
                continue;
            }
        };
        let Some(index) = matrix.iter().position(|cell| cell.name == record.cell) else {
            stray.push(format!(
                "record {}: unknown cell {:?}",
                path.display(),
                record.cell
            ));
            continue;
        };
        let outcome = &mut cells[index];
        if outcome.record.is_some() {
            outcome
                .reasons
                .push(format!("duplicate record {}", path.display()));
            continue;
        }
        outcome
            .reasons
            .extend(record_errors(&matrix[index], &record, archives, expected));
        outcome.record = Some(record);
    }
    for outcome in cells.iter_mut().filter(|outcome| outcome.record.is_none()) {
        outcome.reasons.push("no record".to_string());
    }
    Verification { cells, stray }
}

/// Checks every record like `verify_each` and fails on the first problem found anywhere.
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
) -> Result<BTreeMap<&'a str, &'a str>, String> {
    let mut flags = BTreeMap::new();
    let mut rest = args;
    while let [flag, tail @ ..] = rest {
        let Some(name) = flag
            .strip_prefix("--")
            .filter(|name| required.contains(name) || optional.contains(name))
        else {
            return Err(format!("unknown argument {flag:?}"));
        };
        let [value, after @ ..] = tail else {
            return Err(format!("{flag} needs a value"));
        };
        if flags.insert(name, value.as_str()).is_some() {
            return Err(format!("{flag} given twice"));
        }
        rest = after;
    }
    if let Some(missing) = required.iter().find(|name| !flags.contains_key(*name)) {
        return Err(format!("missing --{missing}"));
    }
    Ok(flags)
}

fn flag(flags: &BTreeMap<&str, &str>, name: &str) -> String {
    flags.get(name).copied().unwrap_or_default().to_owned()
}

fn read_matrix(path: &str) -> Result<Vec<Cell>, String> {
    let text =
        std::fs::read_to_string(path).map_err(|error| format!("cannot read {path}: {error}"))?;
    parse_matrix(&text)
}

pub const QUALIFY_USAGE: &str = "qualify --matrix <release_matrix.json> --cell <name> --archive <path> --rebuild <path> --work <dir> --sha <commit> --version <version> --workflow-ref <ref> --workflow-sha <commit> --run-id <id> --job <name> --out <record.json> [--consumer <template dir> --checkout <rules_dx dir>]";

pub const VERIFY_USAGE: &str = "verify --matrix <release_matrix.json> --records <dir> --archives <dir> --sha <commit> --version <version> --workflow-ref <ref>";

/// Parses `qualify` flags into inputs for the host runner and the record path.
pub fn parse_qualify_args(
    args: &[String],
    runner_os: &str,
    runner_arch: &str,
) -> Result<(QualifyInputs, PathBuf), String> {
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
        &["consumer", "checkout"],
    )?;
    let consumer = match (flags.get("consumer"), flags.get("checkout")) {
        (Some(template), Some(checkout)) => Some(Consumer {
            template: PathBuf::from(*template),
            checkout: PathBuf::from(*checkout),
        }),
        (Some(_), None) => return Err("--consumer needs --checkout".to_owned()),
        (None, Some(_)) => return Err("--checkout needs --consumer".to_owned()),
        (None, None) => None,
    };
    let name = flag(&flags, "cell");
    let cell = read_matrix(&flag(&flags, "matrix"))?
        .into_iter()
        .find(|cell| cell.name == name)
        .ok_or_else(|| format!("unknown cell {name:?}"))?;
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
    };
    Ok((inputs, PathBuf::from(flag(&flags, "out"))))
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
pub fn parse_verify_args(args: &[String]) -> Result<VerifyArgs, String> {
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
pub fn qualify_main(args: &[String], runner: &dyn Runner) -> Result<CellRecord, String> {
    let (inputs, out) = parse_qualify_args(args, std::env::consts::OS, std::env::consts::ARCH)?;
    let record = qualify(&inputs, runner);
    std::fs::write(&out, record.render())
        .map_err(|error| format!("cannot write {}: {error}", out.display()))?;
    Ok(record)
}

/// Renders one block per cell in matrix order, then the problems no cell owns.
pub fn render_verification(verification: &Verification) -> String {
    let mut text = String::new();
    for outcome in &verification.cells {
        match (&outcome.record, outcome.qualified()) {
            (Some(record), true) => text.push_str(&format!(
                "{} qualified {} sha256 {} sha {} version {} job {} run {}\n",
                record.cell,
                record.archive.name,
                record.archive.sha256,
                record.sha,
                record.version,
                record.job,
                record.run_id
            )),
            _ => {
                text.push_str(&format!("{} rejected\n", outcome.cell));
                for reason in &outcome.reasons {
                    text.push_str(&format!("  {reason}\n"));
                }
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
pub fn verify_main(args: &[String]) -> Result<(bool, String), String> {
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

/// Takes the version from the first `dx <version>` line of `dx version` output.
pub fn printed_version(stdout: &str) -> Result<String, String> {
    stdout
        .lines()
        .find_map(|line| line.strip_prefix("dx "))
        .map(|rest| rest.trim().to_owned())
        .filter(|version| !version.is_empty())
        .ok_or_else(|| format!("no `dx <version>` line in {:?}", stdout.trim()))
}

/// Runs `version` from flags: asks the executable for its version and returns it.
pub fn version_main(args: &[String], runner: &dyn Runner) -> Result<String, String> {
    let flags = parse_flags(args, &["exe"], &[])?;
    let stdout = ran(
        runner,
        Path::new(&flag(&flags, "exe")),
        &["version"],
        Path::new("."),
    )?;
    printed_version(&stdout)
}

#[cfg(test)]
#[path = "cell_tests.rs"]
pub(crate) mod tests;
