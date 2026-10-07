//! Runs one runner matrix case from a declared test-input manifest.

#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

use std::collections::BTreeSet;
use std::ffi::OsString;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Duration;

use dx_path::Resolver;
use dx_process::lifecycle::{run, CapturePolicy, ChildOutcome, EnvPolicy, Exit, SpawnSpec};
use quality_result::decode_validated;

const MANIFEST_FORMAT: &str = "1";
const RUNNER_TIMEOUT: Duration = Duration::from_secs(45);
const PRINTER_TIMEOUT: Duration = Duration::from_secs(45);
const MAX_CAPTURE: usize = 64 * 1024 * 1024;
const TEST_SRCDIR_PLACEHOLDER: &str = "$(TEST_SRCDIR)";

#[derive(Debug, Default, PartialEq, Eq)]
struct Manifest {
    name: String,
    producer: String,
    capability: String,
    expected_rel: String,
    runner: String,
    printer: String,
    expected: String,
    stages: Vec<String>,
    sources: Vec<(String, String)>,
    siblings: Vec<(String, String)>,
    tool_binaries: Vec<(String, String)>,
    tool_configs: Vec<(String, String)>,
    tool_editions: Vec<(String, String)>,
    tool_files: Vec<(String, String, String)>,
    tool_envs: Vec<String>,
    upstream: Vec<(String, String)>,
}

#[derive(Debug)]
struct Resolved {
    runner: PathBuf,
    printer: PathBuf,
    expected: PathBuf,
    sources: Vec<(String, PathBuf)>,
    siblings: Vec<(String, PathBuf)>,
    tool_binaries: Vec<(String, PathBuf)>,
    tool_configs: Vec<(String, String)>,
    tool_editions: Vec<(String, String)>,
    tool_files: Vec<(String, String, PathBuf)>,
    tool_envs: Vec<String>,
    upstream: Vec<(String, PathBuf)>,
}

fn main() -> ExitCode {
    match drive() {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("matrix FAIL: {message}");
            ExitCode::FAILURE
        }
    }
}

fn drive() -> Result<(), String> {
    let resolver = resolver_from_env()?;
    let manifest_path = resolve_file(&resolver, &manifest_key_from_env()?, "test manifest")?;
    let text = std::fs::read_to_string(&manifest_path)
        .map_err(|error| format!("cannot read {}: {error}", manifest_path.display()))?;
    let manifest = parse_manifest(&text)?;
    let resolved = resolve_manifest(&resolver, &manifest)?;
    let work = work_dir()?;
    let scratch = work.join("scratch");
    std::fs::create_dir_all(&scratch)
        .map_err(|error| format!("cannot create {}: {error}", scratch.display()))?;
    let out = work.join("out.pb");
    let actual = work.join("actual.txt");
    let cwd = std::env::current_dir()
        .map_err(|error| format!("cannot read the working directory: {error}"))?;
    let child_env = child_env();

    let runner_argv = runner_argv(&manifest, &resolved, &out, &scratch);
    run_child(
        runner_argv,
        &cwd,
        child_env.clone(),
        RUNNER_TIMEOUT,
        "runner failed",
        true,
    )?;

    let bytes =
        std::fs::read(&out).map_err(|error| format!("cannot read {}: {error}", out.display()))?;
    decode_validated(&bytes)
        .map_err(|error| format!("invalid quality result in {}: {error}", out.display()))?;

    let printer_argv = vec![
        resolved.printer.clone().into_os_string(),
        out.as_os_str().to_owned(),
    ];
    let printed = run_child(
        printer_argv,
        &cwd,
        child_env,
        PRINTER_TIMEOUT,
        "printer failed",
        false,
    )?;
    std::fs::write(&actual, &printed)
        .map_err(|error| format!("cannot write {}: {error}", actual.display()))?;
    let printed =
        String::from_utf8(printed).map_err(|_| "print_result output is not UTF-8".to_owned())?;
    validate_print_schema(&printed)
        .map_err(|detail| format!("print_result schema invalid: {detail}"))?;
    validate_print_counts(&printed)
        .map_err(|detail| format!("print_result counts inconsistent: {detail}"))?;

    if std::env::var("UPDATE_EXPECT").as_deref() == Ok("1") {
        return stage_update(&actual, &manifest);
    }
    if let Err(diff) = dx_testing::snapshot_diff(&resolved.expected, &actual, None) {
        eprintln!("{diff}");
        eprintln!("--- actual print_result:");
        eprintln!("{printed}");
        return Err("golden snapshot mismatch (see diff above)".to_owned());
    }
    println!("matrix PASS: {}", manifest.name);
    Ok(())
}

