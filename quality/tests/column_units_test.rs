//! Runs pinned tools on lines with an astral emoji and `é` before the finding,
//! so each parser's column units are checked against what the tool reports.

use std::path::{Path, PathBuf};

use dx_testing::serde_json::Value;
use quality_adapter::commands::{self, Invocation};
use quality_adapter::launch::ResolvedTool;
use quality_adapter::parsers::{self, FileFinding};
use quality_adapter::{place_finding, ColumnEncoding};

fn tool(variable: &str) -> PathBuf {
    let rel = std::env::var(variable).unwrap_or_else(|_| panic!("{variable} must be set"));
    dx_testing::resolve_runfiles(&rel)
}

struct Run {
    code: Option<i32>,
    stdout: String,
    combined: String,
}

/// Runs `invocation` in `cwd`, with the launcher's own runfiles when it has them.
fn run(invocation: &Invocation, cwd: &Path) -> Run {
    let program = Path::new(&invocation.argv[0]);
    let mut command = std::process::Command::new(program);
    command
        .args(&invocation.argv[1..])
        .current_dir(cwd)
        .env("BAZEL_BINDIR", ".");
    let mut own = program.as_os_str().to_owned();
    own.push(".runfiles");
    if Path::new(&own).is_dir() {
        command.env_remove("RUNFILES_MANIFEST_FILE");
        command.env("RUNFILES_DIR", &own);
    }
    let output = command.output().expect("run tool");
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr);
    Run {
        code: output.status.code(),
        combined: format!("{stdout}{stderr}"),
        stdout,
    }
}

fn write(dir: &Path, name: &str, text: &str) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, text).expect("write fixture");
    path
}

/// The byte range of the one finding with `rule`, placed in `text`.
fn placed(findings: &[FileFinding], rule: &str, text: &str, context: &str) -> (u64, Option<u64>) {
    let found = findings
        .iter()
        .find(|found| found.finding.rule_id.ends_with(rule))
        .unwrap_or_else(|| panic!("no {rule} finding ({context}): {findings:#?}"));
    let placed = place_finding(&found.finding, "fixture", text)
        .unwrap_or_else(|err| panic!("{rule} did not place ({context}): {err}: {found:#?}"));
    (
        placed.start_byte.expect("start byte"),
        placed
            .end_byte
            .filter(|end| Some(*end) != placed.start_byte),
    )
}

fn byte_of(text: &str, marker: &str) -> u64 {
    text.find(marker).expect("marker in fixture") as u64
}

/// The SARIF driver version and whether the run states its `columnKind`.
fn sarif_facts(stdout: &str) -> (String, Option<String>) {
    let log: Value = dx_testing::serde_json::from_str(stdout).expect("SARIF output");
    let run = &log["runs"][0];
    let version = run["tool"]["driver"]["version"]
        .as_str()
        .or_else(|| run["tool"]["driver"]["semanticVersion"].as_str())
        .unwrap_or("unknown")
        .to_owned();
    (version, run["columnKind"].as_str().map(str::to_owned))
}

/// Asserts a SARIF tool's `rule` findings land on `markers` in line order,
/// whether the run states its column units or leaves them to `absent`.
fn assert_sarif_lands(
    out: &Run,
    name: &str,
    findings: &[FileFinding],
    absent: ColumnEncoding,
    rule: &str,
    text: &str,
    markers: &[&str],
) {
    let stdout = &out.combined;
    let (version, kind) = sarif_facts(&out.stdout);
    let context = format!("{name} {version}, columnKind {kind:?}");
    let mut found: Vec<&FileFinding> = findings
        .iter()
        .filter(|found| found.finding.rule_id.ends_with(rule))
        .collect();
    found.sort_by_key(|found| found.finding.start.line);
    assert_eq!(found.len(), markers.len(), "{context}: {stdout}");
    let expected = match kind.as_deref() {
        Some("unicodeCodePoints") => ColumnEncoding::Scalars,
        Some("utf16CodeUnits") => ColumnEncoding::Utf16,
        None => absent,
        Some(other) => panic!("unexpected columnKind {other} ({context})"),
    };
    for (found, marker) in found.into_iter().zip(markers) {
        let encoding = match found.finding.encoding {
            ColumnEncoding::ScalarsTabs { .. } => ColumnEncoding::Scalars,
            other => other,
        };
        assert_eq!(encoding, expected, "{context}");
        let placed = place_finding(&found.finding, "fixture", text)
            .unwrap_or_else(|err| panic!("{rule} did not place ({context}): {err}: {stdout}"));
        let line_start = text
            .split_inclusive('\n')
            .take(found.finding.start.line as usize - 1)
            .map(str::len)
            .sum::<usize>();
        let want = line_start + text[line_start..].find(marker).expect("marker on line");
        assert_eq!(
            placed.start_byte,
            Some(want as u64),
            "{context} {marker:?}: {stdout}"
        );
    }
}

