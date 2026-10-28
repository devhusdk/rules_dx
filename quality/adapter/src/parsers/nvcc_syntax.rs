use super::clang_syntax::{diagnostic, include_entry};
use super::{finding, lines, require_findings, FileFinding, ParseError};
use crate::ToolSeverity;
use crate::{ColumnEncoding, TextPosition};

const TOOL: &str = "nvcc_syntax";

/// One EDG (cudafe++/cicc front end) line: `path(LINE): error|warning|remark[ #NNN-D]: message`.
fn edg_diagnostic(line: &str) -> Option<(&str, u64, Option<ToolSeverity>, String, &str)> {
    let mut from = 0;
    while let Some(offset) = line[from..].find("): ") {
        let end = from + offset;
        from = end + 1;
        let Some((file, number)) = line[..end].rsplit_once('(') else {
            continue;
        };
        let Ok(line_no) = number.parse::<u64>() else {
            continue;
        };
        let rest = &line[end + 3..];
        let (word, severity) = [
            ("catastrophic error", Some(ToolSeverity::Error)),
            ("error", Some(ToolSeverity::Error)),
            ("warning", Some(ToolSeverity::Warning)),
            ("remark", None),
            ("note", None),
        ]
        .into_iter()
        .find(|(word, _)| {
            rest.strip_prefix(word)
                .is_some_and(|tail| tail.starts_with(':') || tail.starts_with(" #"))
        })?;
        let tail = &rest[word.len()..];
        let (rule, message) = match tail.strip_prefix(" #") {
            Some(tail) => {
                let (rule, message) = tail.split_once(": ")?;
                (rule.to_owned(), message)
            }
            None => (String::new(), tail.strip_prefix(": ")?),
        };
        return Some((file, line_no, severity, rule, message));
    }
    None
}

/// Whether the line is nvcc housekeeping that never carries a finding.
fn is_noise(line: &str) -> bool {
    line.starts_with("Remark: The warnings can be suppressed")
        || (line.contains(" detected in the compilation of ")
            && (line.ends_with('.') || line.ends_with('"') || line.ends_with('\''))
            && line.split_whitespace().nth(1).is_some_and(|word| {
                word == "error" || word == "errors" || word == "warning" || word == "warnings"
            }))
}

