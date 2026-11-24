use super::{
    FileFinding, ParseError, as_text, check_output_size, code_name, known_native, unformatted_at,
};
use crate::ColumnEncoding;

const TOOL: &str = "mbake";

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
        let actual = parse_mbake(CLEAN.as_bytes(), b"", Some(0), &FILES).expect("ok");
        assert!(actual.is_empty(), "{actual:?}");
        let actual = parse_mbake(b"", b"", Some(0), &FILES).expect("ok");
        assert!(actual.is_empty(), "{actual:?}");
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
        let actual = parse_mbake(out.as_bytes(), b"", Some(0), &FILES).expect("ok");
        assert!(actual.is_empty(), "{actual:?}");
    }

    #[test]
    fn anything_else_is_an_error() {
        let missing = format!("{DIRTY}Error: File not found: /w/x.mk\n");
        assert!(parse_mbake(missing.as_bytes(), b"", Some(2), &FILES).is_err());
        assert!(parse_mbake(missing.as_bytes(), b"", Some(1), &FILES).is_err());
        let config =
            b"Error: Configuration file not found at /w/.bake.toml.\nHint: Run bake init\n";
        assert!(parse_mbake(config, b"", Some(1), &FILES).is_err());
        assert!(parse_mbake(b"", config, Some(0), &FILES).is_err());
        assert!(parse_mbake(b"", b"", Some(1), &FILES).is_err());
        assert!(parse_mbake(DIRTY.as_bytes(), b"", Some(0), &FILES).is_err());
        assert!(parse_mbake(DIRTY.as_bytes(), b"", Some(2), &FILES).is_err());
        assert!(parse_mbake(b"", b"", None, &FILES).is_err());
        assert!(parse_mbake(b"Would reformat: /w/other.mk\n", b"", Some(1), &FILES).is_err());
        assert!(
            parse_mbake(
                b"Traceback (most recent call last):\n",
                b"",
                Some(1),
                &FILES
            )
            .is_err()
        );
        assert!(parse_mbake(b"/w/other.mk:1: Warning: x\n", b"", Some(0), &FILES).is_err());
        assert!(parse_mbake(&[0xff], b"", Some(0), &FILES).is_err());
    }
}
