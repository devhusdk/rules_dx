use super::{as_text, code_name, unformatted_at, FileFinding, ParseError};
use crate::ColumnEncoding;

const HOME_WARNING: &str = "Use of uninitialized value $homeDir in concatenation";
const TOOL: &str = "latexindent";

const FATAL: &str = "FATAL:";

/// `latexindent -k FILE` prints the re-indented document on stdout and exits 0 when the document
/// body is unchanged and 1 when it would change. Nothing is written in check mode. A fatal
/// condition (a missing file, say) prints `FATAL: ...` and exits 3. The check runs on one file
/// per process, so the single checked file is the one a dirty exit names.
///
/// Exit 0 is only clean with an empty stderr, so a crash that still ends at exit 0 is never read
/// as clean, and exit 1 needs a document on stdout.
pub fn parse_latexindent(
    stdout: &[u8],
    stderr: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    let [file] = files else {
        return Err(ParseError::shape(
            TOOL,
            format!("expected one checked file, got {}", files.len()),
        ));
    };
    let document = as_text(TOOL, stdout)?;
    let diagnostics = as_text(TOOL, stderr)?;
    if let Some(line) = [document, diagnostics]
        .iter()
        .flat_map(|text| text.lines())
        .map(str::trim)
        .find(|line| line.starts_with(FATAL))
    {
        return Err(ParseError::Incomplete {
            tool: TOOL,
            detail: line.to_owned(),
        });
    }
    // The hermetic runner environment has no HOME, so latexindent warns once while it looks for
    // a user settings file. That warning is expected and carries no result.
    let unexpected = diagnostics
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty() && !line.starts_with(HOME_WARNING));
    if let Some(line) = unexpected {
        return Err(ParseError::shape(
            TOOL,
            format!("unexpected stderr: {line}"),
        ));
    }
    match code {
        Some(0) => Ok(Vec::new()),
        Some(1) if document.trim().is_empty() => Err(ParseError::shape(
            TOOL,
            "exit 1 without a re-indented document",
        )),
        Some(1) => Ok(vec![unformatted_at(
            TOOL,
            file,
            1,
            1,
            ColumnEncoding::Scalars,
        )]),
        _ => Err(ParseError::shape(
            TOOL,
            format!("unexpected exit {}", code_name(code)),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILES: [&str; 1] = ["/s/doc/a.tex"];
    const DOC: &str = "\\begin{a}\n\t\\item x\n\\end{a}\n";

    fn parse(stdout: &str, stderr: &str, code: i32) -> Result<Vec<FileFinding>, ParseError> {
        parse_latexindent(stdout.as_bytes(), stderr.as_bytes(), Some(code), &FILES)
    }

    #[test]
    fn latexindent_exit_zero_is_clean_even_for_an_empty_document() {
        assert!(parse(DOC, "", 0).expect("parsed").is_empty());
        assert!(parse("", "", 0).expect("parsed").is_empty());
    }

    #[test]
    fn latexindent_exit_one_flags_the_checked_file() {
        let findings = parse(DOC, "", 1).expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].file, "/s/doc/a.tex");
        assert_eq!(findings[0].finding.message, "file is not formatted");
    }

    #[test]
    fn latexindent_exit_one_without_a_document_is_malformed() {
        assert!(parse("", "", 1).is_err());
        assert!(parse(" \n", "", 1).is_err());
    }

    #[test]
    fn latexindent_fatal_is_an_incomplete_run_at_any_exit() {
        let fatal = "FATAL: I couldn't find a.tex, are you sure it exists?\n  Exiting\n";
        for code in [0, 1, 3] {
            let error = parse(fatal, "", code).expect_err("fatal");
            assert!(matches!(error, ParseError::Incomplete { .. }), "{error}");
        }
        assert!(matches!(
            parse("", fatal, 3).expect_err("fatal"),
            ParseError::Incomplete { .. }
        ));
    }

    #[test]
    fn latexindent_unknown_exit_stderr_and_signals_are_never_clean() {
        assert!(parse(DOC, "", 2).is_err());
        assert!(parse("", "", 3).is_err());
        assert!(parse(DOC, "Can't locate Foo.pm\n", 0).is_err());
        assert!(parse(DOC, "warning\n", 1).is_err());
        assert!(parse_latexindent(b"", b"", None, &FILES).is_err());
    }

    #[test]
    fn latexindent_checks_exactly_one_file() {
        assert!(parse_latexindent(b"", b"", Some(0), &[]).is_err());
        assert!(parse_latexindent(b"", b"", Some(0), &["a.tex", "b.tex"]).is_err());
    }
}
