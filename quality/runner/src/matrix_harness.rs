#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::Duration;

use dx_path::Resolver;
use dx_process::lifecycle::{run as run_child, CapturePolicy, ChildOutcome, EnvPolicy, SpawnSpec};
use quality_result::{decode_validated, proto};
use quality_runner::request::{parse_real_request, REQUEST_SCHEMA_VERSION};
use serde_json::Value;

const MANIFEST_ENV: &str = "MATRIX_MANIFEST";
const UPDATE_EXPECT_ENV: &str = "UPDATE_EXPECT";
const TEST_SRCDIR_ENV: &str = "TEST_SRCDIR";
const TEST_TMPDIR_ENV: &str = "TEST_TMPDIR";
const UNDECLARED_ENV: &str = "TEST_UNDECLARED_OUTPUTS_DIR";
const TMPDIR_ENV: &str = "TMPDIR";
const RUNFILES_DIR_ENV: &str = "RUNFILES_DIR";

const CHILD_TIMEOUT: Duration = Duration::from_secs(240);
const MAX_CAPTURE_BYTES: usize = 32 << 20;
const DIFF_CONTEXT: usize = 3;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum HarnessError {
    #[error("missing env {name}")]
    MissingEnv { name: &'static str },
    #[error("no runfiles beside {binary}: {detail}")]
    NoRunfiles { binary: String, detail: String },
    #[error("missing runfile {key}")]
    MissingRunfile { key: String },
    #[error("malformed manifest: {detail}")]
    BadManifest { detail: String },
    #[error("invalid request: {detail}")]
    BadRequest { detail: String },
    #[error("cannot stage scratch: {detail}")]
    Scratch { detail: String },
    #[error("runner failed{detail}")]
    RunnerFailed { detail: String },
    #[error("printer failed{detail}")]
    PrinterFailed { detail: String },
    #[error("print_result schema invalid: {detail}")]
    BadPrinted { detail: String },
    #[error("golden snapshot mismatch (see diff above)")]
    GoldenMismatch,
    #[error("cannot stage update: {detail}")]
    UpdateFailed { detail: String },
}

fn env(name: &'static str) -> Result<String, HarnessError> {
    std::env::var(name)
        .map_err(|_| HarnessError::MissingEnv { name })
        .map(|value| value.to_owned())
}

fn env_or_empty(name: &str) -> String {
    std::env::var(name).unwrap_or_default()
}

fn scratch_base() -> PathBuf {
    let base = env_or_empty(TEST_TMPDIR_ENV);
    if base.is_empty() {
        std::env::temp_dir()
    } else {
        PathBuf::from(base)
    }
}

fn resolver() -> Result<Resolver, HarnessError> {
    let binary = std::env::current_exe().map_err(|error| HarnessError::NoRunfiles {
        binary: "<current exe>".to_owned(),
        detail: error.to_string(),
    })?;
    Resolver::for_binary(&binary).map_err(|error| HarnessError::NoRunfiles {
        binary: binary.display().to_string(),
        detail: error.to_string(),
    })
}

fn resolve(resolver: &Resolver, key: &str) -> Result<PathBuf, HarnessError> {
    resolver
        .lookup_from(key, "")
        .map_err(|_| HarnessError::MissingRunfile {
            key: key.to_owned(),
        })
}

fn manifest_value<'a>(manifest: &'a Value, field: &str) -> Result<&'a Value, HarnessError> {
    manifest.get(field).ok_or_else(|| HarnessError::BadManifest {
        detail: format!("missing field {field:?}"),
    })
}

fn manifest_str<'a>(manifest: &'a Value, field: &str) -> Result<&'a str, HarnessError> {
    manifest_value(manifest, field)?
        .as_str()
        .ok_or_else(|| HarnessError::BadManifest {
            detail: format!("field {field:?} must be a string"),
        })
}

fn manifest_opt_str(manifest: &Value, field: &str) -> Result<Option<String>, HarnessError> {
    match manifest.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(text)) => Ok(Some(text.clone())),
        Some(_) => Err(HarnessError::BadManifest {
            detail: format!("field {field:?} must be a string or null"),
        }),
    }
}

fn manifest_array<'a>(manifest: &'a Value, field: &str) -> Result<&'a [Value], HarnessError> {
    manifest_value(manifest, field)?
        .as_array()
        .map(Vec::as_slice)
        .ok_or_else(|| HarnessError::BadManifest {
            detail: format!("field {field:?} must be an array"),
        })
}

fn manifest_mapping(entry: &Value, role: &str) -> Result<(String, String), HarnessError> {
    let workspace = entry
        .get("workspace")
        .and_then(Value::as_str)
        .ok_or_else(|| HarnessError::BadManifest {
            detail: format!("{role} entry needs a workspace string"),
        })?;
    let key = entry
        .get("key")
        .and_then(Value::as_str)
        .ok_or_else(|| HarnessError::BadManifest {
            detail: format!("{role} entry needs a key string"),
        })?;
    Ok((workspace.to_owned(), key.to_owned()))
}

