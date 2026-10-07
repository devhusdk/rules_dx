#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

use std::ffi::OsString;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

use dx_path::Resolver;
use dx_process::lifecycle::{CapturePolicy, ChildOutcome, EnvPolicy, Exit, SpawnSpec};
use quality_result::decode_validated;

const MANIFEST_SCHEMA: &str = "1";
const SPAWN_TIMEOUT: Duration = Duration::from_secs(240);
const MAX_OUTPUT_BYTES: usize = 8 * 1024 * 1024;
const DIFF_CONTEXT: usize = 3;
const MAX_DIFF_CELLS: usize = 1_000_000;

#[derive(Debug)]
enum Token {
    Literal(String),
    Resolve { prefix: String, key: String },
    Env { prefix: String, name: String },
}

#[derive(Debug)]
struct Manifest {
    name: String,
    expected: String,
    runner: String,
    printer: String,
    tokens: Vec<Token>,
}

struct ChildOutput {
    code: Option<i32>,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

impl ChildOutput {
    fn succeeded(&self) -> bool {
        self.code == Some(0)
    }

    fn status(&self) -> String {
        match self.code {
            Some(code) => format!(" with exit code {code}"),
            None => String::from(" after a signal"),
        }
    }
}

fn main() {
    if let Err(message) = execute() {
        eprintln!("matrix FAIL: {message}");
        std::process::exit(1);
    }
}

fn execute() -> Result<(), String> {
    let argument = std::env::args_os()
        .nth(1)
        .ok_or_else(|| String::from("missing manifest argument"))?;
    let argument = argument
        .to_str()
        .ok_or_else(|| String::from("manifest argument is not valid UTF-8"))?;
    let resolver =
        Resolver::from_env().map_err(|error| format!("no runfiles in the environment: {error}"))?;
    let manifest_path = lookup_manifest(&resolver, argument)?;
    let text = std::fs::read_to_string(&manifest_path)
        .map_err(|error| format!("cannot read {}: {error}", manifest_path.display()))?;
    let manifest = parse_manifest(&text)?;

    let work = test_tmpdir().join("matrix_work");
    let scratch = work.join("scratch");
    std::fs::create_dir_all(&scratch)
        .map_err(|error| format!("cannot create {}: {error}", scratch.display()))?;
    let output = work.join("out.pb");
    let actual_path = work.join("actual.txt");

    let runner = resolve(&resolver, &manifest.runner)?;
    let mut argv = vec![runner.into_os_string()];
    for token in &manifest.tokens {
        argv.push(token_value(token, &resolver)?);
    }
    argv.push(OsString::from("--output"));
    argv.push(output.clone().into_os_string());
    argv.push(OsString::from("--scratch-parent"));
    argv.push(scratch.into_os_string());

    let runner_output = run_child(&argv, "runner")?;
    forward(&runner_output.stdout, &runner_output.stderr)?;
    if !runner_output.succeeded() {
        return Err(format!("runner failed{}", runner_output.status()));
    }

    let bytes = std::fs::read(&output)
        .map_err(|error| format!("cannot read {}: {error}", output.display()))?;
    decode_validated(&bytes).map_err(|error| format!("invalid result protobuf: {error}"))?;

    let printer = resolve(&resolver, &manifest.printer)?;
    let printer_argv = vec![printer.into_os_string(), output.into_os_string()];
    let printed = run_child(&printer_argv, "printer")?;
    forward(&[], &printed.stderr)?;
    if !printed.succeeded() {
        return Err(format!("printer failed{}", printed.status()));
    }

    let actual = printed.stdout;
    std::fs::write(&actual_path, &actual)
        .map_err(|error| format!("cannot write {}: {error}", actual_path.display()))?;
    let actual_text = String::from_utf8_lossy(&actual).into_owned();

    if let Err(detail) = check_schema(&actual_text) {
        eprintln!("print_result schema: {detail}");
        return Err(String::from("print_result schema invalid"));
    }
    if let Err(detail) = check_counts(&actual_text) {
        eprintln!("print_result counts: {detail}");
        return Err(String::from("print_result counts inconsistent"));
    }

    if update_expect() {
        return stage_update(&manifest.name, &actual);
    }

    let expected_path = resolve(&resolver, &manifest.expected)?;
    let expected = std::fs::read(&expected_path)
        .map_err(|error| format!("cannot read {}: {error}", expected_path.display()))?;
    if expected == actual {
        println!("matrix PASS: {}", manifest.name);
        return Ok(());
    }
    report_mismatch(&manifest.expected, &expected, &actual, &actual_text);
    Err(String::from("golden snapshot mismatch (see diff above)"))
}

fn lookup_manifest(resolver: &Resolver, argument: &str) -> Result<PathBuf, String> {
    let workspace = std::env::var("TEST_WORKSPACE").unwrap_or_else(|_| String::from("_main"));
    let candidates = if Path::new(argument).is_absolute() {
        vec![argument.to_owned()]
    } else {
        vec![format!("{workspace}/{argument}"), argument.to_owned()]
    };
    let mut failures = Vec::new();
    for candidate in &candidates {
        match resolve(resolver, candidate) {
            Ok(path) => return Ok(path),
            Err(failure) => failures.push(failure),
        }
    }
    Err(failures.join("; "))
}

fn resolve(resolver: &Resolver, key: &str) -> Result<PathBuf, String> {
    let path = resolver
        .lookup(key)
        .map_err(|error| format!("runfile {key}: {error}"))?;
    if !path.is_file() {
        return Err(format!("missing runfile {key} at {}", path.display()));
    }
    Ok(path)
}

fn token_value(token: &Token, resolver: &Resolver) -> Result<OsString, String> {
    match token {
        Token::Literal(value) => Ok(OsString::from(value)),
        Token::Resolve { prefix, key } => {
            let path = resolve(resolver, key)?;
            let mut value = OsString::from(prefix);
            value.push(path);
            Ok(value)
        }
        Token::Env { prefix, name } => {
            let value = std::env::var_os(name)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| format!("missing environment variable {name}"))?;
            let mut argument = OsString::from(prefix);
            argument.push(value);
            Ok(argument)
        }
    }
}