fn resolver_from_env() -> Result<Resolver, String> {
    resolver_from(
        nonempty_env("RUNFILES_MANIFEST_FILE"),
        nonempty_env("TEST_SRCDIR"),
    )
}

fn resolver_from(
    manifest_file: Option<String>,
    srcdir: Option<String>,
) -> Result<Resolver, String> {
    if let Some(path) = manifest_file {
        return Resolver::from_manifest(PathBuf::from(path)).map_err(|error| error.to_string());
    }
    if let Some(dir) = srcdir {
        return Resolver::from_tree(PathBuf::from(dir)).map_err(|error| error.to_string());
    }
    Err("neither RUNFILES_MANIFEST_FILE nor TEST_SRCDIR names this test's runfiles".to_owned())
}

fn nonempty_env(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|value| !value.is_empty())
}

fn manifest_key_from_env() -> Result<String, String> {
    let target = nonempty_env("TEST_TARGET")
        .ok_or_else(|| "TEST_TARGET is not set; run this test through Bazel".to_owned())?;
    let workspace =
        nonempty_env("TEST_WORKSPACE").ok_or_else(|| "TEST_WORKSPACE is not set".to_owned())?;
    manifest_key(&target, &workspace)
}

fn manifest_key(target: &str, workspace: &str) -> Result<String, String> {
    let label = target.strip_prefix("@@").unwrap_or(target);
    let (_, rest) = label
        .split_once("//")
        .ok_or_else(|| format!("TEST_TARGET {target} is not //package:name"))?;
    let (package, name) = rest
        .split_once(':')
        .ok_or_else(|| format!("TEST_TARGET {target} is not //package:name"))?;
    if package.is_empty() || name.is_empty() {
        return Err(format!("TEST_TARGET {target} is not //package:name"));
    }
    Ok(format!("{workspace}/{package}/{name}.manifest"))
}

fn child_env() -> Vec<(OsString, OsString)> {
    match nonempty_env("TEST_SRCDIR") {
        Some(dir) => vec![(OsString::from("RUNFILES_DIR"), OsString::from(dir))],
        None => Vec::new(),
    }
}

fn work_dir() -> Result<PathBuf, String> {
    let base = nonempty_env("TEST_TMPDIR")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let work = base.join("matrix_work");
    std::fs::create_dir_all(&work)
        .map_err(|error| format!("cannot create {}: {error}", work.display()))?;
    Ok(work)
}

fn resolve_file(resolver: &Resolver, key: &str, what: &str) -> Result<PathBuf, String> {
    let path = resolver
        .lookup(key)
        .map_err(|error| format!("{what} ({key}): {error}"))?;
    if !path.is_file() {
        return Err(format!("missing runfile {key} at {}", path.display()));
    }
    Ok(path)
}

fn resolve_manifest(resolver: &Resolver, manifest: &Manifest) -> Result<Resolved, String> {
    let srcdir = nonempty_env("TEST_SRCDIR");
    let mut sources = Vec::new();
    for (ws_path, key) in &manifest.sources {
        sources.push((ws_path.clone(), resolve_file(resolver, key, "source")?));
    }
    let mut siblings = Vec::new();
    for (ws_path, key) in &manifest.siblings {
        siblings.push((ws_path.clone(), resolve_file(resolver, key, "sibling")?));
    }
    let mut tool_binaries = Vec::new();
    for (tool, key) in &manifest.tool_binaries {
        tool_binaries.push((tool.clone(), resolve_file(resolver, key, "tool binary")?));
    }
    let mut tool_files = Vec::new();
    for (tool, rel, key) in &manifest.tool_files {
        tool_files.push((
            tool.clone(),
            rel.clone(),
            resolve_file(resolver, key, "tool file")?,
        ));
    }
    let mut upstream = Vec::new();
    for (tool, key) in &manifest.upstream {
        upstream.push((
            tool.clone(),
            resolve_file(resolver, key, "upstream diagnostics")?,
        ));
    }
    let mut tool_envs = Vec::new();
    for raw in &manifest.tool_envs {
        tool_envs.push(substitute_tool_env(raw, srcdir.as_deref())?);
    }
    Ok(Resolved {
        runner: resolve_file(resolver, &manifest.runner, "runner")?,
        printer: resolve_file(resolver, &manifest.printer, "printer")?,
        expected: resolve_file(resolver, &manifest.expected, "expected")?,
        sources,
        siblings,
        tool_binaries,
        tool_configs: manifest.tool_configs.clone(),
        tool_editions: manifest.tool_editions.clone(),
        tool_files,
        tool_envs,
        upstream,
    })
}