fn manifest_tool_env(entry: &Value) -> Result<(String, String, String), HarnessError> {
    let tool = entry
        .get("tool")
        .and_then(Value::as_str)
        .ok_or_else(|| HarnessError::BadManifest {
            detail: "tool env entry needs a tool string".to_owned(),
        })?;
    let key = entry
        .get("key")
        .and_then(Value::as_str)
        .ok_or_else(|| HarnessError::BadManifest {
            detail: "tool env entry needs a key string".to_owned(),
        })?;
    let value = entry
        .get("value")
        .and_then(Value::as_str)
        .ok_or_else(|| HarnessError::BadManifest {
            detail: "tool env entry needs a value string".to_owned(),
        })?;
    Ok((tool.to_owned(), key.to_owned(), value.to_owned()))
}

fn substitute_srcdir(value: &str) -> String {
    let test_srcdir = env_or_empty(TEST_SRCDIR_ENV);
    value
        .replace("${TEST_SRCDIR}", &test_srcdir)
        .replace("$TEST_SRCDIR", &test_srcdir)
}

fn spawn(
    program: &Path,
    args: &[String],
    cwd: &Path,
    extra_env: &[(OsString, OsString)],
) -> Result<ChildOutcome, HarnessError> {
    let mut argv = vec![program.as_os_str().to_owned()];
    argv.extend(args.iter().map(|arg| OsString::from(arg)));
    let spec = SpawnSpec {
        argv,
        cwd: cwd.to_path_buf(),
        env: EnvPolicy::Inherited {
            extra: extra_env.to_vec(),
        },
        capture: CapturePolicy {
            max_bytes: MAX_CAPTURE_BYTES,
        },
        timeout: CHILD_TIMEOUT,
    };
    run_child(&spec).map_err(|error| HarnessError::Scratch {
        detail: error.to_string(),
    })
}

fn child_detail(outcome: &ChildOutcome) -> String {
    match outcome {
        ChildOutcome::Finished { exit, stdout: _, stderr } => {
            let code = exit
                .code()
                .map(|code| code.to_string())
                .unwrap_or_else(|| "signal".to_owned());
            let text = String::from_utf8_lossy(stderr);
            let mut tail: Vec<&str> = text.lines().rev().take(20).collect();
            tail.reverse();
            if tail.is_empty() {
                format!(": exit {code}")
            } else {
                format!(": exit {code}\n{}", tail.join("\n"))
            }
        }
        ChildOutcome::TimedOut => format!(": timed out after {}s", CHILD_TIMEOUT.as_secs()),
        ChildOutcome::OutputTooLarge { limit } => {
            format!(": output exceeds {limit} bytes")
        }
    }
}

fn child_succeeded(outcome: &ChildOutcome) -> bool {
    matches!(
        outcome,
        ChildOutcome::Finished {
            exit,
            ..
        } if exit.code() == Some(0)
    )
}

