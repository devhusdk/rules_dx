//! Compiler evidence table: merges evidence lines into one markdown table of compiler row x platform.

use serde_json::Value;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

fn jsonl_files(dir: &Path, found: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries = std::fs::read_dir(dir).map_err(|error| format!("read {dir:?}: {error}"))?;
    for entry in entries {
        let path = entry.map_err(|error| error.to_string())?.path();
        if path.is_dir() {
            jsonl_files(&path, found)?;
        } else if path.extension().is_some_and(|extension| extension == "jsonl") {
            found.push(path);
        }
    }
    Ok(())
}

fn merged_lines(dir: &Path) -> Result<Vec<String>, String> {
    let mut files = Vec::new();
    jsonl_files(dir, &mut files)?;
    files.sort();
    let mut lines = Vec::new();
    for file in files {
        let text =
            std::fs::read_to_string(&file).map_err(|error| format!("read {file:?}: {error}"))?;
        lines.extend(
            text.lines()
                .map(|line| line.replace('\r', ""))
                .filter(|line| !line.is_empty()),
        );
    }
    Ok(lines)
}

fn field<'a>(line: &'a Value, key: &str) -> &'a str {
    line[key].as_str().unwrap_or_default()
}

fn cell(lines: &[Value], row: &str, platform: &str) -> &'static str {
    let found: Vec<&Value> = lines
        .iter()
        .filter(|line| field(line, "row") == row && field(line, "platform") == platform)
        .collect();
    let has = |outcome: &str| found.iter().any(|line| field(line, "outcome") == outcome);
    if found.is_empty() {
        "no evidence"
    } else if has("failed") {
        "failed"
    } else if has("mismatch") {
        "mismatch"
    } else if has("blocked") {
        let stale = found.iter().any(|line| {
            field(line, "outcome") == "blocked" && field(line, "tested_outcome") == "passed"
        });
        if stale { "blocked, passed" } else { "blocked" }
    } else if has("passed") {
        "passed"
    } else {
        "skipped"
    }
}

fn render(lines: &[Value], platforms: &[String]) -> String {
    let platforms: Vec<&str> = platforms
        .iter()
        .map(String::as_str)
        .chain(lines.iter().map(|line| field(line, "platform")))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let rows: BTreeSet<&str> = lines.iter().map(|line| field(line, "row")).collect();
    let mut out = format!("| row | {} |\n| --- |", platforms.join(" | "));
    for _ in &platforms {
        out.push_str(" --- |");
    }
    out.push('\n');
    for row in rows {
        let cells: Vec<&str> = platforms
            .iter()
            .map(|platform| cell(lines, row, platform))
            .collect();
        out.push_str(&format!("| {row} | {} |\n", cells.join(" | ")));
    }
    out
}