fn substitute_tool_env(raw: &str, srcdir: Option<&str>) -> Result<String, String> {
    if !raw.contains(TEST_SRCDIR_PLACEHOLDER) {
        return Ok(raw.to_owned());
    }
    let srcdir =
        srcdir.ok_or_else(|| format!("TEST_SRCDIR is not set, required by tool env {raw}"))?;
    Ok(raw.replace(TEST_SRCDIR_PLACEHOLDER, srcdir))
}

fn runner_argv(
    manifest: &Manifest,
    resolved: &Resolved,
    out: &Path,
    scratch: &Path,
) -> Vec<OsString> {
    let mut argv = vec![
        resolved.runner.clone().into_os_string(),
        OsString::from("--producer"),
        OsString::from(&manifest.producer),
        OsString::from("--capability"),
        OsString::from(&manifest.capability),
        OsString::from("--output"),
        OsString::from(out),
    ];
    for stage in &manifest.stages {
        argv.push(OsString::from("--stage"));
        argv.push(OsString::from(stage));
    }
    for (ws_path, path) in &resolved.sources {
        argv.push(OsString::from("--source"));
        argv.push(prefixed_path(ws_path, path));
    }
    for (ws_path, path) in &resolved.siblings {
        argv.push(OsString::from("--sibling"));
        argv.push(prefixed_path(ws_path, path));
    }
    argv.push(OsString::from("--real"));
    argv.push(OsString::from("--scratch-parent"));
    argv.push(OsString::from(scratch));
    for (tool, path) in &resolved.tool_binaries {
        argv.push(OsString::from("--tool-binary"));
        argv.push(prefixed_path(tool, path));
    }
    for (tool, rel) in &resolved.tool_configs {
        argv.push(OsString::from("--tool-config"));
        argv.push(OsString::from(format!("{tool}={rel}")));
    }
    for (tool, edition) in &resolved.tool_editions {
        argv.push(OsString::from("--tool-edition"));
        argv.push(OsString::from(format!("{tool}={edition}")));
    }
    for (tool, rel, path) in &resolved.tool_files {
        let mut spec = OsString::from(format!("{tool}={rel}"));
        spec.push("=");
        spec.push(path);
        argv.push(OsString::from("--tool-file"));
        argv.push(spec);
    }
    for raw in &resolved.tool_envs {
        argv.push(OsString::from("--tool-env"));
        argv.push(OsString::from(raw));
    }
    for (tool, path) in &resolved.upstream {
        argv.push(OsString::from("--upstream-diagnostics"));
        argv.push(prefixed_path(tool, path));
    }
    argv
}

fn prefixed_path(prefix: &str, path: &Path) -> OsString {
    let mut spec = OsString::from(prefix);
    spec.push("=");
    spec.push(path);
    spec
}

fn run_child(
    argv: Vec<OsString>,
    cwd: &Path,
    env: Vec<(OsString, OsString)>,
    timeout: Duration,
    failure: &str,
    forward_stdout: bool,
) -> Result<Vec<u8>, String> {
    let outcome = run(&SpawnSpec {
        argv,
        cwd: cwd.to_path_buf(),
        env: EnvPolicy::Inherited { extra: env },
        capture: CapturePolicy {
            max_bytes: MAX_CAPTURE,
        },
        timeout,
    })
    .map_err(|error| format!("cannot start {failure}: {error}"))?;
    match outcome {
        ChildOutcome::Finished {
            exit,
            stdout,
            stderr,
        } => {
            if forward_stdout {
                echo(&stdout);
            }
            eprint!("{}", String::from_utf8_lossy(&stderr));
            let _ = std::io::stderr().flush();
            if matches!(exit, Exit::Code(0)) {
                return Ok(stdout);
            }
            Err(format!("{failure} ({})", exit_label(&exit)))
        }
        ChildOutcome::TimedOut => Err(format!(
            "{failure}: child ran past the {} second bound",
            timeout.as_secs()
        )),
        ChildOutcome::OutputTooLarge { limit } => {
            Err(format!("{failure}: child output exceeded {limit} bytes"))
        }
    }
}

fn exit_label(exit: &Exit) -> String {
    match exit {
        Exit::Code(code) => format!("exit {code}"),
        Exit::Signal(signal) => format!("signal {signal}"),
    }
}

