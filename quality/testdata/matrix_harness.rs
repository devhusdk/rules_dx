#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

//! Runs one quality matrix case from its declared test-input manifest.

use std::ffi::{OsStr, OsString};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

use dx_path::Resolver;
use dx_process::lifecycle::{run, CapturePolicy, ChildOutcome, EnvPolicy, SpawnSpec};
use quality_result::decode_validated;

const CAPTURE_LIMIT: usize = 8 * 1024 * 1024;
const CHILD_TIMEOUT: Duration = Duration::from_secs(240);
const COUNTS_INVALID: &str = "print_result counts inconsistent";
const RUNFILES_ROOT_PLACEHOLDER: &str = "@TEST_SRCDIR";
const SCHEMA_INVALID: &str = "print_result schema invalid";

struct Mapping {
    workspace: String,
    key: String,
}

struct ToolFile {
    tool: String,
    rel: String,
    key: String,
}

struct Manifest {
    name: String,
    producer: String,
    capability: String,
    runner: String,
    printer: String,
    expected: String,
    snapshot_dir: String,
    stages: Vec<String>,
    sources: Vec<Mapping>,
    siblings: Vec<Mapping>,
    tools: Vec<(String, String)>,
    configs: Vec<(String, String)>,
    editions: Vec<(String, String)>,
    tool_files: Vec<ToolFile>,
    env: Vec<String>,
    upstream: Vec<(String, String)>,
}

impl Manifest {
    fn parse(text: &str) -> Result<Manifest, String> {
        let mut manifest = Manifest {
            name: String::new(),
            producer: String::new(),
            capability: String::new(),
            runner: String::new(),
            printer: String::new(),
            expected: String::new(),
            snapshot_dir: String::new(),
            stages: Vec::new(),
            sources: Vec::new(),
            siblings: Vec::new(),
            tools: Vec::new(),
            configs: Vec::new(),
            editions: Vec::new(),
            tool_files: Vec::new(),
            env: Vec::new(),
            upstream: Vec::new(),
        };
        for (index, line) in text.lines().enumerate() {
            let fields: Vec<&str> = line.split('\t').collect();
            let number = index + 1;
            match fields[0] {
                "name" => set_scalar(&mut manifest.name, scalar(&fields, number)?, number, "name")?,
                "producer" => set_scalar(
                    &mut manifest.producer,
                    scalar(&fields, number)?,
                    number,
                    "producer",
                )?,
                "capability" => set_scalar(
                    &mut manifest.capability,
                    scalar(&fields, number)?,
                    number,
                    "capability",
                )?,
                "runner" => set_scalar(&mut manifest.runner, scalar(&fields, number)?, number, "runner")?,
                "printer" => set_scalar(
                    &mut manifest.printer,
                    scalar(&fields, number)?,
                    number,
                    "printer",
                )?,
                "expected" => set_scalar(
                    &mut manifest.expected,
                    scalar(&fields, number)?,
                    number,
                    "expected",
                )?,
                "snapshot_dir" => set_scalar(
                    &mut manifest.snapshot_dir,
                    scalar(&fields, number)?,
                    number,
                    "snapshot_dir",
                )?,
                "stage" => manifest.stages.push(scalar(&fields, number)?.to_owned()),
                "source" => manifest.sources.push(mapping(&fields, number)?),
                "sibling" => manifest.siblings.push(mapping(&fields, number)?),
                "tool" => {
                    let (tool, key) = pair(&fields, number)?;
                    manifest.tools.push((tool.to_owned(), key.to_owned()));
                }
                "config" => {
                    let (tool, rel) = pair(&fields, number)?;
                    manifest.configs.push((tool.to_owned(), rel.to_owned()));
                }
                "edition" => {
                    let (tool, value) = pair(&fields, number)?;
                    manifest.editions.push((tool.to_owned(), value.to_owned()));
                }
                "toolfile" => manifest.tool_files.push(tool_file(&fields, number)?),
                "env" => manifest.env.push(scalar(&fields, number)?.to_owned()),
                "upstream" => {
                    let (tool, key) = pair(&fields, number)?;
                    manifest.upstream.push((tool.to_owned(), key.to_owned()));
                }
                record => {
                    return Err(format!(
                        "manifest line {number}: unknown record {record:?}"
                    ));
                }
            }
        }
        for (record, value) in [
            ("name", &manifest.name),
            ("producer", &manifest.producer),
            ("capability", &manifest.capability),
            ("runner", &manifest.runner),
            ("printer", &manifest.printer),
            ("expected", &manifest.expected),
            ("snapshot_dir", &manifest.snapshot_dir),
        ] {
            if value.is_empty() {
                return Err(format!("manifest is missing {record}"));
            }
        }
        Ok(manifest)
    }
}