fn build_request(
    manifest: &Value,
    resolver: &Resolver,
    scratch_parent: &Path,
) -> Result<Vec<u8>, HarnessError> {
    let mut sources = Vec::new();
    for entry in manifest_array(manifest, "sources")? {
        let (workspace, key) = manifest_mapping(entry, "source")?;
        let exec = resolve(resolver, &key)?;
        sources.push(serde_json::json!({"workspace": workspace, "exec": exec.display().to_string()}));
    }
    let mut siblings = Vec::new();
    for entry in manifest_array(manifest, "siblings")? {
        let (workspace, key) = manifest_mapping(entry, "sibling")?;
        let exec = resolve(resolver, &key)?;
        siblings.push(serde_json::json!({"workspace": workspace, "exec": exec.display().to_string()}));
    }
    let mut stages = Vec::new();
    for entry in manifest_array(manifest, "stages")? {
        let tool = entry
            .get("tool")
            .and_then(Value::as_str)
            .ok_or_else(|| HarnessError::BadManifest {
                detail: "stage entry needs a tool string".to_owned(),
            })?;
        let classes = entry
            .get("classes")
            .and_then(Value::as_array)
            .ok_or_else(|| HarnessError::BadManifest {
                detail: "stage entry needs a classes array".to_owned(),
            })?;
        let paths = entry
            .get("sources")
            .and_then(Value::as_array)
            .ok_or_else(|| HarnessError::BadManifest {
                detail: "stage entry needs a sources array".to_owned(),
            })?;
        stages.push(serde_json::json!({"tool": tool, "classes": classes, "sources": paths}));
    }
    let mut tools = BTreeMap::new();
    let tools_doc = manifest_value(manifest, "tools")?
        .as_object()
        .ok_or_else(|| HarnessError::BadManifest {
            detail: "field \"tools\" must be an object".to_owned(),
        })?;
    for (name, entry) in tools_doc {
        let binary = match manifest_opt_str(entry, "binary")? {
            Some(key) => Some(resolve(resolver, &key)?.display().to_string()),
            None => None,
        };
        let mut tool = serde_json::json!({"files": [], "env": [], "upstream": []});
        if let Some(path) = binary {
            tool["binary"] = Value::String(path);
        }
        if let Some(config) = manifest_opt_str(entry, "config")? {
            tool["config"] = Value::String(config);
        }
        if let Some(edition) = manifest_opt_str(entry, "edition")? {
            tool["edition"] = Value::String(edition);
        }
        let files = entry
            .get("files")
            .and_then(Value::as_array)
            .ok_or_else(|| HarnessError::BadManifest {
                detail: format!("tool {name:?} needs a files array"),
            })?;
        for file in files {
            let rel = file
                .get("mirror_rel")
                .and_then(Value::as_str)
                .ok_or_else(|| HarnessError::BadManifest {
                    detail: format!("tool {name:?} file needs a mirror_rel string"),
                })?;
            let key = file
                .get("key")
                .and_then(Value::as_str)
                .ok_or_else(|| HarnessError::BadManifest {
                    detail: format!("tool {name:?} file needs a key string"),
                })?;
            let exec = resolve(resolver, key)?.display().to_string();
            tool["files"]
                .as_array_mut()
                .expect("files is an array")
                .push(serde_json::json!({"mirror_rel": rel, "exec": exec}));
        }
        let env_list = entry
            .get("env")
            .and_then(Value::as_array)
            .ok_or_else(|| HarnessError::BadManifest {
                detail: format!("tool {name:?} needs an env array"),
            })?;
        for item in env_list {
            let (_, key, value) = manifest_tool_env(item)?;
            tool["env"].as_array_mut().expect("env is an array").push(
                serde_json::json!({"key": key, "value": substitute_srcdir(&value)}),
            );
        }
        let upstream = entry
            .get("upstream")
            .and_then(Value::as_array)
            .ok_or_else(|| HarnessError::BadManifest {
                detail: format!("tool {name:?} needs an upstream array"),
            })?;
        for item in upstream {
            let key = item.as_str().ok_or_else(|| HarnessError::BadManifest {
                detail: format!("tool {name:?} upstream needs key strings"),
            })?;
            let exec = resolve(resolver, key)?.display().to_string();
            tool["upstream"]
                .as_array_mut()
                .expect("upstream is an array")
                .push(Value::String(exec));
        }
        tools.insert(name.clone(), tool);
    }
    let doc = serde_json::json!({
        "schema_version": REQUEST_SCHEMA_VERSION,
        "producer": manifest_str(manifest, "producer")?,
        "capability": manifest_str(manifest, "capability")?,
        "stages": stages,
        "sources": sources,
        "siblings": siblings,
        "resolves": [],
        "tools": tools,
        "scratch_parent": scratch_parent.display().to_string(),
    });
    let bytes = serde_json::to_vec(&doc).map_err(|error| HarnessError::BadManifest {
        detail: error.to_string(),
    })?;
    parse_real_request(&bytes).map_err(|error| HarnessError::BadRequest {
        detail: error.to_string(),
    })?;
    Ok(bytes)
}

fn capability_line(capability: i32) -> &'static str {
    match capability {
        1 => "capability LINT",
        2 => "capability TYPECHECK",
        3 => "capability FORMAT",
        _ => "capability UNKNOWN",
    }
}

fn header_count(lines: &[&str], prefix: &str) -> Result<usize, String> {
    for line in lines {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() == 2 && parts[0] == prefix {
            return parts[1].parse::<usize>().map_err(|_| format!("bad {prefix} count"));
        }
    }
    Err(format!("missing header {prefix}"))
}