fn echo(bytes: &[u8]) {
    let mut stdout = std::io::stdout().lock();
    let _ = stdout.write_all(bytes);
    let _ = stdout.flush();
}

fn stage_update(actual: &Path, manifest: &Manifest) -> Result<(), String> {
    let out_dir = nonempty_env("TEST_UNDECLARED_OUTPUTS_DIR")
        .or_else(|| nonempty_env("TMPDIR"))
        .unwrap_or_else(|| "/tmp".to_owned());
    let out_dir = PathBuf::from(out_dir);
    std::fs::create_dir_all(&out_dir)
        .map_err(|error| format!("cannot create {}: {error}", out_dir.display()))?;
    let staged = out_dir.join(format!("{}.expected.update", manifest.name));
    std::fs::copy(actual, &staged)
        .map_err(|error| format!("cannot stage {}: {error}", staged.display()))?;
    println!(
        "snapshot UPDATE_EXPECT: staged fresh actual at {}",
        staged.display()
    );
    println!(
        "copy it to {}, then review before pinning.",
        manifest.expected_rel
    );
    println!("matrix PASS (updated): {}", manifest.name);
    Ok(())
}

fn parse_manifest(text: &str) -> Result<Manifest, String> {
    let mut manifest = Manifest::default();
    let mut format = String::new();
    let mut scalars = BTreeSet::new();
    let mut tokens = text.split('\0');
    while let Some(key) = tokens.next() {
        let value = match tokens.next() {
            Some(value) => value,
            None => {
                if key.is_empty() {
                    break;
                }
                return Err(format!("manifest record {key} has no value"));
            }
        };
        if key.is_empty() {
            break;
        }
        match key {
            "format" => format = value.to_owned(),
            "name" | "producer" | "capability" | "expected_rel" | "runner" | "printer"
            | "expected" => {
                if !scalars.insert(key) {
                    return Err(format!("duplicate manifest key {key}"));
                }
                set_scalar(&mut manifest, key, value)?;
            }
            "stage" => manifest.stages.push(value.to_owned()),
            "source" => manifest.sources.push(split_pair(value, key)?),
            "sibling" => manifest.siblings.push(split_pair(value, key)?),
            "tool_binary" => manifest.tool_binaries.push(split_pair(value, key)?),
            "tool_config" => manifest.tool_configs.push(split_pair(value, key)?),
            "tool_edition" => manifest.tool_editions.push(split_pair(value, key)?),
            "tool_file" => {
                let (tool, rest) = split_once(value, key)?;
                let (rel, path) = split_once(&rest, key)?;
                manifest.tool_files.push((tool, rel, path));
            }
            "tool_env" => manifest.tool_envs.push(value.to_owned()),
            "upstream" => manifest.upstream.push(split_pair(value, key)?),
            other => return Err(format!("unknown manifest key {other}")),
        }
    }
    if format != MANIFEST_FORMAT {
        return Err(format!(
            "manifest format {format} is not supported, want {MANIFEST_FORMAT}"
        ));
    }
    for (field, value) in [
        ("name", &manifest.name),
        ("producer", &manifest.producer),
        ("capability", &manifest.capability),
        ("expected_rel", &manifest.expected_rel),
        ("runner", &manifest.runner),
        ("printer", &manifest.printer),
        ("expected", &manifest.expected),
    ] {
        if value.is_empty() {
            return Err(format!("manifest is missing {field}"));
        }
    }
    Ok(manifest)
}

fn set_scalar(manifest: &mut Manifest, key: &str, value: &str) -> Result<(), String> {
    let slot = match key {
        "name" => &mut manifest.name,
        "producer" => &mut manifest.producer,
        "capability" => &mut manifest.capability,
        "expected_rel" => &mut manifest.expected_rel,
        "runner" => &mut manifest.runner,
        "printer" => &mut manifest.printer,
        "expected" => &mut manifest.expected,
        other => return Err(format!("unknown manifest key {other}")),
    };
    slot.push_str(value);
    Ok(())
}

fn split_once(value: &str, key: &str) -> Result<(String, String), String> {
    let (head, rest) = value
        .split_once('=')
        .ok_or_else(|| format!("malformed {key} record {value}"))?;
    if head.is_empty() {
        return Err(format!("malformed {key} record {value}"));
    }
    Ok((head.to_owned(), rest.to_owned()))
}

