#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

use std::path::{Component, Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Relpath,
    Json,
    Offenses,
    Diff0,
    Warn,
    Diff,
    Listed,
    ListedRel,
    YamlReport,
}

/// The unified-diff header the real tool writes, which differs per tool.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Header {
    Git,
    Orig,
    OrigBanner,
    OrigTimestamped,
}

#[derive(Default)]
struct Flags {
    fix: bool,
    check: bool,
    diff: bool,
    reformat: bool,
    pos_check: bool,
}

fn extension_of(name: &str) -> &str {
    match name.rfind('.') {
        Some(idx) => &name[idx + 1..],
        None => name,
    }
}

fn mode_for(ext: &str) -> Mode {
    match ext {
        "cs" | "qml" => Mode::Relpath,
        "fs" => Mode::Json,
        "rb" => Mode::Offenses,
        "go" => Mode::Diff0,
        "css" | "less" | "scss" | "feature" | "sql" | "xml" => Mode::Warn,
        "jsonnet" | "pkl" | "mod" => Mode::Listed,
        "tf" => Mode::ListedRel,
        "yaml" => Mode::YamlReport,
        _ => Mode::Diff,
    }
}

/// The header shape each real producer writes.
///
/// `git diff` prefixes both sides, but the `diff -u` tools name the file itself and
/// `gofumpt` and `buf` print the `diff` command line above the header.
fn header_for(ext: &str) -> Header {
    match ext {
        "go" => Header::OrigBanner,
        "cue" | "html" | "sh" => Header::Orig,
        "proto" => Header::OrigTimestamped,
        _ => Header::Git,
    }
}

fn wants_fix(ext: &str, flags: &Flags) -> bool {
    match ext {
        "proto" => !flags.diff,
        "html" => flags.reformat && !flags.check,
        "fs" => !flags.pos_check,
        "cs" | "qml" => !flags.pos_check && !flags.check,
        "scala" => !flags.check,
        _ => flags.fix,
    }
}

fn parse(argv: &[String]) -> (Flags, Vec<String>) {
    let mut flags = Flags::default();
    let mut files = Vec::new();
    let mut skip_next = false;
    for arg in argv.iter().skip(1) {
        if skip_next {
            skip_next = false;
            continue;
        }
        match arg.as_str() {
            "--config" | "--config-path" => skip_next = true,
            "--check" => flags.check = true,
            "--diff" | "-d" => flags.diff = true,
            "--reformat" => {
                flags.fix = true;
                flags.reformat = true;
            }
            "-w" | "--write" | "--fix" | "-i" => flags.fix = true,
            "check" => flags.pos_check = true,
            _ if arg.starts_with('-') => {}
            _ => {
                if Path::new(arg).is_file() {
                    files.push(arg.clone());
                }
            }
        }
    }
    (flags, files)
}

fn relpath(cwd: &Path, file: &str) -> String {
    let abs = if Path::new(file).is_absolute() {
        PathBuf::from(file)
    } else {
        cwd.join(file)
    };
    let cwd_parts: Vec<Component<'_>> = cwd.components().collect();
    let abs_parts: Vec<Component<'_>> = abs.components().collect();
    let mut common = 0;
    while common < cwd_parts.len()
        && common < abs_parts.len()
        && cwd_parts[common] == abs_parts[common]
    {
        common += 1;
    }
    if common == 0 {
        return file.to_owned();
    }
    let mut rel = PathBuf::new();
    for _ in common..cwd_parts.len() {
        rel.push("..");
    }
    for part in &abs_parts[common..] {
        rel.push(part);
    }
    if rel.as_os_str().is_empty() {
        return ".".to_owned();
    }
    rel.to_string_lossy().replace('\\', "/")
}