fn check_printed(text: &str, result: &proto::QualityResult) -> Result<(), String> {
    let lines: Vec<&str> = text.split('\n').collect();
    if text.is_empty() {
        return Err("empty print_result".to_owned());
    }
    let first = lines.first().copied().unwrap_or("");
    if first != format!("producer {}", result.producer) {
        return Err(format!(
            "first line must be producer: {}",
            first.chars().take(80).collect::<String>()
        ));
    }
    let second = lines.get(1).copied().unwrap_or("");
    if !matches!(
        second,
        "capability LINT" | "capability FORMAT" | "capability TYPECHECK"
    ) {
        return Err(format!("bad capability: {second}"));
    }
    if second != capability_line(result.capability) {
        return Err(format!("capability line disagrees with result: {second}"));
    }
    let stages = header_count(&lines, "stages")?;
    if stages != result.stages.len() {
        return Err("stage count mismatch".to_owned());
    }
    if lines.iter().filter(|line| line.starts_with("stage ")).count() != stages {
        return Err("stage count mismatch".to_owned());
    }
    if !lines.iter().any(|line| line.starts_with("completed_rounds ")) {
        return Err("missing completed_rounds".to_owned());
    }
    if result.convergence != proto::Convergence::Stable as i32 {
        return Err("convergence must stay STABLE".to_owned());
    }
    if !lines.iter().any(|line| *line == "convergence STABLE") {
        return Err("convergence must stay STABLE".to_owned());
    }
    let initial = header_count(&lines, "initial")?;
    let terminal = header_count(&lines, "terminal")?;
    let replacements = header_count(&lines, "replacements")?;
    if initial != result.initial_diagnostics.len() {
        return Err("initial rows vs header".to_owned());
    }
    if terminal != result.terminal_diagnostics.len() {
        return Err("terminal rows vs header".to_owned());
    }
    let printed_replacements: usize = lines.iter().filter(|line| line.starts_with("replacement ")).count();
    let decoded_replacements: usize = result
        .replacements
        .iter()
        .map(|file| file.edits.len())
        .sum();
    if replacements != result.replacements.len() || printed_replacements != decoded_replacements {
        return Err("replacement rows vs header".to_owned());
    }
    if !text.ends_with('\n') {
        return Err("print_result must end with newline".to_owned());
    }
    Ok(())
}

fn visible(line: &str) -> String {
    line.replace('\r', "\\r")
}

fn unified_diff(expected: &str, actual: &str) -> String {
    let old: Vec<&str> = expected.split('\n').collect();
    let new: Vec<&str> = actual.split('\n').collect();
    let mut prefix = 0;
    while prefix < old.len() && prefix < new.len() && old[prefix] == new[prefix] {
        prefix += 1;
    }
    let mut suffix = 0;
    while suffix < old.len() - prefix
        && suffix < new.len() - prefix
        && old[old.len() - 1 - suffix] == new[new.len() - 1 - suffix]
    {
        suffix += 1;
    }
    let old_start = prefix.saturating_sub(DIFF_CONTEXT);
    let new_start = prefix.saturating_sub(DIFF_CONTEXT);
    let old_end = (old.len() - suffix + DIFF_CONTEXT).min(old.len());
    let new_end = (new.len() - suffix + DIFF_CONTEXT).min(new.len());
    let mut out = vec![
        "--- expected".to_owned(),
        "+++ actual".to_owned(),
        format!(
            "@@ -{},{} +{},{} @@",
            old_start + 1,
            old_end - old_start,
            new_start + 1,
            new_end - new_start
        ),
    ];
    for line in &old[old_start..prefix] {
        out.push(format!(" {}", visible(line)));
    }
    for line in &old[prefix..old.len() - suffix] {
        out.push(format!("-{}", visible(line)));
    }
    for line in &new[prefix..new.len() - suffix] {
        out.push(format!("+{}", visible(line)));
    }
    for line in &new[new.len() - suffix..new_end] {
        out.push(format!(" {}", visible(line)));
    }
    out.join("\n")
}

fn update_dir() -> PathBuf {
    let dir = env_or_empty(UNDECLARED_ENV);
    if !dir.is_empty() {
        return PathBuf::from(dir);
    }
    let tmp = env_or_empty(TMPDIR_ENV);
    if !tmp.is_empty() {
        return PathBuf::from(tmp);
    }
    PathBuf::from("/tmp")
}

fn stage_update(actual: &[u8], update_name: &str) -> Result<PathBuf, HarnessError> {
    let dir = update_dir();
    std::fs::create_dir_all(&dir).map_err(|error| HarnessError::UpdateFailed {
        detail: error.to_string(),
    })?;
    let staged = dir.join(format!("{update_name}.expected.update"));
    std::fs::write(&staged, actual).map_err(|error| HarnessError::UpdateFailed {
        detail: error.to_string(),
    })?;
    Ok(staged)
}

fn update_expected() -> bool {
    env_or_empty(UPDATE_EXPECT_ENV) == "1"
}