fn split_pair(value: &str, key: &str) -> Result<(String, String), String> {
    let (head, rest) = split_once(value, key)?;
    if rest.is_empty() {
        return Err(format!("malformed {key} record {value}"));
    }
    Ok((head, rest))
}

fn validate_print_schema(text: &str) -> Result<(), String> {
    let lines: Vec<&str> = text.lines().collect();
    let first = lines.first().ok_or("empty print_result")?;
    if !first.starts_with("producer //") {
        return Err(format!(
            "first line must be producer: {}",
            first.chars().take(80).collect::<String>()
        ));
    }
    let capability = lines.get(1).ok_or("missing capability line")?;
    if !matches!(
        *capability,
        "capability LINT" | "capability FORMAT" | "capability TYPECHECK"
    ) {
        return Err(format!("bad capability: {capability}"));
    }
    let stages = header_count(&lines, "stages")?;
    let stage_rows = lines
        .iter()
        .filter(|line| line.starts_with("stage "))
        .count();
    if stage_rows != stages {
        return Err("stage count mismatch".to_owned());
    }
    if !lines
        .iter()
        .any(|line| line.starts_with("completed_rounds "))
    {
        return Err("missing completed_rounds".to_owned());
    }
    if !lines.contains(&"convergence STABLE") {
        return Err("convergence must stay STABLE".to_owned());
    }
    Ok(())
}

fn validate_print_counts(text: &str) -> Result<(), String> {
    let lines: Vec<&str> = text.lines().collect();
    let initial = header_count(&lines, "initial")?;
    let terminal = header_count(&lines, "terminal")?;
    let replacements = header_count(&lines, "replacements")?;
    if row_count(&lines, "initial") != initial {
        return Err("initial rows vs header".to_owned());
    }
    if row_count(&lines, "terminal") != terminal {
        return Err("terminal rows vs header".to_owned());
    }
    let replacement_rows = lines
        .iter()
        .filter(|line| line.starts_with("replacement "))
        .count();
    if replacement_rows != replacements {
        return Err("replacement rows vs header".to_owned());
    }
    if !text.ends_with('\n') {
        return Err("print_result must end with newline".to_owned());
    }
    Ok(())
}

fn header_count(lines: &[&str], prefix: &str) -> Result<usize, String> {
    for line in lines {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() == 2 && parts[0] == prefix {
            return parts[1]
                .parse::<usize>()
                .map_err(|_| format!("header {prefix} is not a count: {}", parts[1]));
        }
    }
    Err(format!("missing header {prefix}"))
}