fn warn_rel(cwd: &str, file: &str) -> String {
    let mut prefix = cwd.to_owned();
    prefix.push('/');
    if let Some(rest) = file.strip_prefix(&prefix) {
        return rest.to_owned();
    }
    match file.rsplit('/').next() {
        Some(base) => base.to_owned(),
        None => file.to_owned(),
    }
}

fn is_dirty(path: &str) -> bool {
    match std::fs::read(path) {
        Ok(bytes) => bytes.windows(MARKER.len()).any(|win| win == MARKER),
        Err(_) => false,
    }
}

/// Escapes a path for embedding in a JSON string literal.
fn json_escape(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for char in raw.chars() {
        match char {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            _ => out.push(char),
        }
    }
    out
}

const MARKER: &[u8] = b"BADFMT";
const FIXED: &[u8] = b"fixed";

/// The 1-based line and column of the dirty marker, the way RuboCop reports an offense.
fn marker_position(bytes: &[u8]) -> Option<(u64, u64)> {
    let at = bytes.windows(MARKER.len()).position(|win| win == MARKER)?;
    let before = &bytes[..at];
    let line = 1 + before.iter().filter(|byte| **byte == b'\n').count() as u64;
    let column = match before.iter().rposition(|byte| *byte == b'\n') {
        Some(newline) => String::from_utf8_lossy(&before[newline + 1..])
            .chars()
            .count() as u64,
        None => before.len() as u64,
    };
    Some((line, column + 1))
}

/// The RuboCop JSON report that `standardrb --format json` writes.
///
/// RuboCop reports each path as it was handed on the command line, so the fake keeps the
/// argument instead of shortening it.
fn emit_offenses(files: &[String]) -> i32 {
    let mut entries = Vec::new();
    let mut offenses = 0;
    for file in files {
        let offense = match std::fs::read(file)
            .ok()
            .as_deref()
            .and_then(marker_position)
        {
            Some((line, column)) => {
                offenses += 1;
                let last = column + MARKER.len() as u64;
                format!(
                    concat!(
                        r#"{{"severity": "convention", "message": "Prefer double-quoted strings.", "#,
                        r#""cop_name": "Style/StringLiterals", "correctable": true, "status": "uncorrected", "#,
                        r#""location": {{"start_line": {line}, "start_column": {column}, "last_line": {line}, "#,
                        r#""last_column": {last}, "length": {length}, "line": {line}, "column": {column}}}}}"#
                    ),
                    line = line,
                    column = column,
                    last = last,
                    length = MARKER.len()
                )
            }
            None => String::new(),
        };
        entries.push(format!(
            "{{\"path\": \"{}\", \"offenses\": [{offense}]}}",
            json_escape(file)
        ));
    }
    let count = files.len();
    println!(
        concat!(
            r#"{{"metadata": {{"rubocop_version": "1.75.5", "ruby_engine": "ruby", "#,
            r#""ruby_version": "3.3.6", "ruby_patchlevel": "100"}}, "files": [{}], "#,
            r#""summary": {{"offense_count": {}, "target_file_count": {}, "inspected_file_count": {}}}}}"#
        ),
        entries.join(", "),
        offenses,
        count,
        count
    );
    if offenses > 0 {
        1
    } else {
        0
    }
}

fn rewrite_fixed(path: &str) {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(_) => return,
    };
    let mut out = Vec::with_capacity(bytes.len());
    let mut idx = 0;
    while idx < bytes.len() {
        if bytes[idx..].starts_with(MARKER) {
            out.extend_from_slice(FIXED);
            idx += MARKER.len();
        } else {
            out.push(bytes[idx]);
            idx += 1;
        }
    }
    let tmp = format!("{path}.dxtmp");
    if std::fs::write(&tmp, &out).is_ok() {
        let _ = std::fs::rename(&tmp, path);
    }
}