fn run() -> Result<String, HarnessError> {
    let manifest_key = env(MANIFEST_ENV)?;
    let resolver = resolver()?;
    let manifest_path = resolve(&resolver, &manifest_key)?;
    let manifest_bytes =
        std::fs::read(&manifest_path).map_err(|error| HarnessError::BadManifest {
            detail: format!("cannot read {}: {error}", manifest_path.display()),
        })?;
    let manifest: Value =
        serde_json::from_slice(&manifest_bytes).map_err(|error| HarnessError::BadManifest {
            detail: error.to_string(),
        })?;
    let name = manifest_str(&manifest, "name")?.to_owned();
    let expected_key = manifest_str(&manifest, "expected")?.to_owned();
    let update_name = manifest_str(&manifest, "update_name")?.to_owned();

    let work = scratch_base().join(format!("matrix_work_{name}_{}", std::process::id()));
    let scratch = work.join("scratch");
    std::fs::create_dir_all(&scratch).map_err(|error| HarnessError::Scratch {
        detail: error.to_string(),
    })?;
    let request_path = work.join("request.json");
    let out_path = work.join("out.pb");

    let request_bytes = build_request(&manifest, &resolver, &scratch)?;
    std::fs::write(&request_path, &request_bytes).map_err(|error| HarnessError::Scratch {
        detail: error.to_string(),
    })?;

    let runner_key = manifest_str(&manifest, "runner")?;
    let printer_key = manifest_str(&manifest, "printer")?;
    let runner = resolve(&resolver, runner_key)?;
    let printer = resolve(&resolver, printer_key)?;
    let expected = resolve(&resolver, &expected_key)?;

    let runfiles_dir = env_or_empty(TEST_SRCDIR_ENV);
    let runner_env = [(OsString::from(RUNFILES_DIR_ENV), OsString::from(runfiles_dir))];
    let runner_outcome = spawn(
        &runner,
        &[
            "--request".to_owned(),
            request_path.display().to_string(),
            "--output".to_owned(),
            out_path.display().to_string(),
        ],
        &work,
        &runner_env,
    )?;
    if !child_succeeded(&runner_outcome) {
        return Err(HarnessError::RunnerFailed {
            detail: child_detail(&runner_outcome),
        });
    }
    let printer_outcome = spawn(&printer, &[out_path.display().to_string()], &work, &[])?;
    let actual = match &printer_outcome {
        ChildOutcome::Finished { exit, stdout, .. } if exit.code() == Some(0) => stdout.clone(),
        _ => {
            return Err(HarnessError::PrinterFailed {
                detail: child_detail(&printer_outcome),
            });
        }
    };
    let result_bytes = std::fs::read(&out_path).map_err(|error| HarnessError::BadPrinted {
        detail: error.to_string(),
    })?;
    let result = decode_validated(&result_bytes).map_err(|error| HarnessError::BadPrinted {
        detail: error.to_string(),
    })?;
    let actual_text = String::from_utf8_lossy(&actual).into_owned();
    check_printed(&actual_text, &result).map_err(|detail| HarnessError::BadPrinted { detail })?;

    if update_expected() {
        let staged = stage_update(&actual, &update_name)?;
        println!("snapshot UPDATE_EXPECT: staged fresh actual at {}", staged.display());
        println!("copy it to quality/testdata/matrix/{update_name}.expected.txt, then review before pinning.");
        return Ok(format!("matrix PASS (updated): {name}"));
    }
    let expected_bytes = std::fs::read(&expected).map_err(|error| HarnessError::BadPrinted {
        detail: format!("cannot read {}: {error}", expected.display()),
    })?;
    if expected_bytes != actual {
        let expected_text = String::from_utf8_lossy(&expected_bytes);
        println!("{}", unified_diff(&expected_text, &actual_text));
        println!("--- actual print_result:");
        print!("{actual_text}");
        if !actual_text.ends_with('\n') {
            println!();
        }
        println!(
            "re-run with UPDATE_EXPECT=1 to stage the fresh golden (bazel test --test_env=UPDATE_EXPECT), then review before pinning."
        );
        return Err(HarnessError::GoldenMismatch);
    }
    Ok(format!("matrix PASS: {name}"))
}