#[test]
fn eslint_columns_are_utf16_units() {
    let dir = dx_testing::mkscratch("eslint").expect("scratch");
    let text = "const s = \"\u{1F600}\u{e9}\"; missing();\n";
    let file = write(&dir, "a.js", text);
    let config = write(
        &dir,
        "eslint.config.mjs",
        "export default [{ rules: { \"no-undef\": \"error\" } }];\n",
    );
    let launch = ResolvedTool::javascript("eslint", &tool("DX_ESLINT"));
    let invocation = commands::eslint_check(&launch, &[file.as_path()], &config).expect("argv");
    let out = run(&invocation, &dir);
    let name = file.to_str().expect("utf-8 path");
    let findings = parsers::parse_eslint(out.stdout.as_bytes(), out.code, &[name])
        .unwrap_or_else(|err| panic!("eslint output: {err}: {}", out.combined));
    let start = byte_of(text, "missing");
    assert_eq!(
        placed(&findings, "no-undef", text, "eslint"),
        (start, Some(start + 7)),
        "{}",
        out.combined
    );
}

#[test]
fn checkstyle_columns_are_tab_expanded_scalars() {
    let dir = dx_testing::mkscratch("checkstyle").expect("scratch");
    let text = "class A {\n  String s = \"\u{1F600}\u{e9}\"; int x=1;\n\tString t = \"\u{1F600}\u{e9}\"; int y=2;\n\tString u = \"\u{1F600}\u{1F600}\u{1F600}\u{1F600}\u{1F600}\u{1F600}\u{1F600}\u{1F600}\"; int z=3;\n}\n";
    let file = write(&dir, "A.java", text);
    let config = "<?xml version=\"1.0\"?>\n<!DOCTYPE module PUBLIC \"-//Checkstyle//DTD Checkstyle Configuration 1.3//EN\" \"https://checkstyle.org/dtds/configuration_1_3.dtd\">\n<module name=\"Checker\">\n  <module name=\"TreeWalker\">\n    <module name=\"WhitespaceAround\"/>\n  </module>\n</module>\n";
    let config_path = write(&dir, "checkstyle.xml", config);
    let invocation =
        commands::checkstyle_check(&tool("DX_CHECKSTYLE"), &[file.as_path()], &config_path);
    let out = run(&invocation, &dir);
    let name = file.to_str().expect("utf-8 path");
    let findings = parsers::parse_checkstyle(
        out.stdout.as_bytes(),
        out.code,
        &[name],
        parsers::checkstyle_tab_width(config.as_bytes()),
    )
    .unwrap_or_else(|err| panic!("checkstyle output: {err}: {}", out.combined));
    assert_sarif_lands(
        &out,
        "checkstyle",
        &findings,
        ColumnEncoding::Scalars,
        "WhitespaceAroundCheck",
        text,
        &["=1", "=1", "=2", "=2", "=3", "=3"],
    );
}

#[test]
fn pmd_sarif_columns_land_after_an_emoji() {
    let dir = dx_testing::mkscratch("pmd").expect("scratch");
    let text = "class B {\n  void f() { String s = \"\u{1F600}\u{e9}\"; int unused = (1) + s.length(); }\n}\n";
    let file = write(&dir, "B.java", text);
    let ruleset = write(
        &dir,
        "ruleset.xml",
        "<?xml version=\"1.0\"?>\n<ruleset name=\"t\" xmlns=\"http://pmd.sourceforge.net/ruleset/2.0.0\">\n  <description>t</description>\n  <rule ref=\"category/java/codestyle.xml/UselessParentheses\"/>\n</ruleset>\n",
    );
    let invocation = commands::pmd_check(&tool("DX_PMD"), &[file.as_path()], Some(&ruleset));
    let out = run(&invocation, &dir);
    let name = file.to_str().expect("utf-8 path");
    let findings = parsers::parse_pmd(out.stdout.as_bytes(), out.code, &[name])
        .unwrap_or_else(|err| panic!("pmd output: {err}: {}", out.combined));
    assert_sarif_lands(
        &out,
        "pmd",
        &findings,
        ColumnEncoding::Utf16,
        "UselessParentheses",
        text,
        &["(1)"],
    );
}

#[test]
fn ktlint_sarif_columns_land_after_an_emoji() {
    let dir = dx_testing::mkscratch("ktlint").expect("scratch");
    let text = "val s = \"\u{1F600}\u{e9}\" +  \"x\"\n";
    let file = write(&dir, "S.kt", text);
    let invocation = commands::ktlint_check(&tool("DX_KTLINT"), &[file.as_path()], None);
    let out = run(&invocation, &dir);
    let name = file.to_str().expect("utf-8 path");
    let findings = parsers::parse_ktlint(out.stdout.as_bytes(), out.code, &[name])
        .unwrap_or_else(|err| panic!("ktlint output: {err}: {}", out.combined));
    assert_sarif_lands(
        &out,
        "ktlint",
        &findings,
        ColumnEncoding::Utf16,
        "no-multi-spaces",
        text,
        &[" \"x\""],
    );
}