fn run_child(argv: &[OsString], label: &str) -> Result<ChildOutput, String> {
    let cwd = std::env::current_dir()
        .map_err(|error| format!("{label}: cannot read the working directory: {error}"))?;
    let spec = SpawnSpec {
        argv: argv.to_vec(),
        cwd,
        env: EnvPolicy::Inherited {
            extra: runfiles_dir_env(),
        },
        capture: CapturePolicy {
            max_bytes: MAX_OUTPUT_BYTES,
        },
        timeout: SPAWN_TIMEOUT,
    };
    let outcome = dx_process::lifecycle::run(&spec)
        .map_err(|error| format!("{label} could not start: {error}"))?;
    match outcome {
        ChildOutcome::Finished {
            exit,
            stdout,
            stderr,
        } => {
            let code = match exit {
                Exit::Code(code) => Some(code),
                Exit::Signal(_) => None,
            };
            Ok(ChildOutput {
                code,
                stdout,
                stderr,
            })
        }
        ChildOutcome::TimedOut => Err(format!(
            "{label} timed out after {}s",
            SPAWN_TIMEOUT.as_secs()
        )),
        ChildOutcome::OutputTooLarge { limit } => {
            Err(format!("{label} output exceeds max size {limit} bytes"))
        }
    }
}

fn runfiles_dir_env() -> Vec<(OsString, OsString)> {
    match std::env::var_os("TEST_SRCDIR").filter(|value| !value.is_empty()) {
        Some(value) => vec![(OsString::from("RUNFILES_DIR"), value)],
        None => Vec::new(),
    }
}

fn forward(stdout: &[u8], stderr: &[u8]) -> Result<(), String> {
    if !stdout.is_empty() {
        std::io::stdout()
            .write_all(stdout)
            .map_err(|error| format!("cannot write child output: {error}"))?;
    }
    if !stderr.is_empty() {
        std::io::stderr()
            .write_all(stderr)
            .map_err(|error| format!("cannot write child errors: {error}"))?;
    }
    Ok(())
}

