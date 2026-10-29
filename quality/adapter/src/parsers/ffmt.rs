use super::{
    as_text, check_output_size, code_name, known_native, unformatted_at, FileFinding, ParseError,
};
use crate::ColumnEncoding;

const TOOL: &str = "ffmt";

/// Parses `ffmt --check --no-cache --no-ignore --color never <files>`.
///
/// Measured on 0.4.5:
/// - exit 0 is clean and prints nothing;
/// - exit 1 prints the path of each unformatted file on stdout, one per line, as it was given;
/// - exit 2 is any failure to run: an invalid config, an unsupported extension, or no Fortran
///   file at all;
/// - a file with a syntax error is listed as unformatted, so it is a finding here.
///
/// Anything other than exit 0 with no output, or exit 1 with at least one path that this run
/// named, is an error and never a clean result.
pub fn parse_ffmt(
    stdout: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    check_output_size(TOOL, stdout)?;
    let out = as_text(TOOL, stdout)?;
    let mut dirty: Vec<&str> = Vec::new();
    for line in out.lines() {
        let line = line.trim_end_matches('\r');
        if line.is_empty() {
            continue;
        }
        let file = known_native(TOOL, files, line)?;
        if !dirty.contains(&file) {
            dirty.push(file);
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    const FILES: [&str; 3] = ["/w/a.f90", "/w/b.f90", "/w/sp ace/c.f90"];

    #[test]
    fn clean_run_has_no_findings() {
        assert!(parse_ffmt(b"", Some(0), &FILES).expect("ok").is_empty());
    }

    #[test]
    fn unformatted_files_are_reported_by_name() {
        let findings = parse_ffmt(b"/w/a.f90\n/w/sp ace/c.f90\n", Some(1), &FILES).expect("ok");
        assert_eq!(findings.len(), 2);
        assert_eq!(findings[0].file, "/w/a.f90");
        assert_eq!(findings[0].finding.start.line, 1);
        assert_eq!(findings[0].finding.message, "file is not formatted");
        assert_eq!(findings[1].file, "/w/sp ace/c.f90");
    }

    #[test]
    fn a_path_the_run_did_not_name_is_an_error() {
        assert!(parse_ffmt(b"/w/other.f90\n", Some(1), &FILES).is_err());
    }

    #[test]
    fn exit_codes_that_disagree_with_the_output_are_errors() {
        // Exit 2 is a config or usage failure, including "no Fortran files found".
        assert!(parse_ffmt(b"", Some(2), &FILES).is_err());
        assert!(parse_ffmt(b"/w/a.f90\n", Some(2), &FILES).is_err());
        // Exit 1 with nothing to name, and findings under exit 0.
        assert!(parse_ffmt(b"", Some(1), &FILES).is_err());
        assert!(parse_ffmt(b"/w/a.f90\n", Some(0), &FILES).is_err());
        assert!(parse_ffmt(b"", None, &FILES).is_err());
        assert!(parse_ffmt(&[0xff], Some(0), &FILES).is_err());
    }
}