/// The header lines a tool prints above the hunk.
fn render_header(file: &str, header: Header) -> String {
    match header {
        Header::Git => format!("--- a/{file}\n+++ b/{file}"),
        Header::Orig => format!("--- {file}.orig\n+++ {file}"),
        Header::OrigBanner => format!("diff {file}.orig {file}\n--- {file}.orig\n+++ {file}"),
        Header::OrigTimestamped => format!(
            "diff -u {file}.orig {file}\n--- {file}.orig\t2026-01-02 03:04:05.0 +0000\n+++ {file}\t2026-01-02 03:04:06.0 +0000"
        ),
    }
}

fn emit_diff(file: &str, header: Header) {
    println!(
        "{}\n@@ -1 +1 @@\n-BADFMT\n+fixed",
        render_header(file, header)
    );
}

pub fn run(argv: &[String]) -> i32 {
    let cwd = match std::env::current_dir() {
        Ok(cwd) => cwd,
        Err(_) => return 2,
    };
    let (flags, files) = parse(argv);
    if files.is_empty() {
        return 0;
    }
    let ext = extension_of(&files[0]);
    if wants_fix(ext, &flags) {
        for file in &files {
            if is_dirty(file) {
                rewrite_fixed(file);
            }
        }
        return 0;
    }
    match mode_for(ext) {
        Mode::Relpath => {
            let mut dirty = false;
            for file in &files {
                if is_dirty(file) {
                    println!("{}", relpath(&cwd, file));
                    dirty = true;
                }
            }
            if dirty {
                1
            } else {
                0
            }
        }
        Mode::Json => {
            let mut entries = Vec::new();
            let mut dirty = false;
            for file in &files {
                let rel = relpath(&cwd, file);
                if is_dirty(file) {
                    entries.push(format!(
                        "{{\"path\": \"{rel}\", \"status\": \"needs-formatting\"}}"
                    ));
                    dirty = true;
                } else {
                    entries.push(format!(
                        "{{\"path\": \"{rel}\", \"status\": \"unchanged\"}}"
                    ));
                }
            }
            println!("{{\"files\": [{}]}}", entries.join(", "));
            if dirty {
                99
            } else {
                0
            }
        }
        Mode::Offenses => emit_offenses(&files),
        Mode::Warn => {
            let cwd_text = cwd.to_string_lossy().into_owned();
            let mut dirty = false;
            for file in &files {
                if is_dirty(file) {
                    eprintln!("[warn] {}", warn_rel(&cwd_text, file));
                    dirty = true;
                }
            }
            if dirty {
                1
            } else {
                0
            }
        }
        Mode::Diff => {
            let mut dirty = false;
            for file in &files {
                if is_dirty(file) {
                    emit_diff(file, header_for(ext));
                    dirty = true;
                }
            }
            if dirty {
                1
            } else {
                0
            }
        }
        Mode::Diff0 => {
            for file in &files {
                if is_dirty(file) {
                    emit_diff(file, header_for(ext));
                }
            }
            0
        }
        Mode::Listed => {
            let mut dirty = false;
            for file in &files {
                if is_dirty(file) {
                    println!("{file}");
                    dirty = true;
                }
            }
            if dirty {
                1
            } else {
                0
            }
        }
        Mode::ListedRel => {
            let mut dirty = false;
            for file in &files {
                if is_dirty(file) {
                    println!("{}", relpath(&cwd, file));
                    dirty = true;
                }
            }
            if dirty {
                1
            } else {
                0
            }
        }
        Mode::YamlReport => {
            let mut dirty = false;
            for file in &files {
                if is_dirty(file) {
                    dirty = true;
                }
            }
            if dirty {
                eprintln!("{}", YAMLFMT_HEADER);
                for file in &files {
                    if is_dirty(file) {
                        eprintln!("{file}");
                    }
                }
                1
            } else {
                0
            }
        }
    }
}