fn check_schema(text: &str) -> Result<(), String> {
    let lines: Vec<&str> = text.lines().collect();
    let first = lines
        .first()
        .ok_or_else(|| String::from("empty print_result"))?;
    if !first.starts_with("producer //") {
        let shown: String = first.chars().take(80).collect();
        return Err(format!("first line must be producer: {shown}"));
    }
    let second = lines
        .get(1)
        .ok_or_else(|| String::from("missing capability line"))?;
    if !matches!(
        *second,
        "capability LINT" | "capability FORMAT" | "capability TYPECHECK"
    ) {
        return Err(format!("bad capability: {second}"));
    }
    let stages =
        header_number(&lines, "stages")?.ok_or_else(|| String::from("missing header stages"))?;
    let rendered = lines
        .iter()
        .filter(|line| line.starts_with("stage "))
        .count();
    if rendered as u64 != stages {
        return Err(String::from("stage count mismatch"));
    }
    if !lines
        .iter()
        .any(|line| line.starts_with("completed_rounds "))
    {
        return Err(String::from("missing completed_rounds"));
    }
    if !lines.contains(&"convergence STABLE") {
        return Err(String::from("convergence must stay STABLE"));
    }
    Ok(())
}

fn check_counts(text: &str) -> Result<(), String> {
    let lines: Vec<&str> = text.lines().collect();
    for prefix in ["initial", "terminal"] {
        let header =
            header_number(&lines, prefix)?.ok_or_else(|| format!("missing header {prefix}"))?;
        if row_count(&lines, prefix) as u64 != header {
            return Err(format!("{prefix} rows vs header"));
        }
    }
    let replacements = header_number(&lines, "replacements")?
        .ok_or_else(|| String::from("missing header replacements"))?;
    let rows = lines
        .iter()
        .filter(|line| line.starts_with("replacement "))
        .count();
    if rows as u64 != replacements {
        return Err(String::from("replacement rows vs header"));
    }
    if !text.ends_with('\n') {
        return Err(String::from("print_result must end with newline"));
    }
    Ok(())
}

fn header_number(lines: &[&str], prefix: &str) -> Result<Option<u64>, String> {
    for line in lines {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() == 2 && parts[0] == prefix {
            return parts[1]
                .parse::<u64>()
                .map(Some)
                .map_err(|_| format!("bad {prefix} header: {}", parts[1]));
        }
    }
    Ok(None)
}

fn row_count(lines: &[&str], prefix: &str) -> usize {
    let head = format!("{prefix} ");
    lines
        .iter()
        .filter(|line| line.starts_with(head.as_str()) && line.split_whitespace().count() > 2)
        .count()
}

fn update_expect() -> bool {
    std::env::var("UPDATE_EXPECT").as_deref() == Ok("1")
}

fn stage_update(name: &str, actual: &[u8]) -> Result<(), String> {
    let out_dir = std::env::var_os("TEST_UNDECLARED_OUTPUTS_DIR")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("TMPDIR")
                .filter(|value| !value.is_empty())
                .map(PathBuf::from)
        })
        .unwrap_or_else(|| PathBuf::from("/tmp"));
    std::fs::create_dir_all(&out_dir)
        .map_err(|error| format!("cannot create {}: {error}", out_dir.display()))?;
    let staged = out_dir.join(format!("{name}.expected.update"));
    std::fs::write(&staged, actual)
        .map_err(|error| format!("cannot stage {}: {error}", staged.display()))?;
    println!(
        "snapshot UPDATE_EXPECT: staged fresh actual at {}",
        staged.display()
    );
    println!("copy it to quality/testdata/matrix/{name}.expected.txt, then review before pinning.");
    println!("matrix PASS (updated): {name}");
    Ok(())
}

fn report_mismatch(label: &str, expected: &[u8], actual: &[u8], actual_text: &str) {
    let diff = match (std::str::from_utf8(expected), std::str::from_utf8(actual)) {
        (Ok(expected_text), Ok(actual_text)) => unified_diff(label, expected_text, actual_text),
        _ => String::from("(expected and actual are not both UTF-8)\n"),
    };
    print!("{diff}");
    println!("--- actual print_result:");
    print!("{actual_text}");
    println!(
        "re-run with UPDATE_EXPECT=1 to stage the fresh golden (bazel test --test_env=UPDATE_EXPECT), then review before pinning."
    );
}