/// Parses `nvcc -cuda` (EDG front end) text diagnostics, plus the clang-style lines the
/// host preprocessor prints (a missing include, for example). Identical findings are
/// reported once, because nvcc prints each device diagnostic once per `-gencode`.
/// Diagnostics in files outside `files` are dropped (EDG prints no include chain); a
/// failing exit with no located finding (`nvcc fatal`, an unlocated ptxas error) is an error.
pub fn parse_nvcc_syntax(
    stderr: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    let mut findings: Vec<FileFinding> = Vec::new();
    let mut chain: Vec<(&str, u64)> = Vec::new();
    let mut push = |file: &str, line_no: u64, rule: String, message: String, severity| {
        let candidate = finding(
            TOOL,
            file,
            rule,
            message,
            severity,
            TextPosition {
                line: line_no,
                column: 1,
            },
            ColumnEncoding::Scalars,
        );
        if !findings.contains(&candidate) {
            findings.push(candidate);
        }
    };
    for line in lines(TOOL, stderr)? {
        if is_noise(line) {
            continue;
        }
        if let Some(entry) = include_entry(line) {
            chain.push(entry);
            continue;
        }
        if let Some((file, line_no, severity, rule, message)) = edg_diagnostic(line) {
            chain.clear();
            let Some(severity) = severity else {
                continue;
            };
            let file = file.strip_prefix("./").unwrap_or(file);
            if files.contains(&file) {
                push(file, line_no, rule, message.trim().to_owned(), severity);
            }
            continue;
        }
        let Some((file, line_no, column, severity, message)) = diagnostic(line) else {
            continue;
        };
        let included_from = std::mem::take(&mut chain);
        let Some(severity) = severity else {
            continue;
        };
        let file = file.strip_prefix("./").unwrap_or(file);
        let (file, line_no, prefix) = if files.contains(&file) {
            (file, line_no, String::new())
        } else if let Some((outer, at)) = included_from.iter().find_map(|(path, at)| {
            let path = path.strip_prefix("./").unwrap_or(path);
            files.contains(&path).then_some((path, *at))
        }) {
            (outer, at, format!("{file}:{line_no}:{column}: "))
        } else {
            continue;
        };
        let (message, rule) = match message.rfind(" [") {
            Some(start) if message.ends_with(']') => (
                message[..start].trim_end().to_owned(),
                message[start + 2..message.len() - 1].to_owned(),
            ),
            _ => (message.trim().to_owned(), String::new()),
        };
        push(file, line_no, rule, format!("{prefix}{message}"), severity);
    }
    require_findings(TOOL, &findings, code)?;
    Ok(findings)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILE: &str = "cuda/a.cu";

    fn parse(text: &str, code: i32) -> Result<Vec<FileFinding>, ParseError> {
        parse_nvcc_syntax(text.as_bytes(), Some(code), &[FILE])
    }

    #[test]
    fn reports_an_edg_error_and_a_numbered_warning() {
        let text = "cuda/a.cu(7): error: identifier \"x\" is undefined\n    int y = x;\n            ^\n\ncuda/a.cu(3): warning #177-D: variable \"y\" was declared but never referenced\n\nRemark: The warnings can be suppressed with \"-diag-suppress <warning-number>\"\n\n1 error detected in the compilation of \"cuda/a.cu\".\n";
        let found = parse(text, 1).expect("parsed");
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].finding.message, "identifier \"x\" is undefined");
        assert_eq!(found[0].finding.start.line, 7);
        assert!(matches!(found[0].finding.severity, ToolSeverity::Error));
        assert_eq!(found[1].finding.rule_id, "177-D");
        assert!(matches!(found[1].finding.severity, ToolSeverity::Warning));
    }

    #[test]
    fn duplicates_across_architectures_collapse() {
        let line =
            "cuda/a.cu(3): warning #177-D: variable \"y\" was declared but never referenced\n";
        let found = parse(&line.repeat(3), 0).expect("parsed");
        assert_eq!(found.len(), 1);
    }

    #[test]
    fn host_preprocessor_errors_are_located() {
        let text = "cuda/a.cu:1:10: fatal error: 'missing.h' file not found\n";
        let found = parse(text, 1).expect("parsed");
        assert_eq!(found.len(), 1);
        assert!(matches!(found[0].finding.severity, ToolSeverity::Error));
    }

    #[test]
    fn nvcc_fatal_without_a_finding_fails_closed() {
        assert!(parse("nvcc fatal   : Unknown option '--bogus'\n", 1).is_err());
        assert!(parse("ptxas /tmp/tmpxft_1.ptx, line 5; error   : bad\n", 1).is_err());
        assert!(parse("", 0).expect("clean").is_empty());
    }

    #[test]
    fn a_header_in_the_target_set_is_reported() {
        let text = "cuda/b.cuh(2): error: expected a \";\"\n";
        let found =
            parse_nvcc_syntax(text.as_bytes(), Some(1), &[FILE, "cuda/b.cuh"]).expect("parsed");
        assert_eq!(found[0].file, "cuda/b.cuh");
    }

    #[test]
    fn a_foreign_header_is_dropped_and_fails_closed() {
        let text = "other/c.cuh(2): error: boom\n";
        assert!(parse(text, 1).is_err());
        let warned = parse(text.replace("error", "warning").as_str(), 0).expect("parsed");
        assert!(warned.is_empty());
    }

    #[test]
    fn a_clang_include_chain_maps_to_the_including_unit() {
        let text = "In file included from cuda/a.cu:4:\n./dep/b.h:2:5: error: boom\n";
        let found = parse(text, 1).expect("parsed");
        assert_eq!(found[0].file, FILE);
        assert_eq!(found[0].finding.start.line, 4);
        assert!(found[0].finding.message.starts_with("dep/b.h:2:5: "));
    }

    #[test]
    fn non_utf8_input_is_an_error() {
        assert!(parse_nvcc_syntax(&[0xff], Some(1), &[FILE]).is_err());
    }
}
