//! `diff -u OLD NEW`: the unified diff of two files, for tools that exec `diff` from PATH.

#![deny(clippy::indexing_slicing, clippy::string_slice)]

use std::ffi::OsString;
use std::io::Write;
use std::path::Path;

pub const USAGE: &str = "usage: diff -u OLD NEW";
pub const SAME: i32 = 0;
pub const DIFFERENT: i32 = 1;
pub const TROUBLE: i32 = 2;

const CONTEXT: usize = 3;

/// The unified diff of `old` and `new` under `--- old_name` and `+++ new_name`; empty when equal.
/// Inputs that are not UTF-8 are reported as differing binary files.
pub fn unified(old_name: &str, new_name: &str, old: &[u8], new: &[u8]) -> Vec<u8> {
    if old == new {
        return Vec::new();
    }
    let (Ok(old), Ok(new)) = (std::str::from_utf8(old), std::str::from_utf8(new)) else {
        return format!("Binary files {old_name} and {new_name} differ\n").into_bytes();
    };
    let diff = similar::TextDiff::configure()
        .algorithm(similar::Algorithm::Myers)
        .diff_lines(old, new);
    diff.unified_diff()
        .context_radius(CONTEXT)
        .header(old_name, new_name)
        .to_string()
        .into_bytes()
}

/// Runs `diff -u OLD NEW`, writing the diff to `out`. Returns the exit code.
pub fn run(args: &[OsString], out: &mut dyn Write) -> Result<i32, String> {
    let [flag, old, new] = args else {
        return Err(USAGE.to_owned());
    };
    if flag != "-u" {
        return Err(USAGE.to_owned());
    }
    let read = |path: &OsString| {
        std::fs::read(Path::new(path))
            .map_err(|error| format!("{}: {error}", path.to_string_lossy()))
    };
    let text = unified(
        &old.to_string_lossy(),
        &new.to_string_lossy(),
        &read(old)?,
        &read(new)?,
    );
    if text.is_empty() {
        return Ok(SAME);
    }
    out.write_all(&text)
        .map_err(|error| format!("stdout: {error}"))?;
    Ok(DIFFERENT)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(old: &str, new: &str) -> String {
        String::from_utf8(unified("a", "b", old.as_bytes(), new.as_bytes())).expect("utf-8")
    }

    #[test]
    fn equal_inputs_have_no_diff() {
        assert_eq!(text("one\ntwo\n", "one\ntwo\n"), "");
    }

    #[test]
    fn changed_line_is_one_hunk_with_context() {
        assert_eq!(
            text(
                "1\n2\n3\n4\n5\n6\n7\n8\n9\n",
                "1\n2\n3\n4\nfive\n6\n7\n8\n9\n"
            ),
            "--- a\n+++ b\n@@ -2,7 +2,7 @@\n 2\n 3\n 4\n-5\n+five\n 6\n 7\n 8\n"
        );
    }

    #[test]
    fn distant_changes_are_separate_hunks() {
        let old: String = (1..=20).map(|n| format!("{n}\n")).collect();
        let new = old
            .replace("2\n3\n", "2\nthree\n")
            .replace("18\n", "eighteen\n");
        let diff = text(&old, &new);
        assert_eq!(diff.matches("\n@@ ").count(), 2, "{diff}");
    }

    #[test]
    fn missing_final_newline_is_marked() {
        assert_eq!(
            text("one\n", "one"),
            "--- a\n+++ b\n@@ -1 +1 @@\n-one\n+one\n\\ No newline at end of file\n"
        );
    }

    #[test]
    fn bytes_that_are_not_utf8_differ_as_binary() {
        assert_eq!(
            unified("a", "b", b"\xff\n", b"\xfe\n"),
            b"Binary files a and b differ\n"
        );
    }

    fn args(items: &[&std::path::Path]) -> Vec<OsString> {
        std::iter::once(OsString::from("-u"))
            .chain(items.iter().map(|path| path.as_os_str().to_owned()))
            .collect()
    }

    fn scratch() -> std::path::PathBuf {
        std::path::PathBuf::from(std::env::var_os("TEST_TMPDIR").expect("TEST_TMPDIR"))
    }

    #[test]
    fn run_exits_zero_and_prints_nothing_for_equal_files() {
        let (old, new) = (scratch().join("same_a"), scratch().join("same_b"));
        std::fs::write(&old, "x\n").expect("write");
        std::fs::write(&new, "x\n").expect("write");
        let mut out = Vec::new();
        assert_eq!(run(&args(&[&old, &new]), &mut out), Ok(SAME));
        assert!(out.is_empty());
    }

    #[test]
    fn run_exits_one_and_names_both_files_when_they_differ() {
        let (old, new) = (scratch().join("differ_a"), scratch().join("differ_b"));
        std::fs::write(&old, "x\n").expect("write");
        std::fs::write(&new, "y\n").expect("write");
        let mut out = Vec::new();
        assert_eq!(run(&args(&[&old, &new]), &mut out), Ok(DIFFERENT));
        let want = format!(
            "--- {}\n+++ {}\n@@ -1 +1 @@\n-x\n+y\n",
            old.display(),
            new.display()
        );
        assert_eq!(String::from_utf8(out).expect("utf-8"), want);
    }

    #[test]
    fn run_refuses_other_argument_shapes() {
        let mut out = Vec::new();
        let none: Vec<OsString> = Vec::new();
        assert_eq!(run(&none, &mut out), Err(USAGE.to_owned()));
        let context = ["-c", "a", "b"].map(OsString::from);
        assert_eq!(run(&context, &mut out), Err(USAGE.to_owned()));
    }

    #[test]
    fn run_reports_a_missing_file() {
        let missing = scratch().join("missing");
        let mut out = Vec::new();
        let error = run(&args(&[&missing, &missing]), &mut out).expect_err("missing file");
        assert!(error.contains("missing"), "{error}");
    }
}