/// The report `yamlfmt -lint` writes to stderr above the paths it would rewrite.
const YAMLFMT_HEADER: &str = "The following files had formatting differences:\n";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_diff_reporting_extension_keeps_its_own_header() {
        for (ext, want) in [
            ("go", Header::OrigBanner),
            ("cue", Header::Orig),
            ("html", Header::Orig),
            ("sh", Header::Orig),
            ("proto", Header::OrigTimestamped),
            ("c", Header::Git),
            ("cs", Header::Git),
            ("scala", Header::Git),
        ] {
            assert_eq!(header_for(ext), want, "{ext}");
        }
    }

    /// The tools that name the files they would rewrite print no diff header at all.
    #[test]
    fn the_listing_tools_never_print_a_diff_header() {
        for ext in ["jsonnet", "pkl", "mod", "tf"] {
            assert!(
                matches!(mode_for(ext), Mode::Listed | Mode::ListedRel),
                "{ext}"
            );
        }
    }

    /// `terraform fmt -check` names each file from its working directory, not as handed.
    #[test]
    fn terraform_names_its_files_relative_to_the_working_directory() {
        assert!(matches!(mode_for("tf"), Mode::ListedRel));
        assert_eq!(
            relpath(Path::new("/tmp/dx"), "/tmp/dx/matrix/x.tf"),
            "matrix/x.tf"
        );
        assert_eq!(relpath(Path::new("/tmp/dx/sub"), "/tmp/dx/x.tf"), "../x.tf");
    }

    /// `yamlfmt -lint` reports to stderr, so its shape only survives on stderr.
    #[test]
    fn yamlfmt_reports_on_stderr_rather_than_stdout() {
        assert!(matches!(mode_for("yaml"), Mode::YamlReport));
        assert!(YAMLFMT_HEADER.starts_with("The following files had"));
    }

    #[test]
    fn ruby_reports_rubocop_offenses_rather_than_a_diff() {
        assert_eq!(mode_for("rb"), Mode::Offenses);
    }

    #[test]
    fn the_offense_column_counts_from_the_start_of_its_line() {
        assert_eq!(marker_position(b"puts BADFMT\n"), Some((1, 6)));
        assert_eq!(marker_position(b"puts ok\nx = BADFMT\n"), Some((2, 5)));
        assert_eq!(marker_position(b"puts ok\n"), None);
    }

    /// A reported path goes inside a JSON string literal, so a backslash in it must be escaped
    /// or the report stops being JSON.
    #[test]
    fn a_reported_path_is_escaped_for_the_json_it_sits_in() {
        assert_eq!(json_escape(r#"C:\a\b.py"#), r#"C:\\a\\b.py"#);
        assert_eq!(json_escape("a\"b.py"), "a\\\"b.py");
        assert_eq!(json_escape("a\nb"), "a\\nb");
        assert_eq!(json_escape("plain/a.py"), "plain/a.py");
    }

    /// `Mode::Relpath` and `Mode::ListedRel` print paths the runner matches against its own
    /// absolute names, so they must spell separators the way the checked-in paths do.
    #[test]
    fn a_relative_report_never_names_a_backslash() {
        let rel = relpath(Path::new("/tmp/dx"), "/tmp/dx/sub\\x.tf");
        assert!(!rel.contains('\\'), "{rel}");
    }

    #[test]
    fn each_header_names_the_file_the_way_its_tool_does() {
        assert_eq!(
            render_header("x.cue", Header::Orig),
            "--- x.cue.orig\n+++ x.cue"
        );
        assert_eq!(render_header("x.tf", Header::Git), "--- a/x.tf\n+++ b/x.tf");
        assert_eq!(
            render_header("x.go", Header::OrigBanner),
            "diff x.go.orig x.go\n--- x.go.orig\n+++ x.go"
        );
        assert_eq!(
            render_header("x.proto", Header::OrigTimestamped),
            "diff -u x.proto.orig x.proto\n--- x.proto.orig\t2026-01-02 03:04:05.0 +0000\n+++ x.proto\t2026-01-02 03:04:06.0 +0000"
        );
    }
}