fn scalar<'a>(fields: &[&'a str], number: usize) -> Result<&'a str, String> {
    if fields.len() != 2 {
        return Err(format!("manifest line {number}: want two fields"));
    }
    Ok(fields[1])
}

fn pair<'a>(fields: &[&'a str], number: usize) -> Result<(&'a str, &'a str), String> {
    if fields.len() != 3 {
        return Err(format!("manifest line {number}: want three fields"));
    }
    Ok((fields[1], fields[2]))
}

fn mapping(fields: &[&str], number: usize) -> Result<Mapping, String> {
    let (workspace, key) = pair(fields, number)?;
    Ok(Mapping {
        workspace: workspace.to_owned(),
        key: key.to_owned(),
    })
}

fn tool_file(fields: &[&str], number: usize) -> Result<ToolFile, String> {
    if fields.len() != 4 {
        return Err(format!("manifest line {number}: want four fields"));
    }
    Ok(ToolFile {
        tool: fields[1].to_owned(),
        rel: fields[2].to_owned(),
        key: fields[3].to_owned(),
    })
}

fn set_scalar(
    slot: &mut String,
    value: &str,
    number: usize,
    record: &str,
) -> Result<(), String> {
    if !slot.is_empty() {
        return Err(format!("manifest line {number}: duplicate {record}"));
    }
    slot.push_str(value);
    Ok(())
}

fn main() {
    if let Err(message) = drive() {
        eprintln!("matrix FAIL: {message}");
        let _ = std::io::stdout().flush();
        let _ = std::io::stderr().flush();
        std::process::exit(1);
    }
}

