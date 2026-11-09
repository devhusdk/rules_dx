use super::{
    FileFinding, ParseError, as_text, check_output_size, code_name, known_native, unformatted_at,
};
use crate::ColumnEncoding;

const TOOL: &str = "docstrfmt";

/// Parses `docstrfmt --check --ignore-cache --pyproject-config <config> <files>`.
///
/// Measured on 2.2.1:
/// - everything is written to stderr; stdout is expected to be empty;
/// - `File '<path>' could be reformatted.` names each unformatted file, as an absolute path;
/// - `<n> out of <m> files could be reformatted.` or `<m> files was checked.` closes the run,
///   followed by `Done! 🎉`;
/// - exit 0 is clean and 1 is at least one unformatted file or an error;
/// - a path that does not exist is skipped without a message, so the closing count must equal the
///   number of files passed;
/// - an unreadable file or invalid config prints other lines and `Done, but ...`.
///
/// Anything outside that shape is an error and never a clean run.
pub fn parse_docstrfmt(
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
    let mut total: Option<usize> = None;
    let mut reported: Option<usize> = None;
    let mut done = false;
    for line in out.lines().chain(err.lines()) {
        let line = line.trim_end();
        if line.trim().is_empty() {
            continue;
        }
        if line == "Done! \u{1f389}" {
            done = true;
            continue;
        }
        if let Some(path) = line
            .strip_prefix("File '")
            .and_then(|rest| rest.strip_suffix("' could be reformatted."))
        {
            let file = known_native(TOOL, files, path)?;
            if !dirty.contains(&file) {
                dirty.push(file);
            }
            continue;
        }
        if let Some(summary) = line.strip_suffix(" could be reformatted.")
            && let Some((count, all)) = summary.split_once(" out of ")
            && let (Some(count), Some(all)) = (leading_number(count), leading_number(all))
        {
            reported = Some(count);
            total = Some(all);
            continue;
        }
        if line.ends_with(" checked.")
            && let Some(all) = leading_number(line)
        {
            total = Some(all);
            continue;
        }
        return Err(ParseError::shape(
            TOOL,
            format!("unrecognized output line: {line}"),
        ));
    }

    if !done || total != Some(files.len()) {
        return Err(ParseError::shape(
            TOOL,
            format!(
                "closing count {total:?} does not match {} files, or the run did not finish",
                files.len()
            ),
        ));
    }
    if reported.is_some_and(|count| count != dirty.len()) {
        return Err(ParseError::shape(
            TOOL,
            "summary count does not match the files named".to_owned(),
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

fn leading_number(text: &str) -> Option<usize> {
    text.split_whitespace().next()?.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILES: [&str; 3] = ["/w/a.rst", "/w/b.rst", "/w/sp ace/c.rst"];
    const CLEAN: &str = "3 files was checked.\nDone! \u{1f389}\n";
    const DIRTY: &str = "File '/w/a.rst' could be reformatted.\nFile '/w/sp ace/c.rst' could be reformatted.\n2 out of 3 files could be reformatted.\nDone! \u{1f389}\n";

    #[test]
    fn clean_run_has_no_findings() {
        assert!(
            parse_docstrfmt(b"", CLEAN.as_bytes(), Some(0), &FILES)
                .expect("ok")
                .is_empty()
        );
    }

    #[test]
    fn names_each_unformatted_file() {
        let findings = parse_docstrfmt(b"", DIRTY.as_bytes(), Some(1), &FILES).expect("ok");
        assert_eq!(findings.len(), 2);
        assert_eq!(findings[0].file, "/w/a.rst");
        assert_eq!(findings[1].file, "/w/sp ace/c.rst");
    }

    #[test]
    fn anything_else_is_an_error() {
        assert!(parse_docstrfmt(b"", b"", Some(0), &FILES).is_err());
        assert!(parse_docstrfmt(b"", b"", Some(1), &FILES).is_err());
        assert!(parse_docstrfmt(b"", CLEAN.as_bytes(), Some(1), &FILES).is_err());
        assert!(parse_docstrfmt(b"", CLEAN.as_bytes(), None, &FILES).is_err());
        assert!(parse_docstrfmt(b"", DIRTY.as_bytes(), Some(0), &FILES).is_err());
        assert!(parse_docstrfmt(CLEAN.as_bytes(), b"", Some(0), &FILES[..2]).is_err());
        let none = "0 files was checked.\nDone! \u{1f389}\n";
        assert!(parse_docstrfmt(b"", none.as_bytes(), Some(0), &FILES).is_err());
        let unknown = "File '/w/other.rst' could be reformatted.\n1 out of 3 files could be reformatted.\nDone! \u{1f389}\n";
        assert!(parse_docstrfmt(b"", unknown.as_bytes(), Some(1), &FILES).is_err());
        let failed = "Error: Invalid value for '-p'\n";
        assert!(parse_docstrfmt(b"", failed.as_bytes(), Some(2), &FILES).is_err());
        let crashed = "SyntaxError: invalid syntax:\n3 files was checked.\nDone, but 1 error occurred\n";
        assert!(parse_docstrfmt(b"", crashed.as_bytes(), Some(1), &FILES).is_err());
        let miscount = "File '/w/a.rst' could be reformatted.\n2 out of 3 files could be reformatted.\nDone! \u{1f389}\n";
        assert!(parse_docstrfmt(b"", miscount.as_bytes(), Some(1), &FILES).is_err());
        assert!(parse_docstrfmt(&[0xff], b"", Some(0), &FILES).is_err());
    }
}