fn unified_diff(label: &str, expected: &str, actual: &str) -> String {
    let mut out = format!("--- expected: {label}\n+++ actual\n");
    if expected == actual {
        return out;
    }
    let old: Vec<&str> = expected.lines().collect();
    let new: Vec<&str> = actual.lines().collect();
    let ops = line_ops(&old, &new);
    if !ops.iter().any(|(tag, _)| *tag != ' ') {
        out.push_str("(the two files differ only in trailing bytes)\n");
        return out;
    }
    for (start, end) in hunks(&ops) {
        let mut old_start = 0usize;
        let mut new_start = 0usize;
        let mut old_len = 0usize;
        let mut new_len = 0usize;
        for (index, (tag, _)) in ops.iter().enumerate() {
            if index >= start && index < end {
                if *tag != '+' {
                    old_len += 1;
                }
                if *tag != '-' {
                    new_len += 1;
                }
            } else if index < start {
                if *tag != '+' {
                    old_start += 1;
                }
                if *tag != '-' {
                    new_start += 1;
                }
            }
        }
        let old_head = if old_len == 0 {
            old_start
        } else {
            old_start + 1
        };
        let new_head = if new_len == 0 {
            new_start
        } else {
            new_start + 1
        };
        out.push_str(&format!(
            "@@ -{old_head},{old_len} +{new_head},{new_len} @@\n"
        ));
        for (tag, line) in &ops[start..end] {
            out.push(*tag);
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

fn hunks(ops: &[(char, &str)]) -> Vec<(usize, usize)> {
    let mut hunks: Vec<(usize, usize)> = Vec::new();
    for (index, (tag, _)) in ops.iter().enumerate() {
        if *tag == ' ' {
            continue;
        }
        let start = index.saturating_sub(DIFF_CONTEXT);
        let end = (index + DIFF_CONTEXT + 1).min(ops.len());
        match hunks.last_mut() {
            Some(last) if last.1 >= start => last.1 = last.1.max(end),
            _ => hunks.push((start, end)),
        }
    }
    hunks
}

fn line_ops<'a>(old: &[&'a str], new: &[&'a str]) -> Vec<(char, &'a str)> {
    let rows = old.len();
    let columns = new.len();
    if rows.saturating_mul(columns) > MAX_DIFF_CELLS {
        let mut ops: Vec<(char, &'a str)> = old.iter().map(|line| ('-', *line)).collect();
        ops.extend(new.iter().map(|line| ('+', *line)));
        return ops;
    }
    let stride = columns + 1;
    let mut table = vec![0u32; (rows + 1) * stride];
    for row in (0..rows).rev() {
        for column in (0..columns).rev() {
            table[row * stride + column] = if old[row] == new[column] {
                table[(row + 1) * stride + column + 1] + 1
            } else {
                table[(row + 1) * stride + column].max(table[row * stride + column + 1])
            };
        }
    }
    let mut ops = Vec::with_capacity(rows + columns);
    let (mut row, mut column) = (0usize, 0usize);
    while row < rows && column < columns {
        if old[row] == new[column] {
            ops.push((' ', old[row]));
            row += 1;
            column += 1;
        } else if table[(row + 1) * stride + column] >= table[row * stride + column + 1] {
            ops.push(('-', old[row]));
            row += 1;
        } else {
            ops.push(('+', new[column]));
            column += 1;
        }
    }
    ops.extend(old[row..].iter().map(|line| ('-', *line)));
    ops.extend(new[column..].iter().map(|line| ('+', *line)));
    ops
}

fn parse_manifest(text: &str) -> Result<Manifest, String> {
    let mut schema: Option<String> = None;
    let mut name: Option<String> = None;
    let mut expected: Option<String> = None;
    let mut runner: Option<String> = None;
    let mut printer: Option<String> = None;
    let mut tokens: Vec<Token> = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let number = index + 1;
        let fields = split_fields(line);
        let kind = fields[0];
        match kind {
            "schema" => {
                let value = one_value(&fields, number, kind)?;
                if index != 0 {
                    return Err(format!("line {number}: schema must open the manifest"));
                }
                if value != MANIFEST_SCHEMA {
                    return Err(format!(
                        "line {number}: unsupported schema {value}: want {MANIFEST_SCHEMA}"
                    ));
                }
                schema = Some(value.to_owned());
            }
            "name" | "expected" | "runner" | "printer" => {
                let value = one_value(&fields, number, kind)?;
                let slot = match kind {
                    "name" => &mut name,
                    "expected" => &mut expected,
                    "runner" => &mut runner,
                    _ => &mut printer,
                };
                if slot.is_some() {
                    return Err(format!("line {number}: duplicate {kind} entry"));
                }
                *slot = Some(value.to_owned());
            }
            "arg" => tokens.push(Token::Literal(one_value(&fields, number, kind)?.to_owned())),
            "ref" => {
                require_fields(&fields, 3, number, kind)?;
                tokens.push(Token::Resolve {
                    prefix: fields[1].to_owned(),
                    key: fields[2].to_owned(),
                });
            }
            "env" => {
                require_fields(&fields, 3, number, kind)?;
                tokens.push(Token::Env {
                    prefix: fields[1].to_owned(),
                    name: fields[2].to_owned(),
                });
            }
            _ => return Err(format!("line {number}: unknown directive {kind}")),
        }
    }
    if schema.is_none() {
        return Err(String::from("manifest must open with a schema line"));
    }
    Ok(Manifest {
        name: required(name, "name")?,
        expected: required(expected, "expected")?,
        runner: required(runner, "runner")?,
        printer: required(printer, "printer")?,
        tokens,
    })
}

fn split_fields(line: &str) -> Vec<&str> {
    line.split('\t').collect()
}

fn one_value<'a>(fields: &[&'a str], number: usize, kind: &str) -> Result<&'a str, String> {
    require_fields(fields, 2, number, kind)?;
    Ok(fields[1])
}