fn drive() -> Result<(), String> {
    let manifest_key = required_env("DX_MATRIX_MANIFEST")?;
    let runfiles_root = required_env("TEST_SRCDIR")?;
    let test_tmpdir = required_env("TEST_TMPDIR")?;
    let resolver = resolver()?;
    let manifest_path = checked(&resolver, &manifest_key)?;
    let text = std::fs::read_to_string(&manifest_path)
        .map_err(|error| format!("cannot read the manifest: {error}"))?;
    let manifest = Manifest::parse(&text)?;

    let mut sources = Vec::with_capacity(manifest.sources.len());
    for entry in &manifest.sources {
        sources.push((entry.workspace.clone(), checked(&resolver, &entry.key)?));
    }
    let mut siblings = Vec::with_capacity(manifest.siblings.len());
    for entry in &manifest.siblings {
        siblings.push((entry.workspace.clone(), checked(&resolver, &entry.key)?));
    }
    let mut tools = Vec::with_capacity(manifest.tools.len());
    for (tool, key) in &manifest.tools {
        tools.push((tool.clone(), checked(&resolver, key)?));
    }
    let mut tool_files = Vec::with_capacity(manifest.tool_files.len());
    for entry in &manifest.tool_files {
        tool_files.push((
            entry.tool.clone(),
            entry.rel.clone(),
            checked(&resolver, &entry.key)?,
        ));
    }
    let mut upstream = Vec::with_capacity(manifest.upstream.len());
    for (tool, key) in &manifest.upstream {
        upstream.push((tool.clone(), checked(&resolver, key)?));
    }
    let runner = checked(&resolver, &manifest.runner)?;
    let printer = checked(&resolver, &manifest.printer)?;
    let expected = checked(&resolver, &manifest.expected)?;

    let work = PathBuf::from(&test_tmpdir).join("matrix_work");
    let scratch = work.join("scratch");
    std::fs::create_dir_all(&scratch)
        .map_err(|error| format!("cannot create the scratch directory: {error}"))?;
    let out = work.join("out.pb");
    let actual = work.join("actual.txt");

    let mut argv: Vec<OsString> = vec![runner.into_os_string()];
    push_flag(&mut argv, "--producer", OsStr::new(&manifest.producer));
    push_flag(&mut argv, "--capability", OsStr::new(&manifest.capability));
    push_flag(&mut argv, "--output", out.as_os_str());
    for stage in &manifest.stages {
        push_flag(&mut argv, "--stage", OsStr::new(stage));
    }
    for (workspace, path) in &sources {
        push_flag(&mut argv, "--source", &join_path(workspace, path));
    }
    for (workspace, path) in &siblings {
        push_flag(&mut argv, "--sibling", &join_path(workspace, path));
    }
    argv.push(OsString::from("--real"));
    push_flag(&mut argv, "--scratch-parent", scratch.as_os_str());
    for (tool, path) in &tools {
        push_flag(&mut argv, "--tool-binary", &join_path(tool, path));
    }
    for (tool, rel) in &manifest.configs {
        push_flag(&mut argv, "--tool-config", &join_text(tool, rel));
    }
    for (tool, edition) in &manifest.editions {
        push_flag(&mut argv, "--tool-edition", &join_text(tool, edition));
    }
    for (tool, rel, path) in &tool_files {
        push_flag(&mut argv, "--tool-file", &join_tool_file(tool, rel, path));
    }
    for spec in &manifest.env {
        push_flag(
            &mut argv,
            "--tool-env",
            OsStr::new(&spec.replace(RUNFILES_ROOT_PLACEHOLDER, &runfiles_root)),
        );
    }
    for (tool, path) in &upstream {
        push_flag(&mut argv, "--upstream-diagnostics", &join_path(tool, path));
    }
    match spawn(argv, &runfiles_root, "runner")? {
        ChildOutcome::Finished { exit, stdout, stderr } => {
            forward(&stdout, &stderr)?;
            if exit.code() != Some(0) {
                return Err("runner failed".to_owned());
            }
        }
        ChildOutcome::TimedOut => return Err("runner timed out".to_owned()),
        ChildOutcome::OutputTooLarge { limit } => {
            return Err(format!("runner output exceeded {limit} bytes"));
        }
    }

    let printer_argv = vec![printer.into_os_string(), out.clone().into_os_string()];
    let printed = match spawn(printer_argv, &runfiles_root, "printer")? {
        ChildOutcome::Finished { exit, stdout, stderr } => {
            forward(&[], &stderr)?;
            if exit.code() != Some(0) {
                return Err("printer failed".to_owned());
            }
            stdout
        }
        ChildOutcome::TimedOut => return Err("printer timed out".to_owned()),
        ChildOutcome::OutputTooLarge { limit } => {
            return Err(format!("printer output exceeded {limit} bytes"));
        }
    };
    std::fs::write(&actual, &printed)
        .map_err(|error| format!("cannot write the actual print_result: {error}"))?;

    let result = std::fs::read(&out)
        .map_err(|error| format!("cannot read the quality result: {error}"))?;
    if let Err(error) = decode_validated(&result) {
        eprintln!("{error}");
        return Err(SCHEMA_INVALID.to_owned());
    }

    let text = match std::str::from_utf8(&printed) {
        Ok(text) => text,
        Err(error) => {
            eprintln!("print_result is not UTF-8: {error}");
            return Err(SCHEMA_INVALID.to_owned());
        }
    };
    if let Err(detail) = check_schema(text) {
        eprintln!("{detail}");
        return Err(SCHEMA_INVALID.to_owned());
    }
    if let Err(detail) = check_counts(text) {
        eprintln!("{detail}");
        return Err(COUNTS_INVALID.to_owned());
    }

    if std::env::var("UPDATE_EXPECT").map(|value| value == "1").unwrap_or(false) {
        let out_dir = std::env::var_os("TEST_UNDECLARED_OUTPUTS_DIR")
            .filter(|value| !value.is_empty())
            .or_else(|| std::env::var_os("TMPDIR").filter(|value| !value.is_empty()))
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/tmp"));
        std::fs::create_dir_all(&out_dir)
            .map_err(|error| format!("cannot create {}: {error}", out_dir.display()))?;
        let staged = out_dir.join(format!("{}.expected.update", manifest.name));
        std::fs::write(&staged, &printed)
            .map_err(|error| format!("cannot stage the snapshot: {error}"))?;
        println!(
            "snapshot UPDATE_EXPECT: staged fresh actual at {}",
            staged.display()
        );
        println!(
            "copy it to {}/{}.expected.txt, then review before pinning.",
            manifest.snapshot_dir, manifest.name
        );
        println!("matrix PASS (updated): {}", manifest.name);
        return Ok(());
    }

    let golden = std::fs::read(&expected)
        .map_err(|error| format!("cannot read the golden snapshot: {error}"))?;
    if golden == printed {
        println!("matrix PASS: {}", manifest.name);
        return Ok(());
    }
    print!(
        "{}",
        unified_diff(
            &String::from_utf8_lossy(&golden),
            &String::from_utf8_lossy(&printed),
            &expected,
            &actual,
        )
    );
    println!("--- actual print_result:");
    forward(&printed, &[])?;
    println!(
        "re-run with UPDATE_EXPECT=1 to stage the fresh golden (bazel test --test_env=UPDATE_EXPECT), then review before pinning."
    );
    Err("golden snapshot mismatch (see diff above)".to_owned())
}

