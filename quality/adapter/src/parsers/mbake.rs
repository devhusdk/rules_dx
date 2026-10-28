use super::{
    as_text, check_output_size, code_name, known_native, unformatted_at, FileFinding, ParseError,
};
use crate::ColumnEncoding;

const TOOL: &str = "mbake";

/// Parses `mbake format --check --config <config> <files>`.
///
/// Measured on 1.4.6, with `NO_COLOR=1`, `TERM=dumb` and `COLUMNS=100000` (rich otherwise wraps
/// long lines at 80 columns):
/// - everything is written to stdout; stderr is expected to be empty;
/// - `Would reformat: <path>` names each unformatted file, in argument order;
/// - `<path>:<line>: Warning: ...` lines are rule advisories and never findings;
/// - `Summary: Processed <n> files` and a `✓ ...` status line close the run;
/// - exit 0 is clean, 1 is at least one unformatted file, 2 is a missing input file, and a
///   missing or unreadable config is exit 1 with an `Error:` line.
///
/// mbake never rejects a Makefile as a syntax error (that needs `--validate`, which runs the host
/// `make` and is never passed). The exit code and the `Would reformat:` file names carry the
/// result, not the wording of the prose. Anything outside that shape, including any `Error:`
/// line, is an error and never a clean run. `--diff` is not used: it exits 0 even for files that
/// would change.
pub fn parse_mbake(
    stdout: &[u8],
    stderr: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    check_output_size(TOOL, stdout)?;
    check_output_size(TOOL, stderr)?;
    let out = as_text(TOOL, stdout)?;
    let err = as_text(TOOL, stderr)?;

    let mut dirty: Vec<&str> = Vec::new();
    for line in out.lines().chain(err.lines()) {
        let line = line.trim_end();
        if line.trim().is_empty()
            || line.starts_with("Summary: Processed ")
            || line.starts_with('\u{2713}')
        {
            continue;
        }
        if let Some(path) = line.strip_prefix("Would reformat: ") {
            let file = known_native(TOOL, files, path)?;
            if !dirty.contains(&file) {
                dirty.push(file);
            }
            continue;
        }
        if is_advisory(files, line) {
            continue;
        }
        return Err(ParseError::shape(
            TOOL,
            format!("unrecognized output line: {line}"),
        ));
    }

    let want = if dirty.is_empty() { Some(0) } else { Some(1) };
    if code != want {
        return Err(ParseError::shape(
            TOOL,
            format!(
                "unexpected exit {} with {} unformatted files",
                code_name(code),
                dirty.len()
            ),
        ));
    }
    Ok(dirty
        .into_iter()
        .map(|file| unformatted_at(TOOL, file, 1, 1, ColumnEncoding::Scalars))
        .collect())
}

/// `<known path>:<line>: Warning: <text>`; the path may itself contain colons.
fn is_advisory(files: &[&str], line: &str) -> bool {
    files.iter().any(|file| {
        line.strip_prefix(file)
            .and_then(|rest| rest.strip_prefix(':'))
            .and_then(|rest| rest.split_once(": Warning: "))
            .is_some_and(|(number, _)| {
                !number.is_empty() && number.bytes().all(|b| b.is_ascii_digit())
            })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILES: [&str; 3] = ["/w/Makefile", "/w/b.mk", "/w/sp ace/c.mk"];
    const CLEAN: &str = "\nSummary: Processed 3 files\n\u{2713} All files are properly formatted\n";
    const DIRTY: &str = "/w/Makefile:1: Warning: Consider adding .PHONY declaration for targets: foo\nWould reformat: /w/Makefile\nWould reformat: /w/sp ace/c.mk\n\nSummary: Processed 3 files\n\u{2713} Files would be reformatted\n";

    #[test]
    fn clean_run_has_no_findings() {
        assert!(parse_mbake(CLEAN.as_bytes(), b"", Some(0), &FILES)
            .expect("ok")
            .is_empty());
        assert!(parse_mbake(b"", b"", Some(0), &FILES)
            .expect("ok")
            .is_empty());
    }

    #[test]
    fn unformatted_files_are_reported_by_name_and_advisories_are_skipped() {
        let findings = parse_mbake(DIRTY.as_bytes(), b"", Some(1), &FILES).expect("ok");
        assert_eq!(findings.len(), 2);
        assert_eq!(findings[0].file, "/w/Makefile");
        assert_eq!(findings[0].finding.start.line, 1);
        assert_eq!(findings[0].finding.message, "file is not formatted");
        assert_eq!(findings[1].file, "/w/sp ace/c.mk");
    }

    #[test]
    fn advisories_alone_are_clean() {
        let out = "/w/b.mk:3: Warning: Missing final newline\n";
        assert!(parse_mbake(out.as_bytes(), b"", Some(0), &FILES)
            .expect("ok")
            .is_empty());
    }

    #[test]
    fn anything_else_is_an_error() {
        // A missing input file exits 2 and still lists the others.
        let missing = format!("{DIRTY}Error: File not found: /w/x.mk\n");
        assert!(parse_mbake(missing.as_bytes(), b"", Some(2), &FILES).is_err());
        assert!(parse_mbake(missing.as_bytes(), b"", Some(1), &FILES).is_err());
        // A missing config is exit 1 with an Error line and no file.
        let config =
            b"Error: Configuration file not found at /w/.bake.toml.\nHint: Run bake init\n";
        assert!(parse_mbake(config, b"", Some(1), &FILES).is_err());
        assert!(parse_mbake(b"", config, Some(0), &FILES).is_err());
        // Exit codes that disagree with the output.
        assert!(parse_mbake(b"", b"", Some(1), &FILES).is_err());
        assert!(parse_mbake(DIRTY.as_bytes(), b"", Some(0), &FILES).is_err());
        assert!(parse_mbake(DIRTY.as_bytes(), b"", Some(2), &FILES).is_err());
        assert!(parse_mbake(b"", b"", None, &FILES).is_err());
        // A file the run did not name, and foreign output.
        assert!(parse_mbake(b"Would reformat: /w/other.mk\n", b"", Some(1), &FILES).is_err());
        assert!(parse_mbake(
            b"Traceback (most recent call last):\n",
            b"",
            Some(1),
            &FILES
        )
        .is_err());
        assert!(parse_mbake(b"/w/other.mk:1: Warning: x\n", b"", Some(0), &FILES).is_err());
        assert!(parse_mbake(&[0xff], b"", Some(0), &FILES).is_err());
    }
}