fn require_fields(fields: &[&str], want: usize, number: usize, kind: &str) -> Result<(), String> {
    if fields.len() != want {
        return Err(format!(
            "line {number}: {kind} needs {want} fields, found {}",
            fields.len()
        ));
    }
    Ok(())
}

fn required(slot: Option<String>, name: &str) -> Result<String, String> {
    slot.ok_or_else(|| format!("missing {name} entry"))
}

fn test_tmpdir() -> PathBuf {
    std::env::var_os("TEST_TMPDIR")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    const PRINTED: &str = "\
producer //quality/testdata:matrix_demo
capability LINT
stages 1
stage clippy classes=clippy sources=a.rs
completed_rounds 1
convergence STABLE
initial 1
initial ERROR clippy a.rs 0 3 fixable=false \"bad\"
terminal 1
terminal ERROR clippy a.rs 0 3 fixable=false \"bad\"
replacements 1
replacement a.rs 0 3 \"good\"
";

    fn manifest() -> Manifest {
        parse_manifest(
            "schema\t1\n\
             name\tmatrix_demo\n\
             expected\t_main/quality/testdata/matrix/matrix_demo.expected.txt\n\
             runner\t_main/quality/runner/quality_runner\n\
             printer\t_main/quality/result/print_result\n\
             arg\t--real\n\
             ref\tclippy=\t_main/quality/testdata/fake_clippy\n\
             env\tpydoclint=RUNFILES_DIR=\tTEST_SRCDIR\n",
        )
        .expect("manifest")
    }

    #[test]
    fn a_manifest_reads_its_headers_and_tokens() {
        let manifest = manifest();
        assert_eq!(manifest.name, "matrix_demo");
        assert_eq!(
            manifest.expected,
            "_main/quality/testdata/matrix/matrix_demo.expected.txt"
        );
        assert_eq!(manifest.tokens.len(), 3);
        assert!(matches!(&manifest.tokens[0], Token::Literal(value) if value == "--real"));
        assert!(matches!(
            &manifest.tokens[1],
            Token::Resolve { prefix, key }
                if prefix == "clippy=" && key == "_main/quality/testdata/fake_clippy"
        ));
        assert!(matches!(
            &manifest.tokens[2],
            Token::Env { prefix, name } if prefix == "pydoclint=RUNFILES_DIR=" && name == "TEST_SRCDIR"
        ));
    }

    #[test]
    fn a_manifest_without_a_schema_is_rejected() {
        let error = parse_manifest("name\tmatrix_demo\n").expect_err("no schema");
        assert!(error.contains("schema"), "{error}");
    }

    #[test]
    fn a_manifest_with_an_unknown_directive_is_rejected() {
        let error = parse_manifest("schema\t1\nnonsense\tvalue\n").expect_err("unknown");
        assert!(error.contains("unknown directive nonsense"), "{error}");
    }

    #[test]
    fn a_manifest_with_a_short_line_is_rejected() {
        let error = parse_manifest("schema\t1\nrunner\n").expect_err("short");
        assert!(error.contains("line 2: runner needs 2 fields"), "{error}");
    }

    #[test]
    fn a_manifest_with_a_missing_header_is_rejected() {
        let error = parse_manifest("schema\t1\nname\tonly\n").expect_err("missing");
        assert!(error.contains("missing expected entry"), "{error}");
    }

    #[test]
    fn a_printed_result_that_matches_the_contract_passes() {
        assert!(check_schema(PRINTED).is_ok());
        assert!(check_counts(PRINTED).is_ok());
    }

    #[test]
    fn an_empty_print_result_fails_the_schema_check() {
        let error = check_schema("").expect_err("empty");
        assert_eq!(error, "empty print_result");
    }

    #[test]
    fn a_producer_that_is_not_a_label_fails_the_schema_check() {
        let error = check_schema("producer quality\n").expect_err("producer");
        assert!(error.starts_with("first line must be producer:"), "{error}");
    }

    #[test]
    fn an_unknown_capability_fails_the_schema_check() {
        let error = check_schema("producer //quality/testdata:x\ncapability AUDIT\n")
            .expect_err("capability");
        assert_eq!(error, "bad capability: capability AUDIT");
    }

    #[test]
    fn a_stage_count_that_disagrees_fails_the_schema_check() {
        let text = PRINTED.replace("stages 1", "stages 2");
        let error = check_schema(&text).expect_err("stages");
        assert_eq!(error, "stage count mismatch");
    }

    #[test]
    fn a_missing_convergence_line_fails_the_schema_check() {
        let text = PRINTED.replace("convergence STABLE", "convergence OSCILLATION");
        let error = check_schema(&text).expect_err("convergence");
        assert_eq!(error, "convergence must stay STABLE");
    }

    #[test]
    fn an_initial_row_that_disagrees_with_its_header_fails_the_counts_check() {
        let text = PRINTED.replace("initial 1", "initial 2");
        let error = check_counts(&text).expect_err("initial");
        assert_eq!(error, "initial rows vs header");
    }

    #[test]
    fn a_replacement_row_that_disagrees_with_its_header_fails_the_counts_check() {
        let text = PRINTED.replace("replacements 1", "replacements 0");
        let error = check_counts(&text).expect_err("replacements");
        assert_eq!(error, "replacement rows vs header");
    }

    #[test]
    fn a_result_without_a_trailing_newline_fails_the_counts_check() {
        let error = check_counts(PRINTED.trim_end()).expect_err("newline");
        assert_eq!(error, "print_result must end with newline");
    }

    #[test]
    fn an_identical_snapshot_renders_no_diff() {
        let diff = unified_diff("expected.txt", PRINTED, PRINTED);
        assert_eq!(diff, "--- expected: expected.txt\n+++ actual\n");
    }

    #[test]
    fn a_changed_line_renders_a_unified_hunk() {
        let changed = PRINTED.replace("completed_rounds 1", "completed_rounds 2");
        let diff = unified_diff("expected.txt", PRINTED, &changed);
        assert!(diff.contains("--- expected: expected.txt"), "{diff}");
        assert!(diff.contains("@@"), "{diff}");
        assert!(diff.contains("-completed_rounds 1"), "{diff}");
        assert!(diff.contains("+completed_rounds 2"), "{diff}");
    }

    #[test]
    fn a_snapshot_that_differs_only_in_trailing_bytes_says_so() {
        let diff = unified_diff("expected.txt", PRINTED, PRINTED.trim_end_matches('\n'));
        assert!(diff.contains("differ only in trailing bytes"), "{diff}");
    }

    #[test]
    fn distant_changes_split_into_separate_hunks() {
        let old: Vec<String> = (0..40).map(|index| format!("line {index}")).collect();
        let new: Vec<String> = old
            .iter()
            .enumerate()
            .map(|(index, line)| {
                if index == 0 || index == 39 {
                    line.to_uppercase()
                } else {
                    line.clone()
                }
            })
            .collect();
        let old_refs: Vec<&str> = old.iter().map(String::as_str).collect();
        let new_refs: Vec<&str> = new.iter().map(String::as_str).collect();
        let diff = unified_diff("expected.txt", &old_refs.join("\n"), &new_refs.join("\n"));
        assert_eq!(diff.matches("@@ ").count(), 2, "{diff}");
    }

    #[test]
    fn split_fields_rejects_nothing_on_a_plain_line() {
        assert_eq!(split_fields("schema\t1"), vec!["schema", "1"]);
        assert_eq!(split_fields(""), vec![""]);
    }
}