fn required_env(key: &str) -> Result<String, String> {
    std::env::var(key).map_err(|_| format!("{key} is not set"))
}

fn resolver() -> Result<Resolver, String> {
    let argv0 = std::env::args_os()
        .next()
        .ok_or_else(|| "the harness has no argv0".to_owned())?;
    let mut binary = PathBuf::from(argv0);
    if binary.is_relative() {
        let cwd = std::env::current_dir()
            .map_err(|error| format!("cannot read the working directory: {error}"))?;
        binary = cwd.join(binary);
    }
    if let Ok(resolver) = Resolver::for_binary(&binary) {
        return Ok(resolver);
    }
    let current = std::env::current_exe()
        .map_err(|error| format!("cannot read the current executable: {error}"))?;
    Resolver::for_binary(&current).map_err(|error| format!("no runfiles beside the harness: {error}"))
}

fn checked(resolver: &Resolver, key: &str) -> Result<PathBuf, String> {
    let path = lookup(resolver, key)?;
    if !path.is_file() {
        return Err(format!("missing runfile {}", path.display()));
    }
    Ok(path)
}

fn lookup(resolver: &Resolver, key: &str) -> Result<PathBuf, String> {
    resolver
        .lookup(key)
        .map_err(|_| format!("missing runfile {key}"))
}

fn spawn(argv: Vec<OsString>, runfiles_root: &str, label: &str) -> Result<ChildOutcome, String> {
    let cwd = std::env::current_dir()
        .map_err(|error| format!("{label}: cannot read the working directory: {error}"))?;
    let spec = SpawnSpec {
        argv,
        cwd,
        env: EnvPolicy::Inherited {
            extra: vec![(
                OsString::from("RUNFILES_DIR"),
                OsString::from(runfiles_root),
            )],
        },
        capture: CapturePolicy {
            max_bytes: CAPTURE_LIMIT,
        },
        timeout: CHILD_TIMEOUT,
    };
    run(&spec).map_err(|error| format!("{label}: {error}"))
}

fn forward(stdout: &[u8], stderr: &[u8]) -> Result<(), String> {
    let mut out = std::io::stdout();
    out.write_all(stdout)
        .map_err(|error| format!("cannot write stdout: {error}"))?;
    out.flush()
        .map_err(|error| format!("cannot write stdout: {error}"))?;
    let mut err = std::io::stderr();
    err.write_all(stderr)
        .map_err(|error| format!("cannot write stderr: {error}"))?;
    err.flush()
        .map_err(|error| format!("cannot write stderr: {error}"))?;
    Ok(())
}

fn push_flag(argv: &mut Vec<OsString>, name: &str, value: &OsStr) {
    argv.push(OsString::from(name));
    argv.push(value.to_os_string());
}

fn join_path(prefix: &str, path: &Path) -> OsString {
    let mut value = OsString::from(prefix);
    value.push("=");
    value.push(path);
    value
}

fn join_text(left: &str, right: &str) -> OsString {
    let mut value = OsString::from(left);
    value.push("=");
    value.push(right);
    value
}

fn join_tool_file(tool: &str, rel: &str, path: &Path) -> OsString {
    let mut value = OsString::from(tool);
    value.push("=");
    value.push(rel);
    value.push("=");
    value.push(path);
    value
}

fn split_lines(text: &str) -> Vec<&str> {
    if text.is_empty() {
        return Vec::new();
    }
    text.strip_suffix('\n')
        .unwrap_or(text)
        .split('\n')
        .collect()
}

