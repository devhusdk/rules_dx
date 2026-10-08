#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

//! Layer-2 matrix test executor: runs one quality runner case without host shell tooling.
//!
//! The Starlark rule writes one JSON manifest per case and runs this binary as
//! the test executable. The harness resolves every input through its own
//! runfiles, invokes the quality runner and the print_result printer as child
//! processes, validates the result with quality_result, checks the printed
//! golden bytes, and stages snapshot updates. No Bash, python3, diff, cp, or
//! mkdir processes are launched.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use dx_path::Resolver;
use quality_result::{decode_validated, proto};

pub const MANIFEST_ENV: &str = "MATRIX_HARNESS_MANIFEST";
pub const MANIFEST_VERSION: u32 = 1;
const UPDATE_ENV: &str = "UPDATE_EXPECT";
const UNDECLARED_ENV: &str = "TEST_UNDECLARED_OUTPUTS_DIR";
const TMPDIR_ENV: &str = "TMPDIR";
const TEST_TMPDIR_ENV: &str = "TEST_TMPDIR";
const TEST_SRCDIR_ENV: &str = "TEST_SRCDIR";
const TEST_SRCDIR_TOKEN: &str = "$TEST_SRCDIR";
const MAX_DIFF_LINES: usize = 60;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum HarnessError {
    #[error("missing environment {name}: {detail}")]
    MissingEnv { name: String, detail: String },
    #[error("no runfiles beside {binary}: {detail}")]
    NoRunfiles { binary: String, detail: String },
    #[error("missing runfile {key}: {detail}")]
    MissingRunfile { key: String, detail: String },
    #[error("invalid manifest {path}: {detail}")]
    BadManifest { path: String, detail: String },
    #[error("cannot create {path}: {detail}")]
    UnwritableWorkdir { path: String, detail: String },
    #[error("runner failed: {detail}")]
    RunnerFailed { detail: String },
    #[error("printer failed: {detail}")]
    PrinterFailed { detail: String },
    #[error("invalid result protobuf: {detail}")]
    InvalidResult { detail: String },
    #[error("result rejected: {detail}")]
    BadResult { detail: String },
    #[error("printed result rejected: {detail}")]
    BadPrint { detail: String },
    #[error("golden snapshot mismatch for {name}")]
    GoldenMismatch { name: String },
    #[error("cannot stage snapshot update at {path}: {detail}")]
    UnwritableUpdate { path: String, detail: String },
    #[error("cannot read expected file {path}: {detail}")]
    UnreadableExpected { path: String, detail: String },
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Mapping {
    pub workspace: String,
    pub key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolFile {
    pub mirror_rel: String,
    pub key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnvEntry {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManifestTool {
    #[serde(default)]
    pub binary_key: Option<String>,
    #[serde(default)]
    pub files: Vec<ToolFile>,
    #[serde(default)]
    pub config: Option<String>,
    #[serde(default)]
    pub edition: Option<String>,
    #[serde(default)]
    pub env: Vec<EnvEntry>,
    #[serde(default)]
    pub upstream_keys: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub schema_version: u32,
    pub name: String,
    pub producer: String,
    pub capability: String,
    pub stages: Vec<String>,
    pub sources: Vec<Mapping>,
    pub siblings: Vec<Mapping>,
    #[serde(default)]
    pub tools: BTreeMap<String, ManifestTool>,
    pub runner_key: String,
    pub printer_key: String,
    pub expected_key: String,
    pub update_file: String,
    pub update_hint: String,
}

pub struct Resolved {
    pub manifest: Manifest,
    pub runner: PathBuf,
    pub printer: PathBuf,
    pub expected: PathBuf,
    pub sources: Vec<(String, PathBuf)>,
    pub siblings: Vec<(String, PathBuf)>,
    pub tools: BTreeMap<String, ResolvedTool>,
}

pub struct ResolvedTool {
    pub binary: Option<PathBuf>,
    pub files: Vec<(String, PathBuf)>,
    pub upstream: Vec<PathBuf>,
}

fn lookup(resolver: &Resolver, key: &str) -> Result<PathBuf, HarnessError> {
    resolver
        .lookup_from(key, "")
        .map_err(|error| HarnessError::MissingRunfile {
            key: key.to_owned(),
            detail: error.to_string(),
        })
}

pub fn parse_manifest(bytes: &[u8], path: &str) -> Result<Manifest, HarnessError> {
    let manifest: Manifest =
        serde_json::from_slice(bytes).map_err(|error| HarnessError::BadManifest {
            path: path.to_owned(),
            detail: error.to_string(),
        })?;
    if manifest.schema_version != MANIFEST_VERSION {
        return Err(HarnessError::BadManifest {
            path: path.to_owned(),
            detail: format!(
                "unsupported manifest schema {}: want {MANIFEST_VERSION}",
                manifest.schema_version
            ),
        });
    }
    for field in [
        ("name", &manifest.name),
        ("producer", &manifest.producer),
        ("capability", &manifest.capability),
        ("update_file", &manifest.update_file),
        ("update_hint", &manifest.update_hint),
    ] {
        if field.1.is_empty() {
            return Err(HarnessError::BadManifest {
                path: path.to_owned(),
                detail: format!("empty manifest field {}", field.0),
            });
        }
    }
    Ok(manifest)
}

pub fn resolve_manifest(resolver: &Resolver, manifest: Manifest) -> Result<Resolved, HarnessError> {
    let runner = lookup(resolver, &manifest.runner_key)?;
    let printer = lookup(resolver, &manifest.printer_key)?;
    let expected = lookup(resolver, &manifest.expected_key)?;
    let mut sources = Vec::with_capacity(manifest.sources.len());
    for mapping in &manifest.sources {
        sources.push((mapping.workspace.clone(), lookup(resolver, &mapping.key)?));
    }
    let mut siblings = Vec::with_capacity(manifest.siblings.len());
    for mapping in &manifest.siblings {
        siblings.push((mapping.workspace.clone(), lookup(resolver, &mapping.key)?));
    }
    let mut tools = BTreeMap::new();
    for (tool, entry) in &manifest.tools {
        let binary = entry
            .binary_key
            .as_deref()
            .map(|key| lookup(resolver, key))
            .transpose()?;
        let mut files = Vec::with_capacity(entry.files.len());
        for file in &entry.files {
            files.push((file.mirror_rel.clone(), lookup(resolver, &file.key)?));
        }
        let mut upstream = Vec::with_capacity(entry.upstream_keys.len());
        for key in &entry.upstream_keys {
            upstream.push(lookup(resolver, key)?);
        }
        tools.insert(
            tool.clone(),
            ResolvedTool {
                binary,
                files,
                upstream,
            },
        );
    }
    Ok(Resolved {
        manifest,
        runner,
        printer,
        expected,
        sources,
        siblings,
        tools,
    })
}

fn display(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

pub fn substitute_test_srcdir(value: &str, test_srcdir: &str) -> String {
    value.replace(TEST_SRCDIR_TOKEN, test_srcdir)
}

pub fn runner_argv(
    resolved: &Resolved,
    out_pb: &Path,
    scratch_parent: &Path,
    test_srcdir: &str,
) -> Vec<String> {
    let manifest = &resolved.manifest;
    let mut argv = vec![
        "--producer".to_owned(),
        manifest.producer.clone(),
        "--capability".to_owned(),
        manifest.capability.clone(),
        "--output".to_owned(),
        display(out_pb),
    ];
    for stage in &manifest.stages {
        argv.push("--stage".to_owned());
        argv.push(stage.clone());
    }
    for (workspace, path) in &resolved.sources {
        argv.push("--source".to_owned());
        argv.push(format!("{workspace}={}", display(path)));
    }
    for (workspace, path) in &resolved.siblings {
        argv.push("--sibling".to_owned());
        argv.push(format!("{workspace}={}", display(path)));
    }
    argv.push("--real".to_owned());
    argv.push("--scratch-parent".to_owned());
    argv.push(display(scratch_parent));
    for (tool, entry) in &resolved.tools {
        if let Some(binary) = &entry.binary {
            argv.push("--tool-binary".to_owned());
            argv.push(format!("{tool}={}", display(binary)));
        }
    }
    for (tool, entry) in &manifest.tools {
        if let Some(config) = &entry.config {
            argv.push("--tool-config".to_owned());
            argv.push(format!("{tool}={config}"));
        }
        if let Some(edition) = &entry.edition {
            argv.push("--tool-edition".to_owned());
            argv.push(format!("{tool}={edition}"));
        }
    }
    for (tool, entry) in &resolved.tools {
        for (mirror_rel, path) in &entry.files {
            argv.push("--tool-file".to_owned());
            argv.push(format!("{tool}={mirror_rel}={}", display(path)));
        }
    }
    for (tool, entry) in &manifest.tools {
        for env in &entry.env {
            argv.push("--tool-env".to_owned());
            argv.push(format!(
                "{tool}={}={}",
                env.key,
                substitute_test_srcdir(&env.value, test_srcdir)
            ));
        }
    }
    for (tool, entry) in &resolved.tools {
        for path in &entry.upstream {
            argv.push("--upstream-diagnostics".to_owned());
            argv.push(format!("{tool}={}", display(path)));
        }
    }
    argv
}

pub struct ChildOutput {
    pub status: String,
    pub stdout: Vec<u8>,
    pub stderr: String,
}

pub fn run_child(program: &Path, argv: &[String], role: &str) -> Result<ChildOutput, HarnessError> {
    let output =
        Command::new(program)
            .args(argv)
            .output()
            .map_err(|error| HarnessError::RunnerFailed {
                detail: format!("cannot launch {role} {}: {error}", program.display()),
            })?;
    Ok(ChildOutput {
        status: status_text(&output.status),
        stdout: output.stdout,
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    })
}

fn status_text(status: &std::process::ExitStatus) -> String {
    if status.success() {
        return "exit 0".to_owned();
    }
    match status.code() {
        Some(code) => format!("exit {code}"),
        None => "terminated by signal".to_owned(),
    }
}

fn capability_value(capability: &str) -> Option<i32> {
    match capability {
        "lint" => Some(proto::Capability::Lint as i32),
        "format" => Some(proto::Capability::Format as i32),
        "typecheck" => Some(proto::Capability::Typecheck as i32),
        _ => None,
    }
}

pub fn validate_result(bytes: &[u8], manifest: &Manifest) -> Result<(), HarnessError> {
    let result = decode_validated(bytes).map_err(|error| HarnessError::InvalidResult {
        detail: error.to_string(),
    })?;
    if !manifest.producer.is_empty() && result.producer != manifest.producer {
        return Err(HarnessError::BadResult {
            detail: format!(
                "producer {:?} does not match manifest {:?}",
                result.producer, manifest.producer
            ),
        });
    }
    match capability_value(&manifest.capability) {
        Some(want) if result.capability == want => {}
        _ => {
            return Err(HarnessError::BadResult {
                detail: format!(
                    "capability {} does not match manifest {:?}",
                    result.capability, manifest.capability
                ),
            });
        }
    }
    if result.stages.len() != manifest.stages.len() {
        return Err(HarnessError::BadResult {
            detail: format!(
                "stages {} does not match manifest stages {}",
                result.stages.len(),
                manifest.stages.len()
            ),
        });
    }
    if result.convergence != proto::Convergence::Stable as i32 {
        return Err(HarnessError::BadResult {
            detail: format!("convergence {} is not STABLE", result.convergence),
        });
    }
    Ok(())
}

fn header_count(lines: &[&str], prefix: &str) -> Result<usize, HarnessError> {
    for line in lines {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() == 2 && parts[0] == prefix {
            if let Ok(count) = parts[1].parse::<usize>() {
                return Ok(count);
            }
        }
    }
    Err(HarnessError::BadPrint {
        detail: format!("missing header {prefix}"),
    })
}

fn body_rows(lines: &[&str], prefix: &str) -> usize {
    lines
        .iter()
        .filter(|line| line.starts_with(prefix) && line.split_whitespace().count() > 2)
        .count()
}

pub fn check_printed(text: &str, stages: usize) -> Result<(), HarnessError> {
    let lines: Vec<&str> = text.split('\n').collect();
    let body: Vec<&str> = if text.ends_with('\n') {
        lines[..lines.len().saturating_sub(1)].to_vec()
    } else {
        return Err(HarnessError::BadPrint {
            detail: "print_result must end with newline".to_owned(),
        });
    };
    if body.is_empty() {
        return Err(HarnessError::BadPrint {
            detail: "empty print_result".to_owned(),
        });
    }
    if !body[0].starts_with("producer //") {
        return Err(HarnessError::BadPrint {
            detail: format!(
                "first line must be producer: {}",
                body[0].chars().take(80).collect::<String>()
            ),
        });
    }
    if body.len() < 2
        || !matches!(
            body[1],
            "capability LINT" | "capability FORMAT" | "capability TYPECHECK"
        )
    {
        return Err(HarnessError::BadPrint {
            detail: format!(
                "bad capability: {}",
                body.get(1)
                    .unwrap_or(&"")
                    .chars()
                    .take(80)
                    .collect::<String>()
            ),
        });
    }
    if header_count(&body, "stages")? != stages {
        return Err(HarnessError::BadPrint {
            detail: "stage count mismatch".to_owned(),
        });
    }
    if body_rows(&body, "stage ") != stages {
        return Err(HarnessError::BadPrint {
            detail: "stage rows mismatch".to_owned(),
        });
    }
    if !body
        .iter()
        .any(|line| line.starts_with("completed_rounds "))
    {
        return Err(HarnessError::BadPrint {
            detail: "missing completed_rounds".to_owned(),
        });
    }
    if !body.contains(&"convergence STABLE") {
        return Err(HarnessError::BadPrint {
            detail: "convergence must stay STABLE".to_owned(),
        });
    }
    if body_rows(&body, "initial ") != header_count(&body, "initial")? {
        return Err(HarnessError::BadPrint {
            detail: "initial rows vs header".to_owned(),
        });
    }
    if body_rows(&body, "terminal ") != header_count(&body, "terminal")? {
        return Err(HarnessError::BadPrint {
            detail: "terminal rows vs header".to_owned(),
        });
    }
    if body_rows(&body, "replacement ") != header_count(&body, "replacements")? {
        return Err(HarnessError::BadPrint {
            detail: "replacement rows vs header".to_owned(),
        });
    }
    Ok(())
}

pub fn render_diff(expected: &[u8], actual: &[u8]) -> String {
    let expected_text = String::from_utf8_lossy(expected);
    let actual_text = String::from_utf8_lossy(actual);
    let mut expected_lines: Vec<&str> = expected_text.split('\n').collect();
    let mut actual_lines: Vec<&str> = actual_text.split('\n').collect();
    if expected_lines.last() == Some(&"") {
        expected_lines.pop();
    }
    if actual_lines.last() == Some(&"") {
        actual_lines.pop();
    }
    let mut prefix = 0;
    while prefix < expected_lines.len()
        && prefix < actual_lines.len()
        && expected_lines[prefix] == actual_lines[prefix]
    {
        prefix += 1;
    }
    let mut suffix = 0;
    while suffix < expected_lines.len() - prefix
        && suffix < actual_lines.len() - prefix
        && expected_lines[expected_lines.len() - 1 - suffix]
            == actual_lines[actual_lines.len() - 1 - suffix]
    {
        suffix += 1;
    }
    let expected_mid = &expected_lines[prefix..expected_lines.len() - suffix];
    let actual_mid = &actual_lines[prefix..actual_lines.len() - suffix];
    let mut out = String::from("--- expected\n+++ actual\n");
    let mut shown = 0;
    for (index, line) in expected_mid.iter().enumerate() {
        if shown >= MAX_DIFF_LINES {
            break;
        }
        out.push_str(&format!("-{}: {line}\n", prefix + index + 1));
        shown += 1;
    }
    for (index, line) in actual_mid.iter().enumerate() {
        if shown >= MAX_DIFF_LINES {
            break;
        }
        out.push_str(&format!("+{}: {line}\n", prefix + index + 1));
        shown += 1;
    }
    let hidden = (expected_mid.len() + actual_mid.len()).saturating_sub(shown);
    if hidden > 0 {
        out.push_str(&format!("... {hidden} more differing lines\n"));
    }
    out
}

fn env_value(name: &str) -> Result<String, HarnessError> {
    std::env::var(name).map_err(|_| HarnessError::MissingEnv {
        name: name.to_owned(),
        detail: "the test environment must set it".to_owned(),
    })
}

fn update_dir() -> PathBuf {
    for name in [UNDECLARED_ENV, TMPDIR_ENV] {
        if let Ok(dir) = std::env::var(name) {
            if !dir.is_empty() {
                return PathBuf::from(dir);
            }
        }
    }
    PathBuf::from("/tmp")
}

pub fn stage_update(actual: &[u8], update_file: &str) -> Result<PathBuf, HarnessError> {
    let dir = update_dir();
    std::fs::create_dir_all(&dir).map_err(|error| HarnessError::UnwritableUpdate {
        path: display(&dir),
        detail: error.to_string(),
    })?;
    let path = dir.join(update_file);
    std::fs::write(&path, actual).map_err(|error| HarnessError::UnwritableUpdate {
        path: display(&path),
        detail: error.to_string(),
    })?;
    Ok(path)
}

struct Paths {
    out_pb: PathBuf,
    scratch_parent: PathBuf,
}

fn work_paths() -> Result<Paths, HarnessError> {
    let tmpdir = env_value(TEST_TMPDIR_ENV)?;
    let work = PathBuf::from(tmpdir).join("matrix_work");
    let scratch_parent = work.join("scratch");
    std::fs::create_dir_all(&scratch_parent).map_err(|error| HarnessError::UnwritableWorkdir {
        path: display(&scratch_parent),
        detail: error.to_string(),
    })?;
    Ok(Paths {
        out_pb: work.join("out.pb"),
        scratch_parent,
    })
}

fn runfiles_root(resolver: &Resolver) -> PathBuf {
    let source = resolver.source();
    if source.is_dir() {
        return source.to_path_buf();
    }
    source.parent().map(Path::to_path_buf).unwrap_or_default()
}

fn test_srcdir(resolver: &Resolver) -> String {
    match std::env::var(TEST_SRCDIR_ENV) {
        Ok(dir) if !dir.is_empty() => dir,
        _ => display(&runfiles_root(resolver)),
    }
}

fn invoked_exe() -> Result<PathBuf, HarnessError> {
    let argv0 = std::env::args()
        .next()
        .ok_or_else(|| HarnessError::NoRunfiles {
            binary: "the test executable".to_owned(),
            detail: "no argv[0]".to_owned(),
        })?;
    let path = PathBuf::from(argv0);
    if path.is_absolute() {
        return Ok(path);
    }
    std::env::current_dir()
        .map(|cwd| cwd.join(path))
        .map_err(|error| HarnessError::NoRunfiles {
            binary: "the test executable".to_owned(),
            detail: error.to_string(),
        })
}

fn tree_ancestor(exe: &Path) -> Option<PathBuf> {
    let mut root = PathBuf::new();
    for component in exe.components() {
        root.push(component.as_os_str());
        if component
            .as_os_str()
            .to_string_lossy()
            .ends_with(".runfiles")
        {
            return Some(root);
        }
    }
    None
}

fn resolver_for(exe: &Path) -> Result<Resolver, HarnessError> {
    if let Some(tree) = tree_ancestor(exe) {
        if tree.is_dir() {
            return Resolver::for_tree(&tree).map_err(|error| HarnessError::NoRunfiles {
                binary: display(exe),
                detail: error.to_string(),
            });
        }
    }
    Resolver::for_binary(exe).map_err(|error| HarnessError::NoRunfiles {
        binary: display(exe),
        detail: error.to_string(),
    })
}

fn run() -> Result<(), HarnessError> {
    let exe = match invoked_exe() {
        Ok(exe) => exe,
        Err(_) => std::env::current_exe().map_err(|error| HarnessError::NoRunfiles {
            binary: "the test executable".to_owned(),
            detail: error.to_string(),
        })?,
    };
    let resolver = resolver_for(&exe)?;
    let key = env_value(MANIFEST_ENV)?;
    let manifest_path = lookup(&resolver, &key)?;
    let bytes = std::fs::read(&manifest_path).map_err(|error| HarnessError::BadManifest {
        path: display(&manifest_path),
        detail: error.to_string(),
    })?;
    let manifest = parse_manifest(&bytes, &display(&manifest_path))?;
    let name = manifest.name.clone();
    let resolved = resolve_manifest(&resolver, manifest)?;
    let manifest_ref = &resolved.manifest;
    let paths = work_paths()?;
    let argv = runner_argv(
        &resolved,
        &paths.out_pb,
        &paths.scratch_parent,
        &test_srcdir(&resolver),
    );
    let runner_out = run_child(&resolved.runner, &argv, "runner")?;
    if !runner_out.status.starts_with("exit 0") {
        return Err(HarnessError::RunnerFailed {
            detail: format!("runner {}: {}", runner_out.status, runner_out.stderr),
        });
    }
    let printer_out = run_child(&resolved.printer, &[display(&paths.out_pb)], "printer")?;
    if !printer_out.status.starts_with("exit 0") {
        return Err(HarnessError::PrinterFailed {
            detail: format!("printer {}: {}", printer_out.status, printer_out.stderr),
        });
    }
    let out_bytes = std::fs::read(&paths.out_pb).map_err(|error| HarnessError::InvalidResult {
        detail: format!("cannot read {}: {error}", display(&paths.out_pb)),
    })?;
    validate_result(&out_bytes, manifest_ref)?;
    let actual = String::from_utf8_lossy(&printer_out.stdout).into_owned();
    check_printed(&actual, manifest_ref.stages.len())?;
    if std::env::var(UPDATE_ENV).as_deref() == Ok("1") {
        let staged = stage_update(printer_out.stdout.as_slice(), &manifest_ref.update_file)?;
        println!(
            "snapshot UPDATE_EXPECT: staged fresh actual at {}",
            display(&staged)
        );
        println!(
            "copy it to {}, then review before pinning.",
            manifest_ref.update_hint
        );
        println!("matrix PASS (updated): {name}");
        return Ok(());
    }
    let expected =
        std::fs::read(&resolved.expected).map_err(|error| HarnessError::UnreadableExpected {
            path: display(&resolved.expected),
            detail: error.to_string(),
        })?;
    if expected.as_slice() != printer_out.stdout.as_slice() {
        println!("{}", render_diff(&expected, &printer_out.stdout));
        println!("--- actual print_result:");
        println!("{actual}");
        println!("re-run with UPDATE_EXPECT=1 to stage the fresh golden (bazel test --test_env=UPDATE_EXPECT), then review before pinning.");
        return Err(HarnessError::GoldenMismatch { name });
    }
    println!("matrix PASS: {name}");
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("matrix FAIL: {error}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use prost::Message;

    fn mapping(workspace: &str, key: &str) -> serde_json::Value {
        serde_json::json!({"workspace": workspace, "key": key})
    }

    fn skeleton(extra: serde_json::Value) -> Vec<u8> {
        let mut doc = serde_json::json!({
            "schema_version": 1,
            "name": "matrix_probe",
            "producer": "//quality/testdata:matrix_probe",
            "capability": "format",
            "stages": ["rustfmt;rust;matrix/dirty.rs"],
            "sources": [mapping("matrix/dirty.rs", "rules_dx/quality/testdata/matrix_dirty.rs")],
            "siblings": [],
            "tools": {
                "rustfmt": {
                    "binary_key": "rules_dx/rustfmt",
                    "files": [],
                    "edition": "2021",
                    "env": [],
                    "upstream_keys": [],
                }
            },
            "runner_key": "rules_dx/quality_runner",
            "printer_key": "rules_dx/print_result",
            "expected_key": "rules_dx/expected.txt",
            "update_file": "matrix_probe.expected.update",
            "update_hint": "quality/testdata/matrix/matrix_probe.expected.txt",
        });
        for (key, value) in extra.as_object().expect("extra is an object") {
            doc[key] = value.clone();
        }
        serde_json::to_vec(&doc).expect("skeleton encodes")
    }

    fn valid() -> Manifest {
        parse_manifest(&skeleton(serde_json::json!({})), "skeleton").expect("valid manifest")
    }

    #[test]
    fn manifest_version_is_pinned_and_unknown_fields_rejected() {
        let error = parse_manifest(
            &skeleton(serde_json::json!({"schema_version": 2})),
            "skeleton",
        )
        .expect_err("v2 must fail");
        assert!(
            error.to_string().contains("unsupported manifest schema 2"),
            "{error}"
        );

        let mut raw: serde_json::Value =
            serde_json::from_slice(&skeleton(serde_json::json!({}))).expect("decodes");
        raw["bogus"] = serde_json::json!(true);
        assert!(
            parse_manifest(&serde_json::to_vec(&raw).expect("encodes"), "skeleton").is_err(),
            "an unknown field must fail"
        );
        assert!(
            parse_manifest(b"{not json", "skeleton").is_err(),
            "non-JSON must fail"
        );
    }

    #[test]
    fn empty_manifest_fields_fail() {
        for field in [
            "name",
            "producer",
            "capability",
            "update_file",
            "update_hint",
        ] {
            let error = parse_manifest(&skeleton(serde_json::json!({field: ""})), "skeleton")
                .expect_err("empty field must fail");
            assert!(error.to_string().contains(field), "{error}");
        }
    }

    #[test]
    fn tree_ancestor_stops_at_the_first_runfiles_dir() {
        assert_eq!(
            tree_ancestor(Path::new(
                "/sandbox/bin/quality/testdata/case.runfiles/_main/quality/testdata/case"
            )),
            Some(PathBuf::from("/sandbox/bin/quality/testdata/case.runfiles"))
        );
        assert_eq!(tree_ancestor(Path::new("/out/bin/tool")), None);
    }

    #[test]
    fn test_srcdir_substitution_replaces_only_the_token() {
        assert_eq!(
            substitute_test_srcdir("RUNFILES_DIR=$TEST_SRCDIR", "/runfiles"),
            "RUNFILES_DIR=/runfiles"
        );
        assert_eq!(
            substitute_test_srcdir("JS_BINARY__NO_CD_BINDIR=1", "/runfiles"),
            "JS_BINARY__NO_CD_BINDIR=1"
        );
        assert_eq!(
            substitute_test_srcdir("$TEST_SRCDIR/x=$TEST_SRCDIR", "/r"),
            "/r/x=/r"
        );
    }

    fn sample_result() -> proto::QualityResult {
        proto::QualityResult {
            schema_major: quality_result::SCHEMA_MAJOR,
            producer: "//quality/testdata:matrix_probe".to_owned(),
            capability: proto::Capability::Format as i32,
            stages: vec![proto::Stage {
                tool_id: "rustfmt".to_owned(),
                class_ids: vec!["rust".to_owned()],
                source_paths: vec!["matrix/dirty.rs".to_owned()],
            }],
            completed_rounds: 1,
            convergence: proto::Convergence::Stable as i32,
            ..Default::default()
        }
    }

    #[test]
    fn garbage_result_bytes_fail_validation() {
        let manifest = valid();
        let error = validate_result(&[0xFF, 0xFF, 0xFF], &manifest).expect_err("garbage must fail");
        assert!(
            matches!(error, HarnessError::InvalidResult { .. }),
            "{error}"
        );
    }

    #[test]
    fn result_mismatches_fail_with_the_manifest_values() {
        let manifest = valid();
        let mut result = sample_result();
        result.producer = "//other:target".to_owned();
        let error = validate_result(&result.encode_to_vec(), &manifest).expect_err("producer");
        assert!(error.to_string().contains("//other:target"), "{error}");

        let mut result = sample_result();
        result.capability = proto::Capability::Lint as i32;
        let error = validate_result(&result.encode_to_vec(), &manifest).expect_err("capability");
        assert!(error.to_string().contains("capability"), "{error}");

        let mut result = sample_result();
        result.stages.push(result.stages[0].clone());
        let error = validate_result(&result.encode_to_vec(), &manifest).expect_err("stages");
        assert!(error.to_string().contains("stages 2"), "{error}");

        let mut result = sample_result();
        result.convergence = proto::Convergence::IterationLimit as i32;
        let error = validate_result(&result.encode_to_vec(), &manifest).expect_err("convergence");
        assert!(error.to_string().contains("STABLE"), "{error}");
    }

    #[test]
    fn unstable_results_fail_even_when_counts_would_pass() {
        let manifest = valid();
        let mut result = sample_result();
        result.convergence = proto::Convergence::Oscillation as i32;
        assert!(validate_result(&result.encode_to_vec(), &manifest).is_err());
    }

    fn printed(stages: usize) -> String {
        format!(
            "producer //quality/testdata:matrix_probe\ncapability FORMAT\nstages {stages}\nstage rustfmt classes=rust sources=matrix/dirty.rs\ncompleted_rounds 1\nconvergence STABLE\ninitial 0\nterminal 0\nreplacements 0\n"
        )
    }

    #[test]
    fn printed_counts_must_match_their_headers() {
        assert!(check_printed(&printed(1), 1).is_ok());
        let bad = printed(1).replace("stages 1", "stages 2");
        assert!(check_printed(&bad, 1).is_err(), "stage count must fail");
        assert!(
            check_printed(&printed(1), 2).is_err(),
            "stage rows must fail"
        );
        let bad = printed(1).replace("initial 0", "initial 1");
        assert!(check_printed(&bad, 1).is_err(), "initial rows must fail");
        let bad = printed(1).replace("replacements 0", "replacements 1");
        assert!(
            check_printed(&bad, 1).is_err(),
            "replacement rows must fail"
        );
        let bad = printed(1).replace("convergence STABLE", "convergence OSCILLATION");
        assert!(check_printed(&bad, 1).is_err(), "convergence must fail");
        let bad = printed(1).replace("capability FORMAT", "capability AUDIT");
        assert!(check_printed(&bad, 1).is_err(), "capability must fail");
        let bad = printed(1).replace("producer //", "producer ");
        assert!(check_printed(&bad, 1).is_err(), "producer must fail");
        assert!(check_printed("", 1).is_err(), "empty must fail");
        let no_trailing = printed(1).trim_end().to_owned();
        assert!(check_printed(&no_trailing, 1).is_err(), "newline must fail");
    }

    #[test]
    fn crlf_goldens_do_not_compare_equal() {
        let unix = printed(1);
        let crlf = unix.replace('\n', "\r\n");
        assert_ne!(unix.as_bytes(), crlf.as_bytes());
        let diff = render_diff(unix.as_bytes(), crlf.as_bytes());
        assert!(diff.contains("--- expected"), "{diff}");
        assert!(diff.contains("+++ actual"), "{diff}");
    }

    #[test]
    fn diff_names_line_numbers_on_both_sides() {
        let diff = render_diff(b"a\nb\nc\n", b"a\nB\nc\n");
        assert!(diff.contains("-2: b"), "{diff}");
        assert!(diff.contains("+2: B"), "{diff}");
    }

    #[test]
    fn identical_bytes_render_an_empty_diff_body() {
        let diff = render_diff(b"a\n", b"a\n");
        assert_eq!(diff, "--- expected\n+++ actual\n");
    }
}