fn row_count(lines: &[&str], prefix: &str) -> usize {
    let needle = format!("{prefix} ");
    lines
        .iter()
        .filter(|line| line.starts_with(&needle) && line.split_whitespace().count() > 2)
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(out: &mut String, key: &str, value: &str) {
        out.push_str(key);
        out.push('\0');
        out.push_str(value);
        out.push('\0');
    }

    fn complete_manifest() -> String {
        let mut text = String::new();
        record(&mut text, "format", "1");
        record(&mut text, "name", "matrix_demo");
        record(&mut text, "producer", "//quality/testdata:matrix_demo");
        record(&mut text, "capability", "lint");
        record(
            &mut text,
            "expected_rel",
            "quality/testdata/matrix/matrix_demo.expected.txt",
        );
        record(&mut text, "runner", "_main/quality/runner/quality_runner");
        record(&mut text, "printer", "_main/quality/result/print_result");
        record(
            &mut text,
            "expected",
            "_main/quality/testdata/matrix/matrix_demo.expected.txt",
        );
        record(&mut text, "stage", "lint-a;rust;src/lib.rs");
        record(
            &mut text,
            "source",
            "quality/testdata/clean.rs=_main/quality/testdata/clean.rs",
        );
        record(
            &mut text,
            "sibling",
            "quality/testdata/near.txt=_main/quality/testdata/near.txt",
        );
        record(&mut text, "tool_binary", "ruff=_main/tools/ruff");
        record(&mut text, "tool_config", "ruff=quality/testdata/ruff.toml");
        record(&mut text, "tool_edition", "ruff=2021");
        record(
            &mut text,
            "tool_file",
            "ruff=local.toml=_main/quality/testdata/local.toml",
        );
        record(&mut text, "tool_env", "ruff=RUNFILES_DIR=$(TEST_SRCDIR)");
        record(&mut text, "tool_env", "eslint=JS_BINARY__NO_CD_BINDIR=1");
        record(
            &mut text,
            "upstream",
            "clang_tidy=_main/quality/testdata/up.diagnostics",
        );
        text
    }

    #[test]
    fn a_complete_manifest_parses_with_every_record_kind() {
        let manifest = parse_manifest(&complete_manifest()).expect("manifest");
        assert_eq!(manifest.name, "matrix_demo");
        assert_eq!(manifest.capability, "lint");
        assert_eq!(manifest.stages, vec!["lint-a;rust;src/lib.rs"]);
        assert_eq!(
            manifest.sources,
            vec![(
                "quality/testdata/clean.rs".to_owned(),
                "_main/quality/testdata/clean.rs".to_owned()
            )]
        );
        assert_eq!(
            manifest.tool_files,
            vec![(
                "ruff".to_owned(),
                "local.toml".to_owned(),
                "_main/quality/testdata/local.toml".to_owned()
            )]
        );
        assert_eq!(
            manifest.tool_envs,
            vec![
                "ruff=RUNFILES_DIR=$(TEST_SRCDIR)".to_owned(),
                "eslint=JS_BINARY__NO_CD_BINDIR=1".to_owned()
            ]
        );
        assert_eq!(
            manifest.upstream,
            vec![(
                "clang_tidy".to_owned(),
                "_main/quality/testdata/up.diagnostics".to_owned()
            )]
        );
    }

    #[test]
    fn an_unknown_manifest_key_is_rejected() {
        let mut text = String::new();
        record(&mut text, "format", "1");
        record(&mut text, "mystery", "value");
        let error = parse_manifest(&text).expect_err("unknown");
        assert!(error.contains("unknown manifest key mystery"), "{error}");
    }

    #[test]
    fn a_missing_manifest_format_is_rejected() {
        let error = parse_manifest("name\0x\0").expect_err("missing format");
        assert!(error.contains("not supported"), "{error}");
    }

    #[test]
    fn a_duplicate_scalar_is_rejected() {
        let mut text = String::new();
        record(&mut text, "format", "1");
        record(&mut text, "name", "first");
        record(&mut text, "name", "second");
        let error = parse_manifest(&text).expect_err("duplicate");
        assert!(error.contains("duplicate manifest key name"), "{error}");
    }

    #[test]
    fn a_missing_required_scalar_is_rejected() {
        let mut text = String::new();
        record(&mut text, "format", "1");
        let error = parse_manifest(&text).expect_err("missing");
        assert!(error.contains("manifest is missing name"), "{error}");
    }

    #[test]
    fn a_source_record_without_a_mapping_is_rejected() {
        let mut text = String::new();
        record(&mut text, "format", "1");
        record(&mut text, "source", "no_mapping_here");
        let error = parse_manifest(&text).expect_err("malformed");
        assert!(error.contains("malformed source record"), "{error}");
    }

    #[test]
    fn a_tool_file_record_with_one_equals_is_rejected() {
        let mut text = String::new();
        record(&mut text, "format", "1");
        record(&mut text, "tool_file", "ruff=only-one-separator");
        let error = parse_manifest(&text).expect_err("malformed");
        assert!(error.contains("malformed tool_file record"), "{error}");
    }

    #[test]
    fn a_truncated_manifest_record_is_rejected() {
        let error = parse_manifest("format").expect_err("truncated");
        assert!(
            error.contains("manifest record format has no value"),
            "{error}"
        );
    }

    fn resolved_fixture() -> (Manifest, Resolved) {
        let manifest = parse_manifest(&complete_manifest()).expect("manifest");
        let resolved = Resolved {
            runner: PathBuf::from("/runfiles/quality_runner"),
            printer: PathBuf::from("/runfiles/print_result"),
            expected: PathBuf::from("/runfiles/matrix_demo.expected.txt"),
            sources: vec![(
                "quality/testdata/clean.rs".to_owned(),
                PathBuf::from("/runfiles/a clean.rs"),
            )],
            siblings: vec![(
                "quality/testdata/near.txt".to_owned(),
                PathBuf::from("/runfiles/near.txt"),
            )],
            tool_binaries: vec![("ruff".to_owned(), PathBuf::from("/runfiles/ruff tool"))],
            tool_configs: vec![("ruff".to_owned(), "quality/testdata/ruff.toml".to_owned())],
            tool_editions: vec![("ruff".to_owned(), "2021".to_owned())],
            tool_files: vec![(
                "ruff".to_owned(),
                "local.toml".to_owned(),
                PathBuf::from("/runfiles/local.toml"),
            )],
            tool_envs: vec!["ruff=RUNFILES_DIR=/runfiles".to_owned()],
            upstream: vec![(
                "clang_tidy".to_owned(),
                PathBuf::from("/runfiles/up.diagnostics"),
            )],
        };
        (manifest, resolved)
    }

    #[test]
    fn the_runner_argv_keeps_one_token_per_file_and_the_declared_order() {
        let (manifest, resolved) = resolved_fixture();
        let out = Path::new("/work/out.pb");
        let scratch = Path::new("/work/scratch");
        let argv = runner_argv(&manifest, &resolved, out, scratch);
        let words: Vec<String> = argv
            .iter()
            .map(|word| word.to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            words,
            vec![
                "/runfiles/quality_runner",
                "--producer",
                "//quality/testdata:matrix_demo",
                "--capability",
                "lint",
                "--output",
                "/work/out.pb",
                "--stage",
                "lint-a;rust;src/lib.rs",
                "--source",
                "quality/testdata/clean.rs=/runfiles/a clean.rs",
                "--sibling",
                "quality/testdata/near.txt=/runfiles/near.txt",
                "--real",
                "--scratch-parent",
                "/work/scratch",
                "--tool-binary",
                "ruff=/runfiles/ruff tool",
                "--tool-config",
                "ruff=quality/testdata/ruff.toml",
                "--tool-edition",
                "ruff=2021",
                "--tool-file",
                "ruff=local.toml=/runfiles/local.toml",
                "--tool-env",
                "ruff=RUNFILES_DIR=/runfiles",
                "--upstream-diagnostics",
                "clang_tidy=/runfiles/up.diagnostics",
            ]
        );
    }

    #[test]
    fn a_tool_env_placeholder_needs_the_test_runfiles_directory() {
        let filled = substitute_tool_env("ruff=RUNFILES_DIR=$(TEST_SRCDIR)", Some("/runfiles"))
            .expect("substituted");
        assert_eq!(filled, "ruff=RUNFILES_DIR=/runfiles");
        let plain = substitute_tool_env("eslint=JS_BINARY__NO_CD_BINDIR=1", None).expect("plain");
        assert_eq!(plain, "eslint=JS_BINARY__NO_CD_BINDIR=1");
        let error = substitute_tool_env("ruff=RUNFILES_DIR=$(TEST_SRCDIR)", None)
            .expect_err("missing srcdir");
        assert!(error.contains("TEST_SRCDIR is not set"), "{error}");
    }

    #[test]
    fn a_test_target_and_workspace_name_the_manifest_runfile() {
        assert_eq!(
            manifest_key("//quality/testdata:matrix_demo", "_main").expect("key"),
            "_main/quality/testdata/matrix_demo.manifest"
        );
        assert_eq!(
            manifest_key("@@//quality/testdata:matrix_demo", "_main").expect("key"),
            "_main/quality/testdata/matrix_demo.manifest"
        );
        let error = manifest_key("matrix_demo", "_main").expect_err("malformed");
        assert!(error.contains("is not //package:name"), "{error}");
    }

    fn temp_dir(name: &str) -> PathBuf {
        let base = nonempty_env("TEST_TMPDIR")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        let dir = base.join(format!("matrix-harness-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("scratch");
        dir
    }

    #[test]
    fn a_manifest_runfiles_source_is_preferred_over_the_tree() {
        let dir = temp_dir("manifest-first");
        let manifest = dir.join("runfiles_manifest");
        std::fs::write(&manifest, "_main/pkg/tool.txt /somewhere/tool.txt\n")
            .expect("write manifest");
        let tree = temp_dir("manifest-first-tree");
        std::fs::create_dir_all(tree.join("_main/pkg")).expect("tree");
        let resolver = resolver_from(
            Some(manifest.display().to_string()),
            Some(tree.display().to_string()),
        )
        .expect("manifest source");
        assert_eq!(resolver.source(), manifest);
    }

    #[test]
    fn a_tree_runfiles_source_is_used_without_a_manifest() {
        let tree = temp_dir("tree-only");
        std::fs::create_dir_all(tree.join("_main/pkg")).expect("tree");
        let resolver = resolver_from(None, Some(tree.display().to_string())).expect("tree");
        assert_eq!(resolver.source(), tree);
    }

    #[test]
    fn no_runfiles_source_at_all_is_reported() {
        let error = resolver_from(None, None).expect_err("nothing to read");
        assert!(error.contains("TEST_SRCDIR"), "{error}");
    }

    #[test]
    fn a_missing_runfile_names_its_key_and_path() {
        let tree = temp_dir("missing-runfile");
        let resolver = Resolver::from_tree(tree.clone()).expect("tree");
        let error = resolve_file(&resolver, "_main/pkg/absent.txt", "source").expect_err("absent");
        assert!(error.contains("_main/pkg/absent.txt"), "{error}");
        assert!(error.contains("missing runfile"), "{error}");
    }

    fn sample_print() -> String {
        let mut text = String::new();
        text.push_str("producer //quality/testdata:matrix_demo\n");
        text.push_str("capability LINT\n");
        text.push_str("stages 1\n");
        text.push_str("stage lint-a classes=rust sources=src/lib.rs\n");
        text.push_str("completed_rounds 1\n");
        text.push_str("convergence STABLE\n");
        text.push_str("initial 1\n");
        text.push_str("initial ERROR eslint rule src/a.js 0 5 fixable=false \"boom\"\n");
        text.push_str("terminal 1\n");
        text.push_str("terminal ERROR eslint rule src/a.js 0 5 fixable=false \"boom\"\n");
        text.push_str("replacements 1\n");
        text.push_str("replacement src/a.js 0 5 \"fixed\"\n");
        text
    }

    #[test]
    fn a_print_result_passes_both_validators() {
        let text = sample_print();
        validate_print_schema(&text).expect("schema");
        validate_print_counts(&text).expect("counts");
    }

    #[test]
    fn an_empty_print_result_fails_the_schema() {
        let error = validate_print_schema("").expect_err("empty");
        assert!(error.contains("empty print_result"), "{error}");
    }

    #[test]
    fn a_non_producer_first_line_fails_the_schema() {
        let error = validate_print_schema("nope\ncapability LINT\n").expect_err("first");
        assert!(error.contains("first line must be producer"), "{error}");
    }

    #[test]
    fn an_unsupported_capability_fails_the_schema() {
        let text = sample_print().replace("capability LINT", "capability AUDIT");
        let error = validate_print_schema(&text).expect_err("capability");
        assert!(error.contains("bad capability"), "{error}");
    }

    #[test]
    fn a_stage_count_mismatch_fails_the_schema() {
        let text = sample_print().replace("stages 1", "stages 2");
        let error = validate_print_schema(&text).expect_err("stages");
        assert!(error.contains("stage count mismatch"), "{error}");
    }

    #[test]
    fn a_missing_completed_rounds_header_fails_the_schema() {
        let text: String = sample_print()
            .lines()
            .filter(|line| !line.starts_with("completed_rounds "))
            .map(|line| format!("{line}\n"))
            .collect();
        let error = validate_print_schema(&text).expect_err("rounds");
        assert!(error.contains("missing completed_rounds"), "{error}");
    }

    #[test]
    fn a_non_stable_convergence_fails_the_schema() {
        let text = sample_print().replace("convergence STABLE", "convergence OSCILLATION");
        let error = validate_print_schema(&text).expect_err("convergence");
        assert!(error.contains("convergence must stay STABLE"), "{error}");
    }

    #[test]
    fn an_initial_row_count_mismatch_fails_the_counts() {
        let text = sample_print().replace("initial 1", "initial 2");
        let error = validate_print_counts(&text).expect_err("initial");
        assert!(error.contains("initial rows vs header"), "{error}");
    }

    #[test]
    fn a_terminal_row_count_mismatch_fails_the_counts() {
        let text = sample_print().replace("terminal 1", "terminal 2");
        let error = validate_print_counts(&text).expect_err("terminal");
        assert!(error.contains("terminal rows vs header"), "{error}");
    }

    #[test]
    fn a_replacement_row_count_mismatch_fails_the_counts() {
        let text = sample_print().replace("replacements 1", "replacements 2");
        let error = validate_print_counts(&text).expect_err("replacements");
        assert!(error.contains("replacement rows vs header"), "{error}");
    }

    #[test]
    fn a_missing_trailing_newline_fails_the_counts() {
        let text = sample_print().trim_end_matches('\n').to_owned();
        let error = validate_print_counts(&text).expect_err("newline");
        assert!(error.contains("must end with newline"), "{error}");
    }

    #[test]
    fn a_missing_initial_header_fails_the_counts() {
        let text: String = sample_print()
            .lines()
            .filter(|line| *line != "initial 1")
            .map(|line| format!("{line}\n"))
            .collect();
        let error = validate_print_counts(&text).expect_err("header");
        assert!(error.contains("missing header initial"), "{error}");
    }
}