fn check_schema(text: &str) -> Result<(), String> {
    let lines = split_lines(text);
    let first = lines.first().ok_or_else(|| "empty print_result".to_owned())?;
    if !first.starts_with("producer //") {
        let head: String = first.chars().take(80).collect();
        return Err(format!("first line must be producer: {head}"));
    }
    let capability = lines
        .get(1)
        .ok_or_else(|| "print_result has no capability line".to_owned())?;
    if !matches!(
        *capability,
        "capability LINT" | "capability FORMAT" | "capability TYPECHECK"
    ) {
        return Err(format!("bad capability: {capability}"));
    }
    let stages_line = lines
        .iter()
        .find(|line| line.starts_with("stages "))
        .ok_or_else(|| "print_result has no stages line".to_owned())?;
    let stages: usize = stages_line
        .split_whitespace()
        .nth(1)
        .and_then(|value| value.parse().ok())
        .ok_or_else(|| format!("bad stages line: {stages_line}"))?;
    let stage_rows = lines.iter().filter(|line| line.starts_with("stage ")).count();
    if stage_rows != stages {
        return Err("stage count mismatch".to_owned());
    }
    if !lines
        .iter()
        .any(|line| line.starts_with("completed_rounds "))
    {
        return Err("missing completed_rounds".to_owned());
    }
    if !lines.iter().any(|line| *line == "convergence STABLE") {
        return Err("convergence must stay STABLE".to_owned());
    }
    Ok(())
}

fn check_counts(text: &str) -> Result<(), String> {
    let lines = split_lines(text);
    let initial = header_count(&lines, "initial")?;
    let terminal = header_count(&lines, "terminal")?;
    let replacements = header_count(&lines, "replacements")?;
    if diagnostic_rows(&lines, "initial ") != initial {
        return Err("initial rows vs header".to_owned());
    }
    if diagnostic_rows(&lines, "terminal ") != terminal {
        return Err("terminal rows vs header".to_owned());
    }
    if replacement_rows(&lines) != replacements {
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
                .parse()
                .map_err(|error| format!("bad {prefix} header: {error}"));
        }
    }
    Err(format!("missing header {prefix}"))
}

fn diagnostic_rows(lines: &[&str], prefix: &str) -> usize {
    lines
        .iter()
        .filter(|line| line.starts_with(prefix) && line.split_whitespace().count() > 2)
        .count()
}

fn replacement_rows(lines: &[&str]) -> usize {
    lines
        .iter()
        .filter(|line| line.starts_with("replacement "))
        .count()
}

fn unified_diff(before: &str, after: &str, before_path: &Path, after_path: &Path) -> String {
    let before_lines: Vec<&str> = before.split('\n').collect();
    let after_lines: Vec<&str> = after.split('\n').collect();
    let mut prefix = 0;
    while prefix < before_lines.len()
        && prefix < after_lines.len()
        && before_lines[prefix] == after_lines[prefix]
    {
        prefix += 1;
    }
    let mut suffix = 0;
    while suffix + prefix < before_lines.len()
        && suffix + prefix < after_lines.len()
        && before_lines[before_lines.len() - 1 - suffix]
            == after_lines[after_lines.len() - 1 - suffix]
    {
        suffix += 1;
    }
    let head = prefix.saturating_sub(3);
    let before_tail = before_lines.len() - suffix;
    let after_tail = after_lines.len() - suffix;
    let before_end = (before_tail + 3).min(before_lines.len());
    let after_end = (after_tail + 3).min(after_lines.len());
    let mut diff = String::new();
    diff.push_str(&format!("--- {}\n", before_path.display()));
    diff.push_str(&format!("+++ {}\n", after_path.display()));
    diff.push_str(&format!(
        "@@ -{},{} +{},{} @@\n",
        head + 1,
        before_end - head,
        head + 1,
        after_end - head
    ));
    for line in &before_lines[head..prefix] {
        diff.push_str(&format!(" {line}\n"));
    }
    for line in &before_lines[prefix..before_tail] {
        diff.push_str(&format!("-{line}\n"));
    }
    for line in &after_lines[prefix..after_tail] {
        diff.push_str(&format!("+{line}\n"));
    }
    for line in &before_lines[before_tail..before_end] {
        diff.push_str(&format!(" {line}\n"));
    }
    diff
}