/// `--table EVIDENCE_DIR OUT PLATFORM...`: writes the merged lines to OUT and prints the table.
pub fn print(args: &[String]) -> Result<(), String> {
    let [dir, out, platforms @ ..] = args else {
        return Err("usage: producers_bin --table EVIDENCE_DIR OUT PLATFORM...".to_owned());
    };
    let lines = merged_lines(Path::new(dir))?;
    let mut text = lines.join("\n");
    if !lines.is_empty() {
        text.push('\n');
    }
    std::fs::write(out, text).map_err(|error| format!("write {out}: {error}"))?;
    let parsed: Vec<Value> = lines
        .iter()
        .map(|line| serde_json::from_str(line).map_err(|error| format!("{line}: {error}")))
        .collect::<Result<_, _>>()?;
    print!("{}", render(&parsed, platforms));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table_of(lines: &[&str], platforms: &[&str]) -> (String, String) {
        let dir = dx_test_scratch::scratch("compiler-table");
        let input = dir.path().join("in");
        std::fs::create_dir_all(&input).expect("create evidence dir");
        std::fs::write(input.join("a.jsonl"), lines.join("\n") + "\n").expect("write evidence");
        let out = dir.path().join("all.jsonl");
        let merged = merged_lines(&input).expect("merge evidence");
        let parsed: Vec<Value> = merged
            .iter()
            .map(|line| serde_json::from_str(line).expect("evidence is JSON"))
            .collect();
        let platforms: Vec<String> = platforms.iter().map(|p| (*p).to_owned()).collect();
        let mut args = vec![
            input.to_string_lossy().into_owned(),
            out.to_string_lossy().into_owned(),
        ];
        args.extend(platforms.iter().cloned());
        print(&args).expect("print table");
        (
            render(&parsed, &platforms),
            std::fs::read_to_string(out).expect("read merged evidence"),
        )
    }

    #[test]
    fn keeps_passed_failed_skipped_and_missing_apart() {
        let (table, merged) = table_of(
            &[
                r#"{"platform":"linux_x86_64","row":"gcc","outcome":"passed"}"#,
                r#"{"platform":"linux_x86_64","row":"llvm","outcome":"failed"}"#,
                r#"{"platform":"macos_arm64","row":"llvm","outcome":"skipped"}"#,
            ],
            &["windows_x86_64"],
        );
        assert!(
            table.contains("| row | linux_x86_64 | macos_arm64 | windows_x86_64 |"),
            "{table}"
        );
        assert!(
            table.contains("| gcc | passed | no evidence | no evidence |"),
            "{table}"
        );
        assert!(
            table.contains("| llvm | failed | skipped | no evidence |"),
            "{table}"
        );
        assert_eq!(merged.lines().count(), 3);
    }

    #[test]
    fn never_upgrades_a_skipped_cell() {
        let (table, _) = table_of(
            &[
                r#"{"platform":"linux_x86_64","row":"gcc","outcome":"skipped"}"#,
                r#"{"platform":"linux_x86_64","row":"gcc","outcome":"skipped"}"#,
            ],
            &[],
        );
        assert!(table.contains("| gcc | skipped |"), "{table}");
    }

    #[test]
    fn reports_mismatch_over_passed() {
        let (table, _) = table_of(
            &[
                r#"{"platform":"linux_x86_64","row":"gcc","outcome":"passed"}"#,
                r#"{"platform":"linux_x86_64","row":"gcc","outcome":"mismatch"}"#,
            ],
            &[],
        );
        assert!(table.contains("| gcc | mismatch |"), "{table}");
    }

    #[test]
    fn ranks_blocked_below_mismatch_and_above_passed() {
        let (table, _) = table_of(
            &[
                r#"{"platform":"windows_arm64","row":"gcc","outcome":"blocked","tested_outcome":"skipped"}"#,
                r#"{"platform":"windows_arm64","row":"gcc","outcome":"passed"}"#,
                r#"{"platform":"windows_arm64","row":"llvm","outcome":"blocked","tested_outcome":"skipped"}"#,
                r#"{"platform":"windows_arm64","row":"llvm","outcome":"mismatch"}"#,
                r#"{"platform":"windows_arm64","row":"msvc","outcome":"blocked","tested_outcome":"passed"}"#,
            ],
            &[],
        );
        assert!(table.contains("| gcc | blocked |"), "{table}");
        assert!(table.contains("| llvm | mismatch |"), "{table}");
        assert!(table.contains("| msvc | blocked, passed |"), "{table}");
    }

    #[test]
    fn keeps_a_lone_mismatch() {
        let (table, _) = table_of(
            &[r#"{"platform":"windows_x86_64","row":"msvc","outcome":"mismatch"}"#],
            &[],
        );
        assert!(table.contains("| msvc | mismatch |"), "{table}");
    }

    #[test]
    fn keeps_the_command_and_lockfile_digest() {
        let (_, merged) = table_of(
            &[
                r#"{"platform":"windows_x86_64","row":"msvc","outcome":"passed","command":"bazel test //p:a","lockfile_sha256":"abc"}"#,
            ],
            &[],
        );
        assert!(
            merged.contains(r#""command":"bazel test //p:a""#),
            "{merged}"
        );
        assert!(merged.contains(r#""lockfile_sha256":"abc""#), "{merged}");
    }

    #[test]
    fn strips_carriage_returns_and_blank_lines() {
        let dir = dx_test_scratch::scratch("compiler-table-crlf");
        std::fs::write(dir.path().join("a.jsonl"), "{\"row\":\"gcc\"}\r\n\r\n\n")
            .expect("write evidence");
        assert_eq!(
            merged_lines(dir.path()).expect("merge"),
            vec![r#"{"row":"gcc"}"#.to_owned()]
        );
    }
}