fn main() {
    match run() {
        Ok(line) => {
            println!("{line}");
        }
        Err(error) => {
            eprintln!("matrix FAIL: {error}");
            std::process::exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(prefix: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("{prefix}-{}-{}", std::process::id(), private_seq()));
        std::fs::create_dir_all(&dir).expect("scratch");
        dir
    }

    fn private_seq() -> u64 {
        use std::sync::atomic::{AtomicU64, Ordering};
        static SEQ: AtomicU64 = AtomicU64::new(0);
        SEQ.fetch_add(1, Ordering::Relaxed)
    }

    fn write(dir: &Path, rel: &str, bytes: &[u8]) -> PathBuf {
        let path = dir.join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("parents");
        }
        std::fs::write(&path, bytes).expect("write");
        path
    }

    fn sample_manifest() -> Value {
        serde_json::json!({
            "name": "matrix_sample",
            "producer": "//pkg:sample",
            "capability": "lint",
            "stages": [{"tool": "ruff", "classes": ["python"], "sources": ["a.py"]}],
            "sources": [{"workspace": "a.py", "key": "KEY_SOURCE"}],
            "siblings": [],
            "tools": {"ruff": {
                "binary": "KEY_BINARY",
                "config": "ruff.toml",
                "edition": serde_json::Value::Null,
                "files": [{"mirror_rel": "conf/x.toml", "key": "KEY_TOOLFILE"}],
                "env": [{"tool": "ruff", "key": "EXTRA", "value": "a=b;c,d"}],
                "upstream": [],
            }},
            "runner": "KEY_RUNNER",
            "printer": "KEY_PRINTER",
            "expected": "KEY_EXPECTED",
            "update_name": "matrix_sample",
        })
    }

    #[test]
    fn manifest_keys_translate_verbatim_into_the_request() {
        let dir = scratch("matrix-manifest");
        let mut manifest = sample_manifest();
        manifest["tools"]["ruff"]["upstream"] =
            Value::Array(vec![Value::String("KEY_UPSTREAM".to_owned())]);
        let keys: BTreeMap<&str, PathBuf> = [
            ("KEY_SOURCE", write(&dir, "exec/a.py", b"x = 1\n")),
            ("KEY_BINARY", write(&dir, "exec/ruff", b"tool\n")),
            ("KEY_RUNNER", write(&dir, "exec/runner", b"runner\n")),
            ("KEY_PRINTER", write(&dir, "exec/printer", b"printer\n")),
            ("KEY_EXPECTED", write(&dir, "exec/expected.txt", b"golden\n")),
            ("KEY_TOOLFILE", write(&dir, "exec/conf.toml", b"[tool]\n")),
            ("KEY_UPSTREAM", write(&dir, "exec/diag.txt", b"diag\n")),
        ]
        .into_iter()
        .collect();
        let resolver = scratch_resolver(&dir, &keys);
        let scratch_parent = dir.join("scratch");
        let bytes = build_request(&manifest, &resolver, &scratch_parent).expect("request");
        let request = parse_real_request(&bytes).expect("request parses");
        assert_eq!(request.producer, "//pkg:sample");
        assert_eq!(request.capability, "lint");
        assert_eq!(request.stages.len(), 1);
        assert_eq!(request.stages[0].tool_id, "ruff");
        assert_eq!(request.stages[0].source_paths, ["a.py"]);
        assert_eq!(request.sources.len(), 1);
        assert_eq!(request.sources[0].0, "a.py");
        let tool = request.tools.get("ruff").expect("ruff entry");
        assert_eq!(tool.config_rel.as_deref(), Some("ruff.toml"));
        assert_eq!(tool.files.len(), 1);
        assert_eq!(tool.files[0].0, "conf/x.toml");
        assert_eq!(tool.env, [("EXTRA".to_owned(), "a=b;c,d".to_owned())]);
        assert_eq!(tool.upstream.len(), 1);
        assert_eq!(request.scratch_parent.as_deref(), Some(scratch_parent.display().to_string().as_str()));
    }

    #[test]
    fn unusual_filenames_survive_the_manifest_verbatim() {
        let dir = scratch("matrix-weird");
        let weird = ["a,b.py", "a;b.py", "a=b.py", "a b.py", "dir=x/a;b,c.py"];
        let mut sources = Vec::new();
        let mut keys = BTreeMap::new();
        for (index, name) in weird.iter().enumerate() {
            let path = write(&dir, &format!("exec/{index}.py"), b"x\n");
            let key = format!("KEY_{index}");
            sources.push(serde_json::json!({"workspace": name, "exec": "unused", "key": key}));
            keys.insert(format!("KEY_{index}"), path);
        }
        let binary = write(&dir, "exec/ruff", b"tool\n");
        keys.insert("KEY_BINARY".to_owned(), binary);
        keys.insert("KEY_RUNNER".to_owned(), write(&dir, "exec/runner", b"r\n"));
        keys.insert("KEY_PRINTER".to_owned(), write(&dir, "exec/printer", b"p\n"));
        keys.insert("KEY_EXPECTED".to_owned(), write(&dir, "exec/expected.txt", b"g\n"));
        let manifest = serde_json::json!({
            "name": "matrix_weird",
            "producer": "//pkg:weird",
            "capability": "lint",
            "stages": [{"tool": "ruff", "classes": ["python"], "sources": weird}],
            "sources": sources,
            "siblings": [],
            "tools": {"ruff": {
                "binary": "KEY_BINARY",
                "config": "ruff,;=.toml",
                "edition": serde_json::Value::Null,
                "files": [],
                "env": [{"tool": "ruff", "key": "EXTRA", "value": "a=b;c,d"}],
                "upstream": [],
            }},
            "runner": "KEY_RUNNER",
            "printer": "KEY_PRINTER",
            "expected": "KEY_EXPECTED",
            "update_name": "matrix_weird",
        });
        let resolver = scratch_resolver_from(&dir, keys);
        let bytes = build_request(&manifest, &resolver, &dir).expect("request");
        let request = parse_real_request(&bytes).expect("request parses");
        assert_eq!(request.stages[0].source_paths, weird);
        assert_eq!(
            request.sources.iter().map(|(workspace, _)| workspace.clone()).collect::<Vec<_>>(),
            weird
        );
    }

    #[test]
    fn a_missing_key_names_the_key() {
        let dir = scratch("matrix-missing");
        let resolver = scratch_resolver(&dir, &BTreeMap::new());
        let error = resolve(&resolver, "KEY_ABSENT").expect_err("no such key");
        assert_eq!(
            error,
            HarnessError::MissingRunfile {
                key: "KEY_ABSENT".to_owned()
            }
        );
        assert!(error.to_string().contains("KEY_ABSENT"));
    }

    #[test]
    fn test_srcdir_substitution_covers_both_spellings() {
        let dir = scratch("matrix-srcdir");
        let _ = dir;
        let before = std::env::var_os(TEST_SRCDIR_ENV);
        unsafe {
            std::env::set_var(TEST_SRCDIR_ENV, "/runfiles/tree");
        }
        assert_eq!(substitute_srcdir("RUNFILES_DIR=$TEST_SRCDIR"), "RUNFILES_DIR=/runfiles/tree");
        assert_eq!(substitute_srcdir("RUNFILES_DIR=${TEST_SRCDIR}"), "RUNFILES_DIR=/runfiles/tree");
        assert_eq!(substitute_srcdir("JS_BINARY__NO_CD_BINDIR=1"), "JS_BINARY__NO_CD_BINDIR=1");
        match before {
            Some(value) => unsafe {
                std::env::set_var(TEST_SRCDIR_ENV, value);
            },
            None => unsafe {
                std::env::remove_var(TEST_SRCDIR_ENV);
            },
        }
    }

    #[test]
    fn printed_checks_reject_a_non_stable_convergence() {
        let result = decoded_sample(proto::Convergence::Oscillation as i32);
        let text = printed_sample(&result, "convergence OSCILLATION");
        let error = check_printed(&text, &result).expect_err("oscillation must fail");
        assert!(error.contains("STABLE"), "{error}");
    }

    #[test]
    fn printed_checks_reject_count_mismatches() {
        let result = decoded_sample(proto::Convergence::Stable as i32);
        let mut text = printed_sample(&result, "convergence STABLE");
        text = text.replacen("initial 1", "initial 2", 1);
        let error = check_printed(&text, &result).expect_err("count mismatch must fail");
        assert!(error.contains("initial rows vs header"), "{error}");
    }

    #[test]
    fn printed_checks_reject_a_missing_trailing_newline() {
        let result = decoded_sample(proto::Convergence::Stable as i32);
        let mut text = printed_sample(&result, "convergence STABLE");
        text.pop();
        let error = check_printed(&text, &result).expect_err("missing newline must fail");
        assert!(error.contains("newline"), "{error}");
    }

    #[test]
    fn printed_checks_accept_the_sample() {
        let result = decoded_sample(proto::Convergence::Stable as i32);
        let text = printed_sample(&result, "convergence STABLE");
        check_printed(&text, &result).expect("sample prints clean");
    }

    #[test]
    fn update_dir_prefers_undeclared_outputs_then_tmpdir() {
        let before_undeclared = std::env::var_os(UNDECLARED_ENV);
        let before_tmp = std::env::var_os(TMPDIR_ENV);
        unsafe {
            std::env::set_var(UNDECLARED_ENV, "/out/undeclared");
            std::env::set_var(TMPDIR_ENV, "/out/tmp");
        }
        assert_eq!(update_dir(), PathBuf::from("/out/undeclared"));
        unsafe {
            std::env::remove_var(UNDECLARED_ENV);
        }
        assert_eq!(update_dir(), PathBuf::from("/out/tmp"));
        unsafe {
            std::env::remove_var(TMPDIR_ENV);
        }
        assert_eq!(update_dir(), PathBuf::from("/tmp"));
        match before_undeclared {
            Some(value) => unsafe {
                std::env::set_var(UNDECLARED_ENV, value);
            },
            None => unsafe {
                std::env::remove_var(UNDECLARED_ENV);
            },
        }
        match before_tmp {
            Some(value) => unsafe {
                std::env::set_var(TMPDIR_ENV, value);
            },
            None => unsafe {
                std::env::remove_var(TMPDIR_ENV);
            },
        }
    }

    #[test]
    fn staging_writes_bytes_verbatim() {
        let dir = scratch("matrix-stage");
        let before = std::env::var_os(UNDECLARED_ENV);
        unsafe {
            std::env::set_var(UNDECLARED_ENV, dir.join("out"));
        }
        let body = b"producer //pkg:sample\r\ncapability LINT\r\n";
        let staged = stage_update(body, "matrix_crlf").expect("stage");
        assert_eq!(staged.file_name().and_then(|name| name.to_str()), Some("matrix_crlf.expected.update"));
        assert_eq!(std::fs::read(&staged).expect("read back"), body);
        match before {
            Some(value) => unsafe {
                std::env::set_var(UNDECLARED_ENV, value);
            },
            None => unsafe {
                std::env::remove_var(UNDECLARED_ENV);
            },
        }
    }

    #[test]
    fn diff_shows_changed_lines_with_context_and_marks_cr() {
        let expected = "producer //pkg:sample\ncapability LINT\nstages 1\n";
        let actual = "producer //pkg:sample\ncapability FORMAT\nstages 1\n";
        let diff = unified_diff(expected, actual);
        assert!(diff.contains("--- expected"), "{diff}");
        assert!(diff.contains("+++ actual"), "{diff}");
        assert!(diff.contains("-capability LINT"), "{diff}");
        assert!(diff.contains("+capability FORMAT"), "{diff}");
        assert!(diff.contains(" stages 1"), "{diff}");
        let identical = unified_diff(expected, expected);
        assert!(
            identical.lines().skip(3).all(|line| line.starts_with(' ')),
            "{identical}"
        );
        let cr = unified_diff("a\r\n", "a\n");
        assert!(cr.contains("\\r"), "{cr}");
    }

    #[test]
    fn manifest_mode_resolution_needs_no_runfiles_tree() {
        let dir = scratch("matrix-manifest-mode");
        let tool = write(&dir, "tool.txt", b"from the manifest\n");
        let binary = write(&dir, "tool.exe", b"binary\n");
        write(
            &dir,
            "tool.exe.runfiles_manifest",
            format!("_main/pkg/tool.txt {}\n", tool.display()).as_bytes(),
        );
        let resolver = Resolver::for_binary(&binary).expect("resolver");
        let found = resolve(&resolver, "_main/pkg/tool.txt").expect("key");
        assert_eq!(found, tool);
        assert!(resolve(&resolver, "_main/pkg/absent.txt").is_err());
    }

    fn scratch_resolver(dir: &Path, keys: &BTreeMap<&str, PathBuf>) -> Resolver {
        let owned: BTreeMap<String, PathBuf> = keys
            .iter()
            .map(|(key, path)| ((*key).to_owned(), path.clone()))
            .collect();
        scratch_resolver_from(dir, owned)
    }

    fn scratch_resolver_from(dir: &Path, keys: BTreeMap<String, PathBuf>) -> Resolver {
        let binary = dir.join("harness.exe");
        std::fs::write(&binary, b"binary\n").expect("binary");
        let mut manifest = String::new();
        for (key, path) in &keys {
            manifest.push_str(&format!("{key} {}\n", path.display()));
        }
        std::fs::write(dir.join("harness.exe.runfiles_manifest"), manifest).expect("manifest");
        Resolver::for_binary(&binary).expect("resolver")
    }

    fn decoded_sample(convergence: i32) -> proto::QualityResult {
        proto::QualityResult {
            producer: "//pkg:sample".to_owned(),
            capability: proto::Capability::Lint as i32,
            stages: vec![proto::Stage {
                tool_id: "ruff".to_owned(),
                class_ids: vec!["python".to_owned()],
                source_paths: vec!["a.py".to_owned()],
            }],
            completed_rounds: 1,
            convergence,
            initial_diagnostics: vec![proto::Diagnostic {
                severity: proto::Severity::Warning as i32,
                message: "unused import".to_owned(),
                tool_id: "ruff".to_owned(),
                path: "a.py".to_owned(),
                start_byte: Some(0),
                end_byte: Some(8),
                ..Default::default()
            }],
            terminal_diagnostics: vec![],
            replacements: vec![],
            ..Default::default()
        }
    }

    fn printed_sample(result: &proto::QualityResult, convergence_line: &str) -> String {
        let mut text = String::new();
        text.push_str(&format!("producer {}\n", result.producer));
        text.push_str("capability LINT\n");
        text.push_str(&format!("stages {}\n", result.stages.len()));
        for stage in &result.stages {
            text.push_str(&format!(
                "stage {} classes={} sources={}\n",
                stage.tool_id,
                stage.class_ids.join(","),
                stage.source_paths.join(",")
            ));
        }
        text.push_str(&format!("completed_rounds {}\n", result.completed_rounds));
        text.push_str(&format!("{convergence_line}\n"));
        text.push_str(&format!("initial {}\n", result.initial_diagnostics.len()));
        for diagnostic in &result.initial_diagnostics {
            text.push_str(&format!(
                "initial WARNING {} - {} {:?} {:?}\n",
                diagnostic.tool_id, diagnostic.path, 0, diagnostic.message
            ));
        }
        text.push_str(&format!("terminal {}\n", result.terminal_diagnostics.len()));
        text.push_str(&format!("replacements {}\n", result.replacements.len()));
        text
    }

    #[test]
    fn error_text_stays_stable() {
        assert_eq!(
            HarnessError::MissingRunfile {
                key: "k".to_owned()
            }
            .to_string(),
            "missing runfile k"
        );
        assert_eq!(
            HarnessError::GoldenMismatch.to_string(),
            "golden snapshot mismatch (see diff above)"
        );
    }

    #[test]
    fn child_detail_names_timeouts_and_limits() {
        assert!(child_detail(&ChildOutcome::TimedOut).contains("timed out"));
        assert!(
            child_detail(&ChildOutcome::OutputTooLarge { limit: 8 }).contains('8')
        );
    }
}
